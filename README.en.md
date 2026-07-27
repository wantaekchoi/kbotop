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

Leave it open and the score, count, runners, and play-by-play stay current. For a game in progress it draws each pitch in the strike zone, so you get location and speed, not just the line score.

No API key, one static binary.

## Install

**Homebrew** (macOS · Linux)

```sh
brew install wantaekchoi/tap/kbotop
```

**Prebuilt binary** (macOS arm64/x64 · Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/wantaekchoi/kbotop/releases/latest/download/kbotop-installer.sh | sh
```

**Windows** — download `kbotop-x86_64-pc-windows-msvc.zip` from [Releases](https://github.com/wantaekchoi/kbotop/releases/latest), unzip it, and run `kbotop.exe`. (The install script above also works from a POSIX shell such as Git Bash or MSYS.)

**cargo** (if you have a Rust toolchain)

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
```

Navigation works the way it does in Vim: `j`/`k` to move, `Esc` to back out. What follows is the everyday subset; press `?` in the app for the full list.

**From the game list**, `Enter` opens that game's live view. `Tab` switches to the standings, `F2` changes the date. `o` opens the team's links, `n` the news, `F9` the settings.

**From the standings**, `Enter` opens that team's season numbers, from batting average and OPS through ERA and WHIP.

**In the live view**, `←`/`→` step through the pitches and the selected one shows up in the strike zone and the side view. `[`/`]` rewind through at-bats, and pressing again at an inning's first at-bat **pulls in the inning before it**. The play-by-play moves a line at a time with `j`/`k`, jumps to either end with `gg`/`G`, and the pitch for the line under the cursor appears in the zone alongside it.

## Configuration

The config file follows each platform's convention: `$XDG_CONFIG_HOME/kbotop/config.toml` on Linux (falling back to `~/.config/kbotop/`), `~/Library/Application Support/kbotop/` on macOS, `%APPDATA%\kbotop\` on Windows. Change your team, language, poll interval, and theme from the `F9` screen. Each change saves right away.

A theme is a preset (`default` / `high-contrast` / `mono`) plus an accent color. The accent can be your team's color (`team`), one of six named colors, or a hex value you pick.

```toml
[theme]
preset = "default"
accent = "#ff6600"   # team · none · cyan/green/yellow/magenta/blue/red · #rrggbb
```

`mono` uses no color at all, so it stays readable on monochrome terminals.

## Disclaimer

A fan-made, unofficial tool. Data comes from Naver Sports' public (unofficial) endpoints, and the rights to it belong to the KBO and Naver. For personal, non-commercial use. I act on any rights-holder request promptly.

News comes from each publisher's RSS feed: the headline and a short excerpt only. Follow the link for the full article.

## License

[The Unlicense](LICENSE) — public domain. Dependency license notices are in [THIRD-PARTY.md](THIRD-PARTY.md).
