use ratatui::style::{Color, Modifier, Style};

/// KBO 10구단 팀 컬러(터미널 근사).
pub fn team_color(code: &str) -> Color {
    match code {
        "LG" => Color::Rgb(196, 0, 53),
        // KT wiz 공식 색은 순수 검정. team_badge_style/row 하이라이트 bg로 얹고
        // contrast_fg로 대비 글자색을 고르면 글자 자체는 읽히지만, 순수 검정 배경은
        // 어두운 터미널의 기본 배경과 시각적으로 구분이 안 돼 배지 경계가 사라진다 —
        // 최소 명도를 확보한 진회색으로 낮춰 배경과의 식별성을 유지한다.
        "KT" => Color::Rgb(140, 140, 140),
        "SK" => Color::Rgb(206, 15, 105), // SSG
        "NC" => Color::Rgb(49, 91, 138),
        "HT" => Color::Rgb(234, 0, 44),  // KIA
        "LT" => Color::Rgb(4, 30, 66),   // 롯데
        "SS" => Color::Rgb(0, 100, 176), // 삼성
        "HH" => Color::Rgb(255, 102, 0), // 한화
        "WO" => Color::Rgb(87, 12, 24),  // 키움
        "OB" => Color::Rgb(19, 24, 84),  // 두산
        _ => Color::Gray,
    }
}

/// sRGB 채널 선형화(WCAG 2.x 정의).
fn linearize(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 상대 휘도. Rgb 외 named color는 대표값으로 근사(Gray=0x80, White, Black,
/// 그 외 0.5) — 이 크레이트에서 대비 계산 대상은 사실상 Rgb 팀컬러뿐이다.
fn relative_luminance(c: Color) -> f32 {
    let (r, g, b) = match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::White => (255, 255, 255),
        Color::Black => (0, 0, 0),
        Color::Gray => (128, 128, 128),
        _ => (128, 128, 128),
    };
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG 대비율 (1.0 ~ 21.0).
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// 배경색 위에서 읽히는 글자색(흰/검)을 WCAG 대비율로 고른다.
/// RGB가 아닌 색(이름 색 등)은 흰색을 기본으로 한다.
pub fn contrast_fg(bg: Color) -> Color {
    match bg {
        Color::Rgb(..) => {
            // WCAG 대비율 기준으로 흰/검 중 더 잘 보이는 쪽(luma 128 휴리스틱 승격).
            if contrast_ratio(bg, Color::White) >= contrast_ratio(bg, Color::Black) {
                Color::White
            } else {
                Color::Black
            }
        }
        // 미등록 팀 폴백(Gray) 배지: 흰 글자는 저대비 → 검정(리뷰 Minor).
        Color::Gray => Color::Black,
        _ => Color::White,
    }
}

/// 팀명 배지 스타일: 팀 컬러 배경 + 대비 글자색 + 굵게.
/// 어두운 팀 컬러(예 두산 남색)도 배경으로 쓰면 대비 글자색 덕에 잘 보인다.
pub fn team_badge_style(code: &str) -> Style {
    let bg = team_color(code);
    Style::default()
        .bg(bg)
        .fg(contrast_fg(bg))
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codes_map_to_distinct_non_gray_colors() {
        let codes = ["LG", "KT", "SK", "NC", "HT", "LT", "SS", "HH", "WO", "OB"];
        let colors: Vec<Color> = codes.iter().map(|c| team_color(c)).collect();
        for c in &colors {
            assert_ne!(*c, Color::Gray);
        }
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(
                    colors[i], colors[j],
                    "{} and {} collide",
                    codes[i], codes[j]
                );
            }
        }
    }

    #[test]
    fn unknown_code_is_gray() {
        assert_eq!(team_color("ZZ"), Color::Gray);
    }

    #[test]
    fn contrast_fg_is_white_on_dark_and_black_on_light() {
        // OB 남색(19,24,84)은 어두우므로 흰 글자
        assert_eq!(contrast_fg(Color::Rgb(19, 24, 84)), Color::White);
        // 밝은 배경은 검은 글자
        assert_eq!(contrast_fg(Color::Rgb(240, 240, 240)), Color::Black);
        // 한화 주황(255,102,0)은 휘도가 충분히 높아 검은 글자
        assert_eq!(contrast_fg(Color::Rgb(255, 102, 0)), Color::Black);
    }

    #[test]
    fn team_badge_sets_team_bg_and_contrasting_fg() {
        let style = team_badge_style("OB"); // 어두운 남색
        assert_eq!(style.bg, Some(team_color("OB")));
        assert_eq!(style.fg, Some(Color::White));
    }

    #[test]
    fn contrast_fg_picks_black_on_gray_fallback_badge() {
        assert_eq!(contrast_fg(Color::Gray), Color::Black);
    }

    /// WCAG 정식 대비율: 흰/검 = 21:1, 동일색 = 1:1.
    #[test]
    fn contrast_ratio_matches_wcag_reference_points() {
        let w = Color::Rgb(255, 255, 255);
        let k = Color::Rgb(0, 0, 0);
        assert!((contrast_ratio(w, k) - 21.0).abs() < 0.1);
        assert!((contrast_ratio(w, w) - 1.0).abs() < 0.01);
        // sRGB 선형화 확인점: #808080 vs 검정 ≈ 5.3:1 (감마 무시 산술이면 크게 다름)
        let g = Color::Rgb(128, 128, 128);
        let r = contrast_ratio(g, k);
        assert!((5.0..5.6).contains(&r), "got {r}");
    }

    /// 완전성: 10팀 전부 배지(bg=팀컬러, fg=contrast_fg)가 WCAG AA 4.5:1 이상.
    #[test]
    fn every_team_badge_meets_wcag_aa_contrast() {
        for code in ["LG", "OB", "SK", "KT", "NC", "HT", "LT", "SS", "HH", "WO"] {
            let bg = team_color(code);
            let fg = contrast_fg(bg);
            let r = contrast_ratio(bg, fg);
            assert!(r >= 4.5, "{code}: badge contrast {r:.2} < 4.5");
        }
    }
}
