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

/// 선발 매치업 칸("올러 vs 최원태"). 한글 이름 3자 + " vs " + 3자를 기준으로
/// 잡되 외국인 선수 이름이 길어질 수 있어 여유를 뒀다.
const STARTERS_COL_WIDTH: usize = 20;
/// 구장·중계 칸("대구 · SPOTV").
const VENUE_COL_WIDTH: usize = 18;
/// 선발 칸을 켜는 최소 내부 폭(기존 칼럼 합계 + 팀명 최소폭 + 선발 칸).
const WIDTH_FOR_STARTERS: u16 = 78;
/// 구장 칸까지 켜는 최소 내부 폭.
const WIDTH_FOR_VENUE: u16 = 97;

pub fn render(f: &mut Frame, area: Rect, app: &App, hits: &mut super::hit::HitMap) {
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

    // 폭 예산(v0.23): 남을 때만 선발 매치업 → 구장 순으로 붙인다. 중계 채널은
    // 종료 경기에서 빈 값이라(실측) 칸을 예약하지 않고 구장 칸에 함께 싣는다.
    let inner = area.width.saturating_sub(2);
    let show_starters = inner >= WIDTH_FOR_STARTERS;
    let show_venue = inner >= WIDTH_FOR_VENUE;

    let mut header_cells = vec!["", l.col_away, l.col_score, l.col_home, l.col_status];
    if show_starters {
        header_cells.push(l.col_starters);
    }
    if show_venue {
        header_cells.push(l.col_venue);
    }
    let header = Row::new(header_cells);

    let rows: Vec<Row> = app
        .games
        .iter()
        .map(|g| {
            let (tag, tag_style) = status_tag(g.status, l, &app.theme_preset);
            // 아직 시작 안 한 경기는 점수를 비운다(v0.23). 서버는 예정 경기에도
            // `homeTeamScore: 0`을 주므로(실측) 값만 보면 "0 : 0"이 찍혀 무승부
            // 중인 것처럼 보인다 — 상태로 판단해야 한다. 실행 확인에서 잡혔다.
            let score = match (g.status, g.away_score, g.home_score) {
                (GameStatus::Scheduled, _, _) => "— : —".to_string(),
                (_, Some(a), Some(h)) => format!("{a} : {h}"),
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
            // 선발은 경기 하루 전쯤 확정된다 — 그 전에는 빈 문자열이라 칸을 비운다
            // ("미정" 같은 문구를 지어내지 않는다).
            let starters = match (g.away_starter.as_str(), g.home_starter.as_str()) {
                ("", "") => String::new(),
                (a, h) => format!("{a} vs {h}"),
            };
            // 중계 채널은 끝난 경기에서 빈다 — 구장만 남는다.
            let venue = match (g.stadium.as_str(), g.broadcast.as_str()) {
                ("", "") => String::new(),
                (s, "") => s.to_string(),
                ("", b) => b.to_string(),
                (s, b) => format!("{s} · {b}"),
            };
            let mut cells = vec![
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
            ];
            if show_starters {
                cells.push(Cell::from(super::text::ellipsize(
                    &starters,
                    STARTERS_COL_WIDTH,
                )));
            }
            if show_venue {
                cells.push(Cell::from(super::text::ellipsize(&venue, VENUE_COL_WIDTH)));
            }
            Row::new(cells)
        })
        .collect();

    let mut widths = vec![
        Constraint::Length(6),
        // 팀명은 이 화면의 본체다. 전각 6글자("기아타이거즈"=12칸)까지 안 잘리도록
        // 최소폭을 12로 둔다 — v0.23에서 선발·구장 칸을 붙이자 10으로는 밀렸다.
        Constraint::Min(12),
        Constraint::Length(9),
        Constraint::Min(12),
        Constraint::Length(STATUS_COL_WIDTH as u16),
    ];
    if show_starters {
        widths.push(Constraint::Length(STARTERS_COL_WIDTH as u16));
    }
    if show_venue {
        widths.push(Constraint::Length(VENUE_COL_WIDTH as u16));
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
    // 그린 뒤에 등록한다 — 스크롤 오프셋은 ratatui가 정하므로, 우리가 다시
    // 계산하는 대신 렌더가 끝난 state에서 읽는다(hit.rs 모듈 주석의 이유).
    push_row_hits(hits, area, state.offset(), app.games.len());
}

/// 테이블 본문 각 행의 영역을 등록한다. 본문은 테두리(1) + 헤더 행(1) 아래부터
/// 시작하고, 영역 밖으로 나가는 행은 **화면에 없으므로** 등록하지 않는다.
fn push_row_hits(hits: &mut super::hit::HitMap, area: Rect, offset: usize, len: usize) {
    const HEAD: u16 = 2; // 위 테두리 + 헤더 행
    let body_h = area.height.saturating_sub(HEAD + 1); // 아래 테두리
    for row in 0..body_h {
        let idx = offset + row as usize;
        if idx >= len {
            break;
        }
        let r = Rect::new(
            area.x + 1,
            area.y + HEAD + row,
            area.width.saturating_sub(2),
            1,
        );
        hits.push(r, super::hit::Zone::GameRow(idx));
    }
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
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
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
        term.draw(|f| render(f, f.area(), &app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
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
        term.draw(|f| render(f, f.area(), &app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
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
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
        }]));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
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
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
        }]));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
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
    /// v0.23: 넓은 터미널에서 선발 매치업과 구장이 뜬다.
    #[test]
    fn wide_terminal_shows_starters_and_venue() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::En;
        let mut g = game("g");
        g.away_starter = "올러".into();
        g.home_starter = "최원태".into();
        g.stadium = "대구".into();
        g.broadcast = "SPOTV".into();
        app.apply(Update::Games(vec![g]));

        let text = render_at(&app, 120, 8);
        for needle in ["올러", "최원태", "대구", "SPOTV"] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
    }

    /// 선발이 아직 안 나온 경기(이틀 뒤)는 칸을 비운다 — "미정" 같은 문구를
    /// 지어내지 않는다(모르는 건 안 보여준다).
    #[test]
    fn an_unannounced_starter_leaves_the_cell_blank() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::En;
        let mut g = game("g");
        g.stadium = "대구".into();
        app.apply(Update::Games(vec![g]));

        let text = render_at(&app, 120, 8);
        assert!(text.contains("대구"), "구장은 그 전에도 뜬다:\n{text}");
        assert!(!text.contains("vs"), "선발 없는데 구분자가 떴다:\n{text}");
        assert!(!text.to_lowercase().contains("tbd"));
    }

    /// 끝난 경기는 중계 채널이 비므로 구장만 남는다 — 가운뎃점 구분자가
    /// 덩그러니 남지 않아야 한다.
    #[test]
    fn a_finished_game_shows_the_venue_without_a_dangling_separator() {
        let mut app = App::new(Default::default());
        let mut g = game("g");
        g.stadium = "사직".into();
        g.broadcast = String::new();
        app.apply(Update::Games(vec![g]));

        let text = render_at(&app, 120, 8);
        assert!(text.contains("사직"));
        assert!(!text.contains("· "), "구분자만 남았다:\n{text}");
    }

    /// 좁아지면 뒤쪽부터 뗀다 — 구장이 먼저, 그다음 선발. 팀·스코어는 남는다.
    #[test]
    fn narrow_terminals_drop_venue_then_starters() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::En;
        let mut g = game("g");
        g.away_starter = "올러".into();
        g.home_starter = "최원태".into();
        g.stadium = "대구".into();
        app.apply(Update::Games(vec![g]));

        let mid = render_at(&app, 85, 6);
        assert!(mid.contains("올러"), "선발이 너무 일찍 빠졌다:\n{mid}");
        assert!(!mid.contains("대구"), "좁은데 구장이 남았다:\n{mid}");

        let narrow = render_at(&app, 70, 6);
        assert!(!narrow.contains("올러"), "좁은데 선발이 남았다:\n{narrow}");
        assert!(
            narrow.contains("SK"),
            "팀명은 어떤 폭에서도 남는다:\n{narrow}"
        );
    }

    /// 지정한 폭으로 렌더한 뒤 **공백을 모두 제거한** 문자열을 돌려준다.
    /// ratatui는 전각(2칸) 문자 뒤에 placeholder 공백 셀을 채우므로("올 러"),
    /// 공백을 남기면 한글 부분 문자열 검사가 항상 실패한다(이 파일의 다른
    /// 테스트들과 같은 관례).
    fn render_at(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, f.area(), app, &mut crate::ui::hit::HitMap::default()))
            .unwrap();
        let buf = term.backend().buffer();
        let raw: String = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        raw.chars().filter(|c| !c.is_whitespace()).collect()
    }
    /// 예정 경기는 점수를 비운다 — 서버가 `0`을 주지만(실측) 시작도 안 한 경기가
    /// "0 : 0"으로 뜨면 무승부 중인 것처럼 읽힌다. v0.23 실행 확인에서 잡혔다.
    #[test]
    fn a_scheduled_game_shows_no_score_even_though_the_server_sends_zero() {
        let mut app = App::new(Default::default());
        let mut g = game("g");
        g.status = GameStatus::Scheduled;
        g.away_score = Some(0);
        g.home_score = Some(0);
        app.apply(Update::Games(vec![g]));

        let text = render_at(&app, 120, 6);
        assert!(!text.contains("0:0"), "예정 경기에 0:0이 떴다:\n{text}");
        assert!(text.contains("—:—"), "빈 점수 표시가 없다:\n{text}");
    }

    /// 진행·종료 경기는 0점도 그대로 보여준다 — 실제로 0점일 수 있다.
    #[test]
    fn a_live_game_still_shows_a_genuine_zero_score() {
        let mut app = App::new(Default::default());
        let mut g = game("g");
        g.status = GameStatus::Live;
        g.away_score = Some(0);
        g.home_score = Some(3);
        app.apply(Update::Games(vec![g]));

        let text = render_at(&app, 120, 6);
        assert!(
            text.contains("0:3"),
            "진행 경기의 실제 점수가 사라졌다:\n{text}"
        );
    }
    /// 그린 자리에 등록되는가 — 히트맵의 값어치는 **렌더가 실제로 그린 좌표**를
    /// 담는 데 있다. 첫 행은 테두리와 헤더 행 아래, 그 다음은 한 줄씩.
    #[test]
    fn each_visible_row_is_registered_where_it_was_drawn() {
        let mut app = App::new(Default::default());
        app.games_loaded = true;
        app.games = vec![game("a"), game("b")];
        let mut hits = crate::ui::hit::HitMap::default();
        let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
        term.draw(|f| render(f, f.area(), &app, &mut hits)).unwrap();
        assert_eq!(hits.at(2, 2), Some(crate::ui::hit::Zone::GameRow(0)));
        assert_eq!(hits.at(2, 3), Some(crate::ui::hit::Zone::GameRow(1)));
        assert_eq!(hits.at(2, 4), None, "경기가 둘뿐인데 셋째 줄이 잡혔다");
        assert_eq!(hits.at(2, 1), None, "헤더 행은 누를 것이 아니다");
    }

    /// 목록보다 화면이 낮으면 **안 보이는 행은 등록되지 않는다**. 보이지도 않는
    /// 것을 누를 수 있으면 사용자는 자기가 뭘 눌렀는지 알 수 없다.
    #[test]
    fn rows_below_the_fold_are_not_registered() {
        let mut app = App::new(Default::default());
        app.games_loaded = true;
        app.games = (0..8).map(|i| game(&format!("g{i}"))).collect();
        let mut hits = crate::ui::hit::HitMap::default();
        // 테두리 2 + 헤더 1 = 3줄을 빼면 본문은 2줄뿐이다.
        let mut term = Terminal::new(TestBackend::new(80, 6)).unwrap();
        term.draw(|f| render(f, f.area(), &app, &mut hits)).unwrap();
        assert!(hits.at(2, 2).is_some());
        assert_eq!(hits.at(2, 5), None, "테두리 자리가 클릭 영역이 됐다");
    }
}
