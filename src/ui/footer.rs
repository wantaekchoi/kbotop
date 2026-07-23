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
    let l = app.labels();
    let (text, style) = match &app.last_error {
        Some(err) => (
            // 긴 에러(HTTP 본문 조각 등)는 한 줄 footer에서 조용히 잘린다 —
            // 정직한 말줄임(§15 오버플로 정책).
            super::text::ellipsize(&format!("{}{err}", l.error_prefix), area.width as usize),
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        None => {
            let hint = match (&app.screen, app.tab) {
                (Screen::Live { .. }, _) => {
                    if app.live_pitch_sel.is_some() {
                        // 선택 중: Esc 1회 = 전체보기 복귀임을 명시(직관성).
                        l.hint_live_selected
                    } else {
                        l.hint_live
                    }
                }
                (Screen::List, Tab::Games) => l.hint_list_games,
                (Screen::List, Tab::Standings) => l.hint_list_standings,
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

    /// 긴 에러는 footer 폭에 맞춰 정직하게 말줄임된다(§15 오버플로 정책) —
    /// 조용한 클리핑이면 '…'가 없어 실패한다.
    #[test]
    fn long_error_is_ellipsized_to_the_footer_width() {
        let mut app = App::new(Default::default());
        app.last_error = Some("x".repeat(200));
        let text = render_to_string(&app);
        assert!(text.contains('…'), "expected honest ellipsis in:\n{text}");
    }

    /// 선택 중에는 Esc가 "전체보기 복귀"임을 힌트로 알린다 — 상태별 전환 검증.
    #[test]
    fn live_hint_switches_to_all_pitches_while_a_pitch_is_selected() {
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
        let unselected = render_to_string(&app);
        assert!(unselected.contains("Esc Back"));
        assert!(!unselected.contains("All pitches"));
        app.live_pitch_sel = Some(0);
        let selected = render_to_string(&app);
        assert!(selected.contains("Esc All pitches"));
        assert!(!selected.contains("Esc Back"));
    }

    #[test]
    fn korean_hint_renders_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        let text = render_to_string(&app);
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남긴다
        // (games.rs의 renders_full_width_korean_team_names_without_panic과 동일 사유).
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("도움말") && compact.contains("종료"),
            "unexpected: {text}"
        );
    }
}
