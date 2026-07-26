//! KST 날짜 산술(외부 크레이트 없이). main(CLI 해석)과 ui(옵션 픽커)가 공유한다.

/// civil_from_days (Howard Hinnant): UTC epoch(1970-01-01)로부터 지난 날짜 수
/// (`days`)를 그레고리력 (y, m, d)로 변환하는 순수 함수. kst_today()는 이
/// 함수를 감싸는 얇은 wrapper일 뿐이라, 여기서 알려진 경계값(윤년 2/29,
/// epoch 0)으로 직접 검증할 수 있다 — 출력 "모양"(길이/대시 위치)만 보는
/// 테스트로는 이 산술의 off-by-one(예: `days` 계산이 하루 밀림)을 못 잡는다.
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// civil_from_days의 역함수 (Howard Hinnant days_from_civil): 그레고리력
/// (y, m, d) → epoch(1970-01-01)로부터의 일수. resolve_date의 ±N 산술과
/// "실존 날짜" 왕복 검증에 쓴다.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn format_civil(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// UTC epoch 초 → KST 기준 epoch 일수.
pub fn kst_days(utc_secs: u64) -> i64 {
    (utc_secs as i64 + 9 * 3600).div_euclid(86400)
}

/// "HH:MM:SS" → 자정 기준 경과 초(자릿수·범위 검증만, 날짜 없음). 게임 시작
/// 시각(`ui::live_vm::elapsed_label`)과 예정 경기 카운트다운
/// (`ui::games::scheduled_eta_hm`, v0.15 A-3)이 공유하는 시:분:초 파서다 — 두
/// 곳 다 같은 "HH:MM:SS" 원문 포맷을 다루므로 파싱 자체는 공유하되, "자정
/// 넘김을 어떻게 보정할지"는 호출부마다 다르다(elapsed_label은 항상 미래
/// 방향이라 +24h 고정 보정이 안전하지만, scheduled_eta_hm은 날짜가 있는
/// 절대시각 비교라 이 값만으로 보정하면 안 된다 — games.rs 쪽 주석 참고).
///
/// v0.18~v0.19a까지는 `ui::live`/`ui::live_vm`에 있었다(M-1, v19a 리뷰) — 한
/// 화면의 표현 상태(ViewModel)와 무관한 범용 파서가 다른 화면(games.rs)의
/// 유틸 공급자가 되는 배치였다. 파싱 실패는 None(관용 — 표시 생략).
pub fn parse_hms_secs(hms: &str) -> Option<i64> {
    let mut it = hms.split(':');
    let h: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let s: i64 = it.next().unwrap_or("0").parse().ok()?;
    ((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&s))
        .then_some(h * 3600 + m * 60 + s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_converts_known_boundary_dates() {
        // epoch 0 == 1970-01-01.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2024-02-29 (윤년) == 19782 days since epoch (python: (date(2024,2,29) - date(1970,1,1)).days).
        assert_eq!(civil_from_days(19782), (2024, 2, 29));
        // 2027-01-01 == 20819 days since epoch.
        assert_eq!(civil_from_days(20819), (2027, 1, 1));
    }

    #[test]
    fn days_from_civil_roundtrips_with_civil_from_days() {
        for days in [0i64, 19782, 20819, 20657] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    /// M-1(v19a 리뷰): 이동 전엔 elapsed_label/pitch_interval_label 테스트를
    /// 통한 간접 커버뿐이었다 — 이동한 김에 파서 자체를 직접 잠근다.
    #[test]
    fn parse_hms_secs_converts_valid_time_to_seconds_since_midnight() {
        assert_eq!(parse_hms_secs("00:00:00"), Some(0));
        assert_eq!(parse_hms_secs("20:56:14"), Some(20 * 3600 + 56 * 60 + 14));
        assert_eq!(parse_hms_secs("23:59:59"), Some(23 * 3600 + 59 * 60 + 59));
    }

    #[test]
    fn parse_hms_secs_rejects_out_of_range_or_malformed_input() {
        assert_eq!(parse_hms_secs("24:00:00"), None, "시는 0..24만 유효");
        assert_eq!(parse_hms_secs("10:60:00"), None, "분은 0..60만 유효");
        assert_eq!(parse_hms_secs("10:00:60"), None, "초는 0..60만 유효");
        assert_eq!(parse_hms_secs("garbage"), None);
        assert_eq!(parse_hms_secs(""), None);
    }
}
