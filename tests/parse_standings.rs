use kbotop::source::naver::map::standings_from_json;

const STANDINGS: &str = include_str!("fixtures/standings_2026.json");

#[test]
fn parses_ten_teams_ranked() {
    let s = standings_from_json(STANDINGS).unwrap();
    assert_eq!(s.len(), 10);
    assert_eq!(s[0].rank, 1);
    assert_eq!(s[0].team.name, "삼성");
    // 순위가 오름차순 정렬 (fixture는 배열 순서가 랭킹 역순이라, sort_by_key가
    // 실제로 동작해야만 이 assertion이 통과한다)
    assert!(s.windows(2).all(|w| w[0].rank <= w[1].rank));
}

/// 네이버 응답은 wcRanking/division 같은 형제 필드를 실제로 null로 내려주는
/// 게 확인된다 — ranking/gameBehind 등 숫자 필드에 명시적 null이 와도 그
/// 값만 기본값으로 낮아지고, 전체 standings 응답이 통째로 실패하면 안 된다.
#[test]
fn explicit_null_on_numeric_fields_degrades_instead_of_failing_whole_response() {
    let json = r#"{"result":{"seasonTeamStats":[
        {"teamId":"SS","teamName":"Samsung","ranking":null,"gameCount":10,"winGameCount":6,"loseGameCount":4,"drawnGameCount":0,"wra":0.6,"gameBehind":null}
    ]}}"#;
    let s = standings_from_json(json).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].rank, 0); // null → u16 기본값
    assert_eq!(s[0].game_behind, 0.0); // null → f32 기본값
    assert_eq!(s[0].wins, 6); // 다른 필드는 정상 파싱
}

/// team_id/team_name(String)에 명시적 null이 와도, 숫자 형제 필드들과 동일하게
/// 그 값만 기본값("")으로 낮아지고 팀 로우 전체/응답 전체가 실패하면 안 된다.
#[test]
fn explicit_null_on_team_id_and_name_degrades_instead_of_failing_whole_response() {
    let json = r#"{"result":{"seasonTeamStats":[
        {"teamId":null,"teamName":null,"ranking":1,"gameCount":10,"winGameCount":6,"loseGameCount":4,"drawnGameCount":0,"wra":0.6,"gameBehind":0.0}
    ]}}"#;
    let s = standings_from_json(json).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].team.code, ""); // null → String 기본값
    assert_eq!(s[0].team.name, ""); // null → String 기본값
    assert_eq!(s[0].rank, 1); // 다른 필드는 정상 파싱
}

/// 정수 필드(ranking 등)가 1.0/3e0처럼 소수점·지수 표기의 "숫자로는 유효한
/// 정수값"으로 와도, as_i64()만 쓰면 None이 되어 기본값(0)으로 조용히
/// 뭉개진다 — as_f64() 폴백으로 실제 값을 살려내야 한다.
#[test]
fn decimal_looking_integer_field_is_not_silently_dropped_to_default() {
    let json = r#"{"result":{"seasonTeamStats":[
        {"teamId":"SS","teamName":"S","ranking":1.0,"gameCount":10,"winGameCount":6,"loseGameCount":4,"drawnGameCount":0,"wra":0.6,"gameBehind":0.0}
    ]}}"#;
    let s = standings_from_json(json).unwrap();
    assert_eq!(s[0].rank, 1); // 1.0 → 1 (as_i64() 실패 시 as_f64() 폴백으로 살아나야 함)
}
