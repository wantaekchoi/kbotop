use crate::config::Config;
use crate::model::{Game, GameStatus, LiveState, NewsItem, Standing};
use crate::poller::Update;
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    Standings,
}

/// F2 옵션 픽커의 세 pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Date,
    Team,
    Poll,
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
    /// TUI chrome 표시 언어(main이 --lang/config/env로 감지해 주입). 기본값은
    /// 테스트 결정성을 위해 En — 실사용 경로에서는 main이 항상 덮어쓴다.
    pub lang: crate::ui::i18n::Lang,
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
            news: vec![],
            options: None,
            poll_choice: 5,
            tips_override: None,
            link_picker: None,
            article_view: None,
            news_list: None,
            lang: crate::ui::i18n::Lang::En,
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
                KeyCode::Left => {
                    opt.pane = match opt.pane {
                        Pane::Date => Pane::Poll,
                        Pane::Team => Pane::Date,
                        Pane::Poll => Pane::Team,
                    };
                    opt.cursor = 0;
                }
                KeyCode::Right => {
                    opt.pane = match opt.pane {
                        Pane::Date => Pane::Team,
                        Pane::Team => Pane::Poll,
                        Pane::Poll => Pane::Date,
                    };
                    opt.cursor = 0;
                }
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
        // opener들은 모든 오버레이 consumer 뒤에 둔다 — 링크픽커가 열린 채 F2를
        // 누르면 오버레이가 이중으로 열리던 결함(최종 리뷰 I-1) 방지.
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
        match opt.pane {
            Pane::Date => {
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
            Pane::Team => {
                if let Some((_, code)) = crate::ui::options::team_items(l)
                    .into_iter()
                    .nth(opt.cursor)
                {
                    self.fav_code = code;
                }
            }
            Pane::Poll => {
                if let Some((_, secs)) = crate::ui::options::poll_items(l)
                    .into_iter()
                    .nth(opt.cursor)
                {
                    self.poll_choice = secs;
                }
            }
        }
    }

    fn current_len(&self) -> usize {
        match self.tab {
            Tab::Games => self.games.len(),
            Tab::Standings => self.standings.len(),
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
            // 목록이 열려 있는 채 뉴스가 짧아지면 커서가 범위 밖에 남아, 화면상
            // 마지막 항목이 선택된 것처럼 보이는데 Enter가 조용히 안 먹는 문제가
            // 있었다(리뷰 지적) — 교체 시점에 상태 필드 자체를 새 길이로 clamp한다.
            // 0건이 되면 saturating_sub로 0에 멈춘다(패닉 없음).
            if let Some(list) = &mut self.news_list {
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

    #[test]
    fn options_left_right_switch_pane_and_enter_applies_team() {
        let mut app = App::new(Default::default());
        app.on_key(KeyCode::F(2));
        app.on_key(KeyCode::Right); // Date → Team
        assert!(matches!(app.options.as_ref().unwrap().pane, Pane::Team));
        app.on_key(KeyCode::Down); // cursor 1 = 첫 실제 팀(0 = None 해제 항목)
        app.on_key(KeyCode::Enter);
        assert!(app.options.is_none(), "apply closes the overlay");
        assert!(app.fav_code.is_some(), "team selection applies to fav_code");
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

    #[test]
    fn options_poll_apply_updates_poll_choice() {
        let mut app = App::new(Default::default());
        app.poll_choice = 5;
        app.on_key(KeyCode::F(2));
        app.on_key(KeyCode::Left); // Date → Poll (좌측 순환: Date↔Poll↔Team)
        app.on_key(KeyCode::Down); // 3s → 5s? 항목 순서는 [3,5,10,30] — cursor 1 = 5
        app.on_key(KeyCode::Down); // cursor 2 = 10
        app.on_key(KeyCode::Enter);
        assert_eq!(app.poll_choice, 10);
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

    /// 뉴스가 0건으로 갱신되면(전부 사라짐) 커서가 0에 멈추고 패닉하지 않는다.
    #[test]
    fn news_refresh_to_empty_does_not_panic_and_clamps_cursor_to_zero() {
        let mut app = App::new(Default::default());
        app.apply(crate::poller::Update::News(vec![
            news_item("첫 기사", "https://x.kr/1"),
            news_item("둘째 기사", "https://x.kr/2"),
        ]));
        app.on_key(KeyCode::Char('n'));
        app.on_key(KeyCode::Char('j'));
        assert_eq!(app.news_list.as_ref().unwrap().cursor, 1);

        app.apply(crate::poller::Update::News(vec![]));
        assert_eq!(app.news_list.as_ref().unwrap().cursor, 0);

        // Enter는 조용히 무동작해야 한다(패닉 없음, 기사도 안 열림).
        app.on_key(KeyCode::Enter);
        assert!(app.article_view.is_none());
    }
}
