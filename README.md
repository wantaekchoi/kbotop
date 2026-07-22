<div align="center">

# kbotop

**Watch KBO baseball from your terminal.**
A live scoreboard, text play-by-play, and strike-zone pitch tracking — in the spirit of `htop`.

<!-- badges (활성화 예정) -->
<!-- ![crates.io](https://img.shields.io/crates/v/kbotop) ![downloads](https://img.shields.io/crates/d/kbotop) ![CI](https://img.shields.io/github/actions/workflow/status/wantaekchoi/kbotop/ci.yml) ![license](https://img.shields.io/crates/l/kbotop) -->

🚧 **Work in progress** — 코어 파서 완성, TUI 조립 중. 첫 릴리스(`kbotop`으로 오늘 경기 보기)를 향해 개발 중입니다.

</div>

---

`top`이 프로세스를 보여주듯, **`kbotop`은 오늘의 KBO 경기를 터미널에 띄웁니다.** 화면에 상주하며 실시간으로 갱신되고, `htop`에서 익숙한 조작(F키·`j/k`·`/`·`q`)이 그대로 통합니다 — 다만 대상이 프로세스가 아니라 야구입니다.

## Why

- ⚡ **초경량** — 의존성 제로의 단일 바이너리. `cargo install` 한 줄, 즉시 실행.
- 🔴 **라이브** — 스코어·볼카운트·주자, 문자중계가 화면에서 살아 움직임.
- ◆ **스트라이크존 투구 시각화** — 네이버 PTS 추적 데이터로 구질·구속을 존 위에 그림. *(기존 KBO 터미널 도구엔 없던 것.)*
- ⌨️ **htop을 계승한 UX** — 요약 헤더 + 기능키 바 + top 계열 키바인딩.
- 🔑 **API 키 불필요.**

## Planned features (v1)

| | |
|---|---|
| Games | 오늘 경기 목록 (실시간 자동 갱신) |
| Live | 스코어보드 · 문자중계 · 승률 |
| Strike zone | 투구별 좌표·구속 시각화 |
| Standings | 리그 순위표 |

## Install

```sh
# 릴리스 예정
cargo install kbotop
```

## Usage (예정)

```sh
kbotop                    # 오늘 경기 목록
kbotop --team lg          # 즐겨찾기 팀 라이브로 바로
kbotop --date 2026-07-19  # 지난 경기
```

| Key | Action |
|---|---|
| `↑`/`↓` · `j`/`k` | 이동 |
| `Enter` | 라이브 진입 |
| `Tab` · `F5` | 경기 ↔ 순위 |
| `F1` · `?` | 도움말 |
| `q` · `F10` | 종료 |

## Prior art & thanks

검증된 선배들의 관례를 계승합니다 — [`htop`](https://htop.dev)(크롬·키바인딩), [`mlbt`](https://github.com/mlb-rs/mlbt)(동일 스택 MLB 스트라이크존), [`nba-go`](https://github.com/xxhomey19/nba-go)(라이브 갱신 UX), 그리고 같은 길을 먼저 낸 [`kbo-cli`](https://github.com/jeonbyeongmin/kbo-cli).

## Disclaimer

팬메이드 비공식 도구입니다. 데이터는 네이버 스포츠의 비공식 엔드포인트에서 가져오며, 저작권은 KBO·네이버에 있습니다. 비상업·개인 사용 목적이며, 권리자의 요청이 있으면 즉시 대응합니다.

## License

MIT
