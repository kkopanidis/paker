# Phase 2 — Approval Actions

**Scope:** Bulk destructive/mutative operations driven by the bucket index, gated behind a
proposal-and-approval loop.  Three high-level action builders (delete-by-query,
rename-pattern, sync-plan) produce human-readable proposals; every proposal is
HMAC-signed, shown in a review dialog, and only executed after explicit confirmation.
Executed and rejected proposals are written to a local NDJSON audit log.

---

## Table of Contents

1. [Motivation & Guiding Principles](#1-motivation--guiding-principles)
2. [Architecture Overview](#2-architecture-overview)
3. [ProposalStore](#3-proposalstore)
4. [PolicyEngine](#4-policyengine)
5. [HMAC Tokens](#5-hmac-tokens)
6. [Action Builders](#6-action-builders)
   - 6.1 [DeleteByQuery](#61-deletebyquery-builder)
   - 6.2 [RenamePattern](#62-renamepattern-builder)
   - 6.3 [SyncPlan](#63-syncplan-builder)
7. [IPC Commands](#7-ipc-commands)
8. [React Dialogs](#8-react-dialogs)
9. [Audit Log (NDJSON)](#9-audit-log-ndjson)
10. [Security Model](#10-security-model)
11. [Numbered Task List](#11-numbered-task-list)

---

## 1. Motivation & Guiding Principles

Phase 1 gave users the ability to _query_ and _understand_ a full bucket index.  Phase 2
lets them _act_ on those results — but bulk S3 operations are irreversible.  The goals
are:

- **Preview before commit.** Every bulk action produces a proposal (a structured preview
  of exactly what will be mutated) before any S3 call is made.
- **No double-execution.** A proposal token is single-use; re-submitting the same token
  after execution or expiry is rejected by the backend.
- **Auditable.** Every approval and rejection is appended to a local NDJSON file so users
  can reconstruct what happened to a bucket over time.
- **Webview stays untrusted.** The Rust backend re-derives and validates the HMAC token on
  execution; the frontend cannot forge or tamper with the payload.
- **Fit the existing architecture.** New Rust code lives in `src-tauri/src/assistant/`
  (builders) and `src-tauri/src/commands/assistant.rs` (IPC). New React code follows the
  existing dialog pattern in `src/components/browser/`.

---

## 2. Architecture Overview

```
Frontend                         Tauri IPC                   Rust Backend
────────────────────             ──────────────────────      ────────────────────────────────────
BulkActionBuilder UI ──build──▶  assistant_build_proposal ─▶ ActionBuilder::build()
                                                              PolicyEngine::check()
                         ◀──────  ActionProposal (+ token) ◀─ ProposalStore::insert()

ProposalReviewDialog ──approve─▶  assistant_execute_proposal ▶ ProposalStore::claim()
                                                               HmacToken::verify()
                                                               Executor::run()
                                                               AuditLog::write(Executed)

                     ──reject──▶  assistant_reject_proposal ─▶ ProposalStore::reject()
                                                               AuditLog::write(Rejected)
```

**Key invariants:**

1. `assistant_build_proposal` never mutates S3.
2. `assistant_execute_proposal` requires a valid, unexpired, unclaimed HMAC token.
3. Once claimed (executed or rejected), a token can never be reused.

---

## 3. ProposalStore

**File:** `src-tauri/src/assistant/proposal_store.rs`

The ProposalStore is an in-memory registry of live proposals, wrapped in a
`parking_lot::Mutex`.  It is registered as a Tauri managed state in `lib.rs`.

### Data Structures

```rust
/// Status of a proposal in the store.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Pending,
    Executed,
    Rejected,
}

/// A single entry in the store.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalEntry {
    pub id: String,               // UUIDv4
    pub kind: ActionKind,         // DeleteByQuery | RenamePattern | SyncPlan
    pub connection_id: String,
    pub bucket: String,
    pub payload: serde_json::Value,  // serialised ActionProposal payload
    pub token: String,            // HMAC-signed token (opaque to frontend)
    pub status: ProposalStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
```

### ProposalStore API

```rust
pub struct ProposalStore {
    inner: parking_lot::Mutex<HashMap<String, ProposalEntry>>,
}

impl ProposalStore {
    /// Insert a freshly built proposal; returns the entry (includes token).
    pub fn insert(&self, entry: ProposalEntry) -> ProposalEntry;

    /// Attempt to claim a pending, non-expired proposal.
    /// Returns Err if not found, already claimed, or expired.
    pub fn claim(&self, id: &str, token: &str) -> Result<ProposalEntry, PakerError>;

    /// Mark a proposal as rejected (called from reject IPC).
    pub fn reject(&self, id: &str) -> Result<ProposalEntry, PakerError>;

    /// Evict proposals older than `expires_at`.  Called lazily on insert.
    fn evict_expired(&self);
}
```

**Expiry:** proposals expire after **15 minutes** by default; the constant
`PROPOSAL_TTL_SECS: u64 = 900` lives in the same file.

**Capacity:** `evict_expired` runs on every `insert`; no explicit cap needed (proposals
are typically one at a time per connection).

**State registration in `lib.rs`:**

```rust
.manage(ProposalStore::default())
```

---

## 4. PolicyEngine

**File:** `src-tauri/src/assistant/policy.rs`

The PolicyEngine determines whether a proposed action is subject to an extra safety
gate before the builder is allowed to run.  Rules are evaluated purely in Rust; the
frontend has no influence over them.

### Policy Rules

| Rule | Condition | Effect |
|------|-----------|--------|
| `MaxObjectsHardLimit` | `affected_count > 10_000` | Reject with `PolicyViolation::TooManyObjects` |
| `DestructiveRequiresIndex` | delete/rename on a stale or absent index | Reject with `PolicyViolation::StaleIndex` |
| `GlacierWarning` | any affected object has `GLACIER` or `DEEP_ARCHIVE` storage class | Warn (proposal still allowed; dialog shows badge) |
| `VersionedBucketDelete` | bucket has versioning enabled and action is delete | Warn (versions not deleted, only current objects) |
| `DryRunOnly` | `PAKER_POLICY_DRY_RUN=1` env var set | Reject all mutative actions in integration-test environment |

### Types

```rust
#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "rule")]
pub enum PolicyViolation {
    #[error("proposal would affect {count} objects; limit is {limit}")]
    TooManyObjects { count: usize, limit: usize },
    #[error("bucket index is stale or absent")]
    StaleIndex,
    #[error("dry-run mode active; no mutations allowed")]
    DryRunOnly,
}

pub struct PolicyWarning {
    pub rule: &'static str,
    pub message: String,
}

pub struct PolicyResult {
    pub violations: Vec<PolicyViolation>,
    pub warnings: Vec<PolicyWarning>,
}

pub fn check(ctx: &PolicyContext) -> PolicyResult;
```

### PolicyContext

```rust
pub struct PolicyContext<'a> {
    pub kind: ActionKind,
    pub affected_count: usize,
    pub has_glacier: bool,
    pub bucket_versioned: bool,
    pub index_age_secs: Option<u64>,
}
```

The builder calls `policy::check` after computing the affected object list but before
building the full proposal.  Hard violations abort the build; warnings are attached to
the proposal as `warnings: Vec<String>` and surfaced in the dialog.

---

## 5. HMAC Tokens

**File:** `src-tauri/src/assistant/hmac_token.rs`

HMAC tokens bind a proposal to its exact payload so the backend can re-verify integrity
at execution time without trusting the frontend.

### Design

- **Algorithm:** HMAC-SHA256 (via the `hmac` + `sha2` crates — add to `Cargo.toml`).
- **Key:** A 32-byte random session key generated once at app start and stored in
  `parking_lot::RwLock<[u8; 32]>` as a new managed state `HmacKey`.
- **Message:** `{proposal_id}:{connection_id}:{bucket}:{kind}:{created_at_unix}` — all
  fields that must not change between build and execute.
- **Format:** `v1.{proposal_id}.{base64url(hmac_bytes)}` — the `v1` prefix allows
  future algorithm migration.
- **Expiry:** The `created_at_unix` is embedded in the message; the verifier also checks
  `now - created_at < PROPOSAL_TTL_SECS`.

### API

```rust
pub struct HmacKey(pub [u8; 32]);

impl HmacKey {
    /// Derive a fresh random key.
    pub fn generate() -> Self;
}

/// Produce a token string for a proposal.
pub fn sign(key: &HmacKey, proposal_id: &str, connection_id: &str,
            bucket: &str, kind: &str, created_at: i64) -> String;

/// Verify a token.  Returns Err if signature invalid, expired, or malformed.
pub fn verify(key: &HmacKey, token: &str, proposal_id: &str, connection_id: &str,
              bucket: &str, kind: &str, created_at: i64) -> Result<(), PakerError>;
```

**State registration in `lib.rs`:**

```rust
.manage(HmacKey::generate())
```

**New Cargo dependencies:**

```toml
hmac = "0.12"
sha2 = "0.10"
```

---

## 6. Action Builders

All builders live under `src-tauri/src/assistant/builders/` with a shared module at
`src-tauri/src/assistant/builders/mod.rs`.

### Shared Types

```rust
/// Discriminant used in tokens and audit entries.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind {
    DeleteByQuery,
    RenamePattern,
    SyncPlan,
}

/// A single item in any proposal's preview list.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalItem {
    pub key: String,
    pub size_bytes: u64,
    pub storage_class: Option<String>,
    /// Human-readable description of what will happen to this item.
    pub action_description: String,
}

/// Top-level proposal returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionProposal {
    pub id: String,
    pub kind: ActionKind,
    pub connection_id: String,
    pub bucket: String,
    /// Subset of items shown in the preview (capped at MAX_PREVIEW_ITEMS = 200).
    pub preview_items: Vec<ProposalItem>,
    pub total_affected: usize,
    pub total_bytes: u64,
    pub warnings: Vec<String>,
    /// Opaque HMAC token to pass back to execute.
    pub token: String,
    pub expires_at: String,  // ISO-8601
}
```

---

### 6.1 DeleteByQuery Builder

**File:** `src-tauri/src/assistant/builders/delete_by_query.rs`

Builds a deletion proposal for all objects matching an `IndexQuery` in the local index.
Execution calls the existing `s3::operations::delete_objects_batch` internally.

#### Input

```rust
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteByQueryInput {
    pub connection_id: String,
    pub bucket: String,
    pub query: crate::assistant::query::IndexQuery,
    /// If true, only produce the proposal; never execute (overrides user approve).
    pub dry_run: bool,
}
```

#### Build Flow

1. Call `storage::bucket_index::query_bucket_index(conn, bucket, &input.query)` to get
   matching `IndexedObject` list.
2. Run `policy::check` with `kind=DeleteByQuery`, `affected_count`, `has_glacier`,
   `bucket_versioned` (fetched from `get_bucket_metadata`).
3. Hard violations → return `Err(PakerError::PolicyViolation { .. })`.
4. Cap preview at `MAX_PREVIEW_ITEMS = 200`; build `ProposalItem` list with
   `action_description = format!("Delete s3://{}/{}", bucket, key)`.
5. Sign with `hmac_token::sign(…)`.
6. Insert into `ProposalStore`; return `ActionProposal`.

#### Execution

Called from `assistant_execute_proposal` IPC (see §7):

1. Claim from `ProposalStore` (verifies token + expiry).
2. Re-run `query_bucket_index` to get the live key list (index may have been refreshed
   since build — only use keys present in **both** the stored payload and the live query).
3. Call `s3::operations::delete_objects` in batches of 1000 (S3 SDK limit).
4. Emit `proposal://progress` Tauri events with `{ done, total }`.
5. Write `AuditEntry` on completion.

---

### 6.2 RenamePattern Builder

**File:** `src-tauri/src/assistant/builders/rename_pattern.rs`

Generates copy-then-delete operations that rename objects matching a source glob pattern
to a destination pattern.  Uses the existing `copy_objects` + `delete_objects` S3 ops.

#### Input

```rust
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePatternInput {
    pub connection_id: String,
    pub bucket: String,
    /// Glob pattern for source keys, e.g. `"logs/2024-*/*.log.gz"`
    pub source_pattern: String,
    /// Replacement template.  Capture groups from glob: `{0}` = full match,
    /// named groups from regex syntax `(?P<name>...)` → `{name}`.
    /// Example: `"archive/2024/{1}/{2}.gz"` moves year/month segments.
    pub dest_template: String,
    /// If true, copy only (no delete of source).
    pub copy_only: bool,
    pub query: Option<crate::assistant::query::IndexQuery>,
}
```

#### Build Flow

1. Resolve source objects: if `input.query` is set, call `query_bucket_index`; otherwise
   list all objects from the index with key matching `source_pattern` (converted to
   SQL `LIKE` or Rust regex, see §6.2.1).
2. For each source key: apply `dest_template` substitution → derive `dest_key`.
3. Validate that `dest_key != source_key` for all items.
4. Policy check (kind=RenamePattern).
5. Build `ProposalItem` with `action_description`:
   - Copy-only: `"Copy → s3://{bucket}/{dest_key}"`
   - Rename: `"Rename: {src_key} → {dest_key}"`
6. Sign, store, return `ActionProposal` with both `src_key` and `dest_key` encoded in
   the preview items (dest stored in a `metadata` field added to `ProposalItem`).

#### 6.2.1 Pattern Matching

Use the `glob` crate (add `glob = "0.3"` to `Cargo.toml`) for matching source keys.
Template substitution uses a simple `{n}` positional syntax: split the glob pattern at
`*` wildcards; each `*` becomes capture group `{1}`, `{2}`, etc.  No regex engine
needed in the Rust path; implement `substitute_template(captures: &[&str], template: &str) -> String`.

#### Execution

1. Claim from `ProposalStore`.
2. For each `(src_key, dest_key)` pair: call `s3::operations::copy_object`.
3. If not `copy_only`: call `s3::operations::delete_objects` on all successfully copied
   sources only.
4. Emit `proposal://progress` events.
5. Write `AuditEntry`.

---

### 6.3 SyncPlan Builder

**File:** `src-tauri/src/assistant/builders/sync_plan.rs`

Computes what a one-way sync from a source prefix to a destination prefix would change
(adds, updates, deletes), using the local bucket index as the source of truth.  It does
**not** execute S3 operations itself — it outputs a preview plus optional rclone/aws-cli
commands.  This builder is read-only by nature; it still goes through the
proposal-and-token flow so the audit log records that the plan was generated.

#### Input

```rust
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlanInput {
    pub connection_id: String,
    pub bucket: String,
    pub source_prefix: String,
    pub dest_prefix: String,
    /// "add_only" | "mirror" (default "mirror").
    /// mirror = also delete objects in dest not in source.
    pub mode: SyncMode,
    pub generate_cli: bool,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncMode { AddOnly, Mirror }
```

#### Build Flow

1. Query index for `source_prefix` and `dest_prefix` objects.
2. Compute diff sets:
   - `to_add`: keys in source not in dest (by relative key after stripping prefix).
   - `to_update`: keys in both but source `last_modified` is newer.
   - `to_delete`: keys in dest not in source (only when `mode=Mirror`).
3. Policy check (kind=SyncPlan; violations only for `TooManyObjects`).
4. Build `ProposalItem` list with action descriptions: `"Add"`, `"Update"`, `"Delete"`.
5. Optionally call `assistant::templates::generate_cli_commands` to produce rclone/aws
   command suggestions; attach to proposal as `cli_suggestions: Vec<CliCommandSuggestion>`.
6. Sign (even though this plan is read-only, we token-gate it so the audit log captures
   it). `dry_run=true` is set implicitly; the execute IPC for SyncPlan performs no S3
   mutation — it only finalises the audit entry.
7. Return `ActionProposal` plus `cli_suggestions` in a `SyncPlanProposal` wrapper.

---

## 7. IPC Commands

**File:** `src-tauri/src/commands/assistant.rs` (extend existing file)

All new commands follow the existing `#[tauri::command]` pattern.  They are added to the
`app_commands!` macro in `src-tauri/src/commands/mod.rs`.

### New Commands

```rust
/// Build a proposal for a bulk action.
/// Returns ActionProposal (JSON) on success, or a structured PakerError.
#[tauri::command]
pub async fn assistant_build_proposal(
    input: BuildProposalInput,
    cache: tauri::State<'_, ObjectCacheManager>,
    proposal_store: tauri::State<'_, ProposalStore>,
    hmac_key: tauri::State<'_, HmacKey>,
) -> Result<ActionProposal, PakerError>;

/// Execute a previously built proposal (approve).
#[tauri::command]
pub async fn assistant_execute_proposal(
    proposal_id: String,
    token: String,
    proposal_store: tauri::State<'_, ProposalStore>,
    hmac_key: tauri::State<'_, HmacKey>,
    cache: tauri::State<'_, ObjectCacheManager>,
    app_handle: tauri::AppHandle,
) -> Result<ExecutionResult, PakerError>;

/// Reject a proposal (cancel without executing).
#[tauri::command]
pub async fn assistant_reject_proposal(
    proposal_id: String,
    token: String,
    proposal_store: tauri::State<'_, ProposalStore>,
    hmac_key: tauri::State<'_, HmacKey>,
    app_handle: tauri::AppHandle,
) -> Result<(), PakerError>;

/// List recent proposals (pending + last 50 completed) for the audit view.
#[tauri::command]
pub async fn assistant_list_proposals(
    connection_id: Option<String>,
    proposal_store: tauri::State<'_, ProposalStore>,
) -> Result<Vec<ProposalEntry>, PakerError>;
```

### BuildProposalInput (discriminated union)

```rust
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BuildProposalInput {
    DeleteByQuery(DeleteByQueryInput),
    RenamePattern(RenamePatternInput),
    SyncPlan(SyncPlanInput),
}
```

### ExecutionResult

```rust
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResult {
    pub proposal_id: String,
    pub kind: ActionKind,
    pub objects_affected: usize,
    pub bytes_affected: u64,
    pub errors: Vec<PartialError>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialError {
    pub key: String,
    pub message: String,
}
```

### Progress Events

During execution, the backend emits:

```
event name : "proposal://progress"
payload    : { "proposalId": string, "done": number, "total": number, "phase": string }
```

phases: `"copying"`, `"deleting"`, `"complete"`, `"error"`

### `mod.rs` additions

```rust
$crate::commands::assistant::assistant_build_proposal,
$crate::commands::assistant::assistant_execute_proposal,
$crate::commands::assistant::assistant_reject_proposal,
$crate::commands::assistant::assistant_list_proposals,
```

### `src/lib/tauri.ts` additions

```typescript
export async function assistantBuildProposal(
  input: BuildProposalInput
): Promise<ActionProposal>

export async function assistantExecuteProposal(
  proposalId: string,
  token: string
): Promise<ExecutionResult>

export async function assistantRejectProposal(
  proposalId: string,
  token: string
): Promise<void>

export async function assistantListProposals(
  connectionId?: string
): Promise<ProposalEntry[]>
```

---

## 8. React Dialogs

All dialogs live under `src/components/browser/` following the existing naming convention.
They use shadcn/ui primitives already in the project (`Dialog`, `Button`, `Badge`,
`ScrollArea`, `Progress`, `Table`).

### 8.1 BulkActionBuilderDialog

**File:** `src/components/browser/BulkActionBuilderDialog.tsx`

Entry point for initiating any of the three builders.  Opened from `BrowserToolbar` via
a "Bulk Actions" button (visible only when a bucket index is present).

**Props:**
```typescript
interface Props {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  connectionId: string;
  bucket: string;
}
```

**Layout:**
- Tab strip: "Delete by Query" | "Rename Pattern" | "Sync Plan"
- Each tab renders a sub-form (see below).
- "Preview" button → calls `assistantBuildProposal` → opens `ProposalReviewDialog`.
- Loading spinner during build; error banner on `PolicyViolation`.

**Sub-form: Delete by Query**
- Reuses `BucketIndexSearchDialog`'s query builder controls (key pattern, prefix, size
  range, date range, storage class multi-select).
- "Dry run" checkbox (always checked by default).

**Sub-form: Rename Pattern**
- Source pattern input (`string`, placeholder `logs/2024-*/*.gz`).
- Destination template input (placeholder `archive/{1}/{2}.gz`).
- Live substitution preview for the first 3 matched keys (fetched from the local index
  via `assistantRunIndexQuery`).
- "Copy only" checkbox.

**Sub-form: Sync Plan**
- Source prefix input.
- Destination prefix input.
- Mode radio: "Add only" / "Mirror".
- "Generate CLI commands" checkbox.

---

### 8.2 ProposalReviewDialog

**File:** `src/components/browser/ProposalReviewDialog.tsx`

Shown after a proposal is successfully built.  Presents a full review before the user
approves or rejects.

**Props:**
```typescript
interface Props {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  proposal: ActionProposal;
  onExecuted: (result: ExecutionResult) => void;
  onRejected: () => void;
}
```

**Layout sections:**

1. **Summary header** — action kind badge, bucket name, `total_affected` objects,
   `total_bytes` human-readable, expiry countdown (`expires_at`).
2. **Warning banners** — each `warning` from `proposal.warnings` shown as an amber
   `Alert` component.
3. **Preview table** — `ScrollArea` containing a `Table` with columns: Key, Size, Storage
   Class, Action.  Shows up to 200 rows with a note "…and N more objects" if
   `total_affected > 200`.
4. **CLI suggestions panel** (SyncPlan only) — collapsible code blocks for each
   `CliCommandSuggestion`.
5. **Footer** — "Reject" (secondary) + "Approve & Execute" (destructive variant, red).
   Approve button disabled until user types the bucket name in a confirmation input
   (pattern: `"Type the bucket name to confirm: {bucket}"`).

**Execution flow inside the dialog:**
- On approve: call `assistantExecuteProposal(proposal.id, proposal.token)`.
- Subscribe to `proposal://progress` events to drive an indeterminate → determinate
  `Progress` bar.
- On success: show `ExecutionResult` summary; call `onExecuted`.
- On error: show error inline; allow retry or reject.
- On reject: call `assistantRejectProposal(proposal.id, proposal.token)`; call
  `onRejected`.

---

### 8.3 ProposalAuditDialog

**File:** `src/components/browser/ProposalAuditDialog.tsx`

Read-only log viewer.  Opened from `BrowserToolbar` ("Action History" item in toolbar
overflow menu).

**Props:**
```typescript
interface Props {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  connectionId: string;
}
```

**Layout:**
- Fetches `assistantListProposals(connectionId)` on open.
- Sorted by `created_at` descending.
- Table: Date, Kind, Status badge (Pending/Executed/Rejected), Objects Affected,
  "View details" expand row.

---

### 8.4 Type Additions — `src/types/assistant.ts`

```typescript
export type ActionKind = "deleteByQuery" | "renamePattern" | "syncPlan";

export type ProposalStatus = "pending" | "executed" | "rejected";

export interface ProposalItem {
  key: string;
  sizeBytes: number;
  storageClass?: string;
  actionDescription: string;
  metadata?: Record<string, string>;
}

export interface ActionProposal {
  id: string;
  kind: ActionKind;
  connectionId: string;
  bucket: string;
  previewItems: ProposalItem[];
  totalAffected: number;
  totalBytes: number;
  warnings: string[];
  token: string;
  expiresAt: string;
  cliSuggestions?: CliCommandSuggestion[];  // SyncPlan only
}

export interface ExecutionResult {
  proposalId: string;
  kind: ActionKind;
  objectsAffected: number;
  bytesAffected: number;
  errors: PartialError[];
}

export interface PartialError {
  key: string;
  message: string;
}

export interface ProposalEntry {
  id: string;
  kind: ActionKind;
  connectionId: string;
  bucket: string;
  status: ProposalStatus;
  createdAt: string;
  expiresAt: string;
}

export type BuildProposalInput =
  | ({ kind: "deleteByQuery" } & DeleteByQueryInput)
  | ({ kind: "renamePattern" } & RenamePatternInput)
  | ({ kind: "syncPlan" } & SyncPlanInput);

export interface DeleteByQueryInput {
  connectionId: string;
  bucket: string;
  query: IndexQuery;
  dryRun: boolean;
}

export interface RenamePatternInput {
  connectionId: string;
  bucket: string;
  sourcePattern: string;
  destTemplate: string;
  copyOnly: boolean;
  query?: IndexQuery;
}

export interface SyncPlanInput {
  connectionId: string;
  bucket: string;
  sourcePrefix: string;
  destPrefix: string;
  mode: "addOnly" | "mirror";
  generateCli: boolean;
}
```

---

## 9. Audit Log (NDJSON)

**File:** `src-tauri/src/assistant/audit_log.rs`

Each approved or rejected proposal appends a JSON line to a local file.

### File Location

```
<app_data_dir>/paker-audit.ndjson
```

In portable mode (`storage::paths::is_portable_mode()`):
```
<exe_dir>/data/paker-audit.ndjson
```

Use `storage::paths::app_data_dir(app_handle)` (already in the project) to resolve the
path at runtime.

### Entry Schema

```rust
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    /// ISO-8601 UTC timestamp.
    pub ts: String,
    pub proposal_id: String,
    pub kind: ActionKind,
    pub outcome: AuditOutcome,
    pub connection_id: String,
    pub bucket: String,
    pub objects_affected: usize,
    pub bytes_affected: u64,
    /// Partial errors that did not prevent overall execution.
    pub errors: Vec<PartialError>,
    /// App version for forward/backward compat.
    pub app_version: &'static str,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditOutcome {
    Executed,
    Rejected,
    ExpiredAbandoned,
}
```

### AuditLog API

```rust
pub struct AuditLog {
    path: std::path::PathBuf,
}

impl AuditLog {
    pub fn new(app_handle: &tauri::AppHandle) -> Result<Self, PakerError>;

    /// Append one JSON line atomically (open → write → flush → close).
    pub fn append(&self, entry: &AuditEntry) -> Result<(), PakerError>;
}
```

`AuditLog` is registered as managed state in `lib.rs`:

```rust
.manage(AuditLog::new(&app_handle)?)
```

The write is non-async (fast file append); wrap in `tokio::task::spawn_blocking` inside
the IPC command if needed to avoid blocking the async runtime.

### Rotation / Size Policy

NDJSON files are append-only.  When `append` is called, if the file size exceeds
**50 MB**, the existing file is renamed to `paker-audit.{unix_ts}.ndjson.bak` and a
fresh `paker-audit.ndjson` is started.  Implement as `rotate_if_needed` called before
each append.

---

## 10. Security Model

### Trust Boundary Recap

The Tauri webview is untrusted.  The backend enforces all invariants; the frontend is
display-only.

### HMAC Token Guarantees

| Threat | Mitigation |
|--------|-----------|
| Frontend forges a proposal for keys it didn't receive | HMAC key is never sent to the frontend; token is opaque |
| Frontend replays an expired token | `created_at` is bound into the HMAC message; verifier rejects if `now > created_at + TTL` |
| Frontend executes the same proposal twice | `ProposalStore::claim` transitions status to `Executed`/`Rejected`; subsequent calls return `Err(PakerError::ProposalAlreadyClaimed)` |
| Frontend tampers with the payload (e.g. changes which keys are deleted) | Execution re-queries the index server-side; the frontend payload is only used for display |
| Session key leaked via IPC | `HmacKey` is Tauri managed state; it is never serialised or returned through any IPC command |

### Capability Constraints

No new capabilities are required in `src-tauri/capabilities/default.json`.  All S3
mutations go through existing `s3::operations` calls, which are already privileged
Rust code unreachable from the webview except via the registered IPC surface.

### Vault Interaction

If the vault is enabled and locked, `assistant_execute_proposal` should call
`vault::require_unlocked(vault_state)` (pattern already used in `commands/vault.rs`)
before executing any mutation.  Proposals may be _built_ while the vault is locked
(the index is already loaded), but _execution_ requires an unlocked credential store
because the S3 credentials must be retrieved.

### Input Validation

- `source_pattern` and `dest_template` in `RenamePatternInput` are validated against a
  maximum length of 1024 chars and must not contain null bytes.
- All prefix/key inputs are passed through `path_safety::validate_s3_key` before use.
- `IndexQuery::limit` is capped server-side at 10,000 in `query_bucket_index`.

---

## 11. Numbered Task List

Tasks are ordered by dependency.  Each task maps to a specific file or small set of
files.

### Rust Backend

1. **Add Cargo dependencies**
   - Add `hmac = "0.12"`, `sha2 = "0.10"`, `glob = "0.3"` to `src-tauri/Cargo.toml`.

2. **Create `src-tauri/src/assistant/builders/mod.rs`**
   - Define `ActionKind`, `ProposalItem`, `ActionProposal`, `BuildProposalInput` (discriminated union), `ExecutionResult`, `PartialError`.
   - Re-export sub-builder modules.

3. **Create `src-tauri/src/assistant/hmac_token.rs`**
   - Implement `HmacKey`, `sign()`, `verify()`.
   - Unit tests: sign → verify round-trip; tampered message returns `Err`; expired message returns `Err`.

4. **Create `src-tauri/src/assistant/policy.rs`**
   - Implement `PolicyViolation`, `PolicyWarning`, `PolicyResult`, `PolicyContext`, `check()`.
   - Unit tests: max-objects rule fires at 10_001; glacier warning attached correctly.

5. **Create `src-tauri/src/assistant/proposal_store.rs`**
   - Implement `ProposalEntry`, `ProposalStatus`, `ProposalStore` with `insert`, `claim`, `reject`, `evict_expired`.
   - Unit tests: insert + claim succeeds; double-claim returns `Err`; expired entry cannot be claimed.

6. **Create `src-tauri/src/assistant/audit_log.rs`**
   - Implement `AuditEntry`, `AuditOutcome`, `AuditLog` with `new`, `append`, `rotate_if_needed`.
   - Unit test: write 3 entries, read file, assert 3 lines of valid JSON; rotation when > 50 MB.

7. **Create `src-tauri/src/assistant/builders/delete_by_query.rs`**
   - Implement `DeleteByQueryInput`, `build()` (returns `ActionProposal`), `execute()`.
   - Unit tests with a temp SQLite index: build proposal for 5 matching objects; verify preview items and token present.

8. **Create `src-tauri/src/assistant/builders/rename_pattern.rs`**
   - Implement `RenamePatternInput`, `substitute_template()`, `build()`, `execute()`.
   - Unit tests: pattern `logs/*/*.gz` with dest `archive/{1}/{2}.gz`; verify substitution; copy-only flag skips delete step.

9. **Create `src-tauri/src/assistant/builders/sync_plan.rs`**
   - Implement `SyncPlanInput`, `SyncMode`, `build()` (read-only; no `execute()`).
   - Unit test: source has 5 keys, dest has 3 overlapping + 1 extra; verify `to_add`, `to_update`, `to_delete` counts.

10. **Update `src-tauri/src/assistant/mod.rs`**
    - Re-export `builders`, `hmac_token`, `policy`, `proposal_store`, `audit_log`.

11. **Update `src-tauri/src/lib.rs`**
    - Add `.manage(HmacKey::generate())`.
    - Add `.manage(ProposalStore::default())`.
    - Add `.manage(AuditLog::new(&app_handle)?)` in the `setup` closure (propagate error).

12. **Extend `src-tauri/src/commands/assistant.rs`**
    - Implement `assistant_build_proposal`, `assistant_execute_proposal`,
      `assistant_reject_proposal`, `assistant_list_proposals`.
    - Wire vault `require_unlocked` check in `assistant_execute_proposal`.
    - Emit `proposal://progress` events during execution loops.

13. **Update `src-tauri/src/commands/mod.rs`**
    - Add four new commands to the `app_commands!` macro.

### Frontend

14. **Update `src/types/assistant.ts`**
    - Add all new types from §8.4 (ActionKind, ProposalStatus, ProposalItem,
      ActionProposal, ExecutionResult, PartialError, ProposalEntry, BuildProposalInput,
      DeleteByQueryInput, RenamePatternInput, SyncPlanInput).

15. **Update `src/lib/tauri.ts`**
    - Add `assistantBuildProposal`, `assistantExecuteProposal`, `assistantRejectProposal`,
      `assistantListProposals` wrappers following the existing `invokeSafe` pattern.

16. **Create `src/components/browser/ProposalReviewDialog.tsx`**
    - Summary header, warning banners, preview table (ScrollArea), CLI suggestions panel,
      footer with bucket-name confirmation input, execute/reject buttons.
    - Subscribe to `proposal://progress` events for progress bar.

17. **Create `src/components/browser/BulkActionBuilderDialog.tsx`**
    - Tab strip for three builders.
    - Delete-by-query sub-form (reuse query controls from BucketIndexSearchDialog).
    - Rename-pattern sub-form with live substitution preview.
    - Sync-plan sub-form.
    - "Preview" button → builds proposal → opens ProposalReviewDialog.

18. **Create `src/components/browser/ProposalAuditDialog.tsx`**
    - Fetches `assistantListProposals` on open.
    - Sorted table with expand-row details.

19. **Update `src/components/browser/BrowserToolbar.tsx`**
    - Add "Bulk Actions" button (shown when `bucketIndexReady`).
    - Add "Action History" overflow menu item.
    - Wire open state to `BulkActionBuilderDialog` and `ProposalAuditDialog`.

20. **Update `src/components/layout/AppShell.tsx`** (if toolbar wiring lives here)
    - Pass `bucketIndexReady` flag down to `BrowserToolbar`.

### Polish & Verification

21. **Integration test for execute flow** (`src-tauri/tests/s3_integration.rs`)
    - Feature-gated under `integration-tests`.
    - Build a delete-by-query proposal against a MinIO bucket; approve; verify objects gone.

22. **Vitest tests for new IPC wrappers** (`src/lib/tauri.test.ts`)
    - Mock `invoke` for `assistantBuildProposal`, `assistantExecuteProposal`.
    - Assert correct command names and argument shapes.

23. **Vitest tests for ProposalReviewDialog** (unit)
    - Approve button disabled until bucket name typed; enabled after.
    - Warnings rendered for each `proposal.warnings` entry.

24. **Update `docs/architecture.md`**
    - Add Phase 2 section: ProposalStore, HmacKey, AuditLog as managed state.
    - Update command count.

---

*End of Phase 2 Plan*
