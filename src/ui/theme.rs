use ratatui::style::{Color, Modifier, Style};

/// KBO 10구단 팀 컬러(터미널 근사).
pub fn team_color(code: &str) -> Color {
    match code {
        "LG" => Color::Rgb(196, 0, 53),
        // KT wiz 공식 색은 순수 검정. games.rs/live.rs는 team_badge_style로 이 색을
        // 배경에 얹고 contrast_fg로 대비 글자색을 골라 순수 검정도 배지로는 괜찮지만,
        // standings.rs는 아직 team_color를 foreground로만 쓴다 — 순수 검정이면 어두운
        // 배경 터미널에서 안 보이므로, 최소 명도를 확보한 진회색으로 낮춰 식별성을 유지한다.
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

/// 팀컬러를 액센트(어두운 터미널의 테두리·포인트)로 쓸 수 있게 밝힌 파생색.
/// 검정 대비 3:1(WCAG 비텍스트 최소)에 도달할 때까지 흰색 쪽으로 선형 보간 —
/// 채널 비율(색감)을 유지한 채 명도만 올린다. 이미 충분히 밝으면 원색 그대로.
pub fn accent_on_dark(code: &str) -> Color {
    let base = team_color(code);
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let mut t = 0.0f32;
    loop {
        let mix = |c: u8| -> u8 { (c as f32 + (255.0 - c as f32) * t).round() as u8 };
        let cand = Color::Rgb(mix(r), mix(g), mix(b));
        if contrast_ratio(cand, Color::Rgb(0, 0, 0)) >= 3.0 || t >= 1.0 {
            return cand;
        }
        t += 0.05;
    }
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

/// 응원 팀 액센트(어두운 배경 가시성 보정 포함). None = 테마 미적용(현행 색).
pub fn accent(fav: Option<&str>) -> Option<Color> {
    fav.map(accent_on_dark)
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

    #[test]
    fn accent_derives_from_fav_and_none_without() {
        assert!(accent(None).is_none());
        let a = accent(Some("WO")).unwrap();
        assert!(
            contrast_ratio(a, Color::Rgb(0, 0, 0)) >= 3.0,
            "accent must be visible on dark"
        );
    }

    /// 완전성: 10팀 전부 액센트 파생색이 검정 배경 대비 3:1 이상(어두운 터미널 가정).
    /// 어두운 팀컬러(두산 남색·키움 자주 등)가 테두리/포인트로 안 보이던 문제 해소.
    #[test]
    fn every_team_accent_on_dark_is_visible() {
        for code in ["LG", "OB", "SK", "KT", "NC", "HT", "LT", "SS", "HH", "WO"] {
            let a = accent_on_dark(code);
            let r = contrast_ratio(a, Color::Rgb(0, 0, 0));
            assert!(r >= 3.0, "{code}: accent contrast {r:.2} < 3.0");
            // 색상(hue) 보존 러프 검증: 원색의 최대 채널이 파생색에서도 최대(또는 공동 최대).
            if let (Color::Rgb(r0, g0, b0), Color::Rgb(r1, g1, b1)) = (team_color(code), a) {
                let max0 = r0.max(g0).max(b0);
                let max1 = r1.max(g1).max(b1);
                assert!(
                    (r0 == max0) == (r1 == max1)
                        || (g0 == max0) == (g1 == max1)
                        || (b0 == max0) == (b1 == max1),
                    "{code}: dominant channel changed"
                );
            }
        }
    }
}
