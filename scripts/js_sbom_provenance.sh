#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/js_sbom_provenance.sh <version>

Generates a signed CycloneDX SBOM for the JavaScript SDK npm tarball.
The script runs `npm pack` inside javascript/iroha_js, copies the tarball
and npm-pack metadata into artifacts/js/sbom/<version>/, invokes `syft`
to emit a CycloneDX JSON report, signs the SBOM with cosign (keyless),
and records SHA-256 checksums for both artefacts.

Prerequisites:
  - `syft` available on PATH (override via SYFT env var)
  - `cosign` available on PATH (override via COSIGN env var)
  - Cargo toolchain for `npm run build:native`
  - Tests completed ahead of time (script does not rerun npm test)
USAGE
}

if [[ $# -lt 1 ]]; then
  usage >&2
  exit 1
fi

VERSION=$1
REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || true)
if [[ -z "${REPO_ROOT}" ]]; then
  echo "error: must run inside the repository" >&2
  exit 1
fi

JS_DIR="${REPO_ROOT}/javascript/iroha_js"
DEST="${REPO_ROOT}/artifacts/js/sbom/${VERSION}"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_DIR}"' EXIT

SYFT_BIN=${SYFT:-syft}
COSIGN_BIN=${COSIGN:-cosign}

# Enable keyless signing by default when running inside CI.
export COSIGN_EXPERIMENTAL="${COSIGN_EXPERIMENTAL:-1}"

if ! command -v "${SYFT_BIN}" >/dev/null 2>&1; then
  echo "error: syft is required (set SYFT env var to override)" >&2
  exit 1
fi

if ! command -v "${COSIGN_BIN}" >/dev/null 2>&1; then
  echo "error: cosign is required (set COSIGN env var to override)" >&2
  exit 1
fi

if ! command -v shasum >/dev/null 2>&1; then
  echo "error: shasum is required to compute checksums" >&2
  exit 1
fi

if [[ ! -d "${JS_DIR}" ]]; then
  echo "error: JavaScript SDK directory not found at ${JS_DIR}" >&2
  exit 1
fi

echo "==> Building JavaScript SDK native bindings"
(
  cd "${JS_DIR}"
  npm run build:native
)

echo "==> Packing npm tarball"
(
  cd "${JS_DIR}"
  npm pack --json --pack-destination "${TMP_DIR}" > "${TMP_DIR}/npm-pack.json"
)

TARBALL=$(node -e "
const fs = require('node:fs');
const path = '${TMP_DIR}/npm-pack.json';
const entries = JSON.parse(fs.readFileSync(path, 'utf8'));
if (!Array.isArray(entries) || entries.length === 0) {
  throw new Error('npm pack did not produce metadata');
}
process.stdout.write(entries[0].filename);
")

if [[ -z "${TARBALL}" ]]; then
  echo "error: failed to determine npm pack tarball name" >&2
  exit 1
fi

PKG_VERSION=$(node -e "process.stdout.write(require('${JS_DIR}/package.json').version)")
if [[ "${PKG_VERSION}" != "${VERSION}" ]]; then
  echo "error: package.json version ${PKG_VERSION} does not match requested ${VERSION}" >&2
  exit 1
fi

mkdir -p "${DEST}"
cp "${TMP_DIR}/${TARBALL}" "${DEST}/${TARBALL}"
cp "${TMP_DIR}/npm-pack.json" "${DEST}/npm-pack.json"

echo "==> Generating CycloneDX SBOM via syft"
SBOM_PATH="${DEST}/${TARBALL%.tgz}.cyclonedx.json"
"${SYFT_BIN}" "file:${DEST}/${TARBALL}" -o cyclonedx-json > "${SBOM_PATH}"

echo "==> Signing SBOM with cosign"
"${COSIGN_BIN}" sign-blob --yes --bundle "${SBOM_PATH}.sigstore" "${SBOM_PATH}"

CHECKSUMS="${DEST}/checksums.txt"
: > "${CHECKSUMS}"
(
  cd "${DEST}"
  shasum -a 256 "${TARBALL}" >> "${CHECKSUMS}"
  shasum -a 256 "$(basename "${SBOM_PATH}")" >> "${CHECKSUMS}"
)

echo "Artifacts written to ${DEST}:"
echo "  - ${TARBALL}"
echo "  - $(basename "${SBOM_PATH}")"
echo "  - $(basename "${SBOM_PATH}").sigstore"
echo "  - npm-pack.json"
echo "  - checksums.txt"
