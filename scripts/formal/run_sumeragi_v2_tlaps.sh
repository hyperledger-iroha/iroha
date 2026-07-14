#!/usr/bin/env bash
set -euo pipefail

readonly TLAPM_COMMIT="763bf3c1826d77a4cf206f43d5aa16775da1da33"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_proof_ledger.py"
readonly EVIDENCE_DIR="${REPO_ROOT}/target/formal/sumeragi_v2"
readonly EVIDENCE_PATH="${EVIDENCE_DIR}/proof_evidence.json"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) default_platform="x86_64-linux-gnu" ;;
  Darwin-arm64) default_platform="arm64-darwin" ;;
  *) default_platform="unsupported" ;;
esac

readonly DEFAULT_TLAPM="${REPO_ROOT}/target/tlapm/toolchains/${TLAPM_COMMIT}/${default_platform}/tlapm/bin/tlapm"
readonly TLAPM_BIN="${TLAPM_BIN:-$DEFAULT_TLAPM}"
readonly LOG_DIR="${EVIDENCE_DIR}/tlaps"
readonly TLAPM_THREADS="${SUMERAGI_TLAPS_THREADS:-4}"

[[ -x "$TLAPM_BIN" ]] || {
  echo "pinned TLAPM ${TLAPM_COMMIT} is required at ${TLAPM_BIN}" >&2
  echo "run scripts/formal/install_sumeragi_v2_tlapm.sh first" >&2
  exit 1
}
version="$($TLAPM_BIN --version 2>&1)"
if [[ "$version" != "${TLAPM_COMMIT:0:7}" ]]; then
  echo "expected TLAPM identity ${TLAPM_COMMIT:0:7}, found: ${version}" >&2
  exit 1
fi

python3 "$CHECKER"
source_manifest_sha256="$(python3 "$CHECKER" --print-source-manifest-sha256)"
readonly source_manifest_sha256
rm -rf -- "$LOG_DIR"
mkdir -p "$LOG_DIR"
rm -f -- "$EVIDENCE_PATH"
proof_modules=()
while IFS= read -r module; do
  proof_modules+=("$module")
done < <(python3 "$CHECKER" --print-proof-modules)

echo "[tlaps] deductive proof run with pinned ${TLAPM_COMMIT}"
for module in "${proof_modules[@]}"; do
  echo "[tlaps] checking ${module}"
  args=(--strict --nofp --threads "$TLAPM_THREADS")
  (
    cd "$FORMAL_DIR"
    "$TLAPM_BIN" "${args[@]}" "${module}.tla"
  ) 2>&1 | tee "${LOG_DIR}/${module}.log"
  current_source_manifest_sha256="$(
    python3 "$CHECKER" --print-source-manifest-sha256
  )"
  if [[ "$current_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
    echo "TLA+ sources changed during the TLAPM proof run" >&2
    exit 1
  fi
  printf '%s\n' \
    "SUMERAGI_TLAPS_BACKEND_COMPLETE module=${module} commit=${TLAPM_COMMIT} source_manifest_sha256=${source_manifest_sha256}" \
    | tee -a "${LOG_DIR}/${module}.log"
done
final_source_manifest_sha256="$(python3 "$CHECKER" --print-source-manifest-sha256)"
if [[ "$final_source_manifest_sha256" != "$source_manifest_sha256" ]]; then
  echo "TLA+ sources changed during the TLAPM proof run" >&2
  exit 1
fi
python3 "$CHECKER" \
  --write-evidence "$EVIDENCE_PATH" \
  --tlapm-version "$version" \
  --tlaps-log-dir "$LOG_DIR"
echo "[tlaps] all configured deductive modules discharged; evidence=${EVIDENCE_PATH}"
