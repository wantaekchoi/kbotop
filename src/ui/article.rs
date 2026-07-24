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

/// 한 Line이 폭 `cols`에서 차지하는 시각적 행 수(대략). ratatui의 word-wrap과
/// 정확히 일치하진 않지만(공백 경계 우선) 스크롤 상한·스크롤바 산정에는 충분하다.
/// 전각(CJK) 문자는 display_width가 2로 세므로 CJK 본문에선 char-wrap과 근사한다.
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

    let block = Block::bordered()
        .title(l.title_article)
        .title_bottom(l.article_hint);

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
}
