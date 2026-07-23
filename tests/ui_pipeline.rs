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
