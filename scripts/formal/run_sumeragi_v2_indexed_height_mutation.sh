#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly HEIGHT_MODULE="${FORMAL_DIR}/SumeragiV2ChainLivenessProofs.tla"
readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_indexed_height_contract.py"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"

source_only=false
if (($# == 1)) && [[ "$1" == "--source-only" ]]; then
  source_only=true
elif (($#)); then
  echo "usage: $0 [--source-only]" >&2
  exit 2
fi

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-indexed-height-mutation.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

python3 "$CONTRACT_CHECKER" "$HEIGHT_MODULE"

python3 - "$HEIGHT_MODULE" "$run_dir" <<'PY'
from pathlib import Path
import sys

source_path = Path(sys.argv[1])
output_dir = Path(sys.argv[2])
source = source_path.read_text(encoding="utf-8")

mutations = {
    "weak-node-height.tla": (
        "IndexedExactContextCompleted(initialContext) ==\n"
        "  IF initialContext.height = MaxHeight\n"
        "  THEN IndexedAllResponsiveExactApplicationsAt(initialContext)\n"
        "  ELSE LET nextContext ==\n"
        "             CanonicalIndexedContext(initialContext.height + 1)\n"
        "       IN \\A node \\in Responsive:\n"
        "            node \\in joinedByContext[nextContext]\n",
        "IndexedExactContextCompleted(initialContext) ==\n"
        "  IF initialContext.height = MaxHeight\n"
        "  THEN IndexedAllResponsiveExactApplicationsAt(initialContext)\n"
        "  ELSE \\A node \\in Responsive:\n"
        "         nodeHeight[node] > initialContext.height\n",
    ),
    "missing-exact-recovery.tla": (
        "THEOREM IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress ==\n"
        "  /\\ IndexedChainSpec\n"
        "  /\\ IndexedExactHistoricalRecoveryProgress\n"
        "  /\\ IndexedSuccessorActivationProgress\n",
        "THEOREM IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress ==\n"
        "  /\\ IndexedChainSpec\n"
        "  /\\ IndexedSuccessorActivationProgress\n",
    ),
    "tautological-implication.tla": (
        "IndexedExactHeightLivenessProperty ==\n"
        "  (/\\ VerificationContext \\in AdmissibleContextRecords\n"
        "   /\\ VerificationContext \\in JoinedContexts\n"
        "   /\\ IndexedCore(VerificationContext, 7))\n"
        "    ~> IndexedExactContextCompleted(VerificationContext)\n",
        "IndexedExactHeightLivenessProperty ==\n"
        "  (/\\ VerificationContext \\in AdmissibleContextRecords\n"
        "   /\\ VerificationContext \\in JoinedContexts\n"
        "   /\\ IndexedCore(VerificationContext, 7))\n"
        "    => (/\\ VerificationContext \\in AdmissibleContextRecords\n"
        "        /\\ VerificationContext \\in JoinedContexts\n"
        "        /\\ IndexedCore(VerificationContext, 7))\n",
    ),
    "wrong-predecessor-height.tla": (
        "            CanonicalIndexedContext(initialContext.height - 1), node)\n",
        "            CanonicalIndexedContext(initialContext.height), node)\n",
    ),
}

for filename, (old, new) in mutations.items():
    if source.count(old) != 1:
        raise SystemExit(
            f"mutation fixture drift for {filename}: expected one source fragment, "
            f"found {source.count(old)}"
        )
    (output_dir / filename).write_text(source.replace(old, new, 1), encoding="utf-8")
PY

for mutation in weak-node-height.tla missing-exact-recovery.tla \
  tautological-implication.tla wrong-predecessor-height.tla; do
  set +e
  python3 "$CONTRACT_CHECKER" "$run_dir/$mutation" \
    >"$run_dir/$mutation.stdout" 2>"$run_dir/$mutation.stderr"
  status=$?
  set -e
  if [[ $status -eq 0 ]]; then
    echo "indexed-height source mutation was accepted: $mutation" >&2
    exit 1
  fi
  case "$mutation" in
    weak-node-height.tla)
      marker="IndexedExactContextCompleted must equal only"
      ;;
    missing-exact-recovery.tla)
      marker="IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress must state only"
      ;;
    tautological-implication.tla)
      marker="IndexedExactHeightLivenessProperty must equal only"
      ;;
    wrong-predecessor-height.tla)
      marker="IndexedExactHeightEntrySource must equal only"
      ;;
  esac
  if ! grep -Fq "$marker" "$run_dir/$mutation.stderr"; then
    echo "indexed-height source mutation missed marker '$marker': $mutation" >&2
    cat "$run_dir/$mutation.stderr" >&2
    exit 1
  fi
done

echo "[source] exact next-context, exact-recovery, non-tautology, and height-1 contracts reject their mutations"

if $source_only; then
  exit 0
fi

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
[[ "$actual_sha256" == "$TLA2TOOLS_SHA256" ]] || {
  echo "TLA2Tools checksum mismatch" >&2
  echo "expected: ${TLA2TOOLS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
}
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 93 -seed 164637733644495745
)

run_pass() {
  local name="$1"
  local config="$2"
  local log="$run_dir/$name.log"
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "$run_dir/$name" \
      -config "$config" SumeragiV2IndexedHeightLivenessMutation.tla
  ) >"$log" 2>&1
  grep -Fq "Model checking completed. No error has been found." "$log" || {
    cat "$log" >&2
    exit 1
  }
}

run_liveness_failure() {
  local name="$1"
  local config="$2"
  local log="$run_dir/$name.log"
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "$run_dir/$name" \
      -config "$config" SumeragiV2IndexedHeightLivenessMutation.tla
  ) >"$log" 2>&1
  local status=$?
  set -e
  [[ $status -eq 13 ]] || {
    echo "$name mutation did not fail with TLC liveness status 13" >&2
    cat "$log" >&2
    exit 1
  }
  for marker in "Temporal properties were violated." "Stuttering"; do
    grep -Fq "$marker" "$log" || {
      echo "$name mutation missed TLC marker '$marker'" >&2
      cat "$log" >&2
      exit 1
    }
  done
}

run_pass exact indexed_height_exact.cfg
run_liveness_failure weak-projection indexed_height_weak_projection_bug.cfg
run_pass weak-diagnostic indexed_height_weak_projection_diagnostic.cfg
run_pass exact-recovery indexed_height_recovery_exact.cfg
run_liveness_failure missing-recovery indexed_height_missing_recovery_bug.cfg
run_pass tautology-diagnostic indexed_height_tautology_diagnostic.cfg

echo "[tlc] exact predecessor publication joins every responsive validator"
echo "[tlc] nodeHeight > context passes its weak diagnostic but fails exact next-context membership"
echo "[tlc] omitting exact historical recovery leaves a stuttering lagging-validator counterexample"
echo "[tlc] the tautological implication is semantically vacuous and rejected by the source contract"
