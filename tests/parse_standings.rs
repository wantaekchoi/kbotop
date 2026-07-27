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

/// v0.23: 최근 5경기·연속 기록이 파싱된다.
///
/// **오른쪽이 최신이다** — 10팀 전부에서 `continuousGameResult`와 대조해 확정했다.
/// 그리고 **무승부는 연속을 끊지 않고 건너뛴다**: NC `WWLLD`가 2패, SSG `WWWWD`가
/// 4승이다(KBO 관례). 그래서 연속 기록을 last_five에서 계산하지 않고 응답 값을
/// 그대로 쓴다 — 계산했다면 이 두 팀에서 틀렸을 것이다.
#[test]
fn parses_last_five_and_streak_including_the_draw_cases() {
    const STANDINGS: &str = include_str!("fixtures/standings_2026_streaks.json");
    let rows = standings_from_json(STANDINGS).unwrap();
    assert_eq!(rows.len(), 10);

    let by_name = |n: &str| {
        rows.iter()
            .find(|r| r.team.name == n)
            .expect("team")
            .clone()
    };

    let nc = by_name("NC");
    assert_eq!(nc.last_five, "WWLLD");
    assert_eq!(
        nc.streak, "2패",
        "마지막이 무승부여도 그 앞 연패가 살아 있다"
    );

    let ssg = by_name("SSG");
    assert_eq!(ssg.last_five, "WWWWD");
    assert_eq!(ssg.streak, "4승");

    // 마지막 글자가 곧 연속인 일반 케이스도 함께 고정한다.
    let hanwha = by_name("한화");
    assert_eq!(hanwha.last_five, "WWWLW");
    assert_eq!(hanwha.streak, "1승");

    assert!(
        rows.iter().all(|r| r.last_five.len() == 5),
        "최근 5경기는 다섯 글자 고정폭이어야 칼럼이 안 흔들린다"
    );
}

/// 두 필드가 없거나 null인 응답에서도 파싱이 죽지 않는다(시즌 첫 경기 전 등).
#[test]
fn missing_streak_fields_degrade_to_empty_strings() {
    let json = r#"{"result":{"seasonTeamStats":[
        {"ranking":1,"teamId":"LG","teamName":"LG","gameCount":0,"winGameCount":0,
         "loseGameCount":0,"drawnGameCount":0,"wra":0.0,"gameBehind":0.0,
         "lastFiveGames":null,"continuousGameResult":null}
    ]}}"#;
    let rows = standings_from_json(json).unwrap();
    assert!(rows[0].last_five.is_empty());
    assert!(rows[0].streak.is_empty());
}

/// v0.24: 팀 시즌 성적이 파싱된다. 응답이 팀마다 64개 필드를 주는데 v0.23까지
/// 순위·승패·최근5만 쓰고 나머지를 버리고 있었다.
#[test]
fn parses_team_season_stats() {
    const STANDINGS: &str = include_str!("fixtures/standings_2026_streaks.json");
    let rows = standings_from_json(STANDINGS).unwrap();
    let samsung = rows.iter().find(|r| r.team.name == "삼성").expect("삼성");

    // 타격: 실응답 실측값(2026-07-27 기준)
    assert!((samsung.stats.avg - 0.27632).abs() < 1e-4);
    assert!((samsung.stats.ops - 0.77616).abs() < 1e-4);
    assert_eq!(samsung.stats.homers, 81);
    assert_eq!(samsung.stats.steals, 71);
    assert_eq!(samsung.stats.runs, 533);

    // 투구·수비
    assert!((samsung.stats.era - 4.05861).abs() < 1e-4);
    assert!((samsung.stats.whip - 1.37201).abs() < 1e-4);
    assert_eq!(samsung.stats.quality_starts, 40);
    assert_eq!(samsung.stats.saves, 28);
    assert_eq!(samsung.stats.holds, 58);
    assert_eq!(samsung.stats.errors, 54);

    // 열 팀 모두 성적이 실려야 한다 — 한 팀만 비면 화면에서 그 팀만 빈 상자가 된다.
    assert!(
        rows.iter().all(|r| r.stats.era > 0.0 && r.stats.avg > 0.0),
        "성적이 빈 팀이 있다: {:?}",
        rows.iter()
            .filter(|r| r.stats.era == 0.0)
            .map(|r| &r.team.name)
            .collect::<Vec<_>>()
    );
}

/// 성적 필드가 통째로 없는 응답(시즌 개막 전 등)에서도 파싱이 죽지 않고
/// 0으로 저하한다. 화면은 `games == 0`으로 판단해 오버레이를 열지 않는다.
#[test]
fn missing_stat_fields_degrade_to_zero() {
    let json = r#"{"result":{"seasonTeamStats":[
        {"ranking":1,"teamId":"LG","teamName":"LG","gameCount":0,"winGameCount":0,
         "loseGameCount":0,"drawnGameCount":0,"wra":0.0,"gameBehind":0.0}
    ]}}"#;
    let rows = standings_from_json(json).unwrap();
    assert_eq!(rows[0].games, 0);
    assert_eq!(rows[0].stats, kbotop::model::TeamStats::default());
}
