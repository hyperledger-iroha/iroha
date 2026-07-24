#!/usr/bin/env bash
set -euo pipefail
umask 077

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

abs_output_path() {
  local input="$1"
  if [[ "$input" = /* ]]; then
    printf '%s\n' "$input"
  else
    printf '%s/%s\n' "$(pwd -P)" "$input"
  fi
}

reject_symlinked_output_parent() {
  local label="$1"
  local target="$2"
  local parent
  parent="$(dirname "$target")"
  local current="/"
  local rest="${parent#/}"
  local component
  IFS='/' read -r -a components <<< "$rest"
  for component in "${components[@]}"; do
    [[ -z "$component" || "$component" == "." ]] && continue
    if [[ "$component" == ".." ]]; then
      echo "error: ${label} parent must not contain parent-directory segments" >&2
      exit 1
    fi
    if [[ "$current" == "/" ]]; then
      current="/${component}"
    else
      current="${current}/${component}"
    fi
    if [[ -L "$current" ]]; then
      echo "error: ${label} parent must not be a symlink: ${current}" >&2
      exit 1
    fi
    if [[ -e "$current" && ! -d "$current" ]]; then
      echo "error: ${label} parent component must be a directory: ${current}" >&2
      exit 1
    fi
    if [[ ! -e "$current" ]]; then
      break
    fi
  done
}

validate_output_dir_path() {
  local label="$1"
  local target="$2"
  if [[ -z "$target" ]]; then
    echo "error: ${label} path must not be empty" >&2
    exit 1
  fi
  if [[ -L "$target" ]]; then
    echo "error: ${label} must not be a symlink: ${target}" >&2
    exit 1
  fi
  if [[ -e "$target" && ! -d "$target" ]]; then
    echo "error: ${label} must be a directory path: ${target}" >&2
    exit 1
  fi
  reject_symlinked_output_parent "$label" "$target"
}

prepare_output_dir_path() {
  local label="$1"
  local target="$2"
  validate_output_dir_path "$label" "$target"
  mkdir -p "$target"
  validate_output_dir_path "$label" "$target"
}

validate_output_file_path() {
  local label="$1"
  local target="$2"
  if [[ -z "$target" ]]; then
    echo "error: ${label} path must not be empty" >&2
    exit 1
  fi
  if [[ -L "$target" ]]; then
    echo "error: ${label} must not be a symlink: ${target}" >&2
    exit 1
  fi
  if [[ -e "$target" && ! -f "$target" ]]; then
    echo "error: ${label} must be a regular file path: ${target}" >&2
    exit 1
  fi
  reject_symlinked_output_parent "$label" "$target"
}

prepare_output_file_path() {
  local label="$1"
  local target="$2"
  validate_output_file_path "$label" "$target"
  mkdir -p "$(dirname "$target")"
  validate_output_file_path "$label" "$target"
}

prepare_new_output_file_path() {
  local label="$1"
  local target="$2"
  validate_output_file_path "$label" "$target"
  if [[ -e "$target" ]]; then
    echo "error: ${label} must not already exist: ${target}" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$target")"
  validate_output_file_path "$label" "$target"
  if [[ -e "$target" || -L "$target" ]]; then
    echo "error: ${label} must not already exist: ${target}" >&2
    exit 1
  fi
}

validate_existing_file_path() {
  local label="$1"
  local target="$2"
  if [[ -z "$target" ]]; then
    echo "error: ${label} path must not be empty" >&2
    exit 1
  fi
  if [[ -L "$target" ]]; then
    echo "error: ${label} must not be a symlink: ${target}" >&2
    exit 1
  fi
  reject_symlinked_output_parent "$label" "$target"
  if [[ ! -e "$target" ]]; then
    echo "error: ${label} not found at ${target}" >&2
    exit 1
  fi
  if [[ ! -f "$target" ]]; then
    echo "error: ${label} must be a regular file: ${target}" >&2
    exit 1
  fi
}

validate_existing_executable_file_path() {
  local label="$1"
  local target="$2"
  validate_existing_file_path "$label" "$target"
  if [[ ! -x "$target" ]]; then
    echo "error: ${label} not executable at ${target}" >&2
    exit 1
  fi
}

require_option_value() {
  local option="$1"
  local value="${2-}"
  if [[ -z "$value" || "$value" == --* ]]; then
    echo "error: ${option} requires a value" >&2
    exit 1
  fi
}

require_sha256() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9a-f]{64}$ ]]; then
    echo "error: ${label} must be exactly 64 lowercase hexadecimal characters" >&2
    exit 1
  fi
}

usage() {
  cat <<'USAGE'
sorafs_gateway_self_cert.sh [required options] [optional options]

Runs the SoraFS gateway self-certification harness (the xtask
sorafs-gateway-attest command), producing a signed attestation envelope, JSON
report, and human-readable summary.
Before the harness starts, the script verifies the canonical aggregate release
manifest with its governed raw Ed25519 key and signature through a SHA256-pinned
`sorafs-validate release-manifest` binary.

Parameters may be supplied directly via flags or through a key=value config file.
Command-line flags override config entries.

Required (flag or config):
  signing_key=<path>
      Runtime Ed25519 private key used to sign the gateway attestation.
  signer=<account>
      Account ID recorded in the attestation (e.g., admin@org).
  gateway=<url>
      Explicit regional gateway base URL. No fixture/default target is used.
  release_manifest=<path>
      Canonical aggregate release manifest JSON.
  release_manifest_signature=<path>
      Exactly 64 raw Ed25519 signature bytes.
  release_manifest_public_key=<path>
      Exactly 32 raw Ed25519 public-key bytes.
  trusted_signing_fingerprint=<hex>
      Reviewed SHA256 fingerprint of the raw public key (64 lowercase hex).
  release_manifest_verifier=<path>
      Reviewed `sorafs-validate` executable.
  trusted_release_manifest_verifier_sha256=<hex>
      Reviewed SHA256 digest of that exact executable (64 lowercase hex).

Optional:
  --config <path>                 Config file with key=value pairs.
  --out <dir>                     Output directory (default: <workspace>/artifacts/sorafs_gateway_attest).
  --workspace <path>              Repository root (default: current directory).
  --release-manifest <path>       Same as release_manifest.
  --release-manifest-signature <path>
  --release-manifest-public-key <path>
  --trusted-signing-fingerprint <hex>
  --release-manifest-verifier <path>
  --trusted-release-manifest-verifier-sha256 <hex>
  --denylist-old <bundle.json>    Previously published denylist bundle for diff evidence.
  --denylist-new <bundle.json>    Newly generated denylist bundle for diff evidence.
  --denylist-report <path>        Override path for diff JSON (default: <out>/denylist_diff.json).
  --help                          Show this help message and exit.

Self-asserted signature bundles and OIDC token hashes are not accepted
authenticity evidence.
USAGE
}

run_xtask() {
  local -a args=("$@")
  if cargo --list 2>/dev/null | awk '{print $1}' | grep -qx 'xtask'; then
    cargo xtask "${args[@]}"
  else
    echo "cargo xtask unavailable; falling back to cargo run -p xtask --bin xtask -- ${args[*]}" >&2
    cargo run -p xtask --bin xtask -- "${args[@]}"
  fi
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
manifest_path=""
signature_path=""
public_key_path=""
trusted_signing_fingerprint=""
release_manifest_verifier=""
trusted_release_manifest_verifier_sha256=""
config_path=""
denylist_old_bundle=""
denylist_new_bundle=""
denylist_report=""

cli_workspace=""
cli_signing_key=""
cli_signer=""
cli_gateway=""
cli_out=""
cli_manifest=""
cli_signature=""
cli_public_key=""
cli_trusted_signing_fingerprint=""
cli_release_manifest_verifier=""
cli_trusted_release_manifest_verifier_sha256=""
cli_denylist_old=""
cli_denylist_new=""
cli_denylist_report=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      require_option_value "$1" "${2-}"
      cli_workspace="$(cd "$2" && pwd)"
      workspace="$cli_workspace"
      shift 2
      ;;
    --config)
      require_option_value "$1" "${2-}"
      config_path="$(abs_path "$2")"
      shift 2
      ;;
    --signing-key)
      require_option_value "$1" "${2-}"
      cli_signing_key="$(abs_path "$2")"
      signing_key="$cli_signing_key"
      shift 2
      ;;
    --signer)
      require_option_value "$1" "${2-}"
      cli_signer="$2"
      signer_account="$cli_signer"
      shift 2
      ;;
    --gateway)
      require_option_value "$1" "${2-}"
      cli_gateway="$2"
      gateway_target="$cli_gateway"
      shift 2
      ;;
    --out)
      require_option_value "$1" "${2-}"
      cli_out="$2"
      output_dir="$cli_out"
      shift 2
      ;;
    --release-manifest)
      require_option_value "$1" "${2-}"
      cli_manifest="$(abs_path "$2")"
      manifest_path="$cli_manifest"
      shift 2
      ;;
    --release-manifest-signature)
      require_option_value "$1" "${2-}"
      cli_signature="$(abs_path "$2")"
      signature_path="$cli_signature"
      shift 2
      ;;
    --release-manifest-public-key)
      require_option_value "$1" "${2-}"
      cli_public_key="$(abs_path "$2")"
      public_key_path="$cli_public_key"
      shift 2
      ;;
    --trusted-signing-fingerprint)
      require_option_value "$1" "${2-}"
      cli_trusted_signing_fingerprint="$2"
      trusted_signing_fingerprint="$cli_trusted_signing_fingerprint"
      shift 2
      ;;
    --release-manifest-verifier)
      require_option_value "$1" "${2-}"
      cli_release_manifest_verifier="$(abs_path "$2")"
      release_manifest_verifier="$cli_release_manifest_verifier"
      shift 2
      ;;
    --trusted-release-manifest-verifier-sha256)
      require_option_value "$1" "${2-}"
      cli_trusted_release_manifest_verifier_sha256="$2"
      trusted_release_manifest_verifier_sha256="$cli_trusted_release_manifest_verifier_sha256"
      shift 2
      ;;
    --denylist-old)
      require_option_value "$1" "${2-}"
      cli_denylist_old="$(abs_path "$2")"
      denylist_old_bundle="$cli_denylist_old"
      shift 2
      ;;
    --denylist-new)
      require_option_value "$1" "${2-}"
      cli_denylist_new="$(abs_path "$2")"
      denylist_new_bundle="$cli_denylist_new"
      shift 2
      ;;
    --denylist-report)
      require_option_value "$1" "${2-}"
      cli_denylist_report="$(abs_path "$2")"
      denylist_report="$cli_denylist_report"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "${config_path}" ]]; then
  validate_existing_file_path "gateway self-cert config" "${config_path}"
  while IFS='=' read -r raw_key raw_value; do
    [[ -z "${raw_key// }" || "${raw_key}" =~ ^# ]] && continue
    key="$(echo "$raw_key" | awk '{$1=$1;print}')"
    value="$(echo "$raw_value" | awk '{$1=$1;print}')"
    case "$key" in
      signing_key) [[ -z "$signing_key" ]] && signing_key="$(abs_path "$value")" ;;
      signer) [[ -z "$signer_account" ]] && signer_account="$value" ;;
      gateway) [[ -z "$gateway_target" ]] && gateway_target="$value" ;;
      out) [[ -z "$output_dir" ]] && output_dir="$value" ;;
      release_manifest) [[ -z "$manifest_path" ]] && manifest_path="$(abs_path "$value")" ;;
      release_manifest_signature) [[ -z "$signature_path" ]] && signature_path="$(abs_path "$value")" ;;
      release_manifest_public_key) [[ -z "$public_key_path" ]] && public_key_path="$(abs_path "$value")" ;;
      trusted_signing_fingerprint) [[ -z "$trusted_signing_fingerprint" ]] && trusted_signing_fingerprint="$value" ;;
      release_manifest_verifier) [[ -z "$release_manifest_verifier" ]] && release_manifest_verifier="$(abs_path "$value")" ;;
      trusted_release_manifest_verifier_sha256)
        [[ -z "$trusted_release_manifest_verifier_sha256" ]] &&
          trusted_release_manifest_verifier_sha256="$value"
        ;;
      denylist_old_bundle) [[ -z "$denylist_old_bundle" ]] && denylist_old_bundle="$(abs_path "$value")" ;;
      denylist_new_bundle) [[ -z "$denylist_new_bundle" ]] && denylist_new_bundle="$(abs_path "$value")" ;;
      denylist_report) [[ -z "$denylist_report" ]] && denylist_report="$(abs_path "$value")" ;;
      workspace) [[ -z "$workspace" ]] && workspace="$(cd "$value" && pwd)" ;;
      *)
        echo "error: unknown config key '${key}' in ${config_path}" >&2
        exit 1
        ;;
    esac
  done < "${config_path}"
fi

[[ -n "$cli_workspace" ]] && workspace="$cli_workspace"
[[ -n "$cli_signing_key" ]] && signing_key="$cli_signing_key"
[[ -n "$cli_signer" ]] && signer_account="$cli_signer"
[[ -n "$cli_gateway" ]] && gateway_target="$cli_gateway"
[[ -n "$cli_out" ]] && output_dir="$cli_out"
[[ -n "$cli_manifest" ]] && manifest_path="$cli_manifest"
[[ -n "$cli_signature" ]] && signature_path="$cli_signature"
[[ -n "$cli_public_key" ]] && public_key_path="$cli_public_key"
[[ -n "$cli_trusted_signing_fingerprint" ]] &&
  trusted_signing_fingerprint="$cli_trusted_signing_fingerprint"
[[ -n "$cli_release_manifest_verifier" ]] &&
  release_manifest_verifier="$cli_release_manifest_verifier"
[[ -n "$cli_trusted_release_manifest_verifier_sha256" ]] &&
  trusted_release_manifest_verifier_sha256="$cli_trusted_release_manifest_verifier_sha256"
[[ -n "$cli_denylist_old" ]] && denylist_old_bundle="$cli_denylist_old"
[[ -n "$cli_denylist_new" ]] && denylist_new_bundle="$cli_denylist_new"
[[ -n "$cli_denylist_report" ]] && denylist_report="$cli_denylist_report"

if [[ -z "$workspace" ]]; then
  workspace="${PWD}"
fi

declare -a missing=()
[[ -n "$signing_key" ]] || missing+=("--signing-key")
[[ -n "$signer_account" ]] || missing+=("--signer")
[[ -n "$gateway_target" ]] || missing+=("--gateway")
[[ -n "$manifest_path" ]] || missing+=("--release-manifest")
[[ -n "$signature_path" ]] || missing+=("--release-manifest-signature")
[[ -n "$public_key_path" ]] || missing+=("--release-manifest-public-key")
[[ -n "$trusted_signing_fingerprint" ]] || missing+=("--trusted-signing-fingerprint")
[[ -n "$release_manifest_verifier" ]] || missing+=("--release-manifest-verifier")
[[ -n "$trusted_release_manifest_verifier_sha256" ]] ||
  missing+=("--trusted-release-manifest-verifier-sha256")
if (( ${#missing[@]} > 0 )); then
  echo "error: required gateway self-cert options are missing: ${missing[*]}" >&2
  exit 1
fi

require_sha256 "trusted signing fingerprint" "$trusted_signing_fingerprint"
require_sha256 \
  "trusted release-manifest verifier SHA256" \
  "$trusted_release_manifest_verifier_sha256"
validate_existing_file_path "gateway attestation signing key" "$signing_key"
validate_existing_file_path "aggregate release manifest" "$manifest_path"
validate_existing_file_path "aggregate release-manifest signature" "$signature_path"
validate_existing_file_path "aggregate release-manifest raw public key" "$public_key_path"
validate_existing_executable_file_path \
  "native release-manifest verifier" \
  "$release_manifest_verifier"
release_manifest_signing_helper="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/release_manifest_signing.py"
validate_existing_file_path \
  "release manifest signing helper" \
  "$release_manifest_signing_helper"
if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required" >&2
  exit 1
fi

if [[ -z "${output_dir}" ]]; then
  output_dir="${workspace}/artifacts/sorafs_gateway_attest"
else
  output_dir="$(abs_output_path "${output_dir}")"
fi
prepare_output_dir_path "gateway self-cert output directory" "${output_dir}"

verify_summary_path="${output_dir}/release_manifest.verify.json"
prepare_new_output_file_path \
  "release manifest verification summary" \
  "${verify_summary_path}"

echo "Verifying aggregate release manifest with pinned sorafs-validate..."
verification_json="$(
  python3 "$release_manifest_signing_helper" verify \
    --manifest "$manifest_path" \
    --signature "$signature_path" \
    --public-key "$public_key_path" \
    --trusted-signing-fingerprint "$trusted_signing_fingerprint" \
    --release-manifest-verifier "$release_manifest_verifier" \
    --trusted-release-manifest-verifier-sha256 \
      "$trusted_release_manifest_verifier_sha256"
)"
printf '%s\n' "$verification_json" | tee "$verify_summary_path"

cmd=(
  sorafs-gateway-attest
  --signing-key "${signing_key}"
  --signer-account "${signer_account}"
  --gateway "${gateway_target}"
  --out "${output_dir}"
)

(
  cd "${workspace}"
  run_xtask "${cmd[@]}"
)

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
    else
      diff_report_path="$(abs_output_path "${diff_report_path}")"
    fi
    prepare_output_file_path "denylist diff report" "${diff_report_path}"
    echo "Generating denylist diff evidence..."
    (
      cd "${workspace}"
      run_xtask sorafs-gateway denylist diff \
        --old "${denylist_old_bundle}" \
        --new "${denylist_new_bundle}" \
        --report-json "${diff_report_path}"
    )
    echo "Denylist diff report written to ${diff_report_path}"
  fi
fi
