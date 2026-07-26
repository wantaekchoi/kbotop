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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PitchResult {
    Ball,
    StrikeLooking,
    StrikeSwinging,
    Foul,
    InPlay,
    #[default]
    Unknown,
}

/// 한 구의 PTS 추적 데이터.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pitch {
    pub order: u8,              // 타석 내 구 순번 (ballcount)
    pub plate_x: f32,           // crossPlateX (ft, 포수 시점 좌우)
    pub plate_y: f32, // 플레이트 통과 높이(ft), 투사체 운동으로 계산 (crossPlateY는 거리라 못 씀)
    pub sz_top: f32,  // topSz (타자별 존 상단)
    pub sz_bottom: f32, // bottomSz (타자별 존 하단)
    pub speed_kmh: Option<u16>, // 릴리스 속도벡터로 계산 (없으면 None)
    pub result: PitchResult,
    pub text: String,             // "1구 파울" 등 원문
    pub time_hms: Option<String>, // pitchId(YYMMDD_HHMMSS)에서 파싱한 "HH:MM:SS"
    pub plate_t: f32,             // 릴리스→플레이트 통과 시간(s), 궤적 샘플링용(미상 0)
    // 측면 뷰 궤적 재계산용 릴리스 파라미터(ft, ft/s, ft/s^2)
    pub y0: f32,
    pub vy0: f32,
    pub ay: f32,
    pub z0: f32,
    pub vz0: f32,
    pub az: f32,
}

/// 한 타석의 문자중계·투구 기록(v0.18 "돌려보기"). Naver 응답은 textRelays를
/// 최신 순(내림차순)으로 내려주지만, `LiveState.at_bats`에 담기는 시점엔 이미
/// source::naver::map::live_from_relay가 오래된→최신으로 뒤집어 놓는다.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AtBat {
    /// 응답의 textRelay `no` — 경기 전체에 걸친 타석 시퀀스 번호. 돌려보기 선택은
    /// 인덱스가 아니라 이 값으로 고정한다(아래 active_at_bat 참고).
    pub seq: i64,
    pub batter_name: String, // 타자 등장 안내(type==8)에서 추출, 없으면 빈 문자열
    pub inning_label: String, // 그 타석이 속한 이닝("T9"/"B9" 등, LiveState.inning_label과 동일 규칙)
    pub relay_lines: Vec<String>, // 그 타석의 문자중계(오래된→최신)
    pub pitches: Vec<Pitch>,  // 그 타석의 투구(ptsOptions)
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
    /// 다음 타순의 타자 이름(라인업 batOrder 기반, 미상이면 빈 문자열 — 표시 생략).
    pub next_batter_name: String,
    /// 이 경기에서 응답에 실려 온 타석들(오래된→최신, v0.18 "돌려보기"). 마지막
    /// 항목은 relay_log/current_pitches(현재 타석)와 내용이 일치한다 — 파서가
    /// 같은 로직을 공유하기 때문(source::naver::map의 relay_lines_of/pitches_of).
    /// 손으로 만든 LiveState(테스트 등)가 이 필드를 비워 두면 active_*() 헬퍼가
    /// relay_log/current_pitches로 자동 폴백한다(무회귀).
    pub at_bats: Vec<AtBat>,
}

impl LiveState {
    /// `sel`이 가리키는 at-bat을 반환한다. None이면 최신(마지막) 타석 — "선택 없음 =
    /// 최신을 본다"는 라이브 추종의 기본값. `at_bats`가 비어 있으면 None(호출부가
    /// relay_log/current_pitches로 폴백).
    ///
    /// `sel`은 인덱스가 아니라 [`AtBat::seq`](AtBat::seq)다. 중계 응답은 **현재 이닝만**
    /// 담으므로(실측: `/relay`는 마지막 이닝만, 과거는 `?inning=N`으로 따로 받아야 한다)
    /// 이닝이 넘어가면 `at_bats`가 통째로 갈린다 — 인덱스로 고정하면 같은 자리가 다른
    /// 타석을 가리켜 사용자가 읽던 위치가 조용히 어긋난다. 번호로 찾으면 그런 착시가
    /// 생기지 않고, 그 타석이 응답에서 사라졌다는 사실도 알 수 있다(App::apply가
    /// 그때 라이브로 되돌린다).
    ///
    /// 그럼에도 찾지 못하면 최신 타석으로 낮춘다 — 렌더 경로는 패닉도 빈 화면도 낼 수 없다.
    pub fn active_at_bat(&self, sel: Option<i64>) -> Option<&AtBat> {
        match sel {
            Some(seq) => self
                .at_bats
                .iter()
                .find(|ab| ab.seq == seq)
                .or_else(|| self.at_bats.last()),
            None => self.at_bats.last(),
        }
    }

    /// `seq`가 지금 응답에 남아 있는지. App이 폴링 갱신 때 "보던 타석이 사라졌는지"를
    /// 판정하는 데 쓴다.
    pub fn has_at_bat(&self, seq: i64) -> bool {
        self.at_bats.iter().any(|ab| ab.seq == seq)
    }

    /// 화면에 그릴 활성 투구 목록. at_bats가 있으면 그 타석 것, 없으면(구버전
    /// 손 조립 상태 등) current_pitches로 무회귀 폴백.
    pub fn active_pitches(&self, sel: Option<i64>) -> &[Pitch] {
        self.active_at_bat(sel)
            .map(|ab| ab.pitches.as_slice())
            .unwrap_or(&self.current_pitches)
    }

    /// 화면에 그릴 활성 문자중계 줄. at_bats가 있으면 그 타석 것, 없으면
    /// relay_log로 무회귀 폴백.
    pub fn active_relay_lines(&self, sel: Option<i64>) -> &[String] {
        self.active_at_bat(sel)
            .map(|ab| ab.relay_lines.as_slice())
            .unwrap_or(&self.relay_log)
    }

    /// `at_bats`의 seq가 실제로 전부 유일한지(리뷰 M-2). 응답의 textRelay
    /// `no`가 결측이면 `lenient_int` 관용 파싱이 기본값 0으로 채우므로, 여러
    /// 항목이 동시에 seq==0이 될 수 있다 — 그러면 `[`/`]`(App::on_key)가 쓰는
    /// `position(|ab| ab.seq == seq)`가 항상 **첫 일치 항목**만 찾아 돌려보기가
    /// 그 자리에 갇히고(실측: `]`를 아무리 눌러도 라이브로 복귀 불가) `[`도
    /// 더 이전으로 못 간다. 유일성이 깨졌으면 App이 되감기 네비게이션 자체를
    /// 비활성화한다 — 틀린 자리에 갇히는 것보다 기능을 안 쓰는 편이 안전하다.
    pub fn has_unique_at_bat_seqs(&self) -> bool {
        let mut seqs: Vec<i64> = self.at_bats.iter().map(|ab| ab.seq).collect();
        seqs.sort_unstable();
        seqs.windows(2).all(|w| w[0] != w[1])
    }
}

/// KBO 뉴스 헤드라인 한 건(하단 티커·인앱 오버레이용).
#[derive(Debug, Clone, PartialEq)]
pub struct NewsItem {
    pub title: String,
    pub source: String, // 언론사명(출처 표시용, 결측 시 빈 문자열)
    pub url: String,    // 원문 링크. 빈 값=링크 없음 — 열기 생략.
    /// 목록·오버레이에 보여줄 발췌. HTML 제거·상한(EXCERPT_CHARS) 적용 후 저장하며,
    /// 전문은 어떤 경로로도 담지 않는다(저작권). 결측 시 빈 문자열.
    pub summary: String,
    /// 정렬용 정규화 발행시각 "YYYYMMDDHHMMSS". 여러 피드를 합칠 때 매체별로
    /// 뭉치지 않게 하는 유일한 키다. 해석 실패·결측 시 빈 문자열(정렬 뒤로).
    pub published: String,
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

    fn team(c: &str) -> Team {
        Team {
            code: c.into(),
            name: c.into(),
        }
    }

    fn bare_live_state() -> LiveState {
        LiveState {
            inning_label: String::new(),
            home: team("LG"),
            away: team("KT"),
            home_score: 0,
            away_score: 0,
            count: Count {
                ball: 0,
                strike: 0,
                out: 0,
            },
            bases: BaseState {
                first: false,
                second: false,
                third: false,
            },
            pitcher_name: String::new(),
            batter_name: String::new(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![],
            next_batter_name: String::new(),
            at_bats: vec![],
        }
    }

    fn pitch(order: u8) -> Pitch {
        Pitch {
            order,
            ..Default::default()
        }
    }

    /// at_bats가 비어 있으면(구버전 손 조립 상태 등) active_*()는 relay_log/
    /// current_pitches로 무회귀 폴백해야 한다 — v0.18 이전에 만들어진 상태를
    /// 다루는 코드 경로가 이 필드를 채우지 않아도 패닉·빈 결과 없이 동작해야 함.
    #[test]
    fn active_helpers_fall_back_to_legacy_fields_when_at_bats_is_empty() {
        let mut s = bare_live_state();
        s.relay_log = vec!["레거시 문자중계".into()];
        s.current_pitches = vec![pitch(1), pitch(2)];
        assert!(s.active_at_bat(None).is_none());
        assert_eq!(s.active_pitches(None), s.current_pitches.as_slice());
        assert_eq!(s.active_relay_lines(None), s.relay_log.as_slice());
        // sel이 Some이어도(at_bats가 비어 있으니 의미 없음) 동일하게 폴백.
        assert_eq!(s.active_pitches(Some(0)), s.current_pitches.as_slice());
    }

    /// sel=None은 항상 최신(마지막) at-bat을 가리킨다 — "선택 없음 = 라이브"라는
    /// 이 기능의 기본 계약.
    #[test]
    fn active_at_bat_with_no_selection_resolves_to_the_latest() {
        let mut s = bare_live_state();
        s.at_bats = vec![
            AtBat {
                seq: 10,
                batter_name: "old".into(),
                inning_label: "T1".into(),
                relay_lines: vec!["old line".into()],
                pitches: vec![pitch(1)],
            },
            AtBat {
                seq: 11,
                batter_name: "new".into(),
                inning_label: "T2".into(),
                relay_lines: vec!["new line".into()],
                pitches: vec![pitch(1), pitch(2)],
            },
        ];
        assert_eq!(s.active_at_bat(None).unwrap().batter_name, "new");
        assert_eq!(s.active_pitches(None).len(), 2);
        assert_eq!(s.active_relay_lines(None), ["new line".to_string()]);
    }

    /// sel=Some(seq)는 그 번호의 과거 타석을 가리킨다 — 자리(인덱스)가 아니라
    /// 타석 자체를 지목하므로, 앞쪽 항목이 잘려 나가도 같은 타석이 나온다.
    #[test]
    fn active_at_bat_with_a_selection_resolves_to_that_sequence_number() {
        let mut s = bare_live_state();
        s.at_bats = vec![
            AtBat {
                seq: 10,
                batter_name: "old".into(),
                inning_label: "T1".into(),
                relay_lines: vec![],
                pitches: vec![],
            },
            AtBat {
                seq: 11,
                batter_name: "new".into(),
                inning_label: "T2".into(),
                relay_lines: vec![],
                pitches: vec![],
            },
        ];
        assert_eq!(s.active_at_bat(Some(10)).unwrap().batter_name, "old");

        // 같은 선택이 인덱스였다면 "old"가 사라진 뒤 0번은 "new"가 됐을 것이다.
        // 번호로 고정하므로 그런 착시가 없다 — 사라진 건 사라진 것으로 드러난다.
        s.at_bats.remove(0);
        assert!(!s.has_at_bat(10));
    }

    /// 응답에 없는 번호(예: 이닝이 넘어가 배열이 통째로 갈린 뒤 남은 stale 선택)는
    /// 패닉하지 않고 최신 항목으로 낮아진다 — 무패닉 제약. App::apply가 이 상황을
    /// 감지해 선택 자체를 라이브로 되돌리므로 여기 폴백은 마지막 안전망이다.
    #[test]
    fn active_at_bat_falls_back_to_latest_when_the_sequence_is_gone() {
        let mut s = bare_live_state();
        s.at_bats = vec![AtBat {
            seq: 7,
            batter_name: "only".into(),
            inning_label: "T1".into(),
            relay_lines: vec![],
            pitches: vec![],
        }];
        assert_eq!(s.active_at_bat(Some(99)).unwrap().batter_name, "only");
        assert!(!s.has_at_bat(99));
        assert!(s.has_at_bat(7));
    }

    /// M-2: seq가 실제로 전부 다르면 유일하다고 판정한다(무회귀 — 정상 응답의
    /// 흔한 케이스).
    #[test]
    fn has_unique_at_bat_seqs_is_true_when_all_seqs_differ() {
        let mut s = bare_live_state();
        s.at_bats = vec![
            AtBat {
                seq: 86,
                ..Default::default()
            },
            AtBat {
                seq: 87,
                ..Default::default()
            },
        ];
        assert!(s.has_unique_at_bat_seqs());
    }

    /// M-2: `no`가 결측이면(lenient_int 관용 파싱) 여러 at-bat이 전부 seq==0
    /// 으로 뭉개질 수 있다 — 이때는 유일하지 않다고 판정해야 App이 되감기
    /// 네비게이션을 비활성화할 수 있다(안 그러면 position()이 항상 첫 일치
    /// 항목만 찾아 `]`로도 라이브 복귀가 불가능해진다 — 실측).
    #[test]
    fn has_unique_at_bat_seqs_is_false_when_no_is_missing_and_seqs_collide() {
        let mut s = bare_live_state();
        s.at_bats = vec![
            AtBat {
                seq: 0,
                ..Default::default()
            },
            AtBat {
                seq: 0,
                ..Default::default()
            },
            AtBat {
                seq: 0,
                ..Default::default()
            },
        ];
        assert!(!s.has_unique_at_bat_seqs());
    }

    /// 경계: at-bat이 0개·1개면 자명하게 유일하다(비교할 짝이 없음).
    #[test]
    fn has_unique_at_bat_seqs_is_true_for_zero_or_one_at_bats() {
        let mut s = bare_live_state();
        assert!(s.has_unique_at_bat_seqs());
        s.at_bats = vec![AtBat {
            seq: 0,
            ..Default::default()
        }];
        assert!(s.has_unique_at_bat_seqs());
    }
}
