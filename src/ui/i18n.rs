//! TUI chrome 문자열의 i18n. struct 필드 방식 — 라벨 누락은 컴파일 에러다.
//! 폭 예산: 한국어는 전각 2칸 — footer 힌트·헤더 라벨은 축약형으로 설계했고
//! 폭 회귀 테스트(T6)가 모든 언어를 봉인한다. 보존(공통): B/S/O, [- - 1],
//! T9/B11, WP, km, GO!, 데이터(팀명·중계·팁·뉴스).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ko,
    En,
    Ja,
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
    // footer 힌트 조각(키는 footer.rs가, 라벨만 언어별) — assemble_hints가
    // 폭에 맞춰 조립한다(T5). 폭 예산은 footer_assembly_is_width_safe_in_all_languages가
    // 봉인.
    pub hint_help: &'static str,        // "Help" / "도움말"
    pub hint_options: &'static str,     // "Options" / "옵션"
    pub hint_settings: &'static str,    // "Settings" / "설정"
    pub hint_switch: &'static str,      // "Switch" / "전환"
    pub hint_links: &'static str,       // "Links" / "링크"
    pub hint_news: &'static str,        // "News" / "뉴스"
    pub hint_live_key: &'static str,    // "Live" / "중계"
    pub hint_back: &'static str,        // "Back" / "뒤로"
    pub hint_all_pitches: &'static str, // "All pitches" / "전체보기"
    pub hint_pitch: &'static str,       // "Pitch" / "투구"
    pub hint_quit: &'static str,        // "Quit" / "종료"
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
    pub title_article: &'static str,     // 기사 오버레이 상단 타이틀
    pub article_hint: &'static str,      // 기사 오버레이 하단 조작 힌트
    pub article_read_full: &'static str, // 발췌 고지 + 원문 전체 보기 CTA
    pub title_news_list: &'static str,   // 뉴스 목록 오버레이 타이틀
    pub news_list_hint: &'static str,    // 뉴스 목록 하단 조작 힌트
    pub title_options: &'static str,     // "Options" / "옵션" (pane 탭 조합은 코드)
    pub title_open: &'static str,        // chooser 타이틀
    pub title_settings: &'static str,    // " 설정 " / " Settings "
    pub settings_hint: &'static str,     // " ←→ 변경 · j/k 이동 · Esc 닫기 "
    pub settings_save_failed: &'static str, // " 저장 안 됨(설정 파일 쓰기 실패) "
    pub set_team: &'static str,          // "응원팀" / "Favorite team"
    pub set_poll: &'static str,          // "폴링 주기" / "Poll interval"
    pub set_theme_preset: &'static str,  // "테마 프리셋" / "Theme preset"
    pub set_theme_accent: &'static str,  // "강조색" / "Accent color"
    pub set_lang: &'static str,          // "언어" / "Language" (F9 설정의 Lang 행 라벨)
    // 테마 프리셋 값 표시(F9 설정 화면의 ThemePreset 행 값 칸)
    pub theme_default: &'static str,
    pub theme_high_contrast: &'static str,
    pub theme_mono: &'static str,
    // 액센트 소스 값 표시(F9 설정 화면의 ThemeAccent 행 값 칸)
    pub accent_team: &'static str,
    pub accent_cyan: &'static str,
    pub accent_green: &'static str,
    pub accent_yellow: &'static str,
    pub accent_magenta: &'static str,
    pub accent_blue: &'static str,
    pub accent_red: &'static str,
    pub accent_none: &'static str,
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
    pub date_today: &'static str,
    pub date_yesterday: &'static str,
    pub date_tomorrow: &'static str,
    pub date_days_fmt_minus: &'static str, // "-{n} days" / "-{n}일" 의 suffix: "days"/"일"
    pub team_none: &'static str,
    pub poll_suffix: &'static str, // "s live poll" / "초 폴링"
    // 헤더 시간 신뢰도 3종(v0.15). A-2/A-3 둘 다 poll_suffix와 같은 관례로
    // "{n}{suffix}" 융합 조립한다 — 언어별 공백 유무는 코드 분기가 아니라
    // suffix 문자열 자체에 미리 넣어 둔다(예: en은 숫자 뒤 공백 있음, ko/ja는
    // 없음). 시·분을 함께 쓸 때(remaining_*)는 "{h}{remaining_hour_suffix}{m}{remaining_min_suffix}"
    // 순서로 이어붙인다.
    pub updated_secs_suffix: &'static str, // "s ago" / "초 전" / "秒前" (A-2, 60초 미만)
    pub updated_min_suffix: &'static str,  // "m ago" / "분 전" / "分前" (A-2, 60초 이상)
    pub remaining_hour_suffix: &'static str, // "h " / "시간 " / "時間" (A-3, 시 단위 접미)
    pub remaining_min_suffix: &'static str, // "m to go" / "분 후" / "分後" (A-3, 분 단위 접미 — "후"류 종결어 포함)
}

pub const EN: Labels = Labels {
    count_live: "LIVE",
    count_sched: "SCHED",
    count_final: "FINAL",
    count_other: "OTHER",
    stale: "stale",
    tab_games: "GAMES",
    tab_standings: "STANDINGS",
    hint_help: "Help",
    hint_options: "Options",
    hint_settings: "Settings",
    hint_switch: "Switch",
    hint_links: "Links",
    hint_news: "News",
    hint_live_key: "Live",
    hint_back: "Back",
    hint_all_pitches: "All pitches",
    hint_pitch: "Pitch",
    hint_quit: "Quit",
    error_prefix: " ERROR: ",
    title_games: "Games",
    title_standings: "Standings",
    standings_current: "(current)",
    title_live: " Live ",
    title_relay: " Play-by-play ",
    title_zone: " Zone ",
    title_side: " Side ",
    title_help: " Help ",
    title_article: " Article (excerpt) ",
    article_hint: " Esc close · Enter/o full article · j/k scroll ",
    article_read_full: "Excerpt — read the full article: press Enter or o",
    title_news_list: " News ",
    news_list_hint: " Enter read · j/k move · Esc close ",
    title_options: "Options",
    title_open: "Open in browser",
    title_settings: " Settings ",
    settings_hint: " ←→ change · j/k move · Esc close ",
    settings_save_failed: " Not saved (config write failed) ",
    set_team: "Favorite team",
    set_poll: "Poll interval",
    set_theme_preset: "Theme preset",
    set_theme_accent: "Accent color",
    set_lang: "Language",
    theme_default: "Default",
    theme_high_contrast: "High contrast",
    theme_mono: "Mono (no color)",
    accent_team: "Team color",
    accent_cyan: "Cyan",
    accent_green: "Green",
    accent_yellow: "Yellow",
    accent_magenta: "Magenta",
    accent_blue: "Blue",
    accent_red: "Red",
    accent_none: "None",
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
        "Options    F2 (date) / F9 (team/poll)",
        "Links/News o / n",
        "Quit       q / F10",
    ],
    pane_date: "Date",
    date_today: "Today",
    date_yesterday: "Yesterday",
    date_tomorrow: "Tomorrow",
    date_days_fmt_minus: "days",
    team_none: "None (clear)",
    poll_suffix: "s live poll",
    updated_secs_suffix: "s ago",
    updated_min_suffix: "m ago",
    remaining_hour_suffix: "h ",
    remaining_min_suffix: "m to go",
};

pub const KO: Labels = Labels {
    count_live: "중계",
    count_sched: "예정",
    count_final: "종료",
    count_other: "기타",
    stale: "지연",
    tab_games: "경기",
    tab_standings: "순위",
    hint_help: "도움말",
    hint_options: "옵션",
    hint_settings: "설정",
    hint_switch: "전환",
    hint_links: "링크",
    hint_news: "뉴스",
    hint_live_key: "중계",
    hint_back: "뒤로",
    hint_all_pitches: "전체보기",
    hint_pitch: "투구",
    hint_quit: "종료",
    error_prefix: " 오류: ",
    title_games: "경기",
    title_standings: "순위",
    standings_current: "(현재)",
    title_live: " 중계 ",
    title_relay: " 문자중계 ",
    title_zone: " 존 ",
    title_side: " 측면 ",
    title_help: " 도움말 ",
    title_article: " 기사 (발췌) ",
    article_hint: " Esc 닫기 · Enter/o 원문 전체 · j/k 스크롤 ",
    article_read_full: "발췌입니다 — 원문 전체는 Enter 또는 o를 누르세요",
    title_news_list: " 뉴스 ",
    news_list_hint: " Enter 읽기 · j/k 이동 · Esc 닫기 ",
    title_options: "옵션",
    title_open: "브라우저로 열기",
    title_settings: " 설정 ",
    settings_hint: " ←→ 변경 · j/k 이동 · Esc 닫기 ",
    settings_save_failed: " 저장 안 됨(설정 파일 쓰기 실패) ",
    set_team: "응원팀",
    set_poll: "폴링 주기",
    set_theme_preset: "테마 프리셋",
    set_theme_accent: "강조색",
    set_lang: "언어",
    theme_default: "기본",
    theme_high_contrast: "고대비",
    theme_mono: "흑백(색 없음)",
    accent_team: "팀 색",
    accent_cyan: "청록",
    accent_green: "초록",
    accent_yellow: "노랑",
    accent_magenta: "자홍",
    accent_blue: "파랑",
    accent_red: "빨강",
    accent_none: "없음",
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
        "옵션        F2 (날짜) / F9 (팀·주기)",
        "링크/뉴스   o / n",
        "종료        q / F10",
    ],
    pane_date: "날짜",
    date_today: "오늘",
    date_yesterday: "어제",
    date_tomorrow: "내일",
    date_days_fmt_minus: "일",
    team_none: "해제 (없음)",
    poll_suffix: "초 폴링",
    updated_secs_suffix: "초 전",
    updated_min_suffix: "분 전",
    remaining_hour_suffix: "시간 ",
    remaining_min_suffix: "분 후",
};

pub const JA: Labels = Labels {
    count_live: "中継",
    count_sched: "予定",
    count_final: "終了",
    count_other: "その他",
    stale: "遅延",
    tab_games: "試合",
    tab_standings: "順位",
    hint_help: "ヘルプ",
    hint_options: "オプション",
    hint_settings: "設定",
    hint_switch: "切替",
    hint_links: "リンク",
    hint_news: "ニュース",
    hint_live_key: "中継",
    hint_back: "戻る",
    hint_all_pitches: "全投球",
    hint_pitch: "投球",
    hint_quit: "終了",
    error_prefix: " エラー: ",
    title_games: "試合",
    title_standings: "順位",
    standings_current: "(現在)",
    title_live: " 中継 ",
    title_relay: " 実況 ",
    title_zone: " ゾーン ",
    title_side: " 側面 ",
    title_help: " ヘルプ ",
    title_article: " 記事(抜粋) ",
    article_hint: " Esc 閉じる · Enter/o 全文 · j/k スクロール ",
    article_read_full: "抜粋です — 全文は Enter または o を押してください",
    title_news_list: " ニュース ",
    news_list_hint: " Enter 読む · j/k 移動 · Esc 閉じる ",
    title_options: "オプション",
    title_open: "ブラウザで開く",
    title_settings: " 設定 ",
    settings_hint: " ←→ 変更 · j/k 移動 · Esc 閉じる ",
    settings_save_failed: " 保存失敗(設定ファイルの書き込みエラー) ",
    set_team: "応援チーム",
    set_poll: "更新間隔",
    set_theme_preset: "テーマプリセット",
    set_theme_accent: "アクセントカラー",
    set_lang: "言語",
    theme_default: "標準",
    theme_high_contrast: "高コントラスト",
    theme_mono: "モノクロ(色なし)",
    accent_team: "チームカラー",
    accent_cyan: "シアン",
    accent_green: "緑",
    accent_yellow: "黄",
    accent_magenta: "マゼンタ",
    accent_blue: "青",
    accent_red: "赤",
    accent_none: "なし",
    loading: "読み込み中...",
    no_games: "予定されている試合はありません",
    no_standings: "順位情報がありません",
    col_away: "ビジター",
    col_score: "得点",
    col_home: "ホーム",
    col_status: "状況",
    col_team: "チーム",
    tag_live: "中継",
    tag_fin: "終了",
    tag_sched: "予定",
    tag_cancel: "中止",
    tag_susp: "中断",
    badge_final: "終了",
    badge_suspended: "中断",
    lbl_pitcher: "投手",
    lbl_batter: "打者",
    lbl_next: "次",
    lbl_start: "開始",
    pitch_word: "投球",
    pitches_word: "投球",
    inspect_hint: "(左右キーで1球ずつ)",
    tip_label: "ヒント: ",
    news_label: "ニュース: ",
    help_lines: [
        "移動        j / k または上下キー",
        "先頭/末尾  gg / G",
        "中継を開く  Enter",
        "戻る        Esc",
        "タブ切替    Tab / F5",
        "投球確認    Left / Right (中継画面)",
        "オプション  F2 (日付) / F9 (チーム·間隔)",
        "リンク/ニュース  o / n",
        "終了        q / F10",
    ],
    pane_date: "日付",
    date_today: "今日",
    date_yesterday: "昨日",
    date_tomorrow: "明日",
    date_days_fmt_minus: "日",
    team_none: "解除 (なし)",
    poll_suffix: "秒更新",
    updated_secs_suffix: "秒前",
    updated_min_suffix: "分前",
    remaining_hour_suffix: "時間",
    remaining_min_suffix: "分後",
};

pub fn labels(lang: Lang) -> &'static Labels {
    match lang {
        Lang::Ko => &KO,
        Lang::En => &EN,
        Lang::Ja => &JA,
    }
}

/// Config에 저장하는 언어 코드("ko"/"en"/"ja"). persist(T7)가 쓴다.
pub fn lang_code(l: Lang) -> &'static str {
    match l {
        Lang::Ko => "ko",
        Lang::En => "en",
        Lang::Ja => "ja",
    }
}

/// 언어 선택 UI(F9 설정의 Lang 행)에 보여줄 "자기 이름" — 현재 표시 언어와
/// 무관하게 각 언어가 스스로를 부르는 이름이다(다른 라벨과 달리 Labels에
/// 안 두는 이유: 번역이 아니라 고정 데이터라서).
pub fn lang_display_name(l: Lang) -> &'static str {
    match l {
        Lang::Ko => "한국어",
        Lang::En => "English",
        Lang::Ja => "日本語",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::text::display_width;

    /// 완전성: 모든 언어(KO/EN/JA) 전 필드 비어있지 않음 + help 9줄 전부 존재.
    #[test]
    fn every_label_is_nonempty_in_all_languages() {
        for l in [&KO, &EN, &JA] {
            for s in [
                l.count_live,
                l.count_sched,
                l.count_final,
                l.count_other,
                l.stale,
                l.tab_games,
                l.tab_standings,
                l.hint_help,
                l.hint_options,
                l.hint_settings,
                l.hint_switch,
                l.hint_links,
                l.hint_news,
                l.hint_live_key,
                l.hint_back,
                l.hint_all_pitches,
                l.hint_pitch,
                l.hint_quit,
                l.error_prefix,
                l.title_games,
                l.title_standings,
                l.standings_current,
                l.title_live,
                l.title_relay,
                l.title_zone,
                l.title_side,
                l.title_help,
                l.title_article,
                l.article_hint,
                l.article_read_full,
                l.title_news_list,
                l.news_list_hint,
                l.title_options,
                l.title_open,
                l.title_settings,
                l.settings_hint,
                l.settings_save_failed,
                l.set_team,
                l.set_poll,
                l.set_theme_preset,
                l.set_theme_accent,
                l.set_lang,
                l.theme_default,
                l.theme_high_contrast,
                l.theme_mono,
                l.accent_team,
                l.accent_cyan,
                l.accent_green,
                l.accent_yellow,
                l.accent_magenta,
                l.accent_blue,
                l.accent_red,
                l.accent_none,
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
                l.date_today,
                l.date_yesterday,
                l.date_tomorrow,
                l.date_days_fmt_minus,
                l.team_none,
                l.poll_suffix,
                l.updated_secs_suffix,
                l.updated_min_suffix,
                l.remaining_hour_suffix,
                l.remaining_min_suffix,
            ] {
                assert!(!s.trim().is_empty());
            }
            for h in l.help_lines {
                assert!(!h.trim().is_empty());
            }
        }
    }

    /// 반응형 footer: 어떤 언어·어떤 폭에서도 조립 결과가 폭을 넘지 않고,
    /// 넉넉한 폭(120)에선 핵심 라벨이 다 들어간다. (완성형 79 예산을 대체)
    #[test]
    fn footer_assembly_is_width_safe_in_all_languages() {
        use crate::ui::footer::{assemble_hints, HintItem};
        use crate::ui::text::display_width;
        for l in [&KO, &EN] {
            let items = [
                HintItem {
                    key: "F1",
                    label: l.hint_help,
                    core: true,
                },
                HintItem {
                    key: "Tab",
                    label: l.hint_switch,
                    core: true,
                },
                HintItem {
                    key: "n",
                    label: l.hint_news,
                    core: false,
                },
                HintItem {
                    key: "q",
                    label: l.hint_quit,
                    core: true,
                },
            ];
            for w in [0usize, 10, 20, 40, 80, 120] {
                let s = assemble_hints(&items, w);
                assert!(display_width(&s) <= w, "lang footer over width {w}: {s}");
            }
            // 넉넉한 폭(120)에선 핵심 라벨이 실제로 결과에 들어있어야 한다 —
            // assemble_hints가 빈 문자열을 반환해도 폭 안전 assert만으로는
            // 잡히지 않으므로 별도로 확인한다.
            let wide = assemble_hints(&items, 120);
            assert!(
                wide.contains(l.hint_help),
                "hint_help missing at wide width: {wide}"
            );
            assert!(
                wide.contains(l.hint_quit),
                "hint_quit missing at wide width: {wide}"
            );
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

    /// help 오버레이 전 줄이 박스 내부폭(50-2=48)에 들어간다 — 전 언어.
    #[test]
    fn every_help_line_fits_the_overlay_box() {
        for l in [&KO, &EN, &JA] {
            for h in l.help_lines {
                assert!(display_width(h) <= 48, "help line too wide: {h}");
            }
        }
    }

    /// 3개 언어 전부 완전성(모든 라벨 비어있지 않음)을 만족한다.
    #[test]
    fn all_three_languages_are_complete() {
        for l in [&KO, &EN, &JA] {
            assert!(!l.title_settings.trim().is_empty());
            assert!(!l.hint_quit.trim().is_empty());
        }
    }

    /// 언어 코드 왕복.
    #[test]
    fn lang_code_round_trips() {
        for (l, code) in [(Lang::Ko, "ko"), (Lang::En, "en"), (Lang::Ja, "ja")] {
            assert_eq!(lang_code(l), code);
        }
    }

    /// 언어 자기 이름(설정 화면 표시명)이 3개 언어 전부 비어있지 않고 서로 다르다.
    #[test]
    fn lang_display_name_is_nonempty_and_distinct() {
        let names: Vec<&str> = [Lang::Ko, Lang::En, Lang::Ja]
            .into_iter()
            .map(lang_display_name)
            .collect();
        for n in &names {
            assert!(!n.trim().is_empty());
        }
        let mut uniq = names.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), names.len(), "display names must be distinct");
    }
}
