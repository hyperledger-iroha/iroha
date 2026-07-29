#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-${REPO_ROOT}/dist/taira-rollout}"
PROFILE="${PROFILE:-release}"
ALLOW_DIRTY=0
SKIP_BUILD=0
SKIP_LOCAL_REGRESSIONS=0
KAGEMUSHA_RELEASE_POLICY="${KAGEMUSHA_V4_RELEASE_POLICY_PATH:-}"
KAGEMUSHA_ARTIFACT_ROOT="${KAGEMUSHA_V4_ARTIFACT_ROOT:-}"
IROHAD_RELEASE_FEATURES="embedded-soracloud-runtime,zk-stark"
PRIVACY_RELEASE_EVIDENCE_FEATURE="privacy-release-evidence"
PRIVACY_RELEASE_RUNNER_PACKAGE="iroha_test_network"
PRIVACY_RELEASE_RUNNER_BIN="taira_privacy_release_runner"
PRIVACY_EXACT12_MATRIX="${REPO_ROOT}/fixtures/privacy/exact12_v1.tsv"
PRIVACY_EXPECTATIONS_NORITO="${REPO_ROOT}/fixtures/privacy/native_release_expectations_v1.norito"
PRIVACY_EXPECTATIONS_JSON="${REPO_ROOT}/fixtures/privacy/native_release_expectations_v1.json"
WORKSPACE_SOURCE_MANIFEST_SCRIPT="${REPO_ROOT}/scripts/compute_workspace_source_manifest.py"
TAIRA_RELEASE_AUTHORITY_SCRIPT="${REPO_ROOT}/scripts/taira_release_authority.py"
RELEASE_ARTIFACT_CONTRACT_SCRIPT="${REPO_ROOT}/scripts/release_artifact_contract.py"
RELEASE_MANIFEST_GENERATOR="${REPO_ROOT}/scripts/generate_release_manifest.py"
RELEASE_MANIFEST_SIGNING_HELPER="${REPO_ROOT}/scripts/release_manifest_signing.py"
RELEASE_CHECKSUM_WRITER="${REPO_ROOT}/scripts/write_release_sha256sums.py"
TAIRA_RELEASE_EXTERNAL_SIGNER_PATH="${TAIRA_RELEASE_EXTERNAL_SIGNER_PATH:-}"
TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH="${TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH:-}"
TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT="${TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT:-}"
TAIRA_RELEASE_MANIFEST_VERIFIER_PATH="${TAIRA_RELEASE_MANIFEST_VERIFIER_PATH:-}"
TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256="${TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256:-}"

usage() {
  cat <<'EOF'
Usage: build_taira_rollout_bundle.sh [--output-dir PATH] [--profile debug|release]
                                     [--allow-dirty] [--skip-build]
                                     [--skip-local-regressions]
                                     [--skip-router-regression]
                                     [--kagemusha-release-policy PATH]
                                     [--kagemusha-artifact-root PATH]

Build a deterministic public-Taira rollout bundle from the current `../iroha`
checkout. By default the script refuses to package a dirty worktree so the
result can be tied to one exact git revision. It also runs the focused
`iroha_core` SoraSwap deploy-route router regression and three-hop nested
transfer authority canary before packaging.

Production invocations must enter through
`dpn-api-rust/ops/taira/build-validator-bundle.sh`. That wrapper installs the
reviewed full Cargo lock, verifies its checksum and Rust toolchain, rejects a
dirty source tree, and supplies reviewed build provenance to this script.

The bundle contains:
  - `irohad` and `iroha` from `target/<profile>/`
  - `sorafs_manifest_builder` and `sorafs_tx_stdin_builder` from `target/<profile>/`
  - the feature-separated `taira_privacy_release_runner`
  - authoritative native-privacy receipt, command-manifest, stage-artifact,
    and frozen-expectation Norito files with deterministic JSON projections
  - the checked-in `configs/soranexus/taira/` operator bundle
  - `scripts/render_taira_validator_bundle.py`
  - `scripts/render_taira_edge_nginx_conf.py`
  - `scripts/taira_faucet_canary.py`
  - `configs/soranexus/taira/check_inrou_host_prereqs.sh`
  - `rollout.manifest.json`
  - `sha256sums.txt`
  - `<bundle>.tar.gz`

The authenticated ABI-21/V4 Kagemusha policy and artifact root are mandatory.
They are verified by the production promotion corridor and copied into the
bundle; there is no build or rollout path that omits offline cash.

`--skip-build` is a debug-only convenience. Release bundles always rebuild
every packaged binary from the exact source tree exercised by the gates.

Native privacy evidence is generated only after the ordinary validator build.
The validator uses the production feature set above; the separate evidence
runner alone is built with `privacy-release-evidence`. The canonical workspace
source manifest is checked before build, after build/evidence, and immediately
before archiving so a pre-build report cannot masquerade as release evidence.

Release builds also require these externally provisioned authority inputs:
  TAIRA_RELEASE_EXTERNAL_SIGNER_PATH
  TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH
  TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT
  TAIRA_RELEASE_MANIFEST_VERIFIER_PATH
  TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256

The signer, public key, and independently reviewed native verifier must be
absolute non-symlink regular files outside this checkout. The private signing
key is never accepted by this script.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      [[ $# -ge 2 ]] || {
        echo "missing value for --output-dir" >&2
        exit 1
      }
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --profile)
      [[ $# -ge 2 ]] || {
        echo "missing value for --profile" >&2
        exit 1
      }
      PROFILE="$2"
      shift 2
      ;;
    --allow-dirty)
      ALLOW_DIRTY=1
      shift
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --skip-local-regressions|--skip-router-regression)
      SKIP_LOCAL_REGRESSIONS=1
      shift
      ;;
    --kagemusha-release-policy)
      [[ $# -ge 2 ]] || {
        echo "missing value for --kagemusha-release-policy" >&2
        exit 1
      }
      KAGEMUSHA_RELEASE_POLICY="$2"
      shift 2
      ;;
    --kagemusha-artifact-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --kagemusha-artifact-root" >&2
        exit 1
      }
      KAGEMUSHA_ARTIFACT_ROOT="$2"
      shift 2
      ;;
    -h|--help)
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

case "$PROFILE" in
  debug|release)
    ;;
  *)
    echo "--profile must be one of: debug, release" >&2
    exit 1
    ;;
esac

if [[ "$PROFILE" == "release" && $SKIP_BUILD -eq 1 ]]; then
  echo "refusing --skip-build with --profile release: release binaries must be rebuilt from the exact tested source" >&2
  exit 1
fi
if [[ "$PROFILE" == "release" && $SKIP_LOCAL_REGRESSIONS -eq 1 ]]; then
  echo "refusing --skip-local-regressions with --profile release: every release gate is mandatory" >&2
  exit 1
fi

require_external_release_authority_file() {
  local label="$1"
  local path="$2"
  if [[ -z "$path" || "$path" != /* ]]; then
    echo "$label must be an explicit absolute path" >&2
    exit 1
  fi
  if [[ ! -f "$path" || -L "$path" ]]; then
    echo "$label must be a non-symlink regular file" >&2
    exit 1
  fi
  local canonical_path
  local canonical_repo_root
  canonical_path="$(
    python3 -S -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$path"
  )"
  canonical_repo_root="$(
    python3 -S -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$REPO_ROOT"
  )"
  if [[ "$canonical_path" != "$path" ]]; then
    echo "$label must use its canonical physical path without symlink or parent aliases" >&2
    exit 1
  fi
  if [[ "$canonical_path" == "$canonical_repo_root" || "$canonical_path" == "$canonical_repo_root/"* ]]; then
    echo "$label must be provisioned outside the Iroha checkout" >&2
    exit 1
  fi
}

if [[ "$PROFILE" == "release" ]]; then
  require_external_release_authority_file \
    "Taira release external signer" \
    "$TAIRA_RELEASE_EXTERNAL_SIGNER_PATH"
  require_external_release_authority_file \
    "Taira release raw Ed25519 public key" \
    "$TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH"
  require_external_release_authority_file \
    "Taira pinned native release-manifest verifier" \
    "$TAIRA_RELEASE_MANIFEST_VERIFIER_PATH"
  require_canonical_release_digest() {
    local label="$1"
    local digest="$2"
    if [[ ! "$digest" =~ ^[0-9a-f]{64}$ ]]; then
      echo "$label must be exactly 64 lowercase hexadecimal characters" >&2
      exit 1
    fi
  }
  require_canonical_release_digest \
    "Taira trusted release signing fingerprint" \
    "$TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT"
  require_canonical_release_digest \
    "Taira trusted native release-manifest verifier SHA-256" \
    "$TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256"
fi

python3 - "${SCRIPT_DIR}/config.toml" <<'PY'
import sys

try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError as error:
        raise SystemExit(
            "python3 must provide tomllib (Python 3.11+) or tomli to validate Taira NTS policy"
        ) from error

config_path = sys.argv[1]
try:
    with open(config_path, "rb") as handle:
        config = tomllib.load(handle)
except (OSError, tomllib.TOMLDecodeError) as error:
    raise SystemExit(f"failed to load canonical Taira config {config_path}: {error}") from error

expected = {
    "sample_interval_ms": 5_000,
    "sample_cap_per_round": 8,
    "max_rtt_ms": 500,
    "trim_percent": 10,
    "per_peer_buffer": 16,
    "smoothing_enabled": False,
    "smoothing_alpha": 0.2,
    "max_adjust_ms_per_min": 50,
    "min_samples": 3,
    "max_offset_ms": 1_000,
    "max_confidence_ms": 500,
    "enforcement_mode": "reject",
}
nts = config.get("nts")
if not isinstance(nts, dict):
    raise SystemExit("canonical Taira config must contain the explicit [nts] release policy")
if set(nts) != set(expected):
    raise SystemExit("canonical Taira [nts] release policy has missing or unknown fields")
for field, expected_value in expected.items():
    actual = nts[field]
    if type(actual) is not type(expected_value) or actual != expected_value:
        raise SystemExit(
            f"canonical Taira [nts].{field} must be exactly {expected_value!r}, got {actual!r}"
        )
PY

if [[ -z "$KAGEMUSHA_RELEASE_POLICY" || ! -f "$KAGEMUSHA_RELEASE_POLICY" || -L "$KAGEMUSHA_RELEASE_POLICY" ]]; then
  echo "an authenticated regular Kagemusha release policy is mandatory; set --kagemusha-release-policy" >&2
  exit 1
fi
if [[ -z "$KAGEMUSHA_ARTIFACT_ROOT" || ! -d "$KAGEMUSHA_ARTIFACT_ROOT" || -L "$KAGEMUSHA_ARTIFACT_ROOT" ]]; then
  echo "an authenticated Kagemusha artifact root is mandatory; set --kagemusha-artifact-root" >&2
  exit 1
fi
if [[ -z "$(find "$KAGEMUSHA_ARTIFACT_ROOT" -mindepth 1 -maxdepth 2 -type f -print -quit)" ]]; then
  echo "Kagemusha artifact root contains no release material: $KAGEMUSHA_ARTIFACT_ROOT" >&2
  exit 1
fi

KAGEMUSHA_V4_RELEASE_POLICY_PATH="$KAGEMUSHA_RELEASE_POLICY" \
KAGEMUSHA_V4_ARTIFACT_ROOT="$KAGEMUSHA_ARTIFACT_ROOT" \
  "${REPO_ROOT}/ci/check_kagemusha_production_readiness.sh" promotion

sha256_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  else
    echo "missing shasum/sha256sum for checksum generation" >&2
    exit 1
  fi
}

require_canonical_sha256() {
  local label="$1"
  local digest="$2"
  if [[ ! "$digest" =~ ^[0-9a-f]{64}$ ]]; then
    echo "$label is not one canonical lowercase SHA-256 digest: $digest" >&2
    exit 1
  fi
}

compute_workspace_source_manifest() {
  python3 -I -S "$WORKSPACE_SOURCE_MANIFEST_SCRIPT" --root "$REPO_ROOT"
}

assert_workspace_source_manifest_unchanged() {
  local phase="$1"
  local current_manifest
  current_manifest="$(compute_workspace_source_manifest)"
  require_canonical_sha256 "workspace source manifest at ${phase}" "$current_manifest"
  if [[ "$current_manifest" != "$workspace_source_manifest_sha256" ]]; then
    echo "workspace source changed after the release identity was frozen (${phase})" >&2
    echo "expected: $workspace_source_manifest_sha256" >&2
    echo "actual:   $current_manifest" >&2
    exit 1
  fi
}

validator_lock_path="${REPO_ROOT}/Cargo.lock"
validator_lock_expected_sha="${IROHA_VALIDATOR_LOCK_SHA256:-}"
validator_build_provenance="${IROHA_VALIDATOR_BUILD_PROVENANCE:-}"
if [[ ! "$validator_lock_expected_sha" =~ ^[0-9a-f]{64}$ ]]; then
  echo "IROHA_VALIDATOR_LOCK_SHA256 must contain the reviewed 64-character checksum" >&2
  echo "Use dpn-api-rust/ops/taira/build-validator-bundle.sh instead of invoking this builder directly." >&2
  exit 1
fi
if [[ ! -f "$validator_lock_path" || -L "$validator_lock_path" ]]; then
  echo "reviewed validator Cargo.lock is missing or not a regular file: $validator_lock_path" >&2
  exit 1
fi
validator_lock_actual_sha="$(sha256_file "$validator_lock_path")"
if [[ "$validator_lock_actual_sha" != "$validator_lock_expected_sha" ]]; then
  echo "validator Cargo.lock checksum mismatch" >&2
  echo "expected: $validator_lock_expected_sha" >&2
  echo "actual:   $validator_lock_actual_sha" >&2
  exit 1
fi
if [[ -z "$validator_build_provenance" || ! -f "$validator_build_provenance" || -L "$validator_build_provenance" ]]; then
  echo "IROHA_VALIDATOR_BUILD_PROVENANCE must name the verified DPN build provenance file" >&2
  exit 1
fi

case "$OUTPUT_DIR" in
  /*)
    ;;
  *)
    OUTPUT_DIR="$(pwd -P)/${OUTPUT_DIR}"
    ;;
esac
mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd -P)"

git_head="$(git -C "$REPO_ROOT" rev-parse HEAD)"
git_status="$(git -C "$REPO_ROOT" status --short)"
if ! git -C "$REPO_ROOT" verify-commit "$git_head" >/dev/null 2>&1; then
  echo "refusing to build Taira rollout bundle from an unsigned or unverifiable Git commit: $git_head" >&2
  exit 1
fi

source_validation="$(python3 - "$validator_build_provenance" "$validator_lock_actual_sha" "$git_head" "$git_status" "$ALLOW_DIRTY" <<'PY'
import json
import re
import sys

path, expected_lock_sha, expected_head, status, allow_dirty = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    payload = json.load(stream)
if payload.get("schema_version") != 1:
    raise SystemExit("validator build provenance has an unsupported schema")
if payload.get("validator_lock_sha256") != expected_lock_sha:
    raise SystemExit("validator build provenance lock checksum does not match Cargo.lock")
if payload.get("iroha_git_head") != expected_head:
    raise SystemExit("validator build provenance Git HEAD does not match the source checkout")
worktree_clean = not bool(status)
if payload.get("iroha_worktree_clean") is not worktree_clean:
    raise SystemExit("validator build provenance cleanliness does not match the source checkout")
if worktree_clean:
    print("clean - - -")
elif payload.get("iroha_source_attested") is True:
    source_tree = payload.get("iroha_source_tree_sha256", "")
    tracked_patch = payload.get("iroha_tracked_patch_sha256", "")
    bundle_provenance = payload.get("iroha_source_bundle_provenance_sha256", "")
    if not all(
        re.fullmatch(r"[0-9a-f]{64}", value)
        for value in (source_tree, tracked_patch, bundle_provenance)
    ):
        raise SystemExit("attested validator source provenance has invalid digests")
    print("attested", source_tree, tracked_patch, bundle_provenance)
elif allow_dirty == "1":
    print("debug-dirty - - -")
else:
    raise SystemExit("dirty validator source is neither clean nor the exact attested release patch")
PY
)"
reference_validator_source_mode=""
reference_source_tree_sha=""
reference_tracked_patch_sha=""
reference_source_bundle_provenance_sha=""
read -r reference_validator_source_mode reference_source_tree_sha reference_tracked_patch_sha reference_source_bundle_provenance_sha <<<"$source_validation"

validator_source_bundle="${IROHA_VALIDATOR_SOURCE_BUNDLE_DIR:-}"
validator_source_verifier="${IROHA_VALIDATOR_SOURCE_VERIFIER:-}"
if [[ "$reference_validator_source_mode" == "attested" ]]; then
  if [[ -z "$validator_source_bundle" || ! -d "$validator_source_bundle" || -L "$validator_source_bundle" ]]; then
    echo "attested validator release requires IROHA_VALIDATOR_SOURCE_BUNDLE_DIR" >&2
    exit 1
  fi
  if [[ -z "$validator_source_verifier" || ! -f "$validator_source_verifier" || -L "$validator_source_verifier" ]]; then
    echo "attested validator release requires IROHA_VALIDATOR_SOURCE_VERIFIER" >&2
    exit 1
  fi
  python3 "$validator_source_verifier" verify --repo "$REPO_ROOT" --bundle-dir "$validator_source_bundle"
fi

for release_input in \
  "$WORKSPACE_SOURCE_MANIFEST_SCRIPT" \
  "$TAIRA_RELEASE_AUTHORITY_SCRIPT" \
  "$RELEASE_ARTIFACT_CONTRACT_SCRIPT" \
  "$RELEASE_MANIFEST_GENERATOR" \
  "$RELEASE_MANIFEST_SIGNING_HELPER" \
  "$RELEASE_CHECKSUM_WRITER" \
  "$PRIVACY_EXACT12_MATRIX" \
  "$PRIVACY_EXPECTATIONS_NORITO" \
  "$PRIVACY_EXPECTATIONS_JSON"; do
  if [[ ! -f "$release_input" || -L "$release_input" ]]; then
    echo "native privacy release input is missing or not a regular file: $release_input" >&2
    exit 1
  fi
done

if [[ "$PROFILE" == "release" ]]; then
  release_verifier_actual_sha256="$(
    sha256_file "$TAIRA_RELEASE_MANIFEST_VERIFIER_PATH"
  )"
  if [[ "$release_verifier_actual_sha256" != "$TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" ]]; then
    echo "Taira native release-manifest verifier does not match its reviewed SHA-256" >&2
    echo "expected: $TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" >&2
    echo "actual:   $release_verifier_actual_sha256" >&2
    exit 1
  fi
fi

workspace_source_manifest_sha256="$(compute_workspace_source_manifest)"
require_canonical_sha256 "pre-build workspace source manifest" "$workspace_source_manifest_sha256"
if [[ "$(sha256_file "$validator_lock_path")" != "$validator_lock_actual_sha" ]]; then
  echo "validator Cargo.lock changed after its release checksum was verified" >&2
  exit 1
fi

irohad_core_feature_graph="$(
  cd "$REPO_ROOT"
  cargo tree --locked -e features,no-dev -p irohad \
    --features "$IROHAD_RELEASE_FEATURES" -i iroha_core
)"
if [[ "$irohad_core_feature_graph" == *'iroha-core-tests'* \
  || "$irohad_core_feature_graph" == *'finality-test-fixtures'* ]]; then
  echo "refusing to build validator with finality test-fixture capabilities" >&2
  printf '%s\n' "$irohad_core_feature_graph" >&2
  exit 1
fi
if [[ "$irohad_core_feature_graph" != *'irohad feature "zk-stark"'* \
  || "$irohad_core_feature_graph" != *'iroha_core feature "zk-stark"'* ]]; then
  echo "refusing to build a Taira validator whose exact feature graph omits zk-stark" >&2
  printf '%s\n' "$irohad_core_feature_graph" >&2
  exit 1
fi
if [[ "$irohad_core_feature_graph" == *"$PRIVACY_RELEASE_EVIDENCE_FEATURE"* ]]; then
  echo "refusing to compile the native privacy evidence feature into irohad" >&2
  printf '%s\n' "$irohad_core_feature_graph" >&2
  exit 1
fi

privacy_runner_core_feature_graph="$(
  cd "$REPO_ROOT"
  cargo tree --locked -e features,no-dev \
    -p "$PRIVACY_RELEASE_RUNNER_PACKAGE" \
    --features "$PRIVACY_RELEASE_EVIDENCE_FEATURE" \
    -i iroha_core
)"
if [[ "$privacy_runner_core_feature_graph" != *"$PRIVACY_RELEASE_RUNNER_PACKAGE feature \"$PRIVACY_RELEASE_EVIDENCE_FEATURE\""* \
  || "$privacy_runner_core_feature_graph" != *"iroha_core feature \"$PRIVACY_RELEASE_EVIDENCE_FEATURE\""* ]]; then
  echo "native privacy runner feature graph does not enable the isolated evidence feature" >&2
  printf '%s\n' "$privacy_runner_core_feature_graph" >&2
  exit 1
fi

if [[ $SKIP_LOCAL_REGRESSIONS -ne 1 ]]; then
  (
    cd "$REPO_ROOT"
    cargo test --locked -p iroha_core queue::router::tests::smart_contract_deploy_rule --lib
    cargo test --locked -p iroha_core call_contract_syscall_preserves_root_and_nested_transfer_authorities_in_artifacts --lib
    cargo test --locked -p iroha_core snapshot_hash_reconcile_extends_verified_local_snapshot_ahead_of_kura --lib
    cargo test --locked -p iroha_core snapshot_read_extends_verified_local_snapshot_ahead_of_kura --lib
  )
fi

timestamp="$(env TZ=UTC date '+%Y%m%dT%H%M%SZ')"
bundle_name="taira-rollout-${timestamp}-${git_head:0:12}-${PROFILE}"
bundle_dir="${OUTPUT_DIR}/${bundle_name}"
archive_path="${OUTPUT_DIR}/${bundle_name}.tar.gz"
binary_dir="${REPO_ROOT}/target/${PROFILE}"

mkdir -p "$bundle_dir/bin" "$bundle_dir/configs/soranexus" "$bundle_dir/scripts" "$bundle_dir/provenance" "$bundle_dir/kagemusha/v4"

if [[ $SKIP_BUILD -ne 1 ]]; then
  core_build_args=(
    build
    --locked
    -p irohad
    -p iroha_cli
    --bin irohad
    --bin iroha
    --features "$IROHAD_RELEASE_FEATURES"
  )
  sorafs_build_args=(build --locked -p sorafs_car --features cli --bin sorafs_manifest_builder --bin sorafs_tx_stdin_builder)
  if [[ "$PROFILE" == "release" ]]; then
    core_build_args+=(--release)
    sorafs_build_args+=(--release)
  fi
  (
    cd "$REPO_ROOT"
    cargo "${core_build_args[@]}"
    cargo "${sorafs_build_args[@]}"
  )

  privacy_runner_build_args=(
    build
    --locked
    -p "$PRIVACY_RELEASE_RUNNER_PACKAGE"
    --bin "$PRIVACY_RELEASE_RUNNER_BIN"
    --features "$PRIVACY_RELEASE_EVIDENCE_FEATURE"
  )
  if [[ "$PROFILE" == "release" ]]; then
    privacy_runner_build_args+=(--release)
  fi
  (
    cd "$REPO_ROOT"
    cargo "${privacy_runner_build_args[@]}"
  )
fi

if [[ "$reference_validator_source_mode" == "attested" ]]; then
  python3 "$validator_source_verifier" verify --repo "$REPO_ROOT" --bundle-dir "$validator_source_bundle"
fi

assert_workspace_source_manifest_unchanged "post-build"
if [[ "$(sha256_file "$validator_lock_path")" != "$validator_lock_actual_sha" ]]; then
  echo "validator Cargo.lock changed during the binary builds" >&2
  exit 1
fi

for binary in irohad iroha sorafs_manifest_builder sorafs_tx_stdin_builder "$PRIVACY_RELEASE_RUNNER_BIN"; do
  if [[ ! -x "${binary_dir}/${binary}" ]]; then
    echo "missing built binary: ${binary_dir}/${binary}" >&2
    echo "run without --skip-build or build the ${PROFILE} profile first" >&2
    exit 1
  fi
done

privacy_evidence_tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/taira-privacy-native-release.XXXXXX")"
trap 'rm -rf -- "$privacy_evidence_tmp_dir"' EXIT
privacy_command_manifest_norito_tmp="${privacy_evidence_tmp_dir}/command-manifest-v1.norito"
privacy_command_manifest_json_tmp="${privacy_evidence_tmp_dir}/command-manifest-v1.json"
privacy_stage_artifacts_norito_tmp="${privacy_evidence_tmp_dir}/stage-artifacts-v1.norito"
privacy_stage_artifacts_json_tmp="${privacy_evidence_tmp_dir}/stage-artifacts-v1.json"
privacy_receipt_norito_tmp="${privacy_evidence_tmp_dir}/receipt-v1.norito"
privacy_receipt_json_tmp="${privacy_evidence_tmp_dir}/receipt-v1.json"
privacy_runner_path="${binary_dir}/${PRIVACY_RELEASE_RUNNER_BIN}"
privacy_runner_common_args=(
  --build-profile "$PROFILE"
  --source-sha256 "$workspace_source_manifest_sha256"
  --exact12-matrix "$PRIVACY_EXACT12_MATRIX"
  --expectations-norito "$PRIVACY_EXPECTATIONS_NORITO"
  --expectations-json "$PRIVACY_EXPECTATIONS_JSON"
  --cargo-lock "$validator_lock_path"
  --validator-binary "${binary_dir}/irohad"
)
"$privacy_runner_path" generate \
  "${privacy_runner_common_args[@]}" \
  --command-manifest-norito-out "$privacy_command_manifest_norito_tmp" \
  --command-manifest-json-out "$privacy_command_manifest_json_tmp" \
  --stage-artifacts-norito-out "$privacy_stage_artifacts_norito_tmp" \
  --stage-artifacts-json-out "$privacy_stage_artifacts_json_tmp" \
  --receipt-norito-out "$privacy_receipt_norito_tmp" \
  --receipt-json-out "$privacy_receipt_json_tmp"

for evidence_path in \
  "$privacy_command_manifest_norito_tmp" \
  "$privacy_command_manifest_json_tmp" \
  "$privacy_stage_artifacts_norito_tmp" \
  "$privacy_stage_artifacts_json_tmp" \
  "$privacy_receipt_norito_tmp" \
  "$privacy_receipt_json_tmp"; do
  if [[ ! -s "$evidence_path" || -L "$evidence_path" ]]; then
    echo "native privacy runner emitted no regular evidence artifact: $evidence_path" >&2
    exit 1
  fi
done

"$privacy_runner_path" verify \
  "${privacy_runner_common_args[@]}" \
  --command-manifest-norito "$privacy_command_manifest_norito_tmp" \
  --command-manifest-json "$privacy_command_manifest_json_tmp" \
  --stage-artifacts-norito "$privacy_stage_artifacts_norito_tmp" \
  --stage-artifacts-json "$privacy_stage_artifacts_json_tmp" \
  --receipt-norito "$privacy_receipt_norito_tmp" \
  --receipt-json "$privacy_receipt_json_tmp"

assert_workspace_source_manifest_unchanged "post-evidence"
if [[ "$(sha256_file "$validator_lock_path")" != "$validator_lock_actual_sha" ]]; then
  echo "validator Cargo.lock changed while native privacy evidence was generated" >&2
  exit 1
fi

for binary in irohad iroha sorafs_manifest_builder sorafs_tx_stdin_builder "$PRIVACY_RELEASE_RUNNER_BIN"; do
  cp "${binary_dir}/${binary}" "${bundle_dir}/bin/${binary}"
done

cp -R "${REPO_ROOT}/configs/soranexus/taira" "${bundle_dir}/configs/soranexus/"
cp "${REPO_ROOT}/scripts/render_taira_validator_bundle.py" "${bundle_dir}/scripts/"
cp "${REPO_ROOT}/scripts/render_taira_edge_nginx_conf.py" "${bundle_dir}/scripts/"
cp "${REPO_ROOT}/scripts/taira_faucet_canary.py" "${bundle_dir}/scripts/"
cp "$TAIRA_RELEASE_AUTHORITY_SCRIPT" "${bundle_dir}/scripts/"
cp "$RELEASE_ARTIFACT_CONTRACT_SCRIPT" "${bundle_dir}/scripts/"
cp "$validator_lock_path" "${bundle_dir}/provenance/Cargo.lock"
cp "$validator_build_provenance" "${bundle_dir}/provenance/dpn-validator-build.provenance.json"
privacy_native_relative_dir="provenance/privacy-native"
privacy_native_dir="${bundle_dir}/${privacy_native_relative_dir}"
mkdir -p "$privacy_native_dir"
privacy_release_norito_relative_path="${privacy_native_relative_dir}/receipt-v1.norito"
privacy_release_json_relative_path="${privacy_native_relative_dir}/receipt-v1.json"
privacy_stage_artifacts_norito_relative_path="${privacy_native_relative_dir}/stage-artifacts-v1.norito"
privacy_stage_artifacts_json_relative_path="${privacy_native_relative_dir}/stage-artifacts-v1.json"
privacy_command_manifest_norito_relative_path="${privacy_native_relative_dir}/command-manifest-v1.norito"
privacy_command_manifest_json_relative_path="${privacy_native_relative_dir}/command-manifest-v1.json"
privacy_expectations_norito_relative_path="${privacy_native_relative_dir}/expectations-v1.norito"
privacy_expectations_json_relative_path="${privacy_native_relative_dir}/expectations-v1.json"
privacy_exact12_matrix_relative_path="${privacy_native_relative_dir}/exact12-v1.tsv"
privacy_workspace_source_manifest_relative_path="${privacy_native_relative_dir}/workspace-source-manifest.sha256"
cp "$privacy_receipt_norito_tmp" "${bundle_dir}/${privacy_release_norito_relative_path}"
cp "$privacy_receipt_json_tmp" "${bundle_dir}/${privacy_release_json_relative_path}"
cp "$privacy_stage_artifacts_norito_tmp" "${bundle_dir}/${privacy_stage_artifacts_norito_relative_path}"
cp "$privacy_stage_artifacts_json_tmp" "${bundle_dir}/${privacy_stage_artifacts_json_relative_path}"
cp "$privacy_command_manifest_norito_tmp" "${bundle_dir}/${privacy_command_manifest_norito_relative_path}"
cp "$privacy_command_manifest_json_tmp" "${bundle_dir}/${privacy_command_manifest_json_relative_path}"
cp "$PRIVACY_EXPECTATIONS_NORITO" "${bundle_dir}/${privacy_expectations_norito_relative_path}"
cp "$PRIVACY_EXPECTATIONS_JSON" "${bundle_dir}/${privacy_expectations_json_relative_path}"
cp "$PRIVACY_EXACT12_MATRIX" "${bundle_dir}/${privacy_exact12_matrix_relative_path}"
printf '%s\n' "$workspace_source_manifest_sha256" \
  >"${bundle_dir}/${privacy_workspace_source_manifest_relative_path}"

privacy_release_norito_sha256="$(sha256_file "${bundle_dir}/${privacy_release_norito_relative_path}")"
privacy_release_json_sha256="$(sha256_file "${bundle_dir}/${privacy_release_json_relative_path}")"
privacy_stage_artifacts_norito_sha256="$(sha256_file "${bundle_dir}/${privacy_stage_artifacts_norito_relative_path}")"
privacy_stage_artifacts_json_sha256="$(sha256_file "${bundle_dir}/${privacy_stage_artifacts_json_relative_path}")"
privacy_command_manifest_norito_sha256="$(sha256_file "${bundle_dir}/${privacy_command_manifest_norito_relative_path}")"
privacy_command_manifest_json_sha256="$(sha256_file "${bundle_dir}/${privacy_command_manifest_json_relative_path}")"
privacy_expectations_norito_sha256="$(sha256_file "${bundle_dir}/${privacy_expectations_norito_relative_path}")"
privacy_expectations_json_sha256="$(sha256_file "${bundle_dir}/${privacy_expectations_json_relative_path}")"
privacy_exact12_matrix_sha256="$(sha256_file "${bundle_dir}/${privacy_exact12_matrix_relative_path}")"
privacy_workspace_source_manifest_file_sha256="$(sha256_file "${bundle_dir}/${privacy_workspace_source_manifest_relative_path}")"
validator_binary_sha256="$(sha256_file "${bundle_dir}/bin/irohad")"
privacy_runner_binary_sha256="$(sha256_file "${bundle_dir}/bin/${PRIVACY_RELEASE_RUNNER_BIN}")"

bundled_privacy_runner_common_args=(
  --build-profile "$PROFILE"
  --source-sha256 "$workspace_source_manifest_sha256"
  --exact12-matrix "${bundle_dir}/${privacy_exact12_matrix_relative_path}"
  --expectations-norito "${bundle_dir}/${privacy_expectations_norito_relative_path}"
  --expectations-json "${bundle_dir}/${privacy_expectations_json_relative_path}"
  --cargo-lock "${bundle_dir}/provenance/Cargo.lock"
  --validator-binary "${bundle_dir}/bin/irohad"
)
"${bundle_dir}/bin/${PRIVACY_RELEASE_RUNNER_BIN}" verify \
  "${bundled_privacy_runner_common_args[@]}" \
  --command-manifest-norito "${bundle_dir}/${privacy_command_manifest_norito_relative_path}" \
  --command-manifest-json "${bundle_dir}/${privacy_command_manifest_json_relative_path}" \
  --stage-artifacts-norito "${bundle_dir}/${privacy_stage_artifacts_norito_relative_path}" \
  --stage-artifacts-json "${bundle_dir}/${privacy_stage_artifacts_json_relative_path}" \
  --receipt-norito "${bundle_dir}/${privacy_release_norito_relative_path}" \
  --receipt-json "${bundle_dir}/${privacy_release_json_relative_path}"

assert_workspace_source_manifest_unchanged "post-bundled-runner-verification"
cp "$KAGEMUSHA_RELEASE_POLICY" "${bundle_dir}/kagemusha/release-policy.norito"
cp -R "$KAGEMUSHA_ARTIFACT_ROOT"/. "${bundle_dir}/kagemusha/v4/"
if [[ "$reference_validator_source_mode" == "attested" ]]; then
  mkdir -p "${bundle_dir}/provenance/source-bundle"
  for component in provenance.json tracked.patch untracked.tar untracked.manifest.json source.manifest.json; do
    if [[ ! -f "${validator_source_bundle}/${component}" || -L "${validator_source_bundle}/${component}" ]]; then
      echo "attested source bundle component is missing or not regular: $component" >&2
      exit 1
    fi
    cp "${validator_source_bundle}/${component}" "${bundle_dir}/provenance/source-bundle/${component}"
  done
fi
chmod 755 "${bundle_dir}/configs/soranexus/taira/check_inrou_host_prereqs.sh"

manifest_path="${bundle_dir}/rollout.manifest.json"
checksums_path="${bundle_dir}/sha256sums.txt"

if [[ -n "$git_status" ]]; then
  git_tree_clean=false
else
  git_tree_clean=true
fi

GIT_HEAD="$git_head" \
GIT_STATUS="$git_status" \
GIT_TREE_CLEAN="$git_tree_clean" \
GENERATED_AT="$timestamp" \
PROFILE_NAME="$PROFILE" \
BUNDLE_NAME="$bundle_name" \
SKIP_LOCAL_REGRESSIONS="$SKIP_LOCAL_REGRESSIONS" \
VALIDATOR_LOCK_SHA256="$validator_lock_actual_sha" \
VALIDATOR_SOURCE_MODE="$reference_validator_source_mode" \
VALIDATOR_SOURCE_TREE_SHA256="$reference_source_tree_sha" \
VALIDATOR_TRACKED_PATCH_SHA256="$reference_tracked_patch_sha" \
VALIDATOR_SOURCE_BUNDLE_PROVENANCE_SHA256="$reference_source_bundle_provenance_sha" \
IROHAD_RELEASE_FEATURES="$IROHAD_RELEASE_FEATURES" \
PRIVACY_RELEASE_EVIDENCE_FEATURE="$PRIVACY_RELEASE_EVIDENCE_FEATURE" \
PRIVACY_RELEASE_RUNNER_BIN="$PRIVACY_RELEASE_RUNNER_BIN" \
WORKSPACE_SOURCE_MANIFEST_SHA256="$workspace_source_manifest_sha256" \
PRIVACY_RELEASE_NORITO_PATH="$privacy_release_norito_relative_path" \
PRIVACY_RELEASE_NORITO_SHA256="$privacy_release_norito_sha256" \
PRIVACY_RELEASE_JSON_PATH="$privacy_release_json_relative_path" \
PRIVACY_RELEASE_JSON_SHA256="$privacy_release_json_sha256" \
PRIVACY_STAGE_ARTIFACTS_NORITO_PATH="$privacy_stage_artifacts_norito_relative_path" \
PRIVACY_STAGE_ARTIFACTS_NORITO_SHA256="$privacy_stage_artifacts_norito_sha256" \
PRIVACY_STAGE_ARTIFACTS_JSON_PATH="$privacy_stage_artifacts_json_relative_path" \
PRIVACY_STAGE_ARTIFACTS_JSON_SHA256="$privacy_stage_artifacts_json_sha256" \
PRIVACY_COMMAND_MANIFEST_NORITO_PATH="$privacy_command_manifest_norito_relative_path" \
PRIVACY_COMMAND_MANIFEST_NORITO_SHA256="$privacy_command_manifest_norito_sha256" \
PRIVACY_COMMAND_MANIFEST_JSON_PATH="$privacy_command_manifest_json_relative_path" \
PRIVACY_COMMAND_MANIFEST_JSON_SHA256="$privacy_command_manifest_json_sha256" \
PRIVACY_EXPECTATIONS_NORITO_PATH="$privacy_expectations_norito_relative_path" \
PRIVACY_EXPECTATIONS_NORITO_SHA256="$privacy_expectations_norito_sha256" \
PRIVACY_EXPECTATIONS_JSON_PATH="$privacy_expectations_json_relative_path" \
PRIVACY_EXPECTATIONS_JSON_SHA256="$privacy_expectations_json_sha256" \
PRIVACY_EXACT12_MATRIX_PATH="$privacy_exact12_matrix_relative_path" \
PRIVACY_EXACT12_MATRIX_SHA256="$privacy_exact12_matrix_sha256" \
PRIVACY_WORKSPACE_SOURCE_MANIFEST_PATH="$privacy_workspace_source_manifest_relative_path" \
PRIVACY_WORKSPACE_SOURCE_MANIFEST_FILE_SHA256="$privacy_workspace_source_manifest_file_sha256" \
PRIVACY_NATIVE_RELATIVE_DIR="$privacy_native_relative_dir" \
VALIDATOR_BINARY_SHA256="$validator_binary_sha256" \
PRIVACY_RUNNER_BINARY_SHA256="$privacy_runner_binary_sha256" \
python3 - <<'PY' >"$manifest_path"
import json
import os
import re

status = os.environ.get("GIT_STATUS", "")

digest_names = (
    "WORKSPACE_SOURCE_MANIFEST_SHA256",
    "PRIVACY_RELEASE_NORITO_SHA256",
    "PRIVACY_RELEASE_JSON_SHA256",
    "PRIVACY_STAGE_ARTIFACTS_NORITO_SHA256",
    "PRIVACY_STAGE_ARTIFACTS_JSON_SHA256",
    "PRIVACY_COMMAND_MANIFEST_NORITO_SHA256",
    "PRIVACY_COMMAND_MANIFEST_JSON_SHA256",
    "PRIVACY_EXPECTATIONS_NORITO_SHA256",
    "PRIVACY_EXPECTATIONS_JSON_SHA256",
    "PRIVACY_EXACT12_MATRIX_SHA256",
    "PRIVACY_WORKSPACE_SOURCE_MANIFEST_FILE_SHA256",
    "VALIDATOR_BINARY_SHA256",
    "PRIVACY_RUNNER_BINARY_SHA256",
)
for name in digest_names:
    if re.fullmatch(r"[0-9a-f]{64}", os.environ[name]) is None:
        raise SystemExit(f"{name} is not a canonical SHA-256 digest")


def evidence_pair(prefix: str) -> dict[str, object]:
    return {
        "authoritative": {
            "encoding": "norito",
            "path": os.environ[f"{prefix}_NORITO_PATH"],
            "sha256": os.environ[f"{prefix}_NORITO_SHA256"],
        },
        "deterministic_json_projection": {
            "authoritative": False,
            "path": os.environ[f"{prefix}_JSON_PATH"],
            "sha256": os.environ[f"{prefix}_JSON_SHA256"],
            "typed_equal_to_norito": True,
        },
    }


payload = {
    "generated_at": os.environ["GENERATED_AT"],
    "git_head": os.environ["GIT_HEAD"],
    "git_tree_clean": os.environ["GIT_TREE_CLEAN"] == "true",
    "git_status_lines": [line for line in status.splitlines() if line],
    "cargo_profile": os.environ["PROFILE_NAME"],
    "cargo_locked": True,
    "validator_lock_sha256": os.environ["VALIDATOR_LOCK_SHA256"],
    "validator_source_mode": os.environ["VALIDATOR_SOURCE_MODE"],
    "validator_source_tree_sha256": None
    if os.environ["VALIDATOR_SOURCE_TREE_SHA256"] == "-"
    else os.environ["VALIDATOR_SOURCE_TREE_SHA256"],
    "validator_tracked_patch_sha256": None
    if os.environ["VALIDATOR_TRACKED_PATCH_SHA256"] == "-"
    else os.environ["VALIDATOR_TRACKED_PATCH_SHA256"],
    "validator_source_bundle_provenance_sha256": None
    if os.environ["VALIDATOR_SOURCE_BUNDLE_PROVENANCE_SHA256"] == "-"
    else os.environ["VALIDATOR_SOURCE_BUNDLE_PROVENANCE_SHA256"],
    "irohad_features": os.environ["IROHAD_RELEASE_FEATURES"].split(","),
    "workspace_source_manifest_sha256": os.environ[
        "WORKSPACE_SOURCE_MANIFEST_SHA256"
    ],
    "detached_release_authority": {
        "required": os.environ["PROFILE_NAME"] == "release",
        "schema": "iroha.taira.exact12_release_authority",
        "directory_name": os.environ["BUNDLE_NAME"] + ".authority",
        "manifest": "release_manifest.json",
        "signature": "release_manifest.json.sig",
        "raw_public_key": "release_manifest.json.pub",
        "archive_without_authority_is_admissible": False,
    },
    "privacy_native_release_evidence": {
        "phase": "post_build",
        "authoritative_encoding": "norito",
        "workspace_source_manifest": {
            "algorithm": "sha256",
            "digest": os.environ["WORKSPACE_SOURCE_MANIFEST_SHA256"],
            "digest_file": {
                "path": os.environ["PRIVACY_WORKSPACE_SOURCE_MANIFEST_PATH"],
                "sha256": os.environ[
                    "PRIVACY_WORKSPACE_SOURCE_MANIFEST_FILE_SHA256"
                ],
            },
            "toctou_rechecked": True,
        },
        "binary_identities": {
            "validator": {
                "path": "bin/irohad",
                "sha256": os.environ["VALIDATOR_BINARY_SHA256"],
            },
            "evidence_runner": {
                "path": f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
                "sha256": os.environ["PRIVACY_RUNNER_BINARY_SHA256"],
            },
            "also_bound_by_typed_receipt": True,
        },
        "runner": {
            "path": f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
            "sha256": os.environ["PRIVACY_RUNNER_BINARY_SHA256"],
            "feature": os.environ["PRIVACY_RELEASE_EVIDENCE_FEATURE"],
            "feature_isolated_from_irohad": True,
            "bundled_verify_passed": True,
        },
        "receipt": evidence_pair("PRIVACY_RELEASE"),
        "stage_artifacts": {
            **evidence_pair("PRIVACY_STAGE_ARTIFACTS"),
            "fixed_stage_block_count": 48,
            "contains_witnesses": False,
            "contains_canonical_proof_artifacts": True,
        },
        "command_manifest": evidence_pair("PRIVACY_COMMAND_MANIFEST"),
        "expectations": {
            **evidence_pair("PRIVACY_EXPECTATIONS"),
            "peak_rss_and_elapsed_ceilings_enforced": True,
        },
        "exact12_matrix": {
            "path": os.environ["PRIVACY_EXACT12_MATRIX_PATH"],
            "sha256": os.environ["PRIVACY_EXACT12_MATRIX_SHA256"],
        },
    },
    "bundle_name": os.environ["BUNDLE_NAME"],
    "binaries": [
        "bin/irohad",
        "bin/iroha",
        "bin/sorafs_manifest_builder",
        "bin/sorafs_tx_stdin_builder",
        f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
    ],
    "release_checks": [
        {
            "name": "taira_validator_privacy_feature_isolation",
            "commands": [
                "cargo tree --locked -e features,no-dev -p irohad --features "
                + os.environ["IROHAD_RELEASE_FEATURES"]
                + " -i iroha_core",
                "cargo tree --locked -e features,no-dev -p iroha_test_network "
                "--features "
                + os.environ["PRIVACY_RELEASE_EVIDENCE_FEATURE"]
                + " -i iroha_core",
            ],
            "validator_evidence_feature_present": False,
            "runner_evidence_feature_present": True,
            "skipped": False,
        },
        {
            "name": "taira_native_privacy_post_build_release_evidence",
            "command": f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]} verify '
            "<authoritative bundled evidence paths>",
            "phase": "post_build",
            "bundled_runner": True,
            "skipped": False,
        },
        {
            "name": "soraswap_smart_contract_deploy_router_regression",
            "command": "cargo test --locked -p iroha_core queue::router::tests::smart_contract_deploy_rule --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
        {
            "name": "soraswap_three_hop_nested_transfer_canary",
            "command": "cargo test --locked -p iroha_core call_contract_syscall_preserves_root_and_nested_transfer_authorities_in_artifacts --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
        {
            "name": "snapshot_hash_reconcile_extends_verified_local_snapshot_ahead_of_kura",
            "command": "cargo test --locked -p iroha_core snapshot_hash_reconcile_extends_verified_local_snapshot_ahead_of_kura --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
        {
            "name": "snapshot_read_extends_verified_local_snapshot_ahead_of_kura",
            "command": "cargo test --locked -p iroha_core snapshot_read_extends_verified_local_snapshot_ahead_of_kura --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
    ],
    "included_paths": [
        "configs/soranexus/taira/",
        "kagemusha/release-policy.norito",
        "kagemusha/v4/",
        "scripts/render_taira_validator_bundle.py",
        "scripts/render_taira_edge_nginx_conf.py",
        "scripts/taira_faucet_canary.py",
        "scripts/taira_release_authority.py",
        "scripts/release_artifact_contract.py",
        "provenance/Cargo.lock",
        "provenance/dpn-validator-build.provenance.json",
        f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
        os.environ["PRIVACY_NATIVE_RELATIVE_DIR"] + "/",
        *(
            ["provenance/source-bundle/"]
            if os.environ["VALIDATOR_SOURCE_MODE"] == "attested"
            else []
        ),
    ],
    "required_followup": [
        "before installation, rerun the bundled native privacy verifier against provenance/privacy-native; the Norito files are authoritative and all JSON files are mandatory deterministic typed projections",
        "install the native Inrou prerequisites reported by configs/soranexus/taira/check_inrou_host_prereqs.sh or run the CONFIG_PROFILE=taira container image",
        "install the bundled binaries/config on each public Taira validator",
        "render and install the shared-edge nginx config from the same validator roster before public cutover, preferably with "
        "configs/soranexus/taira/install_taira_edge_nginx_conf.sh and local-roster [[soracloud_alias_routes]] entries for dedicated runtime aliases such as solswap-indexer.sora",
        "restart the validator with the shipped taira-irohad.service or equivalent",
        "run configs/soranexus/taira/check_mcp_rollout.sh --public-root https://<public-torii-root> --validator-root <label>=<validator-url> (once per validator) --require-all-validators --offline-asset-definition-id <registered-scale-2-ds-asset-definition-id> --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "
        + os.environ["GIT_HEAD"]
        + " after the node is back, so stale public edges fail before live scenario acceptance",
        "run configs/soranexus/taira/check_sorafs_rollout.sh after the node is back",
        "run configs/soranexus/taira/verify_soraswap_rollout.sh --public-root https://<public-torii-root> --validator-root <label>=<validator-url> (once per validator) --offline-asset-definition-id <registered-scale-2-ds-asset-definition-id> --offline-expected-identity /run/secrets/taira-offline-release-identity.json --expected-git-sha "
        + os.environ["GIT_HEAD"]
        + " with its default local SoraSwap regressions enabled after the node is back",
    ],
}
print(json.dumps(payload, indent=2, sort_keys=True))
PY

(
  cd "$bundle_dir"
  find . -type f \
    ! -name "$(basename "$checksums_path")" \
    ! -name "$(basename "$archive_path")" \
    -print | LC_ALL=C sort | while IFS= read -r relative_path; do
      clean_path="${relative_path#./}"
      printf '%s  %s\n' "$(sha256_file "$clean_path")" "$clean_path"
    done >"$checksums_path"
)

assert_workspace_source_manifest_unchanged "pre-archive"
if [[ "$(sha256_file "$validator_lock_path")" != "$validator_lock_actual_sha" ]]; then
  echo "validator Cargo.lock changed before the rollout archive was created" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
tar -C "$OUTPUT_DIR" -czf "$archive_path" "$bundle_name"
printf '%s  %s\n' "$(sha256_file "$archive_path")" "$(basename "$archive_path")" >"${archive_path}.sha256"
assert_workspace_source_manifest_unchanged "post-archive"

release_authority_dir="${OUTPUT_DIR}/${bundle_name}.authority"
release_authority_artifacts_dir="${release_authority_dir}/artifacts"
release_authority_payload_name="taira-exact12-release-authority-v1.json"
release_authority_manifest="${release_authority_dir}/release_manifest.json"
release_authority_signature="${release_authority_dir}/release_manifest.json.sig"
release_authority_public_key="${release_authority_dir}/release_manifest.json.pub"

if [[ "$PROFILE" == "release" ]]; then
  if [[ -e "$release_authority_dir" || -L "$release_authority_dir" ]]; then
    echo "refusing to replace existing Taira release authority: $release_authority_dir" >&2
    exit 1
  fi
  mkdir -m 0700 "$release_authority_dir"
  mkdir -m 0755 "$release_authority_artifacts_dir"
  install -m 0555 \
    "$TAIRA_RELEASE_MANIFEST_VERIFIER_PATH" \
    "${release_authority_artifacts_dir}/sorafs-validate"
  install -m 0555 \
    "$TAIRA_RELEASE_AUTHORITY_SCRIPT" \
    "${release_authority_artifacts_dir}/taira_release_authority.py"
  install -m 0444 \
    "$RELEASE_ARTIFACT_CONTRACT_SCRIPT" \
    "${release_authority_artifacts_dir}/release_artifact_contract.py"

  python3 -S "$TAIRA_RELEASE_AUTHORITY_SCRIPT" create \
    --evidence-root "$bundle_dir" \
    --commit "$git_head" \
    --signing-fingerprint "$TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT" \
    --native-verifier-sha256 "$TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" \
    --archive "$archive_path" \
    --output "${release_authority_artifacts_dir}/${release_authority_payload_name}"

  python3 -S "$RELEASE_CHECKSUM_WRITER" \
    --artifacts-dir "$release_authority_artifacts_dir" \
    --output "${release_authority_artifacts_dir}/SHA256SUMS" \
    --file "$release_authority_payload_name" \
    --file release_artifact_contract.py \
    --file sorafs-validate \
    --file taira_release_authority.py

  release_source_date_epoch="$(git -C "$REPO_ROOT" show -s --format=%ct "$git_head")"
  release_os_tag="$(uname -s | tr '[:upper:]' '[:lower:]')"
  release_arch_tag="$(uname -m | tr '[:upper:]' '[:lower:]')"
  release_manifest_args=(
    --artifacts-dir "$release_authority_artifacts_dir"
    --version "taira-${workspace_source_manifest_sha256:0:16}"
    --commit "$git_head"
    --source-date-epoch "$release_source_date_epoch"
    --os-tag "$release_os_tag"
    --arch "$release_arch_tag"
    --artifact "iroha3:taira-exact12:release-evidence:json:${release_authority_payload_name}"
    --artifact "iroha3:taira-authority:release-evidence:binary:release_artifact_contract.py"
    --artifact "iroha3:taira-authority:reference-validator:binary:sorafs-validate"
    --artifact "iroha3:taira-authority:release-evidence:binary:taira_release_authority.py"
  )
  python3 -S "$RELEASE_MANIFEST_GENERATOR" \
    "${release_manifest_args[@]}" \
    --output "$release_authority_manifest"

  python3 -S "$RELEASE_MANIFEST_SIGNING_HELPER" sign \
    --manifest "$release_authority_manifest" \
    --external-signer "$TAIRA_RELEASE_EXTERNAL_SIGNER_PATH" \
    --signing-public-key "$TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH" \
    --trusted-signing-fingerprint "$TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT" \
    --signature-output "$release_authority_signature" \
    --public-key-output "$release_authority_public_key" \
    --release-manifest-verifier "$TAIRA_RELEASE_MANIFEST_VERIFIER_PATH" \
    --trusted-release-manifest-verifier-sha256 \
      "$TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256"

  python3 -S "$RELEASE_MANIFEST_SIGNING_HELPER" verify \
    --manifest "$release_authority_manifest" \
    --signature "$release_authority_signature" \
    --public-key "$release_authority_public_key" \
    --trusted-signing-fingerprint "$TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT" \
    --release-manifest-verifier \
      "${release_authority_artifacts_dir}/sorafs-validate" \
    --trusted-release-manifest-verifier-sha256 \
      "$TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256"

  release_authority_manifest_replay="${release_authority_dir}/release_manifest.replay.json"
  python3 -S "$RELEASE_MANIFEST_GENERATOR" \
    "${release_manifest_args[@]}" \
    --output "$release_authority_manifest_replay"
  cmp "$release_authority_manifest" "$release_authority_manifest_replay"
  rm -f -- "$release_authority_manifest_replay"

  python3 -S \
    "${release_authority_artifacts_dir}/taira_release_authority.py" verify \
    --evidence-root "$bundle_dir" \
    --commit "$git_head" \
    --signing-fingerprint "$TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT" \
    --native-verifier-sha256 "$TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256" \
    --archive "$archive_path" \
    --authority \
      "${release_authority_artifacts_dir}/${release_authority_payload_name}"

  assert_workspace_source_manifest_unchanged "post-signed-release-authority"
fi

echo "Taira rollout bundle ready:"
echo "  manifest: $manifest_path"
echo "  checksums: $checksums_path"
echo "  archive: $archive_path"
echo "  archive checksum: ${archive_path}.sha256"
if [[ "$PROFILE" == "release" ]]; then
  echo "  signed authority: $release_authority_dir"
  echo "  signed authority manifest SHA-256: $(sha256_file "$release_authority_manifest")"
fi
