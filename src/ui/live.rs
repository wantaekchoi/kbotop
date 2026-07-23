use super::strikezone;
use super::theme::team_badge_style;
use crate::app::{App, Screen};
use crate::model::{Game, GameStatus, LiveState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

/// Live/Suspended/Final 외 상태(can_enter_live가 걸러내는 Canceled/Scheduled)는
/// 이 화면에 들어오지 않으므로 배지가 필요 없다 — None을 반환해 그대로 숨긴다.
/// 색은 games.rs의 status_tag와 맞춘다(같은 상태는 같은 색으로 보이도록).
fn status_badge(status: GameStatus) -> Option<(&'static str, Style)> {
    match status {
        GameStatus::Suspended => Some((
            "SUSPENDED",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        GameStatus::Final => Some((
            "FINAL",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )),
        GameStatus::Live | GameStatus::Scheduled | GameStatus::Canceled => None,
    }
}

/// 라이브 뷰: 스코어라인(점수/카운트/주자/승률) + 문자중계(+ 폭 충분 시 스트라이크존).
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Screen::Live { game, state } = &app.screen else {
        return;
    };
    let Some(s) = state else {
        f.render_widget(
            Paragraph::new("loading...").block(Block::bordered().title(" Live ")),
            area,
        );
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    render_scoreline(f, rows[0], s, game, app.live_pitch_sel);

    // 폭이 좁거나 아직 투구 데이터가 없으면 존을 숨기고 중계에 본문 전체를 준다(우아한 저하).
    let wide = rows[1].width >= 70 && !s.current_pitches.is_empty();
    if wide {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        render_relay(f, cols[0], s);
        strikezone::render(f, cols[1], &s.current_pitches, app.live_pitch_sel);
    } else {
        render_relay(f, rows[1], s);
    }
}

fn win_pct(rate: Option<f32>) -> String {
    rate.map(|r| format!("{:.0}%", r * 100.0))
        .unwrap_or_else(|| "-".into())
}

fn render_scoreline(f: &mut Frame, area: Rect, s: &LiveState, game: &Game, sel: Option<usize>) {
    let status = game.status;
    // 3슬롯 ASCII 주자 표시: [3루 2루 1루], 빈 베이스는 '-' — 폭 고정.
    let bases = format!(
        "[{} {} {}]",
        if s.bases.third { "3" } else { "-" },
        if s.bases.second { "2" } else { "-" },
        if s.bases.first { "1" } else { "-" },
    );

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut spans = vec![
        Span::styled(s.away.name.as_str(), team_badge_style(&s.away.code)),
        Span::raw(" "),
        Span::styled(s.away_score.to_string(), bold),
        Span::raw(" : "),
        Span::styled(s.home_score.to_string(), bold),
        Span::raw(" "),
        Span::styled(s.home.name.as_str(), team_badge_style(&s.home.code)),
        Span::raw("   "),
        Span::raw(s.inning_label.as_str()),
    ];
    // 서스펜디드/종료 경기는 스코어라인만 봐서는 진행 중인 경기와 구분이
    // 안 된다 — inning_label 옆에 배지를 붙여 우아하게 저하시킨다.
    if let Some((label, style)) = status_badge(status) {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label, style));
    }
    spans.extend([
        Span::raw("   "),
        Span::raw(format!(
            "B{} S{} O{}",
            s.count.ball, s.count.strike, s.count.out
        )),
        Span::raw("   "),
        Span::raw(bases),
        Span::raw("   "),
        Span::raw(format!(
            "WP {}/{}",
            win_pct(s.away_win_rate),
            win_pct(s.home_win_rate)
        )),
    ]);
    let score_line = Line::from(spans);

    // "HH:MM" 경기 시작 시각("....THH:MM:SS"에서 추출, 실패 시 생략).
    let start_hhmm = game
        .start
        .split('T')
        .nth(1)
        .and_then(|t| t.get(0..5))
        .unwrap_or("");
    let mut detail = format!("P: {}   B: {}", s.pitcher_name, s.batter_name);
    if !s.next_batter_name.is_empty() {
        detail.push_str(&format!("   Next: {}", s.next_batter_name));
    }
    if !start_hhmm.is_empty() {
        detail.push_str(&format!("   Start {start_hhmm}"));
    }
    let detail_line = Line::from(detail);

    // 셋째 줄: 선택된 투구 상세(시각·상대시간·결과 원문) 또는 네비 힌트.
    let pitch_line = match sel.and_then(|i| s.current_pitches.get(i).map(|p| (i, p))) {
        Some((i, p)) => {
            let speed = p
                .speed_kmh
                .map(|k| format!("{k}km"))
                .unwrap_or_else(|| "-".into());
            let time = p.time_hms.as_deref().unwrap_or("-");
            let rel = p
                .time_hms
                .as_deref()
                .and_then(|t| elapsed_label(&game.start, t))
                .unwrap_or_default();
            // 결과 원문이 길면 좁은 터미널에서 조용히 잘린다 — 정직한 말줄임
            // (테두리 2칸 제외한 내부 폭 기준, §15 오버플로 정책).
            Line::from(super::text::ellipsize(
                &format!(
                    "Pitch {}/{}  {}  {} {}  {}",
                    i + 1,
                    s.current_pitches.len(),
                    speed,
                    time,
                    rel,
                    p.text
                ),
                area.width.saturating_sub(2) as usize,
            ))
        }
        None if !s.current_pitches.is_empty() => Line::from(format!(
            "Pitches {}  (Left/Right to inspect)",
            s.current_pitches.len()
        )),
        None => Line::from(""),
    };

    f.render_widget(
        Paragraph::new(vec![score_line, detail_line, pitch_line])
            .block(Block::bordered().title(" Live ")),
        area,
    );
}

/// 경기 시작("....THH:MM:SS")과 투구 시각("HH:MM:SS")의 차 → "(+H:MM)".
/// 자정 넘김(투구 < 시작)은 +24h 보정. 파싱 실패는 None(관용 — 표시 생략).
fn elapsed_label(game_start: &str, pitch_hms: &str) -> Option<String> {
    fn secs(hms: &str) -> Option<i64> {
        let mut it = hms.split(':');
        let h: i64 = it.next()?.parse().ok()?;
        let m: i64 = it.next()?.parse().ok()?;
        let s: i64 = it.next().unwrap_or("0").parse().ok()?;
        ((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&s))
            .then_some(h * 3600 + m * 60 + s)
    }
    let start = secs(game_start.split('T').nth(1)?)?;
    let pitch = secs(pitch_hms)?;
    let mut d = pitch - start;
    if d < 0 {
        d += 24 * 3600;
    }
    Some(format!("(+{}:{:02})", d / 3600, (d % 3600) / 60))
}

fn render_relay(f: &mut Frame, area: Rect, s: &LiveState) {
    // 오래된→최신 순으로 저장돼 있으므로 꼬리(N줄)만 그대로 잘라 쓰면
    // 최신이 리스트 맨 아래에 온다.
    let n = area.height.saturating_sub(2) as usize;
    let start = s.relay_log.len().saturating_sub(n);
    let items: Vec<ListItem> = s.relay_log[start..]
        .iter()
        .map(|l| ListItem::new(format!("· {l}")))
        .collect();
    f.render_widget(
        List::new(items).block(Block::bordered().title(" Play-by-play ")),
        area,
    );
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Screen};
    use crate::model::{Game, GameStatus, Team};
    use ratatui::{backend::TestBackend, Terminal};

    const RELAY: &str = include_str!("../../tests/fixtures/relay_20260719KTLG.json");

    fn team(code: &str, name: &str) -> Team {
        Team {
            code: code.into(),
            name: name.into(),
        }
    }

    fn live_screen() -> Screen {
        live_screen_with_status(GameStatus::Live)
    }

    fn live_screen_with_status(status: GameStatus) -> Screen {
        let state =
            crate::source::naver::map::live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT"))
                .unwrap();
        let game = Game {
            id: "20260719KTLG02026".into(),
            start: "".into(),
            status,
            status_label: state.inning_label.clone(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(state.home_score),
            away_score: Some(state.away_score),
        };
        Screen::Live {
            game,
            state: Some(state),
        }
    }

    fn render_to_string(app: &App, width: u16, height: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// header.rs가 게임 목록 전체를 훑어 "LIVE {n}"/"FINAL {n}" 탈리를 항상 그리므로,
    /// 배지 텍스트("SUSPENDED"/"FINAL") 유무를 검사할 때 전체 앱(crate::ui::draw)을
    /// 쓰면 header의 상시 표시 텍스트와 우연히 겹친다 — live::render만 직접 그려 피한다.
    fn render_live_view_only(app: &App, width: u16, height: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| super::render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn live_view_shows_score_count_and_relay() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let text = render_to_string(&app, 100, 30);
        assert!(text.contains('4')); // away score (fixture)
                                     // 값 특정 검사: "B{} S{} O{}" 포맷 리터럴 자체가 항상 B/S/O 글자를
                                     // 포함하므로(값이 뒤바뀌어도 통과) count 값까지 함께 확인한다.
                                     // fixture 실측값: ball=2, strike=3, out=3 (tests/parse_relay.rs와 동일).
        assert!(text.contains("B2 S3 O3"));
        // ratatui는 전각(2-width) 문자 뒤에 placeholder 공백 셀을 채워 넣으므로
        // (ui/mod.rs 테스트와 동일한 이유) 공백을 제거하고 부분 문자열을 검사한다.
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains("파울") || compact.contains("헛스윙"));
    }

    #[test]
    fn live_view_renders_without_panic_when_narrow() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        // width < 70 → strike zone hidden, relay gets full body; must not panic.
        let _text = render_to_string(&app, 50, 20);
    }

    #[test]
    fn live_view_shows_suspended_badge_for_suspended_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Suspended);
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("SUSPENDED"));
    }

    #[test]
    fn live_view_shows_final_badge_for_finished_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("FINAL"));
    }

    #[test]
    fn live_view_shows_no_badge_for_live_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Live);
        let text = render_live_view_only(&app, 100, 30);
        assert!(!text.contains("SUSPENDED"));
        assert!(!text.contains("FINAL"));
    }

    #[test]
    fn scoreline_team_name_has_team_color_badge() {
        let mut app = App::new(Default::default());
        // away = 두산(OB) 로 스코어라인 렌더
        let state =
            crate::source::naver::map::live_from_relay(RELAY, team("OB", "두산"), team("LG", "LG"))
                .unwrap();
        let game = Game {
            id: "g".into(),
            start: "".into(),
            status: GameStatus::Live,
            status_label: state.inning_label.clone(),
            home: team("LG", "LG"),
            away: team("OB", "두산"),
            home_score: Some(state.home_score),
            away_score: Some(state.away_score),
        };
        app.screen = Screen::Live {
            game,
            state: Some(state),
        };
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| super::render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer();
        let has_badge = buf
            .content()
            .iter()
            .any(|c| c.bg == super::super::theme::team_color("OB"));
        assert!(
            has_badge,
            "scoreline away team name should render on OB team-color background"
        );
    }

    /// 선택된 투구의 상세줄: 순번/전체, 구속, 시각, 상대시간, 결과 원문 전부.
    #[test]
    fn selected_pitch_detail_line_shows_speed_time_elapsed_and_text() {
        let mut app = App::new(Default::default());
        app.screen = live_screen(); // fixture 기반
                                    // fixture의 첫 투구를 선택하고 시각을 주입해 결정적으로 검증한다.
        if let Screen::Live {
            game,
            state: Some(s),
        } = &mut app.screen
        {
            game.start = "2026-07-19T18:30:00".into();
            s.current_pitches[0].time_hms = Some("20:56:14".into());
        }
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("Pitch 1/"), "detail line missing:\n{text}");
        assert!(text.contains("20:56:14"));
        assert!(text.contains("(+2:26)"));
    }

    #[test]
    fn unselected_live_view_advertises_pitch_navigation() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("Left/Right"), "nav hint missing:\n{text}");
    }

    #[test]
    fn detail_line_shows_next_batter_when_known() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.next_batter_name = "홍창기".into();
        }
        let text = render_live_view_only(&app, 100, 30);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("Next:홍창기"),
            "next batter missing:\n{text}"
        );
    }

    #[test]
    fn elapsed_label_formats_and_handles_midnight_rollover() {
        assert_eq!(
            super::elapsed_label("2026-07-19T18:30:00", "20:56:14").as_deref(),
            Some("(+2:26)")
        );
        assert_eq!(
            super::elapsed_label("2026-07-19T23:30:00", "00:10:00").as_deref(),
            Some("(+0:40)") // 자정 넘김 보정
        );
        assert_eq!(super::elapsed_label("garbage", "20:56:14"), None);
    }

    /// 긴 결과 원문은 상세줄에서 말줄임된다(§15 오버플로 정책).
    #[test]
    fn long_pitch_text_is_ellipsized_in_the_detail_line() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.current_pitches[0].text = "매우 긴 결과 설명 ".repeat(20);
        }
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 80, 30);
        assert!(
            text.contains('…'),
            "expected honest ellipsis in detail line"
        );
    }

    #[test]
    fn live_view_shows_loading_when_state_none() {
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: "".into(),
                home: team("LG", "LG"),
                away: team("KT", "KT"),
                home_score: None,
                away_score: None,
            },
            state: None,
        };
        let text = render_to_string(&app, 100, 30);
        assert!(text.contains("Live"));
        assert!(text.contains("loading"));
    }
}
