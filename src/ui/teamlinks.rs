//! 구단 공홈/굿즈몰 정적 링크(2026-07-23 WebSearch/WebFetch 검증).
//! 주의: OB 공홈은 인증서 체인 이슈로 브라우저 검증만(교차 검색 일치),
//! SK·NC·HH는 복수 채널 병존(대표값 채택 — HH는 스파이더 어센틱),
//! LT 굿즈몰은 롯데온 셀러샵(트래킹 파라미터 제거본). 자주 안 바뀌는
//! 값이라 하드코딩(§v0.2-13 원칙: 안 바뀌는 것=하드코딩).
const TEAM_LINKS: &[(&str, &str, &str)] = &[
    (
        "LG",
        "https://www.lgtwins.com/",
        "https://interparkmdshop.com/category/lg트윈스/31/",
    ),
    (
        "OB",
        "https://www.doosanbears.com/",
        "https://www.doosanbearswefan.shop/",
    ),
    (
        "SK",
        "https://www.ssglanders.com/",
        "https://www.ssglandersstore.co.kr/",
    ),
    (
        "KT",
        "https://www.ktwiz.co.kr/",
        "https://ktwizstore.co.kr/",
    ),
    (
        "NC",
        "https://www.ncdinos.com/",
        "https://store.ncdinos.com/",
    ),
    (
        "HT",
        "https://www.tigers.co.kr/",
        "https://teamstore.tigers.co.kr/",
    ),
    (
        "LT",
        "https://www.giantsclub.com/",
        "https://www.lotteon.com/p/display/seller/sellerShop/lottegiants",
    ),
    (
        "SS",
        "https://www.samsunglions.com/",
        "https://samsunglionsmall.com/",
    ),
    (
        "HH",
        "https://www.hanwhaeagles.co.kr/",
        "https://spyder.co.kr/eagles_index.html",
    ),
    (
        "WO",
        "https://heroesbaseball.co.kr/",
        "https://nolmdshop.com/category/키움히어로즈/29/",
    ),
];

pub fn links_for(code: &str) -> Option<(&'static str, &'static str)> {
    TEAM_LINKS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, o, g)| (*o, *g))
}

/// 비ASCII 바이트만 %XX로 인코딩(이미 인코딩된 %·예약문자는 보존) — IRI를
/// open(1)/xdg-open에 안전한 ASCII URL로.
pub fn encode_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for b in url.bytes() {
        if b.is_ascii() {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 브라우저 열기 — 실패는 조용히 무시(관용: TUI가 죽을 일이 아니다).
pub fn open_url(url: &str) {
    let enc = encode_url(url);
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&enc).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(&enc).spawn();
    // `cmd /c start "" <url>` 표준 방식. 첫 빈 인자 `""`는 start의 창 제목
    // 자리라 생략할 수 없다(생략하면 URL이 제목으로 오인돼, URL에 따옴표가
    // 필요한 문자가 있으면 깨진다). 각 인자를 std::process::Command에 개별
    // 전달해 cmd가 셸 메타문자(`&` 등, encode_url이 건드리지 않는 ASCII
    // 예약문자)로 URL을 잘라먹지 않게 한다 — 셸 문자열로 합쳐서 넘기면 `&`가
    // 명령 구분자로 해석되어 뒷부분이 별도 명령으로 실행된다.
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", &enc])
        .spawn();
    #[cfg(not(any(unix, windows)))]
    let _ = &enc; // 미지원 플랫폼: no-op
}

/// 현재 화면 컨텍스트의 링크 항목(라벨, URL). 순수 — 테스트 대상.
pub fn link_items_for_screen(app: &crate::app::App) -> Vec<(String, String)> {
    use crate::app::{Screen, Tab};
    let team_pair = |code: &str, name: &str| -> Vec<(String, String)> {
        match links_for(code) {
            Some((official, goods)) => vec![
                (format!("{name} official site"), official.to_string()),
                (format!("{name} goods shop"), goods.to_string()),
            ],
            None => vec![],
        }
    };
    match &app.screen {
        Screen::Live { game, .. } => {
            let mut v = team_pair(&game.away.code, &game.away.name);
            v.extend(team_pair(&game.home.code, &game.home.name));
            v
        }
        Screen::List => match app.tab {
            Tab::Games => app
                .games
                .get(app.selected)
                .map(|g| {
                    let mut v = team_pair(&g.away.code, &g.away.name);
                    v.extend(team_pair(&g.home.code, &g.home.name));
                    v
                })
                .unwrap_or_default(),
            Tab::Standings => app
                .standings
                .get(app.selected)
                .map(|s| team_pair(&s.team.code, &s.team.name))
                .unwrap_or_default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 완전성: 10팀 전부 공홈·굿즈몰 URL이 비어있지 않은 https다.
    #[test]
    fn every_team_has_https_official_and_goods_urls() {
        for code in ["LG", "OB", "SK", "KT", "NC", "HT", "LT", "SS", "HH", "WO"] {
            let (official, goods) = links_for(code).unwrap_or_else(|| panic!("{code} missing"));
            for url in [official, goods] {
                assert!(url.starts_with("https://"), "{code}: non-https {url}");
            }
        }
        assert!(links_for("XX").is_none());
    }

    /// 한글 경로 URL(LG·WO 굿즈몰)은 percent-encoding 후 ASCII만 남는다 —
    /// open(1)/xdg-open에 안전하게 넘기기 위함.
    #[test]
    fn encoded_urls_are_pure_ascii() {
        for code in ["LG", "OB", "SK", "KT", "NC", "HT", "LT", "SS", "HH", "WO"] {
            let (official, goods) = links_for(code).unwrap();
            for url in [official, goods] {
                let enc = encode_url(url);
                assert!(enc.is_ascii(), "{code}: {enc}");
                assert!(enc.starts_with("https://"));
            }
        }
        // 인코딩 자체 검증: '한'(U+D55C, UTF-8 ED 95 9C)
        assert_eq!(encode_url("https://x.kr/한"), "https://x.kr/%ED%95%9C");
    }
    /// 화면 컨텍스트마다 어느 팀의 링크가 나오는지 — v0.21까지 이 순수 함수가
    /// 통째로 안 덮여 있었다(파일 86.15%). 화면별 분기가 어긋나도 걸리는 게 없었다.
    #[test]
    fn link_items_follow_the_screen_context() {
        use crate::app::{Screen, Tab};
        let mut app = crate::app::App::new(Default::default());

        // 라이브: 원정 → 홈 순으로 두 팀 모두.
        app.screen = Screen::Live {
            game: sample_game("LG", "LG", "KT", "KT"),
            state: None,
        };
        let live_items = link_items_for_screen(&app);
        assert_eq!(live_items.len(), 4, "두 팀 × (공홈, 굿즈)");
        assert!(live_items[0].0.starts_with("KT"), "원정 팀이 먼저다");
        assert!(live_items[2].0.starts_with("LG"));

        // 목록/경기 탭: 커서가 가리키는 경기의 두 팀.
        app.screen = Screen::List;
        app.tab = Tab::Games;
        app.games = vec![
            sample_game("HH", "한화", "OB", "두산"),
            sample_game("SS", "삼성", "NC", "NC"),
        ];
        app.selected = 1;
        let game_items = link_items_for_screen(&app);
        assert_eq!(game_items.len(), 4);
        assert!(game_items[0].0.starts_with("NC"));

        // 목록/순위 탭: 커서 팀 하나만.
        app.tab = Tab::Standings;
        app.standings = vec![sample_standing("WO", "키움")];
        app.selected = 0;
        let standing_items = link_items_for_screen(&app);
        assert_eq!(standing_items.len(), 2, "한 팀 × (공홈, 굿즈)");
        assert!(standing_items[0].0.starts_with("키움"));
    }

    /// 커서가 범위를 벗어났거나 목록이 비면 빈 목록이다 — 링크 픽커가 열려도
    /// 아무것도 없을 뿐, 패닉하지 않는다(무패닉 원칙).
    #[test]
    fn an_out_of_range_cursor_yields_no_links_instead_of_panicking() {
        use crate::app::{Screen, Tab};
        let mut app = crate::app::App::new(Default::default());
        app.screen = Screen::List;
        app.tab = Tab::Games;
        app.selected = 7; // 빈 목록에서 범위 밖
        assert!(link_items_for_screen(&app).is_empty());

        app.tab = Tab::Standings;
        assert!(link_items_for_screen(&app).is_empty());
    }

    /// 링크가 없는 팀 코드(미지의 구단)는 그 팀만 조용히 빠진다.
    #[test]
    fn a_team_without_links_is_skipped_silently() {
        use crate::app::Screen;
        let mut app = crate::app::App::new(Default::default());
        app.screen = Screen::Live {
            game: sample_game("LG", "LG", "ZZ", "미지의구단"),
            state: None,
        };
        let items = link_items_for_screen(&app);
        assert_eq!(items.len(), 2, "링크가 있는 팀 것만 남는다");
        assert!(items.iter().all(|(label, _)| label.starts_with("LG")));
    }

    fn sample_game(
        home_code: &str,
        home_name: &str,
        away_code: &str,
        away_name: &str,
    ) -> crate::model::Game {
        crate::model::Game {
            id: "g".into(),
            start: String::new(),
            status: crate::model::GameStatus::Live,
            status_label: String::new(),
            home: crate::model::Team {
                code: home_code.into(),
                name: home_name.into(),
            },
            away: crate::model::Team {
                code: away_code.into(),
                name: away_name.into(),
            },
            home_score: None,
            away_score: None,
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
        }
    }

    fn sample_standing(code: &str, name: &str) -> crate::model::Standing {
        crate::model::Standing {
            rank: 1,
            team: crate::model::Team {
                code: code.into(),
                name: name.into(),
            },
            games: 100,
            wins: 50,
            losses: 45,
            draws: 5,
            win_rate: 0.526,
            game_behind: 0.0,
            last_five: String::new(),
            streak: String::new(),
        }
    }
}
