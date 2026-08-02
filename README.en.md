<div align="center">

# kbotop

**KBO baseball in your terminal** — scores, text play-by-play, strike-zone pitch tracking.

[![crates.io](https://img.shields.io/crates/v/kbotop?style=flat-square)](https://crates.io/crates/kbotop)
[![Release](https://img.shields.io/github/v/release/wantaekchoi/kbotop?style=flat-square)](https://github.com/wantaekchoi/kbotop/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/wantaekchoi/kbotop/ci.yml?style=flat-square&label=CI)](https://github.com/wantaekchoi/kbotop/actions/workflows/ci.yml)
[![License: Unlicense](https://img.shields.io/badge/license-Unlicense-blue?style=flat-square)](LICENSE)
[![codecov](https://codecov.io/gh/wantaekchoi/kbotop/branch/main/graph/badge.svg)](https://codecov.io/gh/wantaekchoi/kbotop)

![demo](docs/demo.en.gif)

[한국어](README.md)

</div>

Leave it open and the score, count, and play-by-play keep themselves current. No API key, one binary.

## What you get

**Game list** — each game's status and score, the starting matchup, the ballpark and broadcaster. Before first pitch, how many hours and minutes are left.

**Live** — score, inning, count, outs and runners, with the play-by-play underneath. Every pitch is plotted in the strike zone and in a side view with its location and speed, alongside the linescore and the current matchup (the batter's day, the pitcher's pitch count, their career head-to-head). `[` rewinds through past at-bats, and past an inning boundary it pulls in the inning before.

**Standings** — wins, losses, win rate and games behind, plus the last five games and the current streak. `Enter` opens that team's season batting and pitching lines.

**On the side** — headlines from press RSS feeds (`n` for the list and an excerpt) and links to each club's official site and shop (`o`).

Refresh intervals: 5 seconds for live (configurable), 60 for the game list, 90 for standings. A finished game has nothing left to change, so it is checked once every 5 minutes.

## Install

**Homebrew** (macOS · Linux)

```sh
brew install wantaekchoi/tap/kbotop
```

**Prebuilt binary** (macOS arm64/x64 · Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/wantaekchoi/kbotop/releases/latest/download/kbotop-installer.sh | sh
```

**Windows** — download `kbotop-x86_64-pc-windows-msvc.zip` from [Releases](https://github.com/wantaekchoi/kbotop/releases/latest), unzip it, and run `kbotop.exe`. (The script above also works from Git Bash or MSYS.)

**cargo**

```sh
cargo install kbotop
```

## Usage

```sh
kbotop                    # today's games
kbotop --team lg          # straight into your team's live game
kbotop --date yesterday   # also: YYYY-MM-DD, YYYYMMDD, today, tomorrow, +N, -N
kbotop --lang en          # ko / en / ja (default: auto by locale)
kbotop --tz +09:00        # display time zone (default: detected from the system)
kbotop --license          # notices for the statically linked open source
```

| Keys | |
|---|---|
| `j` `k` · `gg` `G` | move |
| `Enter` | open — a game into the live view, a team into its season numbers |
| `Tab` | games ↔ standings |
| `←` `→` | one pitch at a time (live view) |
| `[` `]` | rewind at-bats; past an inning boundary it pulls in the inning before |
| `F2` `F9` | date · settings |
| `o` `n` | team links · news |
| `Esc` `q` | back · quit |

Press `?` in the app for the full list.

The mouse works too. Turn it on in `F9` and you can click to select, click again to open, and scroll with the wheel. It is off by default because while it is on your terminal loses drag-to-select (hold Shift while dragging to copy).

## Configuration

Changes made in `F9` save right away. The file lives at `$XDG_CONFIG_HOME/kbotop/config.toml` on Linux (falling back to `~/.config/kbotop/`), `~/Library/Application Support/kbotop/` on macOS, `%APPDATA%\kbotop\config\` on Windows.

A theme is a preset (`default` · `high-contrast` · `mono`) plus an accent color.

```toml
[theme]
preset = "default"
accent = "#ff6600"   # team · none · cyan/green/yellow/magenta/blue/red · #rrggbb
```

`mono` uses no color at all, so it stays readable on monochrome terminals.

## Disclaimer

A fan-made, unofficial tool. Data comes from Naver Sports' public (unofficial) endpoints, and the rights to it belong to the KBO and Naver. For personal, non-commercial use. I act on any rights-holder request promptly.

News comes from each publisher's RSS feed: the headline and a short excerpt only. Follow the link for the full article.

Running in Korean fetches a list of tip strings from this repository once at startup (they appear in the bottom ticker). If that fails, the built-in list is used. Other languages do not make the request.

## License

[The Unlicense](LICENSE) — public domain. Dependency license notices are in [THIRD-PARTY.md](THIRD-PARTY.md).
