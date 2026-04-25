#!/usr/bin/env bash

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
PROFILE="focused"
OUT_DIR="${ROOT_DIR}/dist/soracloud-production-readiness-$(date -u +%Y%m%dT%H%M%SZ)"
PREPARE_PORTABLE="auto"
SKIP_PORTABLE=0
SKIP_INTEGRATION=0
MIXED_HOST_INVENTORY=""

usage() {
  cat <<'USAGE' >&2
Usage: scripts/ci/run_soracloud_production_readiness.sh [options]

Options:
  --profile focused|full|load       focused runs crate-local gates; full adds portable and multi-peer live flows; load runs route + multi-peer load gates
  --out DIR                         report output directory (default: dist/soracloud-production-readiness-<timestamp>)
  --prepare-portable-assets         prepare verified Debian genericcloud assets when portable env vars are missing
  --no-prepare-portable-assets      require caller-provided IROHA_INROU_PORTABLE_* asset paths
  --skip-portable                   skip PortableVm QEMU smoke even in full profile
  --skip-integration                skip multi-peer integration tests
  --mixed-host-inventory PATH       run the mixed-host Inrou gate against this inventory in full/load profile
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

mkdir -p "$OUT_DIR/logs"
REPORT="${OUT_DIR}/soracloud_production_readiness.md"
TSV="${OUT_DIR}/soracloud_production_readiness.tsv"
: > "$REPORT"
printf "step\tstatus\tduration_seconds\tlog\n" > "$TSV"

cat > "$REPORT" <<EOF
# Soracloud Production Readiness

- Profile: \`${PROFILE}\`
- Started: \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\`
- Workspace: \`${ROOT_DIR}\`

EOF

FAILED=0
STEP_INDEX=0

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
    printf "- Status: \`%s\`\n" "$status"
    printf "- Duration: \`%ss\`\n" "$duration"
    printf "- Log: \`%s\`\n\n" "$log_path"
  } >> "$REPORT"
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
    cd "$ROOT_DIR" && bash -lc "$command"
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
    skip_step "portable inrou smoke" "--skip-portable was set"
    return
  fi
  if maybe_prepare_portable_assets; then
    run_step "portable inrou smoke" \
      "env -u LOG_FORMAT IROHA_INROU_PORTABLE_KERNEL_IMAGE=$(shell_quote "$IROHA_INROU_PORTABLE_KERNEL_IMAGE") IROHA_INROU_PORTABLE_ROOTFS_IMAGE=$(shell_quote "$IROHA_INROU_PORTABLE_ROOTFS_IMAGE") IROHA_INROU_PORTABLE_INITRD_IMAGE=$(shell_quote "${IROHA_INROU_PORTABLE_INITRD_IMAGE:-}") cargo run -p xtask --bin xtask -- soracloud-inrou-smoke portable"
  else
    skip_step "portable inrou smoke" "Portable guest assets are missing; run scripts/ci/prepare_inrou_portable_guest_assets.py --print-env or pass --prepare-portable-assets."
  fi
}

run_integration_load_gates() {
  if [ "$SKIP_INTEGRATION" -eq 1 ]; then
    skip_step "multi-peer soracloud integration load" "--skip-integration was set"
    return
  fi
  run_step "multi-peer soracloud status control plane" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_status_uses_live_torii_control_plane --test iroha_cli -- --nocapture"
  run_step "multi-peer soracloud mutation rollout" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_mutations_use_live_torii_control_plane --test iroha_cli -- --nocapture"
  run_step "multi-peer soracloud training model lifecycle" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_training_and_model_weight_lifecycle_use_live_torii_control_plane --test iroha_cli -- --nocapture"
  run_step "multi-peer soracloud hf shared lease proration" \
    "env -u LOG_FORMAT cargo test -p integration_tests soracloud_hf_shared_lease_prorates_refunds_across_multiple_accounts --test iroha_cli -- --nocapture"
}

run_mixed_host_gate() {
  if [ -z "$MIXED_HOST_INVENTORY" ]; then
    skip_step "mixed-host inrou smoke" "No --mixed-host-inventory was provided."
    return
  fi
  run_step "mixed-host inrou smoke" \
    "env -u LOG_FORMAT cargo run -p xtask --bin xtask -- soracloud-inrou-smoke mixed-host --inventory $(shell_quote "$MIXED_HOST_INVENTORY")"
}

run_focused_gates

case "$PROFILE" in
  focused)
    ;;
  full)
    run_portable_gate
    run_integration_load_gates
    run_mixed_host_gate
    ;;
  load)
    run_integration_load_gates
    run_mixed_host_gate
    ;;
esac

{
  printf "## Summary\n\n"
  if [ "$FAILED" -eq 0 ]; then
    printf "All required Soracloud production readiness gates passed for profile \`%s\`.\n" "$PROFILE"
  else
    printf "One or more Soracloud production readiness gates failed for profile \`%s\`. Inspect the logs above.\n" "$PROFILE"
  fi
  printf "\n- TSV: \`%s\`\n" "$TSV"
} >> "$REPORT"

printf 'Soracloud readiness report: %s\n' "$REPORT"
printf 'Soracloud readiness TSV: %s\n' "$TSV"

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi
