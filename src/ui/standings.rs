use super::theme::{self, contrast_fg, team_badge_style};
use crate::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

/// 최근 5경기 칼럼을 켜는 최소 내부 폭. 고정 칼럼(3+4+4+4+4+6+5=30)에 칸 간격
/// 7개, 팀명 최소 10칸, 그리고 L5 칼럼(5+간격 1)을 더한 값이다.
const WIDTH_FOR_LAST_FIVE: u16 = 53;
/// 연속 칼럼까지 켜는 최소 내부 폭(위 값 + 6 + 간격 1).
const WIDTH_FOR_STREAK: u16 = 60;

/// 순위는 --date와 무관한 시즌 "현재" 스냅샷이다(source.standings(year)) —
/// 과거 날짜를 조회 중이어도 순위만은 오늘 기준임을 타이틀로 밝힌다.
fn block_title(app: &App) -> String {
    let l = app.labels();
    match app.date.get(0..4) {
        Some(y) => format!(" {} {y} {} ", l.title_standings, l.standings_current),
        None => format!(" {} {} ", l.title_standings, l.standings_current),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let l = app.labels();
    // games.rs와 동일한 원칙: 첫 Standings 업데이트가 아직 안 왔으면(앱 기동
    // 직후 Standings 탭으로 전환한 경우) "loading"을, 왔는데 배열이 비어
    // 있으면 "no standings"를 보여준다. 구분 없이 빈 테이블만 그리면 두 상태가
    // 헤더 행만 있는 동일한 화면으로 보인다.
    if !app.standings_loaded {
        f.render_widget(
            Paragraph::new(l.loading).block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }
    if app.standings.is_empty() {
        f.render_widget(
            Paragraph::new(l.no_standings).block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }

    // 폭 예산(v0.23): 좁아지면 뒤쪽 칼럼부터 뗀다 — 연속(STRK) → 최근5(L5) 순.
    // 순위·팀·승패는 이 화면의 본체라 어떤 폭에서도 남는다. 임계값은 고정 칼럼
    // 합계(테두리 2 + 칸 간격 포함)에 팀명 최소폭을 더해 잡았다.
    let inner = area.width.saturating_sub(2);
    let show_last5 = inner >= WIDTH_FOR_LAST_FIVE;
    let show_streak = inner >= WIDTH_FOR_STREAK;

    let mut header_cells = vec!["#", l.col_team, "G", "W", "L", "D", "PCT", "GB"];
    if show_last5 {
        header_cells.push(l.col_last_five);
    }
    if show_streak {
        header_cells.push(l.col_streak);
    }
    let header = Row::new(header_cells);

    let rows: Vec<Row> = app
        .standings
        .iter()
        .map(|s| {
            let mut cells = vec![
                Cell::from(s.rank.to_string()),
                Cell::from(Span::styled(
                    s.team.name.as_str(),
                    team_badge_style(&s.team.code),
                )),
                Cell::from(s.games.to_string()),
                Cell::from(s.wins.to_string()),
                Cell::from(s.losses.to_string()),
                Cell::from(s.draws.to_string()),
                Cell::from(format!("{:.3}", s.win_rate)),
                Cell::from(format!("{:.1}", s.game_behind)),
            ];
            if show_last5 {
                cells.push(Cell::from(s.last_five.as_str()));
            }
            if show_streak {
                cells.push(Cell::from(s.streak.as_str()));
            }
            Row::new(cells)
        })
        .collect();

    let mut widths = vec![
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(6),
        Constraint::Length(5),
    ];
    if show_last5 {
        widths.push(Constraint::Length(5));
    }
    if show_streak {
        // 연속 기록은 "10연승"까지 나올 수 있고 한글 2칸이라 6칸을 잡는다.
        widths.push(Constraint::Length(6));
    }

    let highlight = match theme::accent_for(
        &app.theme_preset,
        &app.theme_accent,
        app.fav_code.as_deref(),
    ) {
        Some(c) => Style::default().bg(c).fg(contrast_fg(c)),
        None => Style::default().add_modifier(Modifier::REVERSED),
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(block_title(app)),
        )
        .row_highlight_style(highlight)
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

    fn standing_of(code: &str, name: &str) -> crate::model::Standing {
        crate::model::Standing {
            rank: 1,
            team: crate::model::Team {
                code: code.into(),
                name: name.into(),
            },
            games: 10,
            wins: 5,
            losses: 5,
            draws: 0,
            win_rate: 0.5,
            game_behind: 0.0,
            last_five: String::new(),
            streak: String::new(),
            stats: Default::default(),
        }
    }

    /// 순위표 팀명은 배지(팀컬러 bg + 대비 글자색)로 렌더 — 배경 무관 가독.
    #[test]
    fn standings_team_names_render_as_badges() {
        let mut app = App::new(Default::default());
        app.tab = crate::app::Tab::Standings;
        app.apply(Update::Standings(vec![
            standing_of("OB", "두산"),
            standing_of("HH", "한화"),
        ]));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();
        for code in ["OB", "HH"] {
            assert!(
                buf.content()
                    .iter()
                    .any(|c| c.bg == super::theme::team_color(code)),
                "{code} 팀명이 팀컬러 배경 배지로 렌더돼야 한다"
            );
        }
    }

    #[test]
    fn korean_title_renders_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.date = "2026-05-29".into();
        app.apply(Update::Standings(vec![]));
        let text = render_to_string(&app);
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남긴다
        // (games.rs의 renders_full_width_korean_team_names_without_panic과 동일 사유).
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains("순위2026(현재)"), "unexpected: {text}");
    }
    /// v0.23: 최근 5경기·연속 칼럼이 뜬다. 헤더와 값 **둘 다** 본다 — 헤더만 보면
    /// 값이 안 채워져도 통과한다.
    #[test]
    fn wide_terminal_shows_last_five_and_streak() {
        let mut app = crate::app::App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::En;
        app.apply(crate::poller::Update::Standings(vec![
            standing_with("LG", 1, "WWLLD", "2패"),
            standing_with("KT", 2, "WWWWD", "4승"),
        ]));
        let text = render_at(&app, 100, 8);
        for needle in ["L5", "STRK", "WWLLD", "WWWWD"] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
    }

    /// 좁아지면 뒤쪽 칼럼부터 뗀다 — 연속이 먼저, 그다음 최근5. 순위·팀·승패는
    /// 어떤 폭에서도 남는다(이 화면의 본체).
    #[test]
    fn narrow_terminals_drop_the_trailing_columns_first() {
        let mut app = crate::app::App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::En;
        app.apply(crate::poller::Update::Standings(vec![standing_with(
            "LG", 1, "WWLLD", "2패",
        )]));

        // 중간 폭: 최근5는 남고 연속은 빠진다.
        let mid = render_at(&app, 57, 6);
        assert!(mid.contains("WWLLD"), "L5가 너무 일찍 빠졌다:\n{mid}");
        assert!(!mid.contains("STRK"), "좁은데 연속 칼럼이 남았다:\n{mid}");

        // 좁은 폭: 둘 다 빠지고 본체만 남는다.
        let narrow = render_at(&app, 45, 6);
        assert!(!narrow.contains("WWLLD"), "좁은데 L5가 남았다:\n{narrow}");
        assert!(!narrow.contains("STRK"));
        assert!(
            narrow.contains("LG"),
            "팀명은 어떤 폭에서도 남아야 한다:\n{narrow}"
        );
    }

    /// 값이 비어 있어도(시즌 첫 경기 전) 칼럼만 빈 채로 뜨고 화면이 깨지지 않는다.
    #[test]
    fn empty_streak_values_render_as_blanks() {
        let mut app = crate::app::App::new(Default::default());
        app.apply(crate::poller::Update::Standings(vec![standing_with(
            "LG", 1, "", "",
        )]));
        let text = render_at(&app, 100, 6);
        assert!(text.contains("LG"));
    }

    fn standing_with(
        code: &str,
        rank: u16,
        last_five: &str,
        streak: &str,
    ) -> crate::model::Standing {
        crate::model::Standing {
            rank,
            team: crate::model::Team {
                code: code.into(),
                name: code.into(),
            },
            games: 90,
            wins: 50,
            losses: 38,
            draws: 2,
            win_rate: 0.568,
            game_behind: 0.0,
            last_five: last_five.into(),
            streak: streak.into(),
            stats: Default::default(),
        }
    }

    /// 렌더 후 공백 제거 — ratatui가 전각 문자 뒤에 placeholder 셀을 넣어
    /// 한글 부분 문자열 검사가 그냥은 실패한다(games.rs와 같은 관례).
    fn render_at(app: &crate::app::App, w: u16, h: u16) -> String {
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        let buf = term.backend().buffer();
        let raw: String = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        raw.chars().filter(|c| !c.is_whitespace()).collect()
    }
}
