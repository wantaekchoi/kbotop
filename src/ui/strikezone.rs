use crate::model::{Pitch, PitchResult};
use crate::ui::i18n::Labels;
use crate::ui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier},
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

/// 범례 영역 높이(줄). 테스트의 canvas/legend 영역 경계 산술도 이 상수를 쓴다
/// (v0.1.2 리뷰 Minor: magic number 결합 해소).
pub(crate) const LEGEND_HEIGHT: u16 = 3;

/// 결과별 색-독립 판독 문자(WCAG 1.4.1). 범례 "1B 145km"의 B.
fn result_letter(result: PitchResult) -> char {
    match result {
        PitchResult::Ball => 'B',
        PitchResult::StrikeLooking | PitchResult::StrikeSwinging => 'S',
        PitchResult::Foul => 'F',
        PitchResult::InPlay => 'H',
        PitchResult::Unknown => 'U',
    }
}

/// PitchResult별 표시 색상.
pub(crate) fn result_color(result: PitchResult) -> Color {
    match result {
        PitchResult::Ball => Color::Green,
        PitchResult::StrikeLooking | PitchResult::StrikeSwinging => Color::Red,
        PitchResult::Foul => Color::Yellow,
        PitchResult::InPlay => Color::Cyan,
        PitchResult::Unknown => Color::Gray,
    }
}

/// 첫 투구의 타자별 존 상·하한(없거나 0이면 기본값). sideview와 공유한다.
pub(crate) fn zone_bounds(pitches: &[Pitch]) -> (f64, f64) {
    pitches
        .first()
        .filter(|p| p.sz_top > 0.0)
        .map(|p| (p.sz_top as f64, p.sz_bottom as f64))
        .unwrap_or((DEFAULT_SZ_TOP, DEFAULT_SZ_BOTTOM))
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
/// preset은 마커·범례의 투구 결과색(theme::status_color/status_fg)을 게이트한다
/// (mono면 무채, 그 외엔 기존과 동일) — 리뷰 Important: 이전엔 게이트가 없었다.
pub fn render(
    f: &mut Frame,
    area: Rect,
    pitches: &[Pitch],
    selected: Option<usize>,
    l: &'static Labels,
    preset: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    use super::sideview;
    let show_side = area.height > 7 + sideview::SIDE_HEIGHT;
    let (zone_area, side_area, list_area) = if show_side {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),
                Constraint::Length(sideview::SIDE_HEIGHT),
                Constraint::Length(LEGEND_HEIGHT),
            ])
            .split(area);
        (rows[0], Some(rows[1]), Some(rows[2]))
    } else if area.height > 7 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(6), Constraint::Length(LEGEND_HEIGHT)])
            .split(area);
        (rows[0], None, Some(rows[1]))
    } else {
        (area, None, None)
    };

    let (sz_top, sz_bottom) = zone_bounds(pitches);

    // Option<usize>는 Copy라서 paint 클로저(move)가 값을 복사해 캡처해도
    // 아래 render_speed_list 호출에 쓸 원본 selected는 그대로 남는다.
    let canvas = Canvas::default()
        .block(Block::bordered().title(l.title_zone))
        .x_bounds([-2.5, 2.5])
        .y_bounds([-0.5, 5.5])
        .paint(move |ctx| {
            ctx.draw(&Rectangle {
                x: -PLATE_HALF_WIDTH_FT,
                y: sz_bottom,
                width: PLATE_HALF_WIDTH_FT * 2.0,
                height: sz_top - sz_bottom,
                // 배경 무관(v0.9): White는 밝은 배경 터미널에서 사라진다.
                // Reset은 터미널 기본 전경색을 상속하며, 도형 자체는
                // 그대로 그려진다(theme::status_color_reset_still_draws_a_braille_marker_on_canvas,
                // zone_box_reset_color_still_draws_border_glyph_on_canvas로 검증됨).
                color: Color::Reset,
            });
            for (idx, p) in pitches.iter().enumerate() {
                if let Some(sel) = selected {
                    if idx != sel {
                        continue; // 선택 모드: 그 구만(겹침 해소)
                    }
                }
                let raw_color = result_color(p.result);
                let x = p.plate_x as f64;
                let y = p.plate_y as f64;
                ctx.draw(&Rectangle {
                    x: x - 0.05,
                    y: y - 0.05,
                    width: 0.1,
                    height: 0.1,
                    color: theme::status_color(preset, raw_color),
                });
                let mut style = theme::status_fg(preset, raw_color);
                if selected == Some(idx) {
                    style = style.add_modifier(Modifier::REVERSED | Modifier::BOLD);
                }
                ctx.print(x, y, Span::styled(p.order.to_string(), style));
            }
        });

    f.render_widget(canvas, fit_zone(zone_area));

    if let Some(side_area) = side_area {
        sideview::render(f, side_area, pitches, selected, l, preset);
    }

    if let Some(list_area) = list_area {
        render_speed_list(f, list_area, pitches, selected, preset);
    }
}

/// 하단 구속 목록: "{순번}{결과문자} {구속}km" (구속 없으면 순번+결과문자만), 결과별 색상.
/// Line/Span만 쓰고 수동 패딩은 하지 않는다(폭 안전). preset=mono면 색을 걷어낸다.
fn render_speed_list(
    f: &mut Frame,
    area: Rect,
    pitches: &[Pitch],
    selected: Option<usize>,
    preset: &str,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut spans: Vec<Span> = Vec::new();
    for (idx, p) in pitches.iter().enumerate() {
        let color = result_color(p.result);
        let text = match p.speed_kmh {
            Some(kmh) => format!("{}{} {}km", p.order, result_letter(p.result), kmh),
            None => format!("{}{}", p.order, result_letter(p.result)),
        };
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        let mut style = theme::status_fg(preset, color);
        if selected == Some(idx) {
            style = style.add_modifier(Modifier::REVERSED);
        }
        spans.push(Span::styled(text, style));
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
            },
        ]
    }

    fn render_to_string(area_w: u16, area_h: u16, pitches: &[Pitch]) -> String {
        let mut term = Terminal::new(TestBackend::new(area_w, area_h)).unwrap();
        term.draw(|f| {
            let area = f.area();
            render(
                f,
                area,
                pitches,
                None,
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            );
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
                ..Default::default()
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
    fn legend_carries_result_letters_for_every_pitch() {
        // WCAG 1.4.1 색 독립성: 색을 못 봐도 범례에서 결과(B/S/F/H)를 읽어야 한다.
        // 완전성: 서로 다른 결과의 투구 전부가 각자의 문자로 렌더된다.
        let pitches = vec![
            Pitch {
                order: 1,
                speed_kmh: Some(145),
                result: PitchResult::Ball,
                ..Default::default()
            },
            Pitch {
                order: 2,
                speed_kmh: Some(150),
                result: PitchResult::StrikeLooking,
                ..Default::default()
            },
            Pitch {
                order: 3,
                speed_kmh: Some(132),
                result: PitchResult::Foul,
                ..Default::default()
            },
            Pitch {
                order: 4,
                speed_kmh: Some(141),
                result: PitchResult::InPlay,
                ..Default::default()
            },
        ];
        let text = render_to_string(40, 20, &pitches);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for tag in ["1B", "2S", "3F", "4H"] {
            assert!(
                compact.contains(tag),
                "result letter {tag} missing in:\n{text}"
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
            ..Default::default()
        });
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                None,
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        // 하단 범례 LEGEND_HEIGHT줄을 제외한 canvas(존) 영역에서만 마커 '9' 를 찾는다 →
        // 범례 순번에서 주워오는 tautology 를 배제한다.
        let h = buf.area().height;
        let zone_bottom = h.saturating_sub(LEGEND_HEIGHT);
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

    /// y_bounds [-0.5, 5.5] 경계 직전의 낮은/높은 공이 잘리지 않는다(리뷰 Minor).
    #[test]
    fn pitches_near_the_y_bounds_edges_still_render_in_canvas() {
        let mut low = sample_pitches()[0].clone();
        low.order = 7;
        low.plate_y = -0.4; // 하한(-0.5) 직전
        low.speed_kmh = None;
        let mut high = sample_pitches()[0].clone();
        high.order = 8;
        high.plate_y = 5.4; // 상한(5.5) 직전
        high.speed_kmh = None;
        let pitches = vec![low, high];
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                None,
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let h = buf.area().height;
        let zone_bottom = h.saturating_sub(LEGEND_HEIGHT + super::super::sideview::SIDE_HEIGHT);
        let mut found = (false, false);
        for y in 0..zone_bottom {
            for x in 0..buf.area().width {
                match buf[(x, y)].symbol() {
                    "7" => found.0 = true,
                    "8" => found.1 = true,
                    _ => {}
                }
            }
        }
        assert!(found.0, "low boundary pitch (y=-0.4) clipped");
        assert!(found.1, "high boundary pitch (y=5.4) clipped");
    }

    /// 세로 공간이 충분하면 존 컬럼에 Side 밴드가 함께 렌더된다.
    #[test]
    fn tall_area_shows_side_band_between_zone_and_legend() {
        let mut pitches = sample_pitches();
        for p in &mut pitches {
            p.plate_t = 0.38;
            p.y0 = 50.0;
            p.vy0 = -130.0;
            p.ay = 21.0;
            p.z0 = 6.0;
            p.vz0 = -0.5;
            p.az = -21.0;
        }
        let text = render_to_string(40, 24, &pitches);
        assert!(text.contains("Side"), "side view title missing:\n{text}");
    }

    /// 공간이 부족하면 Side를 생략하고 기존 레이아웃으로 우아하게 저하한다.
    #[test]
    fn short_area_omits_side_band() {
        let text = render_to_string(40, 12, &sample_pitches());
        assert!(!text.contains("Side"));
    }

    /// 선택된 투구의 범례 항목은 REVERSED로 구분된다.
    #[test]
    fn selected_legend_entry_is_reverse_styled() {
        use ratatui::style::Modifier;
        let pitches = sample_pitches();
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                Some(1),
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let reversed_exists = buf
            .content()
            .iter()
            .any(|c| c.modifier.contains(Modifier::REVERSED));
        assert!(
            reversed_exists,
            "selected pitch must be visibly highlighted"
        );
    }

    /// 선택 시 Zone 캔버스에는 그 구만 남는다(마커 겹침 해소) — 범례는 전 항목 유지.
    #[test]
    fn selection_filters_zone_canvas_to_the_selected_pitch_only() {
        let pitches = sample_pitches(); // order 1,2,3
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                Some(1),
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            )
        })
        .unwrap(); // order 2 선택
        let buf = term.backend().buffer().clone();
        let h = buf.area().height;
        let zone_bottom = h.saturating_sub(LEGEND_HEIGHT + super::super::sideview::SIDE_HEIGHT);
        let (mut saw1, mut saw2, mut saw3) = (false, false, false);
        for y in 0..zone_bottom {
            for x in 0..buf.area().width {
                match buf[(x, y)].symbol() {
                    "1" => saw1 = true,
                    "2" => saw2 = true,
                    "3" => saw3 = true,
                    _ => {}
                }
            }
        }
        assert!(saw2, "selected pitch must render in the zone");
        assert!(
            !saw1 && !saw3,
            "unselected pitches must be filtered out of the zone"
        );
        // 범례(하단 LEGEND_HEIGHT줄)에는 전 순번 유지 — 컨텍스트 보존.
        let mut legend = String::new();
        for y in zone_bottom + super::super::sideview::SIDE_HEIGHT..h {
            for x in 0..buf.area().width {
                legend.push_str(buf[(x, y)].symbol());
            }
        }
        for tag in ["1B", "2S", "3F"] {
            assert!(
                legend.replace(' ', "").contains(tag),
                "legend must keep all entries: {tag}"
            );
        }
    }

    /// 리뷰 Important: mono 프리셋은 존 마커·구속 범례의 투구 결과색
    /// (Green/Red/Yellow/Cyan)을 걷어내야 한다. 결과 문자(B/S/F/H)는 색과
    /// 무관하게 남아야 한다(WCAG 1.4.1 유지, 정보 손실 없음).
    #[test]
    fn mono_preset_strips_pitch_colors_but_keeps_result_letters() {
        use ratatui::style::Color;
        let pitches = vec![
            Pitch {
                order: 1,
                speed_kmh: Some(145),
                result: PitchResult::Ball,
                ..Default::default()
            },
            Pitch {
                order: 2,
                speed_kmh: Some(150),
                result: PitchResult::StrikeLooking,
                ..Default::default()
            },
            Pitch {
                order: 3,
                speed_kmh: Some(132),
                result: PitchResult::Foul,
                ..Default::default()
            },
            Pitch {
                order: 4,
                speed_kmh: Some(141),
                result: PitchResult::InPlay,
                ..Default::default()
            },
        ];
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                None,
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "mono",
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();

        for cell in buf.content() {
            assert!(
                !matches!(
                    cell.fg,
                    Color::Green | Color::Red | Color::Yellow | Color::Cyan
                ),
                "mono strikezone leaked a chromatic pitch color: {:?}",
                cell.fg
            );
        }

        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for tag in ["1B", "2S", "3F", "4H"] {
            assert!(
                compact.contains(tag),
                "mono must keep the result letter {tag}:\n{text}"
            );
        }
    }

    /// 대조(무회귀): mono가 아니면 기존처럼 투구 결과색이 그대로 남는다.
    #[test]
    fn non_mono_preset_keeps_pitch_colors() {
        use ratatui::style::Color;
        let pitches = vec![Pitch {
            order: 1,
            speed_kmh: Some(145),
            result: PitchResult::Ball,
            ..Default::default()
        }];
        let mut term = Terminal::new(TestBackend::new(40, 20)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                None,
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let has_ball_color = buf.content().iter().any(|c| c.fg == Color::Green);
        assert!(
            has_ball_color,
            "non-mono preset must keep the Ball pitch color (Green)"
        );
    }

    /// 배경 무관(v0.9): 존 박스 테두리(Rectangle)는 Color::White로 고정되면
    /// 안 된다(밝은 배경 터미널에서 사라짐). 투구가 있는 라이브 화면을 렌더해
    /// 버퍼 전체에 White fg가 없는지 확인한다. 수정 전(Color::White) 코드면
    /// 이 테스트는 실패(RED)한다. 무회귀로 마커·순번 렌더링과 존 박스 글리프
    /// 자체(사라지지 않음)도 함께 확인한다.
    /// height는 일부러 side band 임계(7+SIDE_HEIGHT=15)를 넘기지 않게 잡는다
    /// — sideview.rs는 이 태스크 범위 밖(자체 White 상수를 별도로 가짐)이라
    /// 버퍼 전체 스캔이 그쪽 이슈까지 잘못 집어내지 않게 격리한다.
    #[test]
    fn zone_border_is_not_fixed_white_and_zone_plus_markers_still_render() {
        use ratatui::style::Color;
        let pitches = sample_pitches();
        let mut term = Terminal::new(TestBackend::new(40, 14)).unwrap();
        term.draw(|f| {
            render(
                f,
                f.area(),
                &pitches,
                None,
                crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                "default",
            )
        })
        .unwrap();
        let buf = term.backend().buffer().clone();

        for cell in buf.content() {
            assert!(
                cell.fg != Color::White,
                "zone border must not be fixed to Color::White \
                 (invisible against a light terminal background): {:?}",
                cell.fg
            );
        }

        // 무회귀 1: 마커 순번은 여전히 렌더된다.
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains('1'),
            "pitch order 1 missing after border color fix:\n{text}"
        );
        assert!(
            text.contains('2'),
            "pitch order 2 missing after border color fix:\n{text}"
        );

        // 무회귀 2: 존 박스(캔버스 Rectangle) 자체가 사라지지 않는다.
        //
        // 리뷰 Important #2: 기존 가드("공백도 빈 braille도 아닌 셀이 있으면 통과")는
        // Block 테두리 문자(┌─┐│└┘)·마커 순번·범례 텍스트만으로도 항상 통과해,
        // 프로덕션에서 존 Rectangle의 ctx.draw 호출을 통째로 지워도 이 테스트가
        // 걸러내지 못했다. 이제는 (a) 투구 0건으로 별도 렌더해 궤적 마커
        // Rectangle을 애초에 안 그리고, (b) Canvas 기본 마커(Marker::Braille,
        // ratatui 0.29 canvas.rs Default 구현)가 찍는 Braille 글리프(U+2801..=U+28FF,
        // 빈 패턴 U+2800 제외)만 스캔한다. Block 테두리는 박스드로잉 문자(U+25xx대)라
        // 이 범위 밖이고, 마커·범례 텍스트도 일반 ASCII라 걸리지 않는다 — 남는
        // Braille 글리프는 오직 존 Rectangle에서만 나올 수 있다.
        let h = buf.area().height;
        let zone_bottom = h.saturating_sub(LEGEND_HEIGHT);

        let mut markerless_term = Terminal::new(TestBackend::new(40, 14)).unwrap();
        markerless_term
            .draw(|f| {
                render(
                    f,
                    f.area(),
                    &[], // 투구 0건: 궤적 마커 Rectangle이 전혀 그려지지 않는다.
                    None,
                    crate::ui::i18n::labels(crate::ui::i18n::Lang::En),
                    "default",
                )
            })
            .unwrap();
        let markerless_buf = markerless_term.backend().buffer().clone();
        let has_zone_braille_glyph = (0..zone_bottom).any(|y| {
            (0..markerless_buf.area().width).any(|x| {
                let mut chars = markerless_buf[(x, y)].symbol().chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) => (0x2801..=0x28FF).contains(&(ch as u32)),
                    _ => false,
                }
            })
        });
        assert!(
            has_zone_braille_glyph,
            "zone box Rectangle braille glyphs disappeared after the color fix \
             (a Block border alone cannot satisfy this guard)"
        );
    }

    /// 결정적 재현(theme::status_color_reset_still_draws_a_braille_marker_on_canvas와
    /// 동일 접근): 존 박스 Rectangle이 Color::Reset이어도 ratatui Canvas가 그 도형을
    /// 그리지 않는 게 아니라 fg만 터미널 기본색으로 상속함을 최소 재현으로 못박는다.
    #[test]
    fn zone_box_reset_color_still_draws_border_glyph_on_canvas() {
        use ratatui::{
            backend::TestBackend,
            widgets::canvas::{Canvas, Rectangle},
            Terminal,
        };

        let mut term = Terminal::new(TestBackend::new(10, 10)).unwrap();
        term.draw(|f| {
            let canvas = Canvas::default()
                .x_bounds([-2.5, 2.5])
                .y_bounds([-0.5, 5.5])
                .paint(move |ctx| {
                    ctx.draw(&Rectangle {
                        x: -PLATE_HALF_WIDTH_FT,
                        y: DEFAULT_SZ_BOTTOM,
                        width: PLATE_HALF_WIDTH_FT * 2.0,
                        height: DEFAULT_SZ_TOP - DEFAULT_SZ_BOTTOM,
                        // 리뷰 Important #1: 이름이 "Reset color still draws" 인데 정작
                        // Color::White를 그려 Reset 경로를 전혀 테스트하지 않았다.
                        // 프로덕션과 동일한 Color::Reset으로 렌더해야 이름과 목적이 맞는다.
                        color: Color::Reset,
                    });
                });
            f.render_widget(canvas, f.area());
        })
        .unwrap();

        let buf = term.backend().buffer().clone();
        let has_border_glyph = buf
            .content()
            .iter()
            .any(|c| c.symbol() != " " && c.symbol() != "\u{2800}");
        assert!(
            has_border_glyph,
            "Color::Reset zone box border must still render a glyph on the canvas \
             (fg falls back to the terminal default, the shape isn't skipped): {:?}",
            buf.content().iter().map(|c| c.symbol()).collect::<String>()
        );
    }
    /// [v0.26 기획용 실측] 폭별로 존이 어떻게 보이는지 센다 — 박스 종횡비,
    /// 범례가 넘치는 줄 수. 지금 40% 고정 분할에 근거가 없어 그 근거를 만든다.
    #[test]
    #[ignore] // 기획용 조사 — `cargo test -- --ignored zone_width_probe`
    fn zone_width_probe() {
        let live = crate::source::naver::map::live_from_relay(
            include_str!("../../tests/fixtures/relay_20260719KTLG.json"),
            crate::model::Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            crate::model::Team {
                code: "KT".into(),
                name: "KT".into(),
            },
        )
        .unwrap();
        let l = crate::ui::i18n::labels(crate::ui::i18n::Lang::Ko);

        for w in [20u16, 24, 28, 32, 36, 40, 48] {
            let h = 24u16;
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| render(f, f.area(), &live.current_pitches, None, l, "default"))
                .unwrap();
            let buf = term.backend().buffer();

            // 존 박스: 점선 테두리(⡇ ⢸ 등)가 아니라 실제 박스 문자를 세는 대신
            // 각 행에서 비지 않은 칸의 좌우 폭을 잰다.
            let mut box_w = 0usize;
            let mut box_h = 0usize;
            for y in 0..h {
                let filled: Vec<usize> = (0..w as usize)
                    .filter(|x| !buf[(*x as u16, y)].symbol().trim().is_empty())
                    .collect();
                if let (Some(a), Some(b)) = (filled.first(), filled.last()) {
                    box_w = box_w.max(b - a + 1);
                    box_h += 1;
                }
            }
            // 범례(맨 아래 LEGEND_HEIGHT 줄)에서 실제로 글자가 있는 줄 수
            let legend_rows = ((h - LEGEND_HEIGHT)..h)
                .filter(|y| (0..w).any(|x| !buf[(x, *y)].symbol().trim().is_empty()))
                .count();
            println!("폭 {w:>2}: 내용폭 {box_w:>2} · 내용높이 {box_h:>2} · 범례 {legend_rows}줄");
        }
    }
}
