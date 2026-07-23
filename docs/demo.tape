# kbotop demo — 키움 9:9 한화 (2026-07-19), 마지막 타석 투구 7개
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
Type "kbotop --date 2026-07-19"
Enter
Show

# today's games list
Sleep 2500ms
Down@450ms 4
Sleep 1s

# live view: 9:9 scoreboard, play-by-play, strike zone (7 pitches)
Enter
Sleep 5s

# back to list, then standings
Escape
Sleep 800ms
Tab
Sleep 2500ms

Type "q"
Sleep 600ms
