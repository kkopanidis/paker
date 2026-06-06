# Paker

A modern, cross-platform desktop browser for S3-compatible storage. Built with Tauri 2, React, and the Rust AWS SDK.

## Features

- **Connection profiles** for AWS S3, MinIO, Cloudflare R2, DigitalOcean Spaces, Backblaze B2, and custom endpoints
- **Three-panel file manager** — connections, buckets, and objects
- **Full file operations** — upload, download, delete, rename, create folder
- **Transfer queue** with live progress for uploads and downloads
- **Portable mode** — place `portable.txt` next to the executable (or set `PAKER_PORTABLE=1`) to store all data in a local `./data/` folder
- **No admin install required** — run from any writable directory

## Requirements

- [Node.js](https://nodejs.org/) 22+
- [Rust](https://rustup.rs/) stable
- Platform WebView (WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux)

## Development

```bash
npm install
npm run tauri dev
```

## Build (portable artifacts)

```bash
npm run tauri build
```

Outputs (under `src-tauri/target/release/bundle/`):

| Platform | Artifact |
|----------|----------|
| macOS | `.app` (zip or tar for portable use) |
| Windows | `.exe` in NSIS or standalone bundle |
| Linux | AppImage |

## Portable usage

1. Copy the built app to any folder (USB drive, Desktop, etc.)
2. Create an empty `portable.txt` file next to the executable
3. Launch — connections and encrypted secrets are stored in `./data/`

Without `portable.txt`, data lives in the OS user data directory and secrets use the system keychain when available.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| F5 | Refresh object list |
| Delete | Delete selected objects |
| Ctrl/Cmd + U | Upload files |

## License

MIT
