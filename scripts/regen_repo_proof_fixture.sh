#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Regenerate the repo lifecycle proof fixtures (snapshot + digest) under a pinned
toolchain so governance evidence stays reproducible.

Usage: scripts/regen_repo_proof_fixture.sh [options] [<snapshot_out> [<digest_out>]]

Options:
  --toolchain <name>   Run cargo with the given rustup toolchain (e.g. "stable"
                       or "nightly-2026-01-01"). Defaults to the active toolchain.
  --bundle-dir <path>  Copy the refreshed snapshot/digest into this directory
                       (repo_proof_snapshot.json, repo_proof_digest.txt).
  --no-verify          Skip the post-refresh test run (useful when only
                       exporting artefacts for review).
  -h, --help           Show this help and exit.

If <snapshot_out>/<digest_out> are omitted the script updates the in-repo
fixtures under crates/iroha_core/tests/fixtures/.
EOF
}

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
SNAPSHOT_PATH="${ROOT}/crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json"
DIGEST_PATH="${ROOT}/crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest"
TOOLCHAIN=""
BUNDLE_DIR=""
VERIFY=yes

while (($#)); do
    case "$1" in
        --toolchain)
            TOOLCHAIN=${2:?"--toolchain requires a value"}
            shift 2
            ;;
        --bundle-dir)
            BUNDLE_DIR=${2:?"--bundle-dir requires a path"}
            shift 2
            ;;
        --no-verify)
            VERIFY=no
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
        *)
            break
            ;;
    esac
done

if (($# >= 1)); then
    SNAPSHOT_PATH=$1
    shift
fi
if (($# >= 1)); then
    DIGEST_PATH=$1
    shift
fi

if [[ -n "${TOOLCHAIN}" ]]; then
    CARGO_BIN=(cargo "+${TOOLCHAIN}")
else
    CARGO_BIN=(cargo)
fi

cd -- "${ROOT}"

tmp_json="$(mktemp)"
tmp_digest="$(mktemp)"
cleanup() {
    rm -f -- "${tmp_json}" "${tmp_digest}"
}
trap cleanup EXIT

export REPO_PROOF_SNAPSHOT_OUT="${tmp_json}"
export REPO_PROOF_DIGEST_OUT="${tmp_digest}"

echo "Generating repo lifecycle proof fixtures with ${CARGO_BIN[*]}..."
if ! "${CARGO_BIN[@]}" test -p iroha_core --lib -- \
    --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture \
    --nocapture; then
    echo "note: initial run failed (expected if fixtures were stale); continuing with regenerated outputs." >&2
fi

if [[ ! -s "${tmp_json}" || ! -s "${tmp_digest}" ]]; then
    echo "error: failed to generate repo proof outputs (snapshot: ${tmp_json}, digest: ${tmp_digest})." >&2
    exit 1
fi

mkdir -p -- "$(dirname "${SNAPSHOT_PATH}")"
cp -- "${tmp_json}" "${SNAPSHOT_PATH}"
mkdir -p -- "$(dirname "${DIGEST_PATH}")"
cp -- "${tmp_digest}" "${DIGEST_PATH}"

digest_hex="$(tr -d '\n\r' < "${tmp_digest}")"

if [[ -n "${BUNDLE_DIR}" ]]; then
    mkdir -p -- "${BUNDLE_DIR}"
    cp -- "${tmp_json}" "${BUNDLE_DIR}/repo_proof_snapshot.json"
    cp -- "${tmp_digest}" "${BUNDLE_DIR}/repo_proof_digest.txt"
    echo "Copied fixtures into ${BUNDLE_DIR}"
fi

unset REPO_PROOF_SNAPSHOT_OUT
unset REPO_PROOF_DIGEST_OUT

if [[ "${VERIFY}" == yes ]]; then
    echo "Verifying refreshed fixtures..."
    "${CARGO_BIN[@]}" test -p iroha_core --lib -- \
        --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture \
        --nocapture
fi

echo "Repo lifecycle proof fixtures refreshed."
echo "  Snapshot: ${SNAPSHOT_PATH}"
echo "  Digest  : ${DIGEST_PATH}"
echo "  Hex     : ${digest_hex}"
