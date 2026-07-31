#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly TLC_MAX_SET_SIZE="1000000"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly TLAPM_FUNCTIONS_SHA256="b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063"
readonly TLAPM_FOLDS_SHA256="aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da"
readonly SEED="19349663"
REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly FIXTURE_DIR="${REPO_ROOT}/crates/iroha_sumeragi_core/tests/fixtures"
readonly EXPECTED="${FIXTURE_DIR}/tlc_replay_witness.tsv"
readonly CONFIG="${FIXTURE_DIR}/tlc_replay_witness.cfg"
readonly NORMALIZER="${REPO_ROOT}/scripts/normalize_sumeragi_v2_tlc_trace.py"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-replay.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
tlapm_compat_dir="${run_dir}/tlapm-stdlib"
mkdir -p "$tlapm_compat_dir"
ln -s "${TLAPM_STDLIB}/Functions.tla" "${tlapm_compat_dir}/Functions.tla"
ln -s "${TLAPM_STDLIB}/Folds.tla" "${tlapm_compat_dir}/Folds.tla"
normalized_trace="${run_dir}/trace.tsv"
tlc_log="${run_dir}/tlc.log"

set +e
(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" -XX:+UseParallelGC "-DTLA-Library=${tlapm_compat_dir}" \
    -cp "$TLA2TOOLS_JAR" tlc2.TLC \
    -maxSetSize "$TLC_MAX_SET_SIZE" \
    -metadir "${run_dir}/states" \
    -workers 1 \
    -depth 500 \
    -seed "$SEED" \
    -simulate num=200 \
    -config "$CONFIG" \
    -tool \
    SumeragiV2TraceWitness
) >"$tlc_log" 2>&1
tlc_status=$?
set -e

# The witness module intentionally asks TLC to violate NoDecision. The
# invariant-violation exit code plus the normalizer's exact tool-message checks
# prove that the trace ends at the first durable model decision; any other
# status is a tooling/model failure.
if [[ "$tlc_status" -ne 12 || ! -s "$tlc_log" ]]; then
  cat "$tlc_log" >&2
  echo "TLC did not emit the expected decision witness (status=${tlc_status})" >&2
  exit 1
fi
sumeragi_v2_tlc_assert_regular_log "replay-decision-witness" "$tlc_log"

if ! python3 "$NORMALIZER" "$tlc_log" --seed "$SEED" >"$normalized_trace"; then
  echo "TLC decision witness normalization failed" >&2
  exit 1
fi
if ! cmp -s "$EXPECTED" "$normalized_trace"; then
  diff -u "$EXPECTED" "$normalized_trace" || true
  echo "normalized Sumeragi v2 replay witness drifted" >&2
  exit 1
fi

action_count="$(grep -c '^[0-9]' "$normalized_trace")"
bash "$REPO_ROOT/scripts/formal/run_sumeragi_v2_harness.sh" --model-replay
echo "TLC replay witness matches ${action_count} checked-in production actions and the production reducer"
