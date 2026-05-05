#!/usr/bin/env bash

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
PROFILE="focused"
OUT_DIR="${ROOT_DIR}/dist/soracloud-production-readiness-$(date -u +%Y%m%dT%H%M%SZ)"
PREPARE_PORTABLE="auto"
SKIP_PORTABLE=0
SKIP_INTEGRATION=0
MIXED_HOST_INVENTORY=""
OBSERVABILITY_EVIDENCE=""
STEP_TIMEOUT_SECONDS="${SORACLOUD_READINESS_STEP_TIMEOUT_SECONDS:-7200}"
CARGO_TARGET_DIR_OVERRIDE="${SORACLOUD_READINESS_CARGO_TARGET_DIR:-}"
ISOLATE_CARGO_TARGET=0
ALLOW_OPEN_BLOCKERS=0

usage() {
  cat <<'USAGE' >&2
Usage: scripts/ci/run_soracloud_production_readiness.sh [options]

Options:
  --profile focused|full|load       focused runs crate-local gates; full adds portable and multi-peer live flows; load runs route + multi-peer load gates
  --out DIR                         report output directory (default: dist/soracloud-production-readiness-<timestamp>)
  --step-timeout-seconds SECONDS    timeout each gate after SECONDS (default: 7200; use 0 to disable)
  --cargo-target-dir DIR            run Cargo gates with this CARGO_TARGET_DIR
  --isolate-cargo-target            run Cargo gates under OUT_DIR/cargo-target to avoid workspace lock contention
  --allow-open-blockers             exit 0 when runnable gates pass but production-only gates are blocked
  --prepare-portable-assets         prepare verified Debian genericcloud assets when portable env vars are missing
  --no-prepare-portable-assets      require caller-provided IROHA_INROU_PORTABLE_* asset paths
  --skip-portable                   skip PortableVm QEMU smoke even in full profile
  --skip-integration                skip multi-peer integration tests
  --mixed-host-inventory PATH       run the mixed-host Inrou gate against this inventory in full/load profile
  --observability-evidence PATH     validate production metrics/status/alert/dashboard evidence in full/load profile
  -h, --help                        show this help
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --profile)
      [ "$#" -ge 2 ] || { echo "ERROR: --profile requires a value" >&2; exit 2; }
      PROFILE="$2"
      shift 2
      ;;
    --out)
      [ "$#" -ge 2 ] || { echo "ERROR: --out requires a directory" >&2; exit 2; }
      OUT_DIR="$2"
      shift 2
      ;;
    --step-timeout-seconds)
      [ "$#" -ge 2 ] || { echo "ERROR: --step-timeout-seconds requires a value" >&2; exit 2; }
      STEP_TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    --cargo-target-dir)
      [ "$#" -ge 2 ] || { echo "ERROR: --cargo-target-dir requires a directory" >&2; exit 2; }
      CARGO_TARGET_DIR_OVERRIDE="$2"
      shift 2
      ;;
    --isolate-cargo-target)
      ISOLATE_CARGO_TARGET=1
      shift
      ;;
    --allow-open-blockers)
      ALLOW_OPEN_BLOCKERS=1
      shift
      ;;
    --prepare-portable-assets)
      PREPARE_PORTABLE=1
      shift
      ;;
    --no-prepare-portable-assets)
      PREPARE_PORTABLE=0
      shift
      ;;
    --skip-portable)
      SKIP_PORTABLE=1
      shift
      ;;
    --skip-integration)
      SKIP_INTEGRATION=1
      shift
      ;;
    --mixed-host-inventory)
      [ "$#" -ge 2 ] || { echo "ERROR: --mixed-host-inventory requires a path" >&2; exit 2; }
      MIXED_HOST_INVENTORY="$2"
      shift 2
      ;;
    --observability-evidence)
      [ "$#" -ge 2 ] || { echo "ERROR: --observability-evidence requires a path" >&2; exit 2; }
      OBSERVABILITY_EVIDENCE="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "ERROR: unknown option: $1" >&2
      usage
      exit 2
      ;;
  esac
done

case "$PROFILE" in
  focused|full|load) ;;
  *)
    echo "ERROR: unknown profile: $PROFILE" >&2
    usage
    exit 2
    ;;
esac

case "$STEP_TIMEOUT_SECONDS" in
  ''|*[!0-9]*)
    echo "ERROR: --step-timeout-seconds must be a non-negative integer" >&2
    exit 2
    ;;
esac

mkdir -p "$OUT_DIR/logs"
rm -f "$OUT_DIR"/logs/*.log

if [ "$ISOLATE_CARGO_TARGET" -eq 1 ] && [ -z "$CARGO_TARGET_DIR_OVERRIDE" ]; then
  CARGO_TARGET_DIR_OVERRIDE="${OUT_DIR}/cargo-target"
fi
if [ -n "$CARGO_TARGET_DIR_OVERRIDE" ]; then
  mkdir -p "$CARGO_TARGET_DIR_OVERRIDE"
  export CARGO_TARGET_DIR="$CARGO_TARGET_DIR_OVERRIDE"
fi

REPORT="${OUT_DIR}/soracloud_production_readiness.md"
TSV="${OUT_DIR}/soracloud_production_readiness.tsv"
: > "$REPORT"
printf "step\tstatus\tduration_seconds\tlog\n" > "$TSV"

cat > "$REPORT" <<EOF
# Soracloud Production Readiness

- Profile: \`${PROFILE}\`
- Started: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\`
- Workspace: \`${ROOT_DIR}\`
- Step timeout seconds: \`${STEP_TIMEOUT_SECONDS}\`
- Cargo target dir: \`${CARGO_TARGET_DIR:-workspace default}\`
- Allow open blockers: \`${ALLOW_OPEN_BLOCKERS}\`
- Mixed-host inventory: \`${MIXED_HOST_INVENTORY:-not provided}\`
- Observability evidence: \`${OBSERVABILITY_EVIDENCE:-not provided}\`

EOF

FAILED=0
BLOCKED=0
STEP_INDEX=0
OPEN_BLOCKERS=()

slugify() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-//; s/-$//'
}

shell_quote() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\"'\"'/g")"
}

record_step() {
  local label="$1"
  local status="$2"
  local duration="$3"
  local log_path="$4"
  printf "%s\t%s\t%s\t%s\n" "$label" "$status" "$duration" "$log_path" >> "$TSV"
  {
    printf "## %s\n\n" "$label"
    printf '%s\n' "- Status: \`${status}\`"
    printf '%s\n' "- Duration: \`${duration}s\`"
    printf '%s\n\n' "- Log: \`${log_path}\`"
  } >> "$REPORT"
}

run_command_with_timeout() {
  local timeout_seconds="$1"
  local command="$2"
  if [ "$timeout_seconds" -eq 0 ]; then
    bash -lc "$command"
    return $?
  fi
  python3 - "$timeout_seconds" "$command" <<'PY'
import os
import signal
import subprocess
import sys

timeout_seconds = int(sys.argv[1])
command = sys.argv[2]
process = subprocess.Popen(["bash", "-lc", command], start_new_session=True)

try:
    raise SystemExit(process.wait(timeout=timeout_seconds))
except subprocess.TimeoutExpired:
    print(
        f"ERROR: step timed out after {timeout_seconds}s; terminating child process group",
        file=sys.stderr,
    )
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()
    raise SystemExit(124)
PY
}

run_step() {
  local label="$1"
  local command="$2"
  STEP_INDEX=$((STEP_INDEX + 1))
  local slug
  slug="$(slugify "$label")"
  local log_path="${OUT_DIR}/logs/$(printf '%02d' "$STEP_INDEX")-${slug}.log"
  local start
  start="$(date -u +%s)"
  {
    printf '+ %s\n' "$command"
    printf '\n'
    cd "$ROOT_DIR" && run_command_with_timeout "$STEP_TIMEOUT_SECONDS" "$command"
  } >"$log_path" 2>&1
  local status=$?
  local end
  end="$(date -u +%s)"
  local duration=$((end - start))
  if [ "$status" -eq 0 ]; then
    record_step "$label" "pass" "$duration" "$log_path"
  else
    FAILED=1
    record_step "$label" "fail(${status})" "$duration" "$log_path"
  fi
}

skip_step() {
  local label="$1"
  local reason="$2"
  STEP_INDEX=$((STEP_INDEX + 1))
  local slug
  slug="$(slugify "$label")"
  local log_path="${OUT_DIR}/logs/$(printf '%02d' "$STEP_INDEX")-${slug}.log"
  printf '%s\n' "$reason" > "$log_path"
  record_step "$label" "skipped" "0" "$log_path"
}

block_step() {
  local label="$1"
  local reason="$2"
  STEP_INDEX=$((STEP_INDEX + 1))
  local slug
  slug="$(slugify "$label")"
  local log_path="${OUT_DIR}/logs/$(printf '%02d' "$STEP_INDEX")-${slug}.log"
  printf '%s\n' "$reason" > "$log_path"
  BLOCKED=1
  OPEN_BLOCKERS+=("${label}: ${reason}")
  record_step "$label" "blocked" "0" "$log_path"
}

portable_assets_present() {
  [ -f "${IROHA_INROU_PORTABLE_KERNEL_IMAGE:-}" ] &&
    [ -f "${IROHA_INROU_PORTABLE_ROOTFS_IMAGE:-}" ] &&
    { [ -z "${IROHA_INROU_PORTABLE_INITRD_IMAGE:-}" ] || [ -f "${IROHA_INROU_PORTABLE_INITRD_IMAGE}" ]; }
}

maybe_prepare_portable_assets() {
  if portable_assets_present; then
    return 0
  fi
  if [ "$PREPARE_PORTABLE" = "1" ] || { [ "$PREPARE_PORTABLE" = "auto" ] && [ "$PROFILE" = "full" ]; }; then
    local env_log="${OUT_DIR}/logs/portable-assets-env.log"
    if (cd "$ROOT_DIR" && python3 scripts/ci/prepare_inrou_portable_guest_assets.py --print-env) >"$env_log" 2>&1; then
      # shellcheck disable=SC1090
      source "$env_log"
      record_step "prepare portable guest assets" "pass" "0" "$env_log"
      return 0
    fi
    FAILED=1
    record_step "prepare portable guest assets" "fail" "0" "$env_log"
    return 1
  fi
  return 1
}

run_focused_gates() {
  run_step "config production posture" \
    "env -u LOG_FORMAT cargo test -p iroha_config soracloud_runtime_ --lib -- --nocapture"
  run_step "config fixture posture" \
    "env -u LOG_FORMAT cargo test -p iroha_config --test fixtures -- --nocapture"
  run_step "torii signed mutation route pressure" \
    "env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_signed_mutation_ -- --nocapture"
  run_step "torii public runtime route pressure" \
    "env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_public_runtime_rate_and_inflight_limits_fail_closed -- --nocapture"
  run_step "runtime stub production rejection" \
    "env -u LOG_FORMAT cargo test -p irohad --bin irohad stub_runtime_ -- --nocapture"
  run_step "embedded runtime manager posture" \
    "env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad manager_config_ -- --nocapture"
}

run_portable_gate() {
  if [ "$SKIP_PORTABLE" -eq 1 ]; then
    block_step "portable inrou smoke" "--skip-portable was set; full production readiness requires the PortableVm QEMU smoke gate."
    return
  fi
  if maybe_prepare_portable_assets; then
    run_step "portable inrou smoke" \
      "env -u LOG_FORMAT IROHA_INROU_PORTABLE_KERNEL_IMAGE=$(shell_quote "$IROHA_INROU_PORTABLE_KERNEL_IMAGE") IROHA_INROU_PORTABLE_ROOTFS_IMAGE=$(shell_quote "$IROHA_INROU_PORTABLE_ROOTFS_IMAGE") IROHA_INROU_PORTABLE_INITRD_IMAGE=$(shell_quote "${IROHA_INROU_PORTABLE_INITRD_IMAGE:-}") cargo run -p xtask --bin xtask -- soracloud-inrou-smoke portable"
  else
    block_step "portable inrou smoke" "Portable guest assets are missing; run scripts/ci/prepare_inrou_portable_guest_assets.py --print-env or pass --prepare-portable-assets."
  fi
}

run_integration_load_gates() {
  if [ "$SKIP_INTEGRATION" -eq 1 ]; then
    block_step "multi-peer soracloud integration load" "--skip-integration was set; full/load readiness requires live multi-peer Soracloud gates."
    return
  fi
  run_step "multi-peer soracloud status control plane" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_status_uses_live_torii_control_plane --test core_api -- --nocapture"
  run_step "multi-peer soracloud mutation rollout" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_mutations_use_live_torii_control_plane --test core_api -- --nocapture"
  run_step "multi-peer soracloud training model lifecycle" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_training_and_model_weight_lifecycle_use_live_torii_control_plane --test core_api -- --nocapture"
  run_step "multi-peer soracloud hf shared lease proration" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_hf_shared_lease_prorates_refunds_across_multiple_accounts --test core_api -- --nocapture"
}

run_mixed_host_gate() {
  if [ -z "$MIXED_HOST_INVENTORY" ]; then
    block_step "mixed-host inrou smoke" "No --mixed-host-inventory was provided; public rollout readiness requires the mixed-host Inrou smoke gate against the operator inventory."
    return
  fi
  run_step "mixed-host inrou smoke" \
    "env -u LOG_FORMAT cargo run -p xtask --bin xtask -- soracloud-inrou-smoke mixed-host --inventory $(shell_quote "$MIXED_HOST_INVENTORY")"
}

run_observability_gate() {
  if [ -z "$OBSERVABILITY_EVIDENCE" ]; then
    block_step "production observability evidence" "No --observability-evidence was provided; public rollout readiness requires operator evidence for Soracloud metrics, status fields, alerts, and dashboards."
    return
  fi
  run_step "production observability evidence" \
    "python3 scripts/ci/check_soracloud_observability_evidence.py --evidence $(shell_quote "$OBSERVABILITY_EVIDENCE")"
}

run_focused_gates

case "$PROFILE" in
  focused)
    ;;
  full)
    run_portable_gate
    run_integration_load_gates
    run_mixed_host_gate
    run_observability_gate
    ;;
  load)
    run_integration_load_gates
    run_mixed_host_gate
    run_observability_gate
    ;;
esac

{
  printf "## Summary\n\n"
  if [ "$FAILED" -eq 0 ] && [ "$BLOCKED" -eq 0 ]; then
    printf "All required Soracloud production readiness gates passed for profile \`%s\`.\n" "$PROFILE"
  elif [ "$FAILED" -eq 0 ]; then
    printf "All runnable Soracloud readiness gates passed for profile \`%s\`, but production readiness is blocked by missing required gates.\n" "$PROFILE"
  else
    printf "One or more Soracloud production readiness gates failed for profile \`%s\`. Inspect the logs above.\n" "$PROFILE"
  fi
  if [ "$BLOCKED" -ne 0 ]; then
    printf "\n### Open Blockers\n\n"
    for blocker in "${OPEN_BLOCKERS[@]}"; do
      printf -- "- %s\n" "$blocker"
    done
  fi
  printf "\n- TSV: \`%s\`\n" "$TSV"
} >> "$REPORT"

printf 'Soracloud readiness report: %s\n' "$REPORT"
printf 'Soracloud readiness TSV: %s\n' "$TSV"

if [ "$FAILED" -ne 0 ] || { [ "$BLOCKED" -ne 0 ] && [ "$ALLOW_OPEN_BLOCKERS" -ne 1 ]; }; then
  exit 1
fi
