use crate::app::{App, Tab};
use crate::model::GameStatus;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// htop의 CPU/Mem 게이지 자리에 해당하는 2줄 요약 헤더.
/// 1행: 상태별 경기 수. 2행: 탭 표시(+ stale 마커).
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let mut live = 0u16;
    let mut sched = 0u16;
    let mut fin = 0u16;
    let mut other = 0u16; // Canceled/Suspended — 정상 종료(FINAL)와 구분해야 한다
    for g in &app.games {
        match g.status {
            GameStatus::Live => live += 1,
            GameStatus::Scheduled => sched += 1,
            GameStatus::Final => fin += 1,
            GameStatus::Canceled | GameStatus::Suspended => other += 1,
        }
    }

    let counts = Line::from(vec![
        Span::styled(
            format!("LIVE {live}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("SCHED {sched}"), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(format!("FINAL {fin}"), Style::default().fg(Color::Green)),
        Span::raw("  "),
        Span::styled(
            format!("OTHER {other}"),
            Style::default().fg(Color::Magenta),
        ),
    ]);

    let games_style = if app.tab == Tab::Games {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default()
    };
    let standings_style = if app.tab == Tab::Standings {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default()
    };

    let mut tab_spans = vec![
        Span::styled(" GAMES ", games_style),
        Span::raw(" | "),
        Span::styled(" STANDINGS ", standings_style),
    ];
    if app.stale {
        tab_spans.push(Span::raw("   "));
        tab_spans.push(Span::styled(
            "stale",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }
    let tabs = Line::from(tab_spans);

    let paragraph = Paragraph::new(vec![counts, tabs])
        .block(Block::default().borders(Borders::ALL).title(" kbotop "));
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::model::{Game, Team};
    use crate::poller::Update;
    use ratatui::{backend::TestBackend, Terminal};

    fn game(id: &str, status: GameStatus) -> Game {
        Game {
            id: id.into(),
            start: "".into(),
            status,
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
        }
    }

    fn render_to_string(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(80, 4)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// docs/CURRENT_STATE.md에 기록된 round-1 버그(Canceled/Suspended가
    /// FINAL로 합산되던 것) 회귀 방지 — 두 상태 모두 OTHER로 집계돼야 한다.
    #[test]
    fn per_status_tally_counts_canceled_and_suspended_as_other_not_final() {
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![
            game("a", GameStatus::Live),
            game("b", GameStatus::Scheduled),
            game("c", GameStatus::Final),
            game("d", GameStatus::Canceled),
            game("e", GameStatus::Suspended),
        ]));
        let text = render_to_string(&app);
        assert!(text.contains("LIVE 1"));
        assert!(text.contains("SCHED 1"));
        assert!(text.contains("FINAL 1"));
        assert!(text.contains("OTHER 2"));
    }
}
