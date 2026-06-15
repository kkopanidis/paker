# Phase 3 — MCP Server (`paker-mcp`)

> **Goal:** Expose Paker's S3 intelligence as a local MCP server so AI agents (Cursor, Claude Desktop, etc.) can introspect S3 buckets, run structured index queries, and generate CLI commands — all without ever seeing raw credentials.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Workspace Layout](#2-workspace-layout)
3. [PakerCore Refactor](#3-pakercore-refactor)
4. [rmcp Integration](#4-rmcp-integration)
5. [The Nine Read-Only Tools](#5-the-nine-read-only-tools)
6. [Security & Scoping Model](#6-security--scoping-model)
7. [Transport & Startup](#7-transport--startup)
8. [Example mcp.json Configurations](#8-example-mcpjson-configurations)
9. [Task List](#9-task-list)
10. [Open Questions](#10-open-questions)

---

## 1. Overview

Paker is a Tauri desktop app. Its Rust core (`paker_lib`) already contains:

- **Connection profiles** — stored in `connections.json` with no secrets
- **S3 client builder** — reads secrets from keyring / AES-GCM vault at call time
- **SQLite bucket index** (`ObjectCacheManager`) — pre-indexed object metadata
- **Assistant module** — NL query parsing, structured index queries, bucket reports, CLI template generation, S3 error explanations

The MCP server reuses all of this without touching Tauri or the GUI.  
It is a **separate binary** (`paker-mcp`) that speaks the [Model Context Protocol](https://modelcontextprotocol.io) over `stdio` so any MCP-capable client can spawn it.

### Non-goals

- No write, copy, delete, or transfer operations exposed via MCP
- No credential values ever returned to the AI client
- No Tauri runtime — the binary must boot in < 200 ms

---

## 2. Workspace Layout

Convert `src-tauri/` into a Cargo workspace so `paker-mcp` can share `paker_lib` without duplicating code.

### File tree (new entries marked `[new]`)

```
paker/
├── src-tauri/
│   ├── Cargo.toml          ← becomes workspace root
│   ├── Cargo.lock
│   ├── paker/              [new]  ← current src-tauri content moved here
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── main.rs
│   │   │   └── ...
│   │   └── tauri.conf.json
│   └── paker-mcp/          [new]
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── core.rs      ← PakerCore
│           └── tools/
│               ├── mod.rs
│               ├── connections.rs
│               ├── buckets.rs
│               ├── objects.rs
│               ├── index.rs
│               └── assistant.rs
```

### `src-tauri/Cargo.toml` (workspace root)

```toml
[workspace]
members = ["paker", "paker-mcp"]
resolver = "2"
```

### `paker-mcp/Cargo.toml`

```toml
[package]
name    = "paker-mcp"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "paker-mcp"
path = "src/main.rs"

[dependencies]
paker_lib   = { path = "../paker" }
rmcp        = { version = "0.2", features = ["server", "transport-io"] }
tokio       = { version = "1", features = ["full"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
anyhow      = "1"
tracing     = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

> **rmcp** is the official Rust MCP SDK (crates.io: `rmcp`). It handles JSON-RPC framing, capability negotiation, tool dispatch, and error serialization over stdio or SSE transports.

---

## 3. PakerCore Refactor

### Problem

Every storage function in `paker_lib` currently takes `&AppHandle` purely to resolve the data directory path via `tauri::Manager::path()`. The MCP binary has no Tauri runtime.

### Solution: path-agnostic storage layer

Introduce `PakerCore` — a plain Rust struct that holds resolved paths and open resources.

#### 3.1 `storage::paths` additions

Add a standalone variant that takes an explicit `PathBuf` instead of `&AppHandle`:

```rust
// storage/paths.rs  (new public function)
pub fn data_dir_explicit(base: &Path) -> Result<PathBuf> {
    fs::create_dir_all(base)
        .with_context(|| format!("failed to create data dir {}", base.display()))?;
    Ok(base.to_path_buf())
}

pub fn connections_path_in(base: &Path) -> PathBuf { base.join("connections.json") }
pub fn index_db_path_in(base: &Path)    -> Result<PathBuf> {
    let path = base.join("index").join("index.db");
    ensure_parent(&path)?;
    Ok(path)
}
```

The existing `AppHandle`-taking variants stay untouched; they delegate to these.

#### 3.2 `storage::profiles` additions

Add path-taking variants alongside the existing `AppHandle` variants:

```rust
// storage/profiles.rs
pub fn list_connections_from(base: &Path) -> Result<Vec<ConnectionProfile>> {
    read_all_from(base)
}

pub fn get_connection_from(base: &Path, id: &str) -> Result<Option<ConnectionProfile>> {
    Ok(read_all_from(base)?.into_iter().find(|p| p.id == id))
}

fn read_all_from(base: &Path) -> Result<Vec<ConnectionProfile>> {
    // same logic as read_all(app) but uses connections_path_in(base)
}
```

#### 3.3 `storage::secrets` additions

The secret loader already uses `AppHandle` to find the data dir and keyring service name. Add:

```rust
// storage/secrets.rs
pub fn get_secret_from(base: &Path, profile_id: &str) -> Result<Option<String>> { ... }
pub fn get_session_token_from(base: &Path, profile_id: &str) -> Result<Option<String>> { ... }
```

#### 3.4 `s3::client` additions

```rust
// s3/client.rs
pub async fn build_client_for_profile_standalone(
    profile: &ConnectionProfile,
    data_dir: &Path,
) -> Result<Client, PakerError> { ... }
```

This uses `get_secret_from` + `get_session_token_from` instead of the `AppHandle` overloads.

#### 3.5 The `PakerCore` struct

Located at `paker-mcp/src/core.rs`:

```rust
use paker_lib::storage::{ConnectionProfile, ObjectCacheManager};
use paker_lib::storage::profiles::list_connections_from;
use std::path::PathBuf;
use anyhow::Result;

pub struct PakerCore {
    pub data_dir:   PathBuf,
    pub cache:      ObjectCacheManager,
    /// Profiles cached at startup; call `reload_profiles` if staleness is a concern.
    profiles:       Vec<ConnectionProfile>,
    /// Optional allow-list from PAKER_MCP_ALLOWED_CONNECTIONS.
    allowed_ids:    Option<Vec<String>>,
}

impl PakerCore {
    pub fn new(data_dir: PathBuf, allowed_ids: Option<Vec<String>>) -> Result<Self> {
        let db_path = paker_lib::storage::paths::index_db_path_in(&data_dir)?;
        let cache   = ObjectCacheManager::open(db_path)?;
        let profiles = list_connections_from(&data_dir)?;
        Ok(Self { data_dir, cache, profiles, allowed_ids })
    }

    pub fn profiles(&self) -> &[ConnectionProfile] {
        match &self.allowed_ids {
            None      => &self.profiles,
            Some(ids) => &self.profiles.iter()
                             .filter(|p| ids.contains(&p.id))
                             .cloned()
                             .collect::<Vec<_>>(),   // cached in practice
        }
    }

    pub fn get_profile(&self, id: &str) -> Option<&ConnectionProfile> {
        self.profiles().iter().find(|p| p.id == id)
    }
}
```

`PakerCore` is wrapped in `Arc<PakerCore>` and passed to the rmcp tool handler.

---

## 4. rmcp Integration

### 4.1 How rmcp works

`rmcp` implements MCP over stdio. The entry point is:

```rust
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let core  = Arc::new(PakerCore::new(resolve_data_dir()?, allowed_ids())?);
    let tools = PakerMcpServer::new(core);
    let (_handle, _) = tools.serve(stdio()).await?;
    Ok(())
}
```

### 4.2 Tool handler struct

```rust
use rmcp::{tool, ServerHandler, model::ServerInfo};

#[derive(Clone)]
pub struct PakerMcpServer {
    core: Arc<PakerCore>,
}

#[rmcp::server]
impl PakerMcpServer {
    // Nine tool methods, see §5
}

impl ServerHandler for PakerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name:    "paker".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        }
    }
}
```

### 4.3 Error mapping

MCP tools must return `Result<CallToolResult, McpError>`. A small helper:

```rust
fn mcp_err(msg: impl std::fmt::Display) -> McpError {
    McpError::new(ErrorCode::InternalError, msg.to_string(), None)
}
```

---

## 5. The Nine Read-Only Tools

All tools return JSON serialized as `CallToolResult::text(serde_json::to_string(&result)?)`.  
None of them return credential values. Live S3 calls are clearly marked.

---

### Tool 1 — `list_connections`

**Purpose:** Return all saved connection profiles so the agent knows what IDs exist.

```
Input:  (none)
Output: Array of ConnectionProfile (id, name, endpoint, region, forcePathStyle, defaultBucket)
        — accessKeyId is intentionally OMITTED from the serialized output
```

**Implementation:** `self.core.profiles()` filtered by `allowed_ids`.  
`access_key_id` is stripped before serialization via a dedicated `SafeConnectionProfile` view struct.

---

### Tool 2 — `list_buckets`

**Purpose:** List S3 buckets available to a connection.  
**⚡ Live S3 call.**

```
Input:  { "connection_id": "conn-abc" }
Output: Array of { name: string, creationDate: string|null }
```

**Implementation:** `build_client_for_profile_standalone` → `client.list_buckets().send()`.

---

### Tool 3 — `list_objects`

**Purpose:** Browse S3 objects under a prefix (single page).  
**⚡ Live S3 call.**

```
Input:  {
  "connection_id": "conn-abc",
  "bucket":        "my-bucket",
  "prefix":        "photos/2024/",   // optional, default ""
  "delimiter":     "/",              // optional, default "/"
  "max_keys":      100               // optional, default 100, max 1000
}
Output: ListObjectsResult { objects, commonPrefixes, continuationToken, isTruncated }
```

**Implementation:** reuses `paker_lib::s3::operations::list_objects` (already extracted as a plain async fn).

---

### Tool 4 — `get_bucket_index_status`

**Purpose:** Check whether a bucket has been indexed in the local SQLite cache.

```
Input:  { "connection_id": "conn-abc", "bucket": "my-bucket" }
Output: BucketIndexMeta | null
        { connectionId, bucket, status, objectCount, startedAt, completedAt, error }
```

**Implementation:** `self.core.cache.get_bucket_index_meta(...)`.

---

### Tool 5 — `search_index`

**Purpose:** Keyword search across indexed object keys (fast SQLite LIKE query).  
Requires a completed index — returns an error if not indexed.

```
Input:  {
  "connection_id": "conn-abc",
  "bucket":        "my-bucket",
  "query":         "invoice",
  "limit":         50,    // optional, default 50, max 200
  "offset":        0      // optional, default 0
}
Output: Array of IndexedObject { key, size, lastModified, etag, storageClass }
```

**Implementation:** `self.core.cache.search_bucket_index(...)`.

---

### Tool 6 — `query_index`

**Purpose:** Structured filter across the bucket index — more powerful than keyword search.

```
Input:  {
  "connection_id":    "conn-abc",
  "bucket":           "my-bucket",
  "prefix":           "logs/",           // optional
  "key_pattern":      "%.log.gz",        // optional SQL LIKE pattern
  "min_size":         1048576,           // optional bytes
  "max_size":         null,              // optional bytes
  "modified_after":   "2024-01-01",      // optional ISO date string
  "modified_before":  null,              // optional ISO date string
  "storage_class":    ["STANDARD","STANDARD_IA"], // optional
  "limit":            100,
  "offset":           0
}
Output: Array of IndexedObject
```

**Implementation:** `self.core.cache.query_bucket_index(connection_id, bucket, &IndexQuery { ... })`.

---

### Tool 7 — `get_bucket_report`

**Purpose:** Return aggregate storage statistics for an indexed bucket.

```
Input:  {
  "connection_id": "conn-abc",
  "bucket":        "my-bucket",
  "top_n":         10    // optional, default 10
}
Output: BucketReport {
  totalObjects, totalBytes,
  topPrefixesByBytes: [{ prefix, objectCount, totalBytes }],
  glacierObjectCount, glacierBytes,
  smallFileCount, smallFileThresholdBytes
}
```

**Implementation:** `self.core.cache.build_bucket_report(...)`.

---

### Tool 8 — `explain_s3_error`

**Purpose:** Return a human-readable explanation of an S3/AWS error code.

```
Input:  { "code": "AccessDenied" }
Output: ErrorExplanation { code, title, description, suggestions: [string] }
```

**Implementation:** `paker_lib::assistant::explain::explain_error_code(&code)` — pure, no I/O.

---

### Tool 9 — `generate_cli_commands`

**Purpose:** Generate `aws s3` / `rclone` command suggestions for common operations.

```
Input:  {
  "connection_id": "conn-abc",
  "bucket":        "my-bucket",
  "operation":     "sync" | "download" | "upload" | "list" | "presign",
  "prefix":        "backups/",    // optional
  "keys":          ["a.txt"],     // optional, for per-key ops
  "local_path":    "/tmp/out"     // optional
}
Output: Array of CliCommandSuggestion { tool, command, description }
```

**Implementation:** Loads the profile name/endpoint from `self.core.get_profile(connection_id)` and calls `paker_lib::assistant::templates::generate_cli_commands(...)`. Credentials are NOT injected into the generated commands — they reference named AWS profiles or rclone remotes instead.

---

## 6. Security & Scoping Model

### 6.1 Credential isolation

- Credentials are loaded from the system keyring / AES-GCM vault only when a live S3 call is made (tools 2 and 3).
- Credentials exist only in-process for the duration of a single tool call and are not retained in `PakerCore`.
- No tool returns `access_key_id`, `secret_access_key`, or `session_token` in its output — enforced by the `SafeConnectionProfile` view type used in tool responses.
- The MCP server process runs with the same OS user as the desktop app; no additional credentials are needed.

### 6.2 Connection allow-list

Set `PAKER_MCP_ALLOWED_CONNECTIONS` to a comma-separated list of connection IDs to restrict the MCP server to a subset of saved connections:

```
PAKER_MCP_ALLOWED_CONNECTIONS=conn-prod,conn-staging
```

If unset, all saved connections are accessible.

Any tool call referencing a connection ID not in the allow-list returns:

```json
{ "error": { "code": -32602, "message": "Connection not found or not permitted" } }
```

### 6.3 Read-only enforcement

- No tools call any mutating S3 API (`PutObject`, `DeleteObject`, `CopyObject`, `CreateBucket`, etc.)
- No tools write to `connections.json`, the SQLite index, or the vault
- The `ObjectCacheManager` is opened read-only for index queries (future: pass `OpenFlags::SQLITE_OPEN_READ_ONLY`)

### 6.4 Input validation

- `max_keys` is clamped to 1000 on `list_objects`
- `limit` is clamped to 200 on `search_index` / `query_index`
- `connection_id` is validated against known profiles before any I/O
- `prefix` and `key_pattern` values are escaped for SQLite LIKE before use (already done in `query_bucket_index`)
- Endpoint URLs supplied by profiles are validated via `paker_lib::error::validate_endpoint_url` before S3 client construction

---

## 7. Transport & Startup

### 7.1 stdio transport

The MCP server speaks JSON-RPC 2.0 over stdin/stdout. This is the default mode and requires no daemon or port binding. The OS spawns the process on demand.

### 7.2 Data directory resolution

The binary resolves the data directory in this order:

1. `PAKER_DATA_DIR` environment variable (explicit override)
2. Platform default: same path as the desktop app's `app_data_dir`
   - macOS: `$HOME/Library/Application Support/com.paker.app/`
   - Windows: `%APPDATA%\com.paker.app\`
   - Linux: `$HOME/.local/share/com.paker.app/`
3. If none resolve, exit with a clear error message

```rust
fn resolve_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(dir) = std::env::var("PAKER_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    // platform default via dirs crate
    let base = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot resolve platform data directory"))?;
    Ok(base.join("com.paker.app"))
}
```

Add `dirs = "5"` to `paker-mcp/Cargo.toml`.

### 7.3 Logging

Logs go to stderr (never stdout — that carries MCP JSON).  
Default filter: `paker_mcp=info,paker_lib=warn`.  
Override via `RUST_LOG`.

### 7.4 Bundling

The `paker-mcp` binary is built alongside the main app in CI:

```
cargo build --release -p paker-mcp
```

On macOS it is placed inside `Paker.app/Contents/MacOS/paker-mcp`.  
On Windows/Linux it is placed next to the main executable.

---

## 8. Example mcp.json Configurations

### Cursor (`~/.cursor/mcp.json` or project `.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "paker": {
      "command": "/Applications/Paker.app/Contents/MacOS/paker-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

### Cursor — restricted to specific connections

```json
{
  "mcpServers": {
    "paker": {
      "command": "/Applications/Paker.app/Contents/MacOS/paker-mcp",
      "args": [],
      "env": {
        "PAKER_MCP_ALLOWED_CONNECTIONS": "conn-staging,conn-dev"
      }
    }
  }
}
```

### Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "paker": {
      "command": "/Applications/Paker.app/Contents/MacOS/paker-mcp",
      "args": [],
      "env": {
        "PAKER_DATA_DIR": "/Users/alice/Library/Application Support/com.paker.app"
      }
    }
  }
}
```

### Windows (PowerShell / Claude Desktop)

```json
{
  "mcpServers": {
    "paker": {
      "command": "C:\\Program Files\\Paker\\paker-mcp.exe",
      "args": []
    }
  }
}
```

### Development (cargo run)

```json
{
  "mcpServers": {
    "paker-dev": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/paker/src-tauri/Cargo.toml", "-p", "paker-mcp"],
      "env": {
        "RUST_LOG": "paker_mcp=debug,paker_lib=debug"
      }
    }
  }
}
```

---

## 9. Task List

Tasks are ordered by dependency. Each task references the section above.

### Phase 3.0 — Workspace restructure

- [ ] **T1** Move current `src-tauri/` content into `src-tauri/paker/` sub-crate; add workspace `Cargo.toml`
- [ ] **T2** Verify existing Tauri build still works after the move (`cargo tauri build` in CI)
- [ ] **T3** Add `paker-mcp/` skeleton with `Cargo.toml` and a no-op `main.rs` (`println!("ok")`)

### Phase 3.1 — `PakerCore` and path-agnostic storage (§3)

- [ ] **T4** Add `data_dir_explicit`, `connections_path_in`, `index_db_path_in` to `storage::paths`
- [ ] **T5** Add `list_connections_from` and `get_connection_from` to `storage::profiles`
- [ ] **T6** Add `get_secret_from` / `get_session_token_from` to `storage::secrets`
- [ ] **T7** Add `build_client_for_profile_standalone` to `s3::client`
- [ ] **T8** Implement `PakerCore::new` in `paker-mcp/src/core.rs`
- [ ] **T9** Unit-test `PakerCore::new` against a temp data dir with a sample `connections.json`

### Phase 3.2 — rmcp server skeleton (§4)

- [ ] **T10** Add `rmcp` dependency to `paker-mcp/Cargo.toml`
- [ ] **T11** Implement `PakerMcpServer` struct with `ServerHandler::get_info`
- [ ] **T12** Wire stdio transport in `main.rs`; confirm `mcp ping` succeeds

### Phase 3.3 — Read-only tools (§5)

- [ ] **T13** Implement `list_connections` (tool 1) with `SafeConnectionProfile` view struct
- [ ] **T14** Implement `get_bucket_index_status` (tool 4) — no S3 call, quick win
- [ ] **T15** Implement `search_index` (tool 5)
- [ ] **T16** Implement `query_index` (tool 6)
- [ ] **T17** Implement `get_bucket_report` (tool 7)
- [ ] **T18** Implement `explain_s3_error` (tool 8)
- [ ] **T19** Implement `generate_cli_commands` (tool 9)
- [ ] **T20** Implement `list_buckets` (tool 2) — first live S3 tool
- [ ] **T21** Implement `list_objects` (tool 3)

### Phase 3.4 — Security & scoping (§6)

- [ ] **T22** Enforce `PAKER_MCP_ALLOWED_CONNECTIONS` in `PakerCore::profiles()` / `get_profile()`
- [ ] **T23** Clamp `max_keys`, `limit` inputs in tool handlers
- [ ] **T24** Open SQLite with `SQLITE_OPEN_READ_ONLY` flag in `PakerCore` (index queries only)
- [ ] **T25** Audit all tool outputs — confirm no field carries raw credentials; add `#[serde(skip)]` or dedicated view types where needed

### Phase 3.5 — Data directory resolution & startup (§7)

- [ ] **T26** Add `dirs` crate; implement `resolve_data_dir()` with env-override
- [ ] **T27** Add startup checks: print actionable error if `connections.json` missing or data dir unreachable
- [ ] **T28** Write startup integration test: spawn `paker-mcp`, send `initialize` + `tools/list`, assert 9 tools returned

### Phase 3.6 — Build, bundle, documentation

- [ ] **T29** Add `paker-mcp` to CI build matrix; gate on all existing `paker_lib` tests passing
- [ ] **T30** Update Tauri `beforeBuildCommand` (or equivalent) to also build `paker-mcp --release`
- [ ] **T31** Add macOS bundle hook to copy `paker-mcp` into `Paker.app/Contents/MacOS/`
- [ ] **T32** Write `docs/mcp-server.md` user-facing docs with example `mcp.json` snippets and tool reference

---

## 10. Open Questions

| # | Question | Impact |
|---|----------|--------|
| Q1 | Should live S3 tools (2 and 3) be gated behind an explicit `--allow-live-s3` flag or env var to prevent accidental data access from untrusted AI clients? | Security |
| Q2 | Should `list_objects` support pagination tokens so agents can iterate large buckets, or keep it single-page and recommend `query_index` for large scans? | UX |
| Q3 | Should `paker-mcp` bundle into the main Paker `.app` or ship as a separate opt-in download? | Distribution |
| Q4 | Should index queries return a `total_count` alongside results for UI progress? (Adds a COUNT(*) subquery) | Performance |
| Q5 | rmcp `0.2` is the latest stable tag as of mid-2025; confirm it supports `tool` macro attribute API before pinning | Dependency |
