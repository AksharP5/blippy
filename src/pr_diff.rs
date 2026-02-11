#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Meta,
    Hunk,
    Context,
    Changed,
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
    let mut pending_removed: Vec<(i64, String, String)> = Vec::new();
    let mut pending_added: Vec<(i64, String, String)> = Vec::new();

    for line in patch.lines() {
        if line.starts_with("@@") {
            flush_change_block(&mut rows, &mut pending_removed, &mut pending_added);
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
            pending_added.push((new_line, line[1..].to_string(), line.to_string()));
            new_line += 1;
            continue;
        }

        if line.starts_with('-') && !line.starts_with("---") {
            pending_removed.push((old_line, line[1..].to_string(), line.to_string()));
            old_line += 1;
            continue;
        }

        if line.starts_with(' ') {
            flush_change_block(&mut rows, &mut pending_removed, &mut pending_added);
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

        flush_change_block(&mut rows, &mut pending_removed, &mut pending_added);
        rows.push(DiffRow {
            kind: DiffKind::Meta,
            old_line: None,
            new_line: None,
            left: String::new(),
            right: String::new(),
            raw: line.to_string(),
        });
    }

    flush_change_block(&mut rows, &mut pending_removed, &mut pending_added);
    rows
}

fn flush_change_block(
    rows: &mut Vec<DiffRow>,
    pending_removed: &mut Vec<(i64, String, String)>,
    pending_added: &mut Vec<(i64, String, String)>,
) {
    if pending_removed.is_empty() && pending_added.is_empty() {
        return;
    }

    let pairs = pending_removed.len().max(pending_added.len());
    for index in 0..pairs {
        let removed = pending_removed.get(index);
        let added = pending_added.get(index);

        if let (Some((old_line, left, removed_raw)), Some((new_line, right, added_raw))) =
            (removed, added)
        {
            rows.push(DiffRow {
                kind: DiffKind::Changed,
                old_line: Some(*old_line),
                new_line: Some(*new_line),
                left: left.clone(),
                right: right.clone(),
                raw: format!("{}\n{}", removed_raw, added_raw),
            });
            continue;
        }

        if let Some((old_line, left, removed_raw)) = removed {
            rows.push(DiffRow {
                kind: DiffKind::Removed,
                old_line: Some(*old_line),
                new_line: None,
                left: left.clone(),
                right: String::new(),
                raw: removed_raw.clone(),
            });
            continue;
        }

        if let Some((new_line, right, added_raw)) = added {
            rows.push(DiffRow {
                kind: DiffKind::Added,
                old_line: None,
                new_line: Some(*new_line),
                left: String::new(),
                right: right.clone(),
                raw: added_raw.clone(),
            });
        }
    }

    pending_removed.clear();
    pending_added.clear();
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
    use super::{DiffKind, parse_patch};

    #[test]
    fn parse_patch_extracts_line_numbers_and_kinds() {
        let rows = parse_patch(Some("@@ -10,2 +20,3 @@\n line\n-old\n+new\n+more\n"));

        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].kind, DiffKind::Hunk);
        assert_eq!(rows[1].kind, DiffKind::Context);
        assert_eq!(rows[1].old_line, Some(10));
        assert_eq!(rows[1].new_line, Some(20));
        assert_eq!(rows[2].kind, DiffKind::Changed);
        assert_eq!(rows[2].old_line, Some(11));
        assert_eq!(rows[2].new_line, Some(21));
        assert_eq!(rows[3].kind, DiffKind::Added);
        assert_eq!(rows[3].new_line, Some(22));
    }

    #[test]
    fn parse_patch_aligns_replacement_blocks_line_by_line() {
        let rows = parse_patch(Some(
            "@@ -3,4 +3,4 @@\n-old-a\n-old-b\n+new-a\n+new-b\n keep\n",
        ));

        assert_eq!(rows[1].kind, DiffKind::Changed);
        assert_eq!(rows[1].old_line, Some(3));
        assert_eq!(rows[1].new_line, Some(3));
        assert_eq!(rows[2].kind, DiffKind::Changed);
        assert_eq!(rows[2].old_line, Some(4));
        assert_eq!(rows[2].new_line, Some(4));
        assert_eq!(rows[3].kind, DiffKind::Context);
    }
}
