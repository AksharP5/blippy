use super::*;

pub(super) fn handle_events(
    app: &mut App,
    conn: &rusqlite::Connection,
    event_rx: &Receiver<AppEvent>,
) -> Result<()> {
    while let Ok(event) = event_rx.try_recv() {
        match event {
            AppEvent::ReposUpdated => {
                if app.view() == View::RepoPicker {
                    app.set_repos(main_data::load_repos(conn)?);
                    app.set_status(String::new());
                }
            }
            AppEvent::ScanFinished => {
                app.set_scanning(false);
                if app.view() == View::RepoPicker {
                    app.set_status(String::new());
                }
            }
            AppEvent::SyncFinished { owner, repo, stats } => {
                app.set_syncing(false);
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    refresh_current_repo_issues(app, conn)?;
                    app.request_repo_labels_sync();
                    let (open_count, closed_count) = app.issue_counts();
                    if stats.not_modified {
                        app.set_status(format!(
                            "No issue changes (open: {}, closed: {})",
                            open_count, closed_count
                        ));
                        continue;
                    }
                    app.set_status(format!(
                        "Synced {} issues (open: {}, closed: {})",
                        stats.issues, open_count, closed_count
                    ));
                }
            }
            AppEvent::SyncProgress {
                owner,
                repo,
                page,
                stats,
            } => {
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    refresh_current_repo_issues(app, conn)?;
                    let (open_count, closed_count) = app.issue_counts();
                    app.set_status(format!(
                        "Syncing page {}: {} issues cached (open: {}, closed: {})",
                        page, stats.issues, open_count, closed_count
                    ));
                }
            }
            AppEvent::SyncFailed {
                owner,
                repo,
                message,
            } => {
                app.set_syncing(false);
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    app.set_status(format!("Sync failed: {}", message));
                }
            }
            AppEvent::CommentsUpdated { issue_id, count } => {
                app.set_comment_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    load_comments_for_issue(app, conn, issue_id)?;
                    app.set_status(format!("Updated {} comments", count));
                }
            }
            AppEvent::CommentsFailed { issue_id, message } => {
                app.set_comment_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Comments unavailable: {}", message));
                }
            }
            AppEvent::IssueUpdated {
                issue_number,
                message,
            } => {
                if message.starts_with("closed")
                    || message.starts_with("close failed")
                    || message.starts_with("reopened")
                    || message.starts_with("reopen failed")
                    || message.starts_with("merged")
                    || message.starts_with("merge failed")
                    || message.starts_with("label update failed")
                    || message.starts_with("assignee update failed")
                {
                    app.clear_pending_issue_action(issue_number);
                }
                if message.starts_with("closed") {
                    app.update_issue_state_by_number(issue_number, "closed");
                }
                if message.starts_with("reopened") {
                    app.update_issue_state_by_number(issue_number, "open");
                }
                if message.starts_with("merged") {
                    app.update_issue_state_by_number(issue_number, "merged");
                }
                app.set_status(format!("#{} {}", issue_number, message));
                app.request_sync();
                if app.current_issue_number() == Some(issue_number) {
                    app.request_comment_sync();
                }
            }
            AppEvent::IssueCreated { issue_number } => {
                app.set_work_item_mode(WorkItemMode::Issues);
                app.set_issue_filter(IssueFilter::Open);
                refresh_current_repo_issues(app, conn)?;
                if app.select_issue_by_number(issue_number)
                    && let Some((issue_id, issue_number)) = app
                        .selected_issue_row()
                        .map(|issue| (issue.id, issue.number))
                {
                    app.set_current_issue(issue_id, issue_number);
                    load_comments_for_issue(app, conn, issue_id)?;
                    app.set_view(View::IssueDetail);
                }
                app.set_status(format!("Created issue #{}", issue_number));
                app.request_sync();
            }
            AppEvent::IssueCreateFailed { message } => {
                app.set_status(format!("Issue creation failed: {}", message));
            }
            AppEvent::IssueLabelsUpdated {
                issue_number,
                labels,
            } => {
                app.clear_pending_issue_action(issue_number);
                app.update_issue_labels_by_number(issue_number, labels.as_str());
                app.set_status(format!("#{} labels updated", issue_number));
                app.request_sync();
            }
            AppEvent::IssueAssigneesUpdated {
                issue_number,
                assignees,
            } => {
                app.clear_pending_issue_action(issue_number);
                app.update_issue_assignees_by_number(issue_number, assignees.as_str());
                app.set_status(format!("#{} assignees updated", issue_number));
                app.request_sync();
            }
            AppEvent::PullRequestFilesUpdated {
                issue_id,
                files,
                pull_request_id,
                viewed_files,
            } => {
                app.set_pull_request_files_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    let count = files.len();
                    app.set_pull_request_files(issue_id, files);
                    app.set_pull_request_view_state(pull_request_id, viewed_files);
                    app.set_status(format!("Loaded {} changed files", count));
                }
            }
            AppEvent::PullRequestFilesFailed { issue_id, message } => {
                app.set_pull_request_files_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("PR files unavailable: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentsUpdated { issue_id, comments } => {
                app.set_pull_request_review_comments_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    let count = comments.len();
                    app.set_pull_request_review_comments(comments);
                    app.set_status(format!("Loaded {} review comments", count));
                }
            }
            AppEvent::PullRequestReviewCommentsFailed { issue_id, message } => {
                app.set_pull_request_review_comments_syncing(false);
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("PR review comments unavailable: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentCreated { issue_id } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.request_pull_request_review_comments_sync();
                    app.set_status("Review comment submitted".to_string());
                }
            }
            AppEvent::PullRequestReviewCommentCreateFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review comment failed: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentUpdated {
                issue_id,
                comment_id,
                body,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.update_pull_request_review_comment_body_by_id(comment_id, body.as_str());
                    app.set_status("Review comment updated".to_string());
                }
            }
            AppEvent::PullRequestReviewCommentUpdateFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review comment update failed: {}", message));
                }
            }
            AppEvent::PullRequestReviewCommentDeleted {
                issue_id,
                comment_id,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.remove_pull_request_review_comment_by_id(comment_id);
                    app.set_status("Review comment deleted".to_string());
                }
            }
            AppEvent::PullRequestReviewCommentDeleteFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review comment delete failed: {}", message));
                }
            }
            AppEvent::PullRequestReviewThreadResolutionUpdated { issue_id, resolved } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.request_pull_request_review_comments_sync();
                    if resolved {
                        app.set_status("Review thread resolved".to_string());
                    } else {
                        app.set_status("Review thread reopened".to_string());
                    }
                }
            }
            AppEvent::PullRequestReviewThreadResolutionFailed { issue_id, message } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_status(format!("Review thread resolution failed: {}", message));
                }
            }
            AppEvent::PullRequestFileViewedUpdated {
                issue_id,
                path,
                viewed,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_pull_request_file_viewed(path.as_str(), viewed);
                    if viewed {
                        app.set_status(format!("Marked {} viewed on GitHub", path));
                    } else {
                        app.set_status(format!("Marked {} unviewed on GitHub", path));
                    }
                }
            }
            AppEvent::PullRequestFileViewedUpdateFailed {
                issue_id,
                path,
                viewed,
                message,
            } => {
                if app.current_issue_id() == Some(issue_id) {
                    app.set_pull_request_file_viewed(path.as_str(), !viewed);
                    app.set_status(format!(
                        "GitHub view state failed for {}: {}",
                        path, message
                    ));
                }
            }
            AppEvent::LinkedPullRequestResolved {
                issue_number,
                pull_requests,
                target,
            } => {
                let pull_numbers = pull_requests
                    .iter()
                    .map(|(pull_number, _url)| *pull_number)
                    .collect::<Vec<i64>>();
                app.set_linked_pull_requests(issue_number, pull_numbers.clone());

                if pull_numbers.is_empty() {
                    if target == LinkedPullRequestTarget::Probe {
                        continue;
                    }
                    app.set_status(format!(
                        "No linked pull request found for #{}",
                        issue_number
                    ));
                    continue;
                }

                if target == LinkedPullRequestTarget::Probe {
                    continue;
                }

                if pull_numbers.len() > 1 {
                    let picker_target = match target {
                        LinkedPullRequestTarget::Tui => LinkedPickerTarget::PullRequestTui,
                        LinkedPullRequestTarget::Browser => LinkedPickerTarget::PullRequestBrowser,
                        LinkedPullRequestTarget::Probe => LinkedPickerTarget::PullRequestTui,
                    };
                    app.open_linked_picker(app.view(), picker_target, pull_numbers);
                    app.set_status(format!(
                        "Found {} linked pull requests for #{}",
                        app.linked_picker_numbers().len(),
                        issue_number
                    ));
                    continue;
                }

                let pull_number = pull_numbers[0];
                let url = pull_requests.into_iter().find_map(|(number, url)| {
                    if number == pull_number {
                        Some(url)
                    } else {
                        None
                    }
                });

                if target == LinkedPullRequestTarget::Tui {
                    app.capture_linked_navigation_origin();
                    refresh_current_repo_issues(app, conn)?;
                    if main_linked_actions::open_pull_request_in_tui(app, conn, pull_number)? {
                        app.set_status(format!(
                            "Opened linked pull request #{} in TUI",
                            pull_number
                        ));
                        continue;
                    }

                    app.clear_linked_navigation_origin();
                    app.set_status(format!(
                        "Linked PR #{} not cached in TUI yet; press r then Shift+P",
                        pull_number
                    ));
                    continue;
                }

                let browser_url = match url {
                    Some(url) => Some(url),
                    None => {
                        if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo())
                        {
                            Some(format!(
                                "https://github.com/{}/{}/pull/{}",
                                owner, repo, pull_number
                            ))
                        } else {
                            None
                        }
                    }
                };

                if let Some(browser_url) = browser_url {
                    if let Err(error) = main_linked_actions::open_url(browser_url.as_str()) {
                        app.set_status(format!("Open linked PR failed: {}", error));
                        continue;
                    }
                    app.set_status(format!(
                        "Opened linked pull request #{} in browser",
                        pull_number
                    ));
                    continue;
                }

                app.set_status(format!(
                    "Linked PR #{} found but URL unavailable",
                    pull_number
                ));
            }
            AppEvent::LinkedPullRequestLookupFailed {
                issue_number,
                message,
                target,
            } => {
                app.end_linked_pull_request_lookup(issue_number);
                if target == LinkedPullRequestTarget::Probe {
                    continue;
                }
                let target_label = match target {
                    LinkedPullRequestTarget::Tui => "TUI",
                    LinkedPullRequestTarget::Browser => "browser",
                    LinkedPullRequestTarget::Probe => "probe",
                };
                app.set_status(format!(
                    "Linked pull request lookup failed for #{} ({}): {}",
                    issue_number, target_label, message
                ));
            }
            AppEvent::LinkedIssueResolved {
                pull_number,
                issues,
                target,
            } => {
                let issue_numbers = issues
                    .iter()
                    .map(|(issue_number, _url)| *issue_number)
                    .collect::<Vec<i64>>();
                app.set_linked_issues_for_pull_request(pull_number, issue_numbers.clone());

                if issue_numbers.is_empty() {
                    if target == LinkedIssueTarget::Probe {
                        continue;
                    }
                    app.set_status(format!("No linked issue found for PR #{}", pull_number));
                    continue;
                }

                if target == LinkedIssueTarget::Probe {
                    continue;
                }

                if issue_numbers.len() > 1 {
                    let picker_target = match target {
                        LinkedIssueTarget::Tui => LinkedPickerTarget::IssueTui,
                        LinkedIssueTarget::Browser => LinkedPickerTarget::IssueBrowser,
                        LinkedIssueTarget::Probe => LinkedPickerTarget::IssueTui,
                    };
                    app.open_linked_picker(app.view(), picker_target, issue_numbers);
                    app.set_status(format!(
                        "Found {} linked issues for PR #{}",
                        app.linked_picker_numbers().len(),
                        pull_number
                    ));
                    continue;
                }

                let issue_number = issue_numbers[0];
                let url = issues.into_iter().find_map(|(number, url)| {
                    if number == issue_number {
                        Some(url)
                    } else {
                        None
                    }
                });

                if target == LinkedIssueTarget::Tui {
                    app.capture_linked_navigation_origin();
                    refresh_current_repo_issues(app, conn)?;
                    if main_linked_actions::open_issue_in_tui(app, conn, issue_number)? {
                        app.set_status(format!("Opened linked issue #{} in TUI", issue_number));
                        continue;
                    }

                    app.clear_linked_navigation_origin();
                    app.set_status(format!(
                        "Linked issue #{} not cached in TUI yet; press r then Shift+P",
                        issue_number
                    ));
                    continue;
                }

                let browser_url = match url {
                    Some(url) => Some(url),
                    None => {
                        if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo())
                        {
                            Some(format!(
                                "https://github.com/{}/{}/issues/{}",
                                owner, repo, issue_number
                            ))
                        } else {
                            None
                        }
                    }
                };

                if let Some(browser_url) = browser_url {
                    if let Err(error) = main_linked_actions::open_url(browser_url.as_str()) {
                        app.set_status(format!("Open linked issue failed: {}", error));
                        continue;
                    }
                    app.set_status(format!("Opened linked issue #{} in browser", issue_number));
                    continue;
                }

                app.set_status(format!(
                    "Linked issue #{} found but URL unavailable",
                    issue_number
                ));
            }
            AppEvent::LinkedIssueLookupFailed {
                pull_number,
                message,
                target,
            } => {
                app.end_linked_issue_lookup(pull_number);
                if target == LinkedIssueTarget::Probe {
                    continue;
                }
                let target_label = match target {
                    LinkedIssueTarget::Tui => "TUI",
                    LinkedIssueTarget::Browser => "browser",
                    LinkedIssueTarget::Probe => "probe",
                };
                app.set_status(format!(
                    "Linked issue lookup failed for PR #{} ({}): {}",
                    pull_number, target_label, message
                ));
            }
            AppEvent::IssueCommentUpdated {
                issue_number,
                comment_id,
                body,
            } => {
                app.update_comment_body_by_id(comment_id, body.as_str());
                app.set_status(format!("#{} comment updated", issue_number));
                app.request_comment_sync();
                app.request_sync();
            }
            AppEvent::IssueCommentDeleted {
                issue_number,
                comment_id,
                count,
            } => {
                app.remove_comment_by_id(comment_id);
                app.update_issue_comments_count_by_number(issue_number, count as i64);
                app.set_status(format!("#{} comment deleted", issue_number));
                app.request_comment_sync();
                app.request_sync();
            }
            AppEvent::RepoLabelsSuggested {
                owner,
                repo,
                labels,
            } => {
                app.set_repo_labels_syncing(false);
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    app.merge_repo_label_colors(labels.clone());
                    if app.view() == View::LabelPicker {
                        let options = labels
                            .iter()
                            .map(|(name, _)| name.clone())
                            .collect::<Vec<String>>();
                        app.merge_label_options(options);
                    }
                }
            }
            AppEvent::RepoAssigneesSuggested {
                owner,
                repo,
                assignees,
            } => {
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                    && app.view() == View::AssigneePicker
                {
                    app.merge_assignee_options(assignees);
                }
            }
            AppEvent::RepoPermissionsResolved {
                owner,
                repo,
                can_edit_issue_metadata,
                can_merge_pull_request,
            } => {
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    app.set_repo_permissions_syncing(false);
                    app.set_repo_issue_metadata_editable(Some(can_edit_issue_metadata));
                    app.set_repo_pull_request_mergeable(Some(can_merge_pull_request));
                    if !can_edit_issue_metadata {
                        app.set_status(
                            "No permission to edit labels/assignees in this repo".to_string(),
                        );
                    }
                }
            }
            AppEvent::RepoPermissionsFailed {
                owner,
                repo,
                message,
            } => {
                if app.current_owner() == Some(owner.as_str())
                    && app.current_repo() == Some(repo.as_str())
                {
                    app.set_repo_permissions_syncing(false);
                    app.set_repo_issue_metadata_editable(None);
                    app.set_repo_pull_request_mergeable(None);
                    app.set_status(format!("Repo permission check failed: {}", message));
                }
            }
        }
    }
    Ok(())
}
