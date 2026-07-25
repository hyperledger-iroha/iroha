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
                         Development-only owner-private raw 32-byte Ed25519 seed used for
                         local packager tests with --development-local-signing.
  --development-local-signing
                         Explicitly enable the development-only local raw-seed signer.
                         Reference-production signing must use the HSM input path.
  --manifest-signature-in <path>
                         Optional 64-byte raw Ed25519 signature produced by an
                         external PKCS#11/HSM signer for this exact manifest.
                         Mutually exclusive with --manifest-signing-key.
  --manifest-public-key <path>
                         Raw 32-byte Ed25519 public key required for signed manifests.
  --manifest-public-key-fingerprint <hex>
                         Required reviewed lowercase SHA256 fingerprint of the
                         exact 32 raw public-key bytes.
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

validate_signing_seed_permissions() {
  local label="$1"
  local target="$2"
  python3 - "$label" "$target" <<'PY'
import os
from pathlib import Path
import stat
import sys

label = sys.argv[1]
path = Path(sys.argv[2])

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

try:
    path_stat = path.lstat()
except OSError as error:
    fail(f"failed to inspect {label} permissions at `{path}`: {error}")

if not stat.S_ISREG(path_stat.st_mode):
    fail(f"{label} must be a regular file: {path}")
if path_stat.st_uid != os.geteuid():
    fail(f"{label} must be owned by the current effective user")
if path_stat.st_nlink != 1:
    fail(f"{label} must have exactly one hard link")

mode = stat.S_IMODE(path_stat.st_mode)
if mode not in {0o400, 0o600}:
    fail(f"{label} permissions must be owner-only 0400 or 0600")
PY
}

snapshot_release_signing_input() {
  local label="$1"
  local source_path="$2"
  local snapshot_path="$3"
  local input_kind="$4"
  python3 - "$label" "$source_path" "$snapshot_path" "$input_kind" <<'PY'
import os
from pathlib import Path
import stat
import sys

label = sys.argv[1]
source_path = Path(sys.argv[2])
snapshot_path = Path(sys.argv[3])
input_kind = sys.argv[4]
expected_bytes_by_kind = {
    "seed": 32,
    "public": 32,
    "signature": 64,
}

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

def read_flags():
    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )

def write_all(fd, payload):
    view = memoryview(payload)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write pinned signing input")
        view = view[written:]

def open_anchored_regular(path):
    if not path.is_absolute():
        fail(f"{label} must use an absolute path")
    components = path.parts[1:]
    if not components or any(component in {"", ".", ".."} for component in components):
        fail(f"{label} path is not canonical")
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    directory_fds = []
    try:
        current_fd = os.open("/", directory_flags)
        directory_fds.append(current_fd)
        for component in components[:-1]:
            current_fd = os.open(
                component,
                directory_flags,
                dir_fd=current_fd,
            )
            directory_fds.append(current_fd)
        leaf = components[-1]
        before = os.stat(leaf, dir_fd=current_fd, follow_symlinks=False)
        source_fd = os.open(leaf, read_flags(), dir_fd=current_fd)
        return source_fd, before, directory_fds
    except OSError:
        for directory_fd in reversed(directory_fds):
            os.close(directory_fd)
        raise

if input_kind not in expected_bytes_by_kind:
    fail("internal release signing input kind is invalid")
expected_bytes = expected_bytes_by_kind[input_kind]

source_fd = -1
snapshot_fd = -1
directory_fds = []
try:
    source_fd, before_path, directory_fds = open_anchored_regular(source_path)
    before = os.fstat(source_fd)
    if not stat.S_ISREG(before.st_mode):
        fail(f"{label} must be a regular file")
    if (before.st_dev, before.st_ino) != (before_path.st_dev, before_path.st_ino):
        fail(f"{label} changed before it could be pinned")
    if before.st_nlink != 1:
        fail(f"{label} must have exactly one hard link")
    if before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        fail(f"{label} must not be group- or world-writable")
    if input_kind == "seed":
        if before.st_uid != os.geteuid():
            fail(f"{label} must be owned by the current effective user")
        mode = stat.S_IMODE(before.st_mode)
        if mode not in {0o400, 0o600}:
            fail(f"{label} permissions must be owner-only 0400 or 0600")
    if before.st_size != expected_bytes:
        fail(f"{label} must contain exactly {expected_bytes} raw bytes")
    payload = bytearray()
    while len(payload) <= expected_bytes:
        chunk = os.read(source_fd, min(8192, expected_bytes + 1 - len(payload)))
        if not chunk:
            break
        payload.extend(chunk)
    if len(payload) != expected_bytes:
        fail(f"{label} must contain exactly {expected_bytes} raw bytes")
    after = os.fstat(source_fd)
    stable_fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(getattr(before, field) != getattr(after, field) for field in stable_fields):
        fail(f"{label} changed while it was pinned")

    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    snapshot_fd = os.open(snapshot_path, flags, 0o600)
    write_all(snapshot_fd, payload)
    os.fsync(snapshot_fd)
except FileNotFoundError:
    fail(f"{label} is missing")
except OSError as error:
    fail(f"failed to pin {label}: {error}")
finally:
    if source_fd >= 0:
        os.close(source_fd)
    if snapshot_fd >= 0:
        os.close(snapshot_fd)
    for directory_fd in reversed(directory_fds):
        os.close(directory_fd)
PY
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
development_local_signing=0
manifest_signature_input=""
manifest_public_key=""
manifest_public_key_fingerprint=""
manifest_signature_path=""
release_signing_snapshot_dir=""
skip_smoke=0

cleanup_release_signing_snapshot() {
  if [[ -z "$release_signing_snapshot_dir" ]]; then
    return
  fi
  rm -f -- \
    "${release_signing_snapshot_dir}/private.seed" \
    "${release_signing_snapshot_dir}/public.key" \
    "${release_signing_snapshot_dir}/external.sig" \
    "${release_signing_snapshot_dir}/preflight.sig" \
    "${release_signing_snapshot_dir}/generated.sig"
  rmdir -- "$release_signing_snapshot_dir" 2>/dev/null || true
}
trap cleanup_release_signing_snapshot EXIT

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
    --manifest-signing-key)
      require_option_value "$1" "${2-}"
      manifest_signing_key="$(abs_path "$2")"
      shift 2
      ;;
    --development-local-signing)
      development_local_signing=1
      shift
      ;;
    --manifest-signature-in)
      require_option_value "$1" "${2-}"
      manifest_signature_input="$(abs_path "$2")"
      shift 2
      ;;
    --manifest-public-key)
      require_option_value "$1" "${2-}"
      manifest_public_key="$(abs_path "$2")"
      shift 2
      ;;
    --manifest-public-key-fingerprint)
      require_option_value "$1" "${2-}"
      manifest_public_key_fingerprint="$2"
      shift 2
      ;;
    --manifest-signature-out)
      require_option_value "$1" "${2-}"
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

if [[ -n "$manifest_signing_key" ]]; then
  validate_existing_file_path "manifest signing key" "$manifest_signing_key"
  validate_signing_seed_permissions "manifest signing key" "$manifest_signing_key"
fi
if [[ -n "$manifest_signature_input" ]]; then
  validate_existing_file_path "external manifest signature" "$manifest_signature_input"
fi
if [[ -n "$manifest_public_key" ]]; then
  validate_existing_file_path "manifest public key" "$manifest_public_key"
fi
if [[ -n "$manifest_public_key_fingerprint" ]] &&
  [[ ! "$manifest_public_key_fingerprint" =~ ^[0-9a-f]{64}$ ]]; then
  echo "error: --manifest-public-key-fingerprint must be exact lowercase 32-byte SHA256 hex" >&2
  exit 1
fi
if [[ -n "$manifest_signing_key" && -n "$manifest_signature_input" ]]; then
  echo "error: --manifest-signing-key and --manifest-signature-in are mutually exclusive" >&2
  exit 1
fi
if [[ -n "$manifest_signing_key" && "$development_local_signing" -ne 1 ]]; then
  echo "error: --manifest-signing-key is development-only and requires --development-local-signing" >&2
  exit 1
fi
if [[ -z "$manifest_signing_key" && "$development_local_signing" -eq 1 ]]; then
  echo "error: --development-local-signing requires --manifest-signing-key" >&2
  exit 1
fi
manifest_signature_requested=0
if [[ -n "$manifest_signing_key" || -n "$manifest_signature_input" ]]; then
  manifest_signature_requested=1
fi
if [[ -n "$manifest_public_key" && "$manifest_signature_requested" -eq 0 ]]; then
  echo "error: --manifest-public-key requires --manifest-signing-key or --manifest-signature-in" >&2
  exit 1
fi
if [[ -n "$manifest_public_key_fingerprint" && "$manifest_signature_requested" -eq 0 ]]; then
  echo "error: --manifest-public-key-fingerprint requires --manifest-signing-key or --manifest-signature-in" >&2
  exit 1
fi
if [[ -n "$manifest_signature_path" && "$manifest_signature_requested" -eq 0 ]]; then
  echo "error: --manifest-signature-out requires --manifest-signing-key or --manifest-signature-in" >&2
  exit 1
fi
if [[ "$manifest_signature_requested" -eq 1 && -z "$manifest_public_key" ]]; then
  echo "error: signed manifests require --manifest-public-key" >&2
  exit 1
fi
if [[ "$manifest_signature_requested" -eq 1 && -z "$manifest_public_key_fingerprint" ]]; then
  echo "error: signed manifests require --manifest-public-key-fingerprint" >&2
  exit 1
fi

manifest_public_key_sha256=""

workspace="$(abs_path "$workspace")"
[[ -z "$out_dir" ]] && out_dir="${workspace}/dist/sorafs-validate-release"
[[ -z "$target" ]] && target="$(host_triple)"
[[ -z "$version" ]] && version="$(default_version)"

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

safe_version="${version//[^A-Za-z0-9._-]/_}"
safe_target="${target//[^A-Za-z0-9._-]/_}"
package_name="sorafs-validate-${safe_version}-${safe_target}"
stage_dir="${out_dir}/${package_name}"
archive_path="${out_dir}/${package_name}.tar.gz"
manifest_path="${out_dir}/${package_name}.manifest.json"
manifest_sha_path="${manifest_path}.sha256"
default_manifest_signature_path="${manifest_path}.sig"
if [[ "$manifest_signature_requested" -eq 1 && -z "$manifest_signature_path" ]]; then
  manifest_signature_path="$default_manifest_signature_path"
fi
binary_sha_path="${out_dir}/${package_name}.sha256"
archive_sha_path="${archive_path}.sha256"
header_path="${workspace}/crates/sorafs_manifest/include/sorafs_reference.h"

reject_release_key_cleanup_target() {
  local label="$1"
  local key_path="$2"
  case "$key_path" in
    "$stage_dir"|"$stage_dir"/*|"$archive_path"|"$manifest_path"|"$manifest_sha_path"|"$binary_sha_path"|"$archive_sha_path")
      echo "error: ${label} must be kept outside generated release artifact paths" >&2
      exit 1
      ;;
  esac
}

validate_new_manifest_signature_output() {
  local output_path="$1"
  shift
  python3 - "$output_path" "$stage_dir" "$@" <<'PY'
import os
from pathlib import Path
import sys

path = Path(sys.argv[1])
stage_dir = Path(sys.argv[2])
generated_paths = [Path(value) for value in sys.argv[3:]]

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

def normalized(value):
    return Path(os.path.abspath(value))

try:
    if path.is_symlink():
        fail(f"release manifest signature output `{path}` must not be a symlink")
    for parent in (path.parent, *path.parent.parents):
        if parent.is_symlink():
            fail(f"release manifest signature output parent `{parent}` must not be a symlink")
        if parent.exists() and not parent.is_dir():
            fail(f"release manifest signature output parent `{parent}` must be a directory")
    try:
        path.lstat()
    except FileNotFoundError:
        pass
    else:
        fail(f"release manifest signature output `{path}` must not already exist")
except OSError as error:
    fail(f"failed to inspect release manifest signature output `{path}`: {error}")

normalized_path = normalized(path)
normalized_stage = normalized(stage_dir)
try:
    normalized_path.relative_to(normalized_stage)
except ValueError:
    pass
else:
    fail("release manifest signature output must not collide with the generated stage")
if normalized_path in {normalized(value) for value in generated_paths}:
    fail("release manifest signature output must not collide with a generated release artifact")
PY
}

validate_existing_file_path "SoraFS reference FFI header" "$header_path"

if [[ "$manifest_signature_path" != "$default_manifest_signature_path" ]] &&
  [[ -e "$default_manifest_signature_path" || -L "$default_manifest_signature_path" ]]; then
  echo "error: stale default manifest signature exists; refuse to publish a replacement manifest" >&2
  exit 1
fi

if [[ "$manifest_signature_requested" -eq 1 ]]; then
  reject_release_key_cleanup_target "manifest public key" "$manifest_public_key"
  if [[ -n "$manifest_signing_key" ]]; then
    reject_release_key_cleanup_target "manifest signing key" "$manifest_signing_key"
  else
    reject_release_key_cleanup_target \
      "external manifest signature" "$manifest_signature_input"
  fi
  if [[ "$manifest_signature_path" == "$manifest_public_key" ]] ||
    [[ -n "$manifest_signing_key" &&
      "$manifest_signature_path" == "$manifest_signing_key" ]] ||
    [[ -n "$manifest_signature_input" &&
      "$manifest_signature_path" == "$manifest_signature_input" ]]; then
    echo "error: release manifest signature output must not overwrite a manifest key or external signature input" >&2
    exit 1
  fi
  validate_new_manifest_signature_output \
    "$manifest_signature_path" \
    "$archive_path" "$manifest_path" "$manifest_sha_path" \
    "$binary_sha_path" "$archive_sha_path"

  manifest_signing_key_source="$manifest_signing_key"
  manifest_signature_input_source="$manifest_signature_input"
  manifest_public_key_source="$manifest_public_key"
  release_signing_snapshot_dir="$(
    mktemp -d "${TMPDIR:-/tmp}/sorafs-release-signing.XXXXXXXX"
  )"
  chmod 0700 "$release_signing_snapshot_dir"
  release_signing_snapshot_dir="$(
    cd "$release_signing_snapshot_dir" && pwd -P
  )"
  snapshot_release_signing_input \
    "manifest public key" "$manifest_public_key_source" \
    "${release_signing_snapshot_dir}/public.key" "public"
  manifest_public_key="${release_signing_snapshot_dir}/public.key"
  if [[ -n "$manifest_signing_key_source" ]]; then
    snapshot_release_signing_input \
      "manifest signing key" "$manifest_signing_key_source" \
      "${release_signing_snapshot_dir}/private.seed" "seed"
    manifest_signing_key="${release_signing_snapshot_dir}/private.seed"
  else
    snapshot_release_signing_input \
      "external manifest signature" "$manifest_signature_input_source" \
      "${release_signing_snapshot_dir}/external.sig" "signature"
    manifest_signature_input="${release_signing_snapshot_dir}/external.sig"
  fi

  manifest_public_key_sha256="$(sha256_file "$manifest_public_key")"
  if [[ "$manifest_public_key_fingerprint" != "$manifest_public_key_sha256" ]]; then
    echo "error: pinned manifest public key does not match the reviewed fingerprint" >&2
    exit 1
  fi
  if [[ -n "$manifest_signing_key" ]]; then
    preflight_signature="${release_signing_snapshot_dir}/preflight.sig"
    if ! "$binary_path" release-manifest \
      --manifest "$header_path" \
      --public-key "$manifest_public_key" \
      --public-key-fingerprint "$manifest_public_key_fingerprint" \
      --signing-seed "$manifest_signing_key" \
      --signature-out "$preflight_signature" \
      --development-local-signing >/dev/null; then
      echo "error: native development-only Ed25519 signing-key preflight failed" >&2
      exit 1
    fi
    rm -f -- "$preflight_signature"
  fi
fi

prepare_output_directory_path "release output directory" "$out_dir"
rm -rf "$stage_dir" "$archive_path" "$manifest_path" "$manifest_sha_path" \
  "$binary_sha_path" "$archive_sha_path"
mkdir -p "$stage_dir/include"
cp "$binary_path" "${stage_dir}/${packaged_binary_name}"
cp "$header_path" "${stage_dir}/include/sorafs_reference.h"

"${stage_dir}/${packaged_binary_name}" --help > "${stage_dir}/HELP.txt"
help_sha="$(sha256_file "${stage_dir}/HELP.txt")"
header_sha="$(sha256_file "${stage_dir}/include/sorafs_reference.h")"

if [[ "$skip_smoke" -eq 0 ]]; then
  "${stage_dir}/${packaged_binary_name}" advert \
    --input "${workspace}/fixtures/sorafs_manifest/provider_admission/advert_v1.to" \
    --now 120 \
    --generated-at 123 \
    --format json > "${stage_dir}/smoke.advert.json"
  "${stage_dir}/${packaged_binary_name}" bundle \
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

write_sha256_sidecar() {
  local output_path="$1"
  local digest="$2"
  local file_name="$3"
  python3 - "$output_path" "$digest" "$file_name" <<'PY'
import os
from pathlib import Path
import stat
import sys

path = Path(sys.argv[1])
digest = sys.argv[2]
file_name = sys.argv[3]

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

def write_open_flags():
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags

def sync_output_parent(path):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def write_all(fd, chunk):
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write SoraFS release checksum sidecar")
        view = view[written:]

def validate_output_path(path):
    try:
        if path.is_symlink():
            fail(f"release checksum sidecar `{path}` must not be a symlink")
        for parent in (path.parent, *path.parent.parents):
            if parent.is_symlink():
                fail(f"release checksum sidecar parent `{parent}` must not be a symlink")
            if parent.exists() and not parent.is_dir():
                fail(f"release checksum sidecar parent `{parent}` must be a directory")
    except OSError as error:
        fail(f"failed to inspect release checksum sidecar `{path}`: {error}")

if not digest or any(character not in "0123456789abcdef" for character in digest):
    fail("release checksum sidecar digest must be lowercase hex")
if not file_name or "/" in file_name or "\\" in file_name or file_name in {".", ".."}:
    fail("release checksum sidecar filename must be a basename")

validate_output_path(path)
body = f"{digest}  {file_name}\n".encode("utf-8")
fd = -1
try:
    fd = os.open(path, write_open_flags(), 0o666)
    if not stat.S_ISREG(os.fstat(fd).st_mode):
        fail(f"release checksum sidecar `{path}` must be a regular file")
    write_all(fd, body)
    os.fsync(fd)
finally:
    if fd >= 0:
        os.close(fd)
sync_output_parent(path)
PY
}

binary_sha="$(sha256_file "${stage_dir}/${packaged_binary_name}")"
write_sha256_sidecar "$binary_sha_path" "$binary_sha" "$packaged_binary_name"

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

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

def read_open_flags():
    flags = os.O_RDONLY
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags

def write_open_flags():
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags

def sync_output_parent(path):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def validate_archive_path(path, label):
    try:
        if path.is_symlink():
            fail(f"{label} `{path}` must not be a symlink")
        for parent in (path.parent, *path.parent.parents):
            if parent.is_symlink():
                fail(f"{label} parent `{parent}` must not be a symlink")
            if parent.exists() and not parent.is_dir():
                fail(f"{label} parent `{parent}` must be a directory when it exists")
    except OSError as error:
        fail(f"failed to inspect {label} `{path}`: {error}")

def scan_stage_entries(root):
    validate_archive_path(root, "release package stage root")
    try:
        return sorted(
            root.rglob("*"),
            key=lambda item: item.relative_to(root).as_posix(),
        )
    except OSError as error:
        fail(f"failed to scan release package stage root `{root}`: {error}")

def add_entry(tar, path, arcname):
    validate_archive_path(path, "release package entry")
    source = path.lstat()
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

    if not stat.S_ISREG(source.st_mode):
        fail(f"release package entry `{path}` must be a regular file or directory")
    info.size = source.st_size
    info.mode = 0o755 if source.st_mode & stat.S_IXUSR else 0o644
    fd = -1
    try:
        fd = os.open(path, read_open_flags())
        handle = os.fdopen(fd, "rb")
        fd = -1
        with handle:
            tar.addfile(info, handle)
    finally:
        if fd >= 0:
            os.close(fd)

validate_archive_path(archive_path, "release package archive")
archive_fd = -1
try:
    archive_fd = os.open(archive_path, write_open_flags(), 0o666)
    raw = os.fdopen(archive_fd, "wb")
    archive_fd = -1
    with raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as gz:
            with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as tar:
                add_entry(tar, stage_dir, package_name)
                for path in scan_stage_entries(stage_dir):
                    relative = path.relative_to(stage_dir).as_posix()
                    add_entry(tar, path, f"{package_name}/{relative}")
        raw.flush()
        os.fsync(raw.fileno())
finally:
    if archive_fd >= 0:
        os.close(archive_fd)
sync_output_parent(archive_path)
PY
archive_sha="$(sha256_file "$archive_path")"
write_sha256_sidecar "$archive_sha_path" "$archive_sha" "$(basename "$archive_path")"

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
export SORAFS_VALIDATE_PACKAGE_BINARY="$packaged_binary_name"
python3 - "$manifest_path" <<'PY'
import json
import os
from pathlib import Path
import stat
import sys

manifest_path = Path(sys.argv[1])

def write_open_flags():
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

def write_all(fd, chunk):
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write SoraFS release manifest")
        view = view[written:]

def sync_output_parent(path):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def validate_manifest_output_path(path):
    try:
        if path.is_symlink():
            fail(f"release manifest output `{path}` must not be a symlink")
        for parent in (path.parent, *path.parent.parents):
            if parent.is_symlink():
                fail(f"release manifest output parent `{parent}` must not be a symlink")
            if parent.exists() and not parent.is_dir():
                fail(f"release manifest output parent `{parent}` must be a directory")
    except OSError as error:
        fail(f"failed to inspect release manifest output `{path}`: {error}")

def write_manifest_no_follow(path, payload):
    validate_manifest_output_path(path)
    rendered = (
        json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    ).encode("utf-8")
    fd = -1
    try:
        fd = os.open(path, write_open_flags(), 0o666)
        descriptor_stat = os.fstat(fd)
        if not stat.S_ISREG(descriptor_stat.st_mode):
            fail(f"release manifest output `{path}` must be a regular file")
        write_all(fd, rendered)
        os.fsync(fd)
    finally:
        if fd >= 0:
            os.close(fd)
    sync_output_parent(path)

stage_files = [
    {
        "path": os.environ["SORAFS_VALIDATE_PACKAGE_BINARY"],
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
    "binary": os.environ["SORAFS_VALIDATE_PACKAGE_BINARY"],
    "binary_sha256": os.environ["SORAFS_VALIDATE_PACKAGE_BINARY_SHA"],
    "ffi_header": "include/sorafs_reference.h",
    "ffi_header_sha256": os.environ["SORAFS_VALIDATE_PACKAGE_HEADER_SHA"],
    "stage_files": stage_files,
    "smoke_checks": os.environ["SORAFS_VALIDATE_PACKAGE_SMOKE"] == "0",
    "smoke_outputs": smoke_checks,
}
write_manifest_no_follow(manifest_path, manifest)
PY
manifest_sha="$(sha256_file "$manifest_path")"
write_sha256_sidecar "$manifest_sha_path" "$manifest_sha" "$(basename "$manifest_path")"

install_manifest_signature() {
  local source_path="$1"
  local target_path="$2"
  local signed_manifest_path="$3"
  python3 - "$source_path" "$target_path" "$signed_manifest_path" <<'PY'
import os
from pathlib import Path
import stat
import sys

source_path = Path(sys.argv[1])
target_path = Path(sys.argv[2])
manifest_path = Path(sys.argv[3])

def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)

def read_open_flags():
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags

def write_open_flags():
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
    )
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags

def sync_output_parent(path):
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    fd = os.open(path.parent, flags)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def write_all(fd, chunk):
    view = memoryview(chunk)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("failed to write SoraFS release manifest signature")
        view = view[written:]

def validate_source(path):
    try:
        if path.is_symlink():
            fail(f"release manifest signature source `{path}` must not be a symlink")
        path_stat = path.lstat()
    except FileNotFoundError:
        fail(f"release manifest signature source `{path}` is missing")
    except OSError as error:
        fail(f"failed to inspect release manifest signature source `{path}`: {error}")
    if not stat.S_ISREG(path_stat.st_mode):
        fail(f"release manifest signature source `{path}` must be a regular file")
    if path_stat.st_size <= 0:
        fail(f"release manifest signature source `{path}` must not be empty")

def validate_target(path):
    try:
        if path.is_symlink():
            fail(f"release manifest signature output `{path}` must not be a symlink")
        for parent in (path.parent, *path.parent.parents):
            if parent.is_symlink():
                fail(f"release manifest signature output parent `{parent}` must not be a symlink")
            if parent.exists() and not parent.is_dir():
                fail(f"release manifest signature output parent `{parent}` must be a directory")
    except OSError as error:
        fail(f"failed to inspect release manifest signature output `{path}`: {error}")

def same_existing_file(left, right):
    try:
        left_stat = left.lstat()
        right_stat = right.lstat()
    except FileNotFoundError:
        return False
    except OSError as error:
        fail(f"failed to compare release manifest signature output `{left}`: {error}")
    return (left_stat.st_dev, left_stat.st_ino) == (
        right_stat.st_dev,
        right_stat.st_ino,
    )

validate_source(source_path)
validate_target(target_path)
target_path.parent.mkdir(parents=True, exist_ok=True)
validate_target(target_path)
if same_existing_file(target_path, manifest_path):
    fail("release manifest signature output must not overwrite the manifest")

read_fd = -1
write_fd = -1
try:
    read_fd = os.open(source_path, read_open_flags())
    if not stat.S_ISREG(os.fstat(read_fd).st_mode):
        fail(f"release manifest signature source `{source_path}` must be a regular file")
    write_fd = os.open(target_path, write_open_flags(), 0o666)
    if not stat.S_ISREG(os.fstat(write_fd).st_mode):
        fail(f"release manifest signature output `{target_path}` must be a regular file")
    while True:
        chunk = os.read(read_fd, 1024 * 1024)
        if not chunk:
            break
        write_all(write_fd, chunk)
    os.fsync(write_fd)
finally:
    if read_fd >= 0:
        os.close(read_fd)
    if write_fd >= 0:
        os.close(write_fd)
sync_output_parent(target_path)
PY
}

if [[ "$manifest_signature_requested" -eq 1 ]]; then
  signature_source_path="$manifest_signature_input"
  signature_tmp_path="${release_signing_snapshot_dir}/generated.sig"
  if [[ -n "$manifest_signing_key" ]]; then
    signature_source_path="$signature_tmp_path"
    if ! "${stage_dir}/${packaged_binary_name}" release-manifest \
      --manifest "$manifest_path" \
      --public-key "$manifest_public_key" \
      --public-key-fingerprint "$manifest_public_key_fingerprint" \
      --signing-seed "$manifest_signing_key" \
      --signature-out "$signature_tmp_path" \
      --development-local-signing >/dev/null; then
      echo "error: native development-only Ed25519 manifest signing failed" >&2
      exit 1
    fi
  elif ! "${stage_dir}/${packaged_binary_name}" release-manifest \
    --manifest "$manifest_path" \
    --public-key "$manifest_public_key" \
    --public-key-fingerprint "$manifest_public_key_fingerprint" \
    --signature "$signature_source_path" >/dev/null; then
    echo "error: native external Ed25519 manifest signature verification failed" >&2
    exit 1
  fi
  if ! install_manifest_signature \
    "$signature_source_path" "$manifest_signature_path" "$manifest_path"; then
    exit 1
  fi
  if ! "${stage_dir}/${packaged_binary_name}" release-manifest \
    --manifest "$manifest_path" \
    --public-key "$manifest_public_key" \
    --public-key-fingerprint "$manifest_public_key_fingerprint" \
    --signature "$manifest_signature_path" >/dev/null; then
    echo "error: installed native Ed25519 manifest signature verification failed" >&2
    exit 1
  fi
fi

echo
echo "SoraFS reference validator release package:"
echo "  Archive : $archive_path"
echo "  Manifest: $manifest_path"
echo "  Manifest SHA256: $manifest_sha_path"
if [[ "$manifest_signature_requested" -eq 1 ]]; then
  echo "  Manifest signature: $manifest_signature_path"
  echo "  Manifest signer fingerprint (reviewed): $manifest_public_key_sha256"
fi
echo "  Binary SHA256 : $binary_sha_path"
echo "  Archive SHA256: $archive_sha_path"
