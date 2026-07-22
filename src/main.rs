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
use kbotop::poller::{self, Command, Update};
use kbotop::source::naver::NaverSource;
use kbotop::source::DataSource;
use kbotop::ui;

#[derive(Parser)]
#[command(
    name = "kbotop",
    version,
    about = "Watch KBO baseball from your terminal."
)]
struct Cli {
    /// Favorite team code to enter live view directly (lg, kt, ssg, ...)
    #[arg(long)]
    team: Option<String>,
    /// Query date YYYY-MM-DD (default: today, KST)
    #[arg(long)]
    date: Option<String>,
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

/// civil_from_days (Howard Hinnant): UTC epoch(1970-01-01)로부터 지난 날짜 수
/// (`days`)를 그레고리력 (y, m, d)로 변환하는 순수 함수. kst_today()는 이
/// 함수를 감싸는 얇은 wrapper일 뿐이라, 여기서 알려진 경계값(윤년 2/29,
/// epoch 0)으로 직접 검증할 수 있다 — 출력 "모양"(길이/대시 위치)만 보는
/// 테스트로는 이 산술의 off-by-one(예: `days` 계산이 하루 밀림)을 못 잡는다.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// UTC epoch 초 → KST 기준 "YYYY-MM-DD". kst_today()가 SystemTime::now()로
/// 얻은 값을 넘기는 얇은 wrapper이고, 테스트는 고정된 epoch 초를 직접 넘겨
/// UTC→KST 자정 넘김(연도 롤오버 포함)까지 검증한다.
fn kst_date_from_utc_secs(utc_secs: i64) -> String {
    let secs = utc_secs + 9 * 3600; // KST = UTC+9
    let days = secs.div_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
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
    let date = cli.date.clone().unwrap_or_else(kst_today);

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
    let handle = poller::spawn(source, date, rx_cmd, tx_up, live_poll_secs);

    let mut app = App::new(cfg);
    let mut watching_id: Option<String> = None;

    let res = run(
        &mut term,
        &mut app,
        &rx_up,
        &tx_cmd,
        &mut watching_id,
        &cli,
        &term_signal,
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
        if app.tab == Tab::Standings && app.standings.is_empty() {
            let _ = tx_cmd.send(Command::RefreshStandings);
        }

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
    fn civil_from_days_converts_known_boundary_dates() {
        // epoch 0 == 1970-01-01.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2024-02-29 (윤년) == 19782 days since epoch (python: (date(2024,2,29) - date(1970,1,1)).days).
        assert_eq!(civil_from_days(19782), (2024, 2, 29));
        // 2027-01-01 == 20819 days since epoch.
        assert_eq!(civil_from_days(20819), (2027, 1, 1));
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
}
