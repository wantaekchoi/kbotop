use super::theme::{self, contrast_fg, team_badge_style};
use crate::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

/// 순위(#) 칸 + 간격 + 팀명 최소폭. 오른쪽 칼럼이 하나도 없을 때의 필요 내부 폭이다.
const BASE_WIDTH: u16 = 3 + 1 + 10;
/// 팀 오른쪽 칼럼들의 폭 — 왼쪽이 더 중요하다. 필요 내부 폭은
/// `BASE_WIDTH + Σ(1 + 칸폭)`이고, 이 식이 주는 L5=53·STRK=60은 v0.23이 손으로
/// 잡아 둔 상수와 정확히 일치한다(실측 확인).
const COL_WIDTHS: [u16; 8] = [4, 4, 4, 4, 6, 5, 5, 6];

/// 내부 폭 `inner`에 온전히 들어가는 오른쪽 칼럼 개수. **줄여서 넣지 않는다** —
/// ratatui는 `Length` 칸이 모자라면 가운데서 잘라 버리는데, 숫자 칸이 잘리면
/// 값이 사라지는 게 아니라 **다른 값으로 읽힌다**(실측: 폭 39에서 100승이
/// `10`으로, 12.5경기차가 `12`로 찍혔다). 들어갈 만큼만 왼쪽부터 붙인다.
fn visible_cols(inner: u16) -> usize {
    // 딱 맞게 채우지 않고 한 칸을 남긴다 — ratatui의 배치는 `Min(10)` 팀 칸과
    // 경쟁하는 제약 풀이라 계산상 정확히 들어맞는 폭에서도 마지막 칸이 1 줄어든다
    // (실측: 내부 폭 41에서 승률이 `0.70`으로 잘렸다).
    let budget = inner.saturating_sub(1);
    let mut need = BASE_WIDTH;
    let mut n = 0;
    for w in COL_WIDTHS {
        let next = need + 1 + w;
        if next > budget {
            break;
        }
        need = next;
        n += 1;
    }
    n
}

/// 연속 기록을 화면 언어로 옮긴다. 서버는 `continuousGameResult`를 "3승"·"1패"·
/// "1무"처럼 **한국어로** 주기 때문에, 영어·일본어 화면에서 STRK 칸만 한글로
/// 남아 있었다(v0.30, 데모 GIF에서 눈에 띄었다). 숫자 + 알려진 접미일 때만
/// 갈아 끼우고, 형식이 조금이라도 다르면 원문을 그대로 둔다 — 서버 표기가
/// 바뀌어도 조용히 틀린 값을 만들지 않는 쪽이 낫다(관용 파싱 원칙).
fn streak_label(l: &super::i18n::Labels, raw: &str) -> String {
    let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return raw.to_string();
    }
    let suffix = match &raw[digits.len()..] {
        "승" => l.streak_win,
        "패" => l.streak_loss,
        "무" => l.streak_draw,
        _ => return raw.to_string(),
    };
    format!("{digits}{suffix}")
}

/// 순위는 --date와 무관한 시즌 "현재" 스냅샷이다(source.standings(year)) —
/// 과거 날짜를 조회 중이어도 순위만은 오늘 기준임을 타이틀로 밝힌다.
fn block_title(app: &App) -> String {
    let l = app.labels();
    match app.date.get(0..4) {
        Some(y) => format!(" {} {y} {} ", l.title_standings, l.standings_current),
        None => format!(" {} {} ", l.title_standings, l.standings_current),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App, hits: &mut super::hit::HitMap) {
    let l = app.labels();
    // games.rs와 동일한 원칙: 첫 Standings 업데이트가 아직 안 왔으면(앱 기동
    // 직후 Standings 탭으로 전환한 경우) "loading"을, 왔는데 배열이 비어
    // 있으면 "no standings"를 보여준다. 구분 없이 빈 테이블만 그리면 두 상태가
    // 헤더 행만 있는 동일한 화면으로 보인다.
    if !app.standings_loaded {
        f.render_widget(
            Paragraph::new(super::pending_body_text(app))
                // 에러 원문은 URL까지 길다 — 한 줄에서 조용히 잘리지 않게 접는다.
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title(block_title(app))),
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

    // 폭 예산(v0.23): 좁아지면 뒤쪽 칼럼부터 뗀다 — 연속(STRK) → 최근5(L5) →
    // 경기차(GB) → … 순. 순위·팀은 이 화면의 본체라 어떤 폭에서도 남는다.
    let inner = area.width.saturating_sub(2);
    let shown = visible_cols(inner);

    let mut header_cells = vec!["#", l.col_team];
    // COL_WIDTHS와 같은 순서 — 하나를 고치면 다른 하나도 고쳐야 한다.
    header_cells.extend(
        [
            "G",
            "W",
            "L",
            "D",
            "PCT",
            "GB",
            l.col_last_five,
            l.col_streak,
        ][..shown]
            .iter(),
    );
    let header = Row::new(header_cells);

    let rows: Vec<Row> = app
        .standings
        .iter()
        .map(|s| {
            let mut cells = vec![
                Cell::from(s.rank.to_string()),
                Cell::from(Span::styled(
                    s.team.name.as_str(),
                    team_badge_style(&app.theme_preset, &s.team.code),
                )),
            ];
            let values = [
                s.games.to_string(),
                s.wins.to_string(),
                s.losses.to_string(),
                s.draws.to_string(),
                format!("{:.3}", s.win_rate),
                format!("{:.1}", s.game_behind),
                s.last_five.clone(),
                streak_label(l, &s.streak),
            ];
            cells.extend(values.into_iter().take(shown).map(Cell::from));
            Row::new(cells)
        })
        .collect();

    let mut widths = vec![Constraint::Length(3), Constraint::Min(10)];
    widths.extend(COL_WIDTHS[..shown].iter().map(|w| Constraint::Length(*w)));

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
    // 그린 뒤에 등록한다 — 스크롤 오프셋은 ratatui가 정하므로, 우리가 다시
    // 계산하는 대신 렌더가 끝난 state에서 읽는다(hit.rs 모듈 주석의 이유).
    hits.push_table_rows(
        area,
        state.offset(),
        app.standings.len(),
        super::hit::Zone::StandingRow,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poller::Update;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_string(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
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

    /// games.rs와 같은 이유로 순위 패널도 실패를 본문에 말한다 — 두 패널이
    /// `ui::pending_body_text` 하나를 공유하므로 한쪽만 고칠 수 없다.
    #[test]
    fn the_body_says_the_error_when_the_first_load_keeps_failing() {
        let mut app = App::new(Default::default());
        app.apply(Update::Error("Dns Failed".into()));
        assert!(!app.standings_loaded);
        let text = render_to_string(&app);
        assert!(
            text.contains("Dns Failed"),
            "본문이 원인을 안 말한다:\n{text}"
        );
        assert!(
            !text.contains("loading"),
            "실패를 아는데 아직 loading이다:\n{text}"
        );
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
        term.draw(|f| render(f, f.area(), &app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
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

    /// **좁아져도 숫자를 반쯤 잘라 보여주지 않는다.** ratatui는 `Length` 칸이
    /// 모자라면 가운데서 자르는데, 그러면 100승이 `10`으로, 12.5경기차가 `12`로
    /// 찍힌다 — 값이 사라지는 게 아니라 **다른 값으로 읽힌다**(v0.30 실측).
    /// 헤더에 뜬 칸은 어떤 폭에서도 값이 온전해야 한다.
    #[test]
    fn a_narrow_terminal_drops_columns_instead_of_halving_the_numbers() {
        let mut app = crate::app::App::new(Default::default());
        let mut st = standing_with("KIA", 1, "WWLLD", "3W");
        st.games = 144;
        st.wins = 100;
        st.losses = 42;
        st.draws = 2;
        st.win_rate = 0.704;
        st.game_behind = 12.5;
        app.apply(Update::Standings(vec![st]));

        let cols = [
            ("G", "144"),
            ("W", "100"),
            ("L", "42"),
            ("D", "2"),
            ("PCT", "0.704"),
            ("GB", "12.5"),
            ("L5", "WWLLD"),
            ("STRK", "3W"),
        ];
        for w in 20..=140u16 {
            let text = render_at(&app, w, 6);
            for (i, (head, value)) in cols.iter().enumerate() {
                if super::visible_cols(w.saturating_sub(2)) > i {
                    assert!(
                        text.contains(value),
                        "폭 {w}: {head} 칸이 떠 있는데 값 {value}가 온전하지 않다:\n{text}"
                    );
                }
            }
        }
    }

    /// **연속 기록도 화면 언어를 따른다.** 서버가 "3승"으로 주는 값이라
    /// 영어·일본어 화면에서 이 칸만 한글로 남아 있었다(v0.30 데모 GIF에서 발견).
    #[test]
    fn the_streak_column_speaks_the_screen_language() {
        for (lang, expected) in [
            (crate::ui::i18n::Lang::Ko, "3승"),
            (crate::ui::i18n::Lang::En, "3W"),
            (crate::ui::i18n::Lang::Ja, "3勝"),
        ] {
            let mut app = crate::app::App::new(Default::default());
            app.lang = lang;
            app.apply(Update::Standings(vec![standing_with(
                "LG", 1, "WWLLD", "3승",
            )]));
            let text = render_at(&app, 100, 6);
            assert!(text.contains(expected), "{lang:?}: {text}");
        }
    }

    /// 서버 표기가 우리가 아는 형식이 아니면 **원문을 그대로 둔다** — 모르는
    /// 것을 아는 척 옮기지 않는다.
    #[test]
    fn an_unknown_streak_format_is_left_alone() {
        let l = &crate::ui::i18n::EN;
        assert_eq!(super::streak_label(l, "3승"), "3W");
        assert_eq!(super::streak_label(l, "1패"), "1L");
        assert_eq!(super::streak_label(l, "2무"), "2D");
        assert_eq!(super::streak_label(l, "10연승"), "10연승");
        assert_eq!(super::streak_label(l, "streak"), "streak");
        assert_eq!(super::streak_label(l, ""), "");
    }

    /// 임계값을 깎으면 이 테스트가 잡는다 — 폭이 넉넉하면 여덟 칸 다, 좁으면
    /// 오른쪽부터 사라지고, 어느 폭에서도 늘어나기만 하지 줄지 않는다.
    #[test]
    fn columns_appear_left_to_right_as_the_terminal_widens() {
        let mut prev = 0;
        for inner in 0..160u16 {
            let n = super::visible_cols(inner);
            assert!(n >= prev, "폭 {inner}에서 칸 수가 줄었다: {prev} → {n}");
            prev = n;
        }
        assert_eq!(super::visible_cols(160), 8);
        assert_eq!(super::visible_cols(14), 0);
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
        term.draw(|f| render(f, f.area(), app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let raw: String = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        raw.chars().filter(|c| !c.is_whitespace()).collect()
    }
}
