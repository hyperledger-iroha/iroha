#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ci/check_android_dashboard_parity.sh [options]

Options (override env defaults):
  --android <path>     Android dashboard JSON export
  --rust <path>        Rust dashboard JSON export
  --allowance <path>   Allowance JSON describing tolerated differences
  --artifact <path>    Recorded diff artefact to compare against
  --archive-dir <dir>  Optional directory to store timestamped snapshots
  -h, --help           Show this help text

Environment overrides:
  ANDROID_DASHBOARD_JSON
  RUST_DASHBOARD_JSON
  ANDROID_DASHBOARD_ALLOWANCES
  ANDROID_DASHBOARD_PARITY_ARTIFACT
  ANDROID_DASHBOARD_PARITY_ARCHIVE_DIR
EOF
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

android_dash="${ANDROID_DASHBOARD_JSON:-dashboards/grafana/android_telemetry_overview.json}"
rust_dash="${RUST_DASHBOARD_JSON:-dashboards/grafana/rust_android_client_telemetry.json}"
allowances="${ANDROID_DASHBOARD_ALLOWANCES:-dashboards/data/android_rust_dashboard_allowances.json}"
archive_dir="${ANDROID_DASHBOARD_PARITY_ARCHIVE_DIR:-}"

while (($#)); do
  case "$1" in
    --android)
      [[ $# -ge 2 ]] || { echo "Missing value for $1" >&2; usage >&2; exit 2; }
      android_dash="$2"
      shift 2
      ;;
    --rust)
      [[ $# -ge 2 ]] || { echo "Missing value for $1" >&2; usage >&2; exit 2; }
      rust_dash="$2"
      shift 2
      ;;
    --allowance)
      [[ $# -ge 2 ]] || { echo "Missing value for $1" >&2; usage >&2; exit 2; }
      allowances="$2"
      shift 2
      ;;
    --artifact)
      [[ $# -ge 2 ]] || { echo "Missing value for $1" >&2; usage >&2; exit 2; }
      artifact="$2"
      shift 2
      ;;
    --archive-dir)
      [[ $# -ge 2 ]] || { echo "Missing value for $1" >&2; usage >&2; exit 2; }
      archive_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

tmp_output="$(mktemp)"
trap 'rm -f "${tmp_output}"' EXIT

cd "${repo_root}"
python3 scripts/telemetry/compare_dashboards.py \
  --android "${android_dash}" \
  --rust "${rust_dash}" \
  --allow-file "${allowances}" \
  --output "${tmp_output}"

if [[ -n "${archive_dir}" ]]; then
  mkdir -p "${archive_dir}"
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  archive_path="${archive_dir%/}/android_vs_rust-${timestamp}.json"
  cp "${tmp_output}" "${archive_path}"
  echo "[android-dashboard-parity] archived snapshot to ${archive_path}"
fi

if ! diff -u "${artifact}" "${tmp_output}" > /dev/null; then
    echo "[android-dashboard-parity] recorded artifact ${artifact} is out of date."
    echo "[android-dashboard-parity] regenerate via:"
    echo "  python3 scripts/telemetry/compare_dashboards.py \\"
    echo "    --android ${android_dash} \\"
    echo "    --rust ${rust_dash} \\"
    echo "    --allow-file ${allowances} \\"
    echo "    --output ${artifact}"
    diff -u "${artifact}" "${tmp_output}" || true
    exit 1
fi

echo "[android-dashboard-parity] dashboards match recorded artifact ${artifact}"
