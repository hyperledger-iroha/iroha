#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  cat <<'USAGE' >&2
Usage: scripts/js_signed_staging.sh <version>

Builds the JS SDK, runs `npm pack`, performs a dry-run `npm publish --provenance`
against the staging tag, and writes the artefacts/logs to
artifacts/js/npm_staging/<version>/.
USAGE
  exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JS_DIR="${ROOT_DIR}/javascript/iroha_js"
DEST="${ROOT_DIR}/artifacts/js/npm_staging/${VERSION}"
REGISTRY="${NPM_STAGING_REGISTRY:-https://registry.npmjs.org/}"

mkdir -p "${DEST}"

pushd "${JS_DIR}" >/dev/null

echo "[js_signed_staging] installing dependencies"
npm ci

echo "[js_signed_staging] building native bindings"
npm run build:native

echo "[js_signed_staging] creating npm pack tarball"
PACK_JSON="$(npm pack --json)"
PACK_FILE="$(node -e 'const payload = JSON.parse(process.argv[1] ?? "[]"); if (!payload.length || !payload[0].filename) { throw new Error("npm pack output missing filename"); } process.stdout.write(payload[0].filename);' "${PACK_JSON}")"
mv "${PACK_FILE}" "${DEST}/"
printf '%s\n' "${PACK_JSON}" >"${DEST}/npm-pack.json"

echo "[js_signed_staging] computing tarball checksum"
(cd "${DEST}" && shasum -a 256 "${PACK_FILE}" >"${PACK_FILE}.sha256")

PUBLISH_LOG="${DEST}/npm-publish.log"
echo "[js_signed_staging] running npm publish --dry-run (registry=${REGISTRY})"
{
  npm publish \
    --access public \
    --provenance \
    --tag staging \
    --registry "${REGISTRY}" \
    --dry-run
} >"${PUBLISH_LOG}" 2>&1

SHA256_SUM="$(cut -d' ' -f1 "${DEST}/${PACK_FILE}.sha256")"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
GIT_COMMIT="$(git -C "${ROOT_DIR}" rev-parse HEAD)"
NODE_VERSION="$(node --version)"
NPM_VERSION="$(npm --version)"

cat >"${DEST}/summary.json" <<JSON
{
  "version": "${VERSION}",
  "package": "${PACK_FILE}",
  "package_sha256": "${SHA256_SUM}",
  "publish_log": "$(basename "${PUBLISH_LOG}")",
  "registry": "${REGISTRY}",
  "git_commit": "${GIT_COMMIT}",
  "timestamp": "${TIMESTAMP}",
  "node_version": "${NODE_VERSION}",
  "npm_version": "${NPM_VERSION}"
}
JSON

popd >/dev/null

echo "[js_signed_staging] staging artefacts written to ${DEST}"
