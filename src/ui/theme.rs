use ratatui::style::Color;

/// KBO 10구단 팀 컬러(터미널 근사).
pub fn team_color(code: &str) -> Color {
    match code {
        "LG" => Color::Rgb(196, 0, 53),
        // KT wiz 공식 색은 순수 검정이지만, team_color는 항상 foreground로만
        // 쓰이고 배경을 짝지어 설정하는 곳이 없다(games.rs/standings.rs/live.rs
        // 모두 마찬가지) — 순수 검정이면 어두운/검정 기본 배경 터미널에서 팀명이
        // 안 보인다. 최소 명도를 확보한 진회색으로 낮춰 식별성을 유지한다.
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
}
