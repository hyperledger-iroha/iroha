#!/usr/bin/env bash
# Check the current checkout for forbidden alternative VM/build surfaces.
# Requires Python 3 and Git; read-only, with no bypass or base-ref exemption.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "${ROOT}/scripts/check_ivm_only.py" --root "${ROOT}" "$@"
