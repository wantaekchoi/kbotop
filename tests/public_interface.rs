//! 1.0에서 얼릴 **공개 인터페이스**를 봉인한다.
//!
//! 여기 적힌 것들은 "구현 세부"가 아니라 **사용자가 자기 손으로 적어 둔 것**이다
//! — 스크립트에 넣은 플래그, `config.toml`에 적은 키, 손가락이 외운 키 바인딩.
//! 이걸 바꾸면 남의 설정이 조용히 깨진다. 그래서 바꾸는 것 자체를 막지는 않되,
//! **모르고 바꾸는 일**은 막는다. 이 파일이 실패하면 둘 중 하나다:
//!   1. 실수로 인터페이스를 건드렸다 → 되돌린다.
//!   2. 일부러 바꿨다 → 여기 목록도 같이 고치고, CHANGELOG에 적는다.
//!
//! **관용 방향은 한쪽이다.** 없어진 것을 늘리는 건 파괴적이고, 새로 더하는 건
//! 아니다 — 그래서 "이 집합과 정확히 같은가"가 아니라 "**이것들이 여전히
//! 있는가**"를 본다. 다만 CLI 플래그만은 정확히 같은지까지 본다(플래그가 하나
//! 늘면 README·도움말도 같이 늘어야 하고, 그건 이미 `docs_match_code.rs`가
//! 본다).

use clap::CommandFactory;
use crossterm::event::KeyCode;
use kbotop::app::{App, Overlay, Screen, Tab};
use kbotop::config::Config;

/// v0.31 기준 우리가 정의한 플래그 전체. 1.0에서 얼린다.
/// `--help`·`--version`은 clap이 붙이는 것이라 여기 없다(`get_arguments()`도
/// 안 돌려준다) — 우리가 실수로 지울 수 있는 건 이 다섯뿐이다.
const SEALED_FLAGS: [&str; 5] = ["team", "date", "lang", "tz", "license"];

#[test]
fn the_command_line_flags_are_exactly_the_sealed_set() {
    let cmd = kbotop::cli::Cli::command();
    let mut actual: Vec<String> = cmd
        .get_arguments()
        .filter_map(|a| a.get_long().map(str::to_string))
        .collect();
    actual.sort();
    let mut sealed: Vec<String> = SEALED_FLAGS.iter().map(|s| s.to_string()).collect();
    sealed.sort();
    assert_eq!(
        actual, sealed,
        "플래그 집합이 바뀌었다. 의도한 변경이면 SEALED_FLAGS와 README 두 판, CHANGELOG를 함께 고칠 것"
    );
}

/// **우리가 정의한 short 옵션은 하나도 없다.** 한 글자를 붙이는 순간 그 글자가
/// 영영 묶이고, 나중에 더 어울리는 쓰임이 생겨도 못 옮긴다(`-t`를 team에 줬다가
/// tz에 주고 싶어지는 식). clap의 `-h`·`-V`는 여기 안 잡힌다.
#[test]
fn we_define_no_short_flags_at_all() {
    let cmd = kbotop::cli::Cli::command();
    let shorts: Vec<char> = cmd.get_arguments().filter_map(|a| a.get_short()).collect();
    assert!(shorts.is_empty(), "short 옵션이 생겼다: {shorts:?}");
}

/// `config.toml`에 사람이 적어 둔 키들. **읽히지 않으면 조용히 기본값으로
/// 돌아가므로**(관용 파싱) 테스트가 없으면 오타 하나로 설정이 죽는다.
#[test]
fn every_documented_config_key_still_loads() {
    // `r##`인 이유: 본문에 `"#ff6600"`이 있어 `r#"…"#`이 거기서 끝나 버린다.
    let sealed = r##"
favorite_team = "lg"
poll_secs = 7
lang = "ja"
timezone = "+09:00"
mouse = true

[theme]
preset = "high-contrast"
accent = "#ff6600"
"##;
    let cfg: Config = toml::from_str(sealed).expect("봉인된 설정을 못 읽었다");
    assert_eq!(cfg.favorite_team.as_deref(), Some("lg"));
    assert_eq!(cfg.poll_secs, 7);
    assert_eq!(cfg.lang.as_deref(), Some("ja"));
    assert_eq!(cfg.timezone.as_deref(), Some("+09:00"));
    assert!(cfg.mouse);
    assert_eq!(cfg.theme.preset, "high-contrast");
    assert_eq!(cfg.theme.accent, "#ff6600");
}

/// **구버전 설정 파일이 그대로 열린다.** v0.27 이전 파일에는 `mouse`가 없고,
/// v0.22 이전에는 `[theme]`가 통째로 없다. 없다고 파일 전체를 버리면 그 사람의
/// 응원 팀·언어까지 함께 날아간다.
#[test]
fn an_older_config_file_still_opens_with_defaults_for_what_it_lacks() {
    let cfg: Config =
        toml::from_str("favorite_team = \"ob\"\npoll_secs = 5\n").expect("구버전 설정을 거절했다");
    assert_eq!(cfg.favorite_team.as_deref(), Some("ob"));
    assert_eq!(cfg.mouse, Config::default().mouse);
    assert_eq!(cfg.theme.preset, Config::default().theme.preset);
}

/// **`[theme]`를 반만 적은 파일도 열린다.** 위 테스트는 `[theme]`가 통째로 없는
/// 경우만 보는데, 손으로 쓴 파일은 `preset`만 적고 `accent`는 안 적기 쉽다
/// (README 예시가 두 줄이라 한 줄만 베끼면 그렇게 된다). `ThemeConfig`의
/// `#[serde(default)]`가 빠지면 그 순간 파일 **전체**가 거절되어 응원 팀·언어까지
/// 함께 날아가는데, 이 경우를 아무도 역직렬화하지 않아 테스트는 전부 초록이었다.
#[test]
fn a_half_written_theme_table_does_not_throw_away_the_rest_of_the_file() {
    let cfg: Config = toml::from_str("favorite_team = \"lg\"\n[theme]\npreset = \"mono\"\n")
        .expect("accent가 없다고 설정 파일 전체를 거절했다");
    assert_eq!(cfg.theme.preset, "mono", "적어 둔 preset은 살아야 한다");
    assert_eq!(
        cfg.theme.accent,
        Config::default().theme.accent,
        "안 적은 accent만 기본값으로"
    );
    assert_eq!(
        cfg.favorite_team.as_deref(),
        Some("lg"),
        "[theme] 때문에 나머지가 날아가면 안 된다"
    );
}

/// 손가락이 외운 키들. 화면별로 **무엇이 일어나야 하는지**까지 본다 —
/// "키가 존재한다"만 보면 동작이 바뀐 걸 못 잡는다.
#[test]
fn the_sealed_key_bindings_still_do_what_they_did() {
    let mut app = App::new(Default::default());
    app.games = vec![game("a"), game("b"), game("c")];

    // j/k·gg/G — 목록 이동
    app.on_key(KeyCode::Char('j'));
    assert_eq!(app.selected, 1, "j");
    app.on_key(KeyCode::Char('k'));
    assert_eq!(app.selected, 0, "k");
    app.on_key(KeyCode::Char('G'));
    assert_eq!(app.selected, 2, "G");
    app.on_key(KeyCode::Char('g'));
    app.on_key(KeyCode::Char('g'));
    assert_eq!(app.selected, 0, "gg");

    // Tab — 탭 전환
    assert_eq!(app.tab, Tab::Games);
    app.on_key(KeyCode::Tab);
    assert_eq!(app.tab, Tab::Standings, "Tab");
    app.on_key(KeyCode::Tab);
    assert_eq!(app.tab, Tab::Games);

    // Enter — 라이브 진입, Esc — 복귀
    app.on_key(KeyCode::Enter);
    assert!(matches!(app.screen, Screen::Live { .. }), "Enter");
    app.on_key(KeyCode::Esc);
    assert!(matches!(app.screen, Screen::List), "Esc");

    // F1/? — 도움말, F2 — 옵션, F9/S — 설정
    for open in [KeyCode::F(1), KeyCode::Char('?')] {
        app.on_key(open);
        assert_eq!(app.top_overlay(), Some(Overlay::Help), "{open:?}");
        app.on_key(KeyCode::Esc);
    }
    app.on_key(KeyCode::F(2));
    assert_eq!(app.top_overlay(), Some(Overlay::Options), "F2");
    app.on_key(KeyCode::Esc);
    for open in [KeyCode::F(9), KeyCode::Char('S')] {
        app.on_key(open);
        assert_eq!(app.top_overlay(), Some(Overlay::Settings), "{open:?}");
        app.on_key(KeyCode::Esc);
    }

    // q/F10 — 종료(true 반환)
    assert!(app.on_key(KeyCode::Char('q')), "q");
    assert!(app.on_key(KeyCode::F(10)), "F10");
}

/// 마우스는 **꺼져 있는 게 기본**이다(v0.31). 이걸 되돌리면 아무 설정도 안 한
/// 사람의 터미널 드래그 선택을 다시 뺏는다.
#[test]
fn the_mouse_stays_off_until_asked() {
    assert!(!Config::default().mouse);
}

fn game(id: &str) -> kbotop::model::Game {
    let team = |c: &str| kbotop::model::Team {
        code: c.into(),
        name: c.into(),
    };
    kbotop::model::Game {
        id: id.into(),
        start: String::new(),
        status: kbotop::model::GameStatus::Live,
        status_label: String::new(),
        home: team("LG"),
        away: team("KT"),
        home_score: Some(0),
        away_score: Some(0),
        away_starter: String::new(),
        home_starter: String::new(),
        stadium: String::new(),
        broadcast: String::new(),
    }
}
