#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build_release_image.sh [options]

Required release controls:
  --source-commit <hex>              Reviewed full source commit.
  --source-date-epoch <epoch>        Canonical release SOURCE_DATE_EPOCH.
  --platform <linux/amd64|linux/arm64>
  --builder-base-image <ref@sha256>  Preprovisioned builder base by digest.
  --runtime-base-image <ref@sha256>  Preprovisioned runtime base by digest.
  --docker <path>                    Exact Docker CLI executable.
  --trusted-docker-sha256 <hex>      Reviewed Docker CLI SHA256.
  --buildx-plugin <path>             Exact docker-buildx plugin executable.
  --trusted-buildx-sha256 <hex>      Reviewed buildx plugin SHA256.
  --trusted-buildx-version <text>    Exact reviewed `docker buildx version`.
  --buildx-builder <name>            Reviewed buildx builder instance.
  --trusted-buildx-builder-inspect-sha256 <hex>
                                      Reviewed exact builder inspection SHA256.

Build inputs and outputs:
  --prebuilt-bin-dir <path>          Required reviewed target binaries.
  --features <list>                  Canonical comma-separated Cargo features.
  --binaries "<list>"                Space-separated binary inventory.
  --tag <tag>                        OCI reference annotation.
  --artifacts-dir <dir>              Existing output directory (default: dist).
  --manifest-out <path>              Builder manifest output path.
  -h, --help                         Show this help.

The builder emits an unsigned deterministic OCI archive. Aggregate signing
occurs only after the pipeline closes and verifies the full release inventory.
EOF
}

log() {
  printf '[release-build-image] %s\n' "$*" >&2
}

require_value() {
  local option="$1"
  local value="${2-}"
  if [[ -z "$value" || "$value" == --* ]]; then
    printf '%s requires a value\n' "$option" >&2
    exit 1
  fi
}

safe_token() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$ ]]; then
    printf '%s must be a bounded safe token\n' "$label" >&2
    exit 1
  fi
}

require_sha256() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9a-f]{64}$ ]]; then
    printf '%s must be exactly 64 lowercase hex characters\n' "$label" >&2
    exit 1
  fi
}

require_digest_reference() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[a-z0-9][a-z0-9._:/-]{0,200}@sha256:[0-9a-f]{64}$ ]]; then
    printf '%s must be a bounded lowercase OCI reference with an exact sha256 digest\n' "$label" >&2
    exit 1
  fi
}

profile="iroha3"
config="nexus"
features=""
binaries=""
prebuilt_bin_dir=""
source_commit=""
source_date_epoch=""
platform=""
builder_base_image=""
runtime_base_image=""
docker_path=""
trusted_docker_sha256=""
buildx_plugin=""
trusted_buildx_sha256=""
trusted_buildx_version=""
buildx_builder=""
trusted_buildx_builder_inspect_sha256=""
image_tag=""
artifacts_dir="dist"
manifest_out=""

while (($#)); do
  case "$1" in
    --features)
      require_value "$1" "${2-}"
      features="$2"
      shift 2
      ;;
    --binaries)
      require_value "$1" "${2-}"
      binaries="$2"
      shift 2
      ;;
    --prebuilt-bin-dir)
      require_value "$1" "${2-}"
      prebuilt_bin_dir="$2"
      shift 2
      ;;
    --source-commit)
      require_value "$1" "${2-}"
      source_commit="$2"
      shift 2
      ;;
    --source-date-epoch)
      require_value "$1" "${2-}"
      source_date_epoch="$2"
      shift 2
      ;;
    --platform)
      require_value "$1" "${2-}"
      platform="$2"
      shift 2
      ;;
    --builder-base-image)
      require_value "$1" "${2-}"
      builder_base_image="$2"
      shift 2
      ;;
    --runtime-base-image)
      require_value "$1" "${2-}"
      runtime_base_image="$2"
      shift 2
      ;;
    --docker)
      require_value "$1" "${2-}"
      docker_path="$2"
      shift 2
      ;;
    --trusted-docker-sha256)
      require_value "$1" "${2-}"
      trusted_docker_sha256="$2"
      shift 2
      ;;
    --buildx-plugin)
      require_value "$1" "${2-}"
      buildx_plugin="$2"
      shift 2
      ;;
    --trusted-buildx-sha256)
      require_value "$1" "${2-}"
      trusted_buildx_sha256="$2"
      shift 2
      ;;
    --trusted-buildx-version)
      require_value "$1" "${2-}"
      trusted_buildx_version="$2"
      shift 2
      ;;
    --buildx-builder)
      require_value "$1" "${2-}"
      buildx_builder="$2"
      shift 2
      ;;
    --trusted-buildx-builder-inspect-sha256)
      require_value "$1" "${2-}"
      trusted_buildx_builder_inspect_sha256="$2"
      shift 2
      ;;
    --tag)
      require_value "$1" "${2-}"
      image_tag="$2"
      shift 2
      ;;
    --artifacts-dir)
      require_value "$1" "${2-}"
      artifacts_dir="$2"
      shift 2
      ;;
    --manifest-out)
      require_value "$1" "${2-}"
      manifest_out="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

for value in \
  "$config" "$features" "$binaries" "$source_commit" "$source_date_epoch" \
  "$platform" "$builder_base_image" "$runtime_base_image" "$docker_path" \
  "$buildx_plugin" "$trusted_buildx_version" "$image_tag" "$artifacts_dir" \
  "$manifest_out" "$buildx_builder"; do
  if [[ "$value" == *$'\n'* || "$value" == *$'\r'* ]]; then
    printf 'release image arguments must not contain control characters\n' >&2
    exit 1
  fi
done
if [[ ! "$features" =~ ^$|^[A-Za-z0-9_+.-]+(,[A-Za-z0-9_+.-]+)*$ ]]; then
  printf 'features must be a canonical comma-separated feature list\n' >&2
  exit 1
fi
if [[ ! "$source_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]]; then
  printf '%s\n' '--source-commit must be a full 40- or 64-hex commit identifier' >&2
  exit 1
fi
if [[ -z "$source_date_epoch" ]]; then
  printf '%s\n' '--source-date-epoch is required' >&2
  exit 1
fi
case "$platform" in
  linux/amd64)
    os_tag="linux"
    arch="amd64"
    target="x86_64-unknown-linux-gnu"
    ;;
  linux/arm64)
    os_tag="linux"
    arch="arm64"
    target="aarch64-unknown-linux-gnu"
    ;;
  *)
    printf '%s\n' '--platform must be exactly linux/amd64 or linux/arm64' >&2
    exit 1
    ;;
esac
require_digest_reference "--builder-base-image" "$builder_base_image"
require_digest_reference "--runtime-base-image" "$runtime_base_image"
require_sha256 "--trusted-docker-sha256" "$trusted_docker_sha256"
require_sha256 "--trusted-buildx-sha256" "$trusted_buildx_sha256"
require_sha256 \
  "--trusted-buildx-builder-inspect-sha256" \
  "$trusted_buildx_builder_inspect_sha256"
safe_token "buildx builder" "$buildx_builder"
if [[ "$docker_path" != /* || "$buildx_plugin" != /* ]]; then
  printf 'docker and buildx plugin paths must be absolute\n' >&2
  exit 1
fi
if (( ${#trusted_buildx_version} == 0 || ${#trusted_buildx_version} > 512 )); then
  printf '%s\n' '--trusted-buildx-version must be non-empty and at most 512 bytes' >&2
  exit 1
fi

if [[ -z "$binaries" ]]; then
  binaries="iroha3d iroha3d_taira sorafs_governance_dag iroha kagami attachment_sanitizer sorafs_external_software_signer"
fi
read -r -a binary_inventory <<< "$binaries"
if (( ${#binary_inventory[@]} == 0 || ${#binary_inventory[@]} > 32 )); then
  printf 'binary inventory must contain between 1 and 32 entries\n' >&2
  exit 1
fi
seen_binaries=" "
for binary in "${binary_inventory[@]}"; do
  safe_token "binary name" "$binary"
  if [[ "$seen_binaries" == *" $binary "* ]]; then
    printf 'binary inventory must not contain duplicates: %s\n' "$binary" >&2
    exit 1
  fi
  seen_binaries+="$binary "
done
binaries="${binary_inventory[*]}"
required_daemon="iroha3d"
if [[ "$config" == "taira" ]]; then
  required_daemon="iroha3d_taira"
fi
if [[ "$seen_binaries" != *" $required_daemon "* ]]; then
  printf 'binary inventory for config %s must include %s\n' "$config" "$required_daemon" >&2
  exit 1
fi

if [[ -z "$prebuilt_bin_dir" ]]; then
  printf '%s\n' '--prebuilt-bin-dir is required for deterministic release images' >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"
version="$(awk -F\" '/^version *=/ { print $2; exit }' Cargo.toml)"
safe_token "version" "$version"
actual_commit="$(git rev-parse HEAD)"
if [[ "$actual_commit" != "$source_commit" ]]; then
  printf 'reviewed source commit does not match the repository HEAD\n' >&2
  exit 1
fi
source_date_epoch="$(
  python3 - "$repo_root/scripts" "$source_date_epoch" <<'EPOCH_PY'
import sys

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import parse_source_date_epoch

print(parse_source_date_epoch(sys.argv[2]))
EPOCH_PY
)"
export SOURCE_DATE_EPOCH="$source_date_epoch"

if [[ -z "$image_tag" ]]; then
  image_tag="hyperledger/iroha:${profile}-${version}"
fi
if [[ ! "$image_tag" =~ ^[a-z0-9][A-Za-z0-9._:/-]{0,254}$ ||
      "$image_tag" != *:* || "$image_tag" == *@* || "$image_tag" == *,* ]]; then
  printf 'image tag must be one bounded canonical tagged OCI reference\n' >&2
  exit 1
fi

artifacts_dir="$(
  python3 - "$repo_root/scripts" "$artifacts_dir" <<'OUTPUT_DIR_PY'
import os
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import scan_inventory_paths

path = Path(os.path.abspath(sys.argv[2]))
scan_inventory_paths(path)
print(path)
OUTPUT_DIR_PY
)"
archive_name="${profile}-${version}-${os_tag}-${arch}-image.oci.tar"
archive_path="$artifacts_dir/$archive_name"
checksum_path="${archive_path}.sha256"
if [[ -z "$manifest_out" ]]; then
  manifest_out="$artifacts_dir/${profile}-${version}-${os_tag}-${arch}-image.json"
else
  manifest_out="$(
    python3 - "$repo_root/scripts" "$manifest_out" <<'MANIFEST_PATH_PY'
import os
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import scan_inventory_paths

path = Path(os.path.abspath(sys.argv[2]))
scan_inventory_paths(path.parent)
print(path)
MANIFEST_PATH_PY
  )"
fi
for output in "$archive_path" "$checksum_path" "$manifest_out"; do
  if [[ -e "$output" || -L "$output" ]]; then
    printf 'release output already exists; refusing stale reuse: %s\n' "$output" >&2
    exit 1
  fi
done

verify_tool_contract() {
  python3 - \
    "$repo_root/scripts" \
    "$docker_path" \
    "$trusted_docker_sha256" \
    "$buildx_plugin" \
    "$trusted_buildx_sha256" <<'TOOL_PY'
import stat
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import ReleaseArtifactError, stable_hash_path

for label, raw_path, expected_sha256 in (
    ("Docker CLI", sys.argv[2], sys.argv[3]),
    ("docker-buildx plugin", sys.argv[4], sys.argv[5]),
):
    info = stable_hash_path(Path(raw_path))
    if info.sha256 != expected_sha256:
        raise ReleaseArtifactError(f"{label} SHA256 is not trusted")
    if not info.mode & stat.S_IXUSR:
        raise ReleaseArtifactError(f"{label} must be owner-executable")
TOOL_PY
}
verify_tool_contract

temp_root="$(mktemp -d "${TMPDIR:-/tmp}/iroha-release-image.XXXXXX")"
temp_root="$(cd "$temp_root" && pwd -P)"
if [[ "$temp_root" == *","* || "$temp_root" == *$'\n'* || "$temp_root" == *$'\r'* ]]; then
  printf 'temporary release path must not contain commas or controls\n' >&2
  exit 1
fi
cleanup() {
  rm -rf -- "$temp_root"
}
trap cleanup EXIT

docker_config="$temp_root/docker-config"
plugin_dir="$temp_root/docker-cli-plugins"
tool_dir="$temp_root/release-tools"
mkdir -m 0700 "$docker_config" "$plugin_dir" "$tool_dir"
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$docker_path" \
  --output "$tool_dir/docker" \
  --mode 0755 \
  --require-executable
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$buildx_plugin" \
  --output "$plugin_dir/docker-buildx" \
  --mode 0755 \
  --require-executable
export DOCKER_CONFIG="$docker_config"
export DOCKER_CLI_PLUGIN_EXTRA_DIRS="$plugin_dir"

version_capture="$temp_root/buildx-version.txt"
python3 "$repo_root/scripts/capture_release_command.py" \
  --output "$version_capture" \
  --executable-root "$tool_dir" \
  --executable-relative "docker" \
  --trusted-executable-sha256 "$trusted_docker_sha256" \
  -- buildx version
observed_buildx_version="$(python3 - "$version_capture" <<'VERSION_PY'
import sys
from pathlib import Path

payload = Path(sys.argv[1]).read_bytes()
try:
    text = payload.decode("utf-8")
except UnicodeDecodeError as exc:
    raise SystemExit(f"buildx version is not UTF-8: {exc}")
if text.endswith("\n"):
    text = text[:-1]
if "\n" in text or "\r" in text:
    raise SystemExit("buildx version must be exactly one line")
print(text, end="")
VERSION_PY
)"
if [[ "$observed_buildx_version" != "$trusted_buildx_version" ]]; then
  printf 'docker buildx version does not match the reviewed exact version\n' >&2
  exit 1
fi
builder_inspect_capture="$temp_root/buildx-builder-inspect.txt"
python3 "$repo_root/scripts/capture_release_command.py" \
  --output "$builder_inspect_capture" \
  --executable-root "$tool_dir" \
  --executable-relative "docker" \
  --trusted-executable-sha256 "$trusted_docker_sha256" \
  -- buildx inspect --builder "$buildx_builder" --bootstrap
observed_builder_inspect_sha256="$(
  python3 - "$repo_root/scripts" "$builder_inspect_capture" <<'BUILDER_INSPECT_PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import stable_hash_path

print(stable_hash_path(Path(sys.argv[2])).sha256)
BUILDER_INSPECT_PY
)"
if [[ "$observed_builder_inspect_sha256" != \
      "$trusted_buildx_builder_inspect_sha256" ]]; then
  printf 'buildx builder inspection does not match the reviewed exact state\n' >&2
  exit 1
fi

build_context="$temp_root/context"
mkdir -m 0700 "$build_context"
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$repo_root/Dockerfile" \
  --output "$build_context/Dockerfile" \
  --mode 0644
mkdir -m 0755 \
  "$build_context/scripts" \
  "$build_context/scripts/ci" \
  "$build_context/dist" \
  "$build_context/configs" \
  "$build_context/configs/sorafs" \
  "$build_context/configs/soranexus" \
  "$build_context/configs/soranexus/taira"
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$repo_root/scripts/docker_entrypoint.sh" \
  --output "$build_context/scripts/docker_entrypoint.sh" \
  --mode 0755 \
  --require-executable
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$repo_root/scripts/ci/package_inrou_runtime_v1.py" \
  --output "$build_context/scripts/ci/package_inrou_runtime_v1.py" \
  --mode 0644
mkdir -m 0755 "$build_context/dist/docker-bin"
for binary in "${binary_inventory[@]}"; do
  python3 "$repo_root/scripts/copy_release_file.py" \
    --source "$prebuilt_bin_dir/$binary" \
    --output "$build_context/dist/docker-bin/$binary" \
    --mode 0755 \
    --require-executable
done
python3 "$repo_root/scripts/copy_release_tree.py" \
  --source-root "$repo_root/defaults" \
  --output-root "$build_context" \
  --destination-prefix "defaults" >/dev/null
python3 "$repo_root/scripts/copy_release_tree.py" \
  --source-root "$repo_root/codec/rans/tables" \
  --output-root "$build_context" \
  --destination-prefix "codec/rans/tables" >/dev/null
python3 "$repo_root/scripts/copy_release_tree.py" \
  --source-root "$repo_root/configs/sorafs/external_software_signer" \
  --output-root "$build_context" \
  --destination-prefix "configs/sorafs/external_software_signer" >/dev/null
python3 "$repo_root/scripts/copy_release_tree.py" \
  --source-root "$repo_root/configs/sorafs/runtime_provider_broker" \
  --output-root "$build_context" \
  --destination-prefix "configs/sorafs/runtime_provider_broker" >/dev/null
context_kind="closed-prebuilt"
context_summary="$(
    python3 - "$repo_root/scripts" "$build_context" <<'CONTEXT_PY'
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import (
    canonical_json_bytes,
    scan_inventory_paths,
    stable_hash_relative,
)

root = Path(sys.argv[2])
paths = scan_inventory_paths(root)
rows = []
for relative in paths:
    info = stable_hash_relative(root, relative)
    rows.append(
        {
            "mode": info.mode,
            "path": relative,
            "sha256": info.sha256,
            "size": info.size,
        }
    )
print(
    json.dumps(
        {
            "file_count": len(rows),
            "sha256": hashlib.sha256(canonical_json_bytes(rows)).hexdigest(),
        },
        sort_keys=True,
        separators=(",", ":"),
    )
)
CONTEXT_PY
)"

oci_layout="$temp_root/oci-layout"
docker_build_args=(
  buildx build
  --builder "$buildx_builder"
  --platform "$platform"
  --file "$build_context/Dockerfile"
  --pull
  --no-cache
  --provenance=false
  --sbom=false
  --output "type=oci,dest=${oci_layout},tar=false,rewrite-timestamp=true,name=${image_tag}"
  --build-arg "PROFILE=deploy"
  --build-arg "FEATURES=${features}"
  --build-arg "CONFIG_PROFILE=${config}"
  --build-arg "BINARIES=${binaries}"
  --build-arg "IROHA_GIT_COMMIT_HASH=${source_commit}"
  --build-arg "SOURCE_DATE_EPOCH=${source_date_epoch}"
  --build-arg "IROHA_RUST_BUILDER_IMAGE=${builder_base_image}"
  --build-arg "IROHA_RUNTIME_IMAGE=${runtime_base_image}"
  --build-arg "IROHA_RELEASE_PREPROVISIONED_BASES=1"
  --build-arg "BUILDKIT_MULTI_PLATFORM=1"
)
docker_build_args+=(--network none --build-arg "USE_PREBUILT=1")
docker_build_args+=("$build_context")

log "Building deterministic OCI layout for ${image_tag} (${platform})"
"$tool_dir/docker" "${docker_build_args[@]}"
verify_tool_contract
private_tool_summary="$(
  python3 - \
    "$repo_root/scripts" \
    "$tool_dir/docker" \
    "$plugin_dir/docker-buildx" <<'PRIVATE_TOOL_PY'
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import stable_hash_path

print(
    json.dumps(
        {
            "buildx": stable_hash_path(Path(sys.argv[3])).sha256,
            "docker": stable_hash_path(Path(sys.argv[2])).sha256,
        },
        sort_keys=True,
        separators=(",", ":"),
    )
)
PRIVATE_TOOL_PY
)"
expected_private_tool_summary="$(
  python3 - "$trusted_docker_sha256" "$trusted_buildx_sha256" <<'EXPECTED_TOOL_PY'
import json
import sys

print(
    json.dumps(
        {
            "buildx": sys.argv[2],
            "docker": sys.argv[1],
        },
        sort_keys=True,
        separators=(",", ":"),
    )
)
EXPECTED_TOOL_PY
)"
if [[ "$private_tool_summary" != "$expected_private_tool_summary" ]]; then
  printf 'private Docker/buildx tools changed during the image build\n' >&2
  exit 1
fi

version_capture_after="$temp_root/buildx-version-after.txt"
python3 "$repo_root/scripts/capture_release_command.py" \
  --output "$version_capture_after" \
  --executable-root "$tool_dir" \
  --executable-relative "docker" \
  --trusted-executable-sha256 "$trusted_docker_sha256" \
  -- buildx version
if ! cmp -s "$version_capture" "$version_capture_after"; then
  printf 'docker buildx version changed during the image build\n' >&2
  exit 1
fi
builder_inspect_capture_after="$temp_root/buildx-builder-inspect-after.txt"
python3 "$repo_root/scripts/capture_release_command.py" \
  --output "$builder_inspect_capture_after" \
  --executable-root "$tool_dir" \
  --executable-relative "docker" \
  --trusted-executable-sha256 "$trusted_docker_sha256" \
  -- buildx inspect --builder "$buildx_builder" --bootstrap
if ! cmp -s "$builder_inspect_capture" "$builder_inspect_capture_after"; then
  printf 'buildx builder inspection changed during the image build\n' >&2
  exit 1
fi

context_summary_after="$(
    python3 - "$repo_root/scripts" "$build_context" <<'CONTEXT_AFTER_PY'
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import (
    canonical_json_bytes,
    scan_inventory_paths,
    stable_hash_relative,
)

root = Path(sys.argv[2])
paths = scan_inventory_paths(root)
rows = []
for relative in paths:
    info = stable_hash_relative(root, relative)
    rows.append(
        {
            "mode": info.mode,
            "path": relative,
            "sha256": info.sha256,
            "size": info.size,
        }
    )
print(
    json.dumps(
        {
            "file_count": len(rows),
            "sha256": hashlib.sha256(canonical_json_bytes(rows)).hexdigest(),
        },
        sort_keys=True,
        separators=(",", ":"),
    )
)
CONTEXT_AFTER_PY
)"
if [[ "$context_summary_after" != "$context_summary" ]]; then
  printf 'closed release image build context changed during the build\n' >&2
  exit 1
fi

log "Writing canonical OCI archive ${archive_name}"
oci_summary="$(
  python3 "$repo_root/scripts/build_release_oci_archive.py" \
    --layout-root "$oci_layout" \
    --output "$archive_path" \
    --source-date-epoch "$source_date_epoch" \
    --expected-ref-name "$image_tag" \
    --expected-os "$os_tag" \
    --expected-architecture "$arch"
)"
archive_sha="$(
  python3 "$repo_root/scripts/write_release_checksum.py" \
    --artifact "$archive_path" \
    --output "$checksum_path" \
    --listed-name "$archive_name"
)"

python3 - \
  "$repo_root/scripts" \
  "$manifest_out" \
  "$profile" \
  "$config" \
  "$version" \
  "$source_commit" \
  "$source_date_epoch" \
  "$os_tag" \
  "$arch" \
  "$target" \
  "$platform" \
  "$features" \
  "$binaries" \
  "$context_kind" \
  "$context_summary" \
  "$builder_base_image" \
  "$runtime_base_image" \
  "$trusted_docker_sha256" \
  "$trusted_buildx_sha256" \
  "$trusted_buildx_version" \
  "$buildx_builder" \
  "$trusted_buildx_builder_inspect_sha256" \
  "$image_tag" \
  "$oci_summary" \
  "$archive_path" \
  "$archive_sha" <<'MANIFEST_PY'
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import (
    canonical_json_bytes,
    exclusive_write_bytes,
    format_source_date_epoch,
    stable_hash_path,
)

(
    manifest_out,
    profile,
    config,
    version,
    commit,
    epoch_raw,
    os_tag,
    arch,
    target,
    platform,
    features,
    binaries,
    context_kind,
    context_summary_raw,
    builder_base_image,
    runtime_base_image,
    docker_sha256,
    buildx_sha256,
    buildx_version,
    buildx_builder,
    buildx_builder_inspect_sha256,
    image_tag,
    oci_summary_raw,
    archive_path_raw,
    archive_sha,
) = sys.argv[2:]
epoch = int(epoch_raw)
context_summary = json.loads(context_summary_raw)
oci_summary = json.loads(oci_summary_raw)
archive_path = Path(archive_path_raw)
archive = stable_hash_path(archive_path)
if archive.sha256 != archive_sha:
    raise SystemExit("OCI archive changed before builder manifest generation")
manifest = {
    "schema": "iroha.release_image_builder_manifest",
    "schema_version": 1,
    "profile": profile,
    "config": config,
    "version": version,
    "commit": commit,
    "source_date_epoch": epoch,
    "built_at": format_source_date_epoch(epoch),
    "os": os_tag,
    "arch": arch,
    "target": target,
    "platform": platform,
    "features": features,
    "binaries": binaries.split(),
    "external_software_signer": {
        "backend": "software",
        "binary": "/usr/local/bin/sorafs_external_software_signer",
        "broker_alias": "/usr/local/libexec/iroha-runtime-provider-broker-v1",
        "smoke": "native-build-stage",
        "windows_supported": False,
    },
    "source_context": {
        "kind": context_kind,
        **context_summary,
    },
    "base_images": {
        "builder": builder_base_image,
        "runtime": runtime_base_image,
    },
    "builder": {
        "docker": {
            "sha256": docker_sha256,
        },
        "buildx": {
            "sha256": buildx_sha256,
            "version": buildx_version,
            "builder": buildx_builder,
            "builder_inspect_sha256": buildx_builder_inspect_sha256,
        },
        "network": "none",
        "output": {
            "type": "oci",
            "tar": False,
            "rewrite_timestamp": True,
        },
        "provenance": False,
        "sbom": False,
    },
    "image": {
        "reference": image_tag,
        **oci_summary,
    },
    "artifacts": [
        {
            "file": archive_path.name,
            "sha256": archive.sha256,
            "size": archive.size,
        }
    ],
}
exclusive_write_bytes(
    Path(manifest_out),
    canonical_json_bytes(manifest),
    mode=0o644,
)
MANIFEST_PY

printf '%s\n' "$archive_path"
