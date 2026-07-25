#!/usr/bin/env bash
# 릴리스 bump 시 CHANGELOG의 버전 섹션을 "기억"이 아니라 "실제 커밋 diff"에서
# 생성한다. feat 브랜치의 conventional 커밋(feat/fix/perf)만 카테고리로 묶어
# Keep a Changelog 형식 초안을 stdout에 출력한다. docs/chore/test/release/
# 내부 문서 커밋은 공개 변경 이력에서 제외한다. 출력은 초안이므로 톤·중복은
# 손으로 다듬어 CHANGELOG.md의 [Unreleased] 자리에 반영한다.
#
# usage: scripts/changelog-section.sh <이전-릴리스-ref> [<head-ref>]
#   예) scripts/changelog-section.sh v0.4-feat-tip HEAD
#   (feat/main 히스토리가 분리돼 있으므로 반드시 feat 브랜치 범위를 넘긴다.)
set -euo pipefail

prev="${1:?이전 릴리스 시점의 ref(태그/커밋)를 넘겨라}"
head="${2:-HEAD}"
range="${prev}..${head}"

emit() {
  local prefix="$1" heading="$2" lines
  # "feat: ", "feat(scope): ", "feat!: " 형태의 제목에서 본문만 뽑는다.
  lines=$(git log --no-merges --format='%s' "$range" \
    | sed -n "s/^${prefix}\(([^)]*)\)\{0,1\}!\{0,1\}: //p")
  if [ -n "$lines" ]; then
    echo "### $heading"
    printf '%s\n' "$lines" | sed 's/^/- /'
    echo
  fi
}

echo "## [Unreleased]"
echo
emit feat Added
emit fix Fixed
emit perf Performance
echo "# ↑ 초안. 톤 다듬고, 사용자에게 무의미한 내부 변경은 지운 뒤 CHANGELOG.md에 반영."
