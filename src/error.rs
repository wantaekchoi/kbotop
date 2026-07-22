use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("network error: {0}")]
    Http(#[from] Box<ureq::Error>),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("config error: {0}")]
    Config(String),
    /// 설정 파일과 무관한 런타임/데이터 문제(응답 형태가 기대와 다름, 폴러
    /// 스레드 패닉 등). `Config`를 이런 경우에도 재사용하면 footer가 실제로는
    /// TOML 설정과 무관한 실패를 "config error: ..."로 잘못 표시한다.
    #[error("{0}")]
    Data(String),
}

pub type Result<T> = std::result::Result<T, Error>;
