#!/usr/bin/env bash
set -euo pipefail

abs_path() {
  local input="$1"
  if [[ "$input" = /* ]]; then
    printf '%s\n' "$input"
  else
    local dir
    dir="$(cd "$(dirname "$input")" && pwd)"
    printf '%s/%s\n' "$dir" "$(basename "$input")"
  fi
}

usage() {
  cat <<'USAGE'
sorafs_gateway_self_cert.sh --signing-key <path> --signer <account> [options]

Runs the SoraFS gateway self-certification harness (cargo xtask sorafs-gateway-attest),
producing a signed attestation envelope, JSON report, and human-readable summary.
When provided with manifest artefacts, the script also runs
`sorafs_cli manifest verify-signature` to ensure release bundles remain valid.

Parameters may be supplied directly via flags or through a key=value config file.
Command-line flags override config entries.

Required (flag or config):
  signing_key=<path>     Ed25519 private key (hex, no prefix) used to sign the attestation.
  signer=<account>       Account ID recorded in the attestation (e.g., admin@org).

Optional:
  --config <path>                 Config file with key=value pairs.
  --gateway <url>                 Gateway base URL (defaults to harness fixture target).
  --out <dir>                     Output directory (default: <workspace>/artifacts/sorafs_gateway_attest).
  --workspace <path>              Repository root (default: current directory).
  --cli <path>                    Prebuilt sorafs_cli binary for manifest verification.
  --manifest <path>               Manifest (.to) to verify after the harness completes.
  --manifest-bundle <path>        Signature bundle JSON to verify.
  --manifest-signature <path>     Detached signature to verify (requires --public-key-hex).
  --public-key-hex <hex>          Public key for detached signature verification.
  --chunk-plan <path>             Chunk plan JSON associated with the manifest.
  --chunk-summary <path>          Chunk summary JSON associated with the manifest.
  --chunk-digest-sha3 <hex>       Expected chunk digest to enforce during verification.
  --expect-token-hash <hex>       Expected BLAKE3 token hash (from signing summary).
  --denylist-old <bundle.json>    Previously published denylist bundle for diff evidence.
  --denylist-new <bundle.json>    Newly generated denylist bundle for diff evidence.
  --denylist-report <path>        Override path for diff JSON (default: <out>/denylist_diff.json).
  --help                          Show this help message and exit.
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

workspace=""
signing_key=""
signer_account=""
gateway_target=""
output_dir=""
cli_path=""
manifest_path=""
bundle_path=""
signature_path=""
public_key_hex=""
chunk_plan_path=""
chunk_summary_path=""
chunk_digest_hex=""
expect_token_hash=""
config_path=""
denylist_old_bundle=""
denylist_new_bundle=""
denylist_report=""

cli_workspace=""
cli_signing_key=""
cli_signer=""
cli_gateway=""
cli_out=""
cli_cli_path=""
cli_manifest=""
cli_bundle=""
cli_signature=""
cli_public_key=""
cli_chunk_plan=""
cli_chunk_summary=""
cli_chunk_digest=""
cli_expect_token_hash=""
cli_denylist_old=""
cli_denylist_new=""
cli_denylist_report=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      cli_workspace="$(cd "$2" && pwd)"
      workspace="$cli_workspace"
      shift 2
      ;;
    --config)
      config_path="$(abs_path "$2")"
      shift 2
      ;;
    --signing-key)
      cli_signing_key="$(abs_path "$2")"
      signing_key="$cli_signing_key"
      shift 2
      ;;
    --signer)
      cli_signer="$2"
      signer_account="$cli_signer"
      shift 2
      ;;
    --gateway)
      cli_gateway="$2"
      gateway_target="$cli_gateway"
      shift 2
      ;;
    --out)
      cli_out="$2"
      output_dir="$cli_out"
      shift 2
      ;;
    --cli)
      cli_cli_path="$(abs_path "$2")"
      cli_path="$cli_cli_path"
      shift 2
      ;;
    --manifest)
      cli_manifest="$(abs_path "$2")"
      manifest_path="$cli_manifest"
      shift 2
      ;;
    --manifest-bundle)
      cli_bundle="$(abs_path "$2")"
      bundle_path="$cli_bundle"
      shift 2
      ;;
    --manifest-signature)
      cli_signature="$(abs_path "$2")"
      signature_path="$cli_signature"
      shift 2
      ;;
    --public-key-hex)
      cli_public_key="$2"
      public_key_hex="$cli_public_key"
      shift 2
      ;;
    --chunk-plan)
      cli_chunk_plan="$(abs_path "$2")"
      chunk_plan_path="$cli_chunk_plan"
      shift 2
      ;;
    --chunk-summary)
      cli_chunk_summary="$(abs_path "$2")"
      chunk_summary_path="$cli_chunk_summary"
      shift 2
      ;;
    --chunk-digest-sha3)
      cli_chunk_digest="$2"
      chunk_digest_hex="$cli_chunk_digest"
      shift 2
      ;;
    --expect-token-hash)
      cli_expect_token_hash="$2"
      expect_token_hash="$cli_expect_token_hash"
      shift 2
      ;;
    --denylist-old)
      cli_denylist_old="$(abs_path "$2")"
      denylist_old_bundle="$cli_denylist_old"
      shift 2
      ;;
    --denylist-new)
      cli_denylist_new="$(abs_path "$2")"
      denylist_new_bundle="$cli_denylist_new"
      shift 2
      ;;
    --denylist-report)
      cli_denylist_report="$(abs_path "$2")"
      denylist_report="$cli_denylist_report"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "${config_path}" ]]; then
  if [[ ! -f "${config_path}" ]]; then
    echo "error: config file not found at ${config_path}" >&2
    exit 1
  fi
  while IFS='=' read -r raw_key raw_value; do
    [[ -z "${raw_key// }" || "${raw_key}" =~ ^# ]] && continue
    key="$(echo "$raw_key" | awk '{$1=$1;print}')"
    value="$(echo "$raw_value" | awk '{$1=$1;print}')"
    case "$key" in
      signing_key) [[ -z "$signing_key" ]] && signing_key="$(abs_path "$value")" ;;
      signer) [[ -z "$signer_account" ]] && signer_account="$value" ;;
      gateway) [[ -z "$gateway_target" ]] && gateway_target="$value" ;;
      out) [[ -z "$output_dir" ]] && output_dir="$value" ;;
      cli) [[ -z "$cli_path" ]] && cli_path="$(abs_path "$value")" ;;
      manifest) [[ -z "$manifest_path" ]] && manifest_path="$(abs_path "$value")" ;;
      manifest_bundle) [[ -z "$bundle_path" ]] && bundle_path="$(abs_path "$value")" ;;
      manifest_signature) [[ -z "$signature_path" ]] && signature_path="$(abs_path "$value")" ;;
      public_key_hex) [[ -z "$public_key_hex" ]] && public_key_hex="$value" ;;
      chunk_plan) [[ -z "$chunk_plan_path" ]] && chunk_plan_path="$(abs_path "$value")" ;;
      chunk_summary) [[ -z "$chunk_summary_path" ]] && chunk_summary_path="$(abs_path "$value")" ;;
      chunk_digest_sha3) [[ -z "$chunk_digest_hex" ]] && chunk_digest_hex="$value" ;;
      expect_token_hash) [[ -z "$expect_token_hash" ]] && expect_token_hash="$value" ;;
      denylist_old_bundle) [[ -z "$denylist_old_bundle" ]] && denylist_old_bundle="$(abs_path "$value")" ;;
      denylist_new_bundle) [[ -z "$denylist_new_bundle" ]] && denylist_new_bundle="$(abs_path "$value")" ;;
      denylist_report) [[ -z "$denylist_report" ]] && denylist_report="$(abs_path "$value")" ;;
      workspace) [[ -z "$workspace" ]] && workspace="$(cd "$value" && pwd)" ;;
      *)
        echo "warning: unknown config key '${key}' in ${config_path}" >&2
        ;;
    esac
  done < "${config_path}"
fi

[[ -n "$cli_workspace" ]] && workspace="$cli_workspace"
[[ -n "$cli_signing_key" ]] && signing_key="$cli_signing_key"
[[ -n "$cli_signer" ]] && signer_account="$cli_signer"
[[ -n "$cli_gateway" ]] && gateway_target="$cli_gateway"
[[ -n "$cli_out" ]] && output_dir="$cli_out"
[[ -n "$cli_cli_path" ]] && cli_path="$cli_cli_path"
[[ -n "$cli_manifest" ]] && manifest_path="$cli_manifest"
[[ -n "$cli_bundle" ]] && bundle_path="$cli_bundle"
[[ -n "$cli_signature" ]] && signature_path="$cli_signature"
[[ -n "$cli_public_key" ]] && public_key_hex="$cli_public_key"
[[ -n "$cli_chunk_plan" ]] && chunk_plan_path="$cli_chunk_plan"
[[ -n "$cli_chunk_summary" ]] && chunk_summary_path="$cli_chunk_summary"
[[ -n "$cli_chunk_digest" ]] && chunk_digest_hex="$cli_chunk_digest"
[[ -n "$cli_expect_token_hash" ]] && expect_token_hash="$cli_expect_token_hash"
[[ -n "$cli_denylist_old" ]] && denylist_old_bundle="$cli_denylist_old"
[[ -n "$cli_denylist_new" ]] && denylist_new_bundle="$cli_denylist_new"
[[ -n "$cli_denylist_report" ]] && denylist_report="$cli_denylist_report"

if [[ -z "$workspace" ]]; then
  workspace="${PWD}"
fi

sample_fixture_dir="$(abs_path "fixtures/sorafs_manifest/ci_sample")"
if [[ -z "$signing_key" ]]; then
  signing_key="${sample_fixture_dir}/gateway_attestor.hex"
fi
if [[ -z "$signer_account" ]]; then
  signer_account="admin@operator"
fi
[[ -z "$chunk_plan_path" ]] && chunk_plan_path="${sample_fixture_dir}/chunk_plan.json"
[[ -z "$chunk_summary_path" ]] && chunk_summary_path="${sample_fixture_dir}/car_summary.json"
if [[ -z "$bundle_path" && -f "${sample_fixture_dir}/manifest.bundle.json" ]]; then
  bundle_path="${sample_fixture_dir}/manifest.bundle.json"
fi
if [[ -z "$manifest_path" && -f "${sample_fixture_dir}/manifest.to" ]]; then
  manifest_path="${sample_fixture_dir}/manifest.to"
fi
[[ -z "$chunk_digest_hex" ]] && chunk_digest_hex="0236c64b664bfff906f32106789baa61e7d4d9fdb4514e173b77a77d2883946e"
[[ -z "$expect_token_hash" ]] && expect_token_hash="7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b"

if [[ -z "${signing_key}" || -z "${signer_account}" ]]; then
  echo "error: --signing-key and --signer are required" >&2
  exit 1
fi

if [[ -z "${output_dir}" ]]; then
  output_dir="${workspace}/artifacts/sorafs_gateway_attest"
else
  output_dir="$(abs_path "${output_dir}")"
fi

cmd=(cargo xtask sorafs-gateway-attest --signing-key "${signing_key}" --signer-account "${signer_account}" --out "${output_dir}")
if [[ -n "${gateway_target}" ]]; then
  cmd+=(--gateway "${gateway_target}")
fi

(
  cd "${workspace}"
  "${cmd[@]}"
)

if [[ -n "${manifest_path}" && ( -n "${bundle_path}" || ( -n "${signature_path}" && -n "${public_key_hex}" )) ]]; then
  if [[ -n "${signature_path}" && -z "${public_key_hex}" ]]; then
    echo "error: --manifest-signature requires --public-key-hex" >&2
    exit 1
  fi

  if [[ -n "${signature_path}" && ! -f "${signature_path}" ]]; then
    echo "error: signature not found at ${signature_path}" >&2
    exit 1
  fi

  if [[ -n "${manifest_path}" && ! -f "${manifest_path}" ]]; then
    echo "error: manifest not found at ${manifest_path}" >&2
    exit 1
  fi
  if [[ -n "${bundle_path}" && ! -f "${bundle_path}" ]]; then
    echo "error: bundle not found at ${bundle_path}" >&2
    exit 1
  fi
  if [[ -n "${chunk_plan_path}" && ! -f "${chunk_plan_path}" ]]; then
    echo "error: chunk plan not found at ${chunk_plan_path}" >&2
    exit 1
  fi
  if [[ -n "${chunk_summary_path}" && ! -f "${chunk_summary_path}" ]]; then
    echo "error: chunk summary not found at ${chunk_summary_path}" >&2
    exit 1
  fi

  if [[ -z "${cli_path}" ]]; then
    cli_path="${workspace}/target/release/sorafs_cli"
    if [[ ! -x "${cli_path}" ]]; then
      echo "Building sorafs_cli (release)..."
      (cd "${workspace}" && cargo build -p sorafs_car --features cli --bin sorafs_cli --release)
    fi
  fi
  if [[ ! -x "${cli_path}" ]]; then
    echo "error: sorafs_cli binary not executable at ${cli_path}" >&2
    exit 1
  fi

  verify_cmd=("${cli_path}" manifest verify-signature --manifest "${manifest_path}")
  if [[ -n "${bundle_path}" ]]; then verify_cmd+=(--bundle "${bundle_path}"); fi
  if [[ -n "${signature_path}" ]]; then verify_cmd+=(--signature "${signature_path}"); fi
  if [[ -n "${public_key_hex}" ]]; then verify_cmd+=(--public-key-hex "${public_key_hex}"); fi
  if [[ -n "${chunk_plan_path}" ]]; then verify_cmd+=(--chunk-plan "${chunk_plan_path}"); fi
  if [[ -n "${chunk_summary_path}" ]]; then verify_cmd+=(--summary "${chunk_summary_path}"); fi
  if [[ -n "${chunk_digest_hex}" ]]; then verify_cmd+=(--chunk-digest-sha3 "${chunk_digest_hex}"); fi
  if [[ -n "${expect_token_hash}" ]]; then verify_cmd+=(--expect-token-hash "${expect_token_hash}"); fi

  verify_summary_path="${output_dir}/manifest.verify.summary.json"
  mkdir -p "${output_dir}"

  echo "Verifying manifest signature with sorafs_cli..."
  (
    cd "${workspace}"
    "${verify_cmd[@]}"
  ) | tee "${verify_summary_path}"
fi

if [[ -n "${denylist_old_bundle}" || -n "${denylist_new_bundle}" ]]; then
  if [[ -z "${denylist_old_bundle}" || -z "${denylist_new_bundle}" ]]; then
    echo "warning: both --denylist-old and --denylist-new are required to run the diff. Skipping." >&2
  else
    if [[ ! -f "${denylist_old_bundle}" ]]; then
      echo "error: denylist old bundle not found at ${denylist_old_bundle}" >&2
      exit 1
    fi
    if [[ ! -f "${denylist_new_bundle}" ]]; then
      echo "error: denylist new bundle not found at ${denylist_new_bundle}" >&2
      exit 1
    fi
    diff_report_path="${denylist_report}"
    if [[ -z "${diff_report_path}" ]]; then
      diff_report_path="${output_dir}/denylist_diff.json"
    fi
    mkdir -p "$(dirname "${diff_report_path}")"
    echo "Generating denylist diff evidence..."
    (
      cd "${workspace}"
      cargo xtask sorafs-gateway denylist diff \
        --old "${denylist_old_bundle}" \
        --new "${denylist_new_bundle}" \
        --report-json "${diff_report_path}"
    )
    echo "Denylist diff report written to ${diff_report_path}"
  fi
fi
