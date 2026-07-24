//! 뉴스 목록 오버레이(v0.7). 항목이 20건을 넘으므로 ratatui List + ListState로
//! windowing을 맡긴다 — 직접 offset을 계산하지 않는다.
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
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
    let reserved = source_width + GAP.len();

    let items: Vec<ListItem> = app
        .news
        .iter()
        .map(|n| {
            let title = super::text::ellipsize(&n.title, inner_width.saturating_sub(reserved));
            let source = super::text::ellipsize(&n.source, source_width);
            ListItem::new(Line::from(vec![
                Span::raw(title),
                Span::raw(GAP),
                Span::styled(source, Style::default().add_modifier(Modifier::DIM)),
            ]))
        })
        .collect();

    let widget = List::new(items)
        .block(
            Block::bordered()
                .title(l.title_news_list)
                .title_bottom(l.news_list_hint),
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
}
