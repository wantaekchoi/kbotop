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

/// 배경색 위에서 읽히는 글자색(흰/검)을 상대 휘도로 고른다.
/// RGB가 아닌 색(이름 색 등)은 흰색을 기본으로 한다.
pub fn contrast_fg(bg: Color) -> Color {
    let (r, g, b) = match bg {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => return Color::White,
    };
    // ITU-R BT.601 luma
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    if lum > 128.0 {
        Color::Black
    } else {
        Color::White
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
}
