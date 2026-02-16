use super::*;

pub(crate) fn checkout_pull_request(app: &mut App) -> Result<()> {
    let issue = match app.current_or_selected_issue() {
        Some(issue) => issue,
        None => {
            app.set_status("No pull request selected".to_string());
            return Ok(());
        }
    };
    if !issue.is_pr {
        app.set_status("Selected item is not a pull request".to_string());
        return Ok(());
    }

    let working_dir = app.current_repo_path().unwrap_or(".").to_string();
    let issue_number = issue.number;
    let number = issue_number.to_string();
    let before_branch = current_git_branch(working_dir.as_str());
    let before_head = current_git_head(working_dir.as_str());

    let output = std::process::Command::new("gh")
        .args(["pr", "checkout", number.as_str()])
        .current_dir(working_dir.as_str())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            app.set_status(format!("PR checkout failed: {}", error));
            return Ok(());
        }
    };

    if output.status.success() {
        return finalize_checkout_status(
            app,
            working_dir.as_str(),
            issue_number,
            before_branch,
            before_head,
        );
    }

    let detached_output = std::process::Command::new("gh")
        .args(["pr", "checkout", number.as_str(), "--detach"])
        .current_dir(working_dir.as_str())
        .output();

    if detached_output
        .as_ref()
        .is_ok_and(|out| out.status.success())
    {
        return finalize_checkout_status(
            app,
            working_dir.as_str(),
            issue_number,
            before_branch,
            before_head,
        );
    }

    let primary_message = command_error_message(&output);
    let detached_message = detached_output
        .as_ref()
        .map(command_error_message)
        .unwrap_or_else(|error| error.to_string());
    let combined = if detached_message.is_empty() || detached_message == primary_message {
        primary_message
    } else if primary_message.is_empty() {
        detached_message
    } else {
        format!("{}; fallback failed: {}", primary_message, detached_message)
    };

    if combined.is_empty() {
        app.set_status(format!("PR checkout failed for #{}", issue_number));
        return Ok(());
    }

    app.set_status(format!("PR checkout failed: {}", combined));
    Ok(())
}

pub(crate) fn finalize_checkout_status(
    app: &mut App,
    working_dir: &str,
    issue_number: i64,
    before_branch: Option<String>,
    before_head: Option<String>,
) -> Result<()> {
    let after_branch = current_git_branch(working_dir);
    let after_head = current_git_head(working_dir);

    if before_branch == after_branch && before_head == after_head {
        if let Some(branch) = after_branch {
            app.set_status(format!(
                "PR #{} already active on {} (no checkout changes)",
                issue_number, branch
            ));
            return Ok(());
        }
        app.set_status(format!(
            "PR #{} already active (no checkout changes)",
            issue_number
        ));
        return Ok(());
    }

    if let Some(branch) = after_branch {
        app.set_status(format!("Checked out PR #{} on {}", issue_number, branch));
        return Ok(());
    }

    app.set_status(format!("Checked out PR #{}", issue_number));
    Ok(())
}

pub(crate) fn command_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(output.stderr.as_slice())
        .trim()
        .to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string()
}

pub(crate) fn current_git_branch(working_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}

pub(crate) fn current_git_head(working_dir: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(working_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(output.stdout.as_slice())
        .trim()
        .to_string();
    if value.is_empty() {
        return None;
    }
    Some(value)
}
