//! **테스트는 사용자의 `config.toml`을 건드리지 않는다.**
//!
//! F9 설정 화면의 키 처리는 값을 바꿀 때마다 곧바로 파일에 되쓴다. 그 저장이
//! 실제 XDG 경로를 스스로 찾아가던 동안, `settings_changes_team` 같은 평범한
//! 단위 테스트 하나가 **개발자의 진짜 설정 파일을 덮어썼다**(실측 2026-08-24:
//! favorite_team·lang·mouse·poll_secs가 기본값으로 갈렸다). 테스트가 사용자
//! 데이터를 파괴한 것이다.
//!
//! 이 파일은 그 회귀만 본다. `cargo`는 통합 테스트를 **파일마다 별도
//! 프로세스**로 돌리므로, 여기서 `HOME`을 바꿔도 다른 테스트와 경합하지 않는다.

use crossterm::event::KeyCode;
use kbotop::app::App;
use std::path::{Path, PathBuf};

#[test]
fn driving_the_settings_screen_writes_nothing_you_did_not_ask_for() {
    let home = std::env::temp_dir().join(format!("kbotop-config-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("격리 HOME을 못 만들었다");
    // 설정 경로 해석은 유닉스에서 HOME(리눅스는 XDG_CONFIG_HOME 우선)을 본다.
    std::env::set_var("HOME", &home);
    std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
    // HOME으로 격리되지 않는 곳(윈도우 AppData)까지 보려고 실제 경로도 함께
    // 스냅샷한다 — "안 만들었다"가 아니라 "안 건드렸다"를 본다.
    let real = kbotop::config::config_path();
    let before = real.as_ref().and_then(|p| std::fs::read(p).ok());

    // 설정 화면을 열어 모든 행을 좌·우·Enter로 흔든다(= persist가 가는 전 경로).
    let mut app = App::new(Default::default());
    app.on_key(KeyCode::F(9));
    for _ in 0..app.settings_rows().len() {
        app.on_key(KeyCode::Right);
        app.on_key(KeyCode::Left);
        app.on_key(KeyCode::Enter);
        app.on_key(KeyCode::Down);
    }

    let leaked = files_under(&home);
    assert!(
        leaked.is_empty(),
        "테스트가 HOME 아래에 파일을 만들었다: {leaked:?}"
    );
    let after = real.as_ref().and_then(|p| std::fs::read(p).ok());
    assert_eq!(
        before, after,
        "테스트가 실제 설정 파일을 건드렸다: {real:?}"
    );

    // **저장이 죽은 것과 구분한다.** persist를 통째로 없애도 위 단언은 통과한다 —
    // 저장할 곳을 주면 거기에는 실제로 써야 한다.
    let injected = home.join("injected.toml");
    app.config_path = Some(injected.clone());
    app.on_key(KeyCode::Right);
    assert!(
        injected.is_file(),
        "저장 경로를 주입했는데 아무것도 안 썼다: {injected:?}"
    );

    let _ = std::fs::remove_dir_all(&home);
}

/// 디렉터리 아래 파일 전부(재귀). 빈 디렉터리는 세지 않는다.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}
