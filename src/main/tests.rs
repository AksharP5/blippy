use super::main_actions::issue_url;
use crate::app::View;
use crate::config::Config;
use crate::store::IssueRow;

fn parse_csv_values(input: &str, strip_at: bool) -> Vec<String> {
    let mut values = Vec::new();
    for raw in input.split(',') {
        let mut value = raw.trim().to_string();
        if strip_at {
            value = value.trim_start_matches('@').to_string();
        }
        if value.is_empty() {
            continue;
        }
        if values
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(value.as_str()))
        {
            continue;
        }
        values.push(value);
    }
    values
}

#[test]
fn parse_csv_values_trims_dedupes_and_strips_at() {
    let values = parse_csv_values(" @alex,alex, sam , ,@Sam", true);
    assert_eq!(values, vec!["alex".to_string(), "sam".to_string()]);
}

#[test]
fn parse_csv_values_keeps_label_case() {
    let values = parse_csv_values("bug,needs-triage,BUG", false);
    assert_eq!(values, vec!["bug".to_string(), "needs-triage".to_string()]);
}

#[test]
fn issue_url_uses_pull_route_for_pull_requests() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 10,
        repo_id: 1,
        number: 42,
        state: "open".to_string(),
        title: "Improve docs".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: true,
    }]);
    app.set_current_issue(10, 42);
    app.set_view(View::IssueDetail);

    let url = issue_url(&app).expect("url");

    assert_eq!(url, "https://github.com/acme/blippy/pull/42");
}

#[test]
fn issue_url_uses_issue_route_for_issues() {
    let mut app = crate::app::App::new(Config::default());
    app.set_current_repo_with_path("acme", "blippy", None);
    app.set_view(View::Issues);
    app.set_issues(vec![IssueRow {
        id: 11,
        repo_id: 1,
        number: 7,
        state: "open".to_string(),
        title: "Bug".to_string(),
        body: String::new(),
        labels: String::new(),
        assignees: String::new(),
        comments_count: 0,
        updated_at: None,
        is_pr: false,
    }]);

    let url = issue_url(&app).expect("url");

    assert_eq!(url, "https://github.com/acme/blippy/issues/7");
}
