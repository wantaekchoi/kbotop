use super::theme::team_color;
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

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    // 첫 Games 업데이트가 아직 안 왔으면(프리페치 순간) "loading"을, 왔는데
    // 배열이 비어 있으면(휴식일/전체 우천취소) "no games"를 보여준다 — live.rs가
    // Option<LiveState>로 이미 구분하는 것과 동일한 원칙. 구분 없이 빈 테이블만
    // 그리면 두 상태가 헤더 행만 있는 동일한 화면으로 보인다.
    if !app.games_loaded {
        f.render_widget(
            Paragraph::new("loading...").block(Block::bordered().title(" Today ")),
            area,
        );
        return;
    }
    if app.games.is_empty() {
        f.render_widget(
            Paragraph::new("No games scheduled today").block(Block::bordered().title(" Today ")),
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
                    Style::default().fg(team_color(&g.away.code)),
                )),
                Cell::from(score),
                Cell::from(Span::styled(
                    g.home.name.as_str(),
                    Style::default().fg(team_color(&g.home.code)),
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
        .block(Block::default().borders(Borders::ALL).title(" Today "))
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
}
