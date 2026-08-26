#!/usr/bin/env bash
#
# Run the default contributor checks for Rust packages affected by the current
# branch and working tree. Requires Git, Cargo, and Python with scripts
# dependencies installed. Use --full for the exhaustive workspace workflow.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
SWIFT_DIR="${REPO_ROOT}/IrohaSwift"
ROUTER="${SCRIPT_DIR}/rust_ci.py"

skip_tests=false
skip_swift=false
full=false
base_ref=""
target_dir=""

usage() {
	cat <<'USAGE'
Usage: scripts/dev_workflow.sh [options]

Runs the affected contributor workflow:
  1) cargo fmt --all -- --check
  2) classify changed packages with locked Cargo metadata
  3) cargo clippy/build/test --locked for the reverse-dependency closure
  4) swift test when Swift sources or shared fixtures changed

Options:
  --base REF       Compare committed changes with REF instead of origin/main.
  --full           Validate every Rust lane and run Swift tests.
  --skip-tests     Omit affected cargo tests.
  --skip-swift     Omit Swift tests.
  --target-dir DIR Set CARGO_TARGET_DIR to avoid build-directory contention.
  -h, --help       Show this help.

Unknown, ambiguous, root-build, fixture, configuration, script, and workflow
changes fail closed to all Rust lanes. The full workspace release workflow
remains the source of exhaustive release evidence.
USAGE
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--base)
		shift
		if [[ $# -eq 0 ]]; then
			echo "error: missing argument for --base" >&2
			usage >&2
			exit 1
		fi
		base_ref="$1"
		;;
	--full)
		full=true
		;;
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
		usage >&2
		exit 1
		;;
	esac
	shift
done

cd -- "${REPO_ROOT}"
if [[ -n "${target_dir}" ]]; then
	export CARGO_TARGET_DIR="${target_dir}"
	echo "Using CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
fi

classification_file="$(mktemp "${TMPDIR:-/tmp}/iroha-rust-lanes.XXXXXX")"
trap 'rm -f -- "${classification_file}"' EXIT

classifier_args=(classify --json-out "${classification_file}")
if [[ "${full}" == true ]]; then
	classifier_args+=(--all)
elif [[ -n "${base_ref}" ]]; then
	classifier_args+=(--base "${base_ref}")
fi

echo "[1/4] cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "[2/4] classify affected Rust packages"
python3 "${ROUTER}" "${classifier_args[@]}" >/dev/null
python3 - "${classification_file}" <<'PY'
import json
import sys

document = json.load(open(sys.argv[1], encoding="utf-8"))
if not document["has_rust"]:
    print("No affected Rust packages.")
else:
    mode = "full (fail-closed)" if document["full"] else "affected"
    lanes = ", ".join(
        f'{lane["name"]}={len(lane["packages"])}' for lane in document["lanes"]
    )
    print(f'{mode}: {len(document["impacted_packages"])} packages ({lanes})')
    for reason in document["reasons"]:
        print(f"  reason: {reason}")
PY

package_csv="$(
	python3 - "${classification_file}" <<'PY'
import json
import sys

document = json.load(open(sys.argv[1], encoding="utf-8"))
print(",".join(document["impacted_packages"]))
PY
)"

if [[ -n "${package_csv}" ]]; then
	checks="clippy,build"
	if [[ "${skip_tests}" == false ]]; then
		checks="${checks},test"
	fi
	echo "[3/4] locked affected Cargo checks (${checks})"
	python3 "${ROUTER}" run --packages "${package_csv}" --checks "${checks}"
else
	echo "[3/4] locked affected Cargo checks (not required)"
fi

swift_required="$(
	python3 - "${classification_file}" "${full}" <<'PY'
import json
import sys

document = json.load(open(sys.argv[1], encoding="utf-8"))
full = sys.argv[2] == "true"
changed = document["changed_paths"]
required = full or any(
    path.startswith((
        "IrohaSwift/",
        "fixtures/",
        "crates/connect_norito_bridge/",
    ))
    or path in {
        "ci/build_offline_cash_swift_fixture.sh",
        "ci/xcode-swift-parity",
        "scripts/dev_workflow.sh",
    }
    for path in changed
)
print("true" if required else "false")
PY
)"

if [[ "${skip_swift}" == true ]]; then
	echo "[4/4] swift test (skipped)"
elif [[ "${swift_required}" != true ]]; then
	echo "[4/4] swift test (not affected)"
elif command -v swift >/dev/null 2>&1; then
	echo "[4/4] swift test (IrohaSwift)"
	IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN="$(
		bash "${REPO_ROOT}/ci/build_offline_cash_swift_fixture.sh" --locked
	)"
	export IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN
	(
		cd -- "${SWIFT_DIR}"
		swift test
	)
else
	echo "error: Swift is required for the affected full SDK suite; install Swift or pass --skip-swift explicitly" >&2
	exit 1
fi
