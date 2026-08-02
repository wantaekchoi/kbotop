# kbotop demo (English) — v0.33: no redraw while idle, speed entries never split. VHS cannot send mouse
# events, so the GIF shows keyboard use only.
# v0.26: a matchup block under the play-by-play
# (today's batter/pitcher lines + career head-to-head).
# v0.20: rewind past inning boundaries ([ pulls in the previous inning),
# line, its pitch shows in the zone), rewind ([ ] at-bats),
# live pitch nav (side/zone), in-app news list (n), settings (S), theme presets, tabs.
# (2026-07-02 KT vs Hanwha, 5 at-bats in the final inning)
# record with: vhs docs/demo.en.tape  (needs `kbotop` on PATH: cargo install --path .)
# Isolate config autosave from the real config via a temp HOME
# (on macOS the directories crate uses $HOME/Library/Application Support).
Output docs/demo.en.gif

Require kbotop

Set Shell "bash"
Set FontSize 18
Set Width 1280
Set Height 760
Set Theme "Dracula"
Set Padding 24
Set TypingSpeed 55ms

Hide
Type "HOME=$(mktemp -d) kbotop --date 2026-07-02 --team kt --lang en"
Enter
Show

# --team: team accent + cheer badge, auto-enters live after first poll
# v0.26: the matchup block under the play-by-play (batter/pitcher lines, head-to-head)
Sleep 5s

# pitch nav: Right steps one at a time — only the selected ball stays in zone/side view
Right@700ms 3
Sleep 1500ms

# rewind (v0.18): `[` steps back through past at-bats — the play-by-play, zone and
# side view follow. The relay response already carried this; we used to discard it.
Type "["
Sleep 700ms
Type "["
Sleep 700ms
Type "["
Sleep 700ms
Type "["
Sleep 700ms
Sleep 800ms

# v0.20~: one more `[` at the first at-bat of the inning pulls in the previous one
# — "loading the 8th" flashes, then the 8th's at-bats prepend to the list
Type "["
Sleep 2600ms
Type "["
Sleep 2000ms

# j/k: stepping through the play-by-play shows that pitch in the zone/side view
# — on a result line, the pitch that produced it stays on screen
Type "k"
Sleep 600ms
Type "k"
Sleep 600ms
Type "j"
Sleep 1600ms

# Esc staircase: clear the cursor, then return to live
Escape
Sleep 800ms
Escape
Sleep 1400ms

# Esc: back to the list — since v0.23 it shows the starters, venue and channel
Escape
Sleep 2600ms

# n: in-app news list — arrow to pick, Enter to read the excerpt (o opens the source)
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

# S: in-app settings — every change is saved to config.toml immediately (survives restart)
Type "S"
Sleep 1400ms
# theme preset cycles: default -> high-contrast -> mono (color disappears)
Down
Sleep 500ms
Down
Sleep 500ms
Right
Sleep 1000ms
Right
Sleep 1600ms
# accent color cycles (team color -> named colors) — under mono the screen stays
# monochrome, only the setting value changes
Down
Sleep 500ms
Right
Sleep 1200ms
Right
Sleep 1600ms
Escape
Sleep 1200ms

# standings (bracket tab)
Tab
Sleep 2600ms

# v0.24: Enter on a standings row opens that team's season stats
Enter
Sleep 3000ms
Escape
Sleep 900ms

Type "q"
Sleep 600ms
