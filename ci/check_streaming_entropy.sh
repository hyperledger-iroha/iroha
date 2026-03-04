#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUT_DIR="${REPO_ROOT}/artifacts/nsc"
DEFAULT_OUT="${OUT_DIR}/entropy_bench.json"

mkdir -p "${OUT_DIR}"

echo "[streaming-entropy-bench] running baseline benchmark"
(
  cd "${REPO_ROOT}"
  cargo xtask streaming-entropy-bench --frames 3 --segments 2 --width 3 \
    --json-out "${DEFAULT_OUT}"
)

if [[ ! -f "${DEFAULT_OUT}" ]]; then
  echo "error: streaming entropy benchmark did not produce ${DEFAULT_OUT}" >&2
  exit 1
fi
echo "[streaming-entropy-bench] wrote ${DEFAULT_OUT#"${REPO_ROOT}/"}"

if [[ "${ENABLE_RANS_BUNDLES:-0}" != "0" ]]; then
  BUNDLED_OUT="${OUT_DIR}/entropy_bench_bundled.json"
  echo "[streaming-entropy-bench] ENABLE_RANS_BUNDLES=1 set; capturing bundled run"
  (
    cd "${REPO_ROOT}"
    RUSTFLAGS="${RUSTFLAGS:-} --cfg norito_enable_rans_bundles" \
      cargo xtask streaming-entropy-bench --frames 3 --segments 2 --width 3 \
        --json-out "${BUNDLED_OUT}"
  )
  if [[ -f "${BUNDLED_OUT}" ]]; then
    echo "[streaming-entropy-bench] wrote ${BUNDLED_OUT#"${REPO_ROOT}/"}"
  else
    echo "error: bundled entropy benchmark did not produce ${BUNDLED_OUT}" >&2
    exit 1
  fi
fi
