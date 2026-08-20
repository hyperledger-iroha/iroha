#!/usr/bin/env bash
set -euo pipefail

# Purpose:
#   Run cargo commands with opportunistic local build accelerators.
#
# Prerequisites:
#   - Cargo must be available on PATH.
#   - `sccache` is optional; enabled automatically when found unless disabled.
#   - A fast linker (`mold`/`lld`/`zld`) is optional and must be requested.
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
  - Reuses Cargo targets through named, repository-local target slots

Options:
  --target-dir DIR        Set CARGO_TARGET_DIR=DIR
  --target-slot NAME      Reuse <repo>/target/cargo-fast/NAME
  --jobs N                Set CARGO_BUILD_JOBS=N (default: Cargo jobserver)
  --no-sccache            Do not auto-enable sccache
  --sccache-dir DIR       Set SCCACHE_DIR; otherwise use sccache's default
  --incremental           Set CARGO_INCREMENTAL=1 for warm local edit loops
  --no-incremental        Set CARGO_INCREMENTAL=0 for sccache-heavy builds
  --stable-local-metadata Set VERGEN_GIT_SHA=local-fast-build
  --zero-debug            Set CARGO_PROFILE_{DEV,TEST}_DEBUG=0
  --linker MODE           Linker: off (default)|auto|mold|lld|ld.lld|zld|ld64.lld|<path>
  --print-env             Print selected env/config and exit
  -h, --help              Show this help

Environment:
  CARGO_FAST_TARGET_ROOT  Override the root used by --target-slot

Examples:
  scripts/cargo_fast.sh -- check -p irohad
  scripts/cargo_fast.sh --target-slot core-tests --incremental -- test -p iroha_core
  scripts/cargo_fast.sh --jobs 6 -- build -p irohad
  scripts/cargo_fast.sh --linker auto -- build -p irohad
USAGE
}

target_dir=""
target_slot=""
target_slot_set=false
jobs=""
jobs_set=false
auto_sccache=true
sccache_dir=""
incremental=false
no_incremental=false
stable_local_metadata=false
linker_mode="off"
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
		--target-slot)
			shift
			if [[ $# -eq 0 ]]; then
				echo "error: missing argument for --target-slot" >&2
				usage >&2
				exit 1
			fi
			target_slot="$1"
			target_slot_set=true
			;;
		--jobs)
			shift
			if [[ $# -eq 0 ]]; then
				echo "error: missing argument for --jobs" >&2
				usage >&2
				exit 1
			fi
			jobs="$1"
			jobs_set=true
			;;
		--no-sccache)
			auto_sccache=false
			;;
		--sccache-dir)
			shift
			if [[ $# -eq 0 ]]; then
				echo "error: missing argument for --sccache-dir" >&2
				usage >&2
				exit 1
			fi
			sccache_dir="$1"
			;;
		--incremental)
			incremental=true
			;;
		--no-incremental)
			no_incremental=true
			;;
		--stable-local-metadata)
			stable_local_metadata=true
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

cargo_serial_jobs=false
expect_cargo_job_value=false
for cargo_arg in "${cargo_args[@]}"; do
	if [[ "${cargo_arg}" == "--" ]]; then
		break
	fi
	if [[ "${expect_cargo_job_value}" == true ]]; then
		if [[ "${cargo_arg}" == "1" ]]; then
			cargo_serial_jobs=true
		fi
		expect_cargo_job_value=false
		continue
	fi
	case "${cargo_arg}" in
	-j | --jobs)
		expect_cargo_job_value=true
		;;
	-j1 | --jobs=1)
		cargo_serial_jobs=true
		;;
	esac
done

if [[ -n "${target_dir}" ]] && [[ "${target_slot_set}" == true ]]; then
	echo "error: --target-dir and --target-slot cannot be used together" >&2
	exit 1
fi

if [[ "${target_slot_set}" == true ]]; then
	if [[ -z "${target_slot}" ]] || [[ "${target_slot}" == "." ]] \
		|| [[ "${target_slot}" == ".." ]] \
		|| [[ "${target_slot}" == *[!A-Za-z0-9._-]* ]]; then
		echo "error: --target-slot must contain only letters, numbers, '.', '_', or '-'" >&2
		exit 1
	fi
	target_root="${CARGO_FAST_TARGET_ROOT:-${REPO_ROOT}/target/cargo-fast}"
	target_dir="${target_root%/}/${target_slot}"
fi

if [[ -n "${target_dir}" ]]; then
	export CARGO_TARGET_DIR="${target_dir}"
fi

if [[ "${jobs_set}" == true ]]; then
	case "${jobs}" in
	'' | *[!0-9]*)
		echo "error: --jobs must be a positive integer" >&2
		exit 1
		;;
	esac
	case "${jobs}" in
	*[1-9]*) ;;
	*)
		echo "error: --jobs must be a positive integer" >&2
		exit 1
		;;
	esac
	export CARGO_BUILD_JOBS="${jobs}"
fi

if [[ "${zero_debug}" == true ]]; then
	export CARGO_PROFILE_DEV_DEBUG=0
	export CARGO_PROFILE_TEST_DEBUG=0
fi

if [[ "${incremental}" == true ]] && [[ "${no_incremental}" == true ]]; then
	echo "error: --incremental and --no-incremental cannot be used together" >&2
	exit 1
fi

if [[ "${incremental}" == true ]]; then
	export CARGO_INCREMENTAL=1
fi

if [[ "${no_incremental}" == true ]]; then
	export CARGO_INCREMENTAL=0
fi

if [[ "${stable_local_metadata}" == true ]]; then
	export VERGEN_GIT_SHA=local-fast-build
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

sccache_active=false
if [[ -n "${RUSTC_WRAPPER:-}" ]] && [[ "${RUSTC_WRAPPER}" == *sccache* ]]; then
	sccache_active=true
fi

active_sccache_dir="${SCCACHE_DIR:-}"
if [[ "${sccache_active}" == true ]]; then
	if [[ -n "${sccache_dir}" ]]; then
		active_sccache_dir="${sccache_dir}"
	fi

	if [[ -n "${active_sccache_dir}" ]]; then
		export SCCACHE_DIR="${active_sccache_dir}"
		mkdir -p "${active_sccache_dir}" >/dev/null 2>&1 || true
	fi
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
else
	echo "[cargo-fast] CARGO_TARGET_DIR=workspace-default"
fi
if [[ -n "${CARGO_BUILD_JOBS:-}" ]]; then
	echo "[cargo-fast] CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}"
else
	echo "[cargo-fast] CARGO_BUILD_JOBS=cargo-default"
fi
if [[ "${CARGO_BUILD_JOBS:-}" == "1" ]] || [[ "${cargo_serial_jobs}" == true ]]; then
	echo "[cargo-fast] warning: one Cargo job serializes compilation; reserve it for constrained or evidence builds" >&2
fi
echo "[cargo-fast] sccache=${enabled_sccache}"
if [[ -n "${SCCACHE_DIR:-}" ]]; then
	echo "[cargo-fast] SCCACHE_DIR=${SCCACHE_DIR}"
fi
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
if [[ "${incremental}" == true ]] || [[ "${no_incremental}" == true ]]; then
	echo "[cargo-fast] CARGO_INCREMENTAL=${CARGO_INCREMENTAL}"
fi
if [[ "${stable_local_metadata}" == true ]]; then
	echo "[cargo-fast] VERGEN_GIT_SHA=${VERGEN_GIT_SHA}"
fi

if [[ "${print_env_only}" == true ]]; then
	exit 0
fi

echo "[cargo-fast] running: cargo ${cargo_args[*]}"
(
	cd -- "${REPO_ROOT}"
	exec cargo "${cargo_args[@]}"
)
