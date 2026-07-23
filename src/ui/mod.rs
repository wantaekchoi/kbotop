pub mod footer;
pub mod games;
pub mod header;
pub mod help;
pub mod live;
pub mod options;
pub mod sideview;
pub mod standings;
pub mod strikezone;
pub mod teamlinks;
pub mod text;
pub mod theme;
pub mod tips;

// options::chooser가 help.rs의 중앙정렬 계산을 재사용한다.
pub(crate) use help::help_rect;

use crate::app::{App, Screen, Tab};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// htop 계승: header(요약) + 본문(Min) + footer(기능키 바) 3단 레이아웃.
/// 높이가 충분하면 본문과 footer 사이에 초보용 팁 한 줄이 끼어드는 4단이 된다
/// (아래 show_tip 분기).
pub fn draw(f: &mut Frame, app: &App) {
    // 높이 20 이상이면 본문-푸터 사이에 초보용 팁 한 줄을 끼운다(부족하면 본문 우선).
    let show_tip = f.area().height >= 20;
    let constraints: Vec<Constraint> = if show_tip {
        vec![
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    header::render(f, chunks[0], app);

    match &app.screen {
        Screen::Live { .. } => live::render(f, chunks[1], app),
        Screen::List => match app.tab {
            Tab::Games => games::render(f, chunks[1], app),
            Tab::Standings => standings::render(f, chunks[1], app),
        },
    }

    if show_tip {
        let minute = app.now_secs / 60;
        // 뉴스 제목은 동적이라 얼마든지 길 수 있다 — 정직한 말줄임(§15).
        // 팁은 소스에서 폭을 강제하지만 같은 벨트를 채워 둔다.
        let width = chunks[2].width as usize;
        let line = if !app.news.is_empty() && minute % 2 == 0 {
            let n = &app.news[current_news_index(app.now_secs, app.news.len())];
            let full = if n.source.is_empty() {
                n.title.clone()
            } else {
                format!("{} — {}", n.title, n.source)
            };
            Line::from(vec![
                Span::styled("News: ", Style::default().add_modifier(Modifier::DIM)),
                Span::styled(
                    text::ellipsize(&full, width.saturating_sub(6)),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("Tip: ", Style::default().add_modifier(Modifier::DIM)),
                Span::styled(
                    text::ellipsize(
                        tips::pick(&app.tips_override, app.now_secs),
                        width.saturating_sub(5),
                    ),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ])
        };
        f.render_widget(Paragraph::new(line), chunks[2]);
        footer::render(f, chunks[3], app);
    } else {
        footer::render(f, chunks[2], app);
    }

    if app.options.is_some() {
        options::render(f, f.area(), app);
    }

    if let Some(picker) = &app.link_picker {
        let items: Vec<ratatui::text::Line> = picker
            .items
            .iter()
            .map(|(l, _)| ratatui::text::Line::from(l.as_str()))
            .collect();
        options::chooser(f, f.area(), "Open in browser", &items, picker.cursor);
    }

    if app.show_help {
        help::render(f, f.area());
    }
}

/// 티커·n 키가 공유하는 현재 뉴스 회전 인덱스 — 계산 드리프트 방지.
pub fn current_news_index(now_secs: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((now_secs / 60 / 2) as usize) % len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Game, GameStatus, Team};
    use crate::poller::Update;
    use ratatui::{backend::TestBackend, Terminal};

    /// 긴 뉴스 제목은 티커에서 정직하게 말줄임된다(§15 오버플로 정책).
    #[test]
    fn long_news_title_is_ellipsized_in_the_ticker() {
        let mut app = App::new(Default::default());
        app.now_secs = 0; // 짝수 분 → News
        app.apply(Update::News(vec![crate::model::NewsItem {
            title: "아주 ".repeat(60),
            source: "테스트일보".into(),
            url: String::new(),
        }]));
        let text = render_to_string(&app);
        assert!(text.contains("News:"));
        assert!(
            text.contains('…'),
            "expected honest ellipsis in ticker:\n{text}"
        );
    }

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

    #[test]
    fn games_and_standings_bodies_render_distinct_identifying_titles() {
        let mut app = App::new(Default::default());
        app.date = "2026-05-29".into();
        app.apply(Update::Games(vec![game("g1", GameStatus::Final, "종료")]));
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
            win_rate: 0.7,
            game_behind: 0.0,
        }]));
        let games_text = render_to_string(&app);
        app.tab = Tab::Standings;
        let standings_text = render_to_string(&app);
        assert!(games_text.contains("Games 2026-05-29"));
        assert!(!games_text.contains("(current)"));
        assert!(standings_text.contains("Standings 2026 (current)"));
        assert!(!standings_text.contains("Games 2026-05-29"));
    }

    /// 짝수 분에는 News(출처 포함), 홀수 분에는 Tip이 하단 줄에 뜬다.
    /// 뉴스가 없으면 항상 Tip(우아한 저하).
    #[test]
    fn bottom_ticker_alternates_news_and_tip_by_minute() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![crate::model::NewsItem {
            title: "타이틀A".into(),
            source: "테스트일보".into(),
            url: String::new(),
        }]));
        let render = |app: &App| {
            let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
            term.draw(|f| draw(f, app)).unwrap();
            term.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        app.now_secs = 0; // 분 0 = 짝수 → News
        let even = render(&app);
        let even_c: String = even.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            even_c.contains("News:"),
            "even minute must show news:\n{even}"
        );
        assert!(
            even_c.contains("타이틀A") && even_c.contains("테스트일보"),
            "news must carry title+source"
        );
        app.now_secs = 60; // 분 1 = 홀수 → Tip
        let odd = render(&app);
        assert!(odd.contains("Tip:"), "odd minute must show tip:\n{odd}");
        // 뉴스 없으면 짝수 분에도 Tip
        app.news.clear();
        app.now_secs = 0;
        let fallback = render(&app);
        assert!(
            fallback.contains("Tip:"),
            "no news → tip fallback:\n{fallback}"
        );
    }

    /// 높이가 충분하면 본문과 푸터 사이에 Tip 줄이 렌더된다(초보 도움).
    #[test]
    fn tip_line_renders_on_tall_terminal_and_hides_on_short() {
        let mut app = App::new(Default::default());
        app.now_secs = 0;
        let tall = {
            let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
            term.draw(|f| draw(f, &app)).unwrap();
            term.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        assert!(tall.contains("Tip:"), "tip line missing on 24-row terminal");
        let short = {
            let mut term = Terminal::new(TestBackend::new(80, 16)).unwrap();
            term.draw(|f| draw(f, &app)).unwrap();
            term.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        assert!(
            !short.contains("Tip:"),
            "tip must yield body space on short terminal"
        );
    }

    #[test]
    fn current_news_index_matches_ticker_rotation() {
        assert_eq!(current_news_index(0, 3), 0); // minute 0 → (0/2)%3
        assert_eq!(current_news_index(120, 3), 1); // minute 2 → 1
        assert_eq!(current_news_index(600, 3), 2); // minute 10 → 5%3
    }
}
