//! F2 옵션 픽커 오버레이 + 공용 chooser(링크 픽커도 재사용).
use super::theme::team_badge_style;
use crate::app::{App, Pane};
use crate::dateutil::{format_civil, kst_days};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// Date pane 항목: (표시 라벨, YYYY-MM-DD). 오늘은 now_secs 기준 KST.
pub fn date_items(now_secs: u64) -> Vec<(String, String)> {
    let today = kst_days(now_secs);
    [
        ("Today", 0i64),
        ("Yesterday", -1),
        ("Tomorrow", 1),
        ("-2 days", -2),
        ("-3 days", -3),
        ("+2 days", 2),
        ("+3 days", 3),
    ]
    .into_iter()
    .map(|(label, off)| {
        let d = format_civil(today + off);
        (format!("{label}  {d}"), d)
    })
    .collect()
}

/// Team pane 항목: (라벨, 코드). 첫 항목은 해제(None).
pub fn team_items() -> Vec<(String, Option<String>)> {
    let mut v = vec![("None (clear)".to_string(), None)];
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

pub fn poll_items() -> Vec<(String, u64)> {
    [3u64, 5, 10, 30]
        .into_iter()
        .map(|s| (format!("{s}s live poll"), s))
        .collect()
}

/// app.rs 커서 경계용 항목 수.
pub fn pane_len(pane: Pane, now_secs: u64) -> usize {
    match pane {
        Pane::Date => date_items(now_secs).len(),
        Pane::Team => team_items().len(),
        Pane::Poll => poll_items().len(),
    }
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

/// F2 옵션 오버레이: 상단 pane 탭(활성 브래킷 — 헤더 탭과 같은 문법) + 항목.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(opt) = &app.options else { return };
    let tab = |p: Pane, label: &str| {
        if opt.pane == p {
            format!("[ {label} ]")
        } else {
            format!("  {label}  ")
        }
    };
    let title = format!(
        "Options  {}|{}|{}",
        tab(Pane::Date, "Date"),
        tab(Pane::Team, "Team"),
        tab(Pane::Poll, "Poll")
    );
    let items: Vec<Line> = match opt.pane {
        Pane::Date => date_items(app.now_secs)
            .into_iter()
            .map(|(l, _)| Line::from(l))
            .collect(),
        Pane::Team => team_items()
            .into_iter()
            .map(|(l, code)| match code {
                Some(c) => Line::from(vec![
                    Span::styled(format!(" {c} "), team_badge_style(&c)),
                    Span::raw(" "),
                    Span::raw(l),
                ]),
                None => Line::from(l),
            })
            .collect(),
        Pane::Poll => poll_items()
            .into_iter()
            .map(|(l, _)| Line::from(l))
            .collect(),
    };
    chooser(f, area, &title, &items, opt.cursor);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 완전성: 오버레이가 세 pane 라벨과 현재 pane의 전 항목을 렌더한다.
    #[test]
    fn overlay_renders_all_pane_labels_and_every_current_item() {
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
        for label in ["Date", "Team", "Poll"] {
            assert!(text.contains(label), "pane label {label} missing");
        }
        for (label, _) in date_items(app.now_secs) {
            assert!(text.contains(&label), "date item {label} missing");
        }
    }

    #[test]
    fn team_items_cover_all_ten_teams_plus_none() {
        let items = team_items();
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
