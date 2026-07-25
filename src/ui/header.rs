use super::i18n::Labels;
use super::theme::{self, team_badge_style};
use crate::app::{App, Tab};
use crate::model::GameStatus;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// UTC epoch 초 → "HH:MM ABBR"(예 `18:26 KST`, `05:26 EDT`). 시간대는
/// `localtime::resolve`가 프로세스 시작 시 한 번 정한 값을 받는다 — 보는
/// 사람이 서울에 있든 뉴욕에 있든 **자기 시계와 맞는 시각**을 봐야 한다
/// (v0.16). 경기일 판단(`dateutil::kst_days`)은 KST 고정이라 별개다.
/// 순수 산술이라 패닉 불가(음수 오프셋은 rem_euclid로 흡수).
fn current_time_label(now_secs: u64, tz: &crate::localtime::TimeZone) -> String {
    let local = (now_secs as i64).wrapping_add(tz.offset_secs as i64);
    let secs_of_day = local.rem_euclid(86400);
    let h = secs_of_day / 3600;
    let m = (secs_of_day % 3600) / 60;
    format!("{h:02}:{m:02} {}", tz.abbrev)
}

/// 마지막 성공 갱신(app.last_update_secs) 이후 경과 → "12초 전"/"1분 전"류
/// (v0.15 A-2). now < last(시계 역행 등 방어)는 saturating_sub로 0에 멈춘다 —
/// 무패닉 원칙, 음수 표기 없음. 시간 단위는 만들지 않는다(그쯤이면 stale
/// 배지가 이미 말해준다 — 설계 §2 A-2).
fn update_age_label(l: &Labels, now_secs: u64, last_secs: u64) -> String {
    let elapsed = now_secs.saturating_sub(last_secs);
    if elapsed < 60 {
        format!("{elapsed}{}", l.updated_secs_suffix)
    } else {
        format!("{}{}", elapsed / 60, l.updated_min_suffix)
    }
}

/// Span 슬라이스의 표시폭 합(display_width 휴리스틱). 우측 정렬 패딩·폭
/// 예산 판정에 쓴다 — Span.content(Cow<str>)는 ratatui-core에서 공개 필드다.
fn spans_width(spans: &[Span]) -> usize {
    spans
        .iter()
        .map(|s| super::text::display_width(&s.content))
        .sum()
}

/// htop의 CPU/Mem 게이지 자리에 해당하는 2줄 요약 헤더.
/// 1행: 상태별 경기 수(+ 우측 정렬 현재 시각, 폭 부족 시 생략).
/// 2행: 탭 표시(+ 마지막 갱신 경과, stale 마커 — 둘 다 폭 부족 시 생략 가능하나
/// stale은 기존 관례대로 무조건 표시한다).
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
            theme::status_fg(&app.theme_preset, Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} {sched}", l.count_sched),
            theme::status_fg(&app.theme_preset, Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} {fin}", l.count_final),
            theme::status_fg(&app.theme_preset, Color::Green),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} {other}", l.count_other),
            theme::status_fg(&app.theme_preset, Color::Magenta),
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

    // A-1: 우측 정렬 현재 시각. 테두리(좌우 1칸씩)를 뺀 내부 폭에 "기존 내용 +
    // 최소 여백(2칸) + 시계"가 다 들어갈 때만 붙인다 — 좁은 터미널에서는
    // 시간 정보부터 조용히 빠진다(기존 counts/배지는 절대 안 건드림, §2 폭 예산).
    let inner_width = area.width.saturating_sub(2) as usize;
    const CLOCK_GAP: usize = 2;
    let clock = current_time_label(app.now_secs, &app.tz);
    let clock_w = super::text::display_width(&clock);
    let counts_w = spans_width(&counts_spans);
    if inner_width >= counts_w + CLOCK_GAP + clock_w {
        let pad = inner_width - counts_w - clock_w;
        counts_spans.push(Span::raw(" ".repeat(pad)));
        counts_spans.push(Span::styled(
            clock,
            theme::status_fg(&app.theme_preset, Color::Gray),
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
            theme::status_fg(&app.theme_preset, Color::Cyan),
        ));
    }
    // A-2: 마지막 성공 갱신 경과 — stale 배지 자리 근처, stale 여부와 무관하게
    // 항상 시도한다("폴링 주기보다 크게 벌어지면 자연히 눈에 띈다" — §2 A-2).
    // 폭이 부족하면 조용히 생략한다(stale 배지 자체는 기존처럼 무조건 표시 —
    // 시간 정보가 먼저 빠지는 게 원칙, §2 폭 예산). stale이 뒤이어 무조건
    // 붙으므로, age를 넣을지 판단할 때 stale이 필요로 할 폭까지 미리 예약해
    // 둔다 — 안 그러면 "age는 딱 들어가지만 그 다음 stale이 밀려 잘리는"
    // 좁은 밴드가 생긴다(핵심 정보가 부가 정보에 밀리면 안 된다는 원칙 위반).
    if let Some(last) = app.last_update_secs {
        let age = update_age_label(l, app.now_secs, last);
        let addition = 3 + super::text::display_width(&age); // "   " 구분자 + 텍스트
        let stale_reserve = if app.stale {
            3 + super::text::display_width(l.stale)
        } else {
            0
        };
        if inner_width >= spans_width(&tab_spans) + addition + stale_reserve {
            tab_spans.push(Span::raw("   "));
            tab_spans.push(Span::styled(
                age,
                theme::status_fg(&app.theme_preset, Color::Gray),
            ));
        }
    }
    if app.stale {
        tab_spans.push(Span::raw("   "));
        tab_spans.push(Span::styled(
            l.stale,
            theme::status_fg(&app.theme_preset, Color::Red).add_modifier(Modifier::BOLD),
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

    fn render_to_string_with_width(app: &App, width: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, 4)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// A-1: 헤더 우측에 "HH:MM KST" 시계가 뜬다(초 단위 없음, 라벨로 KST 명시).
    #[test]
    fn header_shows_current_time_in_kst_when_wide_enough() {
        let mut app = App::new(Default::default());
        // 09:00:00 UTC + 9h = 18:00 KST.
        app.now_secs = 9 * 3600;
        let text = render_to_string(&app);
        assert!(text.contains("18:00 KST"), "clock missing:\n{text}");
    }

    /// 좁은 터미널에서는 시계가 조용히 빠진다(생략 우선순위: 시간 정보부터) —
    /// 기본 App(EN, fav 없음)의 counts 라인 원 폭은 33("LIVE 0  SCHED 0  FINAL 0
    /// OTHER 0"), 시계 추가분은 gap 2 + "HH:MM KST"(9) = 11이라 44부터 필요하다.
    /// 폭 40(내부 38)은 그 밑이라 시계는 빠지되 기존 카운트는 그대로 남는다.
    #[test]
    fn header_omits_clock_when_terminal_is_too_narrow() {
        let mut app = App::new(Default::default());
        app.now_secs = 9 * 3600;
        let text = render_to_string_with_width(&app, 40);
        assert!(!text.contains("KST"), "clock should be omitted:\n{text}");
        assert!(text.contains("LIVE"), "core counts must survive:\n{text}");
    }

    /// 위 좁은-폭 테스트의 대조군: 폭 50(내부 48)은 44 이상이라 시계가 뜬다.
    #[test]
    fn header_shows_clock_when_terminal_is_wide_enough() {
        let mut app = App::new(Default::default());
        app.now_secs = 9 * 3600;
        let text = render_to_string_with_width(&app, 50);
        assert!(text.contains("KST"), "clock should render:\n{text}");
    }

    /// A-2: 마지막 성공 갱신 후 12초가 지났으면 "12s ago"(EN 기본)가 뜬다.
    #[test]
    fn header_shows_seconds_ago_when_recently_updated() {
        let mut app = App::new(Default::default());
        app.now_secs = 100;
        app.last_update_secs = Some(88); // 12초 전
        let text = render_to_string(&app);
        assert!(text.contains("12s ago"), "seconds-ago missing:\n{text}");
    }

    /// 60초 이상 지나면 분 단위로 전환된다("2m ago", 125초 → 내림 2분).
    #[test]
    fn header_shows_minutes_ago_once_over_a_minute() {
        let mut app = App::new(Default::default());
        app.now_secs = 1000;
        app.last_update_secs = Some(1000 - 125);
        let text = render_to_string(&app);
        assert!(text.contains("2m ago"), "minutes-ago missing:\n{text}");
    }

    /// last_update_secs가 None이면(아직 성공 갱신 전) 아무 것도 뜨지 않는다(무패닉).
    #[test]
    fn header_omits_update_age_when_never_updated() {
        let app = App::new(Default::default());
        assert_eq!(app.last_update_secs, None);
        let text = render_to_string(&app);
        assert!(!text.contains("ago"));
    }

    /// 시계 역행 방어: last_update_secs가 now_secs보다 미래여도(시계 조정 등)
    /// saturating_sub로 0에 멈춰 "0s ago"를 보여줄 뿐 패닉하거나 음수를 내지 않는다.
    #[test]
    fn header_update_age_does_not_panic_when_last_is_after_now() {
        let mut app = App::new(Default::default());
        app.now_secs = 10;
        app.last_update_secs = Some(50);
        let text = render_to_string(&app);
        assert!(text.contains("0s ago"), "expected clamp to 0:\n{text}");
    }

    /// 리뷰용 회귀 방지: age 텍스트를 넣을지 판단할 때 뒤이어 무조건 붙는 stale
    /// 배지의 폭까지 미리 예약해 두지 않으면, "age는 딱 들어가지만 stale이
    /// 밀려 잘리는" 좁은 밴드가 생긴다 — 폭 40(내부 38)이 그 밴드다: age 단독
    /// 조건(baseline 26 + age 9 = 35)은 38 이하라 통과하지만, stale 예약(8)까지
    /// 더하면 43 > 38이라 age가 먼저 빠져야 stale이 안전하게 남는다.
    #[test]
    fn header_keeps_stale_badge_even_when_update_age_would_crowd_it_out() {
        let mut app = App::new(Default::default());
        app.stale = true;
        app.now_secs = 100;
        app.last_update_secs = Some(40); // 60초 전 → "1m ago"
        let text = render_to_string_with_width(&app, 40);
        assert!(text.contains("stale"), "stale badge missing:\n{text}");
        assert!(
            !text.contains("ago"),
            "age must yield to stale in this narrow band:\n{text}"
        );
    }

    /// 대조군: 폭이 둘 다 담을 만큼 넉넉하면(60, 내부 58 >= 43) age와 stale이
    /// 함께 뜬다 — 위 테스트가 age를 과도하게 억누르지 않는지 확인한다.
    #[test]
    fn header_shows_both_update_age_and_stale_when_width_allows() {
        let mut app = App::new(Default::default());
        app.stale = true;
        app.now_secs = 100;
        app.last_update_secs = Some(40);
        let text = render_to_string_with_width(&app, 60);
        assert!(text.contains("stale"), "stale badge missing:\n{text}");
        assert!(text.contains("ago"), "age text missing:\n{text}");
    }

    /// 한국어 완성형: "12초 전"/"1분 전"이 설계 예시와 정확히 일치한다.
    #[test]
    fn korean_update_age_matches_design_examples() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.now_secs = 100;
        app.last_update_secs = Some(88); // 12초 전
        let text = render_to_string(&app);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("12초전"),
            "ko seconds-ago missing:\n{text}"
        );

        app.now_secs = 1065;
        app.last_update_secs = Some(1000); // 65초 → 1분
        let text2 = render_to_string(&app);
        let compact2: String = text2.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact2.contains("1분전"),
            "ko minutes-ago missing:\n{text2}"
        );
    }
}
