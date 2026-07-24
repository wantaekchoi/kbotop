//! RSS 2.0 → NewsItem. 매체별 편차가 커서 전부 관용 파싱한다.
//! 실측 편차: dc:date만 있고 pubDate 없음(일간스포츠), description이 빈 문자열
//! (한겨레), description이 사실상 전문(동아·뉴시스) — 상한으로 흡수한다.
use crate::error::{Error, Result};
use crate::model::NewsItem;
use crate::source::text::{lead_excerpt, strip_html_to_text, EXCERPT_CHARS};

/// 자식 원소의 텍스트를 네임스페이스 무시하고(local name 기준) 모아 반환한다.
/// CDATA와 혼합 콘텐츠를 모두 담기 위해 하위 텍스트 노드를 전부 이어붙인다.
/// `is_text()`로 실제 텍스트/CDATA 노드만 골라야 한다 — `descendants()`는 자기
/// 자신(엘리먼트)도 포함하는데, roxmltree의 `Node::text()`는 엘리먼트에 대해
/// "첫 자식이 텍스트면 그 내용"을 반환하는 지름길이라, 필터 없이 그대로 쓰면
/// 첫 텍스트 조각이 두 번(엘리먼트 자신 + 실제 텍스트 노드) 잡혀 중복된다.
fn child_text(node: roxmltree::Node, local: &str) -> String {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == local)
        .map(|c| {
            c.descendants()
                .filter(|d| d.is_text())
                .filter_map(|d| d.text())
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// (year, month, day, hour, minute, second, UTC 기준 오프셋 분) — 오프셋이
/// None이면 표기가 없다는 뜻으로 KST로 간주한다.
type ParsedDateTime = (i64, i64, i64, i64, i64, i64, Option<i64>);

/// RFC822/ISO8601 오프셋 토큰(`GMT`, `UTC`, `Z`, `+0900`, `+09:00`, `-0500` …)을
/// UTC 기준 분단위로 해석한다. 인식하지 못하면 None — 호출부는 "오프셋 없음
/// (=KST로 간주)"과 동일하게 취급한다.
fn offset_minutes(tz: &str) -> Option<i64> {
    let tz = tz.trim();
    if tz.eq_ignore_ascii_case("GMT") || tz.eq_ignore_ascii_case("UTC") || tz == "Z" {
        return Some(0);
    }
    let (sign, rest) = match tz.strip_prefix('+') {
        Some(r) => (1i64, r),
        None => (-1i64, tz.strip_prefix('-')?),
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    let (h, m): (i64, i64) = match digits.len() {
        4 => (digits[..2].parse().ok()?, digits[2..4].parse().ok()?),
        2 => (digits.parse().ok()?, 0),
        _ => return None,
    };
    Some(sign * (h * 60 + m))
}

/// UTC(혹은 KST가 아닌 오프셋) 시각에 `shift_min`분을 더해 KST로 이동한다.
/// 날짜가 자정을 넘나들면(자정 근처·월말·연말) `dateutil`의 순수 날짜 산술로
/// 정확히 다음/이전 날로 넘긴다.
fn shift_to_kst(
    y: i64,
    m: i64,
    d: i64,
    hh: i64,
    mm: i64,
    ss: i64,
    shift_min: i64,
) -> (i64, i64, i64, i64, i64, i64) {
    let days = crate::dateutil::days_from_civil(y, m, d);
    let total_min = days * 1440 + hh * 60 + mm + shift_min;
    let new_days = total_min.div_euclid(1440);
    let rem_min = total_min.rem_euclid(1440);
    let (ny, nm, nd) = crate::dateutil::civil_from_days(new_days);
    (ny, nm, nd, rem_min / 60, rem_min % 60, ss)
}

/// 숫자로 시작하는 날짜: ISO8601("2026-07-23T22:25:00+09:00")나 공백 구분
/// ("2026-07-24 09:18:39", 일간스포츠 dc:date — 타임존 표기 없음). `T` 뒤에서
/// `Z`나 `+`/`-` 오프셋을 찾아 분리해 해석하고, 나머지 숫자로 y/m/d/h/m/s를
/// 채운다.
fn parse_numeric_date(s: &str) -> Option<ParsedDateTime> {
    let (main, offset) = match s.find('T') {
        Some(tpos) => {
            let time_part = &s[tpos + 1..];
            if let Some(zpos) = time_part.find('Z') {
                (&s[..tpos + 1 + zpos], offset_minutes("Z"))
            } else if let Some(opos) = time_part.find(['+', '-']) {
                (&s[..tpos + 1 + opos], offset_minutes(&time_part[opos..]))
            } else {
                (s, None)
            }
        }
        None => (s, None),
    };
    let digits: String = main.chars().filter(|c| c.is_ascii_digit()).collect();
    let (y, m, d, hh, mm, ss): (i64, i64, i64, i64, i64, i64) = match digits.len() {
        n if n >= 14 => (
            digits[0..4].parse().ok()?,
            digits[4..6].parse().ok()?,
            digits[6..8].parse().ok()?,
            digits[8..10].parse().ok()?,
            digits[10..12].parse().ok()?,
            digits[12..14].parse().ok()?,
        ),
        n if n >= 8 => (
            digits[0..4].parse().ok()?,
            digits[4..6].parse().ok()?,
            digits[6..8].parse().ok()?,
            0,
            0,
            0,
        ),
        _ => return None,
    };
    Some((y, m, d, hh, mm, ss, offset))
}

/// RFC822: "Thu, 24 Jul 2026 00:18:39 GMT" (요일은 없을 수도 있다). 다섯 번째
/// 토큰(있으면)을 오프셋으로 해석한다.
fn parse_rfc822_date(s: &str) -> Option<ParsedDateTime> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let parts: Vec<&str> = s.split_whitespace().collect();
    let base = usize::from(parts.first().is_some_and(|p| p.ends_with(',')));
    let day: i64 = parts.get(base)?.parse().ok()?;
    let mon = parts.get(base + 1)?;
    let year: i64 = parts.get(base + 2)?.parse().ok()?;
    let m = MONTHS.iter().position(|x| x.eq_ignore_ascii_case(mon))? as i64 + 1;
    let time_digits: String = parts
        .get(base + 3)
        .map(|t| t.chars().filter(|c| c.is_ascii_digit()).collect())
        .unwrap_or_default();
    let time = format!("{time_digits:0<6}");
    let hh: i64 = time[0..2].parse().unwrap_or(0);
    let mm: i64 = time[2..4].parse().unwrap_or(0);
    let ss: i64 = time[4..6].parse().unwrap_or(0);
    let offset = parts.get(base + 4).and_then(|t| offset_minutes(t));
    Some((year, m, day, hh, mm, ss, offset))
}

/// 피드 날짜를 정렬 가능한 "YYYYMMDDHHMMSS"(KST 기준)로 정규화한다. 실측 편차를
/// 흡수한다: RFC822("Fri, 24 Jul 2026 11:26:41 +0900" 스포츠조선,
/// "Fri, 24 Jul 2026 02:04:07 GMT" 스포티비뉴스 — 이름과 달리 진짜 UTC다),
/// "2026-07-24 09:18:39"(일간스포츠 dc:date — 타임존 표기 없음),
/// ISO8601("2026-07-23T22:25:00+09:00" 경향 dc:date). 해석 실패·결측 시 빈
/// 문자열을 돌려 정렬에서 뒤로 밀리게 한다.
///
/// 오프셋 처리: `+0900`/`+09:00`은 이미 KST라 그대로 두고, `GMT`/`UTC`/`Z`/
/// `+0000`은 9시간을 더해 KST로 맞춘다(스포티비뉴스가 이 경우 — 무시했더니
/// 최신 기사가 목록 29위로 밀리는 회귀가 있었다). 오프셋 표기가 아예 없으면
/// KST로 간주한다(현행 유지). 날짜 넘어감은 `dateutil::days_from_civil`/
/// `civil_from_days`로 정확히 계산해 자정·월말·연말 경계에서도 어긋나지 않는다.
pub(crate) fn normalized_date(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    let parsed = if s.starts_with(|c: char| c.is_ascii_digit()) {
        parse_numeric_date(s)
    } else {
        parse_rfc822_date(s)
    };
    let Some((y, m, d, hh, mm, ss, offset)) = parsed else {
        return String::new();
    };
    const KST_MIN: i64 = 9 * 60;
    let shift = KST_MIN - offset.unwrap_or(KST_MIN);
    let (y, m, d, hh, mm, ss) = if shift == 0 {
        (y, m, d, hh, mm, ss)
    } else {
        shift_to_kst(y, m, d, hh, mm, ss, shift)
    };
    format!("{y:04}{m:02}{d:02}{hh:02}{mm:02}{ss:02}")
}

/// 피드 XML을 NewsItem 목록으로. `label`은 출처(매체명)로 그대로 쓴다.
/// `keep_categories`가 비어 있지 않으면 `<category>`가 그중 하나인 항목만 남긴다.
pub(crate) fn feed_from_xml(
    xml: &str,
    label: &str,
    keep_categories: &[&str],
) -> Result<Vec<NewsItem>> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|e| Error::Data(format!("rss parse error: {e}")))?;
    let mut out = Vec::new();
    for item in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "item")
    {
        let title = strip_html_to_text(&child_text(item, "title"));
        if title.is_empty() {
            continue; // 제목 없는 항목은 표시할 게 없다.
        }
        if !keep_categories.is_empty() {
            let cat = child_text(item, "category");
            if !keep_categories.iter().any(|k| *k == cat) {
                continue;
            }
        }
        let summary = lead_excerpt(
            &strip_html_to_text(&child_text(item, "description")),
            EXCERPT_CHARS,
        );
        // pubDate 우선, 없으면 dc:date(local name "date").
        let raw_date = {
            let p = child_text(item, "pubDate");
            if p.trim().is_empty() {
                child_text(item, "date")
            } else {
                p
            }
        };
        out.push(NewsItem {
            title,
            source: label.to_string(),
            url: child_text(item, "link").trim().to_string(),
            summary,
            published: normalized_date(&raw_date),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIRKS: &str = include_str!("../../../tests/fixtures/rss_quirks.xml");

    /// CDATA·HTML·엔티티를 걷어내고, 제목 없는 항목은 버리고, label을 출처로 넣는다.
    #[test]
    fn feed_from_xml_parses_quirks_leniently() {
        let items = feed_from_xml(QUIRKS, "테스트일보", &[]).unwrap();
        assert_eq!(items.len(), 2, "제목 없는 항목은 버려야 한다");
        assert_eq!(items[0].title, "김도영 동점 3점 홈런");
        assert_eq!(items[0].source, "테스트일보");
        assert_eq!(
            items[0].summary, "9회말 동점 홈런을 터뜨렸다.",
            "HTML 태그가 남으면 안 된다"
        );
        assert_eq!(items[1].summary, "짧은 리드 & 요약", "엔티티 언이스케이프");
        assert!(items[0].url.starts_with("https://example.com/a"));
    }

    /// category 필터가 동작한다(일간스포츠·경향 보강 피드용).
    #[test]
    fn feed_from_xml_filters_by_category() {
        let items = feed_from_xml(QUIRKS, "테스트일보", &["프로야구"]).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "김도영 동점 3점 홈런");
    }

    /// 발췌 상한은 RSS 경로에서도 강제된다(동아·뉴시스는 description이 전문급).
    #[test]
    fn feed_from_xml_caps_excerpt_length() {
        let long = "가".repeat(600);
        let xml = format!(
            r#"<?xml version="1.0"?><rss><channel><item><title>t</title><link>u</link><description>{long}</description></item></channel></rss>"#
        );
        let items = feed_from_xml(&xml, "l", &[]).unwrap();
        assert!(
            items[0].summary.chars().count() <= EXCERPT_CHARS + 1,
            "상한 초과: {}",
            items[0].summary.chars().count()
        );
        assert!(items[0].summary.ends_with('…'));
    }

    /// 깨진 XML은 Err — 무패닉.
    #[test]
    fn feed_from_xml_rejects_malformed_without_panic() {
        assert!(feed_from_xml("<rss><channel><item>", "l", &[]).is_err());
        assert!(feed_from_xml("", "l", &[]).is_err());
    }

    /// 날짜 정규화: 매체별 형식을 모두 정렬 가능한 KST 값으로 흡수한다.
    #[test]
    fn normalized_date_absorbs_feed_format_variance() {
        // RFC822 + KST 오프셋(스포츠조선 실측: +0900) — 그대로 KST
        assert_eq!(
            normalized_date("Fri, 24 Jul 2026 11:26:41 +0900"),
            "20260724112641"
        );
        // 공백 구분, 타임존 표기 없음(일간스포츠 dc:date) — KST로 간주
        assert_eq!(normalized_date("2026-07-24 09:18:39"), "20260724091839");
        // ISO8601 + KST 오프셋(경향 dc:date)
        assert_eq!(
            normalized_date("2026-07-23T22:25:00+09:00"),
            "20260723222500"
        );
        // RFC822 + GMT(스포티비뉴스 실측 — 이름과 달리 진짜 UTC) → +9시간
        assert_eq!(
            normalized_date("Fri, 24 Jul 2026 02:04:07 GMT"),
            "20260724110407"
        );
        // 날짜만 있으면 자정으로(오프셋 없음)
        assert_eq!(normalized_date("2026-07-24"), "20260724000000");
        // 해석 불가·결측은 빈 문자열(정렬 뒤로)
        assert_eq!(normalized_date(""), "");
        assert_eq!(normalized_date("어제"), "");
        assert_eq!(normalized_date("Mon, bogus"), "");
    }

    /// 회귀 재현: 4가지 실측 형식이 모두 같은 순간을 같은 값으로 정규화해야
    /// 한다. 스포티비뉴스의 GMT를 KST와 동일하게 취급했더니 최신 기사가 목록
    /// 29위로 밀리는 회귀가 있었다.
    #[test]
    fn normalized_date_unifies_same_instant_across_feed_formats() {
        let expected = "20260724110407"; // 2026-07-24 11:04:07 KST
        assert_eq!(
            normalized_date("Fri, 24 Jul 2026 02:04:07 GMT"),
            expected,
            "스포티비뉴스류 GMT(진짜 UTC)"
        );
        assert_eq!(
            normalized_date("2026-07-24 11:04:07"),
            expected,
            "타임존 표기 없음 = KST"
        );
        assert_eq!(
            normalized_date("2026-07-24T11:04:07+09:00"),
            expected,
            "명시적 KST 오프셋(ISO8601)"
        );
        assert_eq!(
            normalized_date("Fri, 24 Jul 2026 11:04:07 +0900"),
            expected,
            "스포츠조선류 +0900"
        );
    }

    /// 자정 근처 UTC → KST 변환은 날짜가 다음 날로 넘어간다.
    #[test]
    fn normalized_date_gmt_rolls_over_midnight_to_next_day_kst() {
        assert_eq!(
            normalized_date("Thu, 23 Jul 2026 20:00:00 GMT"),
            "20260724050000"
        );
    }

    /// 연말 UTC → KST 변환은 연도까지 정확히 넘어간다.
    #[test]
    fn normalized_date_gmt_rolls_over_year_boundary() {
        assert_eq!(
            normalized_date("Wed, 31 Dec 2025 20:00:00 GMT"),
            "20260101050000"
        );
    }

    /// 해석 불가 입력은 여전히 패닉하지 않는다(값을 특정하지 않고 호출만 확인).
    #[test]
    fn normalized_date_never_panics_on_garbage_offsets() {
        for input in [
            "not a date",
            "Fri, 99 Zzz 2026 99:99:99 GMT",
            "2026-13-99T99:99:99+99:99",
            "Fri, 24 Jul 2026 02:04:07 +",
            "Fri, 24 Jul 2026 02:04:07 -",
            "2026-07-24T11:04:07+",
            "2026-07-24TZ",
        ] {
            let _ = normalized_date(input);
        }
    }

    /// fixture의 두 항목이 각기 다른 형식인데도 정렬 키가 채워진다.
    #[test]
    fn feed_from_xml_fills_published_from_either_date_field() {
        let items = feed_from_xml(QUIRKS, "테스트일보", &[]).unwrap();
        assert_eq!(
            items[0].published, "20260724091839",
            "dc:date를 읽어야 한다"
        );
        assert_eq!(
            items[1].published, "20260724091839",
            "pubDate를 읽어야 한다(GMT라 KST로 +9시간)"
        );
    }
}
