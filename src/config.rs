use serde::Deserialize;
use std::io::Write;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub favorite_team: Option<String>,
    pub poll_secs: u64,
    pub lang: Option<String>,
    pub theme: ThemeConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            favorite_team: None,
            poll_secs: 5,
            lang: None,
            theme: ThemeConfig::default(),
        }
    }
}

/// 테마 설정: preset(색 사용 여부·정도) × accent(강조색 출처).
/// preset·accent는 T9에서 `ui::theme::accent_for`로 해석된다.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct ThemeConfig {
    pub preset: String,
    pub accent: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            preset: "default".into(),
            accent: "team".into(),
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

/// XDG 설정 파일 경로.
pub fn config_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "kbotop").map(|d| d.config_dir().join("config.toml"))
}

/// 기존 TOML 문자열에 Config의 알려진 키만 덮어써 재직렬화한다. 앱이 모르는
/// 키·섹션은 보존한다(사용자 수기 입력 방어). 주석은 보존하지 않는다
/// (toml::Table DOM 한계 — 의존성 최소주의상 toml_edit 미도입).
///
/// `existing`이 빈 문자열이면(파일 없음) 빈 테이블에서 시작해 항상 Ok다.
/// `existing`이 비어 있지 않은데 파싱에 실패하면(사용자 수기 오타 등) Err를
/// 반환한다 — 호출부가 빈 테이블로 폴백해 파일 전체를 덮어쓰는 것을 막기 위함.
fn merge_into_toml(existing: &str, cfg: &Config) -> Result<String, toml::de::Error> {
    let mut table: toml::Table = existing.parse()?;
    match &cfg.favorite_team {
        Some(t) => {
            table.insert("favorite_team".into(), toml::Value::String(t.clone()));
        }
        None => {
            table.remove("favorite_team");
        }
    }
    table.insert(
        "poll_secs".into(),
        toml::Value::Integer(cfg.poll_secs as i64),
    );
    match &cfg.lang {
        Some(l) => {
            table.insert("lang".into(), toml::Value::String(l.clone()));
        }
        None => {
            table.remove("lang");
        }
    }
    let mut theme = toml::Table::new();
    theme.insert(
        "preset".into(),
        toml::Value::String(cfg.theme.preset.clone()),
    );
    theme.insert(
        "accent".into(),
        toml::Value::String(cfg.theme.accent.clone()),
    );
    table.insert("theme".into(), toml::Value::Table(theme));
    Ok(toml::to_string_pretty(&table).unwrap_or_default())
}

impl Config {
    /// XDG 경로에 원자적으로 저장한다(임시파일 → rename). 디렉터리·파일을 못
    /// 쓰거나(권한 등) 기존 파일이 파싱 불가면 Err를 돌려주되 호출부(설정
    /// 화면)가 조용히 저하한다 — 앱은 죽지 않는다.
    ///
    /// "파일 없음"과 "파일은 있는데 읽기/파싱 실패"를 구분한다: 파일이 없으면
    /// 빈 테이블에서 시작(첫 저장, 정상)하지만, 파일이 있는데 파싱에
    /// 실패하면(사용자 수기 오타 등) 빈 테이블로 폴백하지 않고 Err를 반환해
    /// 파일을 건드리지 않는다 — 그래야 앱이 모르는 키·섹션이 소실되지 않는다.
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path().ok_or_else(|| std::io::Error::other("no config dir"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existing = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let body = merge_into_toml(&existing, self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // pid를 섞어 동시에 저장하는 두 인스턴스의 tmp 파일이 서로 덮어쓰지
        // 않게 한다(rename 자체는 원자적이라 최종 파일은 항상 안전).
        let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(body.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
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
            theme: ThemeConfig::default(),
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

    #[test]
    fn merge_preserves_known_values_round_trip() {
        let cfg = Config {
            favorite_team: Some("LG".into()),
            poll_secs: 7,
            lang: Some("ko".into()),
            theme: ThemeConfig::default(),
        };
        let out = merge_into_toml("", &cfg).unwrap();
        let back = config_from_toml_str(&out);
        assert_eq!(back.favorite_team.as_deref(), Some("LG"));
        assert_eq!(back.poll_secs, 7);
        assert_eq!(back.lang.as_deref(), Some("ko"));
    }

    /// 앱이 모르는 키(사용자 수기 입력)를 save가 날리지 않는다.
    #[test]
    fn merge_preserves_unknown_keys() {
        let existing = "favorite_team = \"OB\"\n[experimental]\nsecret = 42\n";
        let cfg = Config {
            favorite_team: Some("LG".into()),
            poll_secs: 5,
            lang: None,
            theme: ThemeConfig::default(),
        };
        let out = merge_into_toml(existing, &cfg).unwrap();
        // 알려진 키는 갱신
        assert!(out.contains("favorite_team = \"LG\""));
        // 모르는 섹션은 보존
        assert!(
            out.contains("[experimental]"),
            "unknown section dropped:\n{out}"
        );
        assert!(out.contains("secret = 42"));
    }

    /// lang=None이면 키를 쓰지 않는다(기본값 오염 방지, 관용).
    #[test]
    fn merge_omits_none_lang() {
        let cfg = Config {
            favorite_team: None,
            poll_secs: 5,
            lang: None,
            theme: ThemeConfig::default(),
        };
        let out = merge_into_toml("", &cfg).unwrap();
        assert!(
            !out.contains("lang"),
            "None lang must not be written:\n{out}"
        );
    }

    /// 파일이 있는데 파싱 불가(사용자 수기 오타 등)면 빈 테이블로 폴백하지
    /// 않고 Err를 반환해야 한다 — save()가 이를 받아 파일을 건드리지 않고
    /// 알려지지 않은 키·섹션의 소실을 막는 것이 이 결함 수정의 핵심.
    #[test]
    fn merge_returns_err_on_malformed_existing() {
        let existing = "favorite_team = \"OB\"\n[experimental]\nnote = \"oops";
        let cfg = Config::default();
        assert!(
            merge_into_toml(existing, &cfg).is_err(),
            "malformed existing TOML must not be silently swallowed"
        );
    }

    /// existing="" (파일 없음)은 여전히 Ok로 취급되어야 첫 저장이 정상 동작한다.
    #[test]
    fn merge_empty_existing_is_ok() {
        assert!(merge_into_toml("", &Config::default()).is_ok());
    }

    #[test]
    fn theme_config_defaults_and_parses() {
        let c = config_from_toml_str("");
        assert_eq!(c.theme.preset, "default");
        assert_eq!(c.theme.accent, "team");
        let c2 = config_from_toml_str("[theme]\npreset = \"mono\"\naccent = \"cyan\"");
        assert_eq!(c2.theme.preset, "mono");
        assert_eq!(c2.theme.accent, "cyan");
    }
}
