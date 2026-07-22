pub mod footer;
pub mod games;
pub mod header;
pub mod help;
pub mod live;
pub mod standings;
pub mod strikezone;
pub mod theme;

use crate::app::{App, Screen, Tab};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

/// htop 계승: header(요약) + 본문(Min) + footer(기능키 바) 3단 레이아웃.
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    header::render(f, chunks[0], app);

    match &app.screen {
        Screen::Live { .. } => live::render(f, chunks[1], app),
        Screen::List => match app.tab {
            Tab::Games => games::render(f, chunks[1], app),
            Tab::Standings => standings::render(f, chunks[1], app),
        },
    }

    footer::render(f, chunks[2], app);

    if app.show_help {
        help::render(f, f.area());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Game, GameStatus, Team};
    use crate::poller::Update;
    use ratatui::{backend::TestBackend, Terminal};

    fn game(id: &str, status: GameStatus, label: &str) -> Game {
        Game {
            id: id.into(),
            start: "2026-07-19T18:00:00".into(),
            status,
            status_label: label.into(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            home_score: Some(3),
            away_score: Some(2),
        }
    }

    fn render_to_string(app: &App) -> String {
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        buf.content().iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn renders_games_list_without_panic() {
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![
            game("g1", GameStatus::Live, "5회초"),
            game("g2", GameStatus::Final, "경기종료"),
        ]));
        let text = render_to_string(&app);
        assert!(text.contains("LG"));
        assert!(text.contains("KT"));
        assert!(text.contains("LIVE"));
        assert!(text.contains("FIN"));
    }

    #[test]
    fn renders_standings_tab() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![crate::model::Standing {
            rank: 1,
            team: Team {
                code: "HT".into(),
                name: "KIA".into(),
            },
            games: 10,
            wins: 7,
            losses: 3,
            draws: 0,
            win_rate: 0.700,
            game_behind: 0.0,
        }]));
        let text = render_to_string(&app);
        assert!(text.contains("KIA"));
    }

    fn standing(rank: u16, code: &str, name: &str) -> crate::model::Standing {
        crate::model::Standing {
            rank,
            team: Team {
                code: code.into(),
                name: name.into(),
            },
            games: 10,
            wins: rank,
            losses: 3,
            draws: 0,
            win_rate: 0.5,
            game_behind: 0.0,
        }
    }

    /// standings.rs가 games.rs(69행)처럼 TableState로 stateful 렌더해야
    /// app.selected가 highlight_symbol("> ")로 반영된다 — j/k/gg/G가 Standings
    /// 탭에서도 시각적 효과를 내는지에 대한 회귀 방지.
    #[test]
    fn standings_selection_is_reflected_with_highlight_symbol() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![
            standing(1, "HT", "KIA"),
            standing(2, "LG", "LG"),
        ]));
        app.selected = 1;
        let text = render_to_string(&app);
        assert!(text.contains("> "));
    }

    // "Help" 자체는 footer의 "F1 Help" 힌트에도 항상 나타나 tautology가 되므로,
    // 오버레이 본문에만 있는 "Top/Bottom" 문자열로 검증한다(footer/header에는 없음).
    #[test]
    fn help_overlay_renders_when_shown() {
        let mut app = App::new(Default::default());
        app.show_help = true;
        let text = render_to_string(&app);
        assert!(text.contains("Top/Bottom"));
    }

    #[test]
    fn help_overlay_absent_when_not_shown() {
        let app = App::new(Default::default());
        assert!(!app.show_help);
        let text = render_to_string(&app);
        assert!(!text.contains("Top/Bottom"));
    }

    /// 전각(한글) 팀명이 섞여도 패닉 없이 렌더되는지 확인 — §7 폭 안정 회귀 방지.
    #[test]
    fn renders_full_width_korean_team_names_without_panic() {
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![Game {
            id: "g".into(),
            start: "2026-07-19T18:00:00".into(),
            status: GameStatus::Live,
            status_label: "9회말".into(),
            home: Team {
                code: "HT".into(),
                name: "기아타이거즈".into(),
            },
            away: Team {
                code: "OB".into(),
                name: "두산베어스".into(),
            },
            home_score: Some(10),
            away_score: Some(9),
        }]));
        // ratatui는 전각(2-width) 문자 뒤에 placeholder 공백 셀을 채워 넣는다
        // (정상 동작 — 실제 터미널 폭 계산과 일치). 공백을 제거하고 문자 순서만 검증한다.
        let text: String = render_to_string(&app)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(text.contains("기아타이거즈"));
        assert!(text.contains("두산베어스"));
    }
}
