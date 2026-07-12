#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.8.0"
readonly TLA2TOOLS_SHA256="33de7da9ce1b7fffb9d1c184021178dbb051747be48504e65c584c423721a32e"
readonly SEED="19349663"
REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPO_ROOT
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly FIXTURE_DIR="${REPO_ROOT}/crates/iroha_sumeragi_core/tests/fixtures"
readonly EXPECTED="${FIXTURE_DIR}/tlc_replay_witness.tsv"
readonly CONFIG="${FIXTURE_DIR}/tlc_replay_witness.cfg"
readonly NORMALIZER="${REPO_ROOT}/scripts/normalize_sumeragi_v2_tlc_trace.py"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"

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
command -v java >/dev/null 2>&1 || {
  echo "Java is required for TLC" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-replay.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
raw_trace="${run_dir}/trace.json"
normalized_trace="${run_dir}/trace.tsv"
tlc_log="${run_dir}/tlc.log"

set +e
(
  cd "$FORMAL_DIR"
  java -cp "$TLA2TOOLS_JAR" tlc2.TLC \
    -noGenerateSpecTE \
    -metadir "${run_dir}/states" \
    -workers 1 \
    -depth 500 \
    -seed "$SEED" \
    -simulate num=200 \
    -config "$CONFIG" \
    -dumpTrace json "$raw_trace" \
    SumeragiV2TraceWitness
) >"$tlc_log" 2>&1
tlc_status=$?
set -e

# The witness module intentionally asks TLC to violate NoDecision. TLC's
# invariant-violation exit code proves that the JSON ends at the first durable
# model decision; any other status is a tooling/model failure.
if [[ "$tlc_status" -ne 12 || ! -s "$raw_trace" ]]; then
  cat "$tlc_log" >&2
  echo "TLC did not emit the expected decision witness (status=${tlc_status})" >&2
  exit 1
fi

python3 "$NORMALIZER" "$raw_trace" --seed "$SEED" >"$normalized_trace"
if ! cmp -s "$EXPECTED" "$normalized_trace"; then
  diff -u "$EXPECTED" "$normalized_trace" || true
  echo "normalized Sumeragi v2 replay witness drifted" >&2
  exit 1
fi

action_count="$(grep -c '^[0-9]' "$normalized_trace")"
echo "TLC replay witness matches ${action_count} checked-in production actions"
