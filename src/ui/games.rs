use super::theme::team_badge_style;
use crate::app::App;
use crate::model::GameStatus;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

fn status_tag(status: GameStatus) -> (&'static str, Style) {
    match status {
        GameStatus::Live => (
            "LIVE",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        GameStatus::Scheduled => ("SCHED", Style::default().fg(Color::Yellow)),
        GameStatus::Final => ("FIN", Style::default().fg(Color::Gray)),
        GameStatus::Canceled => ("CANC", Style::default().fg(Color::DarkGray)),
        GameStatus::Suspended => ("SUSP", Style::default().fg(Color::Magenta)),
    }
}

/// 본문 블록 타이틀: 이 목록이 "어느 날짜의 경기"인지 밝힌다(Tab UX fix).
fn block_title(app: &App) -> String {
    if app.date.is_empty() {
        " Games ".into()
    } else {
        format!(" Games {} ", app.date)
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    // 첫 Games 업데이트가 아직 안 왔으면(프리페치 순간) "loading"을, 왔는데
    // 배열이 비어 있으면(휴식일/전체 우천취소) "no games"를 보여준다 — live.rs가
    // Option<LiveState>로 이미 구분하는 것과 동일한 원칙. 구분 없이 빈 테이블만
    // 그리면 두 상태가 헤더 행만 있는 동일한 화면으로 보인다.
    if !app.games_loaded {
        f.render_widget(
            Paragraph::new("loading...").block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }
    if app.games.is_empty() {
        f.render_widget(
            Paragraph::new("No games scheduled").block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }

    let header = Row::new(["", "Away", "Score", "Home", "Status"]);

    let rows: Vec<Row> = app
        .games
        .iter()
        .map(|g| {
            let (tag, tag_style) = status_tag(g.status);
            let score = match (g.away_score, g.home_score) {
                (Some(a), Some(h)) => format!("{a} : {h}"),
                _ => "— : —".to_string(),
            };
            Row::new(vec![
                Cell::from(Span::styled(tag, tag_style)),
                Cell::from(Span::styled(
                    g.away.name.as_str(),
                    team_badge_style(&g.away.code),
                )),
                Cell::from(score),
                Cell::from(Span::styled(
                    g.home.name.as_str(),
                    team_badge_style(&g.home.code),
                )),
                Cell::from(g.status_label.as_str()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(9),
        Constraint::Min(10),
        Constraint::Length(14),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(block_title(app)),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = TableState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(table, area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poller::Update;
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
    fn shows_loading_before_first_games_update_arrives() {
        let app = App::new(Default::default());
        assert!(!app.games_loaded);
        let text = render_to_string(&app);
        assert!(text.contains("loading"));
        assert!(!text.contains("No games scheduled"));
    }

    #[test]
    fn shows_no_games_message_when_loaded_and_confirmed_empty() {
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![]));
        assert!(app.games_loaded);
        let text = render_to_string(&app);
        assert!(text.contains("No games scheduled"));
        assert!(!text.contains("loading"));
    }

    /// GAMES 탭이 "무엇의 목록인지"(조회 날짜의 경기)를 타이틀이 말해줘야 한다.
    #[test]
    fn block_title_carries_query_date() {
        let mut app = App::new(Default::default());
        app.date = "2026-05-29".into();
        app.apply(Update::Games(vec![]));
        let text = render_to_string(&app);
        assert!(text.contains("Games 2026-05-29"));
    }

    #[test]
    fn team_name_uses_team_color_background_badge() {
        use crate::model::{Game, GameStatus, Team};
        let mut app = App::new(Default::default());
        // away = 두산(OB, 어두운 남색) — 배지 배경으로 렌더되어야 한다
        app.apply(Update::Games(vec![Game {
            id: "g".into(),
            start: "".into(),
            status: GameStatus::Final,
            status_label: "".into(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "OB".into(),
                name: "두산".into(),
            },
            home_score: Some(3),
            away_score: Some(10),
        }]));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer();
        let has_badge = buf
            .content()
            .iter()
            .any(|c| c.bg == super::super::theme::team_color("OB"));
        assert!(
            has_badge,
            "away team name should render on OB team-color background"
        );
    }

    /// away만 검증하던 기존 테스트의 사각지대 — home 팀명도 배지를 받는다(리뷰 Minor).
    #[test]
    fn home_team_name_also_gets_team_color_badge() {
        use crate::model::{Game, GameStatus, Team};
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![Game {
            id: "g".into(),
            start: "2026-07-19T18:00:00".into(),
            status: GameStatus::Live,
            status_label: "1회초".into(),
            home: Team {
                code: "OB".into(),
                name: "두산".into(),
            },
            away: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            home_score: Some(0),
            away_score: Some(0),
        }]));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();
        let has_home_badge = buf
            .content()
            .iter()
            .any(|c| c.bg == super::super::theme::team_color("OB"));
        assert!(
            has_home_badge,
            "home team OB must render on its color background"
        );
    }
}
