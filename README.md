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

To see which auth source was used (development only):
```bash
GLYPH_AUTH_DEBUG=1 cargo run
```

To force the PAT prompt:
- Temporarily log out of GitHub CLI: `gh auth logout`
- Remove the keychain entry for `glyph` / `github.com` using your OS keychain UI

To reset stored auth from the CLI:
```bash
glyph auth reset
```

## Tests
```bash
cargo test
```

## Cache
- Cache lives in your OS user data directory as `glyph.db`
- Reset cache: `glyph cache reset`

## Sync
- Run `glyph sync` to scan local repos and cache GitHub remotes
- Issues are fetched when you open a repo in the TUI

## Navigation
- Ctrl+G: open repo picker
- Ctrl+R: rescan repos
- j/k or arrow keys: move, gg/G: top/bottom, Enter: open
- r: refresh issues/comments
- o: open in browser
- dd: close issue with preset
- c: open full comments from issue detail
- b or Esc: back from issue detail/comments

## Comment Defaults
Configure close/comment presets in `config.toml`:
```toml
[[comment_defaults]]
name = "close_default"
body = "Closing this issue as resolved."
```
