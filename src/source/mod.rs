pub mod naver;
pub mod rss;
pub(crate) mod text;

use crate::error::Result;
use crate::model::{AtBat, Game, LiveState, Standing};

pub trait DataSource: Send + Sync {
    fn games(&self, date: &str) -> Result<Vec<Game>>;
    fn live(&self, game: &Game) -> Result<LiveState>;
    fn standings(&self, year: u16) -> Result<Vec<Standing>>;

    /// 지난 이닝의 타석들(v0.20 "과거 이닝 돌려보기"). `live()`가 주는 건 마지막
    /// 이닝뿐이라, 그 앞을 보려면 이닝을 지정해 따로 받아야 한다.
    ///
    /// 기본 구현은 빈 목록 — 이 기능이 없는 소스에서는 "그 이닝은 없다"로 조용히
    /// 저하되고, 호출부는 되감기 경계에 그대로 멈춘다.
    fn at_bats_of_inning(&self, game: &Game, inning: u8) -> Result<Vec<AtBat>> {
        let _ = (game, inning);
        Ok(vec![])
    }

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
