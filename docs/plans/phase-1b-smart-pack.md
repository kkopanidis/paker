# Phase 1b — Smart Pack

**Project:** Paker S3 Desktop Browser (Tauri 2 + Rust + React/TypeScript)  
**Status:** Planning  
**Depends on:** Phase 1a (rule_parser, IndexQuery, BucketIndexSearchDialog, BucketReportDialog — all shipped)  
**Goal:** Upgrade the assistant from a basic regex-to-SQL bridge into a first-class "Smart Pack" experience: GBNF-constrained local LLM parsing, persistent query history, a unified assistant drawer panel, and one-click "pack actions" (select-all results, copy as CLI, export).

---

## 1. Scope

### In scope
- **GBNF grammar + llama.cpp integration** — optional on-device LLM fallback that produces structured JSON from freeform queries, bypassing the regex parser for high-confidence cases.  
- **Query history** — SQLite table; last 50 queries per `(connection_id, bucket)`.  
- **Smart Pack drawer** — unified side-panel: NL search, result list, report summary, history, CLI generation, pack actions (select-all, export-as-CSV, copy CLI).  
- **`assistant_parse_query_llm`** IPC command (falls back gracefully to regex parser when no model is loaded).  
- **`assistant_query_history_*`** IPC commands (list, clear).  
- **`assistant_pack_export`** IPC command — exports current result set to a local file or clipboard JSON.  
- TypeScript types + `tauri.ts` wrappers for every new command.  
- Unit tests for GBNF grammar roundtrips (without loading an actual model).

### Out of scope / deferred
- Streaming token-by-token LLM output over IPC (deferred to Phase 2).  
- Multi-step conversational memory / follow-up queries.  
- Remote LLM API (OpenAI, Anthropic) — privacy goal, on-device only.  
- Automatic model download UI (user drops model file into app data folder manually in this phase).  
- Result set diffing between queries ("what changed since last week?").

---

## 2. Architecture overview

```
┌─────────────────────────────────────────┐
│  SmartPackDrawer (React)                │
│  ┌────────────┐  ┌────────────────────┐ │
│  │ NL Query   │  │ Result list +      │ │
│  │ input +    │  │ pack actions       │ │
│  │ history    │  │ (select/export/CLI)│ │
│  └────────────┘  └────────────────────┘ │
│  ┌──────────────────────────────────────┤ │
│  │ BucketReport summary (inlined)      │ │
│  └──────────────────────────────────────┤ │
└─────────────────────────────────────────┘
          │  IPC (Tauri invoke)
          ▼
┌──────────────────────────────────────────┐
│  commands/assistant.rs                   │
│  assistant_parse_query_llm()             │
│  assistant_query_history_list()          │
│  assistant_query_history_clear()         │
│  assistant_pack_export()                 │
└──────────────┬───────────────────────────┘
               │
       ┌───────┴──────────┐
       ▼                  ▼
assistant/llm/         assistant/query/
  gbnf_grammar.rs        rule_parser.rs   ← unchanged (fallback)
  model_runner.rs
  parse_result.rs
               │
               ▼
      storage/bucket_index.rs  (unchanged)
      storage/assistant_history.rs  (new)
               │
               ▼
         SQLite  (existing DB, new tables)
```

### Parsing chain

```
user text
    │
    ├─► rule_parser::parse_natural_language()   [always runs, O(μs)]
    │       │
    │       └─ ParsedAssistantQuery { confidence: High|Medium|Low }
    │
    └─► (if LLM loaded AND confidence != High)
            │
            ▼
        llm::model_runner::run_grammar_parse(text, GBNF_SCHEMA)
            │
            └─ Ok(LlmParsedQuery) → merged/overrides regex result
               Err(_) → regex result used as-is
```

---

## 3. Cargo dependencies

Add to `src-tauri/Cargo.toml`:

```toml
# GBNF / llama.cpp binding — optional, no GPU required for 1–3B quant models
llama-cpp-2 = { version = "0.1", optional = true, features = ["cuda"] }

# Used by LLM parse cache and history
serde_json = "1"        # already present
```

New feature flag so CI/cross-compile still works without llama-cpp:

```toml
[features]
default = []
llm = ["llama-cpp-2"]
integration-tests = []
```

> **Note:** `llama-cpp-2` wraps the upstream llama.cpp C++ library. The `cuda` feature is
> conditional and ignored on macOS/Windows without CUDA.  On Apple Silicon the Metal backend
> activates automatically without extra features.

---

## 4. SQLite schema changes

Two new tables appended to the existing migration that creates `bucket_index_objects` and
`bucket_index_meta`.  Add them in a new migration block inside
`storage/object_cache.rs` (or wherever `CREATE TABLE` statements currently live — check
`storage/index/` if the schema is split there).

```sql
-- Query history (capped by Rust code at 50 rows per composite key)
CREATE TABLE IF NOT EXISTS assistant_query_history (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT    NOT NULL,
    bucket        TEXT    NOT NULL,
    raw_text      TEXT    NOT NULL,
    summary       TEXT    NOT NULL,
    confidence    TEXT    NOT NULL CHECK(confidence IN ('high','medium','low')),
    result_count  INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_aqh_conn_bucket
    ON assistant_query_history(connection_id, bucket, id DESC);

-- Pack export log (optional, for "open last export" UX)
CREATE TABLE IF NOT EXISTS assistant_pack_exports (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id TEXT NOT NULL,
    bucket        TEXT NOT NULL,
    export_path   TEXT,          -- NULL = clipboard
    object_count  INTEGER NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

**Migration guard:** use `IF NOT EXISTS` so existing databases are upgraded automatically
on next launch without wiping data.

---

## 5. New Rust source files

### 5.1 `src-tauri/src/assistant/llm/mod.rs`

```
assistant/llm/
    mod.rs          — public surface (re-exports, feature-gate stub)
    gbnf_grammar.rs — GBNF string constant + unit tests
    model_runner.rs — load/unload model, run inference with grammar, cache handle
    parse_result.rs — LlmParsedQuery struct, merge_with_regex()
```

#### `gbnf_grammar.rs` outline

The GBNF grammar constrains LLM output to a strict JSON schema that maps 1-to-1 with
`IndexQuery`.  This means the model cannot produce free-form text; every token is chosen
from a valid-continuation set.

```
root       ::= "{" ws members ws "}"
members    ::= pair (ws "," ws pair)*
pair       ::= key ws ":" ws value
key        ::= "\"keyPattern\""
             | "\"prefix\""
             | "\"minSize\""
             | "\"maxSize\""
             | "\"modifiedAfter\""
             | "\"modifiedBefore\""
             | "\"storageClass\""

value      ::= string | number | null | string-array
string     ::= "\"" [^"]* "\""
number     ::= [0-9]+
null       ::= "null"
string-array ::= "[" ws "]"
               | "[" ws string (ws "," ws string)* ws "]"
ws         ::= [ \t\n]*
```

Storage class values are further constrained by a `storage-class-value` rule that only
accepts the literal strings `"STANDARD"`, `"STANDARD_IA"`, `"ONEZONE_IA"`,
`"INTELLIGENT_TIERING"`, `"GLACIER"`, `"GLACIER_IR"`, `"DEEP_ARCHIVE"`.

Full GBNF is stored as a `const &str` in `gbnf_grammar.rs` and tested with a roundtrip
that decodes the generated JSON into `IndexQuery`.

#### `model_runner.rs` outline

- `ModelHandle` — `Arc<Mutex<Option<LlamaModel>>>` stored in Tauri app state.
- `try_load_model(app_data_dir) -> Option<ModelHandle>` — looks for
  `$APP_DATA/models/paker-assistant.gguf`; returns `None` if absent (graceful degradation).
- `run_grammar_parse(handle, text, grammar) -> Result<String>` — runs one inference with
  `n_predict = 256`, `temperature = 0.0`, `repeat_penalty = 1.0`.  Returns raw JSON string.
- `#[cfg(not(feature = "llm"))]` stubs return `Err(anyhow!("llm feature not enabled"))` so
  the codebase compiles without the feature flag.

#### `parse_result.rs` outline

```rust
pub struct LlmParsedQuery {
    pub index_query: IndexQuery,
    pub source: ParseSource,  // Llm | Regex
}

pub enum ParseSource { Llm, Regex }

pub fn merge_with_regex(
    llm: Option<LlmParsedQuery>,
    regex: ParsedAssistantQuery,
) -> ParsedAssistantQuery { ... }
```

Merge strategy: if LLM result is available AND regex confidence is not `High`, replace
`query` fields with LLM values; keep `summary` from `describe_index_query`.  If LLM result
is `None`, return the regex result unchanged.

---

### 5.2 `src-tauri/src/storage/assistant_history.rs`

Methods on `ObjectCacheManager`:

```rust
pub fn insert_query_history(
    &self,
    connection_id: &str,
    bucket: &str,
    raw_text: &str,
    summary: &str,
    confidence: &str,
    result_count: usize,
) -> Result<()>

pub fn list_query_history(
    &self,
    connection_id: &str,
    bucket: &str,
    limit: u32,  // default 20
) -> Result<Vec<QueryHistoryItem>>

pub fn clear_query_history(
    &self,
    connection_id: &str,
    bucket: &str,
) -> Result<()>

// Called after insert to enforce the 50-row cap
fn prune_query_history(
    &self,
    connection_id: &str,
    bucket: &str,
) -> Result<()>
```

`QueryHistoryItem` (serializable, sent over IPC):

```rust
pub struct QueryHistoryItem {
    pub id: i64,
    pub raw_text: String,
    pub summary: String,
    pub confidence: String,
    pub result_count: u64,
    pub created_at: String,
}
```

---

## 6. Modified Rust files

### 6.1 `src-tauri/src/assistant/mod.rs`

Add `pub mod llm;` (feature-gated: `#[cfg(feature = "llm")]` wrapper in mod, unconditional
stub otherwise).

### 6.2 `src-tauri/src/commands/assistant.rs`

Add four new commands:

#### `assistant_parse_query_llm`

```rust
#[tauri::command]
pub async fn assistant_parse_query_llm(
    app: AppHandle,
    text: String,
) -> ParsedAssistantQuery {
    let regex_result = parse_natural_language(&text);
    
    #[cfg(feature = "llm")]
    {
        let model = app.state::<ModelHandle>();
        if let Ok(llm_result) = llm::run_grammar_parse(&model, &text) {
            return merge_with_regex(Some(llm_result), regex_result);
        }
    }
    
    regex_result
}
```

The command is always registered (no `#[cfg]` on the `#[tauri::command]`), so the frontend
doesn't need feature-detection logic — it just receives the best available parse.

#### `assistant_query_history_list`

```rust
#[tauri::command]
pub async fn assistant_query_history_list(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    limit: Option<u32>,
) -> Result<Vec<QueryHistoryItem>, PakerError>
```

#### `assistant_query_history_clear`

```rust
#[tauri::command]
pub async fn assistant_query_history_clear(
    app: AppHandle,
    connection_id: String,
    bucket: String,
) -> Result<(), PakerError>
```

#### `assistant_pack_export`

```rust
#[tauri::command]
pub async fn assistant_pack_export(
    app: AppHandle,
    connection_id: String,
    bucket: String,
    keys: Vec<String>,
    format: ExportFormat,   // "csv" | "json" | "clipboard"
    save_path: Option<String>,
) -> Result<String, PakerError>  // returns path or "clipboard"
```

`ExportFormat` is a new enum in `assistant/query/mod.rs` or a sibling module.

### 6.3 `src-tauri/src/commands/mod.rs`

Register the four new commands in the `generate_handler![]` macro list.

### 6.4 `src-tauri/src/lib.rs`

Add `ModelHandle` state management:

```rust
#[cfg(feature = "llm")]
{
    let model_handle = llm::try_load_model(app.path().app_data_dir()?.as_path());
    app.manage(model_handle);
}
```

---

## 7. New TypeScript types (`src/types/assistant.ts`)

Append to the existing file (do not replace):

```typescript
export interface QueryHistoryItem {
  id: number;
  rawText: string;
  summary: string;
  confidence: ParseConfidence;
  resultCount: number;
  createdAt: string;
}

export type ExportFormat = "csv" | "json" | "clipboard";
```

---

## 8. New `tauri.ts` wrappers (`src/lib/tauri.ts`)

Append after the existing `assistantGenerateCli` wrapper:

```typescript
export function assistantParseQueryLlm(text: string): Promise<ParsedAssistantQuery> {
  return invokeSafe<ParsedAssistantQuery>("assistant_parse_query_llm", { text });
}

export function assistantQueryHistoryList(
  connectionId: string,
  bucket: string,
  limit?: number
): Promise<QueryHistoryItem[]> {
  return invokeSafe<QueryHistoryItem[]>("assistant_query_history_list", {
    connectionId,
    bucket,
    limit: limit ?? null,
  });
}

export function assistantQueryHistoryClear(
  connectionId: string,
  bucket: string
): Promise<void> {
  return invokeSafe<void>("assistant_query_history_clear", { connectionId, bucket });
}

export function assistantPackExport(
  connectionId: string,
  bucket: string,
  keys: string[],
  format: ExportFormat,
  savePath?: string
): Promise<string> {
  return invokeSafe<string>("assistant_pack_export", {
    connectionId,
    bucket,
    keys,
    format,
    savePath: savePath ?? null,
  });
}
```

---

## 9. New/modified UI files

### 9.1 `src/components/browser/SmartPackDrawer.tsx` — **new**

A right-side `Sheet` (Radix/shadcn) that replaces the two separate dialogs
(`BucketIndexSearchDialog` and `BucketReportDialog`) at the call-site level.  The two
existing dialog components remain untouched so they can still be used independently.

**Sections:**

1. **Header** — title "Smart Pack", `X` close button, connection/bucket pill.
2. **Query bar** — `Input` + "Search" `Button`, same debounce logic as the existing
   `BucketIndexSearchDialog`.  Calls `assistantParseQueryLlm` instead of
   `assistantParseQuery`.  Parsed summary + confidence badge displayed below the input.
3. **History strip** — horizontal scrollable row of the last 10 queries as chips.
   Clicking a chip re-populates the input + re-runs search.  "Clear" icon at the right end.
4. **Result list** — virtualised list (`@tanstack/react-virtual`) for large result sets.
   Each row: checkbox, key (monospaced), size, last-modified, storage-class badge.
   "Select all" and "Deselect all" buttons above the list.
5. **Pack actions bar** — appears when ≥1 result is checked:
   - "Copy keys" — copies newline-separated keys to clipboard.
   - "Export CSV" — calls `assistantPackExport(..., "csv")`, saves via `rfd` picker or
     downloads to Desktop.
   - "Copy as AWS CLI" — calls `assistantGenerateCli` with selected keys, opens CLI sheet.
6. **Report summary strip** — always-visible collapsible section at the bottom, loaded once
   on drawer open via `assistantGetBucketReport`.

### 9.2 `src/components/browser/BrowserToolbar.tsx` — **modify**

Add `onOpenSmartPack?: () => void` prop and a `PackageSearch` or `Sparkles` icon button
that fires it.  Placed next to the existing `onSearchIndex` button.

### 9.3 `src/components/layout/AppShell.tsx` — **modify**

- Import `SmartPackDrawer`.
- Add `smartPackOpen` state (boolean).
- Wire `onOpenSmartPack` on `BrowserToolbar`.
- Render `<SmartPackDrawer open={smartPackOpen} onOpenChange={setSmartPackOpen} ... />`.
- History is loaded lazily on drawer open.
- After each successful search, call `assistantQueryHistoryList` to refresh the history strip.

### 9.4 `src/types/assistant.ts` — **modify**

Add `QueryHistoryItem` and `ExportFormat` (see §7).

---

## 10. IPC command summary

| Command | Direction | Description |
|---|---|---|
| `assistant_parse_query` | already exists | Regex-only parse (kept as-is) |
| `assistant_parse_query_llm` | **new** | Regex + optional LLM parse |
| `assistant_run_index_query` | already exists | Execute `IndexQuery` against SQLite |
| `assistant_get_bucket_report` | already exists | Bucket stats report |
| `assistant_query_history_list` | **new** | Fetch recent queries for a bucket |
| `assistant_query_history_clear` | **new** | Delete history for a bucket |
| `assistant_pack_export` | **new** | Export result keys as CSV/JSON/clipboard |
| `assistant_explain_error` | already exists | Error code explanation |
| `assistant_generate_cli` | already exists | AWS CLI / rclone commands |

---

## 11. GBNF grammar full outline

```
# Root: optional top-level object (all fields optional)
root    ::= obj | "{" ws "}"

obj     ::= "{" ws field-list ws "}"
field-list ::= field (ws "," ws field)*
             | ""

field ::= key-pattern-field
        | prefix-field
        | min-size-field
        | max-size-field
        | modified-after-field
        | modified-before-field
        | storage-class-field

key-pattern-field    ::= "\"keyPattern\""    ws ":" ws (string | null)
prefix-field         ::= "\"prefix\""        ws ":" ws (string | null)
min-size-field       ::= "\"minSize\""       ws ":" ws (uint64 | null)
max-size-field       ::= "\"maxSize\""       ws ":" ws (uint64 | null)
modified-after-field ::= "\"modifiedAfter\"" ws ":" ws (iso8601-string | null)
modified-before-field::= "\"modifiedBefore\""ws ":" ws (iso8601-string | null)
storage-class-field  ::= "\"storageClass\""  ws ":" ws (sc-array | null)

sc-array  ::= "[" ws "]"
            | "[" ws sc-value (ws "," ws sc-value)* ws "]"
sc-value  ::= "\"STANDARD\""
            | "\"STANDARD_IA\""
            | "\"ONEZONE_IA\""
            | "\"INTELLIGENT_TIERING\""
            | "\"GLACIER\""
            | "\"GLACIER_IR\""
            | "\"DEEP_ARCHIVE\""

# Primitives
string       ::= "\"" char* "\""
char         ::= [^"\\] | "\\" escape
escape       ::= ["\\bfnrt] | "u" [0-9a-fA-F]{4}
uint64       ::= [1-9][0-9]* | "0"
iso8601-string ::= "\"" [0-9]{4} "-" [0-9]{2} "-" [0-9]{2}
                   ("T" [0-9]{2} ":" [0-9]{2} ":" [0-9]{2} "Z")? "\""
null         ::= "null"
ws           ::= [ \t\n\r]*
```

**Test strategy for the grammar:**
- Property-based: generate random `IndexQuery` structs, serialize to JSON, re-parse — verify
  round-trip equality.
- Negative: ensure strings like `"DEEP_ARCHIVE_PLUS"` (not in sc-value) fail grammar
  validation at the `from_str` level.
- No model loaded is required — tests use a grammar-validator-only mode of `llama-cpp-2`
  (`llama_grammar_init` / `llama_grammar_accept_token` can be called without a model).

---

## 12. Integration with `assistant_parse_query`

The existing `assistant_parse_query` command is kept verbatim and continues to be called
by the unchanged `BucketIndexSearchDialog`.

`assistant_parse_query_llm` wraps it:

```
1. Call rule_parser::parse_natural_language(text)          → regex_result
2. If regex_result.confidence == High → return regex_result (skip LLM, fast path)
3. If LLM model not loaded            → return regex_result (graceful)
4. Run grammar-constrained inference  → raw_json
5. Deserialize raw_json → IndexQuery
6. Build LlmParsedQuery { index_query, source: Llm }
7. Call merge_with_regex(Some(llm_result), regex_result)
8. Return merged ParsedAssistantQuery
```

History insertion happens in the **command layer**, not in the parser:

```rust
// Inside assistant_run_index_query (modified)
cache.query_bucket_index(...)
    .map(|results| {
        let _ = cache.insert_query_history(
            &connection_id,
            &bucket,
            &raw_text_hint,   // passed as new optional arg or stored in a channel
            &parsed.summary,
            &parsed.confidence_str(),
            results.len(),
        );
        results
    })
```

> **Design note:** To avoid threading the raw text through `assistant_run_index_query`, an
> alternative is to call `assistant_query_history_insert` as a separate IPC command from the
> frontend immediately after a successful search.  Simpler, avoids changing an existing
> command signature.  Recommended approach for 1b.

---

## 13. Risks and deferrals

| Risk | Likelihood | Mitigation |
|---|---|---|
| `llama-cpp-2` crate doesn't compile on all CI targets | Medium | Wrap behind `[features] llm`; CI runs without the flag by default |
| Grammar-constrained output still produces invalid JSON edge cases | Low | Wrap `serde_json::from_str` in a `Result`; fall back to regex parse on any error |
| Model file too large to bundle (1–3 GB) | N/A | Model is never bundled; user places `.gguf` in app-data dir; Phase 1b ships without a bundled model |
| History table grows without bound | Low | `prune_query_history` keeps ≤50 rows per `(connection_id, bucket)` on every insert |
| SmartPackDrawer too complex to implement cleanly in one pass | Medium | Deliver in two sub-tasks: (a) drawer shell + search + history, (b) pack actions + report strip |
| `@tanstack/react-virtual` adds bundle size | Low | Already used elsewhere in the project (check); if not, tree-shaking limits impact to ~10 KB |

**Deferred:**
- LLM streaming token-by-token output via Tauri event channel → Phase 2.
- "What changed since last query?" diff view → Phase 2.
- Auto-download of quantised model from HuggingFace → Phase 2.
- Multi-bucket query (search across multiple buckets at once) → Phase 2.

---

## 14. Test strategy

### Rust unit tests

| File | Test | What is verified |
|---|---|---|
| `assistant/llm/gbnf_grammar.rs` | `grammar_roundtrip` | Serialize random `IndexQuery` → validate against grammar string |
| `assistant/llm/parse_result.rs` | `merge_prefers_llm` | When LLM result present and regex confidence Low, LLM wins |
| `assistant/llm/parse_result.rs` | `merge_keeps_regex_on_high` | When regex confidence High, skip LLM regardless |
| `storage/assistant_history.rs` | `insert_and_list` | Insert 3 items, list returns all 3 newest-first |
| `storage/assistant_history.rs` | `prune_at_50` | Insert 55 items, assert ≤50 remain after each insert |
| `storage/assistant_history.rs` | `clear_removes_all` | Clear leaves 0 rows for that bucket |

### TypeScript component tests (Vitest / React Testing Library)

| Component | Test | What is verified |
|---|---|---|
| `SmartPackDrawer` | renders closed by default | Sheet is not in the DOM when `open=false` |
| `SmartPackDrawer` | history chips fire re-search | Clicking a chip calls `assistantParseQueryLlm` with chip text |
| `SmartPackDrawer` | pack actions bar appears on selection | Checking a result row shows the export buttons |
| `SmartPackDrawer` | "Copy keys" writes to clipboard | `navigator.clipboard.writeText` called with newline-joined keys |

### Manual smoke test checklist

- [ ] Open Smart Pack drawer → history strip empty on first use.
- [ ] Type "pdf files > 50mb last 30 days" → parsed summary appears within 300 ms.
- [ ] Hit Search → results populate; each row checkable.
- [ ] Select 3 rows → pack actions bar appears.
- [ ] "Export CSV" → file dialog opens; CSV saved with `key,size,lastModified,storageClass` columns.
- [ ] Re-open drawer → previous query appears in history strip.
- [ ] Click history chip → input pre-filled and search runs automatically.
- [ ] "Clear history" → strip empties; SQLite row count confirms 0.
- [ ] Drop a `.gguf` model into app-data/models → relaunch → low-confidence queries now show LLM parse source.
- [ ] Remove model file → app launches without error; falls back to regex parse silently.

---

## 15. Numbered task list with hour estimates

> **Legend:** `[BE]` = Rust back-end, `[FE]` = TypeScript/React, `[BOTH]` = coordination/wiring.

| # | Task | Area | Est. (hrs) |
|---|---|---|---|
| 1 | Create `docs/plans/` directory and write this plan | BOTH | 0.5 |
| 2 | Add `[features] llm` flag to `Cargo.toml`; verify CI compiles without it | BE | 0.5 |
| 3 | `assistant/llm/gbnf_grammar.rs`: write GBNF string constant + unit tests (no model needed) | BE | 2.0 |
| 4 | `assistant/llm/parse_result.rs`: `LlmParsedQuery`, `ParseSource`, `merge_with_regex()` + tests | BE | 1.5 |
| 5 | `assistant/llm/model_runner.rs`: `ModelHandle`, `try_load_model()`, `run_grammar_parse()` stubs + `#[cfg(not(feature="llm"))]` fallbacks | BE | 3.0 |
| 6 | `assistant/llm/mod.rs`: wire the sub-module public API | BE | 0.5 |
| 7 | `storage/assistant_history.rs`: SQLite schema migration + `ObjectCacheManager` methods + tests | BE | 2.5 |
| 8 | `commands/assistant.rs`: add `assistant_parse_query_llm`, `assistant_query_history_list`, `assistant_query_history_clear`, `assistant_pack_export` | BE | 2.0 |
| 9 | `commands/mod.rs` + `lib.rs`: register new commands, add `ModelHandle` state | BE | 0.5 |
| 10 | `src/types/assistant.ts`: add `QueryHistoryItem`, `ExportFormat` | FE | 0.25 |
| 11 | `src/lib/tauri.ts`: add four new wrapper functions | FE | 0.5 |
| 12 | `SmartPackDrawer.tsx`: drawer shell, NL query input, debounce, parsed summary, loading states | FE | 3.0 |
| 13 | `SmartPackDrawer.tsx`: history strip (load, display chips, re-search on click, clear) | FE | 2.0 |
| 14 | `SmartPackDrawer.tsx`: virtualised result list with checkboxes + select-all | FE | 2.5 |
| 15 | `SmartPackDrawer.tsx`: pack actions bar (copy keys, export CSV, copy as AWS CLI) | FE | 2.0 |
| 16 | `SmartPackDrawer.tsx`: report summary strip (collapsible, loaded on open) | FE | 1.5 |
| 17 | `BrowserToolbar.tsx`: add `onOpenSmartPack` prop + toolbar button | FE | 0.5 |
| 18 | `AppShell.tsx`: wire `SmartPackDrawer` (state, props, history refresh after search) | FE | 1.0 |
| 19 | TypeScript component tests (Vitest + RTL) for `SmartPackDrawer` | FE | 2.0 |
| 20 | Manual smoke test + bug-fix pass | BOTH | 2.0 |
| 21 | Final review: linter, `cargo clippy`, TypeScript strict, exhaustive switch guards | BOTH | 1.0 |
| **Total** | | | **~31 hrs** |

### Suggested execution order

**Day 1 (6 h):** Tasks 2–6 (GBNF + LLM module back-end, no actual model needed).  
**Day 2 (5 h):** Tasks 7–9 (history persistence + command registration).  
**Day 3 (6 h):** Tasks 10–12 + 17–18 (TypeScript types, wrappers, drawer shell, toolbar wiring).  
**Day 4 (6 h):** Tasks 13–16 (history strip, result list, pack actions, report strip).  
**Day 5 (4 h):** Tasks 19–21 (tests, smoke test, polish).  

Total calendar time: **~5 focused days** assuming 5–6 h/day of implementation.

---

## 16. File change summary

### Create

```
src-tauri/src/assistant/llm/mod.rs
src-tauri/src/assistant/llm/gbnf_grammar.rs
src-tauri/src/assistant/llm/model_runner.rs
src-tauri/src/assistant/llm/parse_result.rs
src-tauri/src/storage/assistant_history.rs
src/components/browser/SmartPackDrawer.tsx
docs/plans/phase-1b-smart-pack.md          ← this file
```

### Modify

```
src-tauri/Cargo.toml                       — add llm feature + llama-cpp-2 dep
src-tauri/src/assistant/mod.rs             — pub mod llm
src-tauri/src/commands/assistant.rs        — 4 new commands
src-tauri/src/commands/mod.rs              — register new commands in handler macro
src-tauri/src/lib.rs                       — manage ModelHandle state
src-tauri/src/storage/object_cache.rs      — SQLite migration block for 2 new tables
src/types/assistant.ts                     — QueryHistoryItem, ExportFormat
src/lib/tauri.ts                           — 4 new wrapper functions
src/components/browser/BrowserToolbar.tsx  — onOpenSmartPack prop + button
src/components/layout/AppShell.tsx         — SmartPackDrawer wiring
```

### Unchanged (existing assistant infrastructure)

```
src-tauri/src/assistant/query/rule_parser.rs   ← regression-free
src-tauri/src/assistant/query/index_query.rs   ← regression-free
src-tauri/src/assistant/reports/             ← regression-free
src-tauri/src/assistant/templates/           ← regression-free
src-tauri/src/assistant/explain/             ← regression-free
src-tauri/src/commands/bucket_index.rs       ← unchanged
src/components/browser/BucketIndexSearchDialog.tsx  ← kept, still usable standalone
src/components/browser/BucketReportDialog.tsx       ← kept, still usable standalone
```
