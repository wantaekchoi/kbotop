use crate::app::{App, Screen, Tab};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// footer 힌트 한 조각. `core`면 폭이 부족해도 우선 유지한다.
pub struct HintItem {
    pub key: &'static str,
    pub label: &'static str,
    pub core: bool,
}

/// `sep`로 조각을 그리디하게(앞에서부터, 맞는 조각만) 이어붙인다. 안 맞는
/// 조각은 건너뛰고 다음 조각을 계속 시도한다(중단하지 않음). 반환값은
/// (결과 문자열, 포함된 조각 개수).
///
/// 주의: "구분자가 넓어질수록 담기는 개수가 줄거나 같다(0칸이 최대)"는
/// 조각이 3개 이하일 때만 성립하는 경험적 성질이며, 이 함수 자체가 보장하는
/// 불변식이 아니다. 4개 이상이면 반례가 있다 — 조각 폭 [2,12,2,3], 예산
/// 14에서 0칸은 2개만 담기지만(폭12 조각 하나로 예산을 다 씀), 1칸/2칸은
/// 폭12 조각이 건너뛰어지고 나머지 셋 중 셋이 담겨 3개가 된다(그리디가
/// "안 맞으면 건너뛰고 계속"이라 이런 역전이 가능하다). 그래서 호출자
/// (`assemble_hints`)는 "0칸으로 채운 개수 = 이 폭에서 담을 수 있는 최대
/// 개수"라고 가정하지 않고, 후보 구분자 전부를 시도해 최댓값을 구한다.
fn greedy_fill(pieces: &[String], sep: &str, width: usize) -> (String, usize) {
    let mut acc = String::new();
    let mut count = 0usize;
    for p in pieces {
        let piece_sep = if acc.is_empty() { "" } else { sep };
        let candidate_w = super::text::display_width(&acc)
            + super::text::display_width(piece_sep)
            + super::text::display_width(p);
        if candidate_w <= width {
            acc.push_str(piece_sep);
            acc.push_str(p);
            count += 1;
        }
    }
    (acc, count)
}

/// 힌트를 폭에 맞춰 조립한다. 핵심 힌트(core=true)를 먼저 채우고, 남는 폭에
/// 부가 힌트를 순서대로 붙이다 안 들어가면 멈춘다. 각 힌트는 "{key} {label}"
/// 형태다. 폭을 절대 넘지 않는다(전각 안전).
///
/// 두 칸("  ") → 한 칸(" ") → 0칸 세 구분자 후보 전부로 core를 채워보고,
/// 각각 몇 개나 들어가는지 구한 뒤 그 최댓값을 "이 폭에서 담을 수 있는 core
/// 최대 개수"로 삼는다. 그다음 그 최댓값과 같은 개수를 담는 후보 중 가장
/// 넓은 구분자를 고른다: 넉넉한 폭에서는 두 칸으로도 최댓값과 같아져 기존과
/// 동일하게 두 칸으로 보이고, 폭이 빠듯해 두 칸으로는 개수가 줄어드는
/// 구간에서는 한 칸으로 내려가 개수를 지키면서도 조각이 붙어버리지 않게
/// 하며, 그마저 개수를 깎는 극단적인 폭에서만 0칸으로 내려간다. 고른
/// 구분자는 optional을 이어붙일 때도 그대로 쓴다(항목마다 구분자 폭이
/// 들쭉날쭉해지지 않도록).
///
/// (0칸이 아니라) 세 후보 전부에서 최댓값을 구하는 이유: "0칸이 최대
/// 포함 개수"라는 성질은 core 3개 이하에서만 성립한다(`greedy_fill` 문서의
/// 반례 참고). core가 4개 이상으로 늘면 더 넓은 구분자가 오히려 더 많이
/// 담는 경우가 있는데, 0칸만 상한으로 삼으면 그 경우를 "개수 미달"로 오판해
/// 더 나쁜(더 적게 담기는 데다 붙기까지 하는) 0칸 결과로 폴백해버린다. 세
/// 후보 전부의 최댓값을 쓰면 core 개수와 무관하게 항상 "가능한 최대 개수 +
/// 그걸 담는 가장 넓은 구분자"가 보장된다.
pub fn assemble_hints(items: &[HintItem], width: usize) -> String {
    let piece = |it: &HintItem| format!("{} {}", it.key, it.label);
    let core: Vec<String> = items.iter().filter(|it| it.core).map(piece).collect();
    let optional: Vec<String> = items.iter().filter(|it| !it.core).map(piece).collect();

    let candidates: Vec<(&str, String, usize)> = ["  ", " ", ""]
        .into_iter()
        .map(|s| {
            let (joined, count) = greedy_fill(&core, s, width);
            (s, joined, count)
        })
        .collect();
    let max_count = candidates
        .iter()
        .map(|(_, _, count)| *count)
        .max()
        .unwrap_or(0);
    let (sep, mut out) = candidates
        .into_iter()
        .find(|(_, _, count)| *count == max_count)
        .map(|(s, joined, _)| (s, joined))
        .unwrap_or(("", String::new()));

    for p in &optional {
        let piece_sep = if out.is_empty() { "" } else { sep };
        let candidate_w = super::text::display_width(&out)
            + super::text::display_width(piece_sep)
            + super::text::display_width(p);
        if candidate_w <= width {
            out.push_str(piece_sep);
            out.push_str(p);
        }
    }
    out
}

/// 화면 상태에 맞는 힌트 조각들. `render`가 폭에 맞춰 조립한다 — 에러가 떠도
/// 힌트는 그대로 남아야 해서(에러 배너가 한 줄을 통째로 먹으면 나가는 키가
/// 사라진다) 조립 대상 자체는 에러 유무와 무관하게 여기서 정해진다.
///
/// ("/ Find"는 아직 미구현이라 힌트에서 뺐다 — help.rs와 동일 사유.)
///
/// 화면(List/Live)과 탭(Games/Standings)에 따라 힌트를 바꾼다 — Live 화면에서
/// "Enter Live"는 이미 진입한 화면이라 no-op이고(app.rs의 Enter 핸들러는
/// Screen::List에서만 동작), 목록으로 돌아가는 유일한 키인 Esc는 어디에도
/// 안내되지 않아 발견 불가능했다. 마찬가지로 app.rs의 Enter 핸들러는
/// `tab == Tab::Games`일 때만 라이브 화면을 여므로(Standings 탭에서는
/// no-op), Standings 탭에서는 "Enter Live"를 보여주지 않는다.
fn hint_items(app: &App) -> Vec<HintItem> {
    let l = app.labels();
    // 커서가 취소·예정 경기에 있으면 Enter는 아무 일도 하지 않는다
    // (`App::can_enter_live` — 문자중계가 없어 영구 "loading"에 갇히는
    // 걸 막는 가드다). 그런데 힌트는 "Enter 중계"라고 계속 말하고 있어,
    // 눌러 본 사람은 앱이 먹통이 된 줄 안다 — 되는 자리에서만 안내한다.
    let enter_is_dead = matches!(app.screen, Screen::List)
        && app.tab == Tab::Games
        && !app
            .games
            .get(app.selected)
            .is_some_and(|g| App::can_enter_live(g.status));
    let items: Vec<HintItem> = match (&app.screen, app.tab) {
        (Screen::List, Tab::Games) => vec![
            HintItem {
                key: "F1",
                label: l.hint_help,
                core: true,
            },
            HintItem {
                key: "F2",
                label: l.hint_options,
                core: false,
            },
            HintItem {
                key: "F9",
                label: l.hint_settings,
                core: false,
            },
            HintItem {
                key: "Tab",
                label: l.hint_switch,
                core: true,
            },
            HintItem {
                key: "o",
                label: l.hint_links,
                core: false,
            },
            HintItem {
                key: "n",
                label: l.hint_news,
                core: false,
            },
            // 목록에서 라이브로 들어가는 건 이 앱의 주 동작이다. 부가
            // 힌트로 두면 80칸에서 **영어·일본어만** 탈락해(한국어는
            // 73칸으로 들어간다) 같은 화면인데 언어에 따라 주 동작을
            // 안내받는 사람과 못 받는 사람이 갈렸다.
            HintItem {
                key: "Enter",
                label: l.hint_live_key,
                core: true,
            },
            HintItem {
                key: "q",
                label: l.hint_quit,
                core: true,
            },
        ],
        (Screen::List, Tab::Standings) => vec![
            HintItem {
                key: "F1",
                label: l.hint_help,
                core: true,
            },
            // 순위 행에서 Enter를 누르면 그 팀의 시즌 성적이 뜬다
            // (v0.24). 그런데 footer에도 도움말에도 없어서 아는 사람만
            // 쓰는 기능이었다 — Esc를 힌트에 넣은 것과 같은 이유로 넣는다.
            HintItem {
                key: "Enter",
                label: l.hint_team_stats,
                core: false,
            },
            HintItem {
                key: "F2",
                label: l.hint_options,
                core: false,
            },
            HintItem {
                key: "F9",
                label: l.hint_settings,
                core: false,
            },
            HintItem {
                key: "Tab",
                label: l.hint_switch,
                core: true,
            },
            HintItem {
                key: "o",
                label: l.hint_links,
                core: false,
            },
            HintItem {
                key: "n",
                label: l.hint_news,
                core: false,
            },
            HintItem {
                key: "q",
                label: l.hint_quit,
                core: true,
            },
        ],
        (Screen::Live { state, .. }, _) => {
            // v0.18 Esc 계단(app.rs와 순서를 맞춘다): ①투구 또는
            // 문자중계 커서가 있으면 그 커서 전용 복귀 라벨(가장 좁은
            // 단계 — 리뷰 M-5: 이전엔 이 둘을 하나로 묶어 문자중계
            // 커서만 있을 때도 투구 전용 문구("전체보기")가 떴다) →
            // ②과거 타석을 보는 중이면 "라이브로"(중간 단계) → ③그
            // 외엔 "뒤로"(화면 이탈, 가장 넓은 단계).
            let back = if app.live_pitch_sel.is_some() {
                l.hint_all_pitches
            } else if app.live_relay_cursor.is_some() {
                l.hint_latest
            } else if app.live_atbat_sel.is_some() {
                l.hint_go_live
            } else {
                l.hint_back
            };
            let mut items = vec![
                HintItem {
                    key: "F1",
                    label: l.hint_help,
                    core: true,
                },
                HintItem {
                    key: "Esc",
                    label: back,
                    core: true,
                },
            ];
            // 돌려볼 과거 타석이 있을 때만 광고한다(at_bats가 1개뿐이면
            // `[`/`]`가 아무 데도 못 가므로 힌트가 거짓 약속이 된다).
            if state.as_ref().is_some_and(|s| s.at_bats.len() > 1) {
                items.push(HintItem {
                    key: "[ ]",
                    label: l.hint_rewind,
                    core: false,
                });
            }
            items.push(HintItem {
                key: "←→",
                label: l.hint_pitch,
                core: false,
            });
            // 문자중계 줄 커서(j/k, v0.18)를 어디에도 안내하지 않아
            // 발견 불가능했다(리뷰 I-5) — 되감을 줄이 있을 때만
            // 광고한다(rewind 힌트와 같은 "거짓 약속 금지" 원칙).
            if state
                .as_ref()
                .is_some_and(|s| !s.active_relay_lines(app.live_atbat_sel).is_empty())
            {
                items.push(HintItem {
                    key: "j/k",
                    label: l.hint_relay,
                    core: false,
                });
            }
            items.push(HintItem {
                key: "q",
                label: l.hint_quit,
                core: true,
            });
            items
        }
    };
    let mut items = items;
    if enter_is_dead || (app.tab == Tab::Standings && !app.team_stats_available()) {
        items.retain(|h| h.key != "Enter");
    }
    items
}

/// htop 기능키 바: 반전 스타일 한 줄. 최근 에러가 있으면 **왼쪽 절반**에 그 내용을
/// 함께 보여줘 화면이 왜 stale인지 알 수 있게 한다.
///
/// 에러가 한 줄을 통째로 먹지 않는다. 예전에는 배너가 폭 전체를 차지해 힌트가
/// 하나도 안 남았는데, 오프라인이 되면 바로 그 순간 `q Quit`이 사라져 **나가는
/// 법이 화면에서 없어졌다**(실측: 100칸에서 180자 URL 하나로 한 줄이 다 찼다).
/// 이제 에러는 폭의 절반까지만 쓰고 나머지는 힌트 몫이다 — `assemble_hints`가
/// 부가 힌트부터 떨어뜨리므로 core(`q Quit` 등)는 좁은 폭에서도 남는다.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let l = app.labels();
    let width = area.width as usize;
    // 설정 파일이 깨진 건 폴링 에러보다 먼저 알려야 한다 — 사용자가 파일을
    // 고칠 때까지 계속 기본값으로 도는 상태이고, 그 사이 저장도 막혀 있다.
    let error = app
        .config_error
        .as_ref()
        .map(|e| format!("{}{e}", l.config_broken))
        .or_else(|| {
            app.last_error
                .as_ref()
                .map(|e| format!("{}{}", l.error_prefix, compact_error(e)))
        });
    // 에러가 쓰는 폭(절반)과 힌트가 쓰는 폭(나머지 - 구분자 두 칸).
    let err_budget = if error.is_some() { width / 2 } else { 0 };
    let hint_budget = if error.is_some() {
        width.saturating_sub(err_budget + HINT_GAP)
    } else {
        width
    };
    let hints = assemble_hints(&hint_items(app), hint_budget);
    let hint_style = Style::default().add_modifier(Modifier::REVERSED);
    let line = match error {
        Some(msg) => Line::from(vec![
            // 긴 에러(HTTP 본문 조각 등)는 예산에 맞춰 정직하게 말줄임한다
            // (§15 오버플로 정책). 전문은 본문 패널이 보여준다.
            Span::styled(super::text::ellipsize(&msg, err_budget), error_style(app)),
            Span::raw(" ".repeat(HINT_GAP)),
            Span::raw(hints),
        ]),
        None => Line::from(hints),
    };
    f.render_widget(Paragraph::new(line).style(hint_style), area);
}

/// 에러 조각과 힌트 사이 간격.
const HINT_GAP: usize = 2;

/// 에러 조각의 스타일. mono는 색을 안 쓴다 — 여기가 게이트를 안 거치고 흰 글자/
/// 빨간 배경을 직접 쓰고 있었다. `theme.rs`의 정책 주석은 예외를 **팀 배지
/// 하나**로만 인정하는데 이 배너가 조용히 새고 있었고, mono 봉인 테스트는 `fg`만
/// 보고 `bg`를 안 봐서 못 잡았다. 색이 빠져도 에러는 굵게 남아 눈에 띈다.
///
/// `remove_modifier(REVERSED)`: 이 조각은 힌트 바(REVERSED) 위에 얹히므로,
/// 빼 주지 않으면 흰 글자/빨간 배경이 반전돼 빨간 글자/흰 배경으로 뒤집힌다.
fn error_style(app: &App) -> Style {
    if app.theme_preset == "mono" {
        Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
            .remove_modifier(Modifier::REVERSED)
    }
}

/// 에러 한 줄을 footer 폭에 맞게 줄인다 — **URL은 호스트만, 원인은 앞으로.**
///
/// ureq의 전송 실패 메시지는 `network error: <URL>: <원인>` 꼴인데 우리 URL은
/// 쿼리스트링까지 180자가 넘는다. 그대로 두면 한 줄 footer를 URL이 다 먹어
/// **220칸에서도 원인이 잘렸다**(실측). 잘리는 건 언제나 뒤쪽이므로 원인을 앞으로
/// 당기고, 어디에 못 닿았는지는 호스트만 괄호로 남긴다. URL이 없는 메시지는
/// 그대로 돌려준다.
fn compact_error(msg: &str) -> String {
    let Some(scheme_at) = msg.find("://") else {
        return msg.to_string();
    };
    // URL 토큰의 경계는 앞뒤 공백. 공백은 ASCII 한 바이트라 인덱스 산술이 안전하다.
    let start = msg[..scheme_at].rfind(' ').map_or(0, |i| i + 1);
    let end = msg[scheme_at..]
        .find(' ')
        .map_or(msg.len(), |i| scheme_at + i);
    let host_start = scheme_at + "://".len();
    let host_end = msg[host_start..end]
        .find(['/', '?', '#'])
        .map_or(end, |i| host_start + i);
    let host = msg[host_start..host_end].trim_end_matches(':');
    let head = &msg[..start];
    // URL 바로 뒤의 `: `는 URL과 원인을 잇던 구분자라 자리를 옮기면 의미가 없다.
    let cause = msg[end..].trim_start_matches([':', ' ']);
    if cause.is_empty() {
        format!("{head}{host}")
    } else {
        format!("{head}{cause} ({host})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Game, GameStatus, RelayLine, Team};
    use ratatui::{backend::TestBackend, Terminal};

    fn items() -> Vec<HintItem> {
        vec![
            HintItem {
                key: "q",
                label: "Quit",
                core: true,
            },
            HintItem {
                key: "F1",
                label: "Help",
                core: true,
            },
            HintItem {
                key: "Tab",
                label: "Switch",
                core: true,
            },
            HintItem {
                key: "o",
                label: "Links",
                core: false,
            },
            HintItem {
                key: "n",
                label: "News",
                core: false,
            },
        ]
    }

    /// 폭이 넉넉하면 전부 붙는다.
    #[test]
    fn assemble_includes_all_when_wide() {
        let s = assemble_hints(&items(), 80);
        for kw in ["Quit", "Help", "Switch", "Links", "News"] {
            assert!(s.contains(kw), "{kw} missing in wide footer: {s}");
        }
    }

    /// 폭이 부족하면 부가 힌트부터 떨어지되, 핵심 힌트는 항상 남는다.
    ///
    /// 폭 24는 core 3개("q Quit"=6 + "F1 Help"=7 + "Tab Switch"=10)가 0칸
    /// 구분자로만 딱 들어가는 밴드다(0칸 합계 23<=24, 1칸 합계 25>24이라 1칸도
    /// 못 들어감). 이 폭에서 라벨이 붙어 나오는 건 버그가 아니라 "core는
    /// 항상 표시"라는 하드 요구를 지키기 위한 수용된 저하다(assemble_hints
    /// 문서 참고) — core를 떨어뜨리느니 붙여서라도 다 보여준다.
    ///
    /// 예전 버전은 `.contains()`와 폭 안전만 봐서, core가 붙어 나오는 회귀도
    /// 그냥 통과시켰다(의도된 저하와 회귀를 구분 못 함). 여기서는 그 사실을
    /// 명시적으로 못박아(정확히 붙은 문자열과 일치하는지) 검증한다 — "여유가
    /// 있으면 구분자로 읽힌다"는 별도로
    /// `assemble_uses_separator_when_core_fits_with_space`가 검증하므로, 둘을
    /// 합치면 "붙음은 이 좁은 밴드에서만, 그리고 core는 항상 유지"가 함께
    /// 보장된다.
    #[test]
    fn assemble_keeps_core_when_narrow() {
        let s = assemble_hints(&items(), 24);
        assert!(
            super::super::text::display_width(&s) <= 24,
            "over width: {s}"
        );
        for core in ["Quit", "Help", "Switch"] {
            assert!(s.contains(core), "core {core} dropped: {s}");
        }
        assert_eq!(
            s, "q QuitF1 HelpTab Switch",
            "width 24 is the exact-fit-with-bare-separator band (core sum 23 <= 24 < 25); \
             if this ever changes, the band moved and this assertion must be consciously \
             updated — don't just loosen it back to a `.contains()` check"
        );
    }

    /// core 3개가 1칸 구분자로 딱 들어가는 폭(25: 6+1+7+1+10=25)에서는 라벨
    /// 사이에 공백이 있어야 한다 — "여유가 있으면 구분자로 읽힌다"는 별도
    /// 검증. `assemble_keeps_core_when_narrow`(폭 24, 붙음이 스펙)와 짝을
    /// 이뤄, 미래에 붙음 회귀가 폭 25 이상으로 번지면 이 테스트가 깨진다.
    #[test]
    fn assemble_uses_separator_when_core_fits_with_space() {
        let s = assemble_hints(&items(), 25);
        assert!(
            super::super::text::display_width(&s) <= 25,
            "over width: {s}"
        );
        for core in ["Quit", "Help", "Switch"] {
            assert!(s.contains(core), "core {core} dropped: {s}");
        }
        assert_eq!(
            s, "q Quit F1 Help Tab Switch",
            "width 25 fits core with a single-space separator; labels must not be glued"
        );
    }

    /// 극단적으로 좁아도 폭을 넘지 않고 패닉하지 않는다(핵심 힌트가 다 못 들어가도).
    #[test]
    fn assemble_never_exceeds_width_even_tiny() {
        for w in 0..12 {
            let s = assemble_hints(&items(), w);
            assert!(super::super::text::display_width(&s) <= w, "w={w}: {s}");
        }
    }

    /// 2칸 구분자로는 다 못 들어가지만(2칸일 때 총 21칸 > 20) 1칸 구분자로는
    /// 정확히 들어가는(총 20칸) 폭을 골라, 폴백이 곧장 0칸(붙임)으로 건너뛰지
    /// 않고 1칸 구분자 단계를 거치는지 확인한다 — 리뷰 지적 재발 방지.
    #[test]
    fn assemble_uses_single_space_before_bare_concat() {
        let core_items = vec![
            HintItem {
                key: "F1",
                label: "Option", // "F1 Option" = 9칸
                core: true,
            },
            HintItem {
                key: "Tab",
                label: "Switch", // "Tab Switch" = 10칸 → 합계 19칸
                core: true,
            },
        ];
        let s = assemble_hints(&core_items, 20);
        assert!(
            super::super::text::display_width(&s) <= 20,
            "over width: {s}"
        );
        // 2칸이면 21칸이라 못 들어가고, 0칸(붙임)이면 "OptionTab"처럼
        // 라벨이 붙어버린다 — 1칸 구분자로 조각 사이가 갈라져 있어야 한다.
        assert!(
            s.contains("Option Tab"),
            "expected a single-space separator, got: {s}"
        );
        assert!(
            !s.contains("  "),
            "expected exactly one space (not the wide 2-space form): {s}"
        );
    }

    /// 반례 회귀 테스트: core 4개(재리뷰 지적)에서 "0칸이 최대 포함 개수"
    /// 가정이 깨지는 조합을 그대로 재현한다. 조각 폭은 "a "=2, "Z
    /// XXXXXXXXXX"=12, "c "=2, "d e"=3(= [2,12,2,3])이고 예산은 14 —
    /// 0칸으로는 폭12 조각 하나로 예산을 다 써 2개만 들어가지만, 1칸/2칸은
    /// 폭12 조각을 건너뛰고 나머지 3개가 들어가 3개가 된다. 고쳐지지 않은
    /// 알고리즘(0칸 개수를 상한으로 삼음)이라면 이 폭에서 상한을 2로 오판해
    /// 더 넓은 구분자 후보(count=3)를 전부 "개수 불일치"로 거부하고 0칸
    /// 폴백(2개, 붙음)으로 떨어진다 — 이 테스트는 대신 3개가 넓은 구분자로
    /// 나뉘어 나오는지 확인한다.
    #[test]
    fn assemble_picks_wider_separator_that_fits_more_items_with_four_core() {
        let core_items = vec![
            HintItem {
                key: "a",
                label: "",
                core: true,
            }, // "a " = 2칸
            HintItem {
                key: "Z",
                label: "XXXXXXXXXX",
                core: true,
            }, // "Z XXXXXXXXXX" = 12칸
            HintItem {
                key: "c",
                label: "",
                core: true,
            }, // "c " = 2칸
            HintItem {
                key: "d",
                label: "e",
                core: true,
            }, // "d e" = 3칸
        ];
        let s = assemble_hints(&core_items, 14);
        assert!(
            super::super::text::display_width(&s) <= 14,
            "over width: {s}"
        );
        // 0칸 폴백(버그 있는 옛 알고리즘)이면 "Z XXXXXXXXXX"만 들어가고 "d e"는
        // 예산 초과로 탈락한다 — "d e"가 살아있다는 건 더 넓은 구분자(3개
        // 포함) 쪽이 선택됐다는 증거다.
        assert!(
            s.contains("d e"),
            "expected the 3-item (wider-separator) packing to win, got: {s}"
        );
        assert!(
            s.contains("c "),
            "expected the 3-item (wider-separator) packing to win, got: {s}"
        );
        // 넓은 구분자로 뽑혔다면 폭12 조각("Z XXXXXXXXXX")은 예산상 함께 못
        // 들어간다(2+2+12=16 > 14) — 즉 이 반례는 "더 적게 담기는 0칸"과
        // "더 많이 담기는 1칸/2칸" 중 하나를 고르는 트레이드오프이고, 고쳐진
        // 알고리즘은 항목 수가 더 많은 쪽(1칸/2칸)을 고른다.
        assert!(
            !s.contains('X'),
            "wider-separator packing should not fit the width-12 piece: {s}"
        );
    }

    fn render_to_string(app: &App) -> String {
        render_to_string_with_width(app, 80)
    }

    fn render_to_string_with_width(app: &App, width: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, 3)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// 힌트 테스트용 경기 하나. footer는 커서가 가리키는 경기의 상태를 보고
    /// "Enter 중계"를 안내할지 정하므로, 목록 화면 테스트는 경기가 있어야 한다.
    fn game_with(status: GameStatus) -> Game {
        Game {
            id: "g".into(),
            start: String::new(),
            status,
            status_label: String::new(),
            home: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            away: Team {
                code: "KT".into(),
                name: "KT".into(),
            },
            home_score: None,
            away_score: None,
            away_starter: String::new(),
            home_starter: String::new(),
            stadium: String::new(),
            broadcast: String::new(),
        }
    }

    #[test]
    fn list_screen_hint_advertises_enter_live_not_esc() {
        let mut app = App::new(Default::default());
        app.games = vec![game_with(GameStatus::Live)];
        // Games 탭 힌트 전부(F1/F2/F9/Tab/o/n/Enter/q)가 들어가려면 81칸이 필요하다
        // (assemble_hints가 부가 힌트 중 우선순위가 가장 낮은 "Enter Live"부터
        // 떨어뜨리므로, 80칸 표준폭에서는 의도적으로 생략될 수 있다 — 반응형
        // footer의 정상 동작이다). 이 테스트의 목적은 화면→힌트 매핑
        // 정확성(Esc가 절대 안 보임, 여유 있으면 Enter Live가 보임)이라
        // 폭을 넉넉히 준다.
        let text = render_to_string_with_width(&app, 100);
        assert!(text.contains("Enter Live"));
        assert!(!text.contains("Esc"));
    }

    #[test]
    fn live_screen_hint_advertises_esc_back_not_enter_live() {
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: "".into(),
                home: Team {
                    code: "LG".into(),
                    name: "LG".into(),
                },
                away: Team {
                    code: "KT".into(),
                    name: "KT".into(),
                },
                home_score: None,
                away_score: None,
                away_starter: String::new(),
                home_starter: String::new(),
                stadium: String::new(),
                broadcast: String::new(),
            },
            state: None,
        };
        let text = render_to_string(&app);
        assert!(text.contains("Esc Back"));
        assert!(!text.contains("Enter Live"));
    }

    /// app.rs의 Enter 핸들러는 `tab == Tab::Games`일 때만 라이브 화면을 연다 —
    /// Standings 탭에서 Enter는 no-op이므로 힌트가 그걸 광고해서는 안 된다.
    #[test]
    fn standings_tab_hint_does_not_advertise_enter_live() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        let text = render_to_string(&app);
        assert!(!text.contains("Enter Live"));
        assert!(!text.contains("Esc"));
    }

    /// 긴 에러는 footer 폭에 맞춰 정직하게 말줄임된다(§15 오버플로 정책) —
    /// 조용한 클리핑이면 '…'가 없어 실패한다.
    #[test]
    fn long_error_is_ellipsized_to_the_footer_width() {
        let mut app = App::new(Default::default());
        app.last_error = Some("x".repeat(200));
        let text = render_to_string(&app);
        assert!(text.contains('…'), "expected honest ellipsis in:\n{text}");
    }

    /// **에러가 떠도 나가는 법은 화면에 남는다.** 예전에는 에러 배너가 한 줄을
    /// 통째로 차지해, 오프라인이 되는 순간 `q Quit`이 사라졌다 — 100칸에서
    /// 실제 요청 URL 하나로 한 줄이 다 찼다(실측). 원인도 함께 남아야 한다:
    /// 힌트만 남기고 에러를 통째로 본문에만 두면 이번엔 footer가 왜 stale인지
    /// 말하지 않는다.
    #[test]
    fn a_long_error_keeps_both_the_quit_hint_and_the_cause() {
        let mut app = App::new(Default::default());
        app.last_error = Some(format!(
            "network error: https://api-gw.sports.naver.com/schedule/games?{}: Dns Failed",
            "fields=basic&".repeat(14)
        ));
        for width in [80, 100, 220] {
            let text = render_to_string_with_width(&app, width);
            assert!(
                text.contains("q Quit"),
                "{width}칸: 에러가 나가는 키를 먹었다:\n{text}"
            );
            assert!(
                text.contains("Dns Failed"),
                "{width}칸: 원인이 잘렸다:\n{text}"
            );
        }
    }

    /// footer는 **원인을 앞에, URL은 호스트만** 남긴다. 잘리는 건 언제나 뒤쪽이라
    /// 원인이 URL 뒤에 있으면 제일 먼저 잘린다 — 220칸에서도 잘렸다(실측).
    #[test]
    fn compact_error_leads_with_the_cause_and_keeps_only_the_host() {
        assert_eq!(
            compact_error(
                "network error: https://api-gw.sports.naver.com/schedule/games?fields=basic&toDate=2026-08-24: Dns Failed: resolve dns name"
            ),
            "network error: Dns Failed: resolve dns name (api-gw.sports.naver.com)"
        );
    }

    /// URL이 없는 메시지(설정 파일 오류 등)는 손대지 않는다.
    #[test]
    fn compact_error_leaves_a_message_without_a_url_alone() {
        assert_eq!(
            compact_error("could not read config"),
            "could not read config"
        );
    }

    /// 선택 중에는 Esc가 "전체보기 복귀"임을 힌트로 알린다 — 상태별 전환 검증.
    #[test]
    fn live_hint_switches_to_all_pitches_while_a_pitch_is_selected() {
        let mut app = App::new(Default::default());
        app.screen = Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: "".into(),
                home: Team {
                    code: "LG".into(),
                    name: "LG".into(),
                },
                away: Team {
                    code: "KT".into(),
                    name: "KT".into(),
                },
                home_score: None,
                away_score: None,
                away_starter: String::new(),
                home_starter: String::new(),
                stadium: String::new(),
                broadcast: String::new(),
            },
            state: None,
        };
        let unselected = render_to_string(&app);
        assert!(unselected.contains("Esc Back"));
        assert!(!unselected.contains("All pitches"));
        app.live_pitch_sel = Some(0);
        let selected = render_to_string(&app);
        assert!(selected.contains("Esc All pitches"));
        assert!(!selected.contains("Esc Back"));
    }

    fn live_screen_with_relay(lines: Vec<String>) -> Screen {
        Screen::Live {
            game: Game {
                id: "g".into(),
                start: "".into(),
                status: GameStatus::Live,
                status_label: "".into(),
                home: Team {
                    code: "LG".into(),
                    name: "LG".into(),
                },
                away: Team {
                    code: "KT".into(),
                    name: "KT".into(),
                },
                home_score: None,
                away_score: None,
                away_starter: String::new(),
                home_starter: String::new(),
                stadium: String::new(),
                broadcast: String::new(),
            },
            state: Some(crate::model::LiveState {
                inning_label: String::new(),
                home: Team {
                    code: "LG".into(),
                    name: "LG".into(),
                },
                away: Team {
                    code: "KT".into(),
                    name: "KT".into(),
                },
                home_score: 0,
                away_score: 0,
                count: crate::model::Count {
                    ball: 0,
                    strike: 0,
                    out: 0,
                },
                bases: crate::model::BaseState {
                    first: false,
                    second: false,
                    third: false,
                },
                pitcher_name: String::new(),
                batter_name: String::new(),
                home_win_rate: None,
                away_win_rate: None,
                relay_log: lines.into_iter().map(RelayLine::plain).collect(),
                current_pitches: vec![],
                next_batter_name: String::new(),
                at_bats: vec![],
                inning_score: Vec::new(),
                batter_line: None,
                pitcher_line: None,
                matchup: String::new(),
            }),
        }
    }

    /// M-5: 문자중계 커서만 있고(투구 선택은 없이) 있을 때 Esc 라벨은 투구
    /// 전용 문구("All pitches")가 아니라 그 상태에 맞는 라벨이어야 한다 —
    /// 이전엔 pitch_sel과 relay_cursor를 하나로 묶어 커서만 있어도 투구
    /// 문구("All pitches")가 떴다(실측 지적).
    #[test]
    fn live_hint_shows_a_relay_specific_label_when_only_the_relay_cursor_is_selected() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_relay(vec!["line-a".into(), "line-b".into()]);
        app.live_relay_cursor = Some(0);
        let text = render_to_string(&app);
        assert!(
            !text.contains("All pitches"),
            "relay-only cursor must not claim to be about pitches: {text}"
        );
        assert!(
            text.contains("Esc Latest"),
            "expected a relay-specific Esc label: {text}"
        );
    }

    /// I-5: 문자중계 커서(j/k)가 footer 어디에도 안내되지 않아 발견 불가능한
    /// 문제 — 되감을 줄이 있을 때는 힌트로 광고한다.
    #[test]
    fn live_hint_advertises_relay_cursor_when_there_are_lines_to_scroll() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_relay(vec!["line-a".into()]);
        let text = render_to_string(&app);
        assert!(text.contains("j/k"), "expected the j/k relay hint: {text}");
    }

    /// rewind 힌트와 같은 원칙(거짓 약속 금지) — 되감을 줄이 하나도 없으면
    /// j/k 힌트를 보여주지 않는다.
    #[test]
    fn live_hint_omits_relay_cursor_hint_when_there_is_nothing_to_scroll() {
        let mut app = App::new(Default::default());
        app.screen = live_screen_with_relay(vec![]);
        let text = render_to_string(&app);
        assert!(
            !text.contains("j/k"),
            "must not advertise j/k with no relay lines to scroll: {text}"
        );
    }

    #[test]
    fn korean_hint_renders_when_lang_ko() {
        let mut app = App::new(Default::default());
        app.lang = crate::ui::i18n::Lang::Ko;
        let text = render_to_string(&app);
        // 전각 문자는 TestBackend에서 다음 셀에 플레이스홀더 공백을 남긴다
        // (games.rs의 renders_full_width_korean_team_names_without_panic과 동일 사유).
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            compact.contains("도움말") && compact.contains("종료"),
            "unexpected: {text}"
        );
    }
    /// **80칸에서 세 언어 모두 `Enter`(라이브 진입)를 안내해야 한다.**
    ///
    /// 부가 힌트였을 때는 한국어(73칸)만 들어가고 영어·일본어는 탈락했다 —
    /// 같은 화면인데 언어에 따라 주 동작을 안내받는 사람과 못 받는 사람이
    /// 갈렸다. 80×24는 표준 터미널 크기라 흔한 경우다.
    #[test]
    fn the_enter_hint_survives_at_eighty_columns_in_every_language() {
        for lang in [
            crate::ui::i18n::Lang::Ko,
            crate::ui::i18n::Lang::En,
            crate::ui::i18n::Lang::Ja,
        ] {
            let mut app = App::new(Default::default());
            app.lang = lang;
            app.games_loaded = true;
            app.games = vec![game_with(GameStatus::Live)];
            // footer는 한 줄짜리 위젯이라 area를 그대로 주면 첫 행에 그려진다.
            let mut term = Terminal::new(TestBackend::new(80, 1)).unwrap();
            term.draw(|f| render(f, f.area(), &app)).unwrap();
            let buf = term.backend().buffer();
            let last: String = (0..80).map(|x| buf[(x, 0)].symbol().to_string()).collect();
            assert!(
                last.contains("Enter"),
                "{lang:?} 80칸 footer에 Enter가 없다: {last:?}"
            );
        }
    }

    /// **되지도 않는 키를 광고하지 않는다.** 취소·예정 경기에 커서를 두면
    /// Enter는 아무 일도 하지 않는다(`App::can_enter_live` — 문자중계가 없어
    /// 영구 "loading"에 갇히는 걸 막는 가드다). 그런데 footer는 계속 "Enter
    /// 중계"라고 말하고 있어서, 눌러 본 사람은 앱이 먹통이 됐다고 읽는다.
    #[test]
    fn the_enter_hint_disappears_on_a_game_that_cannot_be_entered() {
        for status in [GameStatus::Canceled, GameStatus::Scheduled] {
            let mut app = App::new(Default::default());
            app.games_loaded = true;
            app.games = vec![game_with(status)];
            let text = render_to_string_with_width(&app, 100);
            assert!(
                !text.contains("Enter Live"),
                "{status:?} 경기인데 Enter를 광고한다: {text:?}"
            );
        }
        for status in [GameStatus::Live, GameStatus::Final, GameStatus::Suspended] {
            let mut app = App::new(Default::default());
            app.games_loaded = true;
            app.games = vec![game_with(status)];
            let text = render_to_string_with_width(&app, 100);
            assert!(
                text.contains("Enter Live"),
                "{status:?} 경기인데 Enter가 없다: {text:?}"
            );
        }
    }

    fn standing_with_games(games: u16) -> crate::model::Standing {
        crate::model::Standing {
            rank: 1,
            team: Team {
                code: "LG".into(),
                name: "LG".into(),
            },
            games,
            wins: 0,
            losses: 0,
            draws: 0,
            win_rate: 0.0,
            game_behind: 0.0,
            last_five: String::new(),
            streak: String::new(),
            stats: Default::default(),
        }
    }

    /// **순위 탭의 Enter(팀 성적)도 안내한다.** v0.24에 들어온 기능인데 footer
    /// 에도 도움말에도 없어서, README를 읽은 사람만 아는 상태였다.
    #[test]
    fn the_standings_tab_advertises_enter_for_team_stats() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        // 경기 목록의 취소 경기가 순위 탭 힌트까지 지우면 안 된다.
        app.games = vec![game_with(GameStatus::Canceled)];
        app.standings = vec![standing_with_games(90)];
        let text = render_to_string_with_width(&app, 100);
        assert!(
            text.contains("Enter Stats"),
            "순위 탭 Enter가 없다: {text:?}"
        );
    }

    /// 개막 전에는 성적이 전부 0이라 오버레이가 안 열린다(`team_stats_target`).
    /// 그때는 안내도 하지 않는다 — 거짓 약속을 만들지 않는 같은 규칙이다.
    #[test]
    fn the_standings_enter_hint_is_absent_before_any_game_is_played() {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.standings = vec![standing_with_games(0)];
        let text = render_to_string_with_width(&app, 100);
        assert!(
            !text.contains("Enter"),
            "열리지도 않는 Enter를 광고한다: {text:?}"
        );
    }
}
