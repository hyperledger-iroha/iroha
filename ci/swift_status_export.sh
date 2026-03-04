#!/usr/bin/env bash
set -euo pipefail

# Prototype CI entrypoint for generating the Swift weekly status digest.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/swift-status-export.XXXXXX")"
cleanup() {
  rm -rf "${tmp_dir}"
}
trap cleanup EXIT

copy_feed_artifact() {
  local source="$1"
  local dest="$2"
  local meta_key="$3"

  if [[ -z "${dest}" ]]; then
    return 0
  fi
  if [[ "${source}" == "${dest}" ]]; then
    return 0
  fi

  mkdir -p "$(dirname "${dest}")"
  cp "${source}" "${dest}"
  echo "[swift-status] exported feed copy to ${dest}"

  if [[ -n "${meta_key}" ]]; then
    if command -v buildkite-agent >/dev/null 2>&1; then
      if ! buildkite-agent meta-data set "${meta_key}" "${dest}"; then
        echo "[swift-status] warning: failed to set meta-data key ${meta_key}" >&2
      fi
    else
      echo "[swift-status] buildkite-agent not available; skipping meta-data set for ${meta_key}"
    fi
  fi
}

download_feed() {
  local url="$1"
  local path="$2"
  python3 - "${url}" "${path}" <<'PY'
import sys
from pathlib import Path
import urllib.request

source, dest = sys.argv[1], sys.argv[2]
with urllib.request.urlopen(source) as response:  # nosec: B310 - controlled input
    charset = response.headers.get_content_charset() or "utf-8"
    data = response.read().decode(charset)
Path(dest).write_text(data, encoding="utf-8")
PY
}

resolve_secret() {
  local name="$1"
  local value="${!name:-}"
  if [[ -n "${value}" ]]; then
    printf '%s' "${value}"
    return 0
  fi

  local file_var="${name}_FILE"
  local file_path="${!file_var:-}"
  if [[ -n "${file_path}" ]]; then
    if [[ ! -r "${file_path}" ]]; then
      echo "[swift-status] unable to read secret file ${file_path} for ${name}" >&2
      return 1
    fi
    local file_contents
    file_contents="$(<"${file_path}")"
    printf '%s' "${file_contents}"
    return 0
  fi

  local meta_var="${name}_META_KEY"
  local meta_key="${!meta_var:-}"
  if [[ -n "${meta_key}" ]]; then
    if ! command -v buildkite-agent >/dev/null 2>&1; then
      echo "[swift-status] buildkite-agent not available to resolve meta-data key ${meta_key} for ${name}" >&2
      return 1
    fi
    local meta_value
    if ! meta_value="$(buildkite-agent meta-data get "${meta_key}")"; then
      echo "[swift-status] failed to resolve Buildkite meta-data key ${meta_key} for ${name}" >&2
      return 1
    fi
    printf '%s' "${meta_value}"
    return 0
  fi

  return 1
}

record_meta() {
  local key="$1"
  local value="$2"
  local label="$3"

  if [[ -z "${key}" ]]; then
    return 0
  fi
  if command -v buildkite-agent >/dev/null 2>&1; then
    if ! buildkite-agent meta-data set "${key}" "${value}"; then
      echo "[swift-status] warning: failed to set meta-data ${key}" >&2
    fi
  else
    echo "[swift-status] buildkite-agent unavailable; skipping meta-data export for ${label}" >&2
  fi
}

parity_path="${SWIFT_PARITY_FEED_PATH:-}"
ci_path="${SWIFT_CI_FEED_PATH:-}"
pipeline_metadata_path="${SWIFT_PIPELINE_METADATA_FEED_PATH:-${SWIFT_PIPELINE_METADATA_FEED:-${SWIFT_PIPELINE_METADATA_PATH:-${SWIFT_PIPELINE_METADATA:-${MOBILE_PARITY_PIPELINE_METADATA:-}}}}}"

parity_url=""
if parity_url_resolved="$(resolve_secret "SWIFT_PARITY_FEED_URL")"; then
  parity_url="${parity_url_resolved}"
fi
ci_url=""
if ci_url_resolved="$(resolve_secret "SWIFT_CI_FEED_URL")"; then
  ci_url="${ci_url_resolved}"
fi

pipeline_metadata_url=""
for candidate in SWIFT_PIPELINE_METADATA_FEED_URL SWIFT_PIPELINE_METADATA_URL; do
  if pipeline_metadata_url_resolved="$(resolve_secret "${candidate}")"; then
    pipeline_metadata_url="${pipeline_metadata_url_resolved}"
    break
  fi
done

if [[ -n "${parity_url}" ]]; then
  parity_path="${tmp_dir}/parity.json"
  download_feed "${parity_url}" "${parity_path}"
fi
if [[ -z "${parity_path}" ]]; then
  parity_path="${repo_root}/dashboards/data/mobile_parity.sample.json"
fi

if [[ -n "${ci_url}" ]]; then
  ci_path="${tmp_dir}/ci.json"
  download_feed "${ci_url}" "${ci_path}"
fi
if [[ -z "${ci_path}" ]]; then
  ci_path="${repo_root}/dashboards/data/mobile_ci.sample.json"
fi

if [[ -n "${pipeline_metadata_url}" ]]; then
  pipeline_metadata_path="${tmp_dir}/pipeline_metadata.json"
  download_feed "${pipeline_metadata_url}" "${pipeline_metadata_path}"
fi
if [[ -z "${pipeline_metadata_path}" ]]; then
  pipeline_metadata_path="${repo_root}/dashboards/data/mobile_pipeline_metadata.sample.json"
fi
if [[ ! -f "${pipeline_metadata_path}" ]]; then
  echo "[swift-status] warning: pipeline metadata feed not found at ${pipeline_metadata_path}; falling back to sample data" >&2
  pipeline_metadata_path="${repo_root}/dashboards/data/mobile_pipeline_metadata.sample.json"
fi

declare -a telemetry_args=()
declare -a collector_args=()
parity_export_path="${SWIFT_PARITY_FEED_EXPORT_PATH:-}"
ci_export_path="${SWIFT_CI_FEED_EXPORT_PATH:-}"
pipeline_metadata_export_path="${SWIFT_PIPELINE_METADATA_EXPORT_PATH:-${SWIFT_PIPELINE_METADATA_FEED_EXPORT_PATH:-}}"
parity_export_meta="${SWIFT_PARITY_FEED_PATH_META_KEY:-}"
ci_export_meta="${SWIFT_CI_FEED_PATH_META_KEY:-}"
pipeline_metadata_export_meta="${SWIFT_PIPELINE_METADATA_PATH_META_KEY:-${SWIFT_PIPELINE_METADATA_FEED_PATH_META_KEY:-}}"

if [[ -n "${SWIFT_TELEMETRY_JSON:-}" ]]; then
  telemetry_args+=(--telemetry-json "${SWIFT_TELEMETRY_JSON}")
fi

if [[ -n "${SWIFT_TELEMETRY_SALT_STATUS:-}" ]]; then
  collector_args+=(--salt-config "${SWIFT_TELEMETRY_SALT_STATUS}")
fi
if [[ -n "${SWIFT_TELEMETRY_PROFILE_ALIGNMENT:-}" ]]; then
  collector_args+=(--profile-alignment "${SWIFT_TELEMETRY_PROFILE_ALIGNMENT}")
fi
if [[ -n "${SWIFT_TELEMETRY_OVERRIDES_STORE:-}" ]]; then
  collector_args+=(--overrides-store "${SWIFT_TELEMETRY_OVERRIDES_STORE}")
fi
if [[ -n "${SWIFT_TELEMETRY_SCHEMA_DIFF_REPORT:-}" ]]; then
  collector_args+=(--schema-diff-report "${SWIFT_TELEMETRY_SCHEMA_DIFF_REPORT}")
fi
if [[ -n "${SWIFT_TELEMETRY_SCHEMA_VERSION:-}" ]]; then
  collector_args+=(--schema-version "${SWIFT_TELEMETRY_SCHEMA_VERSION}")
fi
if [[ -n "${SWIFT_TELEMETRY_NOTES_FILE:-}" ]]; then
  collector_args+=(--notes-file "${SWIFT_TELEMETRY_NOTES_FILE}")
fi
if [[ -n "${SWIFT_TELEMETRY_NOTES:-}" ]]; then
  while IFS= read -r note_line; do
    [[ -z "${note_line}" ]] && continue
    collector_args+=(--note "${note_line}")
  done <<<"${SWIFT_TELEMETRY_NOTES}"
fi

if (( ${#collector_args[@]} )); then
  telemetry_file="${tmp_dir}/telemetry.status.json"
  echo "[swift-status] collecting telemetry metadata"
  python3 "${repo_root}/scripts/swift_collect_redaction_status.py" \
    "${collector_args[@]}" \
    --output "${telemetry_file}"
  telemetry_args+=(--telemetry-json "${telemetry_file}")
fi

if [[ -n "${SWIFT_TELEMETRY_SALT_EPOCH:-}" ]]; then
  telemetry_args+=(--salt-epoch "${SWIFT_TELEMETRY_SALT_EPOCH}")
fi
if [[ -n "${SWIFT_TELEMETRY_SALT_ROTATION_HOURS:-}" ]]; then
  telemetry_args+=(--salt-rotation-hours "${SWIFT_TELEMETRY_SALT_ROTATION_HOURS}")
fi
if [[ -n "${SWIFT_TELEMETRY_OVERRIDES_OPEN:-}" ]]; then
  telemetry_args+=(--overrides-open "${SWIFT_TELEMETRY_OVERRIDES_OPEN}")
fi
if [[ -n "${SWIFT_TELEMETRY_PROFILE_ALIGNMENT:-}" ]]; then
  telemetry_args+=(--profile-alignment "${SWIFT_TELEMETRY_PROFILE_ALIGNMENT}")
fi
if [[ -n "${SWIFT_TELEMETRY_SCHEMA_VERSION:-}" ]]; then
  telemetry_args+=(--schema-version "${SWIFT_TELEMETRY_SCHEMA_VERSION}")
fi
if [[ -n "${SWIFT_TELEMETRY_NOTES:-}" && ${#collector_args[@]} -eq 0 ]]; then
  while IFS= read -r note_line; do
    [[ -z "${note_line}" ]] && continue
    telemetry_args+=(--note "${note_line}")
  done <<<"${SWIFT_TELEMETRY_NOTES}"
fi

declare -a enrich_args=()
if (( ${#telemetry_args[@]} )); then
  enrich_args+=("${telemetry_args[@]}")
fi
pipeline_metadata="${pipeline_metadata_path}"
if [[ -n "${pipeline_metadata}" && -f "${pipeline_metadata}" ]]; then
  enrich_args+=(--pipeline-metadata "${pipeline_metadata}")
fi

if (( ${#enrich_args[@]} )); then
  if [[ "${parity_path}" == "${repo_root}/dashboards/data/mobile_parity.sample.json" ]]; then
    parity_copy="${tmp_dir}/parity.enriched.json"
    cp "${parity_path}" "${parity_copy}"
    parity_path="${parity_copy}"
  fi
  echo "[swift-status] enriching parity feed metadata"
  python3 "${repo_root}/scripts/swift_enrich_parity_feed.py" \
    --input "${parity_path}" \
    --output "${parity_path}" \
    "${enrich_args[@]}"
fi

copy_feed_artifact "${parity_path}" "${parity_export_path}" "${parity_export_meta}"
copy_feed_artifact "${ci_path}" "${ci_export_path}" "${ci_export_meta}"
copy_feed_artifact "${pipeline_metadata_path}" "${pipeline_metadata_export_path}" "${pipeline_metadata_export_meta}"

echo "[swift-status] validating feeds"
python3 "${repo_root}/scripts/check_swift_dashboard_data.py" "${parity_path}" "${ci_path}"
python3 "${repo_root}/scripts/check_swift_pipeline_metadata.py" "${pipeline_metadata_path}"

slack_webhook=""
if slack_webhook_resolved="$(resolve_secret "SWIFT_STATUS_SLACK_WEBHOOK")"; then
  slack_webhook="${slack_webhook_resolved}"
fi

declare -a slack_args=()
if [[ -n "${slack_webhook}" ]]; then
  echo "[swift-status] Slack webhook resolved; exporter will notify once digest is generated"
  slack_args=(--slack-webhook "${slack_webhook}")
  if [[ -n "${SWIFT_STATUS_SLACK_PREVIEW_LINES:-}" ]]; then
    slack_args+=(--slack-preview-lines "${SWIFT_STATUS_SLACK_PREVIEW_LINES}")
  fi
else
  echo "[swift-status] Slack webhook not configured; skipping notification"
fi

declare -a metrics_args=()
disable_metrics="${SWIFT_STATUS_DISABLE_METRICS:-}"
metrics_path="${SWIFT_STATUS_METRICS_PATH:-}"
metrics_state="${SWIFT_STATUS_METRICS_STATE:-}"

if [[ -z "${metrics_path}" && -z "${disable_metrics}" ]]; then
  metrics_path="${repo_root}/artifacts/swift/swift_status_metrics.prom"
fi

if [[ -n "${metrics_path}" && -z "${metrics_state}" && -z "${disable_metrics}" ]]; then
  metrics_state="${repo_root}/artifacts/swift/swift_status_metrics_state.json"
fi

if [[ -n "${metrics_path}" ]]; then
  metrics_args=(--metrics-path "${metrics_path}")
  if [[ -n "${metrics_state}" ]]; then
    metrics_args+=(--metrics-state "${metrics_state}")
  fi
  echo "[swift-status] metrics output enabled (${metrics_path})"
elif [[ -n "${disable_metrics}" ]]; then
  echo "[swift-status] metrics output disabled via SWIFT_STATUS_DISABLE_METRICS"
else
  echo "[swift-status] metrics output disabled"
fi

declare -a readiness_args=()
if [[ -n "${SWIFT_STATUS_SKIP_DEFAULT_READINESS_DOCS:-}" ]]; then
  readiness_args+=(--skip-default-readiness-docs)
fi
if [[ -n "${SWIFT_STATUS_READINESS_DOCS:-}" ]]; then
  while IFS= read -r doc_entry; do
    doc_entry="${doc_entry//$'\r'/}"
    [[ -z "${doc_entry}" ]] && continue
    readiness_args+=(--readiness-doc "${doc_entry}")
  done <<<"${SWIFT_STATUS_READINESS_DOCS}"
fi

output_path="${SWIFT_STATUS_EXPORT_OUT:-${tmp_dir}/weekly_digest.md}"
mkdir -p "$(dirname "${output_path}")"
echo "[swift-status] rendering status export to ${output_path}"
cmd=(python3 "${repo_root}/scripts/swift_status_export.py" --parity "${parity_path}" --ci "${ci_path}" --format markdown)
if [[ -n "${slack_args+x}" && ${#slack_args[@]} -gt 0 ]]; then
  cmd+=("${slack_args[@]}")
fi
if [[ -n "${metrics_args+x}" && ${#metrics_args[@]} -gt 0 ]]; then
  cmd+=("${metrics_args[@]}")
fi
if (( ${#readiness_args[@]} )); then
  cmd+=("${readiness_args[@]}")
fi
"${cmd[@]}" > "${output_path}"

summary_path="${SWIFT_STATUS_SUMMARY_OUT:-${tmp_dir}/summary.json}"
mkdir -p "$(dirname "${summary_path}")"
summary_cmd=(python3 "${repo_root}/scripts/swift_status_export.py" --parity "${parity_path}" --ci "${ci_path}" --format json)
if (( ${#readiness_args[@]} )); then
  summary_cmd+=("${readiness_args[@]}")
fi
"${summary_cmd[@]}" > "${summary_path}"

if [[ -n "${SWIFT_STATUS_ARCHIVE_DIR:-}" ]]; then
  archive_dir="${SWIFT_STATUS_ARCHIVE_DIR}"
  archive_tag="${SWIFT_STATUS_ARCHIVE_TAG:-$(date -u +%Y-%m-%d)}"
  archive_base="${SWIFT_STATUS_ARCHIVE_BASENAME:-swift_weekly_digest}"
  mkdir -p "${archive_dir}"
  archive_digest="${archive_dir}/${archive_base}_${archive_tag}.md"
  archive_summary="${archive_dir}/${archive_base}_${archive_tag}.json"
  cp "${output_path}" "${archive_digest}"
  cp "${summary_path}" "${archive_summary}"
  echo "[swift-status] archived digest to ${archive_digest}"
  echo "[swift-status] archived summary to ${archive_summary}"
  record_meta "${SWIFT_STATUS_ARCHIVE_META_KEY:-}" "${archive_digest}" "digest archive"
  record_meta "${SWIFT_STATUS_ARCHIVE_SUMMARY_META_KEY:-}" "${archive_summary}" "summary archive"
fi

echo "[swift-status] done"
