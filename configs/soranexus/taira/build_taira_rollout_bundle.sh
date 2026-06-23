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

The bundle contains:
  - `irohad` and `iroha` from `target/<profile>/`
  - `sorafs_manifest_stub` and `sorafs_tx_stdin_builder` from `target/<profile>/`
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

git_head="$(git -C "$REPO_ROOT" rev-parse HEAD)"
git_status="$(git -C "$REPO_ROOT" status --short)"
if [[ -n "$git_status" && $ALLOW_DIRTY -ne 1 ]]; then
  echo "refusing to build Taira rollout bundle from a dirty worktree" >&2
  printf '%s\n' "$git_status" >&2
  echo "rerun with --allow-dirty only for local debugging" >&2
  exit 1
fi

if [[ $SKIP_LOCAL_REGRESSIONS -ne 1 ]]; then
  (
    cd "$REPO_ROOT"
    cargo test -p iroha_core queue::router::tests::smart_contract_deploy_rule --lib
    cargo test -p iroha_core contract_call_transaction_preserves_three_hop_transfer_authorities --lib
  )
fi

timestamp="$(env TZ=UTC date '+%Y%m%dT%H%M%SZ')"
bundle_name="taira-rollout-${timestamp}-${git_head:0:12}-${PROFILE}"
bundle_dir="${OUTPUT_DIR}/${bundle_name}"
archive_path="${OUTPUT_DIR}/${bundle_name}.tar.gz"
binary_dir="${REPO_ROOT}/target/${PROFILE}"

mkdir -p "$bundle_dir/bin" "$bundle_dir/configs/soranexus" "$bundle_dir/scripts"

if [[ $SKIP_BUILD -ne 1 ]]; then
  core_build_args=(
    build
    -p irohad
    -p iroha_cli
    --bin irohad
    --bin iroha
    --features embedded-soracloud-runtime
  )
  sorafs_build_args=(build -p sorafs_car --features cli --bin sorafs_manifest_stub --bin sorafs_tx_stdin_builder)
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

for binary in irohad iroha sorafs_manifest_stub sorafs_tx_stdin_builder; do
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
    "irohad_features": [
        "embedded-soracloud-runtime",
    ],
    "bundle_name": os.environ["BUNDLE_NAME"],
    "binaries": [
        "bin/irohad",
        "bin/iroha",
        "bin/sorafs_manifest_stub",
        "bin/sorafs_tx_stdin_builder",
    ],
    "prebundle_checks": [
        {
            "name": "soraswap_smart_contract_deploy_router_regression",
            "command": "cargo test -p iroha_core queue::router::tests::smart_contract_deploy_rule --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
        {
            "name": "soraswap_three_hop_nested_transfer_canary",
            "command": "cargo test -p iroha_core contract_call_transaction_preserves_three_hop_transfer_authorities --lib",
            "skipped": os.environ["SKIP_LOCAL_REGRESSIONS"] == "1",
        },
    ],
    "included_paths": [
        "configs/soranexus/taira/",
        "scripts/render_taira_validator_bundle.py",
        "scripts/render_taira_edge_nginx_conf.py",
        "scripts/taira_faucet_canary.py",
    ],
    "required_followup": [
        "install the native Inrou prerequisites reported by configs/soranexus/taira/check_inrou_host_prereqs.sh or run the CONFIG_PROFILE=taira container image",
        "install the bundled binaries/config on each public Taira validator",
        "render and install the shared-edge nginx config from the same validator roster before public cutover, preferably with "
        "configs/soranexus/taira/install_taira_edge_nginx_conf.sh and local-roster [[soracloud_alias_routes]] entries for dedicated runtime aliases such as solswap-indexer.sora",
        "restart the validator with the shipped taira-irohad.service or equivalent",
        "run configs/soranexus/taira/check_sorafs_rollout.sh after the node is back",
        "run configs/soranexus/taira/verify_soraswap_rollout.sh with its default local SoraSwap regressions enabled after the node is back",
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

echo "Taira rollout bundle ready:"
echo "  manifest: $manifest_path"
echo "  checksums: $checksums_path"
echo "  archive: $archive_path"
