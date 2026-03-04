#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Wrapper for the canonical SNNet-5a smoke helper in `scripts/`.
# Keeps docs/examples entries runnable while the maintained implementation
# lives alongside other operational scripts.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"

exec "${repo_root}/scripts/sorafs_direct_mode_smoke.sh" "$@"
