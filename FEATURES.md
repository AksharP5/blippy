# Features

blippy brings core GitHub issue and PR workflows into a terminal-first interface.

See the [feature demo](DEMO.md) for a visual walkthrough of these capabilities in action.

## Repository Discovery and Sync

- Scans local git repositories and indexes GitHub remotes; searches four directory levels inside each repository for nested repositories and finds registered nested worktrees at any depth, subject to scan depth limits and exclusions
- Recognizes GitHub SSH remotes with explicit ports, including `ssh://git@ssh.github.com:443/owner/repo.git`
- Supports direct `owner/repo` repo context from the current working tree
- Keeps a local SQLite cache for fast navigation
- `blippy sync` updates discovered repositories and remotes
- Broken local repositories are reported while scanning continues for healthy repositories
- Stalled GitHub API requests time out after 30 seconds
- Background sync avoids duplicate requests during repository and issue navigation
- Background refresh preserves the reading position of the selected issue and the offset within a surviving comment, even when earlier comments change
- Renamed and transferred repositories retain their cached issues and linked items
- Recreated repositories sync with a fresh cache and clear old context and pending actions on sync completion; late action, metadata, and linked-item results cannot adopt or change the replacement repository
- Cache write failures report successful GitHub actions and keep the terminal session open

## Issues and Pull Requests in One Flow

- Toggle between issues and pull requests from the same list view
- Open/closed tabs and assignee filtering
- Distinguishes merged pull requests from closed pull requests
- Fast list navigation with keyboard-first controls
- Issue and PR detail views with context-aware panes
- HTML blocks in Markdown bodies remain readable as literal text
- State and metadata updates prevent overlapping actions on the same item
- Completed state and metadata actions preserve the selected issue or PR when it remains in the list
- Copy repository, issue, and pull request URLs to the system clipboard
- On supported Unix terminals, bracketed paste inserts text without submitting comments or triggering shortcuts, preserving line breaks in bodies

## Issue Creation in TUI

- Create issues directly in the terminal from issue contexts
- Title/body editor with confirmation dialog before submission
- Opens the newly created issue after success when browsing issues; active editors, pickers, search, and PR review keep their context

## Linked Issue/PR Navigation

- Jump from an issue to its linked PR (and back)
- Jump from a PR to its linked issue (and back)
- Includes issues closed by the pull request
- Open linked items in TUI or browser
- Linked-item lookups follow the selected row when scrolling through the issue or PR list
- Linked metadata is cached to reduce repeated lookups; partial results remain usable, and refresh retries incomplete or failed lookups
- Delayed results cache links without interrupting editors, pickers, search, help, or navigation to another item
- Opening a linked item clears list search and assignee filters to reveal the target

## Pull Request Review Workspace

- View changed files and diff, with option for checkout
- PR checkout uses the selected GitHub remote and local repository
- Merge pull requests directly from the review/detail flow
- Split or expanded diff review modes
- Horizontal diff panning for long lines
- Mark files viewed/unviewed, ignoring repeat toggles until GitHub responds
- Visual multiline range selection within one diff hunk for review comments, including automatic side fallback for added-only or deleted-only selections
- Review comments keep the commit shown when the editor opened. If the PR head or base changes while files load, refresh with `r` to retry.

## Comments and Review Threads

- Add, edit, and delete issue comments
- Read long comments with arrow-key or mouse-wheel scrolling; `j`/`k` still jump between comments
- Add, edit, and delete inline PR review comments
- Resolve or reopen PR review threads
- Navigate comment threads on selected diff lines

## Metadata Editing and Permission Awareness

- Edit labels and assignees for issues/PRs from the TUI
- Merge actions are permission-aware and only enabled for authorized repos
- Label and assignee pickers with inline filtering; background suggestions preserve the highlighted option and checked selections
- Open metadata pickers keep mouse input inside the picker, leaving background issues and repository navigation inactive
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
- Scrollable help overlay with keyboard and mouse wheel controls; input stays in help until it is dismissed
- Configurable keybindings via `~/.config/blippy/keybinds.toml`
- Configurable close-comment presets in `~/.config/blippy/config.toml`
