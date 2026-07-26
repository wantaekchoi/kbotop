# kbotop demo — v0.18: 돌려보기([ ] 타석·j/k 문자중계 커서), 라이브 투구 네비(측면/존),
# 인앱 뉴스 목록 읽기(n), 인앱 설정(S)·테마 프리셋(mono=무채)·언어 전환, 브래킷 탭.
# (2026-07-19 키움 9:9 한화, 마지막 타석 7구)
# record with: vhs docs/demo.tape  (needs `kbotop` on PATH: cargo install --path .)
# 설정 자동저장이 실제 config를 안 건드리도록 임시 HOME으로 격리해 실행한다
# (macOS는 directories 크레이트가 $HOME/Library/Application Support를 쓴다).
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
Type "HOME=$(mktemp -d) kbotop --date 2026-07-19 --team kiwoom --lang ko"
Enter
Show

# --team: 팀 테마 액센트 + 응원 배지, 첫 폴링 후 라이브 자동 진입
Sleep 5s

# 투구 네비: Right로 하나씩 — 선택한 공만 존/측면에 남는다(단독 표시)
Right@700ms 3
Sleep 1500ms

# 돌려보기(v0.18): `[`로 지나간 타석을 되감는다 — 그 타석의 문자중계·존·측면이
# 함께 따라온다. 중계에 이미 실려 오던 데이터인데 지금까진 그리지 않고 버렸다.
Type "["
Sleep 1600ms
Type "["
Sleep 1800ms

# j/k: 문자중계를 한 줄씩 — 화면 밖으로 밀려난 줄도 되짚어 볼 수 있다
Type "k"
Sleep 600ms
Type "k"
Sleep 600ms
Type "j"
Sleep 1600ms

# Esc 계단: 커서 해제 → 최신(라이브) 복귀
Escape
Sleep 800ms
Escape
Sleep 1400ms

# Esc: 목록으로
Escape
Sleep 1200ms

# n: 인앱 뉴스 목록 — 방향키로 고르고 Enter로 발췌 읽기(원문은 o로 브라우저)
Type "n"
Sleep 1400ms
Down
Sleep 600ms
Down
Sleep 600ms
Enter
Sleep 2200ms
Escape
Sleep 700ms
Escape
Sleep 900ms

# S: 인앱 설정 — 변경 즉시 config.toml에 저장(재시작해도 유지)
Type "S"
Sleep 1400ms
# 테마 프리셋으로 이동해 순환: default → high-contrast → mono(색이 사라짐)
Down
Sleep 500ms
Down
Sleep 500ms
Right
Sleep 1000ms
Right
Sleep 1600ms
# 강조색을 순환(팀 색 → 명명색) — mono라 화면색 변화는 없고 설정값만 바뀐다
Down
Sleep 500ms
Right
Sleep 1200ms
Right
Sleep 1600ms
Escape
Sleep 1200ms

# 순위표(브래킷 탭)
Tab
Sleep 1800ms

Type "q"
Sleep 600ms
