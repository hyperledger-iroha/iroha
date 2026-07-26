#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/android_strongbox_attestation_ci.sh [--bundles-root PATH] [--summary-out PATH]

Verify every attestation bundle under the specified root (default: artifacts/android/attestation),
write per-bundle logs/results, and emit an aggregated JSON summary for governance evidence.

Options:
  --bundles-root PATH   Override the root directory that contains attestation bundles.
  --summary-out PATH    Override where the aggregated summary JSON is written. Defaults to
                        <bundles-root>/summary.json.
  -h, --help            Show this message.
USAGE
}

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BUNDLES_ROOT="${REPO_ROOT}/artifacts/android/attestation"
SUMMARY_OUT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundles-root)
      BUNDLES_ROOT="$2"
      shift 2
      ;;
    --summary-out)
      SUMMARY_OUT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[attestation-ci] unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

HARNESS_SCRIPT="${REPO_ROOT}/scripts/android_keystore_attestation.sh"
if [[ ! -x "${HARNESS_SCRIPT}" ]]; then
  echo "[attestation-ci] missing harness script: ${HARNESS_SCRIPT}" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "[attestation-ci] python3 is required to build the aggregated summary" >&2
  exit 1
fi

if [[ -z "${SUMMARY_OUT}" ]]; then
  SUMMARY_OUT="${BUNDLES_ROOT}/summary.json"
fi

relpath() {
  local absolute="$1"
  if [[ "$absolute" == "$REPO_ROOT"* ]]; then
    echo "${absolute#$REPO_ROOT/}"
  else
    echo "$absolute"
  fi
}

add_summary_record() {
  local bundle="$1"
  local status="$2"
  local error="${3:-}"
  local result_path="${4:-}"
  local log_path="${5:-}"
  local record
  record="$(
    BUNDLE="$bundle" STATUS="$status" ERROR="$error" RESULT_PATH="$result_path" LOG_PATH="$log_path" \
      python3 - <<'PY'
import json, os, pathlib

bundle = os.environ["BUNDLE"]
status = os.environ["STATUS"]
error = os.environ.get("ERROR", "")
result_path = os.environ.get("RESULT_PATH", "")
log_path = os.environ.get("LOG_PATH", "")

record = {"bundle": bundle, "status": status}
if log_path:
    record["log_path"] = log_path
if error:
    record["error"] = error
if result_path:
    path = pathlib.Path(result_path)
    if path.is_file():
        try:
            record["result"] = json.loads(path.read_text())
        except Exception as exc:  # pragma: no cover - defensive guard
            record["result_error"] = f"Failed to parse result JSON: {exc}"
    else:
        record["result_missing"] = True
print(json.dumps(record))
PY
  )"
  SUMMARY_ITEMS+=("$record")
}

write_summary() {
  local summary_path="$1"
  local bundles_root="$2"
  local total_dirs="$3"
  local note="${4:-}"
  local items_tmp=""
  if ((${#SUMMARY_ITEMS[@]} > 0)); then
    items_tmp="$(mktemp)"
    printf '%s\n' "${SUMMARY_ITEMS[@]}" >"${items_tmp}"
  fi

  python3 - "$summary_path" "$bundles_root" "$total_dirs" "$note" "${items_tmp:-}" <<'PY'
import datetime
import json
import pathlib
import sys

out_path = pathlib.Path(sys.argv[1]).resolve()
bundles_root = sys.argv[2]
total_dirs = int(sys.argv[3])
note = sys.argv[4]
items_path = pathlib.Path(sys.argv[5]) if len(sys.argv) > 5 and sys.argv[5] else None

items = []
if items_path and items_path.is_file():
    items = [json.loads(line) for line in items_path.read_text(encoding="utf-8").splitlines() if line.strip()]

now = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0)
summary = {
    "schema_version": 1,
    "generated_at": now.isoformat().replace("+00:00", "Z"),
    "bundles_root": bundles_root,
    "searched_bundle_dirs": total_dirs,
    "results": items,
    "success_count": sum(1 for item in items if item.get("status") == "ok"),
    "skipped_count": sum(1 for item in items if str(item.get("status", "")).startswith("skipped")),
}
summary["failure_count"] = len(items) - summary["success_count"] - summary["skipped_count"]
if note:
    summary["note"] = note
out_path.parent.mkdir(parents=True, exist_ok=True)
out_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
print(f"[attestation-ci] wrote summary to {out_path}")
PY

  if [[ -n "${items_tmp}" ]]; then
    rm -f "${items_tmp}"
  fi
}

SUMMARY_ITEMS=()
SUMMARY_NOTE=""
overall_status=0

if [[ ! -d "${BUNDLES_ROOT}" ]]; then
  echo "[attestation-ci] no attestation bundles directory (${BUNDLES_ROOT}); skipping." >&2
  SUMMARY_NOTE="bundles_root_missing"
  write_summary "${SUMMARY_OUT}" "${BUNDLES_ROOT}" 0 "${SUMMARY_NOTE}"
  exit 0
fi

BUNDLE_DIRS=()
while IFS= read -r maybe_bundle; do
  [[ -z "${maybe_bundle}" ]] && continue
  BUNDLE_DIRS+=("${maybe_bundle}")
done < <(find "${BUNDLES_ROOT}" -type d -maxdepth 2 -mindepth 2 | sort)
if [[ ${#BUNDLE_DIRS[@]} -eq 0 ]]; then
  echo "[attestation-ci] no bundle directories found under ${BUNDLES_ROOT}; skipping." >&2
  SUMMARY_NOTE="no_bundle_directories"
  write_summary "${SUMMARY_OUT}" "${BUNDLES_ROOT}" 0 "${SUMMARY_NOTE}"
  exit 0
fi

for bundle in "${BUNDLE_DIRS[@]}"; do
  [[ -z "${bundle}" ]] && continue
  bundle_rel=$(relpath "${bundle}")
  chain="${bundle}/chain.pem"
  challenge="${bundle}/challenge.hex"
  alias_file="${bundle}/alias.txt"
  log_path="${bundle}/attestation_ci.log"
  result_path="${bundle}/result.json"
  rm -f "${log_path}" "${result_path}"

  missing=()
  [[ ! -f "${chain}" ]] && missing+=("chain.pem")
  [[ ! -f "${challenge}" ]] && missing+=("challenge.hex")
  [[ ! -f "${alias_file}" ]] && missing+=("alias.txt")
  if [[ ${#missing[@]} -ne 0 ]]; then
    echo "[attestation-ci] skipping ${bundle_rel} (missing ${missing[*]})" >&2
    add_summary_record "${bundle_rel}" "skipped_missing_inputs" "missing ${missing[*]}" "" ""
    continue
  fi

  roots=()
  while IFS= read -r root_path; do
    [[ -n "${root_path}" ]] && roots+=("$root_path")
  done < <(find "${bundle}" -maxdepth 1 -type f -name 'trust_root_*.pem' | sort)

  bundles=()
  while IFS= read -r bundle_path; do
    [[ -n "${bundle_path}" ]] && bundles+=("$bundle_path")
  done < <(find "${bundle}" -maxdepth 1 -type f -name 'trust_root_bundle_*.zip' | sort)

  if [[ ${#roots[@]} -eq 0 && ${#bundles[@]} -eq 0 ]]; then
    echo "[attestation-ci] no trust roots for ${bundle_rel}; expecting trust_root_*.pem or trust_root_bundle_*.zip files" >&2
    overall_status=1
    add_summary_record "${bundle_rel}" "skipped_missing_trust_roots" "no trust roots in bundle" "" ""
    continue
  fi

  echo "[attestation-ci] verifying ${bundle_rel}"
  args=("--bundle-dir" "${bundle}" "--require-strongbox" "--output" "${result_path}")
  if ((${#roots[@]} > 0)); then
    for root in "${roots[@]}"; do
      args+=("--trust-root" "${root}")
    done
  fi
  if ((${#bundles[@]} > 0)); then
    for bundle_zip in "${bundles[@]}"; do
      args+=("--trust-root-bundle" "${bundle_zip}")
    done
  fi

  log_rel=$(relpath "${log_path}")
  result_rel=$(relpath "${result_path}")
  if ! "${HARNESS_SCRIPT}" "${args[@]}" 2>&1 | tee "${log_path}"; then
    echo "[attestation-ci] verification failed for ${bundle_rel}" >&2
    err_msg=$(tail -n 20 "${log_path}" | tr -d '\r' | tr '\n' ' ')
    overall_status=1
    add_summary_record "${bundle_rel}" "error" "${err_msg}" "" "${log_rel}"
    continue
  fi

  if [[ ! -f "${result_path}" ]]; then
    err_msg="missing result.json after successful harness run"
    echo "[attestation-ci] ${err_msg} for ${bundle_rel}" >&2
    overall_status=1
    add_summary_record "${bundle_rel}" "error" "${err_msg}" "" "${log_rel}"
    continue
  fi

  add_summary_record "${bundle_rel}" "ok" "" "${result_rel}" "${log_rel}"
done

write_summary "${SUMMARY_OUT}" "${BUNDLES_ROOT}" "${#BUNDLE_DIRS[@]}" "${SUMMARY_NOTE}"
exit ${overall_status}
