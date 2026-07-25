#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
release_sorafs_cli.sh --manifest <path> [options]

Signs a canonical aggregate SoraFS release manifest through a reviewed external
Ed25519 signer (for example, a PKCS#11/HSM adapter), then verifies the exact
manifest, raw public key, and 64-byte signature with a SHA256-pinned
`sorafs-validate release-manifest` binary.

Required:
  --manifest <path>
      Canonical aggregate release manifest JSON.
  --external-signer <path>
      Executable Ed25519 signer adapter. Its first two positional arguments are
      MANIFEST_PATH and a new SIGNATURE_OUTPUT_PATH; it must write exactly 64
      raw signature bytes.
  --signing-public-key <path>
      Exactly 32 raw bytes for the governed Ed25519 public key.
  --trusted-signing-fingerprint <hex>
      Reviewed SHA256 fingerprint of the raw public key (64 lowercase hex).
  --release-manifest-verifier <path>
      Reviewed `sorafs-validate` executable.
  --trusted-release-manifest-verifier-sha256 <hex>
      Reviewed SHA256 digest of that exact executable (64 lowercase hex).

Optional:
  --workspace <path>
      Repository/work directory used for default outputs.
  --signature-out <path>
      New raw signature output (default:
      <workspace>/artifacts/sorafs_cli_release/release_manifest.ed25519.sig).
  --public-key-out <path>
      New raw public-key output (default:
      <workspace>/artifacts/sorafs_cli_release/release_manifest.ed25519.pub).
  --verification-summary-out <path>
      New verification receipt (default:
      <workspace>/artifacts/sorafs_cli_release/release_manifest.verify.json).
  --help
      Show this help and exit.

OIDC tokens and self-asserted signature bundles are not release-authenticity
inputs. Keyless cosign is a separate provenance layer.
USAGE
}

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

require_option_value() {
  local option="$1"
  local value="${2-}"
  if [[ -z "$value" || "$value" == --* ]]; then
    echo "error: ${option} requires a value" >&2
    exit 1
  fi
}

reject_symlinked_parent() {
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
  reject_symlinked_parent "$label" "$target"
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

prepare_new_output_file_path() {
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
  reject_symlinked_parent "$label" "$target"
  if [[ -e "$target" ]]; then
    echo "error: ${label} must not already exist: ${target}" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$target")"
  reject_symlinked_parent "$label" "$target"
  if [[ -e "$target" || -L "$target" ]]; then
    echo "error: ${label} must not already exist: ${target}" >&2
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

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace="$(cd "${script_dir}/.." && pwd)"
manifest_path=""
external_signer=""
signing_public_key=""
trusted_signing_fingerprint=""
release_manifest_verifier=""
trusted_release_manifest_verifier_sha256=""
signature_out=""
public_key_out=""
verification_summary_out=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      require_option_value "$1" "${2-}"
      workspace="$(abs_path "$2")"
      shift 2
      ;;
    --manifest)
      require_option_value "$1" "${2-}"
      manifest_path="$(abs_path "$2")"
      shift 2
      ;;
    --external-signer)
      require_option_value "$1" "${2-}"
      external_signer="$(abs_path "$2")"
      shift 2
      ;;
    --signing-public-key)
      require_option_value "$1" "${2-}"
      signing_public_key="$(abs_path "$2")"
      shift 2
      ;;
    --trusted-signing-fingerprint)
      require_option_value "$1" "${2-}"
      trusted_signing_fingerprint="$2"
      shift 2
      ;;
    --release-manifest-verifier)
      require_option_value "$1" "${2-}"
      release_manifest_verifier="$(abs_path "$2")"
      shift 2
      ;;
    --trusted-release-manifest-verifier-sha256)
      require_option_value "$1" "${2-}"
      trusted_release_manifest_verifier_sha256="$2"
      shift 2
      ;;
    --signature-out)
      require_option_value "$1" "${2-}"
      signature_out="$(abs_output_path "$2")"
      shift 2
      ;;
    --public-key-out)
      require_option_value "$1" "${2-}"
      public_key_out="$(abs_output_path "$2")"
      shift 2
      ;;
    --verification-summary-out)
      require_option_value "$1" "${2-}"
      verification_summary_out="$(abs_output_path "$2")"
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

declare -a missing=()
[[ -n "$manifest_path" ]] || missing+=("--manifest")
[[ -n "$external_signer" ]] || missing+=("--external-signer")
[[ -n "$signing_public_key" ]] || missing+=("--signing-public-key")
[[ -n "$trusted_signing_fingerprint" ]] || missing+=("--trusted-signing-fingerprint")
[[ -n "$release_manifest_verifier" ]] || missing+=("--release-manifest-verifier")
[[ -n "$trusted_release_manifest_verifier_sha256" ]] ||
  missing+=("--trusted-release-manifest-verifier-sha256")
if (( ${#missing[@]} > 0 )); then
  echo "error: required release signing options are missing: ${missing[*]}" >&2
  exit 1
fi

require_sha256 "trusted signing fingerprint" "$trusted_signing_fingerprint"
require_sha256 \
  "trusted release-manifest verifier SHA256" \
  "$trusted_release_manifest_verifier_sha256"

validate_existing_file_path "aggregate release manifest" "$manifest_path"
validate_existing_executable_file_path "external Ed25519 signer" "$external_signer"
validate_existing_file_path "raw Ed25519 signing public key" "$signing_public_key"
validate_existing_executable_file_path \
  "native release-manifest verifier" \
  "$release_manifest_verifier"

release_manifest_signing_helper="${script_dir}/release_manifest_signing.py"
validate_existing_file_path \
  "release manifest signing helper" \
  "$release_manifest_signing_helper"
if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required" >&2
  exit 1
fi

output_root="${workspace}/artifacts/sorafs_cli_release"
[[ -n "$signature_out" ]] ||
  signature_out="${output_root}/release_manifest.ed25519.sig"
[[ -n "$public_key_out" ]] ||
  public_key_out="${output_root}/release_manifest.ed25519.pub"
[[ -n "$verification_summary_out" ]] ||
  verification_summary_out="${output_root}/release_manifest.verify.json"
signature_out="$(abs_output_path "$signature_out")"
public_key_out="$(abs_output_path "$public_key_out")"
verification_summary_out="$(abs_output_path "$verification_summary_out")"

if [[ "$signature_out" == "$public_key_out" ||
      "$signature_out" == "$verification_summary_out" ||
      "$public_key_out" == "$verification_summary_out" ]]; then
  echo "error: signature, public-key, and verification-summary outputs must be distinct" >&2
  exit 1
fi

prepare_new_output_file_path "release signature output" "$signature_out"
prepare_new_output_file_path "release public-key output" "$public_key_out"
prepare_new_output_file_path \
  "release verification summary output" \
  "$verification_summary_out"

echo "Signing aggregate release manifest through the reviewed Ed25519 signer..."
verification_json="$(
  python3 "$release_manifest_signing_helper" sign \
    --manifest "$manifest_path" \
    --external-signer "$external_signer" \
    --signing-public-key "$signing_public_key" \
    --trusted-signing-fingerprint "$trusted_signing_fingerprint" \
    --signature-output "$signature_out" \
    --public-key-output "$public_key_out" \
    --release-manifest-verifier "$release_manifest_verifier" \
    --trusted-release-manifest-verifier-sha256 \
      "$trusted_release_manifest_verifier_sha256"
)"
printf '%s\n' "$verification_json" | tee "$verification_summary_out"

validate_existing_file_path \
  "release verification summary output" \
  "$verification_summary_out"

echo
echo "Release authenticity artifacts:"
echo "  Manifest     : $manifest_path"
echo "  Signature    : $signature_out"
echo "  Public key   : $public_key_out"
echo "  Verification : $verification_summary_out"
