<div align="center">

# kbotop

**`top`, but for KBO baseball.**

Live scoreboards, text play-by-play, and strike-zone pitch tracking — right in your terminal.

[![Built with Ratatui](https://img.shields.io/badge/built%20with-ratatui-1c1c1c?style=flat-square)](https://ratatui.rs)
[![License: MIT](https://img.shields.io/github/license/wantaekchoi/kbotop?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-000000?logo=rust&style=flat-square)](https://www.rust-lang.org)

</div>

> 🚧 **Active development.** The data layer is done and already pulling live games; the TUI is being assembled. First release is close.

`kbotop` puts today's KBO (Korea Baseball Organization) games where `htop` puts your processes — a live, self-refreshing dashboard you drive with keys you already know. Watch the score, read the play-by-play, and see every pitch land in the strike zone, without leaving your shell.

## Features

- 🔴 &nbsp;**Live scoreboard** — score, count, bases, refreshed in place
- 🗒️ &nbsp;**Text play-by-play** — the broadcast feed as it happens
- ◆ &nbsp;**Strike-zone pitch tracking** — each pitch's location and speed, on the zone
- 🏆 &nbsp;**Standings** — the league table at a glance
- ⌨️ &nbsp;**htop-style keys** — `j`/`k`, `/`, `?`, `q`, and a function-key bar
- ⚡ &nbsp;**One static binary**, no API key, no config required to start

## Install

```sh
cargo install kbotop
```

<details>
<summary>Other ways (arriving with the first release)</summary>

```sh
# Homebrew
brew install kbotop

# Prebuilt binaries — macOS (arm64/x64) & Linux
# https://github.com/wantaekchoi/kbotop/releases
```

</details>

## Usage

```sh
kbotop                    # today's games
kbotop --team lg          # jump straight into your team's live game
kbotop --date 2026-07-19  # a past date
```

| Key | Action |
|-----|--------|
| `↑` `↓` · `j` `k` | move |
| `Enter` | open live view |
| `Tab` · `F5` | Games ⇄ Standings |
| `/` | find a team |
| `?` · `F1` | help |
| `q` · `F10` | quit |

Keys mirror the in-app `?` help, which stays the source of truth.

## Configuration

`$XDG_CONFIG_HOME/kbotop/config.toml` — or `~/.config/kbotop/config.toml`. Set a favorite team and the poll interval.

## Origin of the name

`htop` → `iotop` → `gotop` → **`kbotop`**. Every `*top` is a live, self-refreshing view over some stream of state — processes, I/O, the GPU. `kbotop` aims that same idea at a baseball game. Not a parody of the name; the lineage of it.

## Disclaimer

A fan-made, unofficial tool. Data comes from Naver Sports' public (unofficial) endpoints, and all rights to it belong to the KBO and Naver. It's for personal, non-commercial use, and we respond promptly to any rights-holder request.

## License

[MIT](LICENSE)
