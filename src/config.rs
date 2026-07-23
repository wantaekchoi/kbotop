use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub favorite_team: Option<String>,
    pub poll_secs: u64,
    pub lang: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            favorite_team: None,
            poll_secs: 5,
            lang: None,
        }
    }
}

impl Config {
    pub fn effective_poll_secs(&self) -> u64 {
        self.poll_secs.max(3)
    }
}

/// TOML 문자열 → Config. 역직렬화 실패(깨진 TOML, 타입 불일치 등)는 관용적으로
/// 기본값으로 폴백한다 — load()에서 분리해 파일 I/O 없이 이 분기를 직접 테스트할 수 있게 한다.
fn config_from_toml_str(s: &str) -> Config {
    toml::from_str(s).unwrap_or_default()
}

/// XDG 설정 경로에서 로드. 파일이 없거나 깨지면 기본값.
pub fn load() -> Config {
    let Some(dirs) = directories::ProjectDirs::from("", "", "kbotop") else {
        return Config::default();
    };
    let path = dirs.config_dir().join("config.toml");
    match std::fs::read_to_string(&path) {
        Ok(s) => config_from_toml_str(&s),
        Err(_) => Config::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_no_file() {
        let c = Config::default();
        assert_eq!(c.poll_secs, 5);
        assert!(c.favorite_team.is_none());
    }

    #[test]
    fn poll_secs_has_floor_of_3() {
        let c = Config {
            favorite_team: None,
            poll_secs: 1,
            lang: None,
        };
        assert_eq!(c.effective_poll_secs(), 3);
    }

    #[test]
    fn config_from_toml_str_falls_back_to_defaults_on_broken_toml() {
        // load()가 실제로 거치는 분기(toml::from_str(&s).unwrap_or_default())를
        // 파일 I/O 없이 직접 검증한다 — "깨진 TOML → panic 대신 기본값"은
        // 프로젝트 하드 제약(관용적 파싱, 패닉 금지)과 직결된다.
        let c = config_from_toml_str("not = [valid : toml");
        assert_eq!(c.poll_secs, 5);
        assert!(c.favorite_team.is_none());
    }

    #[test]
    fn config_from_toml_str_parses_actual_fields() {
        let c = config_from_toml_str("favorite_team = \"LG\"\npoll_secs = 7");
        assert_eq!(c.favorite_team.as_deref(), Some("LG"));
        assert_eq!(c.poll_secs, 7);
    }
}
