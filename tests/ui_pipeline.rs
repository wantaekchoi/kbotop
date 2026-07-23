//! live→strikezone 파이프라인 stress: 실측 fixture를 앱 전체 렌더에 태워
//! 모든 투구가 범례에 완전히 렌더되는지(v0.1.2 최종 리뷰 Important#2).

use kbotop::app::{App, Screen};
use kbotop::model::{Game, GameStatus, Team};
use kbotop::source::naver::map;
use ratatui::{backend::TestBackend, Terminal};

fn team(code: &str, name: &str) -> Team {
    Team {
        code: code.into(),
        name: name.into(),
    }
}

#[test]
fn every_fixture_pitch_reaches_the_legend_through_the_full_app_render() {
    let relay = include_str!("fixtures/relay_20260719KTLG.json");
    let state = map::live_from_relay(relay, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert!(
        state.current_pitches.len() >= 5,
        "fixture must stress with 5+ pitches, got {}",
        state.current_pitches.len()
    );
    let expected: Vec<(u8, Option<u16>)> = state
        .current_pitches
        .iter()
        .map(|p| (p.order, p.speed_kmh))
        .collect();

    let mut app = App::new(Default::default());
    app.screen = Screen::Live {
        game: Game {
            id: "20260719KTLG02026".into(),
            start: "2026-07-19T18:00:00".into(),
            status: GameStatus::Final,
            status_label: String::new(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(state.home_score),
            away_score: Some(state.away_score),
        },
        state: Some(state),
    };

    // 실전 해상도(100x30, README 데모와 동일 오더)에서 전체 앱을 그린다.
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| kbotop::ui::draw(f, &app)).unwrap();
    let text: String = term
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();

    for (order, speed) in expected {
        match speed {
            Some(kmh) => assert!(
                compact.contains(&format!("{kmh}km")),
                "pitch {order} speed {kmh}km missing from legend"
            ),
            None => assert!(
                text.contains(&order.to_string()),
                "pitch {order} (no speed) missing entirely"
            ),
        }
    }
}

/// 한국어 전체 앱 렌더 통합: 80x24에서 패닉 없이 그려지고 핵심 한국어 chrome이
/// 전부 존재한다(완전성) — i18n 이관 누락(영어 잔존)도 함께 잡는다.
#[test]
fn korean_full_app_renders_all_chrome_at_80x24() {
    use kbotop::ui::i18n::Lang;
    let mut app = App::new(Default::default());
    app.lang = Lang::Ko;
    app.date = "2026-05-29".into();
    app.apply(kbotop::poller::Update::Games(vec![]));
    let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
    term.draw(|f| kbotop::ui::draw(f, &app)).unwrap();
    let text: String = term
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    for needle in [
        "[경기]",
        "순위",
        "중계0",
        "예정0",
        "종료0",
        "기타0",
        "경기2026-05-29",
        "도움말",
        "팁:",
    ] {
        assert!(
            compact.contains(needle),
            "{needle} missing in Korean render:\n{text}"
        );
    }
    // 영어 chrome 잔존 없음(데이터·보존 표기 제외) — 대표 잔존 후보 검사.
    for stale in ["GAMES", "STANDINGS", "Help", "Tip:", "loading"] {
        assert!(!text.contains(stale), "English chrome leaked: {stale}");
    }
}
