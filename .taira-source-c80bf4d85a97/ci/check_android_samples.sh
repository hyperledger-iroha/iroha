#!/usr/bin/env bash
# CI helper that enforces the AND5 Android sample gates (lint, unit tests,
# manifest generation, and dependency lock hygiene).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLES_DIR="${ROOT_DIR}/examples/android"
GRADLE_WRAPPER="${SAMPLES_DIR}/gradlew"
CHECK_SAMPLES_SCRIPT="${ROOT_DIR}/scripts/check_android_samples.sh"

if [[ ! -x "${GRADLE_WRAPPER}" ]]; then
  echo "error: expected Gradle wrapper under ${SAMPLES_DIR}" >&2
  exit 1
fi

run_gradle() {
  (cd "${SAMPLES_DIR}" && "${GRADLE_WRAPPER}" "$@")
}

if [[ "${ANDROID_SAMPLES_SKIP_BUILD:-0}" != "1" ]]; then
  echo "==> Building Android SDK + samples (scripts/check_android_samples.sh)"
  "${CHECK_SAMPLES_SCRIPT}"
else
  echo "==> Skipping sample build (ANDROID_SAMPLES_SKIP_BUILD=1)"
fi

if [[ "${ANDROID_SAMPLES_SKIP_LINT_TEST:-0}" != "1" ]]; then
  echo "==> Running lint + unit tests for operator-console and retail-wallet"
  run_gradle \
    :operator-console:lintRelease \
    :operator-console:testReleaseUnitTest \
    :retail-wallet:lintRelease \
    :retail-wallet:testReleaseUnitTest
else
  echo "==> Skipping lint/unit tests (ANDROID_SAMPLES_SKIP_LINT_TEST=1)"
fi

if [[ "${ANDROID_SAMPLES_SKIP_LOCKS:-0}" != "1" ]]; then
  echo "==> Refreshing Gradle dependency locks (gradle -q dependencies --write-locks)"
  run_gradle -q dependencies --write-locks >/dev/null
  if git -C "${ROOT_DIR}" status --short -- \
    "examples/android/**/dependency-locks/**" \
    "examples/android/**/gradle.lockfile" | grep -q '.'; then
    echo "error: Gradle lockfiles changed; refresh them locally and commit the updates." >&2
    git -C "${ROOT_DIR}" status -sb -- \
      "examples/android/**/dependency-locks/**" \
      "examples/android/**/gradle.lockfile"
    exit 1
  fi
else
  echo "==> Skipping dependency lock refresh (ANDROID_SAMPLES_SKIP_LOCKS=1)"
fi

echo "Android sample CI checks completed."
