#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
readonly PROFILE="${1:-ci}"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

[[ -f "$TLA2TOOLS_JAR" ]] || {
  echo "pinned TLA2Tools v${TLA2TOOLS_VERSION} is required at ${TLA2TOOLS_JAR}" >&2
  echo "run scripts/formal/install_sumeragi_v2_tla2tools.sh first" >&2
  exit 1
}
actual_sha256="$(hash_file "$TLA2TOOLS_JAR")"
if [[ "$actual_sha256" != "$TLA2TOOLS_SHA256" ]]; then
  echo "TLA2Tools checksum mismatch" >&2
  echo "expected: ${TLA2TOOLS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi
command -v java >/dev/null 2>&1 || {
  echo "Java is required for TLC" >&2
  exit 1
}

case "$PROFILE" in
  ci)
    readonly TRACE_COUNT=100
    readonly TRACE_DEPTH=100
    ;;
  nightly)
    readonly TRACE_COUNT=1000
    readonly TRACE_DEPTH=200
    ;;
  *)
    echo "usage: $0 [ci|nightly] [configuration ...]" >&2
    exit 2
    ;;
esac
shift || true

allowed_configs=(
  quorum_count
  quorum_stake
  safety_count
  safety_stake
  chain_epoch
  liveness
)
if (($#)); then
  configs=("$@")
else
  configs=("${allowed_configs[@]}")
fi
for config in "${configs[@]}"; do
  if [[ ! " ${allowed_configs[*]} " =~ " ${config} " ]]; then
    echo "unknown Sumeragi v2 TLC configuration: ${config}" >&2
    exit 2
  fi
done

echo "[tlc] COUNTEREXAMPLE SEARCH ONLY — THIS IS NOT DEDUCTIVE PROOF EVIDENCE"
echo "[tlc] pinned TLA2Tools v${TLA2TOOLS_VERSION}; profile=${PROFILE}"

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-tlc.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
seed=424242
for config in "${configs[@]}"; do
  cfg="${config}.cfg"
  metadir="${run_dir}/${config}"
  mkdir -p "$metadir"
  echo "[tlc] bounded check ${cfg}"
  common=(
    java -cp "$TLA2TOOLS_JAR" tlc2.TLC
    -cleanup
    -noGenerateSpecTE
    -metadir "$metadir"
    -workers 1
    -config "$cfg"
  )
  (
    cd "$FORMAL_DIR"
    case "$config" in
      quorum_count|quorum_stake)
        "${common[@]}" SumeragiV2.tla
        ;;
      *)
        "${common[@]}" -depth "$TRACE_DEPTH" -seed "$seed" -aril 0 \
          -simulate "num=${TRACE_COUNT}" SumeragiV2.tla
        ;;
    esac
  )
  seed=$((seed + 7919))
done
echo "[tlc] bounded searches found no counterexample; no proof status was changed"
