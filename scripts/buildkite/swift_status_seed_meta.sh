#!/usr/bin/env bash
set -euo pipefail

# Helper to seed Buildkite meta-data keys required by the Swift status export pipeline.

usage() {
  cat <<'EOF'
Usage: swift_status_seed_meta.sh [options]

Options:
  --parity-url <url>        HTTPS/S3 URL for the Swift parity feed (required).
  --ci-url <url>            HTTPS/S3 URL for the Swift CI feed (required).
  --slack-webhook <url>     Slack webhook URL for the digest notification (required).
  --meta-prefix <prefix>    Override meta-data key prefix (default: swift_status_export).
  --dry-run                 Print the commands that would run without calling buildkite-agent.
  -h, --help                Show this help message.

The script wraps `buildkite-agent meta-data set` so CI pipelines can be primed with
the feed URLs and Slack webhook expected by `.buildkite/swift-status-export.yml`.
EOF
}

meta_prefix="swift_status_export"
parity_url=""
ci_url=""
slack_url=""
dry_run=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --parity-url)
      parity_url="${2:-}"
      shift 2
      ;;
    --ci-url)
      ci_url="${2:-}"
      shift 2
      ;;
    --slack-webhook)
      slack_url="${2:-}"
      shift 2
      ;;
    --meta-prefix)
      meta_prefix="${2:-}"
      shift 2
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${parity_url}" || -z "${ci_url}" || -z "${slack_url}" ]]; then
  echo "Missing required options. parity-url, ci-url, and slack-webhook must be supplied." >&2
  usage >&2
  exit 1
fi

if [[ "${dry_run}" -eq 0 ]] && ! command -v buildkite-agent >/dev/null 2>&1; then
  echo "buildkite-agent not found in PATH; re-run with --dry-run or install the agent." >&2
  exit 1
fi

set_meta() {
  local key="$1"
  local value="$2"
  if [[ "${dry_run}" -eq 1 ]]; then
    echo "[dry-run] buildkite-agent meta-data set ${key} ******"
  else
    buildkite-agent meta-data set "${key}" "${value}"
  fi
}

set_meta "${meta_prefix}.parity_feed_url" "${parity_url}"
set_meta "${meta_prefix}.ci_feed_url" "${ci_url}"
set_meta "${meta_prefix}.slack_webhook" "${slack_url}"

echo "Seeded Swift status export meta-data keys with prefix '${meta_prefix}'."
