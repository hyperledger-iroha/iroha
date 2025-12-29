#!/usr/bin/env bash
# Legacy wrapper preserved for compatibility. Delegates to the Gradle-based CI harness.

set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
HARNESS_FILTER=""
TASK_OVERRIDE=""

usage() {
  cat <<'EOF' >&2
Usage: run_tests.sh [--tests <comma-separated-class-list>] [--tasks "<gradle tasks>"]

Options:
  --tests   Restrict the harness to specific main-based test classes (by fully-qualified name).
            This is forwarded to Gradle via ANDROID_HARNESS_MAINS.
  --tasks   Override the Gradle tasks to run (space-separated), forwarded to ANDROID_GRADLE_TASKS.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tests)
      shift
      [[ $# -gt 0 ]] || { echo "--tests requires an argument" >&2; usage; exit 1; }
      HARNESS_FILTER=$1
      shift
      ;;
    --tasks)
      shift
      [[ $# -gt 0 ]] || { echo "--tasks requires an argument" >&2; usage; exit 1; }
      TASK_OVERRIDE=$1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -n "$HARNESS_FILTER" ]]; then
  export ANDROID_HARNESS_MAINS="$HARNESS_FILTER"
fi
if [[ -n "$TASK_OVERRIDE" ]]; then
  export ANDROID_GRADLE_TASKS="$TASK_OVERRIDE"
fi

echo "run_tests.sh is deprecated; forwarding to ci/run_android_tests.sh (Gradle-based harness)." >&2
exec bash "$ROOT/ci/run_android_tests.sh"
