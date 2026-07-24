# kbotop demo (English) — v0.8+: in-app settings (S), theme presets (mono = no color),
# five languages switched live, in-app news list (n), live pitch nav (side/zone), tabs.
# (2026-07-19 Kiwoom 9:9 Hanwha, last at-bat, 7 pitches)
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
Type "HOME=$(mktemp -d) kbotop --date 2026-07-19 --team kiwoom --lang en"
Enter
Show

# --team: team accent + cheer badge, auto-enters live after first poll
Sleep 5s

# pitch nav: Right steps one at a time — only the selected ball stays in zone/side view
Right@700ms 3
Sleep 1500ms

# Esc once: back to all pitches, twice: back to the list
Escape
Sleep 700ms
Escape
Sleep 1200ms

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
Sleep 1800ms

Type "q"
Sleep 600ms
