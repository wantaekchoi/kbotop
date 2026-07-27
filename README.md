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

터미널에 띄워 두면 점수·볼카운트·주자·문자중계가 알아서 갱신됩니다. 진행 중인 경기는 공 하나하나를 스트라이크존에 그려 로케이션과 구속까지 보여줍니다.

API 키는 필요 없고, 바이너리 하나로 돕니다.

## 설치

**Homebrew** (macOS · Linux)

```sh
brew install wantaekchoi/tap/kbotop
```

**미리 빌드된 바이너리** (macOS arm64/x64 · Linux)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/wantaekchoi/kbotop/releases/latest/download/kbotop-installer.sh | sh
```

**Windows** — [Releases](https://github.com/wantaekchoi/kbotop/releases/latest)에서 `kbotop-x86_64-pc-windows-msvc.zip`을 받아 압축을 풀고 `kbotop.exe`를 실행하세요. (Git Bash·MSYS 같은 POSIX 셸을 쓴다면 위 설치 스크립트도 그대로 동작합니다.)

**cargo** (Rust 툴체인이 있다면)

```sh
cargo install kbotop
```

## 사용법

```sh
kbotop                    # 오늘 경기
kbotop --team lg          # 내 팀 라이브 경기로 바로
kbotop --date yesterday   # YYYY-MM-DD, YYYYMMDD, today, tomorrow, +N, -N
kbotop --lang en          # ko / en / ja (기본: 로케일 자동)
kbotop --tz +09:00        # 표시 시간대 (기본: 시스템 자동 감지)
```

움직이는 방식은 Vim과 같습니다. `j`/`k`로 오르내리고 `Esc`로 물러납니다. 아래는 자주 쓰는 것만 추렸고, 전체 목록은 앱에서 `?`를 누르면 나옵니다.

**경기 목록에서** `Enter`를 누르면 그 경기 라이브로 들어갑니다. `Tab`으로 순위표와 오가고, `F2`로 날짜를 바꿉니다. `o`는 그 팀 구단 링크, `n`은 뉴스, `F9`는 설정입니다.

**순위표에서** `Enter`를 누르면 그 팀의 시즌 성적이 펼쳐집니다 — 타율·OPS부터 평균자책·WHIP까지.

**라이브 화면에서** `←`/`→`로 투구를 하나씩 짚어 보면 그 공이 스트라이크존과 측면 궤적에 뜹니다. `[`/`]`로는 지나간 타석을 되감고, 그 이닝의 첫 타석에서 한 번 더 누르면 **앞 이닝을 받아옵니다**. 문자중계는 `j`/`k`로 한 줄씩, `gg`/`G`로 양 끝까지 움직이며, 커서를 둔 줄의 공이 존에 함께 표시됩니다.

## 설정

설정 파일은 OS 관례를 따릅니다 — Linux는 `$XDG_CONFIG_HOME/kbotop/config.toml`(없으면 `~/.config/kbotop/`), macOS는 `~/Library/Application Support/kbotop/`, Windows는 `%APPDATA%\kbotop\`. `F9` 화면에서 응원 팀·언어·폴링 주기·테마를 바꾸면 바로 저장됩니다.

테마는 프리셋(`default`/`high-contrast`/`mono`)에 강조색을 얹습니다. 강조색은 응원 팀 색(`team`), 미리 정해 둔 색 여섯 개, 또는 16진 값 중에서 고릅니다. 설정 화면(`F9`)에서 부르는 이름과 같습니다.

```toml
[theme]
preset = "default"
accent = "#ff6600"   # team · none · cyan/green/yellow/magenta/blue/red · #rrggbb
```

`mono`는 색을 아예 쓰지 않아 흑백 터미널에서도 읽힙니다.

## 고지

팬이 만든 비공식 도구입니다. 데이터는 네이버 스포츠의 공개(비공식) 엔드포인트에서 가져오며, 권리는 KBO와 네이버에 있습니다. 개인·비상업 용도로 써 주세요. 권리자 요청이 있으면 바로 조치합니다.

뉴스는 각 언론사 RSS에서 헤드라인과 짧은 발췌만 받아 보여주고, 본문은 원문 링크로 넘깁니다.

## 라이선스

[Unlicense](LICENSE) — 퍼블릭 도메인입니다. 의존성 라이선스 고지는 [THIRD-PARTY.md](THIRD-PARTY.md)에 있습니다.
