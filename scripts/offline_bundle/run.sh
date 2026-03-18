#!/usr/bin/env bash
set -euo pipefail

cargo xtask offline-bundle "$@"
