# Contributing to Paker

Thank you for your interest in contributing. This guide covers local setup, workflow expectations, and releases.

## Development setup

1. Install [Node.js](https://nodejs.org/) 22+ and [Rust](https://rustup.rs/) stable.
2. Clone the repository and install dependencies:

```bash
nvm use 22
npm install
npm run tauri dev
```

`npm run tauri dev` starts the Vite dev server and Tauri app with hot reload.

### Useful commands

| Command | Purpose |
|---------|---------|
| `npm run build` | Typecheck and build the React frontend |
| `npm run test:rust` | Run Rust unit/integration tests |
| `npm run lint:rust` | Run Clippy with warnings denied |
| `npm run typecheck` | TypeScript check without emit |

## Branch naming

Use short, descriptive prefixes:

- `feat/` — new features or user-visible improvements
- `fix/` — bug fixes
- `chore/` — tooling, docs, refactors without behavior change

Example: `feat/bucket-index-search`, `fix/transfer-cancel-race`.

## Pull requests

Before opening a PR:

1. **Link an issue** when one exists (or explain the motivation in the PR description).
2. **Screenshots or screen recordings** for any UI change.
3. Run checks locally:
   - `npm run build`
   - `npm run test:rust`
4. Match existing code style — follow patterns in nearby files for naming, module layout, and error handling. Do not commit `data/`, secrets, or local editor artifacts.

Keep PRs focused; prefer several small PRs over one large mixed change.

### CI concurrency

CI uses workflow concurrency with `cancel-in-progress: true` on each branch. Rapid merges to `main` cancel any still-running CI for the same ref, so only the latest commit is validated. This is expected — if your push was superseded, check the workflow run for the newest commit instead.

## Version and release process

Versions are tracked in `src-tauri/tauri.conf.json` (source of truth for the app bundle).

1. Bump the `version` field in `src-tauri/tauri.conf.json`.
2. Sync the npm package version:

```bash
npm run version:sync
```

3. Add a dated entry under `[Unreleased]` → new version section in [CHANGELOG.md](CHANGELOG.md) ([Keep a Changelog](https://keepachangelog.com/) format).
4. Commit the version bump and changelog on `main` (or merge via PR).
5. Create and push an annotated tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Pushing a `v*` tag triggers the release workflow, which builds platform artifacts and publishes a GitHub Release.

## Code style

- **TypeScript/React** (`src/`): functional components, hooks, Tailwind + Radix UI patterns already in the tree; prefer `src/lib/` helpers and `src/types/` for shared types.
- **Rust** (`src-tauri/src/`): commands in `commands/`, S3 logic in `s3/`, persistence in `storage/`, transfers in `transfer/`, indexing in `index/`. Use `thiserror`/`anyhow` patterns consistent with existing modules.
- Avoid drive-by refactors unrelated to your change.

## Questions

Open a [GitHub issue](https://github.com/kkopanidis/paker/issues) for bugs, feature ideas, or questions before large design changes.
