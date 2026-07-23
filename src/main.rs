use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Frame, Terminal};

use kbotop::app::{App, Screen, Tab};
use kbotop::config;
use kbotop::dateutil::{civil_from_days, days_from_civil, format_civil, kst_days};
use kbotop::poller::{self, Command, Update};
use kbotop::source::naver::NaverSource;
use kbotop::source::DataSource;
use kbotop::ui;

#[derive(Parser)]
#[command(
    name = "kbotop",
    version,
    about = "Watch KBO baseball from your terminal.",
    after_long_help = "Examples:\n  kbotop                     today's games\n  kbotop --date yesterday    also: YYYY-MM-DD, YYYYMMDD, today, tomorrow, +N, -N\n  kbotop --date 2026-05-29   a specific date\n  kbotop --team lg           straight into your team's live game (theme + cheer)\n\nKeys:\n  Tab switch · Enter live · Left/Right pitches · F2 options · o team links · n news · ? help · q quit",
    after_help = "Run with --help for examples and key summary."
)]
struct Cli {
    /// Favorite team code to enter live view directly.
    /// Aliases: lg, kt, ssg/sk, nc, kia/ht, lotte/lt, samsung/ss, hanwha/hh, kiwoom/wo, doosan/ob
    #[arg(long)]
    team: Option<String>,
    /// Date: YYYY-MM-DD, YYYYMMDD, today, yesterday, tomorrow, +N, -N (default: today, KST)
    #[arg(long)]
    date: Option<String>,
    /// UI language: ko | en (default: auto by locale)
    #[arg(long)]
    lang: Option<String>,
}

/// 언어 결정: CLI > config > env(LC_ALL→LANG, "ko" 접두) > En.
fn detect_lang(
    cli: Option<&str>,
    config: Option<&str>,
    env_lang: Option<&str>,
) -> Result<kbotop::ui::i18n::Lang, String> {
    use kbotop::ui::i18n::Lang;
    let parse = |s: &str| match s.to_ascii_lowercase().as_str() {
        "ko" | "kr" | "korean" => Ok(Lang::Ko),
        "en" | "english" => Ok(Lang::En),
        other => Err(format!("unsupported --lang: {other} (use ko or en)")),
    };
    if let Some(s) = cli {
        return parse(s);
    }
    if let Some(s) = config {
        return parse(s);
    }
    Ok(match env_lang {
        Some(e) if e.to_ascii_lowercase().starts_with("ko") => Lang::Ko,
        _ => Lang::En,
    })
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
            let sign = if s.starts_with('-') { -1 } else { 1 };
            return Ok(format_civil(today_days + sign * n));
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
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            let _ = execute!(io::stdout(), crossterm::cursor::Show);
        }
        original_hook(info);
    }));

    enable_raw_mode()?;

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

/// 터미널을 원상 복구한다. `run()`의 성공/실패와 무관하게 항상 호출돼야 한다.
/// 세 단계 모두 best-effort로 시도한 뒤 첫 에러를 반환한다 — 앞 단계가 실패해도
/// 뒤 단계(예: show_cursor)를 건너뛰지 않는다.
fn restore_terminal(term: &mut Tui) -> Result<()> {
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
fn install_term_signal_handler() -> Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGQUIT, Arc::clone(&flag))?;
    Ok(flag)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::load();

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
    let env_lang = std::env::var("LC_ALL")
        .ok()
        .or_else(|| std::env::var("LANG").ok());
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

    let mut term = init_terminal()?;

    let source: Arc<dyn DataSource> = Arc::new(NaverSource::new());
    let (tx_cmd, rx_cmd) = mpsc::channel::<Command>();
    let (tx_up, rx_up) = mpsc::channel::<Update>();
    // config.toml의 poll_secs(하한 3s 적용)를 라이브 뷰 폴링 주기로 흘려보낸다 —
    // cfg 자체는 이후 App::new(cfg)로 이동하므로 여기서 먼저 값을 뽑아둔다.
    let live_poll_secs = cfg.effective_poll_secs();
    // date는 poller::spawn으로 move되므로, App에도 필요한 값은 미리 clone해 둔다.
    let date_for_app = date.clone();
    let handle = poller::spawn(
        source,
        date,
        rx_cmd,
        tx_up,
        live_poll_secs,
        poller::STANDINGS_POLL_SECS,
    );

    let mut app = App::new(cfg);
    app.date = date_for_app.clone();
    app.poll_choice = live_poll_secs;
    app.lang = lang;
    app.fav_code = cli
        .team
        .as_deref()
        .or(app.config.favorite_team.as_deref())
        .and_then(team_code)
        .map(str::to_string);
    let mut watching_id: Option<String> = None;
    // F2 픽커가 바꾼 app.date/app.poll_choice를 폴러에 통지하기 위한 "마지막으로
    // 전송한 값" 기억(watching_id와 동일 패턴) — App은 채널을 모르므로 run()이
    // 매 tick 변화를 감지해 대신 보낸다.
    let mut sent_date = date_for_app;
    let mut sent_poll = live_poll_secs;

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
    watching_id: &mut Option<String>,
    cli: &Cli,
    term_signal: &AtomicBool,
    sent_date: &mut String,
    sent_poll: &mut u64,
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

            if is_games {
                if let Some(code) = auto_team.clone() {
                    if let Some(g) = app
                        .games
                        .iter()
                        .find(|g| g.home.code == code || g.away.code == code)
                        .cloned()
                    {
                        // Canceled/Scheduled 즐겨찾기 게임이면 진입을 보류한다(App::on_key와
                        // 동일한 가드) — 취소가 아니라면 다음 Games 폴링(60s)에서 상태가
                        // 바뀌었을 때 재시도할 수 있도록 auto_team을 그대로 남겨둔다.
                        if App::can_enter_live(g.status) {
                            app.screen = Screen::Live {
                                game: g.clone(),
                                state: None,
                            };
                            let _ = tx_cmd.send(Command::WatchGame(g));
                            auto_team = None;
                        }
                    }
                }
            }
        }

        // 화면 전환에 따른 폴러 명령 동기화.
        let current = app.watched_game().map(|g| g.id.clone());
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

        // Standings 탭이 떠 있는 동안은 조건 없이 매 tick RefreshStandings를 보낸다.
        // 이전엔 `standings.is_empty()`일 때만 보내, 최초 로드 이후엔 W/L·GB가
        // 바뀌어도 세션 내내 스냅샷이 얼어붙었다(버그 수정). 실제 fetch는
        // poller.rs의 시간 게이트(STANDINGS_POLL_SECS=90s)가 코얼레싱하므로, 매 tick
        // 보내도 실제 네트워크 호출은 게이트 주기로만 나간다.
        if app.tab == Tab::Standings {
            let _ = tx_cmd.send(Command::RefreshStandings);
        }

        // 스피너 프레임: fetch가 in-flight인 동안 매 tick(~100ms) 회전.
        if app.fetching {
            app.spinner_frame = app.spinner_frame.wrapping_add(1);
        }

        // 초보용 팁 회전(tips::current)이 참조하는 현재 시각. 매 tick 갱신하면
        // 충분하다 — 1분 단위 회전이라 100ms 해상도는 과분하지만 스피너 갱신과
        // 같은 자리에 두면 별도 타이머 없이 자연히 최신 상태를 유지한다.
        app.now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        term.draw(|f: &mut Frame| ui::draw(f, app))?;

        // 입력(100ms 폴링으로 렌더 갱신 보장).
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press && app.on_key(k.code) {
                    break;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
