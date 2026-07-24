use super::dto::{
    ApiEnvelope, Lineup, NewsResult, PtsOption, RelayResult, ScheduleGame, ScheduleResult,
    StandingsResult, TextRelayData,
};
use crate::error::Result;
use crate::model::{
    BaseState, Count, Game, GameStatus, LiveState, NewsItem, Pitch, PitchResult, Standing, Team,
};
use crate::source::text::{lead_excerpt, strip_html_to_text, EXCERPT_CHARS};

fn status_of(g: &ScheduleGame) -> GameStatus {
    if g.cancel {
        return GameStatus::Canceled;
    }
    if g.suspended {
        return GameStatus::Suspended;
    }
    match g.status_code.as_str() {
        "RESULT" => GameStatus::Final,
        "BEFORE" => GameStatus::Scheduled,
        "" => GameStatus::Scheduled,
        _ => GameStatus::Live, // STARTED/LIVE 등 진행중 값 총칭
    }
}

pub fn games_from_schedule(json: &str) -> Result<Vec<Game>> {
    let env: ApiEnvelope<ScheduleResult> = serde_json::from_str(json)?;
    let result = env.result.unwrap_or(ScheduleResult { games: vec![] });
    Ok(result
        .games
        .iter()
        .filter_map(|g| {
            // game_id/양팀 코드가 없으면 이 레코드는 화면에도, 폴링 대상으로도 쓸
            // 수 없다 — 하루치 배열 중 이 한 건만 건너뛰고 나머지는 그대로 보여준다
            // (필드 하나 빠졌다고 전체 목록이 비는 걸 막는다).
            let id = g.game_id.as_deref().filter(|s| !s.is_empty())?.to_string();
            let home_code = g
                .home_team_code
                .as_deref()
                .filter(|s| !s.is_empty())?
                .to_string();
            let away_code = g
                .away_team_code
                .as_deref()
                .filter(|s| !s.is_empty())?
                .to_string();
            Some(Game {
                id,
                start: g.game_date_time.clone(),
                status: status_of(g),
                status_label: g.status_info.clone(),
                home: Team {
                    code: home_code,
                    name: g.home_team_name.clone().unwrap_or_default(),
                },
                away: Team {
                    code: away_code,
                    name: g.away_team_name.clone().unwrap_or_default(),
                },
                home_score: g.home_team_score,
                away_score: g.away_team_score,
            })
        })
        .collect())
}

pub fn standings_from_json(json: &str) -> Result<Vec<Standing>> {
    let env: ApiEnvelope<StandingsResult> = serde_json::from_str(json)?;
    let result = env.result.unwrap_or(StandingsResult {
        season_team_stats: vec![],
    });
    let mut out: Vec<Standing> = result
        .season_team_stats
        .iter()
        .map(|t| Standing {
            rank: t.ranking,
            team: Team {
                code: t.team_id.clone(),
                name: t.team_name.clone(),
            },
            games: t.game_count,
            wins: t.win_game_count,
            losses: t.lose_game_count,
            draws: t.drawn_game_count,
            win_rate: t.wra,
            game_behind: t.game_behind,
        })
        .collect();
    out.sort_by_key(|s| s.rank);
    Ok(out)
}

/// 뉴스 목록: title 없는 항목은 건너뛰고, 깨진 응답은 빈 목록(관용 — 뉴스는
/// 부가 기능이라 실패가 앱에 전파되면 안 된다).
pub fn news_from_json(json: &str) -> Result<Vec<NewsItem>> {
    let env: ApiEnvelope<NewsResult> = serde_json::from_str(json)?;
    let list = env.result.map(|r| r.news_list).unwrap_or_default();
    Ok(list
        .into_iter()
        .filter(|a| !a.title.trim().is_empty())
        .map(|a| {
            let url = if a.oid.is_empty() || a.aid.is_empty() {
                String::new()
            } else {
                format!(
                    "https://m.sports.naver.com/kbaseball/article/{}/{}",
                    a.oid, a.aid
                )
            };
            NewsItem {
                title: a.title,
                source: a.source_name,
                url,
                summary: lead_excerpt(&strip_html_to_text(&a.sub_content), EXCERPT_CHARS),
                published: String::new(),
            }
        })
        .collect())
}

fn parse_u8(s: &str) -> u8 {
    s.trim().parse().unwrap_or(0)
}

fn parse_u16(s: &str) -> u16 {
    s.trim().parse().unwrap_or(0)
}

fn base_on(s: &str) -> bool {
    let s = s.trim();
    s != "0" && !s.is_empty()
}

/// 릴리스 속도벡터(ft/s) → km/h. 성분이 모두 0이면 None.
fn speed_kmh(p: &PtsOption) -> Option<u16> {
    let v = (p.vx0 * p.vx0 + p.vy0 * p.vy0 + p.vz0 * p.vz0).sqrt();
    if v <= 0.0 {
        return None;
    }
    Some((v * 1.09728).round() as u16) // ft/s → km/h (×0.3048×3.6)
}

/// 릴리스→플레이트 통과 시각 t(s). y0 + vy0*t + 0.5*ay*t^2 = crossPlateY의
/// 작은 양의 근. 퇴화(속도·가속 모두 ~0)나 해가 없으면 None.
fn plate_cross_t(p: &PtsOption) -> Option<f32> {
    let a = 0.5 * p.ay;
    let b = p.vy0;
    let c = p.y0 - p.cross_plate_y;
    let t = if a.abs() < 1e-6 {
        if b.abs() < 1e-6 {
            return None;
        }
        -c / b
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        let t1 = (-b - sq) / (2.0 * a);
        let t2 = (-b + sq) / (2.0 * a);
        [t1, t2]
            .into_iter()
            .filter(|t| *t > 0.0)
            .fold(f32::MAX, f32::min)
    };
    (t.is_finite() && t > 0.0 && t < 100.0).then_some(t)
}

/// crossPlateY는 실제로는 "플레이트를 통과했다고 보는 y거리"(포수 쪽 기준
/// 상수, 모든 투구에 걸쳐 ~0.708ft로 동일)이지 높이가 아니다 — 이걸 그대로
/// Pitch.plate_y(스트존 세로축)에 넣으면 모든 투구가 같은 높이에 찍힌다.
/// 실제 높이는 릴리스 위치/속도/가속도(투사체 운동)로 직접 계산해야 한다:
/// plate_cross_t()로 플레이트 통과 시각을 구한 뒤,
/// plate_z = z0 + vz0*t + 0.5*az*t^2로 그 시각의 높이를 구한다.
fn plate_height(p: &PtsOption) -> f32 {
    match plate_cross_t(p) {
        Some(t) => p.z0 + p.vz0 * t + 0.5 * p.az * t * t,
        // 기존 폴백 보존: 완전 퇴화(ay·vy0 모두 ~0)는 crossPlateY, 그 외 z0.
        None => {
            if (0.5 * p.ay).abs() < 1e-6 && p.vy0.abs() < 1e-6 {
                p.cross_plate_y
            } else {
                p.z0
            }
        }
    }
}

/// pitchId "YYMMDD_HHMMSS" → "HH:MM:SS". 형식이 다르면 None(관용 — 이 필드
/// 하나 때문에 투구 전체를 버리지 않는다).
fn time_from_pitch_id(id: &str) -> Option<String> {
    let (date, time) = id.split_once('_')?;
    if date.len() != 6 || time.len() != 6 {
        return None;
    }
    if !date.chars().chain(time.chars()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}:{}:{}", &time[0..2], &time[2..4], &time[4..6]))
}

fn result_of(text: &str) -> PitchResult {
    if text.contains("헛스윙") {
        PitchResult::StrikeSwinging
    } else if text.contains("루킹") || text.contains("스트라이크") {
        PitchResult::StrikeLooking
    } else if text.contains("파울") {
        PitchResult::Foul
    } else if text.contains("볼") {
        PitchResult::Ball
    } else if text.contains("타") || text.contains("아웃") || text.contains("홈런") {
        PitchResult::InPlay
    } else {
        PitchResult::Unknown
    }
}

fn name_of(id: &str, home: &Option<Lineup>, away: &Option<Lineup>) -> String {
    // 빈 문자열은 "id 없음"(currentGameState.pitcher/batter가 null→"")과 "pcode
    // 없는 선수"(Player.pcode가 null→"")가 같은 null_as_default 정책으로 합쳐진
    // 값이라 구분할 수 없다 — 매칭을 시도하면 pcode가 빈 라인업 항목을 "현재
    // 투수/타자"로 잘못 반환할 수 있으므로 애초에 매칭을 시도하지 않는다.
    if id.is_empty() {
        return String::new();
    }
    for lu in [home, away].into_iter().flatten() {
        for p in lu.batter.iter().chain(lu.pitcher.iter()) {
            if p.pcode == id {
                return p.name.clone();
            }
        }
    }
    String::new()
}

pub fn live_from_relay(json: &str, home: Team, away: Team) -> Result<LiveState> {
    let env: ApiEnvelope<RelayResult> = serde_json::from_str(json)?;
    let trd: TextRelayData = env
        .result
        .and_then(|r| r.text_relay_data)
        .ok_or_else(|| crate::error::Error::Data("no textRelayData".into()))?;

    let cgs = trd.current_game_state.unwrap_or_default();
    let count = Count {
        ball: parse_u8(&cgs.ball),
        strike: parse_u8(&cgs.strike),
        out: parse_u8(&cgs.out),
    };
    let bases = BaseState {
        first: base_on(&cgs.base1),
        second: base_on(&cgs.base2),
        third: base_on(&cgs.base3),
    };

    // Naver 중계 응답은 textRelays를 최신 순(내림차순)으로 내려준다. 그중
    // "현재 타석"으로 볼 항목은 (a) 투구 추적 데이터(ptsOptions)를 가진
    // 항목이거나, (b) 아직 투구는 없지만 타자 등장 안내(type==8, 예:
    // "9번타자 천성호")로 막 시작된 타석이다. type==8도 없고 ptsOptions도
    // 없는 항목(승리투수 발표=99, 이닝 시작=0 같은 진행-외 문구)만 건너뛴다.
    // 이걸 구분하지 않고 ptsOptions만 보면, 방금 시작해 아직 무투구인 새
    // 타석을 건너뛰고 이전 타자의 문자중계/스트존을 현재처럼 잘못 보여준다.
    const BATTER_ANNOUNCEMENT_TYPE: i32 = 8;
    let current = trd
        .text_relays
        .iter()
        .find(|t| {
            !t.pts_options.is_empty()
                || t.text_options
                    .iter()
                    .any(|o| o.r#type == BATTER_ANNOUNCEMENT_TYPE)
        })
        .or_else(|| trd.text_relays.first());
    // 앱이 직접 조립하는 UI chrome 텍스트이므로 영어 라벨 하드 제약을 따른다
    // (팀명/중계 텍스트 같은 원문 그대로의 API 데이터와 달리 이건 소스 코드에
    // 박힌 문자열이다). homeOrAway("0"=초/away 공격, "1"=말/home 공격)로 절반
    // 이닝까지 표기한다.
    let inning_label = current
        .map(|t| match t.home_or_away.as_str() {
            "0" => format!("T{}", t.inn),
            "1" => format!("B{}", t.inn),
            _ => format!("Inn {}", t.inn),
        })
        .unwrap_or_default();

    let mut relay_log: Vec<String> = Vec::new();
    let mut current_pitches: Vec<Pitch> = Vec::new();
    if let Some(tr) = current {
        for o in &tr.text_options {
            if !o.text.trim().is_empty() {
                relay_log.push(o.text.clone());
            }
        }
        for p in &tr.pts_options {
            // 같은 ballcount 순번의 텍스트를 매칭(없으면 빈 문자열).
            let text = tr
                .text_options
                .iter()
                .find(|t| t.text.starts_with(&format!("{}구", p.ballcount)))
                .map(|t| t.text.clone())
                .unwrap_or_default();
            current_pitches.push(Pitch {
                order: p.ballcount,
                plate_x: p.cross_plate_x,
                plate_y: plate_height(p),
                sz_top: p.top_sz,
                sz_bottom: p.bottom_sz,
                speed_kmh: speed_kmh(p),
                result: result_of(&text),
                text,
                time_hms: time_from_pitch_id(&p.pitch_id),
                plate_t: plate_cross_t(p).unwrap_or(0.0),
                y0: p.y0,
                vy0: p.vy0,
                ay: p.ay,
                z0: p.z0,
                vz0: p.vz0,
                az: p.az,
            });
        }
    }

    // 다음 타자: 공격 팀 라인업에서 현재 타자의 batOrder를 찾아 다음 타순
    // (9→1 순환)의 첫 항목을 고른다. 교체로 같은 batOrder가 여럿이면 첫
    // 항목을 채택(관용 — 틀릴 수 있는 추정이므로 실패 시 빈 문자열로 생략).
    let batting_lineup = current.and_then(|t| match t.home_or_away.as_str() {
        "0" => trd.away_lineup.as_ref(), // 초 = 원정 공격
        "1" => trd.home_lineup.as_ref(), // 말 = 홈 공격
        _ => None,
    });
    let next_batter_name = batting_lineup
        .and_then(|lu| {
            let cur = lu
                .batter
                .iter()
                .find(|b| !cgs.batter.is_empty() && b.pcode == cgs.batter)?;
            if cur.bat_order == 0 {
                return None;
            }
            let next = cur.bat_order % 9 + 1;
            lu.batter.iter().find(|b| b.bat_order == next)
        })
        .map(|b| b.name.clone())
        .unwrap_or_default();

    let metric = trd.last_valid_metric_option;
    Ok(LiveState {
        inning_label,
        home_score: parse_u16(&cgs.home_score),
        away_score: parse_u16(&cgs.away_score),
        pitcher_name: name_of(&cgs.pitcher, &trd.home_lineup, &trd.away_lineup),
        batter_name: name_of(&cgs.batter, &trd.home_lineup, &trd.away_lineup),
        home,
        away,
        count,
        bases,
        // Naver 응답의 승률은 0~100 퍼센트 값이라 UI(×100 표시)와 맞추기 위해 0~1 소수로 정규화한다.
        home_win_rate: metric
            .as_ref()
            .and_then(|m| m.home_team_win_rate)
            .map(|r| r / 100.0),
        away_win_rate: metric
            .as_ref()
            .and_then(|m| m.away_team_win_rate)
            .map(|r| r / 100.0),
        relay_log,
        current_pitches,
        next_batter_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::naver::dto::ScheduleGame;

    fn base_schedule_game() -> ScheduleGame {
        ScheduleGame {
            game_id: Some("g".into()),
            game_date_time: String::new(),
            home_team_code: Some("LG".into()),
            home_team_name: Some("LG".into()),
            away_team_code: Some("KT".into()),
            away_team_name: Some("KT".into()),
            home_team_score: None,
            away_team_score: None,
            status_code: String::new(),
            status_info: String::new(),
            cancel: false,
            suspended: false,
        }
    }

    #[test]
    fn cancel_takes_precedence_over_suspended_and_status_code() {
        let mut g = base_schedule_game();
        g.cancel = true;
        g.suspended = true;
        g.status_code = "RESULT".into();
        assert_eq!(status_of(&g), GameStatus::Canceled);
    }

    #[test]
    fn suspended_takes_precedence_over_status_code() {
        let mut g = base_schedule_game();
        g.suspended = true;
        g.status_code = "RESULT".into();
        assert_eq!(status_of(&g), GameStatus::Suspended);
    }

    #[test]
    fn before_and_empty_status_code_map_to_scheduled() {
        let mut g = base_schedule_game();
        g.status_code = "BEFORE".into();
        assert_eq!(status_of(&g), GameStatus::Scheduled);

        g.status_code = "".into();
        assert_eq!(status_of(&g), GameStatus::Scheduled);
    }

    #[test]
    fn unrecognized_in_progress_code_maps_to_live() {
        let mut g = base_schedule_game();
        g.status_code = "STARTED".into();
        assert_eq!(status_of(&g), GameStatus::Live);
    }

    #[test]
    fn games_from_schedule_skips_only_the_record_missing_a_team_code() {
        // g2는 homeTeamCode가 아예 빠져 있다(폴링/색상 조회에 필요한 실제 식별자)
        // — 배열 전체가 아니라 이 레코드 하나만 걸러져야 한다.
        let json = r#"{"result":{"games":[
            {"gameId":"g1","homeTeamCode":"LG","homeTeamName":"LG","awayTeamCode":"KT","awayTeamName":"KT","statusCode":"RESULT"},
            {"gameId":"g2","awayTeamCode":"OB","awayTeamName":"OB","statusCode":"RESULT"},
            {"gameId":"g3","homeTeamCode":"SS","homeTeamName":"SS","awayTeamCode":"NC","awayTeamName":"NC","statusCode":"RESULT"}
        ]}}"#;
        let games = games_from_schedule(json).unwrap();
        assert_eq!(games.len(), 2);
        assert!(games.iter().any(|g| g.id == "g1"));
        assert!(games.iter().any(|g| g.id == "g3"));
        assert!(!games.iter().any(|g| g.id == "g2"));
    }

    #[test]
    fn games_from_schedule_keeps_a_record_missing_only_a_cosmetic_team_name() {
        // 표시용 이름만 빠진 경우는 식별자가 아니므로 걸러지지 않고, 빈 이름으로
        // 완만히 처리돼야 한다(테마 색상은 code 기준이라 표시에 지장이 없다).
        let json = r#"{"result":{"games":[
            {"gameId":"g1","homeTeamCode":"LG","awayTeamCode":"KT","awayTeamName":"KT","statusCode":"RESULT"}
        ]}}"#;
        let games = games_from_schedule(json).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].home.name, "");
    }

    /// textRelays 맨 앞(최신) 항목이 막 시작한 타석(투구 0개, 타자 등장 안내만
    /// 있음)이면, ptsOptions가 있는 이전 타석으로 빠지지 않고 이 항목을 현재로
    /// 선택해야 한다 — docs/CURRENT_STATE.md의 "Task 4: at-bat 선택 staleness".
    #[test]
    fn current_at_bat_prefers_fresh_batter_announcement_over_older_pitches() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"inn": 9, "textOptions": [{"seqno": 2, "text": "9번타자 천성호", "type": 8}], "ptsOptions": []},
                {"inn": 9, "textOptions": [{"seqno": 1, "text": "1구 파울", "type": 1}], "ptsOptions": [
                    {"ballcount": 1, "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]}
            ],
            "lastValidMetricOption": {"homeTeamWinRate": 50.0, "awayTeamWinRate": 50.0}
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        assert!(live.relay_log.iter().any(|l| l.contains("9번타자 천성호")));
        assert!(!live.relay_log.iter().any(|l| l.contains("1구 파울")));
        assert!(live.current_pitches.is_empty());
    }

    /// result_of()가 스트라이크존 색상을 좌우하는 분류기다 — 각 분기가 실제로
    /// 해당 문구를 그 PitchResult로 매핑하는지 직접 검증한다(전체 함수 본문을
    /// `Unknown` 반환으로 바꿔도 이 테스트 전에는 전체 스위트가 그대로 통과했다).
    #[test]
    fn result_of_classifies_each_known_pitch_result_phrase() {
        assert_eq!(result_of("1구 헛스윙"), PitchResult::StrikeSwinging);
        assert_eq!(result_of("2구 스트라이크"), PitchResult::StrikeLooking);
        assert_eq!(result_of("루킹 삼진"), PitchResult::StrikeLooking);
        assert_eq!(result_of("3구 파울"), PitchResult::Foul);
        assert_eq!(result_of("4구 볼"), PitchResult::Ball);
        assert_eq!(result_of("5구 타격"), PitchResult::InPlay);
        assert_eq!(result_of("신민재 : 삼진 아웃"), PitchResult::InPlay);
        assert_eq!(result_of("박동원 : 좌익수 뒤 홈런"), PitchResult::InPlay);
        assert_eq!(result_of("9회말 LG 공격"), PitchResult::Unknown);
    }

    /// RELAY 고정픽스처(실제 네이버 응답)를 통해 result_of가 파이프라인 끝단
    /// (Pitch.result)까지 올바르게 이어지는지 확인한다 — 단위 테스트만으로는
    /// live_from_relay 배선이 빠진 회귀를 못 잡는다.
    #[test]
    fn relay_fixture_pitches_carry_the_classified_result() {
        const RELAY: &str = include_str!("../../../tests/fixtures/relay_20260719KTLG.json");
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(RELAY, team("LG"), team("KT")).unwrap();
        // fixture 실측: 현재 타석(천성호)의 1~3구 텍스트는 "1구 파울", "2구 헛스윙",
        // "3구 볼".
        assert_eq!(live.current_pitches[0].result, PitchResult::Foul);
        assert_eq!(live.current_pitches[1].result, PitchResult::StrikeSwinging);
        assert_eq!(live.current_pitches[2].result, PitchResult::Ball);
    }

    /// base_on()의 "주자 있음"(true) 분기 — 기존 테스트는 전부 base1/2/3이 "0"
    /// 이거나 비어 있어 이 분기가 한 번도 실행되지 않았다(base_on 본문을 `false`
    /// 상수로 바꿔도 전체 스위트가 통과했다).
    #[test]
    fn base_on_marks_bases_occupied_for_nonzero_runner_ids() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0","base1":"51100","base2":"0","base3":"66108"},
            "textRelays": [],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        assert!(
            live.bases.first,
            "non-\"0\" base1 must mark first base occupied"
        );
        assert!(
            !live.bases.second,
            "base2 == \"0\" must mean second base empty"
        );
        assert!(
            live.bases.third,
            "non-\"0\" base3 must mark third base occupied"
        );
    }

    /// speed_kmh()의 "성분이 모두 0이면 None" 가드 — 기존 테스트는 전부
    /// vx0/vy0/vz0가 실측값(0이 아님)이라 이 분기가 한 번도 실행되지 않았다
    /// (early-return을 지워도 전체 스위트가 그대로 통과했다).
    #[test]
    fn speed_kmh_returns_none_when_velocity_components_are_all_zero() {
        let p = PtsOption {
            ballcount: 1,
            cross_plate_x: 0.0,
            cross_plate_y: 0.0,
            top_sz: 0.0,
            bottom_sz: 0.0,
            vx0: 0.0,
            vy0: 0.0,
            vz0: 0.0,
            y0: 0.0,
            z0: 0.0,
            ay: 0.0,
            az: 0.0,
            stance: String::new(),
            pitch_id: String::new(),
        };
        assert_eq!(speed_kmh(&p), None);
    }

    #[test]
    fn time_from_pitch_id_parses_yymmdd_hhmmss() {
        assert_eq!(
            time_from_pitch_id("260529_205614"),
            Some("20:56:14".to_string())
        );
    }

    #[test]
    fn time_from_pitch_id_is_lenient_on_malformed_ids() {
        assert_eq!(time_from_pitch_id(""), None);
        assert_eq!(time_from_pitch_id("260529"), None);
        assert_eq!(time_from_pitch_id("260529_20561"), None); // 5자리 시각
        assert_eq!(time_from_pitch_id("260529_2056xx"), None);
        assert_eq!(time_from_pitch_id("abcdef_205614"), None);
    }

    /// fixture의 투구들이 실제 시각과 궤적 파라미터를 실어 나르는지 — 스트존/측면
    /// 뷰가 소비할 데이터의 완전성.
    #[test]
    fn relay_fixture_pitches_carry_time_and_trajectory_params() {
        let state = live_from_relay(
            include_str!("../../../tests/fixtures/relay_20260719KTLG.json"),
            Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            Team {
                code: "KT".into(),
                name: "KT".into(),
            },
        )
        .unwrap();
        assert!(!state.current_pitches.is_empty());
        for p in &state.current_pitches {
            // 궤적 파라미터: 실측 relay에서 y0(릴리스 거리)는 40~60ft 범위.
            assert!(p.y0 > 40.0 && p.y0 < 60.0, "y0 out of range: {}", p.y0);
            assert!(p.plate_t > 0.0, "plate_t must be positive");
        }
        // fixture 실측: 이 fixture는 pitchId를 싣고 있어(구버전과 달리) 5구
        // 모두 시각이 실린다 — "HH:MM:SS", 초 단위로 상승.
        let times: Vec<Option<String>> = state
            .current_pitches
            .iter()
            .map(|p| p.time_hms.clone())
            .collect();
        assert_eq!(
            times,
            vec![
                Some("21:05:40".to_string()),
                Some("21:05:59".to_string()),
                Some("21:06:21".to_string()),
                Some("21:06:46".to_string()),
                Some("21:07:06".to_string()),
            ]
        );
    }

    #[test]
    fn news_from_json_parses_titles_and_sources_leniently() {
        let json = r#"{"code":200,"success":true,"result":{"newsList":[
            {"title":"제목1","sourceName":"연합뉴스"},
            {"title":"제목2"},
            {"sourceName":"출처만"},
            {"title":null,"sourceName":null}
        ]}}"#;
        let items = news_from_json(json).unwrap();
        // 완전성: title이 있는 항목은 전부 수집(2개), 없는 항목은 조용히 건너뜀.
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "제목1");
        assert_eq!(items[0].source, "연합뉴스");
        assert_eq!(items[1].source, ""); // sourceName 결측 관용
                                         // 깨진 응답도 에러 아닌 빈 목록
        assert!(news_from_json("{}").unwrap().is_empty());
    }

    #[test]
    fn news_from_json_builds_article_urls_from_oid_aid_leniently() {
        let json = r#"{"code":200,"success":true,"result":{"newsList":[
            {"title":"제목1","sourceName":"A","oid":"450","aid":"0000152707"},
            {"title":"제목2","sourceName":"B"}
        ]}}"#;
        let items = news_from_json(json).unwrap();
        assert_eq!(
            items[0].url,
            "https://m.sports.naver.com/kbaseball/article/450/0000152707"
        );
        assert_eq!(items[1].url, "", "missing oid/aid → empty url, item kept");
    }

    /// 목록 파싱이 subContent를 발췌(summary)로 채운다 — 기사 본문 fetch 없이
    /// 오버레이에 보여줄 텍스트를 목록 단계에서 확보한다.
    #[test]
    fn news_items_carry_summary_from_sub_content() {
        let json = r#"{"result":{"newsList":[{"title":"t","sourceName":"s","oid":"144","aid":"0001","subContent":"<b>요약</b> 본문 조각"}]}}"#;
        let items = news_from_json(json).unwrap();
        assert_eq!(items[0].summary, "요약 본문 조각", "HTML은 제거돼야 한다");
        let long = "가".repeat(500);
        let json2 = format!(
            r#"{{"result":{{"newsList":[{{"title":"t","sourceName":"s","oid":"1","aid":"2","subContent":"{long}"}}]}}}}"#
        );
        let items2 = news_from_json(&json2).unwrap();
        assert!(
            items2[0].summary.chars().count() <= EXCERPT_CHARS + 1,
            "발췌 상한을 넘으면 안 된다: {}",
            items2[0].summary.chars().count()
        );
    }

    #[test]
    fn next_batter_follows_current_batter_in_the_batting_lineup() {
        let state = live_from_relay(
            include_str!("../../../tests/fixtures/relay_20260719KTLG.json"),
            Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            Team {
                code: "KT".into(),
                name: "KT".into(),
            },
        )
        .unwrap();
        // fixture 실측: 현재 타자는 천성호(batOrder 9, home 라인업, 말공격).
        // 9 % 9 + 1 = 1번 타순인 문성주가 다음 타자로 계산되어야 한다.
        assert_eq!(state.batter_name, "천성호");
        assert_eq!(state.next_batter_name, "문성주");
    }
}
