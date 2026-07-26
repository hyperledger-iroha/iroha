#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
SWIFT_DIR="${REPO_ROOT}/IrohaSwift"

skip_tests=false
skip_swift=false
target_dir=""

usage() {
	cat <<'USAGE'
Usage: scripts/dev_workflow.sh [--skip-tests] [--skip-swift] [--target-dir DIR]

Runs the default contributor workflow:
  1) cargo fmt --all
  2) cargo clippy --workspace --all-targets --locked -- -D warnings
  3) cargo build --workspace --locked
  4) cargo test --workspace --locked (can take several hours)
  5) swift test (from IrohaSwift/)

Use --skip-tests to omit cargo test for quicker iterations and --skip-swift to
skip the Swift SDK suite. Swift tests are skipped automatically if Swift is not
available on PATH. Use --target-dir to set CARGO_TARGET_DIR and avoid build
directory lock timeouts.
USAGE
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--skip-tests)
		skip_tests=true
		;;
	--skip-swift)
		skip_swift=true
		;;
	--target-dir)
		shift
		if [[ $# -eq 0 ]]; then
			echo "error: missing argument for --target-dir" >&2
			usage >&2
			exit 1
		fi
		target_dir="$1"
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "error: unknown option '$1'" >&2
		usage
		exit 1
		;;
	esac
	shift
done

echo "Running contributor workflow guardrails (fmt/clippy/build/test + swift)."
echo "Note: cargo test --workspace may take several hours; use --skip-tests for a quicker pass."

if [[ -n "${target_dir}" ]]; then
	export CARGO_TARGET_DIR="${target_dir}"
	echo "Using CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
fi

echo "[1/5] cargo fmt --all"
cargo fmt --all

echo "[2/5] cargo clippy --workspace --all-targets --locked -- -D warnings"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "[3/5] cargo build --workspace --locked"
cargo build --workspace --locked

if [[ "${skip_tests}" == false ]]; then
	echo "[4/5] cargo test --workspace --locked"
	cargo test --workspace --locked
else
	echo "[4/5] cargo test --workspace --locked (skipped)"
fi

if [[ "${skip_swift}" == true ]]; then
	echo "[5/5] swift test (skipped)"
elif command -v swift >/dev/null 2>&1; then
	echo "[5/5] swift test (IrohaSwift)"
	(
		cd -- "${SWIFT_DIR}"
		swift test
	)
else
	echo "[5/5] swift test (swift not found; skipped)"
	echo "Install Swift and rerun from ${SWIFT_DIR} to exercise the Swift SDK suite."
fi
