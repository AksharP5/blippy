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

            let view_state = services.runtime.block_on(async {
                services
                    .client
                    .pull_request_file_view_state(&owner, &repo, issue_number)
                    .await
            });
            let (pull_request_id, viewed_files, view_state_error) = match view_state {
                Ok((pull_request_id, viewed_files)) => (pull_request_id, viewed_files, None),
                Err(error) => (None, HashSet::new(), Some(error.to_string())),
            };

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
                view_state_error,
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

            let _ = event_tx.send(AppEvent::PullRequestReviewCommentsUpdated {
                issue_id,
                comments: map_review_comments(comments),
            });
        },
    );
}

fn map_review_comments(
    comments: Vec<crate::github::ApiPullRequestReviewComment>,
) -> Vec<PullRequestReviewComment> {
    let mut anchors = HashMap::new();
    for comment in &comments {
        let line = comment.line;
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

    comments
        .into_iter()
        .map(|comment| {
            let anchor = anchors.get(&comment.id).cloned().or_else(|| {
                comment
                    .in_reply_to_id
                    .and_then(|reply_to_id| anchors.get(&reply_to_id).cloned())
            });
            let (line, side, path, anchored) = match anchor {
                Some((line, side, path)) => (line, side, path, true),
                None => (0, ReviewSide::Right, comment.path.clone(), false),
            };

            PullRequestReviewComment {
                id: comment.id,
                thread_id: comment.thread_id,
                resolved: comment.is_resolved,
                anchored,
                path,
                line,
                side,
                body: comment.body.unwrap_or_default(),
                author: comment
                    .user
                    .map(|user| user.login)
                    .unwrap_or_else(|| "unknown".to_string()),
                created_at: comment.created_at,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outdated_review_threads_do_not_anchor_to_the_current_diff() {
        let comment = |id, line, original_line, parent| {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "path": "src/main.rs",
                "line": line,
                "original_line": original_line,
                "side": "LEFT",
                "in_reply_to_id": parent,
                "body": "Review",
                "user": { "login": "alex" }
            }))
            .expect("API review comment")
        };
        let comments = map_review_comments(vec![
            comment(1, None, Some(10), None),
            comment(2, None, Some(10), Some(1)),
            comment(3, Some(20), Some(10), None),
            comment(4, None, None, Some(3)),
        ]);

        assert!(!comments[0].anchored);
        assert!(!comments[1].anchored);
        assert!(comments[2].anchored);
        assert!(comments[3].anchored);
        assert_eq!((comments[2].line, comments[2].side), (20, ReviewSide::Left));
        assert_eq!((comments[3].line, comments[3].side), (20, ReviewSide::Left));
    }
}
