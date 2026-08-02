use crate::config::Config;
use std::collections::{BTreeMap, HashMap};

use crate::model::{AtBat, Game, GameStatus, LiveState, NewsItem, Standing};
use crate::poller::Update;
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    Standings,
}

/// F2 옵션 픽커의 pane. v0.8부터 Date 전용(Team·Poll은 F9 설정으로 이동).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Date,
}

/// F2 옵션 오버레이가 열려 있는 동안의 상태(어느 pane, 커서 위치).
pub struct OptionsState {
    pub pane: Pane,
    pub cursor: usize,
}

/// `o` 링크 픽커가 열려 있는 동안의 상태.
pub struct LinkPickerState {
    pub items: Vec<(String, String)>, // (라벨, URL)
    pub cursor: usize,
}

/// 인앱 뉴스 발췌 오버레이 상태(v0.7). 선택한 항목을 그대로 들고 있으므로
/// 비동기 fetch가 없다 — 열면 즉시 렌더된다.
pub struct ArticleView {
    pub item: crate::model::NewsItem,
    pub scroll: u16,
}

/// 뉴스 목록 오버레이 상태(v0.7). 기사 오버레이가 이 위에 겹칠 수 있다 —
/// Esc는 기사 → 목록 → 닫힘 순으로 한 단계씩 올라온다.
pub struct NewsListState {
    pub cursor: usize,
}

/// 설정 화면 상태(v0.8). F9로 열고, 변경은 즉시 config에 저장한다. save_failed는
/// 마지막 저장이 실패했는지(읽기전용 FS 등) — 화면 하단에 고지한다.
pub struct SettingsState {
    pub cursor: usize,
    pub save_failed: bool,
}

/// 설정 행의 종류. 값 변경은 커서 인덱스가 아니라 이 종류로 분기해, 뒤 태스크가
/// 행을 추가해도 분기가 안 밀린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKind {
    Team,
    Poll,
    ThemePreset,
    ThemeAccent,
    Lang,
    /// 마우스 캡처(v0.27). 끄면 터미널의 드래그 선택·복사가 돌아온다.
    Mouse,
}

/// 언어 순환 순서(F9 설정 화면의 Lang 행). change_setting이 이 순서로 순환한다.
const LANGS: [crate::ui::i18n::Lang; 3] = [
    crate::ui::i18n::Lang::Ko,
    crate::ui::i18n::Lang::En,
    crate::ui::i18n::Lang::Ja,
];

/// 테마 프리셋 순환 순서(F9 설정 화면의 ThemePreset 행). 단일 진실 — change_setting과
/// theme_preset_label이 함께 참조한다.
const THEME_PRESETS: [&str; 3] = ["default", "high-contrast", "mono"];

/// 액센트 소스 순환 순서(F9 설정 화면의 ThemeAccent 행). ui::theme::accent_for가
/// 해석하는 문자열과 반드시 일치해야 한다.
const THEME_ACCENTS: [&str; 8] = [
    "team", "cyan", "green", "yellow", "magenta", "blue", "red", "none",
];

/// 테마 프리셋 값을 설정 화면에 보여줄 라벨로 바꾼다. 알 수 없는 값(구버전
/// config 잔재 등)은 관용적으로 "default" 라벨로 표시한다.
fn theme_preset_label(l: &crate::ui::i18n::Labels, preset: &str) -> &'static str {
    match preset {
        "high-contrast" => l.theme_high_contrast,
        "mono" => l.theme_mono,
        _ => l.theme_default,
    }
}

/// 액센트 값을 설정 화면에 보여줄 라벨로 바꾼다.
///
/// config에 적어 둔 16진 색(v0.22)은 **그 값을 그대로** 보여준다. v0.21까지는
/// 모르는 값을 전부 "team" 라벨로 뭉갰는데, 그러면 `#ff6600`을 적어 둔 사용자가
/// 설정 화면에서 "팀 컬러"를 보고 자기 설정이 사라진 줄 안다 — 화면이 실제 상태와
/// 다른 말을 하면 안 된다. hex도 아닌 진짜 미상 값만 team으로 폴백한다.
fn theme_accent_label(l: &crate::ui::i18n::Labels, accent: &str) -> String {
    match accent {
        "cyan" => l.accent_cyan.to_string(),
        "green" => l.accent_green.to_string(),
        "yellow" => l.accent_yellow.to_string(),
        "magenta" => l.accent_magenta.to_string(),
        "blue" => l.accent_blue.to_string(),
        "red" => l.accent_red.to_string(),
        "none" => l.accent_none.to_string(),
        other if crate::ui::theme::accent_for("default", other, None).is_some() => {
            other.to_string()
        }
        _ => l.accent_team.to_string(),
    }
}

/// `Live`가 `List`보다 훨씬 커서 clippy가 boxing을 권하지만, `App`이 화면당
/// 하나만 들고 있고 교체 빈도도 낮으므로(라이브 진입/이탈, 5s 갱신) 간접 참조를
/// 추가할 실익이 없다 — 브리프의 타입을 그대로 유지.
#[allow(clippy::large_enum_variant)]
pub enum Screen {
    List,
    Live {
        game: Game,
        state: Option<LiveState>,
    },
}

/// 우리가 다루는 마우스 동작. crossterm의 `MouseEventKind`를 그대로 쓰지 않는 건
/// 드래그·우클릭·중간 버튼까지 따라 들어오기 때문이다 — 안 쓰는 걸 타입에 담으면
/// 처리하지 않은 경우가 있는지 알 수 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Click,
    ScrollUp,
    ScrollDown,
}

pub struct App {
    pub config: Config,
    pub tab: Tab,
    pub screen: Screen,
    pub games: Vec<Game>,
    /// 첫 Games 업데이트가 폴러로부터 실제로 도착했는지. 초기값(false)과
    /// "받았는데 빈 배열"(true + games.is_empty())을 구분해야, 프리페치 순간의
    /// 빈 목록과 진짜 경기 없는 날(휴식일/전체 우천취소)을 games.rs가 다른
    /// 메시지로 보여줄 수 있다.
    pub games_loaded: bool,
    /// 첫 Standings 업데이트가 실제로 도착했는지. games_loaded와 같은 이유로
    /// 필요하다 — 없으면 앱 기동 직후 Standings 탭으로 전환했을 때 "로딩 중"과
    /// "받았는데 빈 배열"을 구분 못하고 헤더만 있는 빈 테이블을 보여준다.
    pub standings_loaded: bool,
    pub standings: Vec<Standing>,
    pub selected: usize,
    pub last_error: Option<String>,
    pub stale: bool,
    pub show_help: bool,
    pub pending_g: bool,
    /// 조회 날짜(YYYY-MM-DD, main이 설정). games 본문 타이틀("Games <date>")과
    /// standings 타이틀의 시즌 연도 표기에 쓴다.
    pub date: String,
    /// fetch가 in-flight인지 — 헤더 스피너 표시 여부.
    pub fetching: bool,
    /// 스피너 애니메이션 프레임 카운터(main.rs가 tick마다 증가).
    pub spinner_frame: u8,
    /// 라이브 화면에서 현재 타석 투구 중 짚어보고 있는 순번(None = 전체 보기).
    /// v0.18부터 "현재 타석"은 live_atbat_sel이 가리키는 at-bat(과거일 수도
    /// 있음)이다 — LiveState::active_pitches(live_atbat_sel)를 통해 그 타석의
    /// 투구 목록을 얻은 뒤 이 인덱스로 짚는다.
    pub live_pitch_sel: Option<usize>,
    /// 돌려보기(v0.18): 보고 있는 at-bat의 **시퀀스 번호**(AtBat::seq, 인덱스 아님).
    /// None = 최신(라이브)을 계속 따라간다("라이브 추종") — 폴링으로 새 타석이
    /// 추가돼도 튀지 않는다. Some(seq)는 특정 과거 타석에 고정된 상태로, `[`/`]`로만
    /// 움직이고 Esc나 최신까지 `]`로 다시 따라잡으면 None으로 되돌아간다.
    ///
    /// 인덱스가 아니라 번호인 이유: 중계 응답은 현재 이닝만 담으므로 이닝이 넘어가면
    /// at_bats가 통째로 갈린다. 인덱스로 들고 있으면 같은 자리가 다른 타석을 가리켜
    /// 읽던 위치가 조용히 어긋난다(apply가 사라진 번호를 감지해 라이브로 되돌린다).
    pub live_atbat_sel: Option<i64>,
    /// 돌려보기 중 문자중계 줄 커서(None = 커서 없음, 기존처럼 최신 N줄만
    /// 하이라이트 없이 보여준다). live_atbat_sel이 가리키는 at-bat 안에서만
    /// 유효 — at-bat을 바꾸면(`[`/`]`) 함께 리셋된다.
    pub live_relay_cursor: Option<usize>,
    /// 받아 둔 과거 이닝(v0.20). **경기 id → 이닝 번호 → 그 이닝의 타석들**(v0.21).
    ///
    /// 끝난 이닝은 더 바뀌지 않으므로 한 번만 받는다 — **현재 이닝은 넣지 않는다**
    /// (폴링이 계속 갱신하는 값이라 캐시하면 화면이 그 시점에 멈춘다). 빈 값도
    /// 캐시한다: "받아 봤더니 타석이 없더라"를 기억해야 같은 이닝을 무한히 다시
    /// 묻지 않는다.
    ///
    /// v0.20은 화면을 옮길 때마다 통째로 비웠다(남의 타석이 섞이는 걸 막으려고).
    /// v0.21은 **키가 경기 id라 섞일 수 없으므로** 비우지 않는다 — 나갔다 같은
    /// 경기로 돌아오면 되감아 둔 이닝이 그대로 있다. 낡은 캐시가 안전한 이유는
    /// 여기 들어오는 게 늘 "화면 최전방 이닝 − 1", 즉 이미 지난 이닝이기 때문이다.
    /// 상한은 두지 않는다: 하루 5경기를 다 돌아도 경기당 9이닝 남짓이고, 상한
    /// 로직이 버는 것보다 "언제 버릴지" 판단이 틀릴 위험이 크다.
    pub past_innings: HashMap<String, BTreeMap<u8, Vec<AtBat>>>,
    /// 순위 탭에서 성적을 펼쳐 본 팀의 순위(v0.24). None이면 오버레이가 닫힌 상태.
    /// 인덱스가 아니라 `Standing::rank`로 들고 있는 이유: 폴링이 순위표를 갱신하면
    /// 배열이 다시 정렬돼 같은 인덱스가 다른 팀을 가리킬 수 있다(v0.18에서 타석
    /// 선택을 인덱스에서 seq로 바꾼 것과 같은 이유).
    pub team_stats_rank: Option<u16>,
    /// 지금 받아오는 중인 이닝(None = 요청 없음). 되감기는 한 번에 한 이닝씩만
    /// 거슬러 가므로 하나면 충분하다. 화면에는 "N회 불러오는 중"으로 나간다.
    pub fetching_inning: Option<u8>,
    /// 응원 팀 KBO 코드(main이 --team/config favorite_team 별칭을 해석해 주입).
    /// UI 테마 액센트와 헤더 응원 배지에 쓴다.
    pub fav_code: Option<String>,
    /// UTC epoch 초(main.rs가 tick마다 갱신). 초보용 팁 회전(tips::current)의
    /// 입력으로만 쓰인다 — 실제 벽시계와 무관하게 결정적으로 테스트 가능하다.
    pub now_secs: u64,
    /// 마지막 "성공" 갱신(Games/Standings/Live 반영) 시각의 now_secs 스냅샷
    /// (v0.15 A-2). None = 아직 한 번도 성공한 적 없음. `stale`(이진값)과 달리
    /// "몇 초 전"까지 보여주기 위한 값 — apply()가 Error/Fetching에서는
    /// 갱신하지 않는다(`apply`의 Fetching 분기와 같은 철학: 시도 신호일 뿐 회복이 아니다).
    pub last_update_secs: Option<u64>,
    /// 표시용 시간대(프로세스 시작 시 1회 결정 — `localtime::resolve`).
    /// 경기일 판단(`dateutil::kst_days`)은 여전히 KST 고정이라 별개다.
    pub tz: crate::localtime::TimeZone,
    /// KBO 뉴스 헤드라인(부가 기능). 하단 티커가 짝수 분에 이 목록에서 순환
    /// 표시하고, 비어 있으면 항상 Tip으로 우아하게 저하한다.
    pub news: Vec<NewsItem>,
    /// F2 옵션 오버레이가 열려 있는지 + 어느 pane/커서인지(None = 닫힘).
    pub options: Option<OptionsState>,
    /// 현재 라이브 폴 주기(초). main이 초기값(config.effective_poll_secs())을
    /// 주입하고, F2 Poll pane에서 Enter로 바꾸면 run()이 변화를 감지해 폴러에
    /// Command::SetLivePoll로 통지한다(watched_game과 동일 패턴).
    pub poll_choice: u64,
    /// 하단 팁의 런타임 갱신본(부가 기능, None = 임베드 폴백). 폴러가 시작 시
    /// 1회 GitHub raw에서 가져와 채운다 — 실패해도 이 필드는 None으로 남는다.
    pub tips_override: Option<Vec<String>>,
    /// `o` 링크 픽커가 열려 있는지 + 항목/커서(None = 닫힘).
    pub link_picker: Option<LinkPickerState>,
    /// 인앱 뉴스 발췌 오버레이(부가 기능, v0.7). None = 닫힘. `n`이 현재 티커
    /// 슬롯의 NewsItem을 그대로 담아 즉시 연다(비동기 fetch 없음).
    pub article_view: Option<ArticleView>,
    /// 뉴스 목록 오버레이(부가 기능, v0.7). None = 닫힘. `n`이 열고, Enter로
    /// 커서 항목의 발췌(article_view)를 그 위에 연다.
    pub news_list: Option<NewsListState>,
    /// 인앱 설정 화면(v0.8). None = 닫힘. F9(또는 S)가 연다.
    pub settings: Option<SettingsState>,
    /// TUI chrome 표시 언어(main이 --lang/config/env로 감지해 주입). 기본값은
    /// 테스트 결정성을 위해 En — 실사용 경로에서는 main이 항상 덮어쓴다.
    pub lang: crate::ui::i18n::Lang,
    /// 테마 프리셋("default"/"high-contrast"/"mono", v0.8 T9). main이
    /// config.theme.preset을 주입한다. `ui::theme::accent_for`/`status_fg`가
    /// 이 값으로 chrome 색 사용 여부를 결정한다 — mono면 색 span 0.
    pub theme_preset: String,
    /// 액센트 색 소스("team"/명명색/"none", v0.8 T9). main이 config.theme.accent를
    /// 주입한다. games/standings 선택 하이라이트가 `theme::accent_for`를 통해
    /// 이 값을 쓴다.
    pub theme_accent: String,
    /// 설정 파일을 읽다 실패한 이유(None = 정상). 깨진 파일 위에 **덮어쓰지
    /// 않기 위한** 플래그이기도 하다 — 그냥 기본값으로 갈아타고 저장해 버리면
    /// 사용자의 설정이 조용히 사라진다.
    pub config_error: Option<String>,
    /// 마우스를 쓸지(config `mouse`, F9에서 토글). main이 매 프레임 보고 캡처를
    /// 켜고 끈다 — 끈 즉시 터미널이 드래그 선택을 되찾아야 하기 때문이다.
    pub mouse: bool,
}

/// `--team`이 들어갈 경기를 고른다.
///
/// **더블헤더에서는 진행 중인 쪽을 집는다.** 예전에는 그냥 첫 일치를 썼는데,
/// API가 시작시각 오름차순으로 주므로 항상 1차전이었다 — 2차전이 진행 중인데도
/// 이미 끝난 1차전 화면에 갇혔고(`can_enter_live(Final)`이 true라 그대로
/// 진입한다), 진입 직후 `auto_team`이 비워져 재시도 경로도 없었다.
///
/// 우선순위: 진행 중 > 예정 > 그 외(끝남·취소). 같은 등급이면 먼저 오는 것.
pub fn pick_team_game<'a>(games: &'a [Game], code: &str) -> Option<&'a Game> {
    games
        .iter()
        .filter(|g| g.home.code == code || g.away.code == code)
        .min_by_key(|g| match g.status {
            GameStatus::Live => 0,
            GameStatus::Suspended => 1,
            GameStatus::Scheduled => 2,
            _ => 3,
        })
}

impl App {
    pub fn new(config: Config) -> Self {
        let mouse = config.mouse;
        App {
            config,
            config_error: None,
            tab: Tab::Games,
            screen: Screen::List,
            games: vec![],
            games_loaded: false,
            standings_loaded: false,
            standings: vec![],
            selected: 0,
            last_error: None,
            stale: false,
            show_help: false,
            pending_g: false,
            date: String::new(),
            fetching: false,
            spinner_frame: 0,
            live_pitch_sel: None,
            live_atbat_sel: None,
            live_relay_cursor: None,
            team_stats_rank: None,
            past_innings: HashMap::new(),
            fetching_inning: None,
            fav_code: None,
            now_secs: 0,
            last_update_secs: None,
            tz: crate::localtime::TimeZone::kst(),
            news: vec![],
            options: None,
            poll_choice: 5,
            tips_override: None,
            link_picker: None,
            article_view: None,
            news_list: None,
            settings: None,
            lang: crate::ui::i18n::Lang::En,
            theme_preset: "default".into(),
            theme_accent: "team".into(),
            mouse,
        }
    }

    pub fn labels(&self) -> &'static crate::ui::i18n::Labels {
        crate::ui::i18n::labels(self.lang)
    }

    /// 키 입력 처리. true 반환 시 종료.
    pub fn on_key(&mut self, key: KeyCode) -> bool {
        if self.show_help {
            // 도움말 화면에서는 아무 키나 눌러 닫는다.
            self.show_help = false;
            self.pending_g = false;
            return false;
        }

        if let Some(opt) = &mut self.options {
            match key {
                KeyCode::Esc | KeyCode::F(2) => self.options = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    let len = crate::ui::options::pane_len(
                        opt.pane,
                        self.now_secs,
                        crate::ui::i18n::labels(self.lang),
                    );
                    if len > 0 && opt.cursor + 1 < len {
                        opt.cursor += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    opt.cursor = opt.cursor.saturating_sub(1);
                }
                KeyCode::Enter => self.apply_option(),
                _ => {} // 오버레이가 나머지 키 소비
            }
            self.pending_g = false;
            return false;
        }
        if let Some(picker) = &mut self.link_picker {
            match key {
                KeyCode::Esc | KeyCode::Char('o') => self.link_picker = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    if picker.cursor + 1 < picker.items.len() {
                        picker.cursor += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => picker.cursor = picker.cursor.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some((_, url)) = picker.items.get(picker.cursor) {
                        crate::ui::teamlinks::open_url(url);
                    }
                    self.link_picker = None;
                }
                _ => {}
            }
            self.pending_g = false;
            return false;
        }
        if let Some(view) = &mut self.article_view {
            // 기사 오버레이가 열려 있으면 모든 키를 소비한다(options/link_picker 패턴).
            // scroll 상한은 렌더가 콘텐츠 길이로 clamp하므로 여기선 saturating만.
            match key {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => self.article_view = None,
                KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
                KeyCode::PageDown => view.scroll = view.scroll.saturating_add(10),
                KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(10),
                KeyCode::Char('o') | KeyCode::Enter if !view.item.url.is_empty() => {
                    crate::ui::teamlinks::open_url(&view.item.url);
                }
                _ => {}
            }
            self.pending_g = false;
            return false;
        }
        if let Some(list) = &mut self.news_list {
            // 목록 오버레이가 열려 있으면 모든 키를 소비한다. 기사 오버레이가 이
            // 위에 겹칠 수 있으므로(article_view 블록이 먼저 소비) 여기 도달했다는
            // 것은 기사가 닫혀 있고 목록만 열려 있다는 뜻이다.
            match key {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => self.news_list = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    if list.cursor + 1 < self.news.len() {
                        list.cursor += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => list.cursor = list.cursor.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some(item) = self.news.get(list.cursor).cloned() {
                        self.article_view = Some(ArticleView { item, scroll: 0 });
                    }
                }
                _ => {}
            }
            self.pending_g = false;
            return false;
        }
        // 팀 성적 오버레이(v0.24)가 **화면에 보이는 동안** 키를 소비한다.
        //
        // 조건이 `team_stats_rank.is_some()`이 아니라 `team_stats_target()`인 게
        // 중요하다 — 렌더도 같은 함수를 보므로 "안 보이는데 키만 먹는" 상태가
        // 구조적으로 생길 수 없다. 두 축을 따로 두면 rank는 살아 있는데 그 팀이
        // 순위표에서 사라진 경우(폴링 갱신) 화면 없이 입력만 잠긴다.
        if self.team_stats_target().is_some() {
            if matches!(key, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                self.team_stats_rank = None;
            }
            self.pending_g = false;
            return false;
        }
        if self.settings.is_some() {
            // 설정 오버레이가 열려 있으면 모든 키를 소비한다(options/link_picker
            // 패턴). rows 길이는 settings_rows()가 &self를 빌리므로 &mut
            // self.settings 빌림 전에 먼저 계산해 둔다(borrow 충돌 회피).
            let rows = self.settings_rows().len();
            let st = self.settings.as_mut().unwrap();
            match key {
                KeyCode::Esc | KeyCode::F(9) | KeyCode::Char('q') => self.settings = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    if st.cursor + 1 < rows {
                        st.cursor += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => st.cursor = st.cursor.saturating_sub(1),
                KeyCode::Left | KeyCode::Right | KeyCode::Enter => {
                    // st(=&mut self.settings)의 빌림을 여기서 끝낸다 — 아래
                    // self.change_setting/self.persist가 &mut self를 다시 빌린다.
                    let cursor = st.cursor;
                    let forward = !matches!(key, KeyCode::Left);
                    self.change_setting(cursor, forward);
                    self.persist();
                }
                _ => {}
            }
            self.pending_g = false;
            return false;
        }
        // opener들은 모든 오버레이 consumer 뒤에 둔다 — 링크픽커가 열린 채 F2를
        // 누르면 오버레이가 이중으로 열리던 결함(최종 리뷰 I-1) 방지.
        // `S`는 F9의 별칭이다. 도움말·README에는 F9만 적혀 있어 "안 쓰는 키"로
        // 보이지만, **데모 녹화가 이 별칭에 의존한다** — VHS가 F키를 못 보내서
        // `docs/demo.tape`·`demo.en.tape`가 `Type "S"`를 쓴다. 지우면 릴리스마다
        // 도는 녹화가 조용히 깨진다(`tests/docs_match_code.rs`가 봉인한다).
        if key == KeyCode::F(9) || key == KeyCode::Char('S') {
            self.settings = Some(SettingsState {
                cursor: 0,
                save_failed: false,
            });
            self.pending_g = false;
            return false;
        }
        if key == KeyCode::F(2) {
            self.options = Some(OptionsState {
                pane: Pane::Date,
                cursor: 0,
            });
            self.pending_g = false;
            return false;
        }
        if key == KeyCode::Char('o') {
            let items = crate::ui::teamlinks::link_items_for_screen(self);
            if !items.is_empty() {
                self.link_picker = Some(LinkPickerState { items, cursor: 0 });
            }
            self.pending_g = false;
            return false;
        }

        match key {
            KeyCode::Char('q') | KeyCode::F(10) => {
                self.pending_g = false;
                return true;
            }
            KeyCode::F(1) | KeyCode::Char('?') | KeyCode::Char('h') => {
                self.show_help = true;
                self.pending_g = false;
            }
            KeyCode::Tab | KeyCode::F(5) => {
                // Live에서 Tab은 "다른 화면을 보고 싶다"는 의도 — 목록으로
                // 나가면서 탭을 전환한다(헤더만 바뀌고 본문이 안 바뀌던 혼란 해소).
                if matches!(self.screen, Screen::Live { .. }) {
                    self.screen = Screen::List;
                    self.live_pitch_sel = None;
                    self.live_atbat_sel = None;
                    self.live_relay_cursor = None;
                }
                self.tab = match self.tab {
                    Tab::Games => Tab::Standings,
                    Tab::Standings => Tab::Games,
                };
                self.selected = 0;
                self.pending_g = false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // 라이브 화면에서는 문자중계 줄 커서(돌려보기, v0.18) — 목록의
                // j/k(self.selected)와 같은 키를 겹쳐 쓰되 화면별로 의미가 다르다
                // (Left/Right가 이미 이 패턴이다).
                if matches!(self.screen, Screen::Live { state: Some(_), .. }) {
                    self.move_relay_cursor(true);
                } else {
                    let len = self.current_len();
                    if len > 0 && self.selected + 1 < len {
                        self.selected += 1;
                    }
                }
                self.pending_g = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if matches!(self.screen, Screen::Live { state: Some(_), .. }) {
                    self.move_relay_cursor(false);
                } else if self.selected > 0 {
                    self.selected -= 1;
                }
                self.pending_g = false;
            }
            KeyCode::Left | KeyCode::Right => {
                self.move_pitch_sel(key == KeyCode::Right);
                self.pending_g = false;
            }
            KeyCode::Char('[') | KeyCode::Char(']') => {
                // 돌려보기(v0.18): 이전/다음 타석. `]`로 최신까지 다시 따라오면
                // live_atbat_sel을 None으로 되돌려 "라이브 추종" 모드로 복귀한다
                // (그래야 그 뒤 폴링에서 새 타석이 자동으로 따라와진다).
                // 이닝 경계 요청은 이 블록이 &self.screen을 놓은 뒤에 건다.
                let mut want_earlier_inning = false;
                if let Screen::Live { state: Some(s), .. } = &self.screen {
                    let len = s.at_bats.len();
                    // M-2: `no`가 결측이면 seq가 전부 0(또는 다른 값)으로 뭉개져
                    // 아래 position()이 항상 첫 일치 항목만 찾는다 — 그러면
                    // 되감기가 그 자리에 갇히고 `]`로도 라이브 복귀가 불가능해진다
                    // (실측). seq 유일성이 깨졌으면 잘못된 자리에 갇히는 것보다
                    // 네비게이션 자체를 비활성화하는 편이 안전하다.
                    if len > 0 && s.has_unique_at_bat_seqs() {
                        let last = len - 1;
                        // 선택은 번호로 들고 있으므로 이동할 때만 인덱스로 환산한다.
                        // 번호가 응답에서 사라졌으면(이닝 전환) 최신에서 다시 시작.
                        let cur = self
                            .live_atbat_sel
                            .and_then(|seq| s.at_bats.iter().position(|ab| ab.seq == seq))
                            .unwrap_or(last);
                        // 맨 앞 타석에서 `[`를 한 번 더: 그 앞 이닝을 받아온다(v0.20).
                        // 응답은 현재 이닝만 담으므로, 여기서 멈추면 되감기가 이닝
                        // 경계에 갇힌다 — 늦게 접속한 사람이 앞 이닝을 따라잡는 게
                        // 이 기능의 원래 목적이다.
                        if key == KeyCode::Char('[') && cur == 0 {
                            want_earlier_inning = true;
                        }
                        let next = if key == KeyCode::Char('[') {
                            cur.saturating_sub(1)
                        } else {
                            (cur + 1).min(last)
                        };
                        self.live_atbat_sel = if next >= last {
                            None
                        } else {
                            s.at_bats.get(next).map(|ab| ab.seq)
                        };
                        // 다른 타석의 투구/문자중계 커서는 의미가 다르므로 리셋.
                        self.live_pitch_sel = None;
                        self.live_relay_cursor = None;
                    }
                }
                if want_earlier_inning {
                    self.request_earlier_inning();
                }
                self.pending_g = false;
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    // 라이브 화면에서 `j`/`k`가 문자중계 커서를 잡고 있으므로
                    // `gg`/`G`도 같은 축의 양끝이어야 한다(v0.18 최종 리뷰 Minor).
                    // 그전까지 이 둘은 화면에 보이지도 않는 목록 선택을 움직였고,
                    // 그래서 문자중계 맨 위/맨 아래로 점프할 방법이 아예 없었다.
                    if self.is_live_screen() {
                        self.jump_relay_cursor(false);
                    } else {
                        self.selected = 0;
                    }
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                return false;
            }
            KeyCode::Char('G') => {
                if self.is_live_screen() {
                    self.jump_relay_cursor(true);
                } else {
                    self.selected = self.current_len().saturating_sub(1);
                }
                self.pending_g = false;
            }
            KeyCode::Enter => {
                // 순위 탭의 Enter는 그 팀 성적을 펼친다(v0.24). 경기 탭의
                // Enter(라이브 진입)와 뜻이 다르지만, 두 탭이 같은 키를 각자
                // 쓰는 건 이미 있는 구조다(`o` 링크가 탭마다 다른 팀을 고른다).
                if self.tab == Tab::Standings && matches!(self.screen, Screen::List) {
                    // 열지 말지(경기 전 팀은 성적이 전부 0이라 "기록 없음"과
                    // 구분되지 않는다)는 team_stats_target 한 곳에서 판정한다 —
                    // 여기서도 걸러 두면 같은 규칙이 두 곳에 흩어져 언젠가
                    // 어긋난다(v0.18에서 같은 술어를 공유하게 만든 이유).
                    self.team_stats_rank = self.standings.get(self.selected).map(|s| s.rank);
                    self.pending_g = false;
                    return false;
                }
                if self.tab == Tab::Games && matches!(self.screen, Screen::List) {
                    if let Some(g) = self.games.get(self.selected).cloned() {
                        if Self::can_enter_live(g.status) {
                            // 이전 게임에서 짚어보던 투구/타석 선택이 새 게임으로
                            // 넘어오지 않도록 세 선택을 함께 리셋하는 공통 경로
                            // (리뷰 I-3) — main.rs의 --team 자동 진입과 공유한다.
                            self.enter_live(g);
                        }
                    }
                }
                self.pending_g = false;
            }
            KeyCode::Esc => {
                // v0.18 Esc 계단(3단): ①투구/문자중계 커서 해제 → ②과거 타석
                // 보기 중이면 최신(라이브)로 복귀 → ③화면 자체를 나가 목록으로.
                // 기존 2단(①·③)에 돌려보기 복귀(②)를 끼워 넣은 순서 — 세부
                // 선택부터 지우고, 그다음 "어디를 보고 있는지", 마지막에
                // "어느 화면인지" 순으로 넓어진다.
                if self.live_pitch_sel.is_some() || self.live_relay_cursor.is_some() {
                    self.live_pitch_sel = None;
                    self.live_relay_cursor = None;
                } else if self.live_atbat_sel.is_some() {
                    self.live_atbat_sel = None;
                } else if matches!(self.screen, Screen::Live { .. }) {
                    self.screen = Screen::List;
                }
                self.pending_g = false;
            }
            KeyCode::Char('n') => {
                // 뉴스 목록을 연다(v0.7) — 골라서 Enter로 발췌를 읽는다.
                if !self.news.is_empty() {
                    self.news_list = Some(NewsListState { cursor: 0 });
                }
                self.pending_g = false;
            }
            _ => {
                self.pending_g = false;
            }
        }
        false
    }

    /// `j`/`k` — 문자중계 줄 커서를 한 칸 옮기고, **그 줄이 가리키는 투구를 함께
    /// 고른다**(v0.19 연동). 처음 누르면(`None`) 항상 맨 아래(최신 줄)에서
    /// 시작한다 — 무선택 뷰가 이미 최신 꼬리를 보여주므로 커서가 그 자리에서
    /// 자연스럽게 이어지도록.
    ///
    /// 투구 선택은 커서에서 **유도**되므로(투구가 아닌 줄이면 위쪽 가장 가까운
    /// 투구 — `LiveState::pitch_at_relay_line`) 두 필드가 어긋난 조합이 생기지
    /// 않는다.
    ///
    /// **선택을 이동시키는 자리는 이 메서드와 [`App::move_pitch_sel`] 둘뿐이다**
    /// (리뷰 M-2 — "유일한 자리"는 사실이 아니었다). 라이브 선택을 건드리는
    /// 자리는 이 둘 말고도 Tab·`[`/`]`·Esc·`enter_live`·`apply_option`·폴링
    /// 갱신까지 있지만, 그 자리들은 전부 두 필드를 **동시에 `None`으로
    /// 리셋**할 뿐이라 모순 조합을 만들지 않는다(동시 해제는 항상 안전하다 —
    /// `LiveVm`의 "커서가 이긴다" 규칙이 `None`/`None`을 그대로 선택 없음으로
    /// 읽는다). 이동은 이 두 메서드에서만 일어난다.
    fn move_relay_cursor(&mut self, down: bool) {
        let Screen::Live { state: Some(s), .. } = &self.screen else {
            return;
        };
        let Some(last) = s
            .active_relay_lines(self.live_atbat_sel)
            .len()
            .checked_sub(1)
        else {
            return; // 줄이 하나도 없으면 커서를 세울 자리가 없다(무동작).
        };
        let idx = match (self.live_relay_cursor, down) {
            (None, _) => last,
            (Some(i), true) => (i + 1).min(last),
            // M-3: 위쪽 이동도 범위 밖 커서(줄 수가 줄어든 뒤 남은 stale 값)를
            // 잠가야 한다 — 안 그러면 saturating_sub(1)만으로는 여전히 범위
            // 밖 값이 나온다. `.min(last)`을 뒤에 둬 아래쪽 분기((i+1).min(last))와
            // 같은 모양으로 맞춘다: 범위 밖에서는 어느 방향이든 먼저 `last`로
            // 스냅한다.
            (Some(i), false) => i.saturating_sub(1).min(last),
        };
        let pitch = s.pitch_at_relay_line(self.live_atbat_sel, idx);
        self.live_relay_cursor = Some(idx);
        self.live_pitch_sel = pitch;
    }

    /// 문자중계의 **그 줄**로 커서를 옮긴다(마우스 클릭). `j`/`k`가 한 칸씩
    /// 가는 것과 목적지만 다르고, 투구 연동은 같은 규칙을 쓴다 — 규칙이 두 곳에
    /// 흩어지면 언젠가 어긋난다.
    fn put_relay_cursor(&mut self, idx: usize) {
        let Screen::Live { state: Some(s), .. } = &self.screen else {
            return;
        };
        let Some(last) = s
            .active_relay_lines(self.live_atbat_sel)
            .len()
            .checked_sub(1)
        else {
            return; // 줄이 없으면 세울 자리도 없다(j/k와 같다).
        };
        // 화면에 그린 줄만 등록하므로 여기 오는 값은 범위 안이지만, 폴링이
        // 그 사이 줄 수를 줄였을 수 있다 — 범위 밖이면 조용히 마지막 줄로.
        let idx = idx.min(last);
        let pitch = s.pitch_at_relay_line(self.live_atbat_sel, idx);
        self.live_relay_cursor = Some(idx);
        self.live_pitch_sel = pitch;
    }

    /// 마우스 입력. `zone`은 그 좌표에 그려져 있던 것(빈 곳이면 None).
    ///
    /// **오버레이가 떠 있으면 아무것도 하지 않는다** — 히트맵에는 그 아래 화면의
    /// 영역이 남아 있어서, 처리하면 사용자가 **보고 있지도 않은 것**이 움직인다.
    /// 오버레이는 `Esc`로 닫는다(마우스로 닫지 않는다는 건 설계 결정이다).
    ///
    /// 스크롤은 기존 키 처리(`on_key`)로 흘려보낸다. 목록에서든 문자중계에서든
    /// "한 칸 위/아래"는 이미 `j`/`k`가 정의해 둔 동작이고, 경계 규칙도 거기 있다.
    pub fn on_mouse(&mut self, zone: Option<crate::ui::hit::Zone>, kind: MouseAction) {
        use crate::ui::hit::Zone;
        // 껐으면 여기서 끝난다. main이 캡처를 풀지만 그것만 믿지 않는다 —
        // 모드 전환을 무시하는 터미널도 있고, 푸는 사이에 이미 큐에 들어온
        // 이벤트도 있다. **끔은 끔이어야 한다.**
        if !self.mouse {
            return;
        }
        // 팀 성적은 `team_stats_rank`가 아니라 **`team_stats_target()`**을 본다.
        // 렌더(ui/mod.rs)와 키 소비(on_key)가 v0.24에서 이 함수 하나로 통일한
        // 이유가 그대로 여기에도 적용된다 — rank는 남았는데 그 팀이 순위표에서
        // 사라지면(폴링 갱신) **화면 없이 마우스만 잠긴다.** 그때 키는 멀쩡히
        // 듣고 있어서, 사용자에게는 마우스가 죽은 것처럼만 보인다.
        if self.show_help
            || self.options.is_some()
            || self.settings.is_some()
            || self.article_view.is_some()
            || self.news_list.is_some()
            || self.link_picker.is_some()
            || self.team_stats_target().is_some()
        {
            return;
        }
        // `gg` 대기를 끊는다. on_key는 모든 분기에서 이걸 지운다 — "입력이 오면
        // g 시퀀스는 끝난다"가 그쪽 계약이다. 마우스만 예외로 두면, g를 한 번
        // 누른 뒤 클릭해서 고른 행이 **다음 g 하나에 맨 위로 날아간다.**
        self.pending_g = false;
        match (kind, zone) {
            (MouseAction::Click, Some(Zone::Tab(t))) => {
                if self.tab != t || matches!(self.screen, Screen::Live { .. }) {
                    // 키 `Tab`과 같은 경로를 타되 목적지를 직접 고른다 — 탭이
                    // 둘뿐이라 지금은 결과가 같지만, 클릭은 "저기로"라는 뜻이지
                    // "다음으로"가 아니다.
                    if matches!(self.screen, Screen::Live { .. }) {
                        self.screen = Screen::List;
                        self.live_pitch_sel = None;
                        self.live_atbat_sel = None;
                        self.live_relay_cursor = None;
                    }
                    self.tab = t;
                    self.selected = 0;
                }
            }
            // 한 번 눌러 고르고, 고른 것을 다시 눌러 연다. 첫 클릭에 바로 열면
            // 옆줄을 잘못 눌렀을 때 되돌리는 값이 크다.
            (MouseAction::Click, Some(Zone::GameRow(i))) => {
                if self.selected == i {
                    self.on_key(KeyCode::Enter);
                } else {
                    self.selected = i;
                }
            }
            (MouseAction::Click, Some(Zone::StandingRow(i))) => {
                if self.selected == i {
                    self.on_key(KeyCode::Enter);
                } else {
                    self.selected = i;
                }
            }
            (MouseAction::Click, Some(Zone::RelayLine(i))) => self.put_relay_cursor(i),
            (MouseAction::Click, _) => {}
            // 존·측면 위에서 휠은 공을 넘긴다(`←`/`→`와 같은 축).
            (MouseAction::ScrollUp, Some(Zone::PitchNav)) => {
                self.on_key(KeyCode::Left);
            }
            (MouseAction::ScrollDown, Some(Zone::PitchNav)) => {
                self.on_key(KeyCode::Right);
            }
            (MouseAction::ScrollUp, Some(_)) => {
                self.on_key(KeyCode::Up);
            }
            (MouseAction::ScrollDown, Some(_)) => {
                self.on_key(KeyCode::Down);
            }
            // 빈 곳에서의 휠은 아무것도 하지 않는다 — 어느 목록을 굴릴지
            // 짐작하지 않는다.
            (_, None) => {}
        }
    }

    fn is_live_screen(&self) -> bool {
        matches!(self.screen, Screen::Live { state: Some(_), .. })
    }

    /// `gg`/`G` — 문자중계 커서를 그 타석의 첫 줄/마지막 줄로 보낸다. 한 줄씩
    /// 가는 `j`/`k`와 같은 축이고, 투구 선택도 같은 규칙으로 함께 따라온다.
    fn jump_relay_cursor(&mut self, to_end: bool) {
        let Screen::Live { state: Some(s), .. } = &self.screen else {
            return;
        };
        let Some(last) = s
            .active_relay_lines(self.live_atbat_sel)
            .len()
            .checked_sub(1)
        else {
            return; // 줄이 없으면 세울 자리도 없다(무동작 — j/k와 같다).
        };
        let idx = if to_end { last } else { 0 };
        let pitch = s.pitch_at_relay_line(self.live_atbat_sel, idx);
        self.live_relay_cursor = Some(idx);
        self.live_pitch_sel = pitch;
    }

    /// `←`/`→` — 보고 있는 타석(live_atbat_sel — 과거일 수도 있음)의 투구를
    /// 하나씩 짚어보고, **그 투구의 문자중계 줄로 커서를 옮긴다**(v0.19 연동의
    /// 반대 방향). 순환 없음; 선택 없음 = 전체 보기, `→`는 처음부터 `←`는
    /// 마지막부터 진입.
    ///
    /// 짝이 되는 줄을 못 찾으면 커서를 **비운다**(옛 줄에 남겨 두지 않는다) —
    /// 남겨 두면 커서와 투구 선택이 서로 다른 사건을 가리키는 모순 조합이 되고,
    /// 커서를 우선하는 표현 규칙(`LiveVm`) 때문에 방금 고른 투구가 도로 밀려난다.
    fn move_pitch_sel(&mut self, right: bool) {
        let Screen::Live { state: Some(s), .. } = &self.screen else {
            return;
        };
        let Some(last) = s.active_pitches(self.live_atbat_sel).len().checked_sub(1) else {
            return;
        };
        let idx = match (self.live_pitch_sel, right) {
            (None, true) => 0,
            (None, false) => last,
            (Some(i), true) => (i + 1).min(last),
            (Some(i), false) => i.saturating_sub(1),
        };
        let line = s.relay_line_of_pitch(self.live_atbat_sel, idx);
        self.live_pitch_sel = Some(idx);
        self.live_relay_cursor = line;
    }

    /// Canceled/Scheduled 게임은 relay가 textRelayData를 절대 내려주지 않으므로
    /// 진입시키면 사용자에게 이유를 알릴 수 없는 영구 "loading..." 화면에 갇힌다.
    /// Enter 키 진입(on_key)과 `--team` 자동 진입(main.rs) 두 경로가 각자 가드를
    /// 들고 있으면 언젠가 하나만 고쳐지고 어긋나므로, 이 판단을 여기 한 곳에 둔다.
    pub fn can_enter_live(status: GameStatus) -> bool {
        !matches!(status, GameStatus::Canceled | GameStatus::Scheduled)
    }

    /// Live 화면 진입 공통 경로(리뷰 I-3). Enter 키(on_key)와 `--team` 자동
    /// 진입(main.rs) 두 곳이 각자 화면 전환을 조립하다가, 자동 진입 쪽이 세
    /// 선택(live_pitch_sel/live_atbat_sel/live_relay_cursor) 리셋을 빠뜨린
    /// 채 어긋난 적이 있다 — `can_enter_live`와 같은 이유로 한 곳에 모은다.
    ///
    /// 셋 다 반드시 리셋해야 하는 이유: `live_atbat_sel`(seq)은 경기별 번호라
    /// 다른 경기와 번호 대역이 겹칠 수 있다 — 리셋하지 않으면 폴링 가드
    /// (`!l.has_at_bat(seq)`)가 "우연히 같은 seq가 새 경기에도 있다"고
    /// 오판해 걸러주지 못하고, 새 경기가 처음부터 엉뚱한 과거 타석에 고정된
    /// 채(+그 타석 기준 투구 선택까지 살아남은 채) 열릴 수 있다.
    pub fn enter_live(&mut self, game: Game) {
        // 목록 커서를 이 경기에 맞춘다(v0.22). `--team` 자동 진입은 커서를 건드리지
        // 않아, Esc로 목록에 나오면 커서가 첫 항목(남의 경기)에 있었다 — 거기서
        // Enter를 누르면 방금 보던 경기가 아닌 데로 들어간다. v0.21 실행 확인에서
        // "되감기 캐시가 안 먹는다"고 두 번 오판하게 만든 자리다. 진입 경로가 여기
        // 하나로 모여 있으므로(v0.18) 두 경로가 함께 고쳐진다. 목록에 없는 경기면
        // (날짜 전환 직후 등) 커서를 건드리지 않는다 — 엉뚱한 자리로 옮기느니 그대로 둔다.
        if let Some(i) = self.games.iter().position(|g| g.id == game.id) {
            self.selected = i;
        }
        self.screen = Screen::Live { game, state: None };
        self.live_pitch_sel = None;
        self.live_atbat_sel = None;
        self.live_relay_cursor = None;
        // past_innings는 비우지 않는다(v0.21) — 경기 id로 나뉘어 있어 남의 타석이
        // 섞일 수 없고, 나갔다 돌아오면 되감아 둔 이닝이 그대로 있어야 한다.
        // fetching_inning은 화면에 딸린 상태라 리셋한다.
        self.fetching_inning = None;
    }

    /// 성적 오버레이가 보여줄 팀. 닫혀 있거나 그 순위가 사라졌거나 아직 경기를
    /// 안 치렀으면 None — 화면은 이 값 하나만 보고 그린다.
    pub fn team_stats_target(&self) -> Option<&crate::model::Standing> {
        let rank = self.team_stats_rank?;
        self.standings
            .iter()
            .find(|s| s.rank == rank)
            .filter(|s| Self::has_team_stats(s))
    }

    /// 지금 커서가 가리키는 순위 행을 Enter로 열 수 있는가 — footer 힌트가
    /// 묻는다. 판정은 `team_stats_target`과 같은 술어를 쓴다(경기 전 팀은
    /// 성적이 전부 0이라 "기록 없음"과 구분되지 않아 열지 않는다).
    pub fn team_stats_available(&self) -> bool {
        self.tab == Tab::Standings
            && matches!(self.screen, Screen::List)
            && self
                .standings
                .get(self.selected)
                .is_some_and(Self::has_team_stats)
    }

    fn has_team_stats(s: &crate::model::Standing) -> bool {
        s.games > 0
    }

    /// 그 경기에 대해 받아 둔 이닝들. 없으면 빈 맵을 빌려준다 — 호출부가 매번
    /// Option을 풀지 않게 한다.
    pub fn cached_innings_of(&self, game_id: &str) -> &BTreeMap<u8, Vec<AtBat>> {
        static EMPTY: std::sync::LazyLock<BTreeMap<u8, Vec<AtBat>>> =
            std::sync::LazyLock::new(BTreeMap::new);
        self.past_innings.get(game_id).unwrap_or(&EMPTY)
    }

    /// 라벨("T9"/"B9"/"Inn 9")에서 이닝 번호를 뽑는다. 0이나 숫자 없음은 None —
    /// `inn`이 결측일 때 파서가 만드는 "Inn 0"을 실재하는 이닝으로 오인하면
    /// 0회를 요청하게 된다.
    fn inning_number_of(label: &str) -> Option<u8> {
        let digits: String = label.chars().filter(char::is_ascii_digit).collect();
        digits.parse::<u8>().ok().filter(|n| *n > 0)
    }

    /// 지금 화면에 있는 가장 이른 타석보다 한 이닝 앞을 요청 대상으로 잡는다.
    /// 이미 받아 둔 이닝은 건너뛴다 — 타석이 0건이던 이닝(캐시에 빈 값으로 남는다)에서
    /// 같은 번호를 무한히 다시 묻지 않기 위해서다.
    fn earlier_inning_target(&self) -> Option<u8> {
        let Screen::Live {
            game,
            state: Some(s),
        } = &self.screen
        else {
            return None;
        };
        let first = s
            .at_bats
            .first()
            .and_then(|ab| Self::inning_number_of(&ab.inning_label))?;
        let cached = self.cached_innings_of(&game.id);
        let mut target = first.checked_sub(1)?;
        while target >= 1 && cached.contains_key(&target) {
            target -= 1;
        }
        (target >= 1).then_some(target)
    }

    /// 앞 이닝 요청을 예약한다. 실제 전송은 run() 루프가 `fetching_inning` 변화를
    /// 보고 수행한다(App은 채널을 모른다 — watched_game과 동일 패턴).
    /// 이미 요청이 떠 있으면 아무것도 하지 않는다: 되감기는 한 번에 한 이닝씩 간다.
    fn request_earlier_inning(&mut self) {
        if self.fetching_inning.is_some() {
            return;
        }
        self.fetching_inning = self.earlier_inning_target();
    }

    /// 받아 둔 과거 이닝을 라이브 타석 목록 앞에 잇는다.
    ///
    /// 정렬·중복 판정 축은 `AtBat::seq`(응답의 `no`)다 — 실측상 경기 전체에 걸쳐
    /// 유일·연속이라 이닝을 가로질러도 한 축으로 선다. **겹치면 라이브 쪽이 이긴다**:
    /// 기본 `/relay`와 `?inning=<현재 이닝>`은 같은 데이터를 주는데(실측), 캐시본은
    /// 그 시점에 멈춰 있고 라이브는 폴링으로 계속 갱신되기 때문이다.
    fn merge_past_innings(past: &BTreeMap<u8, Vec<AtBat>>, live: &mut LiveState) {
        if past.is_empty() {
            return;
        }
        let live_seqs: std::collections::HashSet<i64> =
            live.at_bats.iter().map(|ab| ab.seq).collect();
        let mut merged: Vec<AtBat> = past
            .values()
            .flatten()
            .filter(|ab| !live_seqs.contains(&ab.seq))
            .cloned()
            .collect();
        merged.extend(live.at_bats.iter().cloned());
        merged.sort_by_key(|ab| ab.seq);
        // 캐시끼리 겹치는 일은 없어야 하지만(이닝별로 구간이 갈린다), 남으면 선택이
        // seq로 고정되는 규칙이 깨지므로 여기서 막는다.
        merged.dedup_by_key(|ab| ab.seq);
        live.at_bats = merged;
    }

    /// 옵션 픽커 Enter: 현재 pane·커서의 항목을 적용하고 닫는다.
    /// 폴러 통지는 run() 루프가 상태 변화 감지로 수행(App은 채널을 모른다 —
    /// watched_game과 동일 패턴).
    fn apply_option(&mut self) {
        let Some(opt) = self.options.take() else {
            return;
        };
        let l = self.labels();
        // Pane은 v0.8부터 Date 단일 variant다(Team·Poll은 F9 설정으로 이동).
        if let Some((_, date)) = crate::ui::options::date_items(l, self.now_secs)
            .into_iter()
            .nth(opt.cursor)
        {
            if date != self.date {
                self.date = date;
                self.games_loaded = false;
                self.games.clear();
                self.selected = 0;
                self.live_pitch_sel = None;
                self.live_atbat_sel = None;
                self.live_relay_cursor = None;
                // 다른 날짜의 라이브 화면은 무의미 — 목록으로 복귀.
                self.screen = Screen::List;
            }
        }
    }

    fn current_len(&self) -> usize {
        match self.tab {
            Tab::Games => self.games.len(),
            Tab::Standings => self.standings.len(),
        }
    }

    /// 설정 항목(종류, 라벨, 현재값 표시). E(언어)가 항목을 더 확장할 수 있다.
    pub fn settings_rows(&self) -> Vec<(SettingKind, &'static str, String)> {
        let l = self.labels();
        // team_items(options.rs)가 (라벨, 코드)의 단일 진실 — F2 픽커·change_setting과
        // 같은 출처를 써서 화면마다 팀 표기가 갈리는 것을 막는다. 모르는 코드(구버전
        // config 등)는 라벨을 못 찾으므로 코드 문자열 그대로 폴백한다(패닉·빈 값 금지).
        let team = match &self.fav_code {
            Some(code) => crate::ui::options::team_items(l)
                .into_iter()
                .find(|(_, c)| c.as_deref() == Some(code.as_str()))
                .map(|(label, _)| label)
                .unwrap_or_else(|| code.clone()),
            None => l.team_none.to_string(),
        };
        vec![
            (SettingKind::Team, l.set_team, team),
            (
                SettingKind::Poll,
                l.set_poll,
                format!("{}{}", self.poll_choice, l.poll_suffix),
            ),
            (
                SettingKind::ThemePreset,
                l.set_theme_preset,
                theme_preset_label(l, &self.theme_preset).to_string(),
            ),
            (
                SettingKind::ThemeAccent,
                l.set_theme_accent,
                theme_accent_label(l, &self.theme_accent),
            ),
            (
                SettingKind::Lang,
                l.set_lang,
                crate::ui::i18n::lang_display_name(self.lang).to_string(),
            ),
            (
                SettingKind::Mouse,
                l.set_mouse,
                if self.mouse { l.on } else { l.off }.to_string(),
            ),
        ]
    }

    /// 현재 영속 대상(팀·폴링·언어·테마)을 Config로 만들어 저장한다. 실패는 삼켜
    /// settings.save_failed에 반영한다(무패닉·조용한 저하).
    fn persist(&mut self) {
        // 읽다 실패한 파일은 건드리지 않는다. 기본값으로 갈아탄 상태를 되쓰면
        // 사용자가 손으로 적어 둔 것들이 진짜로 사라진다 — 오타 하나 고치면
        // 되는 상황을 복구 불가로 만드는 셈이다.
        if self.config_error.is_some() {
            if let Some(st) = &mut self.settings {
                st.save_failed = true;
            }
            return;
        }
        let cfg = crate::config::Config {
            favorite_team: self.fav_code.clone(),
            poll_secs: self.poll_choice,
            lang: Some(crate::ui::i18n::lang_code(self.lang).to_string()),
            // config의 원본을 그대로 되쓴다 — F9에서 다른 설정을 바꿔도
            // 사용자의 시간대 설정이 지워지면 안 된다(설정 손실 방지).
            timezone: self.config.timezone.clone(),
            mouse: self.mouse,
            theme: crate::config::ThemeConfig {
                preset: self.theme_preset.clone(),
                accent: self.theme_accent.clone(),
            },
        };
        let ok = cfg.save().is_ok();
        if let Some(st) = &mut self.settings {
            st.save_failed = !ok;
        }
    }

    /// 커서 행의 설정을 한 단계 순환한다(forward=→/Enter, back=←). 변경만; 저장은
    /// 호출부(persist)가 한다. **종류로 분기**한다(인덱스 아님) — 뒤 태스크가
    /// settings_rows에 행을 추가해도 이 분기는 안 밀린다.
    fn change_setting(&mut self, cursor: usize, forward: bool) {
        let Some((kind, _, _)) = self.settings_rows().into_iter().nth(cursor) else {
            return;
        };
        match kind {
            SettingKind::Team => {
                // team_items(None 포함)에서 현재 fav_code의 다음/이전으로 순환.
                let items = crate::ui::options::team_items(self.labels());
                let cur = items
                    .iter()
                    .position(|(_, c)| *c == self.fav_code)
                    .unwrap_or(0);
                let n = items.len();
                let next = if forward {
                    (cur + 1) % n
                } else {
                    (cur + n - 1) % n
                };
                self.fav_code = items[next].1.clone();
            }
            SettingKind::Poll => {
                // poll_items가 폴링 간격의 단일 진실(3/5/10/30초) — team_items와
                // 같은 패턴으로 재사용해 리터럴 중복을 없앤다.
                let items = crate::ui::options::poll_items(self.labels());
                let cur = items
                    .iter()
                    .position(|(_, s)| *s == self.poll_choice)
                    .unwrap_or(1);
                let n = items.len();
                let next = if forward {
                    (cur + 1) % n
                } else {
                    (cur + n - 1) % n
                };
                self.poll_choice = items[next].1;
            }
            SettingKind::ThemePreset => {
                let items = THEME_PRESETS;
                let cur = items
                    .iter()
                    .position(|p| *p == self.theme_preset)
                    .unwrap_or(0);
                let n = items.len();
                let next = if forward {
                    (cur + 1) % n
                } else {
                    (cur + n - 1) % n
                };
                self.theme_preset = items[next].to_string();
            }
            SettingKind::ThemeAccent => {
                let items = THEME_ACCENTS;
                let cur = items
                    .iter()
                    .position(|a| *a == self.theme_accent)
                    .unwrap_or(0);
                let n = items.len();
                let next = if forward {
                    (cur + 1) % n
                } else {
                    (cur + n - 1) % n
                };
                self.theme_accent = items[next].to_string();
            }
            SettingKind::Lang => {
                let items = LANGS;
                let cur = items.iter().position(|l| *l == self.lang).unwrap_or(0);
                let n = items.len();
                let next = if forward {
                    (cur + 1) % n
                } else {
                    (cur + n - 1) % n
                };
                self.lang = items[next];
            }
            // 켬/끔뿐이라 방향은 상관없다 — 좌우 어느 쪽이든 뒤집는다.
            SettingKind::Mouse => self.mouse = !self.mouse,
        }
    }

    pub fn apply(&mut self, up: Update) {
        if matches!(up, Update::Fetching) {
            // 시도 신호일 뿐 회복이 아니다 — stale/last_error에 손대지 않는다.
            self.fetching = true;
            return;
        }
        if let Update::News(n) = up {
            // 부가 기능: 본 기능의 stale/last_error, 스피너 생명주기에 관여하지 않는다.
            self.news = n;
            // 목록이 열린 채 뉴스가 0건으로 갱신되면 빈 오버레이가 남는 문제가 있었다
            // — 이 경우 목록을 닫는다. 0건이 아니면 기존대로 커서만 새 길이로
            // clamp한다(마지막 항목이 선택된 것처럼 보이는데 Enter가 안 먹던 문제,
            // saturating_sub로 패닉 없이 0에 멈춘다).
            if self.news.is_empty() {
                self.news_list = None;
            } else if let Some(list) = &mut self.news_list {
                list.cursor = list.cursor.min(self.news.len().saturating_sub(1));
            }
            return;
        }
        if let Update::Tips(t) = up {
            // 부가 기능: stale/last_error/fetching 생명주기에 관여하지 않는다.
            self.tips_override = Some(t);
            return;
        }
        self.fetching = false;
        self.stale = false;
        // last_error는 "현재 화면이 stale인 이유"를 보여주는 값이므로 stale과
        // 생명주기를 맞춘다 — 에러가 아닌 갱신이 오면 지워야 회복 후에도 footer에
        // 옛 에러가 영구히 남는 걸 막는다.
        if !matches!(up, Update::Error(_)) {
            self.last_error = None;
            // A-2: 여기 도달했다는 것은 up이 Games/Standings/Live/Error 중 하나이고
            // (Fetching/News/Tips는 위에서 이미 early return) 지금 Error가 아니라는
            // 뜻이다 — 즉 실제 데이터가 반영되는 성공 갱신에서만 스냅샷을 찍는다.
            self.last_update_secs = Some(self.now_secs);
        }
        match up {
            Update::Games(g) => {
                self.games = g;
                self.games_loaded = true;
                if self.selected >= self.games.len() {
                    self.selected = self.games.len().saturating_sub(1);
                }
                // 보고 있는 경기의 **상태도 따라 갱신한다.** 라이브 화면의 `Game`은
                // 진입 시점의 스냅샷이라, 보는 도중 경기가 끝나도 영원히 Live로
                // 남아 있었다. 그 결과 ①화면 배지가 "종료"로 안 바뀌고
                // ②폴러가 끝난 경기를 5초마다 계속 받는다(종료 경기용 완화 주기가
                // 있는데도 걸리지 않았다 — 하루 3.6GB를 상대 서버에서 받아 왔다).
                if let Screen::Live { game, .. } = &mut self.screen {
                    if let Some(fresh) = self.games.iter().find(|g| g.id == game.id) {
                        *game = fresh.clone();
                    }
                }
            }
            Update::Standings(s) => {
                self.standings = s;
                self.standings_loaded = true;
            }
            Update::Live(id, l) => {
                // 화면 전환 사이 도착한, 이전에 보던 게임의 느린 응답이 새로 선택된
                // 게임의 라이브 상태를 덮어쓰지 않도록 game id를 확인한다.
                if let Screen::Live { game, state } = &mut self.screen {
                    if game.id == id {
                        // 라이브 추종(v0.18 핵심): 과거 타석을 보는 중(live_atbat_sel
                        // = Some)이면 이 폴링이 "현재(라이브) 타석"의 투구 수를
                        // 바꿔도 사용자가 보고 있는 건 다른(이미 끝난) 타석이므로
                        // 그 선택을 건드리지 않는다 — 선택이 튀지 않는다는 게 이
                        // 기능의 핵심 UX. live_atbat_sel이 None(=최신을 보는 중)일
                        // 때만 기존 규칙을 적용한다: 새 타석이면 선택 리셋; 같은
                        // 타석에 투구가 추가된 경우는 선택 유지; 방어적으로 범위
                        // 밖 선택도 해제.
                        if let Some(seq) = self.live_atbat_sel {
                            // 보던 타석이 이번 응답에 없다(응답은 현재 이닝만 담으므로
                            // 이닝이 넘어가면 통째로 갈린다). 다른 타석을 그 자리인 척
                            // 계속 보여주는 대신 라이브로 되돌린다 — 조용히 어긋나느니
                            // 눈에 보이게 최신으로 돌아오는 편이 낫다.
                            if !l.has_at_bat(seq) {
                                self.live_atbat_sel = None;
                                self.live_relay_cursor = None;
                                self.live_pitch_sel = None;
                            }
                        } else {
                            // "새 타석이 시작됐다"는 신호 둘을 OR로 합친다(리뷰 I-2):
                            // ① at_bats.last().seq 변화 — 가장 정확한 신호(seq는
                            // 경기 전체에 걸친 타석 번호라 같은 타석이면 절대 안
                            // 바뀐다). ② current_pitches.len() 감소 — v0.17부터
                            // 있던 기존 신호로, at_bats가 비어 있는(구버전 손 조립
                            // 상태) 폴백 경로에서는 seq를 아예 볼 수 없으므로 여전히
                            // 필요하다. 어느 한쪽만 있어도 새 타석으로 본다 — seq가
                            // 바뀌었는데 마침 투구 수가 같은 경우(예: 둘 다 1구)를
                            // ②만으로는 놓치기 때문이다(실측: live_relay_cursor가
                            // 리셋되지 않고 다른 타자의 줄을 계속 가리켰다).
                            let seq_changed = match (&state, l.at_bats.last()) {
                                (Some(prev), Some(new_last)) => {
                                    prev.at_bats.last().map(|ab| ab.seq) != Some(new_last.seq)
                                }
                                _ => false,
                            };
                            let fewer_pitches = state.as_ref().is_some_and(|prev| {
                                l.current_pitches.len() < prev.current_pitches.len()
                            });
                            if seq_changed || fewer_pitches {
                                self.live_pitch_sel = None;
                                self.live_relay_cursor = None;
                            }
                            // I-3(v19a 리뷰): 이 클램프는 활성 배열
                            // (`active_pitches(live_atbat_sel)`) 기준이어야
                            // 옳다 — `current_pitches`는 우연히 그것과 같은
                            // 값일 뿐이다. 지금 동작은 바뀌지 않는다: 이
                            // else 분기는 `self.live_atbat_sel == None`일
                            // 때만 실행되고, `active_pitches(None)`은
                            // `at_bats.last().pitches`(비어 있으면
                            // `current_pitches`로 폴백)인데 파서가 마지막
                            // at-bat과 current_pitches를 항상 미러링해
                            // 두므로(model.rs LiveState::at_bats 문서 참고)
                            // 오늘은 두 축이 같은 값을 낸다. 축을 미리
                            // 맞춰 두는 이유는 다음 작업(문자중계↔투구
                            // 커서 병합)이 이 클램프를 되감기 중에도 타게
                            // 만들 수 있는데, 그때 `current_pitches`를 쓰면
                            // 과거 타석(짧은 배열)을 라이브 타석(계속 느는
                            // 배열) 길이로 잘못 재는 축 어긋남이 생기기
                            // 때문이다.
                            //
                            // 리뷰 M-2: 위 두 신호(seq_changed/fewer_pitches)에
                            // 안 걸렸는데도 배열이 줄어 이 클램프가 단독으로
                            // 걸리는 경우, live_pitch_sel만 비우고 커서는
                            // 그대로 두면 두 필드가 어긋난다 — 화면은
                            // "커서가 이긴다" 규칙 덕에 안 깨지지만, 다음
                            // `←`/`→` 입력이 옛 커서 위치에서 다시 계산돼
                            // 예상과 다른 자리로 튄다. 다른 모든 리셋 자리와
                            // 규칙을 하나로 맞추기 위해 커서도 함께 비운다.
                            if let Some(i) = self.live_pitch_sel {
                                if i >= l.active_pitches(self.live_atbat_sel).len() {
                                    self.live_pitch_sel = None;
                                    self.live_relay_cursor = None;
                                }
                            }
                        }
                        let mut l = l;
                        let cached = self.past_innings.get(&id);
                        if let Some(past) = cached {
                            Self::merge_past_innings(past, &mut l);
                        }
                        *state = Some(l);
                    }
                }
            }
            Update::Inning {
                game_id,
                inning,
                at_bats,
            } => {
                // Live와 같은 이유로 game id를 확인한다 — 화면을 옮긴 뒤 도착한
                // 이전 게임의 응답이 지금 보는 경기에 남의 타석을 섞으면 안 된다.
                let matches_screen =
                    matches!(&self.screen, Screen::Live { game, .. } if game.id == game_id);
                if matches_screen {
                    self.past_innings
                        .entry(game_id.clone())
                        .or_default()
                        .insert(inning, at_bats);
                    let cached = self.past_innings.get(&game_id).cloned();
                    if let (Some(past), Screen::Live { state: Some(s), .. }) =
                        (cached, &mut self.screen)
                    {
                        Self::merge_past_innings(&past, s);
                    }
                }
                // 요청한 그 이닝이 왔을 때만 푼다. 다른 이닝의 뒤늦은 응답이
                // 진행 중인 요청 표시를 지우면 로딩이 사라진 채로 대기하게 된다.
                if self.fetching_inning == Some(inning) {
                    self.fetching_inning = None;
                }
            }
            Update::Error(e) => {
                self.last_error = Some(e);
                self.stale = true;
                // 실패한 요청의 로딩 표시는 걷는다 — 캐시는 채우지 않으므로 사용자가
                // 다시 눌러 재시도할 수 있다.
                self.fetching_inning = None;
            }
            // compiler-mandated exhaustiveness arms; Fetching/News/Tips는 위 early return이
            // 전부 처리한다. unreachable!()로 두면 미래 리팩토링(early return 제거)이 곧바로
            // 런타임 패닉이 된다 — 이 함수는 렌더 루프에서 catch_unwind 없이 매 Update마다
            // 호출된다(무패닉 원칙).
            Update::Fetching => {}
            Update::News(_) => {}
            Update::Tips(_) => {}
        }
    }

    /// 현재 화면이 요구하는 폴링 대상을 폴러에 알리기 위한 헬퍼(main에서 사용).
    pub fn watched_game(&self) -> Option<&Game> {
        if let Screen::Live { game, .. } = &self.screen {
            Some(game)
        } else {
            None
        }
    }

    /// "이번 프레임이 보여주는 화면이 무엇인가"를 식별하는 최소 키(screen, tab, 오버레이).
    /// main.rs의 렌더 루프가 직전 프레임의 키와 비교해 값이 달라졌을 때만
    /// `term.clear()`를 호출한다 — ratatui 0.30에서 화면 전환(Live↔List,
    /// Games↔Standings) 시 내부 버퍼와 실제 터미널이 어긋나 이전 화면의 착색
    /// 셀이 지워지지 않는 문제(ADR-0007)를 잡기 위함이다.
    ///
    /// **오버레이도 이 키에 포함한다(v0.31).** v0.30까지는 "오버레이는 `Clear`
    /// 위젯이 처리하므로 넣으면 괜한 깜빡임만 생긴다"고 적어 두고 뺐는데, 실측이
    /// 그 전제를 뒤집었다 — 라이브 화면(스트라이크존·측면 궤적의 착색 브라유
    /// 글리프)에서 설정을 열면 그 색 셀 몇 개가 오버레이 위에 그대로 남았다.
    /// 사용자에게는 빈 화면에 색 점이 박힌 **데드픽셀**로 보였다(지적
    /// 2026-08-02). `Clear`는 ratatui 버퍼만 비우므로, diff가 그 셀을 "안 바뀜"
    /// 으로 판정하면 터미널에는 아무것도 안 나가고 옛 색이 살아남는다.
    /// 여기 포함해 전체 재그리기를 한 번 강제하면 사라진다(vhs 녹화로 전후 비교
    /// 확인). 클리어는 오버레이를 열고 닫을 때 한 프레임뿐이고 동기화 출력
    /// (BSU/ESU)으로 감싸므로 깜빡임은 관측되지 않았다.
    pub fn view_key(&self) -> (u8, u8, bool) {
        let screen = match self.screen {
            Screen::List => 0,
            Screen::Live { .. } => 1,
        };
        let tab = match self.tab {
            Tab::Games => 0,
            Tab::Standings => 1,
        };
        let overlay = self.show_help
            || self.settings.is_some()
            || self.options.is_some()
            || self.link_picker.is_some()
            || self.news_list.is_some()
            || self.article_view.is_some()
            || self.team_stats_target().is_some();
        (screen, tab, overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Game, GameStatus, Team};
    use crossterm::event::KeyCode;

    fn game(id: &str) -> Game {
        Game {
            id: id.into(),
            start: "".into(),
            status: GameStatus::Live,
            status_label: "".into(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            home_score: Some(1),
            away_score: Some(2),
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
        }
    }

    // ── 마우스(v0.27) ──────────────────────────────────────────────────────
    // 클릭은 **고르고 나서 연다**. 한 번에 여는 편이 빠르지만, 옆줄을 잘못 눌러
    // 남의 경기로 들어가면 되돌리는 값이 크다.

    #[test]
    fn a_click_on_another_row_selects_it_without_opening() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.games = vec![game("a"), game("b")];
        app.on_mouse(Some(crate::ui::hit::Zone::GameRow(1)), MouseAction::Click);
        assert_eq!(app.selected, 1);
        assert!(
            matches!(app.screen, Screen::List),
            "첫 클릭은 고르기만 해야 한다"
        );
    }

    #[test]
    fn a_click_on_the_selected_row_opens_it() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.games = vec![game("a"), game("b")];
        app.selected = 1;
        app.on_mouse(Some(crate::ui::hit::Zone::GameRow(1)), MouseAction::Click);
        assert!(
            matches!(&app.screen, Screen::Live { game, .. } if game.id == "b"),
            "고른 행을 다시 누르면 그 경기로 들어가야 한다"
        );
    }

    /// 탭 클릭은 "저기로"다 — `Tab` 키처럼 "다음으로"가 아니다. 지금은 탭이
    /// 둘뿐이라 결과가 같지만, 이미 그 탭에 있으면 아무 일도 없어야 한다.
    #[test]
    fn clicking_a_tab_goes_to_that_tab_not_the_next_one() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.on_mouse(
            Some(crate::ui::hit::Zone::Tab(Tab::Standings)),
            MouseAction::Click,
        );
        assert_eq!(app.tab, Tab::Standings);
        app.on_mouse(
            Some(crate::ui::hit::Zone::Tab(Tab::Standings)),
            MouseAction::Click,
        );
        assert_eq!(app.tab, Tab::Standings, "같은 탭을 다시 눌러도 안 넘어간다");
    }

    /// 라이브에서 탭을 누르면 목록으로 나가면서 그 탭으로 간다(키 `Tab`과 같은
    /// 철학 — 헤더만 바뀌고 본문이 그대로면 혼란스럽다).
    #[test]
    fn clicking_a_tab_from_the_live_screen_leaves_it() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.enter_live(game("a"));
        app.on_mouse(
            Some(crate::ui::hit::Zone::Tab(Tab::Games)),
            MouseAction::Click,
        );
        assert!(matches!(app.screen, Screen::List));
    }

    /// 오버레이가 떠 있으면 마우스는 아무것도 하지 않는다. 히트맵에는 그 아래
    /// 화면의 영역이 남아 있어서, 처리하면 **보고 있지도 않은 것**이 움직인다.
    #[test]
    fn the_mouse_does_nothing_while_an_overlay_is_open() {
        let mut app = App::new(Default::default());
        app.games = vec![game("a"), game("b")];
        app.show_help = true;
        app.on_mouse(Some(crate::ui::hit::Zone::GameRow(1)), MouseAction::Click);
        assert_eq!(app.selected, 0, "도움말이 떠 있는데 목록이 움직였다");
        assert!(app.show_help, "마우스로 오버레이가 닫히면 안 된다");
    }

    /// 빈 곳에서의 휠은 아무것도 하지 않는다 — 어느 목록을 굴릴지 짐작하지 않는다.
    #[test]
    fn a_scroll_over_nothing_does_nothing() {
        let mut app = App::new(Default::default());
        app.games = vec![game("a"), game("b")];
        app.on_mouse(None, MouseAction::ScrollDown);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn a_scroll_over_the_list_moves_the_selection() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.games = vec![game("a"), game("b")];
        app.on_mouse(
            Some(crate::ui::hit::Zone::GameRow(0)),
            MouseAction::ScrollDown,
        );
        assert_eq!(app.selected, 1);
        app.on_mouse(
            Some(crate::ui::hit::Zone::GameRow(1)),
            MouseAction::ScrollUp,
        );
        assert_eq!(app.selected, 0);
    }

    /// 껐으면 이벤트가 와도 무시한다. main이 캡처를 풀지만 그것만 믿으면,
    /// 모드 전환을 무시하는 터미널에서 **끈 뒤에도 화면이 움직인다**(pty로
    /// 실제 SGR 이벤트를 흘려 넣어 확인한 실제 결함이다).
    #[test]
    fn turning_the_mouse_off_ignores_events_too() {
        let mut app = App::new(Default::default());
        app.games = vec![game("a"), game("b")];
        app.mouse = false;
        app.on_mouse(Some(crate::ui::hit::Zone::GameRow(1)), MouseAction::Click);
        app.on_mouse(
            Some(crate::ui::hit::Zone::GameRow(0)),
            MouseAction::ScrollDown,
        );
        assert_eq!(app.selected, 0, "껐는데 마우스가 목록을 움직였다");
    }

    /// 설정 화면에서 마우스 행을 좌우로 누르면 켬/끔이 뒤집힌다.
    /// (파일까지 실려 가는지는 config.rs의 `mouse_survives_a_round_trip`이 본다.)
    #[test]
    fn the_settings_row_toggles_the_mouse() {
        let mut app = App::new(Default::default());
        let idx = app
            .settings_rows()
            .iter()
            .position(|(k, _, _)| matches!(k, SettingKind::Mouse))
            .expect("설정에 마우스 행이 없다");
        assert!(!app.mouse, "기본값은 끔이다(v0.31)");
        app.change_setting(idx, true);
        assert!(app.mouse);
        app.change_setting(idx, false);
        assert!(!app.mouse, "반대 방향으로도 뒤집혀야 한다");
    }

    /// 오버레이가 **화면에서 사라지면** 마우스도 함께 풀려야 한다.
    ///
    /// v0.24가 렌더와 키 소비를 `team_stats_target()` 하나로 통일한 이유가
    /// 그대로 여기 적용된다 — v0.27의 마우스 경로만 `team_stats_rank`를 직접
    /// 봐서, rank는 남았는데 그 팀이 순위표에서 사라진 순간(폴링 갱신)
    /// **키는 듣는데 마우스만 죽는** 상태가 생겼다.
    #[test]
    fn the_mouse_unlocks_when_the_overlay_disappears() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.games = vec![game("a"), game("b")];
        app.standings = vec![standing_with_games(1, 10)];
        app.team_stats_rank = Some(1);
        // 폴링이 순위표를 갱신해 그 팀이 사라진다.
        app.standings = vec![];
        assert!(
            app.team_stats_target().is_none(),
            "오버레이는 이미 안 보인다"
        );
        // 키는 정상 동작한다.
        app.on_key(KeyCode::Down);
        assert_eq!(app.selected, 1, "키는 살아 있다");
        // 마우스는?
        app.on_mouse(Some(crate::ui::hit::Zone::GameRow(0)), MouseAction::Click);
        assert_eq!(app.selected, 0, "마우스만 잠겨 있으면 여기서 1로 남는다");
    }

    /// 클릭도 `gg` 대기를 끊는다. on_key는 **모든** 분기에서 이걸 지운다 —
    /// "입력이 오면 g 시퀀스는 끝난다"가 그쪽 계약이고, 마우스만 예외로 두면
    /// 클릭해서 고른 행이 다음 `g` 하나에 맨 위로 날아간다.
    #[test]
    fn a_click_cancels_a_pending_g() {
        let mut app = App::new(Default::default());
        // 마우스는 기본이 꺼짐이다(v0.31) — 마우스 동작을 보는 테스트는 켜고 시작한다.
        app.mouse = true;
        app.games = vec![game("a"), game("b"), game("c")];
        app.on_key(KeyCode::Char('g'));
        app.on_mouse(Some(crate::ui::hit::Zone::GameRow(2)), MouseAction::Click);
        assert_eq!(app.selected, 2);
        app.on_key(KeyCode::Char('g'));
        assert_eq!(app.selected, 2, "클릭 뒤의 g 하나가 gg로 읽혀 맨 위로 갔다");
    }

    /// **깨진 설정 파일은 덮어쓰지 않는다.**
    ///
    /// 기본값으로 갈아탄 상태를 저장해 버리면 사용자가 손으로 적어 둔 것들이
    /// 진짜로 사라진다 — 오타 하나 고치면 되는 상황이 복구 불가가 된다.
    #[test]
    fn a_broken_config_is_never_overwritten() {
        let mut app = App::new(Default::default());
        app.config_error = Some("invalid type: string \"yes\"".into());
        app.settings = Some(SettingsState {
            cursor: 0,
            save_failed: false,
        });
        app.persist();
        assert!(
            app.settings
                .as_ref()
                .map(|s| s.save_failed)
                .unwrap_or(false),
            "저장을 막았으면 그 사실을 화면에 알려야 한다"
        );
    }

    /// **보는 도중 경기가 끝나면 화면의 경기 상태도 따라가야 한다.**
    ///
    /// 라이브 화면의 `Game`은 진입 시점 스냅샷이라 영원히 Live로 남아 있었다.
    /// 그래서 ①배지가 "종료"로 안 바뀌고 ②폴러가 끝난 경기를 5초마다 계속
    /// 받았다(종료 경기용 완화 주기가 있는데도 걸리지 않았다). 기존 폴러 테스트는
    /// "이미 끝난 경기에 새로 들어갈 때"만 봐서 이 전이를 못 잡았다.
    #[test]
    fn a_game_that_ends_while_we_watch_it_updates_on_screen() {
        let mut app = App::new(Default::default());
        let mut g = game("a");
        g.status = GameStatus::Live;
        app.enter_live(g.clone());

        // 폴링이 "그 경기 끝났다"고 알려 온다.
        let mut finished = g.clone();
        finished.status = GameStatus::Final;
        app.apply(crate::poller::Update::Games(vec![finished]));

        let watched = app.watched_game().expect("라이브 화면인데 경기가 없다");
        assert_eq!(
            watched.status,
            GameStatus::Final,
            "경기가 끝났는데 화면은 아직 진행 중으로 안다"
        );
    }

    /// 다른 경기의 소식이 와도 보고 있는 것을 바꾸지 않는다(위 갱신이 과하지 않은지).
    #[test]
    fn another_games_update_does_not_touch_the_one_we_watch() {
        let mut app = App::new(Default::default());
        let mut g = game("a");
        g.status = GameStatus::Live;
        app.enter_live(g);

        let mut other = game("b");
        other.status = GameStatus::Final;
        app.apply(crate::poller::Update::Games(vec![other]));

        assert_eq!(
            app.watched_game().unwrap().status,
            GameStatus::Live,
            "남의 경기 소식이 우리 화면을 바꿨다"
        );
    }

    /// **더블헤더에서 `--team`은 진행 중인 경기로 들어간다.**
    ///
    /// API가 시작시각 오름차순으로 주므로 첫 일치는 늘 1차전이다. 2차전이
    /// 진행 중인데도 이미 끝난 1차전 화면에 갇혔다(실측: 2025-05-11 롯데·NC·KIA
    /// 전부 그랬다).
    #[test]
    fn a_double_header_enters_the_game_that_is_actually_on() {
        let mut first = game("g1");
        first.status = GameStatus::Final;
        let mut second = game("g2");
        second.status = GameStatus::Live;
        // API 순서 그대로: 1차전이 앞에 온다.
        let games = vec![first, second];
        let picked = pick_team_game(&games, "LG").expect("LG 경기가 있다");
        assert_eq!(picked.id, "g2", "끝난 1차전으로 들어갔다");
    }

    /// 둘 다 안 끝났으면 먼저 오는 것(1차전)을 고른다.
    #[test]
    fn a_double_header_before_first_pitch_picks_the_earlier_one() {
        let mut first = game("g1");
        first.status = GameStatus::Scheduled;
        let mut second = game("g2");
        second.status = GameStatus::Scheduled;
        let games = vec![first, second];
        assert_eq!(pick_team_game(&games, "LG").unwrap().id, "g1");
    }

    #[test]
    fn tab_toggles_between_games_and_standings() {
        let mut app = App::new(Default::default());
        assert_eq!(app.tab, Tab::Games);
        app.on_key(KeyCode::Tab);
        assert_eq!(app.tab, Tab::Standings);
    }

    /// view_key는 main.rs가 화면 전환 시에만 term.clear()를 부르기 위한
    /// 최소 식별자다 — Tab으로 Games↔Standings를 오가면 값이 달라져야 한다.
    #[test]
    fn view_key_changes_when_tab_switches() {
        let mut app = App::new(Default::default());
        let before = app.view_key();
        app.on_key(KeyCode::Tab);
        let after = app.view_key();
        assert_ne!(before, after);
    }

    /// List↔Live 전환도 view_key가 달라져야 한다(오늘 라이브 배지 잔상 버그의
    /// 실제 전환 축).
    #[test]
    fn view_key_changes_when_entering_and_leaving_live() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("a")]));
        let list_key = app.view_key();
        app.on_key(KeyCode::Enter);
        assert!(matches!(app.screen, Screen::Live { .. }));
        let live_key = app.view_key();
        assert_ne!(list_key, live_key);

        // Live 화면에서 Tab을 누르면 List로 나가면서 탭도 바뀐다(위 on_key 주석
        // 참고) — view_key도 다시 달라져야 한다.
        app.on_key(KeyCode::Tab);
        assert!(matches!(app.screen, Screen::List));
        let after_tab = app.view_key();
        assert_ne!(live_key, after_tab);
    }

    /// **오버레이는 view_key를 바꾼다(v0.31에서 뒤집힘).** 예전엔 "Clear 위젯이
    /// 처리하니 포함하면 깜빡임만 생긴다"고 반대로 단언하던 자리다 — 실제로는
    /// 라이브 화면의 착색 셀이 오버레이 위에 남아 데드픽셀로 보였다. 위 단위
    /// 테스트가 필드를 직접 세우는 반면 여기서는 **실제 키 경로**로 열고 닫아,
    /// 핸들러가 다른 상태 필드를 쓰기 시작해도 이 계약이 유지되는지 본다.
    #[test]
    fn every_overlay_key_toggles_the_view_key_and_puts_it_back() {
        let mut app = App::new(Default::default());
        // `o`는 열 링크가 있어야 픽커가 뜬다 — 경기가 없으면 아무 일도 안 한다.
        app.games = vec![game("a")];
        let base = app.view_key();

        for (open, close, what) in [
            (KeyCode::Char('?'), KeyCode::Char('?'), "help"),
            (KeyCode::F(2), KeyCode::F(2), "options"),
            (KeyCode::Char('o'), KeyCode::Esc, "link picker"),
            (KeyCode::F(9), KeyCode::F(9), "settings"),
        ] {
            app.on_key(open);
            assert_ne!(app.view_key(), base, "{what}를 열었는데 화면 키가 그대로다");
            app.on_key(close);
            assert_eq!(app.view_key(), base, "{what}를 닫았는데 키가 안 돌아왔다");
        }
    }

    #[test]
    fn down_moves_selection_within_bounds() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("a"), game("b")]));
        assert_eq!(app.selected, 0);
        app.on_key(KeyCode::Down);
        assert_eq!(app.selected, 1);
        app.on_key(KeyCode::Down); // 경계에서 멈춤
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn q_signals_quit() {
        let mut app = App::new(Default::default());
        assert!(app.on_key(KeyCode::Char('q')));
    }

    #[test]
    fn enter_opens_live_screen_for_selected_game() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("a")]));
        app.on_key(KeyCode::Enter);
        assert!(matches!(app.screen, Screen::Live { .. }));
    }

    fn game_with_status(id: &str, status: GameStatus) -> Game {
        let mut g = game(id);
        g.status = status;
        g
    }

    #[test]
    fn enter_does_not_open_live_for_canceled_game() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game_with_status(
            "a",
            GameStatus::Canceled,
        )]));
        app.on_key(KeyCode::Enter);
        assert!(matches!(app.screen, Screen::List));
    }

    #[test]
    fn enter_does_not_open_live_for_scheduled_game() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game_with_status(
            "a",
            GameStatus::Scheduled,
        )]));
        app.on_key(KeyCode::Enter);
        assert!(matches!(app.screen, Screen::List));
    }

    #[test]
    fn stale_live_update_for_previous_game_does_not_overwrite_newly_watched_game() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("a"), game("b")]));
        // "a"를 보다가 "b"로 전환한 상황을 흉내낸다.
        app.screen = Screen::Live {
            game: game("b"),
            state: None,
        };
        // 전환 전에 날아간, "a"용으로 가져온 느린 응답이 뒤늦게 도착.
        let stale_state = crate::source::naver::map::live_from_relay(
            include_str!("../tests/fixtures/relay_20260719KTLG.json"),
            Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            Team {
                code: "KT".into(),
                name: "KT".into(),
            },
        )
        .unwrap();
        app.apply(crate::poller::Update::Live("a".into(), stale_state));
        if let Screen::Live { state, .. } = &app.screen {
            assert!(
                state.is_none(),
                "stale update for a stale id must be dropped"
            );
        } else {
            panic!("expected Screen::Live");
        }
    }

    #[test]
    fn f1_toggles_help() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::F(1));
        assert!(app.show_help);
        app.on_key(KeyCode::Char('x'));
        assert!(!app.show_help);
    }

    #[test]
    fn f10_quits() {
        let mut app = App::new(Default::default());
        assert!(app.on_key(KeyCode::F(10)));
    }

    #[test]
    fn f5_switches_tab() {
        let mut app = App::new(Default::default());
        assert_eq!(app.tab, Tab::Games);
        app.on_key(KeyCode::F(5));
        assert_eq!(app.tab, Tab::Standings);
    }

    #[test]
    fn gg_jumps_to_top_and_g_to_bottom() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![
            game("a"),
            game("b"),
            game("c"),
        ]));
        app.on_key(KeyCode::Down);
        app.on_key(KeyCode::Down);
        assert_eq!(app.selected, 2);
        app.on_key(KeyCode::Char('g'));
        app.on_key(KeyCode::Char('g'));
        assert_eq!(app.selected, 0);
        app.on_key(KeyCode::Char('G'));
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn esc_on_list_does_not_quit() {
        let mut app = App::new(Default::default());
        assert!(!app.on_key(KeyCode::Esc));
    }

    #[test]
    fn apply_error_sets_last_error_and_marks_stale() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Error("boom".into()));
        assert_eq!(app.last_error.as_deref(), Some("boom"));
        assert!(app.stale);
    }

    #[test]
    fn a_later_non_error_update_clears_last_error() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Error("boom".into()));
        assert!(app.last_error.is_some());
        app.apply(crate::poller::Update::Games(vec![game("a")]));
        assert_eq!(app.last_error, None);
        assert!(!app.stale);
    }

    #[test]
    fn fetching_update_raises_flag_and_next_data_update_clears_it() {
        let mut app = App::new(Default::default());
        assert!(!app.fetching);
        app.apply(crate::poller::Update::Fetching);
        assert!(app.fetching);
        app.apply(crate::poller::Update::Games(vec![]));
        assert!(!app.fetching);
    }

    /// Fetching은 "시도"지 "회복"이 아니다 — stale/last_error를 지우면 안 된다.
    #[test]
    fn fetching_does_not_clear_stale_or_last_error() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Error("boom".into()));
        app.apply(crate::poller::Update::Fetching);
        assert!(app.stale);
        assert_eq!(app.last_error.as_deref(), Some("boom"));
    }

    /// A-2: 성공 갱신(Games)이 now_secs 스냅샷으로 last_update_secs를 채운다.
    #[test]
    fn successful_data_update_sets_last_update_secs_to_now() {
        let mut app = App::new(Default::default());
        app.now_secs = 1_800_000_000;
        assert_eq!(app.last_update_secs, None);
        app.apply(crate::poller::Update::Games(vec![game("a")]));
        assert_eq!(app.last_update_secs, Some(1_800_000_000));
    }

    /// A-2: Error는 "시도"가 아니라 실패 신호지만, Games/Live처럼 실제 데이터를
    /// 반영하는 게 아니므로 last_update_secs를 건드리면 안 된다(stale과 짝).
    #[test]
    fn error_update_does_not_set_last_update_secs() {
        let mut app = App::new(Default::default());
        app.now_secs = 1_800_000_000;
        app.apply(crate::poller::Update::Error("boom".into()));
        assert_eq!(app.last_update_secs, None);
    }

    /// A-2: Fetching은 "시도 신호일 뿐 회복이 아니다"(`last_update_secs` 필드 주석과 동일
    /// 철학) — last_update_secs도 stale/last_error와 같은 생명주기를 따른다.
    #[test]
    fn fetching_update_does_not_set_last_update_secs() {
        let mut app = App::new(Default::default());
        app.now_secs = 1_800_000_000;
        app.apply(crate::poller::Update::Fetching);
        assert_eq!(app.last_update_secs, None);
    }

    /// 한 번 성공 갱신을 찍은 뒤 도착하는 Fetching 신호가 그 값을 지우거나
    /// 건드리지 않아야 한다(다음 폴 사이클 진행 중에도 "마지막 성공"은 유지).
    #[test]
    fn last_update_secs_survives_a_later_fetching_signal() {
        let mut app = App::new(Default::default());
        app.now_secs = 1_800_000_000;
        app.apply(crate::poller::Update::Games(vec![]));
        assert_eq!(app.last_update_secs, Some(1_800_000_000));
        app.now_secs = 1_800_000_100;
        app.apply(crate::poller::Update::Fetching);
        assert_eq!(
            app.last_update_secs,
            Some(1_800_000_000),
            "Fetching must not touch last_update_secs"
        );
    }

    #[test]
    fn g_then_other_key_clears_pending() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![
            game("a"),
            game("b"),
            game("c"),
        ]));
        app.on_key(KeyCode::Down); // selected = 1
        app.on_key(KeyCode::Char('g')); // pending_g armed
        app.on_key(KeyCode::Down); // interleaved key → must clear pending_g, selected = 2
        app.on_key(KeyCode::Char('g')); // lone g: arms pending again, must NOT jump to top
        assert_ne!(app.selected, 0); // if pending had lingered, this g would have jumped to 0
    }

    fn live_app_with_pitches(n: u8) -> App {
        let mut app = App::new(Default::default());
        let pitches: Vec<crate::model::Pitch> = (1..=n)
            .map(|i| crate::model::Pitch {
                order: i,
                ..Default::default()
            })
            .collect();
        let state = crate::model::LiveState {
            inning_label: "T1".into(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            home_score: 0,
            away_score: 0,
            count: crate::model::Count {
                ball: 0,
                strike: 0,
                out: 0,
            },
            bases: crate::model::BaseState {
                first: false,
                second: false,
                third: false,
            },
            pitcher_name: String::new(),
            batter_name: String::new(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: pitches.clone(),
            next_batter_name: String::new(),
            // 단일 at-bat으로 미러링해 active_pitches/active_relay_lines가
            // current_pitches와 항상 같은 값을 내도록 한다(v0.18 이전
            // 헬퍼가 이 필드를 몰라도 되는 폴백과 별개로, 새 네비 테스트가
            // 이 상태를 재사용할 수 있게).
            at_bats: vec![crate::model::AtBat {
                seq: 1,
                batter_name: String::new(),
                inning_label: "T1".into(),
                relay_lines: vec![],
                pitches,
            }],
            inning_score: Vec::new(),
            batter_line: None,
            pitcher_line: None,
            matchup: String::new(),
        };
        app.screen = Screen::Live {
            game: game("g"),
            state: Some(state),
        };
        app
    }

    #[test]
    fn right_selects_first_pitch_then_advances_and_stops_at_last() {
        let mut app = live_app_with_pitches(3);
        assert_eq!(app.live_pitch_sel, None);
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, Some(0));
        app.on_key(KeyCode::Right);
        app.on_key(KeyCode::Right);
        app.on_key(KeyCode::Right); // 경계 정지
        assert_eq!(app.live_pitch_sel, Some(2));
    }

    #[test]
    fn left_enters_from_the_last_pitch() {
        let mut app = live_app_with_pitches(3);
        app.on_key(KeyCode::Left);
        assert_eq!(app.live_pitch_sel, Some(2));
        app.on_key(KeyCode::Left);
        assert_eq!(app.live_pitch_sel, Some(1));
    }

    #[test]
    fn esc_clears_selection_first_then_leaves_live() {
        let mut app = live_app_with_pitches(2);
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, Some(0));
        app.on_key(KeyCode::Esc); // 1단계: 선택 해제, 화면 유지
        assert_eq!(app.live_pitch_sel, None);
        assert!(matches!(app.screen, Screen::Live { .. }));
        app.on_key(KeyCode::Esc); // 2단계: 목록 복귀
        assert!(matches!(app.screen, Screen::List));
    }

    /// v0.18 돌려보기 테스트용: n개의 at-bat을 오래된→최신 순으로 담은 App.
    /// 각 at-bat은 batter_name("batter{i}")·inning_label·문자중계 2줄·투구
    /// 1개로 서로 구분된다. 마지막(index n-1) at-bat이 "현재(라이브)"이므로
    /// relay_log/current_pitches도 거기서 미러링해 둔다 — active_*()가 기대하는
    /// "마지막 at-bat = 레거시 필드"라는 파서의 불변식을 테스트 상태에도 맞춘다.
    ///
    /// v0.19: 문자중계 두 줄 중 **아래(최신) 줄만** 그 타석의 유일한 투구와
    /// 짝지어 둔다 — 실제 응답의 타석 모양(타자 등장 안내 → 투구 줄)을 최소로
    /// 흉내 낸 것이라, 연동이 들어온 뒤에도 "투구 줄"과 "투구가 아닌 줄"이 둘 다
    /// 테스트에 존재한다.
    fn live_app_with_at_bats(n: usize) -> App {
        let mut app = App::new(Default::default());
        let at_bats: Vec<crate::model::AtBat> = (0..n)
            .map(|i| crate::model::AtBat {
                // 실제 응답의 no처럼 오래된→최신으로 증가하는 번호. 0부터가 아니라
                // 100부터 시작해 "인덱스와 번호가 우연히 같아 통과하는" 테스트를 막는다.
                seq: 100 + i as i64,
                batter_name: format!("batter{i}"),
                inning_label: format!("T{}", i + 1),
                relay_lines: vec![
                    crate::model::RelayLine::plain(format!("line-{i}-a")),
                    crate::model::RelayLine {
                        text: format!("line-{i}-b"),
                        pitch_idx: Some(0),
                        is_pitch: true,
                        time_hm: None,
                    },
                ],
                pitches: vec![crate::model::Pitch {
                    order: 1,
                    ..Default::default()
                }],
            })
            .collect();
        let last = at_bats.last().expect("n > 0").clone();
        let state = crate::model::LiveState {
            inning_label: last.inning_label.clone(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            home_score: 0,
            away_score: 0,
            count: crate::model::Count {
                ball: 0,
                strike: 0,
                out: 0,
            },
            bases: crate::model::BaseState {
                first: false,
                second: false,
                third: false,
            },
            pitcher_name: String::new(),
            batter_name: last.batter_name.clone(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: last.relay_lines.clone(),
            current_pitches: last.pitches.clone(),
            next_batter_name: String::new(),
            at_bats,
            inning_score: Vec::new(),
            batter_line: None,
            pitcher_line: None,
            matchup: String::new(),
        };
        app.screen = Screen::Live {
            game: game("g"),
            state: Some(state),
        };
        app
    }

    /// `[`/`]`는 이전/다음 타석으로 움직이고 양끝에서 clamp된다. 최신까지
    /// `]`로 다시 따라오면 live_atbat_sel이 None(라이브 추종)으로 되돌아간다.
    #[test]
    fn bracket_keys_navigate_between_at_bats_and_clamp_at_the_boundaries() {
        let mut app = live_app_with_at_bats(3); // batter0(oldest) .. batter2(live)
        assert_eq!(app.live_atbat_sel, None, "기본은 최신(라이브)을 본다");

        app.on_key(KeyCode::Char('['));
        assert_eq!(app.live_atbat_sel, Some(101));
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.live_atbat_sel, Some(100), "가장 오래된 타석");
        app.on_key(KeyCode::Char('[')); // 경계: 더 못 감
        assert_eq!(app.live_atbat_sel, Some(100));

        app.on_key(KeyCode::Char(']'));
        assert_eq!(app.live_atbat_sel, Some(101));
        app.on_key(KeyCode::Char(']')); // 최신에 도달 → 라이브 추종 재개
        assert_eq!(app.live_atbat_sel, None);
        app.on_key(KeyCode::Char(']')); // 이미 최신 — no-op
        assert_eq!(app.live_atbat_sel, None);
    }

    /// at-bat이 하나뿐이면(돌려볼 과거가 없음) `[`/`]`는 무패닉 no-op이다.
    #[test]
    fn bracket_keys_are_noop_with_a_single_at_bat() {
        let mut app = live_app_with_at_bats(1);
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.live_atbat_sel, None);
        app.on_key(KeyCode::Char(']'));
        assert_eq!(app.live_atbat_sel, None);
    }

    /// M-2: `no`가 결측이면(lenient_int 관용 파싱) 여러 at-bat이 전부 seq==0
    /// 으로 뭉개질 수 있다 — 그 상태에서 `[`/`]`를 쓰면 position()이 항상
    /// 첫 일치 항목만 찾아 되감기가 그 자리에 갇히고, `]`를 아무리 눌러도
    /// 라이브(None)로 복귀할 수 없다(실측 재현). seq 유일성이 깨지면 App은
    /// 되감기 네비게이션 자체를 비활성화해 잘못된 자리에 갇히는 것보다
    /// 안전한 쪽을 택한다.
    #[test]
    fn bracket_keys_are_disabled_when_at_bat_seqs_collide_due_to_a_missing_no() {
        let mut app = live_app_with_at_bats(3);
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            for ab in s.at_bats.iter_mut() {
                ab.seq = 0; // "no" 결측 시뮬레이션 — 전 항목 충돌
            }
        }
        app.on_key(KeyCode::Char('['));
        assert_eq!(
            app.live_atbat_sel, None,
            "seq가 유일하지 않으면 되감기 네비게이션이 무동작이어야 한다"
        );
        app.on_key(KeyCode::Char(']'));
        assert_eq!(app.live_atbat_sel, None);
    }

    /// List 화면에서는 `[`/`]`가 아무 것도 하지 않는다(패닉 없음).
    #[test]
    fn bracket_keys_are_noop_on_list_screen() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.live_atbat_sel, None);
        app.on_key(KeyCode::Char(']'));
        assert_eq!(app.live_atbat_sel, None);
    }

    /// 과거 타석을 보는 중에도 Left/Right로 그 타석의 투구를 하나씩 짚을 수
    /// 있다 — live_pitch_sel과 live_atbat_sel은 독립적으로 조합된다.
    #[test]
    fn pitch_selection_combines_with_rewinding_to_a_past_at_bat() {
        let mut app = live_app_with_at_bats(3);
        app.on_key(KeyCode::Char('[')); // batter1(과거)로 이동
        assert_eq!(app.live_atbat_sel, Some(101));
        app.on_key(KeyCode::Right);
        assert_eq!(
            app.live_pitch_sel,
            Some(0),
            "과거 타석 안에서도 투구 선택이 동작해야 한다"
        );
    }

    /// j/k(문자중계 줄 커서, v0.18): 처음 누르면 항상 맨 아래(최신 줄)에서
    /// 시작하고, 그 뒤로는 방향대로 움직이며 양끝에서 clamp된다.
    #[test]
    fn j_k_move_the_relay_cursor_starting_from_the_bottom() {
        let mut app = live_app_with_at_bats(1); // relay_lines: ["line-0-a", "line-0-b"]
        assert_eq!(app.live_relay_cursor, None);
        app.on_key(KeyCode::Char('k'));
        assert_eq!(
            app.live_relay_cursor,
            Some(1),
            "처음 누르면 맨 아래에서 시작"
        );
        app.on_key(KeyCode::Char('k'));
        assert_eq!(app.live_relay_cursor, Some(0));
        app.on_key(KeyCode::Char('k')); // 경계: 더 못 감
        assert_eq!(app.live_relay_cursor, Some(0));
        app.on_key(KeyCode::Char('j'));
        assert_eq!(app.live_relay_cursor, Some(1));
        app.on_key(KeyCode::Char('j')); // 경계: 이미 맨 아래
        assert_eq!(app.live_relay_cursor, Some(1));
    }

    /// v0.19 연동 ①: `j`/`k`로 줄 커서를 옮기면 투구 선택이 따라온다. 헬퍼의
    /// at-bat은 아래 줄만 투구와 짝지어져 있으므로(위 줄은 안내 흉내) 커서
    /// 위치에 따라 선택이 붙었다 풀렸다 해야 한다 — v0.18에선 `j`/`k`가
    /// live_pitch_sel을 아예 건드리지 않았다(커서는 반전 하이라이트뿐).
    #[test]
    fn moving_the_relay_cursor_selects_and_clears_the_linked_pitch() {
        let mut app = live_app_with_at_bats(1);
        assert_eq!(app.live_pitch_sel, None);

        app.on_key(KeyCode::Char('k')); // 맨 아래(투구 줄)에서 시작
        assert_eq!(app.live_relay_cursor, Some(1));
        assert_eq!(
            app.live_pitch_sel,
            Some(0),
            "투구 줄에 커서가 서면 그 투구가 선택돼야 한다"
        );

        app.on_key(KeyCode::Char('k')); // 위 줄(투구가 아닌 줄)로
        assert_eq!(app.live_relay_cursor, Some(0));
        assert_eq!(
            app.live_pitch_sel, None,
            "첫 투구보다 위로 올라가면 선택이 풀린다"
        );
    }

    /// v0.19 연동 ②(반대 방향): `←`/`→`로 투구를 고르면 커서가 그 투구의
    /// 문자중계 줄로 간다 — v0.18에선 커서가 None인 채였다(문자중계는 꼬리만).
    #[test]
    fn selecting_a_pitch_moves_the_relay_cursor_onto_its_line() {
        let mut app = live_app_with_at_bats(1);
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, Some(0));
        assert_eq!(
            app.live_relay_cursor,
            Some(1),
            "그 투구를 서술하는 줄로 커서가 따라와야 한다"
        );
    }

    /// 우아한 저하: 줄과 투구가 짝지어지지 않은 상태(외래키 없는 응답·손 조립)
    /// 에서는 `←`/`→`가 v0.18처럼 투구만 고르고 커서는 세우지 않는다 —
    /// 옛 줄에 커서를 남겨 두면 커서와 투구가 서로 다른 사건을 가리킨다.
    #[test]
    fn pitch_navigation_leaves_the_cursor_alone_when_the_lines_are_not_linked() {
        let mut app = live_app_with_pitches(3); // relay_lines가 비어 있는 상태
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, Some(0));
        assert_eq!(app.live_relay_cursor, None);
    }

    /// Esc 3단 계단(v0.18): ①투구/문자중계 커서 해제 → ②라이브 복귀 →
    /// ③화면 이탈(목록으로). 기존 2단(①·③) 계단에 돌려보기 복귀(②)가 끼어든
    /// 것이므로, 셋 다 한 번씩 확인해 순서가 뒤섞이지 않는지 고정한다.
    #[test]
    fn esc_staircase_clears_local_selection_then_rewind_then_leaves_live() {
        let mut app = live_app_with_at_bats(3);
        app.on_key(KeyCode::Char('['));
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, Some(0));
        assert_eq!(app.live_atbat_sel, Some(101));

        app.on_key(KeyCode::Esc); // 1단계: 투구 선택만 해제
        assert_eq!(app.live_pitch_sel, None);
        assert_eq!(app.live_atbat_sel, Some(101), "아직 과거 타석을 보는 중");
        assert!(matches!(app.screen, Screen::Live { .. }));

        app.on_key(KeyCode::Esc); // 2단계: 라이브로 복귀
        assert_eq!(app.live_atbat_sel, None);
        assert!(matches!(app.screen, Screen::Live { .. }), "화면은 유지");

        app.on_key(KeyCode::Esc); // 3단계: 목록으로
        assert!(matches!(app.screen, Screen::List));
    }

    /// Esc 1단계는 문자중계 커서(pitch 선택 없이 j/k만 쓴 경우)도 함께 해제한다.
    #[test]
    fn esc_first_step_also_clears_a_bare_relay_cursor() {
        let mut app = live_app_with_at_bats(1);
        app.on_key(KeyCode::Char('k'));
        assert_eq!(app.live_relay_cursor, Some(1));
        app.on_key(KeyCode::Esc);
        assert_eq!(app.live_relay_cursor, None);
        assert!(
            matches!(app.screen, Screen::Live { .. }),
            "1단계는 화면을 유지한다"
        );
    }

    /// 라이브 추종의 핵심(v0.18): 과거 타석을 보며 투구까지 선택해 둔 상태에서
    /// 새 폴링이 도착해 라이브 타석이 바뀌어도(기존 "투구 수 감소→선택 리셋"
    /// 신호가 동시에 발생해도) 사용자가 보던 자리가 튀면 안 된다.
    #[test]
    fn rewinding_selection_survives_a_poll_that_advances_the_live_at_bat() {
        let mut app = live_app_with_at_bats(3); // batter0, batter1, batter2(live)
        app.on_key(KeyCode::Char('[')); // batter1로 이동
        assert_eq!(app.live_atbat_sel, Some(101));
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, Some(0));

        // 폴링으로 새 타석(batter3)이 시작됐다고 가정 — 새 타석은 아직
        // 무투구라 기존 "투구 수 감소" 리셋 신호도 함께 발생시킨다(가드가
        // 없었다면 이 신호가 live_pitch_sel을 지웠을 것).
        let fresh = {
            let Screen::Live {
                state: Some(mut s), ..
            } = live_app_with_at_bats(4).screen
            else {
                unreachable!()
            };
            s.current_pitches = vec![];
            s
        };
        app.apply(crate::poller::Update::Live("g".into(), fresh));

        assert_eq!(
            app.live_atbat_sel,
            Some(101),
            "돌려보기 선택이 새 폴링에 튀면 안 된다"
        );
        assert_eq!(
            app.live_pitch_sel,
            Some(0),
            "과거 타석 안의 투구 선택도 유지돼야 한다"
        );
        if let Screen::Live { state: Some(s), .. } = &app.screen {
            assert_eq!(s.at_bats.len(), 4, "새 타석은 실제로 반영된다");
            assert_eq!(
                s.active_at_bat(app.live_atbat_sel).unwrap().batter_name,
                "batter1",
                "여전히 같은 과거 타석을 보고 있어야 한다"
            );
        } else {
            panic!("expected Screen::Live");
        }
    }

    /// 읽던 타석의 **자리(인덱스)가 밀려도** 같은 타석을 계속 봐야 한다. 중계 응답은
    /// 현재 이닝만 담으므로 앞쪽 항목이 빠지고 뒤에 새 타석이 붙는 일이 실제로
    /// 일어난다 — 선택을 인덱스로 들고 있으면 그때 같은 자리가 다른 타자를 가리켜
    /// 사용자가 읽던 위치가 조용히 어긋난다(번호로 고정하는 이유).
    #[test]
    fn rewinding_selection_tracks_the_at_bat_even_when_its_index_shifts() {
        let mut app = live_app_with_at_bats(3); // seq 100,101,102
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.live_atbat_sel, Some(101));

        // 같은 이닝이 이어지는 폴링: 가장 오래된 100이 응답에서 빠지고 103이 붙는다.
        // 보고 있던 101은 인덱스 1 → 0으로 밀린다.
        let shifted = {
            let Screen::Live {
                state: Some(mut s), ..
            } = live_app_with_at_bats(4).screen
            else {
                unreachable!()
            };
            s.at_bats.remove(0);
            s
        };
        app.apply(crate::poller::Update::Live("g".into(), shifted));

        assert_eq!(app.live_atbat_sel, Some(101), "선택은 번호 그대로");
        if let Screen::Live { state: Some(s), .. } = &app.screen {
            assert_eq!(
                s.at_bats.iter().position(|ab| ab.seq == 101),
                Some(0),
                "자리는 실제로 밀렸다(테스트 전제 확인)"
            );
            assert_eq!(
                s.active_at_bat(app.live_atbat_sel).unwrap().batter_name,
                "batter1",
                "자리가 밀려도 읽던 타석 그대로여야 한다"
            );
        } else {
            panic!("expected Screen::Live");
        }
    }

    /// 이닝이 넘어가면 응답이 통째로 갈려 보던 타석이 사라진다. 그때는 다른 타석을
    /// 그 자리인 척 보여주지 말고 라이브로 되돌린다 — 조용히 어긋나느니 눈에 보이게
    /// 최신으로 돌아오는 편이 낫다.
    #[test]
    fn rewinding_returns_to_live_when_the_inning_rolls_over() {
        let mut app = live_app_with_at_bats(3);
        app.on_key(KeyCode::Char('['));
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_atbat_sel, Some(101));
        assert_eq!(app.live_pitch_sel, Some(0));

        // 다음 이닝 응답: 번호대가 통째로 다르다(보던 101이 없다).
        let next_inning = {
            let Screen::Live {
                state: Some(mut s), ..
            } = live_app_with_at_bats(2).screen
            else {
                unreachable!()
            };
            for (i, ab) in s.at_bats.iter_mut().enumerate() {
                ab.seq = 200 + i as i64;
            }
            s
        };
        app.apply(crate::poller::Update::Live("g".into(), next_inning));

        assert_eq!(
            app.live_atbat_sel, None,
            "사라진 타석에 남아 있지 말고 라이브로 복귀"
        );
        assert_eq!(app.live_pitch_sel, None, "그 타석의 투구 선택도 함께 해제");
        assert_eq!(app.live_relay_cursor, None, "문자중계 커서도 함께 해제");
    }

    /// live_atbat_sel이 None(라이브 추종)일 때는 기존 규칙이 그대로 적용돼야
    /// 한다 — new_at_bat_with_fewer_pitches_resets_selection과 짝을 이루는
    /// 무회귀 확인(가드 조건 자체를 고정).
    #[test]
    fn live_follow_mode_still_resets_pitch_selection_on_a_new_at_bat() {
        let mut app = live_app_with_at_bats(3);
        app.on_key(KeyCode::Right); // 라이브(batter2)에서 투구 선택
        assert_eq!(app.live_pitch_sel, Some(0));
        assert_eq!(app.live_atbat_sel, None);

        let fresh = {
            let Screen::Live {
                state: Some(mut s), ..
            } = live_app_with_at_bats(4).screen
            else {
                unreachable!()
            };
            s.current_pitches = vec![];
            s
        };
        app.apply(crate::poller::Update::Live("g".into(), fresh));
        assert_eq!(
            app.live_pitch_sel, None,
            "라이브 추종 중엔 새 타석에서 선택이 리셋돼야 한다(기존 동작)"
        );
    }

    /// I-2: 라이브 추종 중 새 타석이 시작되면 문자중계 커서도 함께 리셋돼야
    /// 한다 — 안 그러면 읽던 자리가 다른 타자의 줄로 조용히 옮겨간다(리뷰
    /// 실측 재현). `live_app_with_at_bats`는 at-bat마다 투구를 정확히
    /// 1개씩만 담으므로, n=3→4로 갈 때 seq는 102→103으로 바뀌지만
    /// current_pitches 길이는 1→1로 그대로다 — "투구 수 감소"라는 기존
    /// 신호로는 이 새 타석을 못 알아채는 경우를 일부러 재현해, seq 비교가
    /// 실제로 필요하다는 것을 증명한다.
    #[test]
    fn live_follow_mode_resets_relay_cursor_on_a_new_at_bat_even_when_pitch_count_is_unchanged() {
        let mut app = live_app_with_at_bats(3); // batter0, batter1, batter2(live, seq 102)
        app.on_key(KeyCode::Char('k')); // 라이브 추종 중 문자중계 커서 시작
        assert_eq!(app.live_relay_cursor, Some(1));

        let fresh = {
            let Screen::Live { state: Some(s), .. } = live_app_with_at_bats(4).screen
            // batter3(live, seq 103), 투구는 여전히 1개
            else {
                unreachable!()
            };
            s
        };
        app.apply(crate::poller::Update::Live("g".into(), fresh));

        assert_eq!(
            app.live_relay_cursor, None,
            "새 타석이 시작되면 이전 타석의 문자중계 커서 자리는 무의미해진다"
        );
        assert_eq!(
            app.live_pitch_sel, None,
            "같은 신호로 투구 선택도 함께 리셋돼야 한다"
        );
    }

    #[test]
    fn arrows_are_noop_on_list_screen() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::Right);
        assert_eq!(app.live_pitch_sel, None);
    }

    #[test]
    fn new_at_bat_with_fewer_pitches_resets_selection() {
        let mut app = live_app_with_pitches(3);
        app.on_key(KeyCode::Right);
        app.on_key(KeyCode::Right); // sel = 1
                                    // 같은 게임 id로 투구 1개짜리(새 타석) 상태 도착 → 선택 리셋
        let fresh = {
            let Screen::Live { state: Some(s), .. } = &live_app_with_pitches(1).screen else {
                unreachable!()
            };
            s.clone()
        };
        app.apply(crate::poller::Update::Live("g".into(), fresh));
        assert_eq!(app.live_pitch_sel, None);
    }

    /// Live에서 Tab: 헤더만 바뀌고 화면이 안 바뀌던 혼란(v0.2 최종 리뷰 기록) 해소 —
    /// 목록으로 나가면서 탭 전환("순위 보고 싶다"를 한 키로).
    #[test]
    fn tab_in_live_returns_to_list_with_the_switched_tab() {
        let mut app = live_app_with_pitches(2);
        app.on_key(KeyCode::Right); // 선택도 있는 상태에서
        assert!(matches!(app.screen, Screen::Live { .. }));
        app.on_key(KeyCode::Tab);
        assert!(
            matches!(app.screen, Screen::List),
            "Tab must leave the live view"
        );
        assert_eq!(app.tab, Tab::Standings);
        assert_eq!(
            app.live_pitch_sel, None,
            "selection must not survive the exit"
        );
        assert_eq!(app.selected, 0);
    }

    /// I-3: `enter_live`(공유 헬퍼)는 이전 게임에서 짚어보던 투구/타석/문자중계
    /// 선택을 셋 다 반드시 리셋한다 — Enter 키 진입(on_key)과 `--team` 자동
    /// 진입(main.rs)이 각자 화면 전환을 들고 있다가 자동 진입 쪽만 리셋을
    /// 빠뜨려, 다른 경기에서 되감기 중이던 선택이 새로 자동 진입한 경기에
    /// 그대로 남는 결함이 있었다(리뷰 I-3). seq는 경기별 번호라 다른 경기와
    /// 대역이 겹칠 수 있어, 리셋하지 않으면 폴링 가드(`!l.has_at_bat(seq)`)가
    /// 걸러주지 못한다.
    #[test]
    fn enter_live_resets_all_three_selections_even_when_a_previous_game_left_them_set() {
        let mut app = live_app_with_at_bats(3);
        // 과거 타석으로 이동한 뒤, 문자중계 커서를 세운다. v0.19 연동 덕에 그
        // 줄(맨 아래 = 투구 줄)의 투구까지 함께 선택되므로, 세 선택이 동시에
        // 살아 있는 상태가 이 두 키로 만들어진다.
        app.on_key(KeyCode::Char('['));
        app.on_key(KeyCode::Char('k'));
        assert!(app.live_atbat_sel.is_some());
        assert!(app.live_pitch_sel.is_some());
        assert!(app.live_relay_cursor.is_some());

        app.enter_live(game("other-game"));

        assert!(matches!(app.screen, Screen::Live { .. }));
        assert_eq!(app.live_atbat_sel, None);
        assert_eq!(app.live_pitch_sel, None);
        assert_eq!(app.live_relay_cursor, None);
    }

    /// News는 보조 기능 — 스피너(fetching) 상태에 관여하면 안 된다(v0.2 최종
    /// 리뷰 권고). 진행 중이던 fetch 표시를 News 도착이 지우지 않는다.
    #[test]
    fn news_update_does_not_touch_the_spinner_flag() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Fetching);
        assert!(app.fetching);
        app.apply(crate::poller::Update::News(vec![]));
        assert!(
            app.fetching,
            "auxiliary news must not clear the in-flight spinner"
        );
    }

    #[test]
    fn f2_opens_options_and_esc_closes_without_change() {
        let mut app = App::new(Default::default());
        app.date = "2026-07-23".into();
        assert!(app.options.is_none());
        app.on_key(KeyCode::F(2));
        assert!(app.options.is_some());
        app.on_key(KeyCode::Esc);
        assert!(app.options.is_none());
        assert_eq!(app.date, "2026-07-23"); // 무변경
    }

    /// 오버레이가 열려 있으면 하위 화면 키(Tab/j/k 등)를 소비한다.
    #[test]
    fn options_overlay_consumes_navigation_keys() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::F(2));
        let tab_before = app.tab;
        app.on_key(KeyCode::Tab);
        assert_eq!(app.tab, tab_before, "Tab must be consumed by the overlay");
    }

    /// Date 적용: date 갱신 + games_loaded 리셋 + Live였다면 List 복귀.
    #[test]
    fn options_date_apply_resets_list_and_leaves_live() {
        let mut app = live_app_with_pitches(2); // 기존 헬퍼(Task 6에서 도입) 재사용
        app.now_secs = 1_800_000_000; // 임의 고정 시각
        app.games_loaded = true;
        app.on_key(KeyCode::F(2)); // Date pane이 기본
        app.on_key(KeyCode::Down); // Today → Yesterday
        app.on_key(KeyCode::Enter);
        assert!(matches!(app.screen, Screen::List));
        assert!(!app.games_loaded);
        assert_eq!(app.selected, 0);
        assert_eq!(
            app.date,
            crate::dateutil::format_civil(crate::dateutil::kst_days(1_800_000_000) - 1)
        );
    }

    /// Tips는 News처럼 보조 — stale/last_error/fetching에 관여하지 않는다.
    #[test]
    fn tips_update_sets_override_without_touching_lifecycles() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Error("boom".into()));
        app.apply(crate::poller::Update::Fetching);
        app.apply(crate::poller::Update::Tips(vec!["원격".into(); 12]));
        assert_eq!(app.tips_override.as_ref().map(|v| v.len()), Some(12));
        assert!(app.stale);
        assert!(app.fetching);
        assert_eq!(app.last_error.as_deref(), Some("boom"));
    }

    /// games 탭에서 o: 선택 경기의 원정/홈 × 공홈/굿즈몰 4항목 픽커가 열린다.
    #[test]
    fn o_on_games_opens_four_link_items_for_the_selected_game() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("g")])); // 기존 헬퍼: KT@LG
        app.on_key(KeyCode::Char('o'));
        let items = &app.link_picker.as_ref().expect("picker must open").items;
        assert_eq!(items.len(), 4);
        let labels: String = items
            .iter()
            .map(|(l, _)| l.as_str())
            .collect::<Vec<_>>()
            .join("|");
        assert!(labels.contains("KT") && labels.contains("LG"));
    }

    /// standings 탭에서 o: 선택 팀의 2항목(공홈/굿즈몰).
    #[test]
    fn o_on_standings_opens_two_link_items_for_the_selected_team() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(crate::poller::Update::Standings(vec![
            crate::model::Standing {
                rank: 1,
                team: crate::model::Team {
                    code: "SS".into(),
                    name: "삼성".into(),
                },
                games: 1,
                wins: 1,
                losses: 0,
                draws: 0,
                win_rate: 1.0,
                game_behind: 0.0,
                last_five: String::new(),
                streak: String::new(),
                stats: Default::default(),
            },
        ]));
        app.on_key(KeyCode::Char('o'));
        let items = &app.link_picker.as_ref().expect("picker must open").items;
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|(_, url)| url.starts_with("https://")));
    }

    /// 링크픽커가 열려 있을 때 F2는 소비만 된다 — 오버레이 이중 오픈 금지
    /// (최종 리뷰 I-1 회귀 방지).
    #[test]
    fn f2_while_link_picker_open_does_not_stack_overlays() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("g")]));
        app.on_key(KeyCode::Char('o'));
        assert!(app.link_picker.is_some());
        app.on_key(KeyCode::F(2));
        assert!(
            app.options.is_none(),
            "F2 must not open options over the link picker"
        );
        assert!(app.link_picker.is_some(), "link picker must stay open");
    }

    #[test]
    fn esc_closes_link_picker_without_opening() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::Games(vec![game("g")]));
        app.on_key(KeyCode::Char('o'));
        app.on_key(KeyCode::Esc);
        assert!(app.link_picker.is_none());
        assert!(
            matches!(app.screen, Screen::List),
            "Esc must close picker, not navigate"
        );
    }

    /// n 키: 뉴스가 없으면 아무 일도 안 일어난다(패닉·상태 변화 없음).
    #[test]
    fn n_with_no_news_is_a_noop() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::Char('n'));
        assert!(matches!(app.screen, Screen::List));
        assert!(app.article_view.is_none());
    }

    fn sample_news_item() -> crate::model::NewsItem {
        crate::model::NewsItem {
            title: "제목텍스트".into(),
            source: "홍길동일보".into(),
            url: "https://m.example.com/x".into(),
            summary: "본문 내용".into(),
            published: String::new(),
        }
    }

    /// n: 뉴스가 있으면 목록을 열고(v0.7), 커서 항목에서 Enter를 누르면 그
    /// 항목을 그대로 담아 발췌 오버레이를 연다(비동기 fetch 없음).
    #[test]
    fn n_opens_list_and_enter_opens_article_view_with_cursor_item() {
        let mut app = App::new(Default::default());
        app.now_secs = 0;
        app.apply(crate::poller::Update::News(vec![sample_news_item()]));
        app.on_key(KeyCode::Char('n'));
        assert!(app.news_list.is_some(), "n must open the list");
        assert!(app.article_view.is_none());
        app.on_key(KeyCode::Enter);
        let v = app
            .article_view
            .as_ref()
            .expect("Enter must open the overlay");
        assert_eq!(v.item, sample_news_item());
        assert_eq!(v.scroll, 0);
    }

    /// n으로 오버레이를 다시 누르면 닫힌다(토글); j/k는 scroll을 움직인다.
    #[test]
    fn article_overlay_consumes_keys_scroll_and_toggle_close() {
        let mut app = App::new(Default::default());
        app.article_view = Some(ArticleView {
            item: sample_news_item(),
            scroll: 0,
        });
        app.on_key(KeyCode::Char('j'));
        assert_eq!(app.article_view.as_ref().unwrap().scroll, 1);
        app.on_key(KeyCode::Char('k'));
        assert_eq!(app.article_view.as_ref().unwrap().scroll, 0);
        app.on_key(KeyCode::Char('k')); // 경계: 0 밑으로 안 내려감
        assert_eq!(app.article_view.as_ref().unwrap().scroll, 0);
        // 오버레이가 열린 동안 Tab 등은 소비된다(하위 화면에 안 샌다).
        let tab_before = app.tab;
        app.on_key(KeyCode::Tab);
        assert_eq!(app.tab, tab_before, "overlay must consume Tab");
        app.on_key(KeyCode::Char('n')); // n 토글로 닫기
        assert!(app.article_view.is_none());
    }

    fn news_item(title: &str, url: &str) -> crate::model::NewsItem {
        crate::model::NewsItem {
            title: title.into(),
            source: "출처".into(),
            url: url.into(),
            summary: "발췌 내용".into(),
            published: String::new(),
        }
    }

    /// n은 목록을 연다. Enter로 선택 항목의 발췌 오버레이로 내려가고,
    /// Esc는 한 단계씩 올라온다(기사→목록→닫힘).
    #[test]
    fn n_opens_list_then_enter_opens_article_then_esc_climbs_back() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![
            news_item("첫 기사", "https://x.kr/1"),
            news_item("둘째 기사", "https://x.kr/2"),
        ]));
        app.on_key(KeyCode::Char('n'));
        assert!(app.news_list.is_some(), "n은 목록을 연다");
        assert!(app.article_view.is_none());

        app.on_key(KeyCode::Char('j'));
        assert_eq!(app.news_list.as_ref().unwrap().cursor, 1);

        app.on_key(KeyCode::Enter);
        let v = app.article_view.as_ref().expect("Enter는 기사를 연다");
        assert_eq!(v.item.title, "둘째 기사", "커서 항목이 열려야 한다");
        assert!(app.news_list.is_some(), "기사 아래에 목록이 남아 있다");

        app.on_key(KeyCode::Esc);
        assert!(app.article_view.is_none(), "Esc는 기사만 닫는다");
        assert!(app.news_list.is_some(), "목록은 유지된다");

        app.on_key(KeyCode::Esc);
        assert!(app.news_list.is_none(), "한 번 더 Esc면 목록도 닫힌다");
    }

    /// 목록 커서는 경계를 넘지 않고, 열려 있는 동안 하위 화면 키를 소비한다.
    #[test]
    fn news_list_cursor_stays_in_bounds_and_consumes_keys() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![news_item("하나", "u")]));
        app.on_key(KeyCode::Char('n'));
        app.on_key(KeyCode::Char('j'));
        assert_eq!(
            app.news_list.as_ref().unwrap().cursor,
            0,
            "1건이면 안 움직인다"
        );
        app.on_key(KeyCode::Char('k'));
        assert_eq!(app.news_list.as_ref().unwrap().cursor, 0);
        let tab_before = app.tab;
        app.on_key(KeyCode::Tab);
        assert_eq!(app.tab, tab_before, "목록이 Tab을 소비한다");
    }

    /// 뉴스가 없으면 n은 무동작(패닉 없음).
    #[test]
    fn n_with_no_news_opens_nothing() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::Char('n'));
        assert!(app.news_list.is_none());
        assert!(app.article_view.is_none());
    }

    /// 리뷰 지적(Important) 재현 시나리오: 목록이 열린 채 뉴스가 3건→1건으로
    /// 줄어들면 커서가 새 길이로 clamp되고, Enter가 실제로 항목을 연다.
    #[test]
    fn news_refresh_clamps_open_list_cursor_and_enter_still_opens_item() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![
            news_item("첫 기사", "https://x.kr/1"),
            news_item("둘째 기사", "https://x.kr/2"),
            news_item("셋째 기사", "https://x.kr/3"),
        ]));
        app.on_key(KeyCode::Char('n'));
        app.on_key(KeyCode::Char('j'));
        app.on_key(KeyCode::Char('j'));
        assert_eq!(app.news_list.as_ref().unwrap().cursor, 2);

        app.apply(crate::poller::Update::News(vec![news_item(
            "새로 온 기사",
            "https://x.kr/new",
        )]));
        assert!(app.news_list.is_some(), "목록은 열린 채 유지된다");
        assert_eq!(
            app.news_list.as_ref().unwrap().cursor,
            0,
            "커서가 새 길이(1건)로 clamp돼야 한다"
        );

        app.on_key(KeyCode::Enter);
        let v = app
            .article_view
            .as_ref()
            .expect("clamp된 커서로 Enter가 실제 항목을 열어야 한다");
        assert_eq!(v.item.title, "새로 온 기사");
    }

    /// 뉴스가 0건으로 갱신되면(전부 사라짐) 목록이 닫히고 패닉하지 않는다.
    #[test]
    fn news_refresh_to_empty_does_not_panic_and_closes_list() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![
            news_item("첫 기사", "https://x.kr/1"),
            news_item("둘째 기사", "https://x.kr/2"),
        ]));
        app.on_key(KeyCode::Char('n'));
        app.on_key(KeyCode::Char('j'));
        assert_eq!(app.news_list.as_ref().unwrap().cursor, 1);

        app.apply(crate::poller::Update::News(vec![]));
        assert!(app.news_list.is_none(), "빈 오버레이를 남기지 않는다");

        // Enter는 조용히 무동작해야 한다(패닉 없음, 기사도 안 열림).
        app.on_key(KeyCode::Enter);
        assert!(app.article_view.is_none());
    }

    /// 목록이 열린 채 뉴스가 0건으로 갱신되면 오버레이를 닫는다(빈 목록 방치 금지).
    #[test]
    fn news_refresh_to_empty_closes_open_list() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![news_item("하나", "u")]));
        app.on_key(KeyCode::Char('n'));
        assert!(app.news_list.is_some());
        app.apply(crate::poller::Update::News(vec![]));
        assert!(app.news_list.is_none(), "empty refresh must close the list");
    }

    /// F9는 설정 화면을 연다. Esc로 닫힌다. j/k는 커서를 경계 내에서 움직인다.
    #[test]
    fn f9_opens_settings_and_esc_closes() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::F(9));
        assert!(app.settings.is_some(), "F9 opens settings");
        app.on_key(KeyCode::Char('j'));
        let rows = app.settings_rows().len();
        assert!(app.settings.as_ref().unwrap().cursor < rows);
        app.on_key(KeyCode::Esc);
        assert!(app.settings.is_none(), "Esc closes settings");
    }

    /// 설정 화면이 열려 있으면 하위 화면 키(Tab)를 소비한다.
    #[test]
    fn settings_overlay_consumes_keys() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::F(9));
        let tab_before = app.tab;
        app.on_key(KeyCode::Tab);
        assert_eq!(app.tab, tab_before, "settings consumes Tab");
    }

    /// 설정 화면에서 폴링 항목을 →로 바꾸면 poll_choice가 다음 단계로 간다.
    #[test]
    fn settings_right_changes_poll_choice() {
        let mut app = App::new(Default::default());
        app.poll_choice = 5;
        app.on_key(KeyCode::F(9));
        // 커서를 폴링 항목(row 1)으로.
        app.on_key(KeyCode::Char('j'));
        app.on_key(KeyCode::Right);
        assert_ne!(app.poll_choice, 5, "→ changes poll interval");
    }

    /// 설정 화면에서 팀 항목을 바꾸면 fav_code가 갱신된다.
    #[test]
    fn settings_changes_team() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::F(9)); // 커서 0 = 팀
        app.on_key(KeyCode::Right);
        assert!(app.fav_code.is_some(), "→ selects a team");
    }

    /// F9 Team 행 값은 KBO 코드가 아니라 team_items의 팀명 라벨을 보여준다
    /// (순위표=`키움`, F2 픽커=`WO  키움 히어로즈`와 표기를 맞춘다).
    #[test]
    fn settings_rows_team_value_shows_team_name_not_just_code() {
        let mut app = App::new(Default::default());
        app.fav_code = Some("WO".to_string());
        let rows = app.settings_rows();
        assert!(matches!(rows[0].0, SettingKind::Team));
        assert_ne!(rows[0].2, "WO", "code alone must not be the display value");
        assert!(
            rows[0].2.contains("키움"),
            "team value must include the team name, got {:?}",
            rows[0].2
        );
    }

    /// team_items에 없는(구버전 config 등) 모르는 코드는 패닉 없이 코드
    /// 문자열 그대로 폴백한다.
    #[test]
    fn settings_rows_team_value_falls_back_to_unknown_code() {
        let mut app = App::new(Default::default());
        app.fav_code = Some("XX".to_string());
        let rows = app.settings_rows();
        assert_eq!(rows[0].2, "XX");
    }

    /// fav_code가 None이면 기존대로 team_none 라벨을 보여준다.
    #[test]
    fn settings_rows_team_value_none_shows_team_none_label() {
        let app = App::new(Default::default());
        assert!(app.fav_code.is_none());
        let rows = app.settings_rows();
        assert_eq!(rows[0].2, app.labels().team_none);
    }

    /// 설정 항목에 테마 프리셋·액센트·언어 행이 추가돼 있다(팀·폴링 뒤 순서).
    #[test]
    fn settings_rows_include_theme_preset_and_accent() {
        let app = App::new(Default::default());
        let rows = app.settings_rows();
        assert_eq!(rows.len(), 6);
        assert!(matches!(rows[2].0, SettingKind::ThemePreset));
        assert!(matches!(rows[3].0, SettingKind::ThemeAccent));
        assert!(matches!(rows[4].0, SettingKind::Lang));
        assert!(matches!(rows[5].0, SettingKind::Mouse));
    }

    /// 설정 화면에서 테마 프리셋 항목을 →로 바꾸면 default→high-contrast→mono
    /// 순으로 순환한다.
    #[test]
    fn settings_right_cycles_theme_preset() {
        let mut app = App::new(Default::default());
        assert_eq!(app.theme_preset, "default");
        app.on_key(KeyCode::F(9));
        app.on_key(KeyCode::Down); // Poll
        app.on_key(KeyCode::Down); // ThemePreset
        app.on_key(KeyCode::Right);
        assert_eq!(app.theme_preset, "high-contrast");
        app.on_key(KeyCode::Right);
        assert_eq!(app.theme_preset, "mono");
        app.on_key(KeyCode::Right); // 순환: mono 다음은 다시 default
        assert_eq!(app.theme_preset, "default");
    }

    /// 테마 액센트 항목도 순환하고, ←는 역방향으로 순환한다.
    #[test]
    fn settings_changes_theme_accent_both_directions() {
        let mut app = App::new(Default::default());
        assert_eq!(app.theme_accent, "team");
        app.on_key(KeyCode::F(9));
        app.on_key(KeyCode::Down); // Poll
        app.on_key(KeyCode::Down); // ThemePreset
        app.on_key(KeyCode::Down); // ThemeAccent
        app.on_key(KeyCode::Right);
        assert_eq!(app.theme_accent, "cyan");
        app.on_key(KeyCode::Left);
        assert_eq!(app.theme_accent, "team");
    }

    /// 설정 화면에서 언어를 바꾸면 App.lang과 라벨이 갈린다.
    #[test]
    fn settings_changes_language() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        app.on_key(KeyCode::F(9));
        // 언어 항목으로 커서 이동(마지막 행 근처) 후 → 로 순환.
        let lang_row = app
            .settings_rows()
            .iter()
            .position(|(_, label, _)| *label == app.labels().set_lang)
            .unwrap();
        for _ in 0..lang_row {
            app.on_key(KeyCode::Char('j'));
        }
        let before = app.lang;
        app.on_key(KeyCode::Right);
        assert_ne!(app.lang, before, "language cycles");
    }
    // ---- v0.20: 과거 이닝 돌려보기 -------------------------------------------------

    /// 모든 타석이 같은 이닝(`T{inning}`)에 속한 라이브 화면. 실제 응답이 그렇다 —
    /// `/relay`는 마지막 이닝 하나만 담는다.
    fn live_app_in_inning(inning: u8, n: usize) -> App {
        let mut app = live_app_with_at_bats(n);
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            for ab in &mut s.at_bats {
                ab.inning_label = format!("T{inning}");
            }
            s.inning_label = format!("T{inning}");
        }
        app
    }

    fn at_bat(seq: i64, inning: u8, batter: &str) -> crate::model::AtBat {
        crate::model::AtBat {
            seq,
            batter_name: batter.into(),
            inning_label: format!("T{inning}"),
            relay_lines: vec![crate::model::RelayLine::plain(format!("{batter}-line"))],
            pitches: vec![crate::model::Pitch {
                order: 1,
                ..Default::default()
            }],
        }
    }

    /// 되감기가 이닝 경계에 닿으면 그 앞 이닝을 예약한다. 이게 없으면 `[`는
    /// 현재 이닝 첫 타석에서 멈추고, 늦게 접속한 사람은 앞 이닝을 볼 방법이 없다.
    #[test]
    fn rewinding_past_the_first_at_bat_asks_for_the_previous_inning() {
        let mut app = live_app_in_inning(5, 3);
        assert_eq!(app.fetching_inning, None);

        app.on_key(KeyCode::Char('[')); // 최신 → 가운데
        app.on_key(KeyCode::Char('[')); // → 맨 앞
        assert_eq!(app.fetching_inning, None, "아직 경계에 닿지 않았다");

        app.on_key(KeyCode::Char('[')); // 경계에서 한 번 더
        assert_eq!(app.fetching_inning, Some(4), "앞 이닝(4회)을 요청해야 한다");
    }

    /// 1회에서는 더 갈 데가 없다 — 0회를 요청하면 안 된다.
    #[test]
    fn rewinding_past_the_first_inning_requests_nothing() {
        let mut app = live_app_in_inning(1, 2);
        for _ in 0..5 {
            app.on_key(KeyCode::Char('['));
        }
        assert_eq!(app.fetching_inning, None);
    }

    /// 요청이 떠 있는 동안 계속 눌러도 요청은 하나다(되감기는 한 이닝씩 간다).
    #[test]
    fn repeated_presses_while_a_request_is_in_flight_do_not_change_the_target() {
        let mut app = live_app_in_inning(5, 1);
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.fetching_inning, Some(4));
        app.on_key(KeyCode::Char('['));
        app.on_key(KeyCode::Char('['));
        assert_eq!(
            app.fetching_inning,
            Some(4),
            "in-flight 요청이 바뀌면 안 된다"
        );
    }

    /// 도착한 과거 이닝은 캐시에 들어가고 화면 목록 **앞에** seq 순으로 붙는다.
    #[test]
    fn an_arriving_past_inning_is_prepended_in_seq_order() {
        let mut app = live_app_in_inning(5, 3); // seq 100,101,102
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![at_bat(98, 4, "earlier-a"), at_bat(99, 4, "earlier-b")],
        });

        let Screen::Live { state: Some(s), .. } = &app.screen else {
            panic!("expected live screen");
        };
        let seqs: Vec<i64> = s.at_bats.iter().map(|ab| ab.seq).collect();
        assert_eq!(seqs, vec![98, 99, 100, 101, 102]);
        assert_eq!(app.fetching_inning, None, "도착했으면 로딩 표시를 푼다");
        assert!(app.cached_innings_of("g").contains_key(&4));
    }

    /// 겹치는 seq는 라이브 쪽이 이긴다 — 기본 `/relay`와 `?inning=<현재 이닝>`은
    /// 같은 데이터를 주는데(실측), 캐시본은 그 시점에 멈춰 있고 라이브는 폴링으로
    /// 계속 갱신된다. 캐시가 이기면 화면이 과거로 되돌아간다.
    #[test]
    fn on_a_seq_collision_the_live_copy_wins_over_the_cached_one() {
        let mut app = live_app_in_inning(5, 1); // seq 100 하나, batter0
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![at_bat(100, 4, "stale-copy"), at_bat(99, 4, "earlier")],
        });

        let Screen::Live { state: Some(s), .. } = &app.screen else {
            panic!("expected live screen");
        };
        assert_eq!(
            s.at_bats.iter().map(|ab| ab.seq).collect::<Vec<_>>(),
            vec![99, 100]
        );
        assert_eq!(
            s.at_bats.last().unwrap().batter_name,
            "batter0",
            "겹친 seq는 라이브 쪽이 남아야 한다"
        );
    }

    /// 캐시는 이어지는 라이브 폴링에도 살아남아야 한다. 폴러가 주는 LiveState는
    /// 늘 현재 이닝만 담으므로, 병합하지 않으면 다음 폴링 한 번에 되감아 둔
    /// 과거 이닝이 화면에서 사라진다.
    #[test]
    fn a_later_live_poll_keeps_the_cached_past_innings() {
        let mut app = live_app_in_inning(5, 2);
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![at_bat(99, 4, "earlier")],
        });

        // 폴러의 다음 응답: 현재 이닝만 담긴 새 상태.
        let fresh = match &app.screen {
            Screen::Live { state: Some(s), .. } => {
                let mut fresh = s.clone();
                fresh.at_bats.retain(|ab| ab.seq >= 100);
                fresh
            }
            _ => panic!("expected live screen"),
        };
        app.apply(Update::Live("g".into(), fresh));

        let Screen::Live { state: Some(s), .. } = &app.screen else {
            panic!("expected live screen");
        };
        assert!(
            s.at_bats.iter().any(|ab| ab.seq == 99),
            "라이브 폴링이 과거 이닝을 지웠다: {:?}",
            s.at_bats.iter().map(|ab| ab.seq).collect::<Vec<_>>()
        );
    }

    /// 타석이 0건인 이닝도 "받아 봤다"로 캐시한다 — 그러지 않으면 같은 이닝을
    /// 무한히 다시 묻는다. 다음 요청은 그 앞 이닝으로 내려간다.
    #[test]
    fn an_empty_inning_is_remembered_and_the_next_request_skips_it() {
        let mut app = live_app_in_inning(5, 1);
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.fetching_inning, Some(4));
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![],
        });
        assert_eq!(app.fetching_inning, None);

        app.on_key(KeyCode::Char('['));
        assert_eq!(
            app.fetching_inning,
            Some(3),
            "빈 이닝을 건너뛰고 그 앞을 묻는다"
        );
    }

    /// 화면을 옮긴 뒤 도착한 이전 경기의 응답이 지금 보는 경기에 섞이면 안 된다.
    #[test]
    fn a_past_inning_for_another_game_is_ignored() {
        let mut app = live_app_in_inning(5, 2);
        app.apply(Update::Inning {
            game_id: "other-game".into(),
            inning: 4,
            at_bats: vec![at_bat(99, 4, "stranger")],
        });

        let Screen::Live { state: Some(s), .. } = &app.screen else {
            panic!("expected live screen");
        };
        assert!(s.at_bats.iter().all(|ab| ab.seq >= 100));
        assert!(
            app.past_innings.is_empty(),
            "다른 경기의 응답은 캐시에도 안 들어간다"
        );
    }

    /// 라이브를 나갔다 **같은 경기로** 돌아오면 되감아 둔 이닝이 그대로 있어야 한다
    /// (v0.21). v0.20까지는 enter_live가 캐시를 통째로 비워 매번 다시 받았다.
    #[test]
    fn returning_to_the_same_game_keeps_its_cached_innings() {
        let mut app = live_app_in_inning(5, 2);
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![at_bat(99, 4, "earlier")],
        });

        app.enter_live(game("g")); // 목록으로 나갔다 같은 경기로 재진입
        assert!(
            app.cached_innings_of("g").contains_key(&4),
            "같은 경기로 돌아왔는데 캐시가 비었다"
        );
        assert_eq!(
            app.fetching_inning, None,
            "요청 중 표시는 화면 상태라 리셋한다"
        );
    }

    /// **재진입 시나리오 전체**: 되감아 8회를 받아 둔 뒤 목록으로 나갔다가 같은
    /// 경기로 다시 들어오면, 첫 폴링(Update::Live)에서 8회가 다시 붙어 있어야 한다.
    ///
    /// 실행 확인이 여기서 애매했다 — PTY 캡처가 프레임 diff라 중간 화면을 놓쳐
    /// "8회로 넘어갔는지"를 눈으로 확정하지 못했다. 그래서 그 지점을 값으로 고정한다.
    /// 이 테스트가 없으면 `enter_live` → `Update::Live` 경로에서 병합이 빠져도
    /// 나머지 테스트는 전부 통과한다.
    #[test]
    fn reentering_the_same_game_restores_the_cached_innings_on_the_next_poll() {
        let mut app = live_app_in_inning(9, 2); // 9회 타석 seq 100·101
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 8,
            at_bats: vec![at_bat(98, 8, "eighth-a"), at_bat(99, 8, "eighth-b")],
        });

        // 목록으로 나갔다가(Screen::List) 같은 경기로 재진입 — state는 None으로 리셋된다.
        app.screen = Screen::List;
        app.enter_live(game("g"));
        assert!(
            matches!(app.screen, Screen::Live { state: None, .. }),
            "재진입 직후에는 아직 상태가 없다"
        );

        // 폴러의 첫 응답: 늘 그렇듯 현재 이닝(9회)만 담겨 온다.
        let mut fresh = match &live_app_in_inning(9, 2).screen {
            Screen::Live { state: Some(s), .. } => s.clone(),
            _ => panic!("expected live screen"),
        };
        fresh.at_bats.retain(|ab| ab.seq >= 100);
        app.apply(Update::Live("g".into(), fresh));

        let Screen::Live { state: Some(s), .. } = &app.screen else {
            panic!("expected live screen");
        };
        assert_eq!(
            s.at_bats.iter().map(|ab| ab.seq).collect::<Vec<_>>(),
            vec![98, 99, 100, 101],
            "재진입 후 첫 폴링에서 받아 둔 8회가 붙지 않았다"
        );

        // 그러면 맨 앞 타석은 8회이므로, 거기서 `[`를 더 눌러도 8회를 다시 묻지 않는다.
        app.live_atbat_sel = Some(98);
        app.on_key(KeyCode::Char('['));
        assert_eq!(
            app.fetching_inning,
            Some(7),
            "8회가 캐시에 있는데 또 요청했다"
        );
    }

    /// 다른 경기의 캐시는 서로 섞이지 않는다. v0.20은 `clear()`로 막았고
    /// v0.21은 키가 경기 id라 **구조적으로** 섞일 수 없다.
    #[test]
    fn each_game_keeps_its_own_inning_cache() {
        let mut app = live_app_in_inning(5, 2);
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![at_bat(99, 4, "earlier")],
        });

        app.enter_live(game("other-game"));
        assert!(
            app.cached_innings_of("other-game").is_empty(),
            "다른 경기가 남의 캐시를 물려받았다"
        );
        assert!(
            app.cached_innings_of("g").contains_key(&4),
            "원래 경기의 캐시까지 사라졌다"
        );
    }

    /// 다른 경기의 캐시가 화면 목록에 섞여 들어오면 안 된다 — 병합은 지금 보는
    /// 경기의 맵만 본다.
    #[test]
    fn another_games_cache_does_not_leak_into_the_merged_list() {
        let mut app = live_app_in_inning(5, 2); // 경기 "g", seq 100·101
        app.apply(Update::Inning {
            game_id: "g".into(),
            inning: 4,
            at_bats: vec![at_bat(99, 4, "earlier")],
        });

        // 다른 경기로 옮기고, 그 경기의 라이브 상태가 도착한다.
        app.enter_live(game("other-game"));
        let mut fresh = match &live_app_in_inning(3, 1).screen {
            Screen::Live { state: Some(s), .. } => s.clone(),
            _ => panic!("expected live screen"),
        };
        fresh.at_bats = vec![at_bat(500, 3, "other-batter")];
        app.apply(Update::Live("other-game".into(), fresh));

        let Screen::Live { state: Some(s), .. } = &app.screen else {
            panic!("expected live screen");
        };
        assert_eq!(
            s.at_bats.iter().map(|ab| ab.seq).collect::<Vec<_>>(),
            vec![500],
            "다른 경기의 캐시가 섞였다"
        );
    }

    /// 요청이 실패하면 로딩 표시는 걷되 캐시는 채우지 않는다 — 다시 눌러
    /// 재시도할 수 있어야 한다(일시적 네트워크 실패로 그 이닝을 영영 못 보면 안 된다).
    #[test]
    fn a_failed_inning_request_clears_the_spinner_and_stays_retryable() {
        let mut app = live_app_in_inning(5, 1);
        app.on_key(KeyCode::Char('['));
        assert_eq!(app.fetching_inning, Some(4));

        app.apply(Update::Error("boom".into()));
        assert_eq!(app.fetching_inning, None);
        assert!(
            app.cached_innings_of("g").is_empty(),
            "실패는 캐시를 채우지 않는다"
        );

        app.on_key(KeyCode::Char('['));
        assert_eq!(
            app.fetching_inning,
            Some(4),
            "같은 이닝을 다시 시도할 수 있어야 한다"
        );
    }

    /// "Inn 0"(응답의 inn이 결측일 때 파서가 만드는 라벨)을 실재 이닝으로 오인해
    /// 0회를 요청하면 안 된다.
    #[test]
    fn an_unknown_inning_label_does_not_produce_a_request() {
        let mut app = live_app_with_at_bats(2);
        if let Screen::Live { state: Some(s), .. } = &mut app.screen {
            for ab in &mut s.at_bats {
                ab.inning_label = "Inn 0".into();
            }
        }
        for _ in 0..3 {
            app.on_key(KeyCode::Char('['));
        }
        assert_eq!(app.fetching_inning, None);
    }

    /// 라이브 화면의 `gg`/`G`는 문자중계 커서의 양끝으로 간다(v0.18 리뷰 Minor).
    /// 그전까지 이 둘은 화면에 보이지도 않는 목록 선택을 움직여, 문자중계 맨 위/
    /// 맨 아래로 점프할 방법이 없었다.
    #[test]
    fn gg_and_shift_g_move_the_relay_cursor_in_the_live_view() {
        let mut app = live_app_with_at_bats(2); // 타석마다 relay_lines 2줄
        assert_eq!(app.live_relay_cursor, None);

        app.on_key(KeyCode::Char('g'));
        app.on_key(KeyCode::Char('g'));
        assert_eq!(app.live_relay_cursor, Some(0), "gg는 첫 줄로");

        app.on_key(KeyCode::Char('G'));
        assert_eq!(app.live_relay_cursor, Some(1), "G는 마지막 줄로");
    }

    /// 목록 화면에서는 기존 동작 그대로다 — 라이브에서만 축이 바뀐다.
    #[test]
    fn gg_and_shift_g_still_move_the_list_selection_outside_the_live_view() {
        let mut app = App::new(Default::default());
        app.games = vec![game("a"), game("b"), game("c")];
        app.selected = 1;

        app.on_key(KeyCode::Char('G'));
        assert_eq!(app.selected, 2);
        assert_eq!(app.live_relay_cursor, None);

        app.on_key(KeyCode::Char('g'));
        app.on_key(KeyCode::Char('g'));
        assert_eq!(app.selected, 0);
    }

    /// `G`로 간 줄이 투구 줄이면 그 투구가 함께 선택된다 — `j`/`k`와 같은 규칙
    /// (v0.19 연동). 커서만 옮기고 투구를 그대로 두면 두 선택이 다른 사건을 가리킨다.
    #[test]
    fn jumping_the_relay_cursor_also_moves_the_pitch_selection() {
        let mut app = live_app_with_at_bats(1); // 마지막 줄(line-0-b)이 투구 줄
        app.on_key(KeyCode::Char('G'));
        assert_eq!(app.live_relay_cursor, Some(1));
        assert_eq!(
            app.live_pitch_sel,
            Some(0),
            "투구 줄이면 그 투구가 함께 선택된다"
        );
    }
    /// config에 적어 둔 16진 액센트는 설정 화면에 **그 값 그대로** 뜬다(v0.22).
    /// v0.21까지는 모르는 값을 전부 "team"으로 뭉개, `#ff6600`을 적어 둔 사용자가
    /// 화면에서 "팀 컬러"를 보고 자기 설정이 사라진 줄 알게 됐다.
    #[test]
    fn a_hex_accent_is_shown_as_itself_in_the_settings_screen() {
        let mut app = App::new(Default::default());
        app.theme_accent = "#ff6600".into();
        let rows = app.settings_rows();
        let (_, _, value) = &rows[3];
        assert_eq!(value, "#ff6600");
    }

    /// hex도 명명색도 아닌 진짜 미상 값만 team으로 폴백한다(기존 관용 유지).
    #[test]
    fn an_unknown_accent_value_still_falls_back_to_the_team_label() {
        let mut app = App::new(Default::default());
        app.theme_accent = "chartreuse".into();
        let rows = app.settings_rows();
        let (_, _, value) = &rows[3];
        assert_eq!(value, app.labels().accent_team);
    }

    /// 라이브에 들어가면 목록 커서도 그 경기를 가리킨다(v0.22). `--team` 자동
    /// 진입이 커서를 안 맞춰, Esc로 나온 뒤 Enter가 남의 경기로 들어가던 결함.
    #[test]
    fn entering_live_points_the_list_cursor_at_that_game() {
        let mut app = App::new(Default::default());
        app.games = vec![game("a"), game("b"), game("c")];
        app.selected = 0;

        app.enter_live(game("c"));
        assert_eq!(app.selected, 2, "커서가 진입한 경기를 가리켜야 한다");
    }

    /// 목록에 없는 경기로 들어가면(날짜 전환 직후 등) 커서를 건드리지 않는다 —
    /// 엉뚱한 자리로 옮기느니 그대로 두는 편이 낫다.
    #[test]
    fn entering_a_game_missing_from_the_list_leaves_the_cursor_alone() {
        let mut app = App::new(Default::default());
        app.games = vec![game("a"), game("b")];
        app.selected = 1;

        app.enter_live(game("elsewhere"));
        assert_eq!(app.selected, 1);
    }
    /// 경기 탭의 Enter는 그대로 라이브에 들어간다 — v0.24에서 순위 탭 Enter에
    /// 다른 뜻(성적 오버레이)을 줬으므로, 같은 키를 나눠 쓰는 이 자리가 회귀에
    /// 가장 취약하다.
    #[test]
    fn enter_on_the_games_tab_still_opens_the_live_view() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Games;
        app.games = vec![game("g")];
        app.selected = 0;

        app.on_key(KeyCode::Enter);
        assert!(
            matches!(app.screen, Screen::Live { .. }),
            "라이브로 안 들어갔다"
        );
        assert!(app.team_stats_rank.is_none(), "경기 탭에서 성적이 열렸다");
    }

    /// 순위 탭의 Enter는 성적을 펼치고, 화면은 목록에 그대로 남는다.
    #[test]
    fn enter_on_the_standings_tab_opens_stats_without_leaving_the_list() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![standing_with_games(1, 94)]));
        app.selected = 0;

        app.on_key(KeyCode::Enter);
        assert_eq!(app.team_stats_rank, Some(1));
        assert!(matches!(app.screen, Screen::List), "화면이 바뀌었다");
    }

    /// 오버레이가 열려 있는 동안에는 다른 키가 목록을 움직이지 않는다 —
    /// 뒤에서 커서가 몰래 이동하면 닫았을 때 다른 팀이 선택돼 있다.
    #[test]
    fn the_stats_overlay_consumes_navigation_keys() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![
            standing_with_games(1, 94),
            standing_with_games(2, 94),
        ]));
        app.selected = 0;
        app.on_key(KeyCode::Enter);

        app.on_key(KeyCode::Down);
        assert_eq!(app.selected, 0, "오버레이 뒤에서 커서가 움직였다");
    }

    /// 폴링으로 순위가 갱신돼 배열이 재정렬돼도 보고 있던 팀이 유지된다 —
    /// 인덱스가 아니라 rank로 들고 있는 이유다(v0.18 seq와 같은 원리).
    #[test]
    fn the_open_team_survives_a_standings_refresh_that_reorders_the_table() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![
            standing_with_games(1, 94),
            standing_with_games(2, 94),
        ]));
        app.selected = 1; // 2위를 펼친다
        app.on_key(KeyCode::Enter);
        assert_eq!(app.team_stats_rank, Some(2));

        // 갱신: 순서가 바뀌어 같은 인덱스가 다른 팀을 가리키게 된다.
        app.apply(Update::Standings(vec![
            standing_with_games(2, 95),
            standing_with_games(1, 95),
        ]));
        assert_eq!(
            app.team_stats_target().map(|s| s.rank),
            Some(2),
            "갱신 후 다른 팀으로 바뀌었다"
        );
    }

    fn standing_with_games(rank: u16, games: u16) -> crate::model::Standing {
        crate::model::Standing {
            rank,
            team: crate::model::Team {
                code: "SS".into(),
                name: format!("팀{rank}"),
            },
            games,
            wins: 50,
            losses: 40,
            draws: 2,
            win_rate: 0.556,
            game_behind: 0.0,
            last_five: "WWLLD".into(),
            streak: "2패".into(),
            stats: crate::model::TeamStats {
                avg: 0.276,
                era: 4.06,
                ..Default::default()
            },
        }
    }
    /// 경기 전 팀에서는 오버레이가 뜨지 않고 **키도 잠기지 않는다**. 판정이
    /// 두 곳(열기·소비)에 흩어져 있으면 "화면은 없는데 입력만 먹히는" 상태가
    /// 생긴다 — 조건을 team_stats_target 하나로 모아 막는다.
    #[test]
    fn a_team_before_its_first_game_neither_opens_nor_locks_input() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![
            standing_with_games(1, 0),
            standing_with_games(2, 0),
        ]));
        app.selected = 0;

        app.on_key(KeyCode::Enter);
        assert!(app.team_stats_target().is_none(), "성적이 열렸다");

        // 입력이 잠기지 않았는지: 커서가 그대로 움직여야 한다.
        app.on_key(KeyCode::Down);
        assert_eq!(app.selected, 1, "화면도 없는데 입력이 잠겼다");
    }

    /// **오버레이를 열고 닫으면 화면 식별자가 바뀐다.** 이 값이 바뀌어야
    /// main.rs가 전체 재그리기를 한 번 강제하고, 그래야 라이브 화면의 착색 셀이
    /// 오버레이 위에 데드픽셀로 남지 않는다(v0.31, 실측으로 확인).
    #[test]
    fn opening_an_overlay_changes_the_view_key() {
        let mut app = App::new(Default::default());
        let base = app.view_key();

        app.show_help = true;
        assert_ne!(app.view_key(), base, "도움말");
        app.show_help = false;
        assert_eq!(app.view_key(), base, "닫으면 되돌아온다");

        app.settings = Some(SettingsState {
            cursor: 0,
            save_failed: false,
        });
        assert_ne!(app.view_key(), base, "설정");
        app.settings = None;

        app.news_list = Some(NewsListState { cursor: 0 });
        assert_ne!(app.view_key(), base, "뉴스 목록");
        app.news_list = None;

        app.standings = vec![standing_with_games(1, 10)];
        app.team_stats_rank = Some(1);
        assert_ne!(app.view_key(), base, "팀 성적");
        app.team_stats_rank = None;

        assert_eq!(app.view_key(), base, "전부 닫으면 처음과 같다");
    }
}
