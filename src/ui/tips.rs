use std::sync::OnceLock;

use crate::ui::i18n::Lang;

/// 야구 초보용 한 줄 팁 원본 — `data/tips.txt`에 한 줄당 하나(사람이 GitHub에서
/// 바로 읽고 PR로 추가할 수 있는 형태). 규칙은 사실 기반 자체 표현(저작권 무관),
/// 본문은 한국어 콘텐츠로 Paragraph에 렌더되므로 폭 안전 — 영어 chrome 하드
/// 제약은 라벨("Tip:")에만 적용된다. '#' 줄과 빈 줄은 무시한다.
/// (v0.3 후보: 릴리스 없이 갱신되도록 GitHub raw에서 런타임 fetch.)
const TIPS_RAW: &str = include_str!("../../data/tips.txt");
/// 영어·일본어 팁(v0.21). 한국어판보다 짧다 — 팁은 초보자용 설명이라 개수보다
/// 정확도가 중요하고, 한 줄 늘릴 때마다 세 언어를 함께 손봐야 하기 때문이다.
/// 회전만 도는 데는 이 정도로 충분하다.
const TIPS_EN: &str = include_str!("../../data/tips.en.txt");
const TIPS_JA: &str = include_str!("../../data/tips.ja.txt");

fn parse_embedded(raw: &'static str) -> Vec<&'static str> {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// 파싱된 팁 목록(언어별, 최초 1회 lazy). 파일이 주석/공백뿐이어도 패닉하지 않도록
/// current()가 빈 목록을 방어한다.
fn tips(lang: Lang) -> &'static [&'static str] {
    static KO: OnceLock<Vec<&'static str>> = OnceLock::new();
    static EN: OnceLock<Vec<&'static str>> = OnceLock::new();
    static JA: OnceLock<Vec<&'static str>> = OnceLock::new();
    match lang {
        Lang::Ko => KO.get_or_init(|| parse_embedded(TIPS_RAW)),
        Lang::En => EN.get_or_init(|| parse_embedded(TIPS_EN)),
        Lang::Ja => JA.get_or_init(|| parse_embedded(TIPS_JA)),
    }
}

/// 현재 분(now_secs/60)에 해당하는 팁 — 1분마다 회전, 의존성 없이 결정적.
/// 그 언어의 팁이 비어 있으면 빈 문자열이고, 호출부(ui/mod.rs)가 팁 줄을 통째로
/// 감춘다 — 틀린 언어로 보여주느니 안 보여주는 편이 낫다(v0.21).
pub fn current(lang: Lang, now_secs: u64) -> &'static str {
    let t = tips(lang);
    if t.is_empty() {
        return "";
    }
    t[((now_secs / 60) as usize) % t.len()]
}

/// 원격 tips.txt 파싱: 주석/빈 줄 제거 후 유효 줄이 10개 이상일 때만 채택 —
/// 깨진/부분 응답이 임베드 팁을 대체하지 못하게 하는 관용 가드.
pub fn parse_remote(raw: &str) -> Option<Vec<String>> {
    let lines: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    (lines.len() >= 10).then_some(lines)
}

/// 런타임 override 우선, 없으면 그 언어의 임베드본으로 회전.
///
/// 원격 갱신(override)은 한국어 파일 하나만 받아 오므로 **한국어일 때만** 쓴다.
/// 다른 언어에서 그걸 쓰면 지금 고치려는 결함(영어 화면의 한국어 팁)이 원격
/// 경로로 되살아난다. 언어별 원격 URL을 늘리지 않는 이유는 갱신 실패 경로가
/// 언어 수만큼 늘기 때문이다.
pub fn pick(override_: &Option<Vec<String>>, lang: Lang, now_secs: u64) -> &str {
    match override_ {
        Some(list) if lang == Lang::Ko && !list.is_empty() => {
            &list[((now_secs / 60) as usize) % list.len()]
        }
        _ => current(lang, now_secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Lang; 3] = [Lang::Ko, Lang::En, Lang::Ja];

    #[test]
    fn rotates_by_minute_and_wraps() {
        let a = current(Lang::Ko, 0);
        let b = current(Lang::Ko, 60);
        assert_ne!(a, b, "adjacent minutes must show different tips");
        // 한 바퀴 돌면 처음으로
        assert_eq!(
            current(Lang::Ko, 0),
            current(Lang::Ko, 60 * tips(Lang::Ko).len() as u64)
        );
    }

    /// data/tips.txt의 유효 줄이 전부 파싱되는지 — 파일 분리 후에도 팁이
    /// 조용히 증발하지 않는다(완전성).
    #[test]
    fn parses_all_seventy_tips_from_the_data_file() {
        assert_eq!(tips(Lang::Ko).len(), 70);
    }

    /// 세 언어 모두 회전할 만큼은 있어야 한다(v0.21). 언어별 파일이 비면
    /// 그 언어에서 팁 줄이 통째로 사라지므로, 실수로 비는 걸 여기서 막는다.
    #[test]
    fn every_language_has_enough_tips_to_rotate() {
        for lang in ALL {
            assert!(
                tips(lang).len() >= 10,
                "{lang:?} has too few tips: {}",
                tips(lang).len()
            );
        }
    }

    /// 완전성: 모든 팁이 비어있지 않고 개행 없는 한 줄이다.
    #[test]
    fn every_tip_is_a_nonempty_single_line() {
        for lang in ALL {
            for t in tips(lang) {
                assert!(!t.trim().is_empty());
                assert!(!t.contains('\n'));
            }
        }
    }

    /// 팁 줄은 wrap 없는 1행이라 80칸 터미널에서 "Tip: "(5) + 본문이 넘치면
    /// 조용히 잘린다(v0.2 리뷰 Important). 전각=2칸 보수 휴리스틱으로 본문
    /// 표시폭을 75칸 이하로 강제한다 — 새 팁 추가 시 이 테스트가 잘림을 막는다.
    #[test]
    fn every_tip_fits_an_80_column_terminal_with_prefix() {
        for lang in ALL {
            for t in tips(lang) {
                let width: usize = t.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
                assert!(
                    width <= 75,
                    "tip too wide for 80-col terminal ({width} > 75): {t}"
                );
            }
        }
    }

    /// 언어를 바꾸면 그 언어의 팁이 나온다 — v0.20까지는 어느 언어에서도
    /// 한국어 팁이 나왔다(영어 데모 프레임에서 발견한 결함).
    #[test]
    fn each_language_shows_its_own_tips() {
        let ko = current(Lang::Ko, 0);
        let en = current(Lang::En, 0);
        let ja = current(Lang::Ja, 0);
        assert_ne!(ko, en);
        assert_ne!(ko, ja);
        assert!(
            en.is_ascii(),
            "English tip should not carry Korean text: {en}"
        );
        assert!(
            !ja.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)),
            "Japanese tip contains Hangul: {ja}"
        );
    }

    /// 런타임 목록이 있으면 그것으로 회전, 없으면 임베드본.
    #[test]
    fn pick_prefers_the_runtime_override() {
        let over = Some(vec!["원격 팁 A".to_string(), "원격 팁 B".to_string()]);
        assert_eq!(pick(&over, Lang::Ko, 0), "원격 팁 A");
        assert_eq!(pick(&over, Lang::Ko, 60), "원격 팁 B");
        assert_eq!(pick(&over, Lang::Ko, 120), "원격 팁 A"); // wrap
        let none: Option<Vec<String>> = None;
        assert_eq!(pick(&none, Lang::Ko, 0), current(Lang::Ko, 0)); // 임베드 폴백
    }

    /// 원격 갱신본은 한국어 파일이라 다른 언어에서는 쓰지 않는다 — 안 그러면
    /// 영어 화면에 한국어 팁이 원격 경로로 되돌아온다(v0.21).
    #[test]
    fn the_remote_override_is_korean_only() {
        let over = Some(vec!["원격 팁 A".to_string(), "원격 팁 B".to_string()]);
        assert_eq!(pick(&over, Lang::En, 0), current(Lang::En, 0));
        assert_eq!(pick(&over, Lang::Ja, 0), current(Lang::Ja, 0));
    }

    /// 원격 파싱: 유효 줄 10개 미만이면 기각(None) — 깨진 응답이 팁을 비우지 않게.
    #[test]
    fn parse_remote_rejects_short_or_garbage_payloads() {
        assert!(parse_remote("").is_none());
        assert!(parse_remote("# comment only\n\n").is_none());
        let nine: String = (0..9).map(|i| format!("팁 {i}\n")).collect();
        assert!(parse_remote(&nine).is_none());
        let ten: String = (0..10).map(|i| format!("팁 {i}\n")).collect();
        let parsed = parse_remote(&ten).expect("10 valid lines must be accepted");
        assert_eq!(parsed.len(), 10);
    }
}
