//! 좌표 → 위젯. 마우스는 렌더와 **반대 방향**의 질문을 한다.
//!
//! 렌더는 레이아웃을 계산하고 버린다. 클릭을 처리하려면 그 좌표가 무엇이었는지
//! 되돌려야 하는데, 방법은 둘이다 — ①이벤트 쪽에서 레이아웃을 다시 계산한다
//! ②렌더가 그리면서 남긴다.
//!
//! ②를 택했다. 계산이 순수 함수라 ①도 가능하지만, **렌더와 히트 테스트가 각자
//! 계산하면 언젠가 어긋난다** — v0.25에서 라인스코어 칸 폭을 두 곳이 따로 정하다
//! 헤더가 값과 어긋난 적이 있다. 그린 좌표를 그대로 쓰면 어긋날 방법이 없다.
//!
//! 등록된 영역은 **그 프레임에서만 유효하다**. 폴링이 목록을 갈아 끼워도 다음
//! 프레임에서 히트맵이 함께 갱신되므로, 여기 담긴 인덱스는 항상 방금 그린 화면의
//! 것이다.

use crate::app::Tab;
use ratatui::layout::{Position, Rect};

/// 클릭할 수 있는 것. 키로 되는 걸 전부 옮기지 않고, **위치를 눈으로 찍는 게
/// 키보다 빠른 것**만 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// 헤더의 탭 이름.
    Tab(Tab),
    /// 경기 목록의 한 행(그 프레임에 그린 `app.games` 인덱스).
    GameRow(usize),
    /// 순위표의 한 행(그 프레임에 그린 `app.standings` 인덱스).
    StandingRow(usize),
    /// 문자중계 한 줄(그 프레임에 그린 표시 줄 인덱스).
    RelayLine(usize),
    /// 존·측면·투구 범례를 아우르는 영역. **휠로만** 투구를 넘긴다.
    ///
    /// 범례("1S 145km  2B 137km")의 공 하나하나를 클릭 대상으로 삼으려면 wrap된
    /// span의 좌표를 알아야 하는데, 그건 ratatui의 줄바꿈을 우리가 다시 구현하는
    /// 일이다(v0.15에서 `line_rows`로 한 번 해 봤고, 재구현은 어긋나기 쉽다).
    /// 투구를 하나씩 넘기는 건 `←`/`→`가 이미 잘하므로 휠로 족하다.
    PitchNav,
}

/// 이번 프레임에서 그린 클릭 가능 영역들.
///
/// 나중에 등록한 것이 이긴다 — 오버레이는 아래 화면 위에 그려지므로, 겹치면
/// 나중에 그린 쪽이 사용자가 실제로 보고 누른 것이다.
#[derive(Default)]
pub struct HitMap {
    zones: Vec<(Rect, Zone)>,
}

impl HitMap {
    /// 빈 영역(폭이나 높이가 0)을 걸러내는 검사는 **두지 않는다**. 뮤테이션으로
    /// 확인해 보니 `Rect::contains`가 빈 사각형에 대해 언제나 false라 검사가
    /// 있으나 없으나 결과가 같았다 — 아래 `an_empty_area_is_not_clickable`이
    /// 그 성질을 봉인한다(ratatui가 동작을 바꾸면 그 테스트가 먼저 깨진다).
    pub fn push(&mut self, area: Rect, zone: Zone) {
        self.zones.push((area, zone));
    }

    pub fn clear(&mut self) {
        self.zones.clear();
    }

    /// 표 본문의 각 행을 등록한다. 본문은 **테두리(1) + 헤더 행(1)** 아래부터
    /// 시작하고, 영역 밖으로 나가는 행은 화면에 없으므로 등록하지 않는다.
    ///
    /// 경기 목록과 순위표가 이 계산을 각자 복사해 갖고 있었다 — 지금은 같지만
    /// 한쪽만 고치면 조용히 갈린다. v0.24가 팀 성적 게이트를 한 함수로 모은
    /// 것과 같은 이유로 여기 하나만 둔다.
    pub fn push_table_rows(
        &mut self,
        area: Rect,
        offset: usize,
        len: usize,
        zone: impl Fn(usize) -> Zone,
    ) {
        const HEAD: u16 = 2; // 위 테두리 + 헤더 행
        let body_h = area.height.saturating_sub(HEAD + 1); // 아래 테두리
        for row in 0..body_h {
            let idx = offset + row as usize;
            if idx >= len {
                break;
            }
            let r = Rect::new(
                area.x + 1,
                area.y + HEAD + row,
                area.width.saturating_sub(2),
                1,
            );
            self.push(r, zone(idx));
        }
    }

    /// 그 좌표에 있는 것. 겹치면 마지막에 등록된 것(= 위에 그려진 것).
    pub fn at(&self, x: u16, y: u16) -> Option<Zone> {
        let p = Position::new(x, y);
        self.zones
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|(_, z)| *z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn a_point_inside_finds_its_zone() {
        let mut m = HitMap::default();
        m.push(r(2, 3, 10, 1), Zone::GameRow(4));
        assert_eq!(m.at(2, 3), Some(Zone::GameRow(4)));
        assert_eq!(m.at(11, 3), Some(Zone::GameRow(4)));
    }

    /// 경계 바로 밖은 잡히지 않아야 한다 — 한 칸 어긋난 클릭이 옆 행을 고르면
    /// 사용자는 자기가 뭘 눌렀는지 알 수 없다.
    #[test]
    fn a_point_outside_finds_nothing() {
        let mut m = HitMap::default();
        m.push(r(2, 3, 10, 1), Zone::GameRow(4));
        assert_eq!(m.at(12, 3), None, "오른쪽 경계 밖");
        assert_eq!(m.at(1, 3), None, "왼쪽 경계 밖");
        assert_eq!(m.at(2, 4), None, "아래 경계 밖");
        assert_eq!(m.at(2, 2), None, "위 경계 밖");
    }

    /// 오버레이가 목록 위에 뜨면 오버레이가 이긴다.
    #[test]
    fn the_last_registered_zone_wins_when_they_overlap() {
        let mut m = HitMap::default();
        m.push(r(0, 0, 20, 10), Zone::GameRow(0));
        m.push(r(5, 5, 5, 1), Zone::StandingRow(3));
        assert_eq!(m.at(5, 5), Some(Zone::StandingRow(3)));
        assert_eq!(m.at(4, 5), Some(Zone::GameRow(0)));
    }

    /// 폭이나 높이가 0인 영역은 화면에 없다 — 좁은 화면에서 접힌 요소를
    /// 클릭할 수 있으면 안 된다. 이걸 `push`에서 걸러내지 않고 `Rect::contains`에
    /// 맡기고 있으므로, 그 성질이 바뀌면 여기서 잡힌다.
    #[test]
    fn an_empty_area_is_not_clickable() {
        let mut m = HitMap::default();
        m.push(r(3, 3, 0, 1), Zone::PitchNav);
        m.push(r(3, 3, 5, 0), Zone::PitchNav);
        assert_eq!(m.at(3, 3), None);
    }

    #[test]
    fn clear_forgets_the_previous_frame() {
        let mut m = HitMap::default();
        m.push(r(0, 0, 5, 1), Zone::Tab(Tab::Games));
        m.clear();
        assert_eq!(m.at(0, 0), None);
    }
}
