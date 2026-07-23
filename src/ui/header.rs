use super::theme::team_badge_style;
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
    let l = app.labels();
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

    let mut counts_spans: Vec<Span> = vec![
        Span::styled(
            format!("{} {live}", l.count_live),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} {sched}", l.count_sched),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} {fin}", l.count_final),
            Style::default().fg(Color::Green),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} {other}", l.count_other),
            Style::default().fg(Color::Magenta),
        ),
    ];

    // 응원 팀 배지: 1행 우측에 팀컬러 배지(bg+대비 글자색) + GO!. 테두리·탭·스피너는
    // 배경 무관 가독을 위해 named color/reverse만 쓰고 팀컬러 fg는 쓰지 않는다(v0.5).
    if let Some(code) = app.fav_code.as_deref() {
        counts_spans.push(Span::raw("   "));
        counts_spans.push(Span::styled(format!(" {code} "), team_badge_style(code)));
        counts_spans.push(Span::styled(
            " GO!",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    }

    let counts = Line::from(counts_spans);

    let active = Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD);
    let inactive = Style::default().add_modifier(Modifier::DIM);
    // 활성 탭은 브래킷으로도 표시한다: 반전이 미묘한 터미널·색각 사용자도
    // 텍스트만으로 현재 탭을 읽을 수 있다(v0.2 Tab UX fix). 라벨 폭을
    // 활성/비활성 동일하게 맞춰 토글 시 우측 요소가 흔들리지 않게 한다.
    let (games_label, games_style, standings_label, standings_style) = match app.tab {
        Tab::Games => (
            format!("[ {} ]", l.tab_games),
            active,
            format!("  {}  ", l.tab_standings),
            inactive,
        ),
        Tab::Standings => (
            format!("  {}  ", l.tab_games),
            inactive,
            format!("[ {} ]", l.tab_standings),
            active,
        ),
    };
    let mut tab_spans = vec![
        Span::styled(games_label, games_style),
        Span::raw(" | "),
        Span::styled(standings_label, standings_style),
    ];
    // fetch in-flight 동안 도는 ASCII 스피너(docker pull 스타일) — 폴링이
    // "지금 뭔가 하고 있음"을 구석에서 알린다. 유휴 시에는 아무것도 없다.
    const SPINNER: [char; 4] = ['|', '/', '-', '\\'];
    if app.fetching {
        tab_spans.push(Span::raw("   "));
        tab_spans.push(Span::styled(
            SPINNER[(app.spinner_frame % 4) as usize].to_string(),
            Style::default().fg(Color::Cyan),
        ));
    }
    if app.stale {
        tab_spans.push(Span::raw("   "));
        tab_spans.push(Span::styled(
            l.stale,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }
    let tabs = Line::from(tab_spans);

    let block = Block::default().borders(Borders::ALL).title(" kbotop ");

    let paragraph = Paragraph::new(vec![counts, tabs]).block(block);
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

    /// 활성 탭은 브래킷 텍스트 단서로도 식별돼야 한다(REVERSED 반전이 안 보이는
    /// 터미널·색각 사용자 대응, WCAG 1.4.1) — v0.2 Tab UX 버그의 핵심 회귀 방지.
    #[test]
    fn active_tab_is_bracketed_games_first() {
        let app = App::new(Default::default());
        let text = render_to_string(&app);
        assert!(text.contains("[ GAMES ]"));
        assert!(!text.contains("[ STANDINGS ]"));
    }

    #[test]
    fn active_tab_bracket_moves_to_standings_on_switch() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        let text = render_to_string(&app);
        assert!(text.contains("[ STANDINGS ]"));
        assert!(!text.contains("[ GAMES ]"));
    }

    /// fetch가 in-flight인 동안 헤더 구석에 스피너가 돈다(docker pull 스타일).
    /// '/' 프레임으로 고정해 탭 구분자 '|'와의 모호성을 피한다.
    #[test]
    fn spinner_shows_while_fetching_and_hides_when_idle() {
        let mut app = App::new(Default::default());
        app.fetching = true;
        app.spinner_frame = 1; // SPINNER[1] == '/'
        let busy = render_to_string(&app);
        assert!(busy.contains('/'), "spinner frame missing:\n{busy}");
        app.fetching = false;
        let idle = render_to_string(&app);
        assert!(!idle.contains('/'));
    }

    /// 응원 팀이 설정되면 헤더에 팀컬러 배지("GO!" 옆)가 뜬다. 테두리는 배경 무관
    /// 가독을 위해 기본 스타일 그대로다(v0.5, 팀컬러 fg 사용 안 함).
    #[test]
    fn favorite_team_gets_cheer_badge() {
        let mut app = App::new(Default::default());
        app.fav_code = Some("LG".into());
        let text = render_to_string(&app);
        assert!(text.contains("GO!"), "cheer badge missing:\n{text}");
        let mut term = Terminal::new(TestBackend::new(80, 4)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();
        let team_bg = crate::ui::theme::team_color("LG");
        assert!(
            buf.content().iter().any(|c| c.bg == team_bg),
            "cheer badge must render on team color background"
        );
    }

    /// fav 설정 여부와 무관하게 활성 탭·스피너는 named color(Cyan)/reverse만 쓴다
    /// (v0.5: 어두운 팀컬러 fg가 밝은 배경에서 안 보이던 문제 해소).
    #[test]
    fn active_tab_and_spinner_use_named_colors_when_fav_set() {
        let mut app = App::new(Default::default());
        app.fav_code = Some("HH".into());
        app.fetching = true;
        app.spinner_frame = 1; // '/'
        let mut term = Terminal::new(TestBackend::new(80, 4)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();
        assert!(
            buf.content().iter().any(|c| c.fg == Color::Cyan),
            "spinner must use Cyan regardless of fav"
        );
        // 팀컬러(HH=주황)가 fg로 새어나가지 않아야 한다 — 배지 fg(contrast_fg)만 예외.
        let team_fg_leak = buf.content().iter().any(|c| {
            c.fg == crate::ui::theme::team_color("HH") && c.bg != crate::ui::theme::team_color("HH")
        });
        assert!(
            !team_fg_leak,
            "team color must not be used as bare fg outside the badge"
        );
    }

    #[test]
    fn no_favorite_team_no_cheer_badge() {
        let app = App::new(Default::default());
        assert_eq!(app.fav_code, None);
        let text = render_to_string(&app);
        assert!(!text.contains("GO!"));
    }

    #[test]
    fn korean_labels_render_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        let text = render_to_string(&app);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("[경기]"),
            "active tab must be Korean:\n{text}"
        );
        assert!(compact.contains("중계0")); // count_live
    }
}
