use kbotop::error::Error;
use kbotop::model::Team;
use kbotop::source::naver::map::live_from_relay;

const RELAY: &str = include_str!("fixtures/relay_20260719KTLG.json");

fn team(c: &str, n: &str) -> Team {
    Team {
        code: c.into(),
        name: n.into(),
    }
}

#[test]
fn parses_current_count_and_score() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.count.ball, 2);
    assert_eq!(live.count.strike, 3);
    assert_eq!(live.count.out, 3);
    assert_eq!(live.home_score, 1);
    assert_eq!(live.away_score, 4);
}

#[test]
fn extracts_pitches_with_plate_coords() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert!(!live.current_pitches.is_empty());
    let p = &live.current_pitches[0];
    // fixture 실측값: crossPlateX=-0.378877, crossPlateY=0.7083, topSz=3.38, bottomSz=1.639
    assert!((p.plate_x - (-0.378877)).abs() < 1e-4);
    assert!(p.sz_top > p.sz_bottom);
}

/// crossPlateY(~0.7083ft)는 모든 투구에 걸쳐 동일한 "플레이트까지의 y거리"
/// 상수일 뿐 높이가 아니다 — plate_y에 그대로 넣으면 스트존에서 모든 공이
/// 같은 줄에 찍힌다(회귀). 투사체 운동으로 계산한 실제 높이는 투구마다
/// 달라야 하고, 대부분 사람 키 범위(대략 0~6ft)에 들어와야 한다.
#[test]
fn computed_plate_y_varies_across_pitches_and_is_mostly_in_a_plausible_height_band() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    let heights: Vec<f32> = live.current_pitches.iter().map(|p| p.plate_y).collect();
    assert!(heights.len() >= 2, "need multiple pitches to compare");

    // crossPlateY(0.7083)로 죄다 뭉치는 옛 회귀와 달리 서로 달라야 한다.
    let first = heights[0];
    assert!(
        heights.iter().any(|h| (h - first).abs() > 1e-3),
        "plate_y must vary across pitches, got: {heights:?}"
    );
    // 옛 버그의 상수값(crossPlateY)에 그대로 고정돼있지 않은지도 확인한다.
    assert!(
        heights.iter().any(|h| (h - 0.7083).abs() > 1e-3),
        "plate_y must not collapse to the old crossPlateY constant, got: {heights:?}"
    );

    // 공이 그라운드에 닿고 넘어가는 등 물리 모델이 실측 트래킹을 벗어나는
    // 극단값(예: 땅볼성 변화구)이 섞일 수 있으므로 "대부분"만 요구한다.
    let plausible = heights.iter().filter(|h| (0.0..6.0).contains(*h)).count();
    assert!(
        plausible * 2 >= heights.len(),
        "expected most plate_y values within a plausible height band (0.0..6.0ft), got: {heights:?}"
    );
}

#[test]
fn builds_relay_log_lines() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert!(live
        .relay_log
        .iter()
        .any(|l| l.contains("파울") || l.contains("헛스윙") || l.contains("볼")));
}

#[test]
fn computes_pitch_speed_from_velocity_vector() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    // first pitch of the current at-bat has a real (non-zero) release velocity vector
    assert_eq!(live.current_pitches[0].speed_kmh, Some(134));
}

/// currentGameState의 문자열 필드(ball 등)가 명시적 null이어도 그 값만
/// 기본값("")으로 낮아지고, 5s마다 도는 라이브 폴링 전체가 실패하면 안 된다.
#[test]
fn explicit_null_on_a_current_game_state_field_degrades_instead_of_failing() {
    let json = r#"{"result":{"textRelayData":{
        "currentGameState": {"ball": null, "strike": "2", "out": "1", "homeScore": "3", "awayScore": "4"},
        "textRelays": [],
        "lastValidMetricOption": null
    }}}"#;
    let live = live_from_relay(json, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.count.ball, 0); // null → "" → parse_u8 기본값 0
    assert_eq!(live.count.strike, 2);
    assert_eq!(live.home_score, 3);
}

/// textRelays 항목의 inn(TextRelay)과 seqno/text/type(TextOption)에 명시적
/// null이 와도 그 필드만 기본값으로 낮아지고, 5s마다 도는 라이브 폴링 전체가
/// 파싱 에러로 실패하면 안 된다.
#[test]
fn explicit_null_inside_a_text_relay_entry_degrades_instead_of_failing() {
    let json = r#"{"result":{"textRelayData":{
        "currentGameState": {},
        "textRelays": [{"inn": null, "textOptions": [{"seqno": null, "text": null, "type": null}], "ptsOptions": []}]
    }}}"#;
    let live = live_from_relay(json, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.inning_label, "Inn 0"); // null → i32 기본값 0, homeOrAway도 null → "" → 알 수 없는 절반이닝
}

/// name_of()가 currentGameState.pitcher/batter의 선수 id를 away/homeLineup에서
/// 실제로 찾아 이름으로 바꾸는지 확인한다(fixture 실측: pitcher=52060은
/// awayLineup 소속 박영현, batter=50054는 homeLineup 소속 천성호 — 이 함수
/// 본문 전체를 `String::new()`로 바꿔도 이 테스트 전에는 전체 스위트가
/// 그대로 통과했다).
#[test]
fn resolves_pitcher_and_batter_names_from_lineups() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.pitcher_name, "박영현");
    assert_eq!(live.batter_name, "천성호");
}

/// currentGameState.pitcher/batter가 null(→"")이고, 라인업의 한 선수 항목도
/// pcode:null(→"")이면, 둘 다 "id 없음"이 아니라 "진짜 pcode 없는 선수"의
/// 빈 문자열과 뒤섞여 그 선수를 현재 투수/타자로 잘못 반환할 수 있다 — 매칭
/// 자체를 시도하지 않고 빈 문자열로 남아야 한다.
#[test]
fn empty_pitcher_and_batter_ids_do_not_match_a_player_with_a_null_pcode() {
    let json = r#"{"result":{"textRelayData":{
        "currentGameState": {"pitcher": null, "batter": null},
        "homeLineup": {"batter": [{"pcode": null, "name": "MysteryPlayer"}], "pitcher": []},
        "awayLineup": null,
        "textRelays": []
    }}}"#;
    let live = live_from_relay(json, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.pitcher_name, "");
    assert_eq!(live.batter_name, "");
}

/// ptsOptions 중 하나의 ballcount가 u8 범위를 벗어나는 값(-1)이어도 그
/// 필드만 기본값(0)으로 완화되고, live_from_relay 전체가 Err가 되어 5초
/// 주기 라이브 폴링이 통째로 stale 처리되면 안 된다.
#[test]
fn out_of_range_ballcount_degrades_instead_of_failing_whole_relay() {
    let json = r#"{"result":{"textRelayData":{
        "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
        "textRelays": [
            {"inn": 9, "textOptions": [], "ptsOptions": [
                {"ballcount": -1, "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
            ]}
        ]
    }}}"#;
    let live = live_from_relay(json, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.current_pitches.len(), 1);
    assert_eq!(live.current_pitches[0].order, 0); // 범위 밖(-1) 값 → u8 기본값 0
}

/// currentGameState.ball처럼 문자열이어야 할 필드가 타입이 안 맞는 값(JSON
/// 숫자)으로 와도 null_as_default처럼 전체 relay 파싱을 Err로 실패시키지
/// 않고 그 필드만 기본값("")으로 완화되어야 한다 — 라운드 5에서 ScheduleGame에만
/// 적용됐던 lenient_string 전환을 CurrentGameState 등에도 확장한 회귀 테스트.
#[test]
fn type_mismatched_current_game_state_field_degrades_instead_of_failing_whole_relay() {
    let json = r#"{"result":{"textRelayData":{
        "currentGameState": {"ball": 2, "strike": "1", "out": "0", "homeScore": "0", "awayScore": "0"},
        "textRelays": [],
        "lastValidMetricOption": null
    }}}"#;
    let live = live_from_relay(json, team("LG", "LG"), team("KT", "KT")).unwrap();
    assert_eq!(live.count.ball, 0); // JSON 숫자(2) → 타입 불일치 → "" → parse_u8 기본값 0
    assert_eq!(live.count.strike, 1);
}

#[test]
fn normalizes_win_rate_to_0_1_fraction() {
    let live = live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap();
    // fixture 실측값(lastValidMetricOption): homeTeamWinRate=0.0, awayTeamWinRate=100.0 (0~100 퍼센트 스케일)
    // → LiveState에는 0~1 소수로 정규화되어 저장돼야 한다 (UI가 ×100 해서 표시하므로).
    let home = live.home_win_rate.unwrap();
    let away = live.away_win_rate.unwrap();
    assert!(
        (0.0..=1.0).contains(&home),
        "home_win_rate out of range: {home}"
    );
    assert!(
        (0.0..=1.0).contains(&away),
        "away_win_rate out of range: {away}"
    );
    assert!((home - 0.0).abs() < 1e-9);
    assert!((away - 1.0).abs() < 1e-9);
    // 승부가 결정된(0/100) 픽스처이므로 합이 1.0에 근접해야 한다.
    assert!((home + away - 1.0).abs() < 1e-9);
}

/// Canceled/Scheduled 경기처럼 relay가 textRelayData 자체를 내려주지 않는
/// 응답에서 live_from_relay가 Err를 반환해야 하고, 그 에러가 `Error::Data`여야
/// 한다 — 라운드 4에서 `Error::Config`를 `Error::Data`로 바꾼 건 footer가 이
/// 실패를 "config error: ..."로 잘못 표시하지 않게 하기 위함이었는데(설정
/// 파일과 무관한 실패다), 그 회귀를 막는 테스트가 없었다(라운드 5).
#[test]
fn missing_text_relay_data_is_a_data_error_not_a_config_error() {
    let json = r#"{"result":{}}"#;
    let err = live_from_relay(json, team("LG", "LG"), team("KT", "KT")).unwrap_err();
    assert!(
        matches!(err, Error::Data(_)),
        "expected Error::Data, got: {err:?}"
    );
    assert!(
        !err.to_string().starts_with("config error:"),
        "unexpected config-error framing: {err}"
    );
}
