#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/run_full_tests.sh [options]

Runs the canonical test workflow for the Iroha workspace. By default this
builds the workspace, executes all fast tests (everything except the
integration suite), and then runs the integration tests that require a local
multi-peer network. Use the flags below to tailor the run to the host
environment.

Options:
  --only-network    Run only the integration tests (skip workspace fast tests).
  --nocapture       Forward --nocapture to the integration tests for verbose logs.
  --target-dir DIR  Set CARGO_TARGET_DIR to avoid build directory lock timeouts.
  -h, --help        Show this message.

Examples:
  scripts/run_full_tests.sh
  scripts/run_full_tests.sh --only-network --nocapture
EOF
}

run_fast=1
integration_args=()
target_dir=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --only-network)
            run_fast=0
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

if [[ -n "${target_dir}" ]]; then
    export CARGO_TARGET_DIR="${target_dir}"
    echo "==> using CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
fi

echo "==> cargo build --workspace"
cargo build --workspace

if (( run_fast )); then
    echo "==> cargo test --workspace --exclude integration_tests"
    cargo test --workspace --exclude integration_tests
else
    echo "==> skipping fast test suite"
fi

echo "==> cargo test -p integration_tests ${integration_args[*]}"
if ((${#integration_args[@]} > 0)); then
    cargo test -p integration_tests -- "${integration_args[@]}"
else
    cargo test -p integration_tests
fi

echo "==> test run completed"
