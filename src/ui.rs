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
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
            Constraint::Length(2),
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
        Line::from(format!(
            "Branch: {}  |  staged: {}  unstaged: {}  untracked: {}",
            app.branch, staged, unstaged, untracked
        )),
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
    frame.render_widget(
        Paragraph::new(app.diff.body.clone())
            .wrap(Wrap { trim: false })
            .block(
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
        StatusLevel::Success => Color::Green,
        StatusLevel::Error => Color::Red,
    };
    let text = format!(
        "{}  |  q quit  r refresh  s stage  u unstage  tab focus  j/k move",
        app.status.text
    );
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(color))
            .block(Block::default().borders(Borders::TOP)),
        area,
    );
}
