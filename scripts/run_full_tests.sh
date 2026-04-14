#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/run_full_tests.sh [options]

Runs the canonical test workflow for the Iroha workspace. By default this
builds the workspace, executes all fast tests (everything except the
integration suite), and then runs the integration tests that require a local
multi-peer network. The integration phase reuses the binaries produced by the
workspace build when available. Use the flags below to tailor the run to the
host environment.

Options:
  --only-network    Run only the integration tests (skip workspace fast tests).
  --fast            Run cargo via scripts/cargo_fast.sh when available.
  --fast-zero-debug When used with --fast, set CARGO_PROFILE_{DEV,TEST}_DEBUG=0.
  --fast-no-incremental
                    When used with --fast, set CARGO_INCREMENTAL=0.
  --nocapture       Forward --nocapture to the integration tests for verbose logs.
  --target-dir DIR  Set CARGO_TARGET_DIR to avoid build directory lock timeouts.
  -h, --help        Show this message.

Examples:
  scripts/run_full_tests.sh
  scripts/run_full_tests.sh --only-network --nocapture
  scripts/run_full_tests.sh --fast --fast-zero-debug --only-network
EOF
}

run_fast=1
integration_args=()
target_dir=""
use_cargo_fast=false
fast_zero_debug=false
fast_no_incremental=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --only-network)
            run_fast=0
            ;;
        --fast)
            use_cargo_fast=true
            ;;
        --fast-zero-debug)
            fast_zero_debug=true
            ;;
        --fast-no-incremental)
            fast_no_incremental=true
            ;;
        --nocapture)
            integration_args+=("--nocapture")
            ;;
        --target-dir)
            if [[ $# -lt 2 ]]; then
                echo "Missing argument for --target-dir" >&2
                usage >&2
                exit 1
            fi
            target_dir="$2"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            integration_args+=("$@")
            break
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
    shift
done

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)
cd "$repo_root"
cargo_runner=(cargo)
if [[ "${use_cargo_fast}" == true ]]; then
    cargo_fast_script="${repo_root}/scripts/cargo_fast.sh"
    if [[ ! -x "${cargo_fast_script}" ]]; then
        echo "scripts/cargo_fast.sh is not available or not executable" >&2
        exit 1
    fi
    cargo_runner=("${cargo_fast_script}")
    if [[ "${fast_zero_debug}" == true ]]; then
        cargo_runner+=("--zero-debug")
    fi
    if [[ "${fast_no_incremental}" == true ]]; then
        cargo_runner+=("--no-incremental")
    fi
    echo "==> using scripts/cargo_fast.sh for cargo commands"
elif [[ "${fast_zero_debug}" == true || "${fast_no_incremental}" == true ]]; then
    echo "--fast-zero-debug and --fast-no-incremental require --fast" >&2
    exit 2
fi

resolve_dir() {
    local path="$1"
    local candidate
    if [[ "${path}" = /* ]]; then
        candidate="${path}"
    else
        candidate="${repo_root}/${path}"
    fi
    mkdir -p "${candidate}"
    (
        cd "${candidate}"
        pwd
    )
}

bin_name() {
    local raw="$1"
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*)
            printf '%s.exe\n' "${raw}"
            ;;
        *)
            printf '%s\n' "${raw}"
            ;;
    esac
}

resolve_existing_binary() {
    local root="$1"
    local bin
    bin="$(bin_name "$2")"
    local candidate
    for candidate in \
        "${root}/debug/${bin}" \
        "${root}/release/${bin}"
    do
        if [[ -f "${candidate}" ]]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    done
    return 1
}

export_if_unset() {
    local name="$1"
    local value="$2"
    if [[ -z "${!name+x}" ]]; then
        export "${name}=${value}"
    fi
}

if [[ -n "${target_dir}" ]]; then
    export CARGO_TARGET_DIR="$(resolve_dir "${target_dir}")"
fi

target_root="$(resolve_dir "${CARGO_TARGET_DIR:-target}")"
export CARGO_TARGET_DIR="${target_root}"
echo "==> using CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"

echo "==> cargo build --workspace"
"${cargo_runner[@]}" -- build --workspace

export_if_unset IROHA_TEST_SKIP_BUILD 1
export_if_unset IROHA_TEST_TARGET_DIR "${target_root}"

if irohad_bin="$(resolve_existing_binary "${target_root}" "iroha3d")"; then
    export_if_unset TEST_NETWORK_BIN_IROHAD "${irohad_bin}"
fi
if iroha_bin="$(resolve_existing_binary "${target_root}" "iroha")"; then
    export_if_unset TEST_NETWORK_BIN_IROHA "${iroha_bin}"
fi
if kagami_bin="$(resolve_existing_binary "${target_root}" "kagami")"; then
    export_if_unset KAGAMI_BIN "${kagami_bin}"
fi
if [[ -z "${IROHA_TEST_NETWORK_PERMIT_DIR+x}" ]]; then
    export IROHA_TEST_NETWORK_PERMIT_DIR="$(mktemp -d)"
fi

echo "==> integration tests configured to reuse built binaries"

if (( run_fast )); then
    echo "==> cargo test --workspace --exclude integration_tests"
    "${cargo_runner[@]}" -- test --workspace --exclude integration_tests
else
    echo "==> skipping fast test suite"
fi

echo "==> cargo test -p integration_tests ${integration_args[*]}"
if ((${#integration_args[@]} > 0)); then
    "${cargo_runner[@]}" -- test -p integration_tests -- "${integration_args[@]}"
else
    "${cargo_runner[@]}" -- test -p integration_tests
fi

echo "==> test run completed"
