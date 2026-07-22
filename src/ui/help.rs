use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// 화면 중앙에 고정 크기(50x14) 도움말 오버레이를 그린다.
pub fn render(f: &mut Frame, area: Rect) {
    let rect = centered_rect(50, 14, area);
    let lines = vec![
        Line::from("Move       j / k or Up / Down"),
        Line::from("Top/Bottom gg / G"),
        Line::from("Open live  Enter"),
        Line::from("Back       Esc"),
        Line::from("Switch tab Tab / F5"),
        Line::from("Find       / (coming soon)"),
        Line::from("Help       ? / F1"),
        Line::from("Quit       q / F10"),
    ];
    let block = Block::bordered().title(" Help ");
    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(Clear, rect);
    f.render_widget(paragraph, rect);
}

/// 주어진 영역 내부에서 고정 크기(width x height)의 중앙 사각형을 계산한다.
/// area보다 크면 area에 맞춰 줄인다.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1]);

    horizontal[1]
}
