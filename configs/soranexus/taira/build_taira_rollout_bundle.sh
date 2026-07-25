#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-${REPO_ROOT}/dist/taira-rollout}"
PROFILE="${PROFILE:-release}"
ALLOW_DIRTY=0
SKIP_BUILD=0
SKIP_LOCAL_REGRESSIONS=0

usage() {
  cat <<'EOF'
Usage: build_taira_rollout_bundle.sh [--output-dir PATH] [--profile debug|release]
                                     [--allow-dirty] [--skip-build]
                                     [--skip-local-regressions]
                                     [--skip-router-regression]

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
  - the checked-in `configs/soranexus/taira/` operator bundle
  - `scripts/render_taira_validator_bundle.py`
  - `scripts/render_taira_edge_nginx_conf.py`
  - `scripts/taira_faucet_canary.py`
  - `configs/soranexus/taira/check_inrou_host_prereqs.sh`
  - `rollout.manifest.json`
  - `sha256sums.txt`
  - `<bundle>.tar.gz`
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

irohad_core_feature_graph="$(
  cd "$REPO_ROOT"
  cargo tree --locked -e features,no-dev -p irohad \
    --features embedded-soracloud-runtime -i iroha_core
)"
if [[ "$irohad_core_feature_graph" == *'iroha-core-tests'* \
  || "$irohad_core_feature_graph" == *'finality-test-fixtures'* ]]; then
  echo "refusing to build validator with finality test-fixture capabilities" >&2
  printf '%s\n' "$irohad_core_feature_graph" >&2
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

mkdir -p "$bundle_dir/bin" "$bundle_dir/configs/soranexus" "$bundle_dir/scripts" "$bundle_dir/provenance"

if [[ $SKIP_BUILD -ne 1 ]]; then
  core_build_args=(
    build
    --locked
    -p irohad
    -p iroha_cli
    --bin irohad
    --bin iroha
    --features embedded-soracloud-runtime
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
fi

if [[ "$reference_validator_source_mode" == "attested" ]]; then
  python3 "$validator_source_verifier" verify --repo "$REPO_ROOT" --bundle-dir "$validator_source_bundle"
fi

for binary in irohad iroha sorafs_manifest_builder sorafs_tx_stdin_builder; do
  if [[ ! -x "${binary_dir}/${binary}" ]]; then
    echo "missing built binary: ${binary_dir}/${binary}" >&2
    echo "run without --skip-build or build the ${PROFILE} profile first" >&2
    exit 1
  fi
  cp "${binary_dir}/${binary}" "${bundle_dir}/bin/${binary}"
done

cp -R "${REPO_ROOT}/configs/soranexus/taira" "${bundle_dir}/configs/soranexus/"
cp "${REPO_ROOT}/scripts/render_taira_validator_bundle.py" "${bundle_dir}/scripts/"
cp "${REPO_ROOT}/scripts/render_taira_edge_nginx_conf.py" "${bundle_dir}/scripts/"
cp "${REPO_ROOT}/scripts/taira_faucet_canary.py" "${bundle_dir}/scripts/"
cp "$validator_lock_path" "${bundle_dir}/provenance/Cargo.lock"
cp "$validator_build_provenance" "${bundle_dir}/provenance/dpn-validator-build.provenance.json"
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
REPO_ROOT="$REPO_ROOT" \
SKIP_LOCAL_REGRESSIONS="$SKIP_LOCAL_REGRESSIONS" \
VALIDATOR_LOCK_SHA256="$validator_lock_actual_sha" \
VALIDATOR_SOURCE_MODE="$reference_validator_source_mode" \
VALIDATOR_SOURCE_TREE_SHA256="$reference_source_tree_sha" \
VALIDATOR_TRACKED_PATCH_SHA256="$reference_tracked_patch_sha" \
VALIDATOR_SOURCE_BUNDLE_PROVENANCE_SHA256="$reference_source_bundle_provenance_sha" \
python3 - <<'PY' >"$manifest_path"
import json
import os

status = os.environ.get("GIT_STATUS", "")
payload = {
    "generated_at": os.environ["GENERATED_AT"],
    "repo_root": os.environ["REPO_ROOT"],
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
    "irohad_features": [
        "embedded-soracloud-runtime",
    ],
    "bundle_name": os.environ["BUNDLE_NAME"],
    "binaries": [
        "bin/irohad",
        "bin/iroha",
        "bin/sorafs_manifest_builder",
        "bin/sorafs_tx_stdin_builder",
    ],
    "prebundle_checks": [
        {
            "name": "soraswap_smart_contract_deploy_router_regression",
            "command": "cargo test --locked -p iroha_core queue::router::tests::smart_contract_deploy_rule --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
        {
            "name": "soraswap_three_hop_nested_transfer_canary",
            "command": "cargo test --locked -p iroha_core contract_call_transaction_preserves_three_hop_transfer_authorities --lib",
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
        "scripts/render_taira_validator_bundle.py",
        "scripts/render_taira_edge_nginx_conf.py",
        "scripts/taira_faucet_canary.py",
        "provenance/Cargo.lock",
        "provenance/dpn-validator-build.provenance.json",
        *(
            ["provenance/source-bundle/"]
            if os.environ["VALIDATOR_SOURCE_MODE"] == "attested"
            else []
        ),
    ],
    "required_followup": [
        "install the native Inrou prerequisites reported by configs/soranexus/taira/check_inrou_host_prereqs.sh or run the CONFIG_PROFILE=taira container image",
        "install the bundled binaries/config on each public Taira validator",
        "render and install the shared-edge nginx config from the same validator roster before public cutover, preferably with "
        "configs/soranexus/taira/install_taira_edge_nginx_conf.sh and local-roster [[soracloud_alias_routes]] entries for dedicated runtime aliases such as solswap-indexer.sora",
        "restart the validator with the shipped taira-irohad.service or equivalent",
        "run configs/soranexus/taira/check_mcp_rollout.sh --public-root https://<public-torii-root> --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "
        + os.environ["GIT_HEAD"]
        + " after the node is back, so stale public edges fail before live scenario acceptance",
        "run configs/soranexus/taira/check_sorafs_rollout.sh after the node is back",
        "run configs/soranexus/taira/verify_soraswap_rollout.sh --expected-git-sha "
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

mkdir -p "$OUTPUT_DIR"
tar -C "$OUTPUT_DIR" -czf "$archive_path" "$bundle_name"
printf '%s  %s\n' "$(sha256_file "$archive_path")" "$(basename "$archive_path")" >"${archive_path}.sha256"

echo "Taira rollout bundle ready:"
echo "  manifest: $manifest_path"
echo "  checksums: $checksums_path"
echo "  archive: $archive_path"
echo "  archive checksum: ${archive_path}.sha256"
