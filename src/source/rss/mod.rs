//! 언론사 RSS 뉴스 소스(v0.7). 발행자가 배포 목적으로 내보낸 채널에서 직접
//! 받는다 — 사용자 기기에서 실행돼 그 사용자에게만 렌더하므로 공중송신이 없다.
//! 실패는 피드 단위로 격리한다(한 피드가 죽어도 나머지로 계속).
pub(crate) mod parse;

use crate::error::{Error, Result};
use crate::model::NewsItem;
use crate::source::NewsSource;
use std::collections::HashSet;

struct Feed {
    url: &'static str,
    label: &'static str,
    /// 비면 전체 유지. 아니면 <category>가 이 중 하나인 항목만.
    categories: &'static [&'static str],
}

/// 2026-07-24 실측으로 살아있음을 확인한 피드. 1차 축은 서버측에서 이미 야구로
/// 좁혀진 두 곳, 보강 둘은 category 필터로 야구만 남긴다.
const FEEDS: &[Feed] = &[
    Feed {
        url: "https://www.sportschosun.com/rss/index_bs.htm",
        label: "스포츠조선",
        categories: &[],
    },
    Feed {
        url: "https://www.spotvnews.co.kr/rss/S1N2.xml",
        label: "스포티비뉴스",
        categories: &[],
    },
    Feed {
        url: "https://isplus.com/rss",
        label: "일간스포츠",
        categories: &["프로야구", "메이저리그"],
    },
    Feed {
        url: "https://www.khan.co.kr/rss/rssdata/kh_sports.xml",
        label: "스포츠경향",
        categories: &["야구"],
    },
];

/// 피드 하나당 ureq 타임아웃(초). `news()`가 FEEDS를 순차 호출하므로 폴러가
/// 한 번의 뉴스 폴에서 블로킹될 수 있는 최악 시간은 대략
/// `FEEDS.len() * FEED_TIMEOUT_SECS`다(현재 4피드 × 5초 = ~20초). 폴러는
/// 단일 스레드로 games/tips/news/live를 순차 처리하고 명령 드레인
/// (`rx.try_recv()`)도 루프 최상단에서만 하므로, 이 시간 동안 라이브 갱신이
/// 지연된다. `q` 종료는 이 영향을 받지 않는다 — main.rs가 `Shutdown` 전송 후
/// 폴러 스레드의 `join`을 기다리지 않고 곧장 드롭한다. 값을 올릴 때는 라이브
/// 갱신 지연 창도 함께 늘어난다는 점을 염두에 둘 것.
const FEED_TIMEOUT_SECS: u64 = 5;

pub struct RssSource {
    agent: ureq::Agent,
    user_agent: String,
}

impl RssSource {
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(FEED_TIMEOUT_SECS))
                .build(),
            user_agent: format!(
                "kbotop/{} (+https://github.com/wantaekchoi/kbotop; personal use)",
                env!("CARGO_PKG_VERSION")
            ),
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
        let mut per_feed = Vec::new();
        for f in FEEDS {
            // 피드 단위 실패 격리 — 한 곳이 죽어도(예: 매경 Cloudflare 챌린지)
            // 나머지로 계속한다.
            if let Ok(items) = self
                .get(f.url)
                .and_then(|xml| parse::feed_from_xml(&xml, f.label, f.categories))
            {
                per_feed.push(items);
            }
        }
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
}
