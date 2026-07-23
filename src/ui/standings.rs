use super::theme::team_color;
use crate::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

/// 순위는 --date와 무관한 시즌 "현재" 스냅샷이다(source.standings(year)) —
/// 과거 날짜를 조회 중이어도 순위만은 오늘 기준임을 타이틀로 밝힌다.
fn block_title(app: &App) -> String {
    match app.date.get(0..4) {
        Some(y) => format!(" Standings {y} (current) "),
        None => " Standings (current) ".into(),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    // games.rs와 동일한 원칙: 첫 Standings 업데이트가 아직 안 왔으면(앱 기동
    // 직후 Standings 탭으로 전환한 경우) "loading"을, 왔는데 배열이 비어
    // 있으면 "no standings"를 보여준다. 구분 없이 빈 테이블만 그리면 두 상태가
    // 헤더 행만 있는 동일한 화면으로 보인다.
    if !app.standings_loaded {
        f.render_widget(
            Paragraph::new("loading...").block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }
    if app.standings.is_empty() {
        f.render_widget(
            Paragraph::new("No standings available")
                .block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }

    let header = Row::new(["#", "Team", "G", "W", "L", "D", "PCT", "GB"]);

    let rows: Vec<Row> = app
        .standings
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(s.rank.to_string()),
                Cell::from(Span::styled(
                    s.team.name.as_str(),
                    Style::default().fg(team_color(&s.team.code)),
                )),
                Cell::from(s.games.to_string()),
                Cell::from(s.wins.to_string()),
                Cell::from(s.losses.to_string()),
                Cell::from(s.draws.to_string()),
                Cell::from(format!("{:.3}", s.win_rate)),
                Cell::from(format!("{:.1}", s.game_behind)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(6),
        Constraint::Length(5),
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
    fn shows_loading_before_first_standings_update_arrives() {
        let app = App::new(Default::default());
        assert!(!app.standings_loaded);
        let text = render_to_string(&app);
        assert!(text.contains("loading"));
        assert!(!text.contains("No standings available"));
    }

    #[test]
    fn shows_no_standings_message_when_loaded_and_confirmed_empty() {
        let mut app = App::new(Default::default());
        app.apply(Update::Standings(vec![]));
        assert!(app.standings_loaded);
        let text = render_to_string(&app);
        assert!(text.contains("No standings available"));
        assert!(!text.contains("loading"));
    }

    /// STANDINGS는 --date와 무관한 "시즌 현재" 순위임을 타이틀이 밝혀야 한다.
    #[test]
    fn block_title_says_season_current_not_the_query_date() {
        let mut app = App::new(Default::default());
        app.date = "2026-05-29".into();
        app.apply(Update::Standings(vec![]));
        let text = render_to_string(&app);
        assert!(text.contains("Standings 2026 (current)"));
        assert!(!text.contains("05-29"));
    }
}
