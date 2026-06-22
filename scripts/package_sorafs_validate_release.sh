#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
package_sorafs_validate_release.sh [options]

Builds and packages the SoraFS reference validator binary (`sorafs-validate`)
and checked C FFI header (`sorafs_reference.h`) for release distribution. The
helper stages a deterministic archive under dist/ without tracking generated
artifacts.

Options:
  --workspace <path>     Repository root (default: script parent/..).
  --out-dir <path>       Output directory (default: <workspace>/dist/sorafs-validate-release).
  --target <triple>      Cargo target triple to build/package (default: rustc host triple).
  --profile <name>       Cargo profile to build (default: release).
  --binary <path>        Prebuilt sorafs-validate binary to package instead of building.
  --target-dir <path>    Cargo target directory override.
  --version <string>     Release version label (default: git describe or commit hash).
  --manifest-signing-key <path>
                         Optional PEM private key used to sign the manifest.
  --manifest-public-key <path>
                         Optional PEM public key used to verify the manifest signature.
  --manifest-signature-out <path>
                         Signature path (default: <manifest>.sig).
  --skip-smoke           Skip committed-fixture smoke checks.
  --help                 Show this help and exit.
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

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    echo "error: sha256sum or shasum is required" >&2
    exit 1
  fi
}

host_triple() {
  rustc -vV | awk '/^host:/ {print $2}'
}

default_version() {
  if git -C "$workspace" describe --tags --dirty --always >/dev/null 2>&1; then
    git -C "$workspace" describe --tags --dirty --always
  else
    git -C "$workspace" rev-parse --short HEAD 2>/dev/null || printf 'unknown'
  fi
}

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir=""
target=""
profile="release"
binary_path=""
target_dir=""
version=""
manifest_signing_key=""
manifest_public_key=""
manifest_signature_path=""
skip_smoke=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      workspace="$(abs_path "$2")"
      shift 2
      ;;
    --out-dir)
      out_dir="$(abs_path "$2")"
      shift 2
      ;;
    --target)
      target="$2"
      shift 2
      ;;
    --profile)
      profile="$2"
      shift 2
      ;;
    --binary)
      binary_path="$(abs_path "$2")"
      shift 2
      ;;
    --target-dir)
      target_dir="$(abs_path "$2")"
      shift 2
      ;;
    --version)
      version="$2"
      shift 2
      ;;
    --manifest-signing-key)
      manifest_signing_key="$(abs_path "$2")"
      shift 2
      ;;
    --manifest-public-key)
      manifest_public_key="$(abs_path "$2")"
      shift 2
      ;;
    --manifest-signature-out)
      manifest_signature_path="$(abs_path "$2")"
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

if [[ -n "$manifest_public_key" && -z "$manifest_signing_key" ]]; then
  echo "error: --manifest-public-key requires --manifest-signing-key" >&2
  exit 1
fi
if [[ -n "$manifest_signing_key" ]]; then
  if [[ ! -f "$manifest_signing_key" ]]; then
    echo "error: manifest signing key not found at $manifest_signing_key" >&2
    exit 1
  fi
  if ! command -v openssl >/dev/null 2>&1; then
    echo "error: openssl is required for --manifest-signing-key" >&2
    exit 1
  fi
fi
if [[ -n "$manifest_public_key" && ! -f "$manifest_public_key" ]]; then
  echo "error: manifest public key not found at $manifest_public_key" >&2
  exit 1
fi

workspace="$(abs_path "$workspace")"
[[ -z "$out_dir" ]] && out_dir="${workspace}/dist/sorafs-validate-release"
[[ -z "$target" ]] && target="$(host_triple)"
[[ -z "$version" ]] && version="$(default_version)"

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
  build_cmd=(cargo build -p sorafs_manifest --bin sorafs-validate "${build_profile_args[@]}")
  if [[ -n "$target" ]]; then
    build_cmd+=(--target "$target")
  fi
  if [[ -n "$target_dir" ]]; then
    build_cmd+=(--target-dir "$target_dir")
  fi
  echo "Building sorafs-validate (${profile}, ${target})..."
  (cd "$workspace" && "${build_cmd[@]}")
  cargo_target_dir="${target_dir:-${workspace}/target}"
  binary_path="${cargo_target_dir}/${target}/${profile_dir}/sorafs-validate"
fi

binary_path="$(abs_path "$binary_path")"
if [[ ! -x "$binary_path" ]]; then
  echo "error: sorafs-validate binary not executable at $binary_path" >&2
  exit 1
fi

mkdir -p "$out_dir"
safe_version="${version//[^A-Za-z0-9._-]/_}"
safe_target="${target//[^A-Za-z0-9._-]/_}"
package_name="sorafs-validate-${safe_version}-${safe_target}"
stage_dir="${out_dir}/${package_name}"
archive_path="${out_dir}/${package_name}.tar.gz"
manifest_path="${out_dir}/${package_name}.manifest.json"
manifest_sha_path="${manifest_path}.sha256"
[[ -z "$manifest_signature_path" ]] && manifest_signature_path="${manifest_path}.sig"
binary_sha_path="${out_dir}/${package_name}.sha256"
archive_sha_path="${archive_path}.sha256"
header_path="${workspace}/crates/sorafs_manifest/include/sorafs_reference.h"

if [[ ! -f "$header_path" ]]; then
  echo "error: SoraFS reference FFI header not found at $header_path" >&2
  exit 1
fi

rm -rf "$stage_dir" "$archive_path" "$manifest_path" "$manifest_sha_path" \
  "$binary_sha_path" "$archive_sha_path"
rm -f "$manifest_signature_path"
mkdir -p "$stage_dir/include"
cp "$binary_path" "${stage_dir}/sorafs-validate"
cp "$header_path" "${stage_dir}/include/sorafs_reference.h"

"${stage_dir}/sorafs-validate" --help > "${stage_dir}/HELP.txt"
help_sha="$(sha256_file "${stage_dir}/HELP.txt")"
header_sha="$(sha256_file "${stage_dir}/include/sorafs_reference.h")"

if [[ "$skip_smoke" -eq 0 ]]; then
  "${stage_dir}/sorafs-validate" advert \
    --input "${workspace}/fixtures/sorafs_manifest/provider_admission/advert_v1.to" \
    --now 120 \
    --generated-at 123 \
    --format json > "${stage_dir}/smoke.advert.json"
  "${stage_dir}/sorafs-validate" bundle \
    --bundle "${workspace}/fixtures/sorafs_manifest" \
    --now 120 \
    --generated-at 123 \
    --format json > "${stage_dir}/smoke.bundle.json"
  smoke_advert_sha="$(sha256_file "${stage_dir}/smoke.advert.json")"
  smoke_bundle_sha="$(sha256_file "${stage_dir}/smoke.bundle.json")"
else
  smoke_advert_sha=""
  smoke_bundle_sha=""
fi

binary_sha="$(sha256_file "${stage_dir}/sorafs-validate")"
printf '%s  %s\n' "$binary_sha" "sorafs-validate" > "$binary_sha_path"

python3 - "$stage_dir" "$archive_path" "$package_name" <<'PY'
import gzip
import os
import stat
import sys
import tarfile
from pathlib import Path

stage_dir = Path(sys.argv[1])
archive_path = Path(sys.argv[2])
package_name = sys.argv[3]

def add_entry(tar, path, arcname):
    source = path.stat()
    info = tarfile.TarInfo(arcname)
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    info.pax_headers = {}
    if path.is_dir():
        info.type = tarfile.DIRTYPE
        info.mode = 0o755
        tar.addfile(info)
        return

    info.size = source.st_size
    info.mode = 0o755 if source.st_mode & stat.S_IXUSR else 0o644
    with path.open("rb") as handle:
        tar.addfile(info, handle)

with archive_path.open("wb") as raw:
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as gz:
        with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as tar:
            add_entry(tar, stage_dir, package_name)
            for path in sorted(stage_dir.rglob("*"), key=lambda item: item.relative_to(stage_dir).as_posix()):
                relative = path.relative_to(stage_dir).as_posix()
                add_entry(tar, path, f"{package_name}/{relative}")
PY
archive_sha="$(sha256_file "$archive_path")"
printf '%s  %s\n' "$archive_sha" "$(basename "$archive_path")" > "$archive_sha_path"

export SORAFS_VALIDATE_PACKAGE_VERSION="$version"
export SORAFS_VALIDATE_PACKAGE_TARGET="$target"
export SORAFS_VALIDATE_PACKAGE_PROFILE="$profile"
export SORAFS_VALIDATE_PACKAGE_ARCHIVE="$(basename "$archive_path")"
export SORAFS_VALIDATE_PACKAGE_ARCHIVE_SHA="$archive_sha"
export SORAFS_VALIDATE_PACKAGE_BINARY_SHA="$binary_sha"
export SORAFS_VALIDATE_PACKAGE_HEADER_SHA="$header_sha"
export SORAFS_VALIDATE_PACKAGE_HELP_SHA="$help_sha"
export SORAFS_VALIDATE_PACKAGE_SMOKE_ADVERT_SHA="$smoke_advert_sha"
export SORAFS_VALIDATE_PACKAGE_SMOKE_BUNDLE_SHA="$smoke_bundle_sha"
export SORAFS_VALIDATE_PACKAGE_SMOKE="$skip_smoke"
python3 - "$manifest_path" <<'PY'
import json
import os
import sys

stage_files = [
    {
        "path": "sorafs-validate",
        "sha256": os.environ["SORAFS_VALIDATE_PACKAGE_BINARY_SHA"],
    },
    {
        "path": "HELP.txt",
        "sha256": os.environ["SORAFS_VALIDATE_PACKAGE_HELP_SHA"],
    },
    {
        "path": "include/sorafs_reference.h",
        "sha256": os.environ["SORAFS_VALIDATE_PACKAGE_HEADER_SHA"],
    },
]
smoke_checks = []
if os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE"] == "0":
    smoke_checks.extend(
        [
            {
                "command": "sorafs-validate advert",
                "output": "smoke.advert.json",
                "sha256": os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE_ADVERT_SHA"],
            },
            {
                "command": "sorafs-validate bundle",
                "output": "smoke.bundle.json",
                "sha256": os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE_BUNDLE_SHA"],
            },
        ]
    )
    stage_files.extend(
        {
            "path": check["output"],
            "sha256": check["sha256"],
        }
        for check in smoke_checks
    )

manifest = {
    "schema_version": 1,
    "package": "sorafs-validate",
    "version": os.environ["SORAFS_VALIDATE_PACKAGE_VERSION"],
    "target": os.environ["SORAFS_VALIDATE_PACKAGE_TARGET"],
    "profile": os.environ["SORAFS_VALIDATE_PACKAGE_PROFILE"],
    "archive": os.environ["SORAFS_VALIDATE_PACKAGE_ARCHIVE"],
    "archive_sha256": os.environ["SORAFS_VALIDATE_PACKAGE_ARCHIVE_SHA"],
    "binary": "sorafs-validate",
    "binary_sha256": os.environ["SORAFS_VALIDATE_PACKAGE_BINARY_SHA"],
    "ffi_header": "include/sorafs_reference.h",
    "ffi_header_sha256": os.environ["SORAFS_VALIDATE_PACKAGE_HEADER_SHA"],
    "stage_files": stage_files,
    "smoke_checks": os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE"] == "0",
    "smoke_outputs": smoke_checks,
}
with open(sys.argv[1], "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
manifest_sha="$(sha256_file "$manifest_path")"
printf '%s  %s\n' "$manifest_sha" "$(basename "$manifest_path")" > "$manifest_sha_path"

if [[ -n "$manifest_signing_key" ]]; then
  mkdir -p "$(dirname "$manifest_signature_path")"
  openssl dgst -sha256 -sign "$manifest_signing_key" \
    -out "$manifest_signature_path" "$manifest_path"
  if [[ -n "$manifest_public_key" ]]; then
    openssl dgst -sha256 -verify "$manifest_public_key" \
      -signature "$manifest_signature_path" "$manifest_path" >/dev/null
  fi
fi

echo
echo "SoraFS reference validator release package:"
echo "  Archive : $archive_path"
echo "  Manifest: $manifest_path"
echo "  Manifest SHA256: $manifest_sha_path"
if [[ -f "$manifest_signature_path" ]]; then
  echo "  Manifest signature: $manifest_signature_path"
fi
echo "  Binary SHA256 : $binary_sha_path"
echo "  Archive SHA256: $archive_sha_path"
