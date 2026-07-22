use crate::config::Config;
use crate::model::{Game, GameStatus, LiveState, Standing};
use crate::poller::Update;
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    Standings,
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
        }
    }

    /// 키 입력 처리. true 반환 시 종료.
    pub fn on_key(&mut self, key: KeyCode) -> bool {
        if self.show_help {
            // 도움말 화면에서는 아무 키나 눌러 닫는다.
            self.show_help = false;
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
                        }
                    }
                }
                self.pending_g = false;
            }
            KeyCode::Esc => {
                if matches!(self.screen, Screen::Live { .. }) {
                    self.screen = Screen::List;
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

    fn current_len(&self) -> usize {
        match self.tab {
            Tab::Games => self.games.len(),
            Tab::Standings => self.standings.len(),
        }
    }

    pub fn apply(&mut self, up: Update) {
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
                        *state = Some(l);
                    }
                }
            }
            Update::Error(e) => {
                self.last_error = Some(e);
                self.stale = true;
            }
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
}
