pub mod naver;

use crate::error::Result;
use crate::model::{Game, LiveState, Standing};

pub trait DataSource: Send + Sync {
    fn games(&self, date: &str) -> Result<Vec<Game>>;
    fn live(&self, game: &Game) -> Result<LiveState>;
    fn standings(&self, year: u16) -> Result<Vec<Standing>>;

    /// KBO 뉴스 헤드라인(부가 기능). 기본 구현은 빈 목록 — 뉴스 없는 소스도 유효.
    fn news(&self) -> Result<Vec<crate::model::NewsItem>> {
        Ok(vec![])
    }

    /// 하단 팁 목록의 런타임 갱신본(부가 기능). 기본은 빈 목록 — 임베드 폴백.
    fn tips(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
