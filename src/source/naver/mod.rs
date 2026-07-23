pub mod dto;
pub mod map;

use crate::error::Result;
use crate::model::{Game, LiveState, NewsItem, Standing};
use crate::source::DataSource;

const BASE: &str = "https://api-gw.sports.naver.com";

pub struct NaverSource {
    agent: ureq::Agent,
    user_agent: String,
}

impl NaverSource {
    pub fn new() -> Self {
        NaverSource {
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(10))
                .build(),
            user_agent: format!(
                "kbotop/{} (+github.com/wantaekchoi/kbotop; personal use)",
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

impl Default for NaverSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSource for NaverSource {
    fn games(&self, date: &str) -> Result<Vec<Game>> {
        let url = format!(
            "{BASE}/schedule/games?upperCategoryId=kbaseball&categoryId=kbo&fromDate={date}&toDate={date}"
        );
        map::games_from_schedule(&self.get(&url)?)
    }

    fn live(&self, game: &Game) -> Result<LiveState> {
        let url = format!("{BASE}/schedule/games/{}/relay", game.id);
        map::live_from_relay(&self.get(&url)?, game.home.clone(), game.away.clone())
    }

    fn standings(&self, year: u16) -> Result<Vec<Standing>> {
        let url = format!("{BASE}/statistics/categories/kbo/seasons/{year}/teams");
        map::standings_from_json(&self.get(&url)?)
    }

    fn news(&self) -> Result<Vec<NewsItem>> {
        let url = format!("{BASE}/news/articles/kbo?size=20");
        map::news_from_json(&self.get(&url)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
