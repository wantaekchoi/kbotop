# 변경 이력

이 프로젝트의 주요 변경 사항을 버전별 스냅샷으로 기록합니다.
형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를 따르며, 버전은 [유의적 버전(SemVer)](https://semver.org/lang/ko/)을 씁니다.

## [Unreleased]

## [0.5.0] - 2026-07-23

### Changed
- 팀 컬러를 배경과 무관하게 읽히도록 통일 — 전경 전용(fg) RGB 팀 컬러를 없애고 배지(배경·글자색을 함께 칠함) 방식으로만 사용. 순위표 팀명도 배지로 렌더. 터미널 기본 배경색과 관계없이 팀 컬러가 항상 읽힙니다.
- `accent_on_dark`(어두운 배경 가정) 제거 — 헤더 테두리·스피너·활성 탭·선택 하이라이트를 배경 무관 방식(명명색·반전·팀색 배지)으로 전환.

### Added
- Synchronized Output(BSU/ESU) 적용 — 라이브 갱신 시 화면 찢김(tearing) 감소.

## [0.4.0] - 2026-07-23

### Added
- TUI 한국어화 — 로케일(ko)에서 화면이 한국어로 표시되고, `--lang ko|en`으로 강제할 수 있습니다. 라벨은 `i18n::Labels`로 관리해 누락 시 컴파일 오류로 잡힙니다.
- 응원 팀 테마 액센트 — WCAG 대비율 기반 파생색으로 어두운 팀 컬러도 보이도록 보정.

### Changed
- `contrast_fg`를 BT.601 휘도 휴리스틱에서 WCAG 대비율 기반으로 승격(팀 배지 글자색 결과는 동일, 무회귀).

## [0.3.0] - 2026-07-23

### Added
- `F2` 옵션 픽커 — 조회 날짜·응원 팀·폴링 주기를 앱 안에서 방향키로 선택.
- `o` 구단 링크 — 선택한 팀의 공식 홈페이지·굿즈몰을 브라우저로 열기.
- `n` 뉴스 열기 — 하단 티커에 표시 중인 KBO 뉴스 기사를 브라우저로 열기.
- 런타임 팁 갱신 — 시작 시 GitHub raw에서 팁 목록을 받아 릴리스 없이 갱신(실패 시 내장본).
- `--help`에 사용 예시와 키 요약 추가.

### Changed
- 투구를 선택하면(`Left`/`Right`) 스트라이크존·측면 뷰에 그 공만 표시(겹침 해소), `Esc`로 전체 보기 복귀.
- 라이브 화면에서 `Tab`을 누르면 목록으로 나가며 탭을 전환.
- README를 한국어 우선으로 전환(`README.md` 한국어, `README.en.md` 영어). crates.io는 영어 README 유지.

### Fixed
- 보조 기능(뉴스 갱신)이 폴링 스피너 상태에 관여하지 않도록 분리.

## [0.2.0] - 2026-07-23

### Added
- 측면 존 뷰 — 투구 궤적(낙차·높이)을 스트라이크존 옆에 표시.
- 투구별 탐색 — `Left`/`Right`로 현재 타석의 투구를 하나씩 보고 시각·구속·경과 시간·결과를 확인.
- 다음 타자 표시 — 라인업 타순 기반.
- 폴링 활동 스피너, 응원 팀 배지, 초보용 야구 규칙 팁 한 줄, 실시간 KBO 뉴스 티커.
- 한국어 README와 4배속 데모 GIF.

### Changed
- 활성 탭을 브래킷(`[ 경기 ]`)으로 표시하고 본문 제목이 무엇을 보여주는지 밝히도록 개선(경기 목록 vs 시즌 순위).
- 팀 이름을 팀 컬러 배지로, 범례에 결과 문자를 병기(색 독립성, WCAG 1.4.1).
- 날짜 입력 직관화 — `--date`가 `today`/`yesterday`/`tomorrow`/`+N`/`-N`/`YYYYMMDD`를 지원.

### Fixed
- 스트라이크존 투구 높이를 플레이트 통과 거리가 아니라 투사체 운동으로 계산(모든 투구가 같은 높이로 찍히던 문제).
- 범례가 칸을 넘어 잘리던 문제(여러 줄로 감쌈).

## [0.1.2] - 2026-07-23

### Fixed
- 가독성 수정 — 스트라이크존 범례 표시, 어두운 팀 컬러 배지 대비, 존 밖 낮은 공 표시 범위.

## [0.1.1] - 2026-07-23

### Fixed
- 순위표 주기적 갱신, 폴링 지수 백오프, 종료 경기 완화된 폴링 주기.

## [0.1.0] - 2026-07-23

### Added
- 첫 공개 릴리스 — 터미널에서 KBO 프로야구를 보는 라이브 TUI. 오늘 경기 목록·순위·라이브 스코어보드·문자중계·스트라이크존 투구 시각화. 네이버 스포츠 데이터, API 키 불필요, 단일 정적 바이너리. cargo/Homebrew/curl 설치.

[Unreleased]: https://github.com/wantaekchoi/kbotop/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/wantaekchoi/kbotop/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/wantaekchoi/kbotop/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/wantaekchoi/kbotop/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/wantaekchoi/kbotop/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/wantaekchoi/kbotop/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/wantaekchoi/kbotop/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/wantaekchoi/kbotop/releases/tag/v0.1.0
