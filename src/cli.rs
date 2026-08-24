//! CLI 정의. **lib에 두는 이유는 테스트가 진짜 플래그 목록을 얻기 위해서다.**
//!
//! 예전에는 `main.rs`(바이너리 크레이트) 안에 있어서, 통합 테스트가
//! `clap::CommandFactory`를 못 쓰고 **소스 텍스트를 긁어** 플래그를 셌다
//! (`#[arg(long)]` 다음 줄이 필드일 거라고 가정하는 방식). 그 방식은
//! `#[arg(long = "team-code")]` 같은 이름 오버라이드도, short 옵션도,
//! `-h`/`-V`도 못 본다 — 즉 플래그가 바뀌어도 통과한다.
//!
//! 여기 있으면 `Cli::command()`로 clap이 아는 그대로를 물어볼 수 있다.

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "kbotop",
    version,
    about = "Watch KBO baseball from your terminal.",
    after_long_help = "Examples:\n  kbotop                     today's games\n  kbotop --date yesterday    also: YYYY-MM-DD, YYYYMMDD, today, tomorrow, +N, -N\n  kbotop --date 2026-05-29   a specific date\n  kbotop --team lg           straight into your team's live game (theme + cheer)\n\nKeys:\n  Tab switch · Enter live · Left/Right pitches · F2 options · o team links · n news · ? help · q quit",
    after_help = "Run with --help for examples and key summary."
)]
pub struct Cli {
    /// Favorite team code to enter live view directly.
    /// Aliases: lg, kt, ssg/sk, nc, kia/ht, lotte/lt, samsung/ss, hanwha/hh, kiwoom/wo, doosan/ob
    #[arg(long)]
    pub team: Option<String>,
    /// Date: YYYY-MM-DD, YYYYMMDD, today, yesterday, tomorrow, +N, -N (default: today, KST)
    // allow_hyphen_values: `-N`(어제로 N일)은 값이 하이픈으로 시작해 clap이
    // 플래그로 오인한다 — `--help`가 `-N`을 광고하는데 `--date -1`이 죽던
    // 문제(v0.17 커버리지 작업 중 발견). `--tz`도 같은 이유로 붙어 있다.
    #[arg(long, allow_hyphen_values = true)]
    pub date: Option<String>,
    /// UI language: ko | en | ja (default: auto by locale)
    #[arg(long)]
    pub lang: Option<String>,
    /// Display time zone: auto | kst | +09:00 | -04:00 (default: auto —
    /// detects the system zone; game dates stay on KST since that is when
    /// KBO plays)
    // allow_hyphen_values: 서쪽 시간대는 값이 `-04:00`처럼 하이픈으로 시작해
    // clap이 플래그로 오인한다(미주 전체가 여기 해당) — 실사용 검증에서 잡힘.
    #[arg(long, allow_hyphen_values = true)]
    pub tz: Option<String>,
    /// Print third-party license notices and exit.
    // 아래는 `//`다. clap은 `///`의 **두 번째 문단부터**를 long help 본문으로
    // 쓰므로, 여기 `///`로 적으면 `--help`에 이 문단이 그대로 찍힌다 —
    // 한국어 내부 메모에 `**`와 백틱까지 리터럴로 새어 나갔다. 다른 필드의
    // 설명은 전부 한 문단짜리라 이 함정을 안 밟았고, 여기만 밟았다.
    //
    // 정적 링크 배포물은 의존성의 저작권 표시·라이선스 전문을 함께 배포해야
    // 한다. 그 고지는 릴리스 아카이브에만 들어 있어서, Homebrew·curl 인스톨러·
    // `cargo install`로 받은 사람은 바이너리만 갖고 고지는 못 받는다. 파일을
    // 따라다니게 하는 대신 바이너리가 스스로 뱉게 한다.
    #[arg(long)]
    pub license: bool,
}
