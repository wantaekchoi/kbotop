use crate::model::{Pitch, PitchResult};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, Paragraph, Wrap,
    },
    Frame,
};

/// 홈플레이트 폭의 절반 (17인치 / 12 / 2, 피트 단위).
const PLATE_HALF_WIDTH_FT: f64 = 17.0 / 12.0 / 2.0;
/// 타자 존 데이터가 없을 때 쓰는 기본값(피트).
const DEFAULT_SZ_TOP: f64 = 3.3;
const DEFAULT_SZ_BOTTOM: f64 = 1.5;

/// PitchResult별 표시 색상.
fn result_color(result: PitchResult) -> Color {
    match result {
        PitchResult::Ball => Color::Green,
        PitchResult::StrikeLooking | PitchResult::StrikeSwinging => Color::Red,
        PitchResult::Foul => Color::Yellow,
        PitchResult::InPlay => Color::Cyan,
        PitchResult::Unknown => Color::Gray,
    }
}

/// mlbt 스타일 종횡비 보정: 터미널 셀은 폭보다 높이가 커서(대략 1:2)
/// 존을 area에 그대로 꽉 채우면 짜부라져 보인다. W:H 비율에 맞는
/// 최대 크기의 rect를 area 중앙에 배치해 돌려준다.
fn fit_zone(area: Rect) -> Rect {
    const W: u16 = 35;
    const H: u16 = 19; // mlbt의 실측 튜닝값
    let ratio = W as f64 / H as f64;
    let wch = (area.width as f64 / ratio) as u16;
    let hcw = (area.height as f64 * ratio) as u16;
    let (fw, fh) = if wch <= area.height {
        (area.width, wch)
    } else {
        (hcw, area.height)
    };
    let xo = area.width.saturating_sub(fw) / 2;
    let yo = area.height.saturating_sub(fh) / 2;
    Rect::new(area.x + xo, area.y + yo, fw.max(1), fh.max(1))
}

/// 스트라이크존: ratatui Canvas 위에 존 박스(Rectangle 외곽선)와 각 투구
/// 위치(작은 Rectangle + 구 순번 텍스트)를 겹쳐 그린다. 공간이 있으면
/// 하단에 최근 투구 구속 목록을 한 줄 덧붙인다.
pub fn render(f: &mut Frame, area: Rect, pitches: &[Pitch]) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let (zone_area, list_area) = if area.height > 7 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(6), Constraint::Length(3)])
            .split(area);
        (rows[0], Some(rows[1]))
    } else {
        (area, None)
    };

    let (sz_top, sz_bottom) = pitches
        .first()
        .filter(|p| p.sz_top > 0.0)
        .map(|p| (p.sz_top as f64, p.sz_bottom as f64))
        .unwrap_or((DEFAULT_SZ_TOP, DEFAULT_SZ_BOTTOM));

    let canvas = Canvas::default()
        .block(Block::bordered().title(" Zone "))
        .x_bounds([-2.5, 2.5])
        .y_bounds([-0.5, 5.5])
        .paint(move |ctx| {
            ctx.draw(&Rectangle {
                x: -PLATE_HALF_WIDTH_FT,
                y: sz_bottom,
                width: PLATE_HALF_WIDTH_FT * 2.0,
                height: sz_top - sz_bottom,
                color: Color::White,
            });
            for p in pitches {
                let color = result_color(p.result);
                let x = p.plate_x as f64;
                let y = p.plate_y as f64;
                ctx.draw(&Rectangle {
                    x: x - 0.05,
                    y: y - 0.05,
                    width: 0.1,
                    height: 0.1,
                    color,
                });
                ctx.print(
                    x,
                    y,
                    Span::styled(p.order.to_string(), Style::default().fg(color)),
                );
            }
        });

    f.render_widget(canvas, fit_zone(zone_area));

    if let Some(list_area) = list_area {
        render_speed_list(f, list_area, pitches);
    }
}

/// 하단 구속 목록: "{순번} {구속}km" (구속 없으면 순번만), 결과별 색상.
/// Line/Span만 쓰고 수동 패딩은 하지 않는다(폭 안전).
fn render_speed_list(f: &mut Frame, area: Rect, pitches: &[Pitch]) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut spans: Vec<Span> = Vec::new();
    for p in pitches {
        let color = result_color(p.result);
        let text = match p.speed_kmh {
            Some(kmh) => format!("{} {}km", p.order, kmh),
            None => format!("{}", p.order),
        };
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(text, Style::default().fg(color)));
    }
    let para = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn sample_pitches() -> Vec<Pitch> {
        vec![
            Pitch {
                order: 1,
                plate_x: -0.3,
                plate_y: 2.5,
                sz_top: 3.3,
                sz_bottom: 1.5,
                speed_kmh: Some(145),
                result: PitchResult::Ball,
                text: "1구 볼".into(),
            },
            Pitch {
                order: 2,
                plate_x: 0.2,
                plate_y: 2.8,
                sz_top: 3.3,
                sz_bottom: 1.5,
                speed_kmh: Some(138),
                result: PitchResult::StrikeLooking,
                text: "2구 스트라이크".into(),
            },
            Pitch {
                order: 3,
                plate_x: 0.0,
                plate_y: 2.2,
                sz_top: 3.3,
                sz_bottom: 1.5,
                speed_kmh: None,
                result: PitchResult::Foul,
                text: "3구 파울".into(),
            },
        ]
    }

    fn render_to_string(area_w: u16, area_h: u16, pitches: &[Pitch]) -> String {
        let mut term = Terminal::new(TestBackend::new(area_w, area_h)).unwrap();
        term.draw(|f| {
            let area = f.area();
            render(f, area, pitches);
        })
        .unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn shows_pitch_order_numbers_and_speed() {
        let pitches = sample_pitches();
        let text = render_to_string(40, 20, &pitches);
        assert!(text.contains('1'), "expected pitch order 1 in:\n{text}");
        assert!(text.contains('2'), "expected pitch order 2 in:\n{text}");
        assert!(text.contains("km"), "expected a speed unit in:\n{text}");
        assert!(text.contains("145"), "expected a speed value in:\n{text}");
    }

    #[test]
    fn empty_pitches_render_without_panic() {
        let text = render_to_string(40, 20, &[]);
        // 빈 목록이어도 존 박스(테두리)는 그려질 수 있으나, 패닉 없이
        // 정상적으로 buffer를 반환하기만 하면 충분하다.
        let _ = text;
    }

    #[test]
    fn tiny_area_does_not_panic() {
        let _text = render_to_string(1, 1, &sample_pitches());
        let _text0 = render_to_string(0, 0, &sample_pitches());
    }

    #[test]
    fn short_area_hides_speed_list_but_still_renders_zone() {
        // height <= 6 → 리스트를 숨기고 존만 렌더. 패닉 없이 동작해야 한다.
        let _text = render_to_string(40, 5, &sample_pitches());
    }

    fn many_pitches() -> Vec<Pitch> {
        // 구속이 서로 다른 8개 — 한 줄에 안 들어가 잘리던 케이스
        (1u8..=8)
            .map(|i| Pitch {
                order: i,
                plate_x: (i as f32 - 4.0) * 0.2,
                plate_y: 2.0 + (i as f32) * 0.1,
                sz_top: 3.3,
                sz_bottom: 1.5,
                speed_kmh: Some(130 + i as u16), // 131..=138, 모두 다름
                result: PitchResult::Ball,
                text: format!("{i}구"),
            })
            .collect()
    }

    #[test]
    fn speed_list_shows_all_pitches_not_truncated() {
        let pitches = many_pitches();
        // 존 우측 폭이 좁아도(40) 8개 구속이 전부 보여야 한다
        let text = render_to_string(40, 20, &pitches);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for kmh in 131..=138u16 {
            assert!(
                compact.contains(&format!("{kmh}km")),
                "speed {kmh}km missing (legend truncated) in:\n{text}"
            );
        }
    }

    #[test]
    fn outlier_pitch_marker_renders_in_canvas_zone_not_just_legend() {
        let mut pitches = sample_pitches();
        pitches.push(Pitch {
            order: 9,
            plate_x: 0.0,
            plate_y: -0.3, // 존 아래(y_bounds 밖이면 canvas에서 클립됨)
            sz_top: 3.3,
            sz_bottom: 1.5,
            speed_kmh: None, // 범례엔 순번만
            result: PitchResult::InPlay,
            text: "9구".into(),
        });
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| render(f, f.area(), &pitches)).unwrap();
        let buf = term.backend().buffer().clone();
        // 하단 범례 3줄을 제외한 canvas(존) 영역에서만 마커 '9' 를 찾는다 →
        // 범례 순번에서 주워오는 tautology 를 배제한다.
        let h = buf.area().height;
        let zone_bottom = h.saturating_sub(3);
        let mut in_zone = false;
        for y in 0..zone_bottom {
            for x in 0..buf.area().width {
                if buf[(x, y)].symbol() == "9" {
                    in_zone = true;
                }
            }
        }
        assert!(
            in_zone,
            "outlier (plate_y=-0.3) marker must render in the canvas zone, not only the legend"
        );
    }
}
