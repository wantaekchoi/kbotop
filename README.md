<div align="center">

# kbotop

**터미널에서 보는 KBO 프로야구** — 스코어, 문자중계, 스트라이크존 투구 추적.

[![crates.io](https://img.shields.io/crates/v/kbotop?style=flat-square)](https://crates.io/crates/kbotop)
[![Release](https://img.shields.io/github/v/release/wantaekchoi/kbotop?style=flat-square)](https://github.com/wantaekchoi/kbotop/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/wantaekchoi/kbotop/ci.yml?style=flat-square&label=CI)](https://github.com/wantaekchoi/kbotop/actions/workflows/ci.yml)
[![License: Unlicense](https://img.shields.io/badge/license-Unlicense-blue?style=flat-square)](LICENSE)
[![codecov](https://codecov.io/gh/wantaekchoi/kbotop/branch/main/graph/badge.svg)](https://codecov.io/gh/wantaekchoi/kbotop)

![demo](docs/demo.gif)

[English](README.en.md)

</div>

띄워 두면 알아서 갱신됩니다. API 키 없이, 바이너리 하나로.

## 설치

**Homebrew** (macOS · Linux)

```sh
brew install wantaekchoi/tap/kbotop
```

**미리 빌드된 바이너리** (macOS arm64/x64 · Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/wantaekchoi/kbotop/releases/latest/download/kbotop-installer.sh | sh
```

**Windows** — [Releases](https://github.com/wantaekchoi/kbotop/releases/latest)에서 `kbotop-x86_64-pc-windows-msvc.zip`을 받아 풀고 `kbotop.exe`를 실행하세요. (Git Bash·MSYS를 쓴다면 위 스크립트도 그대로 됩니다.)

**cargo**

```sh
cargo install kbotop
```

## 사용법

```sh
kbotop                    # 오늘 경기
kbotop --team lg          # 내 팀 라이브로 바로
kbotop --date yesterday   # YYYY-MM-DD, YYYYMMDD, today, tomorrow, +N, -N
kbotop --lang en          # ko / en / ja (기본: 로케일 자동)
kbotop --tz +09:00        # 표시 시간대 (기본: 시스템 자동)
kbotop --license          # 정적 링크된 오픈소스 고지
```

| 키 | |
|---|---|
| `j` `k` · `gg` `G` | 이동 |
| `Enter` | 열기 — 경기는 라이브로, 순위는 팀 성적으로 |
| `Tab` | 경기 ↔ 순위 |
| `←` `→` | 투구 하나씩 (라이브) |
| `[` `]` | 지나간 타석 되감기, 이닝 경계를 넘으면 앞 이닝을 받아옵니다 |
| `F2` `F9` | 날짜 · 설정 |
| `o` `n` | 구단 링크 · 뉴스 |
| `Esc` `q` | 뒤로 · 종료 |

전체 목록은 앱에서 `?`.

마우스도 됩니다. 클릭해 고르고 다시 클릭해 열고, 휠로 굴립니다. 대신 드래그 선택이 앱으로 넘어가니 복사할 때는 Shift를 누른 채 드래그하거나 `F9`에서 끄세요.

## 설정

`F9`에서 바꾸면 바로 저장됩니다. 파일은 Linux `$XDG_CONFIG_HOME/kbotop/config.toml`(없으면 `~/.config/kbotop/`), macOS `~/Library/Application Support/kbotop/`, Windows `%APPDATA%\kbotop\config\`.

테마는 프리셋(`default` · `high-contrast` · `mono`)에 강조색을 얹습니다.

```toml
[theme]
preset = "default"
accent = "#ff6600"   # team · none · cyan/green/yellow/magenta/blue/red · #rrggbb
```

`mono`는 색을 아예 쓰지 않아 흑백 터미널에서도 읽힙니다.

## 고지

팬이 만든 비공식 도구입니다. 데이터는 네이버 스포츠의 공개(비공식) 엔드포인트에서 가져오고 권리는 KBO와 네이버에 있습니다. 개인·비상업 용도로 써 주세요. 권리자 요청이 있으면 바로 조치합니다.

뉴스는 언론사 RSS에서 헤드라인과 짧은 발췌만 받고, 본문은 원문 링크로 넘깁니다.

한국어로 실행하면 시작할 때 이 저장소에서 팁 문구 목록을 한 번 받아옵니다(하단 티커에 쓰입니다). 실패하면 앱에 내장된 목록을 씁니다. 다른 언어에서는 요청하지 않습니다.

## 라이선스

[Unlicense](LICENSE) — 퍼블릭 도메인입니다. 의존성 라이선스 고지는 [THIRD-PARTY.md](THIRD-PARTY.md)에 있습니다.
