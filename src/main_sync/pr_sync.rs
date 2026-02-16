use super::*;

pub(crate) fn start_pull_request_files_sync(
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

pub(crate) fn start_pull_request_review_comments_sync(
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
