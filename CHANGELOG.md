# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.6](https://github.com/AksharP5/blippy/compare/v0.1.5...v0.1.6) - 2026-02-28

### Added

- *(pr)* add permission-aware pull request merge flow
- *(ui)* add transient browser-open status toast

### Fixed

- *(pr)* show merge shortcut in PR contexts
- *(ui)* label PR comment editor titles correctly
- *(ui)* clear stale terminal artifacts after browser launch

### Other

- merge main into feat/pr-merge

## [0.1.5](https://github.com/AksharP5/blippy/compare/v0.1.4...v0.1.5) - 2026-02-16

### Added

- add confirmed TUI issue creation workflow
- handle merged pull request state in filters and status
- show linked item overflow hint in issue detail
- improve linked picker context and multi-link labels
- add linked picker for multiple linked issues and pull requests

### Other

- move main tests into dedicated module
- move sync tests into dedicated module
- split github client into focused modules
- split app navigation into keyboard and mouse modules
- move store tests into dedicated module
- extract shared ui helper module
- extract app input handling module
- split sync workflows into focused modules
- split issue detail and action utility modules
- separate main action utilities
- split ui rendering into focused modules
- split main workflow into modules
- split app module into focused submodules
- extract interaction state
- extract search state
- extract navigation state
- extract metadata picker state
- extract pull request state
- extract pull request diff reset helpers
- centralize pull request state reset
- extract repo context state
- centralize store connection usage
- extract linked navigation state
- extract sync state from app
- streamline worker setup

## [0.1.4](https://github.com/AksharP5/blippy/compare/v0.1.3...v0.1.4) - 2026-02-13

### Fixed

- convert video tag to link for GitHub compatibility

### Other

- extract worker setup helpers
- standardize import ordering
- introduce context structs for high-arity functions
- remove dead code
- apply mechanical clippy fixes
- remove underscore prefixes from used function parameters
- replace images with GitHub-hosted demo videos
- add demo video to README
- update repo image
- increase PR checkout GIF to 60fps for maximum smoothness
- increase PR checkout GIF to 30fps for smoother animation
- convert PR checkout video to GIF for inline display
- update demo with new repo image and PR checkout video
- add DEMO.md with visual walkthrough screenshots

## [0.1.3](https://github.com/AksharP5/blippy/compare/v0.1.2...v0.1.3) - 2026-02-12

### Added

- *(cli)* add --version flag to display installed version

### Other

- *(readme)* use short shell installer command

## [0.1.2](https://github.com/AksharP5/blippy/compare/v0.1.1...v0.1.2) - 2026-02-12

### Added

- *(keybinds)* switch default quit shortcut to Ctrl+C

### Other

- add links
- *(ui)* drop status-copy/page shortcuts and simplify footer line

## [0.1.1](https://github.com/AksharP5/blippy/compare/v0.1.0...v0.1.1) - 2026-02-12

### Fixed

- *(build)* bundle sqlite for windows release builds

## [0.1.0](https://github.com/AksharP5/blippy/releases/tag/v0.1.0) - 2026-02-12

### Added

- *(ui)* refine issue preview context and assignee reset flow
- *(repo)* enforce metadata permissions and load label colors
- *(linked)* cache linked jumps and suggest repo assignees
- *(ui)* improve PR diff workflows and linked metadata visibility
- *(ui)* add bidirectional linked issue/pr navigation chips
- *(theme)* add configurable palettes and theme-aware UI rendering
- expand mouse controls and revise diff pan keys
- add configurable input and horizontal diff panning
- add side-aware multiline review and github thread resolution
- improve pr review ui for visual selection
- support multiline and side-aware pr review comments
- add split diff review workspace for pull requests
- add inline pull request review comment workflow
- open linked pull requests in tui first
- improve PR navigation and linked PR actions
- add full PR changes view and context-aware help
- show pull request file changes in detail view
- add pull request mode and checkout action
- add comment edit/delete controls in comments view
- simplify repo picker and improve issue pickers
- add popup label and assignee pickers
- improve repo picker and add label/assignee actions
- add etag-aware incremental issue sync
- show pending issue badges and sort closed by recency
- increase sync progress refresh responsiveness
- stream issue sync progress during refresh
- toggle through assignees
- improve issue triage flow and editor controls
- render markdown and improve issue detail UI
- enhance issue list and comments view
- improve issue metadata and comment browsing
- add dd close presets
- add dd close presets
- add dd close flow and browser open
- add comment defaults config
- add vim jumps and browser open
- add vim-style navigation hints
- scope sync to repo discovery
- add issue detail and comment polling
- add comment retention fields
- sync issues only
- refine scan and sync triggers
- add repo picker and issues view
- add repo lookup by slug
- add sync command flow
- implement sync mapping and repo sync
- add github api client and sync mapping
- index local repos
- add repo discovery scanning
- detect repo roots and remotes
- add local repo cache table
- add git remote parsing helpers
- add cache read/write layer
- add sqlite cache schema scaffold
- add cache reset command
- add auth reset command
- expose auth source for debugging
- add secure auth token resolution
- bootstrap TUI scaffold and config loading

### Fixed

- *(pr-diff)* keep expanded mode when jumping to top
- *(store)* configure sqlite to wait and use wal mode
- improve repo picker mouse targeting and scrolling
- align pr diff rows with github layout
- restore compact pr diff row layout
- surface unanchored pr comments in review mapping
- load existing pr review comments and strengthen visual highlighting
- clarify pr visual selection and side comment rendering
- sync github review threads and server-side resolution
- clarify pr diff side targeting and empty-state copy
- split linked pull request browser and tui shortcuts
- open comments when pressing enter on pr description
- reduce issue view keybind duplicates
- remove dd status hint
- simplify comments navigation and close wording
- remove focus labels and improve preview scrolling
- make issue detail focus state visually unambiguous
- apply issue state changes immediately after actions
- start sync immediately after background events
- order issues newest-first like GitHub
- persist partial issue sync on paged API failures
- issue closing
- clamp comment scroll to content
- keep comments visible while scrolling
- auto-sync on empty cache
- trigger sync on repo open

### Other

- *(release)* automate release pipeline with release-plz and cargo-dist
- *(cleanup)* normalize auth debug env and remove dead code
- *(rename)* rename project identifiers to blippy
- *(status)* preserve centered mode badge padding
- *(ui)* show full selected issue title in preview
- *(ui)* prioritize linked metadata and expand label readability
- *(ui)* move linked badges to metadata and remove ellipses
- *(ui)* refine help overlays and diff navigation cues
- change styling, keybinds, add '?'
- update to add header/better design
- *(ui)* add linked PR action chips and compact footer hints
- *(ui)* improve picker selection visibility and counts
- *(ui)* improve issue metadata hierarchy and status segmentation
- use pure black surfaces and cleaner active labels
- review theme
- refresh opencode palette and inline pr diff comments
- tighten diff layout and inline review comment contrast
- redesign pr review pane and simplify key hints
- simplify footer hints and declutter review comment labels
- apply tokyo-night palette and clearer focus states
- update UI and keybinds for pull request review
- trim keybind help to primary controls
- poll issues faster with cached incremental sync
- polish GitHub-like TUI visuals
- mention arrow key navigation
- clarify sync behavior
- add repo picker shortcuts
- add sync command
- add cache reset info
- readme
- add auth setup and testing guide
- hardcode github.com auth host
- update gitignore
- Initial commit
