pub mod naver;

use crate::error::Result;
use crate::model::{Game, LiveState, Standing};

pub trait DataSource: Send + Sync {
    fn games(&self, date: &str) -> Result<Vec<Game>>;
    fn live(&self, game: &Game) -> Result<LiveState>;
    fn standings(&self, year: u16) -> Result<Vec<Standing>>;
}
