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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GameStatus, Team};

    /// 기본 구현만 가진 소스. 트레이트가 제공하는 기본 동작의 계약을 여기서 고정한다 —
    /// v0.21까지 이 파일이 42.86%로 전체 최저였던 건 기본 구현이 **한 번도 실행되지
    /// 않아서**다. 누가 기본값을 바꿔도 걸리는 테스트가 없었다.
    struct BareSource;

    impl DataSource for BareSource {
        fn games(&self, _date: &str) -> Result<Vec<Game>> {
            Ok(vec![])
        }
        fn live(&self, _game: &Game) -> Result<LiveState> {
            Err(crate::error::Error::Data("not implemented".into()))
        }
        fn standings(&self, _year: u16) -> Result<Vec<Standing>> {
            Ok(vec![])
        }
    }

    fn dummy_game() -> Game {
        Game {
            id: "g".into(),
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
        }
    }

    /// 과거 이닝을 제공하지 않는 소스는 **빈 목록**을 돌려준다(에러가 아니다).
    /// 호출부(App)는 이걸 "그 이닝은 없다"로 캐시해 되감기 경계에 조용히 멈춘다 —
    /// 에러를 던지면 footer에 붉은 오류가 뜨고 사용자는 고칠 수 없는 실패를 본다.
    #[test]
    fn a_source_without_inning_support_returns_no_at_bats_rather_than_failing() {
        let at_bats = BareSource.at_bats_of_inning(&dummy_game(), 3).unwrap();
        assert!(at_bats.is_empty());
    }

    /// 팁을 제공하지 않는 소스도 빈 목록이다 — 임베드 팁 폴백이 그대로 쓰인다.
    #[test]
    fn a_source_without_tips_returns_an_empty_list() {
        assert!(BareSource.tips().unwrap().is_empty());
    }

    /// 기본 구현은 인자를 쓰지 않지만, 어떤 이닝을 물어도 같은 계약을 지킨다
    /// (경계값에서 패닉하지 않는다 — 무패닉 원칙).
    #[test]
    fn the_default_inning_implementation_is_total_over_its_input() {
        for inning in [0u8, 1, 9, 12, u8::MAX] {
            assert!(BareSource
                .at_bats_of_inning(&dummy_game(), inning)
                .unwrap()
                .is_empty());
        }
    }
}
