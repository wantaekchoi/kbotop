#!/usr/bin/env bash
# THIRD-PARTY.md 재생성 — 릴리스마다 의존성이 바뀌면 다시 돌린다.
#
#   ./scripts/third-party.sh
#
# 정적 링크로 배포하는 바이너리는 의존성의 저작권 표시·라이선스 전문을 함께
# 배포해야 한다(MIT·BSD·ISC·Apache 등). MPL-2.0(option-ext)은 해당 파일의
# 소스 입수 방법 고지가 필요해 함께 수록된다.
#
# cargo-about이 없으면: cargo install cargo-about
set -euo pipefail
cd "$(dirname "$0")/.."
cargo about generate --output-file THIRD-PARTY.md about.hbs
echo "THIRD-PARTY.md 갱신됨"
