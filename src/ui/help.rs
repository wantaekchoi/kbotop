use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// 도움말 본문(`Labels::help_lines`, 3언어 전부 같은 배열 크기) + 테두리(상하
/// 각 1)만큼의 높이 — 하드코딩 매직넘버 대신 실제 줄 수에서 유도한다(M-7:
/// 이전엔 14로 고정해 뒀다가 9→10줄로 늘 때 갱신을 놓쳐, 정확히 그 사이에
/// 낀 터미널 높이(11행)에서 마지막 줄이 잘렸다 — 줄이 늘어도 여기가 자동으로
/// 따라가면 같은 종류의 드리프트가 재발하지 않는다).
const HELP_OVERLAY_HEIGHT: u16 = crate::ui::i18n::EN.help_lines.len() as u16 + 2;

/// 화면 중앙에 고정 크기(50 x [`HELP_OVERLAY_HEIGHT`]) 도움말 오버레이를 그린다.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let l = app.labels();
    let rect = help_rect(50, HELP_OVERLAY_HEIGHT, area);
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

    /// M-7: 높이 예산 고정 테스트 — 폭 예산을 고정하는
    /// `every_help_line_fits_the_overlay_box`(i18n.rs)의 높이 버전. 터미널이
    /// 정확히 `HELP_OVERLAY_HEIGHT`만큼만 있어도(=오버레이가 스스로 요구하는
    /// 최소 크기) 3언어 전부 help_lines 전 줄이 실제로 보여야 한다. 이전엔
    /// 이 높이가 14로 하드코딩돼 있어 줄 수가 늘어도 안 따라갔다 — 지금은
    /// help_lines.len()에서 유도하므로, 앞으로 줄이 늘어도 이 테스트가 계속
    /// 자동으로 통과해야 정상이고, 만약 다시 하드코딩으로 되돌리면 이 테스트가
    /// 잡는다.
    #[test]
    fn help_overlay_shows_every_line_when_the_terminal_is_exactly_tall_enough() {
        for lang in [
            crate::ui::i18n::Lang::Ko,
            crate::ui::i18n::Lang::En,
            crate::ui::i18n::Lang::Ja,
        ] {
            let mut app = App::new(Default::default());
            app.lang = lang;
            let mut term = Terminal::new(TestBackend::new(50, super::HELP_OVERLAY_HEIGHT)).unwrap();
            term.draw(|f| render(f, f.area(), &app)).unwrap();
            let text: String = term
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect();
            let compact_text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let l = app.labels();
            for line in l.help_lines {
                let compact_line: String = line.chars().filter(|c| !c.is_whitespace()).collect();
                assert!(
                    compact_text.contains(&compact_line),
                    "help line clipped at the exact-fit height {}: {line:?}\n{text}",
                    super::HELP_OVERLAY_HEIGHT
                );
            }
        }
    }
}
