use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
pub struct ApiEnvelope<T> {
    pub result: Option<T>,
}

/// `#[serde(default)]`는 키가 아예 없을 때만 기본값을 채운다 — 키는 있지만 값이
/// 명시적 `null`이면(네이버 응답이 실제로 그렇게 준다) 여전히 실패한다.
/// non-Option 필드에 `#[serde(default, deserialize_with = "null_as_default")]`로
/// 붙이면 누락/명시적 null 두 경우 모두 Default로 완만히 처리된다.
fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// null_as_default는 null/누락만 완화할 뿐, 값이 존재하지만 엄격한 정수 타입의
/// 범위를 벗어나거나(예: 음수 sentinel) 타입이 안 맞으면(문자열 등) 여전히
/// Err를 내고, 그 Err가 상위 Vec(경기 배열, ptsOptions 등) 전체 파싱까지
/// 실패시킨다 — 문자열 필드(ball/strike/out)에 이미 적용한 "레코드 하나
/// 때문에 배열 전체가 죽으면 안 된다"는 관용적 파싱 원칙을 정수 필드에도
/// 적용한다. serde_json::Value로 받아 범위/타입이 안 맞으면 그 필드만
/// 기본값으로 내린다(null/누락도 같은 경로로 처리되므로 null_as_default를
/// 대체한다).
fn lenient_int<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<i64> + Default,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    // as_i64()는 1.0/3e0처럼 소수점·지수 표기로 온 정수값에는 None을 반환한다 —
    // 네이버가 정수 필드를 그런 형태로 내려주면(직렬화기 드리프트) 유효한 값이
    // 조용히 기본값으로 뭉개진다. lenient_float과 동일하게 as_f64()로 한 번 더
    // 시도해 그런 값도 살려낸다.
    Ok(
        v.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .and_then(|n| T::try_from(n).ok())
            .unwrap_or_default(),
    )
}

/// lenient_int의 Option<T> 버전. homeTeamScore/awayTeamScore처럼 "값 없음"이
/// 의미를 갖는 필드용 — 범위를 벗어나거나 타입이 안 맞는 값도 null/누락과
/// 동일하게 None으로 완화한다.
fn lenient_int_opt<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<i64>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    // lenient_int와 동일하게 소수점/지수 표기 정수도 as_f64() 폴백으로 살려낸다.
    Ok(
        v.and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .and_then(|n| T::try_from(n).ok()),
    )
}

/// lenient_int의 f32 버전. PTS 좌표/속도 벡터 필드용 — 값이 있지만 숫자가
/// 아니면(문자열 등) null_as_default처럼 배열/전체 파싱을 실패시키는 대신 그
/// 필드만 기본값(0.0)으로 완화한다.
fn lenient_float<'de, D>(deserializer: D) -> std::result::Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(v.and_then(|v| v.as_f64()).map(|n| n as f32).unwrap_or(0.0))
}

/// lenient_float의 Option<f32> 버전. home/awayTeamWinRate처럼 "값 없음"이
/// 의미를 갖는 필드용 — 타입이 안 맞는 값도 null/누락과 동일하게 None으로
/// 완화한다.
fn lenient_float_opt<'de, D>(deserializer: D) -> std::result::Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(v.and_then(|v| v.as_f64()).map(|n| n as f32))
}

/// null_as_default의 String 버전. 값이 존재하지만 문자열이 아니면(숫자/불리언 등)
/// null_as_default는 여전히 Err를 내 상위 배열(경기 목록 등) 전체를 실패시킨다 —
/// 그 필드만 기본값("")으로 완화한다.
fn lenient_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(v.and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default())
}

/// lenient_string의 Option<String> 버전. gameId/homeTeamCode처럼 "값 없음"이
/// 의미를 갖는 식별 필드용 — 타입이 안 맞는 값도 null/누락과 동일하게 None으로
/// 완화한다(games_from_schedule은 이미 None을 "이 레코드만 스킵"으로 처리한다).
fn lenient_string_opt<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(v.and_then(|v| v.as_str().map(str::to_string)))
}

/// null_as_default의 bool 버전. 값이 존재하지만 불리언이 아니면 그 필드만
/// 기본값(false)으로 완화한다.
fn lenient_bool<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(v.and_then(|v| v.as_bool()).unwrap_or(false))
}

#[derive(Deserialize)]
pub struct ScheduleResult {
    #[serde(default, deserialize_with = "null_as_default")]
    pub games: Vec<ScheduleGame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleGame {
    // 식별 필드(id/코드/이름)는 필수로 두면 하루치 스케줄 배열 중 단 하나의
    // 경기라도 필드가 빠지면 serde가 배열 전체 파싱을 실패시킨다(관용적 파싱
    // 원칙 위반). Option + default로 완화하고, 실사용 불가한 레코드는
    // games_from_schedule에서 걸러낸다.
    #[serde(default, deserialize_with = "lenient_string_opt")]
    pub game_id: Option<String>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub game_date_time: String,
    #[serde(default, deserialize_with = "lenient_string_opt")]
    pub home_team_code: Option<String>,
    #[serde(default, deserialize_with = "lenient_string_opt")]
    pub home_team_name: Option<String>,
    #[serde(default, deserialize_with = "lenient_string_opt")]
    pub away_team_code: Option<String>,
    #[serde(default, deserialize_with = "lenient_string_opt")]
    pub away_team_name: Option<String>,
    #[serde(default, deserialize_with = "lenient_int_opt")]
    pub home_team_score: Option<u16>,
    #[serde(default, deserialize_with = "lenient_int_opt")]
    pub away_team_score: Option<u16>,
    #[serde(default, deserialize_with = "lenient_string")]
    pub status_code: String, // "RESULT" | "BEFORE" | 진행중 값
    #[serde(default, deserialize_with = "lenient_string")]
    pub status_info: String, // "9회말"
    #[serde(default, deserialize_with = "lenient_bool")]
    pub cancel: bool,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub suspended: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingsResult {
    #[serde(default, deserialize_with = "null_as_default")]
    pub season_team_stats: Vec<TeamStat>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStat {
    // 네이버 응답은 시즌 초반/신규 팀 등에서 형제 필드(wcRanking, division 등)를
    // 실제로 null로 내려주는 게 fixture에서도 확인된다 — 숫자 필드에 명시적
    // null이 와도 그 필드만 기본값으로 죽고 팀 전체 파싱이 죽지 않게 한다.
    #[serde(default, deserialize_with = "lenient_int")]
    pub ranking: u16,
    #[serde(default, deserialize_with = "lenient_string")]
    pub team_id: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub team_name: String,
    #[serde(default, deserialize_with = "lenient_int")]
    pub game_count: u16,
    #[serde(default, deserialize_with = "lenient_int")]
    pub win_game_count: u16,
    #[serde(default, deserialize_with = "lenient_int")]
    pub lose_game_count: u16,
    #[serde(default, deserialize_with = "lenient_int")]
    pub drawn_game_count: u16,
    #[serde(default, deserialize_with = "lenient_float")]
    pub wra: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub game_behind: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsResult {
    #[serde(default, deserialize_with = "null_as_default")]
    pub news_list: Vec<NewsArticle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsArticle {
    #[serde(default, deserialize_with = "lenient_string")]
    pub title: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub source_name: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub oid: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub aid: String,
    /// 약 100자 요약(HTML 조각 포함 가능) — map.rs가 이걸 발췌(NewsItem.summary)로
    /// 변환한다(HTML 제거 + EXCERPT_CHARS 상한). 결측 시 빈 문자열(관용).
    #[serde(default, deserialize_with = "lenient_string")]
    pub sub_content: String,
}

#[derive(Deserialize)]
pub struct RelayResult {
    #[serde(rename = "textRelayData")]
    pub text_relay_data: Option<TextRelayData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRelayData {
    pub current_game_state: Option<CurrentGameState>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub text_relays: Vec<TextRelay>,
    pub last_valid_metric_option: Option<MetricOption>,
    #[serde(default)]
    pub home_lineup: Option<Lineup>,
    #[serde(default)]
    pub away_lineup: Option<Lineup>,
}

// 값이 문자열("3")로 오므로 String으로 받고 변환은 map에서.
// 필드 하나가 명시적 null이어도(예: ball) 그 값만 빈 문자열로 낮아지고 5s마다
// 도는 라이브 폴링 전체가 죽지 않도록 null_as_default를 적용한다.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CurrentGameState {
    #[serde(default, deserialize_with = "lenient_string")]
    pub home_score: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub away_score: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub strike: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub ball: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub out: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub base1: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub base2: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub base3: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub pitcher: String, // 선수 id
    #[serde(default, deserialize_with = "lenient_string")]
    pub batter: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRelay {
    #[serde(default, deserialize_with = "lenient_int")]
    pub inn: i32,
    /// "0" = 초(away 공격), "1" = 말(home 공격). map.rs가 inning_label(예: "T9"/"B9")을
    /// 만드는 데 쓴다.
    #[serde(default, deserialize_with = "lenient_string")]
    pub home_or_away: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub text_options: Vec<TextOption>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub pts_options: Vec<PtsOption>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOption {
    #[serde(default, deserialize_with = "lenient_int")]
    pub seqno: i64,
    #[serde(default, deserialize_with = "lenient_string")]
    pub text: String,
    #[serde(default, deserialize_with = "lenient_int")]
    pub r#type: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtsOption {
    #[serde(default, deserialize_with = "lenient_int")]
    pub ballcount: u8,
    #[serde(default, deserialize_with = "lenient_float")]
    pub cross_plate_x: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub cross_plate_y: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub top_sz: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub bottom_sz: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub vx0: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub vy0: f32,
    #[serde(default, deserialize_with = "lenient_float")]
    pub vz0: f32,
    /// 릴리스 위치(y, ft) — 홈플레이트로부터의 y거리(추적 시작점, 대략 50~55ft).
    /// plate_height()가 crossPlateY(플레이트 통과 y거리)까지의 낙하 시간을 구할 때 쓴다.
    #[serde(default, deserialize_with = "lenient_float")]
    pub y0: f32,
    /// 릴리스 위치(z, ft) — 투구 높이의 시작점.
    #[serde(default, deserialize_with = "lenient_float")]
    pub z0: f32,
    /// y축 가속도(ft/s^2) — plate_height()가 통과 시각을 구하는 이차식의 계수.
    #[serde(default, deserialize_with = "lenient_float")]
    pub ay: f32,
    /// z축 가속도(ft/s^2, 중력+매그너스 효과 포함) — plate_height()가 통과 높이를 구한다.
    #[serde(default, deserialize_with = "lenient_float")]
    pub az: f32,
    #[serde(default, deserialize_with = "lenient_string")]
    pub stance: String,
    /// "YYMMDD_HHMMSS" 형식의 투구 식별자 — 실제 투구 시각의 유일한 출처.
    #[serde(default, deserialize_with = "lenient_string")]
    pub pitch_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricOption {
    #[serde(default, deserialize_with = "lenient_float_opt")]
    pub home_team_win_rate: Option<f32>,
    #[serde(default, deserialize_with = "lenient_float_opt")]
    pub away_team_win_rate: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lineup {
    #[serde(default, deserialize_with = "null_as_default")]
    pub batter: Vec<Player>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub pitcher: Vec<Player>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    #[serde(default, deserialize_with = "lenient_string")]
    pub pcode: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// 타순(1~9). 교체 선수는 같은 batOrder를 공유한다. 0이면 미상.
    #[serde(default, deserialize_with = "lenient_int")]
    pub bat_order: u8,
}
