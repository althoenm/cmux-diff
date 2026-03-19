use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct LayoutAreas {
    pub header: Rect,
    pub changes: Rect,
    pub diff: Rect,
    pub commit: Rect,
    pub status: Rect,
}

pub fn compute(frame: Rect) -> LayoutAreas {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(frame);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(vertical[1]);

    LayoutAreas {
        header: vertical[0],
        changes: body[0],
        diff: body[1],
        commit: vertical[2],
        status: vertical[3],
    }
}
