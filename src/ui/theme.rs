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
        // DarkGray는 ANSI "bright black"(xterm 8번, #555555)이다. 이걸 몰라서
        // Gray와 같은 값으로 계산하는 바람에, 대비를 재 봐도 실제보다 밝게
        // 나왔다 — 취소 태그가 검은 배경에서 안 읽히는 걸 못 잡은 이유다.
        Color::DarkGray => (85, 85, 85),
        // 나머지 명명색은 터미널 팔레트가 정하므로 우리가 알 수 없다. 중간
        // 회색으로 가정한다(어느 배경에서도 최악은 아닌 값).
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
        // named_color가 accent_for로 내는 6개 명명색(T9 하이라이트 배경) — 표준
        // 팔레트 관례: 밝은 배경(Yellow/Cyan/Green)은 검정 글자, 어두운 배경
        // (Red/Blue/Magenta)은 흰 글자. 이전엔 전부 White 폴백이라 밝은 배경에서
        // 거의 안 읽혔다(리뷰 Important).
        Color::Yellow | Color::Cyan | Color::Green => Color::Black,
        Color::Red | Color::Blue | Color::Magenta => Color::White,
        // 방어적 폴백: White 배경엔 Black, Black 배경엔 White. accent 소스는 이
        // 두 색을 내지 않지만(named_color 참고) 대비 원칙을 지킨다.
        Color::White => Color::Black,
        Color::Black => Color::White,
        _ => Color::White,
    }
}

/// 팀명 배지 스타일: 팀 컬러 배경 + 대비 글자색 + 굵게.
/// 어두운 팀 컬러(예 두산 남색)도 배경으로 쓰면 대비 글자색 덕에 잘 보인다.
pub fn team_badge_style(preset: &str, code: &str) -> Style {
    // mono는 색을 안 쓴다. 배지는 팀 컬러가 데이터라 오래 예외로 뒀지만,
    // README 두 판이 "mono는 색을 아예 쓰지 않아 흑백 터미널에서도 읽힙니다"라고
    // 약속한다 — 공개 약속 쪽을 따른다. 반전만으로도 "여기가 배지"라는 건 남고,
    // 팀 이름은 어차피 글자로 적혀 있다(색이 유일한 단서였던 적이 없다).
    if preset == "mono" {
        return Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED);
    }
    let bg = team_color(code);
    Style::default()
        .bg(bg)
        .fg(contrast_fg(bg))
        .add_modifier(Modifier::BOLD)
}

/// 액센트 색을 preset·accent 소스·응원팀으로 결정한다. mono 프리셋은 항상 None
/// (색 없음). accent=team이면 팀 컬러(배지 아닌 chrome 강조는 명명색 계열로만
/// 쓰이나, team은 유일하게 RGB — 테두리 등 전경에 쓰면 배경무관 위반이므로
/// 호출부는 team 액센트를 배지/반전 맥락에서만 쓴다), named면 매핑, none이면 None.
///
/// T9가 games/standings의 선택 하이라이트(fav_code 기반 team_color 배경)를
/// 이 함수로 대체해 배선했다 — None이면 호출부가 REVERSED로 대체한다.
pub fn accent_for(preset: &str, accent: &str, fav: Option<&str>) -> Option<Color> {
    if preset == "mono" {
        return None;
    }
    match accent {
        "team" => fav.map(team_color),
        "none" => None,
        name => named_color(name),
    }
}

/// 배경 양쪽에서 읽히는 명명색만 허용한다(black/white는 한쪽에서 사라짐).
/// 명명색이 아니면 16진 색(v0.22)으로 한 번 더 해석해 본다.
fn named_color(name: &str) -> Option<Color> {
    match name {
        "cyan" => Some(Color::Cyan),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "magenta" => Some(Color::Magenta),
        "blue" => Some(Color::Blue),
        "red" => Some(Color::Red),
        other => hex_color(other),
    }
}

/// `#rrggbb` / `rrggbb` / `#rgb` → Color::Rgb (v0.22 — config에서 액센트 색을
/// 직접 고른다). 사람이 손으로 적는 값이라 `#` 생략과 3자리 축약까지 받는다.
///
/// 형식이 틀리면 `None`이고 호출부는 "색 없음"으로 조용히 저하한다 — 설정
/// 파일의 오타가 앱을 죽이거나 에러 화면을 띄우면 안 된다(config 폴백 원칙).
///
/// 대비를 여기서 따로 보지 않는 이유: 액센트는 **배경으로만** 쓰이고 그 위 글자색은
/// [`contrast_fg`]가 고른다. 팀 컬러가 임의 RGB인데도 배지가 읽히는 것과 같은
/// 구조라, 밝기와 무관하게 읽힘이 보장된다.
fn hex_color(s: &str) -> Option<Color> {
    let body = s.strip_prefix('#').unwrap_or(s);
    // **이 검사는 바이트 경계를 지킨다 — 지우면 패닉한다.** 아래 슬라이싱은 바이트
    // 단위인데, `len()`도 바이트 길이라 정확히 6바이트인 멀티바이트 문자열(한글 두
    // 글자 "가나" 등)이 6자리 분기로 들어와 문자 중간을 자른다. 실제로 한 번 뺐다가
    // `accent = "가나"`에서 패닉하는 걸 확인하고 되살렸다 — 뮤테이션이 이 줄을
    // "중복"으로 표시했던 건 그때 테스트가 ASCII 입력만 보고 있었기 때문이다.
    // (검출되지 않는 코드가 곧 불필요한 코드는 아니다.)
    if !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |i: usize| u8::from_str_radix(&body[i..i + 2], 16).ok();
    match body.len() {
        6 => Some(Color::Rgb(pair(0)?, pair(2)?, pair(4)?)),
        // 3자리 축약: 각 자리를 두 번 반복(#f60 = #ff6600), CSS와 같은 관례.
        3 => {
            let nib = |i: usize| u8::from_str_radix(&body[i..i + 1], 16).ok().map(|v| v * 17);
            Some(Color::Rgb(nib(0)?, nib(1)?, nib(2)?))
        }
        _ => None,
    }
}

/// header의 LIVE/SCHED/FINAL/OTHER 카운트·스피너·stale 마커처럼 fav/accent
/// 소스가 아니라 항상 고정색을 쓰던 chrome 지점의 게이트. accent_for와 달리
/// 이 지점들은 액센트 소스와 무관하게 원래 정해진 named color를 그대로
/// 유지하되(v0.5의 "배경 무관 가독" 결정), mono 프리셋에서만 그 색을
/// 걷어낸다 — "mono는 색 span 0"을 accent 지점 밖의 chrome까지 확장한다.
/// 팀 배지(team_badge_style)는 데이터라 이 함수를 거치지 않는다(예외 유지).
pub fn status_fg(preset: &str, color: Color) -> Style {
    match preset {
        "mono" => Style::default(),
        HIGH_CONTRAST => Style::default().fg(boost(color)),
        _ => Style::default().fg(color),
    }
}

/// 고대비 프리셋 이름. 설정 화면·config에 쓰이는 값과 같은 문자열이다.
pub const HIGH_CONTRAST: &str = "high-contrast";

/// 고대비에서 **흐린 회색을 터미널 기본 전경색으로 올린다.**
///
/// v0.27까지 `high-contrast`는 이름만 있고 아무 일도 하지 않았다 — 렌더 어디에도
/// 분기가 없어 `default`와 한 셀도 다르지 않았는데, 설정 화면에는 "고대비"로
/// 떠 있었다. 저시력 사용자에게 거짓 옵션을 판 셈이다.
///
/// 왜 회색이냐면, 안 읽히는 게 거기부터이기 때문이다. `DarkGray`(#555)는 검은
/// 배경에서 대비가 **2.82:1**로 WCAG의 비텍스트 최소치(3:1)에도 못 미치고,
/// `Gray`(#808080)는 흰 배경에서 3.95:1로 AA(4.5:1)에 못 미친다.
///
/// 올리는 대상이 `White`가 아니라 `Reset`인 이유: 이 프로젝트는 액센트 명명색에서
/// black/white를 배제했다(밝은 배경에서 흰 글자는 안 보인다). `Reset`은 터미널이
/// 자기 배경에 맞춰 고른 전경색이라 **어느 배경에서든 최대 대비**다.
///
/// 상태를 나타내는 색(빨강=진행 중, 초록=종료 등)은 건드리지 않는다 — 그건
/// 정보이고, 고대비가 정보를 지워서는 안 된다.
fn boost(color: Color) -> Color {
    match color {
        Color::Gray | Color::DarkGray => Color::Reset,
        other => other,
    }
}

/// 흐리게 그릴 때 쓰는 수식자. **고대비에서는 흐리게 하지 않는다.**
///
/// `Modifier::DIM`은 터미널이 전경색을 어둡게 낮추는 것이라, 저시력 사용자에게는
/// 회색과 같은 문제를 일으킨다. 여섯 군데가 이 수식자를 직접 쓰고 있었다.
pub fn dim(preset: &str) -> Modifier {
    if preset == HIGH_CONTRAST {
        Modifier::empty()
    } else {
        Modifier::DIM
    }
}

/// status_fg와 같은 정책을 Style이 아니라 원시 Color로 내야 하는 지점의 게이트.
/// ratatui Canvas Shape(Rectangle/Line 등)의 `color` 필드는 Style을 받지 않고
/// Color를 직접 요구해 status_fg를 못 쓴다 — strikezone의 투구 마커, sideview의
/// 궤적선이 여기 해당한다(리뷰 Important: 이전엔 게이트가 없어 mono에서도
/// Green/Red/Yellow/Cyan 투구색이 그대로 남았다).
/// mono면 Color::Reset(색 지정 없음 → 터미널 기본 전경색 상속)을 낸다.
/// v0.8 재리뷰 Important: 이전엔 White를 냈는데, White는 배경 무관이 아니다
/// (named_color 주석대로 밝은 배경에서 흰 마커는 안 보인다) — 이 프로젝트가
/// accent 명명색에서 black/white를 배제한 것과 같은 이유로 재발.
/// "Reset은 Canvas가 '그리지 않음'으로 특별 취급해 마커가 사라진다"는 초기
/// 구현 주장은 ratatui 0.29 실측으로 반증됐다: 이 파일이 쓰는 기본 마커는
/// Marker::Braille이고, BrailleGrid::paint는 점 비트를 색과 무관하게 세우며,
/// Canvas::render_ref(widgets/canvas.rs)는 문자가 blank braille(U+2800)가
/// 아니면 색과 무관하게 무조건 set_char 한다 — fg만 Reset이면 미설정으로
/// 남을 뿐 글리프는 그대로 그려진다. Reset이 "안 그림"으로 합쳐지는 건
/// HalfBlockGrid 마커(Marker::HalfBlock) 한정이며 이 크레이트는 쓰지 않는다.
/// 아래 status_color_reset_still_draws_a_braille_marker_on_canvas가 이를
/// 코드로 못박는다.
pub fn status_color(preset: &str, color: Color) -> Color {
    match preset {
        "mono" => Color::Reset,
        HIGH_CONTRAST => boost(color),
        _ => color,
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
        let style = team_badge_style("default", "OB"); // 어두운 남색
        assert_eq!(style.bg, Some(team_color("OB")));
        assert_eq!(style.fg, Some(Color::White));
    }

    #[test]
    fn contrast_fg_picks_black_on_gray_fallback_badge() {
        assert_eq!(contrast_fg(Color::Gray), Color::Black);
    }

    /// 명명색 액센트 배경(accent_for가 낼 수 있는 6색)의 대비 글자색 — 리뷰 Important:
    /// 이전엔 전부 White 폴백이라 밝은 배경(Yellow/Cyan/Green)에서 거의 안 읽혔다.
    /// 밝은 배경 → 검정 글자, 어두운 배경 → 흰 글자(표준 팔레트 관례).
    #[test]
    fn contrast_fg_reads_on_named_accent_backgrounds() {
        for bright in [Color::Yellow, Color::Cyan, Color::Green] {
            assert_eq!(
                contrast_fg(bright),
                Color::Black,
                "{bright:?} bg should get black fg"
            );
        }
        for dark in [Color::Red, Color::Blue, Color::Magenta] {
            assert_eq!(
                contrast_fg(dark),
                Color::White,
                "{dark:?} bg should get white fg"
            );
        }
    }

    /// 방어적 폴백: White/Black 자체가 배경으로 오는 경우도 대비 원칙을 지킨다.
    #[test]
    fn contrast_fg_handles_white_and_black_backgrounds_defensively() {
        assert_eq!(contrast_fg(Color::White), Color::Black);
        assert_eq!(contrast_fg(Color::Black), Color::White);
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

    /// mono 프리셋은 어떤 chrome도 색을 쓰지 않는다(색맹·흑백 터미널 대응).
    #[test]
    fn mono_preset_uses_no_color() {
        assert_eq!(accent_for("mono", "team", Some("LG")), None);
        assert_eq!(accent_for("mono", "cyan", None), None);
    }

    /// accent=team이면 팀 컬러, named면 그 명명색, none이면 없음.
    #[test]
    fn accent_source_resolves() {
        assert_eq!(
            accent_for("default", "team", Some("LG")),
            Some(team_color("LG"))
        );
        assert_eq!(
            accent_for("default", "cyan", None),
            Some(ratatui::style::Color::Cyan)
        );
        assert_eq!(accent_for("default", "none", Some("LG")), None);
        // 알 수 없는 명명색은 관용적으로 default 취급(액센트 없음이 아니라 team 폴백은 X — 안전하게 None)
        assert_eq!(accent_for("default", "unknownxyz", None), None);
    }

    /// mono가 아니면 지정색을 그대로 fg로 낸다(기존 header 고정색 외양 불변).
    #[test]
    fn status_fg_keeps_color_outside_mono() {
        let style = status_fg("default", Color::Red);
        assert_eq!(style.fg, Some(Color::Red));
        let style = status_fg("high-contrast", Color::Cyan);
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    /// mono면 색을 걷어내(fg=None → 렌더 시 Reset) chrome 고정색도 사라진다.
    #[test]
    fn status_fg_strips_color_in_mono() {
        let style = status_fg("mono", Color::Red);
        assert_eq!(style.fg, None);
    }

    /// status_color는 Style 대신 원시 Color가 필요한 Canvas Shape 지점의 게이트다.
    /// mono가 아니면 지정색을 그대로 낸다(기존 마커 외양 불변).
    #[test]
    fn status_color_keeps_color_outside_mono() {
        assert_eq!(status_color("default", Color::Red), Color::Red);
        assert_eq!(status_color("high-contrast", Color::Cyan), Color::Cyan);
    }

    /// mono면 Color::Reset(배경 무관: 터미널 기본 전경색 상속)으로 걷어낸다.
    /// v0.8 재리뷰: White는 밝은 배경에서 안 보여 배경 무관 위반이었다.
    #[test]
    fn status_color_goes_reset_in_mono() {
        assert_eq!(status_color("mono", Color::Red), Color::Reset);
        assert_eq!(status_color("mono", Color::Green), Color::Reset);
        assert_eq!(status_color("mono", Color::Cyan), Color::Reset);
    }

    /// 결정적 검증(v0.8 재리뷰): status_color가 mono에서 내는 Color::Reset이
    /// ratatui Canvas 위에서 마커 글리프 자체를 지우는지 실측으로 못박는다.
    /// 구현자는 "Canvas가 Reset을 그리지 않음으로 취급해 마커가 사라진다"고
    /// 주장했으나, 재리뷰어는 ratatui 0.29로 직접 렌더해 이게 사실이 아니라고
    /// 판정했다 — 이 테스트가 그 판정을 코드로 고정한다. 이 크레이트의
    /// strikezone/sideview는 Canvas::default()(Marker::Braille)만 쓰므로
    /// 여기서도 동일 마커로 재현한다(HalfBlock 마커였다면 결과가 달랐을 것).
    #[test]
    fn status_color_reset_still_draws_a_braille_marker_on_canvas() {
        use ratatui::{
            backend::TestBackend,
            widgets::canvas::{Canvas, Rectangle},
            Terminal,
        };

        let color = status_color("mono", Color::Red); // fix 후: Color::Reset
        assert_eq!(color, Color::Reset, "precondition: mono yields Reset");

        let mut term = Terminal::new(TestBackend::new(10, 10)).unwrap();
        term.draw(|f| {
            let canvas = Canvas::default()
                .x_bounds([-1.0, 1.0])
                .y_bounds([-1.0, 1.0])
                .paint(move |ctx| {
                    ctx.draw(&Rectangle {
                        x: -0.3,
                        y: -0.3,
                        width: 0.6,
                        height: 0.6,
                        color,
                    });
                });
            f.render_widget(canvas, f.area());
        })
        .unwrap();

        let buf = term.backend().buffer().clone();
        // blank braille(U+2800)도 공백과 동일한 "빈 셀" 취급이므로 함께 배제한다.
        let has_marker_glyph = buf
            .content()
            .iter()
            .any(|c| c.symbol() != " " && c.symbol() != "\u{2800}");
        assert!(
            has_marker_glyph,
            "Color::Reset marker must still render a glyph on the canvas \
             (fg falls back to the terminal default, the shape isn't skipped): {:?}",
            buf.content().iter().map(|c| c.symbol()).collect::<String>()
        );
    }
    /// v0.22: config에서 액센트 색을 16진으로 직접 고른다. 세 형식 모두 받는다.
    #[test]
    fn accent_accepts_hex_colors_in_three_forms() {
        assert_eq!(
            accent_for("default", "#ff6600", None),
            Some(Color::Rgb(255, 102, 0))
        );
        assert_eq!(
            accent_for("default", "ff6600", None),
            Some(Color::Rgb(255, 102, 0)),
            "# 없는 형식도 받는다"
        );
        assert_eq!(
            accent_for("default", "#f60", None),
            Some(Color::Rgb(255, 102, 0)),
            "3자리 축약은 각 자리를 두 번 반복한다(CSS 관례)"
        );
    }

    /// 형식이 틀리면 색 없음으로 조용히 저하한다 — 설정 오타가 앱을 죽이면 안 된다.
    #[test]
    fn a_malformed_accent_value_degrades_to_no_color() {
        for bad in ["#ff66", "#gggggg", "#1234567", "", "#", "orange", "12345"] {
            assert_eq!(
                accent_for("default", bad, None),
                None,
                "{bad:?} should not resolve to a color"
            );
        }
    }

    /// mono는 색을 아예 쓰지 않는다 — 이 규칙이 hex보다 위다.
    #[test]
    fn mono_preset_still_wins_over_a_hex_accent() {
        assert_eq!(accent_for("mono", "#ff6600", None), None);
    }

    /// 기존 값(team·none·명명색)의 동작은 그대로다.
    #[test]
    fn hex_support_does_not_change_the_existing_accent_values() {
        assert_eq!(
            accent_for("default", "team", Some("LG")),
            Some(team_color("LG"))
        );
        assert_eq!(accent_for("default", "none", Some("LG")), None);
        assert_eq!(accent_for("default", "cyan", None), Some(Color::Cyan));
    }

    /// 액센트는 배경으로 쓰이고 글자색은 contrast_fg가 고른다 — 어떤 hex를 넣어도
    /// 그 위 글자가 WCAG AA(4.5:1)를 넘는다. 대비 로직을 새로 만들지 않은 근거다.
    #[test]
    fn any_hex_accent_gets_a_readable_foreground() {
        for hex in [
            "#000000", "#ffffff", "#ff6600", "#123456", "#7f7f7f", "#00ff00",
        ] {
            let bg = accent_for("default", hex, None).expect("valid hex");
            let fg = contrast_fg(bg);
            let r = contrast_ratio(bg, fg);
            assert!(r >= 4.5, "{hex}: contrast {r:.2} < 4.5");
        }
    }
    /// 슬라이싱이 바이트 단위라 비ASCII 입력에서 패닉하지 않는지 — 무패닉 원칙.
    ///
    /// **"가나"가 핵심 케이스다**: 한글 두 글자가 정확히 6바이트라 6자리 hex 분기로
    /// 들어가고, `body[0..2]`가 '가'의 중간을 잘라 패닉한다(실측). config 파일에
    /// 아무 글자나 적는 건 사용자가 언제든 하는 일이므로 이건 실사용 경로다.
    #[test]
    fn a_multibyte_accent_value_does_not_panic() {
        for weird in [
            "#가나다",
            "#🎨🎨",
            "가나다라마바",
            "#ﬀﬀﬀ",
            "#가나",
            "가나",
            "#aa가",
        ] {
            assert_eq!(accent_for("default", weird, None), None, "{weird:?}");
        }
    }
    /// **chrome 고정색도 어느 배경에서든 읽혀야 한다.**
    ///
    /// 팀 배지는 `every_team_badge_meets_wcag_aa_contrast`가 보는데, 상태 태그·
    /// 시계 같은 chrome 고정색은 아무도 안 봤다. 취소 태그가 `DarkGray`(#555)라
    /// 검은 배경에서 2.82:1이었다 — 비텍스트 최소치 3:1에도 못 미친다.
    #[test]
    fn chrome_grays_stay_legible_on_both_backgrounds() {
        // chrome이 쓰는 무채색은 양쪽 배경 모두에서 비텍스트 최소치를 넘어야 한다.
        let on_dark = contrast_ratio(Color::Gray, Color::Black);
        let on_light = contrast_ratio(Color::Gray, Color::White);
        assert!(
            on_dark >= 3.0,
            "Gray가 어두운 배경에서 {on_dark:.2}:1 — 3:1 미만"
        );
        assert!(
            on_light >= 3.0,
            "Gray가 밝은 배경에서 {on_light:.2}:1 — 3:1 미만"
        );
        // DarkGray는 이 기준을 못 넘는다 — 그래서 chrome에서 뺐다.
        assert!(
            contrast_ratio(Color::DarkGray, Color::Black) < 3.0,
            "DarkGray가 기준을 넘게 됐다면 위 판단을 다시 봐야 한다"
        );
    }
}
