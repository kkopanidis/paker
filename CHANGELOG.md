# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.1] - 2026-06-08

### Changed

- README with UI screenshots and AI-assisted development contribution notes

## [0.7.0] - 2026-06-08

### Added

- Optional master key vault — encrypt connection secrets behind a master password with idle and blur auto-lock, plus OS-authenticated reset

### Fixed

- Connection switching errors when changing active profiles
- Windows Hello authentication for vault reset (IAsyncOperation::join)

### Changed

- Updated Node.js (24) and dependency versions across frontend and Rust toolchain

## [0.6.0] - 2026-06-08

### Added

- In-app update awareness via GitHub releases
- Portable secrets KDF v2 with automatic migration from legacy format

### Changed

- Security hardening — filesystem path scoping on uploads, downloads, and exports; CSP; keychain-primary secrets storage; structured IPC error messages
- CI and release workflow improvements

## [0.5.0] - 2026-06-07

### Added

- Local S3 object cache for faster repeat browsing
- Bucket indexer with search and background indexing
- Inspection UX improvements — object details, bucket properties, and size calculation dialogs

### Changed

- Refined browser toolbar, filter bar, and keyboard shortcuts

## [0.4.0] - 2026-05-01

### Added

- Phase 1 two-pane explorer — optional local filesystem panel alongside remote S3 browser
- Concurrent transfer queue with live progress
- Copy and move operations between prefixes and buckets

### Changed

- Resizable multi-panel layout with connections, buckets, local, and remote panes

## [0.1.0] - 2026-03-15

### Added

- Phase 0 foundation — Tauri 2 + React desktop app for S3-compatible storage
- Connection profiles for AWS S3, MinIO, and custom endpoints
- Bucket browse and object listing with breadcrumbs
- File operations — upload, download, delete, rename, create folder
- Transfer queue for uploads and downloads
- Portable mode via `portable.txt` or `PAKER_PORTABLE=1`

[Unreleased]: https://github.com/kkopanidis/paker/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/kkopanidis/paker/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/kkopanidis/paker/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/kkopanidis/paker/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/kkopanidis/paker/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/kkopanidis/paker/compare/v0.1.0...v0.4.0
[0.1.0]: https://github.com/kkopanidis/paker/releases/tag/v0.1.0
