#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build_release_bundle.sh --profile <iroha2|iroha3> --config <single|nexus|path> \
  --target <triple> --source-commit <hex> --source-date-epoch <epoch> [options]

Options:
  --features <list>              Cargo features (default is profile-specific).
  --target <triple>              Required reviewed Cargo target triple.
  --source-commit <hex>          Required reviewed full source commit.
  --source-date-epoch <epoch>    Required canonical release SOURCE_DATE_EPOCH.
  --prebuilt-bin-dir <path>      Use reviewed prebuilt binaries; skips Cargo.
  --artifacts-dir <path>         Output directory (default: dist).
  --manifest-out <path>          Builder manifest output path.
  --zstd <path>                  Required exact zstd executable.
  --trusted-zstd-sha256 <hex>    Reviewed SHA256 of the exact zstd executable.
  -h, --help                     Show this help.

The builder emits unsigned deterministic artifacts. Aggregate signing occurs
only after the pipeline closes and verifies the complete release inventory.
EOF
}

log() {
  printf '[dual-build] %s\n' "$*" >&2
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

profile=""
config=""
features=""
target=""
source_commit=""
source_date_epoch_arg=""
prebuilt_bin_dir=""
artifacts_dir="dist"
manifest_out=""
trusted_zstd_sha256=""
zstd_path=""

while (($#)); do
  case "$1" in
    --profile)
      require_value "$1" "${2-}"
      profile="$2"
      shift 2
      ;;
    --config)
      require_value "$1" "${2-}"
      config="$2"
      shift 2
      ;;
    --features)
      require_value "$1" "${2-}"
      features="$2"
      shift 2
      ;;
    --target)
      require_value "$1" "${2-}"
      target="$2"
      shift 2
      ;;
    --source-commit)
      require_value "$1" "${2-}"
      source_commit="$2"
      shift 2
      ;;
    --source-date-epoch)
      require_value "$1" "${2-}"
      source_date_epoch_arg="$2"
      shift 2
      ;;
    --prebuilt-bin-dir)
      require_value "$1" "${2-}"
      prebuilt_bin_dir="$2"
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
    --trusted-zstd-sha256)
      require_value "$1" "${2-}"
      trusted_zstd_sha256="$2"
      shift 2
      ;;
    --zstd)
      require_value "$1" "${2-}"
      zstd_path="$2"
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

if [[ -z "$profile" || -z "$config" ]]; then
  usage >&2
  exit 1
fi
case "$profile" in
  iroha2|iroha3) ;;
  *)
    printf 'Unsupported profile value: %s (expected iroha2 or iroha3)\n' "$profile" >&2
    exit 1
    ;;
esac
if [[ -z "$target" || -z "$source_commit" || -z "$source_date_epoch_arg" ||
      -z "$zstd_path" ]]; then
  usage >&2
  exit 1
fi
if [[ "$config" == *$'\n'* || "$config" == *$'\r'* ||
      "$features" == *$'\n'* || "$features" == *$'\r'* ]]; then
  printf 'config and features must not contain control characters\n' >&2
  exit 1
fi
safe_token "target" "$target"
if [[ ! "$source_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]]; then
  printf '%s\n' '--source-commit must be a full 40- or 64-hex identifier' >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
version="$(awk -F\" '/^version *=/ { print $2; exit }' Cargo.toml)"
safe_token "version" "$version"
if [[ ! "$trusted_zstd_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  printf '%s\n' '--trusted-zstd-sha256 is required as 64 lowercase hex' >&2
  exit 1
fi
if [[ "$zstd_path" != /* ]]; then
  printf 'zstd must be an explicit absolute executable path\n' >&2
  exit 1
fi

case "$target" in
  *-apple-darwin) os_tag="mac" ;;
  *-windows-*) os_tag="win" ;;
  *-linux-*) os_tag="linux" ;;
  *)
    printf 'Unsupported release target OS in triple: %s\n' "$target" >&2
    exit 1
    ;;
esac
arch="${target%%-*}"
safe_token "target architecture" "$arch"
commit="$(git rev-parse HEAD)"
if [[ "$source_commit" != "$commit" ]]; then
  printf 'reviewed source commit does not match the repository HEAD\n' >&2
  exit 1
fi
commit="$source_commit"
source_date_epoch="$(
  python3 - "$repo_root/scripts" "$source_date_epoch_arg" <<'EPOCH_PY'
import sys

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import parse_source_date_epoch

print(parse_source_date_epoch(sys.argv[2]))
EPOCH_PY
)"
export SOURCE_DATE_EPOCH="$source_date_epoch"

if [[ -z "$features" ]]; then
  case "$profile" in
    iroha2) features="build-i2" ;;
    iroha3) features="build-i3" ;;
  esac
fi

if [[ -z "$prebuilt_bin_dir" ]]; then
  cargo_command=(cargo build --profile deploy --bins --locked)
  if [[ -n "$target" ]]; then
    cargo_command+=(--target "$target")
  fi
  if [[ -n "$features" ]]; then
    cargo_command+=(--features "$features")
  fi
  log "Building binaries (profile=${profile}, config=${config}, target=${target})"
  "${cargo_command[@]}"
  binary_root="$repo_root/target/$target/deploy"
else
  binary_root="$(
    python3 - "$repo_root/scripts" "$prebuilt_bin_dir" <<'PREBUILT_DIR_PY'
import os
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import scan_inventory_paths

path = Path(os.path.abspath(sys.argv[2]))
scan_inventory_paths(path)
print(path)
PREBUILT_DIR_PY
  )"
fi

daemon_bin="iroha3d"
cli_bin="iroha3"
if [[ "$profile" == "iroha2" ]]; then
  daemon_bin="iroha2d"
  cli_bin="iroha2"
fi
utility_bin="kagami"
sanitizer_bin="attachment_sanitizer"
if [[ "$os_tag" == "win" ]]; then
  daemon_bin="${daemon_bin}.exe"
  cli_bin="${cli_bin}.exe"
  utility_bin="${utility_bin}.exe"
  sanitizer_bin="${sanitizer_bin}.exe"
fi

artifacts_dir="$(
  python3 - "$repo_root/scripts" "$artifacts_dir" <<'OUTPUT_DIR_PY'
import os
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import (
    create_fresh_directory,
    scan_inventory_paths,
)

path = Path(os.path.abspath(sys.argv[2]))
if path.exists():
    scan_inventory_paths(path)
else:
    create_fresh_directory(path)
print(path)
OUTPUT_DIR_PY
)"
archive_name="${profile}-${version}-${os_tag}-${arch}.tar.zst"
archive_path="$artifacts_dir/$archive_name"
checksum_path="${archive_path}.sha256"
if [[ -z "$manifest_out" ]]; then
  manifest_out="$artifacts_dir/${profile}-${version}-${os_tag}-${arch}-manifest.json"
else
  manifest_out="$(
    python3 - "$repo_root/scripts" "$manifest_out" <<'MANIFEST_PATH_PY'
import os
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import (
    create_fresh_directory,
    scan_inventory_paths,
)

path = Path(os.path.abspath(sys.argv[2]))
if path.parent.exists():
    scan_inventory_paths(path.parent)
else:
    create_fresh_directory(path.parent)
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

stage_parent="$(mktemp -d "${TMPDIR:-/tmp}/iroha-release-bundle.XXXXXX")"
stage_parent="$(cd "$stage_parent" && pwd -P)"
stage_root="$stage_parent/stage"
mkdir -m 0755 "$stage_root"
cleanup() {
  rm -rf -- "$stage_parent"
}
trap cleanup EXIT

mkdir -m 0755 "$stage_root/bin" "$stage_root/config"
fixed_files=(
  "LICENSE"
  "PROFILE.toml"
  "bin/$daemon_bin"
  "bin/$cli_bin"
  "bin/$utility_bin"
  "bin/$sanitizer_bin"
)
executables=(
  "bin/$daemon_bin"
  "bin/$cli_bin"
  "bin/$utility_bin"
  "bin/$sanitizer_bin"
)
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$binary_root/$daemon_bin" \
  --output "$stage_root/bin/$daemon_bin" \
  --mode 0755 \
  --require-executable
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$binary_root/$cli_bin" \
  --output "$stage_root/bin/$cli_bin" \
  --mode 0755 \
  --require-executable
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$binary_root/$utility_bin" \
  --output "$stage_root/bin/$utility_bin" \
  --mode 0755 \
  --require-executable
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$binary_root/$sanitizer_bin" \
  --output "$stage_root/bin/$sanitizer_bin" \
  --mode 0755 \
  --require-executable
python3 "$repo_root/scripts/copy_release_file.py" \
  --source "$repo_root/LICENSE" \
  --output "$stage_root/LICENSE" \
  --mode 0644

tree_inventory='[]'
case "$config" in
  single)
    python3 "$repo_root/scripts/copy_release_file.py" \
      --source "$repo_root/defaults/genesis.json" \
      --output "$stage_root/config/genesis.json" \
      --mode 0644
    python3 "$repo_root/scripts/copy_release_file.py" \
      --source "$repo_root/defaults/client.toml" \
      --output "$stage_root/config/client.toml" \
      --mode 0644
    fixed_files+=("config/client.toml" "config/genesis.json")
    if [[ -d "$repo_root/defaults/config.d" ]]; then
      tree_inventory="$(
        python3 "$repo_root/scripts/copy_release_tree.py" \
          --source-root "$repo_root/defaults/config.d" \
          --output-root "$stage_root" \
          --destination-prefix "config/config.d"
      )"
    fi
    ;;
  nexus)
    for name in genesis.json client.toml config.toml; do
      python3 "$repo_root/scripts/copy_release_file.py" \
        --source "$repo_root/defaults/nexus/$name" \
        --output "$stage_root/config/$name" \
        --mode 0644
      fixed_files+=("config/$name")
    done
    ;;
  *)
    if [[ ! -d "$config" || -L "$config" ]]; then
      printf 'Unsupported config value: %s\n' "$config" >&2
      exit 1
    fi
    tree_inventory="$(
      python3 "$repo_root/scripts/copy_release_tree.py" \
        --source-root "$config" \
        --output-root "$stage_root" \
        --destination-prefix "config"
    )"
    ;;
esac

python3 - \
  "$repo_root/scripts" \
  "$stage_root/PROFILE.toml" \
  "$profile" \
  "$config" \
  "$version" \
  "$commit" \
  "$source_date_epoch" \
  "$os_tag" \
  "$arch" \
  "$target" \
  "$features" <<'PROFILE_PY'
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import exclusive_write_bytes, format_source_date_epoch

(
    profile_path,
    profile,
    config,
    version,
    commit,
    epoch_raw,
    os_tag,
    arch,
    target,
    features,
) = sys.argv[2:]
epoch = int(epoch_raw)
values = (
    ("profile", profile),
    ("config", config),
    ("version", version),
    ("commit", commit),
    ("source_date_epoch", epoch),
    ("built_at", format_source_date_epoch(epoch)),
    ("os", os_tag),
    ("arch", arch),
    ("target", target),
    ("features", features),
)
rendered = "\n".join(
    f"{key} = {json.dumps(value, ensure_ascii=True)}" for key, value in values
)
exclusive_write_bytes(Path(profile_path), (rendered + "\n").encode("utf-8"))
PROFILE_PY

archive_command=(
  python3 "$repo_root/scripts/build_release_tar_zst.py"
  --stage-root "$stage_root"
  --output "$archive_path"
  --prefix "${profile}-${version}-${os_tag}-${arch}"
  --source-date-epoch "$source_date_epoch"
  --zstd "$zstd_path"
  --trusted-zstd-sha256 "$trusted_zstd_sha256"
  --file-list-json "$tree_inventory"
)
for relative in "${fixed_files[@]}"; do
  archive_command+=(--file "$relative")
done
for relative in "${executables[@]}"; do
  archive_command+=(--executable "$relative")
done
log "Packaging deterministic bundle $archive_name"
"${archive_command[@]}"

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
  "$commit" \
  "$source_date_epoch" \
  "$os_tag" \
  "$arch" \
  "$target" \
  "$features" \
  "$archive_path" \
  "$archive_sha" \
  "$trusted_zstd_sha256" <<'MANIFEST_PY'
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
    features,
    archive_path_raw,
    archive_sha,
    zstd_sha,
) = sys.argv[2:]
epoch = int(epoch_raw)
archive_path = Path(archive_path_raw)
archive = stable_hash_path(archive_path)
if archive.sha256 != archive_sha:
    raise SystemExit("archive changed before builder manifest generation")
manifest = {
    "schema": "iroha.release_builder_manifest",
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
    "features": features,
    "compressor": {
        "sha256": zstd_sha,
        "arguments": ["-19", "--long=31", "--threads=1", "--no-progress"],
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
