#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCS_DIR="${ROOT_DIR}/docs/source/sdk/android/generated"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/android/codegen_docs"
SUMMARY_SRC="${ROOT_DIR}/artifacts/android/codegen_parity_summary.json"

cd "${ROOT_DIR}"

echo "[android-codegen] generating docs and parity summary via make android-codegen-verify"
make android-codegen-verify

if [[ ! -f "${SUMMARY_SRC}" ]]; then
  echo "[android-codegen] missing parity summary at ${SUMMARY_SRC}" >&2
  exit 1
fi

mkdir -p "${ARTIFACT_DIR}"

echo "[android-codegen] computing hash tree for ${DOCS_DIR}"
bash scripts/docs/hash_tree.sh "${DOCS_DIR}" "${ARTIFACT_DIR}/hash_tree.json"

echo "[android-codegen] archiving generated Markdown"
tar -czf "${ARTIFACT_DIR}/generated-md.tar.gz" -C "$(dirname "${DOCS_DIR}")" "$(basename "${DOCS_DIR}")"

cp "${SUMMARY_SRC}" "${ARTIFACT_DIR}/codegen_parity_summary.json"

HASH_TREE_SOURCE="${DOCS_DIR}/codegen_hash_tree.json"
if [[ -f "${HASH_TREE_SOURCE}" ]]; then
  cp "${HASH_TREE_SOURCE}" "${ARTIFACT_DIR}/codegen_hash_tree.json"
fi

echo "[android-codegen] artefacts ready under ${ARTIFACT_DIR}"
