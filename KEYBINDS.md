# Keybindings

blippy is keyboard-first. Mouse/trackpad support exists, but it can be finicky and keyboard workflows are prioritized.

## Global

- `q`: Quit
- `?`: Toggle help overlay
- `Ctrl+g`: Open repo picker
- `Ctrl+h` / `Ctrl+l`: Switch pane focus in split views
- `j` / `k` (or arrow down/up): Move selection or scroll
- `gg` / `G`: Jump to top/bottom

## Repo Picker

- `/`: Start repository search
- `Enter`: Open selected repository
- `Ctrl+r`: Rescan repositories

Search mode:

- `Enter`: Keep search text and exit search mode
- `Esc`: Clear search text and exit search mode
- `Ctrl+u`: Clear search text

## Issues / Pull Requests List

- `Enter`: Open selected item
- `Tab` / `Shift+Tab`: Cycle open/closed tab
- `1` / `2`: Jump directly to open/closed tab
- `p`: Toggle issues/PR mode
- `a`: Cycle assignee filter
- `Ctrl+a`: Reset assignee filter to all
- `/`: Start issue/PR search
- `r`: Refresh list
- `m`: Add comment
- `l`: Edit labels
- `Shift+A`: Edit assignees
- `u`: Reopen selected closed item
- `dd`: Close selected item via preset flow
- `o`: Open selected item in browser
- `Shift+P`: Open linked PR/issue in TUI
- `Shift+O`: Open linked PR/issue in browser
- `v`: Checkout selected PR locally (`gh pr checkout`)

Search mode:

- `Enter`: Keep query and exit search mode
- `Esc`: Clear query and exit search mode
- `Ctrl+u`: Clear query

## Issue Detail

- `Ctrl+h` / `Ctrl+l`: Switch focus between description and recent comments
- `Enter`: Open focused pane action (comments or PR review when applicable)
- `c`: Open full comments view
- `m`: Add comment
- `l`: Edit labels
- `Shift+A`: Edit assignees
- `u`: Reopen selected closed item
- `dd`: Close selected item via preset flow
- `o`: Open in browser
- `Shift+P`: Open linked PR/issue in TUI
- `Shift+O`: Open linked PR/issue in browser
- `r`: Refresh issue/comments
- `b` or `Esc`: Back

## Issue Comments View

- `j` / `k`: Move between comments
- `m`: Add comment
- `e`: Edit selected comment
- `x`: Delete selected comment
- `l`: Edit labels
- `Shift+A`: Edit assignees
- `u`: Reopen selected closed item
- `dd`: Close selected item via preset flow
- `o`: Open in browser
- `Shift+P`: Open linked PR/issue in TUI
- `Shift+O`: Open linked PR/issue in browser
- `r`: Refresh issue/comments
- `b` or `Esc`: Back

## Pull Request Review View (`Files`)

- `Ctrl+h` / `Ctrl+l`: Focus files pane or diff pane
- `j` / `k`: Move selected file
- `Enter`: Expand diff pane to full width
- `w`: Toggle file viewed/unviewed on GitHub
- `r`: Refresh PR data
- `v`: Checkout PR locally
- `b` or `Esc`: Back (or return to split diff if expanded)

## Pull Request Review View (`Diff`)

- `Ctrl+h` / `Ctrl+l`: Focus files pane or diff pane
- `j` / `k`: Move selected diff row
- `Enter`: Expand to full diff (or return to split when expanded)
- `c`: Collapse/expand selected hunk
- `[` / `]`: Horizontal pan left/right
- `0`: Reset horizontal pan
- `h` / `l`: Select old/new diff side for commenting
- `Shift+V`: Toggle visual range selection
- `m`: Add inline review comment
- `e`: Edit selected inline review comment
- `x`: Delete selected inline review comment
- `Shift+R`: Resolve/reopen selected review thread
- `n` / `p`: Cycle line comments on current diff row
- `r`: Refresh PR data
- `v`: Checkout PR locally
- `b` or `Esc`: Return to split diff (if expanded) or back

## Label / Assignee Pickers

- Type to filter options
- `j` / `k`: Move option selection
- `Space`: Toggle current option
- `Enter`: Apply selection
- `Ctrl+u`: Clear filter text
- `Esc`: Cancel

## Close Preset Picker

- `j` / `k`: Move selection
- `Enter`: Select preset action
- `Esc`: Cancel

## Text Editors (comment body / preset body)

- `Enter`: Submit
- `Shift+Enter`, `Alt+Enter`, or `Ctrl+j`: Insert newline
- `Esc`: Cancel

## Search Qualifiers

- `is:open`, `is:closed`
- `label:<name>`
- `assignee:<user>`
- `assignee:none`
- `#<number>`

## Configurable Default Bindings

All entries below can be overridden in `~/.config/blippy/keybinds.toml` (or under `[keybinds]` in `~/.config/blippy/config.toml`).

| Action | Default |
| --- | --- |
| `quit` | `q` |
| `clear_and_repo_picker` | `ctrl+g` |
| `repo_search` | `/` |
| `issue_search` | `/` |
| `cycle_issue_filter` | `tab` |
| `toggle_work_item_mode` | `p` |
| `cycle_assignee_filter` | `a` |
| `issue_filter_open` | `1` |
| `issue_filter_closed` | `2` |
| `refresh` | `r` |
| `jump_prefix` | `g` |
| `jump_bottom` | `shift+g` |
| `open_comments` | `c` |
| `add_comment` | `m` |
| `toggle_file_viewed` | `w` |
| `collapse_hunk` | `c` |
| `edit_comment` | `e` |
| `delete_comment` | `x` |
| `resolve_thread` | `shift+r` |
| `next_line_comment` | `n` |
| `prev_line_comment` | `p` |
| `review_side_left` | `h` |
| `review_side_right` | `l` |
| `visual_mode` | `shift+v` |
| `edit_labels` | `l` |
| `edit_assignees` | `shift+a` |
| `reopen_issue` | `u` |
| `popup_toggle` | `space` |
| `submit` | `enter` |
| `back` | `b` |
| `back_escape` | `esc` |
| `move_up` | `k` |
| `move_down` | `j` |
| `open_browser` | `o` |
| `open_linked_pr_browser` | `shift+o` |
| `open_linked_pr_tui` | `shift+p` |
| `checkout_pr` | `v` |
| `focus_left` | `ctrl+h` |
| `focus_right` | `ctrl+l` |
| `rescan_repos` | `ctrl+r` |
| `diff_scroll_left` | `[` |
| `diff_scroll_right` | `]` |
| `diff_scroll_reset` | `0` |
