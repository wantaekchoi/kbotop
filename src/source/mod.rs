pub mod naver;
pub mod rss;
pub(crate) mod text;

use crate::error::Result;
use crate::model::{Game, LiveState, Standing};

pub trait DataSource: Send + Sync {
    fn games(&self, date: &str) -> Result<Vec<Game>>;
    fn live(&self, game: &Game) -> Result<LiveState>;
    fn standings(&self, year: u16) -> Result<Vec<Standing>>;

    /// 하단 팁 목록의 런타임 갱신본(부가 기능). 기본은 빈 목록 — 임베드 폴백.
    fn tips(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}

/// 뉴스 전용 소스. 경기 데이터와 생명주기·제공자가 달라 DataSource에서 분리했다
/// (RSS 소스는 경기 데이터를 제공할 수 없다).
pub trait NewsSource: Send + Sync {
    fn news(&self) -> Result<Vec<crate::model::NewsItem>>;
}
