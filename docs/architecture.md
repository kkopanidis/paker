# Paker architecture

Paker is a Tauri 2 desktop app: a React webview UI talks to a Rust backend over a narrow, audited API surface. This document records the trust boundary, capability grants, and frontend–backend integration rules.

## Trust boundary

The **webview** (React + Vite) is untrusted UI code. It must not read arbitrary paths, open native dialogs, or reach the network or filesystem directly. All privileged work—S3 calls, local file I/O, secrets, caches, transfers, and native file/folder pickers—runs in **Rust** and is exposed only through registered Tauri commands (`invoke`) or the small set of core/plugin APIs listed in the boundary audit below.

The **Rust backend** holds credentials (keychain or encrypted portable store), validates inputs, scopes paths, and emits progress events. Custom `invoke` handlers are allowed for all registered commands by default; plugin APIs require explicit capability permissions.

### Portable mode and `secrets.enc`

In portable mode (`portable.txt` or `PAKER_PORTABLE=1`), connection secrets live in `./data/secrets.enc`, encrypted with AES-256-GCM. Key derivation uses Argon2id:

| KDF | When | Material |
|-----|------|----------|
| **v2** (preferred) | OS keychain / secret service available | 32-byte random seed in keychain entry `paker` / `portable-file-key`, concatenated with static app material, salt `paker-portable-salt-v2` |
| **v1** (legacy fallback) | Keyring unavailable (CI, headless) | Static material only, salt `paker-portable-salt-v1` |

**Limitation:** Legacy v1 blobs are offline-decryptable given `secrets.enc`. With v2, `secrets.enc` alone is not enough—the per-host keyring seed on the same machine is also required. On read, v1 blobs are transparently re-encrypted to v2 when the keyring seed is available.

Sensitive files under `./data/` use mode `0o600` on Unix. On Windows portable installs, access control follows NTFS ACLs on the data directory rather than a Unix permission bitmask.

```
┌─────────────────────────────────────┐
│  Webview (React)                    │
│  • UI state, tables, dialogs (Radix)│
│  • invoke() only via lib/tauri.ts   │
│  • listen() for progress events     │
│  • convertFileSrc, drag-drop, opener│
└──────────────┬──────────────────────┘
               │ IPC (invoke / events)
┌──────────────▼──────────────────────┐
│  Rust (Tauri)                       │
│  • S3 (aws-sdk), transfers, index   │
│  • std::fs + app data paths         │
│  • rfd native file/folder dialogs   │
│  • Secrets, SQLite caches, UI state │
└─────────────────────────────────────┘
```

### Rule for new features

**Filesystem and native-dialog features must go through Rust commands**, not frontend Tauri plugins (`@tauri-apps/plugin-fs`, `@tauri-apps/plugin-dialog`, etc.). Add a handler in `src-tauri/src/commands/`, register it in `lib.rs`, and wrap it in `src/lib/tauri.ts`. Do not add `fs`, `dialog`, `http`, or `shell` plugin permissions to capabilities without an explicit security review.

## Why Paker omits `tauri-plugin-dialog` and `tauri-plugin-fs`

Paker deliberately does **not** use `@tauri-apps/plugin-dialog` or `@tauri-apps/plugin-fs` (nor their Rust counterparts). Instead:

| Concern | Approach |
|--------|----------|
| File/folder pickers | [`rfd`](https://docs.rs/rfd) in Rust (`pick_upload_files`, `pick_local_folder`, save dialogs in `s3_ops`, `local_fs`, `bucket_index`) |
| Local directory listing | `std::fs` in `commands/local_fs.rs` with path validation |
| Upload/download/copy | Rust transfer layer; webview receives paths only as opaque strings from commands |
| App data, secrets, caches | `std::fs` under controlled paths in `src-tauri/src/storage/` |

**Rationale:** Keeping dialogs and filesystem access in Rust preserves a single trust boundary. The webview never gets blanket read/write scopes to the host filesystem; each operation is implemented and auditable in one place. Native `rfd` dialogs also match desktop UX without granting the webview plugin-level FS or dialog permissions.

## Capabilities inventory

Defined in `src-tauri/capabilities/default.json` for the `main` window.

### Granted

| Permission | What it enables |
|------------|-----------------|
| **`core:default`** | Bundled core subsets: `app`, `event`, `image`, `menu`, `path`, `resources`, `tray`, `webview`, `window`. In practice this covers **read-only** window/webview metadata, **event** `listen` / `unlisten` / `emit`, path helpers (`join`, `normalize`, etc.), and similar baseline desktop APIs. It does **not** include mutating window APIs (e.g. `hide`, `setTitle`) unless added explicitly. |
| **`opener:default`** | Open `mailto:`, `tel:`, `https://`, `http://` URLs in the default app; **reveal** items in the system file explorer (`reveal_item_in_dir`). Does **not** include unrestricted `open_path`—that requires `opener:allow-open-path` with a scoped `path` allow list if needed. |

### Not granted

The main capability does **not** include:

| Plugin / permission family | Status |
|----------------------------|--------|
| `fs:*` (`tauri-plugin-fs`) | Not installed, not in capabilities |
| `dialog:*` (`tauri-plugin-dialog`) | Not installed, not in capabilities |
| `http:*` (`tauri-plugin-http`) | Not installed, not in capabilities |
| `shell:*` (`tauri-plugin-shell`) | Not installed, not in capabilities |

S3 HTTP traffic runs in Rust via `aws-sdk-s3`, not the Tauri HTTP plugin.

## Desktop-only

Paker targets **desktop only** (macOS, Windows, Linux). The crate retains `mobile_entry_point` and mobile library types for Tauri template compatibility, but there is no mobile UI or release pipeline. Do not add `tauri-plugin-fs` / `tauri-plugin-dialog` without updating this document.

## Deferred (future phases)

- **Code signing** — production distribution hardening (platform-specific).
- **`tauri-plugin-updater`** — in-app updates; not integrated yet (GitHub release banner only).

## Boundary audit

Audit date: **2026-06-07**. Method: repository grep over `package.json`, `src/`, and `src-tauri/`.

### Forbidden frontend plugins

| Check | Result |
|-------|--------|
| `@tauri-apps/plugin-fs` in `package.json` | **Absent** |
| `@tauri-apps/plugin-dialog` in `package.json` | **Absent** |
| `@tauri-apps/plugin-http` in `package.json` | **Absent** |
| `@tauri-apps/plugin-fs` / `plugin-dialog` / `plugin-http` imports in `src/` | **None** |
| `tauri-plugin-fs` / `tauri-plugin-dialog` / `tauri-plugin-http` in `src-tauri/Cargo.toml` | **None** |

Installed Tauri JS deps: `@tauri-apps/api`, `@tauri-apps/plugin-opener` only.

### `invoke()` centralization

All `invoke()` calls live in **`src/lib/tauri.ts`** (sole file importing `invoke` from `@tauri-apps/api/core`). Components and hooks import named functions from `@/lib/tauri` instead of calling `invoke` directly.

Registered commands (51): `list_connections`, `get_connection`, `save_connection`, `delete_connection`, `test_connection`, `list_buckets`, `verify_bucket`, `read_list_cache`, `list_objects`, `calculate_prefix_size`, `get_bucket_metadata`, `pick_upload_files`, `upload_files`, `download_files`, `delete_objects`, `rename_object`, `create_folder`, `head_object`, `check_objects_exist`, `copy_objects`, `move_objects`, `cancel_transfer`, `pause_transfer`, `resume_transfer`, `list_local_dir`, `get_home_dir`, `pick_local_folder`, `get_parent_path`, `get_last_local_dir`, `set_last_local_dir`, `get_transfer_settings`, `get_full_ui_state`, `get_connection_nav`, `set_connection_nav`, `get_bookmarks`, `add_bookmark`, `remove_bookmark`, `get_ui_preferences`, `set_ui_preferences`, `get_panel_layout`, `set_panel_layout`, `presign_object`, `preview_object_to_cache`, `get_bucket_index_status`, `start_bucket_index`, `pause_bucket_index`, `resume_bucket_index`, `cancel_bucket_index`, `search_bucket_index`, `export_bucket_index_csv`.

### Non-`invoke` Tauri APIs in `src/`

These are the only direct `@tauri-apps/*` usages outside `lib/tauri.ts`:

| API | Module | File(s) | Purpose |
|-----|--------|---------|---------|
| `listen` | `@tauri-apps/api/event` | `hooks/useTransfers.ts` | `transfer-progress` events |
| `listen` | `@tauri-apps/api/event` | `hooks/usePrefixSize.ts` | `prefix-size-progress` events |
| `listen` | `@tauri-apps/api/event` | `hooks/useBucketIndex.ts` | `bucket-index-progress` events |
| `listen` | `@tauri-apps/api/event` | `components/browser/SizeCalculationDialog.tsx` | `prefix-size-progress` while dialog open |
| `convertFileSrc` | `@tauri-apps/api/core` | `components/browser/ObjectDetails.tsx` | Asset URLs for cached preview files |
| `getCurrentWebview().onDragDropEvent` | `@tauri-apps/api/webview` | `components/layout/AppShell.tsx` | OS drag-and-drop onto window (`dragDropEnabled: true` in `tauri.conf.json`) |
| `openPath` | `@tauri-apps/plugin-opener` | `components/layout/AppShell.tsx` | Open preview file in default external app |

No other `@tauri-apps/` imports exist under `src/`.

### Re-audit

To repeat this audit:

```bash
# Forbidden plugins
rg '@tauri-apps/plugin-(fs|dialog|http)' package.json src/
rg 'tauri-plugin-(fs|dialog|http)' src-tauri/Cargo.toml

# invoke must be only in tauri.ts
rg 'invoke\(' src/
rg 'from ["'\'']@tauri-apps/' src/
```
