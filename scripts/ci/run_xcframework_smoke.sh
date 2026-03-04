#!/usr/bin/env bash
set -euo pipefail

# XCFramework smoke harness. Executes the Norito demo project against the
# canonical IOS6 device matrix and emits a telemetry snapshot compatible with
# dashboards/mobile_ci.swift. Intended for Buildkite/macOS runners.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEMO_DIR="$ROOT_DIR/examples/ios/NoritoDemoXcode"
PROJECT="$DEMO_DIR/NoritoDemoXcode.xcodeproj"
DERIVED_DATA_ROOT="${IOS6_SMOKE_DERIVED_DATA:-$ROOT_DIR/artifacts/xcframework_smoke}"
RESULT_PATH="${IOS6_SMOKE_RESULTS_PATH:-$ROOT_DIR/artifacts/xcframework_smoke_result.json}"
BRIDGE_SCRIPT="$ROOT_DIR/scripts/build_norito_xcframework.sh"
MATRIX_PATH="${IOS6_SMOKE_MATRIX_PATH:-$ROOT_DIR/configs/swift/xcframework_device_matrix.json}"

mkdir -p "$DERIVED_DATA_ROOT" "$(dirname "$RESULT_PATH")"

log() { printf '%s\n' "$*" >&2; }

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "warning: missing required command '$1'; XCFramework smoke harness will emit telemetry and exit."
    echo '{"generated_at":"'"$(date -u +"%Y-%m-%dT%H:%M:%SZ")"'","buildkite":{"lanes":[]},"devices":{"emulators":{"passes":0,"failures":0},"strongbox_capable":{"passes":0,"failures":0}},"alert_state":{"consecutive_failures":0,"open_incidents":["xcframework_smoke_prereq_missing"]}}' >"$RESULT_PATH"
    exit 0
  fi
}

require_cmd swift
require_cmd xcodebuild
require_cmd xcrun
require_cmd python3

read_matrix_value() {
  local key="$1"
  local fallback="$2"
  if [[ -f "$MATRIX_PATH" ]]; then
    local value rc
    set +e
    value="$(python3 - "$MATRIX_PATH" "$key" "$fallback" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
key = sys.argv[2]
fallback = sys.argv[3]
try:
    data = json.loads(path.read_text())
    raw = data.get(key, "")
    value = raw.strip() if isinstance(raw, str) else str(raw).strip()
    if value:
        print(value)
        raise SystemExit(0)
except Exception:
    pass
print(fallback)
PY
)"
    rc=$?
    set -e
    if [[ $rc -eq 0 && -n "$value" ]]; then
      printf '%s' "$value"
      return 0
    fi
  fi
  printf '%s' "$fallback"
}

IOS6_SMOKE_DEST_IPHONE_SIM="${IOS6_SMOKE_DEST_IPHONE_SIM:-$(read_matrix_value "iphone_sim" "platform=iOS Simulator,name=iPhone 15")}"
IOS6_SMOKE_DEST_IPAD_SIM="${IOS6_SMOKE_DEST_IPAD_SIM:-$(read_matrix_value "ipad_sim" "platform=iOS Simulator,name=iPad (10th generation)")}"
IOS6_SMOKE_DEST_STRONGBOX="${IOS6_SMOKE_DEST_STRONGBOX:-$(read_matrix_value "strongbox" "platform=iOS,name=iPhone 14 Pro")}"
IOS6_SMOKE_DEST_MAC_FALLBACK="${IOS6_SMOKE_DEST_MAC_FALLBACK:-$(read_matrix_value "mac_fallback" "platform=macOS,arch=arm64,variant=Designed for iPad")}"

ensure_disk_budget() {
  local target="$1"
  local min_gb="$2"
  if [[ ! -d "$target" ]]; then
    target="$(dirname "$target")"
  fi
  local available_kb
  available_kb="$(df -k "$target" | awk 'NR==2 {print $4}')"
  if [[ -z "$available_kb" ]]; then
    log "warning: unable to determine free space for $target"
    return 0
  fi
  local min_kb=$((min_gb * 1024 * 1024))
  if (( available_kb < min_kb )); then
    log "error: insufficient disk for XCFramework smoke (need ${min_gb}GB+, have $((available_kb / 1024 / 1024))GB) on $target"
    exit 1
  fi
}

if [[ ! -d "$DEMO_DIR" ]]; then
  log "error: Norito demo directory missing at $DEMO_DIR"
  exit 1
fi

ensure_disk_budget "$DERIVED_DATA_ROOT" 20

if [[ "${IOS6_SMOKE_SKIP_BRIDGE:-0}" != "1" ]]; then
  if [[ -x "$BRIDGE_SCRIPT" ]]; then
    log "[xcframework] building NoritoBridge.xcframework"
    if ! "$BRIDGE_SCRIPT" >/dev/null; then
      log "error: failed to build NoritoBridge.xcframework"
      exit 1
    fi
  else
    log "warning: bridge build script missing at $BRIDGE_SCRIPT; continuing without rebuilding"
  fi
fi

run_xcodebuild_logged() {
  local log_path="$1"
  shift
  local rc
  if command -v xcbeautify >/dev/null 2>&1; then
    set +e
    xcodebuild "$@" 2>&1 | tee "$log_path" | xcbeautify
    rc=${PIPESTATUS[0]}
    set -e
  elif command -v xcpretty >/dev/null 2>&1; then
    set +e
    xcodebuild "$@" 2>&1 | tee "$log_path" | xcpretty
    rc=${PIPESTATUS[0]}
    set -e
  else
    set +e
    xcodebuild "$@" 2>&1 | tee "$log_path"
    rc=${PIPESTATUS[0]}
    set -e
  fi
  return "$rc"
}

extract_destination_field() {
  local dest="$1"
  local field="$2"
  local value rc
  set +e
  value="$(DEST_VALUE="$dest" FIELD_KEY="$field" python3 <<'PY'
import os

dest = os.environ["DEST_VALUE"]
field = os.environ["FIELD_KEY"]
for chunk in dest.split(","):
    chunk = chunk.strip()
    if "=" not in chunk:
        continue
    key, value = chunk.split("=", 1)
    if key == field:
        print(value.strip())
        break
PY
)"
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then
    return 1
  fi
  printf '%s\n' "$value"
}

resolve_sim_udid() {
  local dest="$1"
  local name os_filter udid rc
  name="$(extract_destination_field "$dest" "name")"
  os_filter="$(extract_destination_field "$dest" "OS")"
  [[ -n "$name" ]] || return 1

  set +e
  udid="$(DEST_NAME="$name" DEST_OS_FILTER="${os_filter:-}" python3 <<'PY'
import json
import os
import subprocess

name = os.environ["DEST_NAME"]
os_filter = os.environ.get("DEST_OS_FILTER", "")

result = subprocess.run(
    ["xcrun", "simctl", "list", "devices", "available", "--json"],
    check=True,
    capture_output=True,
    text=True,
)
data = json.loads(result.stdout)

for runtime, devices in data.get("devices", {}).items():
    if os_filter and os_filter not in runtime:
        continue
    for device in devices:
        if device.get("isAvailable") and device.get("name") == name:
            print(device["udid"])
            raise SystemExit(0)
raise SystemExit(1)
PY
)"
  rc=$?
  set -e

  if [[ $rc -ne 0 || -z "$udid" ]]; then
    return 1
  fi

  printf '%s\n' "$udid"
}

sim_destination_available() {
  local dest="$1"
  local udid rc
  set +e
  udid="$(resolve_sim_udid "$dest")"
  rc=$?
  set -e
  if [[ $rc -ne 0 || -z "$udid" ]]; then
    return 1
  fi
  return 0
}

ensure_sim_ready() {
  local dest="$1"
  local name udid attempts=0
  name="$(extract_destination_field "$dest" "name")"
  set +e
  udid="$(resolve_sim_udid "$dest")"
  local rc=$?
  set -e
  if [[ $rc -ne 0 || -z "$udid" ]]; then
    log "warning: simulator '$name' not found (destination: $dest)"
    return 1
  fi

  while (( attempts < 2 )); do
    if xcrun simctl bootstatus "$udid" >/dev/null 2>&1; then
      return 0
    fi
    log "[xcframework] booting simulator '$name' (attempt $((attempts + 1)))"
    xcrun simctl boot "$udid" >/dev/null 2>&1 || true
    if xcrun simctl bootstatus "$udid" -b >/dev/null 2>&1; then
      return 0
    fi
    sleep 5
    ((attempts++))
  done

  log "warning: simulator '$name' failed to boot after $attempts attempt(s)"
  return 1
}

is_boot_failure() {
  local log_path="$1"
  [[ -f "$log_path" ]] || return 1
  if grep -Eiq '(Unable to boot the Simulator|Failed to boot.*Simulator|FBSOpenApplicationErrorDomain|The requested device could not be booted)' "$log_path"; then
    return 0
  fi
  return 1
}

declare -A LANE_STATUS LANE_KIND LANE_DURATION LANE_FAILURE_TIME LANE_MESSAGE
declare -A LANE_TAG
fallback_required=false
fallback_executed=false
open_incidents=()

lane_tag_for() {
  case "$1" in
    iphone-sim) echo "iphone-sim" ;;
    ipad-sim) echo "ipad-sim" ;;
    strongbox) echo "strongbox" ;;
    macos-fallback) echo "mac-fallback" ;;
    *) echo "$1" ;;
  esac
}

set_lane_tag() {
  local lane="$1"
  local tag="$2"
  LANE_TAG["$lane"]="$tag"
  if [[ -n "${BUILDKITE:-}" ]] && command -v buildkite-agent >/dev/null 2>&1; then
    if ! buildkite-agent meta-data set "ci/xcframework-smoke:${lane}:device_tag" "$tag" >/dev/null 2>&1; then
      log "warning: failed to record Buildkite metadata for lane '${lane}'"
    fi
  fi
}

run_lane() {
  local lane="$1"
  local dest="$2"
  local kind="$3"
  local derived="$DERIVED_DATA_ROOT/$lane"
  local start status=pass
  local message=""
  local build_log="$derived/build.log"
  local test_log="$derived/test.log"
  local build_rc=0
  local test_rc=0

  rm -rf "$derived"
  mkdir -p "$derived"

  set_lane_tag "$lane" "$(lane_tag_for "$lane")"

  if [[ "$kind" == "sim" ]]; then
    if ! sim_destination_available "$dest"; then
      message="destination unavailable"
      status=skip
      fallback_required=true
    elif ! ensure_sim_ready "$dest"; then
      message="simulator failed to boot"
      status=skip
      fallback_required=true
    fi
  elif [[ "$kind" == "strongbox" ]]; then
    if [[ -z "$dest" ]]; then
      message="IOS6_SMOKE_DEST_STRONGBOX not set"
      status=skip
    fi
  fi

  if [[ "$status" == "skip" ]]; then
    log "[xcframework] lane '$lane' skipped: $message"
    LANE_STATUS["$lane"]="skip"
    LANE_KIND["$lane"]="$kind"
    LANE_DURATION["$lane"]=0
    LANE_MESSAGE["$lane"]="$message"
    LANE_FAILURE_TIME["$lane"]=""
    return
  fi

  log "[xcframework] lane '$lane' -> $dest"
  start=$(date +%s)
  rm -f "$build_log" "$test_log"

  set +e
  run_xcodebuild_logged "$build_log" \
    -project "$PROJECT" \
    -scheme "NoritoDemoXcode" \
    -configuration Debug \
    -destination "$dest" \
    -derivedDataPath "$derived" \
    build >/dev/null
  build_rc=$?
  if [[ $build_rc -eq 0 ]]; then
    run_xcodebuild_logged "$test_log" \
      -project "$PROJECT" \
      -scheme "NoritoDemoXcode" \
      -configuration Debug \
      -destination "$dest" \
      -derivedDataPath "$derived" \
      test >/dev/null
    test_rc=$?
  else
    test_rc=1
  fi
  set -e
  local end
  end=$(date +%s)
  local duration=$((end - start))

  if [[ $build_rc -ne 0 || $test_rc -ne 0 ]]; then
    if [[ "$kind" == "sim" ]] && { is_boot_failure "$build_log" || is_boot_failure "$test_log"; }; then
      message="simulator boot failure; using macOS fallback"
      status=skip
      fallback_required=true
      duration=0
    else
      status=fail
      message="xcodebuild failed (build_rc=$build_rc test_rc=$test_rc)"
      LANE_FAILURE_TIME["$lane"]="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    fi
  fi

  if [[ "$status" == "skip" ]]; then
    log "[xcframework] lane '$lane' marked for fallback: $message"
    LANE_STATUS["$lane"]="skip"
    LANE_KIND["$lane"]="$kind"
    LANE_DURATION["$lane"]=0
    LANE_MESSAGE["$lane"]="$message"
    LANE_FAILURE_TIME["$lane"]=""
    return
  fi

  if [[ "$status" == "fail" ]]; then
    log "[xcframework] lane '$lane' failed: $message"
  fi

  LANE_STATUS["$lane"]="$status"
  LANE_KIND["$lane"]="$kind"
  LANE_DURATION["$lane"]=$duration
  LANE_MESSAGE["$lane"]="$message"
  if [[ "$status" != "fail" ]]; then
    LANE_FAILURE_TIME["$lane"]=""
  fi

  if [[ "$kind" == "mac" && "$status" != "skip" ]]; then
    fallback_executed=true
  fi
}

LANE_ORDER=(
  "iphone-sim"
  "ipad-sim"
  "strongbox"
)

run_lane "iphone-sim" "$IOS6_SMOKE_DEST_IPHONE_SIM" "sim"
run_lane "ipad-sim" "$IOS6_SMOKE_DEST_IPAD_SIM" "sim"
run_lane "strongbox" "$IOS6_SMOKE_DEST_STRONGBOX" "strongbox"

if $fallback_required; then
  LANE_ORDER+=("macos-fallback")
  run_lane "macos-fallback" "$IOS6_SMOKE_DEST_MAC_FALLBACK" "mac"
else
  LANE_STATUS["macos-fallback"]="skip"
  LANE_KIND["macos-fallback"]="mac"
  LANE_DURATION["macos-fallback"]=0
  LANE_MESSAGE["macos-fallback"]="not required"
  set_lane_tag "macos-fallback" "$(lane_tag_for "macos-fallback")"
fi

lane_rows=""
for lane in "${LANE_ORDER[@]}"; do
  lane_rows+="${lane}\t${LANE_STATUS[$lane]}\t${LANE_KIND[$lane]}\t${LANE_DURATION[$lane]}\t${LANE_FAILURE_TIME[$lane]:-}\t${LANE_MESSAGE[$lane]}\t${LANE_TAG[$lane]:-}\n"
done

if [[ "${LANE_STATUS[strongbox]}" == "skip" ]]; then
  open_incidents+=("xcframework_smoke_strongbox_unavailable")
fi

if $fallback_executed; then
  open_incidents+=("xcframework_smoke_fallback")
fi

OPEN_INCIDENTS_JSON="$(printf '%s\n' "${open_incidents[@]:-}" | python3 -c 'import json,sys; data=[line.strip() for line in sys.stdin if line.strip()]; print(json.dumps(data))')"
export OPEN_INCIDENTS_JSON

python3 <<'PY' <<<"$lane_rows" >"$RESULT_PATH"
import json, sys, datetime, os

open_incidents = json.loads(os.environ.get("OPEN_INCIDENTS_JSON", "[]"))

lanes = []
sim_pass = sim_fail = 0
sb_pass = sb_fail = 0
durations = []
for line in sys.stdin:
    if not line.strip():
        continue
    name, status, kind, duration_s, failure_time, message, device_tag = line.rstrip("\n").split("\t")
    duration = int(duration_s)
    success = status == "pass"
    if duration:
        durations.append(duration)
    if kind == "sim":
        if success:
            sim_pass += 1
        else:
            sim_fail += 1
    if kind == "strongbox":
        if success:
            sb_pass += 1
        else:
            sb_fail += 1
    lane_entry = {
        "name": f"ci/xcframework-smoke:{name}",
        "status": status,
        "success_rate_14_runs": 1.0 if success else 0.0,
        "last_failure": failure_time or None,
        "flake_count": 0,
        "mttr_hours": 0.0 if success else 1.0,
        "notes": message or None,
        "device_tag": device_tag or None,
    }
    lanes.append(lane_entry)

queue_depth = 0
avg_runtime = (sum(durations) / len(durations) / 60.0) if durations else 0.0

generated_at = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

result = {
    "generated_at": generated_at,
    "buildkite": {
        "lanes": lanes,
        "queue_depth": queue_depth,
        "average_runtime_minutes": round(avg_runtime, 2),
    },
    "devices": {
        "emulators": {"passes": sim_pass, "failures": sim_fail},
        "strongbox_capable": {"passes": sb_pass, "failures": sb_fail},
    },
    "alert_state": {
        "consecutive_failures": sim_fail + sb_fail,
        "open_incidents": sorted(set(open_incidents)),
    },
}

print(json.dumps(result))
PY
log "[xcframework] telemetry written to $RESULT_PATH"

ANOMALY_PATH="${IOS6_SMOKE_ANOMALY_PATH:-$ROOT_DIR/artifacts/xcframework_smoke_anomalies.json}"
python3 "${ROOT_DIR}/scripts/swift_smoke_anomalies.py" \
  --result "${RESULT_PATH}" \
  --logs-root "${DERIVED_DATA_ROOT}" \
  --out "${ANOMALY_PATH}"
log "[xcframework] anomaly summary written to $ANOMALY_PATH"
