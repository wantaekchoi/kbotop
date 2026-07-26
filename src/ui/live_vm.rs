//! 라이브 화면의 **표현 상태**(ViewModel) — "무엇을 보여줄지"는 전부 여기서
//! 정하고, [`super::live`]는 그 결과를 받아 **그리기만** 한다. MVVM의 ViewModel /
//! MVP의 Presenter / Elm의 view model — 이름은 달라도 경계는 하나다:
//! **결정은 여기서, 그리기는 저기서.**
//!
//! # 왜 갈랐나 (v0.18에서 치른 대가)
//! - 계산이 렌더 함수 안에 있으니 검증이 "화면 문자열에 이 글자가 있나"로만
//!   가능했다 — 되감기 기능을 통째로 무력화해도 396개 테스트가 전부 통과했다.
//! - "과거 타석엔 그 타석 값만 보여준다"는 **한 규칙**이 타자 축과 이닝 축에
//!   흩어져, 같은 결함을 두 번 고쳤다(`4912944` → 최종 리뷰 I-1). 이제 그 규칙은
//!   [`LiveVm::from_app`]의 `past_at_bat` 한 갈래에서만 갈린다.
//!
//! # 폭(width) 의존 결정을 어디에 뒀나
//! 폭은 렌더 시점에만 알 수 있는 **사실**이지만, "폭이 모자라면 무엇부터
//! 버리는가"는 **결정**이다. 그래서 폭을 인자로 받는 순수 메서드
//! ([`LiveVm::detail_line`] · [`PitchLine::text`] · [`LiveVm::show_strike_zone`])를
//! 여기에 두고, 렌더는 자기가 유일하게 아는 값(폭)을 넘겨 결과를 그리기만
//! 한다. 렌더에는 판단이 남지 않고, 폭 정책은 터미널 없이 단위 테스트된다.
//!
//! # 다음 작업(문자중계 ↔ 투구 뷰 연동)을 감안한 지점
//! 지금은 [`LiveVm::selected_pitch`]와 [`LiveVm::relay_cursor`]가 각각 `App`의
//! 다른 필드에서 온다. 연동이 들어오면 둘은 "지금 보고 있는 사건" 하나의 두
//! 투영이 되는데, **두 선택의 유도는 이 파일의 [`LiveVm::from_app`] 안에서만
//! 바뀌면 된다** — 렌더는 이미 `app.live_pitch_sel`·`app.live_relay_cursor`를
//! 직접 읽지 않고 이 두 필드만 보기 때문이다.
//!
//! ★ 정정(v19a 리뷰 I-2): "바뀌는 곳은 `from_app` 하나뿐"이라고 하면 과장이다.
//! `RelayLine{seqno, kind, text, pitch_order, time_hms}` 같은 구조화된 줄이
//! 들어오면 [`LiveVm::relay_rows`]를 만드는 방식(지금은 단순 불릿 프리픽스)이
//! 반드시 바뀐다 — 다만 그 변경도 이 파일 안(`from_app`과 그 옆의 행 포맷
//! 헬퍼)에 갇힌다. [`super::live::render_relay`]는 이미 **완성된 문자열과
//! 이미 클램프된 커서**만 받으므로(v19a 리뷰 I-1로 옮김) 줄의 표현이 바뀌어도
//! 렌더는 손댈 필요가 없다 — 정확한 불변은 "연동이 들어와도 `live.rs`는 안
//! 바뀐다"이지 "`from_app` 하나만 바뀐다"가 아니다(두 문장은 다르다: 전자는
//! 렌더 파일에 대한 약속이고 후자는 이 파일 **안에서도** 아무것도 안 바뀐다는
//! 뜻이 돼 버린다).

use super::theme;
use crate::app::{App, Screen};
use crate::localtime::KST_OFFSET_SECS;
use crate::model::{AtBat, GameStatus, LiveState, Pitch, RelayLine, Team};
use crate::ui::i18n::Labels;
use ratatui::style::{Color, Modifier, Style};

/// 스코어라인의 "지금 이 순간" 값들(B/S/O·주자·승률). 응답은 이 셋을 **현재
/// 시점 기준으로만** 알려주므로, 과거 타석을 돌려보는 중엔 그 타석의 값인 것처럼
/// 오해된다 — 그래서 [`LiveVm::now_fields`]가 통째로 `None`이 된다. 이 규칙이
/// 사는 곳은 여기 한 곳뿐이다(v0.18엔 타자 축·이닝 축에 흩어져 있었다).
pub(crate) struct NowFields {
    /// "B2 S3 O3"
    pub count: String,
    /// "[3 2 1]" (빈 베이스는 '-', 폭 고정)
    pub bases: String,
    /// "WP 45%/55%"
    pub win_pct: String,
}

/// 스코어라인 셋째 줄. **말줄임 대상인지 아닌지**를 타입으로 구분한다 —
/// 선택 투구 상세(`Detail`)만 폭에 맞춰 자르고, 네비 힌트·빈 줄(`Plain`)은
/// 자르지 않는다(§15 오버플로 정책 + v0.18 동작 그대로).
pub(crate) enum PitchLine {
    /// 선택된 투구의 상세 — 결과 원문이 길면 정직하게 말줄임한다.
    Detail(String),
    /// 네비 힌트 또는 빈 줄 — 폭에 맞춰 자르지 않는다.
    Plain(String),
}

impl PitchLine {
    /// `inner_width`(테두리 2칸을 뺀 내부 폭)에 맞춘 최종 문자열.
    pub(crate) fn text(&self, inner_width: usize) -> String {
        match self {
            PitchLine::Detail(s) => super::text::ellipsize(s, inner_width),
            PitchLine::Plain(s) => s.clone(),
        }
    }
}

/// 본문 폭이 이만큼은 돼야 문자중계 옆에 스트라이크존을 함께 그린다. 더 좁으면
/// 존을 숨기고 중계에 본문 전체를 준다(우아한 저하).
const ZONE_MIN_WIDTH: u16 = 70;

/// 라이브 화면 한 프레임이 그릴 모든 것. 여기 없는 값은 렌더가 알 필요 없고,
/// 여기 있는 값은 렌더가 다시 계산하지 않는다.
pub(crate) struct LiveVm<'a> {
    /// 블록 타이틀 — 라이브면 `l.title_live`, 돌려보기 중이면 "{Rewind} {이닝}
    /// {타자}". 라이브와 절대 헷갈리면 안 된다는 제약이 여기서 지켜진다.
    pub title: String,
    pub away: &'a Team,
    pub home: &'a Team,
    pub away_score: u16,
    pub home_score: u16,
    /// 스코어라인에 쓸 이닝 — 돌려보기 중이면 **그 타석의** 이닝이다(안 바꾸면
    /// 타이틀은 T9, 아랫줄은 B9라 한 화면이 두 이닝을 말한다 — 리뷰 I-1).
    pub inning_label: &'a str,
    /// Suspended/Final 배지(라벨 + 이미 mono 게이트를 거친 스타일). Live 등은
    /// `None`이라 배지가 아예 없다.
    pub status_badge: Option<(&'static str, Style)>,
    /// `None`이면 **돌려보기 중**이라 "지금 이 순간" 값들을 통째로 비운 것이다.
    pub now_fields: Option<NowFields>,
    /// 스코어라인 둘째 줄의 폭 무관 부분(투수/타자 또는 과거 타자, 시작 시각).
    detail_base: String,
    /// 폭이 남을 때만 붙는 경과/소요("   Elapsed (+2:00)"). 값 자체가 없으면
    /// (파싱 실패·끝점 미상) `None`.
    duration_addition: Option<String>,
    pub pitch_line: PitchLine,
    /// 활성 at-bat(라이브 또는 돌려보는 과거 타석)의 투구.
    pub pitches: &'a [Pitch],
    /// 화면 전체가 공유하는 **하나의** 투구 선택(존·측면·상세줄이 전부 이 값을
    /// 본다). 문자중계 커서가 있으면 그 줄에서 유도되고, 없으면 `←`/`→`가 남긴
    /// 값이며, 어느 쪽이든 이미 `pitches` 범위 안임이 보장된다 — 범위 밖은
    /// `None`으로 접힌다(리뷰 I-3a: v0.19a까지는 상세줄과 존이 같은 값을 서로
    /// 다르게 해석했다).
    pub selected_pitch: Option<usize>,
    /// 활성 at-bat의 문자중계 줄(오래된→최신) — **이미 화면에 낼 최종
    /// 문자열**이다(불릿 프리픽스 포함). 렌더는 이 값을 `ListItem::new`에
    /// 그대로 넘기기만 한다. v0.19a까지는 이 필드가 모델 원본(`&[String]`)을
    /// 그대로 통과시켜, 줄을 어떻게 보일지에 대한 결정이 렌더의
    /// `format!("· {entry}")`에 남아 있었다 — 리뷰 I-1로 여기(VM)로 옮겼다.
    pub relay_rows: Vec<String>,
    /// 문자중계 커서. `None`이면 하이라이트 없이 꼬리만 보여준다(기본 상태).
    /// `Some(idx)`는 **이미 `relay_rows` 범위 안으로 클램프됐다** — v0.19a까지
    /// 이 클램프(`idx.min(len-1)`)는 렌더에 있었다(리뷰 I-1). 렌더는 이제 범위
    /// 밖 인덱스를 걱정할 필요가 없다.
    pub relay_cursor: Option<usize>,
    pub relay_title: &'static str,
    /// strikezone 등 VM을 거치지 않는 형제 위젯에 그대로 넘겨줄 라벨
    /// 테이블(통과값). `render()`가 로딩 문구에도 라벨이 필요해 자체적으로
    /// `app.labels()`를 부르므로, 이 필드가 없으면 성공 경로에서 같은 정적
    /// 조회가 두 번(여기와 render()) 일어난다(M-7) — 여기 한 번만 조회해
    /// 재사용한다.
    pub labels: &'static Labels,
}

impl<'a> LiveVm<'a> {
    /// 라이브 화면이고 상태가 도착해 있으면 표현 상태를 만든다. 아직 로딩
    /// 중이거나 다른 화면이면 `None`(렌더가 로딩 문구로 저하).
    pub(crate) fn from_app(app: &'a App) -> Option<Self> {
        let Screen::Live {
            game,
            state: Some(s),
        } = &app.screen
        else {
            return None;
        };
        let l = app.labels();

        // 활성 at-bat 해석: 고른 번호가 **실제로 응답에 있고** 최신이 아닐 때만
        // "과거를 보는 중"이다. 번호만 보고 판정하면, 응답에서 사라진 stale
        // 번호(이닝 전환)에도 Rewind 타이틀이 붙는데 내용은 active_at_bat이
        // 낮춘 최신 타석이라 라벨과 내용이 어긋난다 — 없는 타석을 있는 척
        // 보여주지 않는다는 게 이 기능의 계약이다.
        let active = s.active_at_bat(app.live_atbat_sel);
        let viewing_past = matches!(
            (app.live_atbat_sel, active, s.at_bats.last()),
            (Some(seq), Some(ab), Some(newest)) if ab.seq == seq && seq != newest.seq
        );
        // ★ 되감기 규칙의 유일한 갈림길 — 아래 title·inning_label·now_fields·
        // detail_base가 전부 이 한 값에서 갈린다.
        let past_at_bat = if viewing_past { active } else { None };

        let title = match past_at_bat {
            Some(ab) => rewind_title(l, ab),
            None => l.title_live.to_string(),
        };
        let inning_label: &str = match past_at_bat {
            Some(ab) => &ab.inning_label,
            None => &s.inning_label,
        };
        let now_fields = past_at_bat.is_none().then(|| NowFields {
            count: format!("B{} S{} O{}", s.count.ball, s.count.strike, s.count.out),
            // 3슬롯 ASCII 주자 표시: [3루 2루 1루], 빈 베이스는 '-' — 폭 고정.
            bases: format!(
                "[{} {} {}]",
                if s.bases.third { "3" } else { "-" },
                if s.bases.second { "2" } else { "-" },
                if s.bases.first { "1" } else { "-" },
            ),
            win_pct: format!(
                "WP {}/{}",
                win_pct(s.away_win_rate),
                win_pct(s.home_win_rate)
            ),
        });

        // "HH:MM" 경기 시작 시각("....THH:MM:SS"에서 추출, 실패 시 생략).
        let start_hhmm = game
            .start
            .split('T')
            .nth(1)
            .and_then(|t| t.get(0..5))
            .unwrap_or("");
        let detail_base = detail_prefix(l, s, past_at_bat, start_hhmm);

        // B-2/B-3(v0.18): 경기 경과/소요. Live는 "시작~지금", Final/Suspended는
        // "시작~데이터 안의 마지막 투구 시각"(§2 B-3, ★핵심) — Suspended를 Final과
        // 묶은 이유: "지금"을 쓰면 서스펜디드 상태로 며칠 방치된 경기를 열 때도
        // Final과 똑같이 비현실적인 값(수십 시간)이 나오기 때문이다. 이 화면엔
        // Scheduled/Canceled가 들어오지 않으므로(can_enter_live) 그 두 경우는 값이
        // 없다.
        let duration_addition = game_duration_label(
            game.status,
            &game.start,
            &now_kst_hms(app.now_secs),
            latest_pitch_time(s),
        )
        .map(|dur| {
            let label = if game.status == GameStatus::Live {
                l.lbl_elapsed
            } else {
                l.lbl_duration
            };
            format!("   {label} {dur}")
        });

        let pitches = s.active_pitches(app.live_atbat_sel);

        // I-1(v19a 리뷰): 줄의 표현(불릿 프리픽스)과 범위 밖 커서의 저하를
        // 여기서 끝낸다 — render_relay는 이제 완성된 문자열과 이미 유효한
        // 인덱스만 받는다(§I-1 doc 참고).
        let relay_rows = format_relay_rows(s.active_relay_lines(app.live_atbat_sel));
        let relay_cursor = app
            .live_relay_cursor
            .map(|idx| idx.min(relay_rows.len().saturating_sub(1)));

        // ★ v0.19 연동 + I-3a 통일. 화면 전체가 쓰는 **하나의** 투구 선택을 여기서
        // 확정한다.
        //
        // ① 커서가 있으면 그게 "지금 보고 있는 사건"이고 투구는 그 줄에서
        //    유도된다(투구가 아닌 줄이면 위쪽 가장 가까운 투구 —
        //    `pitch_at_relay_line`). App::on_key가 두 필드를 항상 짝으로 옮기므로
        //    평소엔 `live_pitch_sel`과 같은 값이 나오지만, 둘이 어긋난 상태가
        //    (손 조립·미래의 다른 갱신 경로로) 들어와도 **커서 우선**이라는 한
        //    규칙으로 모순 없이 접힌다.
        // ② 커서가 없으면 `←`/`→`가 남긴 값을 그대로 쓴다(v0.18 그대로).
        // ③ 어느 쪽이든 마지막에 범위 검사를 한 번만 통과시킨다 — v0.19a까지
        //    상세줄(`pitch_line`)은 범위 밖 선택을 힌트로 낮추는데 존에는 원본이
        //    그대로 가서, 같은 상태를 두 위젯이 다르게 해석했다(존은 "일치하는
        //    투구 없음"으로 **빈 화면**이 됐다 — 리뷰 I-3a). 이제 범위 밖은 곧
        //    선택 없음이라 존은 전체 투구를, 상세줄은 네비 힌트를 함께 보여준다.
        let selected_pitch = match relay_cursor {
            Some(i) => s.pitch_at_relay_line(app.live_atbat_sel, i),
            None => app.live_pitch_sel,
        }
        .filter(|i| *i < pitches.len());

        let pitch_line = pitch_line(l, &game.start, pitches, selected_pitch);

        Some(LiveVm {
            title,
            away: &s.away,
            home: &s.home,
            away_score: s.away_score,
            home_score: s.home_score,
            inning_label,
            status_badge: status_badge(game.status, l, &app.theme_preset),
            now_fields,
            detail_base,
            duration_addition,
            pitch_line,
            pitches,
            selected_pitch,
            relay_rows,
            relay_cursor,
            relay_title: l.title_relay,
            labels: l,
        })
    }

    /// 스코어라인 둘째 줄. 경과/소요는 **남는 폭이 있을 때만** 붙는다 —
    /// header.rs A-1/A-2(v0.15)와 같은 방식으로, 만들고 나서 자르는 대신 붙이기
    /// 전에 예산을 본다. 그래야 좁은 터미널에서 시간 정보가 먼저 조용히 빠지고
    /// 투수/타자 같은 기존 정보는 절대 밀리지 않는다.
    pub(crate) fn detail_line(&self, inner_width: usize) -> String {
        let mut detail = self.detail_base.clone();
        if let Some(addition) = &self.duration_addition {
            if inner_width
                >= super::text::display_width(&detail) + super::text::display_width(addition)
            {
                detail.push_str(addition);
            }
        }
        detail
    }

    /// 문자중계 옆에 스트라이크존을 함께 그릴지. 폭이 좁거나 **아직 투구
    /// 데이터가 없으면** 존을 숨기고 중계에 본문 전체를 준다(우아한 저하) —
    /// 뒤쪽 조건은 폭과 무관한 결정이라 원래부터 여기 있어야 했다.
    pub(crate) fn show_strike_zone(&self, body_width: u16) -> bool {
        body_width >= ZONE_MIN_WIDTH && !self.pitches.is_empty()
    }
}

/// Live/Suspended/Final 외 상태(can_enter_live가 걸러내는 Canceled/Scheduled)는
/// 이 화면에 들어오지 않으므로 배지가 필요 없다 — None을 반환해 그대로 숨긴다.
/// 색은 games.rs의 status_tag와 맞춘다(같은 상태는 같은 색으로 보이도록).
/// mono 프리셋은 theme::status_fg 게이트를 거쳐 색을 걷어낸다(header/games와
/// 동일 패턴) — 리뷰 지적: 이전엔 Suspended가 게이트 없이 Magenta를 직접 써
/// mono에서도 자홍색이 남았다.
fn status_badge(
    status: GameStatus,
    l: &'static Labels,
    preset: &str,
) -> Option<(&'static str, Style)> {
    match status {
        GameStatus::Suspended => Some((
            l.badge_suspended,
            theme::status_fg(preset, Color::Magenta).add_modifier(Modifier::BOLD),
        )),
        GameStatus::Final => Some((
            l.badge_final,
            theme::status_fg(preset, Color::Gray).add_modifier(Modifier::BOLD),
        )),
        GameStatus::Live | GameStatus::Scheduled | GameStatus::Canceled => None,
    }
}

/// 돌려보기(v0.18) 중 라이브 타이틀 대신 보여줄 문자열: "{Rewind} {inning}
/// {batter}" — 타자명이 없으면(안내 유실 등) 이닝까지만. 라이브와 절대 헷갈리지
/// 않게 title_live 대신 이 문자열을 블록 타이틀로 쓴다.
fn rewind_title(l: &'static Labels, ab: &AtBat) -> String {
    let mut t = format!(" {} {}", l.rewind_label, ab.inning_label);
    if !ab.batter_name.is_empty() {
        t.push(' ');
        t.push_str(&ab.batter_name);
    }
    t.push(' ');
    t
}

fn win_pct(rate: Option<f32>) -> String {
    rate.map(|r| format!("{:.0}%", r * 100.0))
        .unwrap_or_else(|| "-".into())
}

/// 스코어라인 3번째 줄(디테일)의 "투수/타자(또는 되감기 중 타자만) + 시작
/// 시각" 부분. 경기 경과/소요(B-3 addition)는 폭 예산이 필요해
/// [`LiveVm::detail_line`]에서 따로 붙인다 — 여기는 폭과 무관한 부분만 만든다.
///
/// 돌려보기 중이면 이 줄도 그 타석 것이어야 한다. 라이브 값을 그대로 두면
/// 타이틀은 "Rewind B9 정은원"인데 바로 아랫줄이 "B: 한지윤"이라, 한 화면이
/// 두 타자를 말한다(실행 확인에서 발견 — 타이틀·투구 수만 보던 테스트는
/// 놓쳤다). 과거 타석에 대해 응답이 확실히 알려주는 건 타자뿐이므로,
/// 투수·다음타자는 라이브 값으로 채우지 않고 비운다.
fn detail_prefix(
    l: &'static Labels,
    s: &LiveState,
    past_at_bat: Option<&AtBat>,
    start_hhmm: &str,
) -> String {
    let mut detail = match past_at_bat {
        Some(ab) if !ab.batter_name.is_empty() => {
            format!("{}: {}", l.lbl_batter, ab.batter_name)
        }
        Some(_) => String::new(),
        None => {
            let mut d = format!(
                "{}: {}   {}: {}",
                l.lbl_pitcher, s.pitcher_name, l.lbl_batter, s.batter_name
            );
            if !s.next_batter_name.is_empty() {
                d.push_str(&format!("   {}: {}", l.lbl_next, s.next_batter_name));
            }
            d
        }
    };
    if !start_hhmm.is_empty() {
        // M-3: detail이 비어 있을 때(타자명 없는 과거 타석) 구분자를 무조건
        // 붙이면 "   Start 18:30"처럼 공백 3칸으로 줄이 시작한다 — 구분자는
        // 이미 내용이 있을 때만 필요하다.
        if !detail.is_empty() {
            detail.push_str("   ");
        }
        detail.push_str(&format!("{} {start_hhmm}", l.lbl_start));
    }
    detail
}

/// 스코어라인 셋째 줄: 선택된 투구 상세(시각·상대시간·결과 원문) 또는 네비 힌트.
/// `pitches`는 활성 at-bat(라이브 또는 돌려보기 중인 과거 타석)의 투구다.
fn pitch_line(l: &Labels, game_start: &str, pitches: &[Pitch], sel: Option<usize>) -> PitchLine {
    match sel.and_then(|i| pitches.get(i).map(|p| (i, p))) {
        Some((i, p)) => {
            let speed = p
                .speed_kmh
                .map(|k| format!("{k}km"))
                .unwrap_or_else(|| "-".into());
            let time = p.time_hms.as_deref().unwrap_or("-");
            let rel = p
                .time_hms
                .as_deref()
                .and_then(|t| elapsed_label(game_start, t))
                .unwrap_or_default();
            // B-2(v0.18): 직전 투구 대비 경과("+18초"류) — 첫 투구(i==0)는 직전이
            // 없으므로 생략, i.time_hms 결측·파싱 실패도 관용적으로 생략. 폭
            // 예산은 이 줄 전체를 감싸는 PitchLine::Detail 말줄임 한 번으로
            // 충분하다(B-2는 새 칸이 아니라 이미 있는 상세줄 안에 끼워 넣는
            // 값이라 header.rs류 별도 폭 계산이 필요 없다) — i==0일 때 interval이
            // 빈 문자열이라 기존 렌더와 완전히 동일해 무회귀도 자동으로 만족한다.
            let interval = if i > 0 {
                pitches[i - 1]
                    .time_hms
                    .as_deref()
                    .zip(p.time_hms.as_deref())
                    .and_then(|(prev, cur)| pitch_interval_label(l, prev, cur))
                    .map(|s| format!(" {s}"))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            PitchLine::Detail(format!(
                "{} {}/{}  {}  {} {}{interval}  {}",
                l.pitch_word,
                i + 1,
                pitches.len(),
                speed,
                time,
                rel,
                p.text
            ))
        }
        None if !pitches.is_empty() => PitchLine::Plain(format!(
            "{} {}  {}",
            l.pitches_word,
            pitches.len(),
            l.inspect_hint
        )),
        None => PitchLine::Plain(String::new()),
    }
}

/// 문자중계 원본 줄을 화면에 낼 최종 문자열로 바꾼다(불릿 프리픽스). 렌더는
/// 이 결과를 `ListItem::new`에 그대로 넘기기만 한다 — "줄이 어떻게 보일지"는
/// 여기서 결정한다(v19a 리뷰 I-1: 이전엔 이 결정이 렌더의 `format!("· {entry}")`
/// 두 곳에 흩어져 있었다).
///
/// v0.19 연동이 들어와도 **줄의 겉모습은 그대로 둔다**. 시각·"N구" 마커를 붙일
/// 자리는 여기지만 이번엔 붙이지 않았다:
/// - 시각을 가진 줄은 투구 줄뿐이라(응답이 투구가 아닌 줄엔 시각을 아예 안 준다)
///   칸이 들쭉날쭉해지고, 그 시각은 선택 투구 상세줄이 이미 절대시각·경기 경과·
///   직전 투구 대비 간격까지 보여 준다(v0.18 B-2/B-3) — 같은 값을 두 번 쓰는 값에
///   비해 폭이 비싸다(존을 함께 그릴 때 이 패널은 본문의 60%뿐이다).
/// - 투구 줄/아닌 줄을 다른 글머리로 가르는 안도 접었다. 연동 규칙이 "위쪽 가장
///   가까운 투구를 물려받는다"라 첫 투구 이후의 줄은 전부 어떤 공이든 가리키므로,
///   구분 표시가 없어도 커서를 움직이는 것만으로 관계가 드러난다.
fn format_relay_rows(lines: &[RelayLine]) -> Vec<String> {
    lines
        .iter()
        .map(|entry| format!("· {}", entry.text))
        .collect()
}

/// 경기 시작("....THH:MM:SS")과 어떤 시각("HH:MM:SS")의 차(초) — 자정 넘김
/// (그 시각 < 시작)은 +24h 보정. elapsed_label(표시용 포맷)과
/// game_duration_label(M-4 Live 상한 가드)이 공유한다. 파싱 실패는 None(관용).
///
/// M-1(v19a 리뷰): "HH:MM:SS" 자체의 자릿수·범위 검증(`parse_hms_secs`)은 이
/// 화면의 표현 상태와 무관한 범용 파서라 `crate::dateutil`에 산다 —
/// games.rs::scheduled_eta_hm(v0.15 A-3)도 같은 파서를 쓴다. "자정 넘김을
/// 어떻게 보정할지"만 호출부마다 다르다(여기는 항상 미래 방향이라 +24h 고정
/// 보정이 안전하지만, A-3는 날짜가 있는 절대시각 비교라 이 값만으로 보정하면
/// 안 된다 — games.rs 쪽 주석 참고).
fn elapsed_secs(game_start: &str, hms: &str) -> Option<i64> {
    let start = crate::dateutil::parse_hms_secs(game_start.split('T').nth(1)?)?;
    let cur = crate::dateutil::parse_hms_secs(hms)?;
    let mut d = cur - start;
    if d < 0 {
        d += 24 * 3600;
    }
    Some(d)
}

/// 경기 시작("....THH:MM:SS")과 투구 시각("HH:MM:SS")의 차 → "(+H:MM)".
/// 자정 넘김(투구 < 시작)은 +24h 보정. 파싱 실패는 None(관용 — 표시 생략).
fn elapsed_label(game_start: &str, pitch_hms: &str) -> Option<String> {
    let d = elapsed_secs(game_start, pitch_hms)?;
    Some(format!("(+{}:{:02})", d / 3600, (d % 3600) / 60))
}

/// 이 값(초) 이상의 투구 간격은 "직전 투구 대비"라는 의미를 잃었다고 보고
/// 표시를 생략한다(B-2). 근거: `Pitch.time_hms`는 "HH:MM:SS"뿐 날짜가 없어
/// 자정 넘김은 +24h 한 번만 보정할 수 있다 — 그런데 KBO 서스펜디드 경기는
/// **같은 타석 도중에도** 중단→(다른 날) 재개가 가능해서, 재개 후 첫 투구와
/// 중단 전 마지막 투구의 실제 간격은 몇 시간~며칠일 수 있는데 날짜가 없으니
/// +24h 보정 하나로는 옳게 잡아낼 수 없다(오히려 그럴듯해 보이는 틀린 값을
/// 만들 위험이 더 크다). 반면 피치클락 시대 정상 투구 간격은 수 초~1분,
/// 마운드 방문·챌린지를 포함해도 30분을 넘기는 경우는 거의 없다 — 그 이상은
/// "간격"이 아니라 "데이터가 못 담는 중단"으로 보고 조용히 생략한다(관용
/// 원칙: 틀릴 수 있는 숫자를 보여주는 것보다 생략이 낫다).
const IMPLAUSIBLE_PITCH_GAP_SECS: i64 = 30 * 60;

/// 직전 투구 대비 경과 → "+18초"(60초 미만, 언어별 접미) 또는 "+3:05"(60초
/// 이상, elapsed_label과 같은 자릿수 표기 관례). 자정 넘김은 elapsed_label과
/// 동일한 +24h 보정(둘 다 "HH:MM:SS만 있고 날짜가 없다"는 같은 제약을 공유).
/// 파싱 실패·[`IMPLAUSIBLE_PITCH_GAP_SECS`] 초과는 None(생략).
fn pitch_interval_label(l: &Labels, prev_hms: &str, cur_hms: &str) -> Option<String> {
    let prev = crate::dateutil::parse_hms_secs(prev_hms)?;
    let cur = crate::dateutil::parse_hms_secs(cur_hms)?;
    let mut d = cur - prev;
    if d < 0 {
        d += 24 * 3600;
    }
    if d > IMPLAUSIBLE_PITCH_GAP_SECS {
        return None;
    }
    if d < 60 {
        Some(format!("+{d}{}", l.pitch_interval_secs_suffix))
    } else {
        Some(format!("+{}:{:02}", d / 60, d % 60))
    }
}

/// UTC epoch 초 → KST 기준 "HH:MM:SS". `game.start`·`Pitch.time_hms`는 항상
/// KBO 데이터 자체의 시간대(KST)로 찍혀 있으므로, 진행 중 경기의 "지금까지"를
/// 재려면 **보는 사람의 표시 시간대(v0.16 `app.tz`)가 아니라 KST 고정
/// 오프셋**을 써야 한다 — 뉴욕에서 보고 있어도 데이터의 시계 자체는 서울
/// 시계이기 때문이다(경과는 차이값이라 표시 시간대와 무관, §2 B-3).
fn now_kst_hms(now_secs: u64) -> String {
    let secs_of_day = (now_secs as i64 + KST_OFFSET_SECS as i64).rem_euclid(86400);
    format!(
        "{:02}:{:02}:{:02}",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

/// 경기 데이터 안에서 확인 가능한 가장 최근 투구 시각(경기 전체 기준, 사용자가
/// 지금 돌려보는 중인 과거 타석과 무관하다 — B-3의 종료 경기 끝점은 항상
/// "최신" 타석에서 찾는다). `at_bats`를 최신 타석부터 거슬러 올라가며 각
/// 타석의 마지막 투구부터 역순으로 훑어 처음 나오는 유효한 `time_hms`를
/// 쓴다(마지막 투구의 시각이 결측이어도 관용적으로 그 앞을 본다).
/// `at_bats`가 비어 있으면(구버전 손 조립 상태 등) current_pitches로 무회귀
/// 폴백한다 — active_pitches/active_relay_lines와 같은 관례.
fn latest_pitch_time(s: &LiveState) -> Option<&str> {
    s.at_bats
        .iter()
        .rev()
        .flat_map(|ab| ab.pitches.iter().rev())
        .find_map(|p| p.time_hms.as_deref())
        .or_else(|| {
            s.current_pitches
                .iter()
                .rev()
                .find_map(|p| p.time_hms.as_deref())
        })
}

/// M-4: 진행 중(Live) 경기인데 `now`가 `start`보다 앞서면(상태가 시작 전에
/// Live로 뒤집히거나 사용자 시계가 몇 분 느린 클록 스큐, 실측: 시작 10초
/// 전) `elapsed_secs`의 +24h 자정 보정이 거의 24시간짜리 값("Elapsed
/// (+23:59)")을 만든다 — 진행 중 경기가 그렇게 오래 걸릴 수는 없다(서스펜디드로
/// 넘어가면 Final/Suspended 취급이라 애초에 이 분기에 오지 않는다). B-2가
/// IMPLAUSIBLE_PITCH_GAP_SECS로 막은 것과 같은 위험이라 같은 원칙(생략)으로
/// 막는다.
const IMPLAUSIBLE_LIVE_ELAPSED_SECS: i64 = 12 * 3600;

/// 경기 경과/소요(B-3) → elapsed_label과 같은 "(+H:MM)" 표기. `Live`는
/// `now_hms`(호출부가 [`now_kst_hms`]로 만든 "지금")까지, `Final`·`Suspended`는
/// `end_hms`(호출부가 [`latest_pitch_time`]으로 구한, 데이터 안의 마지막 투구
/// 시각)까지 잰다.
///
/// ★ Final/Suspended에 `now_hms`를 쓰면 안 된다 — 어제 끝난 경기를 오늘 열면
/// "지금까지"가 20시간으로 찍히는 버그가 된다(§2 B-3 핵심 요구). Suspended를
/// Final과 묶은 이유도 같다: 서스펜디드 경기는 재개까지 며칠씩 걸릴 수 있어
/// "지금"을 쓰면 똑같이 비현실적인 값이 나온다 — 이 화면엔 진행 중이거나(Live)
/// 이미 멈춘(Final/Suspended) 경기만 들어오므로(can_enter_live가
/// Scheduled/Canceled를 걸러낸다) 그 두 상태는 다루지 않는다.
///
/// `end_hms`가 None(경기 데이터에 투구 시각이 하나도 없음)이거나 파싱 실패면
/// 생략(관용 원칙).
fn game_duration_label(
    status: GameStatus,
    game_start: &str,
    now_hms: &str,
    end_hms: Option<&str>,
) -> Option<String> {
    let hms = match status {
        GameStatus::Live => Some(now_hms),
        GameStatus::Final | GameStatus::Suspended => end_hms,
        GameStatus::Scheduled | GameStatus::Canceled => None,
    };
    let hms = hms?;
    if status == GameStatus::Live && elapsed_secs(game_start, hms)? > IMPLAUSIBLE_LIVE_ELAPSED_SECS
    {
        return None;
    }
    elapsed_label(game_start, hms)
}

#[cfg(test)]
mod tests {
    use super::{LiveVm, PitchLine};
    use crate::app::{App, Screen};
    use crate::model::{AtBat, BaseState, Count, Game, GameStatus, LiveState, Pitch, Team};

    const RELAY: &str = include_str!("../../tests/fixtures/relay_20260719KTLG.json");

    fn team(code: &str, name: &str) -> Team {
        Team {
            code: code.into(),
            name: name.into(),
        }
    }

    /// 순수 함수 테스트 전용 최소 `LiveState` — 렌더와 무관하므로 team/score
    /// 등은 아무 값이나 둔다.
    fn bare_state() -> LiveState {
        LiveState {
            inning_label: String::new(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
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

    /// fixture 기반 라이브 화면 — live.rs 테스트와 같은 데이터(천성호 타석이
    /// 최신, 최원준 타석의 seq는 87).
    fn live_app(status: GameStatus) -> App {
        let state =
            crate::source::naver::map::live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT"))
                .unwrap();
        let game = Game {
            id: "20260719KTLG02026".into(),
            start: "".into(),
            status,
            status_label: state.inning_label.clone(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(state.home_score),
            away_score: Some(state.away_score),
        };
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game,
            state: Some(state),
        };
        app
    }

    /// 테스트가 fixture 상태를 직접 들여다볼 때 쓰는 접근자.
    fn state_of(app: &App) -> &LiveState {
        match &app.screen {
            Screen::Live { state: Some(s), .. } => s,
            _ => panic!("live screen expected"),
        }
    }

    // ---- ViewModel: 돌려보기 규칙 (v0.18에 뚫렸던 곳) ----

    /// ★ "돌려보기 중엔 감출 필드"를 **문자열 없이** 잠근다. 이전엔 화면 버퍼에
    /// "B2 S3 O3"이 있나 없나로만 볼 수 있었다 — 이제 규칙이 사는 자리(now_fields)
    /// 를 직접 본다. B/S/O·주자·승률은 "지금 이 순간" 값이라 과거 타석 옆에 두면
    /// 그 타석의 카운트로 오해된다.
    #[test]
    fn the_view_model_hides_the_live_only_fields_while_rewinding() {
        let mut app = live_app(GameStatus::Live);
        let live = LiveVm::from_app(&app).unwrap();
        assert!(
            live.now_fields.is_some(),
            "전제: 라이브에서는 지금-이-순간 값들이 채워진다"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석
        let past = LiveVm::from_app(&app).unwrap();
        assert!(
            past.now_fields.is_none(),
            "돌려보기 중엔 B/S/O·주자·승률을 통째로 비워야 한다"
        );
    }

    /// 되감기 규칙은 **한 갈림길**에서만 갈린다: 이닝은 아는 값이라 과거 타석
    /// 것으로 바뀌고, 카운트류는 모르는 값이라 사라진다. v0.18에선 이 한 규칙이
    /// 타자 축·이닝 축에 흩어져 같은 결함을 두 번 고쳤다(4912944 → 리뷰 I-1).
    #[test]
    fn the_view_model_swaps_the_inning_label_for_the_past_at_bats_own() {
        let mut app = live_app(GameStatus::Live);
        let live_inning = LiveVm::from_app(&app).unwrap().inning_label.to_string();

        app.live_atbat_sel = Some(87);
        let past_inning = LiveVm::from_app(&app).unwrap().inning_label.to_string();

        let past_ab = state_of(&app)
            .at_bats
            .iter()
            .find(|ab| ab.seq == 87)
            .expect("fixture must contain at-bat 87");
        assert_eq!(past_inning, past_ab.inning_label);
        assert_ne!(
            past_inning, live_inning,
            "라이브 이닝이 남으면 한 화면이 두 이닝을 말한다"
        );
    }

    /// ★ "되감기가 실제로 다른 데이터를 고른다"를 화면 문자열 없이 잠근다.
    /// v0.18 최종 리뷰가 뮤테이션으로 실증했듯, 렌더 문자열 검사만으로는
    /// 되감기를 통째로 무력화해도 전부 통과할 수 있었다 — 여기서는 VM이 고른
    /// 슬라이스를 그 타석 자신의 데이터와 **동등성으로** 비교한다.
    #[test]
    fn rewinding_actually_selects_the_past_at_bats_own_pitches_and_relay_lines() {
        let mut app = live_app(GameStatus::Live);
        app.live_atbat_sel = Some(87);
        let vm = LiveVm::from_app(&app).unwrap();

        let s = state_of(&app);
        let past = s.at_bats.iter().find(|ab| ab.seq == 87).unwrap();
        let newest = s.at_bats.last().unwrap();

        assert_eq!(vm.pitches, past.pitches.as_slice());
        // relay_rows는 원본에 불릿을 씌운 최종 문자열이다(I-1) — 데이터
        // 선택이 맞는지는 같은 변환(format_relay_rows)을 거친 값과 비교한다.
        assert_eq!(vm.relay_rows, super::format_relay_rows(&past.relay_lines));
        // 대조군: 최신 타석 것을 그대로 들고 있으면 되감기가 아무 일도 안 한
        // 것이다(fixture 실측: 최원준 7구 / 천성호 5구).
        assert_ne!(vm.pitches, newest.pitches.as_slice());
        assert_ne!(vm.relay_rows, super::format_relay_rows(&newest.relay_lines));
    }

    // ---- ViewModel: 문자중계 줄 표현 (v19a 리뷰 I-1 — 렌더에서 옮겨옴) ----

    /// I-1: 줄이 어떻게 보일지(불릿 프리픽스)는 VM이 정한다 — 렌더는 완성된
    /// 문자열을 그대로 찍기만 한다. 화면 버퍼가 아니라 VM이 낸 값을 직접 본다.
    #[test]
    fn relay_rows_carry_the_bullet_prefix_that_the_renderer_used_to_add() {
        let app = live_app(GameStatus::Live);
        let vm = LiveVm::from_app(&app).unwrap();
        let raw = &state_of(&app).at_bats.last().unwrap().relay_lines;
        assert!(!raw.is_empty(), "전제: fixture엔 중계 줄이 있다");
        assert_eq!(vm.relay_rows.len(), raw.len());
        for (row, line) in vm.relay_rows.iter().zip(raw.iter()) {
            assert_eq!(row, &format!("· {}", line.text));
        }
    }

    /// I-1: 범위 밖 문자중계 커서는 VM이 이미 마지막 줄로 낮춰서 내보낸다 —
    /// 렌더는 `idx.min(len-1)` 같은 클램프를 몰라도 된다(v0.18까지는 이
    /// 클램프가 render_relay 안에 있었다).
    #[test]
    fn relay_cursor_out_of_range_is_clamped_by_the_view_model() {
        let mut app = live_app(GameStatus::Live);
        app.live_relay_cursor = Some(9999); // 범위 밖(App이 평소엔 만들지 않는 값)
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(vm.relay_cursor, Some(vm.relay_rows.len() - 1));
    }

    /// 대조군: 범위 안 커서는 그대로 통과한다(무회귀).
    #[test]
    fn relay_cursor_within_range_passes_through_unchanged() {
        let mut app = live_app(GameStatus::Live);
        app.live_relay_cursor = Some(0);
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(vm.relay_cursor, Some(0));
    }

    // ---- v0.19: 문자중계 ↔ 투구 연동 (한 선택의 두 투영) ----

    /// ★ 이 릴리스의 헤드라인: 문자중계 줄에 커서를 두면 **그 줄이 말하는 공**이
    /// 존·측면에 뜬다. fixture 실측(천성호 타석): 0=안내, 1~5=1~5구, 6=결과 요약.
    /// 화면 문자열이 아니라 VM이 낸 선택값을 직접 본다.
    #[test]
    fn putting_the_relay_cursor_on_a_pitch_line_selects_that_pitch() {
        let mut app = live_app(GameStatus::Live);
        for (line, want) in [(1usize, 0usize), (3, 2), (5, 4)] {
            app.live_relay_cursor = Some(line);
            let vm = LiveVm::from_app(&app).unwrap();
            assert_eq!(
                vm.selected_pitch,
                Some(want),
                "{line}번째 줄({:?})은 {want}번 투구를 가리켜야 한다",
                state_of(&app).at_bats.last().unwrap().relay_lines[line].text
            );
        }
    }

    /// 투구가 아닌 줄에 커서가 가면 위쪽 가장 가까운 투구를 물려받는다 —
    /// 결과 요약 줄(마지막)에서는 그 결과를 만든 마지막 투구가 남는다.
    #[test]
    fn the_result_summary_line_keeps_the_pitch_that_produced_it() {
        let mut app = live_app(GameStatus::Live);
        let last_line = state_of(&app).at_bats.last().unwrap().relay_lines.len() - 1;
        app.live_relay_cursor = Some(last_line);
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(vm.selected_pitch, Some(vm.pitches.len() - 1));
    }

    /// 첫 투구보다 위(타자 등장 안내)로 커서를 올리면 투구 선택이 풀린다 —
    /// 존이 그 타석 전체를 보여주는 개요로 돌아간다. `←`/`→`가 남긴 값이 있어도
    /// **커서가 우선**이라, 두 필드가 어긋난 조합이 화면에 나오지 않는다.
    #[test]
    fn the_relay_cursor_wins_over_a_stale_pitch_selection_and_can_clear_it() {
        let mut app = live_app(GameStatus::Live);
        app.live_pitch_sel = Some(3); // 예전에 고른 투구가 남아 있다고 치자
        app.live_relay_cursor = Some(0); // 커서는 타자 등장 안내 줄
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(
            vm.selected_pitch, None,
            "커서가 가리키는 줄엔 아직 던진 공이 없다"
        );

        // 대조군: 커서를 투구 줄로 내리면 그 줄의 투구가 이긴다(3이 아니라 0).
        app.live_relay_cursor = Some(1);
        assert_eq!(LiveVm::from_app(&app).unwrap().selected_pitch, Some(0));
    }

    /// 커서가 없으면 v0.18 그대로 `←`/`→`가 남긴 선택을 쓴다(무회귀).
    #[test]
    fn without_a_relay_cursor_the_pitch_selection_is_used_as_is() {
        let mut app = live_app(GameStatus::Live);
        app.live_pitch_sel = Some(2);
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(vm.selected_pitch, Some(2));
    }

    /// I-3a 통일: 범위 밖 투구 선택은 **하나의** 값으로 접힌다. v0.19a까지는
    /// 상세줄만 힌트로 낮추고 존에는 원본이 그대로 가서, 존이 "일치하는 투구
    /// 없음"으로 빈 화면이 됐다(같은 상태를 두 위젯이 다르게 해석). 이제 둘 다
    /// "선택 없음"을 본다 — 존은 전체 투구, 상세줄은 네비 힌트.
    #[test]
    fn an_out_of_range_pitch_selection_folds_into_no_selection_for_the_whole_screen() {
        let mut app = live_app(GameStatus::Live);
        app.live_pitch_sel = Some(9999);
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(
            vm.selected_pitch, None,
            "존에 범위 밖 인덱스가 그대로 가면 모든 투구가 걸러져 빈 화면이 된다"
        );
        assert!(
            matches!(vm.pitch_line, PitchLine::Plain(_)),
            "상세줄도 같은 판단(네비 힌트)을 해야 한다"
        );
    }

    /// 되감기 중에도 연동은 **그 타석 안에서만** 성립한다 — 과거 타석의 줄이
    /// 최신 타석 투구를 가리키면 존과 문자중계가 서로 다른 타석을 말한다.
    /// fixture 실측: 최원준 타석(seq 87)은 7구, 천성호(최신)는 5구.
    #[test]
    fn the_link_stays_within_the_past_at_bat_while_rewinding() {
        let mut app = live_app(GameStatus::Live);
        app.live_atbat_sel = Some(87);
        app.live_relay_cursor = Some(7); // 안내 1줄 + 7구 → 7번째 줄이 7구
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(vm.pitches.len(), 7, "전제: 최원준 타석은 7구");
        assert_eq!(
            vm.selected_pitch,
            Some(6),
            "최신 타석(5구)이었다면 범위 밖이라 None이 됐을 값"
        );
    }

    /// 활성 at-bat 해석의 계약: 응답에 **없는** 번호(이닝 전환으로 배열이 갈린
    /// 뒤 남은 stale 선택)는 되감기로 치지 않는다 — 없는 타석을 있는 척
    /// 보여주지 않아야 하므로, 최신으로 낮아지면서 라이브와 완전히 같아진다.
    #[test]
    fn a_stale_at_bat_selection_is_not_treated_as_rewinding() {
        let mut app = live_app(GameStatus::Live);
        let live = LiveVm::from_app(&app).unwrap();
        let (live_title, live_inning) = (live.title.clone(), live.inning_label.to_string());

        app.live_atbat_sel = Some(9999); // 응답에 없는 번호
        let vm = LiveVm::from_app(&app).unwrap();
        assert!(
            vm.now_fields.is_some(),
            "stale 선택은 되감기가 아니다 — 라이브 필드가 그대로 있어야 한다"
        );
        assert_eq!(vm.title, live_title, "Rewind 타이틀이 붙으면 안 된다");
        assert_eq!(vm.inning_label, live_inning);
    }

    /// 최신 타석을 명시적으로 고른 것도 되감기가 아니다(경계) — `]`로 최신까지
    /// 따라오면 라이브 추종으로 복귀한다는 App 쪽 계약과 짝을 이룬다.
    #[test]
    fn selecting_the_newest_at_bat_is_not_rewinding() {
        let mut app = live_app(GameStatus::Live);
        let newest_seq = state_of(&app).at_bats.last().unwrap().seq;
        app.live_atbat_sel = Some(newest_seq);
        let vm = LiveVm::from_app(&app).unwrap();
        assert!(vm.now_fields.is_some());
        assert_eq!(vm.title, crate::ui::i18n::EN.title_live);
    }

    /// 돌려보기 타이틀은 그 타석의 이닝·타자를 담는다(라이브와 헷갈리면 안 된다).
    #[test]
    fn rewind_title_carries_the_past_at_bats_inning_and_batter() {
        let ab = AtBat {
            seq: 1,
            batter_name: "최원준".into(),
            inning_label: "T9".into(),
            relay_lines: vec![],
            pitches: vec![],
        };
        assert_eq!(
            super::rewind_title(&crate::ui::i18n::EN, &ab),
            " Rewind T9 최원준 "
        );
        // 타자명이 유실된 타석은 이닝까지만 — 빈 이름으로 공백이 겹치지 않는다.
        let ab = AtBat {
            batter_name: String::new(),
            ..ab
        };
        assert_eq!(
            super::rewind_title(&crate::ui::i18n::EN, &ab),
            " Rewind T9 "
        );
    }

    // ---- ViewModel: 폭 의존 결정 (터미널 없이 검증) ----

    /// 폭 예산: 남는 폭이 있을 때만 경과/소요를 붙인다 — 좁으면 시간 정보가
    /// 먼저 조용히 빠지고 투수/타자는 절대 밀리지 않는다. 이전엔 렌더 결과
    /// 문자열로만 볼 수 있었다.
    #[test]
    fn the_duration_addition_is_dropped_first_when_the_width_is_tight() {
        let mut app = live_app(GameStatus::Final);
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        let vm = LiveVm::from_app(&app).unwrap();

        let base = vm.detail_line(0);
        let full = vm.detail_line(usize::MAX);
        assert!(full.len() > base.len(), "넉넉한 폭에선 소요가 붙는다");
        assert!(full.contains("Duration"));
        assert!(!base.contains("Duration"), "좁으면 소요부터 빠진다");
        // 경계: 딱 맞는 폭이면 붙고, 한 칸 모자라면 안 붙는다.
        let exact = super::super::text::display_width(&full);
        assert_eq!(vm.detail_line(exact), full);
        assert_eq!(vm.detail_line(exact - 1), base);
        // 투수/타자 같은 기존 정보는 어느 폭에서도 그대로 남는다.
        assert!(base.starts_with("P:"), "기존 정보가 밀리면 안 된다: {base}");
    }

    /// 존 표시 결정은 폭만의 문제가 아니다 — 투구 데이터가 없으면 폭이 아무리
    /// 넓어도 숨긴다(우아한 저하). 그 절반은 폭과 무관한 결정이라 VM에 있다.
    #[test]
    fn the_strike_zone_needs_both_enough_width_and_actual_pitches() {
        let app = live_app(GameStatus::Live);
        let vm = LiveVm::from_app(&app).unwrap();
        assert!(!vm.pitches.is_empty(), "전제: fixture엔 투구가 있다");
        assert!(vm.show_strike_zone(70));
        assert!(!vm.show_strike_zone(69), "좁으면 존을 숨긴다");

        let mut empty = live_app(GameStatus::Live);
        if let Screen::Live { state: Some(s), .. } = &mut empty.screen {
            s.at_bats.last_mut().unwrap().pitches.clear();
        }
        let vm = LiveVm::from_app(&empty).unwrap();
        assert!(
            !vm.show_strike_zone(200),
            "투구가 없으면 폭이 넉넉해도 존을 숨긴다"
        );
    }

    /// 셋째 줄은 "말줄임 대상"과 아닌 것이 타입으로 갈린다 — 선택 투구 상세만
    /// 자르고, 네비 힌트는 폭이 좁아도 그대로 둔다(v0.18 동작 유지). 한 줄로
    /// 뭉뚱그렸다면 좁은 터미널에서 힌트에도 '…'가 붙었을 것이다.
    #[test]
    fn only_the_selected_pitch_detail_is_ellipsized_not_the_navigation_hint() {
        let mut app = live_app(GameStatus::Live);
        let hint = LiveVm::from_app(&app).unwrap().pitch_line;
        assert!(matches!(hint, PitchLine::Plain(_)));
        assert_eq!(
            hint.text(5),
            hint.text(usize::MAX),
            "힌트는 폭에 따라 잘리지 않는다"
        );

        app.live_pitch_sel = Some(0);
        let detail = LiveVm::from_app(&app).unwrap().pitch_line;
        assert!(matches!(detail, PitchLine::Detail(_)));
        assert!(
            detail.text(20).ends_with('…'),
            "긴 상세줄은 정직하게 말줄임한다"
        );
    }

    /// 투구가 하나도 없으면 셋째 줄은 빈 줄이다(힌트도 안 띄운다).
    #[test]
    fn the_pitch_line_is_empty_when_there_are_no_pitches() {
        let mut app = live_app(GameStatus::Live);
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.at_bats.last_mut().unwrap().pitches.clear();
        }
        let vm = LiveVm::from_app(&app).unwrap();
        assert_eq!(vm.pitch_line.text(100), "");
    }

    /// 로딩 중(state 미도착)엔 표현 상태를 만들 수 없다 — 렌더가 로딩 문구로
    /// 저하하는 갈림길.
    #[test]
    fn no_view_model_before_the_live_state_arrives() {
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: String::new(),
                home: team("LG", "LG"),
                away: team("KT", "KT"),
                home_score: None,
                away_score: None,
            },
            state: None,
        };
        assert!(LiveVm::from_app(&app).is_none());
        // 라이브가 아닌 화면도 마찬가지.
        app.screen = Screen::List;
        assert!(LiveVm::from_app(&app).is_none());
    }

    /// 상태 배지는 mono 프리셋에서 색을 걷어낸다(배경 무관 제약, header/games와
    /// 동일 게이트) — 이전엔 렌더 버퍼를 뒤져야 알 수 있었다.
    #[test]
    fn the_status_badge_drops_color_under_the_mono_preset() {
        let mut app = live_app(GameStatus::Suspended);
        let (label, style) = LiveVm::from_app(&app).unwrap().status_badge.unwrap();
        assert_eq!(label, crate::ui::i18n::EN.badge_suspended);
        assert!(style.fg.is_some());

        app.theme_preset = "mono".into();
        let (_, mono) = LiveVm::from_app(&app).unwrap().status_badge.unwrap();
        // M-5(v19a 리뷰): `mono.fg != style.fg`는 mono가 *다른* 색을 내도
        // 통과해 버린다 — 규칙은 "mono는 색이 아예 없다"이므로 그대로 단언한다.
        assert!(mono.fg.is_none(), "mono에서는 색이 아예 없어야 한다");

        // 진행 중 경기엔 배지 자체가 없다.
        let live = live_app(GameStatus::Live);
        assert!(LiveVm::from_app(&live).unwrap().status_badge.is_none());
    }

    // ---- 순수 계산 함수 (live.rs에서 그대로 옮겨 온 기존 테스트) ----

    #[test]
    fn elapsed_label_formats_and_handles_midnight_rollover() {
        assert_eq!(
            super::elapsed_label("2026-07-19T18:30:00", "20:56:14").as_deref(),
            Some("(+2:26)")
        );
        assert_eq!(
            super::elapsed_label("2026-07-19T23:30:00", "00:10:00").as_deref(),
            Some("(+0:40)") // 자정 넘김 보정
        );
        assert_eq!(super::elapsed_label("garbage", "20:56:14"), None);
    }

    /// M-3: 타자명 없는 과거 타석(안내 유실 등)의 상세줄은 "Start 18:30"으로
    /// 바로 시작해야 한다 — 구분자를 무조건 붙이면 detail이 비어 있을 때
    /// "   Start 18:30"처럼 공백 3칸으로 시작한다(실측 버그).
    #[test]
    fn detail_prefix_has_no_leading_padding_when_the_past_at_bats_batter_name_is_missing() {
        let s = bare_state();
        let ab = AtBat {
            seq: 1,
            batter_name: String::new(),
            inning_label: "T1".into(),
            relay_lines: vec![],
            pitches: vec![],
        };
        let got = super::detail_prefix(&crate::ui::i18n::EN, &s, Some(&ab), "18:30");
        assert_eq!(got, "Start 18:30", "must not start with padding: {got:?}");
    }

    /// 대조군: 타자명이 있으면 기존처럼 구분자로 이어붙인다(무회귀).
    #[test]
    fn detail_prefix_still_separates_batter_and_start_time_when_batter_name_is_known() {
        let s = bare_state();
        let ab = AtBat {
            seq: 1,
            batter_name: "최원준".into(),
            inning_label: "T1".into(),
            relay_lines: vec![],
            pitches: vec![],
        };
        let got = super::detail_prefix(&crate::ui::i18n::EN, &s, Some(&ab), "18:30");
        assert_eq!(got, "B: 최원준   Start 18:30");
    }

    // ---- B-2: 투구 간격 (pitch_interval_label 순수 함수) ----

    /// 정상 케이스: 60초 미만은 언어별 접미가 붙은 초 단위("+18s").
    #[test]
    fn pitch_interval_label_formats_seconds_under_a_minute() {
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::EN, "20:56:14", "20:56:32"),
            Some("+18s".to_string())
        );
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::KO, "20:56:14", "20:56:32"),
            Some("+18초".to_string())
        );
    }

    /// 60초 이상은 elapsed_label과 같은 자릿수 표기("+M:SS", 언어 무관).
    #[test]
    fn pitch_interval_label_formats_minutes_and_seconds_at_or_above_a_minute() {
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::EN, "10:00:00", "10:01:45"),
            Some("+1:45".to_string())
        );
    }

    /// 자정 넘김("23:59:50" → "00:00:05")은 음수가 아니라 +24h 보정된 15초.
    #[test]
    fn pitch_interval_label_handles_midnight_rollover_without_going_negative() {
        let got = super::pitch_interval_label(&crate::ui::i18n::EN, "23:59:50", "00:00:05");
        assert_eq!(got, Some("+15s".to_string()));
    }

    /// time_hms 파싱 실패(형식 오류)는 관용적으로 생략(None), 무패닉.
    #[test]
    fn pitch_interval_label_omits_on_parse_failure() {
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::EN, "garbage", "20:56:32"),
            None
        );
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::EN, "20:56:14", "garbage"),
            None
        );
    }

    /// 비현실적으로 큰 간격(30분 초과 — IMPLAUSIBLE_PITCH_GAP_SECS)은 생략한다.
    /// 근거는 함수 주석 참고: HH:MM:SS엔 날짜가 없어 서스펜디드 재개 같은
    /// 다중 시간대 간격을 +24h 보정 하나로는 옳게 못 잡아낸다 — 틀릴 수 있는
    /// 숫자를 보여주느니 생략한다.
    #[test]
    fn pitch_interval_label_omits_implausibly_large_gaps() {
        // 31분 차 — 상한(30분) 초과.
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::EN, "10:00:00", "10:31:00"),
            None
        );
        // 경계값: 정확히 30분은 아직 허용.
        assert_eq!(
            super::pitch_interval_label(&crate::ui::i18n::EN, "10:00:00", "10:30:00"),
            Some("+30:00".to_string())
        );
    }

    // ---- B-3: 경기 소요/경과 ----

    /// UTC epoch → KST "HH:MM:SS" 변환(표시 시간대 app.tz와 무관하게 항상 KST
    /// 고정이어야 한다 — 데이터 자체의 시계가 서울 시계이므로).
    #[test]
    fn now_kst_hms_converts_utc_epoch_to_kst_wall_clock() {
        // epoch 41400 = 1970-01-01T11:30:00Z → KST(UTC+9) 20:30:00.
        assert_eq!(super::now_kst_hms(41400), "20:30:00");
    }

    /// 자정 부근 롤오버도 24시간 범위 안으로 정확히 접힌다(음수 없음).
    #[test]
    fn now_kst_hms_wraps_around_midnight() {
        // epoch 0 = 1970-01-01T00:00:00Z → KST 09:00:00.
        assert_eq!(super::now_kst_hms(0), "09:00:00");
    }

    /// 경기 데이터 안의 마지막 투구 시각 — 마지막 at-bat의 마지막 투구부터
    /// 거슬러 올라간다.
    #[test]
    fn latest_pitch_time_finds_the_most_recent_pitch_across_at_bats() {
        let mut s = bare_state();
        s.at_bats = vec![
            AtBat {
                seq: 1,
                batter_name: "a".into(),
                inning_label: "T1".into(),
                relay_lines: vec![],
                pitches: vec![Pitch {
                    time_hms: Some("18:35:00".into()),
                    ..Default::default()
                }],
            },
            AtBat {
                seq: 2,
                batter_name: "b".into(),
                inning_label: "T2".into(),
                relay_lines: vec![],
                pitches: vec![
                    Pitch {
                        time_hms: Some("18:40:00".into()),
                        ..Default::default()
                    },
                    Pitch {
                        time_hms: Some("18:41:00".into()),
                        ..Default::default()
                    },
                ],
            },
        ];
        assert_eq!(super::latest_pitch_time(&s), Some("18:41:00"));
    }

    /// 마지막 투구의 time_hms가 결측이어도(관용 파싱) 그 앞의 유효한 값으로
    /// 물러선다 — "없으면 생략"이 아니라 "찾을 수 있는 데까지 찾는다".
    #[test]
    fn latest_pitch_time_falls_back_past_a_missing_final_timestamp() {
        let mut s = bare_state();
        s.at_bats = vec![AtBat {
            seq: 1,
            batter_name: "a".into(),
            inning_label: "T1".into(),
            relay_lines: vec![],
            pitches: vec![
                Pitch {
                    time_hms: Some("18:35:00".into()),
                    ..Default::default()
                },
                Pitch {
                    time_hms: None,
                    ..Default::default()
                },
            ],
        }];
        assert_eq!(super::latest_pitch_time(&s), Some("18:35:00"));
    }

    /// at_bats가 비어 있으면(구버전 손 조립 상태) current_pitches로 무회귀
    /// 폴백한다 — active_pitches/active_relay_lines와 같은 관례.
    #[test]
    fn latest_pitch_time_falls_back_to_current_pitches_when_at_bats_is_empty() {
        let mut s = bare_state();
        s.current_pitches = vec![Pitch {
            time_hms: Some("19:00:00".into()),
            ..Default::default()
        }];
        assert_eq!(super::latest_pitch_time(&s), Some("19:00:00"));
    }

    /// 진행 중(Live)은 "지금"(now_hms)까지의 경과를 쓴다.
    #[test]
    fn game_duration_label_uses_now_for_live_games() {
        let got =
            super::game_duration_label(GameStatus::Live, "2026-07-19T18:30:00", "20:30:00", None);
        assert_eq!(got.as_deref(), Some("(+2:00)"));
    }

    /// ★핵심: 종료(Final) 경기는 "지금"이 아니라 데이터 안의 끝점(end_hms)을
    /// 쓴다. 이 테스트는 "지금"이 실제 경기 시각과 무관하게 멀리 떨어진 값
    /// (여기선 시작보다 이른 벽시계 시각이라 잘못 계산하면 자정 넘김 보정까지
    /// 겹쳐 19시간 반짜리 값이 나온다)이어도 결과가 흔들리지 않는지 본다 —
    /// "어제 경기를 오늘 열면 20시간이 나오는" 버그의 직접 재현·회귀 검증.
    #[test]
    fn game_duration_label_uses_the_data_endpoint_not_now_for_final_games() {
        let bogus_now = "14:00:00"; // now가 이걸 썼다면 (+19:30)이 나왔을 것
        let got = super::game_duration_label(
            GameStatus::Final,
            "2026-07-19T18:30:00",
            bogus_now,
            Some("21:07:06"),
        );
        assert_eq!(got.as_deref(), Some("(+2:37)"));
        assert_ne!(got.as_deref(), Some("(+19:30)"));
    }

    /// Suspended도 Final과 같은 취급(끝점 기반) — "지금"을 쓰면 서스펜디드로
    /// 며칠 방치된 경기를 열 때 똑같이 비현실적인 값이 나오기 때문이다.
    #[test]
    fn game_duration_label_uses_the_data_endpoint_for_suspended_games_too() {
        let bogus_now = "14:00:00";
        let got = super::game_duration_label(
            GameStatus::Suspended,
            "2026-07-19T18:30:00",
            bogus_now,
            Some("21:07:06"),
        );
        assert_eq!(got.as_deref(), Some("(+2:37)"));
    }

    /// 종료 경기인데 끝점을 하나도 못 찾으면(투구 데이터 전무) 생략한다.
    #[test]
    fn game_duration_label_omits_when_final_has_no_endpoint() {
        let got =
            super::game_duration_label(GameStatus::Final, "2026-07-19T18:30:00", "14:00:00", None);
        assert_eq!(got, None);
    }

    /// 시작 시각 파싱 실패(빈 문자열 등)는 관용적으로 생략.
    #[test]
    fn game_duration_label_omits_when_start_is_unparseable() {
        let got = super::game_duration_label(GameStatus::Live, "", "20:30:00", Some("21:07:06"));
        assert_eq!(got, None);
    }

    /// M-4: 진행 중 경기인데 "지금"이 시작보다 살짝 앞서면(상태가 시작 전에
    /// Live로 뒤집히거나 사용자 시계가 몇 분 느린 클록 스큐, 실측: 시작 10초
    /// 전) +24h 자정 보정이 거의 24시간짜리 값을 만든다 — 진행 중 경기가
    /// 그렇게 오래 걸릴 수는 없으므로(서스펜디드로 넘어가면 Final/Suspended
    /// 취급이라 이 분기에 오지 않는다) 생략해야 한다.
    #[test]
    fn game_duration_label_omits_when_now_is_slightly_before_start_for_a_live_game() {
        let got = super::game_duration_label(
            GameStatus::Live,
            "2026-07-19T18:30:00",
            "18:29:50", // 시작 10초 전(실측 재현)
            None,
        );
        assert_eq!(
            got, None,
            "must not show a near-24h elapsed for a live game"
        );
    }

    /// 대조군: 상한(IMPLAUSIBLE_LIVE_ELAPSED_SECS=12h) 안쪽의 정상적인 진행
    /// 중 경기는 여전히 값을 보여준다(무회귀).
    #[test]
    fn game_duration_label_still_shows_normal_elapsed_within_the_plausible_bound() {
        let got =
            super::game_duration_label(GameStatus::Live, "2026-07-19T18:30:00", "22:30:00", None);
        assert_eq!(got.as_deref(), Some("(+4:00)"));
    }
}
