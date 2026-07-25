use crate::config::Config;
use crate::model::{Game, GameStatus, LiveState, NewsItem, Standing};
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

/// 액센트 값을 설정 화면에 보여줄 라벨로 바꾼다. 알 수 없는 값은 관용적으로
/// "team" 라벨로 표시한다(accent_for의 기본 폴백과 다르지만, 설정 화면
/// 표시는 "무엇이 선택돼 있나"를 최대한 그럴듯하게 보여주는 게 목적이다).
fn theme_accent_label(l: &crate::ui::i18n::Labels, accent: &str) -> &'static str {
    match accent {
        "cyan" => l.accent_cyan,
        "green" => l.accent_green,
        "yellow" => l.accent_yellow,
        "magenta" => l.accent_magenta,
        "blue" => l.accent_blue,
        "red" => l.accent_red,
        "none" => l.accent_none,
        _ => l.accent_team,
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
    pub live_pitch_sel: Option<usize>,
    /// 응원 팀 KBO 코드(main이 --team/config favorite_team 별칭을 해석해 주입).
    /// UI 테마 액센트와 헤더 응원 배지에 쓴다.
    pub fav_code: Option<String>,
    /// UTC epoch 초(main.rs가 tick마다 갱신). 초보용 팁 회전(tips::current)의
    /// 입력으로만 쓰인다 — 실제 벽시계와 무관하게 결정적으로 테스트 가능하다.
    pub now_secs: u64,
    /// 마지막 "성공" 갱신(Games/Standings/Live 반영) 시각의 now_secs 스냅샷
    /// (v0.15 A-2). None = 아직 한 번도 성공한 적 없음. `stale`(이진값)과 달리
    /// "몇 초 전"까지 보여주기 위한 값 — apply()가 Error/Fetching에서는
    /// 갱신하지 않는다(660행 근처 주석과 같은 철학: 시도 신호일 뿐 회복이 아니다).
    pub last_update_secs: Option<u64>,
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
}

impl App {
    pub fn new(config: Config) -> Self {
        App {
            config,
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
            fav_code: None,
            now_secs: 0,
            last_update_secs: None,
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
                }
                self.tab = match self.tab {
                    Tab::Games => Tab::Standings,
                    Tab::Standings => Tab::Games,
                };
                self.selected = 0;
                self.pending_g = false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.current_len();
                if len > 0 && self.selected + 1 < len {
                    self.selected += 1;
                }
                self.pending_g = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                self.pending_g = false;
            }
            KeyCode::Left | KeyCode::Right => {
                // 라이브 화면에서 현 타석 투구를 하나씩 짚어본다(순환 없음).
                // 선택 없음 = 전체 보기; Right는 처음부터, Left는 마지막부터 진입.
                if let Screen::Live { state: Some(s), .. } = &self.screen {
                    let n = s.current_pitches.len();
                    if n > 0 {
                        self.live_pitch_sel = Some(match (self.live_pitch_sel, key) {
                            (None, KeyCode::Right) => 0,
                            (None, _) => n - 1,
                            (Some(i), KeyCode::Right) => (i + 1).min(n - 1),
                            (Some(i), _) => i.saturating_sub(1),
                        });
                    }
                }
                self.pending_g = false;
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.selected = 0;
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                return false;
            }
            KeyCode::Char('G') => {
                self.selected = self.current_len().saturating_sub(1);
                self.pending_g = false;
            }
            KeyCode::Enter => {
                if self.tab == Tab::Games && matches!(self.screen, Screen::List) {
                    if let Some(g) = self.games.get(self.selected).cloned() {
                        if Self::can_enter_live(g.status) {
                            self.screen = Screen::Live {
                                game: g,
                                state: None,
                            };
                            // 이전 게임에서 짚어보던 투구 선택이 새 게임으로 넘어오지 않도록.
                            self.live_pitch_sel = None;
                        }
                    }
                }
                self.pending_g = false;
            }
            KeyCode::Esc => {
                if self.live_pitch_sel.is_some() {
                    // 1단계: 투구 선택 해제(전체 보기 복귀). 화면은 유지.
                    self.live_pitch_sel = None;
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
            KeyCode::Char('/')
            | KeyCode::F(3)
            | KeyCode::F(4)
            | KeyCode::F(6)
            | KeyCode::Char(' ') => {
                // 마일스톤 B에서 구현: 검색, 필터, 정렬, 즐겨찾기. 지금은 인식만 하고 무동작.
                self.pending_g = false;
            }
            _ => {
                self.pending_g = false;
            }
        }
        false
    }

    /// Canceled/Scheduled 게임은 relay가 textRelayData를 절대 내려주지 않으므로
    /// 진입시키면 사용자에게 이유를 알릴 수 없는 영구 "loading..." 화면에 갇힌다.
    /// Enter 키 진입(on_key)과 `--team` 자동 진입(main.rs) 두 경로가 각자 가드를
    /// 들고 있으면 언젠가 하나만 고쳐지고 어긋나므로, 이 판단을 여기 한 곳에 둔다.
    pub fn can_enter_live(status: GameStatus) -> bool {
        !matches!(status, GameStatus::Canceled | GameStatus::Scheduled)
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
                theme_accent_label(l, &self.theme_accent).to_string(),
            ),
            (
                SettingKind::Lang,
                l.set_lang,
                crate::ui::i18n::lang_display_name(self.lang).to_string(),
            ),
        ]
    }

    /// 현재 영속 대상(팀·폴링·언어·테마)을 Config로 만들어 저장한다. 실패는 삼켜
    /// settings.save_failed에 반영한다(무패닉·조용한 저하).
    fn persist(&mut self) {
        let cfg = crate::config::Config {
            favorite_team: self.fav_code.clone(),
            poll_secs: self.poll_choice,
            lang: Some(crate::ui::i18n::lang_code(self.lang).to_string()),
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
                        // 새 타석(투구 수 감소)이면 선택 리셋; 같은 타석에 투구가
                        // 추가된 경우는 선택 유지. 방어적으로 범위 밖 선택도 해제.
                        if let Some(prev) = state {
                            if l.current_pitches.len() < prev.current_pitches.len() {
                                self.live_pitch_sel = None;
                            }
                        }
                        if let Some(i) = self.live_pitch_sel {
                            if i >= l.current_pitches.len() {
                                self.live_pitch_sel = None;
                            }
                        }
                        *state = Some(l);
                    }
                }
            }
            Update::Error(e) => {
                self.last_error = Some(e);
                self.stale = true;
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

    /// "이번 프레임이 보여주는 화면이 무엇인가"를 식별하는 최소 키(screen, tab).
    /// main.rs의 렌더 루프가 직전 프레임의 키와 비교해 값이 달라졌을 때만
    /// `term.clear()`를 호출한다 — ratatui 0.30에서 화면 전환(Live↔List,
    /// Games↔Standings) 시 내부 버퍼와 실제 터미널이 어긋나 이전 화면의 착색
    /// 셀이 지워지지 않는 문제(ADR-0007)를 잡기 위함이다.
    ///
    /// 오버레이(help/settings/article/newslist/options/link_picker)는 이미 `Clear`
    /// 위젯으로 정상 처리되므로 의도적으로 이 키에 포함하지 않는다 — 포함하면
    /// 오버레이를 열고 닫을 때마다 불필요한 전체 화면 클리어(깜빡임)가 생긴다.
    pub fn view_key(&self) -> (u8, u8) {
        let screen = match self.screen {
            Screen::List => 0,
            Screen::Live { .. } => 1,
        };
        let tab = match self.tab {
            Tab::Games => 0,
            Tab::Standings => 1,
        };
        (screen, tab)
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
        }
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

    /// 오버레이(help/options/link_picker/settings)는 Clear 위젯으로 이미 정상
    /// 처리되므로 view_key에 포함되면 안 된다 — 포함되면 오버레이를 열고 닫을
    /// 때마다 main.rs가 불필요하게 term.clear()를 호출해 깜빡임이 생긴다.
    #[test]
    fn view_key_is_unaffected_by_overlays() {
        let mut app = App::new(Default::default());
        let base = app.view_key();

        app.on_key(KeyCode::Char('?')); // help
        assert_eq!(app.view_key(), base);
        app.on_key(KeyCode::Char('?')); // close(아무 키나 닫힘)

        app.on_key(KeyCode::F(2)); // options
        assert_eq!(app.view_key(), base);
        app.on_key(KeyCode::F(2)); // close

        app.on_key(KeyCode::Char('o')); // link_picker
        assert_eq!(app.view_key(), base);
        app.on_key(KeyCode::Esc); // close

        app.on_key(KeyCode::F(9)); // settings
        assert_eq!(app.view_key(), base);
        app.on_key(KeyCode::F(9)); // close
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

    /// A-2: Fetching은 "시도 신호일 뿐 회복이 아니다"(660행 근처 주석과 동일
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
            current_pitches: pitches,
            next_batter_name: String::new(),
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
        assert_eq!(rows.len(), 5);
        assert!(matches!(rows[2].0, SettingKind::ThemePreset));
        assert!(matches!(rows[3].0, SettingKind::ThemeAccent));
        assert!(matches!(rows[4].0, SettingKind::Lang));
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
}
