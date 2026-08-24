//! F2 옵션 픽커 오버레이 + 공용 chooser(링크 픽커도 재사용).
use super::i18n::Labels;
use crate::app::{App, Pane};
use crate::dateutil::format_civil;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// Date pane 항목: (표시 라벨, YYYY-MM-DD).
///
/// 기준(`anchor_days`)은 **지금 보고 있는 날짜**다(`App::date_days`) — 진짜
/// 오늘로 고정하면 -3일로 옮긴 뒤 다시 열어도 여전히 오늘 기준이라 거기서 더
/// 못 간다. `--date`는 임의 날짜를 받는데 앱 안 내비게이션만 오늘 언저리에
/// 묶여 있었다.
///
/// **라벨은 기준이 아니라 진짜 오늘(`today_days`)이 정한다.** 기준만 옮기고
/// 라벨을 그대로 뒀더니 `--date 2026-06-01`에서 세 줄이 전부 거짓말을 했다
/// (오늘 2026-06-01 / 어제 05-31 / 내일 06-02 — 실제 오늘은 08-24). 이제
/// 오늘·어제·내일은 그 줄의 날짜가 실제로 그럴 때만 붙고, 기준일 자신은
/// `date_viewing`, 나머지는 기준에서 며칠인지(`-2일`)로 말한다.
///
/// **진짜 오늘로 돌아갈 자리도 목록이 갖고 있다.** 기준이 오늘에서 멀어지면
/// ±3일 안에 오늘이 없어, 06-01에서 08-24로 돌아가려면 `+3일`을 스물여덟 번
/// 눌러야 했다. 오늘이 목록에 없을 때만 둘째 줄로 끼워 넣는다 — 첫 줄(커서가
/// 놓이는 자리)은 계속 기준일이라 열자마자 Enter를 눌러도 제자리다.
///
/// "-2"/"-3"/"+2"/"+3"의 접미(days/일)는 언어별 완성형이 아니라
/// `l.date_days_fmt_minus`(공백 유무 포함 sep)로 데이터 주도 조립한다 —
/// 언어 분기(match lang) 없이 라벨 데이터만 바뀌면 문구가 따라온다. sep는
/// 접미의 첫 글자가 ASCII(라틴 문자 계열)인지로 결정한다 — 리터럴 "days"
/// 문자열 일치만 보면 놓치는 라틴 변형 접미도 첫 글자 ASCII 판정이면
/// 안전하게 걸린다. 한글/일본어 접미는 첫 글자가 비ASCII라 여전히 sep 없음.
pub fn date_items(l: &'static Labels, anchor_days: i64, today_days: i64) -> Vec<(String, String)> {
    let sep = if l
        .date_days_fmt_minus
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii())
    {
        " "
    } else {
        ""
    };
    let label = |off: i64| match anchor_days + off - today_days {
        0 => l.date_today.to_string(),
        -1 => l.date_yesterday.to_string(),
        1 => l.date_tomorrow.to_string(),
        _ if off == 0 => l.date_viewing.to_string(),
        _ => format!("{off:+}{sep}{}", l.date_days_fmt_minus),
    };
    let mut rows: Vec<(String, i64)> = [0i64, -1, 1, -2, -3, 2, 3]
        .into_iter()
        .map(|off| (label(off), anchor_days + off))
        .collect();
    if !rows.iter().any(|(_, days)| *days == today_days) {
        rows.insert(1, (l.date_today.to_string(), today_days));
    }
    rows.into_iter()
        .map(|(label, days)| {
            let d = format_civil(days);
            (format!("{label}  {d}"), d)
        })
        .collect()
}

/// Team pane 항목: (라벨, 코드). 첫 항목은 해제(None).
pub fn team_items(l: &'static Labels) -> Vec<(String, Option<String>)> {
    let mut v = vec![(l.team_none.to_string(), None)];
    for (code, name) in [
        ("LG", "LG 트윈스"),
        ("OB", "두산 베어스"),
        ("SK", "SSG 랜더스"),
        ("KT", "kt wiz"),
        ("NC", "NC 다이노스"),
        ("HT", "KIA 타이거즈"),
        ("LT", "롯데 자이언츠"),
        ("SS", "삼성 라이온즈"),
        ("HH", "한화 이글스"),
        ("WO", "키움 히어로즈"),
    ] {
        v.push((format!("{code}  {name}"), Some(code.to_string())));
    }
    v
}

pub fn poll_items(l: &'static Labels) -> Vec<(String, u64)> {
    [3u64, 5, 10, 30]
        .into_iter()
        .map(|s| (format!("{s}{}", l.poll_suffix), s))
        .collect()
}

/// app.rs 커서 경계용 항목 수. Pane은 v0.8부터 Date 단일 variant다(Team·Poll은
/// F9 설정으로 이동 — team_items/poll_items 자체는 change_setting/settings_rows가
/// 계속 쓰므로 남아 있다).
pub fn pane_len(pane: Pane, anchor_days: i64, today_days: i64, l: &'static Labels) -> usize {
    let Pane::Date = pane;
    date_items(l, anchor_days, today_days).len()
}

/// 공용 chooser: 중앙 오버레이 박스에 제목+항목 목록(커서 "> ", REVERSED).
pub fn chooser(f: &mut Frame, area: Rect, title: &str, items: &[Line], cursor: usize) {
    let h = (items.len() as u16 + 4).min(area.height);
    let w = 46u16.min(area.width);
    let rect = super::help_rect(w, h, area); // help.rs의 centered_rect를 pub(crate)로 승격해 재사용
    f.render_widget(Clear, rect);
    let mut lines: Vec<Line> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let mut line = item.clone();
        if i == cursor {
            line = line.style(Style::default().add_modifier(Modifier::REVERSED));
            line.spans.insert(0, Span::raw("> "));
        } else {
            line.spans.insert(0, Span::raw("  "));
        }
        lines.push(line);
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(format!(" {title} "))),
        rect,
    );
}

/// F2 옵션 오버레이: 날짜 전용(v0.8 — Team·Poll은 F9 설정으로 이동).
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(opt) = &app.options else { return };
    let l = app.labels();
    let title = format!("{}  [ {} ]", l.title_options, l.pane_date);
    let items: Vec<Line> = date_items(l, app.date_days(), app.today_days())
        .into_iter()
        .map(|(label, _)| Line::from(label))
        .collect();
    chooser(f, area, &title, &items, opt.cursor);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 완전성: F2 오버레이는 v0.8부터 Date 단일 pane — 라벨과 전 항목을 렌더한다.
    #[test]
    fn overlay_renders_date_pane_label_and_every_item() {
        let mut app = crate::app::App::new(Default::default());
        app.now_secs = 1_800_000_000;
        app.options = Some(crate::app::OptionsState {
            pane: crate::app::Pane::Date,
            cursor: 0,
        });
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Date"), "pane label Date missing");
        for (label, _) in date_items(app.labels(), app.date_days(), app.today_days()) {
            assert!(text.contains(&label), "date item {label} missing");
        }
    }

    #[test]
    fn korean_options_date_pane_renders_when_lang_ko() {
        let mut app = crate::app::App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.options = Some(crate::app::OptionsState {
            pane: crate::app::Pane::Date,
            cursor: 0,
        });
        app.now_secs = 1_800_000_000;
        let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        // ratatui는 전각(2-width) 문자 뒤에 placeholder 공백 셀을 채워 넣으므로
        // (live.rs 테스트와 동일한 이유) 공백을 제거하고 부분 문자열을 검사한다.
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for needle in ["날짜", "오늘"] {
            assert!(compact.contains(needle), "{needle} missing:\n{text}");
        }
    }

    /// 리뷰 지적(Minor): 일수 접미사 앞 공백은 "접미사 첫 글자가 ASCII인가"로
    /// 정해진다(en/es는 라틴 문자 접미라 공백 있음, ko는 비ASCII 접미라 공백
    /// 없음). 이 로직에 회귀 방지 테스트가 없었다 — 리터럴 "days" 일치만 보는
    /// 식으로 되돌아가면 "-2días"(es, 공백 소실)를 조용히 만들어낼 수 있다.
    /// date_items()가 조립한 라벨의 접미사 부분(오프셋 뒤·날짜 앞)을 언어별로
    /// 못박는다.
    #[test]
    fn date_items_suffix_separator_is_locale_correct() {
        let anchor = crate::dateutil::kst_days(1_800_000_000);

        // en: 라틴 문자 접미("days") → 숫자와 접미 사이 공백 있음.
        let en = date_items(
            crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
            anchor,
            anchor,
        );
        assert!(
            en[3].0.starts_with("-2 days"),
            "en suffix missing space before 'days': {}",
            en[3].0
        );

        // ko: 비ASCII 접미("일") → 숫자에 바로 붙음(공백 없음).
        let ko = date_items(
            crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko),
            anchor,
            anchor,
        );
        assert!(
            ko[3].0.starts_with("-2일"),
            "ko suffix should attach directly (no space) before '일': {}",
            ko[3].0
        );
        assert!(
            !ko[3].0.starts_with("-2 "),
            "ko suffix must not have a stray space: {}",
            ko[3].0
        );

        // ja: 비ASCII 접미("日") → 숫자에 바로 붙음(공백 없음) — ko와 같은
        // 경로를 다른 접미 문자열로 한 번 더 봉인한다.
        let ja = date_items(
            crate::ui::i18n::labels(crate::ui::i18n::Lang::Ja),
            anchor,
            anchor,
        );
        assert!(
            ja[3].0.starts_with("-2日"),
            "ja suffix should attach directly (no space) before '日': {}",
            ja[3].0
        );
        assert!(
            !ja[3].0.starts_with("-2 "),
            "ja suffix must not have a stray space: {}",
            ja[3].0
        );
    }

    /// **세 이름은 그 이름의 날에만 붙는다.** 기준을 "지금 보고 있는 날"로
    /// 옮기면서 라벨을 그대로 뒀더니 `--date 2026-06-01`에서 세 줄이 전부
    /// 거짓이 됐다 — `오늘 2026-06-01` / `어제 05-31` / `내일 06-02`인데 실제
    /// 오늘은 08-24였다.
    #[test]
    fn the_three_day_names_only_appear_on_the_days_they_name() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        let today = crate::dateutil::kst_days(1_800_000_000);
        let anchor = today - 84; // 석 달쯤 전을 보고 있는 상태
        let items = date_items(l, anchor, today);

        for (label, date) in &items {
            for (name, offset) in [
                (l.date_today, 0i64),
                (l.date_yesterday, -1),
                (l.date_tomorrow, 1),
            ] {
                if label.starts_with(name) {
                    assert_eq!(
                        *date,
                        crate::dateutil::format_civil(today + offset),
                        "'{name}' 라벨이 엉뚱한 날에 붙었다: {label}"
                    );
                }
            }
        }
        assert!(
            items[0].0.starts_with(l.date_viewing),
            "첫 줄은 지금 보는 날이라고 말해야 한다: {}",
            items[0].0
        );
        assert_eq!(items[0].1, crate::dateutil::format_civil(anchor));
    }

    /// **오늘로 돌아갈 줄은 언제나 정확히 하나다.** 기준이 오늘에서 멀면 목록
    /// ±3일 안에 오늘이 없어 `+3일`을 스물여덟 번 눌러야 돌아왔고, 반대로
    /// 무조건 한 줄을 더 넣으면 가까울 때 같은 날이 두 줄이 된다.
    /// 기준이 오늘일 때는 예전 그대로 일곱 줄이다.
    #[test]
    fn there_is_exactly_one_row_for_the_real_today_at_any_distance() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::En);
        let today = crate::dateutil::kst_days(1_800_000_000);
        let today_str = crate::dateutil::format_civil(today);
        for back in [0i64, 1, 3, 4, 40] {
            let items = date_items(l, today - back, today);
            let rows = items.iter().filter(|(_, d)| *d == today_str).count();
            assert_eq!(rows, 1, "{back}일 전을 보는 중: 오늘 줄이 {rows}개");
            // ±3일 안에 오늘이 있으면 줄을 더하지 않는다(중복 금지).
            let expected = if back <= 3 { 7 } else { 8 };
            assert_eq!(items.len(), expected, "{back}일 전을 보는 중: 항목 수");
            assert_eq!(
                items[0].1,
                crate::dateutil::format_civil(today - back),
                "첫 줄은 언제나 기준일이라야 열자마자 Enter가 제자리다"
            );
        }
    }

    #[test]
    fn team_items_cover_all_ten_teams_plus_none() {
        let items = team_items(crate::ui::i18n::labels(crate::ui::i18n::Lang::En));
        assert_eq!(items.len(), 11);
        assert_eq!(items[0].1, None); // 해제 항목
        for code in ["LG", "OB", "SK", "KT", "NC", "HT", "LT", "SS", "HH", "WO"] {
            assert!(
                items.iter().any(|(_, c)| c.as_deref() == Some(code)),
                "team {code} missing from picker"
            );
        }
    }
}
