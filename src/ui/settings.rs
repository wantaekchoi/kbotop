//! 인앱 설정 화면(v0.8). F9로 열고, 세로 항목 리스트 + 현재값을 보여준다.
//! 변경은 즉시 config에 저장된다(app.rs). 하단에 저장 상태를 고지한다.
use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(st) = &app.settings else { return };
    let l = app.labels();
    let rows = app.settings_rows();
    let w = area.width.saturating_sub(4).max(1);
    let h = area.height.saturating_sub(2).max(1);
    let rect = super::help_rect(w, h, area);
    let label_w = 16usize; // 라벨 컬럼(전각 안전; ellipsize로 자름)

    // 값 컬럼 예산 = 박스 내부 폭(테두리 + 커서 표식, newslist.rs와 동일 관례) -
    // 라벨 컬럼(label_w, 실제 렌더 폭이 아니라 예산 그대로) - 구분자("  ", 2칸).
    // 라벨을 최악(꽉 찬 16칸)으로 가정해 보수적으로 계산하므로 라벨이 짧아도
    // 행 전체("> " + 라벨 + "  " + 값")가 절대 내부 폭을 넘지 않는다.
    let inner_width = rect.width.saturating_sub(4) as usize; // 테두리(2) + 커서 표식(2)
    let reserved = label_w + 2; // 라벨 컬럼 + 구분자
    let value_w = inner_width.saturating_sub(reserved);

    let items: Vec<ListItem> = rows
        .iter()
        .map(|(_kind, label, val)| {
            let lab = super::text::ellipsize(label, label_w);
            let value = super::text::ellipsize(val, value_w);
            ListItem::new(Line::from(vec![
                Span::raw(lab),
                Span::raw("  "),
                Span::styled(value, Style::default().add_modifier(Modifier::DIM)),
            ]))
        })
        .collect();

    let hint = if st.save_failed {
        l.settings_save_failed
    } else {
        l.settings_hint
    };
    // 하단 힌트도 title 영역 폭(테두리 2칸 제외)을 넘지 않게 ellipsize한다 —
    // 안 그러면 좁은 터미널에서 ratatui가 말줄임 없이 조용히 잘라 박스 경계를
    // 침범할 수 있다(리뷰 지적 fix 2-4).
    let hint_budget = rect.width.saturating_sub(2) as usize;
    let hint = super::text::ellipsize(hint, hint_budget);
    let widget = List::new(items)
        .block(Block::bordered().title(l.title_settings).title_bottom(hint))
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(Some(st.cursor.min(rows.len().saturating_sub(1))));

    f.render_widget(Clear, rect);
    f.render_stateful_widget(widget, rect, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SettingsState;
    use ratatui::{backend::TestBackend, Terminal};

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

    /// 설정 화면이 열려 있으면 항목 라벨과 현재값이 보인다(패닉 없음).
    #[test]
    fn settings_renders_rows_without_panic() {
        let mut app = App::new(Default::default());
        app.settings = Some(SettingsState {
            cursor: 0,
            save_failed: false,
        });
        let text = render_to_string(&app);
        assert!(text.contains("Settings"));
    }

    /// 닫혀 있으면(None) 아무것도 그리지 않는다.
    #[test]
    fn settings_renders_nothing_when_closed() {
        let app = App::new(Default::default());
        let text = render_to_string(&app);
        assert!(!text.contains("Settings"));
    }

    /// 리뷰 지적(Important): 값 컬럼이 ellipsize 없이 그대로 렌더되면, 스페인어처럼
    /// 값이 긴 언어(예: accent_team "Color del equipo")에서 좁은 터미널(40칸)
    /// 진입 시 마지막 글자가 안내("…") 없이 잘려 박스 경계까지 밀린다. 값도
    /// 라벨과 동일하게 예산껏 ellipsize해서 각 행("> "+라벨+"  "+값")이 박스
    /// 내부 폭을 절대 넘지 않아야 한다. 이 테스트는 render()가 실제로 만든
    /// 버퍼 셀을, render()와 동일한 rect/예산 계산으로 재구성한 기댓값과
    /// 정확히 비교한다 — 값이 안 잘리는 값(Poll/ThemePreset/Lang)과 잘리는
    /// 값(Team/ThemeAccent)을 한 번에 봉인한다.
    #[test]
    fn value_column_ellipsizes_within_box_at_narrow_width_es() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Es;
        app.settings = Some(SettingsState {
            cursor: 0,
            save_failed: false,
        });

        let width = 40u16;
        let height = 24u16;
        let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer().clone();

        // render()와 동일한 rect·예산 계산(박스 좌표를 알아야 행별 내용을 검사할 수 있다).
        let area = Rect::new(0, 0, width, height);
        let w = area.width.saturating_sub(4).max(1);
        let h = area.height.saturating_sub(2).max(1);
        let rect = super::super::help_rect(w, h, area);
        let label_w = 16usize;
        let inner_width = rect.width.saturating_sub(4) as usize;
        let value_w = inner_width.saturating_sub(label_w + 2);

        let left_border = rect.x;
        let right_border = rect.x + rect.width - 1;
        let content_start = left_border + 1 + 2; // 테두리(1) + 커서 표식(2)

        let rows = app.settings_rows();
        assert!(!rows.is_empty());
        for (i, (_kind, label, val)) in rows.iter().enumerate() {
            let y = rect.y + 1 + i as u16;

            // 좌우 테두리는 값 렌더로 침범되지 않는다(박스 경계 보존).
            assert_eq!(
                buf[(left_border, y)].symbol(),
                "│",
                "row {i}: left border overwritten"
            );
            assert_eq!(
                buf[(right_border, y)].symbol(),
                "│",
                "row {i}: right border overwritten"
            );

            let content: String = (content_start..right_border)
                .map(|x| buf[(x, y)].symbol())
                .collect();
            let content = content.trim_end();

            let expected_label = super::super::text::ellipsize(label, label_w);
            let expected_value = super::super::text::ellipsize(val, value_w);
            let expected = format!("{expected_label}  {expected_value}");
            assert_eq!(
                content, expected,
                "row {i} content mismatch (label={label:?} val={val:?})"
            );
            assert!(
                super::super::text::display_width(content) <= inner_width,
                "row {i} exceeds inner width {inner_width}: {content:?}"
            );
        }
    }

    /// 무패닉: 라벨 예산조차 못 채우는 극단적으로 좁은 폭에서도, 모든 언어에서
    /// 패닉 없이 렌더된다(saturating_sub 경계 회귀 방지).
    #[test]
    fn narrow_terminal_renders_without_panic_in_every_language() {
        for lang in [
            crate::ui::i18n::Lang::Ko,
            crate::ui::i18n::Lang::En,
            crate::ui::i18n::Lang::Ja,
            crate::ui::i18n::Lang::ZhTw,
            crate::ui::i18n::Lang::Es,
        ] {
            let mut app = App::new(Default::default());
            app.lang = lang;
            app.settings = Some(SettingsState {
                cursor: 0,
                save_failed: false,
            });
            for width in [1u16, 3, 5, 10, 20, 40] {
                let mut term = Terminal::new(TestBackend::new(width, 24)).unwrap();
                term.draw(|f| render(f, f.area(), &app)).unwrap(); // 패닉 없으면 통과
            }
        }
    }

    /// fix 2-4: 하단 힌트(title_bottom)는 title 영역 폭(테두리 2칸 제외)을
    /// 넘지 않게 ellipsize된다 — 좁은 터미널에서도 박스 모서리(└ ┘)가 힌트
    /// 텍스트로 덮이면 안 된다(리뷰 지적).
    #[test]
    fn bottom_hint_never_overwrites_box_corners_at_narrow_width() {
        let mut app = App::new(Default::default());
        app.settings = Some(SettingsState {
            cursor: 0,
            save_failed: false,
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

    /// fix 2-4: save_failed 힌트(더 긴 문구)도 마찬가지로 모서리를 침범하지
    /// 않아야 한다.
    #[test]
    fn save_failed_hint_never_overwrites_box_corners_at_narrow_width() {
        let mut app = App::new(Default::default());
        app.settings = Some(SettingsState {
            cursor: 0,
            save_failed: true,
        });
        for width in [10u16, 15, 20, 30] {
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
