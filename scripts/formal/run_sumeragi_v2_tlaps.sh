#!/usr/bin/env bash
set -euo pipefail

readonly TLAPM_COMMIT="763bf3c1826d77a4cf206f43d5aa16775da1da33"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_proof_ledger.py"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) default_platform="x86_64-linux-gnu" ;;
  Darwin-arm64) default_platform="arm64-darwin" ;;
  *) default_platform="unsupported" ;;
esac

readonly DEFAULT_TLAPM="${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${default_platform}/tlapm/bin/tlapm"
readonly TLAPM_BIN="${TLAPM_BIN:-$DEFAULT_TLAPM}"
readonly LOG_DIR="${SUMERAGI_TLAPS_LOG_DIR:-${REPO_ROOT}/target/formal/sumeragi_v2/tlaps}"
readonly TLAPM_THREADS="${SUMERAGI_TLAPS_THREADS:-4}"

[[ -x "$TLAPM_BIN" ]] || {
  echo "pinned TLAPM ${TLAPM_COMMIT} is required at ${TLAPM_BIN}" >&2
  echo "run scripts/formal/install_sumeragi_v2_tlapm.sh first" >&2
  exit 1
}
version="$($TLAPM_BIN --version 2>&1)"
if ! grep -Fq "${TLAPM_COMMIT:0:7}" <<<"$version"; then
  echo "expected TLAPM commit ${TLAPM_COMMIT}, found: ${version}" >&2
  exit 1
fi

python3 "$CHECKER"
mkdir -p "$LOG_DIR"
proof_modules=()
while IFS= read -r module; do
  proof_modules+=("$module")
done < <(python3 "$CHECKER" --print-proof-modules)

echo "[tlaps] deductive proof run with pinned ${TLAPM_COMMIT}"
for module in "${proof_modules[@]}"; do
  echo "[tlaps] checking ${module}"
  args=(--strict --safefp --threads "$TLAPM_THREADS")
  if [[ "${SUMERAGI_TLAPS_NOFP:-0}" == "1" ]]; then
    args+=(--nofp)
  fi
  (
    cd "$FORMAL_DIR"
    # `--summary` is intentionally forbidden here: TLAPM documents that it
    # implies `-N`, which parses obligations without invoking any backend.
    "$TLAPM_BIN" "${args[@]}" "${module}.tla"
  ) 2>&1 | tee "${LOG_DIR}/${module}.log"
done
echo "[tlaps] all configured deductive modules discharged"
