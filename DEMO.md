# blippy Demo

A visual walkthrough of blippy's features.

---

## Getting Started

### Help Overlay

Press `?` anytime to see available keybindings.

![Help overlay showing all keyboard shortcuts](images/help-blippy.png)

### Repository Picker

Start by selecting a repository. Press `Ctrl+g` to open the picker.

![Repository picker with search](images/repo-view-blippy.png)

---

## Issues

### Issue List

Browse open and closed issues. Toggle between tabs with `Tab`/`Shift+Tab` or jump directly with `1`/`2`.

![Issue list showing open and closed tabs](images/issue-list-oc-blippy.png)

Closed issues are clearly marked:

![Closed issue list view](images/issue-view-closed-blippy.png)

### Issue Detail View

View issue details, description, and recent comments.

![Issue description pane](images/issue-desc-blippy.png)
### Comments View

Press `c` to see all comments on an issue.

![Full comments view](images/comment-view-blippy.png)

### Comment Management

#### Deleting Comments

Before deletion:

![Comments before deletion](images/issue-comments-before-deleted-blippy.png)

After deletion:

![Comments after deletion](images/issue-comments-after-deleting-blippy.png)

#### Editing Comments

Edit mode:

![Editing a comment](images/issue-edit-comment-blippy.png)

After editing:

![Comment after edit](images/issue-after-edit-comment-blippy.png)

---

## Pull Requests

### PR List

Toggle between issues and PRs with `p`.

![PR list view showing open PRs](images/pr-list-oc-blippy.png)

Closed PRs tab:

![PR list view showing closed PRs](images/pr-view-blippy.png)

### PR Detail View

View PR description and metadata.

![PR description](images/pr-desc-blippy.png)

### PR Checkout

Checkout PRs locally using `gh pr checkout`. Press `v` from any PR view:

![PR checkout demo](images/checkout-pr-video-blippy.gif)

### File Review

Browse changed files and mark them as viewed.

#### Unviewed Files

Files pending review:

![PR file diff not viewed](images/pr-file-diff-not-viewed-blippy.png)

#### Viewed Files

Marked with a checkmark:

![PR viewed file with checkmark](images/pr-viewed-file-checkmark-blippy.png)

### Diff Navigation

#### Extended Diff View

Expand to full width with `Enter`:

![Extended diff view](images/extended-diff-blippy.png)

#### Review Side Indicators

When adding review comments, indicators show which side of the diff you're commenting on:

Left side (old):

![Diff with left arrow indicator for review](images/diff-left-arrow-blippy.png)

Right side (new):

![Diff with right arrow indicator for review](images/diff-right-arrow-blippy.png)

#### Hunk Collapse

Collapse sections with `c`:

![Collapsed hunk in diff](images/diff-collapsed-hunk-blippy.png)

#### Line Selection

Visual range selection for review comments:

![Highlighted lines in diff](images/diff-highlighted-lines-blippy.png)

---

## Review Comments

### Adding Comments

#### In PR Review Mode

![PR review comment interface](images/pr-review-comment-blippy.png)

#### Inline on Diff

Add comments directly on diff lines:

![Comment in diff view](images/pr-comment-in-diff-blippy.png)

### Managing Comments

#### Editing

![Editing a PR comment](images/pr-comment-edit-blippy.png)

After edit:

![PR comment after edit](images/pr-comment-after-edit-blippy.png)

#### Deleting

![Deleted PR comment](images/pr-comment-deleted-blippy.png)

---

## Metadata Editing

### Labels

Press `l` to edit labels.

![Label picker with search](images/label-picker-blippy.png)

### Assignees

Press `Shift+A` to edit assignees.

![Assignee picker with search](images/assignee-picker-blippy.png)

---

## Close Presets

Streamline issue/PR closure with preset comments.

### Close Flow

Starting from issue view:

![Issue view before close](images/close-issue-view-blippy.png)

### Preset Creation

#### Name Your Preset

![Naming a close preset](images/close-preset-name-blippy.png)

#### Define Preset Body

![Editing preset body text](images/close-preset-body-blippy.png)

### Using Presets

Select from your presets when closing:

![Selecting a close preset](images/close-select-preset-blippy.png)

### Result

Issue closed with preset comment:

![Comments after preset close](images/comments-after-preset-close-blippy.png)

---

## Quick Reference

| Action | Key |
|--------|-----|
| Help | `?` |
| Repo picker | `Ctrl+g` |
| Toggle issues/PRs | `p` |
| Open/closed tabs | `Tab`/`1`/`2` |
| Add comment | `m` |
| Edit labels | `l` |
| Edit assignees | `Shift+A` |
| Close item | `dd` |
| Open in browser | `o` |
| Checkout PR | `v` |
| Mark file viewed | `w` |
| Collapse hunk | `c` |
| Visual selection | `Shift+V` |
| Resolve thread | `Shift+R` |
| Pan diff left/right | `[` / `]` |
| Quit | `Ctrl+c` |
