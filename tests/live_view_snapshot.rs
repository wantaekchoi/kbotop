//! 라이브 화면 렌더 스냅샷 하니스 (v19a 리뷰 M-3).
//!
//! **기본적으로 실행되지 않는다** — `cargo test --test live_view_snapshot --
//! --ignored --nocapture`로만 돌린다(무겁다: 수천 개 조합을 실제로 렌더한다).
//!
//! # 왜 이 파일이 있나
//! v19a(라이브 화면 ViewModel/렌더 분리)를 검증할 때, 리팩터 전/후 라이브
//! 화면을 34,020개 조합(크기×상태×언어×프리셋×`game.start`×선택×`now_secs`)
//! 으로 렌더해 셀 단위(심볼+fg+bg+수식자)로 덤프하고 `cmp`로 완전 일치를
//! 확인했다(1,100,897,378 바이트 동일 — task-v19a-report.md §4-1). 그 하니스를
//! 검증 후 지웠더니 다음 리뷰가 재현할 수 없었다(v19a 리뷰 M-3). 이 파일은 그
//! 방법론을 다시 실행 가능하게 남겨 둔다 — 존/측면 뷰 분리, 자매 앱 등 다음
//! 리팩터도 같은 증명이 필요할 것이다.
//!
//! 조합 수는 원래 증명(34,020)보다 작다(빠른 반복을 우선) — 필요하면 아래
//! 배열에 값을 추가해 늘리면 된다. 방법론(축 구성 + 셀 단위 덤프 + 파일 비교)
//! 은 동일하다.
//!
//! # 사용법 (before/after 비교)
//! ```text
//! # 리팩터 전 커밋에서:
//! SNAPSHOT_OUT=/tmp/live_before.txt cargo test --test live_view_snapshot -- --ignored --nocapture
//! # 리팩터 후 커밋에서:
//! SNAPSHOT_OUT=/tmp/live_after.txt cargo test --test live_view_snapshot -- --ignored --nocapture
//! cmp /tmp/live_before.txt /tmp/live_after.txt && echo IDENTICAL
//! ```
//! `SNAPSHOT_OUT`을 생략하면 `target/live_view_snapshot.txt`에 쓴다.

use kbotop::app::{App, Screen};
use kbotop::model::{Game, GameStatus, LiveState, Team};
use kbotop::source::naver::map;
use kbotop::ui::i18n::Lang;
use ratatui::{backend::TestBackend, Terminal};
use std::io::{BufWriter, Write};

const RELAY: &str = include_str!("fixtures/relay_20260719KTLG.json");

fn team(code: &str, name: &str) -> Team {
    Team {
        code: code.into(),
        name: name.into(),
    }
}

/// 매 조합에서 다시 파싱한다 — 상태를 공유하면 한 조합의 손질(예:
/// `at_bats.clear()`)이 다른 조합으로 새어 나간다.
fn fresh_state() -> LiveState {
    map::live_from_relay(RELAY, team("LG", "LG"), team("KT", "KT")).unwrap()
}

/// 문자중계·투구가 하나도 없던 시절(v0.18 이전) 상태로 손질한 응답을 흉내
/// 낸다 — `active_pitches`/`active_relay_lines`/`latest_pitch_time`의
/// current_pitches/relay_log 레거시 폴백 경로를 자극한다(v19a 리뷰 M-4가
/// 지적한 "빠진 축" 중 하나).
fn legacy_no_at_bats_state() -> LiveState {
    let mut s = fresh_state();
    s.at_bats.clear();
    s
}

#[derive(Clone, Copy)]
struct Selection {
    atbat_sel: Option<i64>,
    pitch_sel: Option<usize>,
    relay_cursor: Option<usize>,
}

#[test]
#[ignore]
fn live_view_render_matrix_is_stable_for_before_after_diffing() {
    let out_path = std::env::var("SNAPSHOT_OUT")
        .unwrap_or_else(|_| "target/live_view_snapshot.txt".to_string());
    let mut out = BufWriter::new(std::fs::File::create(&out_path).expect("create snapshot file"));

    // 크기: 존 경계(70/69 폭)와 최소 크기(20x10)를 포함한다(v0.19a 리뷰 §4 근거).
    let sizes: [(u16, u16); 4] = [(100, 30), (70, 20), (69, 20), (20, 10)];
    let statuses = [GameStatus::Live, GameStatus::Final, GameStatus::Suspended];
    let langs = [Lang::Ko, Lang::En, Lang::Ja];
    let presets = ["default", "high-contrast", "mono", "unknown-preset"];
    let starts = ["2026-07-19T18:30:00", "", "not-a-date"];
    // 최신 타석 seq는 fixture 실측상 87보다 크다(다른 live_vm 테스트가 이미
    // seq=87 과거 타석을 기준으로 쓴다) — 아래에서 실제 seq로 다시 검증한다.
    let selections = [
        Selection {
            atbat_sel: None,
            pitch_sel: None,
            relay_cursor: None,
        }, // 라이브, 선택 없음
        Selection {
            atbat_sel: None,
            pitch_sel: Some(0),
            relay_cursor: None,
        }, // 라이브 + 투구 선택
        Selection {
            atbat_sel: Some(87),
            pitch_sel: None,
            relay_cursor: None,
        }, // 되감기
        Selection {
            atbat_sel: Some(87),
            pitch_sel: Some(1),
            relay_cursor: Some(1),
        }, // 되감기 + 투구 + 중계 커서
        Selection {
            atbat_sel: Some(9999),
            pitch_sel: None,
            relay_cursor: None,
        }, // stale seq(응답에 없음)
        Selection {
            atbat_sel: None,
            pitch_sel: None,
            relay_cursor: Some(0),
        }, // 라이브 + 중계 커서만
        // v0.19 연동 축: 커서가 **투구 줄**을 가리키는 경우와, 투구가 아닌
        // 줄(0번 = 타자 등장 안내)을 가리켜 투구 선택이 풀리는 경우. 후자는
        // pitch_sel이 함께 들어와도 커서가 이긴다는 규칙까지 렌더로 덮는다.
        Selection {
            atbat_sel: None,
            pitch_sel: None,
            relay_cursor: Some(2),
        },
        Selection {
            atbat_sel: None,
            pitch_sel: Some(2),
            relay_cursor: Some(0),
        },
    ];
    let now_secs_variants: [u64; 3] = [0, 1_753_000_000, 100];
    // 데이터 변형: 정상 fixture / at_bats가 비어 레거시 폴백을 타는 상태
    // (v19a 리뷰 M-4).
    let data_variants: [fn() -> LiveState; 2] = [fresh_state, legacy_no_at_bats_state];

    let mut combos: u64 = 0;
    for &(w, h) in &sizes {
        for &status in &statuses {
            for &lang in &langs {
                for &preset in &presets {
                    for &start in &starts {
                        for sel in &selections {
                            for &now_secs in &now_secs_variants {
                                for make_state in &data_variants {
                                    let state = make_state();
                                    let game = Game {
                                        id: "20260719KTLG02026".into(),
                                        start: start.into(),
                                        status,
                                        status_label: state.inning_label.clone(),
                                        home: team("LG", "LG"),
                                        away: team("KT", "KT"),
                                        home_score: Some(state.home_score),
                                        away_score: Some(state.away_score),
                                    };
                                    let mut app = App::new(Default::default());
                                    app.lang = lang;
                                    app.theme_preset = preset.into();
                                    app.now_secs = now_secs;
                                    app.live_atbat_sel = sel.atbat_sel;
                                    app.live_pitch_sel = sel.pitch_sel;
                                    app.live_relay_cursor = sel.relay_cursor;
                                    app.screen = Screen::Live {
                                        game,
                                        state: Some(state),
                                    };

                                    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                                    term.draw(|f| {
                                        kbotop::ui::live::render(f, f.area(), &app);
                                    })
                                    .unwrap();

                                    for cell in term.backend().buffer().content() {
                                        writeln!(
                                            out,
                                            "{}|{:?}|{:?}|{:?}",
                                            cell.symbol(),
                                            cell.fg,
                                            cell.bg,
                                            cell.modifier
                                        )
                                        .unwrap();
                                    }
                                    combos += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    out.flush().unwrap();
    eprintln!("live_view_snapshot: rendered {combos} combinations to {out_path}");
    assert!(combos > 0);
}
