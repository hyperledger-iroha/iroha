#!/usr/bin/env bash
set -euo pipefail

SORA_PAYLOAD=${SORA_PAYLOAD:-""}
SORA_OUTPUT_DIR=${SORA_OUTPUT_DIR:-"artifacts"}
SIGSTORE_ID_TOKEN=${SIGSTORE_ID_TOKEN:-""}
TORII_URL=${TORII_URL:-""}
SORA_AUTHORITY=${SORA_AUTHORITY:-""}
SORA_PRIVATE_KEY=${SORA_PRIVATE_KEY:-""}
SORA_CHUNKER_HANDLE=${SORA_CHUNKER_HANDLE:-"sorafs.sf1@1.0.0"}
SORA_PROOF_ENDPOINT=${SORA_PROOF_ENDPOINT:-""}
SORA_STREAM_TOKEN=${SORA_STREAM_TOKEN:-""}
SORA_PROVIDER_ID=${SORA_PROVIDER_ID:-""}

if [[ -z "${SORA_PAYLOAD}" ]]; then
  echo "error: SORA_PAYLOAD must point to a file or directory" >&2
  exit 1
fi
if [[ -z "${TORII_URL}" ]]; then
  echo "error: TORII_URL must reference the target Torii endpoint" >&2
  exit 1
fi
if [[ -z "${SORA_AUTHORITY}" || -z "${SORA_PRIVATE_KEY}" ]]; then
  echo "error: SORA_AUTHORITY and SORA_PRIVATE_KEY are required for manifest submission" >&2
  exit 1
fi

mkdir -p "${SORA_OUTPUT_DIR}"

CAR_PATH="${SORA_OUTPUT_DIR}/payload.car"
PLAN_PATH="${SORA_OUTPUT_DIR}/chunk_plan.json"
CAR_SUMMARY="${SORA_OUTPUT_DIR}/car_summary.json"
MANIFEST_PATH="${SORA_OUTPUT_DIR}/manifest.to"
MANIFEST_JSON="${SORA_OUTPUT_DIR}/manifest.json"
SIGNATURE_BUNDLE="${SORA_OUTPUT_DIR}/manifest.bundle.json"
SIGNATURE_HEX="${SORA_OUTPUT_DIR}/manifest.sig"
SUBMIT_SUMMARY="${SORA_OUTPUT_DIR}/submit_summary.json"
PROOF_STREAM_SUMMARY="${SORA_OUTPUT_DIR}/proof_stream_summary.json"
PROOF_VERIFY_SUMMARY="${SORA_OUTPUT_DIR}/proof_verify_summary.json"

echo "==> Packing CAR archive from ${SORA_PAYLOAD}"
sorafs_cli car pack \
  --input "${SORA_PAYLOAD}" \
  --chunker-handle "${SORA_CHUNKER_HANDLE}" \
  --car-out "${CAR_PATH}" \
  --plan-out "${PLAN_PATH}" \
  --summary-out "${CAR_SUMMARY}"

echo "==> Building manifest"
sorafs_cli manifest build \
  --summary "${CAR_SUMMARY}" \
  --chunk-plan "${PLAN_PATH}" \
  --manifest-out "${MANIFEST_PATH}" \
  --manifest-json-out "${MANIFEST_JSON}"

echo "==> Signing manifest with Sigstore (OIDC fallback: SIGSTORE_ID_TOKEN)"
if [[ -n "${SIGSTORE_ID_TOKEN}" ]]; then
  export SIGSTORE_ID_TOKEN
fi
sorafs_cli manifest sign \
  --manifest "${MANIFEST_PATH}" \
  --bundle-out "${SIGNATURE_BUNDLE}" \
  --signature-out "${SIGNATURE_HEX}"

echo "==> Submitting manifest to Torii"
sorafs_cli manifest submit \
  --manifest "${MANIFEST_PATH}" \
  --chunk-plan "${PLAN_PATH}" \
  --torii-url "${TORII_URL}" \
  --resolve-submitted-epoch=true \
  --authority "${SORA_AUTHORITY}" \
  --private-key "${SORA_PRIVATE_KEY}" \
  --summary-out "${SUBMIT_SUMMARY}"

if [[ -n "${SORA_PROOF_ENDPOINT}" && -n "${SORA_STREAM_TOKEN}" && -n "${SORA_PROVIDER_ID}" ]]; then
  echo "==> Streaming proofs from ${SORA_PROOF_ENDPOINT}"
  sorafs_cli proof stream \
    --manifest "${MANIFEST_PATH}" \
    --gateway-url "${SORA_PROOF_ENDPOINT}" \
    --provider-id "${SORA_PROVIDER_ID}" \
    --samples 32 \
    --stream-token "${SORA_STREAM_TOKEN}" \
    --summary-out "${PROOF_STREAM_SUMMARY}"
fi

echo "==> Verifying CAR contents locally"
sorafs_cli proof verify \
  --manifest "${MANIFEST_PATH}" \
  --car "${CAR_PATH}" \
  --chunk-plan "${PLAN_PATH}" \
  --summary-out "${PROOF_VERIFY_SUMMARY}"

echo "==> Complete. Artefacts written to ${SORA_OUTPUT_DIR}"
