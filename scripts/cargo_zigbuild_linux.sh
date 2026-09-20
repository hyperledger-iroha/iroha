#!/usr/bin/env bash
# Purpose: reproducible AArch64 GNU/Linux Zig driver selection for cargo_fast.sh.
# Prerequisites: Bash, Python 3.10+, Cargo with cargo-zigbuild, Zig, and the Rust
# aarch64-unknown-linux-gnu target. IROHA_ZIG_BINARY optionally pins the absolute
# real Zig executable; otherwise use Zig on PATH. All remaining arguments are
# cargo_fast.sh options followed by -- zigbuild and an explicit Linux target.
# Safe defaults: preserve warm Cargo outputs; no source/cache deletion or deploy.
# --help works before any compiler or backend discovery.
set -euo pipefail
case "${1:-}" in
    --help|-h)
        cat <<'USAGE'
Usage: scripts/cargo_zigbuild_linux.sh [cargo_fast options] -- zigbuild [Cargo options]

Build an explicit aarch64-unknown-linux-gnu target with the corrected Zig driver.
Requires Bash, Python 3.10+, Cargo/cargo-zigbuild, Zig, and the Rust Linux target.
IROHA_ZIG_BINARY optionally pins an absolute real Zig executable; default: Zig on PATH.
Reuses cargo_fast.sh target/job settings and its unchanged native build ownership checks.
The installed cargo-zigbuild driver is invoked directly; Cargo aliases are not expanded.
--print-env checks ownership without requiring Zig or the external driver.
It never cleans caches or deploys.
Example:
  scripts/cargo_zigbuild_linux.sh --jobs 6 -- zigbuild --locked --profile release \
    --target aarch64-unknown-linux-gnu -p irohad --bin iroha3d_taira
USAGE
        exit 0 ;;
esac
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
declare -a wrapper_args build_args
wrapper_args=()
print_env_only=false
while [[ $# -gt 0 && "$1" != -- ]]; do
    [[ "$1" != --print-env ]] || print_env_only=true
    wrapper_args+=("$1")
    shift
done
if [[ $# -lt 2 || "$1" != -- || "$2" != zigbuild ]]; then
    echo "error: pass cargo_fast options, -- zigbuild, and an explicit aarch64-unknown-linux-gnu target" >&2
    exit 1
fi
shift 2
build_args=(build "$@")
found_target=false
expect_target=false
for argument in "$@"; do
    if [[ "${expect_target}" == true ]]; then
        target_value="${argument}"
        expect_target=false
    else
        case "${argument}" in
            --target) expect_target=true; continue ;;
            --target=*) target_value="${argument#--target=}" ;;
            *) continue ;;
        esac
    fi
    case "${target_value}" in
        aarch64-unknown-linux-gnu|aarch64-unknown-linux-gnu.*) found_target=true ;;
        *) echo "error: GNU Zig wrapper requires an explicit aarch64-unknown-linux-gnu target" >&2; exit 1 ;;
    esac
done
if [[ "${found_target}" != true || "${expect_target}" == true ]]; then
    echo "error: GNU Zig wrapper requires an explicit aarch64-unknown-linux-gnu target" >&2
    exit 1
fi
# Print-env runs only Cargo's existing metadata ownership check; it needs neither
# the optional Zig executable nor the external build driver.
if [[ "${print_env_only}" != true ]]; then
    zig_binary="${IROHA_ZIG_BINARY:-$(command -v zig || true)}"
    if [[ -z "$zig_binary" || "$zig_binary" != /* || ! -x "$zig_binary" ]]; then
        echo "error: set IROHA_ZIG_BINARY to an absolute Zig executable" >&2
        exit 1
    fi
    export IROHA_ZIG_BINARY="$zig_binary"
fi
export CARGO_ZIGBUILD_ZIG_PATH="$script_dir/zig_linux_gnu.py"
# cargo-zigbuild otherwise prefers an unrelated Python ziglang installation.
export CARGO_ZIGBUILD_PYTHON_PATH=/usr/bin/false
# cc tracks this input, rebuilding previously cached assembly and recording its
# actual compiler invocation when first entering the corrected build lane.
export CC_ENABLE_DEBUG_OUTPUT=1
# Normalize only the documented known build command. Cargo aliases never handle
# zigbuild: cargo_fast checks the native build argv and dispatches its fixed driver.
exec "$script_dir/cargo_fast.sh" --cargo-zigbuild "${wrapper_args[@]}" -- "${build_args[@]}"
