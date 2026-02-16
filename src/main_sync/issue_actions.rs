use super::*;

pub(crate) fn start_add_comment(
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

pub(crate) fn start_update_comment(
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

pub(crate) fn start_delete_comment(
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

pub(crate) fn start_update_labels(
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

pub(crate) fn start_update_assignees(
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

pub(crate) fn start_reopen_issue(
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

pub(crate) fn start_close_issue(
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
