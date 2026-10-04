use super::*;

pub(crate) fn start_add_comment(
    owner: String,
    repo: String,
    issue_number: i64,
    token: String,
    body: String,
    event_tx: Sender<AppEvent>,
) {
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
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
                        repo: event_repo.clone(),
                        issue_number,
                        message: "commented".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number,
                        message: format!("comment failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(crate) fn start_create_issue(
    owner: String,
    repo: String,
    token: String,
    title: String,
    body: Option<String>,
    event_tx: Sender<AppEvent>,
) {
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_db(
        token,
        event_tx,
        move |message| AppEvent::IssueCreateFailed {
            repo: setup_repo,
            message,
        },
        move |ctx, event_tx| {
            let result = ctx.services.runtime.block_on(async {
                let repo_row = match crate::store::get_repo_by_slug(&ctx.conn, &owner, &repo)? {
                    Some(repo_row) => repo_row,
                    None => {
                        let repo_info = ctx.services.client.get_repo(&owner, &repo).await?;
                        let repo_row = crate::sync::map_repo_to_row(&repo_info);
                        crate::store::upsert_repo(&ctx.conn, &repo_row)?;
                        repo_row
                    }
                };
                let issue = ctx
                    .services
                    .client
                    .create_issue(&owner, &repo, title.as_str(), body.as_deref())
                    .await?;
                Ok::<_, anyhow::Error>((repo_row, issue))
            });

            match result {
                Ok((repo_row, issue)) => {
                    let _ = event_tx.send(AppEvent::IssueCreated {
                        repo: event_repo.clone(),
                        issue: crate::sync::map_issue_to_row(repo_row.id, &issue),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueCreateFailed {
                        repo: event_repo.clone(),
                        message: error.to_string(),
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
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
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
                    let _ = event_tx.send(AppEvent::IssueCommentUpdated {
                        repo: event_repo.clone(),
                        issue_number,
                        comment_id,
                        body,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
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
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
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
                    let _ = event_tx.send(AppEvent::IssueCommentDeleted {
                        repo: event_repo.clone(),
                        issue_number,
                        issue_id,
                        comment_id,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
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
) {
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
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
                        repo: event_repo.clone(),
                        issue_number,
                        labels,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
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
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
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
                        repo: event_repo.clone(),
                        issue_number,
                        assignees: assignees_display,
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
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
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
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
                        repo: event_repo.clone(),
                        issue_number,
                        message: "reopened".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number,
                        message: format!("reopen failed: {}", error),
                    });
                }
            }
        },
    );
}

pub(crate) fn start_merge_pull_request(
    owner: String,
    repo: String,
    pull_number: i64,
    token: String,
    event_tx: Sender<AppEvent>,
) {
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
            issue_number: pull_number,
            message: format!("merge failed: {}", message),
        },
        move |services, event_tx| {
            let result = services.runtime.block_on(async {
                services
                    .client
                    .merge_pull_request(&owner, &repo, pull_number)
                    .await
            });

            match result {
                Ok(()) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number: pull_number,
                        message: "merged".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number: pull_number,
                        message: format!("merge failed: {}", error),
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
    let event_repo = RepoIdentity::new(&owner, &repo);
    let setup_repo = event_repo.clone();
    spawn_with_services(
        token,
        event_tx,
        move |message| AppEvent::IssueUpdated {
            repo: setup_repo,
            issue_number,
            message: format!("close failed: {}", message),
        },
        move |services, event_tx| {
            let result: Result<Option<String>, anyhow::Error> = services.runtime.block_on(async {
                services
                    .client
                    .close_issue(&owner, &repo, issue_number)
                    .await?;

                let mut comment_error = None;
                if let Some(body) = body
                    && let Err(error) = services
                        .client
                        .create_comment(&owner, &repo, issue_number, &body)
                        .await
                {
                    comment_error = Some(error.to_string());
                }

                Ok(comment_error)
            });

            match result {
                Ok(Some(comment_error)) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number,
                        message: format!("closed (comment failed: {})", comment_error),
                    });
                }
                Ok(None) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number,
                        message: "closed".to_string(),
                    });
                }
                Err(error) => {
                    let _ = event_tx.send(AppEvent::IssueUpdated {
                        repo: event_repo.clone(),
                        issue_number,
                        message: format!("close failed: {}", error),
                    });
                }
            }
        },
    );
}
