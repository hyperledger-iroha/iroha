#!/usr/bin/env sh
set -eu

usage() {
    cat <<'USAGE'
Usage: scripts/check_norito_usage.sh

Ensures crates rely on `norito::aos` helpers instead of manual AoS ad-hoc bodies.
Fails if internal framing helpers are referenced outside `crates/norito/src/aos.rs`.
USAGE
}

if [ "${1:-}" = "--help" ]; then
    usage
    exit 0
fi

if ! command -v rg >/dev/null 2>&1; then
    printf '%s\n' "error: ripgrep (rg) is required; install it with brew install ripgrep, apt install ripgrep or cargo install ripgrep" >&2
    exit 2
fi

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
VIOLATIONS=$(rg --glob '!crates/norito/src/aos.rs' --glob '*.rs' 'aos_write_len_and_ver' "$REPO_ROOT" || true)

if [ -n "$VIOLATIONS" ]; then
    printf '%s\n' "error: direct AoS header writes detected; use norito::aos helpers:" >&2
    printf '%s\n' "$VIOLATIONS" >&2
    exit 1
fi

printf '%s\n' "check_norito_usage: ok"
