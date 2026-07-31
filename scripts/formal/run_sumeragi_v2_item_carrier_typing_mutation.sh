#!/usr/bin/env bash
# Check the exact item/carrier typing repair and its Proposal-source mutation.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly TLAPM_FUNCTIONS_SHA256="b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063"
readonly TLAPM_FOLDS_SHA256="aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly SANY_SUCCESS_MARKER="Semantic processing of module SumeragiV2ItemCarrierTypingMutation"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly MODEL="SumeragiV2ItemCarrierTypingMutation.tla"
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

if (($#)); then
  echo "usage: $0" >&2
  exit 2
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
java_version="$($JAVA_BIN -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}
for module_and_hash in \
  "Functions:${TLAPM_FUNCTIONS_SHA256}" \
  "Folds:${TLAPM_FOLDS_SHA256}"; do
  module="${module_and_hash%%:*}"
  expected_sha256="${module_and_hash#*:}"
  [[ -f "${TLAPM_STDLIB}/${module}.tla" ]] || {
    echo "pinned TLAPM ${TLAPM_COMMIT} standard library is required at ${TLAPM_STDLIB}" >&2
    exit 1
  }
  actual_sha256="$(hash_file "${TLAPM_STDLIB}/${module}.tla")"
  [[ "$actual_sha256" == "$expected_sha256" ]] || {
    echo "pinned TLAPM standard-library checksum mismatch for ${module}.tla" >&2
    echo "expected: ${expected_sha256}" >&2
    echo "actual:   ${actual_sha256}" >&2
    exit 1
  }
done

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-item-carrier-typing.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
tlapm_compat_dir="${run_dir}/tlapm-stdlib"
mkdir -p "$tlapm_compat_dir"
ln -s "${TLAPM_STDLIB}/Functions.tla" "${tlapm_compat_dir}/Functions.tla"
ln -s "${TLAPM_STDLIB}/Folds.tla" "${tlapm_compat_dir}/Folds.tla"

set +e
(
  cd "$FORMAL_DIR"
  "$JAVA_BIN" "-DTLA-Library=${tlapm_compat_dir}" \
    -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$MODEL"
) >"${run_dir}/sany.log" 2>&1
sany_status=$?
set -e
if [[ "$sany_status" -ne 0 ]]; then
  echo "SANY returned status ${sany_status}" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
fi
sany_last_nonblank="$(awk 'NF { line = $0 } END { print line }' "${run_dir}/sany.log")"
[[ "$sany_last_nonblank" == "$SANY_SUCCESS_MARKER" ]] || {
  echo "SANY did not end at the expected semantic-processing marker" >&2
  cat "${run_dir}/sany.log" >&2
  exit 1
}
echo "[sany] item/carrier typing mutation parsed with frozen Java 21.0.12"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC "-DTLA-Library=${tlapm_compat_dir}"
  -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -workers 1 -depth 1 -simulate num=1
)

run_case() {
  local label="$1"
  local config="$2"
  local expected_status="$3"
  shift 3
  local log="${run_dir}/${label}.log"
  local actual_status
  local diagnostic_count
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -seed 424252 -metadir "${run_dir}/${label}" \
      -config "$config" "$MODEL"
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  if [[ "$expected_status" -eq 0 ]]; then
    diagnostic_count="$(
      grep -Ec \
        '^[[:space:]]*(Error:|Deadlock reached([.]|$)|Temporal properties were violated[.]$)' \
        "$log" || true
    )"
    [[ "$diagnostic_count" -eq 0 ]] || {
      echo "${label} emitted ${diagnostic_count} failure diagnostics" >&2
      cat "$log" >&2
      exit 1
    }
    sumeragi_v2_tlc_assert_terminal "$label" "$log"
  else
    # This mutant fails while TLC evaluates the configured invariant, before
    # initial-state generation and the normal terminal footer are available.
    diagnostic_count="$(
      grep -Ec \
        '^[[:space:]]*(Error:|Deadlock reached([.]|$)|Temporal properties were violated[.]$)' \
        "$log" || true
    )"
    [[ "$diagnostic_count" -eq 1 ]] || {
      echo "${label} emitted ${diagnostic_count} configuration-time diagnostics" >&2
      cat "$log" >&2
      exit 1
    }
  fi
  for marker in "$@"; do
    if [[ "$expected_status" -eq 0 || "$marker" == "Error: "* ]]; then
      sumeragi_v2_tlc_assert_exact_line "$label" "$log" "$marker"
    elif ! grep -Fq "$marker" "$log"; then
      echo "${label} missed expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  echo "[tlc] ${label}: expected status ${expected_status}"
}

run_case fixed item_carrier_typing_fixed.cfg 0 \
  "Computed 1 initial states..." \
  "Progress: 406 states checked."
run_case loose-proposal-source \
  item_carrier_typing_loose_proposal_source.cfg 151 \
  "Error: The invariant of SelectedTypingImpliesCanonicalProposalCarrier is equal to FALSE"

echo "[tlc] item/carrier typing mutation matrix passed"
