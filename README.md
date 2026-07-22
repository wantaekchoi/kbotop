<div align="center">

# kbotop

**A terminal viewer for live KBO baseball** — scores, text play-by-play, and strike-zone pitch tracking, updating in place.

[![Release](https://img.shields.io/github/v/release/wantaekchoi/kbotop?style=flat-square)](https://github.com/wantaekchoi/kbotop/releases)
[![Built with Ratatui](https://img.shields.io/badge/built%20with-ratatui-1c1c1c?style=flat-square)](https://ratatui.rs)
[![License: MIT](https://img.shields.io/github/license/wantaekchoi/kbotop?style=flat-square)](LICENSE)

</div>

## Introduction

`kbotop` is an interactive viewer for KBO (Korea Baseball Organization) games. It shows today's games as a live, self-refreshing dashboard — the score, the count, the runners, and the text play-by-play, all updating in place while you watch.

For a game in progress it draws each pitch in the strike zone from Naver's pitch-tracking data, so you see location and speed, not just the line score.

No API key. A single static binary.

## Install

Prebuilt binaries for macOS (arm64/x64) and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/wantaekchoi/kbotop/releases/download/v0.1.0/kbotop-installer.sh | sh
```

Or from source:

```sh
git clone https://github.com/wantaekchoi/kbotop
cd kbotop
cargo install --path .
```

<details>
<summary>Coming soon</summary>

`cargo install kbotop` (crates.io) and `brew install wantaekchoi/tap/kbotop` (Homebrew) are on the way.

</details>

## Usage

```sh
kbotop                    # today's games
kbotop --team lg          # straight into your team's live game
kbotop --date 2026-07-19  # a past date
```

Vim-style navigation; the in-app `?` help is the source of truth.

- Move: `j` / `k` or arrow keys
- Open live view: `Enter`
- Games / Standings: `Tab`
- Help: `?`
- Quit: `q`

## Configuration

`$XDG_CONFIG_HOME/kbotop/config.toml`, falling back to `~/.config/kbotop/config.toml`. Sets your favorite team and the poll interval.

## Disclaimer

A fan-made, unofficial tool. Data comes from Naver Sports' public (unofficial) endpoints, and all rights to it belong to the KBO and Naver. For personal, non-commercial use; we respond promptly to any rights-holder request.

## License

[MIT](LICENSE)
