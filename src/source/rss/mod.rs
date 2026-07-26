//! 언론사 RSS 뉴스 소스(v0.7). 발행자가 배포 목적으로 내보낸 채널에서 직접
//! 받는다 — 사용자 기기에서 실행돼 그 사용자에게만 렌더하므로 공중송신이 없다.
//! 실패는 피드 단위로 격리한다(한 피드가 죽어도 나머지로 계속).
pub(crate) mod parse;

use crate::error::{Error, Result};
use crate::model::NewsItem;
use crate::source::NewsSource;
use std::borrow::Cow;
use std::collections::HashSet;

struct Feed {
    /// `Cow`인 이유: 기본 피드는 `&'static str` 리터럴이지만, 테스트에서는
    /// `with_feeds()`로 런타임에 바인딩된 로컬 서버 주소(owned `String`)를
    /// 꽂아야 한다 — 새 타입/크레이트 없이 std `Cow`로 두 경우를 한 필드에서
    /// 수용한다.
    url: Cow<'static, str>,
    label: &'static str,
    /// 비면 전체 유지. 아니면 <category>가 이 중 하나인 항목만.
    categories: &'static [&'static str],
}

/// 2026-07-24 실측으로 살아있음을 확인한 기본 피드. 1차 축은 서버측에서 이미
/// 야구로 좁혀진 두 곳, 보강 둘은 category 필터로 야구만 남긴다.
fn default_feeds() -> Vec<Feed> {
    vec![
        Feed {
            url: Cow::Borrowed("https://www.sportschosun.com/rss/index_bs.htm"),
            label: "스포츠조선",
            categories: &[],
        },
        Feed {
            url: Cow::Borrowed("https://www.spotvnews.co.kr/rss/S1N2.xml"),
            label: "스포티비뉴스",
            categories: &[],
        },
        Feed {
            url: Cow::Borrowed("https://isplus.com/rss"),
            label: "일간스포츠",
            categories: &["프로야구", "메이저리그"],
        },
        Feed {
            url: Cow::Borrowed("https://www.khan.co.kr/rss/rssdata/kh_sports.xml"),
            label: "스포츠경향",
            categories: &["야구"],
        },
    ]
}

/// 피드 하나당 ureq 타임아웃(초). `news()`가 FEEDS를 스레드로 동시 호출하므로
/// 폴러가 한 번의 뉴스 폴에서 블로킹될 수 있는 최악 시간은 대략
/// `FEED_TIMEOUT_SECS`다(가장 느린 피드 하나의 타임아웃, 현재 ~5초 —
/// 순차였던 이전에는 `FEEDS.len() * FEED_TIMEOUT_SECS` ≈ 20초였다). 폴러는
/// 단일 스레드로 games/tips/news/live를 순차 처리하고 명령 드레인
/// (`rx.try_recv()`)도 루프 최상단에서만 하므로, 이 시간 동안 라이브 갱신이
/// 지연된다. `q` 종료는 이 영향을 받지 않는다 — main.rs가 `Shutdown` 전송 후
/// 폴러 스레드의 `join`을 기다리지 않고 곧장 드롭한다. 값을 올릴 때는 라이브
/// 갱신 지연 창도 함께 늘어난다는 점을 염두에 둘 것.
const FEED_TIMEOUT_SECS: u64 = 5;

pub struct RssSource {
    agent: ureq::Agent,
    user_agent: String,
    feeds: Vec<Feed>,
}

impl RssSource {
    pub fn new() -> Self {
        Self::build(default_feeds())
    }

    /// 테스트 전용 생성자: 기본 피드 목록 대신 주어진 목록을 쓴다(피드 URL을
    /// 로컬 mock 서버로 돌려 실네트워크 없이 병렬 fetch·피드별 실패 격리·전체
    /// 실패 Err 경로를 검증하기 위함). `new()`의 기본 동작·기본 피드 목록·
    /// 공개 시그니처는 그대로다.
    #[cfg(test)]
    fn with_feeds(feeds: Vec<Feed>) -> Self {
        Self::build(feeds)
    }

    fn build(feeds: Vec<Feed>) -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(FEED_TIMEOUT_SECS))
                .build(),
            user_agent: format!(
                "kbotop/{} (+https://github.com/wantaekchoi/kbotop; personal use)",
                env!("CARGO_PKG_VERSION")
            ),
            feeds,
        }
    }
}

impl Default for RssSource {
    fn default() -> Self {
        Self::new()
    }
}

/// dedup 키용 URL 정규화. 쿼리를 통째로 버리면 스포티비뉴스처럼 기사 ID가
/// 쿼리에만 있는 URL(`…/articleView.html?idxno=830362`)이 전부 하나로
/// 뭉개진다(2026-07-24 실측: 100건 → 1건). 그렇다고 쿼리를 통째로 남기면
/// 스포츠경향처럼 매 항목에 붙는 `utm_*` 트래킹 파라미터 때문에 같은 기사가
/// 다른 키로 갈라진다. 절충: `utm_`로 시작하는 파라미터만 걸러내고 나머지
/// 쿼리(기사 ID 포함)는 그대로 키에 남긴다. 현재 4개 피드는 서로 다른
/// 도메인이라 피드 간 URL 충돌은 없다.
fn dedup_url_key(url: &str) -> String {
    let Some((path, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|kv| !kv.starts_with("utm_"))
        .collect();
    if kept.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", kept.join("&"))
    }
}

/// 피드별 결과를 합치고 URL(utm 트래킹 파라미터만 제거) 기준으로 중복을
/// 제거한 뒤 **최신순으로 정렬**한다. 정렬이 없으면 매체별로 뭉쳐 나와
/// (스포츠조선 100건 → SPOTV 100건) "최신 뉴스 목록"으로 읽히지 않는다.
/// `published`가 빈 항목은 뒤로 밀린다. URL이 빈 항목은 제목을 dedup 키로
/// 쓴다.
fn merge_feeds(per_feed: Vec<Vec<NewsItem>>) -> Vec<NewsItem> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for items in per_feed {
        for it in items {
            let key = if it.url.is_empty() {
                format!("t:{}", it.title)
            } else {
                format!("u:{}", dedup_url_key(&it.url))
            };
            if seen.insert(key) {
                out.push(it);
            }
        }
    }
    // 문자열 "YYYYMMDDHHMMSS"는 사전순 == 시간순이라 그대로 역정렬하면 최신순이다.
    // 빈 문자열은 가장 작아 자동으로 뒤로 간다.
    out.sort_by(|a, b| b.published.cmp(&a.published));
    out
}

/// 성공한 피드들의 결과를 받아 최종 `news()` 반환값을 판정하는 순수 함수.
/// 실패한 피드는 애초에 `per_feed`에 들어오지 않는다(호출부가 걸러낸다) — 즉
/// `per_feed.is_empty()`는 "성공한 피드가 0개"를 뜻한다.
///
/// - 하나라도 성공했으면(살아있는 피드가 0건을 준 경우 포함) `Ok(병합 결과)`.
///   살아있는 피드의 빈 결과는 실패가 아니라 "야구 기사가 없는 정직한 상태"다.
/// - 전부 실패했으면 `Err` — 폴러의 `if let Ok(n) = ...`가 이를 걸러 이전
///   뉴스를 화면에 그대로 남긴다(네이버 소스 시절 동작과 동일한 회귀 방지).
fn finish(per_feed: Vec<Vec<NewsItem>>) -> Result<Vec<NewsItem>> {
    if per_feed.is_empty() {
        return Err(Error::Data("all news feeds failed".into()));
    }
    Ok(merge_feeds(per_feed))
}

impl NewsSource for RssSource {
    fn news(&self) -> Result<Vec<NewsItem>> {
        // 피드를 동시에 받는다 — 순차면 최악 4×타임아웃(~20초)이 폴러를 막는다.
        // 각 스레드는 fetch+parse를 독립 수행하고, 실패한 피드는 결과에서 빠진다
        // (피드별 실패 격리). ureq::Agent는 Clone이 값싸다(Arc 내부).
        let handles: Vec<_> = self
            .feeds
            .iter()
            .map(|f| {
                let agent = self.agent.clone();
                let ua = self.user_agent.clone();
                let url = f.url.clone();
                let label = f.label;
                let cats = f.categories;
                std::thread::spawn(move || {
                    let body = agent
                        .get(&url)
                        .set("User-Agent", &ua)
                        .call()
                        .map_err(Box::new)
                        .ok()?
                        .into_string()
                        .ok()?;
                    parse::feed_from_xml(&body, label, cats).ok()
                })
            })
            .collect();
        let per_feed: Vec<Vec<NewsItem>> = handles
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect();
        finish(per_feed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, url: &str, published: &str) -> NewsItem {
        NewsItem {
            title: title.into(),
            source: "s".into(),
            url: url.into(),
            summary: String::new(),
            published: published.into(),
        }
    }

    /// 피드별 결과를 합치되 URL 정규화 후 중복을 제거한다(경향은 utm 파라미터가 붙는다).
    #[test]
    fn merge_feeds_dedups_by_normalized_url() {
        let merged = merge_feeds(vec![
            vec![item(
                "a",
                "https://x.kr/1?utm_source=khan_rss&utm_medium=rss",
                "20260724090000",
            )],
            vec![
                item("a", "https://x.kr/1", "20260724090000"),
                item("b", "https://x.kr/2", "20260724080000"),
            ],
        ]);
        assert_eq!(merged.len(), 2, "같은 기사는 한 번만: {merged:?}");
        assert!(merged.iter().any(|i| i.title == "b"));
    }

    /// 회귀 재현: 스포티비뉴스는 기사 ID가 쿼리에만 있다
    /// (`…/articleView.html?idxno=830362`). 쿼리를 통째로 버리면 서로 다른
    /// 기사가 전부 같은 키로 뭉개진다 — idxno만 다른 두 건이 2건으로 남아야
    /// 한다.
    #[test]
    fn merge_feeds_keeps_spotv_query_only_article_ids_distinct() {
        let merged = merge_feeds(vec![vec![
            item(
                "스포티비-1",
                "https://www.spotvnews.co.kr/news/articleView.html?idxno=830362",
                "20260724090000",
            ),
            item(
                "스포티비-2",
                "https://www.spotvnews.co.kr/news/articleView.html?idxno=830363",
                "20260724080000",
            ),
        ]]);
        assert_eq!(
            merged.len(),
            2,
            "쿼리로만 구분되는 기사가 dedup에 사라지면 안 된다: {merged:?}"
        );
    }

    /// 스포츠경향은 링크에 `?utm_source=khan_rss&utm_medium=rss…`가 자동으로
    /// 붙는다. 이 트래킹 파라미터만 다르고 나머지가 같으면 여전히 같은
    /// 기사로 묶여야 한다.
    #[test]
    fn merge_feeds_dedups_khan_utm_tracking_params() {
        let merged = merge_feeds(vec![
            vec![item(
                "경향",
                "https://www.khan.co.kr/article/202607241234?utm_source=khan_rss&utm_medium=rss",
                "20260724090000",
            )],
            vec![item(
                "경향",
                "https://www.khan.co.kr/article/202607241234",
                "20260724090000",
            )],
        ]);
        assert_eq!(
            merged.len(),
            1,
            "utm 파라미터만 다른 건 같은 기사: {merged:?}"
        );
    }

    /// URL이 비면 dedup 키로 쓸 수 없으므로 제목으로 구분한다(항목은 살린다).
    #[test]
    fn merge_feeds_keeps_items_without_url() {
        let merged = merge_feeds(vec![vec![item("a", "", ""), item("b", "", "")]]);
        assert_eq!(merged.len(), 2);
    }

    /// 매체별로 뭉치지 않고 최신순으로 섞인다 — 목록 브라우징의 핵심 요구.
    #[test]
    fn merge_feeds_sorts_newest_first_across_feeds() {
        let merged = merge_feeds(vec![
            vec![
                item("조선-오래된", "https://a.kr/1", "20260724080000"),
                item("조선-최신", "https://a.kr/2", "20260724100000"),
            ],
            vec![item("SPOTV-중간", "https://b.kr/1", "20260724090000")],
        ]);
        let titles: Vec<&str> = merged.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["조선-최신", "SPOTV-중간", "조선-오래된"]);
    }

    /// 날짜를 못 읽은 항목은 뒤로 밀리되 사라지지는 않는다.
    #[test]
    fn merge_feeds_puts_undated_items_last_without_dropping() {
        let merged = merge_feeds(vec![vec![
            item("날짜없음", "https://a.kr/1", ""),
            item("날짜있음", "https://a.kr/2", "20260724100000"),
        ]]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].title, "날짜있음");
        assert_eq!(merged[1].title, "날짜없음");
    }

    /// 모든 피드가 실패(성공 피드 0개)하면 Err — 폴러가 걸러 이전 뉴스를
    /// 화면에 그대로 남긴다. `Ok(vec![])`로 되면 app.apply()가 news를 빈
    /// 벡터로 무조건 교체해 티커/목록이 지워지는 회귀가 재발한다.
    #[test]
    fn finish_errs_when_every_feed_failed() {
        let result = finish(vec![]);
        assert!(
            result.is_err(),
            "all-feeds-failed must not be Ok: {result:?}"
        );
    }

    /// 일부 피드만 성공했으면 Ok이고 성공한 피드들의 항목이 살아 있다(부분
    /// 실패 격리가 finish 이후에도 유지되는지 확인).
    #[test]
    fn finish_oks_partial_success_with_succeeded_items() {
        let result = finish(vec![vec![item("a", "https://a.kr/1", "20260724100000")]]);
        let items = result.expect("partial success must be Ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "a");
    }

    /// 살아있는 피드가 항목 0개를 준 경우는 실패가 아니다 — 피드는 멀쩡한데
    /// 야구 기사가 없는 정직한 상태이므로 Ok(빈 벡터)여야 한다.
    #[test]
    fn finish_oks_empty_vec_when_live_feed_returned_zero_items() {
        let result = finish(vec![vec![]]);
        let items = result.expect("a live feed with zero items must still be Ok");
        assert!(items.is_empty());
    }

    // ---- 이하는 `news()`의 실제 스레드+HTTP+파싱 경로를 로컬 서버로 검증한다.
    // 패턴은 v0.13 `source/naver/mod.rs`의 `LocalServer`와 동일: 포트는
    // `bind("127.0.0.1:0")`으로 OS가 할당(병렬 테스트 충돌 방지), accept
    // 직후 `set_nonblocking(false)`로 macOS에서 accept된 소켓이 리스너의
    // 논블로킹 플래그를 상속해 큰 응답이 잘리는 flake를 막는다. 새 크레이트
    // 없이 std만 사용.

    /// 테스트 전용 최소 HTTP 응답기. 요청을 최대 `max_requests`개 받아 각각
    /// 고정 상태줄+본문을 순서대로 돌려주고 스레드를 끝낸다. accept·읽기에
    /// 상한(5s)을 둬 클라이언트가 끝내 붙지 않아도 테스트가 무기한 블록되지
    /// 않는다.
    struct LocalServer {
        addr: std::net::SocketAddr,
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
                // macOS 등에서 accept된 소켓이 리스너의 nonblocking 플래그를
                // 그대로 물려받는다 — 그대로 두면 write_all이 WouldBlock을
                // 재시도 없이 에러로 반환해 응답이 잘려 전송되는 flaky 실패가
                // 난다(v0.13에서 관측·수정). 이 연결은 명시적으로 블로킹으로
                // 되돌린다.
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
                let response = format!(
                    "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            });
            LocalServer {
                addr,
                handle: Some(handle),
            }
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
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

    /// 테스트용 피드 하나. `with_feeds()`로 `RssSource`에 꽂아 실네트워크
    /// 없이 `news()`의 스레드+HTTP+파싱 전 경로를 태운다.
    fn feed(url: String, label: &'static str) -> Feed {
        Feed {
            url: Cow::Owned(url),
            label,
            categories: &[],
        }
    }

    fn valid_feed_xml(title: &str, link: &str, pub_date: &str) -> String {
        format!(
            r#"<?xml version="1.0"?><rss><channel><item><title>{title}</title><link>{link}</link><pubDate>{pub_date}</pubDate><description>요약</description></item></channel></rss>"#
        )
    }

    /// 피드 하나가 500을 반환해도 나머지 정상 피드의 결과는 살아남는다(피드별
    /// 실패 격리 — v0.7 설계의 핵심).
    #[test]
    fn news_isolates_a_failed_feed_and_keeps_the_succeeding_one() {
        let bad = LocalServer::spawn("HTTP/1.1 500 Internal Server Error", "boom");
        let good = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            valid_feed_xml(
                "정상 기사",
                "https://good.example/1",
                "Fri, 24 Jul 2026 11:00:00 +0900",
            ),
        );
        let src = RssSource::with_feeds(vec![
            feed(bad.base_url(), "bad"),
            feed(good.base_url(), "good"),
        ]);
        let items = src.news().expect("살아있는 피드가 있으면 Ok");
        assert_eq!(
            items.len(),
            1,
            "실패한 피드의 항목은 섞이면 안 된다: {items:?}"
        );
        assert_eq!(items[0].source, "good");
    }

    /// 전체 피드가 실패하면 `Err` — 폴러가 이를 걸러 이전 뉴스를 화면에 그대로
    /// 남긴다(v0.7 회귀 방지 장치). `Ok(vec![])`가 나오면 실 서비스에서
    /// 일시적 전체 장애 때 목록이 비어 화면의 기존 뉴스가 지워진다.
    #[test]
    fn news_errs_when_every_feed_fails_over_the_wire() {
        let bad1 = LocalServer::spawn("HTTP/1.1 500 Internal Server Error", "boom");
        let bad2 = LocalServer::spawn("HTTP/1.1 404 Not Found", "nope");
        let src =
            RssSource::with_feeds(vec![feed(bad1.base_url(), "a"), feed(bad2.base_url(), "b")]);
        let result = src.news();
        assert!(
            result.is_err(),
            "전체 실패는 Err여야 한다(Ok(빈 목록)이면 기존 뉴스가 지워짐): {result:?}"
        );
    }

    /// 깨진 XML(잘린 태그)을 주는 피드는 그 피드만 실패하고 나머지 정상 피드는
    /// 살아남는다. 패닉이 없어야 한다.
    #[test]
    fn news_isolates_a_feed_with_malformed_xml() {
        let broken = LocalServer::spawn("HTTP/1.1 200 OK", "<rss><channel><item>");
        let good = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            valid_feed_xml(
                "정상 기사",
                "https://good.example/1",
                "Fri, 24 Jul 2026 11:00:00 +0900",
            ),
        );
        let src = RssSource::with_feeds(vec![
            feed(broken.base_url(), "broken"),
            feed(good.base_url(), "good"),
        ]);
        let items = src
            .news()
            .expect("깨진 XML 피드가 있어도 정상 피드는 살아남는다");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "good");
    }

    /// 200이지만 본문이 완전히 빈 피드는 XML 파싱 실패로 격리되고, 나머지
    /// 정상 피드 결과는 그대로 살아남는다.
    #[test]
    fn news_isolates_a_feed_with_empty_body() {
        let empty = LocalServer::spawn("HTTP/1.1 200 OK", "");
        let good = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            valid_feed_xml(
                "정상 기사",
                "https://good.example/1",
                "Fri, 24 Jul 2026 11:00:00 +0900",
            ),
        );
        let src = RssSource::with_feeds(vec![
            feed(empty.base_url(), "empty"),
            feed(good.base_url(), "good"),
        ]);
        let items = src
            .news()
            .expect("빈 본문 피드가 있어도 정상 피드는 살아남는다");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "good");
    }

    /// Content-Type이 XML과 무관해도(예: 바이너리로 흔히 쓰는
    /// `application/octet-stream`) 본문이 유효한 RSS면 그대로 파싱된다 —
    /// 프로덕션 코드가 Content-Type을 검사하지 않는다는 걸 명시적으로 고정.
    #[test]
    fn news_parses_body_regardless_of_content_type_header() {
        let body = valid_feed_xml(
            "기사",
            "https://x.example/1",
            "Fri, 24 Jul 2026 11:00:00 +0900",
        );
        let server = LocalServer::spawn(
            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream",
            body,
        );
        let src = RssSource::with_feeds(vec![feed(server.base_url(), "x")]);
        let items = src.news().expect("Content-Type과 무관하게 파싱되어야 한다");
        assert_eq!(items.len(), 1);
    }

    /// `news()` 전체 경로(스레드 fetch + 파싱 + merge_feeds의 dedup·정렬)가
    /// 실제 HTTP 라운드트립을 거쳐도 올바르게 동작한다. 두 피드가 같은
    /// 기사를 겹쳐 주고, 시각 순서도 뒤섞여 있다.
    #[test]
    fn news_dedups_and_sorts_across_feeds_over_the_wire() {
        let feed_a = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            r#"<?xml version="1.0"?><rss><channel>
                <item><title>오래된 기사</title><link>https://x.example/old</link><pubDate>Fri, 24 Jul 2026 08:00:00 +0900</pubDate><description>d</description></item>
                <item><title>공통 기사</title><link>https://x.example/shared</link><pubDate>Fri, 24 Jul 2026 09:00:00 +0900</pubDate><description>d</description></item>
                </channel></rss>"#,
        );
        let feed_b = LocalServer::spawn(
            "HTTP/1.1 200 OK",
            r#"<?xml version="1.0"?><rss><channel>
                <item><title>공통 기사</title><link>https://x.example/shared</link><pubDate>Fri, 24 Jul 2026 09:00:00 +0900</pubDate><description>d</description></item>
                <item><title>최신 기사</title><link>https://y.example/new</link><pubDate>Fri, 24 Jul 2026 10:00:00 +0900</pubDate><description>d</description></item>
                </channel></rss>"#,
        );
        let src = RssSource::with_feeds(vec![
            feed(feed_a.base_url(), "a"),
            feed(feed_b.base_url(), "b"),
        ]);
        let items = src.news().expect("두 피드 모두 정상이면 Ok");
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["최신 기사", "공통 기사", "오래된 기사"],
            "겹치는 기사는 한 번만 남고 최신순으로 정렬돼야 한다: {titles:?}"
        );
    }

    /// 기본 생성자 `new()`는 여전히 내장 4개 피드로 동작한다(공개 시그니처·
    /// 기본 피드 목록 호환 확인 — `with_feeds()` 도입이 기존 동작을 바꾸지
    /// 않았는지에 대한 안전망).
    #[test]
    fn new_still_uses_the_builtin_four_feeds() {
        let src = RssSource::new();
        assert_eq!(src.feeds.len(), 4);
        assert!(src.feeds.iter().any(|f| f.label == "스포츠조선"));
        assert!(src.feeds.iter().any(|f| f.label == "스포티비뉴스"));
        assert!(src.feeds.iter().any(|f| f.label == "일간스포츠"));
        assert!(src.feeds.iter().any(|f| f.label == "스포츠경향"));
    }
}
