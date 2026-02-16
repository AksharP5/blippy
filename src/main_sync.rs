use super::*;

pub(super) fn maybe_start_repo_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if app.syncing() {
        return Ok(());
    }

    if !app.take_sync_request() {
        return Ok(());
    }

    let owner = match app.current_owner() {
        Some(owner) => owner.to_string(),
        None => return Ok(()),
    };
    let repo = match app.current_repo() {
        Some(repo) => repo.to_string(),
        None => return Ok(()),
    };

    start_repo_sync(owner, repo, token.to_string(), event_tx);
    app.set_syncing(true);
    app.set_status("Syncing".to_string());
    Ok(())
}

pub(super) fn maybe_start_repo_permissions_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) {
    if app.repo_permissions_syncing() {
        return;
    }
    if !app.take_repo_permissions_sync_request() {
        return;
    }

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => return,
    };

    start_fetch_repo_permissions(owner, repo, token.to_string(), event_tx);
    app.set_repo_permissions_syncing(true);
}

pub(super) fn maybe_start_repo_labels_sync(app: &mut App, token: &str, event_tx: Sender<AppEvent>) {
    if app.repo_labels_syncing() {
        return;
    }
    if !app.take_repo_labels_sync_request() {
        return;
    }

    let (owner, repo) = match (app.current_owner(), app.current_repo()) {
        (Some(owner), Some(repo)) => (owner.to_string(), repo.to_string()),
        _ => return,
    };

    start_fetch_labels(owner, repo, token.to_string(), event_tx);
    app.set_repo_labels_syncing(true);
}

pub(super) fn maybe_start_issue_poll(app: &mut App, last_poll: &mut Instant) {
    if !matches!(
        app.view(),
        View::Issues | View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return;
    }

    if last_poll.elapsed() < ISSUE_POLL_INTERVAL {
        return;
    }

    app.request_sync();
    *last_poll = Instant::now();
}

pub(super) fn maybe_start_comment_poll(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
    last_poll: &mut Instant,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }

    if app.comment_syncing() {
        return Ok(());
    }

    if !app.take_comment_sync_request() && last_poll.elapsed() < COMMENT_POLL_INTERVAL {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    start_comment_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_comment_syncing(true);
    *last_poll = Instant::now();
    Ok(())
}

pub(super) fn maybe_start_pull_request_files_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }
    if app.pull_request_files_syncing() {
        return Ok(());
    }
    if !app.take_pull_request_files_sync_request() {
        return Ok(());
    }
    if !app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    start_pull_request_files_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_pull_request_files_syncing(true);
    app.set_status("Loading pull request changes".to_string());
    Ok(())
}

pub(super) fn maybe_start_pull_request_review_comments_sync(
    app: &mut App,
    token: &str,
    event_tx: Sender<AppEvent>,
) -> Result<()> {
    if !matches!(
        app.view(),
        View::IssueDetail | View::IssueComments | View::PullRequestFiles
    ) {
        return Ok(());
    }
    if app.pull_request_review_comments_syncing() {
        return Ok(());
    }
    if !app.take_pull_request_review_comments_sync_request() {
        return Ok(());
    }
    if !app.current_issue_row().is_some_and(|issue| issue.is_pr) {
        return Ok(());
    }

    let (owner, repo, issue_id, issue_number) = match (
        app.current_owner(),
        app.current_repo(),
        app.current_issue_id(),
        app.current_issue_number(),
    ) {
        (Some(owner), Some(repo), Some(issue_id), Some(issue_number)) => {
            (owner.to_string(), repo.to_string(), issue_id, issue_number)
        }
        _ => return Ok(()),
    };

    start_pull_request_review_comments_sync(
        owner,
        repo,
        issue_id,
        issue_number,
        token.to_string(),
        event_tx,
    );
    app.set_pull_request_review_comments_syncing(true);
    Ok(())
}

pub(super) fn start_repo_sync(
    owner: String,
    repo: String,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    let error_owner = owner.clone();
    let error_repo = repo.clone();
    spawn_with_db(
        token,
        event_tx,
        move |message| AppEvent::SyncFailed {
            owner: error_owner,
            repo: error_repo,
            message,
        },
        move |ctx, event_tx| {
            let progress_tx = event_tx.clone();
            let result = ctx.services.runtime.block_on(async {
                sync_repo_with_progress(
                    &ctx.services.client,
                    &ctx.conn,
                    &owner,
                    &repo,
                    |page, stats| {
                        let _ = progress_tx.send(AppEvent::SyncProgress {
                            owner: owner.clone(),
                            repo: repo.clone(),
                            page,
                            stats: stats.clone(),
                        });
                    },
                )
                .await
            });
            let stats = match result {
                Ok(stats) => stats,
                Err(error) => {
                    let _ = event_tx.send(AppEvent::SyncFailed {
                        owner: owner.clone(),
                        repo: repo.clone(),
                        message: error.to_string(),
                    });
                    return;
                }
            };
            let _ = event_tx.send(AppEvent::SyncFinished { owner, repo, stats });
        },
    );
}

pub(super) fn start_comment_sync(
    owner: String,
    repo: String,
    issue_id: i64,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_db(
        token,
        event_tx,
        move |message| AppEvent::CommentsFailed { issue_id, message },
        move |ctx, event_tx| {
            let result = ctx.services.runtime.block_on(async {
                ctx.services
                    .client
                    .list_comments(&owner, &repo, issue_number)
                    .await
            });
            let comments = match result {
                Ok(comments) => comments,
                Err(error) => {
                    let _ = event_tx.send(AppEvent::CommentsFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                    return;
                }
            };

            let now = comment_now_epoch();
            let mut count = 0usize;
            for comment in comments {
                let mut row = crate::sync::map_comment_to_row(issue_id, &comment);
                row.last_accessed_at = Some(now);
                let _ = crate::store::upsert_comment(&ctx.conn, &row);
                count += 1;
            }
            let _ = update_issue_comments_count(&ctx.conn, issue_id, count as i64);
            let _ = touch_comments_for_issue(&ctx.conn, issue_id, now);
            let _ = prune_comments(&ctx.conn, COMMENT_TTL_SECONDS, COMMENT_CAP);

            let _ = event_tx.send(AppEvent::CommentsUpdated { issue_id, count });
        },
    );
}

pub(super) fn start_pull_request_files_sync(
    owner: String,
    repo: String,
    issue_id: i64,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestFilesFailed { issue_id, message },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .list_pull_request_files(&owner, &repo, issue_number)
                    .await
            });

            let files = match result {
                Ok(files) => files,
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestFilesFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                    return;
                }
            };

            let (pull_request_id, viewed_files) = services
                .runtime
                .block_on(async {
                    services
                        .client
                        .pull_request_file_view_state(&owner, &repo, issue_number)
                        .await
                })
                .unwrap_or((None, HashSet::new()));

            let mapped = files
                .into_iter()
                .map(|file| PullRequestFile {
                    filename: file.filename,
                    status: file.status,
                    additions: file.additions,
                    deletions: file.deletions,
                    patch: file.patch,
                })
                .collect::<Vec<PullRequestFile>>();
            let _ = event_tx.send(AppEvent::PullRequestFilesUpdated {
                issue_id,
                files: mapped,
                pull_request_id,
                viewed_files,
            });
        },
    );
}

pub(super) fn start_pull_request_review_comments_sync(
    owner: String,
    repo: String,
    issue_id: i64,
    pull_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestReviewCommentsFailed { issue_id, message },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .list_pull_request_review_comments(&owner, &repo, pull_number)
                    .await
            });

            let comments = match result {
                Ok(comments) => comments,
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentsFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                    return;
                }
            };

            let mut anchors = HashMap::new();
            for comment in &comments {
                let line = comment.line.or(comment.original_line);
                let side = comment
                    .side
                    .as_ref()
                    .map(|value| {
                        if value.eq_ignore_ascii_case("left") {
                            ReviewSide::Left
                        } else {
                            ReviewSide::Right
                        }
                    })
                    .unwrap_or(ReviewSide::Right);
                if let Some(line) = line {
                    anchors.insert(comment.id, (line, side, comment.path.clone()));
                }
            }

            let mut mapped = Vec::new();
            for comment in comments {
                let anchor = anchors.get(&comment.id).cloned().or_else(|| {
                    comment
                        .in_reply_to_id
                        .and_then(|reply_to_id| anchors.get(&reply_to_id).cloned())
                });
                let (line, side, path, anchored) = match anchor {
                    Some((line, side, path)) => (line, side, path, true),
                    None => (0, ReviewSide::Right, comment.path.clone(), false),
                };

                mapped.push(PullRequestReviewComment {
                    id: comment.id,
                    thread_id: comment.thread_id,
                    resolved: comment.is_resolved,
                    anchored,
                    path,
                    line,
                    side,
                    body: comment.body.unwrap_or_default(),
                    author: comment.user.login,
                    created_at: comment.created_at,
                });
            }
            let _ = event_tx.send(AppEvent::PullRequestReviewCommentsUpdated {
                issue_id,
                comments: mapped,
            });
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start_create_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    pull_number: i64,
    path: String,
    line: i64,
    side: ReviewSide,
    start_line: Option<i64>,
    start_side: Option<ReviewSide>,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestReviewCommentCreateFailed { issue_id, message },
        move |services, event_tx| {
            let head_sha = services.runtime.block_on(async {
                services
                    .client
                    .pull_request_head_sha(&owner, &repo, pull_number)
                    .await
            });
            let head_sha = match head_sha {
                Ok(head_sha) => head_sha,
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreateFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                    return;
                }
            };

            let created = services.runtime.block_on(async {
                services
                    .client
                    .create_pull_request_review_comment(
                        &owner,
                        &repo,
                        pull_number,
                        head_sha.as_str(),
                        path.as_str(),
                        line,
                        side.as_api_side(),
                        start_line,
                        start_side.map(ReviewSide::as_api_side),
                        body.as_str(),
                    )
                    .await
            });
            match created {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreated { issue_id });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentCreateFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                }
            }
        },
    );
}

pub(super) fn start_update_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    comment_id: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestReviewCommentUpdateFailed { issue_id, message },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .update_pull_request_review_comment(&owner, &repo, comment_id, body.as_str())
                    .await
            });
            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentUpdated {
                        issue_id,
                        comment_id,
                        body,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentUpdateFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                }
            }
        },
    );
}

pub(super) fn start_delete_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    comment_id: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestReviewCommentDeleteFailed { issue_id, message },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .delete_pull_request_review_comment(&owner, &repo, comment_id)
                    .await
            });
            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentDeleted {
                        issue_id,
                        comment_id,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewCommentDeleteFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                }
            }
        },
    );
}

pub(super) fn start_toggle_pull_request_review_thread_resolution(
    owner: String,
    repo: String,
    issue_id: i64,
    thread_id: String,
    resolve: bool,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestReviewThreadResolutionFailed { issue_id, message },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .set_pull_request_review_thread_resolved(
                        &owner,
                        &repo,
                        thread_id.as_str(),
                        resolve,
                    )
                    .await
            });
            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewThreadResolutionUpdated {
                        issue_id,
                        resolved: resolve,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::PullRequestReviewThreadResolutionFailed {
                        issue_id,
                        message: error.to_string(),
                    });
                }
            }
        },
    );
}

pub(super) fn start_set_pull_request_file_viewed(
    issue_id: i64,
    pull_request_id: String,
    path: String,
    viewed: bool,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    let error_path = path.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::PullRequestFileViewedUpdateFailed {
            issue_id,
            path: error_path,
            viewed,
            message,
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .set_pull_request_file_viewed(pull_request_id.as_str(), path.as_str(), viewed)
                    .await
            });
            if result.is_ok() {
                let _ = event_tx.send(AppEvent::PullRequestFileViewedUpdated {
                    issue_id,
                    path,
                    viewed,
                });
                return;
            }
            let _ = event_tx.send(AppEvent::PullRequestFileViewedUpdateFailed {
                issue_id,
                path,
                viewed,
                message: result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_default(),
            });
        },
    );
}

pub(super) fn start_add_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("comment failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .create_comment(&owner, &repo, issue_number, &body)
                    .await
            });

            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: "commented".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("comment failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(super) fn start_update_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    comment_id: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("comment update failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .update_comment(&owner, &repo, comment_id, body.as_str())
                    .await
            });

            match result {
                Ok(()) => {
                    with_store_conn(|conn| {
                        let _ = crate::store::update_comment_body_by_id(
                            conn,
                            comment_id,
                            body.as_str(),
                        );
                    });
                    let _ = event_tx.send(AppEvent::IssueCommentUpdated {
                        issue_number,
                        comment_id,
                        body,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("comment update failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(super) fn start_delete_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    comment_id: i64,
    issue_id: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("comment delete failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .delete_comment(&owner, &repo, comment_id)
                    .await
            });

            match result {
                Ok(()) => {
                    let mut count = 0usize;
                    with_store_conn(|conn| {
                        let _ = crate::store::delete_comment_by_id(conn, comment_id);
                        let comments =
                            crate::store::comments_for_issue(conn, issue_id).unwrap_or_default();
                        count = comments.len();
                        let _ = update_issue_comments_count(conn, issue_id, count as i64);
                    });
                    let _ = event_tx.send(AppEvent::IssueCommentDeleted {
                        issue_number,
                        comment_id,
                        count,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("comment delete failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(super) fn start_update_labels(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    labels: Vec<String>,
    event_tx: Sender<AppEvent>,
    labels_display: String,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("label update failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .update_issue_labels(&owner, &repo, issue_number, &labels)
                    .await
            });
            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::IssueLabelsUpdated {
                        issue_number,
                        labels: labels_display,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("label update failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(super) fn start_update_assignees(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    assignees: Vec<String>,
    event_tx: Sender<AppEvent>,
    assignees_display: String,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("assignee update failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .update_issue_assignees(&owner, &repo, issue_number, &assignees)
                    .await
            });
            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::IssueAssigneesUpdated {
                        issue_number,
                        assignees: assignees_display,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("assignee update failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(super) fn start_fetch_labels(
    owner: String,
    repo: String,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    let error_owner = owner.clone();
    let error_repo = repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |_| AppEvent::RepoLabelsSuggested {
            owner: error_owner,
            repo: error_repo,
            labels: Vec::new(),
        },
        move |services, event_tx| {
            let labels = services
                .runtime
                .block_on(async { services.client.list_labels(&owner, &repo).await });
            let labels = labels
                .unwrap_or_default()
                .into_iter()
                .map(|label| (label.name, label.color))
                .collect::<Vec<(String, String)>>();
            let _ = event_tx.send(AppEvent::RepoLabelsSuggested {
                owner,
                repo,
                labels,
            });
        },
    );
}

pub(super) fn start_fetch_assignees(
    owner: String,
    repo: String,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    let error_owner = owner.clone();
    let error_repo = repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |_| AppEvent::RepoAssigneesSuggested {
            owner: error_owner,
            repo: error_repo,
            assignees: Vec::new(),
        },
        move |services, event_tx| {
            let assignees = services
                .runtime
                .block_on(async { services.client.list_assignees(&owner, &repo).await });
            let _ = event_tx.send(AppEvent::RepoAssigneesSuggested {
                owner,
                repo,
                assignees: assignees.unwrap_or_default(),
            });
        },
    );
}

pub(super) fn start_fetch_repo_permissions(
    owner: String,
    repo: String,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    let error_owner = owner.clone();
    let error_repo = repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::RepoPermissionsFailed {
            owner: error_owner,
            repo: error_repo,
            message,
        },
        move |services, event_tx| {
            let result = services
                .runtime
                .block_on(async { services.client.get_repo(&owner, &repo).await });
            match result {
                Ok(repo_info) => {
                    let permissions = repo_info.permissions.unwrap_or_default();
                    let can_edit_issue_metadata = permissions.push
                        || permissions.triage
                        || permissions.maintain
                        || permissions.admin;
                    let _ = event_tx.send(AppEvent::RepoPermissionsResolved {
                        owner,
                        repo,
                        can_edit_issue_metadata,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::RepoPermissionsFailed {
                        owner,
                        repo,
                        message: error.to_string(),
                    });
                }
            }
        },
    );
}

pub(super) fn start_reopen_issue(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("reopen failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .reopen_issue(&owner, &repo, issue_number)
                    .await
            });

            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: "reopened".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("reopen failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(super) fn start_close_issue(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    body: Option<String>,
    event_tx: Sender<AppEvent>,
) {
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            issue_number,
            message: format!("close failed: {}", message),
        },
        move |services, event_tx| {
            let result: Result<Option<String>, anyhow::Error> = services.runtime.block_on(async {
                let mut comment_error = None;
                if let Some(body) = body
                    && let Err(error) = services
                        .client
                        .create_comment(&owner, &repo, issue_number, &body)
                        .await
                {
                    comment_error = Some(error.to_string());
                }

                services
                    .client
                    .close_issue(&owner, &repo, issue_number)
                    .await?;

                Ok(comment_error)
            });

            match result {
                Ok(Some(comment_error)) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("closed (comment failed: {})", comment_error),
                    });
                }
                Ok(None) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: "closed".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        issue_number,
                        message: format!("close failed: {}", error),
                    });
                }
            }
        },
    );
}
