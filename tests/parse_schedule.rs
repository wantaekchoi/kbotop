use kbotop::model::GameStatus;
use kbotop::source::naver::map::games_from_schedule;

const SCHEDULE: &str = include_str!("fixtures/schedule_20260719.json");

#[test]
fn parses_all_games() {
    let games = games_from_schedule(SCHEDULE).unwrap();
    assert_eq!(games.len(), 5);
}

#[test]
fn maps_final_game_scores_and_names() {
    let games = games_from_schedule(SCHEDULE).unwrap();
    let ktlg = games.iter().find(|g| g.id == "20260719KTLG02026").unwrap();
    assert_eq!(ktlg.status, GameStatus::Final);
    assert_eq!(ktlg.home.name, "LG");
    assert_eq!(ktlg.away.name, "KT");
    assert_eq!(ktlg.home_score, Some(1));
    assert_eq!(ktlg.away_score, Some(4));
}

/// gameDateTime/statusCode/statusInfo/cancel/suspended(non-Option 필드)에 명시적
/// null이 와도 그 필드만 기본값으로 낮아지고, 같은 배열의 다른(정상) 경기까지
/// 함께 날아가면 안 된다.
#[test]
fn explicit_null_on_a_sibling_game_field_degrades_instead_of_failing_whole_day() {
    let json = r#"{"result":{"games":[
        {"gameId":"g1","homeTeamCode":"LG","awayTeamCode":"KT","gameDateTime":null,"statusCode":null,"statusInfo":null,"cancel":null,"suspended":null},
        {"gameId":"g2","homeTeamCode":"HT","awayTeamCode":"OB","statusCode":"RESULT","statusInfo":"경기종료"}
    ]}}"#;
    let games = games_from_schedule(json).unwrap();
    assert_eq!(games.len(), 2);
    let g1 = games.iter().find(|g| g.id == "g1").unwrap();
    assert_eq!(g1.start, ""); // null → String 기본값
    assert_eq!(g1.status, GameStatus::Scheduled); // statusCode null → "" → Scheduled
    assert_eq!(g1.status_label, ""); // null → String 기본값
    let g2 = games.iter().find(|g| g.id == "g2").unwrap();
    assert_eq!(g2.status, GameStatus::Final); // 정상 필드를 가진 형제 레코드는 그대로 파싱
}

/// homeTeamScore가 u16 범위를 벗어나는 값(-1)이어도 null_as_default로는 막을
/// 수 없는 "값은 있지만 타입 범위가 안 맞는" 케이스다 — 이 필드만 None으로
/// 완화되고, 같은 배열의 다른 정상 경기가 배열 전체 파싱 실패로 함께
/// 사라지면 안 된다.
#[test]
fn out_of_range_numeric_field_on_one_game_degrades_instead_of_failing_whole_day() {
    let json = r#"{"result":{"games":[
        {"gameId":"g1","homeTeamCode":"LG","awayTeamCode":"KT","homeTeamScore":-1,"statusCode":"RESULT"},
        {"gameId":"g2","homeTeamCode":"HT","awayTeamCode":"OB","awayTeamScore":3,"statusCode":"RESULT"}
    ]}}"#;
    let games = games_from_schedule(json).unwrap();
    assert_eq!(games.len(), 2);
    let g1 = games.iter().find(|g| g.id == "g1").unwrap();
    assert_eq!(g1.home_score, None); // 범위 밖(-1) 값 → None으로 완화
    let g2 = games.iter().find(|g| g.id == "g2").unwrap();
    assert_eq!(g2.away_score, Some(3)); // 형제 레코드의 정상 값은 그대로 파싱
}
