#!/usr/bin/env bash
# Run Android SDK Gradle tasks (tests + lint + local publish) with a compact summary for CI.

set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
PROJECT_ROOT="$REPO_ROOT/java/iroha_android"
OUTPUT_DIR=${ANDROID_TEST_ARTIFACTS:-"$REPO_ROOT/artifacts/android/tests"}
SUMMARY_OUT=${ANDROID_TEST_SUMMARY_OUT:-"$OUTPUT_DIR/test-summary.json"}
LOG_OUT=${ANDROID_TEST_LOG_OUT:-"$OUTPUT_DIR/test.log"}

GRADLE_BIN=${GRADLE_BIN:-}
JAVA_HOME=${JAVA_HOME:-}

ensure_java_home() {
  if [[ -n "$JAVA_HOME" && -x "$JAVA_HOME/bin/java" ]]; then
    local version
    version=$("$JAVA_HOME/bin/java" -version 2>&1 | head -n1 | awk '{print $3}' | tr -d '"')
    case "$version" in
      17*|21*) echo "$JAVA_HOME"; return 0 ;;
    esac
  fi
  for candidate in \
    "/Library/Java/JavaVirtualMachines/openjdk-21.jdk/Contents/Home" \
    "/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home" \
    "/usr/lib/jvm/java-21-openjdk" \
    "/usr/lib/jvm/java-21-openjdk-amd64"; do
    if [[ -x "$candidate/bin/java" ]]; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

ensure_gradle() {
  if [[ -n "$GRADLE_BIN" && -x "$GRADLE_BIN" ]]; then
    echo "$GRADLE_BIN"
    return 0
  fi
  if [[ -x "$PROJECT_ROOT/gradlew" ]]; then
    echo "$PROJECT_ROOT/gradlew"
    return 0
  fi
  if command -v gradle >/dev/null 2>&1; then
    echo "gradle"
    return 0
  fi
  return 1
}

now_ms() {
  python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
}

record_result() {
  local name=$1
  local status=$2
  local duration_ms=${3:-}
  local error=${4:-}
  RESULTS+=("${name}|${status}|${duration_ms}|${error}")
}

write_summary() {
  local dest=$1
  if [[ -z "$dest" ]]; then
    return
  fi
  printf '%s\n' "${RESULTS[@]}" | python3 - "$dest" <<'PY'
import json
import sys
import datetime

dest = sys.argv[1]
results = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    name, status, duration, error = line.split("|", 3)
    record = {"name": name, "status": status}
    if duration:
        try:
            record["duration_ms"] = int(duration)
        except ValueError:
            record["duration_ms"] = duration
    if error:
        record["error"] = error
    results.append(record)

now = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0)
payload = {
    "schema_version": 1,
    "generated_at": now.isoformat().replace("+00:00", "Z"),
    "total": len(results),
    "failures": sum(1 for r in results if r["status"] != "ok"),
    "results": results,
}
with open(dest, "w", encoding="utf-8") as f:
    json.dump(payload, f, indent=2)
    f.write("\n")
PY
  echo "Wrote test summary to $dest"
}

GRADLE=$(ensure_gradle || true)
if [[ -z "$GRADLE" ]]; then
  echo "Gradle not found; install Gradle or provide GRADLE_BIN" >&2
  exit 1
fi
JAVA_HOME=$(ensure_java_home || true)
if [[ -n "$JAVA_HOME" ]]; then
  export JAVA_HOME
  export ORG_GRADLE_JAVA_HOME="$JAVA_HOME"
fi

mkdir -p "$OUTPUT_DIR"
: >"$LOG_OUT"
RESULTS=()
FAILURES=0

run_task() {
  local label=$1
  shift
  local start_ms end_ms duration
  start_ms=$(now_ms)
  echo "[android-tests] running $label -> $*" | tee -a "$LOG_OUT"
  set +e
  "$GRADLE" -p "$PROJECT_ROOT" --no-daemon --stacktrace "$@" 2>&1 | tee -a "$LOG_OUT"
  local status=${PIPESTATUS[0]}
  set -e
  end_ms=$(now_ms)
  duration=$((end_ms - start_ms))
  if [[ $status -ne 0 ]]; then
    FAILURES=$((FAILURES + 1))
    record_result "$label" "failed" "$duration" "exit code $status"
  else
    record_result "$label" "ok" "$duration" ""
  fi
}

DEFAULT_TASKS=(
  ":core:check"
  ":android:lint"
  ":android:testDebugUnitTest"
  ":android:publishToMavenLocal"
  ":jvm:test"
  ":jvm:publishToMavenLocal"
  ":samples-android:assembleDebug"
)

TASKS=("${DEFAULT_TASKS[@]}")
if [[ -n "${ANDROID_GRADLE_TASKS:-}" ]]; then
    # shellcheck disable=SC2206
    TASKS=(${ANDROID_GRADLE_TASKS})
fi

for task in "${TASKS[@]}"; do
  label=${task#:}
  label=${label//[:]/_}
  run_task "$label" "$task"
done

write_summary "$SUMMARY_OUT"

if [[ $FAILURES -ne 0 ]]; then
  echo "[android-tests] one or more Gradle tasks failed ($FAILURES total)" >&2
  exit 1
fi
