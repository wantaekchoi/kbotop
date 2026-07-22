use crate::app::{App, Screen, Tab};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
    Frame,
};

/// htop 기능키 바: 반전 스타일 한 줄. 최근 에러가 있으면 힌트 대신 그 내용을
/// 보여줘 화면이 왜 stale인지 알 수 있게 한다("/ Find"는 아직 미구현이라
/// 힌트에서 뺐다 — help.rs와 동일 사유).
///
/// 화면(List/Live)과 탭(Games/Standings)에 따라 힌트를 바꾼다 — Live 화면에서
/// "Enter Live"는 이미 진입한 화면이라 no-op이고(app.rs의 Enter 핸들러는
/// Screen::List에서만 동작), 목록으로 돌아가는 유일한 키인 Esc는 어디에도
/// 안내되지 않아 발견 불가능했다. 마찬가지로 app.rs의 Enter 핸들러는
/// `tab == Tab::Games`일 때만 라이브 화면을 여므로(Standings 탭에서는
/// no-op), Standings 탭에서는 "Enter Live"를 보여주지 않는다.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let (text, style) = match &app.last_error {
        Some(err) => (
            format!(" ERROR: {err}"),
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        None => {
            let hint = match (&app.screen, app.tab) {
                (Screen::Live { .. }, _) => " F1 Help   Esc Back   q Quit",
                (Screen::List, Tab::Games) => " F1 Help   Tab Switch   Enter Live   q Quit",
                (Screen::List, Tab::Standings) => " F1 Help   Tab Switch   q Quit",
            };
            (
                hint.to_string(),
                Style::default().add_modifier(Modifier::REVERSED),
            )
        }
    };
    let paragraph = Paragraph::new(text).style(style);
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Game, GameStatus, Team};
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(80, 3)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn list_screen_hint_advertises_enter_live_not_esc() {
        let app = App::new(Default::default());
        let text = render_to_string(&app);
        assert!(text.contains("Enter Live"));
        assert!(!text.contains("Esc"));
    }

    #[test]
    fn live_screen_hint_advertises_esc_back_not_enter_live() {
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: "".into(),
                home: Team {
                    code: "LG".into(),
                    name: "LG".into(),
                },
                away: Team {
                    code: "KT".into(),
                    name: "KT".into(),
                },
                home_score: None,
                away_score: None,
            },
            state: None,
        };
        let text = render_to_string(&app);
        assert!(text.contains("Esc Back"));
        assert!(!text.contains("Enter Live"));
    }

    /// app.rs의 Enter 핸들러는 `tab == Tab::Games`일 때만 라이브 화면을 연다 —
    /// Standings 탭에서 Enter는 no-op이므로 힌트가 그걸 광고해서는 안 된다.
    #[test]
    fn standings_tab_hint_does_not_advertise_enter_live() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        let text = render_to_string(&app);
        assert!(!text.contains("Enter Live"));
        assert!(!text.contains("Esc"));
    }
}
