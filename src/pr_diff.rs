#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Meta,
    Hunk,
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub kind: DiffKind,
    pub old_line: Option<i64>,
    pub new_line: Option<i64>,
    pub left: String,
    pub right: String,
    pub raw: String,
}

pub fn parse_patch(patch: Option<&str>) -> Vec<DiffRow> {
    let patch = match patch {
        Some(patch) => patch,
        None => return Vec::new(),
    };

    let mut rows = Vec::new();
    let mut old_line = 0i64;
    let mut new_line = 0i64;

    for line in patch.lines() {
        if line.starts_with("@@") {
            let (next_old, next_new) = parse_hunk_header(line).unwrap_or((old_line, new_line));
            old_line = next_old;
            new_line = next_new;
            rows.push(DiffRow {
                kind: DiffKind::Hunk,
                old_line: None,
                new_line: None,
                left: String::new(),
                right: String::new(),
                raw: line.to_string(),
            });
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            rows.push(DiffRow {
                kind: DiffKind::Added,
                old_line: None,
                new_line: Some(new_line),
                left: String::new(),
                right: line[1..].to_string(),
                raw: line.to_string(),
            });
            new_line += 1;
            continue;
        }

        if line.starts_with('-') && !line.starts_with("---") {
            rows.push(DiffRow {
                kind: DiffKind::Removed,
                old_line: Some(old_line),
                new_line: None,
                left: line[1..].to_string(),
                right: String::new(),
                raw: line.to_string(),
            });
            old_line += 1;
            continue;
        }

        if line.starts_with(' ') {
            rows.push(DiffRow {
                kind: DiffKind::Context,
                old_line: Some(old_line),
                new_line: Some(new_line),
                left: line[1..].to_string(),
                right: line[1..].to_string(),
                raw: line.to_string(),
            });
            old_line += 1;
            new_line += 1;
            continue;
        }

        rows.push(DiffRow {
            kind: DiffKind::Meta,
            old_line: None,
            new_line: None,
            left: String::new(),
            right: String::new(),
            raw: line.to_string(),
        });
    }

    rows
}

fn parse_hunk_header(line: &str) -> Option<(i64, i64)> {
    let parts = line.split_whitespace().collect::<Vec<&str>>();
    if parts.len() < 3 {
        return None;
    }
    if parts[0] != "@@" {
        return None;
    }
    let old = parts[1];
    let new = parts[2];
    let old = old.strip_prefix('-')?;
    let new = new.strip_prefix('+')?;
    let old = old.split(',').next()?.parse::<i64>().ok()?;
    let new = new.split(',').next()?.parse::<i64>().ok()?;
    Some((old, new))
}

#[cfg(test)]
mod tests {
    use super::{parse_patch, DiffKind};

    #[test]
    fn parse_patch_extracts_line_numbers_and_kinds() {
        let rows = parse_patch(Some(
            "@@ -10,2 +20,3 @@\n line\n-old\n+new\n+more\n",
        ));

        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].kind, DiffKind::Hunk);
        assert_eq!(rows[1].kind, DiffKind::Context);
        assert_eq!(rows[1].old_line, Some(10));
        assert_eq!(rows[1].new_line, Some(20));
        assert_eq!(rows[2].kind, DiffKind::Removed);
        assert_eq!(rows[2].old_line, Some(11));
        assert_eq!(rows[3].kind, DiffKind::Added);
        assert_eq!(rows[3].new_line, Some(21));
        assert_eq!(rows[4].kind, DiffKind::Added);
        assert_eq!(rows[4].new_line, Some(22));
    }
}
