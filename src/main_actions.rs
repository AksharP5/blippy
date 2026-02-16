use super::main_action_utils::*;
use super::*;

pub(super) use super::main_action_utils::issue_url;

pub(super) fn handle_actions(
    app: &mut App,
    conn: &rusqlite::Connection,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    let action = match app.take_action() {
        Some(action) => action,
        None => return Ok(()),
    };

    match action {
        AppAction::PickRepo => {
            let (owner, repo, path) = match app.selected_repo_target() {
                Some(target) => target,
                None => return Ok(()),
            };
            super::main_data::load_issues_for_slug(app, conn, &owner, &repo, Some(path.as_str()))?;
            app.set_view(View::Issues);
            app.request_sync();
        }
        AppAction::PickRemote => {
            let (owner, repo) = match app.remotes().get(app.selected_remote()) {
                Some(remote) => (remote.slug.owner.clone(), remote.slug.repo.clone()),
                None => return Ok(()),
            };
            let repo_path = crate::git::repo_root()?.map(|path| path.to_string_lossy().to_string());
            super::main_data::load_issues_for_slug(app, conn, &owner, &repo, repo_path.as_deref())?;
            app.set_view(View::Issues);
            app.request_sync();
        }
        AppAction::PickIssue => {
            let (issue_id, issue_number, is_pr) = match app.selected_issue_row() {
                Some(issue) => (issue.id, issue.number, issue.is_pr),
                None => return Ok(()),
            };
            app.set_current_issue(issue_id, issue_number);
            app.reset_issue_detail_scroll();
            load_comments_for_issue(app, conn, issue_id)?;
            app.set_view(View::IssueDetail);
            app.set_comment_syncing(false);
            app.request_comment_sync();
            if is_pr {
                app.request_pull_request_files_sync();
                app.request_pull_request_review_comments_sync();
                if app.begin_linked_issue_lookup(issue_number) {
                    if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo()) {
                        super::main_linked_actions::start_linked_issue_lookup(
                            owner.to_string(),
                            repo.to_string(),
                            issue_number,
                            token.to_string(),
                            event_tx.clone(),
                            LinkedIssueTarget::Probe,
                        );
                    } else {
                        app.end_linked_issue_lookup(issue_number);
                    }
                }
            } else if app.begin_linked_pull_request_lookup(issue_number) {
                if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo()) {
                    super::main_linked_actions::start_linked_pull_request_lookup(
                        owner.to_string(),
                        repo.to_string(),
                        issue_number,
                        token.to_string(),
                        event_tx.clone(),
                        LinkedPullRequestTarget::Probe,
                    );
                } else {
                    app.end_linked_pull_request_lookup(issue_number);
                }
            }
        }
        AppAction::OpenInBrowser => {
            if let Some(url) = issue_url(app) {
                if let Err(error) = super::main_linked_actions::open_url(&url) {
                    app.set_status(format!("Open failed: {}", error));
                }
            } else {
                app.set_status("No issue selected".to_string());
            }
        }
        AppAction::CheckoutPullRequest => {
            checkout_pull_request(app)?;
        }
        AppAction::OpenLinkedPullRequestInBrowser => {
            if !super::main_linked_actions::try_open_cached_linked_pull_request(
                app,
                conn,
                LinkedPullRequestTarget::Browser,
            )? {
                super::main_linked_actions::open_linked_pull_request(
                    app,
                    token,
                    event_tx.clone(),
                    LinkedPullRequestTarget::Browser,
                )?;
            }
        }
        AppAction::OpenLinkedPullRequestInTui => {
            if !super::main_linked_actions::try_open_cached_linked_pull_request(
                app,
                conn,
                LinkedPullRequestTarget::Tui,
            )? {
                super::main_linked_actions::open_linked_pull_request(
                    app,
                    token,
                    event_tx.clone(),
                    LinkedPullRequestTarget::Tui,
                )?;
            }
        }
        AppAction::OpenLinkedIssueInBrowser => {
            if !super::main_linked_actions::try_open_cached_linked_issue(
                app,
                conn,
                LinkedIssueTarget::Browser,
            )? {
                super::main_linked_actions::open_linked_issue(
                    app,
                    token,
                    event_tx.clone(),
                    LinkedIssueTarget::Browser,
                )?;
            }
        }
        AppAction::OpenLinkedIssueInTui => {
            if !super::main_linked_actions::try_open_cached_linked_issue(
                app,
                conn,
                LinkedIssueTarget::Tui,
            )? {
                super::main_linked_actions::open_linked_issue(
                    app,
                    token,
                    event_tx.clone(),
                    LinkedIssueTarget::Tui,
                )?;
            }
        }
        AppAction::PickLinkedItem => {
            super::main_linked_actions::open_selected_linked_item(app, conn)?;
        }
        AppAction::AddIssueComment => {
            let (issue_id, issue_number, _) = match selected_issue_for_action(app) {
                Some(issue) => issue,
                None => {
                    app.set_status("No issue selected".to_string());
                    return Ok(());
                }
            };
            app.set_current_issue(issue_id, issue_number);
            app.open_issue_comment_editor(app.view());
        }
        AppAction::EditIssueComment => {
            let return_view = app.view();
            let comment = match app.selected_comment_row() {
                Some(comment) => comment.clone(),
                None => {
                    app.set_status("No comment selected".to_string());
                    return Ok(());
                }
            };
            app.open_comment_edit_editor(return_view, comment.id, comment.body.as_str());
        }
        AppAction::DeleteIssueComment => {
            delete_issue_comment(app, token, event_tx.clone())?;
        }
        AppAction::AddPullRequestReviewComment => {
            let target = match app.selected_pull_request_review_target() {
                Some(target) => target,
                None => {
                    app.set_status("Select a diff line to comment on".to_string());
                    return Ok(());
                }
            };
            app.open_pull_request_review_comment_editor(app.view(), target);
        }
        AppAction::SubmitPullRequestReviewComment => {
            let comment = app.editor().text().to_string();
            submit_pull_request_review_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::EditPullRequestReviewComment => {
            let return_view = app.view();
            let comment = match app.selected_pull_request_review_comment() {
                Some(comment) => comment.clone(),
                None => {
                    app.set_status("No review comment selected".to_string());
                    return Ok(());
                }
            };
            app.open_pull_request_review_comment_edit_editor(
                return_view,
                comment.id,
                comment.body.as_str(),
            );
        }
        AppAction::DeletePullRequestReviewComment => {
            delete_pull_request_review_comment(app, token, event_tx.clone())?;
        }
        AppAction::ResolvePullRequestReviewComment => {
            resolve_pull_request_review_comment(app, token, event_tx.clone())?;
        }
        AppAction::TogglePullRequestFileViewed => {
            toggle_pull_request_file_viewed(app, token, event_tx.clone())?;
        }
        AppAction::SubmitEditedPullRequestReviewComment => {
            let comment = app.editor().text().to_string();
            update_pull_request_review_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::EditLabels => {
            if !ensure_can_edit_issue_metadata(app) {
                return Ok(());
            }
            let return_view = app.view();
            let (issue_id, issue_number, _) = match selected_issue_for_action(app) {
                Some(issue) => issue,
                None => {
                    app.set_status("No issue selected".to_string());
                    return Ok(());
                }
            };
            app.set_current_issue(issue_id, issue_number);
            let labels = selected_issue_labels(app).unwrap_or_default();
            let options = label_options_for_repo(app);
            app.open_label_picker(return_view, options, labels.as_str());
            app.request_repo_labels_sync();
        }
        AppAction::EditAssignees => {
            if !ensure_can_edit_issue_metadata(app) {
                return Ok(());
            }
            let return_view = app.view();
            let (issue_id, issue_number, _) = match selected_issue_for_action(app) {
                Some(issue) => issue,
                None => {
                    app.set_status("No issue selected".to_string());
                    return Ok(());
                }
            };
            app.set_current_issue(issue_id, issue_number);
            let assignees = selected_issue_assignees(app).unwrap_or_default();
            let options = assignee_options_for_repo(app);
            app.open_assignee_picker(return_view, options, assignees.as_str());
            if let (Some(owner), Some(repo)) = (app.current_owner(), app.current_repo()) {
                start_fetch_assignees(
                    owner.to_string(),
                    repo.to_string(),
                    token.to_string(),
                    event_tx.clone(),
                );
            }
        }
        AppAction::SubmitIssueComment => {
            let comment = app.editor().text().to_string();
            post_issue_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::SubmitEditedComment => {
            let comment = app.editor().text().to_string();
            update_issue_comment(app, token, comment, event_tx.clone())?;
        }
        AppAction::SubmitLabels => {
            let labels = app.selected_labels();
            update_issue_labels(app, token, labels, event_tx.clone())?;
        }
        AppAction::SubmitAssignees => {
            let assignees = app.selected_assignees();
            update_issue_assignees(app, token, assignees, event_tx.clone())?;
        }
        AppAction::CloseIssue => {
            if let Some((issue_id, issue_number, _)) = selected_issue_for_action(app) {
                app.set_current_issue(issue_id, issue_number);
            }
            app.set_selected_preset(0);
            app.set_view(View::CommentPresetPicker);
        }
        AppAction::ReopenIssue => {
            reopen_issue(app, token, event_tx.clone())?;
        }
        AppAction::PickPreset => handle_preset_selection(app, conn, token, event_tx)?,
        AppAction::SubmitComment => {
            let comment = app.editor().text().to_string();
            close_issue_with_comment(app, token, Some(comment), event_tx.clone())?;
        }
        AppAction::SavePreset => {
            save_preset_from_editor(app)?;
            app.set_view(View::CommentPresetPicker);
        }
    }
    Ok(())
}
