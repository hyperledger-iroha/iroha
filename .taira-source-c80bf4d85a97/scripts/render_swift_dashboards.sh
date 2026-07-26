#!/usr/bin/env bash
set -euo pipefail

# Renders the Swift parity/CI dashboards using the provided JSON payloads.
# Defaults to the sample data checked into dashboards/data/.
# A third argument (or SWIFT_PIPELINE_METADATA_FEED) optionally renders the
# shared pipeline metadata summary.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
swift_bin="${SWIFT_BIN:-swift}"

default_parity="${repo_root}/dashboards/data/mobile_parity.sample.json"
default_ci="${repo_root}/dashboards/data/mobile_ci.sample.json"
default_pipeline="${repo_root}/dashboards/data/mobile_pipeline_metadata.sample.json"

parity_json="${1:-${SWIFT_PARITY_FEED:-$default_parity}}"
ci_json="${2:-${SWIFT_CI_FEED:-$default_ci}}"
pipeline_json="${3:-${SWIFT_PIPELINE_METADATA_FEED:-$default_pipeline}}"
module_cache="${SWIFT_MODULECACHE_PATH:-${SWIFT_MODULECACHE_DIR:-${repo_root}/.swift-module-cache}}"
output_dir="${SWIFT_DASHBOARD_OUTPUT_DIR:-}"

mkdir -p "${module_cache}"
if [[ -n "${output_dir}" ]]; then
  mkdir -p "${output_dir}"
fi

echo "=== Swift Norito Parity ==="
parity_output=()
if [[ -n "${output_dir}" ]]; then
  parity_output=(--output "${output_dir}/mobile_parity.txt")
fi
"${swift_bin}" -module-cache-path "${module_cache}" "${repo_root}/dashboards/mobile_parity.swift" "${parity_json}" "${parity_output[@]}"
if [[ -n "${output_dir}" ]]; then
  echo "summary written to ${output_dir}/mobile_parity.txt"
fi
echo
echo "=== Swift CI Health ==="
ci_output=()
if [[ -n "${output_dir}" ]]; then
  ci_output=(--output "${output_dir}/mobile_ci.txt")
fi
"${swift_bin}" -module-cache-path "${module_cache}" "${repo_root}/dashboards/mobile_ci.swift" "${ci_json}" "${ci_output[@]}"
if [[ -n "${output_dir}" ]]; then
  echo "summary written to ${output_dir}/mobile_ci.txt"
fi

if [[ -n "${pipeline_json}" && -f "${pipeline_json}" ]]; then
  echo
  echo "=== Swift Pipeline Metadata ==="
  pipeline_output=()
  if [[ -n "${output_dir}" ]]; then
    pipeline_output=(--output "${output_dir}/mobile_pipeline_metadata.txt")
  fi
  "${swift_bin}" -module-cache-path "${module_cache}" "${repo_root}/dashboards/mobile_pipeline_metadata.swift" "${pipeline_json}" "${pipeline_output[@]}"
  if [[ -n "${output_dir}" ]]; then
    echo "summary written to ${output_dir}/mobile_pipeline_metadata.txt"
  fi
else
  echo
  echo "=== Swift Pipeline Metadata ==="
  echo "Pipeline metadata feed not found (${pipeline_json}); skipping."
fi
