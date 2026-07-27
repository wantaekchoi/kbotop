use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub favorite_team: Option<String>,
    pub poll_secs: u64,
    pub lang: Option<String>,
    /// 표시 시간대: `auto`(기본) · `kst` · `+09:00` 류 오프셋. 자동 감지가 안
    /// 되는 환경(Windows·컨테이너)에서 사용자가 직접 정하는 탈출구.
    pub timezone: Option<String>,
    pub theme: ThemeConfig,
    /// 마우스로 클릭·스크롤할지(v0.27). **끄면 터미널이 드래그 선택·복사를
    /// 되찾는다** — 마우스 캡처를 켜면 그 입력이 앱으로 넘어오기 때문이다.
    /// 기본값이 켬인 이유는 꺼 두면 아무도 기능을 발견하지 못하고, 켠 쪽에는
    /// Shift+드래그라는 우회 수단이 있기 때문이다(README에 적어 뒀다).
    pub mouse: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            favorite_team: None,
            poll_secs: 5,
            lang: None,
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: true,
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
pub fn config_path() -> Option<PathBuf> {
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
    match &cfg.timezone {
        Some(t) => {
            table.insert("timezone".into(), toml::Value::String(t.clone()));
        }
        None => {
            table.remove("timezone");
        }
    }
    table.insert("mouse".into(), toml::Value::Boolean(cfg.mouse));
    // [theme] 테이블도 top-level과 동일한 원리로 다룬다: 통째로 재생성하지
    // 않고 기존 하위 키(사용자 수기 입력·미래 버전의 신규 키)를 읽어와 아는
    // 키(preset·accent)만 덮어쓴다. 기존 [theme]가 테이블이 아니거나 없으면
    // 빈 테이블에서 새로 시작한다.
    let mut theme = match table.get("theme") {
        Some(toml::Value::Table(t)) => t.clone(),
        _ => toml::Table::new(),
    };
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

// --- 다중 인스턴스 저장 직렬화 (락파일, std-only) ---
//
// 원자적 rename만으로는 "두 인스턴스가 거의 동시에 save()"하는 경우를 막지
// 못한다: 각자 기존 파일을 읽어 merge한 뒤 tmp→rename하므로, 나중에
// rename하는 쪽이 앞선 쪽의 변경을 통째로 덮어써 lost-update가 난다. 락파일로
// "읽기→merge→쓰기" 구간 전체를 직렬화한다.

/// 락 파일이 이 시간(초)보다 오래되면 크래시로 잔재만 남은 것으로 보고
/// 탈취한다(정상 저장은 수 ms~수십 ms면 끝나므로 넉넉히 잡음).
const LOCK_STALE_SECS: u64 = 10;
/// 락 획득 재시도 최대 횟수. stale 락 탈취 직후의 재시도도 이 예산을 쓴다.
const LOCK_MAX_ATTEMPTS: u32 = 8;
const LOCK_RETRY_DELAY_MS: u64 = 50;

/// 저장 락 RAII 가드. 스코프를 벗어나면(정상 반환·Err·`?` 조기반환 모두)
/// Drop에서 락 파일을 제거한다 — 락 해제를 성공 경로에만 수동으로 넣으면
/// 중간의 `?`가 락을 영영 남길 수 있어, 그 실수 자체를 구조적으로 없앤다.
struct SaveLock {
    path: PathBuf,
}

impl Drop for SaveLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// config 경로에 대응하는 락 파일 경로(`config.toml` → `config.toml.lock`).
fn lock_path_for(config_path: &Path) -> PathBuf {
    let mut os = config_path.as_os_str().to_os_string();
    os.push(".lock");
    PathBuf::from(os)
}

/// 락 파일 mtime이 임계치보다 오래됐으면 크래시로 남은 잔재로 간주한다.
/// 메타데이터를 못 읽으면(경합 중 다른 프로세스가 이미 지웠을 수 있음)
/// stale로 단정하지 않는다 — 다음 create_new 시도가 자연히 처리한다.
fn is_stale_lock(lock_path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(lock_path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age > Duration::from_secs(LOCK_STALE_SECS))
        .unwrap_or(false)
}

/// 저장 락을 잡는다. `OpenOptions::create_new`는 POSIX상 원자적이라 두
/// 인스턴스가 동시에 시도해도 하나만 성공한다. 이미 잡혀 있으면 stale
/// 여부를 확인해 잔재면 제거 후 즉시 재시도하고, 살아있는 락이면 짧게
/// 대기 후 재시도한다. 예산(LOCK_MAX_ATTEMPTS)을 다 쓰면 None을 돌려주고
/// 호출부가 Err로 조용히 저하한다.
fn acquire_lock(lock_path: &Path) -> Option<SaveLock> {
    for _ in 0..LOCK_MAX_ATTEMPTS {
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(lock_path)
        {
            Ok(_) => {
                return Some(SaveLock {
                    path: lock_path.to_path_buf(),
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_stale_lock(lock_path) {
                    let _ = std::fs::remove_file(lock_path);
                    continue; // 잔재 정리 직후이므로 대기 없이 바로 재시도
                }
                std::thread::sleep(Duration::from_millis(LOCK_RETRY_DELAY_MS));
            }
            // 락 파일 자체를 못 만드는 상황(권한 등) — 재시도해도 소용없음
            Err(_) => return None,
        }
    }
    None
}

/// 저장 코어: 기존 파일 읽기 → merge → tmp 파일 write → rename. 락 상태와
/// 무관한 순수 저장 로직만 담당한다(락 획득은 호출부 save_to의 몫).
///
/// "파일 없음"과 "파일은 있는데 읽기/파싱 실패"를 구분한다: 파일이 없으면
/// 빈 테이블에서 시작(첫 저장, 정상)하지만, 파일이 있는데 파싱에
/// 실패하면(사용자 수기 오타 등) 빈 테이블로 폴백하지 않고 Err를 반환해
/// 파일을 건드리지 않는다 — 그래야 앱이 모르는 키·섹션이 소실되지 않는다.
fn write_config(path: &Path, cfg: &Config) -> std::io::Result<()> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let body = merge_into_toml(&existing, cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // pid를 섞어 동시에 저장하는 두 인스턴스의 tmp 파일이 서로 덮어쓰지
    // 않게 한다(rename 자체는 원자적이라 최종 파일은 항상 안전).
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Config {
    /// XDG 경로에 원자적으로 저장한다(락파일로 직렬화 + 임시파일 → rename).
    /// 디렉터리·파일을 못 쓰거나(권한 등), 기존 파일이 파싱 불가하거나,
    /// 락을 못 잡으면 Err를 돌려주되 호출부(설정 화면)가 조용히 저하한다 —
    /// 앱은 죽지 않는다. 실제 경로는 여기서 고정하고, 테스트 가능한 코어는
    /// save_to에 둔다(경로 주입).
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path().ok_or_else(|| std::io::Error::other("no config dir"))?;
        self.save_to(&path)
    }

    /// 경로를 주입 가능한 저장 코어. 실 XDG config를 건드리지 않고 락·저장
    /// 로직을 테스트하기 위해 save()에서 분리했다.
    ///
    /// 락 파일(`<path>.lock`)을 먼저 잡아 저장을 직렬화한다. 락은 RAII
    /// 가드(SaveLock)로 관리되어, write_config가 `?`로 조기반환하든
    /// 정상적으로 끝나든 이 함수를 벗어날 때 항상 제거된다.
    fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lock_path = lock_path_for(path);
        let _lock = acquire_lock(&lock_path)
            .ok_or_else(|| std::io::Error::other("save lock busy, giving up"))?;
        write_config(path, self)?;
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
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: true,
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
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: true,
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
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: true,
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
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: true,
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

    /// [theme] 하위의 사용자 수기 입력·미래 버전 신규 키를 save가 날리지
    /// 않는다 — top-level 미지 키 보존과 동일한 원리를 중첩 테이블에도 적용.
    #[test]
    fn merge_preserves_unknown_theme_keys() {
        let existing = "[theme]\ncustom_future_key = 1\npreset = \"x\"\n";
        let cfg = Config {
            favorite_team: None,
            poll_secs: 5,
            lang: None,
            timezone: None,
            theme: ThemeConfig {
                preset: "mono".into(),
                accent: "cyan".into(),
            },
            mouse: true,
        };
        let out = merge_into_toml(existing, &cfg).unwrap();
        assert!(
            out.contains("custom_future_key = 1"),
            "unknown [theme] key dropped:\n{out}"
        );
        assert!(
            out.contains("preset = \"mono\""),
            "preset not updated:\n{out}"
        );
        assert!(
            out.contains("accent = \"cyan\""),
            "accent not updated:\n{out}"
        );
    }

    // --- save 락파일 직렬화 테스트 ---
    //
    // 실제 XDG config를 절대 건드리지 않도록, config_path()/save()가 아니라
    // 경로 주입 가능한 save_to()·lock_path_for()를 직접 호출해 std::env::temp_dir()
    // 아래 유니크 경로에서만 동작을 검증한다.

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// std::env::temp_dir() 아래 이 테스트 실행 전용 유니크 디렉터리 경로.
    /// pid + nanos + 카운터를 섞어 병렬 테스트 스레드 간 충돌을 피한다.
    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let n = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kbotop-config-test-{label}-{}-{nanos}-{n}",
            std::process::id()
        ))
    }

    /// 저장 성공 후 config 파일이 쓰이고, 락 파일은 남지 않는다.
    #[test]
    fn save_to_writes_config_and_releases_lock_on_success() {
        let dir = unique_test_dir("ok");
        let path = dir.join("config.toml");
        let cfg = Config {
            favorite_team: Some("LG".into()),
            poll_secs: 9,
            lang: None,
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: true,
        };

        cfg.save_to(&path).expect("save_to should succeed");

        let saved = std::fs::read_to_string(&path).expect("config file should exist");
        assert!(saved.contains("favorite_team = \"LG\""));
        assert!(
            !lock_path_for(&path).exists(),
            "lock file must be removed after a successful save"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 다른 인스턴스가 저장 중임을 뜻하는 신선한(mtime 최근) 락이 있으면,
    /// 재시도 예산을 다 쓰고 Err를 반환하며 그 락을 함부로 지우지 않는다.
    #[test]
    fn save_to_fails_when_a_fresh_lock_is_held() {
        let dir = unique_test_dir("busy");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let lock = lock_path_for(&path);
        std::fs::write(&lock, b"").expect("seed a fresh lock file");

        let cfg = Config::default();
        let result = cfg.save_to(&path);

        assert!(
            result.is_err(),
            "save_to must not proceed while a fresh lock is held"
        );
        assert!(
            lock.exists(),
            "save_to must not remove a lock it doesn't own"
        );
        assert!(
            !path.exists(),
            "config file must not be written while locked out"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// mtime이 stale 임계치보다 오래된 락(크래시 잔재)은 탈취되어 저장이
    /// 정상 진행된다 — 데드락 방지.
    #[test]
    fn save_to_steals_a_stale_lock() {
        let dir = unique_test_dir("stale");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let lock = lock_path_for(&path);
        let f = std::fs::File::create(&lock).expect("seed a stale lock file");
        let old = SystemTime::now() - Duration::from_secs(LOCK_STALE_SECS + 5);
        f.set_modified(old).expect("backdate lock mtime");
        drop(f);

        let cfg = Config::default();
        let result = cfg.save_to(&path);

        assert!(result.is_ok(), "stale lock should be stolen: {result:?}");
        assert!(
            !lock.exists(),
            "lock must be released after the stolen save completes"
        );
        assert!(path.exists(), "config file should have been written");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// write_config가 `?`로 조기반환해도(기존 파일이 파싱 불가) save_to의
    /// RAII 락 가드는 락 파일을 남기지 않는다.
    #[test]
    fn save_to_releases_lock_even_when_write_config_fails() {
        let dir = unique_test_dir("failcore");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        // 기존 파일이 파싱 불가 → write_config가 merge_into_toml의 Err를
        // `?`로 조기반환한다.
        std::fs::write(&path, "favorite_team = \"OB\"\n[broken\nnope").unwrap();

        let cfg = Config::default();
        let result = cfg.save_to(&path);

        assert!(
            result.is_err(),
            "malformed existing config must propagate as Err, not be swallowed"
        );
        assert!(
            !lock_path_for(&path).exists(),
            "lock must be released even when the save core returns Err early"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
    /// 마우스 설정이 파일을 왕복한다 — 껐다가 앱을 다시 켰더니 도로 켜져 있으면
    /// 끈 의미가 없다. merge_into_toml에서 실제로 빠뜨렸던 결함이다(실행 중
    /// config.toml에 항목이 안 남는 것으로 드러났다).
    #[test]
    fn mouse_survives_a_round_trip() {
        let cfg = Config {
            favorite_team: None,
            poll_secs: 5,
            lang: None,
            timezone: None,
            theme: ThemeConfig::default(),
            mouse: false,
        };
        let body = merge_into_toml("", &cfg).unwrap();
        assert!(body.contains("mouse"), "저장 내용에 mouse가 없다:\n{body}");
        let back: Config = toml::from_str(&body).unwrap();
        assert!(!back.mouse);
    }
}
