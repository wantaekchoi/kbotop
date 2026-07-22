use crate::error::Result;
use crate::model::{Game, LiveState, Standing};
use crate::source::DataSource;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

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

/// 폴링 스레드: 명령을 받아 주기적으로 소스를 호출하고 Update를 보낸다.
/// `live_poll_secs`는 라이브 뷰(relay) 폴링 주기다 — 설계 문서의 "라이브 뷰: 5초
/// 주기(설정 가능, 하한 3초)"에 해당하며, 호출자가 `Config::effective_poll_secs()`로
/// 하한을 적용해 넘긴다. games(60s)/standings(10s) 주기는 설계상 설정 대상이 아니므로
/// 그대로 하드코딩을 유지한다.
pub fn spawn(
    source: Arc<dyn DataSource>,
    date: String,
    rx: Receiver<Command>,
    tx: Sender<Update>,
    live_poll_secs: u64,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::time::{Duration, Instant};
        let mut watching: Option<Game> = None;
        let mut next_games = Instant::now();
        let mut next_live = Instant::now();
        // standings는 아직 못 받은 동안 main이 매 tick(~100ms)마다 RefreshStandings를
        // 재전송한다 — next_games/next_live와 같은 방식의 시간 게이트로 중복 요청을
        // 걸러내지 않으면, 응답이 느리거나 실패하는 동안(최대 10s 블로킹) 커맨드
        // 드레인 루프가 요청마다 블로킹 HTTP를 반복 호출해 Games/Live 폴링이
        // 굶주리고 Shutdown 처리까지 지연된다.
        let mut next_standings = Instant::now();

        loop {
            // 논블로킹 명령 처리
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Command::WatchGame(g) => {
                        watching = Some(g);
                        next_live = Instant::now();
                    }
                    Command::StopWatch => {
                        watching = None;
                    }
                    Command::RefreshStandings => {
                        let now = Instant::now();
                        if now >= next_standings {
                            let year = date.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(2026);
                            match call_source(|| source.standings(year)) {
                                Ok(s) => {
                                    let _ = tx.send(Update::Standings(s));
                                }
                                Err(e) => {
                                    let _ = tx.send(Update::Error(e.to_string()));
                                }
                            }
                            next_standings = now + Duration::from_secs(10);
                        }
                        // 게이트 이전 중복 요청은 조용히 버린다(이미 하나 처리 중/직후).
                    }
                    Command::Shutdown => return,
                }
            }

            let now = Instant::now();
            if now >= next_games {
                match call_source(|| source.games(&date)) {
                    Ok(g) => {
                        let _ = tx.send(Update::Games(g));
                    }
                    Err(e) => {
                        let _ = tx.send(Update::Error(e.to_string()));
                    }
                }
                next_games = now + Duration::from_secs(60);
            }
            if let Some(g) = &watching {
                if now >= next_live {
                    match call_source(|| source.live(g)) {
                        Ok(l) => {
                            let _ = tx.send(Update::Live(g.id.clone(), l));
                        }
                        Err(e) => {
                            let _ = tx.send(Update::Error(e.to_string()));
                        }
                    }
                    next_live = now + Duration::from_secs(live_poll_secs);
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
        let handle = spawn(source, "2026-07-19".into(), rx_cmd, tx_up, 5);

        // main.rs가 standings가 빌 때마다 매 tick(~100ms) 재전송하는 상황을
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
        let handle = spawn(source, "2026-07-19".into(), rx_cmd, tx_up, 5);

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
}
