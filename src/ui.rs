use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::AppState;
use crate::model::{ChangeSection, FocusArea, StatusLevel};

pub fn render(frame: &mut Frame<'_>, app: &AppState) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, app, vertical[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(vertical[1]);

    render_changes(frame, app, body[0]);
    render_diff(frame, app, body[1]);
    render_commit(frame, app, vertical[2]);
    render_status(frame, app, vertical[3]);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffLineKind {
    Header,
    Hunk,
    Added,
    Removed,
    Plain,
}

fn render_header(frame: &mut Frame<'_>, app: &AppState, area: ratatui::layout::Rect) {
    let staged = app.section_count(ChangeSection::Staged);
    let unstaged = app.section_count(ChangeSection::Unstaged);
    let untracked = app.section_count(ChangeSection::Untracked);
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
            Span::styled(
                "Branch ",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", app.branch),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  |  staged: {}  unstaged: {}  untracked: {}",
                staged, unstaged, untracked
            )),
        ]),
    ]);

    frame.render_widget(
        Paragraph::new(text).block(Block::default().title("cmux-diff").borders(Borders::ALL)),
        area,
    );
}

fn render_changes(frame: &mut Frame<'_>, app: &AppState, area: ratatui::layout::Rect) {
    let mut items = Vec::new();
    let mut selected_row = None;
    let mut row_index = 0usize;

    for section in ChangeSection::ALL {
        let count = app.section_count(section);
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!("{} ({count})", section.title()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])));
        row_index += 1;

        for entry in app.entries_in_section(section) {
            if app.selected_entry_id.as_deref() == Some(entry.id().as_str()) {
                selected_row = Some(row_index);
            }
            items.push(ListItem::new(Line::from(vec![Span::raw(format!(
                "  {}",
                entry.display_path()
            ))])));
            row_index += 1;
        }

        if count == 0 {
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
        Style::default().bg(Color::Blue).fg(Color::Black)
    } else {
        Style::default().bg(Color::DarkGray)
    };

    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title("Local Changes")
                    .borders(Borders::ALL),
            )
            .highlight_style(highlight)
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

fn render_diff(frame: &mut Frame<'_>, app: &AppState, area: ratatui::layout::Rect) {
    let content_width = area.width.saturating_sub(2) as usize;
    frame.render_widget(
        Paragraph::new(styled_diff_text(&app.diff.body, content_width)).block(
            Block::default()
                .title(app.diff.title.clone())
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_commit(frame: &mut Frame<'_>, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.focus == FocusArea::CommitInput {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let help = "c focus commit | Enter/g commit | Esc back to file list";
    let text = Text::from(vec![
        Line::from(app.commit.message.as_str()),
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title("Commit Message")
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_status(frame: &mut Frame<'_>, app: &AppState, area: ratatui::layout::Rect) {
    let color = match app.status.level {
        StatusLevel::Info => Color::Gray,
        StatusLevel::Success => Color::Gray,
        StatusLevel::Error => Color::Red,
    };
    let x_hint = match app.selected_entry().map(|entry| entry.section) {
        Some(ChangeSection::Staged) => "x rollback",
        Some(ChangeSection::Unstaged | ChangeSection::Untracked) => "x delete",
        None => "x delete/rollback",
    };
    let help = format!(
        "q quit  r refresh  s stage  u unstage  {}  tab focus  j/k move",
        x_hint
    );
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
