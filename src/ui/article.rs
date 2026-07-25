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
use std::collections::VecDeque;

/// ZWSP(zero-width space, U+200B)·NBSP(non-breaking space, U+00A0)까지 포함해
/// ratatui `StyledGrapheme::is_whitespace()`와 같은 줄바꿈 경계 판정을 문자
/// 단위로 재현한다(`ratatui-core-0.1.2/src/text/grapheme.rs`). NBSP는 시각적
/// 으로 공백이어도 단어 경계로 취급하지 않고(두 단어를 하나로 묶어 안
/// 끊는다), ZWSP는 폭이 0인데도 끊어도 되는 지점으로 취급한다.
fn is_wrap_whitespace(ch: char) -> bool {
    const ZWSP: char = '\u{200B}';
    const NBSP: char = '\u{00A0}';
    ch == ZWSP || (ch.is_whitespace() && ch != NBSP)
}

/// 문자 1개의 터미널 셀 폭. 새 unicode-width 의존성을 추가하지 않고
/// `Span::width()`(ratatui 경유)로 잰다. `WordWrapper`가 실제로 렌더에 쓰는
/// `CellWidth::cell_width()`(`ratatui-widgets-0.3.2/src/buffer/cell_width.rs`)
/// 와 달리 `Span::width()`는 순정 unicode-width라 반각 카타카나 탁점/반탁점
/// (U+FF9E/U+FF9F)의 터미널 호환 보정(+1칸)이 빠져 있다 — 그대로 두면 그
/// 두 문자에 한해 실제보다 좁게 재 **과소추정 위험**이 생기므로 같은 보정을
/// 여기서도 더한다(불변식 유지가 우선이라 사소해 보여도 닫는다).
fn wrap_char_width(ch: char) -> u16 {
    const HALFWIDTH_DAKUTEN: char = '\u{FF9E}';
    const HALFWIDTH_HANDAKUTEN: char = '\u{FF9F}';
    let mut buf = [0u8; 4];
    let s: &str = ch.encode_utf8(&mut buf);
    let w = Span::raw(s).width() as u16;
    if ch == HALFWIDTH_DAKUTEN || ch == HALFWIDTH_HANDAKUTEN {
        w.saturating_add(1)
    } else {
        w
    }
}

/// 한 `Line`이 폭 `cols`에서 차지하는 **실제 렌더 행 수**. `render()`가 실제로
/// 쓰는 `Wrap { trim: false }`(ratatui-widgets 0.3.2 `WordWrapper::process_input`,
/// `reflow.rs`)와 **같은 규칙**으로 행 수를 센다 — char-wrap 폭 나눗셈이
/// 아니라, 공백 경계에서 단어를 커밋하고 대기 중인 단어+공백이 폭을 넘기는
/// 순간 새 행으로 넘기며, 한 단어가 폭보다 길면 그 단어 안에서도 강제로
/// 쪼갠다(`reflow.rs`의 `word_found` / `untrimmed_overflow` /
/// `pending_word_overflow` 세 조건과 "줄이 꺾이는 순간 대기 중인 공백 중
/// 남은 폭만큼만 버린다"는 후처리를 그대로 옮겼다). `render()`는 항상
/// `trim: false`만 쓰므로 이 함수도 그 값을 하드코딩한다 — trim=true 전용
/// 분기(trimmed_overflow/whitespace_overflow, 선행 공백 폐기)는 없다.
///
/// # 이력
/// - v0.7~v0.8: `display_width`(비ASCII=2칸 고정) 기반 char-wrap 추정 —
///   안전하지만 부정확(단어 경계를 모른 채 폭으로만 나눔).
/// - v0.9: `Line::width()`(정확한 unicode-width)로 "정밀화"했다가 되돌림 —
///   여전히 char-wrap 나눗셈이라 word-wrap slack(줄 끝 여백)을 못 덮어,
///   악센트 라틴 발췌를 스크롤할 때 마지막 콘텐츠 줄(원문 CTA)에 도달하지
///   못하는 실제 회귀가 났다. 비ASCII 과다계산이 우연히 그 slack을 가려주던
///   게 "안전마진"이었을 뿐, 진짜 word-wrap 인식은 아니었다(당시 백로그로
///   남김).
/// - v0.15: 그 백로그를 해소 — char-wrap 나눗셈을 버리고 `WordWrapper` 상태
///   기계 자체를 폭 계산 목적으로 재구현했다(이 함수). 더 이상 "우연한
///   과다계산"에 기대지 않는다: slack이 있으면 있는 만큼만 정확히 잡고,
///   없으면 실제 렌더 행 수와 정확히 일치한다. 유일하게 남은 안전마진은
///   `wrap_char_width`의 반각 탁점 보정(위 문서 참고) 뿐이며, 그것도
///   "필요해서"가 아니라 실제 `cell_width()`와 정확히 맞추기 위함이다.
///
/// **목표 불변식(증명이 아니라 테스트된 성질)**: 반환값은 실제 렌더 행 수
/// 이상이어야 한다(과소추정=
/// 스크롤 상한이 낮아져 마지막 줄에 도달 못 하는 실버그, 과대추정=빈 줄까지
/// 스크롤되는 무해한 코스메틱). 아래 `line_rows_matches_or_exceeds_actual_*`
/// 테스트들이 `TestBackend`로 실제 `Paragraph::wrap()` 렌더 행 수와 비교해
/// 폭·콘텐츠 조합별로 이를 봉인하고, `extreme_scroll_with_accented_summary_
/// still_reaches_cta_line`이 v0.9 회귀를 다시 봉인한다.
///
/// **알려진 한계**(v0.15 최종 리뷰의 퍼징이 찾음): 이 함수는 **문자** 단위로
/// 도는데 `WordWrapper`는 **그래핌** 단위로 돌고 `CellWidth::cell_width()`를
/// 쓴다. 그래서 ① 그래핌 클러스터를 쪼갤 수 있다고 잘못 보고(ZWJ·지역표시자
/// 이모지) ② 제어문자를 ratatui는 아예 버리는데(`styled_graphemes()`가 필터)
/// 여기선 남기며 ③ 반각 탁점 조합에서 폭이 어긋나, 그런 입력에선 **한 행 적게
/// 셀 수 있다**. 실제 도달 가능성은 사실상 없다 — `line_rows`에 오는 뉴스
/// 텍스트는 전부 `source::text::normalize_whitespace`(`split_whitespace`)를
/// 거쳐 탭·NBSP·수직탭이 제거되고, CTA 줄은 i18n 상수다. 증상도 "마지막 줄이
/// 한 행 안 잡힘"이라 패닉이 아니라 코스메틱이다. 근본 해결(그래핌 순회 +
/// cell_width + 제어문자 필터)은 백로그.
fn line_rows(line: &Line, cols: usize) -> u16 {
    let max_w: u16 = cols.max(1).min(u16::MAX as usize) as u16;

    let mut rows: u16 = 0;
    let mut line_width: u16 = 0; // 현재 행에 이미 커밋된 폭
    let mut word_width: u16 = 0; // 대기 중인 단어 폭
    let mut word_present = false; // 대기 중인 단어가 있는지(폭0 문자 대비 별도 추적)
    let mut whitespace_width: u16 = 0; // 대기 중인 공백 런의 총 폭
    let mut pending_whitespace: VecDeque<u16> = VecDeque::new(); // 개별 공백 문자 폭(줄 꺾일 때 뒤쪽만 버려야 해서 개별 보관)
    let mut line_has_content = false; // 현재 행에 뭔가(공백이라도) 커밋됐는지
    let mut non_whitespace_previous = false;

    for ch in line.spans.iter().flat_map(|s| s.content.chars()) {
        let is_ws = is_wrap_whitespace(ch);
        let symbol_width = wrap_char_width(ch);

        // ratatui: 폭 자체보다 넓은 심볼은 통째로 무시(스킵)한다.
        if symbol_width > max_w {
            continue;
        }

        let word_found = non_whitespace_previous && is_ws;
        let untrimmed_overflow = !line_has_content
            && word_width
                .saturating_add(whitespace_width)
                .saturating_add(symbol_width)
                > max_w;

        if word_found || untrimmed_overflow {
            // trim:false라 대기 중인 공백은 항상 커밋(선행 공백 폐기 없음).
            if !pending_whitespace.is_empty() {
                line_has_content = true;
            }
            line_width = line_width.saturating_add(whitespace_width);
            whitespace_width = 0;
            pending_whitespace.clear();

            if word_present {
                line_has_content = true;
            }
            line_width = line_width.saturating_add(word_width);
            word_width = 0;
            word_present = false;
        }

        let line_full = line_width >= max_w;
        let pending_word_overflow = symbol_width > 0
            && line_width
                .saturating_add(whitespace_width)
                .saturating_add(word_width)
                >= max_w;

        if line_full || pending_word_overflow {
            rows = rows.saturating_add(1);
            let mut remaining = max_w.saturating_sub(line_width);
            line_width = 0;
            line_has_content = false;

            // 줄 끝에 걸친 공백은 남은 폭만큼만 이번 줄에 흡수하고 버린다.
            while let Some(&front) = pending_whitespace.front() {
                if front > remaining {
                    break;
                }
                whitespace_width = whitespace_width.saturating_sub(front);
                remaining = remaining.saturating_sub(front);
                pending_whitespace.pop_front();
            }

            // 줄이 꺾이자마자 나온 공백은 다음 단어에 포함시키지 않는다.
            if is_ws && pending_whitespace.is_empty() {
                continue;
            }
        }

        if is_ws {
            whitespace_width = whitespace_width.saturating_add(symbol_width);
            pending_whitespace.push_back(symbol_width);
        } else {
            word_width = word_width.saturating_add(symbol_width);
            word_present = true;
        }
        non_whitespace_previous = !is_ws;
    }

    // 남은 대기분 flush(trim:false라 공백도 항상 커밋).
    if !pending_whitespace.is_empty() {
        line_has_content = true;
    }
    if word_present {
        line_has_content = true;
    }
    if line_has_content {
        rows = rows.saturating_add(1);
    }
    rows.max(1) // 빈 줄도 화면에서 빈 1행을 차지한다.
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

    // 콘텐츠 총 행 수(line_rows가 실제 word-wrap 규칙을 시뮬레이션)로 스크롤
    // 상한을 구해 clamp한다 — 상태의 scroll은 무한 증가할 수 있어도(app.rs는
    // saturating만) 빈 공간으로 넘어가지 않게 한다.
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

    /// `line_rows`가 흉내내려는 대상 그 자체 — 실제
    /// `Paragraph::new(text).wrap(Wrap { trim: false })`(article.rs `render()`가
    /// 실제로 쓰는 것과 동일한 설정)를 렌더해 진짜 행 수를 잰다.
    ///
    /// # v0.15b: "빈칸 스캔" 판정을 "손댄 행(cell canary)" 판정으로 교체
    /// 리뷰 지적: 예전 구현은 높이 1짜리 뷰포트를 scroll 0부터 늘려가며 렌더해
    /// "뷰포트가 완전히 빈칸(모든 셀이 `" "`)이 되는 첫 scroll"을 실제 행 수로
    /// 삼았다. 그런데 `Wrap { trim: false }`는 공백이 길게 이어지면 **공백만
    /// 으로 이루어진 정상 행**을 커밋한다(`is_wrap_whitespace`/`line_rows` 위
    /// 문서, ratatui 자신의 `reflow.rs::line_composer_char_plus_lots_of_spaces`
    /// 류 패턴). 그런 행은 화면상 "빈칸"과 셀 내용이 똑같아 예전 오라클이
    /// 실제보다 훨씬 적게 셌다(`"a"+공백20+"b"` @폭1 → 예전 1, 진짜 12).
    ///
    /// 새 구현은 "빈칸이냐"가 아니라 "그 행을 렌더러가 실제로 건드렸느냐"로
    /// 판정한다: 렌더 영역을 콘텐츠에 절대 나오지 않는 sentinel 문자
    /// (U+E000, 사설 영역)로 미리 채운 `Buffer`를 만들고, 그 위에 `Paragraph`
    /// 를 **스크롤 없이 한 번만**(`text_area.height`를 콘텐츠가 절대 못 넘을
    /// 만큼 넉넉히 잡아서) 렌더한다. `ratatui-widgets-0.3.2` `paragraph.rs`
    /// `render_lines`/`render_line`을 보면, 렌더러는 각 행의 `WrappedLine`이
    /// 가진 grapheme만큼만 셀을 덮어쓰고 그 이상은 손대지 않는다 — 그리고
    /// `trim:false`에서 공백만 있는 행도 공백 grapheme을 실제로 커밋하므로
    /// (선행 공백 폐기 없음) 그 행의 첫 셀은 sentinel이 아닌 `" "`로 바뀐다.
    /// 즉 "행이 존재하는가"와 "행이 빈칸으로 보이는가"가 이제 셀 값 수준에서
    /// 구별된다: sentinel이 하나라도 지워진 행 = 실제로 존재하는 행, 처음부터
    /// 끝까지 sentinel 그대로인 행 = 콘텐츠가 거기까지 도달하지 못했다는 뜻
    /// (`render_lines`는 `composer.next_line()`이 `None`을 반환하는 순간
    /// 멈추므로 "건드린 행"은 항상 0번부터 연속이다 — 중간에 구멍이 없다).
    ///
    /// # 주의: 이 함수는 여전히 진짜 `render()` 코드 경로를 그대로 타므로,
    /// 극좁은 폭(1~3)에서 넓은 문자를 섞으면 리뷰가 발견한 별개의 ratatui
    /// 버그(`WordWrapper`가 `max_line_width`보다 넓은 행을 내 `render_line`의
    /// 버퍼 인덱싱이 패닉)가 여기서도 그대로 재현된다(직접 확인:
    /// `"a안b녕c"` @폭2 호출 시 패닉). 이 함수를 쓰는 테스트는 그런 조합을
    /// 피해야 한다 — 아래 콤보 목록은 넓은 문자(한글 등)를 폭 10 미만에서
    /// 쓰지 않는다. 반대로 이번에 새로 추가한 두 사례(공백+ASCII, 폭1)는
    /// 전부 폭1 단일문자라 이 패닉과 무관함을 직접 실행으로 확인했다.
    fn actual_rendered_rows(text: &str, cols: u16) -> u16 {
        use ratatui::buffer::{Buffer, Cell};
        use ratatui::layout::Rect;
        use ratatui::widgets::Widget;

        /// 콘텐츠에 절대 나오지 않는 사설 영역(Private Use Area) 문자 —
        /// 공백도 아니고 어떤 실제 텍스트에도 안 쓰이므로 "렌더러가 이
        /// 셀을 건드렸는가"를 오검출 없이 판정하는 카나리아로 쓴다.
        const SENTINEL: &str = "\u{E000}";

        let width = cols.max(1);
        // 콘텐츠가 아무리 길어도 넘지 않을 높이. `line_rows`의 불변식(행 수는
        // 항상 렌더 행 수 이상)과 마찬가지로, trim:false에서 매 행은 최소
        // grapheme 1개 이상을 커밋해야 존재하므로(문자 하나가 여러 행에
        // 걸쳐 나뉘어 재사용되는 일은 없다) 총 행 수는 문자 수를 넘을 수
        // 없다 — 안전한 상한.
        let height = (text.chars().count() as u16).saturating_add(1).max(1);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::filled(area, Cell::new(SENTINEL));

        let paragraph = Paragraph::new(Line::from(text.to_string())).wrap(Wrap { trim: false });
        (&paragraph).render(area, &mut buf);

        let mut rows = 0u16;
        for y in 0..height {
            let touched = (0..width).any(|x| buf[(x, y)].symbol() != SENTINEL);
            if touched {
                rows = y + 1;
            } else {
                // render_lines는 next_line()이 None을 반환하는 순간 멈추므로
                // 건드린 행은 항상 0번부터 연속 — 첫 미건드림에서 멈춰도 안전.
                break;
            }
        }
        rows
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

    /// v0.15 이전: `display_width`(비ASCII=2칸 고정) 휴리스틱이 "á" 200개(폭1
    /// 문자, 공백 없는 단일 토큰)를 40칸에서 400/40=10행으로 **과다** 계산
    /// 했다(실제는 200/40=5행). 그 과다계산이 "의도된 안전마진"이었다.
    ///
    /// v0.15: word-wrap을 직접 시뮬레이션하므로 더 이상 과다계산이 필요 없다
    /// — 공백이 전혀 없는 단일 토큰은 word-wrap도 char-wrap과 똑같이 폭마다
    /// 강제로 쪼개므로(끊을 단어 경계가 없어 slack이 생길 자리가 없다) 정확한
    /// 값 5가 나온다. `actual_rendered_rows`(실제 `Paragraph::wrap()` 렌더)와도
    /// 정확히 일치함을 함께 확인해 "5가 맞는 답"이라는 주장 자체를 검증한다.
    #[test]
    fn line_rows_exact_for_unbreakable_long_word_no_slack_possible() {
        let text = "á".repeat(200);
        let line = Line::from(text.clone());
        assert_eq!(
            line_rows(&line, 40),
            5,
            "single unbreakable token: word-wrap degenerates to char-wrap, no slack"
        );
        assert_eq!(
            line_rows(&line, 40),
            actual_rendered_rows(&text, 40),
            "estimate must match the real Paragraph::wrap() render exactly here"
        );
    }

    /// 한글(전각, width-2, 공백 없는 단일 토큰)은 v0.9 이전에도, word-wrap
    /// 시뮬레이션인 v0.15에서도 같은 값(무회귀) — 전각 문자는 실제로도 2칸이라
    /// char-wrap과 word-wrap이 우연히 같고, 20칸을 8칸 단위로 나누면 딱 떨어지지
    /// 않아 강제분할 지점도 char-wrap과 동일하다.
    ///
    /// ASCII "hello world"(공백 있는 두 단어)는 다르다 — 예전 char-wrap
    /// 추정(ceil(11/5)=3)은 "hello"가 5칸 행을 정확히 채우고 "world"가 새
    /// 행을 시작한다는 실제 word-wrap 결과(2행)보다 1행 많은 **과다추정**이었다
    /// (불변식 위반은 아니었지만 부정확했다). word-wrap을 직접 시뮬레이션하는
    /// v0.15는 정확한 값 2를 낸다 — `actual_rendered_rows`와 동일함을 같이
    /// 확인한다.
    #[test]
    fn line_rows_exact_for_korean_and_ascii_word_boundary() {
        let ko_text = "한".repeat(10); // 10자 * 2칸 = 20칸, 공백 없음
        let ko = Line::from(ko_text.clone());
        assert_eq!(line_rows(&ko, 8), 3); // ceil(20/8) = 3, char-wrap과 우연히 동일
        assert_eq!(line_rows(&ko, 8), actual_rendered_rows(&ko_text, 8));

        let ascii_text = "hello world".to_string(); // "hello"(5칸) + " " + "world"(5칸)
        let ascii = Line::from(ascii_text.clone());
        assert_eq!(
            line_rows(&ascii, 5),
            2,
            "word boundary at the space means no forced mid-word split needed"
        );
        assert_eq!(line_rows(&ascii, 5), actual_rendered_rows(&ascii_text, 5));
    }

    /// 실제 word-wrap slack이 쌓이는 회귀 재현: 폭(inner≈74)보다 짧지만 74칸
    /// 행에 2개만 들어가는 긴 악센트 "단어"(25칸, 공백 없는 단일 토큰)를 줄당
    /// 10개씩 6줄 채운다. word-wrap은 단어 경계에서 넘기므로 한 행에 2단어
    /// (51/74칸)만 쓰고 23칸을 버려 실제 렌더 행 수(5행/줄)가 char-wrap
    /// 추정치(4행/줄, `Line::width()`)보다 커진다 — 총 6줄 슬랙이 쌓이면
    /// scroll을 최댓값(u16::MAX)으로 줘도 clamp된 scroll이 실제 마지막 줄
    /// (원문 CTA)에 못 미친다.
    ///
    /// 이 마진을 RED로 드러내는 건 **폭1 비ASCII summary 콘텐츠**이고, summary는
    /// UI 언어와 무관한 데이터다(chrome/CTA만 app.lang을 따른다). 번체 중국어·
    /// 스페인어 UI 지원 종료 후에도 chrome은 생존 언어(Ko)로 두고 summary만
    /// 악센트 라틴 그대로 유지해 마진 트리거를 보존한다.
    ///
    /// `article_hint`(하단 힌트, ko=" Esc 닫기 · Enter/o 원문 전체 · j/k 스크롤 ")
    /// 에도 "원문 전체"가 들어 있어 그 문구로 단언하면 스크롤과 무관하게 항상
    /// 통과하는 가짜 테스트가 된다. 대신 `article_read_full`(CTA 줄, ko="발췌입니다
    /// — 원문 전체는 Enter 또는 o를 누르세요")에만 있는 "누르세요"로 단언한다.
    ///
    /// 되돌린 과대추정 heuristic(`display_width`)이 CTA 도달을 보장한다.
    /// `Line::width()`로 정밀화된 채로 두면(word-wrap slack을 못 덮으면) 이
    /// 테스트가 FAIL한다.
    #[test]
    fn extreme_scroll_with_accented_summary_still_reaches_cta_line() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko; // article_read_full="발췌입니다 — 원문 전체는 Enter 또는 o를 누르세요"
        let mut item = sample();
        let word = "á".repeat(25); // 74칸 행에 2개만 들어가는 긴 악센트 단일 토큰
        let line = std::iter::repeat_n(word, 10).collect::<Vec<_>>().join(" ");
        item.summary = std::iter::repeat_n(line, 6).collect::<Vec<_>>().join("\n");
        app.article_view = Some(ArticleView {
            item,
            scroll: u16::MAX,
        });
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남기므로
        // (다른 테스트와 동일한 이유) 공백을 걷어내고 비교한다.
        let compact: String = render_to_string(&app)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            compact.contains("누르세요"),
            "scrolled-to-end should still reveal the CTA line, got:\n{compact}"
        );
    }

    /// (name, estimate, actual)을 표에 쌓고, `estimate < actual`(과소추정,
    /// 불변식 위반)이면 `violations`에 적립하는 공용 채점기. 원래 7×5=35
    /// 조합 그리드와, v0.15b에서 추가한 공백-과다 2사례 그리드가 같은 판정
    /// 로직을 공유하게 해 "오라클을 고쳤더니 판정 기준도 같이 바뀌었다"는
    /// 의심을 없앤다.
    fn score_combo(
        name: &str,
        text: &str,
        cols: u16,
        table: &mut String,
        violations: &mut Vec<String>,
    ) {
        let line = Line::from(text.to_string());
        let estimate = line_rows(&line, cols as usize);
        let actual = actual_rendered_rows(text, cols);
        let verdict = match estimate.cmp(&actual) {
            std::cmp::Ordering::Equal => "exact",
            std::cmp::Ordering::Greater => "over(safe)",
            std::cmp::Ordering::Less => "UNDER!!",
        };
        table.push_str(&format!(
            "{name:<41} {cols:>4}  {estimate:>8}  {actual:>6}  {verdict}\n"
        ));
        if estimate < actual {
            violations.push(format!(
                "{name} cols={cols}: estimate={estimate} < actual={actual}"
            ));
        }
    }

    /// v0.15 핵심 불변식 봉인: `line_rows` 추정치는 실제 `Paragraph::wrap()`
    /// 렌더 행 수 이상이어야 한다(과소추정=스크롤 상한이 낮아 마지막 줄에
    /// 도달 못 하는 실버그, 과대추정=빈 줄까지 스크롤되는 무해한 코스메틱).
    /// ASCII 짧은 단어(정상 word-wrap)·ASCII 긴 단어(강제 분할)·한글(전각,
    /// 공백 있음/없음)·악센트 라틴(v0.9 회귀의 주인공, 공백 있음/없음)·혼합
    /// 콘텐츠를 여러 폭(10/20/37/74/120)에서 `actual_rendered_rows`(실제 렌더)
    /// 와 비교한다. word-wrap을 직접 시뮬레이션하므로 대부분 정확히 일치해야
    /// 하고, 실패하면 (이름, 폭, 추정치, 실제)를 출력한다.
    ///
    /// v0.15b: `actual_rendered_rows`를 cell-canary 방식으로 강화했지만, 이
    /// 그리드(7 사례 × 5 폭 = 35 조합)의 기존 폭·콘텐츠 조합 자체는 전혀
    /// 바꾸지 않았다 — 오라클을 고쳐도 기존 판정이 안 깨짐을 그대로 봉인한다.
    #[test]
    fn line_rows_matches_or_exceeds_actual_render_across_combinations() {
        let cases: [(&str, String); 7] = [
            (
                "ascii_short_words",
                "The quick brown fox jumps over the lazy dog and keeps running \
                 through the field without stopping for anything at all today"
                    .to_string(),
            ),
            (
                "ascii_long_word_forced_split",
                format!("prefix {} suffix", "x".repeat(150)),
            ),
            (
                "korean_fullwidth_spaced",
                "안녕하세요 반갑습니다 오늘 경기 결과를 확인해 보겠습니다 야구 중계 화면".repeat(2),
            ),
            ("korean_fullwidth_no_space", "한글전각문자테스트".repeat(20)),
            (
                "accented_latin_spaced",
                "café résumé naïve façade jalapeño piñata león ñandú".repeat(4),
            ),
            ("accented_latin_no_space_v0_9_regression", "á".repeat(300)),
            (
                "mixed_ascii_korean_accented",
                "Game 경기 café ñ 결과 abc123 한글 mixed content 테스트 é win".repeat(3),
            ),
        ];
        let widths: [u16; 5] = [10, 20, 37, 74, 120];

        let mut table = String::from(
            "case                                     cols  estimate  actual  verdict\n",
        );
        let mut violations: Vec<String> = Vec::new();
        for (name, text) in &cases {
            for &cols in &widths {
                score_combo(name, text, cols, &mut table, &mut violations);
            }
        }
        eprintln!("{table}");
        assert!(
            violations.is_empty(),
            "line_rows underestimated actual rendered rows (invariant violated):\n{}",
            violations.join("\n")
        );
    }

    /// v0.15b 리뷰 대응: 오라클의 판정 결함(빈칸=미렌더로 오인)을 정확히
    /// 드러내는 두 사례를 봉인한다 — 리뷰어가 손 트레이스로 확인한 값과
    /// `line_rows`·강화된 오라클이 정확히 일치해야 한다(수용 기준 1, 2).
    ///
    /// - `"a"` + 공백 20개 + `"b"` @폭1: "a"가 1행, 공백 20개가 각 1칸씩
    ///   20행(폭1이라 공백 하나도 한 행을 꽉 채워 곧장 꺾인다), "b"가 1행 —
    ///   총 1+20+1=22가 아니라 **12**인 이유는 `Wrap{trim:false}`가 줄이 꺾일
    ///   때 "이번 줄에 걸친 만큼만" 공백을 흡수하고 나머지는 버리기
    ///   때문이다(line_rows 위 문서의 `pending_whitespace` 후처리, 리뷰가
    ///   손 트레이스로 확인). 폭1 단일문자 콘텐츠라 극좁은 폭 패닉과 무관함을
    ///   `actual_rendered_rows` 자체 실행으로 확인했다(별도 프로브에서
    ///   패닉 없이 12 반환 재현).
    /// - `"a"` + 공백 70개 @폭1: 같은 이유로 **36**.
    ///
    /// 두 사례 모두 이 그리드에만 있는 폭1(다른 7사례 그리드는 폭10 미만을
    /// 안 쓴다 — 넓은 문자 포함 콘텐츠를 폭1~3에서 렌더하면 별개의 ratatui
    /// 버그로 패닉하기 때문, 위 `actual_rendered_rows` 문서 참고)을 포함해도
    /// 안전하다 — 폭1 단일문자(ASCII/공백)뿐이라 그 패닉 조건과 무관하다.
    #[test]
    fn oracle_and_line_rows_exact_for_whitespace_heavy_review_cases() {
        let a_then_spaces_then_b = "a".to_string() + &" ".repeat(20) + "b";
        let a_then_many_spaces = "a".to_string() + &" ".repeat(70);

        assert_eq!(
            actual_rendered_rows(&a_then_spaces_then_b, 1),
            12,
            "oracle must count whitespace-only rows, not treat them as blank/unrendered"
        );
        assert_eq!(
            line_rows(&Line::from(a_then_spaces_then_b.clone()), 1),
            12,
            "line_rows was already correct per review hand-trace"
        );

        assert_eq!(
            actual_rendered_rows(&a_then_many_spaces, 1),
            36,
            "oracle must count whitespace-only rows, not treat them as blank/unrendered"
        );
        assert_eq!(
            line_rows(&Line::from(a_then_many_spaces.clone()), 1),
            36,
            "line_rows was already correct per review hand-trace"
        );

        // 조합 목록에도 봉인(수용 기준 2): line_rows와 강화된 오라클이
        // 정확히 일치함을 표로 남긴다.
        let mut table = String::from(
            "case                                     cols  estimate  actual  verdict\n",
        );
        let mut violations: Vec<String> = Vec::new();
        score_combo(
            "whitespace_ascii_a_spaces20_b",
            &a_then_spaces_then_b,
            1,
            &mut table,
            &mut violations,
        );
        score_combo(
            "whitespace_ascii_a_spaces70",
            &a_then_many_spaces,
            1,
            &mut table,
            &mut violations,
        );
        eprintln!("{table}");
        assert!(
            violations.is_empty(),
            "line_rows underestimated actual rendered rows for whitespace-heavy content:\n{}",
            violations.join("\n")
        );
    }

    /// fix 2-4: 하단 힌트(title_bottom)는 title 영역 폭(테두리 2칸 제외)을
    /// 넘지 않게 ellipsize된다 — 좁은 터미널에서도 박스 모서리(└ ┘)가 힌트
    /// 텍스트로 덮이면 안 된다(리뷰 지적).
    #[test]
    fn bottom_hint_never_overwrites_box_corners_at_narrow_width() {
        crate::ui::test_support::assert_bottom_hint_keeps_box_corners(
            &[10, 15, 20, 30, 52, 80],
            |app| {
                app.article_view = Some(ArticleView {
                    item: sample(),
                    scroll: 0,
                });
            },
            render,
        );
    }
}
