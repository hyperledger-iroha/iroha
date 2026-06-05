#!/usr/bin/env bash
# Run TLC checks for the ZK verifier admission model.
#
# Prerequisites:
#   - java on PATH
#   - TLC_JAR or TLA2TOOLS_JAR set, or the Apalache-bundled TLC jar installed at
#     target/apalache/toolchains/v${APALACHE_VERSION:-0.52.2}/lib/apalache.jar
#
# Usage:
#   scripts/formal/zk_tlc.sh fast
#   scripts/formal/zk_tlc.sh mutations
#   scripts/formal/zk_tlc.sh all
#   scripts/formal/zk_tlc.sh bug-missing-vk

set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: scripts/formal/zk_tlc.sh {fast|mutations|all|bug-missing-vk|bug-wrong-vk-hash|bug-circuit-mismatch|bug-omit-stark-domain-binding|bug-disabled-backend-acceptance|bug-diagnostic-endpoint-promotion|bug-fastpq-claim-mismatch|bug-replay-acceptance}

Modes:
  fast                                  Run the passing verifier-admission model.
  mutations                             Run every expected-failure mutation config.
  all                                   Run fast, then every mutation.
  bug-*                                 Run one mutation and require an invariant counterexample.

Environment:
  TLC_JAR / TLA2TOOLS_JAR               Path to a TLC-compatible jar.
  APALACHE_VERSION                      Version used in the default Apalache toolchain path.
  TLC_WORKERS                           TLC worker count, default 1.
  TLC_JAVA_OPTS                         Additional options passed to java.
EOF
}

mode="${1:-fast}"

case "$mode" in
  -h|--help|help)
    usage
    exit 0
    ;;
esac

mutation_modes=(
  bug-missing-vk
  bug-wrong-vk-hash
  bug-circuit-mismatch
  bug-omit-stark-domain-binding
  bug-disabled-backend-acceptance
  bug-diagnostic-endpoint-promotion
  bug-fastpq-claim-mismatch
  bug-replay-acceptance
)

if [[ "$mode" == "all" ]]; then
  "$0" fast
  "$0" mutations
  exit 0
fi

if [[ "$mode" == "mutations" ]]; then
  for mutation in "${mutation_modes[@]}"; do
    "$0" "$mutation"
  done
  exit 0
fi

root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
spec_dir="$root_dir/docs/formal/zk"
module="ZkVerifierAdmission"
apalache_version="${APALACHE_VERSION:-0.52.2}"
default_tlc_jar="$root_dir/target/apalache/toolchains/v${apalache_version}/lib/apalache.jar"
expect_failure=0

case "$mode" in
  fast)
    cfg_file="$spec_dir/ZkVerifierAdmission_fast.cfg"
    ;;
  bug-missing-vk|bug-wrong-vk-hash|bug-circuit-mismatch|bug-omit-stark-domain-binding|bug-disabled-backend-acceptance|bug-diagnostic-endpoint-promotion|bug-fastpq-claim-mismatch|bug-replay-acceptance)
    cfg_bug_name="${mode#bug-}"
    cfg_bug_name="${cfg_bug_name//-/_}"
    cfg_file="$spec_dir/ZkVerifierAdmission_bug_${cfg_bug_name}.cfg"
    expect_failure=1
    ;;
  *)
    usage
    exit 2
    ;;
esac

tlc_jar="${TLC_JAR:-${TLA2TOOLS_JAR:-$default_tlc_jar}}"
if [[ "$tlc_jar" != /* ]]; then
  tlc_jar="$root_dir/$tlc_jar"
fi

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

workers="${TLC_WORKERS:-1}"
run_dir="$root_dir/target/tlc/zk-$mode"
mkdir -p "$run_dir"
log_file="$run_dir/tlc.log"

set +e
(
  cd "$spec_dir"
  java ${TLC_JAVA_OPTS:-} -cp "$tlc_jar" tlc2.TLC \
    -cleanup \
    -workers "$workers" \
    -metadir "$run_dir" \
    -config "$cfg_file" \
    "$module"
) 2>&1 | tee "$log_file"
tlc_status=${PIPESTATUS[0]}
set -e

if [[ "$expect_failure" -eq 1 ]]; then
  if [[ "$tlc_status" -eq 0 ]]; then
    echo "error: ZK TLC '$mode' unexpectedly passed" >&2
    exit 1
  fi
  if grep -Eq "Invariant .* is violated|Error: Invariant|Error: The invariant of .* is equal to FALSE|Temporal properties were violated" "$log_file"; then
    echo "[formal] ZK TLC '$mode' produced the expected failure"
    exit 0
  fi
  echo "error: ZK TLC '$mode' failed without the expected invariant violation" >&2
  exit "$tlc_status"
fi

if [[ "$tlc_status" -ne 0 ]]; then
  exit "$tlc_status"
fi

echo "[formal] ZK TLC '$mode' check passed"
