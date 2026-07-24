//! 인앱 뉴스 발췌 오버레이(v0.7). `n`이 여는 중앙 큰 박스 — 제목(강조) +
//! 매체 + 발췌(폭 안전 wrap) + 스크롤바. 선택한 NewsItem을 그대로 렌더하므로
//! 비동기 fetch·loading 상태가 없다.
use crate::app::App;
use ratatui::{
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// 한 Line이 폭 `cols`에서 차지하는 시각적 행 수(대략). ratatui의 실제 렌더는
/// `Wrap`로 **word-wrap**(단어 경계 우선)한다 — 줄 끝에 단어가 안 들어가면
/// 통째로 다음 행으로 넘기므로 행 끝에 여백(slack)이 생긴다. 그래서 실제 행
/// 수는 항상 **char-wrap 추정치(폭을 cols로 그냥 나눈 값) 이상**이다.
///
/// 이 함수는 `ui::text::display_width`(비ASCII을 전부 2칸으로 보는 휴리스틱)
/// 로 폭을 잰다. 한글·CJK 전각 문자는 실제로도 2칸이라 정확하지만, 악센트
/// (é, ñ 등) 같은 실제 width-1 비ASCII 문자는 두 배로 **과다 계산**한다. 이
/// 과다 계산은 버그가 아니라 **의도된 안전마진**이다 — word-wrap slack을
/// 미리 덮어줘서 스크롤 상한이 실제 렌더 행 수보다 작아지는 사고를 막는다.
///
/// 한때 `Line::width()`(ratatui 자체가 실제 wrap에도 쓰는 unicode-width 기준
/// 정확한 폭)로 "정밀화"했지만(fix 2-5), 그러면 이 안전마진이 사라져 Es(스페
/// 인어 악센트) 로케일에서 긴 발췌를 스크롤할 때 마지막 콘텐츠 줄(원문 CTA)
/// 에 도달하지 못하는 실제 회귀가 생겼다 — **스크롤 상한 추정기는 절대
/// 과소추정하면 안 되고, 과대추정으로 편향돼야 안전하다**(과대추정은 빈
/// 줄까지 스크롤되는 무해한 코스메틱, 과소추정은 마지막 줄 도달불가라는
/// 실버그다). 그래서 되돌린다. 진짜 word-wrap-aware 행 수 계산은 백로그.
fn line_rows(line: &Line, cols: usize) -> u16 {
    let w = cols.max(1);
    let width: usize = line
        .spans
        .iter()
        .map(|s| crate::ui::text::display_width(&s.content))
        .sum();
    (width.max(1)).div_ceil(w) as u16
}

/// 기사 오버레이를 그린다. area 대비 여백만 남긴 큰 중앙 박스.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(view) = &app.article_view else {
        return;
    };
    let l = app.labels();

    // 큰 박스: 좌우 2칸·상하 1칸 여백(help_rect는 area보다 크면 area로 clamp).
    let w = area.width.saturating_sub(4).max(1);
    let h = area.height.saturating_sub(2).max(1);
    let rect = super::help_rect(w, h, area);
    let inner = rect.inner(Margin::new(1, 1)); // 테두리 안쪽(본문 렌더 영역)

    // 하단 힌트도 title 영역 폭(테두리 2칸 제외)을 넘지 않게 ellipsize한다 —
    // 안 그러면 좁은 터미널에서 ratatui가 말줄임 없이 조용히 잘라 박스 경계를
    // 침범할 수 있다(리뷰 지적 fix 2-4).
    let hint_budget = rect.width.saturating_sub(2) as usize;
    let hint = super::text::ellipsize(l.article_hint, hint_budget);
    let block = Block::bordered().title(l.title_article).title_bottom(hint);

    f.render_widget(Clear, rect);

    let item = &view.item;
    // 제목(BOLD) → 매체(DIM) → 빈 줄 → 발췌 → 빈 줄 → 원문 CTA.
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        item.title.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if !item.source.is_empty() {
        lines.push(Line::from(Span::styled(
            item.source.clone(),
            Style::default().add_modifier(Modifier::DIM),
        )));
    }
    lines.push(Line::from(""));
    for bl in item.summary.split('\n') {
        lines.push(Line::from(bl.to_string()));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        l.article_read_full,
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    // 콘텐츠 총 행 수(wrap 반영 근사)로 스크롤 상한을 구해 clamp한다 — 상태의
    // scroll은 무한 증가할 수 있어도(app.rs는 saturating만) 빈 공간으로 넘어가지
    // 않게 한다.
    let total: u16 = lines
        .iter()
        .map(|ln| line_rows(ln, inner.width as usize))
        .fold(0u16, |a, r| a.saturating_add(r));
    let max_scroll = total.saturating_sub(inner.height);
    let scroll = view.scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(block, rect);
    f.render_widget(paragraph, inner);

    // 스크롤 가능한 분량일 때만 스크롤바(우측 세로, 코너 침범 방지 세로 여백).
    if total > inner.height {
        let mut state = ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            rect.inner(Margin::new(0, 1)),
            &mut state,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, ArticleView};
    use crate::model::NewsItem;
    use ratatui::{backend::TestBackend, Terminal};

    fn sample() -> NewsItem {
        NewsItem {
            title: "제목텍스트".into(),
            source: "홍길동일보".into(),
            url: "https://m.example.com/x".into(),
            summary: "본문 내용입니다.\n".repeat(40),
            published: String::new(),
        }
    }

    fn render_to_string(app: &App) -> String {
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// 발췌가 채워진 항목은 제목과 발췌를 렌더한다(한국어).
    #[test]
    fn renders_title_and_summary_when_populated() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.article_view = Some(ArticleView {
            item: sample(),
            scroll: 0,
        });
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남긴다.
        let compact: String = render_to_string(&app)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(compact.contains("제목텍스트"), "title missing:\n{compact}");
        assert!(compact.contains("본문"), "summary missing");
    }

    /// 과도한 scroll 값이어도 clamp되어 패닉 없이 렌더된다(빈 공간 방어).
    #[test]
    fn over_scroll_is_clamped_without_panic() {
        let mut app = App::new(Default::default());
        app.article_view = Some(ArticleView {
            item: sample(),
            scroll: 9999,
        });
        let _ = render_to_string(&app); // 패닉 없으면 통과
    }

    /// line_rows는 `ui::text::display_width`(비ASCII=2칸 고정)를 쓰므로
    /// 악센트(é, ñ 등) 같은 실제 width-1 비ASCII 문자를 두 배로 과다 계산한다
    /// — width-1 문자 200개를 40칸에 나누면 실제로는 5행(200/40)이지만
    /// 과다계산은 400/40=10행을 낸다. 이 과다계산은 버그가 아니라 **의도된
    /// 안전마진**이다: ratatui의 실제 렌더는 word-wrap이라 줄 끝에 단어가
    /// 안 들어가면 다음 행으로 넘겨 슬랙(빈 여백)이 생기고, 그만큼 실제 행
    /// 수는 char-wrap 추정치보다 커질 수 있다. display_width의 과다계산이
    /// 그 슬랙을 미리 덮어줘서 스크롤 상한이 과소추정되는(마지막 줄 도달
    /// 불가) 사고를 막는다.
    #[test]
    fn line_rows_over_estimates_width_one_non_ascii_as_safety_margin() {
        let line = Line::from("á".repeat(200));
        assert_eq!(
            line_rows(&line, 40),
            10,
            "expected the doubled-width over-estimate, not the exact width"
        );
    }

    /// fix 2-5 무회귀: 한글(전각, width-2)·ASCII 문자는 unicode-width로 재도
    /// 예전과 동일한 값이 나와야 한다(한글은 실제로도 2칸이라 무회귀).
    #[test]
    fn line_rows_unchanged_for_korean_and_ascii() {
        let ko = Line::from("한".repeat(10)); // 10자 * 2칸 = 20칸
        assert_eq!(line_rows(&ko, 8), 3); // ceil(20/8) = 3, 예전과 동일

        let ascii = Line::from("hello world".to_string()); // 11칸
        assert_eq!(line_rows(&ascii, 5), 3); // ceil(11/5) = 3, 예전과 동일
    }

    /// 실제 word-wrap slack이 쌓이는 회귀 재현: 폭(inner≈74)보다 짧지만 74칸
    /// 행에 2개만 들어가는 긴 악센트 "단어"(25칸, 공백 없는 단일 토큰)를 줄당
    /// 10개씩 6줄 채운다. word-wrap은 단어 경계에서 넘기므로 한 행에 2단어
    /// (51/74칸)만 쓰고 23칸을 버려 실제 렌더 행 수(5행/줄)가 char-wrap
    /// 추정치(4행/줄, `Line::width()`)보다 커진다 — 총 6줄 슬랙이 쌓이면
    /// scroll을 최댓값(u16::MAX)으로 줘도 clamp된 scroll이 실제 마지막 줄
    /// (원문 CTA)에 못 미친다.
    ///
    /// `article_hint`(하단 힌트)에도 "artículo completo"가 들어 있어 그 문구로
    /// 단언하면 스크롤과 무관하게 항상 통과하는 가짜 테스트가 된다. 대신
    /// `article_read_full`(CTA 줄)에만 있는 "pulsa Enter u o"로 단언한다.
    ///
    /// 되돌린 과대추정 heuristic(`display_width`)이 CTA 도달을 보장한다.
    /// `Line::width()`로 정밀화된 채로 두면(word-wrap slack을 못 덮으면) 이
    /// 테스트가 FAIL한다.
    #[test]
    fn extreme_scroll_with_accented_summary_still_reaches_cta_line() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Es; // article_read_full="Extracto — lee el artículo completo: pulsa Enter u o"
        let mut item = sample();
        let word = "á".repeat(25); // 74칸 행에 2개만 들어가는 긴 악센트 단일 토큰
        let line = std::iter::repeat(word)
            .take(10)
            .collect::<Vec<_>>()
            .join(" ");
        item.summary = std::iter::repeat(line)
            .take(6)
            .collect::<Vec<_>>()
            .join("\n");
        app.article_view = Some(ArticleView {
            item,
            scroll: u16::MAX,
        });
        let text = render_to_string(&app);
        assert!(
            text.contains("pulsa Enter u o"),
            "scrolled-to-end should still reveal the CTA line, got:\n{text}"
        );
    }

    /// fix 2-4: 하단 힌트(title_bottom)는 title 영역 폭(테두리 2칸 제외)을
    /// 넘지 않게 ellipsize된다 — 좁은 터미널에서도 박스 모서리(└ ┘)가 힌트
    /// 텍스트로 덮이면 안 된다(리뷰 지적).
    #[test]
    fn bottom_hint_never_overwrites_box_corners_at_narrow_width() {
        let mut app = App::new(Default::default());
        app.article_view = Some(ArticleView {
            item: sample(),
            scroll: 0,
        });
        for width in [10u16, 15, 20, 30, 52, 80] {
            let area = Rect::new(0, 0, width, 24);
            let mut term = Terminal::new(TestBackend::new(width, 24)).unwrap();
            term.draw(|f| render(f, f.area(), &app)).unwrap();
            let buf = term.backend().buffer().clone();

            let w = area.width.saturating_sub(4).max(1);
            let h = area.height.saturating_sub(2).max(1);
            let rect = super::super::help_rect(w, h, area);
            let bottom_y = rect.y + rect.height - 1;
            let left_x = rect.x;
            let right_x = rect.x + rect.width - 1;

            assert_eq!(
                buf[(left_x, bottom_y)].symbol(),
                "└",
                "width {width}: bottom-left corner overwritten by hint"
            );
            assert_eq!(
                buf[(right_x, bottom_y)].symbol(),
                "┘",
                "width {width}: bottom-right corner overwritten by hint"
            );
        }
    }
}
