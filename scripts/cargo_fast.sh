#!/usr/bin/env bash
set -euo pipefail

# Purpose:
#   Run cargo commands with opportunistic local build accelerators.
#
# Prerequisites:
#   - Cargo must be available on PATH.
#   - `sccache` is optional; enabled automatically when found unless disabled.
#   - A fast linker (`mold`/`lld`/`zld`) is optional; probed before use.
#
# Safe defaults:
#   - Falls back to system defaults when accelerators are unavailable.
#   - Never mutates repository files.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

usage() {
	cat <<'USAGE'
Usage: scripts/cargo_fast.sh [options] -- <cargo args...>
       scripts/cargo_fast.sh [options] <cargo args...>

Runs `cargo` with optional accelerators when available:
  - Enables `sccache` when found (unless --no-sccache is used)
  - Enables a supported fast linker via RUSTFLAGS when found

Options:
  --target-dir DIR   Set CARGO_TARGET_DIR=DIR
  --no-sccache       Do not auto-enable sccache
  --zero-debug       Set CARGO_PROFILE_{DEV,TEST}_DEBUG=0 for faster local builds
  --linker MODE      Linker preference: auto|off|mold|lld|ld.lld|zld|ld64.lld|<path>
  --print-env        Print selected env/config and exit
  -h, --help         Show this help

Examples:
  scripts/cargo_fast.sh -- check -p irohad
  scripts/cargo_fast.sh --target-dir /tmp/iroha_fast -- test -p irohad --no-run
  scripts/cargo_fast.sh --linker off -- build -p irohad
USAGE
}

target_dir=""
auto_sccache=true
linker_mode="auto"
zero_debug=false
print_env_only=false

declare -a cargo_args
cargo_args=()

while [[ $# -gt 0 ]]; do
	case "$1" in
	--target-dir)
		shift
		if [[ $# -eq 0 ]]; then
			echo "error: missing argument for --target-dir" >&2
			usage >&2
			exit 1
		fi
		target_dir="$1"
		;;
		--no-sccache)
			auto_sccache=false
			;;
		--zero-debug)
			zero_debug=true
			;;
		--linker)
			shift
		if [[ $# -eq 0 ]]; then
			echo "error: missing argument for --linker" >&2
			usage >&2
			exit 1
		fi
		linker_mode="$1"
		;;
	--print-env)
		print_env_only=true
		;;
	-h | --help)
		usage
		exit 0
		;;
	--)
		shift
		while [[ $# -gt 0 ]]; do
			cargo_args+=("$1")
			shift
		done
		break
		;;
	-*)
		echo "error: unknown option '$1'" >&2
		usage >&2
		exit 1
		;;
	*)
		cargo_args+=("$1")
		;;
	esac
	shift || true
done

if [[ ${#cargo_args[@]} -eq 0 ]]; then
	echo "error: missing cargo arguments" >&2
	usage >&2
	exit 1
fi

if [[ -n "${target_dir}" ]]; then
	export CARGO_TARGET_DIR="${target_dir}"
fi

if [[ "${zero_debug}" == true ]]; then
	export CARGO_PROFILE_DEV_DEBUG=0
	export CARGO_PROFILE_TEST_DEBUG=0
fi

if ! command -v cargo >/dev/null 2>&1; then
	echo "error: cargo not found on PATH" >&2
	exit 1
fi

supports_fuse_ld() {
	local candidate="$1"
	local compiler
	local tmpdir

	if command -v cc >/dev/null 2>&1; then
		compiler="$(command -v cc)"
	elif command -v clang >/dev/null 2>&1; then
		compiler="$(command -v clang)"
	elif command -v gcc >/dev/null 2>&1; then
		compiler="$(command -v gcc)"
	else
		return 1
	fi

	tmpdir="$(mktemp -d)"
	printf 'int main(void) { return 0; }\n' >"${tmpdir}/probe.c"
	if "${compiler}" -fuse-ld="${candidate}" "${tmpdir}/probe.c" -o "${tmpdir}/probe" >/dev/null 2>&1; then
		rm -rf "${tmpdir}"
		return 0
	fi
	rm -rf "${tmpdir}"
	return 1
}

select_linker() {
	local mode="$1"
	local os
	local -a candidates
	local detected_path
	candidates=()
	os="$(uname -s)"

	add_if_present() {
		local name="$1"
		if command -v "${name}" >/dev/null 2>&1; then
			detected_path="$(command -v "${name}")"
			candidates+=("${detected_path}")
		fi
	}

	case "${mode}" in
	off)
		return 1
		;;
	auto)
		if [[ "${os}" == "Darwin" ]]; then
			add_if_present "zld"
			add_if_present "ld64.lld"
			add_if_present "lld"
		elif [[ "${os}" == "Linux" ]]; then
			add_if_present "mold"
			add_if_present "lld"
			add_if_present "ld.lld"
		else
			add_if_present "lld"
		fi
		;;
		mold | lld | zld | ld.lld | ld64.lld)
			add_if_present "${mode}"
			;;
	*)
		candidates+=("${mode}")
		;;
	esac

	for candidate in "${candidates[@]}"; do
		if supports_fuse_ld "${candidate}"; then
			echo "${candidate}"
			return 0
		fi
	done

	return 1
}

enabled_sccache="no"
if [[ "${auto_sccache}" == true ]]; then
	if [[ -n "${RUSTC_WRAPPER:-}" ]]; then
		enabled_sccache="already-set(${RUSTC_WRAPPER})"
	elif command -v sccache >/dev/null 2>&1; then
		export RUSTC_WRAPPER="$(command -v sccache)"
		enabled_sccache="yes(${RUSTC_WRAPPER})"
	else
		enabled_sccache="not-found"
	fi
else
	enabled_sccache="disabled"
fi

selected_linker=""
if selected_linker="$(select_linker "${linker_mode}" 2>/dev/null)"; then
	linker_flag="-Clink-arg=-fuse-ld=${selected_linker}"
	if [[ -n "${RUSTFLAGS:-}" ]]; then
		export RUSTFLAGS="${RUSTFLAGS} ${linker_flag}"
	else
		export RUSTFLAGS="${linker_flag}"
	fi
fi

echo "[cargo-fast] repo=${REPO_ROOT}"
if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
	echo "[cargo-fast] CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
fi
echo "[cargo-fast] sccache=${enabled_sccache}"
if [[ -n "${selected_linker}" ]]; then
	echo "[cargo-fast] linker=${selected_linker} (via -fuse-ld)"
else
	echo "[cargo-fast] linker=system-default"
fi
if [[ -n "${RUSTFLAGS:-}" ]]; then
	echo "[cargo-fast] RUSTFLAGS=${RUSTFLAGS}"
fi
if [[ "${zero_debug}" == true ]]; then
	echo "[cargo-fast] CARGO_PROFILE_DEV_DEBUG=${CARGO_PROFILE_DEV_DEBUG}"
	echo "[cargo-fast] CARGO_PROFILE_TEST_DEBUG=${CARGO_PROFILE_TEST_DEBUG}"
fi

if [[ "${print_env_only}" == true ]]; then
	exit 0
fi

echo "[cargo-fast] running: cargo ${cargo_args[*]}"
(
	cd -- "${REPO_ROOT}"
	exec cargo "${cargo_args[@]}"
)
