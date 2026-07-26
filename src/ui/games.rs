use super::i18n::Labels;
use super::theme::{self, contrast_fg, team_badge_style};
use crate::app::App;
use crate::model::GameStatus;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

/// KST(UTC+9) 고정 오프셋 — header.rs의 A-1 결정과 동일 근거·관례(새 크레이트
/// 금지 + dateutil::kst_days와의 일관성). Game.start("YYYY-MM-DDTHH:MM:SS")도
/// 이 프로젝트 관례상 KST 벽시계로 간주한다(main.rs/RSS 파서와 동일 가정).
const KST_OFFSET_SECS: i64 = 9 * 3600;

/// Scheduled 경기의 시작까지 남은 시간을 (시, 분)으로 계산한다(v0.15 A-3).
/// `Game.start`는 날짜까지 포함하므로(live.rs::elapsed_label이 다루는 투구
/// 시각과 달리) "시:분만 보고 자정이면 +24h"류 보정을 하지 않는다 — 그 보정은
/// 날짜 정보가 없을 때만 안전한 휴리스틱이고, 여기서는 절대 UTC epoch로
/// 정확히 변환해 직접 뺄셈하는 편이 더 정확하고 "이미 지난 시각은 표시하지
/// 않는다"(설계 §2 A-3, 억지 음수 표기 금지) 요구도 자연스럽게 만족한다.
/// 파싱 실패·과거 시각은 None(무패닉·표시 생략 — 기존 관용 원칙).
fn scheduled_eta_hm(now_secs: u64, start: &str) -> Option<(i64, i64)> {
    let (date, time) = start.split_once('T')?;
    let mut d = date.splitn(3, '-');
    let y: i64 = d.next()?.parse().ok()?;
    let m: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    // 연도까지 범위를 막는다 — 월·일만 검사하면 서버가 보낸 터무니없는 연도가
    // 아래 `days * 86400`에서 오버플로한다(debug 빌드는 패닉, release는 wrap해
    // 말도 안 되는 카운트다운). 렌더 경로엔 catch_unwind가 없으므로(app.rs 주석
    // 참고) 여기서 걸러 표시를 생략하는 게 맞다.
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let days = crate::dateutil::days_from_civil(y, m, day);
    // HH:MM:SS 파싱은 라이브 화면의 계산부와 공유(같은 원문 포맷의 시:분:초
    // 자릿수·범위 검증) — v19a 리뷰 M-1: 화면 표현과 무관한 범용 파서라
    // dateutil로 옮겼다(v0.19에선 live.rs → live_vm.rs로만 옮겨져 있었다).
    let time_of_day = crate::dateutil::parse_hms_secs(time)?;
    let target_utc = days * 86400 + time_of_day - KST_OFFSET_SECS;
    let remaining = target_utc - now_secs as i64;
    if remaining <= 0 {
        return None;
    }
    // 올림 — "0분 후"처럼 임박한 경기를 0으로 보여주는 대신 최소 1분으로 반올림.
    // i64::div_ceil은 아직 unstable(int_roundings)이라 직접 계산한다(remaining
    // > 0이 위에서 이미 보장돼 있어 이 식이 안전하다).
    let total_min = (remaining + 59) / 60;
    Some((total_min / 60, total_min % 60))
}

/// (시, 분) → "1시간 20분 후"류 완성형(i18n Labels 경유, poll_suffix와 동일한
/// "{n}{suffix}" 융합 조립 — 언어별 공백 유무는 suffix 문자열 자체가 결정).
fn scheduled_eta_label(l: &Labels, hours: i64, mins: i64) -> String {
    if hours > 0 {
        format!(
            "{hours}{}{mins}{}",
            l.remaining_hour_suffix, l.remaining_min_suffix
        )
    } else {
        format!("{mins}{}", l.remaining_min_suffix)
    }
}

/// Status 열의 폭(widths 배열과 A-3 ellipsize 예산이 공유하는 단일 진실).
const STATUS_COL_WIDTH: usize = 14;

fn status_tag(status: GameStatus, l: &Labels, preset: &str) -> (&'static str, Style) {
    match status {
        GameStatus::Live => (
            l.tag_live,
            theme::status_fg(preset, Color::Red).add_modifier(Modifier::BOLD),
        ),
        GameStatus::Scheduled => (l.tag_sched, theme::status_fg(preset, Color::Yellow)),
        GameStatus::Final => (l.tag_fin, theme::status_fg(preset, Color::Gray)),
        GameStatus::Canceled => (l.tag_cancel, theme::status_fg(preset, Color::DarkGray)),
        GameStatus::Suspended => (l.tag_susp, theme::status_fg(preset, Color::Magenta)),
    }
}

/// 본문 블록 타이틀: 이 목록이 "어느 날짜의 경기"인지 밝힌다(Tab UX fix).
fn block_title(app: &App) -> String {
    let l = app.labels();
    if app.date.is_empty() {
        format!(" {} ", l.title_games)
    } else {
        format!(" {} {} ", l.title_games, app.date)
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let l = app.labels();
    // 첫 Games 업데이트가 아직 안 왔으면(프리페치 순간) "loading"을, 왔는데
    // 배열이 비어 있으면(휴식일/전체 우천취소) "no games"를 보여준다 — live.rs가
    // Option<LiveState>로 이미 구분하는 것과 동일한 원칙. 구분 없이 빈 테이블만
    // 그리면 두 상태가 헤더 행만 있는 동일한 화면으로 보인다.
    if !app.games_loaded {
        f.render_widget(
            Paragraph::new(l.loading).block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }
    if app.games.is_empty() {
        f.render_widget(
            Paragraph::new(l.no_games).block(Block::bordered().title(block_title(app))),
            area,
        );
        return;
    }

    let header = Row::new(["", l.col_away, l.col_score, l.col_home, l.col_status]);

    let rows: Vec<Row> = app
        .games
        .iter()
        .map(|g| {
            let (tag, tag_style) = status_tag(g.status, l, &app.theme_preset);
            let score = match (g.away_score, g.home_score) {
                (Some(a), Some(h)) => format!("{a} : {h}"),
                _ => "— : —".to_string(),
            };
            // A-3: Scheduled 경기는 상태 칸에 "남은 시간"을 보여준다 — 서버가
            // 주는 status_label은 경기 전엔 대개 정보가 없다(빈 값·일반 문구).
            // 계산 실패(파싱 오류) 또는 이미 지난 시각(데이터 지연)이면 조용히
            // 기존 status_label로 저하한다(무패닉·§15 오버플로 정책과 같은 관용).
            let status_cell = if g.status == GameStatus::Scheduled {
                scheduled_eta_hm(app.now_secs, &g.start)
                    .map(|(h, m)| {
                        super::text::ellipsize(&scheduled_eta_label(l, h, m), STATUS_COL_WIDTH)
                    })
                    .unwrap_or_else(|| g.status_label.clone())
            } else {
                g.status_label.clone()
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
                Cell::from(status_cell),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(9),
        Constraint::Min(10),
        Constraint::Length(STATUS_COL_WIDTH as u16),
    ];

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

    // home/away는 의도적으로 "LG"·"OB"를 피한다 — 아래 selection_highlight_* 테스트들이
    // team_color("LG")/team_color("OB")를 "이 픽스처에는 없는 색" 기준으로 비교에 쓰기
    // 때문에, 두 팀을 fixture에 넣으면 그 팀 배지 배경이 우연히 겹쳐 오탐이 난다.
    fn game(id: &str) -> crate::model::Game {
        use crate::model::{GameStatus, Team};
        crate::model::Game {
            id: id.into(),
            start: "".into(),
            status: GameStatus::Live,
            status_label: "".into(),
            home: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            away: Team {
                code: "SK".into(),
                name: "SK".into(),
            },
            home_score: Some(1),
            away_score: Some(2),
        }
    }

    /// fav 설정 시 목록 선택 하이라이트가 team_color 배경으로 바뀐다(REVERSED 단독 대체).
    #[test]
    fn selection_highlight_uses_team_color_when_fav_set() {
        let mut app = App::new(Default::default());
        // OB는 KT@SK 픽스처에 없는 팀 — 배지에서는 절대 안 나오므로, 버퍼에 이 bg가
        // 있다면 오직 선택 하이라이트에서만 나온 것이다(KT를 쓰면 KT 자체 배지 bg와
        // 겹쳐 하이라이트 로직이 깨져도 통과하는 tautology가 된다).
        app.fav_code = Some("OB".into());
        app.apply(Update::Games(vec![game("g")])); // KT@SK 픽스처(OB 아님)
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();
        assert!(
            buf.content()
                .iter()
                .any(|c| c.bg == super::super::theme::team_color("OB")),
            "선택 행이 team_color(fav) 배경을 써야 한다"
        );
    }

    /// fav 미설정이면 현행(REVERSED) 그대로 — LG(픽스처에 없는 팀) 컬러 셀이 없어야 한다.
    /// game()의 KT/SK는 자체 배지로 team_color("KT")를 항상 그리므로 그 색은 비교 기준으로
    /// 쓸 수 없다(픽스처에 없는 LG로 "fav 기반 배경이 전혀 추가되지 않았다"를 검증한다).
    #[test]
    fn selection_highlight_unchanged_without_fav() {
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![game("g")]));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();
        assert!(!buf
            .content()
            .iter()
            .any(|c| c.bg == super::super::theme::team_color("LG")));
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

    #[test]
    fn korean_title_and_columns_render_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.date = "2026-05-29".into();
        app.apply(Update::Games(vec![game("g")]));
        let text = render_to_string(&app);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains("경기2026-05-29"));
        assert!(compact.contains("원정") && compact.contains("홈") && compact.contains("상태"));
    }

    // ---- A-3: 예정 경기 카운트다운 -------------------------------------

    /// 2026-07-19T00:00:00 KST의 UTC epoch초. `dateutil::days_from_civil`은
    /// 이미 알려진 경계값(윤년·연도 롤오버)으로 별도 검증돼 있으므로(dateutil.rs
    /// 자체 테스트) 여기서는 그 결과를 그대로 재사용해 "KST 자정"이라는 기준
    /// 시각을 만든다 — 매직 넘버를 하드코딩하지 않는다.
    fn midnight_kst_utc(y: i64, m: i64, d: i64) -> i64 {
        crate::dateutil::days_from_civil(y, m, d) * 86400 - KST_OFFSET_SECS
    }

    fn scheduled_game(id: &str, start: &str) -> crate::model::Game {
        let mut g = game(id);
        g.status = GameStatus::Scheduled;
        g.start = start.into();
        g.status_label = "".into();
        g
    }

    /// 미래 시각(1시간 20분 후) → (1, 20).
    #[test]
    fn scheduled_eta_hm_future_time_returns_hours_and_minutes() {
        let now = midnight_kst_utc(2026, 7, 19) as u64 + 18 * 3600; // 18:00 KST
        let start = "2026-07-19T19:20:00"; // 19:20 KST = 1h20m 후
        assert_eq!(scheduled_eta_hm(now, start), Some((1, 20)));
    }

    /// 60초 미만으로 임박해도 최소 1분으로 올림한다("0분 후" 금지).
    #[test]
    fn scheduled_eta_hm_rounds_up_to_at_least_one_minute() {
        let now = midnight_kst_utc(2026, 7, 19) as u64 + 18 * 3600;
        let start = "2026-07-19T18:00:30"; // 30초 후
        assert_eq!(scheduled_eta_hm(now, start), Some((0, 1)));
    }

    /// 이미 지난 시각(Scheduled인데 시작이 과거) → None(억지 음수 표기 금지).
    #[test]
    fn scheduled_eta_hm_returns_none_for_a_past_start() {
        let now = midnight_kst_utc(2026, 7, 19) as u64 + 18 * 3600;
        let start = "2026-07-19T17:59:00"; // 1분 전(과거)
        assert_eq!(scheduled_eta_hm(now, start), None);
    }

    /// 정각(0초 남음)도 "이미 지남" 취급 — 경계값.
    #[test]
    fn scheduled_eta_hm_returns_none_when_exactly_now() {
        let now = midnight_kst_utc(2026, 7, 19) as u64 + 18 * 3600;
        let start = "2026-07-19T18:00:00";
        assert_eq!(scheduled_eta_hm(now, start), None);
    }

    /// 날짜가 다음날로 넘어가는 경우(심야 더블헤더 2차전 등)도 절대시각
    /// 비교라 자연스럽게 맞는다 — live.rs식 "시:분만 보고 자정이면 +24h"
    /// 보정 없이도 정확하다.
    #[test]
    fn scheduled_eta_hm_handles_a_start_after_midnight_the_next_day() {
        let now = midnight_kst_utc(2026, 7, 19) as u64 + 23 * 3600 + 50 * 60; // 23:50 KST
        let start = "2026-07-20T00:30:00"; // 다음날 00:30 KST = 40분 후
        assert_eq!(scheduled_eta_hm(now, start), Some((0, 40)));
    }

    /// 파싱 실패(형식 오류·T 없음·월 범위 밖 등)는 패닉 없이 None.
    #[test]
    fn scheduled_eta_hm_parse_failures_do_not_panic() {
        let now = midnight_kst_utc(2026, 7, 19) as u64;
        for bad in ["garbage", "2026-07-19", "2026-13-40T18:00:00", ""] {
            assert_eq!(scheduled_eta_hm(now, bad), None, "input: {bad}");
        }
    }

    /// 연도는 서버가 보내는 값이라 터무니없이 클 수 있다. 범위를 안 막으면
    /// `days * 86400`이 오버플로해 debug 빌드가 렌더 도중 패닉했다(최종 리뷰
    /// 지적) — 렌더 경로엔 catch_unwind가 없으니 표시 생략으로 떨어져야 한다.
    #[test]
    fn scheduled_eta_hm_absurd_year_is_dropped_instead_of_overflowing() {
        let now = midnight_kst_utc(2026, 7, 19) as u64;
        for bad in [
            "999999999999-01-01T00:00:00",
            "92233720368547758-01-01T00:00:00",
            "63113904-01-01T00:00:00",
            "0000-01-01T00:00:00",
            "-1-01-01T00:00:00",
        ] {
            assert_eq!(scheduled_eta_hm(now, bad), None, "input: {bad}");
        }
    }

    /// (시,분) → 완성형 문자열: 설계 §2 A-3의 예시 "1시간 20분 후"와 정확히 일치.
    #[test]
    fn scheduled_eta_label_matches_the_design_example_in_korean() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        assert_eq!(scheduled_eta_label(l, 1, 20), "1시간 20분 후");
    }

    /// 시가 0이면 분만 보여준다("20분 후", 시간 부분 생략).
    #[test]
    fn scheduled_eta_label_omits_hour_part_when_zero() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        assert_eq!(scheduled_eta_label(l, 0, 20), "20분 후");
    }

    /// EN/JA도 각 언어 완성형으로 조립된다(i18n 경유 확인).
    #[test]
    fn scheduled_eta_label_renders_in_english_and_japanese() {
        let en = crate::ui::i18n::labels(crate::ui::i18n::Lang::En);
        assert_eq!(scheduled_eta_label(en, 1, 20), "1h 20m to go");
        let ja = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ja);
        assert_eq!(scheduled_eta_label(ja, 1, 20), "1時間20分後");
    }

    /// 렌더 통합: Scheduled 경기 목록에 카운트다운 문자열이 실제로 나타난다.
    #[test]
    fn render_shows_countdown_for_a_scheduled_game() {
        let mut app = App::new(Default::default());
        app.now_secs = midnight_kst_utc(2026, 7, 19) as u64 + 18 * 3600;
        app.apply(Update::Games(vec![scheduled_game(
            "g",
            "2026-07-19T19:20:00",
        )]));
        let text = render_to_string(&app);
        assert!(
            text.contains("1h 20m to go"),
            "countdown missing from rendered table:\n{text}"
        );
    }

    /// 렌더 통합: 이미 지난 시작 시각이면 카운트다운 대신 기존 status_label로
    /// 조용히 저하한다(빈 문자열이어도 패닉 없음).
    #[test]
    fn render_falls_back_to_status_label_for_a_past_scheduled_start() {
        let mut app = App::new(Default::default());
        app.now_secs = midnight_kst_utc(2026, 7, 19) as u64 + 18 * 3600;
        let mut g = scheduled_game("g", "2026-07-19T17:00:00"); // 1시간 전(과거)
        g.status_label = "예정".into();
        app.apply(Update::Games(vec![g]));
        let text = render_to_string(&app);
        assert!(
            !text.contains("to go"),
            "must not show a bogus countdown:\n{text}"
        );
        // ratatui는 전각(2-width) 문자 뒤에 placeholder 공백 셀을 채워 넣으므로
        // (다른 games.rs 테스트와 동일 사유) 공백을 제거하고 검사한다.
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("예정"),
            "must fall back to status_label:\n{text}"
        );
    }

    /// 렌더 통합: start가 파싱 불가능해도 패닉하지 않고 status_label로 저하한다.
    #[test]
    fn render_does_not_panic_when_scheduled_start_is_unparseable() {
        let mut app = App::new(Default::default());
        let mut g = scheduled_game("g", "garbage");
        g.status_label = "TBD".into();
        app.apply(Update::Games(vec![g]));
        let text = render_to_string(&app);
        assert!(text.contains("TBD"));
    }

    /// 렌더 통합: Live 등 Scheduled가 아닌 경기는 기존처럼 status_label 그대로다
    /// (카운트다운 로직이 다른 상태를 건드리지 않는다는 회귀 방지).
    #[test]
    fn render_leaves_non_scheduled_status_label_untouched() {
        let mut app = App::new(Default::default());
        app.apply(Update::Games(vec![game("g")])); // status: Live, status_label: ""
        let text = render_to_string(&app);
        assert!(!text.contains("to go"));
    }
}
