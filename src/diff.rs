use crate::model::DiffHunk;

pub fn effective_diff_scroll(
    body: &str,
    requested: u16,
    content_width: usize,
    content_height: usize,
    wrap: bool,
) -> u16 {
    if content_width == 0 || content_height == 0 {
        return 0;
    }

    let total_rows = displayed_row_count(body, content_width, wrap);
    let max_scroll = total_rows.saturating_sub(content_height) as u16;
    requested.min(max_scroll)
}

pub fn displayed_row_count(body: &str, content_width: usize, wrap: bool) -> usize {
    if content_width == 0 {
        return 0;
    }

    let mut total = 0usize;
    for line in body.lines() {
        total += displayed_rows_for_line(line, content_width, wrap);
    }

    total.max(1)
}

pub fn raw_line_index_for_scroll(
    body: &str,
    scroll: u16,
    content_width: usize,
    wrap: bool,
) -> usize {
    if content_width == 0 {
        return 0;
    }

    let mut current_offset = 0usize;
    for (index, line) in body.lines().enumerate() {
        let line_rows = displayed_rows_for_line(line, content_width, wrap);
        if scroll as usize <= current_offset + line_rows.saturating_sub(1) {
            return index;
        }
        current_offset += line_rows;
    }

    body.lines().count().saturating_sub(1)
}

pub fn scroll_for_raw_line(
    body: &str,
    target_line: usize,
    content_width: usize,
    wrap: bool,
) -> u16 {
    if content_width == 0 {
        return 0;
    }

    let mut current_offset = 0usize;
    for (index, line) in body.lines().enumerate() {
        if index == target_line {
            return current_offset.min(u16::MAX as usize) as u16;
        }
        current_offset += displayed_rows_for_line(line, content_width, wrap);
    }

    current_offset.min(u16::MAX as usize) as u16
}

pub fn parse_diff_hunks(body: &str) -> Vec<DiffHunk> {
    body.lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            parse_hunk_new_start(line).map(|new_start| DiffHunk {
                line_index,
                new_start,
            })
        })
        .collect()
}

fn parse_hunk_new_start(line: &str) -> Option<usize> {
    if !line.starts_with("@@") {
        return None;
    }

    let plus_start = line.find(" +")?;
    let remainder = &line[plus_start + 2..];
    let line_number = remainder
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .next()?;
    line_number.parse().ok()
}

fn displayed_rows_for_line(line: &str, content_width: usize, wrap: bool) -> usize {
    if !wrap {
        return 1;
    }

    let width = line.chars().count().max(1);
    width.div_ceil(content_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_math_respects_wrapped_lines() {
        let body = "abcdefgh\nx";
        assert_eq!(displayed_row_count(body, 4, true), 3);
        assert_eq!(raw_line_index_for_scroll(body, 1, 4, true), 0);
        assert_eq!(raw_line_index_for_scroll(body, 2, 4, true), 1);
        assert_eq!(scroll_for_raw_line(body, 1, 4, true), 2);
    }

    #[test]
    fn scroll_math_respects_unwrapped_lines() {
        let body = "abcdefgh\nijklmnop\nx";
        assert_eq!(displayed_row_count(body, 4, false), 3);
        assert_eq!(effective_diff_scroll(body, 99, 4, 2, false), 1);
        assert_eq!(scroll_for_raw_line(body, 2, 4, false), 2);
    }

    #[test]
    fn parses_hunk_locations_from_unified_diff() {
        let body = "\
diff --git a/src/main.rs b/src/main.rs
@@ -1,2 +4,7 @@
 context
@@ -9 +20,3 @@ fn thing()
";

        let hunks = parse_diff_hunks(body);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].line_index, 1);
        assert_eq!(hunks[0].new_start, 4);
        assert_eq!(hunks[1].new_start, 20);
    }
}
