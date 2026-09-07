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
Reuses cargo_fast.sh target/job settings. It never cleans caches or deploys.
Example:
  scripts/cargo_zigbuild_linux.sh --jobs 6 -- zigbuild --locked --profile release \
    --target aarch64-unknown-linux-gnu -p irohad --bin iroha3d_taira
USAGE
        exit 0 ;;
esac
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
zig_binary="${IROHA_ZIG_BINARY:-$(command -v zig || true)}"
if [[ -z "$zig_binary" || "$zig_binary" != /* || ! -x "$zig_binary" ]]; then
    echo "error: set IROHA_ZIG_BINARY to an absolute Zig executable" >&2
    exit 1
fi
found_zigbuild=false
found_target=false
for argument in "$@"; do
    [[ "$argument" != zigbuild ]] || found_zigbuild=true
    case "$argument" in
        aarch64-unknown-linux-gnu|aarch64-unknown-linux-gnu.*|--target=aarch64-unknown-linux-gnu|--target=aarch64-unknown-linux-gnu.*)
            found_target=true ;;
    esac
done
if [[ "$found_zigbuild" != true || "$found_target" != true ]]; then
    echo "error: pass cargo_fast options, -- zigbuild, and an explicit aarch64-unknown-linux-gnu target" >&2
    exit 1
fi
export IROHA_ZIG_BINARY="$zig_binary"
export CARGO_ZIGBUILD_ZIG_PATH="$script_dir/zig_linux_gnu.py"
# cargo-zigbuild otherwise prefers an unrelated Python ziglang installation.
export CARGO_ZIGBUILD_PYTHON_PATH=/usr/bin/false
# cc tracks this input, rebuilding previously cached assembly and recording its
# actual compiler invocation when first entering the corrected build lane.
export CC_ENABLE_DEBUG_OUTPUT=1
exec "$script_dir/cargo_fast.sh" "$@"
