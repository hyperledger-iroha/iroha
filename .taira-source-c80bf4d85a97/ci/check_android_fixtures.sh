#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

maybe_set_buildkite_meta() {
  local key="$1"
  local value="$2"
  if [[ -z "${key}" || -z "${value}" ]]; then
    return 0
  fi
  if ! command -v buildkite-agent >/dev/null 2>&1; then
    echo "[android-fixtures] buildkite-agent unavailable; skipping meta-data export for ${key}" >&2
    return 0
  fi
  if ! buildkite-agent meta-data set "${key}" "${value}"; then
    echo "[android-fixtures] warning: failed to record Buildkite meta-data ${key}" >&2
  fi
}

resources="${ANDROID_FIXTURE_RESOURCES:-java/iroha_android/src/test/resources}"
payloads="${ANDROID_FIXTURE_PAYLOADS:-${resources}/transaction_payloads.json}"
manifest="${ANDROID_FIXTURE_MANIFEST:-${resources}/transaction_fixtures.manifest.json}"

timestamped_summary=""
if [[ -n "${ANDROID_PARITY_SUMMARY:-}" ]]; then
  summary_target="${ANDROID_PARITY_SUMMARY}"
else
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  timestamped_summary="artifacts/android/parity/${stamp}/summary.json"
  summary_target="${timestamped_summary}"
fi

mkdir -p "$(dirname "${summary_target}")"
extra_args=(--json-out "${summary_target}")
pipeline_metadata="${ANDROID_PARITY_PIPELINE_METADATA:-${MOBILE_PARITY_PIPELINE_METADATA:-}}"
if [[ -n "${pipeline_metadata}" ]]; then
  if [[ -f "${pipeline_metadata}" ]]; then
    extra_args+=(--pipeline-metadata "${pipeline_metadata}")
  else
    echo "[android-fixtures] warning: pipeline metadata file not found at ${pipeline_metadata}" >&2
  fi
fi

cd "${repo_root}"
echo "[android-fixtures] verifying Android fixture integrity"
python3 scripts/check_android_fixtures.py \
  --resources "${resources}" \
  --fixtures "${payloads}" \
  --manifest "${manifest}" \
  --quiet \
  "${extra_args[@]}"
echo "[android-fixtures] parity confirmed"

python3 - "${summary_target}" "${repo_root}" <<'PY'
import json
import sys
from pathlib import Path

summary_path = Path(sys.argv[1]).resolve()
repo_root = Path(sys.argv[2]).resolve()

if not summary_path.exists():
    print(f"[android-fixtures] missing summary JSON at {summary_path}", file=sys.stderr)
    sys.exit(1)

try:
    payload = json.loads(summary_path.read_text(encoding="utf-8"))
except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
    print(f"[android-fixtures] invalid JSON in {summary_path}: {exc}", file=sys.stderr)
    sys.exit(1)

result = payload.get("result", {})
status = result.get("status")
error_count = result.get("error_count", 0)
if status != "ok" or error_count:
    print(f"[android-fixtures] parity summary reports status={status!r} errors={error_count}", file=sys.stderr)
    for entry in result.get("errors", []):
        print(f"[android-fixtures] summary error: {entry}", file=sys.stderr)
    sys.exit(1)

for key in ("resources_dir", "fixtures_path", "manifest_path"):
    value = payload.get(key)
    if not value:
        print(f"[android-fixtures] summary missing {key}", file=sys.stderr)
        sys.exit(1)
    path = Path(value)
    try:
        path.resolve().relative_to(repo_root)
    except ValueError:
        print(f"[android-fixtures] {key}={path} is outside repository {repo_root}", file=sys.stderr)
        sys.exit(1)
PY

latest_summary_rel=""
latest_summary_path=""
if [[ -n "${timestamped_summary}" ]]; then
  latest_summary_rel="artifacts/android/parity/latest/summary.json"
  latest_summary_path="${repo_root}/${latest_summary_rel}"
  mkdir -p "$(dirname "${latest_summary_path}")"
  cp "${summary_target}" "${latest_summary_path}"
  echo "[android-fixtures] wrote summaries to ${summary_target} and ${latest_summary_path}"
else
  echo "[android-fixtures] wrote summary to ${summary_target}"
fi

summary_abs="${summary_target}"
if [[ "${summary_abs}" != /* ]]; then
  summary_abs="${repo_root}/${summary_abs}"
fi
maybe_set_buildkite_meta "${ANDROID_PARITY_SUMMARY_META_KEY:-}" "${summary_abs}"

latest_summary_abs=""
if [[ -n "${latest_summary_path}" ]]; then
  latest_summary_abs="${latest_summary_path}"
  if [[ "${latest_summary_abs}" != /* ]]; then
    latest_summary_abs="${repo_root}/${latest_summary_abs}"
  fi
  maybe_set_buildkite_meta "${ANDROID_PARITY_LATEST_META_KEY:-}" "${latest_summary_abs}"
fi

if [[ -n "${ANDROID_PARITY_S3_PREFIX:-}" ]]; then
  if [[ -z "${timestamped_summary}" ]]; then
    echo "[android-fixtures] warning: S3 uploads require the default parity summary layout" >&2
  else
    s3_args=(
      --prefix "${ANDROID_PARITY_S3_PREFIX}"
      --upload "${summary_abs}:${timestamped_summary}"
    )
    if [[ -n "${latest_summary_rel}" && -n "${latest_summary_path}" ]]; then
      s3_args+=(--upload "${latest_summary_path}:${latest_summary_rel}")
    fi
    if [[ -n "${ANDROID_PARITY_S3_CLI:-}" ]]; then
      s3_args+=(--cli "${ANDROID_PARITY_S3_CLI}")
    fi
    if [[ -n "${ANDROID_PARITY_S3_EXTRA_ARGS:-}" ]]; then
      # shellcheck disable=SC2206
      extra_parts=( ${ANDROID_PARITY_S3_EXTRA_ARGS} )
      for part in "${extra_parts[@]}"; do
        s3_args+=(--extra-arg "${part}")
      done
    fi
    echo "[android-fixtures] uploading parity summary to ${ANDROID_PARITY_S3_PREFIX}"
    if ! python3 scripts/android_parity_s3.py "${s3_args[@]}"; then
      echo "[android-fixtures] failed to upload parity summary to S3" >&2
      exit 1
    fi
  fi
fi

if [[ -n "${ANDROID_PARITY_METRICS_PATH:-}" ]]; then
  echo "[android-fixtures] exporting metrics to ${ANDROID_PARITY_METRICS_PATH}"
  cluster_label="${ANDROID_PARITY_METRICS_CLUSTER:-default}"
  python3 scripts/android_parity_metrics.py \
    --summary "${summary_target}" \
    --output "${ANDROID_PARITY_METRICS_PATH}" \
    --cluster "${cluster_label}"
fi
