#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly TLC_MAX_SET_SIZE="1000000"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly TLAPM_FUNCTIONS_SHA256="b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063"
readonly TLAPM_FOLDS_SHA256="aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da"
readonly TLC_FINISHED_PATTERN='^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) at \([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\)$'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"
readonly PROFILE="${1:-ci}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) readonly TLAPM_PLATFORM="x86_64-linux-gnu" ;;
  Darwin-arm64) readonly TLAPM_PLATFORM="arm64-darwin" ;;
  *)
    echo "unsupported TLAPM host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac
readonly TLAPM_STDLIB="${TLAPM_STDLIB:-${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${TLAPM_PLATFORM}/tlapm/lib/tlapm/stdlib}"

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
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required for TLC" >&2
  exit 1
}
for module in Functions Folds; do
  [[ -f "${TLAPM_STDLIB}/${module}.tla" ]] || {
    echo "pinned TLAPM ${TLAPM_COMMIT} standard library is required at ${TLAPM_STDLIB}" >&2
    echo "run scripts/formal/install_sumeragi_v2_tlapm.sh first" >&2
    exit 1
  }
done
for module_and_hash in \
  "Functions:${TLAPM_FUNCTIONS_SHA256}" \
  "Folds:${TLAPM_FOLDS_SHA256}"; do
  module="${module_and_hash%%:*}"
  expected_sha256="${module_and_hash#*:}"
  actual_sha256="$(hash_file "${TLAPM_STDLIB}/${module}.tla")"
  if [[ "$actual_sha256" != "$expected_sha256" ]]; then
    echo "pinned TLAPM standard-library checksum mismatch for ${module}.tla" >&2
    echo "expected: ${expected_sha256}" >&2
    echo "actual:   ${actual_sha256}" >&2
    exit 1
  fi
done

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
  effective_lock_acquisition
  resume_locked_commit_witness
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
tlapm_compat_dir="${run_dir}/tlapm-stdlib"
mkdir -p "$tlapm_compat_dir"
ln -s "${TLAPM_STDLIB}/Functions.tla" "${tlapm_compat_dir}/Functions.tla"
ln -s "${TLAPM_STDLIB}/Folds.tla" "${tlapm_compat_dir}/Folds.tla"
seed=424242
for config in "${configs[@]}"; do
  cfg="${config}.cfg"
  metadir="${run_dir}/${config}"
  tlc_log="${run_dir}/${config}.log"
  mkdir -p "$metadir"
  echo "[tlc] bounded check ${cfg}"
  simulation_config=0
  case "$config" in
    safety_count|safety_stake|chain_epoch|liveness)
      simulation_config=1
      ;;
  esac
  common=(
    "$JAVA_BIN" -XX:+UseParallelGC "-DTLA-Library=${tlapm_compat_dir}"
    -cp "$TLA2TOOLS_JAR" tlc2.TLC
    -cleanup
    -maxSetSize "$TLC_MAX_SET_SIZE"
    -metadir "$metadir"
    -workers 1
    -config "$cfg"
  )
  set +e
  (
    cd "$FORMAL_DIR"
    case "$config" in
      quorum_count|quorum_stake)
        "${common[@]}" SumeragiV2.tla
        ;;
      safety_count|safety_stake)
        "${common[@]}" -depth "$TRACE_DEPTH" -seed "$seed" -aril 0 \
          -simulate "num=${TRACE_COUNT}" SumeragiV2.tla
        ;;
      chain_epoch)
        "${common[@]}" -depth "$TRACE_DEPTH" -seed "$seed" -aril 0 \
          -simulate "num=${TRACE_COUNT}" SumeragiV2ChainEpoch.tla
        ;;
      liveness)
        "${common[@]}" -depth "$TRACE_DEPTH" -seed "$seed" -aril 0 \
          -simulate "num=${TRACE_COUNT}" SumeragiV2AsyncNetwork.tla
        ;;
      effective_lock_acquisition)
        "${common[@]}" SumeragiV2EffectiveLockAcquisition.tla
        ;;
      resume_locked_commit_witness)
        "${common[@]}" SumeragiV2ResumeVoteWitness.tla
        ;;
      *)
        echo "internal error: unclassified TLC configuration ${config}" >&2
        exit 2
        ;;
    esac
  ) >"$tlc_log" 2>&1
  tlc_status=$?
  set -e
  cat "$tlc_log"

  if [[ "$config" == "resume_locked_commit_witness" ]]; then
    # This module negates the required recovery state solely so TLC emits the
    # shortest bounded trace reaching it.  Exit 12 plus the exact invariant
    # name distinguishes that witness from a type error, deadlock, or tool
    # failure; an accidental no-counterexample result must also fail closed.
    if [[ "$tlc_status" -ne 12 ]] ||
      ! grep -Fq \
        "Invariant NoRecoveredHistoricalLockedCommitSigning is violated." \
        "$tlc_log"; then
      echo "TLC did not emit the exact locked-Commit recovery witness (status=${tlc_status})" >&2
      exit 1
    fi
    echo "[tlc] observed required historical locked-Commit ResumeVote witness"
  elif ((simulation_config)); then
    simulation_header_count="$(
      grep -Ec "^Running Random Simulation with seed ${seed} with 1 worker " \
        "$tlc_log" || true
    )"
    initial_state_count="$(
      grep -Fxc "Computed 1 initial states..." "$tlc_log" || true
    )"
    progress_count="$(
      grep -Ec '^Progress: [1-9][0-9]* states checked[.]$' "$tlc_log" || true
    )"
    finished_count="$(
      grep -Ec "$TLC_FINISHED_PATTERN" "$tlc_log" || true
    )"
    error_count="$(grep -Ec '^(Error:|Deadlock reached[.])' "$tlc_log" || true)"
    if [[ "$tlc_status" -ne 0 \
      || "$simulation_header_count" != 1 \
      || "$initial_state_count" != 1 \
      || "$progress_count" -lt 1 \
      || "$finished_count" != 1 \
      || "$error_count" != 0 ]]; then
      echo "TLC bounded simulation ${cfg} did not report one exact successful run (status=${tlc_status}, header=${simulation_header_count}, init=${initial_state_count}, progress=${progress_count}, finished=${finished_count}, errors=${error_count})" >&2
      exit 1
    fi
    echo "[tlc] completed ${TRACE_COUNT} deterministic traces at depth ${TRACE_DEPTH}"
  elif [[ "$tlc_status" -ne 0 ]] ||
    [[ "$(grep -Fxc "Model checking completed. No error has been found." "$tlc_log" || true)" != 1 ]]; then
    echo "TLC bounded check ${cfg} did not report exact successful completion (status=${tlc_status})" >&2
    exit 1
  fi
  seed=$((seed + 7919))
done
echo "[tlc] all exhaustive searches, deterministic simulations, and the recovery witness had their exact expected outcomes; no proof status was changed"
