//! wrap 없는 1행 위젯을 위한 표시폭 유틸 — 조용한 클리핑 대신 정직한 말줄임.

/// 전각=2칸 보수 휴리스틱 표시폭(ASCII=1, 그 외=2). 한글·전각 문자에서
/// ratatui(unicode-width)와 일치하고, 애매폭 문자는 넓게 잡아 잘림을
/// 과대평가한다(안전한 쪽으로 틀린다).
pub fn display_width(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

/// `max_cols`를 넘는 문자열을 문자 경계에서 자르고 "…"(2칸 예약)를 붙인다.
/// 넘지 않으면 그대로 반환. 전각 문자를 중간에서 쪼개지 않는다.
pub fn ellipsize(s: &str, max_cols: usize) -> String {
    if display_width(s) <= max_cols {
        return s.to_string();
    }
    let budget = max_cols.saturating_sub(2); // '…' 몫
    let mut w = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        let cw = if c.is_ascii() { 1 } else { 2 };
        if w + cw > budget {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_strings_pass_through_unchanged() {
        assert_eq!(ellipsize("hello", 10), "hello");
        assert_eq!(ellipsize("한글", 4), "한글"); // 정확히 맞으면 말줄임 없음
    }

    #[test]
    fn long_strings_get_honest_ellipsis_within_budget() {
        let out = ellipsize("가나다라마바사", 8);
        assert!(out.ends_with('…'));
        // 결과 폭이 max_cols를 넘지 않는다('…'=2칸 회계).
        assert!(
            display_width(&out) <= 8,
            "width {} > 8: {out}",
            display_width(&out)
        );
    }

    /// 전각 문자가 예산 경계에 걸리면 그 문자를 통째로 버린다(반쪽 렌더 금지).
    #[test]
    fn fullwidth_char_is_never_split_at_the_boundary() {
        // budget = 5-2 = 3칸 → '가'(2) 다음 '나'(2)는 3을 초과 → "가…"
        assert_eq!(ellipsize("가나다", 5), "가…");
    }

    #[test]
    fn zero_and_tiny_budgets_do_not_panic() {
        assert_eq!(ellipsize("가나다", 0), "…");
        assert_eq!(ellipsize("가나다", 1), "…");
        assert_eq!(ellipsize("", 0), "");
    }

    #[test]
    fn mixed_ascii_korean_width_accounting() {
        assert_eq!(display_width("B2 S3 O3"), 8);
        assert_eq!(display_width("파울"), 4);
        let out = ellipsize("Pitch 1/7 145km 3구 파울", 12);
        assert!(display_width(&out) <= 12);
        assert!(out.starts_with("Pitch"));
    }
}
