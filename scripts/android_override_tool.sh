#!/usr/bin/env bash
# Android telemetry override helper wrapper.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec python3 "${SCRIPT_DIR}/android_override_tool.py" "$@"
