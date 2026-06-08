# Contributing to Paker

Thank you for your interest in contributing. This guide covers local setup, workflow expectations, and releases.

## Development setup

1. Install [Node.js](https://nodejs.org/) 24+ and [Rust](https://rustup.rs/) stable.
2. Clone the repository and install dependencies:

```bash
nvm use 24
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
   - `npm run lint`
   - `npm run test:all`
   - `npm run build`
4. Match existing code style — follow patterns in nearby files for naming, module layout, and error handling. Do not commit `data/`, secrets, or local editor artifacts.

Keep PRs focused; prefer several small PRs over one large mixed change.

### AI-assisted work

AI-assisted contributions are welcome when the contributor owns the result. Review generated code before submitting it, keep the final diff understandable, cite trade-offs where relevant, and run the same checks as any other change.

Do not commit private prompts, agent transcripts, local editor configuration, secrets, or other machine-local artifacts.

### CI concurrency

CI uses workflow concurrency with `cancel-in-progress: true` on each branch. Rapid merges to `main` cancel any still-running CI for the same ref, so only the latest commit is validated. This is expected — if your push was superseded, check the workflow run for the newest commit instead.

## Version and release process

**The git tag is the source of truth for a release.** The release workflow reads `vX.Y.Z` from the pushed tag, writes that version into the project files, builds, and uploads all artifacts to that tag's GitHub Release.

### Cut a release

1. Merge the changes you want to ship to `main`.
2. Add a dated entry under `[Unreleased]` → new version section in [CHANGELOG.md](CHANGELOG.md) ([Keep a Changelog](https://keepachangelog.com/) format).
3. Create and push a tag on the commit to ship:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

Pushing a `v*` tag triggers the [release workflow](.github/workflows/release.yml), which syncs the tag version into `src-tauri/tauri.conf.json` (and related files), builds platform artifacts, and publishes a GitHub Release when all jobs finish.

To rebuild an existing tag without re-pushing, use **Actions → Release → Run workflow** and enter the tag (e.g. `v0.6.0`).

### After a release (optional housekeeping)

Bump `src-tauri/tauri.conf.json` to the next development version on `main`, then sync:

```bash
npm run version:sync
```

This keeps local and CI dev builds on the upcoming version; it does not affect binaries already shipped under the release tag.

### Local version sync

`npm run version:sync` propagates the version from `src-tauri/tauri.conf.json` to `package.json`, `package-lock.json`, and `Cargo.toml`. Use this after editing the dev version on `main`, not when cutting a release (CI sets the version from the tag).

## Code style

- **TypeScript/React** (`src/`): functional components, hooks, Tailwind + Radix UI patterns already in the tree; prefer `src/lib/` helpers and `src/types/` for shared types.
- **Rust** (`src-tauri/src/`): commands in `commands/`, S3 logic in `s3/`, persistence in `storage/`, transfers in `transfer/`, indexing in `index/`. Use `thiserror`/`anyhow` patterns consistent with existing modules.
- Avoid drive-by refactors unrelated to your change.

## Questions

Open a [GitHub issue](https://github.com/kkopanidis/paker/issues) for bugs, feature ideas, or questions before large design changes.
