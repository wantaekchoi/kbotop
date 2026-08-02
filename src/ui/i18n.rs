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
    pub hint_rewind: &'static str,      // "Rewind" / "돌려보기" ([ ]로 과거 타석 이동, v0.18)
    pub hint_go_live: &'static str, // "Live" / "라이브로" (Esc: 과거 타석 보기 → 최신 복귀, v0.18)
    /// "Relay" / "중계" (footer j/k 힌트, v0.18 리뷰 I-5 — 문자중계 줄 커서를
    /// 발견할 방법이 어디에도 없었다).
    pub hint_relay: &'static str,
    /// "Latest" / "최신" (Esc: 문자중계 커서만 있을 때, v0.18 리뷰 M-5 — 이전엔
    /// 이 상태에서도 투구 전용 문구인 hint_all_pitches를 재사용해 라벨이 실제
    /// 상태와 안 맞았다).
    pub hint_latest: &'static str,
    pub error_prefix: &'static str,
    /// 설정 파일을 못 읽었을 때 띄우는 문구(v0.28). 뒤에 파서 메시지가 붙는다.
    /// 이 상태에서는 저장도 막으므로 "고칠 때까지 안 건드린다"는 뜻까지 담는다.
    pub config_broken: &'static str,
    // 블록 타이틀 조각
    pub title_games: &'static str,     // " {t} {date} " 조합
    pub title_standings: &'static str, // " {t} {year} {current} "
    pub standings_current: &'static str,
    pub title_live: &'static str, // 완성형 " ... "
    pub title_relay: &'static str,
    /// 과거 이닝을 받아오는 동안 문자중계 블록 타이틀에 덧붙이는 문구(v0.20).
    /// `{}` 자리에 이닝 번호가 들어간다 — 어느 이닝을 기다리는지 보여야
    /// 사용자가 "멈춘 건지 받는 중인지"를 구분한다.
    pub loading_inning: &'static str,
    /// 돌려보기(v0.18) 중 라이브 타이틀 대신 쓰는 접두("Rewind"/"돌려보기") —
    /// 과거 타석의 이닝·타자명과 함께 조합해 라이브와 헷갈리지 않게 한다.
    pub rewind_label: &'static str,
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
    pub set_mouse: &'static str,         // "마우스" / "Mouse" (F9 설정의 Mouse 행 라벨)
    pub on: &'static str,                // 켬/끔 — 지금은 마우스 행만 쓴다
    pub off: &'static str,
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
    /// 대결 요약 블록(v0.26). 좁은 칸에 들어가므로 어느 언어에서도 짧게.
    pub title_matchup: &'static str,
    pub matchup_batter: &'static str,
    pub matchup_pitcher: &'static str,
    pub matchup_career: &'static str,
    pub matchup_innings: &'static str,
    pub matchup_hits: &'static str,
    pub matchup_pitches: &'static str,
    /// 팀 성적 오버레이(v0.24). 라벨 칸은 14칸이라 어느 언어에서도 짧게 둔다.
    pub title_team_stats: &'static str,
    pub team_stats_hint: &'static str,
    /// 순위 탭 footer의 Enter 힌트(v0.30). 팀 성적 오버레이는 v0.24에 들어왔는데
    /// footer에도 도움말에도 안 적혀 있어 아는 사람만 쓰는 기능이었다.
    pub hint_team_stats: &'static str,
    /// 연속 기록(STRK) 칸의 접미(v0.30). 서버는 "3승"·"1패"·"1무"처럼 **한국어로**
    /// 준다 — 영어·일본어 화면에서 이 칸만 한글로 남아 있었다. 숫자와 접미를
    /// 갈라 여기서 갈아 끼운다(형식이 달라지면 원문 그대로 둔다).
    pub streak_win: &'static str,
    pub streak_loss: &'static str,
    pub streak_draw: &'static str,
    pub stats_batting: &'static str,
    pub stats_pitching: &'static str,
    pub stat_avg: &'static str,
    pub stat_obp: &'static str,
    pub stat_slg: &'static str,
    pub stat_ops: &'static str,
    pub stat_runs: &'static str,
    pub stat_rbi: &'static str,
    pub stat_hr: &'static str,
    pub stat_sb: &'static str,
    pub stat_era: &'static str,
    pub stat_whip: &'static str,
    pub stat_qs: &'static str,
    pub stat_save: &'static str,
    pub stat_hold: &'static str,
    pub stat_so: &'static str,
    pub stat_hr_allowed: &'static str,
    pub stat_err: &'static str,
    pub col_starters: &'static str,
    pub col_venue: &'static str,
    /// 순위표 최근 5경기·연속 기록 칼럼 헤더(v0.23). 폭이 빡빡한 칼럼이라
    /// 어느 언어에서도 짧게 둔다.
    pub col_last_five: &'static str,
    pub col_streak: &'static str,
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
    // 경기 경과/소요(v0.18 B-3, live.rs::game_duration_label 전용)
    pub lbl_elapsed: &'static str, // "Elapsed" / "경과" (진행 중 경기, 시작~지금)
    pub lbl_duration: &'static str, // "Duration" / "소요" (종료/중단 경기, 시작~마지막 투구)
    // 투구 간격(v0.18 B-2, live.rs::pitch_interval_label 전용). "{n}{suffix}"
    // 조립 관례는 poll_suffix 등과 동일 — 60초 이상은 이 접미 없이 "+M:SS"로
    // 표기한다(elapsed_label과 같은 표기 관례).
    pub pitch_interval_secs_suffix: &'static str, // "초" / "s" / "秒"
    // 티커
    pub tip_label: &'static str,  // "Tip: " / "팁: "
    pub news_label: &'static str, // "News: " / "뉴스: "
    // help 오버레이(순서 고정 11줄, v0.18에서 Rewind·Relay 두 줄 추가)
    pub help_lines: [&'static str; 12],
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
    // 뉴스 발행 경과(v0.18 B-1, newslist.rs 전용). header의 A-2(초/분 2단계)와
    // 달리 뉴스는 초 단위 정밀도가 필요 없어 "분 미만/분/시간/일" 4단계다 —
    // "{n}{suffix}" 조립 관례(poll_suffix 등과 동일)는 그대로 유지.
    pub news_age_now: &'static str, // "방금" / "Just now" / "たった今" (1분 미만 전부)
    pub news_age_min_suffix: &'static str, // "분 전" / "m ago" / "分前" (1분~59분)
    pub news_age_hour_suffix: &'static str, // "시간 전" / "h ago" / "時間前" (1시간~23시간)
    pub news_age_day_suffix: &'static str, // "일 전" / "d ago" / "日前" (1일 이상)
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
    hint_rewind: "Rewind",
    hint_go_live: "Live",
    hint_relay: "Relay",
    hint_latest: "Latest",
    error_prefix: " ERROR: ",
    config_broken: " config.toml could not be read, using defaults (not overwriting it): ",
    title_games: "Games",
    title_standings: "Standings",
    standings_current: "(current)",
    title_live: " Live ",
    title_relay: " Play-by-play ",
    loading_inning: "loading inning {}",
    rewind_label: "Rewind",
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
    set_mouse: "Mouse",
    on: "on",
    off: "off",
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
    title_matchup: " Matchup ",
    matchup_batter: "B",
    matchup_pitcher: "P",
    matchup_career: "vs",
    matchup_innings: "ip",
    matchup_hits: "h",
    matchup_pitches: "p",
    title_team_stats: "season stats",
    team_stats_hint: " Esc close ",
    hint_team_stats: "Stats",
    streak_win: "W",
    streak_loss: "L",
    streak_draw: "D",
    stats_batting: "Batting",
    stats_pitching: "Pitching",
    stat_avg: "AVG",
    stat_obp: "OBP",
    stat_slg: "SLG",
    stat_ops: "OPS",
    stat_runs: "Runs",
    stat_rbi: "RBI",
    stat_hr: "HR",
    stat_sb: "SB",
    stat_era: "ERA",
    stat_whip: "WHIP",
    stat_qs: "QS",
    stat_save: "Saves",
    stat_hold: "Holds",
    stat_so: "SO",
    stat_hr_allowed: "HR allowed",
    stat_err: "Errors",
    col_starters: "Starters",
    col_venue: "Venue",
    col_last_five: "L5",
    col_streak: "STRK",
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
    lbl_elapsed: "Elapsed",
    lbl_duration: "Duration",
    pitch_interval_secs_suffix: "s",
    tip_label: "Tip: ",
    news_label: "News: ",
    help_lines: [
        "Move       j / k or Up / Down",
        "Top/Bottom gg / G",
        "Open       Enter (games=relay, standings=stats)",
        "Back       Esc",
        "Switch tab Tab / F5",
        "Pitch      Left / Right (live view)",
        "Rewind     [ / ] (live view, loads past innings)",
        "Relay      j / k, gg / G (live view)",
        "Options    F2 (date) / F9 (settings)",
        "Links/News o / n",
        "Mouse      click / wheel (click again to open)",
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
    news_age_now: "Just now",
    news_age_min_suffix: "m ago",
    news_age_hour_suffix: "h ago",
    news_age_day_suffix: "d ago",
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
    hint_rewind: "돌려보기",
    hint_go_live: "라이브로",
    hint_relay: "중계",
    hint_latest: "최신",
    error_prefix: " 오류: ",
    config_broken: " 설정 파일을 못 읽어 기본값으로 실행합니다(파일은 그대로 둡니다): ",
    title_games: "경기",
    title_standings: "순위",
    standings_current: "(현재)",
    title_live: " 중계 ",
    title_relay: " 문자중계 ",
    loading_inning: "{}회 불러오는 중",
    rewind_label: "돌려보기",
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
    set_mouse: "마우스",
    on: "켬",
    off: "끔",
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
    title_matchup: " 이 대결 ",
    matchup_batter: "타자",
    matchup_pitcher: "투수",
    matchup_career: "통산",
    matchup_innings: "이닝",
    matchup_hits: "피안타",
    matchup_pitches: "구",
    title_team_stats: "시즌 성적",
    team_stats_hint: " Esc 닫기 ",
    hint_team_stats: "성적",
    streak_win: "승",
    streak_loss: "패",
    streak_draw: "무",
    stats_batting: "타격",
    stats_pitching: "투구·수비",
    stat_avg: "타율",
    stat_obp: "출루율",
    stat_slg: "장타율",
    stat_ops: "OPS",
    stat_runs: "득점",
    stat_rbi: "타점",
    stat_hr: "홈런",
    stat_sb: "도루",
    stat_era: "평균자책",
    stat_whip: "WHIP",
    stat_qs: "QS",
    stat_save: "세이브",
    stat_hold: "홀드",
    stat_so: "탈삼진",
    stat_hr_allowed: "피홈런",
    stat_err: "실책",
    col_starters: "선발",
    col_venue: "구장",
    col_last_five: "최근5",
    col_streak: "연속",
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
    lbl_elapsed: "경과",
    lbl_duration: "소요",
    pitch_interval_secs_suffix: "초",
    tip_label: "팁: ",
    news_label: "뉴스: ",
    help_lines: [
        "이동        j / k 또는 방향키",
        "맨위/맨아래 gg / G",
        "열기        Enter (경기=중계, 순위=성적)",
        "뒤로        Esc",
        "탭 전환     Tab / F5",
        "투구 보기   좌우 방향키 (중계 화면)",
        "돌려보기    [ / ] (중계 화면, 지난 이닝까지)",
        "중계 커서   j / k · gg / G (중계 화면)",
        "옵션        F2 (날짜) / F9 (설정)",
        "링크/뉴스   o / n",
        "마우스      클릭·휠 (다시 클릭하면 열기)",
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
    news_age_now: "방금",
    news_age_min_suffix: "분 전",
    news_age_hour_suffix: "시간 전",
    news_age_day_suffix: "일 전",
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
    hint_rewind: "巻き戻し",
    hint_go_live: "ライブへ",
    hint_relay: "実況",
    hint_latest: "最新",
    error_prefix: " エラー: ",
    config_broken: " 設定ファイルを読めず既定値で動作します(ファイルはそのまま): ",
    title_games: "試合",
    title_standings: "順位",
    standings_current: "(現在)",
    title_live: " 中継 ",
    title_relay: " 実況 ",
    loading_inning: "{}回を読み込み中",
    rewind_label: "巻き戻し",
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
    set_mouse: "マウス",
    on: "オン",
    off: "オフ",
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
    title_matchup: " この対戦 ",
    matchup_batter: "打者",
    matchup_pitcher: "投手",
    matchup_career: "通算",
    matchup_innings: "回",
    matchup_hits: "被安打",
    matchup_pitches: "球",
    title_team_stats: "シーズン成績",
    team_stats_hint: " Esc 閉じる ",
    hint_team_stats: "成績",
    streak_win: "勝",
    streak_loss: "敗",
    streak_draw: "分",
    stats_batting: "打撃",
    stats_pitching: "投球·守備",
    stat_avg: "打率",
    stat_obp: "出塁率",
    stat_slg: "長打率",
    stat_ops: "OPS",
    stat_runs: "得点",
    stat_rbi: "打点",
    stat_hr: "本塁打",
    stat_sb: "盗塁",
    stat_era: "防御率",
    stat_whip: "WHIP",
    stat_qs: "QS",
    stat_save: "セーブ",
    stat_hold: "ホールド",
    stat_so: "奪三振",
    stat_hr_allowed: "被本塁打",
    stat_err: "失策",
    col_starters: "先発",
    col_venue: "球場",
    col_last_five: "直近5",
    col_streak: "連続",
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
    lbl_elapsed: "経過",
    lbl_duration: "所要時間",
    pitch_interval_secs_suffix: "秒",
    tip_label: "ヒント: ",
    news_label: "ニュース: ",
    help_lines: [
        "移動        j / k または上下キー",
        "先頭/末尾  gg / G",
        "開く        Enter (試合=中継・順位=成績)",
        "戻る        Esc",
        "タブ切替    Tab / F5",
        "投球確認    Left / Right (中継画面)",
        "巻き戻し    [ / ] (中継画面·前の回まで)",
        "実況        j / k · gg / G (中継画面)",
        "オプション  F2 (日付) / F9 (設定)",
        "リンク/ニュース  o / n",
        "マウス      クリック·ホイール(再クリックで開く)",
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
    news_age_now: "たった今",
    news_age_min_suffix: "分前",
    news_age_hour_suffix: "時間前",
    news_age_day_suffix: "日前",
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

    /// 완전성: 모든 언어(KO/EN/JA) 전 필드 비어있지 않음 + help_lines 전 줄 존재.
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
                l.lbl_elapsed,
                l.lbl_duration,
                l.pitch_interval_secs_suffix,
                l.hint_rewind,
                l.hint_go_live,
                l.hint_relay,
                l.hint_latest,
                l.rewind_label,
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
                l.news_age_now,
                l.news_age_min_suffix,
                l.news_age_hour_suffix,
                l.news_age_day_suffix,
                l.config_broken,
                l.loading_inning,
                l.set_mouse,
                l.on,
                l.off,
                l.title_matchup,
                l.matchup_batter,
                l.matchup_pitcher,
                l.matchup_career,
                l.matchup_innings,
                l.matchup_hits,
                l.matchup_pitches,
                l.title_team_stats,
                l.team_stats_hint,
                l.hint_team_stats,
                l.streak_win,
                l.streak_loss,
                l.streak_draw,
                l.stats_batting,
                l.stats_pitching,
                l.stat_avg,
                l.stat_obp,
                l.stat_slg,
                l.stat_ops,
                l.stat_runs,
                l.stat_rbi,
                l.stat_hr,
                l.stat_sb,
                l.stat_era,
                l.stat_whip,
                l.stat_qs,
                l.stat_save,
                l.stat_hold,
                l.stat_so,
                l.stat_hr_allowed,
                l.stat_err,
                l.col_starters,
                l.col_venue,
                l.col_last_five,
                l.col_streak,
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
        for l in [&KO, &EN, &JA] {
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
        for l in [&KO, &EN, &JA] {
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
    /// **완전성 검사가 실제로 모든 라벨을 보는가.**
    ///
    /// `every_label_is_nonempty_in_all_languages`는 필드를 손으로 나열한다.
    /// 그래서 v0.23 이후 추가된 라벨 서른다섯 개(팀 성적 16종, 대결 블록 7종,
    /// 마우스 3종 …)가 **한 번도 검사받지 않았다** — 빈 문자열로 번역해 두어도
    /// 아무도 몰랐다. 나열을 자동화할 수는 없으니, 나열이 빠짐없는지를 검사한다.
    #[test]
    fn the_completeness_check_covers_every_label_field() {
        const SRC: &str = include_str!("i18n.rs");
        let struct_start = SRC.find("pub struct Labels {").expect("struct를 못 찾았다");
        let struct_end = struct_start + SRC[struct_start..].find("\n}").expect("끝을 못 찾았다");
        let fields: Vec<&str> = SRC[struct_start..struct_end]
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub "))
            .filter(|l| l.contains("&'static str,")) // 배열(help_lines)은 별도 테스트가 본다
            .filter_map(|l| l.split(':').next())
            .map(str::trim)
            .collect();
        assert!(fields.len() > 80, "필드 파싱이 깨졌다: {}", fields.len());

        let fn_start = SRC
            .find("fn every_label_is_nonempty_in_all_languages()")
            .expect("완전성 테스트를 못 찾았다");
        let fn_end = fn_start
            + SRC[fn_start..]
                .find("\n    }")
                .expect("함수 끝을 못 찾았다");
        let body = &SRC[fn_start..fn_end];

        let missing: Vec<&str> = fields
            .iter()
            .filter(|f| !body.contains(&format!("l.{f},")))
            .copied()
            .collect();
        assert!(
            missing.is_empty(),
            "완전성 검사가 빠뜨린 라벨 {}개: {:?}",
            missing.len(),
            missing
        );
    }
}
