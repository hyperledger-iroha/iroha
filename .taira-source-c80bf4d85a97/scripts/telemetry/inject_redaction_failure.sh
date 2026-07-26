#!/usr/bin/env bash
# Android telemetry redaction failure injector (shell wrapper).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "${SCRIPT_DIR}/inject_redaction_failure.py" "$@"
