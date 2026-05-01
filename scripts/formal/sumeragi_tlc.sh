#!/bin/bash
set -euo pipefail

mode="${1:-frontier-small}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
spec_dir="$root_dir/docs/formal/sumeragi"
apalache_version="${APALACHE_VERSION:-0.52.2}"
default_tlc_jar="$root_dir/target/apalache/toolchains/v${apalache_version}/lib/apalache.jar"

case "$mode" in
  frontier-small)
    module="SumeragiFrontierRecovery"
    cfg_file="$spec_dir/SumeragiFrontierRecovery_tlc_small.cfg"
    ;;
  *)
    echo "usage: $0 {frontier-small}" >&2
    exit 2
    ;;
esac

tlc_jar="${TLC_JAR:-${TLA2TOOLS_JAR:-$default_tlc_jar}}"
workers="${TLC_WORKERS:-1}"
run_dir="$root_dir/target/tlc/sumeragi-$mode"
mkdir -p "$run_dir"

if [[ ! -f "$cfg_file" ]]; then
  echo "error: missing config '$cfg_file'" >&2
  exit 2
fi

if [[ ! -f "$tlc_jar" ]]; then
  echo "error: TLC jar '$tlc_jar' not found" >&2
  echo "hint: run scripts/formal/install_apalache.sh ${apalache_version}, or set TLC_JAR/TLA2TOOLS_JAR" >&2
  exit 127
fi

if ! command -v java >/dev/null 2>&1; then
  echo "error: java not found" >&2
  exit 127
fi

(
  cd "$spec_dir"
  java ${TLC_JAVA_OPTS:-} -cp "$tlc_jar" tlc2.TLC \
    -cleanup \
    -workers "$workers" \
    -metadir "$run_dir" \
    -config "$cfg_file" \
    "$module"
)

echo "[formal] Sumeragi TLC '$mode' check passed"
