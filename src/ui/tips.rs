use std::sync::OnceLock;

/// 야구 초보용 한 줄 팁 원본 — `data/tips.txt`에 한 줄당 하나(사람이 GitHub에서
/// 바로 읽고 PR로 추가할 수 있는 형태). 규칙은 사실 기반 자체 표현(저작권 무관),
/// 본문은 한국어 콘텐츠로 Paragraph에 렌더되므로 폭 안전 — 영어 chrome 하드
/// 제약은 라벨("Tip:")에만 적용된다. '#' 줄과 빈 줄은 무시한다.
/// (v0.3 후보: 릴리스 없이 갱신되도록 GitHub raw에서 런타임 fetch.)
const TIPS_RAW: &str = include_str!("../../data/tips.txt");

/// 파싱된 팁 목록(최초 1회 lazy). 파일이 주석/공백뿐이어도 패닉하지 않도록
/// current()가 빈 목록을 방어한다.
fn tips() -> &'static [&'static str] {
    static TIPS: OnceLock<Vec<&'static str>> = OnceLock::new();
    TIPS.get_or_init(|| {
        TIPS_RAW
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    })
}

/// 현재 분(now_secs/60)에 해당하는 팁 — 1분마다 회전, 의존성 없이 결정적.
/// 팁 파일이 비어 있으면 빈 문자열(표시만 조용히 생략됨).
pub fn current(now_secs: u64) -> &'static str {
    let t = tips();
    if t.is_empty() {
        return "";
    }
    t[((now_secs / 60) as usize) % t.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_by_minute_and_wraps() {
        let a = current(0);
        let b = current(60);
        assert_ne!(a, b, "adjacent minutes must show different tips");
        // 한 바퀴 돌면 처음으로
        assert_eq!(current(0), current(60 * tips().len() as u64));
    }

    /// data/tips.txt의 유효 줄이 전부 파싱되는지 — 파일 분리 후에도 팁이
    /// 조용히 증발하지 않는다(완전성).
    #[test]
    fn parses_all_seventy_tips_from_the_data_file() {
        assert_eq!(tips().len(), 70);
    }

    /// 완전성: 모든 팁이 비어있지 않고 개행 없는 한 줄이다.
    #[test]
    fn every_tip_is_a_nonempty_single_line() {
        for t in tips() {
            assert!(!t.trim().is_empty());
            assert!(!t.contains('\n'));
        }
    }

    /// 팁 줄은 wrap 없는 1행이라 80칸 터미널에서 "Tip: "(5) + 본문이 넘치면
    /// 조용히 잘린다(v0.2 리뷰 Important). 전각=2칸 보수 휴리스틱으로 본문
    /// 표시폭을 75칸 이하로 강제한다 — 새 팁 추가 시 이 테스트가 잘림을 막는다.
    #[test]
    fn every_tip_fits_an_80_column_terminal_with_prefix() {
        for t in tips() {
            let width: usize = t.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
            assert!(
                width <= 75,
                "tip too wide for 80-col terminal ({width} > 75): {t}"
            );
        }
    }
}
