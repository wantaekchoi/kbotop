<div align="center">

# kbotop

**A terminal viewer for live KBO baseball** — scores, text play-by-play, and strike-zone pitch tracking, updating in place.

[![Built with Ratatui](https://img.shields.io/badge/built%20with-ratatui-1c1c1c?style=flat-square)](https://ratatui.rs)
[![License: MIT](https://img.shields.io/github/license/wantaekchoi/kbotop?style=flat-square)](LICENSE)

</div>

> **Work in progress — not released yet.** No install command works today. The data layer already pulls live games from Naver Sports; the terminal UI is being built.

## Introduction

`kbotop` is an interactive viewer for KBO (Korea Baseball Organization) games. It shows today's games as a live, self-refreshing dashboard — the score, the count, the runners, and the text play-by-play, all updating in place while you watch.

For a game in progress it draws each pitch in the strike zone from Naver's pitch-tracking data, so you see location and speed, not just the line score.

No API key. A single static binary.

## Status

- [x] Live data layer — schedule, scoreboard, text play-by-play, pitch tracking, and standings, verified against the live Naver Sports API
- [ ] Terminal UI — games list, live view, strike zone _(in progress)_
- [ ] First release — `cargo install`, Homebrew, and prebuilt binaries for macOS / Linux

Watch or star the repo to hear when it ships.

## License

[MIT](LICENSE)
