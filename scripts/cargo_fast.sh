#!/usr/bin/env bash
set -euo pipefail

# Purpose:
#   Run cargo commands with opportunistic local build accelerators.
#
# Prerequisites:
#   - Cargo must be available on PATH.
#   - `sccache` is optional; enabled automatically when found unless disabled.
#   - A fast linker (`mold`/`lld`/`zld`) is optional and must be requested.
#   - Python 3 checks explicit target paths without modifying their caches.
#
# Safe defaults:
#   - Only auto linker selection may fall back when an accelerator is unavailable.
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
  - Rejects explicit targets owned by an ancestor or neighbouring Cargo source tree

Options:
  --target-dir DIR        Set CARGO_TARGET_DIR=DIR
  --target-slot NAME      Reuse <repo>/target/cargo-fast/NAME
  --jobs N                Set CARGO_BUILD_JOBS=N (default: Cargo jobserver)
  --preserve-build-limits Keep an inherited local single-worker build fingerprint
  --no-sccache            Do not auto-enable sccache
  --sccache-dir DIR       Set SCCACHE_DIR; otherwise use sccache's default
  --incremental           Set CARGO_INCREMENTAL=1 for warm local edit loops
  --no-incremental        Set CARGO_INCREMENTAL=0 for sccache-heavy builds
  --stable-local-metadata Set VERGEN_GIT_SHA=local-fast-build
                          Reject an inherited IROHA_GIT_COMMIT_HASH before Cargo
  --zero-debug            Set CARGO_PROFILE_{DEV,TEST}_DEBUG=0
  --linker MODE           Linker: off (default)|auto|mold|lld|ld.lld|zld|ld64.lld|<path>
                          Explicit modes must pass the native compiler probe
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
preserve_build_limits=false
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
		--preserve-build-limits)
			preserve_build_limits=true
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

# Some local automation environments export a complete single-worker resource
# fingerprint to every child shell. That is useful for unwrapped commands but
# defeats this wrapper's fast-path defaults. Clear only the exact fingerprint:
# partial caller settings, conventional CI environments, and an explicit
# --preserve-build-limits request remain untouched.
has_inherited_single_worker_fingerprint() {
	[[ "${CARGO_BUILD_JOBS:-}" == "1" ]] \
		&& [[ "${CARGO_INCREMENTAL:-}" == "0" ]] \
		&& [[ "${CARGO_PROFILE_DEV_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_TEST_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_BENCH_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_UNITS:-}" == "1" ]] \
		&& [[ "${CMAKE_BUILD_PARALLEL_LEVEL:-}" == "1" ]]
}

cleared_inherited_build_limits=false
if [[ "${preserve_build_limits}" == false ]] \
	&& [[ -z "${CI:-}" ]] \
	&& [[ -z "${GITHUB_ACTIONS:-}" ]] \
	&& has_inherited_single_worker_fingerprint; then
	unset CARGO_BUILD_JOBS
	unset CARGO_INCREMENTAL
	unset CARGO_PROFILE_DEV_CODEGEN_UNITS
	unset CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_UNITS
	unset CARGO_PROFILE_TEST_CODEGEN_UNITS
	unset CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_UNITS
	unset CARGO_PROFILE_RELEASE_CODEGEN_UNITS
	unset CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS
	unset CARGO_PROFILE_BENCH_CODEGEN_UNITS
	unset CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_UNITS
	unset CMAKE_BUILD_PARALLEL_LEVEL
	cleared_inherited_build_limits=true
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

# Cargo's own --target-dir takes precedence over the environment. Check its
# effective explicit path as well as wrapper options and inherited selections.
# Stop at the separator so a test/program argument is never interpreted here.
target_owner_path="${CARGO_TARGET_DIR:-}"
target_owner_manifest="${REPO_ROOT}/Cargo.toml"
expect_cargo_target_value=false
expect_cargo_manifest_value=false
for cargo_arg in "${cargo_args[@]}"; do
	if [[ "${cargo_arg}" == "--" ]]; then
		break
	fi
	if [[ "${expect_cargo_target_value}" == true ]]; then
		target_owner_path="${cargo_arg}"
		expect_cargo_target_value=false
		continue
	fi
	if [[ "${expect_cargo_manifest_value}" == true ]]; then
		target_owner_manifest="${cargo_arg}"
		expect_cargo_manifest_value=false
		continue
	fi
	case "${cargo_arg}" in
	--target-dir)
		expect_cargo_target_value=true
		;;
	--target-dir=*)
		target_owner_path="${cargo_arg#--target-dir=}"
		;;
	--manifest-path)
		expect_cargo_manifest_value=true
		;;
	--manifest-path=*)
		target_owner_manifest="${cargo_arg#--manifest-path=}"
		;;
	-C | -C?*)
		echo "error: cargo-fast does not accept Cargo -C; invoke the selected source tree's wrapper" >&2
		exit 1
		;;
	esac
done
if [[ -n "${target_owner_path}" ]]; then
	if ! command -v python3 >/dev/null 2>&1; then
		echo "error: python3 is required to check explicit Cargo target ownership" >&2
		exit 1
	fi
	python3 "${SCRIPT_DIR}/check_cargo_target_owner.py" \
		--source-root="${REPO_ROOT}" --target-dir="${target_owner_path}" \
		--manifest-path="${target_owner_manifest}"
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
	if [[ "${IROHA_GIT_COMMIT_HASH+x}" == x ]]; then
		echo "error: --stable-local-metadata conflicts with IROHA_GIT_COMMIT_HASH; omit the local flag for an exact release build, or unset the sealed marker for development" >&2
		exit 1
	fi
	export VERGEN_GIT_SHA=local-fast-build
fi

if ! command -v cargo >/dev/null 2>&1; then
	echo "error: cargo not found on PATH" >&2
	exit 1
fi

selected_linker=""
selected_fuse_arg=""
linker_compiler=""
linker_probe_error=""

# A requested accelerator is scoped to the native driver selected below. Pin that
# exact driver in rustc's flags after probing, rather than probe cc but let Cargo
# link with a different configured executable. Callers with explicit cross/driver
# configuration retain full control through --linker off and their own flags.
linker_environment_supported() {
	local argument variable
	if [[ -n "${CARGO_ENCODED_RUSTFLAGS+x}" ]]; then
		linker_probe_error="CARGO_ENCODED_RUSTFLAGS would supersede this wrapper's RUSTFLAGS"
		return 1
	fi
	case "${RUSTFLAGS:-}" in
	*-Clinker* | *'linker='* | *-fuse-ld* | *link-args*)
		linker_probe_error="RUSTFLAGS already selects a linker or a complete linker argument list"
		return 1
		;;
	esac
	if [[ -n "${CARGO_BUILD_TARGET:-}" ]]; then
		linker_probe_error="CARGO_BUILD_TARGET requires an explicitly managed target linker"
		return 1
	fi
	for variable in ${!CARGO_TARGET_@}; do
		if [[ "${variable}" == *_LINKER ]] && [[ -n "${!variable}" ]]; then
			linker_probe_error="a CARGO_TARGET_*_LINKER override is already set"
			return 1
		fi
	done
	for argument in "${cargo_args[@]}"; do
		[[ "${argument}" == "--" ]] && break
		case "${argument}" in
		--target | --target=* | --config | --config=*)
			linker_probe_error="Cargo --target/--config requires an explicitly managed target linker"
			return 1
			;;
		esac
	done
	return 0
}

supports_fuse_ld() {
	local candidate="$1"
	local tmpdir program named resolved
	tmpdir="$(mktemp -d)" || return 1
	printf 'int main(void) { return 0; }\n' >"${tmpdir}/probe.c"
	# Clang accepts an exact path. GCC accepts only its known linker names.
	if "${linker_compiler}" -fuse-ld="${candidate}" "${tmpdir}/probe.c" -o "${tmpdir}/probe" >"${tmpdir}/log" 2>&1; then
		selected_fuse_arg="${candidate}"
	else
		linker_probe_error="$(cat "${tmpdir}/log")"
		case "${candidate##*/}" in
		ld.lld | lld) named="lld"; program="ld.lld" ;;
		ld.mold | mold) named="mold"; program="ld.mold" ;;
		*) rm -rf "${tmpdir}"; return 1 ;;
		esac
		# GCC/collect2 may search its toolchain before PATH. Do not replace a
		# custom linker path with a same-named but different executable.
		resolved="$("${linker_compiler}" -print-prog-name="${program}" 2>/dev/null)" || resolved=""
		if [[ "${resolved}" != */* ]]; then
			resolved="$(type -P "${resolved}" 2>/dev/null)" || resolved=""
		fi
		if [[ -z "${resolved}" ]] || [[ ! "${candidate}" -ef "${resolved}" ]]; then
			linker_probe_error="${linker_probe_error}
named fallback -fuse-ld=${named} resolves '${resolved:-nothing}', not requested '${candidate}'"
			rm -rf "${tmpdir}"
			return 1
		fi
		if ! "${linker_compiler}" -fuse-ld="${named}" "${tmpdir}/probe.c" -o "${tmpdir}/probe" >"${tmpdir}/log" 2>&1; then
			linker_probe_error="$(cat "${tmpdir}/log")"
			rm -rf "${tmpdir}"
			return 1
		fi
		selected_fuse_arg="${named}"
	fi
	selected_linker="${candidate}"
	rm -rf "${tmpdir}"
	return 0
}

select_linker() {
	local mode="$1"
	local os
	local -a candidates
	local detected_path
	candidates=()
	os="$(uname -s)"
	linker_environment_supported || return 1
	if linker_compiler="$(type -P cc)"; then
		if [[ "${linker_compiler}" != /* ]]; then
			linker_compiler="${PWD}/${linker_compiler}"
		fi
	else
		linker_probe_error="native cc driver not found on PATH"
		return 1
	fi
	case "${linker_compiler}" in
	*[[:space:]]*) linker_probe_error="compiler path contains whitespace unsupported by RUSTFLAGS"; return 1 ;;
	esac

	add_if_present() {
		local name="$1"
		if detected_path="$(type -P "${name}")"; then
			if [[ "${detected_path}" != /* ]]; then
				detected_path="${PWD}/${detected_path}"
			fi
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
			add_if_present "ld.lld"
		else
			add_if_present "lld"
		fi
		;;
	lld)
		if [[ "${os}" == "Darwin" ]]; then add_if_present "ld64.lld"; else add_if_present "ld.lld"; fi
		;;
		mold | zld | ld.lld | ld64.lld)
			add_if_present "${mode}"
			;;
	*)
		add_if_present "${mode}"
		;;
	esac

	# Bash 3.2 treats an empty array expansion as unbound under set -u.
	if [[ ${#candidates[@]} -eq 0 ]]; then return 1; fi
	for candidate in "${candidates[@]}"; do
		case "${candidate}" in
		*[[:space:]]*) linker_probe_error="linker path contains whitespace unsupported by RUSTFLAGS"; continue ;;
		esac
		if supports_fuse_ld "${candidate}"; then
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

if [[ "${linker_mode}" != off ]] && select_linker "${linker_mode}"; then
	linker_flag="-Clinker=${linker_compiler} -Clink-arg=-fuse-ld=${selected_fuse_arg}"
	if [[ -n "${RUSTFLAGS:-}" ]]; then
		export RUSTFLAGS="${RUSTFLAGS} ${linker_flag}"
	else
		export RUSTFLAGS="${linker_flag}"
	fi
elif [[ "${linker_mode}" != off ]]; then
	if [[ "${linker_mode}" != auto ]]; then
		echo "error: requested linker '${linker_mode}' cannot be honored by native driver '${linker_compiler:-unselected}'" >&2
		echo "${linker_probe_error:-requested executable was not found on PATH}" >&2
		echo "use --linker off with explicit target/compiler flags, or --linker auto to permit fallback" >&2
		exit 1
	fi
	echo "[cargo-fast] auto linker unavailable; using system-default (${linker_probe_error:-no candidate executable})" >&2
fi

echo "[cargo-fast] repo=${REPO_ROOT}"
if [[ "${cleared_inherited_build_limits}" == true ]]; then
	echo "[cargo-fast] cleared inherited local single-worker build limits (use --preserve-build-limits to retain them)"
fi
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
	echo "[cargo-fast] linker=${selected_linker} (driver=${linker_compiler}, -fuse-ld=${selected_fuse_arg})"
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
# Replace this shell so it never rereads a changed script after a long build.
cd -- "${REPO_ROOT}"
exec cargo "${cargo_args[@]}"
