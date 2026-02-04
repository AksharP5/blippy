# Glyph

Maintainer-first TUI for triaging GitHub Issues and PRs.

## Requirements
- Rust toolchain (Rust 1.93+ recommended)
- GitHub CLI (optional, used for auth if available)
- OS keychain available (macOS Keychain, Windows Credential Manager, Linux Secret Service)

## Quick Start
```bash
cargo run
```

Press `q` to quit.

## Authentication (v0.1)
On startup, auth is resolved in this order:
1. `gh auth token --hostname github.com`
2. OS keychain (`service=glyph`, `account=github.com`)
3. Prompt for PAT (input hidden), then store in keychain

Tokens are never written to config or db.

## Personal Access Token (PAT)
If prompted for a PAT, create one in GitHub settings:

### Fine-grained token (recommended by GitHub)
Settings → Developer settings → Personal access tokens → Fine-grained tokens → Generate new token

Suggested permissions:
- Repository metadata: Read
- Issues: Read/Write
- Pull requests: Read/Write

### Classic token (simpler)
Settings → Developer settings → Personal access tokens → Tokens (classic) → Generate new token

Suggested scopes:
- `repo` (required for private repos and full Issues/PR access)
- `read:org` (needed if you access org repositories)

## Testing Auth
```bash
gh auth status
gh auth token --hostname github.com
```

To force the PAT prompt:
- Temporarily log out of GitHub CLI: `gh auth logout`
- Remove the keychain entry for `glyph` / `github.com` using your OS keychain UI

## Tests
```bash
cargo test
```
