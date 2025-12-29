#!/usr/bin/env bash
set -euo pipefail

function usage() {
  cat <<'EOF'
Usage: scripts/verify_sorafs_fixtures.sh [--dir <PATH>] [--allow-online]

Verifies the SoraFS gateway fixture bundle by invoking
`cargo xtask sorafs-gateway-fixtures --verify`.

Options:
  --dir <PATH>       Fixture directory to verify (default: fixtures/sorafs_gateway/1.0.0)
  --allow-online     Allow Cargo to access the network (default: offline)
  -h, --help         Show this message.
EOF
}

FIXTURE_DIR="fixtures/sorafs_gateway/1.0.0"
ALLOW_ONLINE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir)
      [[ $# -lt 2 ]] && { echo "error: --dir requires a path" >&2; usage; exit 1; }
      FIXTURE_DIR="$2"
      shift 2
      ;;
    --allow-online)
      ALLOW_ONLINE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option $1" >&2
      usage
      exit 1
      ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
if [[ "${ALLOW_ONLINE}" -eq 0 ]]; then
  export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
fi

echo "[verify] checking fixtures under ${FIXTURE_DIR}"
cargo xtask sorafs-gateway-fixtures --verify --out "${FIXTURE_DIR}"
