#!/usr/bin/env bash
set -euo pipefail

# Purpose:
#   Opt-in local RUSTC_WRAPPER selected by `scripts/cargo_fast.sh
#   --parallel-frontend N`. It runs rustc's parallel front end
#   (`-Zthreads=N`) for this repository's own crates, whose single-threaded
#   type checking, borrow checking and monomorphization dominate local builds.
#
# Scope:
#   - Only compilations whose crate root lives under <repo>/crates/ get the
#     flag. Registry and vendored dependencies, build-script probes and
#     every other rustc invocation run unchanged.
#   - `-Zthreads` is an unstable rustc option. `RUSTC_BOOTSTRAP=1` is set for
#     the selected rustc process only; it never reaches the environment of
#     build scripts or other crates. The flag changes scheduling inside rustc,
#     not its inputs, so Cargo reuses the same artifacts with or without it.
#   - Local development only: cargo_fast.sh refuses this mode for release,
#     deploy and packaging profiles.
#
# Environment (set by cargo_fast.sh):
#   CARGO_FAST_PARALLEL_FRONTEND_THREADS  thread count (required, >= 2)
#   CARGO_FAST_INNER_RUSTC_WRAPPER        optional wrapper to chain (e.g. sccache)

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 1 ]]; then
	echo "rustc_parallel_frontend.sh: missing rustc path" >&2
	exit 2
fi
rustc="$1"
shift

threads="${CARGO_FAST_PARALLEL_FRONTEND_THREADS:-}"
inner="${CARGO_FAST_INNER_RUSTC_WRAPPER:-}"

run() {
	if [[ -n "${inner}" ]]; then
		exec "${inner}" "$@"
	fi
	exec "$@"
}

# Select only real compilations of a crate root under <repo>/crates/.
# Cargo runs rustc from the workspace root, so roots may be relative.
selected=false
if [[ -n "${threads}" ]]; then
	for arg in "$@"; do
		case "${arg}" in
		*.rs) ;;
		*) continue ;;
		esac
		case "${arg}" in
		/*) path="${arg}" ;;
		*) path="${PWD}/${arg}" ;;
		esac
		case "${path}" in
		"${REPO_ROOT}"/crates/*)
			selected=true
			break
			;;
		esac
	done
fi

if [[ "${selected}" == true ]]; then
	# `run` replaces this process, so the export reaches this rustc only.
	export RUSTC_BOOTSTRAP=1
	run "${rustc}" "$@" "-Zthreads=${threads}"
fi
run "${rustc}" "$@"
