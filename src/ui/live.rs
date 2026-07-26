use super::strikezone;
use super::theme::{self, team_badge_style};
use crate::app::{App, Screen};
use crate::localtime::KST_OFFSET_SECS;
use crate::model::{AtBat, Game, GameStatus, LiveState, Pitch};
use crate::ui::i18n::Labels;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
    Frame,
};

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

/// 라이브 뷰: 스코어라인(점수/카운트/주자/승률) + 문자중계(+ 폭 충분 시 스트라이크존).
/// v0.18부터 `app.live_atbat_sel`이 가리키는 at-bat(과거일 수도 있음)을
/// "활성" 데이터로 삼는다 — None(기본값)이면 최신(라이브)과 완전히 동일하게
/// 그려져 기존 화면과 무회귀다.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let l = app.labels();
    let Screen::Live { game, state } = &app.screen else {
        return;
    };
    let Some(s) = state else {
        f.render_widget(
            Paragraph::new(l.loading).block(Block::bordered().title(l.title_live)),
            area,
        );
        return;
    };

    // 활성 at-bat 해석: 고른 번호가 **실제로 응답에 있고** 최신이 아닐 때만 "과거를
    // 보는 중"이다. 번호만 보고 판정하면, 응답에서 사라진 stale 번호(이닝 전환)에도
    // Rewind 타이틀이 붙는데 내용은 active_at_bat이 낮춘 최신 타석이라 라벨과 내용이
    // 어긋난다 — 없는 타석을 있는 척 보여주지 않는다는 게 이 기능의 계약이다.
    let active = s.active_at_bat(app.live_atbat_sel);
    let viewing_past = matches!(
        (app.live_atbat_sel, active, s.at_bats.last()),
        (Some(seq), Some(ab), Some(newest)) if ab.seq == seq && seq != newest.seq
    );
    let title = match (viewing_past, active) {
        (true, Some(ab)) => rewind_title(l, ab),
        _ => l.title_live.to_string(),
    };
    let pitches = s.active_pitches(app.live_atbat_sel);
    let relay_lines = s.active_relay_lines(app.live_atbat_sel);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    render_scoreline(
        f,
        rows[0],
        s,
        game,
        app.live_pitch_sel,
        l,
        &app.theme_preset,
        &title,
        pitches,
        app.now_secs,
        if viewing_past { active } else { None },
    );

    // 폭이 좁거나 아직 투구 데이터가 없으면 존을 숨기고 중계에 본문 전체를 준다(우아한 저하).
    let wide = rows[1].width >= 70 && !pitches.is_empty();
    if wide {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        render_relay(
            f,
            cols[0],
            relay_lines,
            app.live_relay_cursor,
            l.title_relay,
        );
        strikezone::render(
            f,
            cols[1],
            pitches,
            app.live_pitch_sel,
            l,
            &app.theme_preset,
        );
    } else {
        render_relay(
            f,
            rows[1],
            relay_lines,
            app.live_relay_cursor,
            l.title_relay,
        );
    }
}

fn win_pct(rate: Option<f32>) -> String {
    rate.map(|r| format!("{:.0}%", r * 100.0))
        .unwrap_or_else(|| "-".into())
}

/// 스코어라인 3번째 줄(디테일)의 "투수/타자(또는 되감기 중 타자만) + 시작
/// 시각" 부분. 경기 경과/소요(B-3 addition, 폭 예산이 필요해 area.width가
/// 있어야 하는 render_scoreline 쪽에서 별도로 붙인다)는 여기 포함하지 않는다
/// — 순수 함수로 남겨야 render 없이 직접 단위 테스트할 수 있다.
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

#[allow(clippy::too_many_arguments)]
fn render_scoreline(
    f: &mut Frame,
    area: Rect,
    s: &LiveState,
    game: &Game,
    sel: Option<usize>,
    l: &'static Labels,
    preset: &str,
    title: &str,
    pitches: &[Pitch],
    now_secs: u64,
    // 돌려보기 중이면 그 과거 타석, 라이브를 보는 중이면 None.
    past_at_bat: Option<&AtBat>,
) {
    let status = game.status;
    // 3슬롯 ASCII 주자 표시: [3루 2루 1루], 빈 베이스는 '-' — 폭 고정.
    let bases = format!(
        "[{} {} {}]",
        if s.bases.third { "3" } else { "-" },
        if s.bases.second { "2" } else { "-" },
        if s.bases.first { "1" } else { "-" },
    );

    // 되감기 중엔 이닝도 활성 at-bat(과거) 것으로 바꾼다 — 안 바꾸면 타이틀은
    // rewind_title이 만든 과거 이닝(예: T9)을, 바로 이 줄은 라이브 이닝(B9)을
    // 말해 한 화면이 두 이닝을 동시에 말한다(4912944가 타자에 대해 고친
    // 것과 정확히 같은 결함이 이닝 축에 남아 있었다 — 리뷰 I-1). 이닝은
    // AtBat.inning_label로 확실히 아는 값이라 바꿀 수 있다.
    let inning_label: &str = match past_at_bat {
        Some(ab) => &ab.inning_label,
        None => &s.inning_label,
    };
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut spans = vec![
        Span::styled(s.away.name.as_str(), team_badge_style(&s.away.code)),
        Span::raw(" "),
        Span::styled(s.away_score.to_string(), bold),
        Span::raw(" : "),
        Span::styled(s.home_score.to_string(), bold),
        Span::raw(" "),
        Span::styled(s.home.name.as_str(), team_badge_style(&s.home.code)),
        Span::raw("   "),
        Span::raw(inning_label),
    ];
    // 서스펜디드/종료 경기는 스코어라인만 봐서는 진행 중인 경기와 구분이
    // 안 된다 — inning_label 옆에 배지를 붙여 우아하게 저하시킨다.
    if let Some((label, style)) = status_badge(status, l, preset) {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(label, style));
    }
    // B/S/O·주자·WP는 "지금 이 순간"의 값이라, 과거 타석을 보는 중에 그대로
    // 두면 그 타석의 카운트인 것처럼 오해된다(4912944가 P:/Next:를 비운 것과
    // 같은 원칙 — 과거 타석에 대해 확실히 아는 것만 보여주고 모르는 건
    // 비운다). 이닝은 아는 값이라 위에서 이미 바꿨지만, 이 셋은 응답이
    // 과거 타석 기준으로 알려주지 않으므로 되감기 중엔 통째로 생략한다.
    if past_at_bat.is_none() {
        spans.extend([
            Span::raw("   "),
            Span::raw(format!(
                "B{} S{} O{}",
                s.count.ball, s.count.strike, s.count.out
            )),
            Span::raw("   "),
            Span::raw(bases),
            Span::raw("   "),
            Span::raw(format!(
                "WP {}/{}",
                win_pct(s.away_win_rate),
                win_pct(s.home_win_rate)
            )),
        ]);
    }
    let score_line = Line::from(spans);

    // "HH:MM" 경기 시작 시각("....THH:MM:SS"에서 추출, 실패 시 생략).
    let start_hhmm = game
        .start
        .split('T')
        .nth(1)
        .and_then(|t| t.get(0..5))
        .unwrap_or("");
    let mut detail = detail_prefix(l, s, past_at_bat, start_hhmm);
    // B-2/B-3(v0.18): 경기 경과/소요. Live는 "시작~지금", Final/Suspended는
    // "시작~데이터 안의 마지막 투구 시각"(§2 B-3, ★핵심) — Suspended를 Final과
    // 묶은 이유: "지금"을 쓰면 서스펜디드 상태로 며칠 방치된 경기를 열 때도
    // Final과 똑같이 비현실적인 값(수십 시간)이 나오기 때문이다. 이 화면엔
    // Scheduled/Canceled가 들어오지 않으므로(can_enter_live) 그 두 경우는 값이
    // 없다.
    //
    // 폭 예산은 header.rs A-1/A-2(v0.15)와 같은 방식으로 처리한다: 문자열을
    // 만들고 나서 ellipsize로 뒤를 자르는 대신, **붙이기 전에** 남은 폭을
    // 계산해 들어갈 때만 붙인다 — 그래야 좁은 터미널에서 시간 정보가 먼저
    // 조용히 빠지고 투수/타자 같은 기존 정보는 절대 밀리지 않는다.
    if let Some(dur) = game_duration_label(
        status,
        &game.start,
        &now_kst_hms(now_secs),
        latest_pitch_time(s),
    ) {
        let label = if status == GameStatus::Live {
            l.lbl_elapsed
        } else {
            l.lbl_duration
        };
        let addition = format!("   {label} {dur}");
        let inner_width = area.width.saturating_sub(2) as usize;
        if inner_width
            >= super::text::display_width(&detail) + super::text::display_width(&addition)
        {
            detail.push_str(&addition);
        }
    }
    let detail_line = Line::from(detail);

    // 셋째 줄: 선택된 투구 상세(시각·상대시간·결과 원문) 또는 네비 힌트.
    // `pitches`는 활성 at-bat(라이브 또는 돌려보기 중인 과거 타석)의 투구다.
    let pitch_line = match sel.and_then(|i| pitches.get(i).map(|p| (i, p))) {
        Some((i, p)) => {
            let speed = p
                .speed_kmh
                .map(|k| format!("{k}km"))
                .unwrap_or_else(|| "-".into());
            let time = p.time_hms.as_deref().unwrap_or("-");
            let rel = p
                .time_hms
                .as_deref()
                .and_then(|t| elapsed_label(&game.start, t))
                .unwrap_or_default();
            // B-2(v0.18): 직전 투구 대비 경과("+18초"류) — 첫 투구(i==0)는 직전이
            // 없으므로 생략, i.time_hms 결측·파싱 실패도 관용적으로 생략. 폭
            // 예산은 이 줄 전체를 감싸는 아래 ellipsize 한 번으로 충분하다(B-2는
            // 새 칸이 아니라 이미 있는 상세줄 안에 끼워 넣는 값이라 header.rs류
            // 별도 폭 계산이 필요 없다) — i==0일 때 interval이 빈 문자열이라
            // 기존 렌더와 완전히 동일해 무회귀도 자동으로 만족한다.
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
            // 결과 원문이 길면 좁은 터미널에서 조용히 잘린다 — 정직한 말줄임
            // (테두리 2칸 제외한 내부 폭 기준, §15 오버플로 정책).
            Line::from(super::text::ellipsize(
                &format!(
                    "{} {}/{}  {}  {} {}{interval}  {}",
                    l.pitch_word,
                    i + 1,
                    pitches.len(),
                    speed,
                    time,
                    rel,
                    p.text
                ),
                area.width.saturating_sub(2) as usize,
            ))
        }
        None if !pitches.is_empty() => Line::from(format!(
            "{} {}  {}",
            l.pitches_word,
            pitches.len(),
            l.inspect_hint
        )),
        None => Line::from(""),
    };

    f.render_widget(
        Paragraph::new(vec![score_line, detail_line, pitch_line])
            .block(Block::bordered().title(title)),
        area,
    );
}

/// "HH:MM:SS" → 자정 기준 경과 초. 게임 시작 시각(elapsed_label)과 예정 경기
/// 카운트다운(games.rs::scheduled_eta_hm, v0.15 A-3)이 공유하는 시:분:초 파서 —
/// 두 곳 다 같은 "HH:MM:SS" 원문 포맷을 다루므로 파싱 자체(자릿수·범위 검증)는
/// 공유하되, "자정 넘김을 어떻게 보정할지"는 호출부마다 다르다(elapsed_label은
/// 항상 미래 방향이라 +24h 고정 보정이 안전하지만, A-3는 날짜가 있는 절대시각
/// 비교라 이 값만으로 보정하면 안 된다 — games.rs 쪽 주석 참고). 파싱 실패는
/// None(관용 — 표시 생략).
pub(crate) fn parse_hms_secs(hms: &str) -> Option<i64> {
    let mut it = hms.split(':');
    let h: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let s: i64 = it.next().unwrap_or("0").parse().ok()?;
    ((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&s))
        .then_some(h * 3600 + m * 60 + s)
}

/// 경기 시작("....THH:MM:SS")과 어떤 시각("HH:MM:SS")의 차(초) — 자정 넘김
/// (그 시각 < 시작)은 +24h 보정. elapsed_label(표시용 포맷)과
/// game_duration_label(M-4 Live 상한 가드)이 공유한다. 파싱 실패는 None(관용).
fn elapsed_secs(game_start: &str, hms: &str) -> Option<i64> {
    let start = parse_hms_secs(game_start.split('T').nth(1)?)?;
    let cur = parse_hms_secs(hms)?;
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
    let prev = parse_hms_secs(prev_hms)?;
    let cur = parse_hms_secs(cur_hms)?;
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
///
/// M-4: 진행 중(Live) 경기인데 `now`가 `start`보다 앞서면(상태가 시작 전에
/// Live로 뒤집히거나 사용자 시계가 몇 분 느린 클록 스큐, 실측: 시작 10초
/// 전) `elapsed_secs`의 +24h 자정 보정이 거의 24시간짜리 값("Elapsed
/// (+23:59)")을 만든다 — 진행 중 경기가 그렇게 오래 걸릴 수는 없다(서스펜디드로
/// 넘어가면 Final/Suspended 취급이라 애초에 이 분기에 오지 않는다). B-2가
/// IMPLAUSIBLE_PITCH_GAP_SECS로 막은 것과 같은 위험이라 같은 원칙(생략)으로
/// 막는다 — 상한은 [`IMPLAUSIBLE_LIVE_ELAPSED_SECS`].
const IMPLAUSIBLE_LIVE_ELAPSED_SECS: i64 = 12 * 3600;

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

/// 문자중계 목록. `cursor`가 None이면(기본 상태, 기존 무회귀) 꼬리(N줄)만
/// 하이라이트 없이 보여준다 — 오래된→최신 순 저장이라 이렇게 자르면 최신이
/// 리스트 맨 아래에 온다. `cursor`가 Some(i)면(v0.18 돌려보기 j/k 커서)
/// 전체 줄을 ListState 기반 스테이트풀 리스트로 그려 i번째를 반전 하이라이트
/// 하고, ratatui가 그 줄이 보이도록 자동으로 스크롤한다(settings.rs·
/// newslist.rs와 같은 ListState 관용).
fn render_relay(
    f: &mut Frame,
    area: Rect,
    lines: &[String],
    cursor: Option<usize>,
    title: &'static str,
) {
    match cursor {
        Some(idx) => {
            let items: Vec<ListItem> = lines
                .iter()
                .map(|entry| ListItem::new(format!("· {entry}")))
                .collect();
            let widget = List::new(items)
                .block(Block::bordered().title(title))
                .highlight_symbol("> ")
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            let mut state = ListState::default();
            state.select(Some(idx.min(lines.len().saturating_sub(1))));
            f.render_stateful_widget(widget, area, &mut state);
        }
        None => {
            let n = area.height.saturating_sub(2) as usize;
            let start = lines.len().saturating_sub(n);
            let items: Vec<ListItem> = lines[start..]
                .iter()
                .map(|entry| ListItem::new(format!("· {entry}")))
                .collect();
            f.render_widget(List::new(items).block(Block::bordered().title(title)), area);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Screen};
    use crate::model::{AtBat, BaseState, Count, Game, GameStatus, LiveState, Pitch, Team};
    use ratatui::{backend::TestBackend, Terminal};

    const RELAY: &str = include_str!("../../tests/fixtures/relay_20260719KTLG.json");

    fn team(code: &str, name: &str) -> Team {
        Team {
            code: code.into(),
            name: name.into(),
        }
    }

    fn live_screen() -> Screen {
        live_screen_with_status(GameStatus::Live)
    }

    fn live_screen_with_status(status: GameStatus) -> Screen {
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
        Screen::Live {
            game,
            state: Some(state),
        }
    }

    /// fixture 대신 손으로 조립한 최소 상태(B-3 폭 예산 테스트 전용) — 이름
    /// 길이를 정확히 통제해야 "딱 안 들어가는 폭"을 결정적으로 고를 수 있다.
    /// `at_bats`를 비우고 `current_pitches`에 마지막 투구 하나만 담아
    /// `latest_pitch_time`의 무회귀 폴백 경로도 함께 검증한다.
    fn bare_live_screen(status: GameStatus, game_start: &str, end_hms: &str) -> Screen {
        let state = LiveState {
            inning_label: "T9".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: 3,
            away_score: 2,
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
            pitcher_name: "Kim".into(),
            batter_name: "Lee".into(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![Pitch {
                time_hms: Some(end_hms.into()),
                ..Default::default()
            }],
            next_batter_name: String::new(),
            at_bats: vec![],
        };
        let game = Game {
            id: "g".into(),
            start: game_start.into(),
            status,
            status_label: "".into(),
            home: team("LG", "LG"),
            away: team("KT", "KT"),
            home_score: Some(3),
            away_score: Some(2),
        };
        Screen::Live {
            game,
            state: Some(state),
        }
    }

    /// `latest_pitch_time` 순수 함수 테스트 전용 최소 `LiveState` — 렌더와
    /// 무관하므로 team/score 등은 아무 값이나 둔다.
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

    fn render_to_string(app: &App, width: u16, height: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// header.rs가 게임 목록 전체를 훑어 "LIVE {n}"/"FINAL {n}" 탈리를 항상 그리므로,
    /// 배지 텍스트("SUSPENDED"/"FINAL") 유무를 검사할 때 전체 앱(crate::ui::draw)을
    /// 쓰면 header의 상시 표시 텍스트와 우연히 겹친다 — live::render만 직접 그려 피한다.
    fn render_live_view_only(app: &App, width: u16, height: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| super::render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn live_view_shows_score_count_and_relay() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let text = render_to_string(&app, 100, 30);
        assert!(text.contains('4')); // away score (fixture)
                                     // 값 특정 검사: "B{} S{} O{}" 포맷 리터럴 자체가 항상 B/S/O 글자를
                                     // 포함하므로(값이 뒤바뀌어도 통과) count 값까지 함께 확인한다.
                                     // fixture 실측값: ball=2, strike=3, out=3 (tests/parse_relay.rs와 동일).
        assert!(text.contains("B2 S3 O3"));
        // ratatui는 전각(2-width) 문자 뒤에 placeholder 공백 셀을 채워 넣으므로
        // (ui/mod.rs 테스트와 동일한 이유) 공백을 제거하고 부분 문자열을 검사한다.
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains("파울") || compact.contains("헛스윙"));
    }

    #[test]
    fn live_view_renders_without_panic_when_narrow() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        // width < 70 → strike zone hidden, relay gets full body; must not panic.
        let _text = render_to_string(&app, 50, 20);
    }

    #[test]
    fn live_view_shows_suspended_badge_for_suspended_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Suspended);
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("SUSPENDED"));
    }

    #[test]
    fn live_view_shows_final_badge_for_finished_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("FINAL"));
    }

    #[test]
    fn live_view_shows_no_badge_for_live_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Live);
        let text = render_live_view_only(&app, 100, 30);
        assert!(!text.contains("SUSPENDED"));
        assert!(!text.contains("FINAL"));
    }

    #[test]
    fn scoreline_team_name_has_team_color_badge() {
        let mut app = App::new(Default::default());
        // away = 두산(OB) 로 스코어라인 렌더
        let state =
            crate::source::naver::map::live_from_relay(RELAY, team("OB", "두산"), team("LG", "LG"))
                .unwrap();
        let game = Game {
            id: "g".into(),
            start: "".into(),
            status: GameStatus::Live,
            status_label: state.inning_label.clone(),
            home: team("LG", "LG"),
            away: team("OB", "두산"),
            home_score: Some(state.home_score),
            away_score: Some(state.away_score),
        };
        app.screen = Screen::Live {
            game,
            state: Some(state),
        };
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| super::render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer();
        let has_badge = buf
            .content()
            .iter()
            .any(|c| c.bg == super::super::theme::team_color("OB"));
        assert!(
            has_badge,
            "scoreline away team name should render on OB team-color background"
        );
    }

    /// 선택된 투구의 상세줄: 순번/전체, 구속, 시각, 상대시간, 결과 원문 전부.
    #[test]
    fn selected_pitch_detail_line_shows_speed_time_elapsed_and_text() {
        let mut app = App::new(Default::default());
        app.screen = live_screen(); // fixture 기반
                                    // fixture의 첫 투구를 선택하고 시각을 주입해 결정적으로 검증한다.
        if let Screen::Live {
            game,
            state: Some(s),
        } = &mut app.screen
        {
            game.start = "2026-07-19T18:30:00".into();
            s.current_pitches[0].time_hms = Some("20:56:14".into());
            // 렌더는 active_pitches(v0.18)를 통해 at_bats의 마지막 항목을 읽으므로
            // (current_pitches는 무회귀용 미러) 여기도 함께 갱신해야 반영된다.
            s.at_bats.last_mut().unwrap().pitches[0].time_hms = Some("20:56:14".into());
        }
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("Pitch 1/"), "detail line missing:\n{text}");
        assert!(text.contains("20:56:14"));
        assert!(text.contains("(+2:26)"));
    }

    #[test]
    fn unselected_live_view_advertises_pitch_navigation() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("Left/Right"), "nav hint missing:\n{text}");
    }

    #[test]
    fn detail_line_shows_next_batter_when_known() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.next_batter_name = "홍창기".into();
        }
        let text = render_live_view_only(&app, 100, 30);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("Next:홍창기"),
            "next batter missing:\n{text}"
        );
    }

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

    /// 긴 결과 원문은 상세줄에서 말줄임된다(§15 오버플로 정책).
    #[test]
    fn long_pitch_text_is_ellipsized_in_the_detail_line() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            let long_text = "매우 긴 결과 설명 ".repeat(20);
            s.current_pitches[0].text = long_text.clone();
            // active_pitches(v0.18)가 at_bats의 마지막 항목을 읽으므로 함께 갱신.
            s.at_bats.last_mut().unwrap().pitches[0].text = long_text;
        }
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 80, 30);
        assert!(
            text.contains('…'),
            "expected honest ellipsis in detail line"
        );
    }

    #[test]
    fn korean_live_labels_render_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.screen = live_screen();
        let text = render_live_view_only(&app, 100, 30);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("문자중계"),
            "relay title must be Korean:\n{text}"
        );
        assert!(compact.contains("투수:") && compact.contains("타자:"));
        assert!(text.contains("좌우 키로 하나씩") || compact.contains("투구"));
    }

    #[test]
    fn live_view_shows_loading_when_state_none() {
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: "".into(),
                home: team("LG", "LG"),
                away: team("KT", "KT"),
                home_score: None,
                away_score: None,
            },
            state: None,
        };
        let text = render_to_string(&app, 100, 30);
        assert!(text.contains("Live"));
        assert!(text.contains("loading"));
    }

    /// v0.18 돌려보기: 과거 타석을 고르면 타이틀에 그 타자·이닝이 드러나고,
    /// 기본 라이브 타이틀(" Live ")은 더는 보이지 않는다 — 라이브와 헷갈리면
    /// 안 된다는 제약의 직접 검증. fixture 실측: 가장 오래된 타석(index 0)은
    /// 최원준.
    #[test]
    fn selecting_a_past_at_bat_shows_its_batter_and_inning_instead_of_the_live_title() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let live_text = render_live_view_only(&app, 100, 30);
        assert!(
            live_text.contains(" Live "),
            "default view keeps the live title"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석의 textRelay no
        let past_text = render_live_view_only(&app, 100, 30);
        let compact: String = past_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("최원준"),
            "past at-bat's batter must appear in the title:\n{past_text}"
        );
        assert!(
            compact.contains("Rewind"),
            "must be explicitly marked as rewinding:\n{past_text}"
        );
        assert!(
            !past_text.contains(" Live "),
            "must not show the plain live title while rewinding:\n{past_text}"
        );
    }

    /// 돌려보기 중에는 스코어라인의 투수/타자 줄도 그 타석을 따라야 한다. 이 줄이
    /// 라이브 값으로 남아 있으면 타이틀은 과거 타자를, 바로 아랫줄은 현재 타자를
    /// 말해 한 화면이 서로 다른 두 타석을 가리킨다 — 실행 확인에서 잡힌 결함이라
    /// (타이틀·투구 수만 검증하던 기존 테스트는 통과했다) 여기서 봉인한다.
    #[test]
    fn rewinding_replaces_the_live_batter_line_instead_of_leaving_it_behind() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();

        let live_text = render_live_view_only(&app, 100, 30);
        let live_compact: String = live_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            live_compact.contains("천성호"),
            "전제: 라이브에서는 현재 타자가 보인다:\n{live_text}"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석
        let past_text = render_live_view_only(&app, 100, 30);
        let compact: String = past_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("최원준"),
            "보고 있는 타석의 타자가 있어야 한다:\n{past_text}"
        );
        assert!(
            !compact.contains("천성호"),
            "라이브 타자가 남으면 한 화면이 두 타자를 말한다:\n{past_text}"
        );
    }

    /// I-1: 되감기 중엔 스코어라인의 이닝도 활성 at-bat 것으로 바뀌어야 한다.
    /// 타이틀(rewind_title)은 이미 T9(최원준)를 말하는데 바로 아랫줄
    /// 스코어라인이 라이브 이닝(B9)을 그대로 두면 한 화면이 두 이닝을
    /// 동시에 말한다 — 4912944가 타자에 대해 고친 것과 정확히 같은 결함이
    /// 이닝 축에 남아 있었다(최종 리뷰 지적). 동시에 B/S/O·주자·WP는 "지금
    /// 이 순간"의 값이라 과거 타석 옆에 두면 그 타석의 카운트로 오해된다 —
    /// 이닝(아는 값)은 바꾸고, 카운트류(모르는 값)는 비운다.
    #[test]
    fn rewinding_replaces_the_scoreline_inning_and_hides_live_only_fields() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();

        let live_text = render_live_view_only(&app, 100, 30);
        assert!(
            live_text.contains("B9"),
            "전제: 라이브 이닝은 B9:\n{live_text}"
        );
        assert!(
            live_text.contains("B2 S3 O3"),
            "전제: 라이브에서는 카운트가 보인다:\n{live_text}"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석(T9)
        let past_text = render_live_view_only(&app, 100, 30);
        assert!(
            past_text.contains("T9"),
            "스코어라인 이닝이 활성 at-bat(T9)으로 바뀌어야 한다:\n{past_text}"
        );
        assert!(
            !past_text.contains("B9"),
            "라이브 이닝(B9)이 남아 있으면 한 화면이 두 이닝을 말한다:\n{past_text}"
        );
        assert!(
            !past_text.contains("B2 S3 O3"),
            "돌려보기 중엔 카운트를 비워야 한다(지금 이 순간 값이라 오해를 부른다):\n{past_text}"
        );
    }

    /// M-9: "과거 타석을 고르면 그 타석의 문자중계가 뜬다"를 직접 확인한다
    /// (기존엔 "라이브 타자 이름이 남아 있으면 실패"하는 식으로만 우연히
    /// 잡혔다 — active_relay_lines가 sel을 무시해도 안 걸릴 수 있는 검증).
    /// fixture 실측: 천성호(라이브, no=97)의 마지막 줄은 "포수 태그아웃"으로
    /// 끝나고, 최원준(과거, no=87)의 마지막 줄은 "볼넷"으로 끝난다 — 서로
    /// 배타적인 결과 문구라 어느 타석의 중계가 실제로 그려졌는지 직접 안다.
    #[test]
    fn selecting_a_past_at_bat_actually_swaps_in_that_at_bats_own_relay_lines() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();

        let live_text = render_live_view_only(&app, 100, 30);
        let live_compact: String = live_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            live_compact.contains("포수태그아웃"),
            "전제: 라이브 문자중계는 현재 타석(천성호) 것이다:\n{live_text}"
        );
        assert!(!live_compact.contains("볼넷"));

        app.live_atbat_sel = Some(87); // fixture 최원준 타석
        let past_text = render_live_view_only(&app, 100, 30);
        let compact: String = past_text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("볼넷"),
            "과거 타석(최원준)의 문자중계가 실제로 그려져야 한다:\n{past_text}"
        );
        assert!(
            !compact.contains("포수태그아웃"),
            "라이브 문자중계가 남아 있으면 안 된다:\n{past_text}"
        );
    }

    /// 과거 타석을 보는 중엔 존/측면·투구 상세줄이 그 타석 자신의 투구를 써야
    /// 한다(현재 타석 것이 아니라) — fixture 실측: 최신(천성호)은 5구, 가장
    /// 오래된 타석(최원준)은 7구.
    #[test]
    fn selecting_a_past_at_bat_uses_that_at_bats_own_pitch_count() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let live_text = render_live_view_only(&app, 100, 30);
        assert!(
            live_text.contains("Pitches 5"),
            "live at-bat has 5 pitches:\n{live_text}"
        );

        app.live_atbat_sel = Some(87); // fixture 최원준 타석의 textRelay no
        let past_text = render_live_view_only(&app, 100, 30);
        assert!(
            past_text.contains("Pitches 7"),
            "past at-bat must report its own pitch count, not the live at-bat's:\n{past_text}"
        );
    }

    /// 문자중계 커서(v0.18 j/k)는 live_relay_cursor가 Some일 때만 하이라이트를
    /// 그린다 — 기본 상태(None)는 기존과 똑같이 하이라이트가 없어야 한다
    /// (기존 라이브 화면 무회귀).
    #[test]
    fn relay_cursor_highlight_only_appears_when_a_line_is_selected() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        let default_text = render_live_view_only(&app, 100, 30);
        assert!(
            !default_text.contains("> ·"),
            "no cursor highlight in the default view:\n{default_text}"
        );

        app.live_relay_cursor = Some(0);
        let cursor_text = render_live_view_only(&app, 100, 30);
        assert!(
            cursor_text.contains("> ·"),
            "cursor highlight must appear once a relay line is selected:\n{cursor_text}"
        );
    }

    /// I-4: 되감기(문자중계 커서)의 존재 이유 자체를 증명한다 — 패널 높이보다
    /// 긴 문자중계에서 커서 없는 기본 뷰는 꼬리(최신) N줄만 보여주지만
    /// (무회귀), 커서가 화면 밖으로 스크롤된 오래된 줄을 가리키면 ratatui의
    /// 자동 스크롤로 그 줄이 실제로 렌더 결과에 나타나야 한다. 리뷰가
    /// 뮤테이션(Some(idx) 분기에도 꼬리 슬라이스를 되살림 — "커서는 있지만
    /// 되감아 볼 수는 없는" 상태)으로 보였듯, 커서 심볼 유무만 보는 위
    /// 테스트(relay_cursor_highlight_only_appears_when_a_line_is_selected)는
    /// 이 결함을 못 잡는다 — 이 테스트는 내용 자체를 직접 확인한다.
    #[test]
    fn moving_the_relay_cursor_to_an_old_line_reveals_it_past_the_default_tail_window() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        // fixture의 최신 at-bat 문자중계는 몇 줄뿐이라, 패널 높이보다 확실히
        // 긴 인위적인 relay_lines로 교체해 "꼬리만 보이는" 창을 만든다.
        let long_lines: Vec<String> = (0..30).map(|i| format!("line-{i}")).collect();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.at_bats.last_mut().unwrap().relay_lines = long_lines;
        }

        let default_text = render_live_view_only(&app, 100, 30);
        assert!(
            !default_text.contains("line-0"),
            "전제: 커서 없는 기본 뷰는 꼬리만 보여줘 가장 오래된 줄(line-0)이 \
             안 보여야 한다:\n{default_text}"
        );

        // 커서를 맨 위(가장 오래된 줄, 인덱스 0)로 옮긴다.
        app.live_relay_cursor = Some(0);
        let cursor_text = render_live_view_only(&app, 100, 30);
        assert!(
            cursor_text.contains("line-0"),
            "커서가 화면 밖 오래된 줄을 가리키면 그 줄이 실제로 보여야 한다 \
             (되감기 기능의 핵심 계약):\n{cursor_text}"
        );
    }

    /// 돌려보기 + 문자중계 커서 + 좁은 폭 조합이 패닉하지 않는다(무패닉 제약).
    ///
    /// M-1: seq는 인덱스가 아니라 응답의 textRelay `no`다 — fixture의 실제
    /// seq 범위는 86~98이라 `Some(0)`은 `active_at_bat`이 못 찾고 즉시
    /// 최신으로 낮춰(폴백) 실제로는 되감기 경로(rewind_title·과거 타자줄)를
    /// 전혀 안 타면서도 통과해 버렸다(이름과 달리 무의미한 테스트). fixture
    /// 최원준 타석의 실제 seq(87)를 써야 진짜로 그 경로를 태운다.
    #[test]
    fn rewind_view_with_relay_cursor_renders_without_panic_when_narrow() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        app.live_atbat_sel = Some(87);
        app.live_relay_cursor = Some(0);
        let _text = render_to_string(&app, 40, 15);
    }

    /// 첫/마지막 타석 경계에서도(sel이 범위를 벗어나도) 패닉 없이 clamp된
    /// 결과를 보여준다 — active_at_bat의 clamp가 렌더까지 이어지는지 확인.
    #[test]
    fn unknown_at_bat_selection_falls_back_to_live_instead_of_panicking() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        // 응답에 없는 번호(이닝이 넘어가 배열이 갈린 뒤 남은 stale 선택).
        app.live_atbat_sel = Some(9999);
        let text = render_live_view_only(&app, 100, 30);
        // 최신 타석으로 낮아지므로 라이브 타이틀 그대로 — 없는 타석을 있는 척
        // 그리지 않는다(App::apply가 폴링 때 선택 자체를 되돌린다).
        assert!(
            text.contains(" Live "),
            "unknown seq falls back to the latest at-bat:\n{text}"
        );
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

    /// 실제 fixture 간격(천성호 타석 1구→2구, 19초)이 화면에 "+19s"로 나온다.
    /// i>0(직전 투구가 있는 경우)만 간격을 붙인다는 계약의 통합 검증.
    #[test]
    fn pitch_interval_appears_for_the_second_pitch_using_real_fixture_gap() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        app.live_pitch_sel = Some(1); // 천성호 2구째(fixture 실측: 21:05:40→21:05:59, 19초)
        let text = render_live_view_only(&app, 100, 30);
        assert!(text.contains("+19s"), "expected pitch interval:\n{text}");
    }

    /// 첫 투구(i==0)는 직전이 없으므로 간격을 붙이지 않는다 — 붙였다면 i-1
    /// 인덱스가 언더플로해 패닉하거나(usize) 엉뚱한 값을 보여줬을 것이다.
    #[test]
    fn pitch_interval_is_absent_for_the_first_pitch() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        app.live_pitch_sel = Some(0);
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains("+19s") && !text.contains("+18s"),
            "no interval expected for the first pitch:\n{text}"
        );
    }

    /// 직전 투구의 time_hms가 결측이면(관용 파싱) 간격을 생략한다 — 무패닉.
    #[test]
    fn pitch_interval_is_absent_when_previous_pitch_time_is_missing() {
        let mut app = App::new(Default::default());
        app.screen = live_screen();
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            s.current_pitches[0].time_hms = None;
            s.at_bats.last_mut().unwrap().pitches[0].time_hms = None;
        }
        app.live_pitch_sel = Some(1);
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains("+19s"),
            "interval must be omitted when the previous pitch's time is missing:\n{text}"
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

    /// 진행 중 경기: 스코어라인에 "Elapsed (+H:MM)"이 실제로 그려진다.
    #[test]
    fn live_view_shows_elapsed_time_for_a_live_game() {
        let mut app = App::new(Default::default());
        app.screen = live_screen(); // GameStatus::Live
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 41_400; // KST 20:30:00 → 시작 대비 +2:00
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            text.contains("Elapsed (+2:00)"),
            "expected live elapsed time:\n{text}"
        );
    }

    /// M-4 통합 검증: "지금"이 시작 10초 전인 진행 중 경기를 실제로 렌더해도
    /// "Elapsed (+23:59)" 같은 값이 화면에 나오지 않는다(Elapsed 자체가 생략).
    #[test]
    fn live_view_omits_elapsed_when_now_is_slightly_before_game_start() {
        let mut app = App::new(Default::default());
        app.screen = live_screen(); // GameStatus::Live
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 34_190; // KST 18:29:50 — 시작 10초 전(클록 스큐 실측 재현)
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains("Elapsed"),
            "must not show an implausible near-24h elapsed:\n{text}"
        );
    }

    /// ★핵심 회귀: 종료 경기를 "지금"과 무관한 값(어제 경기를 오늘 연 상황을
    /// 흉내낸 이른 벽시계 now_secs)으로 열어도 총 소요가 데이터 안의 마지막
    /// 투구 시각 기준으로 정확히 나오고, "지금까지" 계산이었다면 나왔을
    /// 20시간대 값이 아니다.
    #[test]
    fn live_view_shows_total_duration_for_a_finished_game_using_the_last_pitch_not_now() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 18_000; // KST 14:00:00 — "지금"을 썼다면 (+19:30) 버그
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            text.contains("Duration (+2:37)"),
            "expected total duration from the last recorded pitch:\n{text}"
        );
        assert!(
            !text.contains("(+19:30)"),
            "must not use 'now' for a finished game (yesterday's-game-shows-20h bug):\n{text}"
        );
    }

    /// 과거 타석을 돌려보는 중이어도(live_atbat_sel = Some(과거 seq)) 종료
    /// 경기의 소요는 그 타석이 아니라 항상 "경기 전체의 최신"을 기준으로
    /// 삼는다 — 값이 라이브일 때와 똑같아야 한다.
    #[test]
    fn game_duration_is_unaffected_by_viewing_a_past_at_bat() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        if let Screen::Live { game, .. } = &mut app.screen {
            game.start = "2026-07-19T18:30:00".into();
        }
        app.now_secs = 18_000;
        let latest_text = render_live_view_only(&app, 100, 30);
        assert!(latest_text.contains("Duration (+2:37)"));

        app.live_atbat_sel = Some(87); // fixture 최원준 타석(가장 오래된 타석)
        let past_text = render_live_view_only(&app, 100, 30);
        assert!(
            past_text.contains("Duration (+2:37)"),
            "duration must stay based on the latest at-bat while rewinding:\n{past_text}"
        );
    }

    /// 종료 경기인데 투구 데이터가 전혀 없으면(끝점 미상) 총 소요를 생략한다 —
    /// 기존 화면과 동일하게 아무것도 안 보여준다(무패닉·무회귀).
    #[test]
    fn duration_is_omitted_when_game_has_no_pitch_data_at_all() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_status(GameStatus::Final);
        if let Screen::Live {
            game,
            state: Some(s),
        } = &mut app.screen
        {
            game.start = "2026-07-19T18:30:00".into();
            s.at_bats.clear();
            s.current_pitches.clear();
        }
        let text = render_live_view_only(&app, 100, 30);
        assert!(
            !text.contains(crate::ui::i18n::EN.lbl_duration),
            "duration must be omitted without an endpoint:\n{text}"
        );
    }

    /// 폭 예산: 좁은 터미널에선 소요/경과 정보가 먼저 빠지고(투수/타자 등
    /// 기존 정보는 그대로 남는다), 넉넉한 폭에선 나타난다.
    #[test]
    fn game_duration_is_dropped_first_when_the_area_is_too_narrow() {
        let mut app = App::new(Default::default());
        app.screen = bare_live_screen(GameStatus::Final, "2026-07-19T18:30:00", "21:07:06");

        let wide_text = render_live_view_only(&app, 60, 20);
        assert!(
            wide_text.contains("Duration (+2:37)"),
            "wide enough area must show duration:\n{wide_text}"
        );
        assert!(wide_text.contains("Kim") && wide_text.contains("Lee"));

        let narrow_text = render_live_view_only(&app, 20, 20);
        assert!(
            !narrow_text.contains("Duration"),
            "duration must be the first thing dropped in a narrow area:\n{narrow_text}"
        );
    }
}
