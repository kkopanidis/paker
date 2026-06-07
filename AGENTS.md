# AGENTS.md

Short guide for AI coding agents working in this repository.

## Project

**Paker** is a cross-platform desktop S3 browser built with **Tauri 2** (Rust backend) and **React** (TypeScript frontend). It connects to S3-compatible storage, manages connection profiles, browses buckets/objects, and runs upload/download/copy/move transfers.

## Layout

| Path | Role |
|------|------|
| `src/` | React UI — components, hooks, types, `lib/tauri.ts` invoke wrappers |
| `src-tauri/src/commands/` | Tauri command handlers exposed to the frontend |
| `src-tauri/src/s3/` | AWS SDK client setup and S3 operations |
| `src-tauri/src/storage/` | Local persistence — secrets, caches, UI state, paths |
| `src-tauri/src/transfer/` | Transfer queue and concurrent upload/download |
| `src-tauri/src/index/` | Bucket indexing |
| `src-tauri/tauri.conf.json` | App version and bundle config (version source of truth) |

Frontend talks to Rust only through `invoke()` — add commands in `commands/`, register them in `lib.rs`, and wrap them in `src/lib/tauri.ts`.

## Do not commit

- `data/` — local portable/dev data directory
- `secrets.enc` or any credentials
- `node_modules/`, `dist/`, `src-tauri/target/`
- `.cursor/plans/` — internal planning artifacts

## Before opening a PR

```bash
nvm use 22
npm run build
npm run test:rust
```

Match existing patterns: functional React components, Tailwind + Radix UI, Rust modules by domain. Keep changes scoped; do not edit `package.json`, workflows, or release config unless explicitly asked.

## Key constraints

- Portable mode stores encrypted secrets under `./data/`; non-portable uses OS keychain + app data dir.
- Never log or expose connection secrets in UI, errors, or telemetry.
- S3 errors should surface user-actionable messages without leaking internal paths or keys.
