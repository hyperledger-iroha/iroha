#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE_DIR="${REPO_ROOT}/fixtures/sorafs_orchestrator/multi_peer_parity_v1"
ARTIFACT_ROOT="${REPO_ROOT}/artifacts/sorafs_orchestrator_sdk"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
RUN_DIR="${ARTIFACT_ROOT}/${TIMESTAMP}"
RESULTS_TSV="${RUN_DIR}/results.tsv"
SUMMARY_PATH="${RUN_DIR}/summary.json"
MATRIX_PATH="${RUN_DIR}/matrix.md"
FIXTURE_SNAPSHOT_DIR="${RUN_DIR}/fixture"
METADATA_SNAPSHOT="${FIXTURE_SNAPSHOT_DIR}/metadata.json"
GIT_REV="$(git -C "${REPO_ROOT}" rev-parse HEAD)"
GENERATED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
SUMMARY_WRITTEN=0

if [[ ! -d "${FIXTURE_DIR}" ]]; then
  echo "[sorafs-sdk] missing orchestrator fixture directory: ${FIXTURE_DIR}" >&2
  exit 1
fi

mkdir -p "${RUN_DIR}" "${FIXTURE_SNAPSHOT_DIR}"
ln -sfn "${RUN_DIR}" "${ARTIFACT_ROOT}/latest"
: >"${RESULTS_TSV}"

echo "[sorafs-sdk] writing parity artefacts to ${RUN_DIR}"

copy_fixture_file() {
  local source_file="${FIXTURE_DIR}/$1"
  local target_file="${FIXTURE_SNAPSHOT_DIR}/$1"
  if [[ -f "${source_file}" ]]; then
    cp "${source_file}" "${target_file}"
  else
    echo "[sorafs-sdk] warning: fixture file missing (${source_file})" >&2
  fi
}

copy_fixture_file "metadata.json"
copy_fixture_file "plan.json"
copy_fixture_file "providers.json"
copy_fixture_file "telemetry.json"
copy_fixture_file "options.json"

write_summary() {
  if [[ "${SUMMARY_WRITTEN}" -eq 1 ]]; then
    return
  fi
  SUMMARY_WRITTEN=1
python3 - "${RESULTS_TSV}" "${METADATA_SNAPSHOT}" "${SUMMARY_PATH}" \
    "${MATRIX_PATH}" "${GIT_REV}" "${GENERATED_AT}" "${RUN_DIR}" \
    "${FIXTURE_SNAPSHOT_DIR}" <<'PY'
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

(
    tsv_path,
    metadata_path,
    summary_path,
    matrix_path,
    git_rev,
    generated_at,
    run_dir,
    fixture_dir,
) = sys.argv[1:9]

def load_results(path: str) -> list[dict]:
    results: list[dict] = []
    if not os.path.exists(path):
        return results
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line:
                continue
            parts = line.split("\t", 4)
            if len(parts) != 5:
                continue
            sdk, label, status, duration, command = parts
            try:
                duration_value = int(duration)
            except ValueError:
                duration_value = 0
            results.append(
                {
                    "sdk": sdk,
                    "label": label,
                    "status": status,
                    "duration_seconds": duration_value,
                    "command": command,
                }
            )
    return results

def load_metadata(path: str) -> dict:
    if not os.path.exists(path):
        return {}
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)

results = load_results(tsv_path)
metadata = load_metadata(metadata_path)

summary: dict[str, object] = {
    "generated_at": generated_at,
    "git_rev": git_rev,
    "artifact_dir": run_dir,
    "fixture_snapshot_dir": fixture_dir,
    "matrix_path": matrix_path,
    "results": results,
    "success": bool(results) and all(entry["status"] == "passed" for entry in results),
}

if metadata:
    summary["fixture"] = {
        "path": metadata_path,
        "name": metadata.get("fixture"),
        "version": metadata.get("version"),
        "profile_handle": metadata.get("profile_handle"),
        "provider_count": metadata.get("provider_count"),
        "chunk_count": metadata.get("chunk_count"),
        "payload_bytes": metadata.get("payload_bytes"),
        "timestamp_hint": metadata.get("now_unix_secs"),
    }

with open(summary_path, "w", encoding="utf-8") as handle:
    json.dump(summary, handle, indent=2)
    handle.write("\n")

matrix_lines: list[str] = []
matrix_lines.append("# SoraFS Orchestrator SDK Parity Matrix")
matrix_lines.append("")
matrix_lines.append(f"- Generated: {generated_at}")
matrix_lines.append(f"- Git revision: {git_rev}")
matrix_lines.append(f"- Artifact directory: {run_dir}")
if metadata:
    fixture_name = metadata.get("fixture") or "unknown"
    version = metadata.get("version") or "unversioned"
    payload_bytes = metadata.get("payload_bytes")
    chunk_count = metadata.get("chunk_count")
    details = f"{fixture_name} (version {version})"
    extras = []
    if chunk_count is not None:
        extras.append(f"{chunk_count} chunks")
    if payload_bytes is not None:
        extras.append(f"{payload_bytes} bytes")
    if extras:
        details = f"{details}, " + ", ".join(extras)
    matrix_lines.append(f"- Fixture: {details}")
matrix_lines.append("")
matrix_lines.append("| SDK | Check | Status | Duration (s) | Command |")
matrix_lines.append("| --- | --- | --- | ---: | --- |")
if results:
    for entry in results:
        command_for_table = entry["command"].replace("|", r"\|")
        matrix_lines.append(
            f"| {entry['sdk']} | {entry['label']} | {entry['status']} | {entry['duration_seconds']} | `{command_for_table}` |"
        )
else:
    matrix_lines.append("| (none) | — | — | — | — |")

Path(matrix_path).write_text("\n".join(matrix_lines) + "\n", encoding="utf-8")

print(f"[sorafs-sdk] parity summary written to {summary_path}")
print(f"[sorafs-sdk] parity matrix written to {matrix_path}")
PY
}

trap write_summary EXIT

run_sdk_test() {
  local sdk="$1"
  shift
  local label="$1"
  shift
  local start
  start=$(date +%s)
  echo "[sorafs-sdk][${sdk}] running ${label}..."
  set +e
  "$@"
  local exit_code=$?
  set -e
  local duration status command_str
  duration=$(( $(date +%s) - start ))
  printf -v command_str '%q ' "$@"
  command_str="${command_str% }"
  if [[ "${exit_code}" -eq 0 ]]; then
    status="passed"
    echo "[sorafs-sdk][${sdk}] ${label} ok"
  else
    status="failed"
    echo "[sorafs-sdk][${sdk}] ${label} failed" >&2
  fi
  printf "%s\t%s\t%s\t%s\t%s\n" \
    "${sdk}" "${label}" "${status}" "${duration}" "${command_str}" >>"${RESULTS_TSV}"
  return "${exit_code}"
}

run_sdk_test rust "orchestrator parity" \
  cargo test -p sorafs_orchestrator rust_orchestrator_fetch_suite_is_deterministic -- --nocapture

run_sdk_test python "bindings parity (iroha_python_rs)" \
  cargo test -p iroha_python_rs sorafs_multi_fetch_local -- --nocapture

run_sdk_test javascript "orchestrator parity" \
  bash -c 'cd '"${REPO_ROOT}/javascript/iroha_js"' && npm ci && npm run build:native && node --test test/sorafsOrchestrator.parity.test.js'

run_sdk_test swift "orchestrator parity" \
  bash -c 'cd '"${REPO_ROOT}/IrohaSwift"' && swift test --filter SorafsOrchestratorParityTests'

echo "[sorafs-sdk] all orchestrator parity suites passed"
