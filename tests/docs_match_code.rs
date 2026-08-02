//! 공개 문서가 실제 동작과 어긋나지 않는지 검사한다.
//!
//! v0.25에서 README를 한 줄씩 훑다 **사실이 틀린 곳 넷**을 찾았다 — 설정 파일
//! 경로가 Linux 기준으로만 적혀 있었고, 설치 스크립트가 Windows에서 그냥 돌지
//! 않는데 그렇게 적혀 있었고, v0.24에서 추가한 순위 탭 `Enter`가 빠져 있었고,
//! CHANGELOG의 버전 비교 링크가 v0.17에서 멈춰 있었다. **검사가 없어서 릴리스
//! 여섯 번을 그대로 지났다.**
//!
//! 문서는 사람이 읽는 산문이라 전부를 기계로 검증할 수는 없다. 하지만 **코드와
//! 대응되는 사실**(CLI 플래그, 버전 목록, 두 언어판의 구조)은 검사할 수 있고,
//! 위 넷 중 셋이 그 부류였다.

const README_KO: &str = include_str!("../README.md");
const README_EN: &str = include_str!("../README.en.md");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");
const CARGO_TOML: &str = include_str!("../Cargo.toml");

/// `--help`가 광고하는 플래그는 README 사용법에도 있어야 한다.
///
/// v0.16에서 `--tz`를 추가하고 README에 안 적어, 시간대를 직접 정할 수 있다는
/// 사실이 여덟 릴리스 동안 문서에 없었다(v0.25에서 발견).
///
/// **clap에게 직접 묻는다.** v0.28까지는 `src/main.rs`를 텍스트로 긁어
/// `#[arg(long)]` 다음 줄을 필드로 가정했는데, 그 방식은 `#[arg(long = "...")]`
/// 이름 오버라이드도 short 옵션도 못 보고, 단언이 `>= 4`라 **파서가 하나를
/// 놓쳐도 통과**했다. `Cli`를 lib으로 옮겨 이 질문이 가능해졌다.
#[test]
fn every_cli_flag_appears_in_both_readmes() {
    use clap::CommandFactory;
    let cmd = kbotop::cli::Cli::command();
    let flags: Vec<String> = cmd
        .get_arguments()
        .filter_map(|a| a.get_long())
        // help/version은 clap이 자동으로 넣는 것이라 README가 따로 안내하지 않는다.
        .filter(|n| *n != "help" && *n != "version")
        .map(str::to_string)
        .collect();

    assert_eq!(
        flags.len(),
        5,
        "플래그 개수가 바뀌었다 — README와 이 숫자를 함께 고칠 것: {flags:?}"
    );

    for flag in &flags {
        let needle = format!("--{flag}");
        for (name, doc) in [("README.md", README_KO), ("README.en.md", README_EN)] {
            assert!(
                doc.contains(&needle),
                "{name}에 `{needle}`이 없다 — 플래그를 추가하고 문서를 안 고쳤다"
            );
        }
    }
}

/// short 옵션을 조용히 늘리지 않는다.
///
/// `-t` 같은 걸 하나 추가하면 그것도 영구히 남는 공개 인터페이스인데, 긴 이름만
/// 세던 검사는 아무 말도 하지 않았다. 우리가 정의한 인자에는 short가 없고,
/// `-h`/`-V`는 clap이 자체 처리해 `get_arguments()`에 나오지 않는다(실제 실행은
/// 된다 — 그건 clap이 보장한다).
#[test]
fn we_do_not_add_short_flags_without_noticing() {
    use clap::CommandFactory;
    let cmd = kbotop::cli::Cli::command();
    let shorts: Vec<char> = cmd.get_arguments().filter_map(|a| a.get_short()).collect();
    assert!(
        shorts.is_empty(),
        "short 옵션이 생겼다 — 의도한 것이면 이 테스트를 고칠 것: {shorts:?}"
    );
}

/// CHANGELOG의 모든 버전 제목에는 하단 링크 정의가 있어야 한다.
///
/// Keep a Changelog 형식은 `## [0.24.0]`을 링크로 만들려고 문서 끝에
/// `[0.24.0]: <compare URL>`을 둔다. v0.18~v0.24 일곱 개가 빠져 있었다.
#[test]
fn every_changelog_version_has_a_link_definition() {
    let versions: Vec<&str> = CHANGELOG
        .lines()
        .filter_map(|l| l.strip_prefix("## ["))
        .filter_map(|l| l.split(']').next())
        .collect();

    assert!(versions.len() > 10, "버전 제목을 못 찾았다: {versions:?}");

    for v in &versions {
        let def = format!("\n[{v}]: ");
        assert!(
            CHANGELOG.contains(&def),
            "CHANGELOG에 `[{v}]` 링크 정의가 없다 — 제목이 링크로 안 걸린다"
        );
    }
}

/// `[Unreleased]` 비교 링크는 **가장 최근 릴리스**를 가리켜야 한다.
///
/// v0.17을 가리킨 채 방치돼, v0.18 이후의 변경이 통째로 빠져 보였다.
#[test]
fn the_unreleased_link_points_at_the_latest_release() {
    let latest = CHANGELOG
        .lines()
        .filter_map(|l| l.strip_prefix("## ["))
        .filter_map(|l| l.split(']').next())
        .find(|v| *v != "Unreleased")
        .expect("릴리스가 하나도 없다");

    let expected =
        format!("[Unreleased]: https://github.com/wantaekchoi/kbotop/compare/v{latest}...HEAD");
    assert!(
        CHANGELOG.contains(&expected),
        "Unreleased 링크가 최신({latest})을 안 가리킨다"
    );
}

/// 최신 CHANGELOG 항목의 버전이 `Cargo.toml`과 같아야 한다 — 버전을 올리고
/// 변경 이력을 안 쓰거나, 그 반대를 막는다.
#[test]
fn the_latest_changelog_entry_matches_the_crate_version() {
    let crate_version = CARGO_TOML
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|l| l.split('"').next())
        .expect("Cargo.toml에 version이 없다");

    let latest = CHANGELOG
        .lines()
        .filter_map(|l| l.strip_prefix("## ["))
        .filter_map(|l| l.split(']').next())
        .find(|v| *v != "Unreleased")
        .expect("릴리스 항목이 없다");

    assert_eq!(
        latest, crate_version,
        "CHANGELOG 최신 항목({latest})과 crate 버전({crate_version})이 다르다"
    );
}

/// 두 언어판의 **절 구성**이 같아야 한다. 한쪽에만 절을 더하면 다른 쪽 독자가
/// 그 내용을 통째로 못 본다(CONTRIBUTING이 "README.md를 고쳤으면 README.en.md도
/// 맞춰 주세요"라고 적어 둔 규칙을 기계로 강제한다).
#[test]
fn both_readmes_have_the_same_section_count() {
    let count = |doc: &str| doc.lines().filter(|l| l.starts_with("## ")).count();
    assert_eq!(
        count(README_KO),
        count(README_EN),
        "README 두 판의 절 개수가 다르다 — 한쪽만 고쳤다"
    );
}

/// 설치 안내가 실제 릴리스 자산 이름을 가리켜야 한다. Windows는 설치 스크립트가
/// 아니라 zip을 받는 경로라(스크립트는 POSIX 셸이 필요하다) 파일명이 문서에
/// 박혀 있고, 그 이름이 dist 설정의 타깃과 어긋나면 링크가 죽는다.
#[test]
fn the_windows_asset_name_matches_the_dist_target() {
    let dist = include_str!("../dist-workspace.toml");
    assert!(
        dist.contains("x86_64-pc-windows-msvc"),
        "dist 타깃에서 Windows가 빠졌는데 README는 zip을 안내하고 있다"
    );
    for (name, doc) in [("README.md", README_KO), ("README.en.md", README_EN)] {
        assert!(
            doc.contains("kbotop-x86_64-pc-windows-msvc.zip"),
            "{name}에 Windows 자산 이름이 없다"
        );
    }
}

/// 설정 파일 경로가 **실제 경로**와 맞아야 한다.
///
/// README 두 판이 Windows 경로를 `%APPDATA%\kbotop\`이라고 적어 뒀는데,
/// `directories`가 실제로 쓰는 건 그 아래 `config\`까지다. 아무도 검사하지
/// 않아 몇 릴리스를 그대로 지났다. 여기서는 **지금 이 플랫폼의 경로**만
/// 검증할 수 있으므로, CI가 도는 리눅스와 개발 머신의 macOS가 각각 자기
/// 몫을 본다.
#[test]
fn the_documented_config_path_matches_the_real_one() {
    let path = kbotop::config::config_path().expect("설정 경로를 못 구한다");
    let shown = path.to_string_lossy().replace('\\', "/");
    // 홈 디렉터리는 문서에서 `~`나 환경변수로 적히므로, 그 아래 꼬리만 본다.
    let tail: Vec<&str> = shown.rsplit('/').take(3).collect();
    let tail = format!("{}/{}/{}", tail[2], tail[1], tail[0]);
    for (name, doc) in [("README.md", README_KO), ("README.en.md", README_EN)] {
        let normalized = doc.replace('\\', "/");
        assert!(
            normalized.contains(&tail)
                || normalized.contains(tail.trim_end_matches("/config.toml")),
            "{name}에 이 플랫폼의 설정 경로({tail})가 없다"
        );
    }
}

/// `--license`가 실제 고지를 뱉는가.
///
/// 이 플래그가 있는 이유는 Homebrew·curl 인스톨러·`cargo install`로 받은
/// 사람에게 고지가 **닿지 않기 때문**이다(그 셋은 바이너리만 남긴다). 그래서
/// 고지가 비거나 엉뚱해지면 세 채널이 통째로 의무를 못 지킨다.
#[test]
fn the_license_flag_prints_the_real_notice() {
    const NOTICE: &str = include_str!("../THIRD-PARTY.md");
    assert!(
        NOTICE.len() > 10_000,
        "고지가 너무 짧다: {}바이트",
        NOTICE.len()
    );
    assert!(NOTICE.contains("Unlicense"), "고지에 우리 라이선스가 없다");
    assert!(
        NOTICE.contains("ratatui") && NOTICE.contains("crossterm"),
        "고지에 핵심 의존성이 없다"
    );
}

/// 고지에 적힌 **버전**이 실제 lock과 맞아야 한다.
///
/// 고지가 v0.15 시절에 멈춘 채 열두 릴리스를 지났고, 그동안 `either 1.16.0`처럼
/// 낡은 버전이 적혀 있었다. "크레이트 이름이 다 있는가"만 보면 이런 드리프트를
/// 못 잡는다 — 이름은 그대로이기 때문이다.
///
/// `Cargo.lock`에는 우리가 안 쓰는 플랫폼의 크레이트도 있으므로 **고지에 적힌
/// 것만** 대조한다(고지 쪽이 배포 타깃으로 한정된 정확한 목록이다).
#[test]
fn the_notice_lists_the_versions_we_actually_lock() {
    const NOTICE: &str = include_str!("../THIRD-PARTY.md");
    const LOCK: &str = include_str!("../Cargo.lock");

    // Cargo.lock: name = "x" 다음 줄에 version = "y"
    let mut locked: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    let lines: Vec<&str> = LOCK.lines().collect();
    for w in lines.windows(2) {
        let (Some(name), Some(ver)) = (
            w[0].strip_prefix("name = \"")
                .and_then(|l| l.split('"').next()),
            w[1].strip_prefix("version = \"")
                .and_then(|l| l.split('"').next()),
        ) else {
            continue;
        };
        locked.entry(name).or_default().push(ver);
    }
    assert!(
        locked.len() > 50,
        "Cargo.lock 파싱이 깨졌다: {}",
        locked.len()
    );

    // 고지: `- [name version](url)` 또는 `- name version`
    let mut checked = 0;
    let mut stale: Vec<String> = Vec::new();
    for line in NOTICE.lines() {
        let Some(rest) = line.strip_prefix("- ") else {
            continue;
        };
        let inner = rest
            .strip_prefix('[')
            .map_or(rest, |r| r.split(']').next().unwrap_or(r));
        let mut parts = inner.rsplitn(2, ' ');
        let (Some(ver), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if name == "kbotop" {
            continue; // 우리 버전은 Cargo.toml 쪽 테스트가 본다
        }
        let Some(versions) = locked.get(name) else {
            continue; // lock에 없는 이름은 이 테스트의 관심 밖
        };
        checked += 1;
        if !versions.contains(&ver) {
            stale.push(format!("{name}: 고지 {ver} / lock {versions:?}"));
        }
    }
    assert!(checked > 50, "대조한 크레이트가 너무 적다: {checked}");
    assert!(
        stale.is_empty(),
        "고지가 낡았다 — `./scripts/third-party.sh`를 다시 돌려야 한다:\n{}",
        stale.join("\n")
    );
}

/// 데모 녹화가 쓰는 키는 코드에 살아 있어야 한다.
///
/// VHS는 F키를 못 보내서 tape가 `S`(F9 별칭)를 누른다. 그런데 `S`는 도움말에도
/// README에도 없어서 "아무도 안 쓰는 키"로 보인다 — 죽은 코드로 판단해 지우면
/// **릴리스마다 도는 녹화가 조용히 깨진다.** 화면에는 설정이 안 열린 채로 녹화가
/// 끝나고, 그걸 알아채는 건 GIF를 프레임 단위로 볼 때뿐이다.
#[test]
fn the_keys_the_demo_tapes_press_still_exist() {
    const APP_RS: &str = include_str!("../src/app.rs");
    for tape in [
        include_str!("../docs/demo.tape"),
        include_str!("../docs/demo.en.tape"),
    ] {
        for line in tape.lines() {
            let Some(rest) = line.trim().strip_prefix("Type \"") else {
                continue;
            };
            let Some(keys) = rest.split('"').next() else {
                continue;
            };
            // 한 글자짜리 키만 본다(`[`처럼 여러 번 눌리는 것도 포함).
            let mut chars = keys.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                continue;
            };
            if c.is_whitespace() {
                continue;
            }
            let needle = format!("KeyCode::Char('{c}')");
            assert!(
                APP_RS.contains(&needle),
                "데모 tape가 누르는 `{c}`를 app.rs가 더 이상 처리하지 않는다"
            );
        }
    }
}

/// 한 줄에서 등장 순서대로 정수를 모두 뽑는다. 문서 문장 안의 숫자를 **그 문장
/// 안에서만** 읽기 위한 것 — 문서 전체에 `contains("60")`을 하면 어디에 있든
/// 통과해 버려 검사가 아무것도 안 지킨다(이 파일에서 실제로 그렇게 썼다가
/// 상수를 300→240으로 바꿔도 초록이라 걸렸다).
fn numbers_in(line: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in line.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(cur.parse().unwrap());
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push(cur.parse().unwrap());
    }
    out
}

/// `marker`로 시작하는 줄을 찾는다.
fn line_starting_with<'a>(doc: &'a str, marker: &str) -> &'a str {
    doc.lines()
        .find(|l| l.starts_with(marker))
        .unwrap_or_else(|| panic!("'{marker}'로 시작하는 줄이 문서에 없다"))
}

/// README가 문장으로 적어 둔 **갱신 주기 네 개**가 실제 상수와 같은지 본다.
///
/// v0.31에서 "무엇이 보이나" 절을 쓰면서 라이브 5초·목록 60초·순위 90초·종료
/// 5분을 적었다. 이런 숫자는 조용히 늙는다 — v0.29에서 종료 경기 주기를 30초에서
/// 5분으로 바꿨을 때 아무 문서도 따라오지 않았고, 그 사실을 아무도 몰랐다.
/// 문장 **그 줄에서** 순서대로 읽어 대조한다.
#[test]
fn the_documented_poll_intervals_match_the_constants() {
    use kbotop::config::Config;
    use kbotop::poller::{FINAL_LIVE_POLL_SECS, GAMES_POLL_SECS, STANDINGS_POLL_SECS};

    let expected = vec![
        Config::default().effective_poll_secs(),
        GAMES_POLL_SECS,
        STANDINGS_POLL_SECS,
        FINAL_LIVE_POLL_SECS / 60,
    ];
    for (doc, marker, name) in [
        (README_KO, "갱신 주기는", "README.md"),
        (README_EN, "Refresh intervals:", "README.en.md"),
    ] {
        let line = line_starting_with(doc, marker);
        assert_eq!(
            numbers_in(line),
            expected,
            "{name}의 갱신 주기 문장이 상수와 다르다: {line:?}"
        );
    }
}

/// 위 테스트는 종료 경기 주기를 **분으로** 대조한다. 초 단위 상수가 60으로
/// 안 떨어지면 문서가 반올림된 거짓말을 하게 되므로 여기서 막는다.
#[test]
fn the_finished_game_interval_is_a_whole_number_of_minutes() {
    assert_eq!(
        kbotop::poller::FINAL_LIVE_POLL_SECS % 60,
        0,
        "README가 '5분'처럼 분으로 적는다 — 분으로 안 떨어지면 문서를 초로 고쳐야 한다"
    );
}
