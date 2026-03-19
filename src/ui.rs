use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::AppState;
use crate::diff::effective_diff_scroll;
use crate::layout;
use crate::model::{ChangeEntry, ChangeSection, FocusArea, StatusLevel};

pub fn render(frame: &mut Frame<'_>, app: &AppState) {
    let areas = layout::compute(frame.area());

    render_header(frame, app, areas.header);
    render_changes(frame, app, areas.changes);
    render_diff(frame, app, areas.diff);
    render_commit(frame, app, areas.commit);
    render_status(frame, app, areas.status);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffLineKind {
    Header,
    Hunk,
    Added,
    Removed,
    Plain,
}

fn render_header(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let staged = app.total_section_count(ChangeSection::Staged);
    let unstaged = app.total_section_count(ChangeSection::Unstaged);
    let untracked = app.total_section_count(ChangeSection::Untracked);
    let filter_value = if app.filter_query.is_empty() {
        "off".to_string()
    } else {
        format!(
            "{} ({}/{})",
            app.filter_query,
            app.changes.len(),
            app.total_change_count()
        )
    };
    let hunk_value = app
        .current_hunk_position()
        .map(|(index, total)| format!("{index}/{total}"))
        .unwrap_or_else(|| "none".to_string());
    let text = Text::from(vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", app.repo_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.repo_root.as_str()),
        ]),
        Line::from(vec![
            badge("branch", &app.branch, Color::LightMagenta, false),
            Span::raw("  "),
            badge(
                "wrap",
                if app.diff_wrap { "on" } else { "off" },
                Color::LightBlue,
                false,
            ),
            Span::raw(format!(
                "  staged {}  unstaged {}  untracked {}",
                staged, unstaged, untracked
            )),
        ]),
        Line::from(vec![
            badge("focus", app.focus_label(), focus_color(app.focus), true),
            Span::raw("  "),
            badge(
                "filter",
                &filter_value,
                Color::Yellow,
                app.focus == FocusArea::FilterInput,
            ),
            Span::raw("  "),
            badge(
                "hunk",
                &hunk_value,
                Color::Cyan,
                app.focus == FocusArea::DiffView,
            ),
        ]),
    ]);

    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .title("cmux-diff")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(focus_color(app.focus))),
        ),
        area,
    );
}

fn render_changes(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let mut items = Vec::new();
    let mut selected_row = None;
    let mut row_index = 0usize;
    let content_width = area.width.saturating_sub(6) as usize;

    for section in ChangeSection::ALL {
        let visible = app.section_count(section);
        let total = app.total_section_count(section);
        let count_label = if app.is_filter_active() {
            format!("{visible}/{total}")
        } else {
            total.to_string()
        };
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{} ({count_label})", section.title()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])));
        row_index += 1;

        for entry in app.entries_in_section(section) {
            if app.selected_entry_id.as_deref() == Some(entry.id().as_str()) {
                selected_row = Some(row_index);
            }
            items.push(ListItem::new(change_entry_line(entry, content_width)));
            row_index += 1;
        }

        if visible == 0 {
            items.push(ListItem::new(Line::from(vec![Span::styled(
                "  <none>",
                Style::default().fg(Color::DarkGray),
            )])));
            row_index += 1;
        }
    }

    let mut state = ListState::default();
    state.select(selected_row);
    let highlight = if app.focus == FocusArea::FileList {
        Style::default()
            .bg(Color::LightBlue)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray)
    };

    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title(pane_title(
                        "Local Changes",
                        app.focus == FocusArea::FileList,
                    ))
                    .borders(Borders::ALL)
                    .border_style(pane_border(
                        app.focus == FocusArea::FileList,
                        Color::LightBlue,
                    )),
            )
            .highlight_style(highlight)
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_diff(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let content_width = area.width.saturating_sub(2) as usize;
    let content_height = area.height.saturating_sub(2) as usize;
    let scroll = effective_diff_scroll(
        &app.diff.body,
        app.diff_scroll,
        content_width,
        content_height,
        app.diff_wrap,
    );
    let hunk_suffix = app
        .current_hunk_position()
        .map(|(index, total)| format!(" · hunk {index}/{total}"))
        .unwrap_or_default();
    let wrap_suffix = if app.diff_wrap {
        " · wrap"
    } else {
        " · nowrap"
    };

    frame.render_widget(
        Paragraph::new(styled_diff_text(&app.diff.body, content_width))
            .wrap(Wrap {
                trim: !app.diff_wrap,
            })
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .title(format!("{}{}{}", app.diff.title, hunk_suffix, wrap_suffix))
                    .borders(Borders::ALL)
                    .border_style(pane_border(app.focus == FocusArea::DiffView, Color::Cyan)),
            ),
        area,
    );
}

fn render_commit(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let help = "c focus commit | Enter/g commit | Tab next pane | Esc files";
    let text = Text::from(vec![
        Line::from(app.commit.message.as_str()),
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title(pane_title(
                        "Commit Message",
                        app.focus == FocusArea::CommitInput,
                    ))
                    .borders(Borders::ALL)
                    .border_style(pane_border(
                        app.focus == FocusArea::CommitInput,
                        Color::Green,
                    )),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_status(frame: &mut Frame<'_>, app: &AppState, area: Rect) {
    let color = match app.status.level {
        StatusLevel::Info => Color::Gray,
        StatusLevel::Success => Color::Gray,
        StatusLevel::Error => Color::Red,
    };
    let x_hint = match app.selected_entry().map(|entry| entry.section) {
        Some(ChangeSection::Staged) => "x rollback",
        Some(ChangeSection::Unstaged | ChangeSection::Untracked) => "x delete",
        None => "x rollback/delete",
    };
    let help = match app.focus {
        FocusArea::FileList => format!(
            "j/k move  s stage  u unstage  {}  n/p hunks  o open  w wrap  / filter  tab next",
            x_hint
        ),
        FocusArea::DiffView => {
            "j/k scroll  n/p hunks  o open  w wrap  / filter  tab next  Esc files".to_string()
        }
        FocusArea::CommitInput => "type message  Enter commit  tab filter  Esc files".to_string(),
        FocusArea::FilterInput => {
            "type filter  Backspace edit  Ctrl-U clear  Enter keep  tab files".to_string()
        }
    };
    let text = Text::from(vec![
        Line::from(Span::styled(
            app.status.text.as_str(),
            Style::default().fg(color),
        )),
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn badge(label: &str, value: &str, color: Color, active: bool) -> Span<'static> {
    let background = if active { color } else { Color::DarkGray };
    let foreground = if active { Color::Black } else { color };
    Span::styled(
        format!(" {label}: {value} "),
        Style::default()
            .fg(foreground)
            .bg(background)
            .add_modifier(Modifier::BOLD),
    )
}

fn pane_title(title: &str, active: bool) -> String {
    if active {
        format!("{title} • active")
    } else {
        title.to_string()
    }
}

fn pane_border(active: bool, color: Color) -> Style {
    if active {
        Style::default().fg(color)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn focus_color(focus: FocusArea) -> Color {
    match focus {
        FocusArea::FileList => Color::LightBlue,
        FocusArea::DiffView => Color::Cyan,
        FocusArea::CommitInput => Color::Green,
        FocusArea::FilterInput => Color::Yellow,
    }
}

fn styled_diff_text(body: &str, content_width: usize) -> Text<'static> {
    let mut lines = Vec::new();

    for raw_line in body.lines() {
        let kind = classify_diff_line(raw_line);
        let style = diff_line_style(kind);
        let rendered_line = if matches!(kind, DiffLineKind::Added | DiffLineKind::Removed) {
            pad_line_for_background(raw_line, content_width)
        } else {
            raw_line.to_string()
        };
        lines.push(Line::from(Span::styled(rendered_line, style)));
    }

    if lines.is_empty() {
        lines.push(Line::from(""));
    }

    Text::from(lines)
}

fn change_entry_line(entry: &ChangeEntry, content_width: usize) -> Line<'static> {
    let indent = "  ".repeat(entry.tree_depth().min(4) + 1);
    let name = entry.file_name();
    let mut parent = entry.parent_path().unwrap_or_default();
    if let Some(original) = &entry.original_path {
        if parent.is_empty() {
            parent = format!("← {original}");
        } else {
            parent = format!("{parent}  ← {original}");
        }
    }

    let stat_width = stats_width(entry);
    let left_budget = content_width.saturating_sub(stat_width);
    let min_gap = if stat_width > 0 { 1 } else { 0 };
    let base_width = indent.chars().count() + name.chars().count();
    let parent_budget = left_budget.saturating_sub(base_width + min_gap);
    let parent = truncate_for_width(&parent, parent_budget);
    let used_width = base_width
        + if parent.is_empty() {
            0
        } else {
            1 + parent.chars().count()
        }
        + min_gap;
    let spacer = " ".repeat(content_width.saturating_sub(used_width + stat_width));

    let mut spans = vec![
        Span::raw(indent),
        Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
    ];
    if !parent.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(parent, Style::default().fg(Color::DarkGray)));
    }
    if stat_width > 0 {
        spans.push(Span::raw(" "));
        spans.push(Span::raw(spacer));
        if entry.additions > 0 {
            spans.push(Span::styled(
                format!("+{}", entry.additions),
                Style::default().fg(Color::LightGreen),
            ));
        }
        if entry.additions > 0 && entry.deletions > 0 {
            spans.push(Span::raw(" "));
        }
        if entry.deletions > 0 {
            spans.push(Span::styled(
                format!("-{}", entry.deletions),
                Style::default().fg(Color::LightRed),
            ));
        }
    }

    Line::from(spans)
}

fn classify_diff_line(line: &str) -> DiffLineKind {
    if line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("similarity index ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
    {
        DiffLineKind::Header
    } else if line.starts_with("@@") {
        DiffLineKind::Hunk
    } else if line.starts_with('+') {
        DiffLineKind::Added
    } else if line.starts_with('-') {
        DiffLineKind::Removed
    } else {
        DiffLineKind::Plain
    }
}

fn diff_line_style(kind: DiffLineKind) -> Style {
    match kind {
        DiffLineKind::Header => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        DiffLineKind::Hunk => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        DiffLineKind::Added => Style::default()
            .fg(Color::Rgb(214, 255, 214))
            .bg(Color::Rgb(21, 68, 42)),
        DiffLineKind::Removed => Style::default()
            .fg(Color::Rgb(255, 217, 217))
            .bg(Color::Rgb(96, 34, 40)),
        DiffLineKind::Plain => Style::default(),
    }
}

fn pad_line_for_background(line: &str, content_width: usize) -> String {
    let visible_width = line.chars().count();
    if content_width == 0 || visible_width >= content_width {
        return line.to_string();
    }

    format!("{line:<width$}", width = content_width)
}

fn stats_width(entry: &ChangeEntry) -> usize {
    match (entry.additions > 0, entry.deletions > 0) {
        (false, false) => 0,
        (true, false) => 1 + digit_count(entry.additions),
        (false, true) => 1 + digit_count(entry.deletions),
        (true, true) => 3 + digit_count(entry.additions) + digit_count(entry.deletions),
    }
}

fn digit_count(value: usize) -> usize {
    value.max(1).to_string().len()
}

fn truncate_for_width(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let width = value.chars().count();
    if width <= max_width {
        return value.to_string();
    }

    if max_width == 1 {
        return "…".to_string();
    }

    let mut output = String::new();
    for ch in value.chars().take(max_width - 1) {
        output.push(ch);
    }
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChangeSection;

    #[test]
    fn classifies_diff_metadata_before_add_remove_markers() {
        assert_eq!(
            classify_diff_line("+++ b/src/main.rs"),
            DiffLineKind::Header
        );
        assert_eq!(
            classify_diff_line("--- a/src/main.rs"),
            DiffLineKind::Header
        );
        assert_eq!(classify_diff_line("@@ -1,2 +1,2 @@"), DiffLineKind::Hunk);
        assert_eq!(classify_diff_line("+let x = 1;"), DiffLineKind::Added);
        assert_eq!(classify_diff_line("-let x = 0;"), DiffLineKind::Removed);
        assert_eq!(classify_diff_line(" context"), DiffLineKind::Plain);
    }

    #[test]
    fn pads_added_and_removed_lines_for_full_width_highlighting() {
        let text = styled_diff_text("+new\n-old", 8);

        assert_eq!(text.lines.len(), 2);
        assert_eq!(text.lines[0].spans[0].content.as_ref(), "+new    ");
        assert_eq!(text.lines[1].spans[0].content.as_ref(), "-old    ");
        assert_eq!(
            text.lines[0].spans[0].style.bg,
            Some(Color::Rgb(21, 68, 42))
        );
        assert_eq!(
            text.lines[1].spans[0].style.bg,
            Some(Color::Rgb(96, 34, 40))
        );
    }

    #[test]
    fn active_pane_titles_are_labeled() {
        assert_eq!(pane_title("Diff", true), "Diff • active");
        assert_eq!(pane_title("Diff", false), "Diff");
    }

    #[test]
    fn change_rows_include_parent_path_and_diff_stats() {
        let entry = ChangeEntry {
            section: ChangeSection::Unstaged,
            path: "src/deep/file.rs".to_string(),
            original_path: None,
            staged_status: None,
            unstaged_status: Some('M'),
            additions: 12,
            deletions: 3,
        };

        let line = change_entry_line(&entry, 40);
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(rendered.contains("file.rs"));
        assert!(rendered.contains("src/deep"));
        assert!(rendered.contains("+12"));
        assert!(rendered.contains("-3"));
    }

    #[test]
    fn truncates_parent_paths_to_fit_available_width() {
        assert_eq!(truncate_for_width("src/really/long/path", 8), "src/rea…");
        assert_eq!(truncate_for_width("short", 8), "short");
        assert_eq!(truncate_for_width("path", 1), "…");
    }
}
