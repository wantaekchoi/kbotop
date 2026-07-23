use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// 화면 중앙에 고정 크기(50x14) 도움말 오버레이를 그린다.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let l = app.labels();
    let rect = help_rect(50, 14, area);
    let lines: Vec<Line> = l.help_lines.iter().map(|s| Line::from(*s)).collect();
    let block = Block::bordered().title(l.title_help);
    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(Clear, rect);
    f.render_widget(paragraph, rect);
}

/// 주어진 영역 내부에서 고정 크기(width x height)의 중앙 사각형을 계산한다.
/// area보다 크면 area에 맞춰 줄인다. options::chooser도 재사용한다.
pub(crate) fn help_rect(width: u16, height: u16, area: Rect) -> Rect {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn korean_help_lines_render_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        let text = render_to_string(&app);
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남긴다
        // (games.rs의 renders_full_width_korean_team_names_without_panic과 동일 사유).
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("도움말"),
            "expected Korean title in:\n{text}"
        );
        assert!(
            compact.contains("이동"), // help_lines[0]의 선두 단어
            "expected first Korean help line in:\n{text}"
        );
    }
}
