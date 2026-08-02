pub mod dto;
pub mod map;

use crate::error::Result;
use crate::model::{AtBat, Game, LiveState, Standing};
use crate::source::DataSource;

const BASE: &str = "https://api-gw.sports.naver.com";
// 네이버 API와 무관한 별개 도메인(프로젝트 저장소 raw) — `base`와 분리된
// 필드로 둬야 `with_base()`로 API base를 로컬 테스트 서버로 바꿔도 이 URL의
// 기본값(실 서비스 주소)이 실수로 같이 바뀌지 않는다.
const TIPS_URL: &str = "https://raw.githubusercontent.com/wantaekchoi/kbotop/main/data/tips.txt";

pub struct NaverSource {
    agent: ureq::Agent,
    user_agent: String,
    base: String,
    tips_url: String,
}

impl NaverSource {
    pub fn new() -> Self {
        Self::build(BASE.to_string(), TIPS_URL.to_string(), true)
    }

    /// 테스트 전용 생성자: API base와 tips URL을 모두 주어진 주소로
    /// override한다(로컬 mock 서버를 가리키게 해 실네트워크 없이 HTTP 경로를
    /// 검증하기 위함). `new()`의 기본 동작·기본 URL·공개 시그니처는 그대로다.
    #[cfg(test)]
    fn with_base(base: impl Into<String>) -> Self {
        let base = base.into();
        // 로컬 mock은 http라 여기서는 https 강제를 끈다(프로덕션 경로는 켠다).
        Self::build(base.clone(), base, false)
    }

    fn build(base: String, tips_url: String, https_only: bool) -> Self {
        NaverSource {
            agent: ureq::AgentBuilder::new()
                // https로 시작한 요청이 리다이렉트를 타고 **평문으로 떨어지는
                // 것**을 막는다. ureq 기본값은 이를 허용한다.
                .https_only(https_only)
                .timeout(std::time::Duration::from_secs(10))
                .build(),
            user_agent: format!(
                "kbotop/{} (+github.com/wantaekchoi/kbotop; personal use)",
                env!("CARGO_PKG_VERSION")
            ),
            base,
            tips_url,
        }
    }

    fn get(&self, url: &str) -> Result<String> {
        let body = self
            .agent
            .get(url)
            .set("User-Agent", &self.user_agent)
            .call()
            .map_err(Box::new)?
            .into_string()?;
        Ok(body)
    }
}

impl Default for NaverSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSource for NaverSource {
    fn games(&self, date: &str) -> Result<Vec<Game>> {
        // `fields`를 붙이지 않으면 기본 19개 필드만 온다 — 선발투수·구장·중계
        // 채널은 요청해야 실린다(v0.23 실측). `basic`은 기존에 받던 것들이라
        // 빼면 안 된다.
        let url = format!(
            "{}/schedule/games?fields=basic,stadium,homeStarterName,awayStarterName,broadChannel&upperCategoryId=kbaseball&categoryId=kbo&fromDate={date}&toDate={date}",
            self.base
        );
        map::games_from_schedule(&self.get(&url)?)
    }

    fn live(&self, game: &Game) -> Result<LiveState> {
        let url = format!("{}/schedule/games/{}/relay", self.base, game.id);
        map::live_from_relay(&self.get(&url)?, game.home.clone(), game.away.clone())
    }

    fn at_bats_of_inning(&self, game: &Game, inning: u8) -> Result<Vec<AtBat>> {
        // 같은 relay 엔드포인트에 이닝만 얹는다. 응답 스키마는 기본 호출과 동일하고
        // 요청한 이닝의 초·말이 모두 담긴다(실측). 범위 밖 이닝은 200 + 빈 배열이라
        // 에러 경로를 타지 않는다.
        let url = format!(
            "{}/schedule/games/{}/relay?inning={inning}",
            self.base, game.id
        );
        map::at_bats_from_relay(&self.get(&url)?)
    }

    fn standings(&self, year: u16) -> Result<Vec<Standing>> {
        let url = format!(
            "{}/statistics/categories/kbo/seasons/{year}/teams",
            self.base
        );
        map::standings_from_json(&self.get(&url)?)
    }

    // 네이버 API가 아니라 프로젝트 저장소 raw지만, HTTP 클라이언트를 가진 유일한
    // 소스 객체라 여기 얹는다 — 별도 소스 추상화는 YAGNI.
    fn tips(&self) -> Result<Vec<String>> {
        let raw = self.get(&self.tips_url)?;
        Ok(crate::ui::tips::parse_remote(&raw).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GameStatus, Team};
    use crate::source::DataSource;

    #[test]
    #[ignore] // 네트워크 필요: `cargo test -- --ignored`로 실행
    fn fetches_today_games() {
        let src = NaverSource::new();
        let games = src.games("2026-07-19").unwrap();
        assert!(!games.is_empty());
    }

    /// 프로젝트가 요구하는 UA 포맷을 독립적인 리터럴 조각들로 검증한다 — 네트워크
    /// 불필요. `new()` 내부와 동일한 `format!()` 호출을 그대로 재구성해 비교하면
    /// UA 리터럴이 깨져도 이 테스트가 항상 통과하는 항진명제가 된다(리뷰 라운드
    /// 5) — 대신 각 조각을 별도로 하드코딩해서 `new()`의 리터럴 변경이 실제로
    /// 이 테스트를 실패시키게 한다.
    #[test]
    fn user_agent_matches_required_format() {
        let src = NaverSource::new();
        assert!(
            src.user_agent.starts_with("kbotop/"),
            "unexpected UA prefix: {}",
            src.user_agent
        );
        assert!(
            src.user_agent.contains(env!("CARGO_PKG_VERSION")),
            "UA missing crate version: {}",
            src.user_agent
        );
        assert!(
            src.user_agent
                .ends_with(" (+github.com/wantaekchoi/kbotop; personal use)"),
            "unexpected UA suffix: {}",
            src.user_agent
        );
    }

    /// 테스트 전용 최소 HTTP 응답기. `TcpListener::bind("127.0.0.1:0")`로 OS가
    /// 할당한 포트에서 요청을 정확히 1개만 받아 고정 상태줄+`Content-Length`+
    /// 본문을 돌려주고 스레드를 끝낸다. accept 대기·읽기에 각각 상한(5s)을 둬
    /// 클라이언트가 끝내 연결하지 않는 버그가 나도 테스트 프로세스가 무기한
    /// 블록되지 않는다. 새 크레이트(mockito 등) 없이 std만으로 구현.
    struct LocalServer {
        addr: std::net::SocketAddr,
        request_rx: std::sync::mpsc::Receiver<Vec<u8>>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl LocalServer {
        fn spawn(status_line: impl Into<String>, body: impl Into<String>) -> Self {
            let status_line = status_line.into();
            let body = body.into();
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
            listener
                .set_nonblocking(true)
                .expect("set listener nonblocking");
            let addr = listener.local_addr().expect("read local_addr");
            let (tx, rx) = std::sync::mpsc::channel();
            let handle = std::thread::spawn(move || {
                use std::io::{Read, Write};
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                let mut accepted = None;
                while std::time::Instant::now() < deadline {
                    match listener.accept() {
                        Ok((s, _)) => {
                            accepted = Some(s);
                            break;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
                let Some(mut stream) = accepted else {
                    return;
                };
                // 리스너의 nonblocking 플래그가 accept된 소켓에 그대로 상속되는
                // 플랫폼(macOS 등)이 있다 — 그대로 두면 큰 본문(relay fixture,
                // 200KB+)을 쓸 때 `write_all`이 `WouldBlock`을 재시도 없이
                // 그대로 에러로 반환해 응답이 잘린 채 전송되는 flaky 실패가
                // 난다. 요청 1개를 다루는 이 연결은 명시적으로 블로킹으로
                // 되돌려 일반적인 blocking read/write를 보장한다.
                stream.set_nonblocking(false).ok();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .ok();
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = tx.send(buf);
                let response = format!(
                    "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            });
            LocalServer {
                addr,
                request_rx: rx,
                handle: Some(handle),
            }
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }

        /// 캡처된 요청 헤더(첫 줄 포함)를 문자열로 돌려준다. 5초 안에 요청이
        /// 오지 않으면 `None` — 여기서도 무기한 블록되지 않는다.
        fn recv_request(&self) -> Option<String> {
            self.request_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        }
    }

    impl Drop for LocalServer {
        fn drop(&mut self) {
            // 스레드는 요청 1개 처리 후 스스로 끝나므로 보통 즉시 join된다.
            // accept 타임아웃(5s) 경로로 빠졌더라도 여기서 마저 기다려 좀비
            // 스레드를 남기지 않는다.
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn dummy_game() -> Game {
        Game {
            id: "20260719KTLG02026".into(),
            start: String::new(),
            status: GameStatus::Live,
            status_label: String::new(),
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
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
        }
    }

    #[test]
    fn games_parses_a_200_response_with_real_fixture_body() {
        const SCHEDULE: &str = include_str!("../../../tests/fixtures/schedule_20260719.json");
        let server = LocalServer::spawn("HTTP/1.1 200 OK", SCHEDULE);
        let src = NaverSource::with_base(server.base_url());
        let games = src.games("2026-07-19").unwrap();
        assert!(!games.is_empty());
    }

    /// v0.23: 선발투수·구장·중계 채널은 `fields`로 요청해야 온다. 이 파라미터가
    /// 빠지면 응답에 그 필드가 아예 없어 화면이 조용히 비므로, 요청 자체를 고정한다.
    #[test]
    fn games_requests_the_extra_fields_needed_for_starters_and_venue() {
        const SCHEDULE: &str = include_str!("../../../tests/fixtures/schedule_20260719.json");
        let server = LocalServer::spawn("HTTP/1.1 200 OK", SCHEDULE);
        let src = NaverSource::with_base(server.base_url());
        let _ = src.games("2026-07-19");

        let req = server.recv_request().expect("no request captured");
        let line = req.lines().next().unwrap_or_default();
        for field in [
            "homeStarterName",
            "awayStarterName",
            "stadium",
            "broadChannel",
        ] {
            assert!(line.contains(field), "{field} not requested: {line}");
        }
        assert!(line.contains("basic"), "basic fields must stay: {line}");
    }

    #[test]
    fn standings_parses_a_200_response_with_real_fixture_body() {
        const STANDINGS: &str = include_str!("../../../tests/fixtures/standings_2026.json");
        let server = LocalServer::spawn("HTTP/1.1 200 OK", STANDINGS);
        let src = NaverSource::with_base(server.base_url());
        let standings = src.standings(2026).unwrap();
        assert!(!standings.is_empty());
    }

    #[test]
    fn live_parses_a_200_response_with_real_fixture_body() {
        const RELAY: &str = include_str!("../../../tests/fixtures/relay_20260719KTLG.json");
        let server = LocalServer::spawn("HTTP/1.1 200 OK", RELAY);
        let src = NaverSource::with_base(server.base_url());
        let live = src.live(&dummy_game()).unwrap();
        assert!(!live.current_pitches.is_empty());
    }

    /// 과거 이닝 경로는 요청 URL에 `?inning=N`을 실어야 하고(이게 없으면 서버가
    /// 마지막 이닝을 주므로 되감기가 제자리를 맴돈다), 응답에서 그 이닝의 타석을
    /// 뽑아야 한다. 실응답 fixture(`?inning=3`)로 양쪽을 함께 본다.
    #[test]
    fn at_bats_of_inning_requests_that_inning_and_parses_its_at_bats() {
        const INN3: &str = include_str!("../../../tests/fixtures/relay_20260726LGHH_inn3.json");
        let server = LocalServer::spawn("HTTP/1.1 200 OK", INN3);
        let src = NaverSource::with_base(server.base_url());
        let at_bats = src.at_bats_of_inning(&dummy_game(), 3).unwrap();

        let req = server.recv_request().expect("no request captured");
        let first_line = req.lines().next().unwrap_or_default();
        assert!(
            first_line.contains("/relay?inning=3"),
            "inning was not asked for: {first_line}"
        );
        assert_eq!(at_bats.len(), 8);
    }

    #[test]
    fn tips_parses_a_200_response_body_into_lines() {
        let body: String = (1..=12)
            .map(|i| format!("tip {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let server = LocalServer::spawn("HTTP/1.1 200 OK", body);
        let src = NaverSource::with_base(server.base_url());
        let tips = src.tips().unwrap();
        assert_eq!(tips.len(), 12);
    }

    #[test]
    fn games_non_200_status_maps_to_err_not_panic() {
        let server = LocalServer::spawn("HTTP/1.1 404 Not Found", "not found");
        let src = NaverSource::with_base(server.base_url());
        assert!(src.games("2026-07-19").is_err());
    }

    #[test]
    fn standings_non_200_status_maps_to_err_not_panic() {
        let server = LocalServer::spawn("HTTP/1.1 500 Internal Server Error", "boom");
        let src = NaverSource::with_base(server.base_url());
        assert!(src.standings(2026).is_err());
    }

    #[test]
    fn live_non_200_status_maps_to_err_not_panic() {
        let server = LocalServer::spawn("HTTP/1.1 404 Not Found", "not found");
        let src = NaverSource::with_base(server.base_url());
        assert!(src.live(&dummy_game()).is_err());
    }

    #[test]
    fn tips_non_200_status_maps_to_err_not_panic() {
        let server = LocalServer::spawn("HTTP/1.1 500 Internal Server Error", "boom");
        let src = NaverSource::with_base(server.base_url());
        assert!(src.tips().is_err());
    }

    #[test]
    fn games_truncated_json_body_maps_to_err_not_panic() {
        // 200 응답이지만 본문이 중간에 잘려 있다 — 관용 파싱이 패닉 없이
        // Err로 떨어지는지 확인한다.
        let server = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            r#"{"result":{"games":[{"gameId":"g1","homeTeamCode":"LG""#,
        );
        let src = NaverSource::with_base(server.base_url());
        assert!(src.games("2026-07-19").is_err());
    }

    #[test]
    fn standings_truncated_json_body_maps_to_err_not_panic() {
        let server = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            r#"{"result":{"seasonTeamStats":[{"teamId":"WO""#,
        );
        let src = NaverSource::with_base(server.base_url());
        assert!(src.standings(2026).is_err());
    }

    #[test]
    fn user_agent_header_is_actually_sent_over_the_wire() {
        let server = LocalServer::spawn("HTTP/1.1 200 OK", "{}");
        let src = NaverSource::with_base(server.base_url());
        // 파싱 성공 여부는 무관 — 요청이 실제로 나갔는지, 그 안의 UA 헤더가
        // `new()`가 세팅하는 값과 일치하는지만 본다(기존
        // `user_agent_matches_required_format`은 문자열 포맷만 보고 실제
        // 전송은 보지 않았다).
        let _ = src.games("2026-07-19");
        let request = server
            .recv_request()
            .expect("local server never received a request");
        let expected_header = format!("user-agent: {}", src.user_agent).to_lowercase();
        assert!(
            request.to_lowercase().contains(&expected_header),
            "request head missing expected User-Agent header.\nrequest head:\n{request}"
        );
    }
}
