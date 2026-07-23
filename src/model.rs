#[derive(Debug, Clone, PartialEq)]
pub struct Team {
    pub code: String, // KBO 내부 코드 (LG, HT ...)
    pub name: String, // 표시명 (API TeamName)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Scheduled, // 경기 전
    Live,      // 진행 중
    Final,     // 종료
    Canceled,  // 취소/우천
    Suspended, // 서스펜디드
}

impl GameStatus {
    pub fn is_live(self) -> bool {
        matches!(self, GameStatus::Live)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub id: String,
    pub start: String, // gameDateTime 원문 (표시용)
    pub status: GameStatus,
    pub status_label: String, // statusInfo (예: "9회말")
    pub home: Team,
    pub away: Team,
    pub home_score: Option<u16>,
    pub away_score: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseState {
    pub first: bool,
    pub second: bool,
    pub third: bool,
}

impl BaseState {
    pub fn runner_count(self) -> u8 {
        self.first as u8 + self.second as u8 + self.third as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Count {
    pub ball: u8,
    pub strike: u8,
    pub out: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PitchResult {
    Ball,
    StrikeLooking,
    StrikeSwinging,
    Foul,
    InPlay,
    Unknown,
}

/// 한 구의 PTS 추적 데이터.
#[derive(Debug, Clone, PartialEq)]
pub struct Pitch {
    pub order: u8,              // 타석 내 구 순번 (ballcount)
    pub plate_x: f32,           // crossPlateX (ft, 포수 시점 좌우)
    pub plate_y: f32, // 플레이트 통과 높이(ft), 투사체 운동으로 계산 (crossPlateY는 거리라 못 씀)
    pub sz_top: f32,  // topSz (타자별 존 상단)
    pub sz_bottom: f32, // bottomSz (타자별 존 하단)
    pub speed_kmh: Option<u16>, // 릴리스 속도벡터로 계산 (없으면 None)
    pub result: PitchResult,
    pub text: String, // "1구 파울" 등 원문
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveState {
    pub inning_label: String, // statusInfo/inn 조합
    pub home: Team,
    pub away: Team,
    pub home_score: u16,
    pub away_score: u16,
    pub count: Count,
    pub bases: BaseState,
    pub pitcher_name: String,
    pub batter_name: String,
    pub home_win_rate: Option<f32>,
    pub away_win_rate: Option<f32>,
    pub relay_log: Vec<String>,      // 최근 문자중계 텍스트 (오래된→최신)
    pub current_pitches: Vec<Pitch>, // 현재 타석 투구들
}

#[derive(Debug, Clone, PartialEq)]
pub struct Standing {
    pub rank: u16,
    pub team: Team,
    pub games: u16,
    pub wins: u16,
    pub losses: u16,
    pub draws: u16,
    pub win_rate: f32,
    pub game_behind: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_state_counts_runners() {
        let bases = BaseState {
            first: true,
            second: false,
            third: true,
        };
        assert_eq!(bases.runner_count(), 2);
    }

    #[test]
    fn game_status_is_live_only_when_playing() {
        assert!(GameStatus::Live.is_live());
        assert!(!GameStatus::Scheduled.is_live());
        assert!(!GameStatus::Final.is_live());
    }
}
