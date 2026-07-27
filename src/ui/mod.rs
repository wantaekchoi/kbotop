pub mod article;
pub mod footer;
pub mod games;
pub mod header;
pub mod help;
pub mod i18n;
pub mod live;
/// 라이브 화면의 표현 상태(ViewModel) — `live`는 이걸 받아 그리기만 한다.
pub(crate) mod live_vm;
pub mod newslist;
pub mod options;
pub mod settings;
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
    let l = app.labels();
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
        let line = if !app.news.is_empty() && minute.is_multiple_of(2) {
            let n = &app.news[current_news_index(app.now_secs, app.news.len())];
            let full = if n.source.is_empty() {
                n.title.clone()
            } else {
                format!("{} — {}", n.title, n.source)
            };
            Line::from(vec![
                Span::styled(l.news_label, Style::default().add_modifier(Modifier::DIM)),
                Span::styled(
                    text::ellipsize(
                        &full,
                        width.saturating_sub(text::display_width(l.news_label)),
                    ),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(l.tip_label, Style::default().add_modifier(Modifier::DIM)),
                Span::styled(
                    text::ellipsize(
                        tips::pick(&app.tips_override, app.lang, app.now_secs),
                        width.saturating_sub(text::display_width(l.tip_label)),
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
        options::chooser(f, f.area(), l.title_open, &items, picker.cursor);
    }

    if app.show_help {
        help::render(f, f.area(), app);
    }

    if app.settings.is_some() {
        settings::render(f, f.area(), app);
    }

    // 뉴스 목록은 기사보다 아래 층 — 기사가 목록 위에 겹쳐 열릴 수 있으므로
    // (on_key의 소비 순서와 대응) 목록을 먼저 그리고 기사를 그 위에 그린다.
    if app.news_list.is_some() {
        newslist::render(f, f.area(), app);
    }

    // 기사 오버레이는 최상위 — 열려 있으면 on_key가 다른 오버레이 오픈을 막으므로
    // 실제로 겹치는 일은 없지만, 그려질 땐 가장 위에 온다.
    if app.article_view.is_some() {
        article::render(f, f.area(), app);
    }
}

/// 티커·n 키가 공유하는 현재 뉴스 회전 인덱스 — 계산 드리프트 방지.
pub fn current_news_index(now_secs: u64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((now_secs / 60 / 2) as usize) % len
}

/// article/newslist/settings 세 오버레이 테스트가 공유하는 fix 2-4 가드
/// 헬퍼(v0.10 DRY 정리 — 복붙 4곳 통합). 프로덕션 코드 아님: 전부
/// `#[cfg(test)]`이고 `pub(crate)`라 각 ui 서브모듈의 `#[cfg(test)] mod tests`
/// 에서만 보인다.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::app::App;
    use ratatui::{backend::TestBackend, layout::Rect, Frame, Terminal};

    /// 하단 힌트(title_bottom)가 좁은 폭에서 박스 하단 좌/우 모서리(└/┘)를
    /// 덮지 않는지 검증한다 — 오버레이별 `setup`(App에 오버레이 상태를 채움)과
    /// 해당 모듈의 `render`를 넘겨받아, 각 `widths` 값마다 렌더 → `help_rect`로
    /// 박스 좌표를 재계산 → 모서리 심볼을 단언한다(기존 4곳의 로직 그대로).
    pub(crate) fn assert_bottom_hint_keeps_box_corners(
        widths: &[u16],
        setup: impl FnOnce(&mut App),
        render: fn(&mut Frame, Rect, &App),
    ) {
        let mut app = App::new(Default::default());
        setup(&mut app);

        for &width in widths {
            let area = Rect::new(0, 0, width, 24);
            let mut term = Terminal::new(TestBackend::new(width, 24)).unwrap();
            term.draw(|f| render(f, f.area(), &app)).unwrap();
            let buf = term.backend().buffer().clone();

            let w = area.width.saturating_sub(4).max(1);
            let h = area.height.saturating_sub(2).max(1);
            let rect = super::help_rect(w, h, area);
            let bottom_y = rect.y + rect.height - 1;
            let left_x = rect.x;
            let right_x = rect.x + rect.width - 1;

            assert_eq!(
                buf[(left_x, bottom_y)].symbol(),
                "└",
                "width {width}: bottom-left corner overwritten by hint"
            );
            assert_eq!(
                buf[(right_x, bottom_y)].symbol(),
                "┘",
                "width {width}: bottom-right corner overwritten by hint"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BaseState, Count, Game, GameStatus, LiveState, Team};
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
            summary: String::new(),
            published: String::new(),
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
            summary: String::new(),
            published: String::new(),
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

    /// draw() 최상위 층에서 기사 오버레이가 렌더된다(제목이 화면에 나타남).
    #[test]
    fn article_overlay_renders_through_draw() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.article_view = Some(crate::app::ArticleView {
            item: crate::model::NewsItem {
                title: "제목텍스트".into(),
                source: String::new(),
                url: String::new(),
                summary: "본문 내용".into(),
                published: String::new(),
            },
            scroll: 0,
        });
        let compact: String = render_to_string(&app)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            compact.contains("제목텍스트"),
            "article title must render via draw:\n{compact}"
        );
    }

    #[test]
    fn current_news_index_matches_ticker_rotation() {
        assert_eq!(current_news_index(0, 3), 0); // minute 0 → (0/2)%3
        assert_eq!(current_news_index(120, 3), 1); // minute 2 → 1
        assert_eq!(current_news_index(600, 3), 2); // minute 10 → 5%3
    }

    /// 한국어 lang일 때 하단 티커가 "팁: " 라벨을 쓴다(폭 예산도 라벨 폭에 맞춰 동적).
    #[test]
    fn korean_tip_label_renders_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.now_secs = 60; // 홀수 분 → Tip
        let text = render_to_string(&app);
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남긴다
        // (renders_full_width_korean_team_names_without_panic과 동일 사유).
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("팁:"),
            "expected Korean tip label:\n{text}"
        );
    }

    /// draw 최상위에서 목록이 렌더되고 제목이 보인다.
    #[test]
    fn news_list_renders_through_draw() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.apply(Update::News(vec![crate::model::NewsItem {
            title: "목록항목제목".into(),
            source: "출처".into(),
            url: String::new(),
            summary: String::new(),
            published: String::new(),
        }]));
        app.news_list = Some(crate::app::NewsListState { cursor: 0 });
        let compact: String = render_to_string(&app)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            compact.contains("목록항목제목"),
            "목록 제목이 보여야 한다:\n{compact}"
        );
    }

    /// mono 프리셋에서는 chrome이 색을 전혀 쓰지 않는다(팀 배지 데이터는 예외 —
    /// 배지가 없는 기본 목록 화면으로 검증). 렌더 버퍼의 모든 셀 fg가 무채색이다.
    #[test]
    fn mono_preset_renders_without_color() {
        use ratatui::style::Color;
        let mut app = App::new(Default::default());
        app.theme_preset = "mono".into();
        app.theme_accent = "cyan".into();
        // 팀 배지가 없는 상태(경기 없음, fav 미설정)에서 chrome만 렌더.
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        for cell in buf.content() {
            assert!(
                matches!(cell.fg, Color::Reset)
                    || matches!(cell.fg, Color::White | Color::Black | Color::Gray),
                "mono chrome used a color: {:?}",
                cell.fg
            );
        }
    }

    /// mono 봉인 보강(리뷰 Minor): base 목록 화면만 보던 기존 테스트는 live.rs의
    /// Suspended 배지(수정 1 이전엔 게이트 없이 Magenta 직접 사용)를 못 봤다.
    /// Suspended 라이브 화면을 렌더해 "SUSPENDED" 배지 셀의 fg가 무채색인지
    /// 직접 검사한다 — 팀 배지(LG/KT RGB)는 데이터 예외라 전체 버퍼가 아니라
    /// 배지 셀만 표적으로 검사한다.
    #[test]
    fn mono_preset_seals_suspended_badge_in_live_view() {
        use ratatui::style::Color;
        let mut app = App::new(Default::default());
        app.theme_preset = "mono".into();
        app.theme_accent = "cyan".into();

        let team = |code: &str, name: &str| Team {
            code: code.into(),
            name: name.into(),
        };
        let state = LiveState {
            inning_label: "9회말".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: 3,
            away_score: 2,
            count: Count {
                ball: 0,
                strike: 0,
                out: 0,
            },
            bases: BaseState {
                first: false,
                second: false,
                third: false,
            },
            pitcher_name: String::new(),
            batter_name: String::new(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![],
            next_batter_name: String::new(),
            at_bats: vec![],
        };
        let game = Game {
            id: "g".into(),
            start: "".into(),
            status: GameStatus::Suspended,
            status_label: "9회말".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(3),
            away_score: Some(2),
        };
        app.screen = Screen::Live {
            game,
            state: Some(state),
        };

        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        let cells: Vec<_> = buf.content().iter().collect();

        // "SUSPENDED"는 i18n 영문 라벨(badge_suspended)에서만 나오는 유일한 문자열이라
        // 버퍼에서 이 연속 셀 시퀀스를 찾아 그 fg만 표적 검사한다.
        let badge_chars: Vec<char> = "SUSPENDED".chars().collect();
        let mut found = false;
        for start in 0..cells.len().saturating_sub(badge_chars.len() - 1) {
            let is_match = badge_chars
                .iter()
                .enumerate()
                .all(|(i, ch)| cells[start + i].symbol() == ch.to_string());
            if !is_match {
                continue;
            }
            found = true;
            for cell in &cells[start..start + badge_chars.len()] {
                assert!(
                    matches!(cell.fg, Color::Reset)
                        || matches!(cell.fg, Color::White | Color::Black | Color::Gray),
                    "mono Suspended badge used a chromatic color: {:?}",
                    cell.fg
                );
            }
        }
        assert!(found, "SUSPENDED badge not found in rendered buffer");
    }

    /// 최종 리뷰 Important 수정 봉인: mono 프리셋에서도 스트라이크존 마커·구속
    /// 범례·측면뷰 궤적이 투구 결과색(Green/Red/Yellow/Cyan)으로 새던 문제
    /// (strikezone::render/sideview::render가 preset 게이트 없이 항상
    /// result_color를 fg로 씀). 투구 데이터가 있는 라이브 화면을 존이 실제로
    /// 그려지는 넓은 폭(live.rs의 wide 기준 70 이상)·측면뷰까지 나오는 높이로
    /// 렌더해 버퍼 전체에 유채색 fg가 없는지 검사한다. 수정 전(게이트 없음)이면
    /// 이 테스트는 실패한다.
    #[test]
    fn mono_preset_seals_strikezone_and_sideview_pitch_colors_in_live_view() {
        use crate::model::{Pitch, PitchResult};
        use ratatui::style::Color;

        let mut app = App::new(Default::default());
        app.theme_preset = "mono".into();
        app.theme_accent = "cyan".into();

        let team = |code: &str, name: &str| Team {
            code: code.into(),
            name: name.into(),
        };
        let mk_pitch = |order: u8, result: PitchResult| Pitch {
            order,
            plate_x: (order as f32 - 2.0) * 0.2,
            plate_y: 2.5,
            sz_top: 3.3,
            sz_bottom: 1.5,
            speed_kmh: Some(140 + order as u16),
            result,
            text: format!("{order}구"),
            // 측면뷰 궤적이 실제로 그려지도록 릴리스 파라미터를 채운다
            // (sideview.rs traj_pitch와 동일 값).
            plate_t: 0.39,
            y0: 50.0,
            vy0: -130.0,
            ay: 21.0,
            z0: 6.0 + (order as f32 - 1.0) * 0.5,
            vz0: -0.5,
            az: -21.0,
            ..Default::default()
        };
        let state = LiveState {
            inning_label: "5회초".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: 1,
            away_score: 2,
            count: Count {
                ball: 1,
                strike: 2,
                out: 1,
            },
            bases: BaseState {
                first: true,
                second: false,
                third: false,
            },
            pitcher_name: "투수".into(),
            batter_name: "타자".into(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![
                mk_pitch(1, PitchResult::Ball),
                mk_pitch(2, PitchResult::StrikeLooking),
                mk_pitch(3, PitchResult::Foul),
                mk_pitch(4, PitchResult::InPlay),
            ],
            next_batter_name: String::new(),
            at_bats: vec![],
        };
        let game = Game {
            id: "g".into(),
            start: "".into(),
            status: GameStatus::Live,
            status_label: "5회초".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(1),
            away_score: Some(2),
        };
        app.screen = Screen::Live {
            game,
            state: Some(state),
        };

        // 80x30: width>=70(live.rs wide 기준) + 측면뷰가 나오는 충분한 높이.
        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();

        for cell in buf.content() {
            assert!(
                !matches!(
                    cell.fg,
                    Color::Green | Color::Red | Color::Yellow | Color::Cyan
                ),
                "mono strikezone/sideview leaked a chromatic pitch color: {:?} at {:?}",
                cell.fg,
                cell.symbol()
            );
        }

        // 정보 손실 없음: 색이 빠져도 구속 범례의 결과 문자는 남아야 한다.
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for tag in ["1B", "2S", "3F", "4H"] {
            assert!(
                compact.contains(tag),
                "mono must keep the result letter {tag}:\n{text}"
            );
        }
    }

    /// 대조(무회귀): mono가 아닌 프리셋에서는 라이브 화면의 투구색이 기존처럼 남는다.
    #[test]
    fn default_preset_keeps_strikezone_pitch_colors_in_live_view() {
        use crate::model::{Pitch, PitchResult};
        use ratatui::style::Color;

        let mut app = App::new(Default::default());
        // theme_preset은 기본값("default")을 그대로 둔다.

        let team = |code: &str, name: &str| Team {
            code: code.into(),
            name: name.into(),
        };
        let pitch = Pitch {
            order: 1,
            plate_x: 0.0,
            plate_y: 2.5,
            sz_top: 3.3,
            sz_bottom: 1.5,
            speed_kmh: Some(145),
            result: PitchResult::Ball, // result_color(Ball) == Green
            text: "1구".into(),
            ..Default::default()
        };
        let state = LiveState {
            inning_label: "5회초".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: 1,
            away_score: 2,
            count: Count {
                ball: 1,
                strike: 0,
                out: 0,
            },
            bases: BaseState {
                first: false,
                second: false,
                third: false,
            },
            pitcher_name: String::new(),
            batter_name: String::new(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![pitch],
            next_batter_name: String::new(),
            at_bats: vec![],
        };
        let game = Game {
            id: "g".into(),
            start: "".into(),
            status: GameStatus::Live,
            status_label: "5회초".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(1),
            away_score: Some(2),
        };
        app.screen = Screen::Live {
            game,
            state: Some(state),
        };

        let mut term = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let buf = term.backend().buffer().clone();
        let has_green = buf.content().iter().any(|c| c.fg == Color::Green);
        assert!(
            has_green,
            "default preset must keep the Ball pitch color (Green) in the live strikezone"
        );
    }

    /// draw 최상위에서 설정 화면이 렌더되고 제목이 보인다.
    #[test]
    fn settings_renders_through_draw() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.settings = Some(crate::app::SettingsState {
            cursor: 0,
            save_failed: false,
        });
        let compact: String = render_to_string(&app)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            compact.contains("설정"),
            "settings title missing:\n{compact}"
        );
    }
}
