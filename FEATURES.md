# Features

blippy brings core GitHub issue and PR workflows into a terminal-first interface.

## Repository Discovery and Sync

- Scans local git repositories and indexes GitHub remotes
- Supports direct `owner/repo` repo context from the current working tree
- Keeps a local SQLite cache for fast navigation
- `blippy sync` updates discovered repositories and remotes

## Issues and Pull Requests in One Flow

- Toggle between issues and pull requests from the same list view
- Open/closed tabs and assignee filtering
- Fast list navigation with keyboard-first controls
- Issue and PR detail views with context-aware panes

## Linked Issue/PR Navigation

- Jump from an issue to its linked PR (and back)
- Jump from a PR to its linked issue (and back)
- Open linked items in TUI or browser
- Linked metadata is cached to reduce repeated lookups

## Pull Request Review Workspace

- View changed files and diff, with option for checkout
- Split or expanded diff review modes
- Horizontal diff panning for long lines
- Mark files viewed/unviewed
- Visual multiline range selection for review comments

## Comments and Review Threads

- Add, edit, and delete issue comments
- Add, edit, and delete inline PR review comments
- Resolve or reopen PR review threads
- Navigate comment threads on selected diff lines

## Metadata Editing and Permission Awareness

- Edit labels and assignees for issues/PRs from the TUI
- Label and assignee pickers with inline filtering
- Editing is permission-aware and checks repo capabilities

## Search and Filters

- Repository search by owner/repo/path/remote
- Issue/PR search with GitHub-style qualifiers
- Supported qualifiers include:
  - `is:open`, `is:closed`
  - `label:<name>`
  - `assignee:<user>`, `assignee:none`
  - `#<number>`

## Themes and Customization

- Built-in themes: `github_dark`, `midnight`, `graphite`
- Configurable keybindings via `~/.config/blippy/keybinds.toml`
- Configurable close-comment presets in `~/.config/blippy/config.toml`
