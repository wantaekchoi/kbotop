# kbotop demo — v0.4: 한국어 chrome(--lang ko), 팀 테마(--team) 자동 라이브,
# 측면 뷰, 투구 네비(선택 시 단독 표시), F2 옵션 픽커, 브래킷 탭, 팁/뉴스 티커.
# (2026-07-19 키움 9:9 한화, 마지막 타석 7구)
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
Type "kbotop --date 2026-07-19 --team kiwoom --lang ko"
Enter
Show

# --team: 팀 테마 액센트(파생색) + 응원 배지, 첫 폴링 후 라이브 자동 진입
Sleep 5s

# 투구 네비: Right로 하나씩 — 선택한 공만 존/측면에 남는다(단독 표시)
Right@700ms 3
Sleep 1500ms

# Esc 1회: 전체보기 복귀, 2회: 목록
Escape
Sleep 800ms
Escape
Sleep 1200ms

# o 링크 픽커: 구단 공홈/굿즈몰 (Enter 없이 열람만 — VHS는 F2 미지원이라
# F2 옵션 픽커 장면은 생략)
Type "o"
Sleep 1200ms
Down
Sleep 700ms
Escape
Sleep 800ms

# 목록: 팀 배지 + 하단 팁/뉴스, Tab: 순위표
Tab
Sleep 2200ms
Tab
Sleep 1000ms

Type "q"
Sleep 600ms
