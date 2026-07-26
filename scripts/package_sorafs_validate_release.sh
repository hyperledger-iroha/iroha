#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
package_sorafs_validate_release.sh --target <triple> --version <label> \
  --source-commit <hex> --source-date-epoch <epoch> [options]

Builds and packages the SoraFS reference validator binary (`sorafs-validate`)
and checked C FFI header (`sorafs_reference.h`) for release distribution. The
helper stages a deterministic archive under dist/ without tracking generated
artifacts.

Options:
  --workspace <path>     Repository root (default: script parent/..).
  --out-dir <path>       Output directory (default: <workspace>/dist/sorafs-validate-release).
  --target <triple>      Required reviewed Cargo target triple.
  --profile <name>       Cargo profile to build (default: release).
  --binary <path>        Prebuilt sorafs-validate binary to package instead of building.
  --target-dir <path>    Cargo target directory override.
  --version <string>     Required release version label.
  --source-commit <hex>  Required reviewed full source commit.
  --source-date-epoch <epoch>
                         Required canonical release SOURCE_DATE_EPOCH.
  --skip-smoke           Skip committed-fixture smoke checks.
  --help                 Show this help and exit.

The packager is an unsigned deterministic artifact/checksum producer. Retired
package-manifest signing options are rejected. Production signing occurs once,
after every package enters the canonical aggregate release manifest.
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

require_option_value() {
  local option="$1"
  local value="${2-}"
  if [[ -z "$value" || "$value" == --* ]]; then
    echo "error: ${option} requires a value" >&2
    exit 1
  fi
}

reject_symlinked_path_parent() {
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
  reject_symlinked_path_parent "$label" "$target"
  if [[ ! -e "$target" ]]; then
    echo "error: ${label} not found at $target" >&2
    exit 1
  fi
  if [[ ! -f "$target" ]]; then
    echo "error: ${label} must be a regular file: $target" >&2
    exit 1
  fi
}

validate_existing_executable_file_path() {
  local label="$1"
  local target="$2"
  validate_existing_file_path "$label" "$target"
  if [[ ! -x "$target" ]]; then
    echo "error: ${label} not executable at $target" >&2
    exit 1
  fi
}

prepare_output_directory_path() {
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
  reject_symlinked_path_parent "$label" "$target"
  if [[ -e "$target" && ! -d "$target" ]]; then
    echo "error: ${label} must be a directory path: ${target}" >&2
    exit 1
  fi
  mkdir -p "$target"
  if [[ -L "$target" ]]; then
    echo "error: ${label} must not be a symlink: ${target}" >&2
    exit 1
  fi
  reject_symlinked_path_parent "$label" "$target"
  if [[ ! -d "$target" ]]; then
    echo "error: ${label} must be a directory path: ${target}" >&2
    exit 1
  fi
}

validate_release_token() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$ ]]; then
    echo "error: ${label} must be a bounded safe release token" >&2
    exit 1
  fi
}

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir=""
target=""
profile="release"
binary_path=""
target_dir=""
version=""
source_commit=""
source_date_epoch_arg=""
skip_smoke=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      require_option_value "$1" "${2-}"
      workspace="$(abs_path "$2")"
      shift 2
      ;;
    --out-dir)
      require_option_value "$1" "${2-}"
      out_dir="$(abs_path "$2")"
      shift 2
      ;;
    --target)
      require_option_value "$1" "${2-}"
      target="$2"
      shift 2
      ;;
    --profile)
      require_option_value "$1" "${2-}"
      profile="$2"
      shift 2
      ;;
    --binary)
      require_option_value "$1" "${2-}"
      binary_path="$(abs_path "$2")"
      shift 2
      ;;
    --target-dir)
      require_option_value "$1" "${2-}"
      target_dir="$(abs_path "$2")"
      shift 2
      ;;
    --version)
      require_option_value "$1" "${2-}"
      version="$2"
      shift 2
      ;;
    --source-commit)
      require_option_value "$1" "${2-}"
      source_commit="$2"
      shift 2
      ;;
    --source-date-epoch)
      require_option_value "$1" "${2-}"
      source_date_epoch_arg="$2"
      shift 2
      ;;
    --skip-smoke)
      skip_smoke=1
      shift
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

workspace="$(abs_path "$workspace")"
[[ -z "$out_dir" ]] && out_dir="${workspace}/dist/sorafs-validate-release"
if [[ -z "$target" || -z "$version" || -z "$source_commit" ||
      -z "$source_date_epoch_arg" ]]; then
  usage >&2
  exit 1
fi
validate_release_token "target" "$target"
validate_release_token "profile" "$profile"
validate_release_token "version" "$version"
if [[ ! "$source_commit" =~ ^([0-9a-f]{40}|[0-9a-f]{64})$ ]]; then
  echo "error: --source-commit must be a full 40- or 64-hex identifier" >&2
  exit 1
fi
actual_commit="$(git -C "$workspace" rev-parse HEAD)"
if [[ "$source_commit" != "$actual_commit" ]]; then
  echo "error: reviewed source commit does not match the workspace HEAD" >&2
  exit 1
fi
source_date_epoch="$(
  python3 - "${workspace}/scripts" "$source_date_epoch_arg" <<'EPOCH_PY'
import sys

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import parse_source_date_epoch

print(parse_source_date_epoch(sys.argv[2]))
EPOCH_PY
)"
export SOURCE_DATE_EPOCH="$source_date_epoch"

case "$target" in
  *-windows-*) packaged_binary_name="sorafs-validate.exe" ;;
  *) packaged_binary_name="sorafs-validate" ;;
esac

case "$profile" in
  release)
    profile_dir="release"
    build_profile_args=(--release)
    ;;
  *)
    profile_dir="$profile"
    build_profile_args=(--profile "$profile")
    ;;
esac

if [[ -z "$binary_path" ]]; then
  build_cmd=(cargo build --locked -p sorafs_manifest --bin sorafs-validate "${build_profile_args[@]}")
  if [[ -n "$target" ]]; then
    build_cmd+=(--target "$target")
  fi
  if [[ -n "$target_dir" ]]; then
    build_cmd+=(--target-dir "$target_dir")
  fi
  echo "Building sorafs-validate (${profile}, ${target})..."
  (cd "$workspace" && "${build_cmd[@]}")
  cargo_target_dir="${target_dir:-${workspace}/target}"
  binary_path="${cargo_target_dir}/${target}/${profile_dir}/${packaged_binary_name}"
fi

binary_path="$(abs_path "$binary_path")"
validate_existing_executable_file_path "sorafs-validate binary" "$binary_path"

package_name="sorafs-validate-${version}-${target}"
stage_dir="${out_dir}/${package_name}"
archive_path="${out_dir}/${package_name}.tar.gz"
manifest_path="${out_dir}/${package_name}.manifest.json"
manifest_sha_path="${manifest_path}.sha256"
binary_sha_path="${out_dir}/${package_name}.sha256"
archive_sha_path="${archive_path}.sha256"
header_path="${workspace}/crates/sorafs_manifest/include/sorafs_reference.h"

validate_existing_file_path "SoraFS reference FFI header" "$header_path"
retired_signature_path="${manifest_path}.sig"
if [[ -e "$retired_signature_path" || -L "$retired_signature_path" ]]; then
  echo "error: retired package-manifest signature exists; remove it before unsigned packaging" >&2
  exit 1
fi

prepare_output_directory_path "release output directory" "$out_dir"
for generated_path in \
  "$stage_dir" "$archive_path" "$manifest_path" "$manifest_sha_path" \
  "$binary_sha_path" "$archive_sha_path"; do
  if [[ -e "$generated_path" || -L "$generated_path" ]]; then
    echo "error: release output already exists; refuse stale output reuse: ${generated_path}" >&2
    exit 1
  fi
done
mkdir -m 0755 "$stage_dir"
mkdir -m 0755 "$stage_dir/include"
python3 "${workspace}/scripts/copy_release_file.py" \
  --source "$binary_path" \
  --output "${stage_dir}/${packaged_binary_name}" \
  --mode 0755 \
  --require-executable
python3 "${workspace}/scripts/copy_release_file.py" \
  --source "$header_path" \
  --output "${stage_dir}/include/sorafs_reference.h" \
  --mode 0644

python3 "${workspace}/scripts/capture_release_command.py" \
  --output "${stage_dir}/HELP.txt" \
  --executable-root "$stage_dir" \
  --executable-relative "$packaged_binary_name" \
  -- --help

if [[ "$skip_smoke" -eq 0 ]]; then
  python3 "${workspace}/scripts/capture_release_command.py" \
    --output "${stage_dir}/smoke.advert.json" \
    --executable-root "$stage_dir" \
    --executable-relative "$packaged_binary_name" \
    -- advert \
      --input "${workspace}/fixtures/sorafs_manifest/provider_admission/advert_v1.to" \
      --now 120 \
      --generated-at 123 \
      --format json
  python3 "${workspace}/scripts/capture_release_command.py" \
    --output "${stage_dir}/smoke.bundle.json" \
    --executable-root "$stage_dir" \
    --executable-relative "$packaged_binary_name" \
    -- bundle \
      --bundle "${workspace}/fixtures/sorafs_manifest" \
      --now 120 \
      --generated-at 123 \
      --format json
fi

stage_inventory=(
  "HELP.txt"
  "include/sorafs_reference.h"
  "$packaged_binary_name"
)
if [[ "$skip_smoke" -eq 0 ]]; then
  stage_inventory+=("smoke.advert.json" "smoke.bundle.json")
fi
archive_command=(
  python3 "${workspace}/scripts/build_release_tar_gz.py"
  --stage-root "$stage_dir"
  --output "$archive_path"
  --prefix "$package_name"
  --source-date-epoch "$source_date_epoch"
  --executable "$packaged_binary_name"
)
for relative_path in "${stage_inventory[@]}"; do
  archive_command+=(--file "$relative_path")
done
stage_inventory_json="$("${archive_command[@]}")"
binary_sha="$(
  python3 - "$stage_inventory_json" "$packaged_binary_name" <<'PY'
import json
import re
import sys

inventory = json.loads(sys.argv[1])
digest = inventory.get(sys.argv[2])
if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
    raise SystemExit("invalid captured binary digest")
print(digest)
PY
)"
python3 "${workspace}/scripts/write_release_checksum.py" \
  --digest "$binary_sha" \
  --output "$binary_sha_path" \
  --listed-name "$packaged_binary_name" >/dev/null
archive_sha="$(
  python3 "${workspace}/scripts/write_release_checksum.py" \
    --artifact "$archive_path" \
    --output "$archive_sha_path" \
    --listed-name "$(basename "$archive_path")"
)"

export SORAFS_VALIDATE_PACKAGE_VERSION="$version"
export SORAFS_VALIDATE_PACKAGE_TARGET="$target"
export SORAFS_VALIDATE_PACKAGE_PROFILE="$profile"
export SORAFS_VALIDATE_PACKAGE_COMMIT="$source_commit"
SORAFS_VALIDATE_PACKAGE_ARCHIVE="$(basename "$archive_path")"
export SORAFS_VALIDATE_PACKAGE_ARCHIVE
export SORAFS_VALIDATE_PACKAGE_ARCHIVE_SHA="$archive_sha"
export SORAFS_VALIDATE_PACKAGE_SMOKE="$skip_smoke"
export SORAFS_VALIDATE_PACKAGE_BINARY="$packaged_binary_name"
export SORAFS_VALIDATE_PACKAGE_STAGE_INVENTORY="$stage_inventory_json"
export SORAFS_VALIDATE_PACKAGE_SOURCE_DATE_EPOCH="$source_date_epoch"
python3 - "${workspace}/scripts" "$manifest_path" <<'PY'
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, sys.argv[1])
from release_artifact_contract import (  # noqa: E402
    canonical_json_bytes,
    exclusive_write_bytes,
    format_source_date_epoch,
    parse_source_date_epoch,
)

manifest_path = Path(sys.argv[2])
inventory = json.loads(os.environ["SORAFS_VALIDATE_PACKAGE_STAGE_INVENTORY"])
stage_files = [
    {"path": path, "sha256": inventory[path]} for path in sorted(inventory)
]
smoke_checks = []
if os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE"] == "0":
    smoke_checks = [
        {
            "command": "sorafs-validate advert",
            "output": "smoke.advert.json",
            "sha256": inventory["smoke.advert.json"],
        },
        {
            "command": "sorafs-validate bundle",
            "output": "smoke.bundle.json",
            "sha256": inventory["smoke.bundle.json"],
        },
    ]

epoch = parse_source_date_epoch(
    os.environ["SORAFS_VALIDATE_PACKAGE_SOURCE_DATE_EPOCH"]
)
binary = os.environ["SORAFS_VALIDATE_PACKAGE_BINARY"]
manifest = {
    "schema_version": 1,
    "package": "sorafs-validate",
    "version": os.environ["SORAFS_VALIDATE_PACKAGE_VERSION"],
    "commit": os.environ["SORAFS_VALIDATE_PACKAGE_COMMIT"],
    "target": os.environ["SORAFS_VALIDATE_PACKAGE_TARGET"],
    "profile": os.environ["SORAFS_VALIDATE_PACKAGE_PROFILE"],
    "archive": os.environ["SORAFS_VALIDATE_PACKAGE_ARCHIVE"],
    "archive_sha256": os.environ["SORAFS_VALIDATE_PACKAGE_ARCHIVE_SHA"],
    "source_date_epoch": epoch,
    "built_at": format_source_date_epoch(epoch),
    "binary": binary,
    "binary_sha256": inventory[binary],
    "ffi_header": "include/sorafs_reference.h",
    "ffi_header_sha256": inventory["include/sorafs_reference.h"],
    "stage_files": stage_files,
    "smoke_checks": os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE"] == "0",
    "smoke_outputs": smoke_checks,
}
exclusive_write_bytes(
    manifest_path,
    canonical_json_bytes(manifest),
    mode=0o644,
)
PY
python3 "${workspace}/scripts/write_release_checksum.py" \
  --artifact "$manifest_path" \
  --output "$manifest_sha_path" \
  --listed-name "$(basename "$manifest_path")" >/dev/null

echo
echo "SoraFS reference validator release package:"
echo "  Archive : $archive_path"
echo "  Manifest: $manifest_path"
echo "  Manifest SHA256: $manifest_sha_path"
echo "  Binary SHA256 : $binary_sha_path"
echo "  Archive SHA256: $archive_sha_path"
