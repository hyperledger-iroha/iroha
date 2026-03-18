#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

ARTIFACT_DIR_INPUT="${1:-${SCRIPT_DIR}/artifacts}"
AUTHORITY="${AUTHORITY:-simulation@operator}"
PRIVATE_KEY="${PRIVATE_KEY:-ed25519:simulation00000000000000000000000000000000000000000000000000000000000000}"

mkdir -p "${ARTIFACT_DIR_INPUT}"
ARTIFACT_DIR="$(cd "${ARTIFACT_DIR_INPUT}" && pwd)"

declare -r SCRIPT_DIR REPO_ROOT ARTIFACT_DIR AUTHORITY PRIVATE_KEY

log() {
  printf '[capacity-sim] %s\n' "$*" >&2
}

ensure_dir() {
  mkdir -p "$1"
}

run_capacity() {
  (cd "${REPO_ROOT}" && cargo run -q -p sorafs_car --features cli --bin sorafs_manifest_stub -- capacity "$@")
}

ensure_dir "${ARTIFACT_DIR}/quota_negotiation"
ensure_dir "${ARTIFACT_DIR}/failover"
ensure_dir "${ARTIFACT_DIR}/slashing"

log "Generating provider declarations..."
for provider in alpha beta gamma; do
  spec="${SCRIPT_DIR}/scenarios/quota_negotiation/provider_${provider}_declaration.json"
  prefix="${ARTIFACT_DIR}/quota_negotiation/provider_${provider}_declaration"
  run_capacity declaration \
    "--spec=${spec}" \
    "--json-out=${prefix}_summary.json" \
    "--norito-out=${prefix}.to" \
    "--base64-out=${prefix}.b64" \
    "--request-out=${prefix}_request.json" \
    "--authority=${AUTHORITY}" \
    "--private-key=${PRIVATE_KEY}" \
    --quiet
done

log "Building replication order artefacts..."
repl_spec="${SCRIPT_DIR}/scenarios/quota_negotiation/replication_order.json"
repl_prefix="${ARTIFACT_DIR}/quota_negotiation/replication_order"
run_capacity replication-order \
  "--spec=${repl_spec}" \
  "--json-out=${repl_prefix}_summary.json" \
  "--norito-out=${repl_prefix}.to" \
  "--base64-out=${repl_prefix}.b64" \
  --quiet

log "Capturing telemetry windows for failover scenario..."
telemetry_authority="${AUTHORITY}"
telemetry_key="${PRIVATE_KEY}"

telemetry_specs=(
  "alpha_primary:${SCRIPT_DIR}/scenarios/failover/telemetry_alpha_primary.json"
  "alpha_outage:${SCRIPT_DIR}/scenarios/failover/telemetry_alpha_outage.json"
  "beta_failover:${SCRIPT_DIR}/scenarios/failover/telemetry_beta_failover.json"
)

for entry in "${telemetry_specs[@]}"; do
  name="${entry%%:*}"
  spec="${entry#*:}"
  prefix="${ARTIFACT_DIR}/failover/telemetry_${name}"
  run_capacity telemetry \
    "--spec=${spec}" \
    "--json-out=${prefix}_summary.json" \
    "--norito-out=${prefix}.to" \
    "--base64-out=${prefix}.b64" \
    "--request-out=${prefix}_request.json" \
    "--authority=${telemetry_authority}" \
    "--private-key=${telemetry_key}" \
    --quiet
done

log "Emitting slashing dispute artefacts..."
dispute_spec="${SCRIPT_DIR}/scenarios/slashing/capacity_dispute.json"
dispute_prefix="${ARTIFACT_DIR}/slashing/capacity_dispute"
run_capacity dispute \
  "--spec=${dispute_spec}" \
  "--json-out=${dispute_prefix}_summary.json" \
  "--norito-out=${dispute_prefix}.to" \
  "--base64-out=${dispute_prefix}.b64" \
  "--request-out=${dispute_prefix}_request.json" \
  "--authority=${AUTHORITY}" \
  "--private-key=${PRIVATE_KEY}" \
  --quiet

log "Capacity simulation CLI artefacts written to ${ARTIFACT_DIR}"
