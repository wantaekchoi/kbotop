//! TUI chrome 문자열의 i18n. struct 필드 방식 — 라벨 누락은 컴파일 에러다.
//! 폭 예산: 한국어는 전각 2칸 — footer 힌트·헤더 라벨은 축약형으로 설계했고
//! 폭 회귀 테스트(T6)가 두 언어 모두 봉인한다. 보존(공통): B/S/O, [- - 1],
//! T9/B11, WP, km, GO!, 데이터(팀명·중계·팁·뉴스).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
}

pub struct Labels {
    // 헤더 1행 카운트
    pub count_live: &'static str,
    pub count_sched: &'static str,
    pub count_final: &'static str,
    pub count_other: &'static str,
    pub stale: &'static str,
    // 헤더 2행 탭 — 활성 "[ {t} ]" / 비활성 "  {t}  " 조합은 코드가 한다
    pub tab_games: &'static str,
    pub tab_standings: &'static str,
    // footer 힌트(상태별 완성형 문자열 — 폭 검증 대상)
    pub hint_list_games: &'static str,
    pub hint_list_standings: &'static str,
    pub hint_live: &'static str,
    pub hint_live_selected: &'static str,
    pub error_prefix: &'static str,
    // 블록 타이틀 조각
    pub title_games: &'static str,     // " {t} {date} " 조합
    pub title_standings: &'static str, // " {t} {year} {current} "
    pub standings_current: &'static str,
    pub title_live: &'static str, // 완성형 " ... "
    pub title_relay: &'static str,
    pub title_zone: &'static str,
    pub title_side: &'static str,
    pub title_help: &'static str,
    pub title_options: &'static str, // "Options" / "옵션" (pane 탭 조합은 코드)
    pub title_open: &'static str,    // chooser 타이틀
    // 상태 문구
    pub loading: &'static str,
    pub no_games: &'static str,
    pub no_standings: &'static str,
    // games 테이블 헤더 · 상태 태그(Table 셀 — 짧게 유지)
    pub col_away: &'static str,
    pub col_score: &'static str,
    pub col_home: &'static str,
    pub col_status: &'static str,
    pub col_team: &'static str,
    pub tag_live: &'static str,
    pub tag_fin: &'static str,
    pub tag_sched: &'static str,
    pub tag_cancel: &'static str,
    pub tag_susp: &'static str,
    // 라이브 배지·라벨
    pub badge_final: &'static str,
    pub badge_suspended: &'static str,
    pub lbl_pitcher: &'static str,  // "P" / "투수"
    pub lbl_batter: &'static str,   // "B" / "타자"
    pub lbl_next: &'static str,     // "Next" / "다음"
    pub lbl_start: &'static str,    // "Start" / "시작"
    pub pitch_word: &'static str,   // "Pitch" / "투구"
    pub pitches_word: &'static str, // "Pitches" / "투구"
    pub inspect_hint: &'static str, // "(Left/Right to inspect)" / "(좌우 키로 하나씩)"
    // 티커
    pub tip_label: &'static str,  // "Tip: " / "팁: "
    pub news_label: &'static str, // "News: " / "뉴스: "
    // help 오버레이(순서 고정 9줄)
    pub help_lines: [&'static str; 9],
    // F2 픽커
    pub pane_date: &'static str,
    pub pane_team: &'static str,
    pub pane_poll: &'static str,
    pub date_today: &'static str,
    pub date_yesterday: &'static str,
    pub date_tomorrow: &'static str,
    pub date_days_fmt_minus: &'static str, // "-{n} days" / "-{n}일" 의 suffix: "days"/"일"
    pub team_none: &'static str,
    pub poll_suffix: &'static str, // "s live poll" / "초 폴링"
}

pub const EN: Labels = Labels {
    count_live: "LIVE",
    count_sched: "SCHED",
    count_final: "FINAL",
    count_other: "OTHER",
    stale: "stale",
    tab_games: "GAMES",
    tab_standings: "STANDINGS",
    hint_list_games: " F1 Help   F2 Options   Tab Switch   o Links   n News   Enter Live   q Quit",
    hint_list_standings: " F1 Help   F2 Options   Tab Switch   o Links   n News   q Quit",
    hint_live: " F1 Help   Esc Back   Left/Right Pitch   q Quit",
    hint_live_selected: " F1 Help   Esc All pitches   Left/Right Pitch   q Quit",
    error_prefix: " ERROR: ",
    title_games: "Games",
    title_standings: "Standings",
    standings_current: "(current)",
    title_live: " Live ",
    title_relay: " Play-by-play ",
    title_zone: " Zone ",
    title_side: " Side ",
    title_help: " Help ",
    title_options: "Options",
    title_open: "Open in browser",
    loading: "loading...",
    no_games: "No games scheduled",
    no_standings: "No standings available",
    col_away: "Away",
    col_score: "Score",
    col_home: "Home",
    col_status: "Status",
    col_team: "Team",
    tag_live: "LIVE",
    tag_fin: "FIN",
    tag_sched: "SCHED",
    tag_cancel: "CANCEL",
    tag_susp: "SUSP",
    badge_final: "FINAL",
    badge_suspended: "SUSPENDED",
    lbl_pitcher: "P",
    lbl_batter: "B",
    lbl_next: "Next",
    lbl_start: "Start",
    pitch_word: "Pitch",
    pitches_word: "Pitches",
    inspect_hint: "(Left/Right to inspect)",
    tip_label: "Tip: ",
    news_label: "News: ",
    help_lines: [
        "Move       j / k or Up / Down",
        "Top/Bottom gg / G",
        "Open live  Enter",
        "Back       Esc",
        "Switch tab Tab / F5",
        "Pitch      Left / Right (live view)",
        "Options    F2 (date / team / poll)",
        "Links/News o / n",
        "Quit       q / F10",
    ],
    pane_date: "Date",
    pane_team: "Team",
    pane_poll: "Poll",
    date_today: "Today",
    date_yesterday: "Yesterday",
    date_tomorrow: "Tomorrow",
    date_days_fmt_minus: "days",
    team_none: "None (clear)",
    poll_suffix: "s live poll",
};

pub const KO: Labels = Labels {
    count_live: "중계",
    count_sched: "예정",
    count_final: "종료",
    count_other: "기타",
    stale: "지연",
    tab_games: "경기",
    tab_standings: "순위",
    hint_list_games: " F1 도움말  F2 옵션  Tab 전환  o 링크  n 뉴스  Enter 중계  q 종료",
    hint_list_standings: " F1 도움말  F2 옵션  Tab 전환  o 링크  n 뉴스  q 종료",
    hint_live: " F1 도움말  Esc 뒤로  좌우 투구  q 종료",
    hint_live_selected: " F1 도움말  Esc 전체보기  좌우 투구  q 종료",
    error_prefix: " 오류: ",
    title_games: "경기",
    title_standings: "순위",
    standings_current: "(현재)",
    title_live: " 중계 ",
    title_relay: " 문자중계 ",
    title_zone: " 존 ",
    title_side: " 측면 ",
    title_help: " 도움말 ",
    title_options: "옵션",
    title_open: "브라우저로 열기",
    loading: "불러오는 중...",
    no_games: "예정된 경기가 없습니다",
    no_standings: "순위 정보가 없습니다",
    col_away: "원정",
    col_score: "점수",
    col_home: "홈",
    col_status: "상태",
    col_team: "팀",
    tag_live: "중계",
    tag_fin: "종료",
    tag_sched: "예정",
    tag_cancel: "취소",
    tag_susp: "중단",
    badge_final: "종료",
    badge_suspended: "중단",
    lbl_pitcher: "투수",
    lbl_batter: "타자",
    lbl_next: "다음",
    lbl_start: "시작",
    pitch_word: "투구",
    pitches_word: "투구",
    inspect_hint: "(좌우 키로 하나씩)",
    tip_label: "팁: ",
    news_label: "뉴스: ",
    help_lines: [
        "이동        j / k 또는 방향키",
        "맨위/맨아래 gg / G",
        "중계 열기   Enter",
        "뒤로        Esc",
        "탭 전환     Tab / F5",
        "투구 보기   좌우 방향키 (중계 화면)",
        "옵션        F2 (날짜 / 팀 / 주기)",
        "링크/뉴스   o / n",
        "종료        q / F10",
    ],
    pane_date: "날짜",
    pane_team: "팀",
    pane_poll: "주기",
    date_today: "오늘",
    date_yesterday: "어제",
    date_tomorrow: "내일",
    date_days_fmt_minus: "일",
    team_none: "해제 (없음)",
    poll_suffix: "초 폴링",
};

pub fn labels(lang: Lang) -> &'static Labels {
    match lang {
        Lang::Ko => &KO,
        Lang::En => &EN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::text::display_width;

    /// 완전성: 두 언어 전 필드 비어있지 않음 + help 9줄 전부 존재.
    #[test]
    fn every_label_is_nonempty_in_both_languages() {
        for l in [&KO, &EN] {
            for s in [
                l.count_live,
                l.count_sched,
                l.count_final,
                l.count_other,
                l.stale,
                l.tab_games,
                l.tab_standings,
                l.hint_list_games,
                l.hint_list_standings,
                l.hint_live,
                l.hint_live_selected,
                l.error_prefix,
                l.title_games,
                l.title_standings,
                l.standings_current,
                l.title_live,
                l.title_relay,
                l.title_zone,
                l.title_side,
                l.title_help,
                l.title_options,
                l.title_open,
                l.loading,
                l.no_games,
                l.no_standings,
                l.col_away,
                l.col_score,
                l.col_home,
                l.col_status,
                l.col_team,
                l.tag_live,
                l.tag_fin,
                l.tag_sched,
                l.tag_cancel,
                l.tag_susp,
                l.badge_final,
                l.badge_suspended,
                l.lbl_pitcher,
                l.lbl_batter,
                l.lbl_next,
                l.lbl_start,
                l.pitch_word,
                l.pitches_word,
                l.inspect_hint,
                l.tip_label,
                l.news_label,
                l.pane_date,
                l.pane_team,
                l.pane_poll,
                l.date_today,
                l.date_yesterday,
                l.date_tomorrow,
                l.date_days_fmt_minus,
                l.team_none,
                l.poll_suffix,
            ] {
                assert!(!s.trim().is_empty());
            }
            for h in l.help_lines {
                assert!(!h.trim().is_empty());
            }
        }
    }

    /// 폭 예산: footer 힌트 전 상태가 두 언어 모두 79칸 이하.
    #[test]
    fn every_footer_hint_fits_80_columns() {
        for l in [&KO, &EN] {
            for h in [
                l.hint_list_games,
                l.hint_list_standings,
                l.hint_live,
                l.hint_live_selected,
            ] {
                assert!(
                    display_width(h) <= 79,
                    "hint too wide ({}): {h}",
                    display_width(h)
                );
            }
        }
    }

    /// 탭 라벨: 활성 "[ t ]"과 비활성 "  t  "의 폭이 언어별로 동일(레이아웃 불변).
    #[test]
    fn tab_labels_keep_symmetric_width_per_language() {
        for l in [&KO, &EN] {
            for t in [l.tab_games, l.tab_standings] {
                assert_eq!(
                    display_width(&format!("[ {t} ]")),
                    display_width(&format!("  {t}  "))
                );
            }
        }
    }

    /// help 오버레이 전 줄이 박스 내부폭(50-2=48)에 들어간다 — 두 언어 모두.
    #[test]
    fn every_help_line_fits_the_overlay_box() {
        for l in [&KO, &EN] {
            for h in l.help_lines {
                assert!(display_width(h) <= 48, "help line too wide: {h}");
            }
        }
    }
}
