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
release_sorafs_cli.sh --manifest <path> [options]

Builds the SoraFS CLI (if needed), signs the provided manifest, and verifies the
resulting bundle with `sorafs_cli manifest verify-signature` so release jobs
fail fast when signatures or chunk metadata drift.

Required:
  --manifest <path>          Path to the Norito manifest (.to) slated for release.

Optional:
  --config <path>          Config file with key=value pairs.
  --workspace <path>         Repository root (default: script parent/..).
  --cli <path>               Prebuilt sorafs_cli binary to use instead of building.
  --chunk-plan <path>        Chunk plan JSON emitted by `car pack`.
  --chunk-summary <path>     Summary JSON emitted by `car pack`.
  --chunk-digest-sha3 <hex>  Hex digest to cross-check when signing/verifying.
  --bundle-out <path>        Where to write the signature bundle
                             (default: <workspace>/artifacts/sorafs_cli_release/manifest.bundle.json).
  --signature-out <path>     Where to write the detached signature
                             (default: <workspace>/artifacts/sorafs_cli_release/manifest.sig).
  --expect-token-hash <hex>  Optional BLAKE3 hash of the identity token to enforce.
  --identity-token <jwt>     Inline OIDC token passed to `manifest sign`.
  --identity-token-env <VAR> Environment variable holding the OIDC token.
  --identity-token-file <p>  File containing the OIDC token.
  --identity-token-provider <name>
                             Provider helper (e.g., github-actions) for token retrieval.
  --identity-token-audience <aud>
                             Audience override when using `--identity-token-provider`.
  --issued-at <unix>         Override the `issued_at_unix` timestamp recorded in summaries.
  --help                     Show this help and exit.
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

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli_path=""
manifest_path=""
chunk_plan_path=""
chunk_summary_path=""
chunk_digest_hex=""
bundle_out=""
signature_out=""
expect_token_hash=""
identity_token_inline=""
identity_token_env=""
identity_token_file=""
identity_token_provider=""
identity_token_audience=""
issued_at_override=""
config_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      workspace="$(abs_path "$2")"
      shift 2
      ;;
    --cli)
      cli_path="$(abs_path "$2")"
      shift 2
      ;;
    --config)
      config_path="$(abs_path "$2")"
      shift 2
      ;;
    --manifest)
      manifest_path="$(abs_path "$2")"
      shift 2
      ;;
    --chunk-plan)
      chunk_plan_path="$(abs_path "$2")"
      shift 2
      ;;
    --chunk-summary)
      chunk_summary_path="$(abs_path "$2")"
      shift 2
      ;;
    --chunk-digest-sha3)
      chunk_digest_hex="$2"
      shift 2
      ;;
    --bundle-out)
      bundle_out="$(abs_path "$2")"
      shift 2
      ;;
    --signature-out)
      signature_out="$(abs_path "$2")"
      shift 2
      ;;
    --expect-token-hash)
      expect_token_hash="$2"
      shift 2
      ;;
    --identity-token)
      identity_token_inline="$2"
      shift 2
      ;;
    --identity-token-env)
      identity_token_env="$2"
      shift 2
      ;;
    --identity-token-file)
      identity_token_file="$(abs_path "$2")"
      shift 2
      ;;
    --identity-token-provider)
      identity_token_provider="$2"
      shift 2
      ;;
    --identity-token-audience)
      identity_token_audience="$2"
      shift 2
      ;;
    --issued-at)
      issued_at_override="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$manifest_path" ]]; then
  manifest_path=""
fi

manifest_path="$(abs_path "$manifest_path")"
cli_manifest=""
cli_chunk_plan=""
cli_chunk_summary=""
cli_chunk_digest=""
cli_expect_token_hash=""

if [[ -n "$config_path" ]]; then
  if [[ ! -f "$config_path" ]]; then
    echo "error: config file not found at $config_path" >&2
    exit 1
  fi
  while IFS='=' read -r raw_key raw_value; do
    [[ -z "${raw_key// }" || "${raw_key}" =~ ^# ]] && continue
    key="$(echo "$raw_key" | awk '{$1=$1;print}')"
    value="$(echo "$raw_value" | awk '{$1=$1;print}')"
    case "$key" in
      workspace) [[ -z "$workspace" ]] && workspace="$(cd "$value" && pwd)" ;;
      manifest) cli_manifest="$(abs_path "$value")" ;;
      chunk_plan) cli_chunk_plan="$(abs_path "$value")" ;;
      chunk_summary) cli_chunk_summary="$(abs_path "$value")" ;;
      chunk_digest_sha3) cli_chunk_digest="$value" ;;
      expect_token_hash) cli_expect_token_hash="$value" ;;
      bundle_out) [[ -z "$bundle_out" ]] && bundle_out="$(abs_path "$value")" ;;
      signature_out) [[ -z "$signature_out" ]] && signature_out="$(abs_path "$value")" ;;
      cli) [[ -z "$cli_path" ]] && cli_path="$(abs_path "$value")" ;;
      identity_token) [[ -z "$identity_token_inline" ]] && identity_token_inline="$value" ;;
      identity_token_env) [[ -z "$identity_token_env" ]] && identity_token_env="$value" ;;
      identity_token_file) [[ -z "$identity_token_file" ]] && identity_token_file="$(abs_path "$value")" ;;
      identity_token_provider) [[ -z "$identity_token_provider" ]] && identity_token_provider="$value" ;;
      identity_token_audience) [[ -z "$identity_token_audience" ]] && identity_token_audience="$value" ;;
      *)
        echo "warning: unknown config key '$key' in $config_path" >&2
        ;;
    esac
  done < "$config_path"
fi

[[ -z "$manifest_path" && -n "$cli_manifest" ]] && manifest_path="$cli_manifest"
[[ -z "$chunk_plan_path" && -n "$cli_chunk_plan" ]] && chunk_plan_path="$cli_chunk_plan"
[[ -z "$chunk_summary_path" && -n "$cli_chunk_summary" ]] && chunk_summary_path="$cli_chunk_summary"
[[ -z "$chunk_digest_hex" && -n "$cli_chunk_digest" ]] && chunk_digest_hex="$cli_chunk_digest"
[[ -z "$expect_token_hash" && -n "$cli_expect_token_hash" ]] && expect_token_hash="$cli_expect_token_hash"

# Provide fixture defaults if still unset
if [[ -z "$manifest_path" ]]; then
  manifest_path="$(abs_path "fixtures/sorafs_manifest/ci_sample/manifest.to")"
fi
[[ -z "$chunk_plan_path" ]] && chunk_plan_path="$(abs_path "fixtures/sorafs_manifest/ci_sample/chunk_plan.json")"
[[ -z "$chunk_summary_path" ]] && chunk_summary_path="$(abs_path "fixtures/sorafs_manifest/ci_sample/car_summary.json")"
[[ -z "$chunk_digest_hex" ]] && chunk_digest_hex="0236c64b664bfff906f32106789baa61e7d4d9fdb4514e173b77a77d2883946e"
[[ -z "$expect_token_hash" ]] && expect_token_hash="7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b"

if [[ ! -f "$manifest_path" ]]; then
  echo "error: manifest not found at $manifest_path" >&2
  exit 1
fi

if [[ -n "$chunk_plan_path" && ! -f "$chunk_plan_path" ]]; then
  echo "error: chunk plan not found at $chunk_plan_path" >&2
  exit 1
fi

if [[ -n "$chunk_summary_path" && ! -f "$chunk_summary_path" ]]; then
  echo "error: chunk summary not found at $chunk_summary_path" >&2
  exit 1
fi

identity_flag_count=0
[[ -n "$identity_token_inline" ]] && identity_flag_count=$((identity_flag_count + 1))
[[ -n "$identity_token_env" ]] && identity_flag_count=$((identity_flag_count + 1))
[[ -n "$identity_token_file" ]] && identity_flag_count=$((identity_flag_count + 1))
[[ -n "$identity_token_provider" ]] && identity_flag_count=$((identity_flag_count + 1))
if (( identity_flag_count > 1 )); then
  echo "error: identity token flags are mutually exclusive" >&2
  exit 1
fi
if [[ -z "$identity_token_provider" && -n "$identity_token_audience" ]]; then
  echo "error: --identity-token-audience requires --identity-token-provider" >&2
  exit 1
fi
if [[ -n "$identity_token_provider" && -z "$identity_token_audience" ]]; then
  echo "error: --identity-token-provider now requires --identity-token-audience" >&2
  exit 1
fi

output_root="${workspace}/artifacts/sorafs_cli_release"
if [[ -n "$bundle_out" ]]; then
  output_root="$(dirname "$bundle_out")"
else
  bundle_out="${output_root}/manifest.bundle.json"
fi
if [[ -n "$signature_out" ]]; then
  output_root="$(dirname "$signature_out")"
else
  signature_out="${output_root}/manifest.sig"
fi
mkdir -p "$output_root"

sign_summary_path="${output_root}/manifest.sign.summary.json"
verify_summary_path="${output_root}/manifest.verify.summary.json"

if [[ -z "$cli_path" ]]; then
  cli_path="${workspace}/target/release/sorafs_cli"
  if [[ ! -x "$cli_path" ]]; then
    echo "Building sorafs_cli (release)..."
    (cd "$workspace" && cargo build -p sorafs_car --features cli --bin sorafs_cli --release)
  fi
else
  cli_path="$(abs_path "$cli_path")"
fi
if [[ ! -x "$cli_path" ]]; then
  echo "error: sorafs_cli binary not executable at $cli_path" >&2
  exit 1
fi

sign_cmd=("$cli_path" "manifest" "sign" "--manifest=$manifest_path" "--bundle-out=$bundle_out" "--signature-out=$signature_out")
if [[ -n "$chunk_plan_path" ]]; then sign_cmd+=("--chunk-plan=$chunk_plan_path"); fi
if [[ -n "$chunk_summary_path" ]]; then sign_cmd+=("--summary=$chunk_summary_path"); fi
if [[ -n "$chunk_digest_hex" ]]; then sign_cmd+=("--chunk-digest-sha3=$chunk_digest_hex"); fi
if [[ -n "$identity_token_inline" ]]; then sign_cmd+=("--identity-token=$identity_token_inline"); fi
if [[ -n "$identity_token_env" ]]; then sign_cmd+=("--identity-token-env=$identity_token_env"); fi
if [[ -n "$identity_token_file" ]]; then sign_cmd+=("--identity-token-file=$identity_token_file"); fi
if [[ -n "$identity_token_provider" ]]; then
  sign_cmd+=("--identity-token-provider=$identity_token_provider")
  if [[ -n "$identity_token_audience" ]]; then
    sign_cmd+=("--identity-token-audience=$identity_token_audience")
  fi
fi
if [[ -n "$issued_at_override" ]]; then
  sign_cmd+=("--issued-at=$issued_at_override")
fi

echo "Signing manifest with sorafs_cli..."
"${sign_cmd[@]}" | tee "$sign_summary_path"

auto_token_hash=""
if command -v python3 >/dev/null 2>&1; then
  auto_token_hash="$(python3 - "$sign_summary_path" -c 'import json, sys;\ntry:\n    with open(sys.argv[1], "r", encoding="utf-8") as handle:\n        value = json.load(handle).get("identity_token_hash_blake3_hex")\n    if isinstance(value, str):\n        print(value, end="")\nexcept Exception:\n    pass' 2>/dev/null || true)"
fi

expected_hash="$expect_token_hash"
if [[ -z "$expected_hash" && -n "$auto_token_hash" ]]; then
  expected_hash="$auto_token_hash"
fi

verify_summary_source="$chunk_summary_path"
if [[ -z "$verify_summary_source" ]]; then
  verify_summary_source="$sign_summary_path"
fi

verify_cmd=("$cli_path" "manifest" "verify-signature" "--manifest=$manifest_path" "--bundle=$bundle_out")
if [[ -n "$verify_summary_source" ]]; then verify_cmd+=("--summary=$verify_summary_source"); fi
if [[ -n "$chunk_plan_path" ]]; then verify_cmd+=("--chunk-plan=$chunk_plan_path"); fi
if [[ -n "$chunk_digest_hex" ]]; then verify_cmd+=("--chunk-digest-sha3=$chunk_digest_hex"); fi
if [[ -n "$expected_hash" ]]; then verify_cmd+=("--expect-token-hash=$expected_hash"); fi

echo "Verifying signature bundle..."
"${verify_cmd[@]}" | tee "$verify_summary_path"

echo
echo "Release artifacts:"
echo "  Bundle   : $bundle_out"
echo "  Signature: $signature_out"
echo "  Sign log : $sign_summary_path"
echo "  Verify log: $verify_summary_path"
