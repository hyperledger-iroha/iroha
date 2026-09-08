#!/usr/bin/env bash
# Validate FASTPQ rollout capture consistency, telemetry and rollback evidence.
# Requires Python 3.10+ with scripts/requirements.txt installed. Set
# FASTPQ_ROLLOUT_BUNDLE to a bundle directory or manifest; defaults to the
# repository's artifacts/fastpq_rollouts directory. Set the independently
# trusted Ed25519 public key in FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY. An optional
# FASTPQ_XTASK_BIN selects a reviewed prebuilt xtask; otherwise Cargo runs it.
# This gate is read-only apart from ordinary Cargo build caching.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_BUNDLE_ROOT="$REPO_ROOT/artifacts/fastpq_rollouts"

TARGET_ROOT="${FASTPQ_ROLLOUT_BUNDLE:-}"

if [[ -z "$TARGET_ROOT" ]]; then
  TARGET_ROOT="$DEFAULT_BUNDLE_ROOT"
fi

if [[ ! -e "$TARGET_ROOT" ]]; then
  echo "[fastpq] rollout bundle path not found: $TARGET_ROOT" >&2
  exit 1
fi

MANIFESTS=()
if [[ -d "$TARGET_ROOT" ]]; then
  while IFS= read -r -d '' manifest; do
    MANIFESTS+=("$manifest")
  done < <(find "$TARGET_ROOT" -type f -name fastpq_bench_manifest.json -print0)
else
  if [[ "$TARGET_ROOT" == *.json ]]; then
    MANIFESTS+=("$TARGET_ROOT")
  else
    echo "[fastpq] FASTPQ_ROLLOUT_BUNDLE must point to a directory or manifest json" >&2
    exit 1
  fi
fi

if [[ ${#MANIFESTS[@]} -eq 0 ]]; then
  echo "[fastpq] no fastpq_bench_manifest.json files found under $TARGET_ROOT" >&2
  exit 1
fi

function validate_manifest() (
  local manifest="$1"
  local manifest_snapshot
  manifest_snapshot="$(mktemp "${TMPDIR:-/tmp}/fastpq-rollout-manifest.XXXXXX")"
  trap 'rm -f "$manifest_snapshot"' EXIT
  cp "$manifest" "$manifest_snapshot"
  python3 "$REPO_ROOT/scripts/fastpq/validate_rollout_manifest.py" \
    "$manifest_snapshot" --repo-root "$REPO_ROOT"
  if [[ -z "${FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY:-}" ]]; then
    echo "[fastpq] FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY must name an independently trusted Ed25519 release key" >&2
    exit 1
  fi
  if [[ -n "${FASTPQ_XTASK_BIN:-}" ]]; then
    "$FASTPQ_XTASK_BIN" fastpq-verify-bench-manifest \
      --manifest "$manifest_snapshot" --trusted-public-key "$FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY"
  else
    cargo run --locked --manifest-path "$REPO_ROOT/Cargo.toml" -p xtask \
      --features dev-tools --bin xtask -- fastpq-verify-bench-manifest \
      --manifest "$manifest_snapshot" --trusted-public-key "$FASTPQ_ROLLOUT_TRUSTED_PUBLIC_KEY"
  fi
)

function validate_grafana() {
  local bundle_dir="$1"
  local grafana_json="$bundle_dir/grafana_fastpq_acceleration.json"
  if [[ ! -f "$grafana_json" ]]; then
    echo "[fastpq] missing grafana export: $grafana_json" >&2
    exit 1
  fi
  python3 - "$grafana_json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
with path.open("r", encoding="utf-8") as fh:
    data = json.load(fh)

dashboard = data.get("dashboard", data)
uid = dashboard.get("uid")
if uid != "fastpq-acceleration":
    raise SystemExit(f"[fastpq] grafana export {path} has uid={uid!r}, expected 'fastpq-acceleration'")

annotations = dashboard.get("annotations") or {}
entries = annotations.get("list") if isinstance(annotations, dict) else annotations
if not isinstance(entries, list) or not entries:
    raise SystemExit(f"[fastpq] grafana export {path} missing rollout annotations")

def has_rollout(entry: dict) -> bool:
    text = (entry.get("text") or entry.get("title") or "")
    return "rollout" in text.lower() or "pilot" in text.lower() or "ramp" in text.lower()

if not any(has_rollout(entry) for entry in entries if isinstance(entry, dict)):
    raise SystemExit(f"[fastpq] grafana export {path} lacks rollout annotations (pilot/ramp/default)")
PY
}

function validate_alert_bundle() {
  local bundle_dir="$1"
  local repo_alert="$REPO_ROOT/dashboards/alerts/fastpq_acceleration_rules.yml"
  local repo_test="$REPO_ROOT/dashboards/alerts/tests/fastpq_acceleration_rules.test.yml"
  local bundle_alert="$bundle_dir/alerts/fastpq_acceleration_rules.yml"
  local bundle_test="$bundle_dir/alerts/tests/fastpq_acceleration_rules.test.yml"

  if [[ ! -f "$bundle_alert" || ! -f "$bundle_test" ]]; then
    echo "[fastpq] missing alert snapshot under $bundle_dir/alerts" >&2
    exit 1
  fi

  if ! cmp -s "$repo_alert" "$bundle_alert"; then
    echo "[fastpq] alert rules in $bundle_alert diverge from repository copy" >&2
    exit 1
  fi

  if ! cmp -s "$repo_test" "$bundle_test"; then
    echo "[fastpq] alert tests in $bundle_test diverge from repository copy" >&2
    exit 1
  fi
}

function validate_rollback() {
  local bundle_dir="$1"
  local log_path="$bundle_dir/rollback_drill.log"
  local metrics_path="$bundle_dir/metrics_rollback.prom"

  if [[ ! -f "$log_path" ]]; then
    echo "[fastpq] missing rollback log: $log_path" >&2
    exit 1
  fi
  if [[ ! -f "$metrics_path" ]]; then
    echo "[fastpq] missing rollback metrics scrape: $metrics_path" >&2
    exit 1
  fi

  if ! grep -q 'telemetry::fastpq\.execution_mode' "$log_path"; then
    echo "[fastpq] rollback log lacks telemetry entries: $log_path" >&2
    exit 1
  fi
  if ! grep -q 'resolved="cpu"' "$log_path"; then
    echo "[fastpq] rollback log must record resolved=\"cpu\": $log_path" >&2
    exit 1
  fi
  if ! grep -Eq 'resolved="(metal|cuda|opencl|gpu)"' "$log_path"; then
    echo "[fastpq] rollback log must capture GPU restoration (resolved=\"metal\"/\"cuda\"/\"opencl\")" >&2
    exit 1
  fi

  if ! grep -q 'fastpq_execution_mode_total' "$metrics_path"; then
    echo "[fastpq] metrics scrape missing fastpq_execution_mode_total: $metrics_path" >&2
    exit 1
  fi
  if ! grep -Eq 'fastpq[._]execution_mode_resolutions_total' "$metrics_path"; then
    echo "[fastpq] metrics scrape missing fastpq_execution_mode_resolutions_total: $metrics_path" >&2
    exit 1
  fi
  if ! grep -Eq 'fastpq_execution_mode_total\{[^}]*backend="cpu"' "$metrics_path" \
    && ! grep -Eq 'fastpq_execution_mode_total\{[^}]*mode="cpu"' "$metrics_path"; then
    echo "[fastpq] metrics scrape must record cpu fastpq_execution_mode_total samples: $metrics_path" >&2
    exit 1
  fi
  if ! grep -Eq 'fastpq_execution_mode_total\{[^}]*backend="(metal|cuda|opencl)"' "$metrics_path" \
    && ! grep -Eq 'fastpq_execution_mode_total\{[^}]*mode="(metal|cuda|opencl)"' "$metrics_path"; then
    echo "[fastpq] metrics scrape must record gpu fastpq_execution_mode_total samples: $metrics_path" >&2
    exit 1
  fi
}

function validate_row_usage() {
  local bundle_dir="$1"
  local usage_dir="$bundle_dir/row_usage"

  if [[ ! -d "$usage_dir" ]]; then
    echo "[fastpq] missing row_usage directory: $usage_dir" >&2
    exit 1
  fi

  local previous_shopt
  previous_shopt="$(shopt -p nullglob || true)"
  shopt -s nullglob
  local usage_files=("$usage_dir"/*.json)
  if [[ ${#usage_files[@]} -eq 0 ]]; then
    echo "[fastpq] no row_usage JSON files found under $usage_dir" >&2
    eval "$previous_shopt"
    exit 1
  fi
  eval "$previous_shopt"

  python3 "$REPO_ROOT/scripts/fastpq/validate_row_usage_snapshot.py" "${usage_files[@]}"
}

for manifest in "${MANIFESTS[@]}"; do
  bundle_dir="$(dirname "$manifest")"
  echo "[fastpq] validating rollout bundle: $bundle_dir"
  validate_manifest "$manifest"
  validate_grafana "$bundle_dir"
  validate_alert_bundle "$bundle_dir"
  validate_rollback "$bundle_dir"
  validate_row_usage "$bundle_dir"
  echo "[fastpq] bundle ok: $bundle_dir"
done

echo "[fastpq] rollout evidence bundles validated: ${#MANIFESTS[@]}"
