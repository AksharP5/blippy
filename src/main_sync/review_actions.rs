use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_create_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    pull_number: i64,
    commit_id: String,
    path: String,
    line: i64,
    side: ReviewSide,
    start_line: Option<i64>,
    start_side: Option<ReviewSide>,
    token: String,
    body: String,
    submission: EditorSubmission,
    event_tx: Sender<AppEvent>,
) {
    let setup_submission = submission.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| {
            editor_submission_event(
                setup_submission,
                false,
                AppEvent::PullRequestReviewCommentCreateFailed { issue_id, message },
            )
        },
        move |services, event_tx| {
            let created = services.runtime.block_on(async {
                services
                    .client
                    .create_pull_request_review_comment(
                        &owner,
                        &repo,
                        pull_number,
                        commit_id.as_str(),
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
                    let _ = event_tx.send(editor_submission_event(
                        submission,
                        true,
                        AppEvent::PullRequestReviewCommentCreated { issue_id },
                    ));
                }
                Err(error) => {
                    let _ = event_tx.send(editor_submission_event(
                        submission,
                        false,
                        AppEvent::PullRequestReviewCommentCreateFailed {
                            issue_id,
                            message: error.to_string(),
                        },
                    ));
                }
            }
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_update_pull_request_review_comment(
    owner: String,
    repo: String,
    issue_id: i64,
    comment_id: i64,
    token: String,
    body: String,
    submission: EditorSubmission,
    event_tx: Sender<AppEvent>,
) {
    let setup_submission = submission.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| {
            editor_submission_event(
                setup_submission,
                false,
                AppEvent::PullRequestReviewCommentUpdateFailed { issue_id, message },
            )
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .update_pull_request_review_comment(&owner, &repo, comment_id, body.as_str())
                    .await
            });
            match result {
                Ok(()) => {
                    let _ = event_tx.send(editor_submission_event(
                        submission,
                        true,
                        AppEvent::PullRequestReviewCommentUpdated {
                            issue_id,
                            comment_id,
                            body,
                        },
                    ));
                }
                Err(error) => {
                    let _ = event_tx.send(editor_submission_event(
                        submission,
                        false,
                        AppEvent::PullRequestReviewCommentUpdateFailed {
                            issue_id,
                            message: error.to_string(),
                        },
                    ));
                }
            }
        },
    );
}

pub(crate) fn start_delete_pull_request_review_comment(
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

pub(crate) fn start_toggle_pull_request_review_thread_resolution(
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

pub(crate) fn start_set_pull_request_file_viewed(
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
