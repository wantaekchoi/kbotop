use super::live_vm::LiveVm;
use super::strikezone;
use super::theme::team_badge_style;
use crate::app::{App, Screen};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
    Frame,
};

/// 라이브 뷰: 스코어라인(점수/카운트/주자/승률) + 문자중계(+ 폭 충분 시 스트라이크존).
///
/// **여기부터 아래는 그리기만 한다.** 무엇을 보여줄지(활성 at-bat 해석, 타이틀,
/// 돌려보기 중 감출 필드, 라벨 문자열, 폭 예산 정책)는 전부
/// [`super::live_vm::LiveVm`]이 정한다 — 이 파일은 그 결과를 위젯으로 옮길 뿐이라
/// 새 규칙을 여기에 적어 넣을 자리가 없다(v0.18에 같은 규칙이 두 군데로 흩어진
/// 사고의 재발 방지).
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if !matches!(app.screen, Screen::Live { .. }) {
        return;
    }
    // 표현 상태를 못 만드는 경우 = 아직 상태가 안 왔다(로딩) — vm이 없으므로
    // 라벨은 이 경로에서만 직접 조회한다(M-7: 성공 경로는 아래 vm.labels를
    // 재사용해 app.labels()를 두 번 안 부른다).
    let Some(vm) = LiveVm::from_app(app) else {
        let l = app.labels();
        f.render_widget(
            Paragraph::new(l.loading).block(Block::bordered().title(l.title_live)),
            area,
        );
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    render_scoreline(f, rows[0], &vm);

    // 폭이 좁거나 아직 투구 데이터가 없으면 존을 숨기고 중계에 본문 전체를 준다(우아한 저하).
    if vm.show_strike_zone(rows[1].width) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        render_relay(f, cols[0], &vm);
        strikezone::render(
            f,
            cols[1],
            vm.pitches,
            vm.selected_pitch,
            vm.labels,
            &app.theme_preset,
        );
    } else {
        render_relay(f, rows[1], &vm);
    }
}

/// 스코어라인 3줄(점수·상태배지·지금-이-순간 값 / 상세 / 투구줄)을 그린다.
/// 폭은 이 함수만 아는 사실이므로 VM의 폭 인자 메서드에 그대로 넘긴다.
fn render_scoreline(f: &mut Frame, area: Rect, vm: &LiveVm) {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut spans = vec![
        Span::styled(vm.away.name.as_str(), team_badge_style(&vm.away.code)),
        Span::raw(" "),
        Span::styled(vm.away_score.to_string(), bold),
        Span::raw(" : "),
        Span::styled(vm.home_score.to_string(), bold),
        Span::raw(" "),
        Span::styled(vm.home.name.as_str(), team_badge_style(&vm.home.code)),
        Span::raw("   "),
        Span::raw(vm.inning_label),
    ];
    // 서스펜디드/종료 경기는 스코어라인만 봐서는 진행 중인 경기와 구분이
    // 안 된다 — inning_label 옆에 배지를 붙여 우아하게 저하시킨다.
    if let Some((label, style)) = vm.status_badge {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label, style));
    }
    // None이면 돌려보기 중이라 "지금 이 순간" 값들이 통째로 빠진 것이다(판단은 VM).
    if let Some(now) = &vm.now_fields {
        spans.extend([
            Span::raw("   "),
            Span::raw(now.count.as_str()),
            Span::raw("   "),
            Span::raw(now.bases.as_str()),
            Span::raw("   "),
            Span::raw(now.win_pct.as_str()),
        ]);
    }

    let inner_width = area.width.saturating_sub(2) as usize;
    f.render_widget(
        Paragraph::new(vec![
            Line::from(spans),
            Line::from(vm.detail_line(inner_width)),
            Line::from(vm.pitch_line.text(inner_width)),
        ])
        .block(Block::bordered().title(vm.title.as_str())),
        area,
    );
}

/// 문자중계 목록. 커서가 None이면(기본 상태, 기존 무회귀) 꼬리(N줄)만
/// 하이라이트 없이 보여준다 — 오래된→최신 순 저장이라 이렇게 자르면 최신이
/// 리스트 맨 아래에 온다. Some(i)면(v0.18 돌려보기 j/k 커서) 전체 줄을
/// ListState 기반 스테이트풀 리스트로 그려 i번째를 반전 하이라이트 하고,
/// ratatui가 그 줄이 보이도록 자동으로 스크롤한다(settings.rs·newslist.rs와
/// 같은 ListState 관용).
///
/// `vm.relay_rows`는 이미 화면에 낼 최종 문자열(불릿 포함)이고 `vm.relay_cursor`는
/// 이미 범위 안으로 클램프돼 있다(v19a 리뷰 I-1) — 이 함수는 그 두 값을 그대로
/// 옮길 뿐, 줄의 표현이나 범위 밖 커서에 대한 결정을 하지 않는다.
///
/// 꼬리 창(`area.height - 2`) 계산이 여기 남은 이유: 이건 "무엇을 보여줄까"가
/// 아니라 커서 분기에서 ratatui ListState가 이미 해 주는 **뷰포트 산수**의
/// 짝이다. VM으로 올리면 위젯이 내부적으로 하는 일을 밖에서 한 번 더 흉내 내는
/// 중복이 된다.
fn render_relay(f: &mut Frame, area: Rect, vm: &LiveVm) {
    let (rows, title) = (&vm.relay_rows, vm.relay_title);
    match vm.relay_cursor {
        Some(idx) => {
            let items: Vec<ListItem> = rows.iter().map(|row| ListItem::new(row.clone())).collect();
            let widget = List::new(items)
                .block(Block::bordered().title(title))
                .highlight_symbol("> ")
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            let mut state = ListState::default();
            state.select(Some(idx));
            f.render_stateful_widget(widget, area, &mut state);
        }
        None => {
            let n = area.height.saturating_sub(2) as usize;
            let start = rows.len().saturating_sub(n);
            let items: Vec<ListItem> = rows[start..]
                .iter()
                .map(|row| ListItem::new(row.clone()))
                .collect();
            f.render_widget(List::new(items).block(Block::bordered().title(title)), area);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Screen};
    use crate::model::{BaseState, Count, Game, GameStatus, LiveState, Pitch, Team};
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

    /// fixture 대신 손으로 조립한 최소 상태(B-3 폭 예산 테스트 전용) — 이름
    /// 길이를 정확히 통제해야 "딱 안 들어가는 폭"을 결정적으로 고를 수 있다.
    /// `at_bats`를 비우고 `current_pitches`에 마지막 투구 하나만 담아
    /// `latest_pitch_time`의 무회귀 폴백 경로도 함께 검증한다.
    fn bare_live_screen(status: GameStatus, game_start: &str, end_hms: &str) -> Screen {
        let state = LiveState {
            inning_label: "T9".into(),
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
            pitcher_name: "Kim".into(),
            batter_name: "Lee".into(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![Pitch {
                time_hms: Some(end_hms.into()),
                ..Default::default()
            }],
            next_batter_name: String::new(),
            at_bats: vec![],
        };
        let game = Game {
            id: "g".into(),
            start: game_start.into(),
            status,
            status_label: "".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(3),
            away_score: Some(2),
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
            // 렌더는 active_pitches(v0.18)를 통해 at_bats의 마지막 항목을 읽으므로
            // (current_pitches는 무회귀용 미러) 여기도 함께 갱신해야 반영된다.
            s.at_bats.last_mut().unwrap().pitches[0].time_hms = Some("20:56:14".into());
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

    /// 긴 결과 원문은 상세줄에서 말줄임된다(§15 오버플로 정책).
    #[test]
    fn long_pitch_text_is_ellipsized_in_the_detail_line() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            let long_text = "매우 긴 결과 설명 ".repeat(20);
            s.current_pitches[0].text = long_text.clone();
            // active_pitches(v0.18)가 at_bats의 마지막 항목을 읽으므로 함께 갱신.
            s.at_bats.last_mut().unwrap().pitches[0].text = long_text;
        }
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 80, 30);
        assert!(
            text.contains('…'),
            "expected honest ellipsis in detail line"
        );
    }

    #[test]
    fn korean_live_labels_render_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.screen = live_screen();
        let text = render_live_view_only(&app, 100, 30);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("문자중계"),
            "relay title must be Korean:\n{text}"
        );
        assert!(compact.contains("투수:") && compact.contains("타자:"));
        assert!(text.contains("좌우 키로 하나씩") || compact.contains("투구"));
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

    /// v0.18 돌려보기: 과거 타석을 고르면 타이틀에 그 타자·이닝이 드러나고,
    /// 기본 라이브 타이틀(" Live ")은 더는 보이지 않는다 — 라이브와 헷갈리면
    /// 안 된다는 제약의 직접 검증. fixture 실측: 가장 오래된 타석(index 0)은
    /// 최원준.
    #[test]
    fn selecting_a_past_at_bat_shows_its_batter_and_inning_instead_of_the_live_title() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let live_text = render_live_view_only(&app, 100, 30);
        assert!(
            live_text.contains(" Live "),
            "default view keeps the live title"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석의 textRelay no
        let past_text = render_live_view_only(&app, 100, 30);
        let compact: String = past_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("최원준"),
            "past at-bat's batter must appear in the title:\n{past_text}"
        );
        assert!(
            compact.contains("Rewind"),
            "must be explicitly marked as rewinding:\n{past_text}"
        );
        assert!(
            !past_text.contains(" Live "),
            "must not show the plain live title while rewinding:\n{past_text}"
        );
    }

    /// 돌려보기 중에는 스코어라인의 투수/타자 줄도 그 타석을 따라야 한다. 이 줄이
    /// 라이브 값으로 남아 있으면 타이틀은 과거 타자를, 바로 아랫줄은 현재 타자를
    /// 말해 한 화면이 서로 다른 두 타석을 가리킨다 — 실행 확인에서 잡힌 결함이라
    /// (타이틀·투구 수만 검증하던 기존 테스트는 통과했다) 여기서 봉인한다.
    #[test]
    fn rewinding_replaces_the_live_batter_line_instead_of_leaving_it_behind() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();

        let live_text = render_live_view_only(&app, 100, 30);
        let live_compact: String = live_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            live_compact.contains("천성호"),
            "전제: 라이브에서는 현재 타자가 보인다:\n{live_text}"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석
        let past_text = render_live_view_only(&app, 100, 30);
        let compact: String = past_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("최원준"),
            "보고 있는 타석의 타자가 있어야 한다:\n{past_text}"
        );
        assert!(
            !compact.contains("천성호"),
            "라이브 타자가 남으면 한 화면이 두 타자를 말한다:\n{past_text}"
        );
    }

    /// I-1: 되감기 중엔 스코어라인의 이닝도 활성 at-bat 것으로 바뀌어야 한다.
    /// 타이틀(rewind_title)은 이미 T9(최원준)를 말하는데 바로 아랫줄
    /// 스코어라인이 라이브 이닝(B9)을 그대로 두면 한 화면이 두 이닝을
    /// 동시에 말한다 — 4912944가 타자에 대해 고친 것과 정확히 같은 결함이
    /// 이닝 축에 남아 있었다(최종 리뷰 지적). 동시에 B/S/O·주자·WP는 "지금
    /// 이 순간"의 값이라 과거 타석 옆에 두면 그 타석의 카운트로 오해된다 —
    /// 이닝(아는 값)은 바꾸고, 카운트류(모르는 값)는 비운다.
    #[test]
    fn rewinding_replaces_the_scoreline_inning_and_hides_live_only_fields() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();

        let live_text = render_live_view_only(&app, 100, 30);
        assert!(
            live_text.contains("B9"),
            "전제: 라이브 이닝은 B9:\n{live_text}"
        );
        assert!(
            live_text.contains("B2 S3 O3"),
            "전제: 라이브에서는 카운트가 보인다:\n{live_text}"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석(T9)
        let past_text = render_live_view_only(&app, 100, 30);
        assert!(
            past_text.contains("T9"),
            "스코어라인 이닝이 활성 at-bat(T9)으로 바뀌어야 한다:\n{past_text}"
        );
        assert!(
            !past_text.contains("B9"),
            "라이브 이닝(B9)이 남아 있으면 한 화면이 두 이닝을 말한다:\n{past_text}"
        );
        assert!(
            !past_text.contains("B2 S3 O3"),
            "돌려보기 중엔 카운트를 비워야 한다(지금 이 순간 값이라 오해를 부른다):\n{past_text}"
        );
    }

    /// M-9: "과거 타석을 고르면 그 타석의 문자중계가 뜬다"를 직접 확인한다
    /// (기존엔 "라이브 타자 이름이 남아 있으면 실패"하는 식으로만 우연히
    /// 잡혔다 — active_relay_lines가 sel을 무시해도 안 걸릴 수 있는 검증).
    /// fixture 실측: 천성호(라이브, no=97)의 마지막 줄은 "포수 태그아웃"으로
    /// 끝나고, 최원준(과거, no=87)의 마지막 줄은 "볼넷"으로 끝난다 — 서로
    /// 배타적인 결과 문구라 어느 타석의 중계가 실제로 그려졌는지 직접 안다.
    #[test]
    fn selecting_a_past_at_bat_actually_swaps_in_that_at_bats_own_relay_lines() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();

        let live_text = render_live_view_only(&app, 100, 30);
        let live_compact: String = live_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            live_compact.contains("포수태그아웃"),
            "전제: 라이브 문자중계는 현재 타석(천성호) 것이다:\n{live_text}"
        );
        assert!(!live_compact.contains("볼넷"));

        app.live_atbat_sel = Some(87); // fixture 최원준 타석
        let past_text = render_live_view_only(&app, 100, 30);
        let compact: String = past_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("볼넷"),
            "과거 타석(최원준)의 문자중계가 실제로 그려져야 한다:\n{past_text}"
        );
        assert!(
            !compact.contains("포수태그아웃"),
            "라이브 문자중계가 남아 있으면 안 된다:\n{past_text}"
        );
    }

    /// 과거 타석을 보는 중엔 존/측면·투구 상세줄이 그 타석 자신의 투구를 써야
    /// 한다(현재 타석 것이 아니라) — fixture 실측: 최신(천성호)은 5구, 가장
    /// 오래된 타석(최원준)은 7구.
    #[test]
    fn selecting_a_past_at_bat_uses_that_at_bats_own_pitch_count() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let live_text = render_live_view_only(&app, 100, 30);
        assert!(
            live_text.contains("Pitches 5"),
            "live at-bat has 5 pitches:\n{live_text}"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석의 textRelay no
        let past_text = render_live_view_only(&app, 100, 30);
        assert!(
            past_text.contains("Pitches 7"),
            "past at-bat must report its own pitch count, not the live at-bat's:\n{past_text}"
        );
    }

    /// 문자중계 커서(v0.18 j/k)는 live_relay_cursor가 Some일 때만 하이라이트를
    /// 그린다 — 기본 상태(None)는 기존과 똑같이 하이라이트가 없어야 한다
    /// (기존 라이브 화면 무회귀).
    #[test]
    fn relay_cursor_highlight_only_appears_when_a_line_is_selected() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let default_text = render_live_view_only(&app, 100, 30);
        assert!(
            !default_text.contains("> ·"),
            "no cursor highlight in the default view:\n{default_text}"
        );

        app.live_relay_cursor = Some(0);
        let cursor_text = render_live_view_only(&app, 100, 30);
        assert!(
            cursor_text.contains("> ·"),
            "cursor highlight must appear once a relay line is selected:\n{cursor_text}"
        );
    }

    /// I-4: 되감기(문자중계 커서)의 존재 이유 자체를 증명한다 — 패널 높이보다
    /// 긴 문자중계에서 커서 없는 기본 뷰는 꼬리(최신) N줄만 보여주지만
    /// (무회귀), 커서가 화면 밖으로 스크롤된 오래된 줄을 가리키면 ratatui의
    /// 자동 스크롤로 그 줄이 실제로 렌더 결과에 나타나야 한다. 리뷰가
    /// 뮤테이션(Some(idx) 분기에도 꼬리 슬라이스를 되살림 — "커서는 있지만
    /// 되감아 볼 수는 없는" 상태)으로 보였듯, 커서 심볼 유무만 보는 위
    /// 테스트(relay_cursor_highlight_only_appears_when_a_line_is_selected)는
    /// 이 결함을 못 잡는다 — 이 테스트는 내용 자체를 직접 확인한다.
    #[test]
    fn moving_the_relay_cursor_to_an_old_line_reveals_it_past_the_default_tail_window() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        // fixture의 최신 at-bat 문자중계는 몇 줄뿐이라, 패널 높이보다 확실히
        // 긴 인위적인 relay_lines로 교체해 "꼬리만 보이는" 창을 만든다.
        let long_lines: Vec<String> = (0..30).map(|i| format!("line-{i}")).collect();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.at_bats.last_mut().unwrap().relay_lines = long_lines
                .into_iter()
                .map(crate::model::RelayLine::plain)
                .collect();
        }

        let default_text = render_live_view_only(&app, 100, 30);
        assert!(
            !default_text.contains("line-0"),
            "전제: 커서 없는 기본 뷰는 꼬리만 보여줘 가장 오래된 줄(line-0)이 \
             안 보여야 한다:\n{default_text}"
        );

        // 커서를 맨 위(가장 오래된 줄, 인덱스 0)로 옮긴다.
        app.live_relay_cursor = Some(0);
        let cursor_text = render_live_view_only(&app, 100, 30);
        assert!(
            cursor_text.contains("line-0"),
            "커서가 화면 밖 오래된 줄을 가리키면 그 줄이 실제로 보여야 한다 \
             (되감기 기능의 핵심 계약):\n{cursor_text}"
        );
    }

    /// 돌려보기 + 문자중계 커서 + 좁은 폭 조합이 패닉하지 않는다(무패닉 제약).
    ///
    /// M-1: seq는 인덱스가 아니라 응답의 textRelay `no`다 — fixture의 실제
    /// seq 범위는 86~98이라 `Some(0)`은 `active_at_bat`이 못 찾고 즉시
    /// 최신으로 낮춰(폴백) 실제로는 되감기 경로(rewind_title·과거 타자줄)를
    /// 전혀 안 타면서도 통과해 버렸다(이름과 달리 무의미한 테스트). fixture
    /// 최원준 타석의 실제 seq(87)를 써야 진짜로 그 경로를 태운다.
    #[test]
    fn rewind_view_with_relay_cursor_renders_without_panic_when_narrow() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        app.live_atbat_sel = Some(87);
        app.live_relay_cursor = Some(0);
        let _text = render_to_string(&app, 40, 15);
    }

    /// 첫/마지막 타석 경계에서도(sel이 범위를 벗어나도) 패닉 없이 clamp된
    /// 결과를 보여준다 — active_at_bat의 clamp가 렌더까지 이어지는지 확인.
    #[test]
    fn unknown_at_bat_selection_falls_back_to_live_instead_of_panicking() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        // 응답에 없는 번호(이닝이 넘어가 배열이 갈린 뒤 남은 stale 선택).
        app.live_atbat_sel = Some(9999);
        let text = render_live_view_only(&app, 100, 30);
        // 최신 타석으로 낮아지므로 라이브 타이틀 그대로 — 없는 타석을 있는 척
        // 그리지 않는다(App::apply가 폴링 때 선택 자체를 되돌린다).
        assert!(
            text.contains(" Live "),
            "unknown seq falls back to the latest at-bat:\n{text}"
        );
    }

    // ---- B-2: 투구 간격 (렌더 결합 — 순수 함수 검증은 live_vm.rs) ----

    /// 실제 fixture 간격(천성호 타석 1구→2구, 19초)이 화면에 "+19s"로 나온다.
    /// i>0(직전 투구가 있는 경우)만 간격을 붙인다는 계약의 통합 검증.
    #[test]
    fn pitch_interval_appears_for_the_second_pitch_using_real_fixture_gap() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        app.live_pitch_sel = Some(1); // 천성호 2구째(fixture 실측: 21:05:40→21:05:59, 19초)
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("+19s"), "expected pitch interval:\n{text}");
    }

    /// 첫 투구(i==0)는 직전이 없으므로 간격을 붙이지 않는다 — 붙였다면 i-1
    /// 인덱스가 언더플로해 패닉하거나(usize) 엉뚱한 값을 보여줬을 것이다.
    #[test]
    fn pitch_interval_is_absent_for_the_first_pitch() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains("+19s") && !text.contains("+18s"),
            "no interval expected for the first pitch:\n{text}"
        );
    }

    /// 직전 투구의 time_hms가 결측이면(관용 파싱) 간격을 생략한다 — 무패닉.
    #[test]
    fn pitch_interval_is_absent_when_previous_pitch_time_is_missing() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.current_pitches[0].time_hms = None;
            s.at_bats.last_mut().unwrap().pitches[0].time_hms = None;
        }
        app.live_pitch_sel = Some(1);
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains("+19s"),
            "interval must be omitted when the previous pitch's time is missing:\n{text}"
        );
    }

    // ---- B-3: 경기 소요/경과 (렌더 결합 — 순수 함수 검증은 live_vm.rs) ----

    /// 진행 중 경기: 스코어라인에 "Elapsed (+H:MM)"이 실제로 그려진다.
    #[test]
    fn live_view_shows_elapsed_time_for_a_live_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen(); // GameStatus::Live
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 41_400; // KST 20:30:00 → 시작 대비 +2:00
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            text.contains("Elapsed (+2:00)"),
            "expected live elapsed time:\n{text}"
        );
    }

    /// M-4 통합 검증: "지금"이 시작 10초 전인 진행 중 경기를 실제로 렌더해도
    /// "Elapsed (+23:59)" 같은 값이 화면에 나오지 않는다(Elapsed 자체가 생략).
    #[test]
    fn live_view_omits_elapsed_when_now_is_slightly_before_game_start() {
        let mut app = App::new(Default::default());
        app.screen = live_screen(); // GameStatus::Live
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 34_190; // KST 18:29:50 — 시작 10초 전(클록 스큐 실측 재현)
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains("Elapsed"),
            "must not show an implausible near-24h elapsed:\n{text}"
        );
    }

    /// ★핵심 회귀: 종료 경기를 "지금"과 무관한 값(어제 경기를 오늘 연 상황을
    /// 흉내낸 이른 벽시계 now_secs)으로 열어도 총 소요가 데이터 안의 마지막
    /// 투구 시각 기준으로 정확히 나오고, "지금까지" 계산이었다면 나왔을
    /// 20시간대 값이 아니다.
    #[test]
    fn live_view_shows_total_duration_for_a_finished_game_using_the_last_pitch_not_now() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 18_000; // KST 14:00:00 — "지금"을 썼다면 (+19:30) 버그
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            text.contains("Duration (+2:37)"),
            "expected total duration from the last recorded pitch:\n{text}"
        );
        assert!(
            !text.contains("(+19:30)"),
            "must not use 'now' for a finished game (yesterday's-game-shows-20h bug):\n{text}"
        );
    }

    /// 과거 타석을 돌려보는 중이어도(live_atbat_sel = Some(과거 seq)) 종료
    /// 경기의 소요는 그 타석이 아니라 항상 "경기 전체의 최신"을 기준으로
    /// 삼는다 — 값이 라이브일 때와 똑같아야 한다.
    #[test]
    fn game_duration_is_unaffected_by_viewing_a_past_at_bat() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 18_000;
        let latest_text = render_live_view_only(&app, 100, 30);
        assert!(latest_text.contains("Duration (+2:37)"));

        app.live_atbat_sel = Some(87); // fixture 최원준 타석(가장 오래된 타석)
        let past_text = render_live_view_only(&app, 100, 30);
        assert!(
            past_text.contains("Duration (+2:37)"),
            "duration must stay based on the latest at-bat while rewinding:\n{past_text}"
        );
    }

    /// 종료 경기인데 투구 데이터가 전혀 없으면(끝점 미상) 총 소요를 생략한다 —
    /// 기존 화면과 동일하게 아무것도 안 보여준다(무패닉·무회귀).
    #[test]
    fn duration_is_omitted_when_game_has_no_pitch_data_at_all() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        if let Screen::Live {
            game,
            state: Some(s),
        } = &mut app.screen
        {
            game.start = "2026-07-19T18:30:00".into();
            s.at_bats.clear();
            s.current_pitches.clear();
        }
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains(crate::ui::i18n::EN.lbl_duration),
            "duration must be omitted without an endpoint:\n{text}"
        );
    }

    /// 폭 예산: 좁은 터미널에선 소요/경과 정보가 먼저 빠지고(투수/타자 등
    /// 기존 정보는 그대로 남는다), 넉넉한 폭에선 나타난다.
    #[test]
    fn game_duration_is_dropped_first_when_the_area_is_too_narrow() {
        let mut app = App::new(Default::default());
        app.screen = bare_live_screen(GameStatus::Final, "2026-07-19T18:30:00", "21:07:06");

        let wide_text = render_live_view_only(&app, 60, 20);
        assert!(
            wide_text.contains("Duration (+2:37)"),
            "wide enough area must show duration:\n{wide_text}"
        );
        assert!(wide_text.contains("Kim") && wide_text.contains("Lee"));

        let narrow_text = render_live_view_only(&app, 20, 20);
        assert!(
            !narrow_text.contains("Duration"),
            "duration must be the first thing dropped in a narrow area:\n{narrow_text}"
        );
    }
}
