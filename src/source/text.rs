//! 소스 공통 텍스트 처리 — HTML 평문화와 발췌.
//! 네이버 기사 본문과 RSS `description`이 모두 HTML 조각을 담고 있어 공유한다.

/// 인앱에 표시할 발췌의 최대 길이(전각 포함 char 단위). 저작권상 전문 복제를
/// 피하기 위한 상한이며, 모든 뉴스 경로가 이 값을 통과해야 한다.
pub(crate) const EXCERPT_CHARS: usize = 200;

/// HTML 조각 → 평문. 정규식 없이 수동 문자 스캔: '<'…'>' 구간은 태그로 보고
/// 제거하되 br 계열만 줄바꿈으로 치환한 뒤, 엔티티 언이스케이프와 공백 정리.
pub(crate) fn strip_html_to_text(html: &str) -> String {
    let mut stripped = String::with_capacity(html.len());
    let mut chars = html.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            stripped.push(c);
            continue;
        }
        let mut tag = String::new();
        for c2 in chars.by_ref() {
            if c2 == '>' {
                break;
            }
            tag.push(c2);
        }
        // 태그명(선행 '/'·속성 제외)이 정확히 "br"일 때만 줄바꿈.
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if name == "br" {
            stripped.push('\n');
        }
    }
    normalize_whitespace(&unescape_entities(&stripped))
}

/// `&amp;`는 다른 엔티티를 모두 치환한 뒤 마지막에 처리한다 — 먼저 하면
/// "&amp;lt;"가 두 번 풀려 "<"로 잘못 언이스케이프된다.
///
/// `&apos;`/`&middot;`는 발행사가 CDATA 안에서 이중 이스케이프해 내보내는
/// 경우가 실측에 있었다(스포티비뉴스 제목 34/100건, 일간스포츠 발췌
/// 6/25건) — XML 파서는 규격대로 리터럴 텍스트로 넘기므로 여기서 걷어낸다.
fn unescape_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&middot;", "·")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// 줄 내부 연속 공백을 한 칸으로, 연속 빈 줄을 한 줄로 축소하고 앞뒤를 trim한다.
fn normalize_whitespace(s: &str) -> String {
    let mut out_lines: Vec<String> = Vec::new();
    let mut prev_blank = false;
    for raw_line in s.split('\n') {
        let collapsed = raw_line.split_whitespace().collect::<Vec<_>>().join(" ");
        let blank = collapsed.is_empty();
        if blank && prev_blank {
            continue;
        }
        out_lines.push(collapsed);
        prev_blank = blank;
    }
    out_lines.join("\n").trim().to_string()
}

/// 평문에서 리드 발췌만 남긴다(저작권: 전문 미표시). `max_chars`(전각 포함 char
/// 단위)를 넘으면 단어 중간 절단을 피해 마지막 공백에서 끊고 '…'를 붙인다.
/// 이미 짧으면 원문 그대로. 멀티바이트 경계 안전(무패닉).
pub(crate) fn lead_excerpt(text: &str, max_chars: usize) -> String {
    let mut end = text.len();
    for (count, (i, _)) in text.char_indices().enumerate() {
        if count == max_chars {
            end = i;
            break;
        }
    }
    if end >= text.len() {
        return text.to_string();
    }
    let head = &text[..end];
    let cut = match head.rfind(char::is_whitespace) {
        Some(p) if p > 0 => &head[..p],
        _ => head,
    };
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_to_text_handles_tags_and_entities() {
        let input = "<p>a&amp;b</p><br>c &lt;d&gt;";
        assert_eq!(strip_html_to_text(input), "a&b\nc <d>");
    }

    /// 실측: 스포티비뉴스 제목의 `&apos;`, 일간스포츠 발췌의 `&middot;`가
    /// 화면에 그대로 노출됐다(예: "LG 7연패 했는데 &apos;5.5경기 역전&apos;",
    /// "배찬승(19&middot;삼성 라이온즈)").
    #[test]
    fn unescape_entities_decodes_apos_and_middot() {
        assert_eq!(
            unescape_entities("&apos;5.5경기 역전&apos;"),
            "'5.5경기 역전'"
        );
        assert_eq!(
            unescape_entities("19&middot;삼성 라이온즈"),
            "19·삼성 라이온즈"
        );
    }

    /// `&amp;`를 마지막에 처리해야 이중 이스케이프(`&amp;lt;`, `&amp;apos;` 등)가
    /// 두 번 풀리지 않는다 — 새로 추가한 엔티티도 이 순서 규칙을 따라야 한다.
    #[test]
    fn unescape_entities_does_not_double_unescape_amp_sequences() {
        assert_eq!(unescape_entities("&amp;lt;"), "&lt;");
        assert_eq!(unescape_entities("&amp;apos;"), "&apos;");
        assert_eq!(unescape_entities("&amp;middot;"), "&middot;");
    }

    /// br 계열만 줄바꿈. "br"로 시작하는 다른 태그(<broom>)는 삭제만 한다.
    #[test]
    fn only_real_br_tags_become_newlines() {
        assert_eq!(strip_html_to_text("a<br>b<br/>c<br />d"), "a\nb\nc\nd");
        assert_eq!(strip_html_to_text(r#"a<br class="x">b"#), "a\nb");
        assert_eq!(
            strip_html_to_text("a<broom>b"),
            "ab",
            "<broom> must not insert a newline"
        );
    }

    /// 발췌: 짧은 글은 그대로, 긴 글은 경계에서 끊고 '…'. 멀티바이트 안전.
    #[test]
    fn lead_excerpt_truncates_at_word_boundary_with_ellipsis() {
        assert_eq!(lead_excerpt("짧은 글", 100), "짧은 글");
        let long = "가나다 라마바 ".repeat(50); // 350 chars
        let ex = lead_excerpt(&long, 20);
        assert!(ex.chars().count() <= 21, "len {}", ex.chars().count());
        assert!(ex.ends_with('…'), "must mark truncation: {ex}");
        assert!(!ex.contains("  "), "no dangling partial word spacing: {ex}");
        // 다양한 컷 위치에서 패닉이 없어야 한다(char 경계 안전).
        for n in 0..40 {
            let _ = lead_excerpt(&long, n);
        }
    }
}
