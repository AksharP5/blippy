# blippy

GitHub in your terminal.

blippy is a keyboard-first TUI for GitHub issues and pull requests.

## Requirements

- Rust toolchain (`1.93+` recommended) for source builds
- GitHub CLI (`gh`) is heavily recommended for the best workflow (auth fallback, PR checkout, and smoother GitHub integration)
- OS keychain support (macOS Keychain, Windows Credential Manager, Linux Secret Service)

## Install

### npm (global)

```bash
npm i -g blippy
```

### Homebrew

```bash
brew install AksharP5/tap/blippy
```

### Shell installer (macOS/Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/AksharP5/blippy/releases/latest/download/blippy-installer.sh | sh
```

### PowerShell installer (Windows)

```powershell
irm https://github.com/AksharP5/blippy/releases/latest/download/blippy-installer.ps1 | iex
```

### Build from source

```bash
cargo install --git https://github.com/AksharP5/blippy
```

## CLI Commands

- `blippy`: launch the TUI
- `blippy sync`: scan local repos and cache GitHub remotes
- `blippy auth reset`: remove stored auth token from keychain
- `blippy cache reset`: remove local cache database

## What You Can Do

- Browse and manage issues and pull requests
- Open linked issues/PRs in TUI or browser
- Review PR diffs with inline comments and thread resolution
- Edit labels and assignees (when repository permissions allow)
- Customize themes, keybindings, and close-comment presets

See `FEATURES.md` for a full feature breakdown.

## Keyboard and Mouse

- blippy prioritizes keyboard workflows for reliability
- Mouse/trackpad support exists, but it can be finicky
- Full key reference: `KEYBINDS.md`

## Configuration

- Config file: `~/.config/blippy/config.toml`
- Keybind overrides: `~/.config/blippy/keybinds.toml`
- Example keybind file: `keybinds.example.toml`

Theme example:

```toml
theme = "midnight"
```

Available built-in themes:

- `github_dark` (default)
- `midnight`
- `graphite`

Comment preset example:

```toml
[[comment_defaults]]
name = "close_default"
body = "Closing this issue as resolved."
```

## Documentation

- Authentication and PAT setup: `AUTH.md`
- Feature guide: `FEATURES.md`
- Key reference: `KEYBINDS.md`
- Contributing guide: `CONTRIBUTING.md`
- Release history: `CHANGELOG.md` 
