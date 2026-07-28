//! 로컬 시간대 오프셋 결정(외부 크레이트 없이).
//!
//! kbotop을 보는 사람이 서울에 있는지 뉴욕에 있는지 모른다 — **표시 시각은 그
//! 사람이 있는 곳 기준**이어야 한다. 반면 **경기일 판단은 KST 고정**이다
//! (`dateutil::kst_days`): 뉴욕에 있어도 보는 건 "KBO의 오늘 경기"지 자기
//! 로컬의 오늘이 아니다. 이 파일은 앞의 절반(표시 시각)만 담당한다.
//!
//! `chrono`/`time`/`libc` 없이 std만 쓴다(새 크레이트 금지 제약).

/// KST(UTC+9) 오프셋 — 자동 감지가 전부 실패했을 때의 폴백이자,
/// KBO 데이터가 원래 쓰는 기준.
pub const KST_OFFSET_SECS: i32 = 9 * 3600;

/// 결정된 표시 시간대: UTC 기준 오프셋(초)과 표기용 약어.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZone {
    pub offset_secs: i32,
    /// `KST`·`EDT` 같은 약어. TZif에서 못 얻으면 `UTC+9` 형태로 만든다.
    pub abbrev: String,
}

impl TimeZone {
    pub fn kst() -> Self {
        Self {
            offset_secs: KST_OFFSET_SECS,
            abbrev: "KST".to_string(),
        }
    }

    fn from_offset(offset_secs: i32) -> Self {
        Self {
            offset_secs,
            abbrev: offset_abbrev(offset_secs),
        }
    }

    /// 이 시간대가 KBO 기준(KST)과 같은가 — 같으면 화면에서 굳이 다른 표기를
    /// 할 필요가 없다(한국 사용자 무변화).
    pub fn is_kst(&self) -> bool {
        self.offset_secs == KST_OFFSET_SECS
    }
}

/// `+09:00` 같은 오프셋 표기를 만든다(약어를 못 얻었을 때).
fn offset_abbrev(offset_secs: i32) -> String {
    let sign = if offset_secs < 0 { '-' } else { '+' };
    let abs = offset_secs.abs();
    let (h, m) = (abs / 3600, (abs % 3600) / 60);
    if m == 0 {
        format!("UTC{sign}{h}")
    } else {
        format!("UTC{sign}{h}:{m:02}")
    }
}

/// 표시용 시간대를 정한다. 폴백 사슬 — 앞에서 성공하면 멈춘다.
///
/// 1. `setting`(CLI `--tz` / config `timezone`): `auto` · `kst` · `+09:00` 류
/// 2. `TZ` 환경변수가 **숫자 오프셋**이면 사용(IANA 이름은 여기서 못 씀)
/// 3. Unix `/etc/localtime`(TZif) 파싱 — DST까지 자동 처리
/// 4. 전부 실패 → KST
///
/// `now_secs`는 UTC epoch 초(TZif에서 "지금 적용되는" 구간을 고르는 데 쓴다).
/// `--tz`로 받은 값이 우리가 해석할 수 있는 것인지 본다.
///
/// [`resolve`]는 **관용적으로** 파싱한다 — config는 영속 상태라, 모르는 값이
/// 들어 있다고 앱이 뜨지 않으면 곤란하다. 그러나 **CLI는 fail-fast여야 한다**
/// (`main.rs`가 `--lang`·`--team`·`--date`에 대해 이미 그렇게 한다).
///
/// 이 구분이 없으면 `--tz Asia/Seoul`이 **아무 말 없이 무시되고** 사용자는
/// 자기가 지정한 시간대로 보고 있다고 믿는다. IANA 이름은 가장 흔한 표기라
/// 실제로 밟는다.
pub fn is_supported_setting(raw: &str) -> bool {
    let t = raw.trim();
    t.is_empty()
        || t.eq_ignore_ascii_case("auto")
        || t.eq_ignore_ascii_case("kst")
        || parse_offset(t).is_some()
}

pub fn resolve(setting: Option<&str>, now_secs: u64) -> TimeZone {
    if let Some(s) = setting {
        let t = s.trim();
        if !t.is_empty() && !t.eq_ignore_ascii_case("auto") {
            if t.eq_ignore_ascii_case("kst") {
                return TimeZone::kst();
            }
            if let Some(off) = parse_offset(t) {
                return TimeZone::from_offset(off);
            }
            // 알 수 없는 값은 조용히 무시하고 자동 감지로 넘어간다(관용 파싱).
        }
    }

    if let Some(off) = std::env::var("TZ").ok().and_then(|v| parse_offset(&v)) {
        return TimeZone::from_offset(off);
    }

    #[cfg(unix)]
    if let Some(tz) = from_tzif_file("/etc/localtime", now_secs) {
        return tz;
    }
    #[cfg(not(unix))]
    let _ = now_secs; // Windows: std로 로컬 오프셋을 얻을 방법이 없다 — 아래 KST 폴백.

    TimeZone::kst()
}

/// `+09:00` `-0400` `UTC+9` `+09` 같은 **숫자 오프셋** 표기를 초로. IANA
/// 이름(`Asia/Seoul`)은 여기서 처리하지 않는다(None → 다음 폴백 단계로).
fn parse_offset(raw: &str) -> Option<i32> {
    let s = raw.trim();
    let s = s
        .strip_prefix("UTC")
        .or_else(|| s.strip_prefix("utc"))
        .or_else(|| s.strip_prefix("GMT"))
        .or_else(|| s.strip_prefix("gmt"))
        .unwrap_or(s);
    let s = s.trim();
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, s.strip_prefix('+').unwrap_or(s)),
    };
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit() || b == b':') {
        return None;
    }
    let (h_str, m_str) = match rest.split_once(':') {
        Some((h, m)) => (h, m),
        // 콜론 없는 형태: 4자리면 HHMM, 그 외는 시(H/HH)로 본다.
        None if rest.len() == 4 => rest.split_at(2),
        None => (rest, "0"),
    };
    let h: i32 = h_str.parse().ok()?;
    let m: i32 = m_str.parse().ok()?;
    if !(0..=14).contains(&h) || !(0..60).contains(&m) {
        return None; // UTC 오프셋 실제 범위(-12~+14)를 벗어나면 오타로 본다.
    }
    Some(sign * (h * 3600 + m * 60))
}

/// TZif 파일을 읽어 `now_secs`에 적용되는 오프셋·약어를 얻는다.
/// 실패(파일 없음·매직 불일치·잘림·구조 이상)는 전부 `None` — 무패닉.
#[cfg(unix)]
fn from_tzif_file(path: &str, now_secs: u64) -> Option<TimeZone> {
    let data = std::fs::read(path).ok()?;
    parse_tzif(&data, now_secs as i64)
}

/// TZif 파서. v2/v3이면 64비트 블록을, 아니면 v1 32비트 블록을 쓴다.
/// (RFC 8536. 헤더 44바이트 + 데이터 블록 구조.)
fn parse_tzif(data: &[u8], now: i64) -> Option<TimeZone> {
    let (counts, body) = tzif_header(data)?;
    let v1_len = counts.block_len(4);

    // v2+ 는 v1 블록 뒤에 같은 구조를 64비트 시각으로 한 번 더 담는다.
    let version = data.get(4).copied().unwrap_or(b'\0');
    if version >= b'2' {
        let second = body.get(v1_len..)?;
        let (c2, body2) = tzif_header(second)?;
        return tzif_lookup(&c2, body2, now, 8);
    }
    tzif_lookup(&counts, body, now, 4)
}

struct Counts {
    isutcnt: usize,
    isstdcnt: usize,
    leapcnt: usize,
    timecnt: usize,
    typecnt: usize,
    charcnt: usize,
}

impl Counts {
    /// 데이터 블록 길이(시각 필드 폭 `tw`는 v1=4, v2+=8).
    fn block_len(&self, tw: usize) -> usize {
        self.timecnt * tw
            + self.timecnt
            + self.typecnt * 6
            + self.charcnt
            + self.leapcnt * (tw + 4)
            + self.isstdcnt
            + self.isutcnt
    }
}

/// 44바이트 헤더를 읽고 (카운트, 데이터 시작 슬라이스)를 준다.
fn tzif_header(data: &[u8]) -> Option<(Counts, &[u8])> {
    if data.len() < 44 || &data[0..4] != b"TZif" {
        return None;
    }
    let u32_at = |i: usize| -> usize {
        u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize
    };
    let counts = Counts {
        isutcnt: u32_at(20),
        isstdcnt: u32_at(24),
        leapcnt: u32_at(28),
        timecnt: u32_at(32),
        typecnt: u32_at(36),
        charcnt: u32_at(40),
    };
    // typecnt=0이면 오프셋을 하나도 못 고른다 — 이상 파일로 본다.
    if counts.typecnt == 0 {
        return None;
    }
    Some((counts, data.get(44..)?))
}

/// 데이터 블록에서 `now`에 적용되는 local time type을 찾아 오프셋·약어를 낸다.
fn tzif_lookup(c: &Counts, body: &[u8], now: i64, tw: usize) -> Option<TimeZone> {
    let trans_end = c.timecnt.checked_mul(tw)?;
    let idx_end = trans_end.checked_add(c.timecnt)?;
    let types_end = idx_end.checked_add(c.typecnt.checked_mul(6)?)?;
    let chars_end = types_end.checked_add(c.charcnt)?;
    if body.len() < chars_end {
        return None;
    }

    let read_time = |i: usize| -> i64 {
        let s = &body[i * tw..i * tw + tw];
        let mut v: i64 = if s[0] & 0x80 != 0 { -1 } else { 0 };
        for b in s {
            v = (v << 8) | (*b as i64);
        }
        v
    };

    // 마지막으로 `now` 이하인 transition의 타입 인덱스. 없으면(=모든 전환이
    // 미래거나 전환이 없으면) 첫 번째 타입을 쓴다(RFC 권장 관용).
    let mut type_idx = 0usize;
    for i in 0..c.timecnt {
        if read_time(i) <= now {
            type_idx = body[trans_end + i] as usize;
        } else {
            break;
        }
    }
    if type_idx >= c.typecnt {
        return None;
    }

    let t = &body[idx_end + type_idx * 6..idx_end + type_idx * 6 + 6];
    let utoff = i32::from_be_bytes([t[0], t[1], t[2], t[3]]);
    if !(-89999..=93599).contains(&utoff) {
        return None; // RFC 8536이 정한 유효 범위 밖.
    }
    let desig_idx = t[5] as usize;

    let abbrev = body
        .get(types_end + desig_idx..chars_end)
        .and_then(|s| s.split(|b| *b == 0).next())
        .and_then(|s| std::str::from_utf8(s).ok())
        .filter(|s| !s.is_empty() && s.is_ascii())
        .map(|s| s.to_string())
        .unwrap_or_else(|| offset_abbrev(utoff));

    Some(TimeZone {
        offset_secs: utoff,
        abbrev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_offset_forms() {
        assert_eq!(parse_offset("+09:00"), Some(9 * 3600));
        assert_eq!(parse_offset("-04:00"), Some(-4 * 3600));
        assert_eq!(parse_offset("UTC+9"), Some(9 * 3600));
        assert_eq!(parse_offset("+0530"), Some(5 * 3600 + 30 * 60));
        assert_eq!(parse_offset("-0330"), Some(-(3 * 3600 + 30 * 60)));
        assert_eq!(parse_offset(" +09 "), Some(9 * 3600));
    }

    /// IANA 이름·쓰레기 값은 여기서 처리하지 않는다(다음 폴백 단계로 넘김).
    #[test]
    fn rejects_non_numeric_or_out_of_range() {
        assert_eq!(parse_offset("Asia/Seoul"), None);
        assert_eq!(parse_offset(""), None);
        assert_eq!(parse_offset("+99:00"), None);
        assert_eq!(parse_offset("+09:99"), None);
        assert_eq!(parse_offset("garbage"), None);
    }

    /// 명시 설정이 자동 감지를 이긴다 — Windows·컨테이너처럼 감지가 안 되는
    /// 환경에서 사용자가 직접 정할 수 있어야 한다.
    #[test]
    fn explicit_setting_wins_over_detection() {
        assert_eq!(resolve(Some("kst"), 0).offset_secs, KST_OFFSET_SECS);
        assert_eq!(resolve(Some("-04:00"), 0).offset_secs, -4 * 3600);
        assert_eq!(resolve(Some("+05:30"), 0).offset_secs, 5 * 3600 + 30 * 60);
    }

    /// 알 수 없는 설정값은 앱을 죽이지 않고 자동 감지로 넘어간다(관용 파싱).
    #[test]
    fn unknown_setting_falls_through_without_panic() {
        let tz = resolve(Some("Mars/Olympus"), 1_800_000_000);
        assert!(tz.offset_secs.abs() <= 14 * 3600);
    }

    /// 깨진 바이트·빈 입력에 패닉하지 않는다.
    #[test]
    fn malformed_tzif_is_rejected_without_panic() {
        assert_eq!(parse_tzif(&[], 0), None);
        assert_eq!(parse_tzif(b"NOPE", 0), None);
        assert_eq!(parse_tzif(&[0u8; 100], 0), None);
        let mut truncated = b"TZif2".to_vec();
        truncated.extend_from_slice(&[0u8; 30]);
        assert_eq!(parse_tzif(&truncated, 0), None);
    }

    /// 실제 시스템 TZif를 파싱할 수 있어야 한다(Unix 한정). 값 자체는 실행
    /// 환경에 따라 다르므로 "유효 범위 안"만 단언한다.
    #[cfg(unix)]
    #[test]
    fn parses_system_tzif_when_present() {
        if let Ok(data) = std::fs::read("/etc/localtime") {
            if let Some(tz) = parse_tzif(&data, 1_800_000_000) {
                assert!(
                    (-12 * 3600..=14 * 3600).contains(&tz.offset_secs),
                    "offset out of range: {}",
                    tz.offset_secs
                );
                assert!(!tz.abbrev.is_empty());
            }
        }
    }

    #[test]
    fn kst_is_recognized_as_the_kbo_baseline() {
        assert!(TimeZone::kst().is_kst());
        assert!(!TimeZone::from_offset(-4 * 3600).is_kst());
    }

    #[test]
    fn offset_abbrev_formats_hours_and_minutes() {
        assert_eq!(offset_abbrev(9 * 3600), "UTC+9");
        assert_eq!(offset_abbrev(-4 * 3600), "UTC-4");
        assert_eq!(offset_abbrev(5 * 3600 + 30 * 60), "UTC+5:30");
    }
}
