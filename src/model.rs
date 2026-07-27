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
    /// 선발투수(원정/홈, v0.23). 경기 **하루 전쯤** 확정되며 그 전에는 응답이 빈
    /// 문자열을 준다(실측: 이틀 뒤 경기는 `""`) — 빈 값은 화면에서 생략한다.
    pub away_starter: String,
    pub home_starter: String,
    /// 구장명("잠실"·"대구"). 예정·진행·종료 어느 상태에서도 온다.
    pub stadium: String,
    /// 중계 채널("SPOTV"). **종료된 경기에서는 빈 문자열**이다(실측) — 끝난
    /// 경기에 중계 채널은 의미가 없으니 그대로 생략한다.
    pub broadcast: String,
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

/// 문자중계 한 줄 + **그 줄이 가리키는 투구**(v0.19 연동). v0.18까지 문자중계는
/// 순수 `String`이라 아무 식별자가 없었고, 그래서 `j`/`k` 줄 커서가 하는 일은
/// 하이라이트 반전 하나뿐이었다 — 이 필드가 커서에 존재 이유를 준다.
///
/// # 왜 `pitch_idx`(인덱스)이고 `ballcount`(구 순번)가 아닌가
/// 화면의 투구 선택(`App::live_pitch_sel`)·스트라이크존·측면 뷰는 전부
/// `AtBat::pitches`의 **인덱스**로 말한다. 반면 응답의 `ballcount`는 타석 안의
/// 구 순번이라 둘이 항상 같지는 않다 — 피치클락 위반이 한 카운트를 먹고 지나간
/// 타석은 실제로 `ballcount`가 2부터 시작한다(실측 2026-07-25: KT-롯데 4회
/// 김현수, LG-한화 3회 구본혁). 순번을 그대로 인덱스로 쓰면 그런 타석에서 전부
/// 한 칸씩 어긋나므로, 여기엔 처음부터 인덱스를 담는다.
///
/// # 왜 seqno·type·시각을 안 담나
/// - `seqno`·`type`: 매칭이 응답의 명시적 외래키(`ptsPitchId`)로 끝나므로 줄
///   전체를 분류할 필요는 없다. 다만 "투구 줄인가"와 "매칭된 투구가 있는가"는
///   서로 다른 질문이다(아래 `is_pitch` 참고) — 그래서 `type` 전체가 아니라
///   그 구분 한 비트만 남긴다.
/// - 시각: 투구 줄의 시각은 이미 짝지어진 [`Pitch::time_hms`]에 있고(중복 보관은
///   두 값이 어긋날 자리를 만든다), 투구가 아닌 줄에는 응답에 시각 자체가 없다.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RelayLine {
    /// 화면에 낼 원문(응답 textOption.text 그대로 — 앱이 조립한 chrome이 아니다).
    pub text: String,
    /// 같은 at-bat `pitches` 안에서 이 줄이 서술하는 투구의 인덱스.
    /// `None` = 매칭된 투구가 없다 — 이유가 둘이다: 애초에 투구 줄이 아니거나
    /// (`is_pitch == false`), 투구 줄인데 짝을 못 찾았거나(`is_pitch == true`,
    /// 아래 참고). 두 경우를 구분하지 않으면 후자에서 carry-down이 거짓말을
    /// 한다(리뷰 v19b I-1).
    pub pitch_idx: Option<usize>,
    /// 응답이 이 줄을 투구 줄이라고 말했는가(`textOption.ptsPitchId`가 비어
    /// 있지 않다 — 값 자체가 매칭됐는지와는 별개다).
    ///
    /// `is_pitch && pitch_idx.is_none()`은 **추적 데이터 없는 진짜 투구**다
    /// (`ptsPitchId == "-1"` 센티널, 실측 2026-07-25~07 5일치 7,476줄 중 2건).
    /// 이 상태에서 위 투구를 carry-down으로 물려받으면 "8구 타격" 줄인데
    /// 화면은 "7구 파울"을 보여주는 식으로 줄과 화면이 다른 사건을 가리키게
    /// 된다 — [`LiveState::pitch_at_relay_line`]이 이 비트로 그 상황을 막는다.
    /// 투구가 아닌 줄(결과 요약 등)의 carry-down은 이 필드와 무관하게 그대로
    /// 유지된다(의도된 동작).
    pub is_pitch: bool,
    /// 그 줄의 시각 "HH:MM"(v0.25). **투구 줄에만 있다** — 응답이 시각을 싣는
    /// 곳이 `textOption.ptsPitchId`(`YYMMDD_HHMMSS`)뿐이고, 타자 등장 안내나
    /// 결과 요약 줄은 그 값이 비어 있다(실측). 초까지는 선택 투구 상세줄이
    /// 이미 보여주므로 목록에서는 분까지만 쓴다.
    pub time_hm: Option<String>,
}

impl RelayLine {
    /// 투구와 짝이 없는 줄(손으로 조립하는 테스트 상태·레거시 경로용).
    /// `is_pitch: false` — 애초에 투구 줄이 아닌 경우의 기본값이다.
    pub fn plain(text: impl Into<String>) -> Self {
        RelayLine {
            text: text.into(),
            pitch_idx: None,
            is_pitch: false,
            time_hm: None,
        }
    }
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
    pub relay_lines: Vec<RelayLine>, // 그 타석의 문자중계(오래된→최신)
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
    pub relay_log: Vec<RelayLine>,   // 최근 문자중계 (오래된→최신)
    pub current_pitches: Vec<Pitch>, // 현재 타석 투구들
    /// 다음 타순의 타자 이름(라인업 batOrder 기반, 미상이면 빈 문자열 — 표시 생략).
    pub next_batter_name: String,
    /// 이 경기에서 응답에 실려 온 타석들(오래된→최신, v0.18 "돌려보기"). 마지막
    /// 항목은 relay_log/current_pitches(현재 타석)와 내용이 일치한다 — 파서가
    /// 같은 로직을 공유하기 때문(source::naver::map의 relay_lines_of/pitches_of).
    /// 손으로 만든 LiveState(테스트 등)가 이 필드를 비워 두면 active_*() 헬퍼가
    /// relay_log/current_pitches로 자동 폴백한다(무회귀).
    pub at_bats: Vec<AtBat>,
    /// 이닝별 득점(v0.25, 오래된→최신). 연장이면 그만큼 길어진다.
    pub inning_score: Vec<InningCell>,
    /// 현재 타자의 그 경기 성적. 이름을 못 찾거나 아직 기록이 없으면 None.
    pub batter_line: Option<BatterLine>,
    /// 현재 투수의 그 경기 성적.
    pub pitcher_line: Option<PitcherLine>,
}

/// 라인스코어 한 칸. `away`·`home`은 응답 원문 그대로의 문자열이다 —
/// **`"-"`(그 반이닝을 하지 않음)를 0으로 바꾸지 않는다.** 홈팀이 이기고 있으면
/// 9회말을 치지 않는데, 그걸 0으로 찍으면 "0점 냈다"는 거짓말이 된다.
#[derive(Debug, Clone, PartialEq)]
pub struct InningCell {
    pub inning: u8,
    pub away: String,
    pub home: String,
}

/// 타자의 그 경기 성적(v0.25).
#[derive(Debug, Clone, PartialEq)]
pub struct BatterLine {
    pub hits: u16,
    pub at_bats: u16,
    pub season_avg: f32,
}

/// 투수의 그 경기 성적(v0.25).
#[derive(Debug, Clone, PartialEq)]
pub struct PitcherLine {
    pub innings: f32,
    pub hits_allowed: u16,
    pub pitches: u16,
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
    pub fn active_relay_lines(&self, sel: Option<i64>) -> &[RelayLine] {
        self.active_at_bat(sel)
            .map(|ab| ab.relay_lines.as_slice())
            .unwrap_or(&self.relay_log)
    }

    /// **문자중계 줄 → 투구**(v0.19 연동의 절반). `line`번째 줄을 보고 있을 때
    /// 존·측면에 띄울 투구의 인덱스.
    ///
    /// # 투구가 아닌 줄에 커서가 가면 (설계 판단)
    /// 그 줄 **위쪽(더 오래된 쪽)에서 가장 가까운 투구 줄**의 투구를 물려받는다
    /// ("이 사건이 벌어졌을 때 마운드에 있던 공"). 근거:
    /// - 결과 요약("박동원 : 좌익수 뒤 홈런")·주자 진루("2루주자 : 홈인")·
    ///   피치클락 위반은 **직전 투구가 만든 결과**다. 그 공을 그대로 띄우는 게
    ///   줄이 말하는 내용과 정확히 일치한다 — 홈런 줄에 커서를 두면 얻어맞은
    ///   그 공이 존에 남는다.
    /// - 타석 첫 투구보다 위에 있는 줄(타자 등장 안내·투수 교체)은 아직 아무
    ///   공도 던지지 않았으므로 `None` = 선택 해제 → 존이 그 타석 **전체**를
    ///   보여주는 개요 상태로 돌아간다.
    /// - "직전 선택을 유지"(대안)를 택하지 않은 이유: 같은 줄에 커서가 있어도
    ///   **어느 방향으로 왔는지**에 따라 다른 공이 뜬다 — 화면이 커서 위치만으로
    ///   설명되지 않는 숨은 상태를 갖게 된다. 이 규칙은 커서 위치의 순수 함수다.
    ///
    /// # 추적 없는 투구 줄에서는 carry-down을 정지한다(리뷰 v19b I-1)
    /// 커서가 **바로 그 줄**(`is_pitch == true && pitch_idx.is_none()` —
    /// `ptsPitchId == "-1"` 센티널, 추적 데이터가 없는 진짜 투구)에 있으면
    /// 위 투구를 물려받지 않고 선택을 해제한다. 물려받으면 "8구 타격" 줄에
    /// "7구"가 뜨는 식으로 줄과 화면이 다른 사건을 가리키게 된다 — 이 릴리스가
    /// 없애려던 바로 그 상태다. 커서가 **그다음** 줄(결과 요약 등, 투구 줄이
    /// 아님)로 넘어가면 이 가드는 걸리지 않고 평소처럼 더 위의 마지막 실제
    /// 투구까지 계속 물려받는다 — 그건 의도된 동작이다.
    ///
    /// 범위 밖 `line`은 마지막 줄로 낮춘다(무패닉). 줄이 하나도 없으면 `None`.
    pub fn pitch_at_relay_line(&self, sel: Option<i64>, line: usize) -> Option<usize> {
        let lines = self.active_relay_lines(sel);
        let last = lines.len().checked_sub(1)?;
        let slice = &lines[..=line.min(last)];
        if slice
            .last()
            .is_some_and(|l| l.is_pitch && l.pitch_idx.is_none())
        {
            return None; // 추적 없는 투구 줄 — 다른 공을 이 줄의 공인 척 보여주지 않는다
        }
        slice.iter().rev().find_map(|l| l.pitch_idx)
    }

    /// **투구 → 문자중계 줄**(연동의 나머지 절반). `pitch`번째 투구를 서술하는
    /// 줄의 인덱스. 짝이 없으면(연동 불가 응답·손 조립 상태) `None` — 호출부는
    /// 커서를 세우지 않고 v0.18과 똑같이 꼬리 뷰로 남는다(우아한 저하).
    pub fn relay_line_of_pitch(&self, sel: Option<i64>, pitch: usize) -> Option<usize> {
        self.active_relay_lines(sel)
            .iter()
            .position(|l| l.pitch_idx == Some(pitch))
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
    /// 최근 5경기 결과 "WWLDW"(v0.23). **오른쪽이 최신**이다 — 10팀 전부에서
    /// `continuousGameResult`와 대조해 확정했다(한화 `WWWLW`=1승, 롯데
    /// `LLLLW`=1승, KT `WDWWL`=1패). 결측이면 빈 문자열.
    pub last_five: String,
    /// 연속 기록 "3승"·"1패"(응답 원문 그대로). **무승부는 연속을 끊지 않고
    /// 건너뛴다** — SSG `WWWWD`가 4승, NC `WWLLD`가 2패인 게 그 증거다(KBO 관례).
    /// 그래서 이 값을 last_five에서 계산하지 않고 응답 값을 그대로 쓴다.
    pub streak: String,
    /// 팀 시즌 성적(v0.24). 순위 응답이 팀마다 64개 필드를 주는데 순위·승패만
    /// 쓰고 있었다. 별도 구조체로 묶은 이유: v0.23에서 `Game`에 4필드를 늘렸을 때
    /// 손으로 만든 테스트 픽스처 24곳이 한꺼번에 깨졌다 — 여기 필드가 더 늘어도
    /// `Standing` 리터럴은 이 한 줄만 신경 쓰면 된다.
    pub stats: TeamStats,
}

/// 팀 시즌 성적. 전부 응답 원값이며 결측은 0이다 — 0과 "기록 없음"을 구분할 수
/// 없으므로 화면은 `Standing::games == 0`(경기 전)일 때 아예 보여주지 않는다.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamStats {
    // 타격
    pub avg: f32,
    pub obp: f32,
    pub slg: f32,
    pub ops: f32,
    pub runs: u16,
    pub rbi: u16,
    pub homers: u16,
    pub steals: u16,
    // 투구·수비
    pub era: f32,
    pub whip: f32,
    pub quality_starts: u16,
    pub saves: u16,
    pub holds: u16,
    pub strikeouts: u16,
    pub homers_allowed: u16,
    pub errors: u16,
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
            inning_score: Vec::new(),
            batter_line: None,
            pitcher_line: None,
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
        s.relay_log = vec![RelayLine::plain("레거시 문자중계")];
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
                relay_lines: vec![RelayLine::plain("old line")],
                pitches: vec![pitch(1)],
            },
            AtBat {
                seq: 11,
                batter_name: "new".into(),
                inning_label: "T2".into(),
                relay_lines: vec![RelayLine::plain("new line")],
                pitches: vec![pitch(1), pitch(2)],
            },
        ];
        assert_eq!(s.active_at_bat(None).unwrap().batter_name, "new");
        assert_eq!(s.active_pitches(None).len(), 2);
        assert_eq!(s.active_relay_lines(None), [RelayLine::plain("new line")]);
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

    // ---- v0.19: 문자중계 줄 ↔ 투구 (양방향 조회) ----

    /// 실제 타석 모양: 안내 → 1구 → 2구 → 결과 요약 → 주자 진루.
    fn linked_at_bat_state() -> LiveState {
        let mut s = bare_live_state();
        s.at_bats = vec![AtBat {
            seq: 1,
            batter_name: "타자".into(),
            inning_label: "T1".into(),
            relay_lines: vec![
                RelayLine::plain("1번타자 최원준"),
                RelayLine {
                    text: "1구 볼".into(),
                    pitch_idx: Some(0),
                    is_pitch: true,
                    time_hm: None,
                },
                RelayLine {
                    text: "2구 타격".into(),
                    pitch_idx: Some(1),
                    is_pitch: true,
                    time_hm: None,
                },
                RelayLine::plain("최원준 : 좌익수 뒤 홈런"),
                RelayLine::plain("1루주자 김현수 : 홈인"),
            ],
            pitches: vec![pitch(1), pitch(2)],
        }];
        s
    }

    /// 투구 줄에 커서를 두면 그 투구가 나온다(연동의 기본 계약).
    #[test]
    fn a_pitch_line_resolves_to_its_own_pitch() {
        let s = linked_at_bat_state();
        assert_eq!(s.pitch_at_relay_line(None, 1), Some(0));
        assert_eq!(s.pitch_at_relay_line(None, 2), Some(1));
    }

    /// 투구가 **아닌** 줄은 위쪽에서 가장 가까운 투구를 물려받는다 — 결과 요약과
    /// 주자 진루는 마지막 투구(2구)가 만든 사건이므로 그 공이 존에 남아야 한다.
    /// 이 규칙이 없으면 홈런 줄을 읽는 동안 존이 비거나(선택 해제) 방금 전
    /// 무관한 공이 남는다.
    #[test]
    fn a_non_pitch_line_carries_down_the_nearest_pitch_above_it() {
        let s = linked_at_bat_state();
        assert_eq!(s.pitch_at_relay_line(None, 3), Some(1), "결과 요약 → 2구");
        assert_eq!(s.pitch_at_relay_line(None, 4), Some(1), "주자 진루 → 2구");
    }

    /// 첫 투구보다 위에 있는 줄(타자 등장 안내)은 아직 던진 공이 없으므로
    /// 선택 해제 — 존이 그 타석 전체를 보여주는 개요 상태로 돌아간다.
    #[test]
    fn a_line_above_the_first_pitch_selects_no_pitch_at_all() {
        let s = linked_at_bat_state();
        assert_eq!(s.pitch_at_relay_line(None, 0), None);
    }

    /// 범위 밖 줄 번호는 마지막 줄로 낮추고, 줄이 아예 없으면 None(무패닉).
    #[test]
    fn pitch_at_relay_line_is_lenient_about_out_of_range_and_empty_input() {
        let s = linked_at_bat_state();
        assert_eq!(s.pitch_at_relay_line(None, 9999), Some(1), "마지막 줄 취급");
        assert_eq!(bare_live_state().pitch_at_relay_line(None, 0), None);
    }

    /// 반대 방향: 투구 → 그 투구를 서술하는 줄. 짝이 없는 투구는 None이라
    /// 호출부가 커서를 세우지 않는다(우아한 저하).
    #[test]
    fn relay_line_of_pitch_finds_the_line_that_describes_it() {
        let s = linked_at_bat_state();
        assert_eq!(s.relay_line_of_pitch(None, 0), Some(1));
        assert_eq!(s.relay_line_of_pitch(None, 1), Some(2));
        assert_eq!(s.relay_line_of_pitch(None, 7), None, "없는 투구");
        assert_eq!(
            bare_live_state().relay_line_of_pitch(None, 0),
            None,
            "줄 정보가 없는 상태(레거시 폴백)에선 짝을 못 찾는다"
        );
    }

    /// 두 조회는 **보고 있는 타석**(sel) 안에서만 유효해야 한다 — 과거 타석을
    /// 돌려보는 중에 최신 타석의 줄/투구를 섞어 보면 존과 문자중계가 서로 다른
    /// 타석을 말한다.
    #[test]
    fn the_relay_pitch_lookups_stay_inside_the_at_bat_being_viewed() {
        let mut s = linked_at_bat_state();
        let mut newest = s.at_bats[0].clone();
        newest.seq = 2;
        newest.relay_lines = vec![RelayLine {
            text: "1구 헛스윙".into(),
            pitch_idx: Some(0),
            is_pitch: true,
            time_hm: None,
        }];
        newest.pitches = vec![pitch(1)];
        s.at_bats.push(newest);

        // 최신(sel=None): 줄이 하나뿐이라 0번 줄이 곧 0번 투구.
        assert_eq!(s.pitch_at_relay_line(None, 0), Some(0));
        assert_eq!(s.relay_line_of_pitch(None, 1), None);
        // 과거 타석(seq=1): 0번 줄은 안내라 선택 없음, 1번 투구는 2번째 줄.
        assert_eq!(s.pitch_at_relay_line(Some(1), 0), None);
        assert_eq!(s.relay_line_of_pitch(Some(1), 1), Some(2));
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
