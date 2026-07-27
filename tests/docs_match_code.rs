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
#[test]
fn every_cli_flag_appears_in_both_readmes() {
    // clap 정의를 파싱하는 대신 소스에서 `#[arg(long)]` 아래 필드명을 읽는다 —
    // 바이너리를 실행하지 않고도 플래그 목록을 얻는 가장 단순한 경로다.
    let main_rs = include_str!("../src/main.rs");
    let flags: Vec<String> = main_rs
        .lines()
        .collect::<Vec<_>>()
        .windows(3)
        .filter(|w| w[0].trim_start().starts_with("#[arg(long"))
        .filter_map(|w| {
            // `    team: Option<String>,` → team
            let line = w[1].trim_start();
            let name = line.split(':').next()?.trim();
            (!name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
                .then(|| name.replace('_', "-"))
        })
        .collect();

    assert!(
        flags.len() >= 4,
        "플래그를 못 찾았다 — 파싱 방식이 깨졌을 수 있다: {flags:?}"
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
