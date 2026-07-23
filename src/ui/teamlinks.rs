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
    #[cfg(not(unix))]
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
}
