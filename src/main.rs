use std::env;
use std::io;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use cmux_diff::app::AppState;
use cmux_diff::layout;
use cmux_diff::model::{FocusArea, StatusMessage};
use cmux_diff::ui;
use ratatui::Terminal;
use ratatui::backend::TermionBackend;
use termion::async_stdin;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

fn main() -> Result<()> {
    let path = env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let mut app = AppState::new(path.as_ref())?;

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = stdout.into_alternate_screen()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut keys = async_stdin().keys();

    loop {
        let areas = layout::compute(terminal.size()?.into());
        app.set_diff_viewport(
            areas.diff.width.saturating_sub(2) as usize,
            areas.diff.height.saturating_sub(2) as usize,
        );
        terminal.draw(|frame| ui::render(frame, &app))?;

        if let Some(next_key) = keys.next() {
            match next_key {
                Ok(key) => {
                    if handle_key(key, &mut app)? {
                        break;
                    }
                }
                Err(error) => {
                    app.status = StatusMessage::error(format!("Input error: {error}"));
                }
            }
        } else {
            thread::sleep(Duration::from_millis(30));
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

fn handle_key(key: Key, app: &mut AppState) -> Result<bool> {
    match app.focus {
        FocusArea::FileList => match key {
            Key::Char('q') => return Ok(true),
            Key::Char('j') | Key::Down => {
                app.move_selection(1)?;
            }
            Key::Char('k') | Key::Up => {
                app.move_selection(-1)?;
            }
            Key::Char('r') => {
                app.refresh(None)?;
            }
            Key::Char('s') => {
                app.stage_selected()?;
            }
            Key::Char('u') => {
                app.unstage_selected()?;
            }
            Key::Char('x') => {
                app.discard_selected()?;
            }
            Key::Char('o') => {
                app.open_selected_in_editor()?;
            }
            Key::Char('n') => {
                app.jump_to_next_hunk();
            }
            Key::Char('p') => {
                app.jump_to_previous_hunk();
            }
            Key::Char('w') => {
                app.toggle_diff_wrap();
            }
            Key::Char('c') => {
                app.focus_commit();
            }
            Key::Char('/') => {
                app.focus_filter();
            }
            Key::Char('g') => {
                app.commit()?;
            }
            Key::Char('\t') => {
                app.toggle_focus();
            }
            _ => {}
        },
        FocusArea::DiffView => match key {
            Key::Char('q') => return Ok(true),
            Key::Char('j') | Key::Down => {
                app.scroll_diff(1);
            }
            Key::Char('k') | Key::Up => {
                app.scroll_diff(-1);
            }
            Key::Char('n') => {
                app.jump_to_next_hunk();
            }
            Key::Char('p') => {
                app.jump_to_previous_hunk();
            }
            Key::Char('o') => {
                app.open_selected_in_editor()?;
            }
            Key::Char('w') => {
                app.toggle_diff_wrap();
            }
            Key::Char('/') => {
                app.focus_filter();
            }
            Key::Char('r') => {
                app.refresh(None)?;
            }
            Key::Char('\t') => {
                app.toggle_focus();
            }
            Key::Esc => {
                app.focus_file_list();
            }
            _ => {}
        },
        FocusArea::CommitInput => match key {
            Key::Char('q') => return Ok(true),
            Key::Esc => {
                app.focus_file_list();
            }
            Key::Backspace => {
                app.backspace_commit();
            }
            Key::Char('\n') => {
                app.commit()?;
            }
            Key::Char('\t') => {
                app.toggle_focus();
            }
            Key::Char(c) if !c.is_control() => {
                app.push_commit_char(c);
            }
            _ => {}
        },
        FocusArea::FilterInput => match key {
            Key::Char('q') => return Ok(true),
            Key::Esc => {
                app.focus_file_list();
            }
            Key::Backspace => {
                app.backspace_filter()?;
            }
            Key::Ctrl('u') => {
                app.clear_filter()?;
            }
            Key::Char('\n') => {
                app.focus_file_list();
            }
            Key::Char('\t') => {
                app.toggle_focus();
            }
            Key::Char(c) if !c.is_control() => {
                app.push_filter_char(c)?;
            }
            _ => {}
        },
    }

    Ok(false)
}
