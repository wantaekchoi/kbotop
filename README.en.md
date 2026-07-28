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

The mouse works too: click to select, click again to open, wheel to scroll. It does take drag-to-select away from your terminal, so hold Shift while dragging to copy, or turn it off in `F9`.

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
