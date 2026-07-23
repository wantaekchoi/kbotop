use super::strikezone;
use super::theme::team_badge_style;
use crate::app::{App, Screen};
use crate::model::{GameStatus, LiveState};
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
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    render_scoreline(f, rows[0], s, game.status);

    // 폭이 좁거나 아직 투구 데이터가 없으면 존을 숨기고 중계에 본문 전체를 준다(우아한 저하).
    let wide = rows[1].width >= 70 && !s.current_pitches.is_empty();
    if wide {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        render_relay(f, cols[0], s);
        strikezone::render(f, cols[1], &s.current_pitches);
    } else {
        render_relay(f, rows[1], s);
    }
}

fn win_pct(rate: Option<f32>) -> String {
    rate.map(|r| format!("{:.0}%", r * 100.0))
        .unwrap_or_else(|| "-".into())
}

fn render_scoreline(f: &mut Frame, area: Rect, s: &LiveState, status: GameStatus) {
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

    let detail_line = Line::from(format!("P: {}   B: {}", s.pitcher_name, s.batter_name));

    f.render_widget(
        Paragraph::new(vec![score_line, detail_line]).block(Block::bordered().title(" Live ")),
        area,
    );
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
