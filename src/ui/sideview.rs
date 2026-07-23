use super::strikezone::{result_color, zone_bounds};
use crate::model::Pitch;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{
        canvas::{Canvas, Line as CanvasLine},
        Block,
    },
    Frame,
};

/// 측면 뷰 밴드 높이(줄). strikezone::render의 3분할 레이아웃이 쓴다.
pub(crate) const SIDE_HEIGHT: u16 = 8;

/// 릴리스(t=0)→플레이트 통과(t=plate_t)를 n등분한 (y거리, z높이) 샘플.
/// plate_t가 0 이하(미상)면 빈 Vec — 호출부는 그 투구의 궤적을 생략한다.
pub fn trajectory_points(p: &Pitch, n: usize) -> Vec<(f64, f64)> {
    if p.plate_t <= 0.0 || n == 0 {
        return vec![];
    }
    (0..=n)
        .map(|i| {
            let t = p.plate_t * (i as f32) / (n as f32);
            let y = p.y0 + p.vy0 * t + 0.5 * p.ay * t * t;
            let z = p.z0 + p.vz0 * t + 0.5 * p.az * t * t;
            (y as f64, z as f64)
        })
        .collect()
}

/// 측면 뷰: x축 = 홈플레이트로부터의 거리(ft, 좌=플레이트/우=릴리스),
/// y축 = 높이(ft). 투구 궤적(낙차)을 결과색 선으로 그리고 플레이트 통과점에
/// 순번을 찍는다. 플레이트 위치(x≈0.7)에 존 상·하한 눈금을 세로선으로 표시.
pub fn render(f: &mut Frame, area: Rect, pitches: &[Pitch], selected: Option<usize>) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (sz_top, sz_bottom) = zone_bounds(pitches);
    let canvas = Canvas::default()
        .block(Block::bordered().title(" Side "))
        .x_bounds([0.0, 55.0])
        .y_bounds([0.0, 8.0])
        .paint(move |ctx| {
            ctx.draw(&CanvasLine {
                x1: 0.7,
                y1: sz_bottom,
                x2: 0.7,
                y2: sz_top,
                color: Color::White,
            });
            for (idx, p) in pitches.iter().enumerate() {
                if let Some(sel) = selected {
                    if idx != sel {
                        continue; // 선택 모드: 그 구만(겹침 해소)
                    }
                }
                let color = result_color(p.result);
                let pts = trajectory_points(p, 16);
                for w in pts.windows(2) {
                    ctx.draw(&CanvasLine {
                        x1: w[0].0,
                        y1: w[0].1,
                        x2: w[1].0,
                        y2: w[1].1,
                        color,
                    });
                }
                if let Some((y, z)) = pts.last() {
                    ctx.print(
                        *y,
                        *z,
                        Span::styled(p.order.to_string(), Style::default().fg(color)),
                    );
                }
            }
        });
    f.render_widget(canvas, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Pitch, PitchResult};
    use ratatui::{backend::TestBackend, Terminal};

    /// 실측에 가까운 궤적 파라미터의 투구(y0≈50ft, vy0≈-130ft/s).
    fn traj_pitch(order: u8) -> Pitch {
        Pitch {
            order,
            plate_x: 0.0,
            plate_y: 2.5,
            sz_top: 3.3,
            sz_bottom: 1.5,
            speed_kmh: Some(140),
            result: PitchResult::Ball,
            text: format!("{order}구 볼"),
            // plate_t=0.38 solves y(t)≈0 for these y0/vy0/ay (brief's 8/21≈0.381
            // boundary sits right at the "< 2.0" assertion; 0.39 clears it with margin).
            plate_t: 0.39,
            y0: 50.0,
            vy0: -130.0,
            ay: 21.0,
            // order로 z0를 갈라 각 투구의 플레이트 도달 높이를 서로 다른 canvas
            // row에 떨어뜨린다 — 동일 궤적이면 순번 마커가 같은 셀에서 겹쳐써진다.
            // 0.9ft 간격: order 3도 y_bounds([0,8]) 안(z0=7.8)에 들어오도록.
            z0: 6.0 + (order as f32 - 1.0) * 0.9,
            vz0: -0.5,
            az: -21.0,
            ..Default::default()
        }
    }

    #[test]
    fn trajectory_points_run_from_release_to_plate() {
        let pts = trajectory_points(&traj_pitch(1), 16);
        assert_eq!(pts.len(), 17); // 0..=n
        let (y_first, _) = pts[0];
        let (y_last, z_last) = *pts.last().unwrap();
        assert!((y_first - 50.0).abs() < 0.01, "starts at release y0");
        assert!(y_last < 2.0, "ends near the plate, got {y_last}");
        assert!(
            z_last > 0.0 && z_last < 6.0,
            "plate height plausible: {z_last}"
        );
    }

    #[test]
    fn trajectory_points_empty_when_plate_t_unknown() {
        let mut p = traj_pitch(1);
        p.plate_t = 0.0;
        assert!(trajectory_points(&p, 16).is_empty());
    }

    /// 완전성: 모든 투구의 순번 마커가 Side 영역에 전부 렌더된다.
    #[test]
    fn side_view_renders_every_pitch_order_marker() {
        let pitches: Vec<Pitch> = (1u8..=3).map(traj_pitch).collect();
        let mut term = Terminal::new(TestBackend::new(60, 10)).unwrap();
        term.draw(|f| render(f, f.area(), &pitches, None)).unwrap();
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        for o in ["1", "2", "3"] {
            assert!(text.contains(o), "order {o} missing in side view:\n{text}");
        }
    }

    #[test]
    fn tiny_area_does_not_panic() {
        let mut term = Terminal::new(TestBackend::new(1, 1)).unwrap();
        term.draw(|f| render(f, f.area(), &[traj_pitch(1)], None))
            .unwrap();
    }

    /// 선택 시 Side 밴드에도 그 구 궤적만 남는다.
    #[test]
    fn selection_filters_side_band_to_the_selected_trajectory_only() {
        let pitches: Vec<Pitch> = (1u8..=3).map(traj_pitch).collect();
        let mut term = Terminal::new(TestBackend::new(60, 10)).unwrap();
        term.draw(|f| render(f, f.area(), &pitches, Some(2)))
            .unwrap(); // order 3 선택
        let text: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains('3'));
        assert!(
            !text.contains('1') && !text.contains('2'),
            "unselected trajectories filtered:\n{text}"
        );
    }
}
