# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
