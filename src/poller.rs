use crate::error::Result;
use crate::model::{Game, GameStatus, LiveState, NewsItem, Standing};
use crate::source::DataSource;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 폴러 → App 방향 메시지.
///
/// `LiveState`가 나머지 variant보다 커서 clippy가 boxing을 권하지만,
/// 채널을 통해 스레드당 몇 개 안 되는 메시지만 오가므로(폴링 주기 5s~60s)
/// 힙 간접 참조를 추가할 정도로 핫패스가 아니다 — 브리프의 타입을 그대로 유지.
#[allow(clippy::large_enum_variant)]
pub enum Update {
    Games(Vec<Game>),
    /// 이 상태를 가져온 대상 게임의 id를 함께 실어, 화면 전환 사이 도착한
    /// 이전 게임의 느린 응답이 새 게임 상태를 덮어쓰지 않도록 한다.
    Live(String, LiveState),
    Standings(Vec<Standing>),
    Error(String),
    /// HTTP 호출 직전 신호 — UI 스피너용. 데이터/에러 Update 도착이 완료 신호다.
    Fetching,
    /// KBO 뉴스 헤드라인(부가 기능). 실패는 절대 Update::Error로 보내지 않고
    /// 조용히 무시한다 — 뉴스 실패가 footer의 본 기능 에러 표시를 오염시키면 안 된다.
    News(Vec<NewsItem>),
}

/// App → 폴러 방향 명령.
pub enum Command {
    WatchGame(Game), // 라이브 화면 진입: 이 게임을 relay 폴링
    StopWatch,       // 목록 화면 복귀
    RefreshStandings,
    Shutdown,
}

/// 소스 호출(games/live/standings)을 감싸 패닉이 이 스레드 밖으로 새어나가지
/// 않게 한다. 이 스레드가 패닉한 채 unwind되면(dev/release 공통 `panic = "unwind"`
/// 기본값 — release도 `panic = "abort"`를 켜지 않는다, Cargo.toml 참고) main.rs의
/// 전역 panic hook이 *이 스레드에서* raw mode 해제/alt-screen 이탈/커서 표시를
/// 실행하는데, Tui를 소유한 메인 스레드는 이를 전혀 모른 채 계속 루프를 돌며
/// term.draw()를 호출해 이미 벗어난 화면 위에 계속 그림을 그린다.
/// 프로젝트 하드 제약("파싱은 관용적이며 패닉 금지")이 정확히 겨냥하는 지점이
/// 바로 이 외부 JSON 파싱 경로이므로, 패닉을 여기서 흡수해 Update::Error로
/// 변환하고 루프를 계속하게 한다.
fn call_source<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic in poller thread".to_string());
            Err(crate::error::Error::Data(format!("poller panicked: {msg}")))
        }
    }
}

/// standings 갱신 게이트 운영값(리뷰 수정: 기존 10s에서 완화). main.rs는 Standings
/// 탭이 떠 있는 동안 매 tick(~100ms) 조건 없이 `Command::RefreshStandings`를 보내고
/// (이전엔 `standings.is_empty()`일 때만 보내 최초 로드 후 다시는 안 오는 버그가
/// 있었다), 실제 fetch는 이 상수로 spawn()에 넘긴 게이트가 코얼레싱한다.
pub const STANDINGS_POLL_SECS: u64 = 90;

/// 워치 중인 게임이 `GameStatus::Final`일 때 쓰는 완화된 live 폴링 주기. 종료된
/// 경기는 relay 데이터가 더 바뀌지 않으므로 라이브 기본 주기(3~5s)로 계속 두드릴
/// 이유가 없다(design §12: "과도한 폴링으로 차단·민폐" 대응).
const FINAL_LIVE_POLL_SECS: u64 = 30;

/// 뉴스 헤드라인 폴링 주기. 부가 기능이라 games(60s)보다도 느슨하게 둔다.
const NEWS_POLL_SECS: u64 = 300;

/// 지수 백오프 상한. `base`가 이미 이보다 크면(games 기본 60s처럼) 그 base를
/// 그대로 상한으로 쓴다 — 원래 느슨한 주기가 에러 중에도 더 느려지지는 않는다.
const BACKOFF_CAP_SECS: u64 = 60;

/// 연속 에러 횟수(`errors`)에 따라 다음 폴링까지의 대기시간을 지수 백오프한다.
/// `base * 2^min(errors, 6)`을 상한(`BACKOFF_CAP_SECS`, 단 base 자체가 그보다 크면
/// base)으로 자른다. 429/5xx 등 네트워크 실패가 이어져도 무례하게 재시도를
/// 폭주시키지 않기 위한 설계 제약(design §6·§12)이며, 성공 한 번으로 호출부가
/// errors를 0으로 리셋하면 다음 계산은 다시 base로 돌아간다. 순수 함수로 분리해
/// 스레드/실시간 대기 없이 백오프 곡선 자체를 검증한다.
fn backoff_delay(base: Duration, errors: u32) -> Duration {
    let shift = errors.min(6);
    let scaled = base.saturating_mul(1u32 << shift);
    let cap = base.max(Duration::from_secs(BACKOFF_CAP_SECS));
    scaled.min(cap)
}

/// 폴링 스레드: 명령을 받아 주기적으로 소스를 호출하고 Update를 보낸다.
/// `live_poll_secs`는 라이브 뷰(relay) 폴링 주기다 — 설계 문서의 "라이브 뷰: 5초
/// 주기(설정 가능, 하한 3초)"에 해당하며, 호출자가 `Config::effective_poll_secs()`로
/// 하한을 적용해 넘긴다. `standings_poll_secs`는 순위표 갱신 게이트로, 운영
/// 호출자는 `STANDINGS_POLL_SECS`(90s)를 넘기고, 테스트는 더 짧은 값을 넣어
/// "게이트 경과 후 재개"를 실시간 대기 없이 가깝게 검증한다. games(60s) 주기는
/// 설계상 설정 대상이 아니므로 그대로 하드코딩을 유지한다.
///
/// games/live 각각 연속 에러 횟수를 세어 `backoff_delay`로 다음 폴링 대기를
/// 지수 백오프하고, 성공하면 카운터를 0으로 리셋한다. 또한 워치 중인 게임이
/// `GameStatus::Final`이면(데이터가 더 바뀌지 않으므로) live 기본 주기 대신
/// `FINAL_LIVE_POLL_SECS`(30s)를 쓴다 — Live/Suspended는 기존처럼
/// `live_poll_secs` 그대로 사용한다.
pub fn spawn(
    source: Arc<dyn DataSource>,
    date: String,
    rx: Receiver<Command>,
    tx: Sender<Update>,
    live_poll_secs: u64,
    standings_poll_secs: u64,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut watching: Option<Game> = None;
        let mut next_games = Instant::now();
        let mut next_live = Instant::now();
        // standings는 main이 Standings 탭이 떠 있는 동안 매 tick(~100ms)마다
        // RefreshStandings를 재전송한다 — next_games/next_live와 같은 방식의 시간
        // 게이트로 중복 요청을 걸러내지 않으면, 응답이 느리거나 실패하는 동안(최대
        // 10s 블로킹) 커맨드 드레인 루프가 요청마다 블로킹 HTTP를 반복 호출해
        // Games/Live 폴링이 굶주리고 Shutdown 처리까지 지연된다.
        let mut next_standings = Instant::now();
        let mut next_news = Instant::now();
        let mut games_errors: u32 = 0;
        let mut live_errors: u32 = 0;

        loop {
            // 논블로킹 명령 처리
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Command::WatchGame(g) => {
                        watching = Some(g);
                        next_live = Instant::now();
                        // 새 게임 감시 시작: 이전 게임에서 쌓인 백오프 상태를 물려받지
                        // 않는다 — 다른 게임의 연속 에러가 이 게임의 첫 폴링부터 지연시키면
                        // 안 된다.
                        live_errors = 0;
                    }
                    Command::StopWatch => {
                        watching = None;
                    }
                    Command::RefreshStandings => {
                        let now = Instant::now();
                        if now >= next_standings {
                            let year = date.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(2026);
                            let _ = tx.send(Update::Fetching);
                            match call_source(|| source.standings(year)) {
                                Ok(s) => {
                                    let _ = tx.send(Update::Standings(s));
                                }
                                Err(e) => {
                                    let _ = tx.send(Update::Error(e.to_string()));
                                }
                            }
                            next_standings = now + Duration::from_secs(standings_poll_secs);
                        }
                        // 게이트 이전 중복 요청은 조용히 버린다(이미 하나 처리 중/직후) —
                        // 탭이 Standings인 동안 main이 매 tick 재전송해도 실제 fetch는
                        // 게이트 주기로만 나간다(버그 수정: 게이트 경과 후엔 다시 통과해
                        // 갱신이 세션 내내 반복된다).
                    }
                    Command::Shutdown => return,
                }
            }

            let now = Instant::now();
            if now >= next_games {
                let _ = tx.send(Update::Fetching);
                match call_source(|| source.games(&date)) {
                    Ok(g) => {
                        let _ = tx.send(Update::Games(g));
                        games_errors = 0;
                    }
                    Err(e) => {
                        let _ = tx.send(Update::Error(e.to_string()));
                        games_errors = games_errors.saturating_add(1);
                    }
                }
                next_games = now + backoff_delay(Duration::from_secs(60), games_errors);
            }
            if now >= next_news {
                // 뉴스는 부가 기능: 실패를 Update::Error로 보내면 본 기능(footer)이
                // 오염되므로 성공만 반영하고 실패는 조용히 다음 주기로 미룬다.
                if let Ok(n) = call_source(|| source.news()) {
                    let _ = tx.send(Update::News(n));
                }
                next_news = now + Duration::from_secs(NEWS_POLL_SECS);
            }
            if let Some(g) = &watching {
                if now >= next_live {
                    // 종료된 경기는 relay 데이터가 더 바뀌지 않으므로 완화된 주기를 쓴다
                    // (Live/Suspended는 기존 live_poll_secs 그대로).
                    let base = if g.status == GameStatus::Final {
                        Duration::from_secs(FINAL_LIVE_POLL_SECS)
                    } else {
                        Duration::from_secs(live_poll_secs)
                    };
                    let _ = tx.send(Update::Fetching);
                    match call_source(|| source.live(g)) {
                        Ok(l) => {
                            let _ = tx.send(Update::Live(g.id.clone(), l));
                            live_errors = 0;
                        }
                        Err(e) => {
                            let _ = tx.send(Update::Error(e.to_string()));
                            live_errors = live_errors.saturating_add(1);
                        }
                    }
                    next_live = now + backoff_delay(base, live_errors);
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{Error, Result};
    use crate::model::{BaseState, Count, Team};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    /// standings() 호출 횟수만 세고 항상 실패하는 소스 — RefreshStandings가
    /// 게이트 없이 매번 블로킹 호출로 이어지는지 확인하기 위한 더블.
    struct CountingSource {
        standings_calls: Arc<AtomicUsize>,
    }

    impl DataSource for CountingSource {
        fn games(&self, _date: &str) -> Result<Vec<Game>> {
            Ok(vec![])
        }
        fn live(&self, _game: &Game) -> Result<LiveState> {
            Err(Error::Config("not used in this test".into()))
        }
        fn standings(&self, _year: u16) -> Result<Vec<Standing>> {
            self.standings_calls.fetch_add(1, Ordering::SeqCst);
            Err(Error::Config("boom".into()))
        }
    }

    #[test]
    fn refresh_standings_flood_is_coalesced_by_time_gate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let source: Arc<dyn DataSource> = Arc::new(CountingSource {
            standings_calls: calls.clone(),
        });
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_up, rx_up) = mpsc::channel::<Update>();
        let handle = spawn(
            source,
            "2026-07-19".into(),
            rx_cmd,
            tx_up,
            5,
            STANDINGS_POLL_SECS,
        );

        // main.rs가 Standings 탭이 떠 있는 동안 매 tick(~100ms) 재전송하는 상황을
        // 흉내내 RefreshStandings를 몰아서 보낸다.
        for _ in 0..50 {
            let _ = tx_cmd.send(Command::RefreshStandings);
        }
        let _ = tx_cmd.send(Command::Shutdown);
        handle.join().unwrap();

        // Shutdown은 큐의 맨 뒤에 있으므로 join()이 반환한 시점엔 앞선 50개의
        // RefreshStandings가 모두 드레인된 뒤다 — 게이트가 없다면 50번 모두
        // source.standings()를 블로킹 호출했겠지만, 게이트가 있으면 1회로 수렴한다.
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        drop(rx_up); // 미수신 Update가 쌓여도 상관없음(테스트 관심사 아님)
    }

    /// games()를 호출할 때마다 패닉하는 소스 — 폴러 스레드가 패닉을 흡수하고
    /// (스레드 자체는 죽지 않고) Update::Error로 변환해 계속 도는지 확인한다.
    struct PanickingGamesSource;

    impl DataSource for PanickingGamesSource {
        fn games(&self, _date: &str) -> Result<Vec<Game>> {
            panic!("boom: simulated panic in games()");
        }
        fn live(&self, _game: &Game) -> Result<LiveState> {
            Err(Error::Config("not used in this test".into()))
        }
        fn standings(&self, _year: u16) -> Result<Vec<Standing>> {
            Err(Error::Config("not used in this test".into()))
        }
    }

    #[test]
    fn panic_inside_games_call_is_caught_and_reported_as_error_without_killing_the_thread() {
        let source: Arc<dyn DataSource> = Arc::new(PanickingGamesSource);
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_up, rx_up) = mpsc::channel::<Update>();
        let handle = spawn(
            source,
            "2026-07-19".into(),
            rx_cmd,
            tx_up,
            5,
            STANDINGS_POLL_SECS,
        );

        // 첫 메시지는 호출 직전 스피너용 Fetching이므로 건너뛰고, 패닉이
        // 흡수돼 나온 실제 결과(Update::Error)를 확인한다.
        let fetching = rx_up
            .recv_timeout(Duration::from_secs(2))
            .expect("expected an Update before timeout");
        assert!(matches!(fetching, Update::Fetching));

        let up = rx_up
            .recv_timeout(Duration::from_secs(2))
            .expect("expected an Update before timeout");
        match up {
            Update::Error(e) => assert!(e.contains("panic"), "unexpected error text: {e}"),
            _ => panic!("expected Update::Error from the panicking games() call"),
        }

        let _ = tx_cmd.send(Command::Shutdown);
        // 패닉이 catch_unwind를 뚫고 나갔다면 스레드 자체가 unwind되어
        // join()이 Err(Box<dyn Any>)를 반환했을 것이다 — Ok(())라는 것 자체가
        // 스레드가 살아남아 정상적으로 return했다는 증거다.
        assert!(handle.join().is_ok());
    }

    #[test]
    fn refresh_standings_recurs_after_gate_elapses_not_just_once() {
        // 버그 회귀 테스트: 이전엔 main.rs가 `standings.is_empty()`일 때만
        // RefreshStandings를 보내, 최초 로드 이후엔 탭이 Standings에 계속 머물러도
        // 다시는 fetch가 나가지 않았다(세션 내내 스냅샷 고정). 지금은 탭이 떠 있는
        // 동안 조건 없이 재전송하고 poller 쪽 시간 게이트가 코얼레싱하므로, 게이트를
        // 한 번 넘기면 두 번째 fetch가 나가야 한다. 실제 운영 게이트(90s)를 그대로
        // 쓰면 테스트가 느려지므로 spawn()의 standings_poll_secs에 짧은 값(1s)을
        // 넣어 동일한 로직을 빠르게 검증한다.
        let calls = Arc::new(AtomicUsize::new(0));
        let source: Arc<dyn DataSource> = Arc::new(CountingSource {
            standings_calls: calls.clone(),
        });
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_up, rx_up) = mpsc::channel::<Update>();
        let handle = spawn(source, "2026-07-19".into(), rx_cmd, tx_up, 5, 1);

        let _ = tx_cmd.send(Command::RefreshStandings);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "first RefreshStandings after spawn should fetch once"
        );

        // 게이트(1s)를 넘긴 뒤 재전송하면 두 번째 fetch가 나가야 한다 — 여기가
        // "최초 로드 후 얼어붙는다"는 원래 버그가 고쳐졌음을 보여주는 핵심 지점.
        std::thread::sleep(Duration::from_millis(900));
        let _ = tx_cmd.send(Command::RefreshStandings);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "standings should refresh again once the gate elapses, not freeze after the first load"
        );

        let _ = tx_cmd.send(Command::Shutdown);
        handle.join().unwrap();
        drop(rx_up);
    }

    #[test]
    fn backoff_delay_grows_with_errors_then_caps() {
        let base = Duration::from_secs(5);
        assert_eq!(backoff_delay(base, 0), Duration::from_secs(5));
        assert_eq!(backoff_delay(base, 1), Duration::from_secs(10));
        assert_eq!(backoff_delay(base, 2), Duration::from_secs(20));
        assert_eq!(backoff_delay(base, 3), Duration::from_secs(40));
        // 5 * 2^4 = 80 > 60s 상한이므로 잘린다.
        assert_eq!(backoff_delay(base, 4), Duration::from_secs(60));
        assert_eq!(backoff_delay(base, 20), Duration::from_secs(60));
    }

    #[test]
    fn backoff_delay_does_not_shrink_a_base_already_at_or_above_the_cap() {
        // games처럼 base(60s)가 이미 상한과 같으면, 에러가 쌓여도 그 이상 느려지지
        // 않는다(원래도 느슨한 주기라 추가로 느려질 필요가 없다) — 하지만 base보다
        // 빨라지지도 않는다.
        let base = Duration::from_secs(60);
        assert_eq!(backoff_delay(base, 0), base);
        assert_eq!(backoff_delay(base, 5), base);
    }

    fn sample_game(id: &str, status: GameStatus) -> Game {
        Game {
            id: id.into(),
            start: "".into(),
            status,
            status_label: "".into(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            home_score: None,
            away_score: None,
        }
    }

    fn sample_live_state() -> LiveState {
        LiveState {
            inning_label: "".into(),
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
            count: Count {
                ball: 0,
                strike: 0,
                out: 0,
            },
            bases: BaseState {
                first: false,
                second: false,
                third: false,
            },
            pitcher_name: "".into(),
            batter_name: "".into(),
            home_win_rate: None,
            away_win_rate: None,
            relay_log: vec![],
            current_pitches: vec![],
            next_batter_name: String::new(),
        }
    }

    /// live()를 1번째=에러, 2번째=성공, 3번째=에러로 응답하는 소스 — 연속 에러 시
    /// 백오프가 커지고 성공 한 번으로 리셋되는지를 실제 호출 간격으로 검증하기
    /// 위한 더블.
    struct FlakyLiveSource {
        live_calls: Arc<AtomicUsize>,
    }

    impl DataSource for FlakyLiveSource {
        fn games(&self, _date: &str) -> Result<Vec<Game>> {
            Ok(vec![])
        }
        fn live(&self, _game: &Game) -> Result<LiveState> {
            let n = self.live_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 2 {
                Ok(sample_live_state())
            } else {
                Err(Error::Config("boom".into()))
            }
        }
        fn standings(&self, _year: u16) -> Result<Vec<Standing>> {
            Err(Error::Config("not used in this test".into()))
        }
    }

    #[test]
    fn live_backoff_widens_on_consecutive_errors_and_resets_on_success() {
        let live_calls = Arc::new(AtomicUsize::new(0));
        let source: Arc<dyn DataSource> = Arc::new(FlakyLiveSource {
            live_calls: live_calls.clone(),
        });
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_up, rx_up) = mpsc::channel::<Update>();
        // live_poll_secs=1로 짧게 둬, 실제 라이브 기본 주기(3~5s)를 기다리지 않고도
        // 백오프 배율을 관찰한다.
        let handle = spawn(
            source,
            "2026-07-19".into(),
            rx_cmd,
            tx_up,
            1,
            STANDINGS_POLL_SECS,
        );

        let _ = tx_cmd.send(Command::WatchGame(sample_game("a", GameStatus::Live)));

        // 초기 Update::Games(빈 목록)는 관심사가 아니므로 건너뛰고 live 관련
        // 이벤트(Update::Live/Update::Error)만 3개(에러·성공·에러) 도착 시각을 모은다.
        let mut live_events_at = Vec::new();
        while live_events_at.len() < 3 {
            match rx_up
                .recv_timeout(Duration::from_secs(5))
                .expect("expected an Update before timeout")
            {
                Update::Live(..) | Update::Error(_) => live_events_at.push(Instant::now()),
                Update::Games(_) | Update::Standings(_) => {}
                Update::Fetching => {}
                Update::News(_) => {}
            }
        }

        let gap_after_error = live_events_at[1] - live_events_at[0];
        let gap_after_success = live_events_at[2] - live_events_at[1];

        assert!(
            gap_after_error >= Duration::from_millis(1800),
            "expected the delay to widen past the 1s base after an error, got {gap_after_error:?}"
        );
        assert!(
            gap_after_success < gap_after_error,
            "expected the gap to shrink back toward base after a successful fetch (reset), \
             got success={gap_after_success:?} vs post-error={gap_after_error:?}"
        );
        assert!(
            gap_after_success < Duration::from_millis(1800),
            "expected the post-success gap to be back near the 1s base, not still backed off, \
             got {gap_after_success:?}"
        );

        let _ = tx_cmd.send(Command::Shutdown);
        handle.join().unwrap();
    }

    /// live() 호출마다 성공을 반환하며 호출 횟수만 세는 소스 — Final 상태 게임이
    /// 촘촘한 base 주기 대신 완화된 주기를 쓰는지 확인하기 위한 더블.
    struct CountingLiveSource {
        live_calls: Arc<AtomicUsize>,
    }

    impl DataSource for CountingLiveSource {
        fn games(&self, _date: &str) -> Result<Vec<Game>> {
            Ok(vec![])
        }
        fn live(&self, _game: &Game) -> Result<LiveState> {
            self.live_calls.fetch_add(1, Ordering::SeqCst);
            Ok(sample_live_state())
        }
        fn standings(&self, _year: u16) -> Result<Vec<Standing>> {
            Err(Error::Config("not used in this test".into()))
        }
    }

    /// 폴러는 각 HTTP 호출 직전 Fetching을 보낸다 — games 첫 폴링에서 확인.
    #[test]
    fn poller_announces_fetching_before_each_call() {
        let source: Arc<dyn DataSource> = Arc::new(PanickingGamesSource);
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_up, rx_up) = mpsc::channel::<Update>();
        let handle = spawn(
            source,
            "2026-07-19".into(),
            rx_cmd,
            tx_up,
            5,
            STANDINGS_POLL_SECS,
        );
        let first = rx_up.recv_timeout(Duration::from_secs(2)).expect("update");
        assert!(
            matches!(first, Update::Fetching),
            "first message must be Fetching"
        );
        let second = rx_up.recv_timeout(Duration::from_secs(2)).expect("update");
        assert!(matches!(second, Update::Error(_)));
        let _ = tx_cmd.send(Command::Shutdown);
        assert!(handle.join().is_ok());
    }

    #[test]
    fn final_status_watched_game_uses_relaxed_live_interval_not_the_tight_base() {
        let calls = Arc::new(AtomicUsize::new(0));
        let source: Arc<dyn DataSource> = Arc::new(CountingLiveSource {
            live_calls: calls.clone(),
        });
        let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
        let (tx_up, rx_up) = mpsc::channel::<Update>();
        // live_poll_secs=1: Final이 아니었다면 3초 동안 최소 2~3번은 호출됐어야 한다.
        let handle = spawn(
            source,
            "2026-07-19".into(),
            rx_cmd,
            tx_up,
            1,
            STANDINGS_POLL_SECS,
        );
        let _ = tx_cmd.send(Command::WatchGame(sample_game("a", GameStatus::Final)));

        std::thread::sleep(Duration::from_millis(3000));
        let n = calls.load(Ordering::SeqCst);
        assert_eq!(
            n, 1,
            "a Final-status watched game should use the relaxed (~30s) interval, \
             but live() was called {n} times within 3s (looks like the tight 1s base)"
        );

        let _ = tx_cmd.send(Command::Shutdown);
        handle.join().unwrap();
        drop(rx_up);
    }
}
