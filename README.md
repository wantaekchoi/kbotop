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

## 무엇이 보이나

**경기 목록** — 그날 경기의 상태와 점수, 선발 맞대결, 구장과 중계 채널. 시작 전이면 몇 시간 몇 분 남았는지.

**라이브** — 점수·이닝·볼카운트·아웃·주자에 문자중계가 붙습니다. 투구 하나하나가 스트라이크존과 측면 궤적에 로케이션과 구속으로 찍히고, 이닝별 득점과 지금 대결(타자의 오늘 성적, 투수의 투구 수, 둘의 통산 상대 전적)이 함께 뜹니다. `[`로 지나간 타석을 되감고, 이닝 경계를 넘으면 앞 이닝을 받아옵니다.

**순위** — 승패·승률·게임차에 최근 5경기와 연승·연패. `Enter`로 그 팀의 시즌 타격·투구 기록을 펼칩니다.

**곁들이** — 언론사 RSS 헤드라인(`n`으로 목록과 발췌), 구단 공식 사이트·굿즈 링크(`o`).

갱신 주기는 라이브 5초(설정 가능), 경기 목록 60초, 순위 90초입니다. 끝난 경기는 더 바뀔 게 없으므로 5분에 한 번만 확인합니다.

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

마우스도 됩니다. `F9`에서 켜면 클릭해 고르고 다시 클릭해 열고, 휠로 굴립니다. 켜는 동안에는 터미널의 드래그 선택이 앱으로 넘어가므로(복사할 때는 Shift를 누른 채 드래그) 기본값은 꺼짐입니다.

## 설정

`F9`에서 바꾸면 바로 저장됩니다. 파일은 Linux `$XDG_CONFIG_HOME/kbotop/config.toml`(없으면 `~/.config/kbotop/`), macOS `~/Library/Application Support/kbotop/`, Windows `%APPDATA%\kbotop\config\`.

테마는 프리셋(`default` · `high-contrast` · `mono`)에 강조색을 얹습니다.

```toml
favorite_team = "lg"     # 응원 팀 별칭 (--team과 같은 값)
poll_secs = 5            # 라이브 갱신 주기(초)
lang = "ko"              # ko · en · ja
timezone = "auto"        # auto · kst · +09:00 류 오프셋
mouse = false            # 클릭·휠 (켜면 터미널 드래그 선택을 가져갑니다)

[theme]
preset = "default"       # default · high-contrast · mono
accent = "#ff6600"       # team · none · cyan/green/yellow/magenta/blue/red · #rrggbb
```

없는 키는 기본값으로 채우므로, 예전 버전이 쓴 파일도 그대로 열립니다.

`mono`는 색을 아예 쓰지 않아 흑백 터미널에서도 읽힙니다.

## 고지

팬이 만든 비공식 도구입니다. 데이터는 네이버 스포츠의 공개(비공식) 엔드포인트에서 가져오고 권리는 KBO와 네이버에 있습니다. 개인·비상업 용도로 써 주세요. 권리자 요청이 있으면 바로 조치합니다.

뉴스는 언론사 RSS에서 헤드라인과 짧은 발췌만 받고, 본문은 원문 링크로 넘깁니다.

한국어로 실행하면 시작할 때 이 저장소에서 팁 문구 목록을 한 번 받아옵니다(하단 티커에 쓰입니다). 실패하면 앱에 내장된 목록을 씁니다. 다른 언어에서는 요청하지 않습니다.

## 버전 정책

1.0부터 CLI 플래그(`--team`·`--date`·`--lang`·`--tz`·`--license`)와 "설정"에 적어 둔 `config.toml` 키는 유지됩니다 — 없애거나 이름을 바꾸는 건 2.0에서만 합니다. 키 표와 앱 도움말(`?`)에 있는 키도 하던 일을 계속합니다. 새 키가 더해질 수는 있지만 이미 있는 키의 뜻을 바꾸지는 않습니다.

화면 구성·색·문구, 그리고 데이터 출처는 이 약속 밖입니다. 데이터는 네이버 스포츠 한 곳에서만 오고 대체 경로가 없어서, 그쪽이 막히면 새 값이 들어오지 않습니다.

## 라이선스

[Unlicense](LICENSE) — 퍼블릭 도메인입니다. 의존성 라이선스 고지는 [THIRD-PARTY.md](THIRD-PARTY.md)에 있습니다.
