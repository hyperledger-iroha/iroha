#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-${REPO_ROOT}/dist/taira-rollout}"
PROFILE="${PROFILE:-release}"
ALLOW_DIRTY=0
SKIP_BUILD=0
SKIP_LOCAL_REGRESSIONS=0
IROHAD_RELEASE_FEATURES="embedded-soracloud-runtime,zk-stark"
PRIVACY_RELEASE_EVIDENCE_FEATURE="privacy-release-evidence"
PRIVACY_RELEASE_RUNNER_PACKAGE="iroha_test_network"
PRIVACY_RELEASE_RUNNER_BIN="taira_privacy_release_runner"
BOOTLE_LANTERN_BROKER_BIN="taira_bootle_lantern_broker"
SOFTWARE_SIGNER_BIN="sorafs_external_software_signer"
KAGAMI_BIN="kagami"
PRIVACY_BOOTSTRAP_PLAN_TEMPLATE="${SCRIPT_DIR}/privacy_bootstrap_plan.json"
PRIVACY_BOOTSTRAP_CONFIG_TEMPLATE="${SCRIPT_DIR}/config.toml"
PRIVACY_BOOTSTRAP_GENESIS_TEMPLATE="${SCRIPT_DIR}/genesis.json"
PRIVACY_BOOTSTRAP_VALIDATOR="${SCRIPT_DIR}/validate_privacy_bootstrap.py"
PRIVACY_ROLLOUT_PLAN="${SCRIPT_DIR}/privacy_rollout_plan_v1.json"
PRIVACY_ROLLOUT_VALIDATOR="${REPO_ROOT}/scripts/taira_privacy_rollout_contract.py"
TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR="${TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR:-}"
PRIVACY_BOOTSTRAP_PLAN="$PRIVACY_BOOTSTRAP_PLAN_TEMPLATE"
PRIVACY_BOOTSTRAP_CONFIG="$PRIVACY_BOOTSTRAP_CONFIG_TEMPLATE"
PRIVACY_BOOTSTRAP_GENESIS="$PRIVACY_BOOTSTRAP_GENESIS_TEMPLATE"
PRIVACY_BOOTSTRAP_BROKER_PUBLIC=""
privacy_release_input_snapshot_dir=""
privacy_composer_tmp_dir=""
privacy_evidence_tmp_dir=""
PRIVACY_EXACT12_MATRIX="${REPO_ROOT}/fixtures/privacy/exact12_v1.tsv"
PRIVACY_EXPECTATIONS_NORITO="${REPO_ROOT}/fixtures/privacy/native_release_expectations_v1.norito"
PRIVACY_EXPECTATIONS_JSON="${REPO_ROOT}/fixtures/privacy/native_release_expectations_v1.json"
PRIVACY_X509_RESOURCE_NORITO="${REPO_ROOT}/fixtures/privacy/zk_x509_native_resource_v1.norito"
PRIVACY_X509_RESOURCE_JSON="${REPO_ROOT}/fixtures/privacy/zk_x509_native_resource_v1.json"
WORKSPACE_SOURCE_MANIFEST_SCRIPT="${REPO_ROOT}/scripts/compute_workspace_source_manifest.py"
TAIRA_RELEASE_AUTHORITY_SCRIPT="${REPO_ROOT}/scripts/taira_release_authority.py"
RELEASE_ARTIFACT_CONTRACT_SCRIPT="${REPO_ROOT}/scripts/release_artifact_contract.py"

usage() {
  cat <<'EOF'
Usage: build_taira_rollout_bundle.sh [--output-dir PATH] [--profile debug|release]
                                     [--allow-dirty] [--skip-build]
                                     [--skip-local-regressions]
                                     [--skip-router-regression]

Build a deterministic unsigned public-Taira rollout bundle from the current `../iroha`
checkout. By default the script refuses to package a dirty worktree so the
result can be tied to one exact git revision. It also runs the focused
`iroha_core` SoraSwap deploy-route router regression and three-hop nested
transfer authority canary before packaging.

Production invocations must enter through
`dpn-api-rust/ops/taira/build-validator-bundle.sh`. That wrapper installs the
reviewed full Cargo lock, verifies its checksum and Rust toolchain, rejects a
dirty source tree, and supplies reviewed build provenance to this script.

The bundle contains:
  - `iroha3d` and `iroha` from `target/<profile>/`
  - the peer-1-only native `taira_bootle_lantern_broker`
  - `sorafs_manifest_builder` and `sorafs_tx_stdin_builder` from `target/<profile>/`
  - the feature-separated `taira_privacy_release_runner`
  - authoritative native-privacy receipt, command-manifest, stage-artifact,
    frozen-expectation, and X.509 native-resource-certificate Norito files
    with deterministic JSON projections
  - the checked-in `configs/soranexus/taira/` operator bundle
  - `scripts/render_taira_validator_bundle.py`
  - `scripts/render_taira_edge_nginx_conf.py`
  - `scripts/taira_faucet_canary.py`
  - `configs/soranexus/taira/check_inrou_host_prereqs.sh`
  - `rollout.manifest.json`
  - `sha256sums.txt`
  - `<bundle>.tar.gz`

`--skip-build` is a debug-only convenience. Release bundles always rebuild
every packaged binary from the exact source tree exercised by the gates.

Native privacy evidence is generated only after the ordinary validator build.
The validator uses the production feature set above; the separate evidence
runner alone is built with `privacy-release-evidence`. The canonical workspace
source manifest is checked before build, after build/evidence, and immediately
before archiving so a pre-build report cannot masquerade as release evidence.

This builder never receives a release signer, signing public key, trusted
fingerprint, or native manifest verifier. Use
`scripts/finalize_taira_rollout_authority.py` after this command completes to
authenticate and sign the immutable archive in a separate process which never
invokes Cargo or a source-built executable.

`TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR` must be an absolute canonical
owner-private staging directory containing exactly the four secret-free files
copied from the protected release input before this builder starts by
`kagami privacy-bootstrap render-taira-release-v1`:
  privacy_bootstrap_plan.json
  config.toml
  genesis.json
  bootle_lantern_broker_public.json
The staging directory is snapshotted again and the files are independently
recomposed with the freshly built native Kagami binary before they can enter
the bundle. Never pass the protected source input path to this process.
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

if [[ "$PROFILE" == "release" && $SKIP_BUILD -eq 1 ]]; then
  echo "refusing --skip-build with --profile release: release binaries must be rebuilt from the exact tested source" >&2
  exit 1
fi
if [[ "$PROFILE" == "release" && $SKIP_LOCAL_REGRESSIONS -eq 1 ]]; then
  echo "refusing --skip-local-regressions with --profile release: every release gate is mandatory" >&2
  exit 1
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Taira privacy release evidence must be built natively on Linux" >&2
  exit 1
fi
case "$(uname -m)" in
  aarch64)
    ;;
  *)
    echo "Taira first-release archive requires native Linux aarch64" >&2
    exit 1
    ;;
esac
if ! command -v readelf >/dev/null 2>&1; then
  echo "readelf is required to attest the static Taira privacy release runner" >&2
  exit 1
fi

cleanup_taira_privacy_temp_dirs() {
  local path
  for path in \
    "${privacy_release_input_snapshot_dir:-}" \
    "${privacy_composer_tmp_dir:-}" \
    "${privacy_evidence_tmp_dir:-}"; do
    if [[ -n "$path" && -d "$path" \
      && "$path" == "${TMPDIR:-/tmp}/taira-privacy-"* ]]; then
      rm -rf -- "$path"
    fi
  done
}
trap cleanup_taira_privacy_temp_dirs EXIT

if [[ "$PROFILE" == "release" ]]; then
  if [[ -z "$TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR" \
    || "$TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR" != /* \
    || ! -d "$TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR" \
    || -L "$TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR" ]]; then
    echo "TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR must be an absolute non-symlink directory" >&2
    exit 1
  fi
  canonical_privacy_release_input_dir="$(
    cd "$TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR" && pwd -P
  )"
  canonical_repo_root="$(cd "$REPO_ROOT" && pwd -P)"
  if [[ "$canonical_privacy_release_input_dir" != "$TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR" ]]; then
    echo "TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR must use its canonical physical path" >&2
    exit 1
  fi
  if [[ "$canonical_privacy_release_input_dir" == "$canonical_repo_root" \
    || "$canonical_privacy_release_input_dir" == "$canonical_repo_root/"* ]]; then
    echo "TAIRA_PRIVACY_RELEASE_INPUT_SNAPSHOT_DIR must be staged outside the Iroha checkout" >&2
    exit 1
  fi
  python3 -I -S - "$canonical_privacy_release_input_dir" <<'PY'
import hashlib
import os
import stat
import sys

info = os.lstat(sys.argv[1])
if (
    not stat.S_ISDIR(info.st_mode)
    or stat.S_ISLNK(info.st_mode)
    or info.st_uid != os.getuid()
    or info.st_gid != os.getgid()
    or stat.S_IMODE(info.st_mode) != 0o700
):
    raise SystemExit(
        "privacy release input snapshot must be owner-held at exact mode 0700"
    )
PY

  privacy_release_input_snapshot_dir="$(
    mktemp -d "${TMPDIR:-/tmp}/taira-privacy-release-input.XXXXXX"
  )"
  chmod 0700 "$privacy_release_input_snapshot_dir"
  python3 -I -S - \
    "$canonical_privacy_release_input_dir" \
    "$privacy_release_input_snapshot_dir" <<'PY'
import os
import stat
import sys

source_path, destination_path = sys.argv[1:]
expected = {
    "bootle_lantern_broker_public.json": 4 * 1024 * 1024,
    "config.toml": 8 * 1024 * 1024,
    "genesis.json": 16 * 1024 * 1024,
    "privacy_bootstrap_plan.json": 8 * 1024 * 1024,
}
flags = os.O_RDONLY | os.O_CLOEXEC
if hasattr(os, "O_NOFOLLOW"):
    flags |= os.O_NOFOLLOW
directory_flags = flags | os.O_DIRECTORY
source_before = os.stat(source_path, follow_symlinks=False)
source_fd = os.open(source_path, directory_flags)
destination_fd = os.open(destination_path, directory_flags)


def directory_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
    )


def file_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


try:
    if directory_identity(source_before) != directory_identity(os.fstat(source_fd)):
        raise SystemExit("privacy release input directory changed while it was opened")
    actual = sorted(os.listdir(source_fd))
    if actual != sorted(expected):
        raise SystemExit(
            "privacy release input directory must contain exactly "
            f"{sorted(expected)}, got {actual}"
        )
    if os.listdir(destination_fd):
        raise SystemExit("privacy release snapshot directory was not created empty")
    captured = {}
    captured_destination = {}
    for name, limit in expected.items():
        before = os.stat(name, dir_fd=source_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid != os.getuid()
            or before.st_gid != os.getgid()
            or stat.S_IMODE(before.st_mode) != 0o600
        ):
            raise SystemExit(
                f"privacy release input snapshot must be owner-held, single-link, and exact mode 0600: {name}"
            )
        if before.st_size <= 0 or before.st_size > limit:
            raise SystemExit(
                f"privacy release input {name} must contain 1..{limit} bytes"
            )
        input_fd = os.open(name, flags, dir_fd=source_fd)
        try:
            opened = os.fstat(input_fd)
            if file_identity(before) != file_identity(opened):
                raise SystemExit(f"privacy release input changed while opening: {name}")
            chunks: list[bytes] = []
            remaining = limit + 1
            while remaining:
                chunk = os.read(input_fd, min(64 * 1024, remaining))
                if not chunk:
                    break
                chunks.append(chunk)
                remaining -= len(chunk)
            payload = b"".join(chunks)
            after = os.fstat(input_fd)
            if file_identity(opened) != file_identity(after):
                raise SystemExit(f"privacy release input changed while reading: {name}")
            if len(payload) != opened.st_size or not payload or len(payload) > limit:
                raise SystemExit(f"privacy release input has an invalid bounded snapshot: {name}")
        finally:
            os.close(input_fd)
        path_after = os.stat(name, dir_fd=source_fd, follow_symlinks=False)
        if file_identity(before) != file_identity(path_after):
            raise SystemExit(f"privacy release input path changed while reading: {name}")
        captured[name] = file_identity(before)
        output_flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
        if hasattr(os, "O_NOFOLLOW"):
            output_flags |= os.O_NOFOLLOW
        output_fd = os.open(name, output_flags, 0o600, dir_fd=destination_fd)
        try:
            offset = 0
            while offset < len(payload):
                offset += os.write(output_fd, payload[offset:])
            os.fsync(output_fd)
            output_info = os.fstat(output_fd)
            if (
                not stat.S_ISREG(output_info.st_mode)
                or output_info.st_nlink != 1
                or output_info.st_uid != os.getuid()
                or output_info.st_gid != os.getgid()
                or stat.S_IMODE(output_info.st_mode) != 0o600
                or output_info.st_size != len(payload)
            ):
                raise SystemExit(f"privacy release resnapshot output is unsafe: {name}")
            captured_destination[name] = (
                file_identity(output_info),
                hashlib.sha256(payload).hexdigest(),
            )
        finally:
            os.close(output_fd)
    os.fsync(destination_fd)
    if sorted(os.listdir(source_fd)) != sorted(expected):
        raise SystemExit("privacy release input inventory changed during snapshot")
    for name, expected_identity in captured.items():
        if file_identity(os.stat(name, dir_fd=source_fd, follow_symlinks=False)) != expected_identity:
            raise SystemExit(f"privacy release input changed after snapshot: {name}")
    if sorted(os.listdir(destination_fd)) != sorted(expected):
        raise SystemExit("privacy release resnapshot inventory changed during copy")
    for name, (expected_identity, expected_digest) in captured_destination.items():
        before = os.stat(name, dir_fd=destination_fd, follow_symlinks=False)
        if file_identity(before) != expected_identity:
            raise SystemExit(f"privacy release resnapshot path changed: {name}")
        output_fd = os.open(name, flags, dir_fd=destination_fd)
        try:
            opened = os.fstat(output_fd)
            if file_identity(opened) != expected_identity:
                raise SystemExit(f"privacy release resnapshot changed while opening: {name}")
            digest = hashlib.sha256()
            remaining = opened.st_size
            while remaining:
                chunk = os.read(output_fd, min(64 * 1024, remaining))
                if not chunk:
                    raise SystemExit(f"privacy release resnapshot was truncated: {name}")
                digest.update(chunk)
                remaining -= len(chunk)
            if os.read(output_fd, 1):
                raise SystemExit(f"privacy release resnapshot grew while reading: {name}")
            if file_identity(os.fstat(output_fd)) != expected_identity:
                raise SystemExit(f"privacy release resnapshot changed while reading: {name}")
        finally:
            os.close(output_fd)
        if digest.hexdigest() != expected_digest:
            raise SystemExit(f"privacy release resnapshot content changed: {name}")
        if file_identity(os.stat(name, dir_fd=destination_fd, follow_symlinks=False)) != expected_identity:
            raise SystemExit(f"privacy release resnapshot path changed after reading: {name}")
    if directory_identity(source_before) != directory_identity(os.fstat(source_fd)):
        raise SystemExit("privacy release input directory changed during snapshot")
    source_after = os.stat(source_path, follow_symlinks=False)
    if directory_identity(source_before) != directory_identity(source_after):
        raise SystemExit("privacy release input directory path changed during snapshot")
finally:
    os.close(destination_fd)
    os.close(source_fd)
PY
  PRIVACY_BOOTSTRAP_PLAN="${privacy_release_input_snapshot_dir}/privacy_bootstrap_plan.json"
  PRIVACY_BOOTSTRAP_CONFIG="${privacy_release_input_snapshot_dir}/config.toml"
  PRIVACY_BOOTSTRAP_GENESIS="${privacy_release_input_snapshot_dir}/genesis.json"
  PRIVACY_BOOTSTRAP_BROKER_PUBLIC="${privacy_release_input_snapshot_dir}/bootle_lantern_broker_public.json"
fi

python3 - "$PRIVACY_BOOTSTRAP_CONFIG" <<'PY'
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
dpn_validator_release_commit="${IROHA_DPN_VALIDATOR_RELEASE_COMMIT:-}"
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
if [[ ! "$dpn_validator_release_commit" =~ ^[0-9a-f]{40}$ ]]; then
  echo "IROHA_DPN_VALIDATOR_RELEASE_COMMIT must contain the exact lowercase 40-hex DPN release commit" >&2
  exit 1
fi
export IROHA_DPN_VALIDATOR_RELEASE_COMMIT="$dpn_validator_release_commit"

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

source_validation="$(python3 - "$validator_build_provenance" "$validator_lock_actual_sha" "$git_head" "$git_status" "$ALLOW_DIRTY" "$dpn_validator_release_commit" <<'PY'
import json
import re
import sys

path, expected_lock_sha, expected_head, status, allow_dirty, expected_dpn_commit = sys.argv[1:]
with open(path, encoding="utf-8") as stream:
    payload = json.load(stream)
if payload.get("schema_version") != 1:
    raise SystemExit("validator build provenance has an unsupported schema")
if payload.get("validator_lock_sha256") != expected_lock_sha:
    raise SystemExit("validator build provenance lock checksum does not match Cargo.lock")
if payload.get("iroha_git_head") != expected_head:
    raise SystemExit("validator build provenance Git HEAD does not match the source checkout")
if payload.get("dpn_validator_release_commit") != expected_dpn_commit:
    raise SystemExit("validator build provenance DPN release commit does not match the pinned release input")
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

release_inputs=(
  "$WORKSPACE_SOURCE_MANIFEST_SCRIPT"
  "$TAIRA_RELEASE_AUTHORITY_SCRIPT"
  "$RELEASE_ARTIFACT_CONTRACT_SCRIPT"
  "$PRIVACY_BOOTSTRAP_PLAN_TEMPLATE"
  "$PRIVACY_BOOTSTRAP_CONFIG_TEMPLATE"
  "$PRIVACY_BOOTSTRAP_GENESIS_TEMPLATE"
  "$PRIVACY_BOOTSTRAP_PLAN"
  "$PRIVACY_BOOTSTRAP_CONFIG"
  "$PRIVACY_BOOTSTRAP_GENESIS"
  "$PRIVACY_BOOTSTRAP_VALIDATOR"
  "$PRIVACY_ROLLOUT_PLAN"
  "$PRIVACY_ROLLOUT_VALIDATOR"
  "$PRIVACY_EXACT12_MATRIX"
  "$PRIVACY_EXPECTATIONS_NORITO"
  "$PRIVACY_EXPECTATIONS_JSON"
  "$PRIVACY_X509_RESOURCE_NORITO"
  "$PRIVACY_X509_RESOURCE_JSON"
)
if [[ "$PROFILE" == "release" ]]; then
  release_inputs+=("$PRIVACY_BOOTSTRAP_BROKER_PUBLIC")
fi
for release_input in "${release_inputs[@]}"; do
  if [[ ! -s "$release_input" || -L "$release_input" ]] \
    || [[ "$(stat -c '%h' "$release_input")" != "1" ]]; then
    echo "native privacy release input is missing or not a regular file: $release_input" >&2
    exit 1
  fi
done

privacy_bootstrap_mode="auto"
if [[ "$PROFILE" == "release" ]]; then
  privacy_bootstrap_mode="release"
fi
privacy_bootstrap_validator_args=(
  --mode "$privacy_bootstrap_mode"
  --plan "$PRIVACY_BOOTSTRAP_PLAN"
  --config "$PRIVACY_BOOTSTRAP_CONFIG"
  --genesis "$PRIVACY_BOOTSTRAP_GENESIS"
  --matrix "$PRIVACY_EXACT12_MATRIX"
)
if [[ "$PROFILE" == "release" ]]; then
  privacy_bootstrap_validator_args+=(
    --broker-public "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC"
  )
fi
python3 "$PRIVACY_BOOTSTRAP_VALIDATOR" \
  "${privacy_bootstrap_validator_args[@]}"
python3 -I -S "$PRIVACY_ROLLOUT_VALIDATOR" verify-plan \
  --plan "$PRIVACY_ROLLOUT_PLAN"

workspace_source_manifest_sha256="$(compute_workspace_source_manifest)"
require_canonical_sha256 "pre-build workspace source manifest" "$workspace_source_manifest_sha256"
python3 -I -S - "$validator_build_provenance" "$workspace_source_manifest_sha256" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    payload = json.load(stream)
if payload.get("workspace_source_manifest_sha256") != sys.argv[2]:
    raise SystemExit(
        "validator build provenance workspace manifest does not match the reconstructed source"
    )
PY
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
bundle_name="taira-rollout-${timestamp}-${git_head:0:12}-${PROFILE}-linux-aarch64"
bundle_dir="${OUTPUT_DIR}/${bundle_name}"
archive_path="${OUTPUT_DIR}/${bundle_name}.tar.gz"
binary_dir="${REPO_ROOT}/target/${PROFILE}"

mkdir -p "$bundle_dir/bin" "$bundle_dir/libexec" "$bundle_dir/configs/soranexus" \
  "$bundle_dir/scripts" "$bundle_dir/provenance" "$bundle_dir/share/iroha/sorafs"

if [[ $SKIP_BUILD -ne 1 ]]; then
  core_build_args=(
    build
    --locked
    -p irohad
    -p iroha_cli
    --bin iroha3d
    --bin iroha
    --bin "$BOOTLE_LANTERN_BROKER_BIN"
    --bin "$SOFTWARE_SIGNER_BIN"
    --features "$IROHAD_RELEASE_FEATURES"
  )
  sorafs_build_args=(build --locked -p sorafs_car --features cli --bin sorafs_manifest_builder --bin sorafs_tx_stdin_builder)
  kagami_build_args=(build --locked -p iroha_kagami --bin "$KAGAMI_BIN")
  if [[ "$PROFILE" == "release" ]]; then
    core_build_args+=(--release)
    sorafs_build_args+=(--release)
    kagami_build_args+=(--release)
  fi
  (
    cd "$REPO_ROOT"
    cargo "${core_build_args[@]}"
    cargo "${sorafs_build_args[@]}"
    if [[ "$PROFILE" == "release" ]]; then
      cargo "${kagami_build_args[@]}"
    fi
  )

  privacy_runner_build_args=(
    rustc
    --locked
    -p "$PRIVACY_RELEASE_RUNNER_PACKAGE"
    --bin "$PRIVACY_RELEASE_RUNNER_BIN"
    --features "$PRIVACY_RELEASE_EVIDENCE_FEATURE"
  )
  if [[ "$PROFILE" == "release" ]]; then
    privacy_runner_build_args+=(--release)
  fi
  privacy_runner_build_args+=(-- -C target-feature=+crt-static)
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

for binary in iroha3d iroha "$BOOTLE_LANTERN_BROKER_BIN" "$SOFTWARE_SIGNER_BIN" sorafs_manifest_builder sorafs_tx_stdin_builder "$PRIVACY_RELEASE_RUNNER_BIN"; do
  if [[ ! -x "${binary_dir}/${binary}" ]]; then
    echo "missing built binary: ${binary_dir}/${binary}" >&2
    echo "run without --skip-build or build the ${PROFILE} profile first" >&2
    exit 1
  fi
done

if [[ "$PROFILE" == "release" ]]; then
  kagami_path="${binary_dir}/${KAGAMI_BIN}"
  if [[ ! -x "$kagami_path" ]]; then
    echo "missing freshly built native privacy release composer: $kagami_path" >&2
    exit 1
  fi
  privacy_composer_tmp_dir="$(
    mktemp -d "${TMPDIR:-/tmp}/taira-privacy-composer.XXXXXX"
  )"
  chmod 0700 "$privacy_composer_tmp_dir"
  activation_instructions="${privacy_composer_tmp_dir}/activation-instructions.json"
  activation_report="${privacy_composer_tmp_dir}/activation-report.json"
  composed_plan="${privacy_composer_tmp_dir}/privacy_bootstrap_plan.json"
  composed_config="${privacy_composer_tmp_dir}/config.toml"
  composed_genesis="${privacy_composer_tmp_dir}/genesis.json"
  composed_broker_public="${privacy_composer_tmp_dir}/bootle_lantern_broker_public.json"

  "$kagami_path" privacy-bootstrap emit-taira-v1 \
    --instructions-output "$activation_instructions" \
    --report-output "$activation_report"
  "$kagami_path" privacy-bootstrap render-taira-release-v1 \
    --activation-instructions "$activation_instructions" \
    --activation-report "$activation_report" \
    --broker-public-export "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC" \
    --plan-template "$PRIVACY_BOOTSTRAP_PLAN_TEMPLATE" \
    --config-template "$PRIVACY_BOOTSTRAP_CONFIG_TEMPLATE" \
    --genesis-template "$PRIVACY_BOOTSTRAP_GENESIS_TEMPLATE" \
    --plan-output "$composed_plan" \
    --config-output "$composed_config" \
    --genesis-output "$composed_genesis" \
    --broker-public-output "$composed_broker_public"

  compare_composed_privacy_input() {
    local label="$1"
    local reviewed="$2"
    local recomposed="$3"
    if ! cmp -s "$reviewed" "$recomposed"; then
      echo "reviewed Taira privacy ${label} differs from native recomposition" >&2
      exit 1
    fi
  }
  compare_composed_privacy_input \
    "plan" "$PRIVACY_BOOTSTRAP_PLAN" "$composed_plan"
  compare_composed_privacy_input \
    "config" "$PRIVACY_BOOTSTRAP_CONFIG" "$composed_config"
  compare_composed_privacy_input \
    "genesis" "$PRIVACY_BOOTSTRAP_GENESIS" "$composed_genesis"
  compare_composed_privacy_input \
    "broker public export" "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC" "$composed_broker_public"

  PRIVACY_BOOTSTRAP_PLAN="$composed_plan"
  PRIVACY_BOOTSTRAP_CONFIG="$composed_config"
  PRIVACY_BOOTSTRAP_GENESIS="$composed_genesis"
  PRIVACY_BOOTSTRAP_BROKER_PUBLIC="$composed_broker_public"
  python3 "$PRIVACY_BOOTSTRAP_VALIDATOR" \
    --mode release \
    --plan "$PRIVACY_BOOTSTRAP_PLAN" \
    --config "$PRIVACY_BOOTSTRAP_CONFIG" \
    --genesis "$PRIVACY_BOOTSTRAP_GENESIS" \
    --matrix "$PRIVACY_EXACT12_MATRIX" \
    --broker-public "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC"
  assert_workspace_source_manifest_unchanged "post-native-privacy-composition"
fi

privacy_runner_path="${binary_dir}/${PRIVACY_RELEASE_RUNNER_BIN}"
if readelf --program-headers --wide "$privacy_runner_path" \
  | grep -E '(^|[[:space:]])INTERP([[:space:]]|$)' >/dev/null; then
  echo "Taira privacy release runner must not contain a PT_INTERP segment" >&2
  exit 1
fi
if readelf --dynamic --wide "$privacy_runner_path" \
  | grep -E '\(NEEDED\)' >/dev/null; then
  echo "Taira privacy release runner must not contain DT_NEEDED entries" >&2
  exit 1
fi

privacy_evidence_tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/taira-privacy-native-release.XXXXXX")"
privacy_command_manifest_norito_tmp="${privacy_evidence_tmp_dir}/command-manifest-v1.norito"
privacy_command_manifest_json_tmp="${privacy_evidence_tmp_dir}/command-manifest-v1.json"
privacy_stage_artifacts_norito_tmp="${privacy_evidence_tmp_dir}/stage-artifacts-v1.norito"
privacy_stage_artifacts_json_tmp="${privacy_evidence_tmp_dir}/stage-artifacts-v1.json"
privacy_receipt_norito_tmp="${privacy_evidence_tmp_dir}/receipt-v1.norito"
privacy_receipt_json_tmp="${privacy_evidence_tmp_dir}/receipt-v1.json"
privacy_runner_common_args=(
  --build-profile "$PROFILE"
  --source-sha256 "$workspace_source_manifest_sha256"
  --exact12-matrix "$PRIVACY_EXACT12_MATRIX"
  --expectations-norito "$PRIVACY_EXPECTATIONS_NORITO"
  --expectations-json "$PRIVACY_EXPECTATIONS_JSON"
  --x509-resource-norito "$PRIVACY_X509_RESOURCE_NORITO"
  --x509-resource-json "$PRIVACY_X509_RESOURCE_JSON"
  --cargo-lock "$validator_lock_path"
  --validator-binary "${binary_dir}/iroha3d"
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

for binary in iroha3d iroha "$BOOTLE_LANTERN_BROKER_BIN" "$SOFTWARE_SIGNER_BIN" sorafs_manifest_builder sorafs_tx_stdin_builder "$PRIVACY_RELEASE_RUNNER_BIN"; do
  cp "${binary_dir}/${binary}" "${bundle_dir}/bin/${binary}"
done
cp "${binary_dir}/${SOFTWARE_SIGNER_BIN}" \
  "${bundle_dir}/libexec/iroha-runtime-provider-broker-v1"
chmod 0755 "${bundle_dir}/bin/${SOFTWARE_SIGNER_BIN}" \
  "${bundle_dir}/libexec/iroha-runtime-provider-broker-v1"
cmp "${bundle_dir}/bin/${SOFTWARE_SIGNER_BIN}" \
  "${bundle_dir}/libexec/iroha-runtime-provider-broker-v1"
"${bundle_dir}/bin/${SOFTWARE_SIGNER_BIN}" --help >/dev/null
"${bundle_dir}/libexec/iroha-runtime-provider-broker-v1" --help >/dev/null

signer_asset_root="${bundle_dir}/share/iroha/sorafs"
mkdir -p "$signer_asset_root/external_software_signer" \
  "$signer_asset_root/runtime_provider_broker"
cp "${REPO_ROOT}/configs/sorafs/external_software_signer/README.md" \
  "$signer_asset_root/external_software_signer/README.md"
cp "${REPO_ROOT}/configs/sorafs/external_software_signer/sorafs-external-software-signer@.service" \
  "$signer_asset_root/external_software_signer/"
cp -R "${REPO_ROOT}/configs/sorafs/external_software_signer/systemd" \
  "$signer_asset_root/external_software_signer/"
cp "${REPO_ROOT}/configs/sorafs/runtime_provider_broker/README.md" \
  "$signer_asset_root/runtime_provider_broker/README.md"
cp -R "${REPO_ROOT}/configs/sorafs/runtime_provider_broker/systemd" \
  "$signer_asset_root/runtime_provider_broker/"

cp -R "${REPO_ROOT}/configs/soranexus/taira" "${bundle_dir}/configs/soranexus/"
cp "${REPO_ROOT}/scripts/render_taira_validator_bundle.py" "${bundle_dir}/scripts/"
cp "${REPO_ROOT}/scripts/render_taira_edge_nginx_conf.py" "${bundle_dir}/scripts/"
cp "${REPO_ROOT}/scripts/taira_faucet_canary.py" "${bundle_dir}/scripts/"
cp "$PRIVACY_ROLLOUT_VALIDATOR" "${bundle_dir}/scripts/"
cp "$TAIRA_RELEASE_AUTHORITY_SCRIPT" "${bundle_dir}/scripts/"
cp "$RELEASE_ARTIFACT_CONTRACT_SCRIPT" "${bundle_dir}/scripts/"
cp "$validator_lock_path" "${bundle_dir}/provenance/Cargo.lock"
cp "$validator_build_provenance" "${bundle_dir}/provenance/dpn-validator-build.provenance.json"
privacy_bootstrap_relative_dir=""
privacy_bootstrap_plan_relative_path=""
privacy_bootstrap_config_relative_path=""
privacy_bootstrap_genesis_relative_path=""
privacy_bootstrap_broker_public_relative_path=""
privacy_bootstrap_plan_sha256="-"
privacy_bootstrap_config_sha256="-"
privacy_bootstrap_genesis_sha256="-"
privacy_bootstrap_broker_public_sha256="-"
if [[ "$PROFILE" == "release" ]]; then
  bundled_taira_dir="${bundle_dir}/configs/soranexus/taira"
  cp "$PRIVACY_BOOTSTRAP_PLAN" "${bundled_taira_dir}/privacy_bootstrap_plan.json"
  cp "$PRIVACY_BOOTSTRAP_CONFIG" "${bundled_taira_dir}/config.toml"
  cp "$PRIVACY_BOOTSTRAP_GENESIS" "${bundled_taira_dir}/genesis.json"

  privacy_bootstrap_relative_dir="provenance/privacy-bootstrap"
  privacy_bootstrap_dir="${bundle_dir}/${privacy_bootstrap_relative_dir}"
  mkdir -m 0755 "$privacy_bootstrap_dir"
  privacy_bootstrap_plan_relative_path="${privacy_bootstrap_relative_dir}/privacy_bootstrap_plan.json"
  privacy_bootstrap_config_relative_path="${privacy_bootstrap_relative_dir}/config.toml"
  privacy_bootstrap_genesis_relative_path="${privacy_bootstrap_relative_dir}/genesis.json"
  privacy_bootstrap_broker_public_relative_path="${privacy_bootstrap_relative_dir}/bootle_lantern_broker_public.json"
  cp "$PRIVACY_BOOTSTRAP_PLAN" "${bundle_dir}/${privacy_bootstrap_plan_relative_path}"
  cp "$PRIVACY_BOOTSTRAP_CONFIG" "${bundle_dir}/${privacy_bootstrap_config_relative_path}"
  cp "$PRIVACY_BOOTSTRAP_GENESIS" "${bundle_dir}/${privacy_bootstrap_genesis_relative_path}"
  cp "$PRIVACY_BOOTSTRAP_BROKER_PUBLIC" \
    "${bundle_dir}/${privacy_bootstrap_broker_public_relative_path}"

  python3 "${bundled_taira_dir}/validate_privacy_bootstrap.py" \
    --mode release \
    --plan "${bundled_taira_dir}/privacy_bootstrap_plan.json" \
    --config "${bundled_taira_dir}/config.toml" \
    --genesis "${bundled_taira_dir}/genesis.json" \
    --matrix "$PRIVACY_EXACT12_MATRIX" \
    --broker-public "${bundle_dir}/${privacy_bootstrap_broker_public_relative_path}"
  cmp "$PRIVACY_ROLLOUT_PLAN" "${bundled_taira_dir}/privacy_rollout_plan_v1.json"
  python3 -I -S "${bundle_dir}/scripts/taira_privacy_rollout_contract.py" verify-plan \
    --plan "${bundled_taira_dir}/privacy_rollout_plan_v1.json"

  privacy_bootstrap_plan_sha256="$(
    sha256_file "${bundle_dir}/${privacy_bootstrap_plan_relative_path}"
  )"
  privacy_bootstrap_config_sha256="$(
    sha256_file "${bundle_dir}/${privacy_bootstrap_config_relative_path}"
  )"
  privacy_bootstrap_genesis_sha256="$(
    sha256_file "${bundle_dir}/${privacy_bootstrap_genesis_relative_path}"
  )"
  privacy_bootstrap_broker_public_sha256="$(
    sha256_file "${bundle_dir}/${privacy_bootstrap_broker_public_relative_path}"
  )"
fi
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
privacy_x509_resource_norito_relative_path="${privacy_native_relative_dir}/zk-x509-resource-v1.norito"
privacy_x509_resource_json_relative_path="${privacy_native_relative_dir}/zk-x509-resource-v1.json"
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
cp "$PRIVACY_X509_RESOURCE_NORITO" "${bundle_dir}/${privacy_x509_resource_norito_relative_path}"
cp "$PRIVACY_X509_RESOURCE_JSON" "${bundle_dir}/${privacy_x509_resource_json_relative_path}"
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
privacy_x509_resource_norito_sha256="$(sha256_file "${bundle_dir}/${privacy_x509_resource_norito_relative_path}")"
privacy_x509_resource_json_sha256="$(sha256_file "${bundle_dir}/${privacy_x509_resource_json_relative_path}")"
privacy_exact12_matrix_sha256="$(sha256_file "${bundle_dir}/${privacy_exact12_matrix_relative_path}")"
privacy_workspace_source_manifest_file_sha256="$(sha256_file "${bundle_dir}/${privacy_workspace_source_manifest_relative_path}")"
validator_binary_sha256="$(sha256_file "${bundle_dir}/bin/iroha3d")"
bootle_lantern_broker_binary_sha256="$(sha256_file "${bundle_dir}/bin/${BOOTLE_LANTERN_BROKER_BIN}")"
privacy_runner_binary_sha256="$(sha256_file "${bundle_dir}/bin/${PRIVACY_RELEASE_RUNNER_BIN}")"
software_signer_binary_sha256="$(sha256_file "${bundle_dir}/bin/${SOFTWARE_SIGNER_BIN}")"
software_signer_broker_alias_sha256="$(sha256_file "${bundle_dir}/libexec/iroha-runtime-provider-broker-v1")"
if [[ "$software_signer_binary_sha256" != "$software_signer_broker_alias_sha256" ]]; then
  echo "external software signer and runtime-provider broker alias differ" >&2
  exit 1
fi

bundled_privacy_runner_common_args=(
  --build-profile "$PROFILE"
  --source-sha256 "$workspace_source_manifest_sha256"
  --exact12-matrix "${bundle_dir}/${privacy_exact12_matrix_relative_path}"
  --expectations-norito "${bundle_dir}/${privacy_expectations_norito_relative_path}"
  --expectations-json "${bundle_dir}/${privacy_expectations_json_relative_path}"
  --x509-resource-norito "${bundle_dir}/${privacy_x509_resource_norito_relative_path}"
  --x509-resource-json "${bundle_dir}/${privacy_x509_resource_json_relative_path}"
  --cargo-lock "${bundle_dir}/provenance/Cargo.lock"
  --validator-binary "${bundle_dir}/bin/iroha3d"
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
DPN_VALIDATOR_RELEASE_COMMIT="$dpn_validator_release_commit" \
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
BOOTLE_LANTERN_BROKER_BIN="$BOOTLE_LANTERN_BROKER_BIN" \
SOFTWARE_SIGNER_BIN="$SOFTWARE_SIGNER_BIN" \
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
PRIVACY_X509_RESOURCE_NORITO_PATH="$privacy_x509_resource_norito_relative_path" \
PRIVACY_X509_RESOURCE_NORITO_SHA256="$privacy_x509_resource_norito_sha256" \
PRIVACY_X509_RESOURCE_JSON_PATH="$privacy_x509_resource_json_relative_path" \
PRIVACY_X509_RESOURCE_JSON_SHA256="$privacy_x509_resource_json_sha256" \
PRIVACY_EXACT12_MATRIX_PATH="$privacy_exact12_matrix_relative_path" \
PRIVACY_EXACT12_MATRIX_SHA256="$privacy_exact12_matrix_sha256" \
PRIVACY_WORKSPACE_SOURCE_MANIFEST_PATH="$privacy_workspace_source_manifest_relative_path" \
PRIVACY_WORKSPACE_SOURCE_MANIFEST_FILE_SHA256="$privacy_workspace_source_manifest_file_sha256" \
PRIVACY_NATIVE_RELATIVE_DIR="$privacy_native_relative_dir" \
PRIVACY_BOOTSTRAP_RELATIVE_DIR="$privacy_bootstrap_relative_dir" \
PRIVACY_BOOTSTRAP_PLAN_PATH="$privacy_bootstrap_plan_relative_path" \
PRIVACY_BOOTSTRAP_PLAN_SHA256="$privacy_bootstrap_plan_sha256" \
PRIVACY_BOOTSTRAP_CONFIG_PATH="$privacy_bootstrap_config_relative_path" \
PRIVACY_BOOTSTRAP_CONFIG_SHA256="$privacy_bootstrap_config_sha256" \
PRIVACY_BOOTSTRAP_GENESIS_PATH="$privacy_bootstrap_genesis_relative_path" \
PRIVACY_BOOTSTRAP_GENESIS_SHA256="$privacy_bootstrap_genesis_sha256" \
PRIVACY_BOOTSTRAP_BROKER_PUBLIC_PATH="$privacy_bootstrap_broker_public_relative_path" \
PRIVACY_BOOTSTRAP_BROKER_PUBLIC_SHA256="$privacy_bootstrap_broker_public_sha256" \
VALIDATOR_BINARY_SHA256="$validator_binary_sha256" \
BOOTLE_LANTERN_BROKER_BINARY_SHA256="$bootle_lantern_broker_binary_sha256" \
PRIVACY_RUNNER_BINARY_SHA256="$privacy_runner_binary_sha256" \
SOFTWARE_SIGNER_BINARY_SHA256="$software_signer_binary_sha256" \
SOFTWARE_SIGNER_BROKER_ALIAS_SHA256="$software_signer_broker_alias_sha256" \
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
    "PRIVACY_X509_RESOURCE_NORITO_SHA256",
    "PRIVACY_X509_RESOURCE_JSON_SHA256",
    "PRIVACY_EXACT12_MATRIX_SHA256",
    "PRIVACY_WORKSPACE_SOURCE_MANIFEST_FILE_SHA256",
    "VALIDATOR_BINARY_SHA256",
    "BOOTLE_LANTERN_BROKER_BINARY_SHA256",
    "PRIVACY_RUNNER_BINARY_SHA256",
    "SOFTWARE_SIGNER_BINARY_SHA256",
    "SOFTWARE_SIGNER_BROKER_ALIAS_SHA256",
)
if os.environ["PROFILE_NAME"] == "release":
    digest_names += (
        "PRIVACY_BOOTSTRAP_PLAN_SHA256",
        "PRIVACY_BOOTSTRAP_CONFIG_SHA256",
        "PRIVACY_BOOTSTRAP_GENESIS_SHA256",
        "PRIVACY_BOOTSTRAP_BROKER_PUBLIC_SHA256",
    )
else:
    for name in (
        "PRIVACY_BOOTSTRAP_PLAN_SHA256",
        "PRIVACY_BOOTSTRAP_CONFIG_SHA256",
        "PRIVACY_BOOTSTRAP_GENESIS_SHA256",
        "PRIVACY_BOOTSTRAP_BROKER_PUBLIC_SHA256",
    ):
        if os.environ[name] != "-":
            raise SystemExit(f"debug bundle unexpectedly carries {name}")
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


privacy_bootstrap_release = None
if os.environ["PROFILE_NAME"] == "release":
    privacy_bootstrap_release = {
        "schema": "iroha.taira.privacy-bootstrap-release-bundle.v1",
        "native_recomposition_passed": True,
        "bundled_release_validation_passed": True,
        "secret_free": True,
        "plan": {
            "path": os.environ["PRIVACY_BOOTSTRAP_PLAN_PATH"],
            "sha256": os.environ["PRIVACY_BOOTSTRAP_PLAN_SHA256"],
            "operator_copy": "configs/soranexus/taira/privacy_bootstrap_plan.json",
        },
        "peer_1_config": {
            "path": os.environ["PRIVACY_BOOTSTRAP_CONFIG_PATH"],
            "sha256": os.environ["PRIVACY_BOOTSTRAP_CONFIG_SHA256"],
            "operator_copy": "configs/soranexus/taira/config.toml",
            "designated_validator": "taira-validator-1",
        },
        "genesis": {
            "path": os.environ["PRIVACY_BOOTSTRAP_GENESIS_PATH"],
            "sha256": os.environ["PRIVACY_BOOTSTRAP_GENESIS_SHA256"],
            "operator_copy": "configs/soranexus/taira/genesis.json",
        },
        "broker_public_export": {
            "path": os.environ["PRIVACY_BOOTSTRAP_BROKER_PUBLIC_PATH"],
            "sha256": os.environ["PRIVACY_BOOTSTRAP_BROKER_PUBLIC_SHA256"],
            "bound_by_plan_sha256": True,
        },
    }


payload = {
    "dpn_validator_release_commit": os.environ["DPN_VALIDATOR_RELEASE_COMMIT"],
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
    "privacy_bootstrap_release": privacy_bootstrap_release,
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
                "path": "bin/iroha3d",
                "sha256": os.environ["VALIDATOR_BINARY_SHA256"],
            },
            "bootle_lantern_broker": {
                "path": f'bin/{os.environ["BOOTLE_LANTERN_BROKER_BIN"]}',
                "sha256": os.environ["BOOTLE_LANTERN_BROKER_BINARY_SHA256"],
                "designated_validator": "taira-validator-1",
                "public_export_path": os.environ[
                    "PRIVACY_BOOTSTRAP_BROKER_PUBLIC_PATH"
                ],
                "public_export_sha256": os.environ[
                    "PRIVACY_BOOTSTRAP_BROKER_PUBLIC_SHA256"
                ],
                "native_composition_bound": os.environ["PROFILE_NAME"] == "release",
            },
            "evidence_runner": {
                "path": f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
                "sha256": os.environ["PRIVACY_RUNNER_BINARY_SHA256"],
            },
            "external_software_signer": {
                "backend": "software",
                "path": f'bin/{os.environ["SOFTWARE_SIGNER_BIN"]}',
                "sha256": os.environ["SOFTWARE_SIGNER_BINARY_SHA256"],
                "broker_alias_path": "libexec/iroha-runtime-provider-broker-v1",
                "broker_alias_sha256": os.environ[
                    "SOFTWARE_SIGNER_BROKER_ALIAS_SHA256"
                ],
                "byte_identical_alias": True,
                "native_help_smoke_passed": True,
                "auto_launch_roles": [
                    "proof-outcome",
                    "repair",
                    "reserve",
                    "orderbook",
                ],
                "promotion_requires_separate_l2_host": True,
                "windows_supported": False,
            },
            "validator_and_evidence_runner_bound_by_typed_receipt": True,
            "broker_binary_bound_by_release_manifest": True,
            "broker_public_export_bound_by_plan_and_release_manifest": (
                os.environ["PROFILE_NAME"] == "release"
            ),
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
        "x509_native_resource_certificate": evidence_pair(
            "PRIVACY_X509_RESOURCE"
        ),
        "exact12_matrix": {
            "path": os.environ["PRIVACY_EXACT12_MATRIX_PATH"],
            "sha256": os.environ["PRIVACY_EXACT12_MATRIX_SHA256"],
        },
    },
    "bundle_name": os.environ["BUNDLE_NAME"],
    "binaries": [
        "bin/iroha3d",
        "bin/iroha",
        f'bin/{os.environ["BOOTLE_LANTERN_BROKER_BIN"]}',
        "bin/sorafs_manifest_builder",
        "bin/sorafs_tx_stdin_builder",
        f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
        f'bin/{os.environ["SOFTWARE_SIGNER_BIN"]}',
        "libexec/iroha-runtime-provider-broker-v1",
    ],
    "release_checks": [
        {
            "name": "taira_native_privacy_release_composition",
            "command": "freshly built kagami privacy-bootstrap render-taira-release-v1 <reviewed public inputs>",
            "fresh_native_recomposition": os.environ["PROFILE_NAME"] == "release",
            "byte_identical_to_reviewed_inputs": os.environ["PROFILE_NAME"] == "release",
            "bundled_release_validation": os.environ["PROFILE_NAME"] == "release",
            "skipped": os.environ["PROFILE_NAME"] != "release",
        },
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
        "scripts/render_taira_validator_bundle.py",
        "scripts/render_taira_edge_nginx_conf.py",
        "scripts/taira_faucet_canary.py",
        "scripts/taira_privacy_rollout_contract.py",
        "scripts/taira_release_authority.py",
        "scripts/release_artifact_contract.py",
        "provenance/Cargo.lock",
        "provenance/dpn-validator-build.provenance.json",
        f'bin/{os.environ["PRIVACY_RELEASE_RUNNER_BIN"]}',
        "share/iroha/sorafs/external_software_signer/",
        "share/iroha/sorafs/runtime_provider_broker/",
        os.environ["PRIVACY_NATIVE_RELATIVE_DIR"] + "/",
        *(
            [os.environ["PRIVACY_BOOTSTRAP_RELATIVE_DIR"] + "/"]
            if os.environ["PROFILE_NAME"] == "release"
            else []
        ),
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
        "install and qualify exactly the four native external-software-signer roles; promotion remains on a separately administered L2 host and is never auto-launched with validators",
        "render and install the shared-edge nginx config from the same validator roster before public cutover, preferably with "
        "configs/soranexus/taira/install_taira_edge_nginx_conf.sh and local-roster [[soracloud_alias_routes]] entries for dedicated runtime aliases such as solswap-indexer.sora",
        "restart the validator with the shipped taira-irohad.service or equivalent",
        "run configs/soranexus/taira/check_mcp_rollout.sh --public-root https://<public-torii-root> --validator-root <label>=<validator-url> (once per validator) --require-all-validators --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "
        + os.environ["GIT_HEAD"]
        + " after the node is back, so stale public edges fail before live scenario acceptance",
        "run configs/soranexus/taira/check_sorafs_rollout.sh after the node is back",
        "run configs/soranexus/taira/verify_soraswap_rollout.sh --public-root https://<public-torii-root> --validator-root <label>=<validator-url> (once per validator) --expected-git-sha "
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

echo "Unsigned Taira rollout bundle ready:"
echo "  manifest: $manifest_path"
echo "  checksums: $checksums_path"
echo "  archive: $archive_path"
echo "  archive checksum: ${archive_path}.sha256"
echo "  next: authenticate with scripts/finalize_taira_rollout_authority.py"
