use super::dto::{
    ApiEnvelope, Lineup, PtsOption, RelayResult, ScheduleGame, ScheduleResult, StandingsResult,
    TextRelay, TextRelayData,
};
use crate::error::Result;
use crate::model::{
    AtBat, BaseState, Count, Game, GameStatus, LiveState, Pitch, PitchResult, RelayLine, Standing,
    Team,
};

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

/// "현재 타석"으로 볼 항목인지: (a) 투구 추적 데이터(ptsOptions)를 가졌거나,
/// (b) 아직 투구는 없지만 타자 등장 안내(type==8)로 막 시작된 타석. 둘 다
/// 아니면(승리투수 발표=99, 이닝 시작=0 같은 진행-외 문구) 타석으로 치지
/// 않는다 — live_from_relay의 `current` 선택과 at_bats 구성이 이 판정을
/// 공유한다(어긋나면 "현재 타석"과 "at_bats 마지막 항목"이 달라진다).
const BATTER_ANNOUNCEMENT_TYPE: i32 = 8;

fn is_at_bat_worthy(t: &TextRelay) -> bool {
    !t.pts_options.is_empty()
        || t.text_options
            .iter()
            .any(|o| o.r#type == BATTER_ANNOUNCEMENT_TYPE)
}

/// 앱이 직접 조립하는 UI chrome 텍스트(영어 라벨 하드 제약을 따른다 — 팀명/
/// 중계 텍스트 같은 원문 그대로의 API 데이터와 달리 이건 소스 코드에 박힌
/// 문자열이다). homeOrAway("0"=초/away 공격, "1"=말/home 공격)로 절반 이닝까지
/// 표기한다. LiveState.inning_label과 AtBat.inning_label이 이 함수를 공유한다.
fn inning_label_of(t: &TextRelay) -> String {
    match t.home_or_away.as_str() {
        "0" => format!("T{}", t.inn),
        "1" => format!("B{}", t.inn),
        _ => format!("Inn {}", t.inn),
    }
}

/// 문자중계 줄 → 투구 인덱스(v0.19 연동). 응답이 직접 실어 주는 외래키
/// `textOption.ptsPitchId` ↔ `ptsOption.pitchId`로 잇는다. 빈 id는 잇지 않는다
/// (양쪽이 다 결측이면 `"" == ""`로 엉뚱한 줄이 첫 투구에 붙어 버린다).
///
/// # 왜 이 방법인가 (후보 3종을 실측으로 떨어뜨린 근거)
/// 구현 당시 표본은 2026-07-25 5경기 × 9이닝(타석 514건·투구 1,575건). 이후
/// 리뷰가 5개 날짜·24경기·응답 218개(타석 2,454건·투구 7,474건)로 재검증해
/// 아래 결론을 재확인하고 예외 하나를 더 찾았다(표는 [`crate::source::naver::dto::TextOption::pts_pitch_id`]
/// 참고).
/// - **① 텍스트 "N구"에서 순번 추출 — 틀린다.** 투구가 아닌 줄도 "N구"로
///   시작한다: `type==7`의 "1구 피치클락 타자위반 스트라이크" / "1구 피치클락
///   투수위반 볼"(구현 시 표본 2건, 큰 표본에서는 8건). 이 줄엔 추적 데이터가
///   없는데 순번만 보면 첫 투구에 붙는다. 게다가 그 타석들은 위반이 카운트를
///   먹어 `ballcount`가 어긋나는데, 앞이 밀리기만 하는 게 아니라 타석 중간에
///   번호가 통째로 비는 경우도 있다(`[1,2,3,5]` 등, 큰 표본에서 확인) —
///   순번을 인덱스로 쓰면 이후 줄이 임의 위치에서 어긋난다.
/// - **② seqno 순서 ↔ ptsOptions 순서 — 맞지만 가정이 많다.** 실측으론
///   타석 전부에서 `type==1` 줄 수와 투구 수가 정확히 같고 순서도 1:1이지만,
///   "투구 줄은 type 1이다"·"둘의 순서가 같다"는 두 전제에 기댄다.
/// - **③ 타임스탬프 근접 — 불가능.** textOption엔 시각 필드가 아예 없다.
/// - **④ `ptsPitchId` 외래키 — 채택.** 언어·순서·타입 코드 어디에도 기대지
///   않는 유일한 방법이며, ②와 결과가 일치한다(1,575/1,575, 큰 표본에서도
///   7,474/7,474). 다만 "투구 줄 전부가 유효한 값을 갖는다"는 아니다 —
///   `"-1"` 센티널(추적 데이터 없는 진짜 투구)이 투구 줄 7,476건 중 2건 있고,
///   그 값은 어떤 `ptsOptions`와도 안 맞는다(고아 txt→pts 2건). 양방향 고아
///   pts→txt·타석 안 중복·타석 경계를 넘는 재사용은 전부 0건이다.
fn pitch_idx_of_line(t: &TextRelay, pts_pitch_id: &str) -> Option<usize> {
    if pts_pitch_id.is_empty() {
        return None;
    }
    t.pts_options
        .iter()
        .position(|p| p.pitch_id == pts_pitch_id)
}

/// 타석의 문자중계 줄(오래된→최신, 응답 원문 seqno 순서 그대로 — 한 항목 안의
/// textOptions는 이미 오름차순이다). 빈 텍스트는 건너뛴다. 투구 줄은 자기가
/// 서술하는 투구의 인덱스를 함께 싣는다([`pitch_idx_of_line`]).
///
/// `is_pitch`는 `ptsPitchId`가 비어 있지 않은지로 정한다 — 매칭 성공 여부와는
/// 별개다. `ptsPitchId == "-1"`(추적 데이터 없는 진짜 투구, 실측 7,476줄 중
/// 2건)인 줄은 `is_pitch: true`이면서 `pitch_idx_of_line`이 `None`을 내는데,
/// [`crate::model::LiveState::pitch_at_relay_line`]이 이 비트로 그 상태를
/// "투구 줄이 아님"과 구분해 carry-down을 막는다(리뷰 v19b I-1).
fn relay_lines_of(t: &TextRelay) -> Vec<RelayLine> {
    t.text_options
        .iter()
        .filter(|o| !o.text.trim().is_empty())
        .map(|o| RelayLine {
            text: o.text.clone(),
            pitch_idx: pitch_idx_of_line(t, &o.pts_pitch_id),
            is_pitch: !o.pts_pitch_id.is_empty(),
        })
        .collect()
}

/// 타석의 투구 목록(ptsOptions → Pitch). 짝이 되는 문자중계 줄의 텍스트를 실어
/// 결과 분류(`result_of`)와 상세줄 원문에 쓴다(없으면 빈 문자열).
///
/// 매칭은 `ptsPitchId` 외래키가 우선이고, 그 값이 없는 응답에서만 기존
/// ballcount 접두("{N}구") 매칭으로 물러선다. 실측 7,474건에서 두 방법의 결과가
/// 완전히 같아 동작 변화는 없고, 대신 "1구 피치클락 …" 같은 **투구가 아닌 줄이
/// 접두만으로 투구에 붙는** 경로가 원천 차단된다. 폴백을 남긴 이유는 관용
/// 파싱이다 — 외래키를 안 주는 응답에서 `Pitch.text`가 통째로 비면 존 색상
/// 분류까지 함께 죽는다.
///
/// # 리뷰 M-4 — 이 우선순위는 실 데이터로는 회귀 테스트가 못 지킨다
/// 두 방법이 실측 전수(7,474건)에서 100% 일치하므로, 실제 응답으로 이
/// 분기를 통째로 지워도(v0.18 접두 전용으로 되돌려도) 기존 테스트가 전부
/// 통과한다 — 순수 견고화이지 제거해야 할 죽은 코드가 아니다. 우선순위
/// 자체를 지키는 것은 합성 테스트
/// `pitches_of_prefers_the_foreign_key_match_over_an_earlier_same_prefix_decoy`
/// (같은 접두로 시작하는 가짜 줄과 진짜 줄을 함께 둬 두 방법이 갈리게
/// 만든 경우)뿐이다.
fn pitches_of(t: &TextRelay) -> Vec<Pitch> {
    t.pts_options
        .iter()
        .map(|p| {
            let text = t
                .text_options
                .iter()
                .find(|o| !o.pts_pitch_id.is_empty() && o.pts_pitch_id == p.pitch_id)
                .or_else(|| {
                    t.text_options
                        .iter()
                        .find(|o| o.text.starts_with(&format!("{}구", p.ballcount)))
                })
                .map(|o| o.text.clone())
                .unwrap_or_default();
            Pitch {
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
            }
        })
        .collect()
}

/// 타자 등장 안내(type==8, "9번타자 천성호" 형식) 텍스트에서 이름만 뽑는다.
/// 안내가 없으면 빈 문자열(관용 — pts만 있고 안내가 빠진 항목도 버리지 않고
/// 표현하되, 이름 칸만 비운다). "번타자 " 구분자가 없는 낯선 형식이면 원문
/// 전체를 폴백으로 돌려준다(정보를 아예 잃는 것보다 낫다).
fn batter_name_of(t: &TextRelay) -> String {
    let Some(o) = t
        .text_options
        .iter()
        .find(|o| o.r#type == BATTER_ANNOUNCEMENT_TYPE)
    else {
        return String::new();
    };
    match o.text.split_once("번타자 ") {
        Some((_, name)) => name.trim().to_string(),
        None => o.text.clone(),
    }
}

fn at_bat_of(t: &TextRelay) -> AtBat {
    AtBat {
        seq: t.no,
        batter_name: batter_name_of(t),
        inning_label: inning_label_of(t),
        relay_lines: relay_lines_of(t),
        pitches: pitches_of(t),
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

/// 응답의 textRelays에서 타석 목록을 만든다(오래된→최신).
///
/// 라이브 경로(`live_from_relay`)와 과거 이닝 경로(`at_bats_from_relay`)가 이
/// 함수를 공유한다 — 두 경로가 술어를 따로 들면 같은 타석이 한쪽에서는 잡히고
/// 다른 쪽에서는 빠져 목록을 이을 때 구멍이 생긴다(v0.18에서 `current` 선택과
/// at_bats 구성을 같은 술어로 묶은 것과 같은 이유).
fn at_bats_of(text_relays: &[TextRelay]) -> Vec<AtBat> {
    let mut at_bats: Vec<AtBat> = text_relays
        .iter()
        .filter(|t| is_at_bat_worthy(t))
        .map(at_bat_of)
        .collect();
    at_bats.reverse(); // 응답 원문(최신→오래된)을 오래된→최신으로.
    at_bats
}

/// `?inning=N` 응답에서 **그 이닝의 타석만** 뽑는다(v0.20 "과거 이닝 돌려보기").
///
/// 스코어보드·카운트·투수/타자는 일부러 버린다. 실측상 `currentGameState`는 어느
/// 이닝을 요청해도 **현재** 값이라(요청이 3회여도 `inn`은 9), 그걸 과거 이닝 화면에
/// 섞으면 "한 화면이 두 상황을 말하는" v0.18의 결함이 그대로 재현된다. 과거 이닝에서
/// 확실한 것은 그 이닝에 실제로 일어난 타석뿐이다.
///
/// 범위 밖 이닝은 200 + 빈 `textRelays`로 오므로(실측 `?inning=99`) 빈 목록을
/// 돌려준다 — 에러가 아니어야 호출부가 "그 이닝은 없다"로 캐시해 재요청을 멈춘다.
pub fn at_bats_from_relay(json: &str) -> Result<Vec<AtBat>> {
    let env: ApiEnvelope<RelayResult> = serde_json::from_str(json)?;
    let trd: TextRelayData = env
        .result
        .and_then(|r| r.text_relay_data)
        .ok_or_else(|| crate::error::Error::Data("no textRelayData".into()))?;
    Ok(at_bats_of(&trd.text_relays))
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

    // Naver 중계 응답은 textRelays를 최신 순(내림차순)으로 내려준다. is_at_bat_worthy가
    // "현재 타석"으로 볼 항목을 판정한다 — type==8도 없고 ptsOptions도 없는
    // 항목(승리투수 발표=99, 이닝 시작=0 같은 진행-외 문구)만 건너뛴다. 이걸
    // 구분하지 않고 ptsOptions만 보면, 방금 시작해 아직 무투구인 새 타석을
    // 건너뛰고 이전 타자의 문자중계/스트존을 현재처럼 잘못 보여준다.
    let current = trd
        .text_relays
        .iter()
        .find(|t| is_at_bat_worthy(t))
        .or_else(|| trd.text_relays.first());
    let inning_label = current.map(inning_label_of).unwrap_or_default();

    let relay_log: Vec<RelayLine> = current.map(relay_lines_of).unwrap_or_default();
    let current_pitches: Vec<Pitch> = current.map(pitches_of).unwrap_or_default();

    // 과거 타석들(v0.18 "돌려보기"): is_at_bat_worthy를 만족하는 모든 항목을
    // AtBat으로 만든다. 이 필터는 `current` 선택과 동일한 술어를 쓰므로,
    // 최소 하나가 걸리는 정상 상황에서는 이 벡터의 마지막 항목이 항상
    // `current`와 같은 textRelay에서 나온다(reverse 후 마지막 = 원본에서
    // 가장 먼저 매칭된 항목 = `current`) — relay_log/current_pitches와
    // 어긋나지 않는다. text_relays 전부가 진행-외 문구뿐이면(예: 이닝
    // 시작 직후, 아직 첫 타자도 안내되지 않음) 여기는 비지만, 그 경우
    // current_pitches도 항상 비어 있으므로(진행-외 항목은 pts_options가
    // 없다) 서로 어긋나지 않는다.
    let at_bats = at_bats_of(&trd.text_relays);

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
        at_bats,
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
        assert!(live
            .relay_log
            .iter()
            .any(|l| l.text.contains("9번타자 천성호")));
        assert!(!live.relay_log.iter().any(|l| l.text.contains("1구 파울")));
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

    /// v0.18 "돌려보기": fixture 실측 — textRelays 13건 중 진행-외 문구뿐인
    /// 3건(승리투수 발표 1건 + 이닝 시작 2건)을 제외한 10건이 실제 타석이다.
    /// at_bats는 오래된→최신 순으로 서고, 마지막 항목이 곧 "현재 타석"
    /// (relay_log/current_pitches)과 내용이 같아야 한다 — 파서가 둘 다 같은
    /// 헬퍼(relay_lines_of/pitches_of)를 쓰기 때문.
    #[test]
    fn at_bats_are_ordered_oldest_to_newest_and_mirror_the_current_at_bat() {
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
        assert_eq!(state.at_bats.len(), 10, "10 real at-bats in the fixture");
        // 가장 오래된 항목(1번타자 최원준, KT 공격 맨 처음)이 first().
        assert_eq!(state.at_bats.first().unwrap().batter_name, "최원준");
        // 가장 최신 항목(9번타자 천성호)이 last() = 현재 타석과 동일해야 한다.
        let latest = state.at_bats.last().unwrap();
        assert_eq!(latest.batter_name, "천성호");
        assert_eq!(latest.pitches, state.current_pitches);
        assert_eq!(latest.relay_lines, state.relay_log);

        // seq는 응답의 textRelay `no` 원문이어야 한다(fixture 실측: 최원준 87 →
        // 천성호 97, 진행-외 문구 3건을 걸러 10건). 돌려보기 선택이 이 번호로
        // 고정되므로, 0부터의 인덱스로 채워 넣으면 이닝이 갈릴 때 조용히 어긋난다.
        assert_eq!(state.at_bats.first().unwrap().seq, 87);
        assert_eq!(latest.seq, 97);
        let seqs: Vec<i64> = state.at_bats.iter().map(|ab| ab.seq).collect();
        let mut sorted = seqs.clone();
        sorted.sort_unstable();
        assert_eq!(seqs, sorted, "seq는 오래된→최신으로 증가해야 한다");
    }

    /// 과거 타석(현재가 아닌 at-bat)도 자기 자신의 투구 데이터를 온전히
    /// 담아야 한다 — 이게 이 태스크의 핵심("파서가 버리던 걸 살린다"). fixture
    /// 실측: 가장 오래된 타석(1번타자 최원준)은 7구를 던졌다.
    #[test]
    fn a_past_at_bat_carries_its_own_full_pitch_history() {
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
        let oldest = &state.at_bats[0];
        assert_eq!(oldest.pitches.len(), 7);
        // 과거 타석의 투구도 현재 타석과 동일한 파이프라인(result_of 등)을
        // 거쳐 분류된 결과를 담고 있어야 스트존/측면뷰가 재사용할 수 있다.
        assert!(oldest
            .pitches
            .iter()
            .any(|p| p.result != PitchResult::Unknown));
        // 이 타석은 두 번째로 오래된 타석(2번타자 김현수)과 구분되는 자기만의
        // 문자중계를 갖는다 — at_bats[1]과 섞여 있지 않은지 확인.
        assert!(oldest.relay_lines.iter().any(|l| l.text.contains("최원준")));
        assert!(!oldest.relay_lines.iter().any(|l| l.text.contains("김현수")));
    }

    /// 진행-외 문구뿐인 항목(승리투수 발표 type==99, 이닝 시작 type==0)은
    /// at_bats에서 걸러진다 — `current` 선택과 동일한 관용 규칙을 공유한다는
    /// 걸 손으로 만든 최소 fixture로 명시적으로 고정한다(실 fixture만으로는
    /// "왜" 3건이 빠졌는지가 개수 비교로만 드러나 회귀를 놓치기 쉽다).
    #[test]
    fn at_bats_filters_out_progress_only_entries_between_real_at_bats() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"inn": 9, "homeOrAway": "1", "textOptions": [
                    {"seqno": 3, "text": "=====", "type": 99},
                    {"seqno": 4, "text": "승리투수: 고영표", "type": 99}
                ], "ptsOptions": []},
                {"inn": 9, "homeOrAway": "1", "textOptions": [
                    {"seqno": 2, "text": "9번타자 천성호", "type": 8},
                    {"seqno": 2, "text": "1구 파울", "type": 1}
                ], "ptsOptions": [
                    {"ballcount": 1, "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]},
                {"inn": 9, "homeOrAway": "1", "textOptions": [
                    {"seqno": 1, "text": "9회말 LG 공격", "type": 0}
                ], "ptsOptions": []},
                {"inn": 9, "homeOrAway": "0", "textOptions": [
                    {"seqno": 0, "text": "8번타자 박동원", "type": 8}
                ], "ptsOptions": []}
            ],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        assert_eq!(live.at_bats.len(), 2, "only the two real at-bats survive");
        // reverse 후 오래된→최신: 원문 마지막(가장 오래된) 박동원이 first.
        assert_eq!(live.at_bats[0].batter_name, "박동원");
        assert_eq!(live.at_bats[1].batter_name, "천성호");
        assert!(!live
            .at_bats
            .iter()
            .any(|ab| ab.relay_lines.iter().any(|l| l.text.contains("승리투수"))));
        assert!(!live.at_bats.iter().any(|ab| ab
            .relay_lines
            .iter()
            .any(|l| l.text.contains("9회말 LG 공격"))));
    }

    // ---- v0.19: 문자중계 줄 ↔ 투구 연동 (relay_lines_of의 ptsPitchId 조인) ----

    fn kt_lg_fixture() -> LiveState {
        live_from_relay(
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
        .unwrap()
    }

    /// 연동의 뼈대: 투구 줄은 자기 투구의 **인덱스**를 싣고, 투구가 아닌 줄
    /// (타자 등장 안내·결과 요약)은 아무것도 싣지 않는다. fixture 실측(천성호
    /// 타석 = 최신, no=97): 안내 1줄 + 투구 5줄 + 결과 요약 1줄.
    #[test]
    fn pitch_relay_lines_carry_the_index_of_the_pitch_they_describe() {
        let state = kt_lg_fixture();
        let ab = state.at_bats.last().unwrap();
        let links: Vec<Option<usize>> = ab.relay_lines.iter().map(|l| l.pitch_idx).collect();
        assert_eq!(
            links,
            vec![None, Some(0), Some(1), Some(2), Some(3), Some(4), None],
            "안내 → 1~5구 → 결과 요약 순서로 이어져야 한다: {:?}",
            ab.relay_lines.iter().map(|l| &l.text).collect::<Vec<_>>()
        );
        // 링크가 가리키는 투구가 실제로 그 줄이 말하는 투구인지 — 인덱스만
        // 맞고 내용이 어긋나면 존이 엉뚱한 공을 띄운다.
        for line in &ab.relay_lines {
            if let Some(i) = line.pitch_idx {
                assert_eq!(
                    ab.pitches[i].text, line.text,
                    "링크된 투구의 원문이 그 줄과 같아야 한다"
                );
            }
        }
    }

    /// 투수 교체·수비위치 변경(type==2)이 안내와 첫 투구 **사이에** 끼어도
    /// 링크가 밀리지 않는다 — 순서·개수에 기대지 않고 외래키로 잇기 때문.
    /// fixture 실측: 오지환 타석(no=94)은 안내 1 + 교체 3 + 투구 5 + 요약 1줄.
    #[test]
    fn substitution_lines_between_the_announcement_and_the_pitches_do_not_shift_the_links() {
        let state = kt_lg_fixture();
        let ab = state
            .at_bats
            .iter()
            .find(|ab| ab.seq == 94)
            .expect("fixture must contain at-bat 94");
        let links: Vec<Option<usize>> = ab.relay_lines.iter().map(|l| l.pitch_idx).collect();
        assert_eq!(
            links,
            vec![
                None,
                None,
                None,
                None,
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None
            ]
        );
        assert!(
            ab.relay_lines[1].text.contains("교체"),
            "전제: 교체 줄이 있다"
        );
    }

    /// ★ 매칭 방법 ①("N구" 텍스트에서 순번 추출)이 실제로 틀리는 지점.
    /// 실측(2026-07-25 KT-롯데 4회 김현수 / LG-한화 3회 구본혁)을 최소 JSON으로
    /// 재현했다: 피치클락 위반 줄(type==7)이 "1구 …"로 시작하지만 추적
    /// 데이터가 없고, 위반이 카운트를 먹어 실제 투구는 ballcount 2부터
    /// 시작한다. 텍스트 순번으로 이었다면 위반 줄이 첫 투구에 붙고 나머지가
    /// 한 칸씩 밀렸을 것이다 — 외래키로 이으면 위반 줄은 링크가 없고
    /// "2구 볼"이 0번 투구를 가리킨다.
    #[test]
    fn a_pitch_clock_violation_line_looks_like_a_pitch_but_is_not_linked() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"no": 35, "inn": 4, "homeOrAway": "0", "textOptions": [
                    {"seqno": 179, "text": "2번타자 김현수", "type": 8},
                    {"seqno": 180, "text": "1구 피치클락 타자위반 스트라이크", "type": 7},
                    {"seqno": 181, "text": "2구 볼", "type": 1, "ptsPitchId": "260725_190332"},
                    {"seqno": 182, "text": "3구 파울", "type": 1, "ptsPitchId": "260725_190359"}
                ], "ptsOptions": [
                    {"ballcount": 2, "pitchId": "260725_190332", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 3, "pitchId": "260725_190359", "crossPlateX": 0.2, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]}
            ],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        let ab = &live.at_bats[0];
        let links: Vec<Option<usize>> = ab.relay_lines.iter().map(|l| l.pitch_idx).collect();
        assert_eq!(
            links,
            vec![None, None, Some(0), Some(1)],
            "피치클락 위반 줄은 투구가 아니다 — 링크가 없어야 한다"
        );
        // 투구 순번(ballcount)은 2부터인데 인덱스는 0부터다: 이 어긋남이
        // pitch_idx를 순번이 아니라 인덱스로 담는 이유다.
        assert_eq!(ab.pitches[0].order, 2);
        assert_eq!(ab.pitches[0].text, "2구 볼");
    }

    /// 우아한 저하: 응답이 외래키를 안 주면(구버전/부분 응답) 줄은 그대로
    /// 보이되 연동만 없다 — 패닉도, 엉뚱한 링크도 없다. 특히 양쪽 id가 모두
    /// 비어 있을 때 `"" == ""`로 첫 투구에 아무 줄이나 붙으면 안 된다.
    #[test]
    fn relay_lines_degrade_to_no_link_when_the_response_omits_pitch_ids() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"no": 1, "inn": 1, "homeOrAway": "0", "textOptions": [
                    {"seqno": 1, "text": "1번타자 최원준", "type": 8},
                    {"seqno": 2, "text": "1구 파울", "type": 1}
                ], "ptsOptions": [
                    {"ballcount": 1, "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]}
            ],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        let ab = &live.at_bats[0];
        assert!(
            ab.relay_lines.iter().all(|l| l.pitch_idx.is_none()),
            "외래키가 없으면 연동 없이 저하한다"
        );
        assert_eq!(ab.relay_lines.len(), 2, "줄 자체는 그대로 보여야 한다");
        // 외래키가 없어도 투구 원문은 기존 ballcount 접두 폴백으로 살아 있다
        // (존 색상 분류가 여기 달려 있다).
        assert_eq!(ab.pitches[0].text, "1구 파울");
        assert_eq!(ab.pitches[0].result, PitchResult::Foul);
    }

    /// ★ 리뷰 v19b I-1의 실측 재현: `ptsPitchId == "-1"`(추적 데이터 없는 진짜
    /// 투구, 20260614LTLG 3회 no=21 황성빈 타석을 최소 JSON으로 고정한 것 —
    /// 합성이 아니라 실측이라 다음 리뷰가 재현할 수 있다). 7개 투구는 정상
    /// 추적됐고, 8번째 "8구 타격"만 `ptsPitchId="-1"`이라 어떤 `ptsOption`과도
    /// 짝지어지지 않는다.
    ///
    /// 고쳐지기 전 버그: 이 줄에 커서를 두면 carry-down이 위쪽 7구를 이 줄의
    /// 공인 척 보여줬다("8구 타격"인데 존·상세줄은 "7구 파울"). 고친 뒤에는
    /// 그 줄 자체에서는 선택이 풀리고(`None`), 그 아래 결과 요약 줄(원래
    /// 투구 줄이 아님)로 커서가 더 내려가면 평소처럼 마지막 실제 투구(7구)를
    /// 계속 물려받는다 — 그건 의도된 동작이다.
    #[test]
    fn a_pitch_with_no_tracking_data_stops_carry_down_instead_of_borrowing_the_pitch_above_it() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"no": 21, "inn": 3, "homeOrAway": "0", "textOptions": [
                    {"seqno": 1, "text": "3번타자 황성빈", "type": 8},
                    {"seqno": 2, "text": "1구 파울", "type": 1, "ptsPitchId": "20260614_190001"},
                    {"seqno": 3, "text": "2구 파울", "type": 1, "ptsPitchId": "20260614_190010"},
                    {"seqno": 4, "text": "3구 볼", "type": 1, "ptsPitchId": "20260614_190020"},
                    {"seqno": 5, "text": "4구 파울", "type": 1, "ptsPitchId": "20260614_190030"},
                    {"seqno": 6, "text": "5구 볼", "type": 1, "ptsPitchId": "20260614_190040"},
                    {"seqno": 7, "text": "6구 파울", "type": 1, "ptsPitchId": "20260614_190050"},
                    {"seqno": 8, "text": "7구 파울", "type": 1, "ptsPitchId": "20260614_190060"},
                    {"seqno": 9, "text": "투수 투수판 이탈", "type": 7},
                    {"seqno": 10, "text": "8구 타격", "type": 1, "ptsPitchId": "-1"},
                    {"seqno": 11, "text": "황성빈 : 우익수 오른쪽 1루타", "type": 6}
                ], "ptsOptions": [
                    {"ballcount": 1, "pitchId": "20260614_190001", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 2, "pitchId": "20260614_190010", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 3, "pitchId": "20260614_190020", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 4, "pitchId": "20260614_190030", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 5, "pitchId": "20260614_190040", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 6, "pitchId": "20260614_190050", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"},
                    {"ballcount": 7, "pitchId": "20260614_190060", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]}
            ],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LT"), team("LG")).unwrap();
        let ab = &live.at_bats[0];

        // 파서: "8구 타격" 줄은 투구 줄이라고 응답이 말하지만(`is_pitch`) 짝을
        // 못 찾는다(`pitch_idx: None`) — "애초에 투구 줄이 아님"과 구분돼야
        // 하는 바로 그 상태다.
        let untracked = &ab.relay_lines[9];
        assert_eq!(untracked.text, "8구 타격");
        assert!(
            untracked.is_pitch,
            "ptsPitchId가 있으니 투구 줄로 표시돼야 한다"
        );
        assert_eq!(
            untracked.pitch_idx, None,
            "\"-1\"은 어떤 투구와도 안 맞는다"
        );

        // 모델: 커서가 그 줄 자체에 있으면 위 7구를 물려받지 않고 선택이
        // 풀린다(고쳐진 동작). 리뷰가 재현한 버그는 여기서 Some(6)이 나오는
        // 것이었다.
        assert_eq!(
            live.pitch_at_relay_line(None, 9),
            None,
            "추적 없는 투구 줄 자체에서는 carry-down이 멈춰야 한다"
        );

        // 그 줄 위(투수판 이탈, 투구 줄이 아님)와 아래(결과 요약, 투구 줄이
        // 아님)는 평소 carry-down 그대로 마지막 실제 투구(7구=인덱스 6)를
        // 물려받는다 — 이건 의도된 동작이라 바뀌면 안 된다.
        assert_eq!(
            live.pitch_at_relay_line(None, 8),
            Some(6),
            "투수판 이탈 줄은 여전히 7구를 물려받는다"
        );
        assert_eq!(
            live.pitch_at_relay_line(None, 10),
            Some(6),
            "결과 요약 줄은 추적 없는 8구를 건너뛰고 마지막 실제 투구를 물려받는다"
        );
    }

    /// 리뷰 M-4: `pitches_of`의 외래키 우선 분기는 실측 7,474건 전부에서
    /// 접두("{ballcount}구") 폴백과 결과가 100% 일치해 어떤 실데이터 테스트도
    /// 이 분기를 무력화하면 잡아내지 못한다(실증: 통째로 지워도 450+ 테스트
    /// 그대로 통과). 실 데이터로는 두 방법이 갈리지 않으므로, 두 방법이
    /// **갈리도록 합성한** 타석으로 순서를 고정한다.
    ///
    /// 같은 접두("1구")로 시작하는 텍스트가 두 줄 있고, 그중 **먼저 나오는
    /// 줄은 가짜**(ptsPitchId 없음)다. 외래키 우선이면 진짜 줄(두 번째)을
    /// 찾아야 하고, v0.18처럼 접두만으로 찾으면 순서상 먼저인 가짜 줄을
    /// 집어 버린다.
    #[test]
    fn pitches_of_prefers_the_foreign_key_match_over_an_earlier_same_prefix_decoy() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"no": 1, "inn": 1, "homeOrAway": "0", "textOptions": [
                    {"seqno": 1, "text": "1번타자 최원준", "type": 8},
                    {"seqno": 2, "text": "1구 몸에 맞는 볼(가짜, 외래키 없음)", "type": 1},
                    {"seqno": 3, "text": "1구 파울", "type": 1, "ptsPitchId": "P1"}
                ], "ptsOptions": [
                    {"ballcount": 1, "pitchId": "P1", "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]}
            ],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        let ab = &live.at_bats[0];
        assert_eq!(
            ab.pitches[0].text, "1구 파울",
            "외래키가 가리키는 진짜 줄을 집어야 한다 — 접두만 봤다면 먼저 나온 가짜 줄이 집혔을 것이다"
        );
    }

    /// batter_name_of: "N번타자 이름" 형식에서 이름만 뽑는다. 안내 자체가 없는
    /// 항목(pts만 있는 경우)은 빈 문자열로 관용 처리한다.
    #[test]
    fn at_bat_batter_name_extracts_the_name_after_the_batting_order_prefix() {
        let json = r#"{"result":{"textRelayData":{
            "currentGameState": {"ball":"0","strike":"0","out":"0","homeScore":"0","awayScore":"0"},
            "textRelays": [
                {"inn": 3, "homeOrAway": "0", "textOptions": [
                    {"seqno": 1, "text": "1구 파울", "type": 1}
                ], "ptsOptions": [
                    {"ballcount": 1, "crossPlateX": 0.1, "crossPlateY": 0.5, "topSz": 3.3, "bottomSz": 1.6, "vx0": 1.0, "vy0": 1.0, "vz0": 1.0, "stance": "R"}
                ]},
                {"inn": 3, "homeOrAway": "0", "textOptions": [
                    {"seqno": 0, "text": "3번타자 안현민", "type": 8}
                ], "ptsOptions": []}
            ],
            "lastValidMetricOption": null
        }}}"#;
        let team = |c: &str| Team {
            code: c.into(),
            name: c.into(),
        };
        let live = live_from_relay(json, team("LG"), team("KT")).unwrap();
        assert_eq!(live.at_bats.len(), 2);
        assert_eq!(live.at_bats[0].batter_name, "안현민");
        // 안내 없이 pts만 있는 항목은 이름을 알 수 없으므로 빈 문자열.
        assert_eq!(live.at_bats[1].batter_name, "");
    }
}
