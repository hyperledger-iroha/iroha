#!/usr/bin/env bash
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

function validate_manifest() {
  local manifest="$1"
  python3 - "$manifest" "$REPO_ROOT" <<'PY'
import json
import sys
from pathlib import Path

manifest_path = Path(sys.argv[1])
repo_root = Path(sys.argv[2])
QUEUE_HEADROOM_MIN = 1.0
LDE_ZERO_FILL_LIMIT_MS = 0.40

with manifest_path.open("r", encoding="utf-8") as fh:
    signed = json.load(fh)

payload = signed.get("payload")
if not isinstance(payload, dict):
    raise SystemExit(f"[fastpq] manifest missing payload: {manifest_path}")

if payload.get("version") != 1:
    raise SystemExit(f"[fastpq] unexpected manifest version in {manifest_path}")

benches = payload.get("benches")
if not isinstance(benches, list) or len(benches) < 2:
    raise SystemExit(f"[fastpq] manifest {manifest_path} must contain at least two benches (metal/cuda)")

labels = {bench.get("label") for bench in benches}
missing = {"metal", "cuda"} - labels
if missing:
    raise SystemExit(
        f"[fastpq] manifest {manifest_path} missing required bench labels: {', '.join(sorted(missing))}"
    )

for bench in benches:
    label = bench.get("label")
    path_value = bench.get("path")
    if not path_value:
        raise SystemExit(f"[fastpq] bench {label} missing path in {manifest_path}")
    bench_path = Path(path_value)
    if not bench_path.is_absolute():
        bench_path = (repo_root / bench_path).resolve()
    if not bench_path.is_file():
        raise SystemExit(f"[fastpq] bench file not found ({bench_path}) for manifest {manifest_path}")
    rows = bench.get("rows")
    if rows is None or rows < 20_000:
        raise SystemExit(f"[fastpq] bench {label} rows below 20k in {manifest_path}")

    with bench_path.open("r", encoding="utf-8") as bench_fh:
        bench_blob = json.load(bench_fh)
    metadata = bench_blob.get("metadata")
    if not isinstance(metadata, dict):
        raise SystemExit(f"[fastpq] {bench_path} missing metadata block")
    labels_meta = metadata.get("labels")
    if not isinstance(labels_meta, dict):
        raise SystemExit(f"[fastpq] {bench_path} missing metadata.labels required for Stage7-1")
    required_labels = ("device_class", "gpu_kind")
    for key in required_labels:
        value = labels_meta.get(key)
        if not isinstance(value, str) or not value.strip():
            raise SystemExit(
                f"[fastpq] {bench_path} metadata.labels.{key} missing or empty; "
                "wrap_benchmark must emit per-device labels for Stage7-1"
            )

    if label == "metal":
        benchmarks = bench_blob.get("benchmarks")
        if not isinstance(benchmarks, dict):
            raise SystemExit(f"[fastpq] {bench_path} missing benchmarks block")
        queue = benchmarks.get("metal_dispatch_queue")
        if not isinstance(queue, dict):
            raise SystemExit(f"[fastpq] {bench_path} missing metal_dispatch_queue telemetry required for Stage7-1")
        try:
            limit = float(queue.get("limit"))
            max_in_flight = float(queue.get("max_in_flight"))
        except (TypeError, ValueError):
            raise SystemExit(
                f"[fastpq] {bench_path} reports non-numeric queue metrics (limit={queue.get('limit')}, max_in_flight={queue.get('max_in_flight')})"
            )
        if limit < 1.0:
            raise SystemExit(f"[fastpq] {bench_path} reports queue limit < 1 (limit={limit})")
        if limit - max_in_flight < QUEUE_HEADROOM_MIN:
            raise SystemExit(
                f"[fastpq] {bench_path} queue headroom below Stage7-1 requirement: limit={limit}, max_in_flight={max_in_flight}"
            )

        hotspots = benchmarks.get("zero_fill_hotspots")
        if not isinstance(hotspots, list) or not hotspots:
            raise SystemExit(f"[fastpq] {bench_path} missing zero_fill_hotspots telemetry required for Stage7-1")
        lde_entries = [
            entry for entry in hotspots
            if isinstance(entry, dict) and entry.get("operation") == "lde"
        ]
        if not lde_entries:
            raise SystemExit(f"[fastpq] {bench_path} zero_fill_hotspots lacks LDE entries")
        for entry in lde_entries:
            try:
                mean_ms = float(entry.get("mean_ms"))
            except (TypeError, ValueError):
                raise SystemExit(
                    f"[fastpq] {bench_path} zero_fill_hotspots LDE entry missing numeric mean_ms"
                )
            if mean_ms > LDE_ZERO_FILL_LIMIT_MS:
                raise SystemExit(
                    f"[fastpq] {bench_path} zero_fill_hotspots mean {mean_ms:.3f} ms exceeds Stage7-1 limit ({LDE_ZERO_FILL_LIMIT_MS} ms)"
                )

constraints = payload.get("constraints") or {}
require_rows = constraints.get("require_rows")
if require_rows is None or require_rows < 20_000:
    raise SystemExit(f"[fastpq] manifest {manifest_path} missing require_rows ≥ 20000")

max_ops = constraints.get("max_operation_ms") or {}
lde_limit = max_ops.get("lde")
if lde_limit is None or float(lde_limit) > 950.0:
    raise SystemExit(f"[fastpq] manifest {manifest_path} must enforce --max-operation-ms lde=950")

min_speed = constraints.get("min_operation_speedup") or {}
fft_speed = min_speed.get("fft")
if fft_speed is None or float(fft_speed) < 1.0:
    raise SystemExit(f"[fastpq] manifest {manifest_path} must enforce --min-operation-speedup fft=1.0")
PY
}

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
