# kbotop demo — v0.2: 팀 테마(--team) 자동 라이브 진입, 측면 뷰, 투구 네비,
# 브래킷 탭, 하단 팁/뉴스 티커. (2026-07-19 키움 9:9 한화, 마지막 타석 7구)
# record with: vhs docs/demo.tape  (needs `kbotop` on PATH: cargo install --path .)
Output docs/demo.gif

Require kbotop

Set Shell "bash"
Set FontSize 18
Set Width 1280
Set Height 760
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 55ms

Hide
Type "kbotop --date 2026-07-19 --team kiwoom"
Enter
Show

# --team: 팀 테마 액센트 + 응원 배지, 첫 폴링 후 라이브 자동 진입
# (스코어라인·문자중계·정면 존 + 측면 궤적 밴드 + 범례)
Sleep 5s

# 투구 네비: Right로 1구부터 하나씩 — 상세줄(구속·시각·경과·결과)과 하이라이트
Right@700ms 4
Sleep 1500ms

# Esc 1회: 선택 해제(전체 보기), 2회: 목록 복귀
Escape
Sleep 800ms
Escape
Sleep 1200ms

# 목록: 팀 컬러 배지 + 하단 팁/뉴스 줄, Tab: 브래킷 탭 → 순위표(시즌 현재)
Tab
Sleep 2500ms
Tab
Sleep 1200ms

Type "q"
Sleep 600ms
