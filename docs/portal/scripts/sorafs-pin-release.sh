#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

log() {
  printf '[sorafs-pin] %s\n' "$*" >&2
}

err() {
  log "error: $*"
  exit 1
}

split_list() {
  local raw="${1:-}"
  local -n dest_ref="$2"
  if [[ -z "${raw}" ]]; then
    return
  fi
  local token trimmed
  for token in ${raw//,/ }; do
    trimmed="${token#"${token%%[![:space:]]*}"}"
    trimmed="${trimmed%"${trimmed##*[![:space:]]}"}"
    [[ -n "${trimmed}" ]] && dest_ref+=("${trimmed}")
  done
}

append_asset_descriptor() {
  local target="$1"
  local label="$2"
  local verify_summary="$3"
  local proof_summary="$4"
  local bundle_path="$5"
  local signature_path="$6"
  local sign_summary="$7"
  local submit_summary="$8"
  local submission_flag="$9"
  local alias_namespace="${10}"
  local alias_name="${11}"
  local alias_proof="${12}"
  local metadata_label="${13}"
  local alias_label="${14}"
  local torii_url="${15}"
  local submitted_epoch="${16}"
  local authority="${17}"
  local car_path="${18}"
  local plan_path="${19}"
  python3 - "$target" <<'PY'
import json, sys, pathlib

(
    path,
    label,
    verify_summary,
    proof_summary,
    bundle_path,
    signature_path,
    sign_summary,
    submit_summary,
    submission_flag,
    alias_namespace,
    alias_name,
    alias_proof,
    metadata_label,
    alias_label,
    torii_url,
    submitted_epoch,
    authority,
    car_path,
    plan_path,
) = sys.argv[1:20]

entry = {
    "label": label,
    "verify_summary": verify_summary or None,
    "proof_summary": proof_summary or None,
    "bundle_path": bundle_path or None,
    "signature_path": signature_path or None,
    "sign_summary": sign_summary or None,
    "submit_summary": submit_summary or None,
    "submission_performed": submission_flag.lower() == "true",
    "alias_namespace": alias_namespace or None,
    "alias_name": alias_name or None,
    "alias_proof_path": alias_proof or None,
    "metadata_label": metadata_label or None,
    "alias_label": alias_label or None,
    "torii_url": torii_url or None,
    "submitted_epoch": submitted_epoch or None,
    "authority": authority or None,
    "car_path": car_path or None,
    "plan_path": plan_path or None,
}

target = pathlib.Path(path)
if target.exists():
    try:
        existing = json.loads(target.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover
        raise SystemExit(f"failed to parse {path}: {exc}") from exc
else:
    existing = []

existing.append(entry)
target.write_text(json.dumps(existing, indent=2), encoding="utf-8")
PY
}

package_payload() {
  local label="$1"
  local input_path="$2"
  local prefix="$3"
  local metadata_label="$4"
  local alias_label="$5"
  local alias_namespace="$6"
  local alias_name="$7"
  local alias_proof="$8"
  local successor_of="$9"
  shift 9
  local metadata_flags=("$@")

  local car="${prefix}.car"
  local plan="${prefix}.plan.json"
  local summary="${prefix}.car.summary.json"
  local manifest="${prefix}.manifest.to"
  local manifest_json="${prefix}.manifest.json"
  local bundle="${prefix}.manifest.bundle.json"
  local signature="${prefix}.manifest.sig"
  local sign_summary="${prefix}.manifest.sign.summary.json"
  local verify_summary="${prefix}.manifest.verify.summary.json"
  local proof_summary="${prefix}.proof.summary.json"
  local submit_summary="${prefix}.manifest.submit.summary.json"
  local submit_response="${prefix}.manifest.submit.response.json"

  log "Packing ${label} payload (${input_path})"
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    car pack \
    --input="${input_path}" \
    --chunker-handle="${CHUNKER_HANDLE}" \
    --car-out="${car}" \
    --plan-out="${plan}" \
    --summary-out="${summary}"

  local manifest_args=(
    "--summary=${summary}"
    "--manifest-out=${manifest}"
    "--manifest-json-out=${manifest_json}"
    "--pin-min-replicas=${PIN_MIN_REPLICAS}"
    "--pin-storage-class=${PIN_STORAGE_CLASS}"
    "--pin-retention-epoch=${PIN_RETENTION_EPOCH}"
  )
  if [[ -n "${metadata_label}" ]]; then
    manifest_args+=( "--metadata" "label=${metadata_label}" )
  fi
  if [[ -n "${alias_label}" ]]; then
    manifest_args+=( "--metadata" "alias_label=${alias_label}" )
  fi
  if [[ "${#metadata_flags[@]}" -gt 0 ]]; then
    manifest_args+=( "${metadata_flags[@]}" )
  fi

  log "Building ${label} manifest"
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    manifest build \
    "${manifest_args[@]}"

  log "Signing ${label} manifest bundle via Sigstore"
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    manifest sign \
    --manifest="${manifest}" \
    --chunk-plan="${plan}" \
    --bundle-out="${bundle}" \
    --signature-out="${signature}" \
    --include-token="${INCLUDE_TOKEN}" \
    "${identity_args[@]}" | tee "${sign_summary}"

  log "Verifying ${label} manifest signature bundle"
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    manifest verify-signature \
    --manifest="${manifest}" \
    --bundle="${bundle}" \
    --chunk-plan="${plan}" | tee "${verify_summary}"

  log "Verifying ${label} CAR payload"
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    proof verify \
    --manifest="${manifest}" \
    --car="${car}" | tee "${proof_summary}"

  local submit_performed=false
  if ! "${SKIP_SUBMIT}" && [[ -n "${TORII_URL}" ]] && [[ -n "${AUTHORITY}" ]] && [[ -n "${SUBMITTED_EPOCH}" ]]; then
    local submit_args=(
      "--manifest=${manifest}"
      "--torii-url=${TORII_URL}"
      "--submitted-epoch=${SUBMITTED_EPOCH}"
      "--chunk-plan=${plan}"
      "--authority=${AUTHORITY}"
      "--summary-out=${submit_summary}"
      "--response-out=${submit_response}"
    )
    if [[ -n "${PRIVATE_KEY}" && -n "${PRIVATE_KEY_FILE}" ]]; then
      err "only one of --private-key or --private-key-file may be provided"
    elif [[ -n "${PRIVATE_KEY}" ]]; then
      submit_args+=( "--private-key=${PRIVATE_KEY}" )
    elif [[ -n "${PRIVATE_KEY_FILE}" ]]; then
      submit_args+=( "--private-key-file=${PRIVATE_KEY_FILE}" )
    elif [[ -n "${SORA_FS_PRIVATE_KEY_FILE:-}" ]]; then
      submit_args+=( "--private-key-file=${SORA_FS_PRIVATE_KEY_FILE}" )
    else
      err "missing private key for manifest submission"
    fi

    if [[ -n "${alias_namespace}" || -n "${alias_name}" || -n "${alias_proof}" ]]; then
      if [[ -z "${alias_namespace}" || -z "${alias_name}" || -z "${alias_proof}" ]]; then
        err "${label} alias namespace, name, and proof must all be provided together"
      fi
      submit_args+=(
        "--alias-namespace=${alias_namespace}"
        "--alias-name=${alias_name}"
        "--alias-proof=${alias_proof}"
      )
    fi

    if [[ -n "${successor_of}" ]]; then
      submit_args+=( "--successor-of=${successor_of}" )
    fi

    log "Submitting ${label} manifest to Torii (${TORII_URL})"
    cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
      manifest submit \
      "${submit_args[@]}"
    submit_performed=true
  else
    log "Skipping ${label} manifest submission (SKIP_SUBMIT=${SKIP_SUBMIT}, TORII_URL='${TORII_URL}')"
  fi

  LAST_MANIFEST_PATH="${manifest}"
  LAST_MANIFEST_JSON="${manifest_json}"
  LAST_MANIFEST_BUNDLE="${bundle}"
  LAST_MANIFEST_SIGNATURE="${signature}"
  LAST_SIGN_SUMMARY="${sign_summary}"
  LAST_VERIFY_SUMMARY="${verify_summary}"
  LAST_PROOF_SUMMARY="${proof_summary}"
  LAST_SUBMIT_SUMMARY="${submit_summary}"
  LAST_SUBMIT_RESPONSE="${submit_response}"
  LAST_SUBMISSION_PERFORMED="${submit_performed}"
  LAST_ALIAS_NAMESPACE="${alias_namespace}"
  LAST_ALIAS_NAME="${alias_name}"
  LAST_ALIAS_PROOF="${alias_proof}"
  LAST_METADATA_LABEL="${metadata_label}"
  LAST_ALIAS_LABEL="${alias_label}"
  LAST_CAR_PATH="${car}"
  LAST_PLAN_PATH="${plan}"
}

generate_zonefile_bundle() {
  local plan_path="$1"
  if [[ -z "${DNS_ZONEFILE_OUT}" && -z "${DNS_ZONEFILE_RESOLVER_SNIPPET}" ]]; then
    return
  fi
  local zonefile_out="${DNS_ZONEFILE_OUT}"
  if [[ -z "${zonefile_out}" ]]; then
    if [[ -n "${DNS_ZONE}" && -n "${DNS_HOSTNAME}" ]]; then
      zonefile_out="artifacts/sns/zonefiles/${DNS_ZONE}/${DNS_HOSTNAME}.json"
    else
      log "Skipping zonefile skeleton: provide --dns-zonefile-out or set DNS_ZONE + DNS_HOSTNAME"
      return
    fi
  fi
  local resolver_snippet="${DNS_ZONEFILE_RESOLVER_SNIPPET}"
  if [[ -z "${resolver_snippet}" && -n "${DNS_HOSTNAME}" ]]; then
    resolver_snippet="ops/soradns/static_zones.${DNS_HOSTNAME}.json"
  fi
  if [[ ${#DNS_ZONEFILE_IPV4_VALUES[@]} -eq 0 && ${#DNS_ZONEFILE_IPV6_VALUES[@]} -eq 0 && -z "${DNS_ZONEFILE_CNAME}" ]]; then
    log "Skipping zonefile skeleton: no IPv4/IPv6/CNAME entries were supplied"
    return
  fi

  local -a args=( "--cutover-plan=${plan_path}" "--out=${zonefile_out}" )
  if [[ -n "${resolver_snippet}" ]]; then
    args+=( "--resolver-snippet-out=${resolver_snippet}" )
  fi
  if [[ -n "${DNS_ZONEFILE_TTL}" ]]; then
    args+=( "--ttl=${DNS_ZONEFILE_TTL}" )
  fi
  if [[ -n "${DNS_ZONEFILE_VERSION}" ]]; then
    args+=( "--zonefile-version=${DNS_ZONEFILE_VERSION}" )
  fi
  if [[ -n "${DNS_ZONEFILE_EFFECTIVE_AT}" ]]; then
    args+=( "--effective-at=${DNS_ZONEFILE_EFFECTIVE_AT}" )
  fi
  if [[ -n "${DNS_ZONEFILE_PROOF}" ]]; then
    args+=( "--zonefile-proof=${DNS_ZONEFILE_PROOF}" )
  fi
  if [[ -n "${DNS_ZONEFILE_CID}" ]]; then
    args+=( "--zonefile-cid=${DNS_ZONEFILE_CID}" )
  fi
  if [[ -n "${DNS_GAR_DIGEST}" ]]; then
    args+=( "--gar-digest=${DNS_GAR_DIGEST}" )
  fi
  if [[ -n "${DNS_ZONEFILE_FREEZE_STATE}" ]]; then
    args+=( "--freeze-state=${DNS_ZONEFILE_FREEZE_STATE}" )
  fi
  if [[ -n "${DNS_ZONEFILE_FREEZE_TICKET}" ]]; then
    args+=( "--freeze-ticket=${DNS_ZONEFILE_FREEZE_TICKET}" )
  fi
  if [[ -n "${DNS_ZONEFILE_FREEZE_EXPIRES_AT}" ]]; then
    args+=( "--freeze-expires-at=${DNS_ZONEFILE_FREEZE_EXPIRES_AT}" )
  fi
  local note
  for note in "${DNS_ZONEFILE_FREEZE_NOTES_VALUES[@]}"; do
    args+=( "--freeze-note=${note}" )
  done
  local value
  for value in "${DNS_ZONEFILE_IPV4_VALUES[@]}"; do
    args+=( "--ipv4=${value}" )
  done
  for value in "${DNS_ZONEFILE_IPV6_VALUES[@]}"; do
    args+=( "--ipv6=${value}" )
  done
  if [[ -n "${DNS_ZONEFILE_CNAME}" ]]; then
    args+=( "--cname-target=${DNS_ZONEFILE_CNAME}" )
  fi
  for value in "${DNS_ZONEFILE_SPKI_VALUES[@]}"; do
    args+=( "--spki-pin=${value}" )
  done
  for value in "${DNS_ZONEFILE_TXT_VALUES[@]}"; do
    args+=( "--txt=${value}" )
  done
  if [[ -n "${DNS_CHANGE_TICKET}" ]]; then
    args+=( "--txt" "ChangeTicket=${DNS_CHANGE_TICKET}" )
  fi

  log "Generating zonefile skeleton (${zonefile_out})"
  if ! python3 scripts/sns_zonefile_skeleton.py "${args[@]}"; then
    err "failed to generate zonefile skeleton"
  fi
}

usage() {
  cat <<'EOF'
Usage: sorafs-pin-release.sh [options]

Packages the docs portal build output, emits reproducible SoraFS artefacts
(CAR, manifest, plan, bundle), verifies them, and optionally submits the
manifest to Torii. Environment variables provide sane defaults so CI workflows
can wire secrets without inline arguments.

Options:
  --build-dir PATH              Path to built docs (default: build)
  --artifact-dir PATH           Directory for general artefacts (default: artifacts)
  --sorafs-dir PATH             Directory for SoraFS artefacts (default: $artifact_dir/sorafs)
  --chunker-handle HANDLE       Chunker handle (default: sorafs.sf1@1.0.0)
  --pin-min-replicas N          Pin policy replicas (default: 5)
  --pin-storage-class CLASS     hot|warm|cold (default: warm)
  --pin-retention-epoch N       Retention epoch (default: 14)
  --pin-label LABEL             Metadata label recorded in manifest
  --alias LABEL                 Human readable alias label for reporting
  --proposal-alias LABEL        Alias proposal recorded in artefacts
  --alias-namespace NS          Alias namespace for manifest submit
  --alias-name NAME             Alias name for manifest submit
  --alias-proof PATH            Alias proof bundle for manifest submit
  --torii-url URL               Torii endpoint root for manifest submit
  --submitted-epoch N           Epoch recorded with submission
  --authority ACCOUNT_ID        Account ID used to sign the submission
  --private-key KEY             Private key for submission authority
  --private-key-file PATH       Private key file path for submission authority
  --successor-of HEX            Optional successor-of manifest digest
  --identity-token-provider P   Sigstore provider (e.g., github-actions)
  --identity-token-audience AUD Sigstore audience (required with provider)
  --identity-token-env VAR      Env var exposing a Sigstore OIDC token
  --include-token true|false    Whether to embed the OIDC token in bundle (default: false)
  --dns-change-ticket ID        Change ticket recorded in the DNS cutover descriptor
  --dns-cutover-window RANGE    ISO8601 window for production DNS cutover (e.g., 2026-03-21T15:00Z/2026-03-21T15:30Z)
  --dns-hostname HOSTNAME       Production hostname (defaults to \$DNS_HOSTNAME when set)
  --dns-zone ZONE               DNS zone (defaults to \$DNS_ZONE when set)
  --ops-contact CONTACT         Ops/SRE contact recorded in the descriptor
  --cache-purge-endpoint URL    Cache purge endpoint recorded in the DNS cutover descriptor
  --cache-purge-auth-env VAR    Env var name carrying the cache purge token (default: CACHE_PURGE_TOKEN)
  --previous-dns-plan PATH      Path to the previous DNS cutover descriptor for rollback metadata
  --dns-zonefile-out PATH       Destination for the generated zonefile skeleton (defaults to artifacts/sns/zonefiles/<zone>/<hostname>.json when DNS zone/hostname are known)
  --dns-zonefile-resolver-snippet PATH  Resolver snippet path for static zone injection
  --dns-zonefile-ttl SECONDS    TTL applied to generated DNS records (default: 600)
  --dns-zonefile-ipv4 ADDRESS   IPv4 address to publish (repeatable)
  --dns-zonefile-ipv6 ADDRESS   IPv6 address to publish (repeatable)
  --dns-zonefile-cname TARGET   Optional CNAME target published alongside A/AAAA records
  --dns-zonefile-spki-pin PIN   SHA-256 SPKI pin (base64) recorded in TXT entries (repeatable)
  --dns-zonefile-txt KEY=VALUE  Additional TXT entry recorded in the skeleton (repeatable)
  --dns-zonefile-freeze-state STATE    Guardian freeze state recorded in the zonefile metadata
  --dns-zonefile-freeze-ticket ID      Guardian/council ticket reference for freezes
  --dns-zonefile-freeze-expires-at TS  RFC3339 timestamp for thawing a freeze
  --dns-zonefile-freeze-note NOTE      Additional freeze note recorded in the metadata (repeatable)
  --dns-zonefile-version LABEL  Override the computed zonefile version label
  --dns-zonefile-effective-at TS  Override the `effective_at` timestamp (RFC3339)
  --dns-zonefile-proof VALUE    Override the proof literal recorded in the zonefile metadata
  --dns-zonefile-cid VALUE      Override the CID recorded in the zonefile metadata
  --dns-gar-digest HEX          BLAKE3 digest of the signed GAR payload (required for gateway zonefiles)
  --route-rollback-manifest PATH  Manifest JSON used when generating the route rollback headers
  --route-rollback-label LABEL    Human-readable label for the rollback route binding
  --route-rollback-release-tag TAG  Release tag recorded in the rollback plan metadata
  --openapi-dir PATH           Directory copied into the OpenAPI CAR (default: docs/portal/static/openapi)
  --openapi-label LABEL        Metadata label recorded in the OpenAPI manifest (default: docs-openapi)
  --openapi-alias LABEL        Human readable alias label for the OpenAPI artefact
  --openapi-alias-namespace NS Alias namespace for OpenAPI manifest submit
  --openapi-alias-name NAME    Alias name for OpenAPI manifest submit
  --openapi-alias-proof PATH   Alias proof bundle for OpenAPI manifest submit
  --openapi-successor-of HEX   Optional successor-of manifest digest for OpenAPI manifests
  --openapi-sbom-source PATH   OpenAPI JSON used for SBOM generation (default: docs/portal/static/openapi/torii.json)
  --portal-sbom-label LABEL    Metadata label for the portal SBOM manifest (default: docs-portal-sbom)
  --portal-sbom-alias LABEL    Human readable alias label for the portal SBOM artefact
  --portal-sbom-alias-namespace NS  Alias namespace for the portal SBOM manifest
  --portal-sbom-alias-name NAME Alias name for the portal SBOM manifest
  --portal-sbom-alias-proof PATH  Alias proof bundle for the portal SBOM manifest
  --openapi-sbom-label LABEL   Metadata label for the OpenAPI SBOM manifest (default: docs-openapi-sbom)
  --openapi-sbom-alias LABEL   Human readable alias label for the OpenAPI SBOM artefact
  --openapi-sbom-alias-namespace NS  Alias namespace for the OpenAPI SBOM manifest
  --openapi-sbom-alias-name NAME Alias name for the OpenAPI SBOM manifest
  --openapi-sbom-alias-proof PATH  Alias proof bundle for the OpenAPI SBOM manifest
  --skip-openapi               Skip OpenAPI CAR/manifest generation
  --skip-sbom                  Skip SBOM generation/packaging
  --syft-bin PATH              Path to the syft executable (default: syft)
  --skip-submit                Do not attempt Torii submission even if creds exist
  -h, --help                    Show this help message

Relevant environment defaults (can be overridden via flags):
  PIN_ALIAS, PIN_PROPOSAL_ALIAS, PIN_ALIAS_NAMESPACE, PIN_ALIAS_NAME,
  PIN_ALIAS_PROOF_PATH, TORII_URL, AUTHORITY, PRIVATE_KEY,
  PRIVATE_KEY_FILE, SUBMITTED_EPOCH, PIN_MIN_REPLICAS,
  PIN_STORAGE_CLASS, PIN_RETENTION_EPOCH, PIN_LABEL,
  CHUNKER_HANDLE, DOCS_RELEASE_TAG, DOCS_RELEASE_SOURCE,
  IDENTITY_TOKEN_PROVIDER, IDENTITY_TOKEN_AUDIENCE,
  IDENTITY_TOKEN_ENV, INCLUDE_TOKEN,
  DNS_CHANGE_TICKET, DNS_CUTOVER_WINDOW, DNS_HOSTNAME,
  DNS_ZONE, DNS_OPS_CONTACT, DNS_CACHE_PURGE_ENDPOINT,
  DNS_CACHE_PURGE_AUTH_ENV, DNS_PREVIOUS_PLAN,
  DNS_ZONEFILE_OUT, DNS_ZONEFILE_RESOLVER_SNIPPET,
  DNS_ZONEFILE_TTL, DNS_ZONEFILE_IPV4, DNS_ZONEFILE_IPV6,
  DNS_ZONEFILE_CNAME, DNS_ZONEFILE_SPKI, DNS_ZONEFILE_TXT,
  DNS_ZONEFILE_FREEZE_STATE, DNS_ZONEFILE_FREEZE_TICKET,
  DNS_ZONEFILE_FREEZE_EXPIRES_AT, DNS_ZONEFILE_FREEZE_NOTES,
  DNS_ZONEFILE_VERSION, DNS_ZONEFILE_EFFECTIVE_AT,
  DNS_ZONEFILE_PROOF, DNS_ZONEFILE_CID, DNS_GAR_DIGEST,
  ROUTE_ROLLBACK_MANIFEST_JSON, ROUTE_ROLLBACK_LABEL,
  ROUTE_ROLLBACK_RELEASE_TAG, ROUTE_PROOF_STATUS,
  OPENAPI_DIR, OPENAPI_LABEL, OPENAPI_ALIAS, OPENAPI_ALIAS_NAMESPACE,
  OPENAPI_ALIAS_NAME, OPENAPI_ALIAS_PROOF_PATH, OPENAPI_SUCCESSOR_OF,
  OPENAPI_SBOM_SOURCE, PORTAL_SBOM_LABEL, PORTAL_SBOM_ALIAS,
  PORTAL_SBOM_ALIAS_NAMESPACE, PORTAL_SBOM_ALIAS_NAME,
  PORTAL_SBOM_ALIAS_PROOF_PATH, OPENAPI_SBOM_LABEL, OPENAPI_SBOM_ALIAS,
  OPENAPI_SBOM_ALIAS_NAMESPACE, OPENAPI_SBOM_ALIAS_NAME,
  OPENAPI_SBOM_ALIAS_PROOF_PATH, SYFT_BIN
EOF
}

BUILD_DIR="${BUILD_DIR:-build}"
ARTIFACT_DIR="${ARTIFACT_DIR:-artifacts}"
SORA_DIR="${SORA_DIR:-${ARTIFACT_DIR}/sorafs}"
CHUNKER_HANDLE="${CHUNKER_HANDLE:-sorafs.sf1@1.0.0}"
PIN_MIN_REPLICAS="${PIN_MIN_REPLICAS:-5}"
PIN_STORAGE_CLASS="${PIN_STORAGE_CLASS:-warm}"
PIN_RETENTION_EPOCH="${PIN_RETENTION_EPOCH:-14}"
PIN_LABEL="${PIN_LABEL:-docs-portal}"
PRIMARY_ALIAS="${PIN_ALIAS:-}"
PROPOSAL_ALIAS="${PIN_PROPOSAL_ALIAS:-}"
ALIAS_NAMESPACE="${PIN_ALIAS_NAMESPACE:-}"
ALIAS_NAME="${PIN_ALIAS_NAME:-}"
ALIAS_PROOF_PATH="${PIN_ALIAS_PROOF_PATH:-}"
TORII_URL="${TORII_URL:-${SORA_FS_TORII_URL:-}}"
SUBMITTED_EPOCH="${SUBMITTED_EPOCH:-${SORA_FS_SUBMITTED_EPOCH:-}}"
AUTHORITY="${AUTHORITY:-${SORA_FS_AUTHORITY:-}}"
PRIVATE_KEY="${PRIVATE_KEY:-${SORA_FS_PRIVATE_KEY:-}}"
PRIVATE_KEY_FILE="${PRIVATE_KEY_FILE:-}"
SUCCESSOR_OF="${PIN_SUCCESSOR_OF:-}"
IDENTITY_TOKEN_PROVIDER="${IDENTITY_TOKEN_PROVIDER:-${SIGSTORE_IDENTITY_PROVIDER:-}}"
IDENTITY_TOKEN_AUDIENCE="${IDENTITY_TOKEN_AUDIENCE:-sorafs-devportal}"
IDENTITY_TOKEN_ENV="${IDENTITY_TOKEN_ENV:-}"
INCLUDE_TOKEN="${INCLUDE_TOKEN:-false}"
DNS_CHANGE_TICKET="${DNS_CHANGE_TICKET:-}"
DNS_CUTOVER_WINDOW="${DNS_CUTOVER_WINDOW:-}"
DNS_HOSTNAME="${DNS_HOSTNAME:-}"
DNS_ZONE="${DNS_ZONE:-}"
DNS_OPS_CONTACT="${DNS_OPS_CONTACT:-}"
DNS_CACHE_PURGE_ENDPOINT="${DNS_CACHE_PURGE_ENDPOINT:-}"
DNS_CACHE_PURGE_AUTH_ENV="${DNS_CACHE_PURGE_AUTH_ENV:-CACHE_PURGE_TOKEN}"
DNS_PREVIOUS_PLAN="${DNS_PREVIOUS_PLAN:-}"
DNS_ZONEFILE_OUT="${DNS_ZONEFILE_OUT:-}"
DNS_ZONEFILE_RESOLVER_SNIPPET="${DNS_ZONEFILE_RESOLVER_SNIPPET:-}"
DNS_ZONEFILE_TTL="${DNS_ZONEFILE_TTL:-600}"
DNS_ZONEFILE_IPV4="${DNS_ZONEFILE_IPV4:-}"
DNS_ZONEFILE_IPV6="${DNS_ZONEFILE_IPV6:-}"
DNS_ZONEFILE_CNAME="${DNS_ZONEFILE_CNAME:-}"
DNS_ZONEFILE_SPKI="${DNS_ZONEFILE_SPKI:-}"
DNS_ZONEFILE_TXT="${DNS_ZONEFILE_TXT:-}"
DNS_ZONEFILE_FREEZE_STATE="${DNS_ZONEFILE_FREEZE_STATE:-}"
DNS_ZONEFILE_FREEZE_TICKET="${DNS_ZONEFILE_FREEZE_TICKET:-}"
DNS_ZONEFILE_FREEZE_EXPIRES_AT="${DNS_ZONEFILE_FREEZE_EXPIRES_AT:-}"
DNS_ZONEFILE_FREEZE_NOTES="${DNS_ZONEFILE_FREEZE_NOTES:-}"
DNS_ZONEFILE_VERSION="${DNS_ZONEFILE_VERSION:-}"
DNS_ZONEFILE_EFFECTIVE_AT="${DNS_ZONEFILE_EFFECTIVE_AT:-}"
DNS_ZONEFILE_PROOF="${DNS_ZONEFILE_PROOF:-}"
DNS_ZONEFILE_CID="${DNS_ZONEFILE_CID:-}"
DNS_GAR_DIGEST="${DNS_GAR_DIGEST:-}"
DNS_ZONEFILE_IPV4_VALUES=()
DNS_ZONEFILE_IPV6_VALUES=()
DNS_ZONEFILE_SPKI_VALUES=()
DNS_ZONEFILE_TXT_VALUES=()
DNS_ZONEFILE_FREEZE_NOTES_VALUES=()
split_list "${DNS_ZONEFILE_IPV4}" DNS_ZONEFILE_IPV4_VALUES
split_list "${DNS_ZONEFILE_IPV6}" DNS_ZONEFILE_IPV6_VALUES
split_list "${DNS_ZONEFILE_SPKI}" DNS_ZONEFILE_SPKI_VALUES
split_list "${DNS_ZONEFILE_TXT}" DNS_ZONEFILE_TXT_VALUES
split_list "${DNS_ZONEFILE_FREEZE_NOTES}" DNS_ZONEFILE_FREEZE_NOTES_VALUES
ROUTE_ROLLBACK_MANIFEST_JSON="${ROUTE_ROLLBACK_MANIFEST_JSON:-}"
ROUTE_ROLLBACK_LABEL="${ROUTE_ROLLBACK_LABEL:-}"
ROUTE_ROLLBACK_RELEASE_TAG="${ROUTE_ROLLBACK_RELEASE_TAG:-}"
ROUTE_PROOF_STATUS="${ROUTE_PROOF_STATUS:-ok}"
SKIP_SUBMIT=false
OPENAPI_DIR="${OPENAPI_DIR:-docs/portal/static/openapi}"
OPENAPI_LABEL="${OPENAPI_LABEL:-docs-openapi}"
OPENAPI_ALIAS="${OPENAPI_ALIAS:-}"
OPENAPI_ALIAS_NAMESPACE="${OPENAPI_ALIAS_NAMESPACE:-}"
OPENAPI_ALIAS_NAME="${OPENAPI_ALIAS_NAME:-}"
OPENAPI_ALIAS_PROOF_PATH="${OPENAPI_ALIAS_PROOF_PATH:-}"
OPENAPI_SUCCESSOR_OF="${OPENAPI_SUCCESSOR_OF:-}"
OPENAPI_SBOM_SOURCE="${OPENAPI_SBOM_SOURCE:-docs/portal/static/openapi/torii.json}"
PORTAL_SBOM_LABEL="${PORTAL_SBOM_LABEL:-docs-portal-sbom}"
PORTAL_SBOM_ALIAS="${PORTAL_SBOM_ALIAS:-}"
PORTAL_SBOM_ALIAS_NAMESPACE="${PORTAL_SBOM_ALIAS_NAMESPACE:-}"
PORTAL_SBOM_ALIAS_NAME="${PORTAL_SBOM_ALIAS_NAME:-}"
PORTAL_SBOM_ALIAS_PROOF_PATH="${PORTAL_SBOM_ALIAS_PROOF_PATH:-}"
OPENAPI_SBOM_LABEL="${OPENAPI_SBOM_LABEL:-docs-openapi-sbom}"
OPENAPI_SBOM_ALIAS="${OPENAPI_SBOM_ALIAS:-}"
OPENAPI_SBOM_ALIAS_NAMESPACE="${OPENAPI_SBOM_ALIAS_NAMESPACE:-}"
OPENAPI_SBOM_ALIAS_NAME="${OPENAPI_SBOM_ALIAS_NAME:-}"
OPENAPI_SBOM_ALIAS_PROOF_PATH="${OPENAPI_SBOM_ALIAS_PROOF_PATH:-}"
SKIP_OPENAPI=false
SKIP_SBOM=false
SYFT_BIN="${SYFT_BIN:-syft}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build-dir) BUILD_DIR="$2"; shift 2 ;;
    --artifact-dir) ARTIFACT_DIR="$2"; shift 2 ;;
    --sorafs-dir) SORA_DIR="$2"; shift 2 ;;
    --chunker-handle) CHUNKER_HANDLE="$2"; shift 2 ;;
    --pin-min-replicas) PIN_MIN_REPLICAS="$2"; shift 2 ;;
    --pin-storage-class) PIN_STORAGE_CLASS="$2"; shift 2 ;;
    --pin-retention-epoch) PIN_RETENTION_EPOCH="$2"; shift 2 ;;
    --pin-label) PIN_LABEL="$2"; shift 2 ;;
    --alias) PRIMARY_ALIAS="$2"; shift 2 ;;
    --proposal-alias) PROPOSAL_ALIAS="$2"; shift 2 ;;
    --alias-namespace) ALIAS_NAMESPACE="$2"; shift 2 ;;
    --alias-name) ALIAS_NAME="$2"; shift 2 ;;
    --alias-proof) ALIAS_PROOF_PATH="$2"; shift 2 ;;
    --torii-url) TORII_URL="$2"; shift 2 ;;
    --submitted-epoch) SUBMITTED_EPOCH="$2"; shift 2 ;;
    --authority) AUTHORITY="$2"; shift 2 ;;
    --private-key) PRIVATE_KEY="$2"; shift 2 ;;
    --private-key-file) PRIVATE_KEY_FILE="$2"; shift 2 ;;
    --successor-of) SUCCESSOR_OF="$2"; shift 2 ;;
    --identity-token-provider) IDENTITY_TOKEN_PROVIDER="$2"; shift 2 ;;
    --identity-token-audience) IDENTITY_TOKEN_AUDIENCE="$2"; shift 2 ;;
    --identity-token-env) IDENTITY_TOKEN_ENV="$2"; shift 2 ;;
    --include-token) INCLUDE_TOKEN="$2"; shift 2 ;;
    --dns-change-ticket) DNS_CHANGE_TICKET="$2"; shift 2 ;;
    --dns-cutover-window) DNS_CUTOVER_WINDOW="$2"; shift 2 ;;
    --dns-hostname) DNS_HOSTNAME="$2"; shift 2 ;;
    --dns-zone) DNS_ZONE="$2"; shift 2 ;;
    --ops-contact) DNS_OPS_CONTACT="$2"; shift 2 ;;
    --cache-purge-endpoint) DNS_CACHE_PURGE_ENDPOINT="$2"; shift 2 ;;
    --cache-purge-auth-env) DNS_CACHE_PURGE_AUTH_ENV="$2"; shift 2 ;;
    --previous-dns-plan) DNS_PREVIOUS_PLAN="$2"; shift 2 ;;
    --dns-zonefile-out) DNS_ZONEFILE_OUT="$2"; shift 2 ;;
    --dns-zonefile-resolver-snippet) DNS_ZONEFILE_RESOLVER_SNIPPET="$2"; shift 2 ;;
    --dns-zonefile-ttl) DNS_ZONEFILE_TTL="$2"; shift 2 ;;
    --dns-zonefile-ipv4) DNS_ZONEFILE_IPV4_VALUES+=("$2"); shift 2 ;;
    --dns-zonefile-ipv6) DNS_ZONEFILE_IPV6_VALUES+=("$2"); shift 2 ;;
    --dns-zonefile-cname) DNS_ZONEFILE_CNAME="$2"; shift 2 ;;
    --dns-zonefile-spki-pin) DNS_ZONEFILE_SPKI_VALUES+=("$2"); shift 2 ;;
    --dns-zonefile-txt) DNS_ZONEFILE_TXT_VALUES+=("$2"); shift 2 ;;
    --dns-zonefile-freeze-state) DNS_ZONEFILE_FREEZE_STATE="$2"; shift 2 ;;
    --dns-zonefile-freeze-ticket) DNS_ZONEFILE_FREEZE_TICKET="$2"; shift 2 ;;
    --dns-zonefile-freeze-expires-at) DNS_ZONEFILE_FREEZE_EXPIRES_AT="$2"; shift 2 ;;
    --dns-zonefile-freeze-note) DNS_ZONEFILE_FREEZE_NOTES_VALUES+=("$2"); shift 2 ;;
    --dns-zonefile-version) DNS_ZONEFILE_VERSION="$2"; shift 2 ;;
    --dns-zonefile-effective-at) DNS_ZONEFILE_EFFECTIVE_AT="$2"; shift 2 ;;
    --dns-zonefile-proof) DNS_ZONEFILE_PROOF="$2"; shift 2 ;;
    --dns-zonefile-cid) DNS_ZONEFILE_CID="$2"; shift 2 ;;
    --dns-gar-digest) DNS_GAR_DIGEST="$2"; shift 2 ;;
    --route-rollback-manifest) ROUTE_ROLLBACK_MANIFEST_JSON="$2"; shift 2 ;;
    --route-rollback-label) ROUTE_ROLLBACK_LABEL="$2"; shift 2 ;;
    --route-rollback-release-tag) ROUTE_ROLLBACK_RELEASE_TAG="$2"; shift 2 ;;
    --openapi-dir) OPENAPI_DIR="$2"; shift 2 ;;
    --openapi-label) OPENAPI_LABEL="$2"; shift 2 ;;
    --openapi-alias) OPENAPI_ALIAS="$2"; shift 2 ;;
    --openapi-alias-namespace) OPENAPI_ALIAS_NAMESPACE="$2"; shift 2 ;;
    --openapi-alias-name) OPENAPI_ALIAS_NAME="$2"; shift 2 ;;
    --openapi-alias-proof) OPENAPI_ALIAS_PROOF_PATH="$2"; shift 2 ;;
    --openapi-successor-of) OPENAPI_SUCCESSOR_OF="$2"; shift 2 ;;
    --skip-openapi) SKIP_OPENAPI=true; shift ;;
    --openapi-sbom-source) OPENAPI_SBOM_SOURCE="$2"; shift 2 ;;
    --portal-sbom-label) PORTAL_SBOM_LABEL="$2"; shift 2 ;;
    --portal-sbom-alias) PORTAL_SBOM_ALIAS="$2"; shift 2 ;;
    --portal-sbom-alias-namespace) PORTAL_SBOM_ALIAS_NAMESPACE="$2"; shift 2 ;;
    --portal-sbom-alias-name) PORTAL_SBOM_ALIAS_NAME="$2"; shift 2 ;;
    --portal-sbom-alias-proof) PORTAL_SBOM_ALIAS_PROOF_PATH="$2"; shift 2 ;;
    --openapi-sbom-label) OPENAPI_SBOM_LABEL="$2"; shift 2 ;;
    --openapi-sbom-alias) OPENAPI_SBOM_ALIAS="$2"; shift 2 ;;
    --openapi-sbom-alias-namespace) OPENAPI_SBOM_ALIAS_NAMESPACE="$2"; shift 2 ;;
    --openapi-sbom-alias-name) OPENAPI_SBOM_ALIAS_NAME="$2"; shift 2 ;;
    --openapi-sbom-alias-proof) OPENAPI_SBOM_ALIAS_PROOF_PATH="$2"; shift 2 ;;
    --skip-sbom) SKIP_SBOM=true; shift ;;
    --syft-bin) SYFT_BIN="$2"; shift 2 ;;
    --skip-submit) SKIP_SUBMIT=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *)
      err "unknown argument: $1"
      ;;
  esac
done

common_metadata_flags=()
if [[ -n "${DOCS_RELEASE_TAG:-}" ]]; then
  common_metadata_flags+=( "--metadata" "release_tag=${DOCS_RELEASE_TAG}" )
fi
if [[ -n "${DOCS_RELEASE_SOURCE:-}" ]]; then
  common_metadata_flags+=( "--metadata" "release_source=${DOCS_RELEASE_SOURCE}" )
fi

if [[ ! -d "${BUILD_DIR}" ]]; then
  err "build directory '${BUILD_DIR}' not found (run npm run build first)"
fi

mkdir -p "${ARTIFACT_DIR}" "${SORA_DIR}"

if [[ ! -f "${BUILD_DIR}/checksums.sha256" ]]; then
  err "missing ${BUILD_DIR}/checksums.sha256 (ensure scripts/write-checksums.mjs ran)"
fi

cp "${BUILD_DIR}/checksums.sha256" "${ARTIFACT_DIR}/checksums.sha256"
if [[ -f "${BUILD_DIR}/release.json" ]]; then
  cp "${BUILD_DIR}/release.json" "${ARTIFACT_DIR}/release.json"
fi

ARCHIVE="${SORA_DIR}/portal.tar.gz"
PORTAL_PREFIX="${SORA_DIR}/portal"
OPENAPI_PREFIX="${SORA_DIR}/openapi"
PORTAL_SBOM_PREFIX="${SORA_DIR}/portal.sbom"
OPENAPI_SBOM_PREFIX="${SORA_DIR}/openapi.sbom"
CAR_OUT="${PORTAL_PREFIX}.car"
PLAN_OUT="${PORTAL_PREFIX}.plan.json"
CAR_SUMMARY="${PORTAL_PREFIX}.car.summary.json"
MANIFEST_OUT="${PORTAL_PREFIX}.manifest.to"
MANIFEST_JSON="${PORTAL_PREFIX}.manifest.json"
BUNDLE_OUT="${PORTAL_PREFIX}.manifest.bundle.json"
SIGNATURE_OUT="${PORTAL_PREFIX}.manifest.sig"
SIGN_SUMMARY="${PORTAL_PREFIX}.manifest.sign.summary.json"
VERIFY_SUMMARY="${PORTAL_PREFIX}.manifest.verify.summary.json"
PROOF_SUMMARY="${PORTAL_PREFIX}.proof.summary.json"
SUBMIT_SUMMARY="${PORTAL_PREFIX}.manifest.submit.summary.json"
SUBMIT_RESPONSE="${PORTAL_PREFIX}.manifest.submit.response.json"
PIN_REPORT="${SORA_DIR}/portal.pin.report.json"
PIN_PROPOSAL_PATH="${SORA_DIR}/portal.pin.proposal.json"
CUTOVER_PLAN="${SORA_DIR}/portal.dns-cutover.json"
ROUTE_PLAN_JSON="${SORA_DIR}/gateway.route_plan.json"
ROUTE_HEADERS="${SORA_DIR}/gateway.route.headers.txt"
ROUTE_ROLLBACK_HEADERS="${SORA_DIR}/gateway.route.rollback.headers.txt"
ADDITIONAL_ASSETS_FILE="${SORA_DIR}/portal.additional_assets.json"

log "Archiving build output (${BUILD_DIR}) -> ${ARCHIVE}"
tar -C "${BUILD_DIR}" -czf "${ARCHIVE}" .

printf '[]\n' > "${ADDITIONAL_ASSETS_FILE}"

identity_args=()
if [[ -n "${IDENTITY_TOKEN_ENV}" ]]; then
  identity_args+=( "--identity-token-env=${IDENTITY_TOKEN_ENV}" )
elif [[ -n "${IDENTITY_TOKEN_PROVIDER}" ]]; then
  if [[ -z "${IDENTITY_TOKEN_AUDIENCE}" ]]; then
    err "`--identity-token-provider` requires an audience"
  fi
  identity_args+=(
    "--identity-token-provider=${IDENTITY_TOKEN_PROVIDER}"
    "--identity-token-audience=${IDENTITY_TOKEN_AUDIENCE}"
  )
elif [[ -n "${SIGSTORE_ID_TOKEN:-}" ]]; then
  identity_args+=( "--identity-token-env=SIGSTORE_ID_TOKEN" )
elif [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  identity_args+=(
    "--identity-token-provider=github-actions"
    "--identity-token-audience=${IDENTITY_TOKEN_AUDIENCE}"
  )
fi

if [[ "${#identity_args[@]}" -eq 0 ]]; then
  err "no Sigstore identity token source configured (set IDENTITY_TOKEN_PROVIDER or SIGSTORE_ID_TOKEN)"
fi

portal_metadata_flags=("${common_metadata_flags[@]}")

log "Packing deterministic CAR and chunk plan"
package_payload \
  "portal" \
  "${ARCHIVE}" \
  "${PORTAL_PREFIX}" \
  "${PIN_LABEL}" \
  "${PRIMARY_ALIAS}" \
  "${ALIAS_NAMESPACE}" \
  "${ALIAS_NAME}" \
  "${ALIAS_PROOF_PATH}" \
  "${SUCCESSOR_OF}" \
  "${portal_metadata_flags[@]}"

portal_submit_performed="${LAST_SUBMISSION_PERFORMED}"

proposal_alias="${PROPOSAL_ALIAS:-${PRIMARY_ALIAS}}"
if [[ -n "${SUBMITTED_EPOCH}" ]]; then
  log "Building governance proposal summary"
  proposal_args=(
    "--manifest=${MANIFEST_OUT}"
    "--chunk-plan=${PLAN_OUT}"
    "--submitted-epoch=${SUBMITTED_EPOCH}"
    "--proposal-out=${PIN_PROPOSAL_PATH}"
  )
  if [[ -n "${SUCCESSOR_OF}" ]]; then
    proposal_args+=( "--successor-of=${SUCCESSOR_OF}" )
  fi
  if [[ -n "${proposal_alias}" ]]; then
    proposal_args+=( "--alias-hint=${proposal_alias}" )
  fi
  cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
    manifest proposal \
    "${proposal_args[@]}"
else
  log "Skipping governance proposal (missing SUBMITTED_EPOCH)"
fi

openapi_metadata_flags=("${common_metadata_flags[@]}")
if [[ "${SKIP_OPENAPI}" == "true" ]]; then
  log "Skipping OpenAPI packaging (--skip-openapi supplied)"
else
  if [[ -d "${OPENAPI_DIR}" ]]; then
    log "Packaging OpenAPI payload from ${OPENAPI_DIR}"
    package_payload \
      "openapi" \
      "${OPENAPI_DIR}" \
      "${OPENAPI_PREFIX}" \
      "${OPENAPI_LABEL}" \
      "${OPENAPI_ALIAS}" \
      "${OPENAPI_ALIAS_NAMESPACE}" \
      "${OPENAPI_ALIAS_NAME}" \
      "${OPENAPI_ALIAS_PROOF_PATH}" \
      "${OPENAPI_SUCCESSOR_OF}" \
      "${openapi_metadata_flags[@]}"
    append_asset_descriptor \
      "${ADDITIONAL_ASSETS_FILE}" \
      "openapi" \
      "${LAST_VERIFY_SUMMARY}" \
      "${LAST_PROOF_SUMMARY}" \
      "${LAST_MANIFEST_BUNDLE}" \
      "${LAST_MANIFEST_SIGNATURE}" \
      "${LAST_SIGN_SUMMARY}" \
      "${LAST_SUBMIT_SUMMARY}" \
      "${LAST_SUBMISSION_PERFORMED}" \
      "${OPENAPI_ALIAS_NAMESPACE}" \
      "${OPENAPI_ALIAS_NAME}" \
      "${OPENAPI_ALIAS_PROOF_PATH}" \
      "${OPENAPI_LABEL}" \
      "${OPENAPI_ALIAS}" \
      "${TORII_URL}" \
      "${SUBMITTED_EPOCH}" \
      "${AUTHORITY}" \
      "${LAST_CAR_PATH}" \
      "${LAST_PLAN_PATH}"
  else
    log "warning: OpenAPI directory '${OPENAPI_DIR}' not found; skipping packaging"
  fi
fi

portal_sbom_metadata_flags=("${common_metadata_flags[@]}")
openapi_sbom_metadata_flags=("${common_metadata_flags[@]}")
if [[ "${SKIP_SBOM}" == "true" ]]; then
  log "Skipping SBOM packaging (--skip-sbom supplied)"
else
  if ! command -v "${SYFT_BIN}" >/dev/null 2>&1; then
    err "syft not found (set SYFT_BIN or rerun with --skip-sbom)"
  fi
  PORTAL_SBOM_JSON="${PORTAL_SBOM_PREFIX}.json"
  OPENAPI_SBOM_JSON="${OPENAPI_SBOM_PREFIX}.json"
  log "Generating portal SBOM via ${SYFT_BIN}"
  "${SYFT_BIN}" "dir:${BUILD_DIR}" -o json >"${PORTAL_SBOM_JSON}"
  if [[ -f "${OPENAPI_SBOM_SOURCE}" ]]; then
    log "Generating OpenAPI SBOM via ${SYFT_BIN}"
    "${SYFT_BIN}" "file:${OPENAPI_SBOM_SOURCE}" -o json >"${OPENAPI_SBOM_JSON}"
  else
    log "warning: OpenAPI SBOM source '${OPENAPI_SBOM_SOURCE}' missing; skipping OpenAPI SBOM packaging"
    OPENAPI_SBOM_JSON=""
  fi

  if [[ -s "${PORTAL_SBOM_JSON}" ]]; then
    package_payload \
      "portal-sbom" \
      "${PORTAL_SBOM_JSON}" \
      "${PORTAL_SBOM_PREFIX}" \
      "${PORTAL_SBOM_LABEL}" \
      "${PORTAL_SBOM_ALIAS}" \
      "${PORTAL_SBOM_ALIAS_NAMESPACE}" \
      "${PORTAL_SBOM_ALIAS_NAME}" \
      "${PORTAL_SBOM_ALIAS_PROOF_PATH}" \
      "" \
      "${portal_sbom_metadata_flags[@]}"
    append_asset_descriptor \
      "${ADDITIONAL_ASSETS_FILE}" \
      "portal-sbom" \
      "${LAST_VERIFY_SUMMARY}" \
      "${LAST_PROOF_SUMMARY}" \
      "${LAST_MANIFEST_BUNDLE}" \
      "${LAST_MANIFEST_SIGNATURE}" \
      "${LAST_SIGN_SUMMARY}" \
      "${LAST_SUBMIT_SUMMARY}" \
      "${LAST_SUBMISSION_PERFORMED}" \
      "${PORTAL_SBOM_ALIAS_NAMESPACE}" \
      "${PORTAL_SBOM_ALIAS_NAME}" \
      "${PORTAL_SBOM_ALIAS_PROOF_PATH}" \
      "${PORTAL_SBOM_LABEL}" \
      "${PORTAL_SBOM_ALIAS}" \
      "${TORII_URL}" \
      "${SUBMITTED_EPOCH}" \
      "${AUTHORITY}" \
      "${LAST_CAR_PATH}" \
      "${LAST_PLAN_PATH}"
  else
    log "warning: portal SBOM JSON '${PORTAL_SBOM_JSON}' was not generated; skipping packaging"
  fi

  if [[ -n "${OPENAPI_SBOM_JSON}" && -s "${OPENAPI_SBOM_JSON}" ]]; then
    package_payload \
      "openapi-sbom" \
      "${OPENAPI_SBOM_JSON}" \
      "${OPENAPI_SBOM_PREFIX}" \
      "${OPENAPI_SBOM_LABEL}" \
      "${OPENAPI_SBOM_ALIAS}" \
      "${OPENAPI_SBOM_ALIAS_NAMESPACE}" \
      "${OPENAPI_SBOM_ALIAS_NAME}" \
      "${OPENAPI_SBOM_ALIAS_PROOF_PATH}" \
      "" \
      "${openapi_sbom_metadata_flags[@]}"
    append_asset_descriptor \
      "${ADDITIONAL_ASSETS_FILE}" \
      "openapi-sbom" \
      "${LAST_VERIFY_SUMMARY}" \
      "${LAST_PROOF_SUMMARY}" \
      "${LAST_MANIFEST_BUNDLE}" \
      "${LAST_MANIFEST_SIGNATURE}" \
      "${LAST_SIGN_SUMMARY}" \
      "${LAST_SUBMIT_SUMMARY}" \
      "${LAST_SUBMISSION_PERFORMED}" \
      "${OPENAPI_SBOM_ALIAS_NAMESPACE}" \
      "${OPENAPI_SBOM_ALIAS_NAME}" \
      "${OPENAPI_SBOM_ALIAS_PROOF_PATH}" \
      "${OPENAPI_SBOM_LABEL}" \
      "${OPENAPI_SBOM_ALIAS}" \
      "${TORII_URL}" \
      "${SUBMITTED_EPOCH}" \
      "${AUTHORITY}" \
      "${LAST_CAR_PATH}" \
      "${LAST_PLAN_PATH}"
  fi
fi

release_tag="${DOCS_RELEASE_TAG:-}"
release_source="${DOCS_RELEASE_SOURCE:-}"

node - \
  "${VERIFY_SUMMARY}" \
  "${PROOF_SUMMARY}" \
  "${PIN_REPORT}" \
  "${SUBMIT_SUMMARY}" \
  "${PIN_PROPOSAL_PATH}" \
  "${portal_submit_performed}" \
  "${BUILD_DIR}" \
  "${PLAN_OUT}" \
  "${BUNDLE_OUT}" \
  "${SIGN_SUMMARY}" \
  "${SIGNATURE_OUT}" \
  "${PRIMARY_ALIAS}" \
  "${PROPOSAL_ALIAS}" \
  "${ALIAS_NAMESPACE}" \
  "${ALIAS_NAME}" \
  "${ALIAS_PROOF_PATH}" \
  "${release_tag}" \
  "${release_source}" \
  "${TORII_URL}" \
  "${SUBMITTED_EPOCH}" \
  "${AUTHORITY}" \
  "${GATEWAY_BINDING_JSON}" \
  "${GATEWAY_HEADERS}" \
  "${ADDITIONAL_ASSETS_FILE}" <<'NODE'
const fs = require('node:fs');

const [
  ,
  ,
  verifyPath,
  proofPath,
  reportPath,
  submitSummaryPath,
  pinProposalPath,
  submitPerformedRaw,
  buildDir,
  planPath,
  bundlePath,
  signSummaryPath,
  signaturePath,
  aliasLabel,
  proposalAlias,
  aliasNamespace,
  aliasName,
  aliasProofPath,
  releaseTag,
  releaseSource,
  toriiUrl,
  submittedEpoch,
  authority,
  gatewayBindingPath,
  gatewayHeaderPath,
  additionalAssetsPath
] = process.argv;
const submitPerformed = submitPerformedRaw === 'true';
function readJsonOrNull(p) {
  if (!p || p === 'null') return null;
  if (!fs.existsSync(p)) return null;
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}
function readTextOrNull(p) {
  if (!p || p === 'null') return null;
  if (!fs.existsSync(p)) return null;
  return fs.readFileSync(p, 'utf8');
}
const verify = readJsonOrNull(verifyPath);
const proof = readJsonOrNull(proofPath);
if (!verify || !proof) {
  console.error('failed to read verification summaries');
  process.exit(1);
}
const submit = readJsonOrNull(submitSummaryPath);
const manifestInfo = {
  path: verify.manifest_path,
  blake3_hex: verify.manifest_blake3_hex,
  chunk_digest_sha3_hex: verify.chunk_digest_sha3_256_hex || null,
  chunk_plan_chunk_count: verify.chunk_plan_chunk_count || null,
  chunk_plan_source: verify.chunk_plan_source || null
};
const carInfo = {
  path: proof.car_path,
  size_bytes: proof.car_size,
  chunk_count: proof.chunk_count,
  payload_bytes: proof.payload_bytes,
  payload_digest_hex: proof.payload_digest_hex,
  car_digest_hex: proof.car_digest_hex,
  chunker_handle: proof.chunker_handle
};
const signingInfo = {
  bundle_path: bundlePath,
  signature_path: signaturePath,
  sign_summary_path: signSummaryPath,
  verify_summary_path: verifyPath
};
const bindingJson = readJsonOrNull(gatewayBindingPath);
const bindingHeaders = readTextOrNull(gatewayHeaderPath);
const gatewayBinding = bindingJson
  ? {
      json_path: gatewayBindingPath,
      headers_path: gatewayHeaderPath,
      headers_text: bindingHeaders,
      alias: bindingJson.alias || null,
      hostname: bindingJson.hostname || null,
      content_cid: bindingJson.content_cid || null,
      proof_status: bindingJson.proof_status || null,
      generated_at: bindingJson.generated_at || null,
      headers: bindingJson.headers || null
    }
  : null;
const submissionInfo = submit ? {
  summary_path: submitSummaryPath,
  torii_url: submit.torii_url,
  submitted_epoch: submit.submitted_epoch,
  authority: submit.authority,
  manifest_digest_hex: submit.manifest_digest_hex,
  alias_namespace: submit.alias_namespace || null,
  alias_name: submit.alias_name || null
} : null;
const pinProposal = readJsonOrNull(pinProposalPath);
const additionalAssets = [];
const additionalAssetDescriptors = readJsonOrNull(additionalAssetsPath);
if (Array.isArray(additionalAssetDescriptors)) {
  for (const descriptor of additionalAssetDescriptors) {
    const assetVerify = readJsonOrNull(descriptor.verify_summary);
    const assetProof = readJsonOrNull(descriptor.proof_summary);
    if (!assetVerify || !assetProof) {
      console.warn(`skipping asset ${descriptor.label} due to missing summaries`);
      continue;
    }
    additionalAssets.push({
      label: descriptor.label,
      metadata_label: descriptor.metadata_label || null,
      alias_label: descriptor.alias_label || null,
      manifest: {
        path: assetVerify.manifest_path,
        blake3_hex: assetVerify.manifest_blake3_hex,
        chunk_digest_sha3_hex: assetVerify.chunk_digest_sha3_256_hex || null,
        chunk_plan_chunk_count: assetVerify.chunk_plan_chunk_count || null,
        chunk_plan_source: assetVerify.chunk_plan_source || descriptor.plan_path || null
      },
      car: {
        path: descriptor.car_path || assetProof.car_path,
        size_bytes: assetProof.car_size,
        chunk_count: assetProof.chunk_count,
        payload_bytes: assetProof.payload_bytes,
        payload_digest_hex: assetProof.payload_digest_hex,
        car_digest_hex: assetProof.car_digest_hex,
        chunker_handle: assetProof.chunker_handle,
        pin_policy: assetProof.pin_policy || null
      },
      signing: {
        bundle_path: descriptor.bundle_path || null,
        signature_path: descriptor.signature_path || null,
        sign_summary_path: descriptor.sign_summary || null,
        verify_summary_path: descriptor.verify_summary || null
      },
      submission: descriptor.submission_performed
        ? {
            summary_path: descriptor.submit_summary || null,
            torii_url: descriptor.torii_url || toriiUrl,
            submitted_epoch: descriptor.submitted_epoch || submittedEpoch,
            authority: descriptor.authority || authority
          }
        : null,
      alias_binding: descriptor.submission_performed
        ? {
            namespace: descriptor.alias_namespace || null,
            name: descriptor.alias_name || null,
            proof_path: descriptor.alias_proof_path || null
          }
        : null
    });
  }
}
const report = {
  build_dir: buildDir,
  plan_path: planPath,
  release_tag: releaseTag || null,
  release_source: releaseSource || null,
  chunker_handle: proof.chunker_handle,
  alias_label: aliasLabel || null,
  manifest: manifestInfo,
  car: carInfo,
  pin_policy: proof.pin_policy || null,
  signing: signingInfo,
  submission: submitPerformed ? submissionInfo : null,
  alias_binding: submitPerformed ? {
    namespace: aliasNamespace || submit?.alias_namespace || null,
    name: aliasName || submit?.alias_name || null,
    proof_path: aliasProofPath || null
  } : null,
  proposal: pinProposal,
  gateway_binding: gatewayBinding,
  additional_assets: additionalAssets
};
fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
NODE

GATEWAY_BINDING_JSON="${SORA_DIR}/portal.gateway.binding.json"
GATEWAY_HEADERS="${SORA_DIR}/portal.gateway.headers.txt"
binding_args=(
  "--manifest" "${MANIFEST_JSON}"
  "--json-out" "${GATEWAY_BINDING_JSON}"
  "--headers-out" "${GATEWAY_HEADERS}"
)
if [[ -n "${PRIMARY_ALIAS}" ]]; then
  binding_args+=( "--alias" "${PRIMARY_ALIAS}" )
fi
if [[ -n "${DNS_HOSTNAME}" ]]; then
  binding_args+=( "--hostname" "${DNS_HOSTNAME}" )
fi
if [[ -n "${PROPOSAL_ALIAS}" ]]; then
  binding_args+=( "--route-label" "${PROPOSAL_ALIAS}" )
fi
if [[ -n "${ROUTE_PROOF_STATUS}" ]]; then
  binding_args+=( "--proof-status" "${ROUTE_PROOF_STATUS}" )
fi
log "Rendering gateway header template (${GATEWAY_HEADERS})"
if [[ -z "${PRIMARY_ALIAS}" || -z "${DNS_HOSTNAME}" ]]; then
  log "warning: skipped gateway binding headers (missing --alias or --dns-hostname)"
elif ! cargo xtask soradns-binding-template "${binding_args[@]}"; then
  log "warning: failed to generate gateway binding headers"
fi

if [[ -f "${GATEWAY_BINDING_JSON}" ]]; then
  log "Verifying gateway binding (${GATEWAY_BINDING_JSON})"
  verify_args=(
    "--binding=${GATEWAY_BINDING_JSON}"
    "--manifest-json=${MANIFEST_JSON}"
  )
  if [[ -n "${PRIMARY_ALIAS}" ]]; then
    verify_args+=( "--alias=${PRIMARY_ALIAS}" )
  fi
  if [[ -n "${DNS_HOSTNAME}" ]]; then
    verify_args+=( "--hostname=${DNS_HOSTNAME}" )
  fi
  if [[ -n "${ROUTE_PROOF_STATUS}" ]]; then
    verify_args+=( "--proof-status=${ROUTE_PROOF_STATUS}" )
  fi
  cargo xtask soradns-verify-binding "${verify_args[@]}"
else
  log "warning: skipped gateway binding verification (missing ${GATEWAY_BINDING_JSON})"
fi

if [[ -n "${DNS_HOSTNAME}" ]]; then
  route_args=(
    route
    plan
    "--manifest-json=${MANIFEST_JSON}"
    "--out=${ROUTE_PLAN_JSON}"
    "--headers-out=${ROUTE_HEADERS}"
    "--hostname=${DNS_HOSTNAME}"
    "--proof-status=${ROUTE_PROOF_STATUS}"
  )
  if [[ -n "${PRIMARY_ALIAS}" ]]; then
    route_args+=( "--alias=${PRIMARY_ALIAS}" )
  fi
  if [[ -n "${PROPOSAL_ALIAS}" ]]; then
    route_args+=( "--route-label=${PROPOSAL_ALIAS}" )
  fi
  if [[ -n "${DOCS_RELEASE_TAG:-}" ]]; then
    route_args+=( "--release-tag=${DOCS_RELEASE_TAG}" )
  fi
  if [[ -n "${DNS_CUTOVER_WINDOW}" ]]; then
    route_args+=( "--cutover-window=${DNS_CUTOVER_WINDOW}" )
  fi
  if [[ -n "${ROUTE_ROLLBACK_MANIFEST_JSON}" ]]; then
    route_args+=( "--rollback-manifest-json=${ROUTE_ROLLBACK_MANIFEST_JSON}" )
    route_args+=( "--rollback-headers-out=${ROUTE_ROLLBACK_HEADERS}" )
    if [[ -n "${ROUTE_ROLLBACK_LABEL}" ]]; then
      route_args+=( "--rollback-route-label=${ROUTE_ROLLBACK_LABEL}" )
    fi
    if [[ -n "${ROUTE_ROLLBACK_RELEASE_TAG}" ]]; then
      route_args+=( "--rollback-release-tag=${ROUTE_ROLLBACK_RELEASE_TAG}" )
    fi
  fi
  log "Rendering route plan (${ROUTE_PLAN_JSON})"
  if ! scripts/sorafs-gateway "${route_args[@]}"; then
    log "warning: failed to generate route plan (gateway.route_plan.json)"
  elif [[ -f "${ROUTE_PLAN_JSON}" ]]; then
    log "Annotating pin report with route plan (${ROUTE_PLAN_JSON})"
    node - <<'NODE' \
      "${PIN_REPORT}" \
      "${ROUTE_PLAN_JSON}" \
      "${ROUTE_HEADERS}" \
      "${ROUTE_ROLLBACK_HEADERS}"
const fs = require('node:fs');

const [
  ,
  ,
  reportPath,
  routePlanPath,
  headersPath,
  rollbackHeadersPath
] = process.argv;

try {
  const report = JSON.parse(fs.readFileSync(reportPath, 'utf8'));
  const routePlan = JSON.parse(fs.readFileSync(routePlanPath, 'utf8'));
  report.route_plan = {
    path: routePlanPath,
    headers_path: fs.existsSync(headersPath) ? headersPath : null,
    rollback_headers_path: fs.existsSync(rollbackHeadersPath)
      ? rollbackHeadersPath
      : null,
    alias: routePlan.alias ?? null,
    hostname: routePlan.hostname ?? null,
    content_cid: routePlan.content_cid ?? null,
    route_binding: routePlan.route_binding ?? null,
    headers: routePlan.headers ?? null,
    headers_template: routePlan.headers_template ?? null,
    generated_at: routePlan.generated_at ?? null
  };
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
} catch (error) {
  console.error(`[route-plan] ${error.message}`);
  process.exit(1);
}
NODE
  else
    log "warning: generated route plan not found; pin report was not annotated"
  fi
else
  log "Skipping route plan (DNS hostname not supplied)"
fi

log "Pinned artefacts written to ${SORA_DIR}"
log "Report: ${PIN_REPORT}"
if ${portal_submit_performed}; then
  log "Generating DNS cutover descriptor (${CUTOVER_PLAN})"
  cutover_args=( "--pin-report=${PIN_REPORT}" "--out=${CUTOVER_PLAN}" )
  if [[ -n "${DNS_CHANGE_TICKET}" ]]; then
    cutover_args+=( "--change-ticket=${DNS_CHANGE_TICKET}" )
  fi
  if [[ -n "${DNS_CUTOVER_WINDOW}" ]]; then
    cutover_args+=( "--cutover-window=${DNS_CUTOVER_WINDOW}" )
  fi
  if [[ -n "${DNS_HOSTNAME}" ]]; then
    cutover_args+=( "--dns-hostname=${DNS_HOSTNAME}" )
  fi
  if [[ -n "${DNS_ZONE}" ]]; then
    cutover_args+=( "--dns-zone=${DNS_ZONE}" )
  fi
  if [[ -n "${DNS_OPS_CONTACT}" ]]; then
    cutover_args+=( "--ops-contact=${DNS_OPS_CONTACT}" )
  fi
  if [[ -n "${DNS_CACHE_PURGE_ENDPOINT}" ]]; then
    cutover_args+=( "--cache-purge-endpoint=${DNS_CACHE_PURGE_ENDPOINT}" )
  fi
  if [[ -n "${DNS_CACHE_PURGE_AUTH_ENV}" ]]; then
    cutover_args+=( "--cache-purge-auth-env=${DNS_CACHE_PURGE_AUTH_ENV}" )
  fi
  if [[ -n "${DNS_PREVIOUS_PLAN}" ]]; then
    cutover_args+=( "--previous-plan=${DNS_PREVIOUS_PLAN}" )
  fi
  node scripts/generate-dns-cutover-plan.mjs "${cutover_args[@]}"
  log "DNS cutover plan: ${CUTOVER_PLAN}"
  generate_zonefile_bundle "${CUTOVER_PLAN}"
else
  log "Skipping DNS cutover descriptor because manifest submission/alias binding was not performed"
  if [[ -n "${DNS_ZONEFILE_OUT}" || -n "${DNS_ZONEFILE_RESOLVER_SNIPPET}" ]]; then
    log "Skipping zonefile skeleton because DNS cutover descriptor was not generated"
  fi
fi
