use super::*;

pub(crate) fn start_repo_sync(
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

pub(crate) fn start_comment_sync(
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

pub(crate) fn start_fetch_labels(
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

pub(crate) fn start_fetch_assignees(
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

pub(crate) fn start_fetch_repo_permissions(
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
                    let can_merge_pull_request =
                        permissions.push || permissions.maintain || permissions.admin;
                    let _ = event_tx.send(AppEvent::RepoPermissionsResolved {
                        owner,
                        repo,
                        can_edit_issue_metadata,
                        can_merge_pull_request,
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
