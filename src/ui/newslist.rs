//! 뉴스 목록 오버레이(v0.7). 항목이 20건을 넘으므로 ratatui List + ListState로
//! windowing을 맡긴다 — 직접 offset을 계산하지 않는다.
use super::i18n::Labels;
use super::theme;
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState},
    Frame,
};

/// 제목과 매체명 사이 구분 공백(ASCII 2칸이라 바이트 길이 = 표시폭).
const GAP: &str = "  ";

/// 매체명 컬럼에 예약할 표시폭 상한. 현재 RSS 피드 매체명 중 가장 넓은
/// "스포티비뉴스"(6자 → `ui::text::display_width` 기준 12칸, 전각)를 그대로
/// 담을 수 있는 값이다 — 과거 고정 10칸은 이보다 좁아 매체명이 잘리거나 줄이
/// 밀렸다(리뷰 지적). 이보다 더 넓은 매체명이 오더라도 `ellipsize`가 안전하게
/// 잘라 이 상한을 넘겨 제목 폭을 잠식하지 않는다.
const SOURCE_COL_MAX_WIDTH: usize = 12;

/// 발행 경과 칸의 표시폭 상한(v0.18 B-1, 리뷰 Important-1로 계산 방식 개정).
/// 이 칸의 실제 예약 폭은 `age_col_width()`가 **현재 언어 `Labels`만으로**
/// 계산하는 고정값이고, 이 상수는 그 계산값을 담는 안전 상한(clamp)일
/// 뿐이다 — `age_col_width()`가 이 값을 넘기더라도(예: 상상 이상으로 오래된
/// 기사로 일 자릿수가 커짐) `ellipsize`가 상한 안에서 안전하게 자른다.
/// (과거 주석은 "23시간 전(ko, 7칸)"이라 적었지만 공백 1칸을 빠뜨린
/// 오산이었다 — `display_width`로 실측하면 "23"(2) + "시간 전"(전각 3문자
/// "시간전"=6 + 공백 1 = 7) = 9칸이다. `age_col_width()`가 실제로 계산하는
/// 값도 9다.)
const AGE_COL_MAX_WIDTH: usize = 10;

/// 경과 칸을 넣고도 제목에 최소 이만큼은 남아야 표시한다. 이보다 좁으면
/// 경과 시간부터 조용히 생략한다(설계 §2 B-1 "경과 시간부터 생략" 우선순위 —
/// 매체명 칸은 기존 관례를 건드리지 않고 그대로 둔다).
const MIN_TITLE_WIDTH_WITH_AGE: usize = 6;

/// `NewsItem.published`("YYYYMMDDHHMMSS", KST 고정 — `rss::parse::normalized_date`의
/// 출력 포맷, model.rs 문서 참고)를 UTC epoch 초로 파싱한다. 관용 파싱: 길이가
/// 14가 아니거나 숫자가 아니면 즉시 None, 월/시/분/초가 범위를 벗어나도 None.
/// 일(day)은 단순 1–31 범위 검사가 아니라 **그 연·월에 실존하는 날짜인지**를
/// 왕복 검증한다(윤년 포함, 아래 참고) — 리뷰 Important-2: 예전에는 "2월
/// 30일"·"4월 31일"처럼 형식은 맞지만 실존하지 않는 날짜가 통과해
/// `days_from_civil`이 조용히 다른 날짜(3/2, 5/1)로 롤오버시켰다. 어느
/// 경우든 패닉하지 않는다(슬라이싱 전에 전부 ASCII 숫자임을 확인해
/// char-boundary 패닉도 없다).
fn parse_published_epoch(published: &str) -> Option<i64> {
    let s = published.trim();
    if s.len() != 14 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: i64 = s[4..6].parse().ok()?;
    let d: i64 = s[6..8].parse().ok()?;
    let hh: i64 = s[8..10].parse().ok()?;
    let mm: i64 = s[10..12].parse().ok()?;
    let ss: i64 = s[12..14].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    let days = crate::dateutil::days_from_civil(y, m, d);
    // 왕복 검증: days_from_civil은 순수 산술이라 실존하지 않는 (y, m, d)도
    // 패닉 없이 어떤 day count로든 계산해 버린다. 그 day count를
    // civil_from_days로 다시 캘린더 날짜로 되돌렸을 때 원래 (y, m, d)와
    // 정확히 일치해야만 실존하는 날짜로 인정한다 — 월별 실제 일수(윤년의
    // 2/29 포함)를 하드코딩된 표 없이 기존 검증된 산술만으로 정확히
    // 반영한다. 불일치(예: 2/30 → 3/2, 4/31 → 5/1)면 None.
    if crate::dateutil::civil_from_days(days) != (y, m, d) {
        return None;
    }
    const KST_OFFSET_SECS: i64 = 9 * 3600;
    Some(days * 86400 + hh * 3600 + mm * 60 + ss - KST_OFFSET_SECS)
}

/// 발행 후 경과를 "3분 전"류 상대 표기로 만든다. header.rs의 `update_age_label`
/// (v0.15 A-2, 초/분 2단계)과 톤은 맞추되 단위 세분화는 다르다 — 뉴스는 초 단위
/// 정밀도가 필요 없어 분 미만을 전부 `news_age_now`("방금") 하나로 뭉친다.
/// 미래 시각(서버 시계 오차 등으로 published가 now_secs보다 나중인 경우)은
/// 음수로 보이면 안 되므로 0으로 clamp한다 — 자연히 "방금"과 합류해 별도
/// 분기 없이 요구사항(무패닉·음수 금지)을 만족한다.
fn news_age_label(l: &Labels, now_secs: u64, published_epoch: i64) -> String {
    let elapsed = (now_secs as i64 - published_epoch).max(0) as u64;
    if elapsed < 60 {
        l.news_age_now.to_string()
    } else if elapsed < 3600 {
        format!("{}{}", elapsed / 60, l.news_age_min_suffix)
    } else if elapsed < 86400 {
        format!("{}{}", elapsed / 3600, l.news_age_hour_suffix)
    } else {
        format!("{}{}", elapsed / 86400, l.news_age_day_suffix)
    }
}

/// 파싱+상대 표기를 이어붙인 편의 함수. `published`가 비어 있거나 관용 파싱에
/// 실패하면 None — 호출부는 그 항목의 경과 칸을 비워 둔다(표시 생략, 무패닉).
fn news_age_for(l: &Labels, now_secs: u64, published: &str) -> Option<String> {
    parse_published_epoch(published).map(|epoch| news_age_label(l, now_secs, epoch))
}

/// 발행 경과 칸에 예약할 표시폭을 **현재 언어 `Labels`만으로** 계산한다 —
/// 리뷰 Important-1의 핵심 수정. 이전에는 "이번 프레임에 실제로 렌더되는 age
/// 문자열들의 실측 최댓값"을 썼는데, 이는 `SOURCE_COL_MAX_WIDTH`가 쓰는
/// 관례와 똑같아 보이지만 잘못 적용된 것이었다 — 매체명은 뉴스 목록이
/// 갱신될 때만 바뀌어 프레임 사이에 안정적이지만, age는 `app.now_secs`가
/// tick마다(main.rs, ~100ms) 갱신되며 매 프레임 값이 달라지므로 같은 목록·
/// 같은 폭에서도 시계가 흐르는 것만으로 칼럼 폭이 흔들려 제목이 밀렸다.
///
/// 이 함수가 반환하는 값은 언어가 바뀌지 않는 한 절대 변하지 않는
/// 상수라서, `now_secs`가 무엇이든 같은 언어에서는 항상 같은 예약폭이
/// 나온다. 네 후보 중 최댓값을 취한다:
/// - `news_age_now`("방금"류, 분 미만은 숫자가 없다)
/// - "59" + `news_age_min_suffix` (분 단위는 59분까지 최대 2자리)
/// - "23" + `news_age_hour_suffix` (시간 단위는 23시간까지 최대 2자리)
/// - "999" + `news_age_day_suffix` (일 단위 자릿수 상한은 3자리 = 최대
///   999일 ≈ 2.7년으로 잡는다. `default_feeds()`(`rss/mod.rs`)의 매체는
///   전부 활발히 갱신되는 스포츠 매체라 각 피드가 노출하는 최신 항목들은
///   보통 며칠~몇 주 안에 전부 새 기사로 교체된다 — 실전에서 화면에 뜰 수
///   있는 "가장 오래된 기사"가 몇 년 전일 일은 사실상 없다. 3자리는 그
///   보수적 기대치보다 훨씬 넉넉한 여유이고, 그 이상(4자리 이상)을 상정하면
///   평소에 전혀 안 쓰이는 폭만 매 프레임 낭비하게 된다. 혹시라도 이 가정을
///   벗어나는 값이 오더라도 아래 `.min(AGE_COL_MAX_WIDTH)`와 `ellipsize`가
///   안전망이다.)
fn age_col_width(l: &Labels) -> usize {
    use super::text::display_width;
    [
        display_width(l.news_age_now),
        display_width(&format!("59{}", l.news_age_min_suffix)),
        display_width(&format!("23{}", l.news_age_hour_suffix)),
        display_width(&format!("999{}", l.news_age_day_suffix)),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
    .min(AGE_COL_MAX_WIDTH)
}

/// `s`를 `width`칸(표시폭 기준)까지 ASCII 공백으로 오른쪽에 채운다 — 이미
/// `width` 이상이면 그대로 둔다(트렁케이션은 호출부가 `ellipsize`로 미리
/// 끝낸 뒤 불러야 한다). `format!("{:<N}")`은 유니코드 스칼라 개수로 폭을
/// 세어 한글·전각 문자의 폭을 과소평가하므로 여기서는 쓸 수 없고,
/// `super::text::display_width`로 실제 표시폭을 잰다. 왼쪽 정렬(텍스트를
/// 앞에 두고 남는 칸을 뒤에 채움)로 고정한 이유: 매체명 칼럼(`source`)이
/// `ellipsize`로 문자열의 앞부분을 보존하고 넘칠 때만 끝을 자르는 것과 같은
/// "왼쪽 정렬" 방향을 age 칼럼에도 그대로 맞춘 것이다.
fn pad_to_width(s: &str, width: usize) -> String {
    let w = super::text::display_width(s);
    if w >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - w))
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(list) = &app.news_list else {
        return;
    };
    let l = app.labels();
    let w = area.width.saturating_sub(4).max(1);
    let h = area.height.saturating_sub(2).max(1);
    let rect = super::help_rect(w, h, area);
    let inner_width = rect.width.saturating_sub(4) as usize; // 테두리 + 커서 표식

    // 이번에 실릴 매체명들의 실제 표시폭 최댓값으로 컬럼을 잡는다(상한으로 캡).
    // 목록이 비면 0 — 어차피 렌더할 항목이 없다.
    let source_width = app
        .news
        .iter()
        .map(|n| super::text::display_width(&n.source))
        .max()
        .unwrap_or(0)
        .min(SOURCE_COL_MAX_WIDTH);

    // B-1: 발행 경과("3분 전"). 항목별로 파싱 성패가 갈릴 수 있으므로(피드마다
    // published 형식 편차) 우선 전부 계산해 둔다. 폭은 (리뷰 Important-1 수정
    // 이후) source_width와 달리 실측 최댓값이 아니라 age_col_width(l)의
    // 언어 고정값이다 — now_secs가 매 tick 바뀌어도 이 값 자체는 절대
    // 변하지 않는다.
    let ages: Vec<Option<String>> = app
        .news
        .iter()
        .map(|n| news_age_for(l, app.now_secs, &n.published))
        .collect();
    let age_width = age_col_width(l);
    // 목록에 파싱 가능한 published가 하나도 없으면(예: 전부 빈 문자열)
    // age 칼럼 자체를 예약하지 않는다 — 이건 now_secs가 아니라 목록
    // 데이터에만 의존하는 판단이라(파싱 성패는 시간과 무관), 목록이 그대로인
    // 한 프레임 사이에 흔들리지 않는다.
    let any_age = ages.iter().any(|a| a.is_some());

    let reserved_base = source_width + GAP.len();
    // 폭이 부족하면 경과 시간부터 생략한다(설계 §2 B-1 우선순위) — 매체명 칸의
    // 기존 예약 방식은 건드리지 않고, 경과 칸을 넣었을 때 제목에 남는 폭이
    // 최소 기준 이상일 때만 켠다.
    let show_age = any_age
        && inner_width.saturating_sub(reserved_base + GAP.len() + age_width)
            >= MIN_TITLE_WIDTH_WITH_AGE;
    let reserved = reserved_base + if show_age { GAP.len() + age_width } else { 0 };

    let items: Vec<ListItem> = app
        .news
        .iter()
        .zip(ages.iter())
        .map(|(n, age)| {
            let title = super::text::ellipsize(&n.title, inner_width.saturating_sub(reserved));
            let source = super::text::ellipsize(&n.source, source_width);
            let mut spans = vec![
                Span::raw(title),
                Span::raw(GAP),
                Span::styled(source, Style::default().add_modifier(Modifier::DIM)),
            ];
            if show_age {
                // ellipsize로 상한 안에 안전하게 자른 뒤(고정폭 상한을 혹시
                // 넘는 극단값 대비), pad_to_width로 왼쪽 정렬 고정폭 렌더 —
                // 자릿수가 다른 age 문자열끼리도 칸이 흔들리지 않는다.
                let age_text = super::text::ellipsize(age.as_deref().unwrap_or(""), age_width);
                let age_text = pad_to_width(&age_text, age_width);
                spans.push(Span::raw(GAP));
                spans.push(Span::styled(
                    age_text,
                    theme::status_fg(&app.theme_preset, Color::Gray),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    // 하단 힌트도 title 영역 폭(테두리 2칸 제외)을 넘지 않게 ellipsize한다 —
    // 안 그러면 좁은 터미널에서 ratatui가 말줄임 없이 조용히 잘라 박스 경계를
    // 침범할 수 있다(리뷰 지적 fix 2-4).
    let hint_budget = rect.width.saturating_sub(2) as usize;
    let hint = super::text::ellipsize(l.news_list_hint, hint_budget);
    let widget = List::new(items)
        .block(
            Block::bordered()
                .title(l.title_news_list)
                .title_bottom(hint),
        )
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default();
    state.select(Some(list.cursor.min(app.news.len().saturating_sub(1))));

    f.render_widget(Clear, rect);
    f.render_stateful_widget(widget, rect, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, NewsListState};
    use crate::model::NewsItem;
    use ratatui::{backend::TestBackend, Terminal};

    fn item(title: &str, source: &str) -> NewsItem {
        NewsItem {
            title: title.into(),
            source: source.into(),
            url: "https://x.kr/1".into(),
            summary: "발췌".into(),
            published: String::new(),
        }
    }

    fn item_with_published(title: &str, source: &str, published: &str) -> NewsItem {
        let mut n = item(title, source);
        n.published = published.into();
        n
    }

    /// `normalized_date`(rss/parse.rs)와 동일한 "YYYYMMDDHHMMSS"(KST) 포맷으로
    /// 테스트용 published 문자열을 만든다.
    fn kst_string(y: i64, m: i64, d: i64, hh: i64, mm: i64, ss: i64) -> String {
        format!("{y:04}{m:02}{d:02}{hh:02}{mm:02}{ss:02}")
    }

    /// 위 kst_string이 나타내는 KST 벽시계 시각의 UTC epoch 초. `dateutil`의
    /// (이미 검증된) 순수 날짜 산술을 그대로 재사용해 기댓값을 만든다 —
    /// `parse_published_epoch`이 같은 산술을 올바르게 조립하는지가 테스트
    /// 대상이라, days_from_civil 자체의 정확성(별도 테스트로 이미 봉인)까지
    /// 여기서 다시 증명할 필요는 없다.
    fn kst_epoch(y: i64, m: i64, d: i64, hh: i64, mm: i64, ss: i64) -> i64 {
        crate::dateutil::days_from_civil(y, m, d) * 86400 + hh * 3600 + mm * 60 + ss - 9 * 3600
    }

    fn render_lines(app: &App, width: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(width, 24)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..buf.area().height)
            .map(|y| {
                (0..buf.area().width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    /// "스포티비뉴스"(6자=12칸)처럼 과거 고정 10칸 예약을 넘던 매체명도 잘리지
    /// 않고 통째로 렌더돼야 한다(리뷰 지적 Minor 재현).
    #[test]
    fn wide_source_name_is_not_truncated() {
        let mut app = App::new(Default::default());
        app.news = vec![item("어떤 기사 제목", "스포티비뉴스")];
        app.news_list = Some(NewsListState { cursor: 0 });
        let lines = render_lines(&app, 80);
        let joined = lines.join("");
        let compact: String = joined.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("스포티비뉴스"),
            "source name truncated:\n{compact}"
        );
    }

    /// 매체명 컬럼은 실제 매체명들의 표시폭 최댓값을 예약하므로, 서로 다른
    /// 폭의 매체명이 섞여 있어도(4자/6자) 어느 쪽도 잘리지 않는다.
    #[test]
    fn mixed_width_sources_all_render_untruncated() {
        let mut app = App::new(Default::default());
        app.news = vec![
            item("첫 기사", "스포츠조선"),     // 10칸
            item("둘째 기사", "스포티비뉴스"), // 12칸(최댓값)
            item("셋째 기사", "일간스포츠"),   // 10칸
        ];
        app.news_list = Some(NewsListState { cursor: 0 });
        let compact: String = render_lines(&app, 80)
            .join("")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        for src in ["스포츠조선", "스포티비뉴스", "일간스포츠"] {
            assert!(compact.contains(src), "{src} truncated:\n{compact}");
        }
    }

    /// 매체명이 극단적으로 길어도(상한 초과) ellipsize로 안전하게 잘려
    /// 패닉 없이 렌더된다. 좁은 터미널에서도 마찬가지.
    #[test]
    fn extremely_long_source_and_narrow_terminal_do_not_panic() {
        let mut app = App::new(Default::default());
        app.news = vec![item("제목", &"매".repeat(50))];
        app.news_list = Some(NewsListState { cursor: 0 });
        let _ = render_lines(&app, 80); // 패닉 없으면 통과
        let _ = render_lines(&app, 5); // 매우 좁은 터미널도 패닉 없음
    }

    /// 목록이 비어 있으면(뉴스 0건) 패닉 없이 빈 목록을 렌더한다.
    #[test]
    fn empty_news_list_renders_without_panic() {
        let mut app = App::new(Default::default());
        app.news = vec![];
        app.news_list = Some(NewsListState { cursor: 0 });
        let _ = render_lines(&app, 80);
    }

    /// fix 2-4: 하단 힌트(title_bottom)는 title 영역 폭(테두리 2칸 제외)을
    /// 넘지 않게 ellipsize된다 — 좁은 터미널에서도 박스 모서리(└ ┘)가 힌트
    /// 텍스트로 덮이면 안 된다(리뷰 지적).
    #[test]
    fn bottom_hint_never_overwrites_box_corners_at_narrow_width() {
        crate::ui::test_support::assert_bottom_hint_keeps_box_corners(
            &[10, 15, 20, 30, 52, 80],
            |app| {
                app.news = vec![item("어떤 기사 제목", "매체")];
                app.news_list = Some(NewsListState { cursor: 0 });
            },
            render,
        );
    }

    // ---- B-1: 뉴스 발행 경과 ----

    /// 정규화된 KST 문자열("YYYYMMDDHHMMSS")을 UTC epoch로 정확히 조립한다.
    /// kst_days(같은 순간을 KST 일수로 환산)로도 교차 검증한다.
    #[test]
    fn parse_published_epoch_parses_normalized_kst_string() {
        let s = kst_string(2026, 7, 24, 9, 18, 39);
        let epoch = parse_published_epoch(&s).expect("must parse a well-formed value");
        assert_eq!(epoch, kst_epoch(2026, 7, 24, 9, 18, 39));
        // 09:18:39 KST는 UTC로도 같은 날(00:18:39 UTC) → KST 일수는 그대로다.
        assert_eq!(
            crate::dateutil::kst_days(epoch as u64),
            crate::dateutil::days_from_civil(2026, 7, 24)
        );
    }

    /// 자정 근처(KST 00시대) 값도 정확히 하루 전 UTC로 내려간다(자정 경계 회귀).
    #[test]
    fn parse_published_epoch_handles_midnight_kst() {
        let s = kst_string(2026, 7, 24, 0, 30, 0);
        let epoch = parse_published_epoch(&s).expect("must parse");
        assert_eq!(epoch, kst_epoch(2026, 7, 24, 0, 30, 0));
        // 00:30 KST = 전날 15:30 UTC.
        assert_eq!(
            crate::dateutil::civil_from_days(epoch.div_euclid(86400)),
            (2026, 7, 23)
        );
    }

    /// 길이·자릿수·범위가 어긋난 값은 전부 None — 어떤 것도 패닉하지 않는다.
    /// (`published`가 항상 정규화된 값이라 실전에서는 드물지만, 관용 파싱
    /// 원칙상 방어해야 한다.)
    #[test]
    fn parse_published_epoch_rejects_malformed_without_panic() {
        for bad in [
            "",
            "어제",
            "not-a-date",
            "2026-07-24",
            "202607240918",     // 13자 미만
            "2026072409183900", // 14자 초과
            "20261324091839",   // 월 13
            "20260732091839",   // 일 32
            "20260724251839",   // 시 25
            "20260724096139",   // 분 61
            "20260724091899",   // 초 99
            "2026072a091839",   // 숫자 아님
        ] {
            assert_eq!(
                parse_published_epoch(bad),
                None,
                "expected None for {bad:?}"
            );
        }
    }

    /// 리뷰 Important-2: 형식(길이·자릿수·범위)은 맞지만 달력에 실존하지
    /// 않는 날짜는 거부돼야 한다 — 예전에는 1..=31 범위 검사만 해서
    /// "2월 30일"·"4월 31일" 같은 값이 통과해 `days_from_civil`이 조용히
    /// 다른 날짜(3/2, 5/1)로 롤오버시켰다(패닉은 없었지만 문서-코드
    /// 불일치). 왕복 검증(days_from_civil → civil_from_days)이 윤년 규칙
    /// (4/100/400)을 포함해 월별 실제 일수를 정확히 반영하는지 확인한다.
    #[test]
    fn parse_published_epoch_rejects_nonexistent_calendar_dates() {
        // 2024년은 윤년(4로 나누어떨어지고 100으로는 안 나누어떨어짐) → 2/29 있음.
        assert!(
            parse_published_epoch(&kst_string(2024, 2, 29, 9, 18, 39)).is_some(),
            "2024-02-29 is a real leap day and must parse"
        );
        // 2026년은 평년 → 2/29 없음.
        assert_eq!(
            parse_published_epoch(&kst_string(2026, 2, 29, 9, 18, 39)),
            None,
            "2026-02-29 does not exist (not a leap year)"
        );
        // 2월 30일은 어느 해에도 존재하지 않는다.
        assert_eq!(
            parse_published_epoch(&kst_string(2026, 2, 30, 9, 18, 39)),
            None,
            "February never has a 30th"
        );
        // 4월은 30일까지 — 31일은 존재하지 않는다.
        assert_eq!(
            parse_published_epoch(&kst_string(2026, 4, 31, 9, 18, 39)),
            None,
            "April has only 30 days"
        );
        // 12월 31일은 항상 존재한다(정상 경계값이 여전히 통과함을 확인).
        assert!(
            parse_published_epoch(&kst_string(2026, 12, 31, 9, 18, 39)).is_some(),
            "December 31st is always valid"
        );
    }

    /// 윤년 규칙의 100/400 예외까지 왕복 검증이 정확히 반영하는지 확인한다
    /// (4로만 보면 윤년처럼 보이는 1900, 400 예외로 다시 윤년인 2000).
    #[test]
    fn parse_published_epoch_leap_year_100_400_exception_boundaries() {
        // 1900은 4의 배수지만 100의 배수이면서 400의 배수는 아니므로 평년 → 2/29 없음.
        assert_eq!(
            parse_published_epoch(&kst_string(1900, 2, 29, 0, 0, 0)),
            None,
            "1900 is divisible by 100 but not 400: not a leap year"
        );
        // 2000은 400의 배수라 다시 윤년 → 2/29 있음.
        assert!(
            parse_published_epoch(&kst_string(2000, 2, 29, 0, 0, 0)).is_some(),
            "2000 is divisible by 400: it is a leap year"
        );
    }

    /// 경계값: 59초/60초/59분/60분/23시간/24시간에서 "분 미만→분→시간→일"
    /// 버킷이 정확히 갈린다(설계 예시 톤을 영어 라벨로 확인).
    #[test]
    fn news_age_label_buckets_at_boundaries() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::En);
        assert_eq!(news_age_label(l, 100, 100), "Just now"); // 0s
        assert_eq!(news_age_label(l, 159, 100), "Just now"); // 59s
        assert_eq!(news_age_label(l, 160, 100), "1m ago"); // 60s
        assert_eq!(news_age_label(l, 100 + 3599, 100), "59m ago");
        assert_eq!(news_age_label(l, 100 + 3600, 100), "1h ago");
        assert_eq!(news_age_label(l, 100 + 86399, 100), "23h ago");
        assert_eq!(news_age_label(l, 100 + 86400, 100), "1d ago");
        assert_eq!(news_age_label(l, 100 + 5 * 86400, 100), "5d ago");
    }

    /// 한국어 완성형: 설계 예시("3분 전")와 정확히 일치한다.
    #[test]
    fn korean_news_age_matches_design_examples() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        assert_eq!(news_age_label(l, 100 + 180, 100), "3분 전");
        assert_eq!(news_age_label(l, 100 + 2 * 3600, 100), "2시간 전");
        assert_eq!(news_age_label(l, 100 + 5 * 86400, 100), "5일 전");
    }

    /// 미래 시각(서버 시계 오차 등으로 published > now)은 음수로 새지 않고
    /// "방금"으로 흡수된다 — 무패닉 + 요구사항(음수 표기 금지) 동시 충족.
    #[test]
    fn news_age_label_clamps_future_timestamps_to_just_now() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        assert_eq!(news_age_label(l, 100, 500), "방금");
        let l_en = crate::ui::i18n::labels(crate::ui::i18n::Lang::En);
        assert_eq!(news_age_label(l_en, 100, 1_000_000), "Just now");
    }

    /// news_age_for: 파싱 실패(빈 문자열·깨진 값)는 None으로 흡수해 항목의
    /// 경과 칸을 조용히 비운다.
    #[test]
    fn news_age_for_returns_none_on_missing_or_broken_published() {
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::En);
        assert_eq!(news_age_for(l, 100, ""), None);
        assert_eq!(news_age_for(l, 100, "garbage"), None);
        assert!(news_age_for(l, 100, &kst_string(2026, 7, 24, 9, 0, 0)).is_some());
    }

    /// 넓은 터미널에서는 목록 각 항목에 발행 경과가 실제로 보인다.
    #[test]
    fn news_age_appears_in_wide_render_for_a_recent_item() {
        let mut app = App::new(Default::default());
        let published = kst_string(2026, 7, 24, 9, 0, 0);
        let epoch = kst_epoch(2026, 7, 24, 9, 0, 0);
        app.now_secs = (epoch + 125) as u64; // 2분 5초 후 → "2m ago"
        app.news = vec![item_with_published("최신 기사 제목", "매체", &published)];
        app.news_list = Some(NewsListState { cursor: 0 });
        let compact: String = render_lines(&app, 80)
            .join("")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(compact.contains("2mago"), "age text missing:\n{compact}");
    }

    /// 좁은 터미널에서는 경과 시간부터 조용히 생략되지만(폭 예산 우선순위),
    /// 제목은 여전히 보이고 패닉하지 않는다 — 기존 폭 회귀 테스트와 같은 결.
    #[test]
    fn news_age_is_omitted_at_narrow_width_but_title_still_renders() {
        let mut app = App::new(Default::default());
        let published = kst_string(2026, 7, 24, 9, 0, 0);
        let epoch = kst_epoch(2026, 7, 24, 9, 0, 0);
        app.now_secs = (epoch + 125) as u64;
        app.news = vec![item_with_published("어떤 기사 제목", "매체", &published)];
        app.news_list = Some(NewsListState { cursor: 0 });

        let wide: String = render_lines(&app, 80).join("");
        assert!(
            wide.contains("2m ago"),
            "wide render must show age:\n{wide}"
        );

        let narrow_lines = render_lines(&app, 20); // 패닉 없으면 1차 통과
        let narrow = narrow_lines.join("");
        assert!(
            !narrow.contains("2m ago"),
            "narrow render must omit age:\n{narrow}"
        );
        let compact: String = narrow.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("어떤"),
            "title must survive even without the age column:\n{compact}"
        );
    }

    /// 항목마다 published 파싱 성패가 갈려도(피드 편차) 패닉 없이 섞여 렌더된다
    /// — 파싱 실패 항목은 그 칸만 비워 둔다.
    #[test]
    fn mixed_valid_and_broken_published_renders_without_panic() {
        let mut app = App::new(Default::default());
        let published = kst_string(2026, 7, 24, 9, 0, 0);
        app.now_secs = (kst_epoch(2026, 7, 24, 9, 0, 0) + 125) as u64;
        app.news = vec![
            item_with_published("정상 기사", "매체A", &published),
            item_with_published("깨진 날짜 기사", "매체B", "깨진값"),
            item("결측 기사", "매체C"), // published 빈 문자열
        ];
        app.news_list = Some(NewsListState { cursor: 0 });
        let compact: String = render_lines(&app, 80)
            .join("")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(compact.contains("정상기사"));
        assert!(compact.contains("깨진날짜기사"));
        assert!(compact.contains("결측기사"));
        assert!(
            compact.contains("2mago"),
            "valid item's age must still show"
        );
    }

    /// 경과 칸이 있어도(넓은 폭) 하단 힌트 박스 모서리는 여전히 보존된다 —
    /// fix 2-4 회귀 방지 범위를 age 칼럼 도입 이후로 확장.
    #[test]
    fn bottom_hint_corners_survive_with_age_column_present() {
        let published = kst_string(2026, 7, 24, 9, 0, 0);
        let epoch = kst_epoch(2026, 7, 24, 9, 0, 0);
        crate::ui::test_support::assert_bottom_hint_keeps_box_corners(
            &[10, 15, 20, 30, 52, 80],
            |app| {
                app.now_secs = (epoch + 125) as u64;
                app.news = vec![item_with_published("어떤 기사 제목", "매체", &published)];
                app.news_list = Some(NewsListState { cursor: 0 });
            },
            render,
        );
    }

    // ---- 리뷰 Important-1: age 칼럼 폭 안정성 ----

    /// `age_col_width`는 언어에 대해서만 결정되는 상수라, 같은 언어에서
    /// 반복 호출해도 항상 같은 값을 낸다 — `now_secs`를 인자로 받지 않는다는
    /// 시그니처 자체가 "프레임에 의존하지 않는다"는 계약이다. en 기준값은
    /// max("Just now"=8, "59m ago"=7, "23h ago"=7, "999d ago"=8) = 8.
    #[test]
    fn age_col_width_is_a_language_constant() {
        let en = crate::ui::i18n::labels(crate::ui::i18n::Lang::En);
        assert_eq!(age_col_width(en), age_col_width(en));
        assert_eq!(age_col_width(en), 8);

        // ko 기준값은 max("방금"=4, "59분 전"=7, "23시간 전"=9, "999일 전"=8) = 9
        // — M-1에서 정정한 "23시간 전 = 9칸" 실측과 정확히 일치해야 한다.
        let ko = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        assert_eq!(age_col_width(ko), 9);
    }

    /// age 칼럼은 고정 폭 안에서 왼쪽 정렬(뒤쪽을 공백으로 채움)로 렌더된다
    /// — source 칼럼이 `ellipsize`로 문자열의 앞부분을 보존하고 넘칠 때만
    /// 끝을 자르는 것과 같은 "왼쪽 정렬" 방향을 그대로 따른다. 짧은 문자열
    /// ("방금")도 칸을 가득 채우도록 뒤에 공백이 붙어야 한다.
    #[test]
    fn age_text_is_left_aligned_and_padded_to_the_fixed_column_width() {
        let ko = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);
        let w = age_col_width(ko);
        let padded_short = pad_to_width(ko.news_age_now, w); // "방금" (짧음)
        let padded_long = pad_to_width("23시간 전", w); // 정확히 최댓값
        assert_eq!(crate::ui::text::display_width(&padded_short), w);
        assert_eq!(crate::ui::text::display_width(&padded_long), w);
        assert!(padded_short.starts_with(ko.news_age_now));
        assert!(padded_long.starts_with("23시간 전"));
        // 짧은 문자열은 실제로 뒤에 공백이 붙어 있어야 "채워졌다"고 할 수 있다.
        assert!(padded_short.ends_with(' '));
    }

    /// ★ 리뷰 Important-1 핵심 증거: 같은 뉴스 목록·같은 터미널 폭에서
    /// `now_secs`만 여러 값으로 바꿔 렌더해도 제목이 잘리는 위치(=화면에
    /// 남는 'A' 개수)가 완전히 동일해야 한다. age 칼럼 폭이 "이번 프레임
    /// 실측 최댓값"이던 시절에는 분→시간→일 자릿수·단위 전환 경계를 넘을
    /// 때마다 age_width가 바뀌어 제목이 한 칸씩 밀렸다(리뷰 실측: "A"×60
    /// 제목이 36→35자로 잘림). 자릿수·단위 전환 경계 3종(9↔10분,
    /// 59분↔1시간, 23시간↔1일)을 전부 포함한다.
    #[test]
    fn title_truncation_point_is_stable_across_age_digit_and_unit_boundaries() {
        let long_title = "A".repeat(60);
        // (경계 설명, published 시각으로부터 흐른 초)
        let cases: [(&str, u64); 6] = [
            ("9분대(자릿수 1)", 9 * 60 + 30),
            ("10분대(자릿수 2, 9↔10분 전환)", 10 * 60 + 30),
            ("59분대(분 버킷 최댓값)", 59 * 60),
            ("1시간대(59분↔1시간 전환)", 60 * 60),
            ("23시간대(시간 버킷 최댓값)", 23 * 3600),
            ("1일대(23시간↔1일 전환)", 24 * 3600),
        ];

        let mut a_counts: Vec<(&str, usize)> = Vec::new();
        for (label, elapsed) in cases {
            let mut app = App::new(Default::default());
            let published = kst_string(2026, 7, 1, 0, 0, 0);
            let epoch = kst_epoch(2026, 7, 1, 0, 0, 0);
            app.now_secs = (epoch + elapsed as i64) as u64;
            app.news = vec![item_with_published(&long_title, "abcd", &published)];
            app.news_list = Some(NewsListState { cursor: 0 });

            let lines = render_lines(&app, 60);
            // 이 렌더에서 'A'는 오직 title에서만 나온다 — 렌더된 'A' 개수가
            // 곧 "제목이 잘리는 위치"다.
            let a_count: usize = lines.iter().map(|l| l.matches('A').count()).sum();
            a_counts.push((label, a_count));
        }

        let first = a_counts[0].1;
        assert!(
            a_counts.iter().all(|(_, c)| *c == first),
            "title truncation length must be identical across every age boundary: {a_counts:?}"
        );
    }
}
