#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}"
export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"

read -r -a MODES <<<"${SM_PERF_MODES:-scalar auto neon-force}"

if [[ ${#MODES[@]} -eq 0 ]]; then
    echo "[sm-perf] no modes supplied via SM_PERF_MODES" >&2
    exit 1
fi

EXTRA_ARGS=()
if (($#)); then
    EXTRA_ARGS=("$@")
fi

for mode in "${MODES[@]}"; do
    echo "[sm-perf] running mode '${mode}'"
    cmd=("${REPO_ROOT}/scripts/sm_perf.sh" --mode "${mode}")
    if ((${#EXTRA_ARGS[@]})); then
        cmd+=("${EXTRA_ARGS[@]}")
    fi
    if ! "${cmd[@]}"; then
        echo "[sm-perf] mode '${mode}' failed" >&2
        exit 1
    fi
done

echo "[sm-perf] all modes passed"
