<div align="center">

# kbotop

**A terminal viewer for live KBO baseball**: scores, text play-by-play, and strike-zone pitch tracking, updating in place.

[![crates.io](https://img.shields.io/crates/v/kbotop?style=flat-square)](https://crates.io/crates/kbotop)
[![Release](https://img.shields.io/github/v/release/wantaekchoi/kbotop?style=flat-square)](https://github.com/wantaekchoi/kbotop/releases)
[![Built with Ratatui](https://img.shields.io/badge/built%20with-ratatui-1c1c1c?style=flat-square)](https://ratatui.rs)
[![License: MIT](https://img.shields.io/github/license/wantaekchoi/kbotop?style=flat-square)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/kbotop?style=flat-square)](https://crates.io/crates/kbotop)
[![CI](https://img.shields.io/github/actions/workflow/status/wantaekchoi/kbotop/ci.yml?style=flat-square&label=CI)](https://github.com/wantaekchoi/kbotop/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.75-blue?style=flat-square)](https://www.rust-lang.org)

![demo](docs/demo.gif)

[한국어](README.md)

</div>

## Introduction

`kbotop` is an interactive viewer for KBO (Korea Baseball Organization) games. It shows today's games as a live, self-refreshing dashboard: the score, the count, the runners, and the text play-by-play, all updating in place while you watch.

For a game in progress it draws each pitch in the strike zone from Naver's pitch-tracking data, so you see location and speed, not just the line score.

No API key. A single static binary.

## Install

```sh
# crates.io
cargo install kbotop

# Homebrew
brew install wantaekchoi/tap/kbotop

# prebuilt binary (macOS arm64/x64, Linux)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/wantaekchoi/kbotop/releases/latest/download/kbotop-installer.sh | sh
```

## Usage

```sh
kbotop                    # today's games
kbotop --team lg          # straight into your team's live game
kbotop --date yesterday   # also: YYYY-MM-DD, YYYYMMDD, today, tomorrow, +N, -N
kbotop --lang en          # UI language (default: auto by locale, ko/en)
```

Vim-style navigation; the in-app `?` help is the source of truth.

- Move: `j` / `k` or arrow keys
- Open live view: `Enter`
- Games / Standings: `Tab`
- Options picker (date / team / poll): `F2`
- Team links (official site / goods shop): `o`
- Open the news headline: `n`
- Inspect pitches: `Left` / `Right` (live view)
- Help: `?`
- Quit: `q`

The UI speaks Korean on ko locales; force English with `--lang en`.

## Configuration

`$XDG_CONFIG_HOME/kbotop/config.toml`, falling back to `~/.config/kbotop/config.toml`. Sets your favorite team and the poll interval.

## Disclaimer

A fan-made, unofficial tool. Data comes from Naver Sports' public (unofficial) endpoints, and all rights to it belong to the KBO and Naver. For personal, non-commercial use; we respond promptly to any rights-holder request.

## License

[MIT](LICENSE)
