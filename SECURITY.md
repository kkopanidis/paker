# Security Policy

## Supported versions

Security fixes are provided for the **latest release on `main`**. Older tagged releases are not actively supported unless noted in a security advisory.

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Earlier releases | No |

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report security issues through [GitHub private security advisories](https://github.com/kkopanidis/paker/security/advisories/new) for this repository. Include:

- A clear description of the issue and impact
- Steps to reproduce
- Affected version(s)
- Any suggested fix or mitigation, if you have one

We aim to acknowledge reports within a few business days and will coordinate disclosure once a fix is available.

## In scope

Reports we treat as security-relevant for Paker include:

- **Credential storage** — encryption of `secrets.enc`, OS keychain integration, key derivation in portable mode, or leakage of connection secrets through logs, IPC, or the UI
- **Portable mode** — unsafe handling of the portable data directory, predictable encryption keys, or path traversal when reading/writing portable data
- **S3 operations** — bugs in the app that cause unauthorized access, data exposure, or integrity failures beyond what the user's cloud credentials already allow (e.g. sending secrets to unintended endpoints, failing to validate TLS when configured)
- **Local filesystem IPC** — bypass of `LocalFsScope` via upload, download, export, or preview-open commands (reading or writing paths outside allowed directories)
- **File permissions** — sensitive files (`secrets.enc`, `connections.json`, `ui_state.json`) world-readable on Unix when the app should restrict access to the owner. On Unix, Paker writes these with mode `0o600` (owner read/write only). On Windows, portable `./data/` files rely on NTFS ACLs inherited from the parent directory; the app does not set a Unix-style permission mask.
- **Portable KDF** — `secrets.enc` encrypted with only the legacy static KDF (v1) can be decrypted offline by anyone with the file. When a per-host keyring seed is present, portable mode uses KDF v2 (Argon2id over keyring seed + static material); in that case `secrets.enc` alone is **not** sufficient to recover credentials without the seed stored in the OS keychain on the same machine.

## Out of scope

The following are generally **not** considered Paker application vulnerabilities:

- **User IAM misconfiguration** — overly permissive bucket policies, shared access keys, or account compromise outside the app
- Issues that require physical access to an unlocked machine with an already-authenticated session
- Denial of service against remote S3 endpoints caused by normal API usage patterns
- Vulnerabilities in third-party dependencies without a demonstrable exploit path in Paker (we still appreciate responsible reports and will track upstream fixes)

Thank you for helping keep Paker and its users safe.
