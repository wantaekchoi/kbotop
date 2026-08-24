use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, BeginSynchronizedUpdate, EndSynchronizedUpdate,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Frame, Terminal};

use kbotop::app::{App, MouseAction, Tab};
use kbotop::config;
use kbotop::dateutil::{civil_from_days, days_from_civil, format_civil, kst_days};
use kbotop::poller::{self, Command, Update};
use kbotop::source::naver::NaverSource;
use kbotop::source::rss::RssSource;
use kbotop::source::{DataSource, NewsSource};
use kbotop::ui;

use kbotop::cli::Cli;

/// 언어 결정: CLI > config > env(LC_ALL→LANG, "ko"/"ja" 접두) > En.
/// config는 영속 상태라 모르는 값(구버전에서 저장된 이제 없는 언어)이면 무시하고 env/기본으로 폴백한다 — CLI만 fail-fast.
fn detect_lang(
    cli: Option<&str>,
    config: Option<&str>,
    env_lang: Option<&str>,
) -> Result<kbotop::ui::i18n::Lang, String> {
    use kbotop::ui::i18n::Lang;
    let parse = |s: &str| match s.to_ascii_lowercase().as_str() {
        "ko" | "kr" | "korean" => Ok(Lang::Ko),
        "en" | "english" => Ok(Lang::En),
        "ja" | "japanese" => Ok(Lang::Ja),
        other => Err(format!("unsupported --lang: {other} (use ko, en, or ja)")),
    };
    if let Some(s) = cli {
        return parse(s);
    }
    if let Some(s) = config {
        if let Ok(lang) = parse(s) {
            return Ok(lang);
        }
        // config에 이제 없는 언어(구버전 zh-TW/es 등)는 무시하고 env/기본으로 폴백 —
        // 영속 config가 앱 시작을 막으면 안 된다(CLI만 fail-fast).
    }
    Ok(match env_lang.map(|e| e.to_ascii_lowercase()) {
        Some(ref e) if e.starts_with("ko") => Lang::Ko,
        Some(ref e) if e.starts_with("ja") => Lang::Ja,
        _ => Lang::En,
    })
}

/// POSIX 로케일 사슬에서 언어 힌트를 고른다: `LC_ALL` > `LC_MESSAGES` > `LANG`.
///
/// 가운데가 빠져 있었다 — `LC_MESSAGES`만 지정한 사람은 조용히 영어를 봤다.
/// 빈 값은 "설정 안 됨"이다(POSIX): `LC_ALL= LANG=ko_KR.UTF-8 kbotop`은 흔한
/// 형태인데, 빈 문자열을 그대로 받으면 뒤 단계를 가려 버린다.
///
/// env 접근을 인자로 받아 순수 함수로 둔다 — 테스트가 프로세스 전역 환경변수를
/// 건드리면 병렬로 도는 다른 테스트까지 흔든다.
fn locale_from(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| get(key).filter(|v| !v.is_empty()))
}

/// 팀 별칭 → KBO 내부 코드.
fn team_code(alias: &str) -> Option<&'static str> {
    Some(match alias.to_lowercase().as_str() {
        "lg" => "LG",
        "kt" => "KT",
        "ssg" | "sk" => "SK",
        "nc" => "NC",
        "kia" | "ht" => "HT",
        "lotte" | "lt" => "LT",
        "samsung" | "ss" => "SS",
        "hanwha" | "hh" => "HH",
        "kiwoom" | "wo" => "WO",
        "doosan" | "ob" => "OB",
        _ => return None,
    })
}

/// UTC epoch 초 → KST 기준 "YYYY-MM-DD". kst_today()가 SystemTime::now()로
/// 얻은 값을 넘기는 얇은 wrapper이고, 테스트는 고정된 epoch 초를 직접 넘겨
/// UTC→KST 자정 넘김(연도 롤오버 포함)까지 검증한다.
fn kst_date_from_utc_secs(utc_secs: i64) -> String {
    format_civil(kst_days(utc_secs as u64))
}

/// 자정을 넘겨 KST의 "오늘"이 바뀌었을 때 조회 날짜를 따라가게 한다.
///
/// 앱은 시작할 때 정한 날짜를 세션 내내 들고 있었다 — 헤더 시계는 새 날짜의
/// 시각을 가리키는데 목록은 어제 경기 그대로였고, 화면 어디에도 그 사실이
/// 적히지 않았다(v0.29 상주 리뷰가 지적, v0.30에서 수정). 반대로 F2 픽커나
/// `--date`로 **다른 날을 골라 둔 사용자**의 화면을 마음대로 옮기면 안 되므로,
/// 지금 보고 있는 날짜가 아직 "직전의 오늘"일 때만 새 오늘로 민다.
/// 순수 함수라 자정 경계를 실시간 대기 없이 검증한다.
fn rolled_over_date(shown: &str, prev_today: &str, today: &str) -> Option<String> {
    (today != prev_today && shown == prev_today).then(|| today.to_string())
}

/// 외부 크레이트(chrono) 없이 `SystemTime`만으로 KST 기준 오늘 날짜를 계산한다.
fn kst_today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    kst_date_from_utc_secs(secs)
}

/// --date 입력을 YYYY-MM-DD로 정규화한다. 지원: YYYY-MM-DD, YYYYMMDD,
/// today/yesterday/tomorrow, +N/-N(오늘±N일, KST). 잘못된 입력은 조용히
/// 오늘로 폴백하지 않고 Err — 호출부가 TUI 진입 전에 정직하게 종료한다.
fn resolve_date(input: &str, today_days: i64) -> Result<String, String> {
    let s = input.trim();
    match s.to_ascii_lowercase().as_str() {
        "today" => return Ok(format_civil(today_days)),
        "yesterday" => return Ok(format_civil(today_days - 1)),
        "tomorrow" => return Ok(format_civil(today_days + 1)),
        _ => {}
    }
    if let Some(rest) = s.strip_prefix('+').or_else(|| s.strip_prefix('-')) {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            let n: i64 = rest
                .parse()
                .map_err(|_| format!("day offset too large: {s}"))?;
            let sign: i64 = if s.starts_with('-') { -1 } else { 1 };
            // checked_*로 막는다. 디버그 빌드는 패닉하고 릴리스는 조용히
            // 랩어라운드해 **엉뚱한 날짜로 요청이 나갔다**(둘 다 나쁘다).
            // 그리고 넘치지 않더라도 연도가 네 자리를 벗어나면 거절한다 —
            // `format_civil`은 "2737907008958-07-04" 같은 문자열도 군말 없이
            // 만들어 주고, 그게 그대로 URL에 실린다.
            let days = sign
                .checked_mul(n)
                .and_then(|d| today_days.checked_add(d))
                .ok_or_else(|| format!("day offset too large: {s}"))?;
            let out = format_civil(days);
            if !is_four_digit_year(&out) {
                return Err(format!("day offset too large: {s}"));
            }
            return Ok(out);
        }
    }
    let bytes = s.as_bytes();
    let dashed = s.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-';
    let compact = s.len() == 8;
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 8 && (dashed || compact) {
        let y: i64 = digits[0..4].parse().unwrap();
        let m: i64 = digits[4..6].parse().unwrap();
        let d: i64 = digits[6..8].parse().unwrap();
        // 왕복 변환으로 실존 날짜만 통과시킨다(2월 31일 등 거부).
        if civil_from_days(days_from_civil(y, m, d)) == (y, m, d) {
            return Ok(format!("{y:04}-{m:02}-{d:02}"));
        }
        return Err(format!("not a real calendar date: {s}"));
    }
    Err(format!(
        "unsupported date: {s} (use YYYY-MM-DD, YYYYMMDD, today, yesterday, tomorrow, +N, -N)"
    ))
}

/// 이번 tick에 화면을 다시 그릴 이유가 있는가.
///
/// 유휴일 때 안 그리는 게 목적이지만(실측: 12초 유휴 5,093바이트 → 747바이트,
/// 하루 35MB → 5MB), **빼먹으면 화면이 멈추는 조건**이 있어서 한 자리에 모은다:
/// - `pending` — 폴러 업데이트나 키·마우스가 상태를 바꿨다
/// - `fetching` — 스피너가 도는 중이다(1초에 한 번 움직이면 멈춘 것처럼 보인다)
/// - `second_changed` — 시계와 "N초 전"이 초 단위다. 빼면 둘 다 얼어붙는다
/// - `size_changed` — 창 크기가 바뀌었다
fn should_redraw(pending: bool, fetching: bool, second_changed: bool, size_changed: bool) -> bool {
    pending || fetching || second_changed || size_changed
}

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// raw mode + alternate screen으로 진입해 터미널을 초기화한다.
fn init_terminal() -> Result<Tui> {
    // 패닉 시에도 터미널(raw mode/alternate screen/커서)을 복구한 뒤 기존 훅을 호출한다.
    // release 프로파일도 panic = "unwind"(기본값)를 유지하므로 이 훅은 항상 실행되지만,
    // 훅 실행 시점엔 아직 스택이 풀리는 중이라 Terminal의 Drop(커서 복원)이 돌기 전이므로
    // 커서 Show까지 이 훅에서 직접 처리해야 한다.
    // 정상 종료 경로(restore_terminal)는 그대로 유지되며 이 훅은 패닉 케이스만 보완한다.
    // poller::spawn이 백그라운드 스레드에서 돌리는 소스 호출은 poller.rs의
    // catch_unwind로 이미 패닉을 흡수해 스레드를 살려둔다. 훅 자체는 스레드를
    // 가리지 않고 "어느 스레드가 패닉했든" 실행되므로, 여기서 무조건 raw
    // mode/alt screen/커서를 건드리면 poller 스레드의 (곧 catch_unwind로 회복될)
    // 패닉조차 아직 살아있는 메인 렌더 루프 밑에서 터미널을 망가뜨린다. main
    // 스레드의 패닉일 때만 복구 로직을 실행하고, 로깅용 original_hook 호출은
    // 항상 유지한다.
    let original_hook = std::panic::take_hook();
    let main_id = std::thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() == main_id {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), DisableMouseCapture);
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = execute!(io::stdout(), crossterm::cursor::Show);
        }
        original_hook(info);
    }));

    // TTY가 아니면 여기서 먼저 걸린다. 그대로 두면 `Error: Device not configured
    // (os error 6)`가 나가는데, `kbotop | less`나 CI에서 그걸 보는 사람은 원인을
    // 짐작할 수 없다 — 인자 오류가 `kbotop: ...`으로 나가는 것과 접두사도 다르다.
    if let Err(e) = enable_raw_mode() {
        eprintln!("kbotop: needs an interactive terminal (could not enter raw mode: {e})");
        std::process::exit(2);
    }

    // 이후 단계가 실패하면 이미 켜둔 raw mode/alternate screen을 되돌린 뒤
    // 에러를 반환한다 — 그러지 않으면 main()이 `?`로 즉시 종료돼 터미널이
    // 반쯤 초기화된 채로 남는다.
    let mut out = io::stdout();
    if let Err(e) = execute!(out, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(e.into());
    }

    match Terminal::new(CrosstermBackend::new(out)) {
        Ok(term) => Ok(term),
        Err(e) => {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            Err(e.into())
        }
    }
}

/// `format_civil`이 만든 문자열이 네 자리 연도인가.
///
/// 이 함수는 어떤 큰 수를 줘도 문자열을 만들어 낸다("-25252734927764530-03-11").
/// 실재하지 않는 날짜를 API에 그대로 보내지 않으려면 여기서 막아야 한다.
fn is_four_digit_year(s: &str) -> bool {
    let year = s.split('-').next().unwrap_or("");
    year.len() == 4 && year.chars().all(|c| c.is_ascii_digit())
}

/// Ctrl+C인가.
///
/// raw mode는 ISIG를 끄므로 **SIGINT가 아예 오지 않는다** — 그래서 시그널
/// 핸들러도 이걸 못 잡는다. crossterm은 `Char('c')` + CONTROL로 넘겨주는데,
/// `App::on_key`는 KeyCode만 받아 수식키를 보지 않는다. 즉 아무도 처리하지
/// 않으면 Ctrl+C가 `_ => {}`로 떨어져 **아무 일도 일어나지 않는다**(v1.0 준비
/// 중 pty로 실측했다 — 앱이 그대로 살아 있었다).
///
/// 대문자까지 보는 건 Ctrl+Shift+C를 누르는 사람이 있기 때문이다.
fn is_interrupt(k: &KeyEvent) -> bool {
    k.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('C'))
}

/// 터미널을 원상 복구한다. `run()`의 성공/실패와 무관하게 항상 호출돼야 한다.
/// 세 단계 모두 best-effort로 시도한 뒤 첫 에러를 반환한다 — 앞 단계가 실패해도
/// 뒤 단계(예: show_cursor)를 건너뛰지 않는다.
fn restore_terminal(term: &mut Tui) -> Result<()> {
    // 마우스 캡처는 켜지지 않았을 수도 있다(설정이 끔이거나 미지원 터미널) —
    // 끄는 건 어느 쪽이든 안전하고, 안 풀면 앱이 끝난 뒤에도 터미널이 마우스를
    // 먹는다. 실패는 삼킨다: 아래 세 단계를 막을 이유가 없다.
    let _ = execute!(term.backend_mut(), DisableMouseCapture);
    let r1 = disable_raw_mode();
    let r2 = execute!(term.backend_mut(), LeaveAlternateScreen);
    let r3 = term.show_cursor();
    r1?;
    r2?;
    r3?;
    Ok(())
}

/// SIGTERM/SIGHUP/SIGINT/SIGQUIT 기본 처리(무시 없는 즉시 종료)는 unwind/Drop/패닉
/// 훅 어느 것도 거치지 않아 raw mode/alternate screen/커서를 복구하지 못한 채
/// 터미널을 망가뜨린다. 플래그만 세우는 최소 핸들러를 등록해 run()의 기존
/// 100ms 폴링 루프가 이를 감지하고 정상 종료 경로(restore_terminal)로
/// 빠지게 한다.
/// raw mode가 ISIG를 꺼서 앱 안에서 Ctrl+C(SIGINT)/Ctrl+\(SIGQUIT)를 직접
/// 누르는 경로는 원래 안전하지만, kill(2)로 전달되는 진짜 시그널(프로세스
/// 매니저/IDE stop 버튼/systemd/디버거 등)은 termios ISIG 설정과 무관하게
/// 프로세스 기본 동작을 그대로 트리거한다. signal_hook::consts::TERM_SIGNALS
/// (크레이트 자체가 정의하는 "종료 요청" 표준 그룹)가 정확히
/// `[SIGTERM, SIGQUIT, SIGINT]`이므로 SIGQUIT도 동일하게 등록해야 한다.
///
/// Windows에는 SIGTERM/SIGHUP/SIGQUIT 등가물이 없어 signal-hook 자체가
/// 지원하지 않는다(크레이트도 Cargo.toml에서 `cfg(unix)` 전용 의존성으로
/// 옮겨져 있다). Windows에서 Ctrl+C는 run()의 `is_interrupt`가 받는다 —
/// crossterm이 `Char('c')` + CONTROL로 넘겨주는 것을 거기서 종료로 해석한다.
/// 이 플래그는 항상 false로 유지되고
/// run()의 term_signal 체크가 두 플랫폼에서 동일한 타입으로 컴파일되게 하는
/// 용도로만 존재한다.
#[cfg(unix)]
fn install_term_signal_handler() -> Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGQUIT, Arc::clone(&flag))?;
    Ok(flag)
}

/// Windows: 위 문서 주석 참고 — 등록할 시그널이 없으므로 항상 false인 플래그만
/// 반환한다.
#[cfg(not(unix))]
fn install_term_signal_handler() -> Result<Arc<AtomicBool>> {
    Ok(Arc::new(AtomicBool::new(false)))
}

/// 종료 신호를 본 뒤 이만큼 지나도록 프로세스가 살아 있으면 갇힌 것으로 본다.
/// 정상 종료는 100ms 루프 한 바퀴 + 터미널 복구가 전부라 2초면 넉넉하다.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// 감시 스레드가 깨어나는 주기.
const WATCHDOG_TICK: Duration = Duration::from_millis(100);

/// 종료 신호를 본 시각으로부터 `grace`가 지났는가 — 감시 스레드의 유일한 판단을
/// 시각을 인자로 받는 순수 함수로 떼어 둔다(스레드도 프로세스도 없이 검증되게).
fn should_force_exit(signaled_at: Option<Instant>, now: Instant, grace: Duration) -> bool {
    signaled_at.is_some_and(|t| now.duration_since(t) >= grace)
}

/// **종료 신호를 받고도 메인 루프가 안 끝나면 프로세스를 강제로 끝낸다.**
///
/// crossterm 0.29의 unix 이벤트 소스에는 tty가 EOF에 닿으면 빠져나오지 못하는
/// 루프가 있다(`src/event/source/unix/mio.rs`의 `TTY_TOKEN` 분기):
/// `read()`가 `Ok(0)`을 주면 `read_count > 0`이 아니라 파서를 전진시키지 않는데,
/// `break`도 `return`도 하지 않아 같은 `read()`를 그대로 다시 부른다.
/// `WouldBlock`·`Interrupted`가 아닌 에러(터미널이 사라진 뒤의 `EIO` 등)도
/// 마찬가지로 삼켜서 결과가 같다. 그 안에 들어가면 `event::poll`도
/// `event::read`도 영영 돌아오지 않는다 — 둘 다 이 함수를 지난다.
///
/// 터미널이 사라지면(창을 닫거나 SSH가 끊기면) 커널이 SIGHUP을 보내고
/// [`install_term_signal_handler`]가 건 핸들러가 플래그를 세우지만, 메인
/// 스레드가 저 루프에 갇혀 있으면 **run()으로 제어가 돌아오지 않아 플래그를
/// 볼 기회 자체가 없다.** 실측(2026-08-03): 그 상태로 CPU 코어 하나를 100%
/// 물고 6일 넘게 돌던 프로세스 두 개를 발견했다. 노트북에 띄워 두는 앱이라
/// (v0.33 유휴 렌더 게이트가 겨냥한 바로 그 사용 방식) 이건 조용히 배터리를
/// 태우는 결함이다.
///
/// 그래서 판정을 **메인 스레드 밖**에 둔다. 여기서 신호를 보고 유예 시간이
/// 지나도록 프로세스가 살아 있으면 터미널을 되돌리고 끝낸다. 정상 종료가
/// 먼저 이기면 프로세스와 함께 이 스레드도 사라지므로 아무 일도 일어나지 않는다.
///
/// 업스트림(crossterm)에 고쳐지면 이 스레드는 없어도 되지만, 그때까지는
/// 라이브러리 안쪽 루프를 우리가 깰 방법이 이것뿐이다.
fn spawn_shutdown_watchdog(flag: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut signaled_at: Option<Instant> = None;
        loop {
            std::thread::sleep(WATCHDOG_TICK);
            if signaled_at.is_none() && flag.load(Ordering::Relaxed) {
                signaled_at = Some(Instant::now());
            }
            if should_force_exit(signaled_at, Instant::now(), SHUTDOWN_GRACE) {
                // 터미널이 아직 살아 있다면(SIGTERM 등) 되돌려 놓고 나간다.
                // 이미 사라졌다면 이 쓰기들은 조용히 실패할 뿐이라 무해하다.
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), DisableMouseCapture);
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                let _ = execute!(io::stdout(), crossterm::cursor::Show);
                // 정상 경로도 신호를 받으면 Ok(())로 끝나므로 종료 코드를 맞춘다.
                std::process::exit(0);
            }
        }
    });
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    // **다른 인자를 보기 전에 처리한다.** 고지는 바이너리가 스스로 뱉는 것이라
    // 옆에 무엇이 오든 같아야 하는데, --date 검증이 위에 있던 동안에는
    // `--license --date zzz`가 exit 2, `--license --team zzz`는 고지 출력 후
    // exit 0이었다 — 같은 플래그가 옆 인자에 따라 다르게 동작했다.
    if cli.license {
        print!("{}", include_str!("../THIRD-PARTY.md"));
        return Ok(());
    }
    let (cfg, config_error) = config::load();

    // KST 오늘의 epoch 일수 — kst_today()와 동일 산술(+9h) 공유.
    let today_days = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        kst_days(secs)
    };
    let date = match cli.date.as_deref() {
        None => kst_today(),
        Some(s) => match resolve_date(s, today_days) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("kbotop: {e}");
                std::process::exit(2);
            }
        },
    };
    // 알 수 없는 --team 별칭은 조용히 무시하지 않는다(v0.1.2 리뷰 Minor).
    if let Some(alias) = cli.team.as_deref() {
        if team_code(alias).is_none() {
            eprintln!(
                "kbotop: unknown team alias: {alias} (valid: lg kt ssg/sk nc kia/ht lotte/lt samsung/ss hanwha/hh kiwoom/wo doosan/ob)"
            );
            std::process::exit(2);
        }
    }
    // `--tz`도 fail-fast다. resolve()는 config를 위해 관용적으로 파싱하는데,
    // CLI 값까지 거기 얹으면 `--tz Asia/Seoul`이 조용히 무시된 채 시스템
    // 시간대로 표시된다 — 사용자는 자기가 지정한 대로 보고 있다고 믿는다.
    if let Some(tz) = cli.tz.as_deref() {
        if !kbotop::localtime::is_supported_setting(tz) {
            eprintln!(
                "kbotop: unsupported --tz: {tz} (use auto, kst, or an offset like +09:00; IANA names such as Asia/Seoul are not supported)"
            );
            std::process::exit(2);
        }
    }
    let env_lang = locale_from(|k| std::env::var(k).ok());
    let lang = match detect_lang(
        cli.lang.as_deref(),
        cfg.lang.as_deref(),
        env_lang.as_deref(),
    ) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("kbotop: {e}");
            std::process::exit(2);
        }
    };

    // raw mode 진입 전에 등록해야, 등록 직후~raw mode 진입 사이의 좁은 창에서
    // 신호가 와도 놓치지 않는다.
    let term_signal = install_term_signal_handler()?;
    // 메인 루프가 라이브러리 안쪽 루프에 갇혀 신호를 못 보는 경우의 최후 방어.
    spawn_shutdown_watchdog(Arc::clone(&term_signal));

    let mut term = init_terminal()?;

    let source: Arc<dyn DataSource> = Arc::new(NaverSource::new());
    let news_source: Arc<dyn NewsSource> = Arc::new(RssSource::new());
    let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
    let (tx_up, rx_up) = mpsc::channel::<Update>();
    // config.toml의 poll_secs(하한 3s 적용)를 라이브 뷰 폴링 주기로 흘려보낸다 —
    // cfg 자체는 이후 App::new(cfg)로 이동하므로 여기서 먼저 값을 뽑아둔다.
    let live_poll_secs = cfg.effective_poll_secs();
    // date는 poller::spawn으로 move되므로, App에도 필요한 값은 미리 clone해 둔다.
    let date_for_app = date.clone();
    let handle = poller::spawn(
        source,
        news_source,
        date,
        rx_cmd,
        tx_up,
        poller::PollConfig {
            live_secs: live_poll_secs,
            standings_secs: poller::STANDINGS_POLL_SECS,
            want_tips: lang == kbotop::ui::i18n::Lang::Ko,
        },
    );

    let mut app = App::new(cfg);
    app.config_error = config_error;
    // 표시 시간대는 프로세스 시작 시 1회만 정한다(매 프레임 파일 I/O 금지).
    // CLI > config > TZ > /etc/localtime > KST 순서는 localtime::resolve가 담당.
    app.tz = kbotop::localtime::resolve(
        cli.tz.as_deref().or(app.config.timezone.as_deref()),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    );
    app.date = date_for_app.clone();
    app.poll_choice = live_poll_secs;
    app.lang = lang;
    app.theme_preset = app.config.theme.preset.clone();
    app.theme_accent = app.config.theme.accent.clone();
    app.fav_code = cli
        .team
        .as_deref()
        .or(app.config.favorite_team.as_deref())
        .and_then(team_code)
        .map(str::to_string);
    let mut watching_id: Option<(String, kbotop::model::GameStatus)> = None;
    // F2 픽커가 바꾼 app.date/app.poll_choice를 폴러에 통지하기 위한 "마지막으로
    // 전송한 값" 기억(watching_id와 동일 패턴) — App은 채널을 모르므로 run()이
    // 매 tick 변화를 감지해 대신 보낸다.
    let mut sent_date = date_for_app;
    let mut sent_poll = live_poll_secs;
    // 과거 이닝 요청은 App이 fetching_inning으로 예약하고 여기서 폴러에 통지한다
    // (App은 채널을 모른다 — watched_game/SetDate와 동일 패턴).
    let mut sent_inning: Option<u8> = None;

    let res = run(
        &mut term,
        &mut app,
        &rx_up,
        &tx_cmd,
        &mut watching_id,
        &cli,
        &term_signal,
        &mut sent_date,
        &mut sent_poll,
        &mut sent_inning,
    );

    // 터미널 복구는 run()의 성공 여부와 무관하게 항상 실행한다 — 복구 먼저, 에러 전파는 그 다음.
    let _ = tx_cmd.send(Command::Shutdown);
    let restore_res = restore_terminal(&mut term);
    // 폴러 스레드의 join을 기다리지 않는다: games()/live()/standings() 중 하나가
    // 마침 HTTP 타임아웃(최대 10s, 최악 ~30s) 대기 중일 때 handle.join()을 부르면
    // 터미널은 이미 복구됐는데도 그만큼 프로세스 종료가 지연돼 사용자에게는
    // "q를 눌렀는데 멈춘 것"처럼 보인다. 폴러는 청산이 필요한 상태를 갖지 않고
    // (전송 실패도 `let _ = tx.send(...)`로 흡수) 네트워크 호출은 self-contained이므로
    // join 없이 프로세스를 끝내도 안전하다 — Rust는 main() 반환 시 남은 스레드를
    // 즉시 종료한다.
    drop(handle);

    combine_run_and_restore(res, restore_res)
}

/// res(run() 결과)와 restore_res(restore_terminal() 결과)를 하나의 Result로 합친다.
/// `res.and(restore_res)`는 res가 이미 Err면 restore_res의 Err를 조용히 버린다 —
/// run()은 실패했지만 터미널 복구까지 실패해 raw mode/alt screen/커서가 망가진
/// 채로 남았다는 사실이 사라진다. 순수 함수로 분리해 세 분기를 직접 테스트한다.
fn combine_run_and_restore(res: Result<()>, restore_res: Result<()>) -> Result<()> {
    match (res, restore_res) {
        (Ok(()), r) => r,
        (Err(e), Ok(())) => Err(e),
        (Err(e), Err(re)) => Err(e.context(format!("also failed to restore terminal: {re}"))),
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    term: &mut Tui,
    app: &mut App,
    rx_up: &mpsc::Receiver<Update>,
    tx_cmd: &mpsc::Sender<Command>,
    watching_id: &mut Option<(String, kbotop::model::GameStatus)>,
    cli: &Cli,
    term_signal: &AtomicBool,
    sent_date: &mut String,
    sent_poll: &mut u64,
    sent_inning: &mut Option<u8>,
) -> Result<()> {
    // 팀 지정 시 첫 Games 수신 후 자동 진입 처리 플래그. `--team`이 없으면
    // config.toml의 favorite_team을 대신 쓴다 — 그러지 않으면 config 파일로만
    // 즐겨찾기 팀을 설정한 사용자는 자동 진입이 조용히 동작하지 않는다.
    let mut auto_team = cli
        .team
        .as_deref()
        .or(app.config.favorite_team.as_deref())
        .and_then(team_code)
        .map(str::to_string);

    // 직전 프레임의 화면 식별자(App::view_key). ratatui 0.30에서 화면 전환
    // (Live↔List, Games↔Standings) 시 내부 버퍼와 실제 터미널 상태가 어긋나
    // 이전 화면의 착색 셀이 지워지지 않는 문제(ADR-0007)가 있어, 이 값이
    // 바뀐 프레임에서만 term.clear()를 호출한다. 매 프레임 clear하면 깜빡임이
    // 생기므로 반드시 전환이 실제로 일어난 프레임에서만 불러야 한다.
    let mut last_view_key = app.view_key();
    // 이번 프레임에서 그린 클릭 영역. 렌더가 채우고, 마우스 이벤트가 되묻는다.
    let mut hits = ui::hit::HitMap::default();
    // 지금 캡처가 켜져 있는지. 설정과 어긋나면 아래 루프가 맞춘다.
    let mut mouse_on = false;
    // 직전 tick의 KST 오늘. 이 값이 바뀌는 순간이 자정 넘김이다.
    let mut prev_today = kst_today();
    // **가만히 있으면 다시 그리지 않는다.** 루프는 입력을 100ms마다 깨워 보지만
    // 그때마다 전체 화면을 다시 그릴 이유는 없다 — 화면에서 가장 빠른 것도
    // "N초 전"과 스피너라 1초면 충분하다. 매 tick 그리면 하루 86만 번
    // write+flush에 BSU/ESU 이스케이프가 붙는데(노트북에 띄워 두는 앱이다)
    // 열에 아홉은 직전 프레임과 같은 그림이다.
    //
    // 다시 그리는 조건: ①폴러 업데이트 ②키·마우스 ③초가 바뀜 ④크기가 바뀜.
    let mut dirty = true;
    let mut last_second = 0u64;
    let mut last_size = term.size().unwrap_or_default();

    loop {
        // 외부 SIGTERM/SIGHUP 수신 시 q를 누른 것과 동일하게 정상 종료 경로로
        // 빠진다 — 기본 처리(즉시 프로세스 종료)에 맡기면 터미널 복구가 실행되지
        // 않는다.
        if term_signal.load(Ordering::Relaxed) {
            break;
        }

        // 폴러 업데이트 반영.
        while let Ok(up) = rx_up.try_recv() {
            let is_games = matches!(up, Update::Games(_));
            app.apply(up);
            dirty = true;

            if is_games {
                if let Some(code) = auto_team.clone() {
                    if let Some(g) = kbotop::app::pick_team_game(&app.games, &code).cloned() {
                        // Canceled/Scheduled 즐겨찾기 게임이면 진입을 보류한다(App::on_key와
                        // 동일한 가드) — 취소가 아니라면 다음 Games 폴링(60s)에서 상태가
                        // 바뀌었을 때 재시도할 수 있도록 auto_team을 그대로 남겨둔다.
                        if App::can_enter_live(g.status) {
                            // App::on_key의 Enter 진입과 같은 공통 경로(리뷰 I-3) —
                            // 이전에는 여기만 screen 대입 후 세 선택 리셋을 빠뜨려,
                            // 다른 경기에서 되감기 중이던 선택이 자동 진입한 새
                            // 경기에 그대로 남는 결함이 있었다.
                            app.enter_live(g.clone());
                            let _ = tx_cmd.send(Command::WatchGame(g));
                            auto_team = None;
                        }
                    }
                }
            }
        }

        // 초보용 팁 회전(tips::current)이 참조하는 현재 시각. 매 tick 갱신하면
        // 충분하다 — 1분 단위 회전이라 100ms 해상도는 과분하다. 아래 자정 넘김
        // 판정이 이 값을 쓰므로 SetDate 통지보다 **먼저** 갱신해야 한다.
        app.now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // 자정을 넘겼으면 조회 날짜를 오늘로 민다(아래 SetDate 통지가 폴러에 전달).
        let today = kst_date_from_utc_secs(app.now_secs as i64);
        if let Some(next) = rolled_over_date(&app.date, &prev_today, &today) {
            app.date = next;
        }
        prev_today = today;

        // 화면 전환에 따른 폴러 명령 동기화.
        //
        // **상태까지 함께 본다.** id만 비교하면 같은 경기를 계속 보는 동안
        // Live → Final 전이를 놓치고, 폴러는 진입 시점의 스냅샷을 든 채
        // 끝난 경기를 5초마다 계속 받는다.
        let current = app.watched_game().map(|g| (g.id.clone(), g.status));
        if current != *watching_id {
            match &current {
                Some(_) => {
                    if let Some(g) = app.watched_game().cloned() {
                        let _ = tx_cmd.send(Command::WatchGame(g));
                    }
                }
                None => {
                    let _ = tx_cmd.send(Command::StopWatch);
                }
            }
            *watching_id = current;
        }
        // F2 픽커 적용 감지: App은 채널을 모르므로 여기서 변화를 폴러에 통지한다.
        if app.date != *sent_date {
            let _ = tx_cmd.send(Command::SetDate(app.date.clone()));
            *sent_date = app.date.clone();
        }
        if app.poll_choice != *sent_poll {
            let _ = tx_cmd.send(Command::SetLivePoll(app.poll_choice));
            *sent_poll = app.poll_choice;
        }
        // 되감기가 이닝 경계에 닿아 App이 앞 이닝을 예약했으면 여기서 한 번만 보낸다.
        // 응답이 오면 App이 fetching_inning을 None으로 되돌리므로 같은 이닝을
        // 다시 예약해도(사용자가 또 눌러도) 새 요청으로 나간다.
        if app.fetching_inning != *sent_inning {
            if let Some(inning) = app.fetching_inning {
                if let Some(g) = app.watched_game().cloned() {
                    let _ = tx_cmd.send(Command::FetchInning { game: g, inning });
                }
            }
            *sent_inning = app.fetching_inning;
        }
        // Standings 탭이 떠 있는 동안은 조건 없이 매 tick RefreshStandings를 보낸다.
        // 이전엔 `standings.is_empty()`일 때만 보내, 최초 로드 이후엔 W/L·GB가
        // 바뀌어도 세션 내내 스냅샷이 얼어붙었다(버그 수정). 실제 fetch는
        // poller.rs의 시간 게이트(STANDINGS_POLL_SECS=90s)가 코얼레싱하므로, 매 tick
        // 보내도 실제 네트워크 호출은 게이트 주기로만 나간다.
        if app.tab == Tab::Standings {
            let _ = tx_cmd.send(Command::RefreshStandings);
        }

        // 스피너 프레임: fetch가 in-flight인 동안 매 tick(~100ms) 회전.
        // 도는 동안에는 초 단위 게이트를 건너뛴다 — 스피너는 "지금 뭔가 하고
        // 있다"를 보이는 게 전부라 1초에 한 번 움직이면 멈춘 것처럼 보인다.
        // fetch는 짧으므로 유휴 절감은 그대로 남는다.
        if app.fetching {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }

        let second_changed = app.now_secs != last_second;
        last_second = app.now_secs;
        // 크기는 백엔드에 묻는다(ioctl — DSR과 달리 터미널의 답을 기다리지 않는다).
        let size_now = term.size().unwrap_or(last_size);
        let size_changed = size_now != last_size;
        last_size = size_now;

        if should_redraw(dirty, app.fetching, second_changed, size_changed) {
            dirty = false;
            // 동기화 출력(BSU/ESU): 미지원 터미널은 이스케이프를 무시하므로 안전.
            let _ = execute!(std::io::stdout(), BeginSynchronizedUpdate);
            // 화면이 실제로 전환된 프레임에서만 clear한다(ADR-0007) — 매 프레임
            // clear하면 깜빡임이 생기므로 직전 view_key와 다를 때만 호출한다.
            let view_key = app.view_key();
            if view_key != last_view_key {
                // `Terminal::clear()`를 쓰지 않는다. 그건 지우기 전에 커서 위치를
                // **DSR(ESC[6n)로 터미널에 물어보고** 답을 stdin에서 읽어 되돌린다.
                // 대체 화면에서 커서를 숨겨 둔 이 앱에는 되돌릴 커서가 없는데,
                // 대가는 크다: DSR에 답하지 않는 터미널에서는 2초를 기다린 끝에
                // 에러가 나고 `?`가 앱을 그 자리에서 끝낸다(실측: pty에서 Enter로
                // 화면을 바꾸는 순간 "The cursor position could not be read"로 종료).
                // 응답을 stdin에서 긁어가므로 그 사이 눌린 키를 삼킬 수도 있다.
                // `resize`는 같은 일(화면 지우기 + 백버퍼 리셋으로 전체 재그리기)을
                // DSR 없이 한다 — ADR-0007이 필요로 한 건 그 둘뿐이다.
                let size = term.size()?;
                term.resize(size.into())?;
                last_view_key = view_key;
            }
            term.draw(|f: &mut Frame| ui::draw(f, app, &mut hits))?;
            let _ = execute!(std::io::stdout(), EndSynchronizedUpdate);
        }

        // 마우스 캡처는 설정을 따라간다. F9에서 끈 그 순간 터미널이 드래그
        // 선택·복사를 되찾아야 하므로, 매 프레임 현재 값과 비교해 전환한다.
        // execute! 실패는 삼킨다 — 마우스를 못 켜는 터미널에서도 앱은 돌아야
        // 한다(무패닉·조용한 저하). 그런 터미널은 Event::Mouse를 안 보낼 뿐이다.
        if app.mouse != mouse_on {
            let _ = if app.mouse {
                execute!(std::io::stdout(), EnableMouseCapture)
            } else {
                execute!(std::io::stdout(), DisableMouseCapture)
            };
            mouse_on = app.mouse;
        }

        // 입력(100ms 폴링으로 렌더 갱신 보장).
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(k) => {
                    if k.kind != KeyEventKind::Press {
                        continue;
                    }
                    // Ctrl+C가 먼저다. 이건 앱의 키 바인딩이 아니라 프로세스
                    // 종료 요청이라 on_key로 내려보내지 않는다.
                    if is_interrupt(&k) || app.on_key(k.code) {
                        break;
                    }
                    dirty = true;
                }
                Event::Mouse(m) => {
                    // 누르는 순간이 아니라 **떼는 순간**에 반응한다 — 눌렀다가
                    // 마음이 바뀌어 커서를 옮기고 떼면 아무 일도 안 일어나는 게
                    // 데스크톱 관례다. 드래그·우클릭·가운데 버튼은 여기서 걸러진다.
                    let action = match m.kind {
                        MouseEventKind::Up(MouseButton::Left) => Some(MouseAction::Click),
                        MouseEventKind::ScrollUp => Some(MouseAction::ScrollUp),
                        MouseEventKind::ScrollDown => Some(MouseAction::ScrollDown),
                        _ => None,
                    };
                    if let Some(a) = action {
                        app.on_mouse(hits.at(m.column, m.row), a);
                        dirty = true;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 감시 스레드는 **신호를 보기 전에는 절대 프로세스를 끝내지 않는다.**
    /// 이게 깨지면 멀쩡히 돌던 앱이 2초 만에 혼자 죽는다.
    #[test]
    fn the_watchdog_stays_quiet_until_a_signal_arrives() {
        let now = Instant::now();
        assert!(!should_force_exit(None, now, SHUTDOWN_GRACE));
        assert!(!should_force_exit(
            None,
            now + Duration::from_secs(3600),
            SHUTDOWN_GRACE
        ));
    }

    /// 신호를 봤어도 **유예 시간 안에는 기다린다** — 정상 종료 경로가 이길
    /// 기회를 줘야 터미널 복구·폴러 정리가 제 순서대로 끝난다.
    #[test]
    fn the_watchdog_gives_the_normal_shutdown_path_its_grace_period() {
        let signaled = Instant::now();
        assert!(!should_force_exit(Some(signaled), signaled, SHUTDOWN_GRACE));
        assert!(!should_force_exit(
            Some(signaled),
            signaled + SHUTDOWN_GRACE - Duration::from_millis(1),
            SHUTDOWN_GRACE
        ));
    }

    /// 유예가 지나도 살아 있으면 강제 종료한다 — crossterm의 EOF 루프에 갇혀
    /// 100% CPU로 도는 상태를 끊는 유일한 수단이다(2026-08-03 실측 결함).
    #[test]
    fn the_watchdog_fires_once_the_grace_period_has_passed() {
        let signaled = Instant::now();
        assert!(should_force_exit(
            Some(signaled),
            signaled + SHUTDOWN_GRACE,
            SHUTDOWN_GRACE
        ));
        assert!(should_force_exit(
            Some(signaled),
            signaled + SHUTDOWN_GRACE + Duration::from_secs(10),
            SHUTDOWN_GRACE
        ));
    }

    /// 큰 오프셋은 패닉하거나 엉뚱한 날짜를 만들지 않고 **거절**한다.
    /// 디버그 빌드는 `attempt to add with overflow`로 죽었고, 릴리스는
    /// 랩어라운드해서 조용히 이상한 날짜로 요청을 보냈다.
    #[test]
    fn a_huge_day_offset_is_rejected_instead_of_overflowing() {
        for arg in [
            "+9223372036854775807",
            "-9223372036854775807",
            "+999999999999",
        ] {
            let out = resolve_date(arg, 20_000);
            assert!(out.is_err(), "{arg}를 거절하지 않았다: {out:?}");
        }
    }

    /// 평범한 오프셋은 그대로 동작해야 한다(위 방어가 과하지 않은지).
    #[test]
    fn ordinary_day_offsets_still_work() {
        let base = resolve_date("+0", 20_000).unwrap();
        assert_eq!(resolve_date("today", 20_000).unwrap(), base);
        assert!(resolve_date("-1", 20_000).unwrap() < base);
        assert!(resolve_date("+365", 20_000).unwrap() > base);
    }

    /// Ctrl+C는 종료 요청이다. raw mode가 SIGINT를 막으므로 여기서 못 잡으면
    /// **앱을 빠져나갈 방법이 하나 줄어든다**(pty 실측에서 실제로 안 죽었다).
    #[test]
    fn ctrl_c_is_an_interrupt() {
        let ev = |c: char, m: KeyModifiers| KeyEvent::new(KeyCode::Char(c), m);
        assert!(is_interrupt(&ev('c', KeyModifiers::CONTROL)));
        assert!(
            is_interrupt(&ev('C', KeyModifiers::CONTROL | KeyModifiers::SHIFT)),
            "Ctrl+Shift+C도 종료로 친다"
        );
    }

    /// 수식키 없는 `c`는 그냥 글자다 — 이걸 종료로 치면 미래에 `c` 바인딩을
    /// 못 만든다.
    #[test]
    fn a_bare_c_is_not_an_interrupt() {
        let ev = |c: char, m: KeyModifiers| KeyEvent::new(KeyCode::Char(c), m);
        assert!(!is_interrupt(&ev('c', KeyModifiers::NONE)));
        assert!(!is_interrupt(&ev('c', KeyModifiers::ALT)));
        assert!(!is_interrupt(&ev('q', KeyModifiers::CONTROL)));
    }

    #[test]
    fn kst_today_has_iso_date_shape() {
        let s = kst_today();
        assert_eq!(s.len(), 10, "unexpected length: {s}");
        let bytes = s.as_bytes();
        assert_eq!(bytes[4], b'-', "expected dash at index 4: {s}");
        assert_eq!(bytes[7], b'-', "expected dash at index 7: {s}");
        assert!(s.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        }));
    }

    #[test]
    fn kst_date_from_utc_secs_handles_year_rollover_across_the_kst_offset() {
        // 2026-12-31T23:59:59Z + 9h == 2027-01-01T08:59:59 KST.
        assert_eq!(kst_date_from_utc_secs(1798761599), "2027-01-01");
    }

    #[test]
    fn kst_date_from_utc_secs_handles_epoch_start() {
        assert_eq!(kst_date_from_utc_secs(0), "1970-01-01");
    }

    /// **자정을 넘기면 목록도 오늘로 넘어간다.** v0.29까지는 시작할 때 정한
    /// 날짜를 세션 내내 들고 있어서, 헤더 시계는 새 날짜인데 목록은 어제
    /// 경기였다 — 그것도 아무 표시 없이.
    #[test]
    fn the_shown_date_follows_midnight_when_the_user_did_not_pick_one() {
        assert_eq!(
            rolled_over_date("2026-08-02", "2026-08-02", "2026-08-03").as_deref(),
            Some("2026-08-03")
        );
    }

    /// **골라 둔 날짜는 건드리지 않는다.** `--date`나 F2 픽커로 다른 날을 보고
    /// 있는 사람의 화면을 자정에 마음대로 옮기면 그건 고장이다.
    #[test]
    fn a_date_the_user_picked_survives_midnight() {
        assert_eq!(
            rolled_over_date("2026-05-29", "2026-08-02", "2026-08-03"),
            None
        );
    }

    /// 자정을 안 넘긴 tick(하루 종일 거의 전부)에서는 아무 일도 없어야 한다 —
    /// 매 tick SetDate가 나가면 폴러가 games 게이트를 계속 리셋해 60초 주기가
    /// 무너진다.
    #[test]
    fn nothing_happens_while_the_day_has_not_changed() {
        assert_eq!(
            rolled_over_date("2026-08-02", "2026-08-02", "2026-08-02"),
            None
        );
    }

    #[test]
    fn combine_run_and_restore_returns_restore_result_when_run_ok() {
        let r = combine_run_and_restore(Ok(()), Ok(()));
        assert!(r.is_ok());

        let r = combine_run_and_restore(Ok(()), Err(anyhow::anyhow!("restore boom")));
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("restore boom"));
    }

    #[test]
    fn combine_run_and_restore_preserves_run_error_when_restore_ok() {
        let r = combine_run_and_restore(Err(anyhow::anyhow!("run boom")), Ok(()));
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("run boom"));
        assert!(!msg.contains("also failed to restore terminal"));
    }

    #[test]
    fn combine_run_and_restore_preserves_both_errors_when_both_fail() {
        let r = combine_run_and_restore(
            Err(anyhow::anyhow!("run boom")),
            Err(anyhow::anyhow!("restore boom")),
        );
        let err = r.unwrap_err();
        // 기본(non-alternate) Display는 최상위 context 메시지만 보여주므로,
        // 체인 전체(원래 run() 에러 포함)를 보려면 alternate({:#}) 포맷이 필요하다.
        let msg = format!("{err:#}");
        assert!(msg.contains("run boom"));
        assert!(msg.contains("also failed to restore terminal"));
        assert!(msg.contains("restore boom"));
    }

    #[test]
    fn team_code_maps_known_aliases_case_insensitively() {
        assert_eq!(team_code("lg"), Some("LG"));
        assert_eq!(team_code("SSG"), Some("SK"));
        assert_eq!(team_code("sk"), Some("SK"));
        assert_eq!(team_code("HT"), Some("HT"));
        assert_eq!(team_code("kia"), Some("HT"));
        assert_eq!(team_code("doosan"), Some("OB"));
        assert_eq!(team_code("nope"), None);
    }

    /// 위 테스트가 놓친 별칭들(kt/nc와 각 팀의 두 번째 별칭 kt/nc/lt/ss/hh/wo) —
    /// 커버리지 실측에서 이 가지들만 미실행으로 나왔다.
    #[test]
    fn team_code_covers_remaining_aliases() {
        assert_eq!(team_code("kt"), Some("KT"));
        assert_eq!(team_code("KT"), Some("KT"));
        assert_eq!(team_code("nc"), Some("NC"));
        assert_eq!(team_code("NC"), Some("NC"));
        assert_eq!(team_code("lotte"), Some("LT"));
        assert_eq!(team_code("lt"), Some("LT"));
        assert_eq!(team_code("samsung"), Some("SS"));
        assert_eq!(team_code("ss"), Some("SS"));
        assert_eq!(team_code("hanwha"), Some("HH"));
        assert_eq!(team_code("hh"), Some("HH"));
        assert_eq!(team_code("kiwoom"), Some("WO"));
        assert_eq!(team_code("wo"), Some("WO"));
        assert_eq!(team_code("ob"), Some("OB"));
        assert_eq!(team_code(""), None);
    }

    #[test]
    fn resolve_date_accepts_iso_compact_and_keywords() {
        // 2026-07-23 == days_from_civil(2026, 7, 23)
        let today = days_from_civil(2026, 7, 23);
        assert_eq!(resolve_date("2026-05-29", today).unwrap(), "2026-05-29");
        assert_eq!(resolve_date("20260529", today).unwrap(), "2026-05-29");
        assert_eq!(resolve_date("today", today).unwrap(), "2026-07-23");
        assert_eq!(resolve_date("yesterday", today).unwrap(), "2026-07-22");
        assert_eq!(resolve_date("tomorrow", today).unwrap(), "2026-07-24");
        assert_eq!(resolve_date("-1", today).unwrap(), "2026-07-22");
        assert_eq!(resolve_date("+7", today).unwrap(), "2026-07-30");
        // 월말/연말 경계
        assert_eq!(resolve_date("-23", today).unwrap(), "2026-06-30");
        assert_eq!(resolve_date("+162", today).unwrap(), "2027-01-01");
    }

    #[test]
    fn resolve_date_rejects_bad_input() {
        let today = days_from_civil(2026, 7, 23);
        assert!(resolve_date("2026-02-31", today).is_err()); // 존재하지 않는 날짜
        assert!(resolve_date("05-29", today).is_err());
        assert!(resolve_date("nonsense", today).is_err());
        assert!(resolve_date("2026/05/29", today).is_err());
    }

    /// 커버리지 실측에서 안 덮인 두 가지: (1) `+`/`-` 뒤에 오는 오프셋 숫자가
    /// i64 파싱 범위를 넘는 경우, (2) `+`/`-` 접두는 있지만 뒤가 비어있거나
    /// 숫자가 아니어서 오프셋 분기를 통과하지 못하고 아래 날짜 형식 검사로
    /// 떨어지는 경우(둘 다 최종적으로 길이 불일치라 일반 unsupported 에러로 종결).
    #[test]
    fn resolve_date_rejects_overflowing_and_malformed_offsets() {
        let today = days_from_civil(2026, 7, 23);

        let overflow = resolve_date("+99999999999999999999", today).unwrap_err();
        assert!(
            overflow.contains("day offset too large"),
            "unexpected message: {overflow}"
        );
        let overflow_neg = resolve_date("-99999999999999999999", today).unwrap_err();
        assert!(
            overflow_neg.contains("day offset too large"),
            "unexpected message: {overflow_neg}"
        );

        // 부호만 있고 숫자가 없음 — offset 분기를 통과하지 못하고 fall through.
        assert!(resolve_date("+", today).is_err());
        assert!(resolve_date("-", today).is_err());
        // 부호 뒤에 숫자가 아닌 문자 — 마찬가지로 fall through.
        assert!(resolve_date("+abc", today).is_err());
        assert!(resolve_date("-abc", today).is_err());
    }

    /// POSIX 사슬은 `LC_ALL` > `LC_MESSAGES` > `LANG`이다. 가운데가 빠져 있어
    /// `LC_MESSAGES=ko_KR.UTF-8 LANG=en_US.UTF-8`로 띄우면 영어가 나왔다.
    #[test]
    fn the_locale_chain_puts_lc_messages_between_lc_all_and_lang() {
        // 설정된 변수만 Some을 돌려준다 — 안 그러면 항상 LC_ALL이 이긴다.
        let env = |set: &'static [(&'static str, &'static str)]| {
            move |key: &str| {
                set.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            locale_from(env(&[("LC_MESSAGES", "ko"), ("LANG", "en")])).as_deref(),
            Some("ko"),
            "LC_MESSAGES가 LANG을 이겨야 한다"
        );
        assert_eq!(
            locale_from(env(&[
                ("LC_ALL", "ko"),
                ("LC_MESSAGES", "en"),
                ("LANG", "en")
            ]))
            .as_deref(),
            Some("ko"),
            "LC_ALL이 가장 세다"
        );
        assert_eq!(
            locale_from(env(&[("LANG", "en")])).as_deref(),
            Some("en"),
            "둘 다 없으면 LANG"
        );
    }

    /// 빈 값은 "설정 안 됨"이다(POSIX). `LC_ALL= LANG=ko_KR.UTF-8`은 흔한
    /// 형태인데, 빈 문자열을 그대로 받으면 뒤 단계를 가린다.
    #[test]
    fn an_empty_locale_variable_does_not_shadow_the_next_one() {
        let get = |key: &str| {
            Some(match key {
                "LC_ALL" | "LC_MESSAGES" => String::new(),
                _ => "ko_KR.UTF-8".to_string(),
            })
        };
        assert_eq!(locale_from(get).as_deref(), Some("ko_KR.UTF-8"));
        assert_eq!(locale_from(|_| None), None);
    }

    #[test]
    fn detect_lang_priority_cli_config_env() {
        use kbotop::ui::i18n::Lang;
        assert_eq!(
            detect_lang(Some("en"), Some("ko"), Some("ko_KR.UTF-8")).unwrap(),
            Lang::En
        );
        assert_eq!(
            detect_lang(None, Some("en"), Some("ko_KR.UTF-8")).unwrap(),
            Lang::En
        );
        assert_eq!(
            detect_lang(None, None, Some("ko_KR.UTF-8")).unwrap(),
            Lang::Ko
        );
        assert_eq!(
            detect_lang(None, None, Some("en_US.UTF-8")).unwrap(),
            Lang::En
        );
        assert_eq!(detect_lang(None, None, None).unwrap(), Lang::En);
        assert!(detect_lang(Some("jp"), None, None).is_err()); // fail fast
    }

    /// 일본어 지역화(T10): CLI/config/env 전 경로에서 인식된다.
    #[test]
    fn detect_lang_recognizes_japanese() {
        use kbotop::ui::i18n::Lang;
        assert_eq!(detect_lang(Some("ja"), None, None).unwrap(), Lang::Ja);
        assert_eq!(detect_lang(None, Some("ja"), None).unwrap(), Lang::Ja);
        assert_eq!(
            detect_lang(None, None, Some("ja_JP.UTF-8")).unwrap(),
            Lang::Ja
        );
    }

    /// v0.10 회귀 봉인: config에 남아있는 v0.8/v0.9 시절 언어(zh-TW, es 등)는
    /// 앱 시작을 막지 않고 무시된 뒤 env/기본으로 관용적으로 폴백한다.
    /// CLI는 명시적 사용자 입력이므로 여전히 fail-fast를 유지한다.
    #[test]
    fn detect_lang_config_falls_back_gracefully_for_removed_languages() {
        use kbotop::ui::i18n::Lang;
        // 구버전 config 값(zh-TW)은 무시하고 env로 폴백.
        assert_eq!(
            detect_lang(None, Some("zh-TW"), Some("en_US.UTF-8")).unwrap(),
            Lang::En
        );
        // 구버전 config 값(es)에 env도 없으면 기본(En)으로 폴백.
        assert_eq!(detect_lang(None, Some("es"), None).unwrap(), Lang::En);
        // 여전히 유효한 config 값은 그대로 사용된다(무회귀).
        assert_eq!(detect_lang(None, Some("ko"), None).unwrap(), Lang::Ko);
        // CLI는 여전히 fail-fast: 잘못된 값은 에러.
        assert!(detect_lang(Some("zh-TW"), None, None).is_err());
        // config에 임의의 쓰레기 값이 있어도 무시하고 env로 폴백.
        assert_eq!(
            detect_lang(None, Some("garbage"), Some("ja_JP.UTF-8")).unwrap(),
            Lang::Ja
        );
    }

    /// `--tz`는 `allow_hyphen_values = true`라 `-04:00`처럼 하이픈으로 시작하는
    /// 값도 공백으로 분리된 형태(`--tz -04:00`)로 정상 파싱된다(필드 위 주석이
    /// 설명하는 바로 그 이유).
    #[test]
    fn cli_parses_negative_tz_offset_as_space_separated_value() {
        let cli = Cli::try_parse_from(["kbotop", "--tz", "-04:00"])
            .expect("allow_hyphen_values should accept a hyphen-led tz value");
        assert_eq!(cli.tz.as_deref(), Some("-04:00"));
    }

    /// 대조 사례: `--date`는 `allow_hyphen_values`가 없다. `resolve_date`는
    /// "-N" 형태의 오프셋을 지원하지만(위 `resolve_date_accepts_iso_compact_and_keywords`
    /// 참고), clap 파서 단계에서 공백으로 분리된 `--date -1`은 `-1`을 값이 아니라
    /// 미지의 플래그로 오인해 파싱 자체가 실패한다. `--date=-1`(등호 결합형)은
    /// 정상 동작한다. 이 비대칭은 기존 동작이며, 이 태스크는 main.rs 커버리지
    /// 보강만 하므로 clap 속성을 바꾸지 않는다 — 발견한 그대로 회귀 테스트로
    /// 문서화한다.
    #[test]
    fn cli_date_accepts_negative_offsets_in_both_forms() {
        // --help가 `-N`(어제로 N일)을 광고하므로 둘 다 받아야 한다. 공백
        // 구분 형태가 죽던 걸 v0.17에서 고쳤다(allow_hyphen_values) —
        // --tz도 v0.16에서 같은 이유로 같은 처방을 받았다.
        for args in [
            vec!["kbotop", "--date", "-1"],
            vec!["kbotop", "--date=-1"],
            vec!["kbotop", "--date", "-30"],
        ] {
            let cli = Cli::try_parse_from(args.clone())
                .unwrap_or_else(|e| panic!("{args:?} should parse: {e}"));
            assert!(cli.date.as_deref().is_some_and(|d| d.starts_with('-')));
        }
        // 양수 오프셋·키워드는 그대로.
        assert_eq!(
            Cli::try_parse_from(["kbotop", "--date", "+2"])
                .expect("positive offset")
                .date
                .as_deref(),
            Some("+2")
        );
        assert_eq!(
            Cli::try_parse_from(["kbotop", "--date", "today"])
                .expect("keyword")
                .date
                .as_deref(),
            Some("today")
        );
    }

    /// --help가 예시와 키 요약까지 보여준다 — 초행 사용자의 발견 가능성.
    #[test]
    fn long_help_carries_examples_and_key_summary() {
        use clap::CommandFactory;
        let help = Cli::command().render_long_help().to_string();
        for needle in [
            "Examples:",
            "kbotop --date yesterday",
            "kbotop --team lg",
            "YYYY-MM-DD",
            "tomorrow",
            "Keys:",
            "F2",
        ] {
            assert!(help.contains(needle), "--help missing {needle:?}:\n{help}");
        }
    }

    /// **가만히 있으면 그리지 않는다** — 그게 이 게이트의 전부다.
    #[test]
    fn an_idle_tick_does_not_redraw() {
        assert!(!should_redraw(false, false, false, false));
    }

    /// **초가 바뀌면 반드시 그린다.** 이 조건을 빼는 "최적화"가 가장 그럴듯한데,
    /// 빼면 헤더 시계와 "N초 전"이 그 자리에 얼어붙는다 — 앱이 죽은 것처럼 보인다.
    #[test]
    fn a_new_second_always_redraws_or_the_clock_freezes() {
        assert!(should_redraw(false, false, true, false));
    }

    /// 스피너가 도는 동안에는 초 게이트를 건너뛴다(1초에 한 칸이면 멈춘 것 같다).
    #[test]
    fn a_spinner_in_flight_keeps_animating_between_seconds() {
        assert!(should_redraw(false, true, false, false));
    }

    /// 창 크기 변경과 상태 변화(폴러 업데이트·키)도 각각 단독으로 그린다.
    #[test]
    fn a_resize_or_a_state_change_redraws_on_its_own() {
        assert!(should_redraw(false, false, false, true), "리사이즈");
        assert!(should_redraw(true, false, false, false), "상태 변화");
    }
}
