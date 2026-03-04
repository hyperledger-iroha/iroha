#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
XTASK_TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/target-codex}"
PORTAL_ARTIFACT_DIR="${REPO_ROOT}/artifacts/docs_portal"

tmp_files=()
cleanup() {
  for file in "${tmp_files[@]:-}"; do
    [[ -n "${file:-}" && -f "${file}" ]] && rm -f "${file}"
  done
}
trap cleanup EXIT

HOST_SUMMARY_TMP="$(mktemp -t soradns-host-summary.XXXXXX.json)"
GAR_TEMPLATE_TMP="$(mktemp -t soradns-gar-template.XXXXXX.json)"
tmp_files+=("${HOST_SUMMARY_TMP}" "${GAR_TEMPLATE_TMP}")

echo "[docs-portal] verifying SoraDNS host patterns for docs.sora"
(
  cd "${REPO_ROOT}"
  CARGO_TARGET_DIR="${XTASK_TARGET_DIR}" cargo xtask soradns-hosts \
    --name docs.sora \
    --json-out "${HOST_SUMMARY_TMP}" \
    --verify-host-patterns "${REPO_ROOT}/docs/examples/soradns_host_patterns_docs_sora.json"
)

echo "[docs-portal] verifying GAR template matches checked-in reference"
(
  cd "${REPO_ROOT}"
  CARGO_TARGET_DIR="${XTASK_TARGET_DIR}" cargo xtask soradns-gar-template \
    --name docs.sora \
    --manifest-cid bafybeigdyrzt2vx7demoexamplecid \
    --manifest-digest 8a2a332d5e52edc13ed088b79b6b2940af0b31c7f7fbb9324a88acdf1a0af07d \
    --telemetry-label dg-3 \
    --valid-from 1735771200 \
    --valid-until 1767307200 \
    --json-out "${GAR_TEMPLATE_TMP}"
)

if ! cmp -s "${GAR_TEMPLATE_TMP}" "${REPO_ROOT}/docs/examples/soradns_gar_docs_sora.json"; then
  echo "error: generated GAR template diverges from docs/examples/soradns_gar_docs_sora.json" >&2
  diff -u "${REPO_ROOT}/docs/examples/soradns_gar_docs_sora.json" "${GAR_TEMPLATE_TMP}" || true
  exit 1
fi

echo "[docs-portal] running gateway probe fixtures"
"${REPO_ROOT}/ci/check_sorafs_gateway_probe.sh"

cd "${REPO_ROOT}/docs/portal"

mkdir -p "${PORTAL_ARTIFACT_DIR}"

echo "[docs-portal] verifying SDK ledger recipe parity"
node scripts/check-sdk-recipes.mjs

echo "[docs-portal] verifying OpenAPI snapshot signatures"
node scripts/check-openapi-signatures.mjs --allow-unsigned=2025-q2

if command -v npm >/dev/null 2>&1; then
  if [[ -n "${CI:-}" ]]; then
    npm ci
  else
    npm install
  fi
else
  echo "error: npm is required to build the developer portal" >&2
  exit 1
fi

npm run build
npm run check:links

LINK_REPORT_SRC="build/link-report.json"
LINK_REPORT_DST="${PORTAL_ARTIFACT_DIR}/link-report.json"
if [[ -f "${LINK_REPORT_SRC}" ]]; then
  cp "${LINK_REPORT_SRC}" "${LINK_REPORT_DST}"
  echo "[docs-portal] saved link report to ${LINK_REPORT_DST}"
else
  echo "error: docs portal link-report.json not found after check:links run" >&2
  exit 1
fi

node - <<'NODE'
const fs = require('node:fs');
const path = require('node:path');

const expectations = [
  {file: ['index.html'], needle: 'Build on Iroha with confidence'},
  {file: ['sorafs', 'quickstart', 'index.html'], needle: 'Simulate multi-provider retrieval'},
  {file: ['sorafs', 'manifest-pipeline', 'index.html'], needle: 'Chunk deterministically'},
  {file: ['norito', 'overview', 'index.html'], needle: 'Norito is the binary serialization layer'},
  {file: ['reference', 'publishing-checklist', 'index.html'], needle: 'Publishing Checklist'}
];

for (const {file, needle} of expectations) {
  const target = path.join('build', ...file);
  const html = fs.readFileSync(target, 'utf8');
  if (!html.includes(needle)) {
    console.error(`error: expected text "${needle}" missing from ${target}`);
    process.exit(1);
  }
}
NODE
