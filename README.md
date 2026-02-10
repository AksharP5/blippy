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
- / in repo picker: search repo groups (owner/repo/path/remote)
- Ctrl+h/j/k/l: switch focus between panes (issues list/preview, issue description/recent comments)
- j/k or arrow keys: move or scroll focused pane
- Ctrl+u / Ctrl+d: page up/down in focused pane
- gg/G: jump top/bottom in focused pane
- f: cycle issue filter (open/closed)
- 1/2: switch issue tab (open/closed), or use `[` and `]`
- a/A: cycle assignee filter (all/unassigned/users)
- /: search issues by number/title/body/labels/assignees (Enter keep, Esc clear)
- m: add comment to selected issue
- l: edit issue labels (comma-separated)
- s: edit issue assignees
- u: reopen selected closed issue
- Enter: open selected issue
- r: refresh issues/comments
- o: open in browser
- dd: close issue with preset
- c: open full comments from issue detail
- n/p: jump next/previous comment in full comments view
- b or Esc: back from issue detail/comments
- comment editor: `Enter` submit, `Shift+Enter` newline (`Ctrl+j` fallback)

Search supports simple GitHub-like qualifiers:
- `is:open` or `is:closed`
- `label:bug`
- `assignee:alex`
- `assignee:none` for unassigned
- `#123` for exact issue number

## Comment Defaults
Configure close/comment presets in `config.toml`:
```toml
[[comment_defaults]]
name = "close_default"
body = "Closing this issue as resolved."
```
