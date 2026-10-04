# Features

blippy brings core GitHub issue and PR workflows into a terminal-first interface.

See the [feature demo](DEMO.md) for a visual walkthrough of these capabilities in action.

## Repository Discovery and Sync

- Scans local git repositories, including nested worktrees, and indexes GitHub remotes
- Recognizes GitHub SSH remotes with explicit ports
- Supports direct `owner/repo` repo context from the current working tree
- Keeps a local SQLite cache for fast navigation
- `blippy sync` updates discovered repositories and remotes
- Broken local repositories are reported while scanning continues for healthy repositories
- Stalled GitHub API requests time out after 30 seconds
- Background sync avoids duplicate requests during repository and issue navigation
- Renamed and transferred repositories retain their cached issues and linked items

## Issues and Pull Requests in One Flow

- Toggle between issues and pull requests from the same list view
- Open/closed tabs and assignee filtering
- Distinguishes merged pull requests from closed pull requests
- Fast list navigation with keyboard-first controls
- Issue and PR detail views with context-aware panes
- State and metadata updates prevent overlapping actions on the same item
- Copy repository, issue, and pull request URLs to the system clipboard

## Issue Creation in TUI

- Create issues directly in the terminal from issue contexts
- Title/body editor with confirmation dialog before submission
- Automatically navigates to the newly created issue after success

## Linked Issue/PR Navigation

- Jump from an issue to its linked PR (and back)
- Jump from a PR to its linked issue (and back)
- Includes issues closed by the pull request
- Open linked items in TUI or browser
- Linked metadata is cached to reduce repeated lookups
- Opening a linked item clears list search and assignee filters to reveal the target

## Pull Request Review Workspace

- View changed files and diff, with option for checkout
- PR checkout uses the selected GitHub remote and local repository
- Merge pull requests directly from the review/detail flow
- Split or expanded diff review modes
- Horizontal diff panning for long lines
- Mark files viewed/unviewed, ignoring repeat toggles until GitHub responds
- Visual multiline range selection within one diff hunk for review comments

## Comments and Review Threads

- Add, edit, and delete issue comments
- Add, edit, and delete inline PR review comments
- Resolve or reopen PR review threads
- Navigate comment threads on selected diff lines

## Metadata Editing and Permission Awareness

- Edit labels and assignees for issues/PRs from the TUI
- Merge actions are permission-aware and only enabled for authorized repos
- Label and assignee pickers with inline filtering
- Editing is permission-aware and checks repo capabilities

## Search and Filters

- Repository search by owner/repo/path/remote
- Issue/PR search with GitHub-style qualifiers
- Supported qualifiers include:
  - `is:open`, `is:closed`, `is:merged`
  - `label:<name>`
  - `assignee:<user>`, `assignee:none`
  - `#<number>`

## Themes and Customization

- Built-in themes: `github_dark`, `midnight`, `graphite`
- Configurable keybindings via `~/.config/blippy/keybinds.toml`
- Configurable close-comment presets in `~/.config/blippy/config.toml`
