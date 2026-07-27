//! 팀 시즌 성적 오버레이(v0.24). 순위 탭에서 `Enter`로 열고 `Esc`로 닫는다.
//!
//! 순위 응답은 팀마다 64개 필드를 주는데 v0.23까지 순위·승패·최근5만 쓰고
//! 나머지를 버리고 있었다. 순위표 한 줄에는 더 들어갈 자리가 없어(v0.23에서
//! 칼럼 둘을 붙이며 이미 빡빡해졌다) 오버레이로 뺐다.
use crate::app::App;
use crate::model::Standing;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

/// 두 묶음을 나란히 놓는 데 필요한 최소 내부 폭. 이보다 좁으면 세로로 쌓는다.
const WIDTH_FOR_TWO_COLUMNS: u16 = 56;
/// 라벨 칸 — 전각 라벨("퀄리티스타트")이 잘리지 않는 폭.
const LABEL_WIDTH: usize = 14;

/// 지표 한 줄. 값이 문자열인 이유: 비율(0.276)과 개수(81)를 같은 자리에서
/// 오른쪽 정렬하려면 포맷을 먼저 확정해야 한다.
fn row(label: &str, value: String) -> Line<'static> {
    let lab = super::text::ellipsize(label, LABEL_WIDTH);
    let pad = LABEL_WIDTH.saturating_sub(super::text::display_width(&lab));
    Line::from(vec![
        Span::raw(format!("{lab}{}", " ".repeat(pad))),
        Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
    ])
}

/// 비율 지표는 소수 세 자리(야구 관례: .276), 평균자책·WHIP은 두 자리.
fn ratio(v: f32) -> String {
    format!("{v:.3}")
}

fn two_dp(v: f32) -> String {
    format!("{v:.2}")
}

fn batting_lines(l: &crate::ui::i18n::Labels, s: &Standing) -> Vec<Line<'static>> {
    let st = &s.stats;
    vec![
        Line::from(Span::styled(
            l.stats_batting.to_string(),
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        row(l.stat_avg, ratio(st.avg)),
        row(l.stat_obp, ratio(st.obp)),
        row(l.stat_slg, ratio(st.slg)),
        row(l.stat_ops, ratio(st.ops)),
        row(l.stat_runs, st.runs.to_string()),
        row(l.stat_rbi, st.rbi.to_string()),
        row(l.stat_hr, st.homers.to_string()),
        row(l.stat_sb, st.steals.to_string()),
    ]
}

fn pitching_lines(l: &crate::ui::i18n::Labels, s: &Standing) -> Vec<Line<'static>> {
    let st = &s.stats;
    vec![
        Line::from(Span::styled(
            l.stats_pitching.to_string(),
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        row(l.stat_era, two_dp(st.era)),
        row(l.stat_whip, two_dp(st.whip)),
        row(l.stat_qs, st.quality_starts.to_string()),
        row(l.stat_save, st.saves.to_string()),
        row(l.stat_hold, st.holds.to_string()),
        row(l.stat_so, st.strikeouts.to_string()),
        row(l.stat_hr_allowed, st.homers_allowed.to_string()),
        row(l.stat_err, st.errors.to_string()),
    ]
}

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let Some(s) = app.team_stats_target() else {
        return;
    };
    let l = app.labels();

    let batting = batting_lines(l, s);
    let pitching = pitching_lines(l, s);

    // 상자는 **내용 높이에 맞춘다**(v0.24 실행 확인). 다른 오버레이처럼 화면
    // 전체를 덮게 두면 아홉 줄짜리 표가 스무 줄 넘는 빈 상자 안에 떠 있다.
    // 두 칸 배치면 긴 쪽 만큼, 세로로 쌓으면 두 묶음 + 사이 빈 줄만큼.
    let w = area.width.saturating_sub(4).max(1);
    let two_col = w.saturating_sub(2) >= WIDTH_FOR_TWO_COLUMNS;
    let content_h = if two_col {
        batting.len().max(pitching.len())
    } else {
        batting.len() + 1 + pitching.len()
    } as u16;
    let h = (content_h + 2).min(area.height.saturating_sub(2)).max(3);
    let rect = super::help_rect(w, h, area);

    let title = format!(" {} {} ", s.team.name, l.title_team_stats);
    let hint_budget = rect.width.saturating_sub(2) as usize;
    let hint = super::text::ellipsize(l.team_stats_hint, hint_budget);
    let block = Block::bordered().title(title).title_bottom(hint);
    let inner = block.inner(rect);

    f.render_widget(Clear, rect);
    f.render_widget(block, rect);

    // 폭이 남으면 두 묶음을 나란히, 좁으면 세로로 쌓는다. 세로로 쌓을 때는
    // 높이가 모자랄 수 있는데, Paragraph가 잘라 주므로 패닉하지 않는다(무패닉).
    if inner.width >= WIDTH_FOR_TWO_COLUMNS {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);
        f.render_widget(Paragraph::new(batting), cols[0]);
        f.render_widget(Paragraph::new(pitching), cols[1]);
    } else {
        let mut all = batting;
        all.push(Line::from(""));
        all.extend(pitching);
        f.render_widget(Paragraph::new(all), inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Tab};
    use crate::model::{Standing, Team, TeamStats};
    use crate::poller::Update;
    use ratatui::{backend::TestBackend, Terminal};

    fn sample(games: u16) -> Standing {
        Standing {
            rank: 1,
            team: Team {
                code: "SS".into(),
                name: "삼성".into(),
            },
            games,
            wins: 57,
            losses: 35,
            draws: 2,
            win_rate: 0.620,
            game_behind: 0.0,
            last_five: "LWWWL".into(),
            streak: "1패".into(),
            stats: TeamStats {
                avg: 0.276,
                obp: 0.369,
                slg: 0.407,
                ops: 0.776,
                runs: 533,
                rbi: 507,
                homers: 81,
                steals: 71,
                era: 4.06,
                whip: 1.37,
                quality_starts: 40,
                saves: 28,
                holds: 58,
                strikeouts: 694,
                homers_allowed: 87,
                errors: 54,
            },
        }
    }

    fn open_app(games: u16) -> App {
        let mut app = App::new(Default::default());
        app.tab = Tab::Standings;
        app.apply(Update::Standings(vec![sample(games)]));
        app.selected = 0;
        app.on_key(crossterm::event::KeyCode::Enter);
        app
    }

    /// 렌더 후 공백 제거 — ratatui가 전각 문자 뒤에 placeholder 셀을 넣는다
    /// (games.rs·standings.rs와 같은 관례).
    fn render_at(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| render(f, f.area(), app)).unwrap();
        let buf = term.backend().buffer();
        let raw: String = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        raw.chars().filter(|c| !c.is_whitespace()).collect()
    }

    /// 순위 탭에서 Enter를 누르면 그 팀 성적이 뜬다. 타격·투구 양쪽 대표 지표를
    /// 함께 본다 — 한쪽만 보면 묶음 하나가 통째로 빠져도 통과한다.
    #[test]
    fn enter_on_standings_opens_the_team_stats_overlay() {
        let app = open_app(94);
        let text = render_at(&app, 90, 16);
        assert!(text.contains("삼성"), "팀명이 없다:\n{text}");
        for needle in ["0.276", "0.776", "81", "4.06", "1.37", "54"] {
            assert!(text.contains(needle), "{needle} missing:\n{text}");
        }
    }

    /// 경기 전(`games == 0`)에는 열지 않는다 — 성적이 전부 0이라 "기록 없음"과
    /// 구분되지 않는다. 0으로 가득한 상자를 보여주느니 안 여는 편이 낫다.
    #[test]
    fn a_team_with_no_games_played_does_not_open() {
        let app = open_app(0);
        assert!(app.team_stats_target().is_none());
        let text = render_at(&app, 90, 16);
        assert!(!text.contains("0.000"), "빈 성적 상자가 떴다:\n{text}");
    }

    /// 좁으면 두 묶음을 세로로 쌓는다 — 지표가 사라지지 않아야 한다.
    #[test]
    fn a_narrow_overlay_stacks_the_groups_instead_of_dropping_them() {
        let app = open_app(94);
        let text = render_at(&app, 46, 24);
        assert!(text.contains("0.276"), "타격이 사라졌다:\n{text}");
        assert!(text.contains("4.06"), "투구가 사라졌다:\n{text}");
    }

    /// Esc로 닫힌다.
    #[test]
    fn esc_closes_the_overlay() {
        let mut app = open_app(94);
        assert!(app.team_stats_target().is_some());
        app.on_key(crossterm::event::KeyCode::Esc);
        assert!(app.team_stats_target().is_none());
    }
    /// 상자 높이가 내용에 맞는다 — 아홉 줄짜리 표가 화면 전체를 덮는 빈 상자
    /// 안에 떠 있으면 안 된다(v0.24 실행 확인에서 잡힌 것).
    #[test]
    fn the_box_is_only_as_tall_as_its_contents() {
        let app = open_app(94);
        let mut term = Terminal::new(TestBackend::new(90, 40)).unwrap();
        term.draw(|f| render(f, f.area(), &app)).unwrap();
        let buf = term.backend().buffer();

        // 테두리가 그려진 행 수를 센다(위·아래 두 줄 + 사이).
        let border_rows: Vec<u16> = (0..40)
            .filter(|y| {
                (0..90).any(|x| buf[(x, *y)].symbol() == "│" || buf[(x, *y)].symbol() == "┌")
            })
            .collect();
        let height = border_rows.last().unwrap() - border_rows.first().unwrap() + 1;
        assert!(
            height <= 13,
            "상자가 내용({}줄)보다 훨씬 크다: {height}줄",
            9
        );
    }
}
