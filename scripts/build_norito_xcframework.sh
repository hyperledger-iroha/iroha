#!/usr/bin/env bash
set -euo pipefail
umask 077
PATH=/usr/bin:/bin
export PATH
unset \
  DYLD_INSERT_LIBRARIES \
  DYLD_LIBRARY_PATH \
  LD_LIBRARY_PATH \
  LD_PRELOAD \
  SDKROOT \
  PYTHONHOME \
  PYTHONPATH

resolve_trusted_python312() {
  local candidate canonical
  local override="${MOBILE_SDK_PYTHON_BINARY:-}"
  local candidates=()

  if [[ -n "$override" ]]; then
    if [[ "$override" != /* || ! -f "$override" || -L "$override" || ! -x "$override" ]]; then
      echo "[-] MOBILE_SDK_PYTHON_BINARY must be an absolute canonical regular executable" >&2
      return 1
    fi
    candidates=("$override")
  else
    candidates=(
      /opt/homebrew/opt/python@3.12/bin/python3.12
      /opt/homebrew/bin/python3.12
      /usr/local/opt/python@3.12/bin/python3.12
      /usr/local/bin/python3.12
      /usr/bin/python3.12
      /usr/bin/python3
    )
  fi

  for candidate in "${candidates[@]}"; do
    [[ -f "$candidate" && -x "$candidate" ]] || continue
    if ! canonical="$(
      env -i \
        HOME=/tmp \
        PATH=/usr/bin:/bin \
        TMPDIR=/tmp \
        LANG=C.UTF-8 \
        LC_ALL=C.UTF-8 \
        "$candidate" -I -S -B -c '
import os
import pathlib
import stat
import sys

if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
    raise SystemExit(1)
if "SDKROOT" in os.environ:
    raise SystemExit(1)
resolved = pathlib.Path(sys.executable).resolve(strict=True)
metadata = resolved.stat()
if not stat.S_ISREG(metadata.st_mode) or not os.access(resolved, os.X_OK):
    raise SystemExit(1)
print(resolved)
'
    )"; then
      continue
    fi
    if [[ "$canonical" != /* || ! -f "$canonical" || -L "$canonical" || ! -x "$canonical" ]]; then
      continue
    fi
    if [[ -n "$override" && "$canonical" != "$override" ]]; then
      echo "[-] MOBILE_SDK_PYTHON_BINARY must already name its canonical executable" >&2
      return 1
    fi
    printf '%s\n' "$canonical"
    return 0
  done

  if [[ -n "$override" ]]; then
    echo "[-] MOBILE_SDK_PYTHON_BINARY must be an isolated Python 3.12 executable" >&2
  else
    echo "[-] A trusted absolute Python 3.12 executable is required" >&2
  fi
  return 1
}

PYTHON_BINARY="$(resolve_trusted_python312)" || exit 1

run_python312_clean() {
  env -i \
    HOME=/tmp \
    PATH=/usr/bin:/bin \
    TMPDIR=/tmp \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$PYTHON_BINARY" -I -S -B "$@"
}

# Purpose: build or assemble a release-authenticated NoritoBridge.xcframework
# from the Rust connect_norito_bridge crate. Matrix producers compile one thin
# Apple slice; the assembler accepts only five run-bound evidence bundles and
# never compiles Rust.
# - Produces a static-library XCFramework with iOS device, universal iOS
#   simulator, and universal macOS slices so Xcode links it without trying to
#   embed/sign a framework inside simulator app bundles.
# - Bridge packaging skips the broader Norito bindings sync gate because unrelated
#   Kotlin/Java parity drift should not block rebuilding the Swift bridge artifact.
# Prerequisites: macOS with Python 3.12, Git, Xcode (xcodebuild/xcrun/lipo/nm),
# rustup, exact Rust 1.93.1 Cargo/rustc/rustdoc, installed canonical Apple Rust
# targets, and an offline-populated Cargo home.
# - IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH selects the authenticated external
#   privacy-release lock; ordinary builds consume the repository-root lock.
# - MOBILE_SDK_PYTHON_BINARY may select an absolute canonical Python 3.12
#   executable when the fixed Homebrew/system locators are unavailable.
# - Required environment: NORITO_BRIDGE_OUT_DIR, NORITO_BRIDGE_BUILD_DIR,
#   CARGO_TARGET_DIR, RUSTC, RUSTDOC, CARGO_BUILD_JOBS=1, CARGO_INCREMENTAL=0,
#   CARGO_NET_OFFLINE=true, and RUSTC_BOOTSTRAP=1.
# - Matrix-only environment: NORITO_BRIDGE_SLICE_BUILD_ID must be the same
#   canonical run-attempt token for all five producers and their assembler.
#
# Usage:
#   scripts/build_norito_xcframework.sh
#   scripts/build_norito_xcframework.sh --bridge-version 1.0.0
#   scripts/build_norito_xcframework.sh --archive-output /absolute/NoritoBridge.xcframework.zip
#   scripts/build_norito_xcframework.sh --privacy-production-enabled
#   scripts/build_norito_xcframework.sh --privacy-production-enabled --allow-dirty-source
#   scripts/build_norito_xcframework.sh --produce-slice ios-arm64 --slice-output-root /absolute/fresh-root
#   scripts/build_norito_xcframework.sh --assemble-slices /absolute/fresh-common-root
#
# NORITO_BRIDGE_OUT_DIR and NORITO_BRIDGE_BUILD_DIR are mandatory external
# cache roots. The first-release owner never creates build or artifact output
# inside the reviewed repository.

print_usage() {
  cat <<EOF
Usage:
  $0 [--bridge-version <version>] [--archive-output <absolute.zip>] [--privacy-production-enabled] [--allow-dirty-source]
  $0 --produce-slice <ios-arm64|ios-sim-arm64|ios-sim-x64|macos-arm64|macos-x64> --slice-output-root <absolute-empty-root> [--bridge-version <version>] [--privacy-production-enabled] [--allow-dirty-source]
  $0 --assemble-slices <absolute-common-root> [--bridge-version <version>] [--archive-output <absolute.zip>] [--privacy-production-enabled] [--allow-dirty-source]
EOF
}
if [[ $# -eq 1 && ( "$1" == --help || "$1" == -h ) ]]; then
  print_usage
  exit 0
fi

SCRIPT_DIR="$(cd "${BASH_SOURCE[0]%/*}" && pwd -P)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
CRATE_DIR="$ROOT_DIR/crates/connect_norito_bridge"
INC_DIR="$CRATE_DIR/include"
if [[ -z "${NORITO_BRIDGE_OUT_DIR:-}" || -z "${NORITO_BRIDGE_BUILD_DIR:-}" ]]; then
  echo "[-] NORITO_BRIDGE_OUT_DIR and NORITO_BRIDGE_BUILD_DIR are required external cache directories" >&2
  exit 1
fi
OUT_DIR="$NORITO_BRIDGE_OUT_DIR"
BUILD_DIR="$NORITO_BRIDGE_BUILD_DIR"
PUBLISH_ROOT=""

reject_retired_mode() {
  local retired_name="$1"
  local retired_presence="$2"
  if [[ "$retired_presence" == "set" ]]; then
    echo "[-] $retired_name is not part of the first-release build contract" >&2
    exit 1
  fi
}
reject_retired_mode \
  NORITO_BRIDGE_BUILD_LOCK_HELD "${NORITO_BRIDGE_BUILD_LOCK_HELD+set}"
reject_retired_mode \
  NORITO_BRIDGE_PRESERVE_CARGO_TARGETS "${NORITO_BRIDGE_PRESERVE_CARGO_TARGETS+set}"
reject_retired_mode \
  NORITO_BRIDGE_SKIP_CARGO_BUILDS "${NORITO_BRIDGE_SKIP_CARGO_BUILDS+set}"
if [[ "${CARGO_BUILD_JOBS:-}" != "1" \
    || "${CARGO_INCREMENTAL:-}" != "0" \
    || "${CARGO_NET_OFFLINE:-}" != "true" \
    || "${RUSTC_BOOTSTRAP:-}" != "1" ]]; then
  echo "[-] NoritoBridge requires CARGO_BUILD_JOBS=1, CARGO_INCREMENTAL=0, CARGO_NET_OFFLINE=true, and RUSTC_BOOTSTRAP=1" >&2
  exit 1
fi
if [[ -z "${CARGO_TARGET_DIR:-}" || -z "${RUSTC:-}" || -z "${RUSTDOC:-}" ]]; then
  echo "[-] NoritoBridge requires explicit CARGO_TARGET_DIR, RUSTC, and RUSTDOC" >&2
  exit 1
fi
CARGO_TARGET_DIR="$(run_python312_clean - "$CARGO_TARGET_DIR" "$ROOT_DIR" <<'PY'
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
source_root = Path(sys.argv[2])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit("CARGO_TARGET_DIR must be an absolute canonical directory")
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit("CARGO_TARGET_DIR must already exist") from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or metadata.st_uid != os.geteuid()
    or not os.access(candidate, os.R_OK | os.W_OK | os.X_OK)
    or candidate == source_root
    or source_root in candidate.parents
):
    raise SystemExit(
        "CARGO_TARGET_DIR must be one writable, non-symbolic canonical directory "
        "outside the Iroha source tree"
    )
print(candidate)
PY
)" || exit 1
export CARGO_TARGET_DIR

canonical_writable_directory() {
  run_python312_clean - "$1" "$2" "$ROOT_DIR" <<'PY'
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
label = sys.argv[2]
source_root = Path(sys.argv[3])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit(f"{label} must be an absolute canonical directory")
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(f"{label} must already exist") from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or metadata.st_uid != os.geteuid()
    or not os.access(candidate, os.R_OK | os.W_OK | os.X_OK)
    or candidate == source_root
    or source_root in candidate.parents
):
    raise SystemExit(
        f"{label} must be a writable, non-symbolic canonical directory "
        "outside the Iroha source tree"
    )
print(candidate)
PY
}
OUT_DIR="$(canonical_writable_directory "$OUT_DIR" NORITO_BRIDGE_OUT_DIR)" || exit 1
BUILD_DIR="$(canonical_writable_directory "$BUILD_DIR" NORITO_BRIDGE_BUILD_DIR)" || exit 1
paths_overlap() {
  [[ "$1" == "$2" || "$1" == "$2"/* || "$2" == "$1"/* ]]
}
if paths_overlap "$CARGO_TARGET_DIR" "$BUILD_DIR" \
    || paths_overlap "$CARGO_TARGET_DIR" "$OUT_DIR" \
    || paths_overlap "$BUILD_DIR" "$OUT_DIR"; then
  echo "[-] NoritoBridge Cargo target, build, and output directories must be pairwise disjoint" >&2
  exit 1
fi
STAGE_DIR="$BUILD_DIR/stage"

TARGET_BUILD_LOCK="$CARGO_TARGET_DIR/.NoritoBridge.cargo.lockfile"
STAGE_BUILD_LOCK="$BUILD_DIR/.NoritoBridge.stage.lockfile"
OUTPUT_PUBLISH_LOCK="$OUT_DIR/.NoritoBridge.publish.lockfile"
LOCK_DESCRIPTOR_ENV=NORITO_BRIDGE_BUILD_LOCK_FDS
if [[ -z "${NORITO_BRIDGE_BUILD_LOCK_FDS:-}" ]]; then
  LOCK_RUNNER="$ROOT_DIR/scripts/exec_with_file_lock.py"
  [[ -f "$LOCK_RUNNER" && ! -L "$LOCK_RUNNER" ]] || {
    echo "[-] NoritoBridge build lock runner is unavailable: $LOCK_RUNNER" >&2
    exit 1
  }
  exec "$PYTHON_BINARY" -I -S -B "$LOCK_RUNNER" \
    "$LOCK_DESCRIPTOR_ENV" \
    "$TARGET_BUILD_LOCK" "$STAGE_BUILD_LOCK" "$OUTPUT_PUBLISH_LOCK" \
    -- \
    "$SCRIPT_DIR/build_norito_xcframework.sh" "$@"
fi
NORITO_BRIDGE_OUTPUT_LOCK_FD="$(run_python312_clean - \
  "$NORITO_BRIDGE_BUILD_LOCK_FDS" \
  "$OUTPUT_PUBLISH_LOCK" \
  "$TARGET_BUILD_LOCK" "$STAGE_BUILD_LOCK" "$OUTPUT_PUBLISH_LOCK" <<'PY'
import fcntl
import os
from pathlib import Path
import stat
import sys


descriptors = sys.argv[1].split(",")
output_lock = Path(sys.argv[2])
lock_paths = sorted((Path(value) for value in sys.argv[3:]), key=os.fspath)
if len(descriptors) != len(lock_paths) or any(not value.isdecimal() for value in descriptors):
    raise SystemExit("NoritoBridge build lock descriptor inventory is malformed")
for raw_descriptor, lock_path in zip(descriptors, lock_paths, strict=True):
    descriptor = int(raw_descriptor)
    try:
        descriptor_metadata = os.fstat(descriptor)
        path_metadata = lock_path.lstat()
    except OSError as error:
        raise SystemExit(f"NoritoBridge build lock is unavailable: {error}") from None
    if (
        not stat.S_ISREG(descriptor_metadata.st_mode)
        or descriptor_metadata.st_nlink != 1
        or descriptor_metadata.st_uid != os.geteuid()
        or (descriptor_metadata.st_dev, descriptor_metadata.st_ino)
        != (path_metadata.st_dev, path_metadata.st_ino)
    ):
        raise SystemExit(f"NoritoBridge build lock is not authenticated: {lock_path}")
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        raise SystemExit(f"NoritoBridge build lock is not held: {lock_path}: {error}") from None
print(descriptors[lock_paths.index(output_lock)])
PY
)" || exit 1
export NORITO_BRIDGE_OUTPUT_LOCK_FD

LIB_CRATE_NAME="connect_norito_bridge"
FRAMEWORK_NAME="NoritoBridge"
STATIC_LIB_NAME="libNoritoBridge.a"
cleanup_build_state() {
  local status=$?
  trap - EXIT HUP INT TERM
  set +e
  if [[ -n "$PUBLISH_ROOT" && -d "$PUBLISH_ROOT" \
    && "${PUBLISH_ROOT##*/}" == .NoritoBridge.publish.* ]]; then
    rm -rf -- "$PUBLISH_ROOT"
  fi
  exit "$status"
}
trap cleanup_build_state EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

set_fixed_deployment_target() {
  local deployment_name="$1"
  local deployment_value="$2"
  local ambient_presence="$3"
  local ambient_value="$4"
  if [[ "$ambient_presence" == "set" && "$ambient_value" != "$deployment_value" ]]; then
    echo "[-] $deployment_name is fixed at $deployment_value for the first release" >&2
    exit 1
  fi
  printf -v "$deployment_name" '%s' "$deployment_value"
  export "$deployment_name"
}
set_fixed_deployment_target \
  IPHONEOS_DEPLOYMENT_TARGET 15.0 \
  "${IPHONEOS_DEPLOYMENT_TARGET+set}" "${IPHONEOS_DEPLOYMENT_TARGET-}"
set_fixed_deployment_target \
  IPHONESIMULATOR_DEPLOYMENT_TARGET 15.0 \
  "${IPHONESIMULATOR_DEPLOYMENT_TARGET+set}" "${IPHONESIMULATOR_DEPLOYMENT_TARGET-}"
set_fixed_deployment_target \
  MACOSX_DEPLOYMENT_TARGET 12.0 \
  "${MACOSX_DEPLOYMENT_TARGET+set}" "${MACOSX_DEPLOYMENT_TARGET-}"
readonly IPHONEOS_DEPLOYMENT_TARGET
readonly IPHONESIMULATOR_DEPLOYMENT_TARGET
readonly MACOSX_DEPLOYMENT_TARGET
export IPHONEOS_DEPLOYMENT_TARGET
export IPHONESIMULATOR_DEPLOYMENT_TARGET
export MACOSX_DEPLOYMENT_TARGET

BRIDGE_VERSION=""
ARCHIVE_OUTPUT=""
PRIVACY_PRODUCTION_ENABLED=0
ALLOW_DIRTY_SOURCE=0
PRODUCE_SLICE_ID=""
SLICE_OUTPUT_ROOT=""
ASSEMBLE_SLICE_ROOT=""
if [[ -n "${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH+x}" ]]; then
  if [[ -z "${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH}" ]]; then
    echo "[-] IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH must not be empty" >&2
    exit 1
  fi
  CARGO_LOCKFILE="$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"
else
  CARGO_LOCKFILE="$ROOT_DIR/Cargo.lock"
fi
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bridge-version)
      shift
      BRIDGE_VERSION="${1:-}"
      if [[ -z "$BRIDGE_VERSION" ]]; then
        echo "[-] --bridge-version requires a value" >&2
        exit 1
      fi
      ;;
    --bridge-version=*)
      BRIDGE_VERSION="${1#*=}"
      ;;
    --archive-output)
      shift
      ARCHIVE_OUTPUT="${1:-}"
      if [[ -z "$ARCHIVE_OUTPUT" ]]; then
        echo "[-] --archive-output requires a value" >&2
        exit 1
      fi
      ;;
    --archive-output=*)
      ARCHIVE_OUTPUT="${1#*=}"
      if [[ -z "$ARCHIVE_OUTPUT" ]]; then
        echo "[-] --archive-output requires a value" >&2
        exit 1
      fi
      ;;
    --privacy-production-enabled)
      PRIVACY_PRODUCTION_ENABLED=1
      ;;
    --allow-dirty-source)
      ALLOW_DIRTY_SOURCE=1
      ;;
    --produce-slice)
      shift
      PRODUCE_SLICE_ID="${1:-}"
      if [[ -z "$PRODUCE_SLICE_ID" ]]; then
        echo "[-] --produce-slice requires a canonical Apple slice id" >&2
        exit 1
      fi
      ;;
    --produce-slice=*)
      PRODUCE_SLICE_ID="${1#*=}"
      if [[ -z "$PRODUCE_SLICE_ID" ]]; then
        echo "[-] --produce-slice requires a canonical Apple slice id" >&2
        exit 1
      fi
      ;;
    --slice-output-root)
      shift
      SLICE_OUTPUT_ROOT="${1:-}"
      if [[ -z "$SLICE_OUTPUT_ROOT" ]]; then
        echo "[-] --slice-output-root requires an absolute existing empty directory" >&2
        exit 1
      fi
      ;;
    --slice-output-root=*)
      SLICE_OUTPUT_ROOT="${1#*=}"
      if [[ -z "$SLICE_OUTPUT_ROOT" ]]; then
        echo "[-] --slice-output-root requires an absolute existing empty directory" >&2
        exit 1
      fi
      ;;
    --assemble-slices)
      shift
      ASSEMBLE_SLICE_ROOT="${1:-}"
      if [[ -z "$ASSEMBLE_SLICE_ROOT" ]]; then
        echo "[-] --assemble-slices requires an absolute existing common directory" >&2
        exit 1
      fi
      ;;
    --assemble-slices=*)
      ASSEMBLE_SLICE_ROOT="${1#*=}"
      if [[ -z "$ASSEMBLE_SLICE_ROOT" ]]; then
        echo "[-] --assemble-slices requires an absolute existing common directory" >&2
        exit 1
      fi
      ;;
    *)
      echo "[-] Unknown argument: $1" >&2
      echo "    Usage: $0 [--bridge-version <version>] [--archive-output <absolute-path>] [--privacy-production-enabled] [--allow-dirty-source] [--produce-slice <id> --slice-output-root <absolute-path> | --assemble-slices <absolute-path>]" >&2
      exit 1
      ;;
  esac
  shift
done

MATRIX_MODE=ordinary
if [[ -n "$PRODUCE_SLICE_ID" || -n "$SLICE_OUTPUT_ROOT" ]]; then
  if [[ -z "$PRODUCE_SLICE_ID" || -z "$SLICE_OUTPUT_ROOT" ]]; then
    echo "[-] --produce-slice and --slice-output-root must be supplied together" >&2
    exit 1
  fi
  MATRIX_MODE=produce
fi
if [[ -n "$ASSEMBLE_SLICE_ROOT" ]]; then
  if [[ "$MATRIX_MODE" != ordinary ]]; then
    echo "[-] Slice production and assembly modes are mutually exclusive" >&2
    exit 1
  fi
  MATRIX_MODE=assemble
fi
if [[ "$MATRIX_MODE" == produce && -n "$ARCHIVE_OUTPUT" ]]; then
  echo "[-] --archive-output is valid only for ordinary or slice-assembly mode" >&2
  exit 1
fi
if [[ "$MATRIX_MODE" != ordinary ]]; then
  MATRIX_BUILD_ID="${NORITO_BRIDGE_SLICE_BUILD_ID:-}"
  if [[ ! "$MATRIX_BUILD_ID" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]]; then
    echo "[-] NORITO_BRIDGE_SLICE_BUILD_ID is required and must be a canonical run-attempt token in matrix mode" >&2
    exit 1
  fi
else
  MATRIX_BUILD_ID=""
fi

canonical_matrix_root() {
  run_python312_clean - "$1" "$2" "$3" "$ROOT_DIR" \
    "$CARGO_TARGET_DIR" "$BUILD_DIR" "$OUT_DIR" <<'PY'
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
label = sys.argv[2]
mode = sys.argv[3]
source_root = Path(sys.argv[4])
disjoint_roots = tuple(Path(value) for value in sys.argv[5:])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit(f"{label} must be an absolute canonical directory")
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(f"{label} must already exist") from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or metadata.st_uid != os.geteuid()
    or metadata.st_mode & 0o022
    or not os.access(candidate, os.R_OK | os.X_OK)
    or (mode == "produce" and not os.access(candidate, os.W_OK))
    or candidate == source_root
    or source_root in candidate.parents
    or candidate in source_root.parents
):
    raise SystemExit(
        f"{label} must be an owner-controlled, non-symbolic canonical directory "
        "outside the Iroha source tree"
    )
for other in disjoint_roots:
    if candidate == other or candidate in other.parents or other in candidate.parents:
        raise SystemExit(f"{label} must be disjoint from every build/cache/output root")
if mode == "produce" and any(candidate.iterdir()):
    raise SystemExit(f"{label} must be empty before slice production")
print(candidate)
PY
}
if [[ "$MATRIX_MODE" == produce ]]; then
  SLICE_OUTPUT_ROOT="$(canonical_matrix_root \
    "$SLICE_OUTPUT_ROOT" NORITO_BRIDGE_SLICE_OUTPUT_ROOT produce)" || exit 1
elif [[ "$MATRIX_MODE" == assemble ]]; then
  ASSEMBLE_SLICE_ROOT="$(canonical_matrix_root \
    "$ASSEMBLE_SLICE_ROOT" NORITO_BRIDGE_ASSEMBLE_SLICE_ROOT assemble)" || exit 1
fi

if [[ -n "$ARCHIVE_OUTPUT" ]]; then
  ARCHIVE_OUTPUT="$(run_python312_clean - \
    "$ARCHIVE_OUTPUT" "${SOURCE_DATE_EPOCH:-}" "$ROOT_DIR" <<'PY'
import os
from pathlib import Path
import re
import stat
import sys

output = Path(sys.argv[1])
source_date_epoch = sys.argv[2]
source_root = Path(sys.argv[3])
if re.fullmatch(r"0|[1-9][0-9]*", source_date_epoch) is None:
    raise SystemExit(
        "SOURCE_DATE_EPOCH must be an explicit canonical unsigned integer "
        "with --archive-output"
    )
epoch = int(source_date_epoch, 10)
if not 315_532_800 <= epoch <= 4_354_819_199:
    raise SystemExit("SOURCE_DATE_EPOCH is outside the ZIP timestamp range")
if (
    not output.is_absolute()
    or output != Path(os.path.abspath(output))
    or output.name in {"", ".", ".."}
    or output.suffix != ".zip"
):
    raise SystemExit("--archive-output must be an absolute canonical .zip filename")
try:
    parent_metadata = output.parent.lstat()
    canonical_parent = output.parent.resolve(strict=True)
except OSError as error:
    raise SystemExit(f"--archive-output parent is unavailable: {error}") from None
if (
    canonical_parent != output.parent
    or stat.S_ISLNK(parent_metadata.st_mode)
    or not stat.S_ISDIR(parent_metadata.st_mode)
    or not os.access(output.parent, os.R_OK | os.W_OK | os.X_OK)
    or output.parent == source_root
    or source_root in output.parent.parents
):
    raise SystemExit(
        "--archive-output parent must be a writable canonical directory "
        "outside the Iroha source tree"
    )
try:
    metadata = output.lstat()
except FileNotFoundError:
    pass
else:
    raise SystemExit("--archive-output must not already exist")
print(output)
PY
)" || exit 1
fi

CARGO_FEATURE_ARGS=()
if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" ]]; then
  CARGO_FEATURE_ARGS+=(--features privacy-production-enabled)
  echo "[+] Enabling the audited privacy production bridge feature for every Apple slice" >&2
else
  echo "[+] Privacy proof dispatch remains fail-closed (default bridge build)" >&2
fi

PINNED_RUST_TOOLCHAIN="1.93.1"
SOURCE_SEAL_SCRIPT="$ROOT_DIR/scripts/norito_bridge_source_seal.py"
HERMETIC_RUNNER="$ROOT_DIR/scripts/run_mobile_hermetic_command.py"
APPLE_ARTIFACT_VALIDATOR="$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py"
USER_HOME_DIR="$(run_python312_clean -c \
  'import os,pwd; print(pwd.getpwuid(os.getuid()).pw_dir)')"
USER_HOME_DIR="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$USER_HOME_DIR")"
GIT_BINARY="/usr/bin/git"
RUSTUP_BINARY="$USER_HOME_DIR/.cargo/bin/rustup"
RUSTUP_CANONICAL_BINARY="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$RUSTUP_BINARY")"
[[ -e "$RUSTUP_BINARY" && -x "$RUSTUP_BINARY" ]] || {
  echo "[-] The pinned rustup dispatcher is unavailable: $RUSTUP_BINARY" >&2
  exit 1
}
for tool_path in "$PYTHON_BINARY" "$GIT_BINARY" "$RUSTUP_CANONICAL_BINARY"; do
  [[ -f "$tool_path" && ! -L "$tool_path" && -x "$tool_path" ]] || {
    echo "[-] Pinned Python, Git, and rustup executables are required: $tool_path" >&2
    exit 1
  }
done
for required_input in \
  "$SOURCE_SEAL_SCRIPT" "$HERMETIC_RUNNER" "$APPLE_ARTIFACT_VALIDATOR" \
  "$ROOT_DIR/rust-toolchain.toml"; do
  [[ -f "$required_input" && ! -L "$required_input" ]] || {
    echo "[-] Required NoritoBridge build input is unavailable: $required_input" >&2
    exit 1
  }
done
ACTUAL_RUST_TOOLCHAIN="$(
  sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' \
    "$ROOT_DIR/rust-toolchain.toml"
)"
if [[ "$ACTUAL_RUST_TOOLCHAIN" != "$PINNED_RUST_TOOLCHAIN" ]]; then
  echo "[-] NoritoBridge production builds require exact Rust $PINNED_RUST_TOOLCHAIN" >&2
  exit 1
fi

MOBILE_CARGO_HOME="$USER_HOME_DIR/.cargo"
MOBILE_RUSTUP_HOME="$USER_HOME_DIR/.rustup"
MOBILE_TMPDIR="/tmp"
for directory in "$USER_HOME_DIR" "$MOBILE_CARGO_HOME" "$MOBILE_RUSTUP_HOME" "$MOBILE_TMPDIR"; do
  [[ "$directory" == /* ]] || {
    echo "[-] NoritoBridge build directories must be absolute: $directory" >&2
    exit 1
  }
done
RUSTUP_ENV=(
  HOME="$USER_HOME_DIR"
  PATH="${RUSTUP_BINARY%/*}:/usr/bin:/bin"
  RUSTUP_HOME="$MOBILE_RUSTUP_HOME"
  CARGO_HOME="$MOBILE_CARGO_HOME"
  TMPDIR="$MOBILE_TMPDIR"
  LANG=C.UTF-8
  LC_ALL=C.UTF-8
)
CARGO_BINARY="$(
  env -i "${RUSTUP_ENV[@]}" \
    "$RUSTUP_BINARY" which --toolchain "$PINNED_RUST_TOOLCHAIN" cargo
)"
RUSTC_BINARY="$(
  env -i "${RUSTUP_ENV[@]}" \
    "$RUSTUP_BINARY" which --toolchain "$PINNED_RUST_TOOLCHAIN" rustc
)"
RUSTDOC_BINARY="$(
  env -i "${RUSTUP_ENV[@]}" \
    "$RUSTUP_BINARY" which --toolchain "$PINNED_RUST_TOOLCHAIN" rustdoc
)"
RUSTUP_CANONICAL_AFTER_DISPATCH="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$RUSTUP_BINARY")"
if [[ "$RUSTUP_CANONICAL_AFTER_DISPATCH" != "$RUSTUP_CANONICAL_BINARY" ]]; then
  echo "[-] The pinned rustup dispatcher changed during tool resolution" >&2
  exit 1
fi
CARGO_BINARY="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$CARGO_BINARY")"
RUSTC_BINARY="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$RUSTC_BINARY")"
RUSTDOC_BINARY="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$RUSTDOC_BINARY")"
[[ -x "$CARGO_BINARY" && -x "$RUSTC_BINARY" && -x "$RUSTDOC_BINARY" ]] || {
  echo "[-] Exact Rust $PINNED_RUST_TOOLCHAIN Cargo/rustc/rustdoc executables are unavailable" >&2
  exit 1
}
for supplied_tool in RUSTC RUSTDOC; do
  expected_tool="${supplied_tool}_BINARY"
  if [[ "${!supplied_tool}" != "${!expected_tool}" \
      || ! -f "${!supplied_tool}" || -L "${!supplied_tool}" \
      || ! -x "${!supplied_tool}" ]]; then
    echo "[-] $supplied_tool must be the canonical Rust $PINNED_RUST_TOOLCHAIN executable: ${!expected_tool}" >&2
    exit 1
  fi
done

run_source_seal() {
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${PYTHON_BINARY%/*}:${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${RUSTDOC_BINARY%/*}:${GIT_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    NORITO_BRIDGE_SEAL_HOME="$USER_HOME_DIR" \
    NORITO_BRIDGE_SEAL_CARGO_HOME="$MOBILE_CARGO_HOME" \
    NORITO_BRIDGE_SEAL_RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
    NORITO_BRIDGE_SEAL_TMPDIR="$MOBILE_TMPDIR" \
    NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    NORITO_BRIDGE_SEAL_CARGO="$CARGO_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTC="$RUSTC_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTDOC="$RUSTDOC_BINARY" \
    "$PYTHON_BINARY" -I -S -B "$SOURCE_SEAL_SCRIPT" "$@"
}

run_source_git() {
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${GIT_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    GIT_OPTIONAL_LOCKS=0 \
    "$GIT_BINARY" "$@"
}

run_isolated_python() {
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${PYTHON_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$PYTHON_BINARY" -I -S -B "$@"
}

selected_cargo_lock_seal() {
  run_isolated_python - "$CARGO_LOCKFILE" <<'PY'
import hashlib
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
if not candidate.is_absolute():
    raise SystemExit("selected Cargo lock path must be absolute")
if candidate != Path(os.path.abspath(candidate)):
    raise SystemExit("selected Cargo lock path must be canonical")
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(
        "selected Cargo lock must be a non-symbolic regular file"
    ) from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISREG(metadata.st_mode)
    or metadata.st_nlink != 1
):
    raise SystemExit(
        "selected Cargo lock must be a singly linked non-symbolic regular file"
    )
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(candidate, flags)
digest = hashlib.sha256()
try:
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise SystemExit("selected Cargo lock must remain singly linked")
    observed = 0
    while chunk := os.read(descriptor, 1024 * 1024):
        observed += len(chunk)
        digest.update(chunk)
    after = os.fstat(descriptor)
finally:
    os.close(descriptor)
before_identity = (
    before.st_dev,
    before.st_ino,
    before.st_mode,
    before.st_nlink,
    before.st_size,
    before.st_mtime_ns,
    before.st_ctime_ns,
)
after_identity = (
    after.st_dev,
    after.st_ino,
    after.st_mode,
    after.st_nlink,
    after.st_size,
    after.st_mtime_ns,
    after.st_ctime_ns,
)
visible = candidate.lstat()
if (
    before_identity != after_identity
    or observed != before.st_size
    or stat.S_ISLNK(visible.st_mode)
    or (visible.st_dev, visible.st_ino) != (before.st_dev, before.st_ino)
):
    raise SystemExit("selected Cargo lock changed identity while it was read")
print(
    ":".join(
        (
            digest.hexdigest(),
            str(before.st_dev),
            str(before.st_ino),
            str(before.st_mode),
            str(before.st_nlink),
            str(before.st_size),
            str(before.st_mtime_ns),
            str(before.st_ctime_ns),
        )
    )
)
PY
}

CARGO_LOCK_SEAL_START="$(selected_cargo_lock_seal)"
CARGO_LOCK_SHA256_START="${CARGO_LOCK_SEAL_START%%:*}"

assert_selected_cargo_lock() {
  local phase="$1"
  local current_seal
  if ! current_seal="$(selected_cargo_lock_seal)"; then
    echo "[-] Selected Cargo lock became unreadable during $phase" >&2
    exit 1
  fi
  if [[ "$current_seal" != "$CARGO_LOCK_SEAL_START" ]]; then
    echo "[-] Selected Cargo lock changed during $phase" >&2
    exit 1
  fi
}

bridge_source_fingerprint() {
  run_source_seal fingerprint --root "$ROOT_DIR" --platform apple \
    --lockfile-path "$CARGO_LOCKFILE"
}

bridge_source_status() {
  run_source_seal status --root "$ROOT_DIR" --platform apple \
    --lockfile-path "$CARGO_LOCKFILE"
}

assert_selected_cargo_lock "initial source authentication"
SOURCE_COMMIT_START=$(run_source_git -C "$ROOT_DIR" rev-parse --verify HEAD)
SOURCE_STATUS_START=$(bridge_source_status)
SOURCE_FINGERPRINT_START=$(bridge_source_fingerprint)
assert_selected_cargo_lock "initial source authentication"
if [[ -n "$SOURCE_STATUS_START" && "$ALLOW_DIRTY_SOURCE" != "1" ]]; then
  echo "[-] NoritoBridge production artifacts require a clean dependency-closure source tree" >&2
  echo "    Commit the bridge inputs or pass --allow-dirty-source for a fingerprint-bound local integration artifact." >&2
  exit 1
fi

assert_bridge_source_seal() {
  local phase="$1"
  local current_commit current_status current_fingerprint
  assert_selected_cargo_lock "$phase"
  if ! current_commit=$(run_source_git -C "$ROOT_DIR" rev-parse --verify HEAD) \
      || ! current_status=$(bridge_source_status) \
      || ! current_fingerprint=$(bridge_source_fingerprint); then
    echo "[-] NoritoBridge source became unreadable during $phase; refusing mixed-source Apple slices" >&2
    exit 1
  fi
  if [[ "$current_commit" != "$SOURCE_COMMIT_START" \
      || "$current_status" != "$SOURCE_STATUS_START" \
      || "$current_fingerprint" != "$SOURCE_FINGERPRINT_START" ]]; then
    echo "[-] NoritoBridge source changed during $phase; refusing mixed-source Apple slices" >&2
    exit 1
  fi
  assert_selected_cargo_lock "$phase"
}

if [[ "${NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY:-0}" == "1" ]]; then
  if [[ -n "${NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE:-}" ]]; then
    printf '\nsource-seal-negative-test\n' >> \
      "$ROOT_DIR/$NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE"
  fi
  assert_bridge_source_seal "source-seal self-test"
  exit 0
fi

sha256_file() {
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${PYTHON_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$PYTHON_BINARY" -I -S -B - "$1" <<'PY'
from pathlib import Path
import hashlib
import sys

path = Path(sys.argv[1])
digest = hashlib.sha256()
with path.open("rb") as handle:
    while chunk := handle.read(1024 * 1024):
        digest.update(chunk)
print(digest.hexdigest())
PY
}

PYTHON_VERSION="$(run_python312_clean -c \
  'import platform; print(platform.python_version())')"
GIT_VERSION="$(
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH=/usr/bin:/bin \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$GIT_BINARY" --version \
      | sed -nE 's/^git version ([0-9]+(\.[0-9]+){1,3}).*/\1/p'
)"
RUSTUP_VERSION="$(
  env -i "${RUSTUP_ENV[@]}" "$RUSTUP_BINARY" --version \
    | sed -nE '1s/^rustup ([0-9]+(\.[0-9]+){1,2}).*/\1/p'
)"
for tool_version in "$PYTHON_VERSION" "$RUSTUP_VERSION"; do
  [[ "$tool_version" =~ ^[0-9]+(\.[0-9]+){1,2}$ ]] || {
    echo "[-] Native build tool returned a non-canonical version: $tool_version" >&2
    exit 1
  }
done
[[ "$GIT_VERSION" =~ ^[0-9]+(\.[0-9]+){1,3}$ ]] || {
  echo "[-] Git returned a non-canonical version: $GIT_VERSION" >&2
  exit 1
}
PYTHON_BINARY_SHA256="$(sha256_file "$PYTHON_BINARY")"
GIT_BINARY_SHA256="$(sha256_file "$GIT_BINARY")"
RUSTUP_BINARY_SHA256="$(sha256_file "$RUSTUP_BINARY")"
HERMETIC_RUNNER_SHA256="$(sha256_file "$HERMETIC_RUNNER")"
SOURCE_SEAL_SCRIPT_SHA256="$(sha256_file "$SOURCE_SEAL_SCRIPT")"
APPLE_ARTIFACT_VALIDATOR_SHA256="$(sha256_file "$APPLE_ARTIFACT_VALIDATOR")"

CARGO_VERSION_VERBOSE="$(
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:/usr/bin:/bin" \
    RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
    CARGO_HOME="$MOBILE_CARGO_HOME" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$CARGO_BINARY" --version --verbose
)"
RUSTC_VERSION_VERBOSE="$(
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:/usr/bin:/bin" \
    RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
    CARGO_HOME="$MOBILE_CARGO_HOME" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$RUSTC_BINARY" --version --verbose
)"
RUSTDOC_VERSION_VERBOSE="$(
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${RUSTDOC_BINARY%/*}:/usr/bin:/bin" \
    RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
    CARGO_HOME="$MOBILE_CARGO_HOME" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$RUSTDOC_BINARY" --version --verbose
)"
CARGO_RELEASE="$(sed -n 's/^release: //p' <<<"$CARGO_VERSION_VERBOSE")"
CARGO_COMMIT_HASH="$(sed -n 's/^commit-hash: //p' <<<"$CARGO_VERSION_VERBOSE")"
RUSTC_RELEASE="$(sed -n 's/^release: //p' <<<"$RUSTC_VERSION_VERBOSE")"
RUSTC_COMMIT_HASH="$(sed -n 's/^commit-hash: //p' <<<"$RUSTC_VERSION_VERBOSE")"
RUSTDOC_RELEASE="$(sed -n 's/^release: //p' <<<"$RUSTDOC_VERSION_VERBOSE")"
RUSTDOC_COMMIT_HASH="$(sed -n 's/^commit-hash: //p' <<<"$RUSTDOC_VERSION_VERBOSE")"
if [[ "$CARGO_RELEASE" != "$PINNED_RUST_TOOLCHAIN" \
  || "$RUSTC_RELEASE" != "$PINNED_RUST_TOOLCHAIN" \
  || "$RUSTDOC_RELEASE" != "$PINNED_RUST_TOOLCHAIN" \
  || ! "$CARGO_COMMIT_HASH" =~ ^[0-9a-f]{40}$ \
  || ! "$RUSTC_COMMIT_HASH" =~ ^[0-9a-f]{40}$ \
  || ! "$RUSTDOC_COMMIT_HASH" =~ ^[0-9a-f]{40}$ \
  || "$RUSTDOC_COMMIT_HASH" != "$RUSTC_COMMIT_HASH" ]]; then
  echo "[-] Cargo/rustc/rustdoc identity does not match exact Rust $PINNED_RUST_TOOLCHAIN" >&2
  exit 1
fi
CARGO_BINARY_SHA256="$(sha256_file "$CARGO_BINARY")"
RUSTC_BINARY_SHA256="$(sha256_file "$RUSTC_BINARY")"
RUSTDOC_BINARY_SHA256="$(sha256_file "$RUSTDOC_BINARY")"

XCODE_SELECT_BINARY="/usr/bin/xcode-select"
XCRUN_BINARY="/usr/bin/xcrun"
XCODEBUILD_BINARY="/usr/bin/xcodebuild"
for apple_tool in "$XCODE_SELECT_BINARY" "$XCRUN_BINARY" "$XCODEBUILD_BINARY"; do
  [[ -x "$apple_tool" ]] || {
    echo "[-] Required Apple developer tool is unavailable: $apple_tool" >&2
    exit 1
  }
done
if [[ -n "${NORITO_BRIDGE_DEVELOPER_DIR:-}" ]]; then
  XCODE_DEVELOPER_DIR="$NORITO_BRIDGE_DEVELOPER_DIR"
else
  XCODE_DEVELOPER_DIR="$(
    env -i \
      HOME="$USER_HOME_DIR" \
      PATH=/usr/bin:/bin \
      TMPDIR="$MOBILE_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$XCODE_SELECT_BINARY" -p
  )"
fi
[[ "$XCODE_DEVELOPER_DIR" == /* && -d "$XCODE_DEVELOPER_DIR" ]] || {
  echo "[-] Xcode developer directory is invalid: $XCODE_DEVELOPER_DIR" >&2
  exit 1
}
XCODE_DEVELOPER_DIR="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$XCODE_DEVELOPER_DIR")"

xcrun_value() {
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH=/usr/bin:/bin \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
    "$XCRUN_BINARY" "$@"
}

IPHONEOS_SDKROOT="$(xcrun_value --sdk iphoneos --show-sdk-path)"
IPHONESIMULATOR_SDKROOT="$(xcrun_value --sdk iphonesimulator --show-sdk-path)"
MACOSX_SDKROOT="$(xcrun_value --sdk macosx --show-sdk-path)"
IPHONEOS_SDK_VERSION="$(xcrun_value --sdk iphoneos --show-sdk-version)"
IPHONESIMULATOR_SDK_VERSION="$(xcrun_value --sdk iphonesimulator --show-sdk-version)"
MACOSX_SDK_VERSION="$(xcrun_value --sdk macosx --show-sdk-version)"
LIPO_BINARY="$(xcrun_value --find lipo)"
NM_BINARY="$(xcrun_value --find nm)"
for sdk_variable in IPHONEOS_SDKROOT IPHONESIMULATOR_SDKROOT MACOSX_SDKROOT; do
  sdkroot="${!sdk_variable}"
  printf -v "$sdk_variable" '%s' "$(run_python312_clean -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$sdkroot")"
done
for sdkroot in "$IPHONEOS_SDKROOT" "$IPHONESIMULATOR_SDKROOT" "$MACOSX_SDKROOT"; do
  [[ "$sdkroot" == /* && -d "$sdkroot" && ! -L "$sdkroot" ]] || {
    echo "[-] Xcode returned an invalid SDK root: $sdkroot" >&2
    exit 1
  }
done
LIPO_BINARY="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$LIPO_BINARY")"
NM_BINARY="$(run_python312_clean -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$NM_BINARY")"
[[ -f "$LIPO_BINARY" && ! -L "$LIPO_BINARY" && -x "$LIPO_BINARY" \
    && -f "$NM_BINARY" && ! -L "$NM_BINARY" && -x "$NM_BINARY" ]] || {
  echo "[-] Xcode lipo/nm executables are unavailable" >&2
  exit 1
}
LIPO_BINARY_SHA256="$(sha256_file "$LIPO_BINARY")"
NM_BINARY_SHA256="$(sha256_file "$NM_BINARY")"
XCODE_VERSION_OUTPUT="$(
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH=/usr/bin:/bin \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
    "$XCODEBUILD_BINARY" -version
)"
XCODE_VERSION="$(sed -n 's/^Xcode //p' <<<"$XCODE_VERSION_OUTPUT")"
XCODE_BUILD_VERSION="$(sed -n 's/^Build version //p' <<<"$XCODE_VERSION_OUTPUT")"
for sdk_version in \
  "$IPHONEOS_SDK_VERSION" "$IPHONESIMULATOR_SDK_VERSION" "$MACOSX_SDK_VERSION"; do
  [[ "$sdk_version" =~ ^[0-9]+(\.[0-9]+){1,2}$ ]] || {
    echo "[-] Xcode returned a non-canonical SDK version: $sdk_version" >&2
    exit 1
  }
done
if [[ ! "$XCODE_VERSION" =~ ^[0-9]+(\.[0-9]+){0,2}$ \
  || ! "$XCODE_BUILD_VERSION" =~ ^[A-Za-z0-9.]+$ ]]; then
  echo "[-] Xcode returned a non-canonical toolchain identity" >&2
  exit 1
fi

echo "[+] Using iOS deployment target (device): $IPHONEOS_DEPLOYMENT_TARGET" >&2
echo "[+] Using iOS deployment target (simulator): $IPHONESIMULATOR_DEPLOYMENT_TARGET" >&2

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR" "$OUT_DIR"

DEVICE_TRIPLE="aarch64-apple-ios"
SIM_ARM_TRIPLE="aarch64-apple-ios-sim"
SIM_X64_TRIPLE="x86_64-apple-ios"
MACOS_ARM_TRIPLE="aarch64-apple-darwin"
MACOS_X64_TRIPLE="x86_64-apple-darwin"

if [[ -z "${BRIDGE_VERSION}" ]]; then
  VERSION_SOURCE="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  BRIDGE_VERSION=$(sed -nE \
    's/.*expectedVersion[^\"]*\"([^\"]+)\".*/\1/p' \
    "$VERSION_SOURCE" | head -n1)
fi
if [[ -z "${BRIDGE_VERSION}" ]]; then
  echo "[-] Unable to determine NoritoBridge version for artifact manifest" >&2
  exit 1
fi
BRIDGE_BUNDLE_VERSION="${BRIDGE_VERSION%%-*}"
if [[ -z "$BRIDGE_BUNDLE_VERSION" ]]; then
  BRIDGE_BUNDLE_VERSION="1"
fi

slice_configuration() {
  SLICE_ID="$1"
  case "$SLICE_ID" in
    ios-arm64)
      SLICE_PROFILE=apple-ios-device
      SLICE_SDKROOT="$IPHONEOS_SDKROOT"
      SLICE_SDK_NAME=iphoneos
      SLICE_SDK_VERSION="$IPHONEOS_SDK_VERSION"
      SLICE_DEPLOYMENT_TARGET="$IPHONEOS_DEPLOYMENT_TARGET"
      SLICE_TARGET_TRIPLE="$DEVICE_TRIPLE"
      SLICE_ARCHITECTURE=arm64
      SLICE_LABEL="iOS device"
      ;;
    ios-sim-arm64)
      SLICE_PROFILE=apple-ios-simulator
      SLICE_SDKROOT="$IPHONESIMULATOR_SDKROOT"
      SLICE_SDK_NAME=iphonesimulator
      SLICE_SDK_VERSION="$IPHONESIMULATOR_SDK_VERSION"
      SLICE_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET"
      SLICE_TARGET_TRIPLE="$SIM_ARM_TRIPLE"
      SLICE_ARCHITECTURE=arm64
      SLICE_LABEL="arm64 simulator"
      ;;
    ios-sim-x64)
      SLICE_PROFILE=apple-ios-simulator
      SLICE_SDKROOT="$IPHONESIMULATOR_SDKROOT"
      SLICE_SDK_NAME=iphonesimulator
      SLICE_SDK_VERSION="$IPHONESIMULATOR_SDK_VERSION"
      SLICE_DEPLOYMENT_TARGET="$IPHONESIMULATOR_DEPLOYMENT_TARGET"
      SLICE_TARGET_TRIPLE="$SIM_X64_TRIPLE"
      SLICE_ARCHITECTURE=x86_64
      SLICE_LABEL="x86_64 simulator"
      ;;
    macos-arm64)
      SLICE_PROFILE=apple-macos
      SLICE_SDKROOT="$MACOSX_SDKROOT"
      SLICE_SDK_NAME=macosx
      SLICE_SDK_VERSION="$MACOSX_SDK_VERSION"
      SLICE_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET"
      SLICE_TARGET_TRIPLE="$MACOS_ARM_TRIPLE"
      SLICE_ARCHITECTURE=arm64
      SLICE_LABEL="arm64 macOS"
      ;;
    macos-x64)
      SLICE_PROFILE=apple-macos
      SLICE_SDKROOT="$MACOSX_SDKROOT"
      SLICE_SDK_NAME=macosx
      SLICE_SDK_VERSION="$MACOSX_SDK_VERSION"
      SLICE_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET"
      SLICE_TARGET_TRIPLE="$MACOS_X64_TRIPLE"
      SLICE_ARCHITECTURE=x86_64
      SLICE_LABEL="x86_64 macOS"
      ;;
    *)
      echo "[-] Unknown canonical Apple slice id: $SLICE_ID" >&2
      return 1
      ;;
  esac
}

stage_cargo_library() {
  local target_triple="$1"
  local label="$2"
  local cargo_build_root="$3"
  local source_library="$cargo_build_root/$target_triple/release/lib${LIB_CRATE_NAME}.a"
  local staged_library="$STAGE_DIR/cargo-libraries/$target_triple/lib${LIB_CRATE_NAME}.a"
  if ! run_isolated_python - "$source_library" "$staged_library" <<'PY'
import os
from pathlib import Path
import shutil
import stat
import sys

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
try:
    metadata = source.lstat()
except OSError:
    raise SystemExit("Cargo did not produce a readable Apple static library") from None
if (
    source.resolve(strict=True) != source
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISREG(metadata.st_mode)
    or metadata.st_nlink != 1
    or metadata.st_uid != os.geteuid()
):
    raise SystemExit("Cargo Apple static library is not an owner-controlled regular file")
destination.parent.mkdir(parents=True, mode=0o700, exist_ok=False)
source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
source_descriptor = os.open(source, source_flags)
destination_descriptor = os.open(
    destination,
    os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
    0o600,
)
try:
    before = os.fstat(source_descriptor)
    with os.fdopen(os.dup(source_descriptor), "rb", closefd=True) as reader:
        with os.fdopen(os.dup(destination_descriptor), "wb", closefd=True) as writer:
            shutil.copyfileobj(reader, writer, 1024 * 1024)
            writer.flush()
            os.fsync(writer.fileno())
    after = os.fstat(source_descriptor)
finally:
    os.close(destination_descriptor)
    os.close(source_descriptor)
identity = lambda value: (
    value.st_dev,
    value.st_ino,
    value.st_mode,
    value.st_nlink,
    value.st_size,
    value.st_mtime_ns,
    value.st_ctime_ns,
)
visible = source.lstat()
if (
    identity(before) != identity(after)
    or (visible.st_dev, visible.st_ino) != (after.st_dev, after.st_ino)
):
    destination.unlink(missing_ok=True)
    raise SystemExit("Cargo Apple static library changed while it was staged")
PY
  then
    echo "[-] Missing or unauthenticated $label static library after Cargo build: $source_library" >&2
    exit 1
  fi
  printf '%s\n' "$staged_library"
}

run_hermetic_apple_cargo() {
  local profile="$1"
  local sdkroot="$2"
  local slice_target_dir="$3"
  shift 3
  local cargo_subcommand="$1"
  shift
  local cargo_status
  local platform_environment=()
  case "$profile" in
    apple-ios-device)
      platform_environment=(
        --set "DEVELOPER_DIR=$XCODE_DEVELOPER_DIR"
        --set "IPHONEOS_DEPLOYMENT_TARGET=$IPHONEOS_DEPLOYMENT_TARGET"
        --set "SDKROOT=$sdkroot"
      )
      ;;
    apple-ios-simulator)
      platform_environment=(
        --set "DEVELOPER_DIR=$XCODE_DEVELOPER_DIR"
        --set "IPHONEOS_DEPLOYMENT_TARGET=$IPHONESIMULATOR_DEPLOYMENT_TARGET"
        --set "IPHONESIMULATOR_DEPLOYMENT_TARGET=$IPHONESIMULATOR_DEPLOYMENT_TARGET"
        --set "SDKROOT=$sdkroot"
      )
      ;;
    apple-macos)
      platform_environment=(
        --set "DEVELOPER_DIR=$XCODE_DEVELOPER_DIR"
        --set "MACOSX_DEPLOYMENT_TARGET=$MACOSX_DEPLOYMENT_TARGET"
        --set "SDKROOT=$sdkroot"
      )
      ;;
    *)
      echo "[-] Unknown hermetic Apple Cargo profile: $profile" >&2
      exit 1
      ;;
  esac
  assert_selected_cargo_lock "the $profile Cargo preflight"
  if run_isolated_python "$HERMETIC_RUNNER" \
      --profile "$profile" \
      --set "CARGO=$CARGO_BINARY" \
      --set "CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS" \
      --set "CARGO_HOME=$MOBILE_CARGO_HOME" \
      --set "CARGO_INCREMENTAL=0" \
      --set "CARGO_NET_OFFLINE=true" \
      --set "CARGO_TARGET_DIR=$slice_target_dir" \
      --set "HOME=$USER_HOME_DIR" \
      --set "LANG=C.UTF-8" \
      --set "LC_ALL=C.UTF-8" \
      --set "NORITO_SKIP_BINDINGS_SYNC=1" \
      --set "PATH=${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${RUSTDOC_BINARY%/*}:/usr/bin:/bin" \
      --set "RUSTC=$RUSTC_BINARY" \
      --set "RUSTC_BOOTSTRAP=1" \
      --set "RUSTDOC=$RUSTDOC_BINARY" \
      --set "RUSTUP_HOME=$MOBILE_RUSTUP_HOME" \
      --set "TMPDIR=$MOBILE_TMPDIR" \
      "${platform_environment[@]}" \
      -- "$CARGO_BINARY" "$cargo_subcommand" \
      -Z unstable-options --lockfile-path "$CARGO_LOCKFILE" "$@"; then
    cargo_status=0
  else
    cargo_status=$?
  fi
  assert_selected_cargo_lock "the $profile Cargo invocation"
  return "$cargo_status"
}

prepare_slice_target_dir() {
  local target_triple="$1"
  local slice_target_dir="$CARGO_TARGET_DIR/$target_triple"
  run_isolated_python - "$CARGO_TARGET_DIR" "$slice_target_dir" <<'PY'
import os
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
child = Path(sys.argv[2])
if child.parent != root:
    raise SystemExit("Apple slice Cargo root is not a direct child of CARGO_TARGET_DIR")
try:
    child.mkdir(mode=0o700)
except FileExistsError:
    pass
metadata = child.lstat()
if (
    child.resolve(strict=True) != child
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or metadata.st_uid != os.geteuid()
    or metadata.st_mode & 0o022
    or not os.access(child, os.R_OK | os.W_OK | os.X_OK)
):
    raise SystemExit("Apple slice Cargo root is not an owner-controlled canonical directory")
PY
  printf '%s\n' "$slice_target_dir"
}

scrub_cached_slice_library() {
  local slice_target_dir="$1"
  local target_triple="$2"
  run_isolated_python - "$slice_target_dir" "$target_triple" "$LIB_CRATE_NAME" <<'PY'
import os
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
target_triple = sys.argv[2]
crate_name = sys.argv[3]
relative_parents = (target_triple, "release")
parent = root
for component in relative_parents:
    parent = parent / component
    try:
        metadata = parent.lstat()
    except FileNotFoundError:
        raise SystemExit(0)
    if (
        parent.resolve(strict=True) != parent
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
    ):
        raise SystemExit("cached Apple target contains an unauthenticated child root")
library = parent / f"lib{crate_name}.a"
try:
    metadata = library.lstat()
except FileNotFoundError:
    raise SystemExit(0)
if metadata.st_uid != os.geteuid():
    raise SystemExit("cached first-party Apple library is not owned by this runner")
# unlink removes a symlink itself rather than following it; every parent was
# authenticated above, so this cannot escape the fixed target-triple root.
library.unlink()
PY
}

build_one_apple_slice() {
  local slice_id="$1"
  local slice_target_dir
  slice_configuration "$slice_id"
  if [[ "$MATRIX_MODE" == produce ]]; then
    slice_target_dir="$(prepare_slice_target_dir "$SLICE_TARGET_TRIPLE")" || exit 1
  else
    # Ordinary local builds preserve Cargo throughput by sharing the caller's
    # single warm target. Matrix jobs alone isolate their one producer lane.
    slice_target_dir="$CARGO_TARGET_DIR"
  fi
  # Never accept an old first-party library merely because dependency caches
  # restored it. Removing the exact output forces Cargo to authenticate and
  # relink this slice while preserving the warm dependency target.
  scrub_cached_slice_library "$slice_target_dir" "$SLICE_TARGET_TRIPLE"
  echo "[+] Building $SLICE_LABEL ($SLICE_TARGET_TRIPLE)" >&2
  run_hermetic_apple_cargo \
    "$SLICE_PROFILE" "$SLICE_SDKROOT" "$slice_target_dir" \
    build --locked --offline --jobs 1 -p "$LIB_CRATE_NAME" --lib --release \
    --target "$SLICE_TARGET_TRIPLE" \
    "${CARGO_FEATURE_ARGS[@]+"${CARGO_FEATURE_ARGS[@]}"}"
  assert_bridge_source_seal "the $slice_id Apple slice build"
}

MATRIX_CONTEXT_PATH="$STAGE_DIR/apple-slice-context.json"
write_matrix_context() {
  run_isolated_python - \
    "$MATRIX_CONTEXT_PATH" "$MATRIX_BUILD_ID" "$BRIDGE_VERSION" \
    "$SOURCE_COMMIT_START" "$SOURCE_STATUS_START" "$SOURCE_FINGERPRINT_START" \
    "$CARGO_LOCK_SHA256_START" "$ALLOW_DIRTY_SOURCE" \
    "$PRIVACY_PRODUCTION_ENABLED" "$PINNED_RUST_TOOLCHAIN" \
    "$CARGO_RELEASE" "$CARGO_COMMIT_HASH" "$CARGO_BINARY_SHA256" \
    "$RUSTC_RELEASE" "$RUSTC_COMMIT_HASH" "$RUSTC_BINARY_SHA256" \
    "$RUSTDOC_RELEASE" "$RUSTDOC_COMMIT_HASH" "$RUSTDOC_BINARY_SHA256" \
    "$HERMETIC_RUNNER_SHA256" "$SOURCE_SEAL_SCRIPT_SHA256" \
    "$APPLE_ARTIFACT_VALIDATOR_SHA256" \
    "$PYTHON_VERSION" "$PYTHON_BINARY_SHA256" \
    "$GIT_VERSION" "$GIT_BINARY_SHA256" \
    "$RUSTUP_VERSION" "$RUSTUP_BINARY_SHA256" \
    "$XCODE_VERSION" "$XCODE_BUILD_VERSION" \
    "$IPHONEOS_SDK_VERSION" "$IPHONESIMULATOR_SDK_VERSION" \
    "$MACOSX_SDK_VERSION" "$IPHONEOS_DEPLOYMENT_TARGET" \
    "$IPHONESIMULATOR_DEPLOYMENT_TARGET" "$MACOSX_DEPLOYMENT_TARGET" \
    "$LIPO_BINARY_SHA256" "$NM_BINARY_SHA256" <<'PY'
import hashlib
import json
from pathlib import Path
import sys

(
    output,
    build_id,
    bridge_version,
    source_commit,
    source_status,
    source_fingerprint,
    cargo_lock_sha256,
    allow_dirty,
    privacy_enabled,
    rust_channel,
    cargo_release,
    cargo_commit,
    cargo_sha256,
    rustc_release,
    rustc_commit,
    rustc_sha256,
    rustdoc_release,
    rustdoc_commit,
    rustdoc_sha256,
    hermetic_runner_sha256,
    source_seal_script_sha256,
    apple_artifact_validator_sha256,
    python_version,
    python_sha256,
    git_version,
    git_sha256,
    rustup_version,
    rustup_sha256,
    xcode_version,
    xcode_build,
    iphoneos_sdk,
    iphonesimulator_sdk,
    macosx_sdk,
    iphoneos_deployment,
    iphonesimulator_deployment,
    macosx_deployment,
    lipo_sha256,
    nm_sha256,
) = sys.argv[1:]
privacy = privacy_enabled == "1"
payload = {
    "schema": "iroha.norito-bridge.apple-slice-context.v1",
    "build_id": build_id,
    "bridge_version": bridge_version,
    "source": {
        "commit": source_commit,
        "status": source_status,
        "status_sha256": hashlib.sha256(source_status.encode("utf-8")).hexdigest(),
        "tree_dirty": bool(source_status),
        "fingerprint_sha256": source_fingerprint,
        "cargo_lock_sha256": cargo_lock_sha256,
    },
    "mode": {
        "allow_dirty_source": allow_dirty == "1",
        "privacy_production_enabled": privacy,
        "cargo_features": ["privacy-production-enabled"] if privacy else [],
    },
    "rust": {
        "channel": rust_channel,
        "cargo_release": cargo_release,
        "cargo_commit_hash": cargo_commit,
        "cargo_binary_sha256": cargo_sha256,
        "rustc_release": rustc_release,
        "rustc_commit_hash": rustc_commit,
        "rustc_binary_sha256": rustc_sha256,
        "rustdoc_release": rustdoc_release,
        "rustdoc_commit_hash": rustdoc_commit,
        "rustdoc_binary_sha256": rustdoc_sha256,
    },
    "producer_tools": {
        "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
        "hermetic_runner_sha256": hermetic_runner_sha256,
        "source_seal_schema": "iroha.norito-bridge-source-seal.v1",
        "source_seal_script_sha256": source_seal_script_sha256,
        "apple_artifact_validator_sha256": apple_artifact_validator_sha256,
        "python_version": python_version,
        "python_binary_sha256": python_sha256,
        "git_version": git_version,
        "git_binary_sha256": git_sha256,
        "rustup_version": rustup_version,
        "rustup_binary_sha256": rustup_sha256,
    },
    "apple": {
        "xcode_version": xcode_version,
        "xcode_build_version": xcode_build,
        "sdk_versions": {
            "iphoneos": iphoneos_sdk,
            "iphonesimulator": iphonesimulator_sdk,
            "macosx": macosx_sdk,
        },
        "deployment_targets": {
            "iphoneos": iphoneos_deployment,
            "iphonesimulator": iphonesimulator_deployment,
            "macosx": macosx_deployment,
        },
        "lipo_binary_sha256": lipo_sha256,
        "nm_binary_sha256": nm_sha256,
    },
}
Path(output).write_text(
    json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="utf-8",
)
PY
}

write_slice_bundle() {
  local source_library="$1"
  local bundle_root="$SLICE_OUTPUT_ROOT/$SLICE_ID"
  run_isolated_python - \
    "$SLICE_OUTPUT_ROOT" "$bundle_root" "$source_library" \
    "$MATRIX_CONTEXT_PATH" "$SLICE_ID" "$SLICE_TARGET_TRIPLE" \
    "$SLICE_PROFILE" "$SLICE_SDK_NAME" "$SLICE_SDK_VERSION" \
    "$SLICE_DEPLOYMENT_TARGET" "$SLICE_ARCHITECTURE" \
    "$LIPO_BINARY" "$NM_BINARY" "$XCODE_DEVELOPER_DIR" \
    "$APPLE_ARTIFACT_VALIDATOR" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import stat
import subprocess
import sys

(
    root_raw,
    bundle_raw,
    source_raw,
    context_raw,
    slice_id,
    target_triple,
    profile,
    sdk_name,
    sdk_version,
    deployment_target,
    architecture,
    lipo_raw,
    nm_raw,
    developer_dir,
    validator_raw,
) = sys.argv[1:]
root = Path(root_raw)
bundle = Path(bundle_raw)
source = Path(source_raw)
context_path = Path(context_raw)
if bundle.parent != root or bundle.name != slice_id or any(root.iterdir()):
    raise SystemExit("slice output root is no longer fresh and empty")
bundle.mkdir(mode=0o700)
library = bundle / "libconnect_norito_bridge.a"

source_metadata = source.lstat()
if (
    source.resolve(strict=True) != source
    or stat.S_ISLNK(source_metadata.st_mode)
    or not stat.S_ISREG(source_metadata.st_mode)
    or source_metadata.st_nlink != 1
    or source_metadata.st_uid != os.geteuid()
):
    raise SystemExit("staged slice library is not an owner-controlled regular file")
source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
source_descriptor = os.open(source, source_flags)
library_descriptor = os.open(
    library,
    os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
    0o600,
)
try:
    before = os.fstat(source_descriptor)
    with os.fdopen(os.dup(source_descriptor), "rb", closefd=True) as reader:
        with os.fdopen(os.dup(library_descriptor), "wb", closefd=True) as writer:
            shutil.copyfileobj(reader, writer, 1024 * 1024)
            writer.flush()
            os.fsync(writer.fileno())
    after = os.fstat(source_descriptor)
finally:
    os.close(library_descriptor)
    os.close(source_descriptor)
identity = lambda value: (
    value.st_dev,
    value.st_ino,
    value.st_mode,
    value.st_nlink,
    value.st_size,
    value.st_mtime_ns,
    value.st_ctime_ns,
)
if identity(before) != identity(after):
    raise SystemExit("staged slice library changed during bundle production")

tool_environment = {
    "HOME": "/tmp",
    "PATH": "/usr/bin:/bin",
    "TMPDIR": "/tmp",
    "LANG": "C.UTF-8",
    "LC_ALL": "C.UTF-8",
    "DEVELOPER_DIR": developer_dir,
}
actual_architectures = subprocess.run(
    [lipo_raw, "-archs", str(library)],
    check=True,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    env=tool_environment,
).stdout.split()
if actual_architectures != [architecture]:
    raise SystemExit(
        f"slice {slice_id} architecture is not exact: {actual_architectures!r}"
    )
symbols_output = subprocess.run(
    [nm_raw, "-gUj", str(library)],
    check=True,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    env=tool_environment,
).stdout.decode("utf-8", "strict")
symbols = sorted(
    {
        line.strip().removeprefix("_")
        for line in symbols_output.splitlines()
        if line.strip()
    }
)
validator = runpy.run_path(validator_raw)
if validator.get("REQUIRED_NATIVE_BRIDGE_ABI_VERSION") != 22:
    raise SystemExit("Apple artifact validator does not require exact native ABI 22")
required_symbols = set(validator["EXPECTED_REQUIRED_SYMBOLS"])
forbidden_symbols = set(validator["EXPECTED_FORBIDDEN_SYMBOLS"])
missing = sorted(required_symbols - set(symbols))
forbidden = sorted(forbidden_symbols & set(symbols))
expected_kagemusha = {
    symbol for symbol in required_symbols
    if symbol.startswith("connect_norito_kagemusha_")
}
actual_kagemusha = {
    symbol for symbol in symbols
    if symbol.startswith("connect_norito_kagemusha_")
}
if missing:
    raise SystemExit("slice is missing ABI22 required exports: " + ", ".join(missing))
if forbidden:
    raise SystemExit("slice contains forbidden exports: " + ", ".join(forbidden))
if actual_kagemusha != expected_kagemusha:
    raise SystemExit("slice Kagemusha export inventory is not exact")
symbol_bytes = (("\n".join(symbols) + "\n") if symbols else "").encode("utf-8")
required_bytes = ("\n".join(sorted(required_symbols)) + "\n").encode("utf-8")
forbidden_bytes = ("\n".join(sorted(forbidden_symbols)) + "\n").encode("utf-8")
library_bytes = library.read_bytes()
with context_path.open("r", encoding="utf-8") as handle:
    context = json.load(handle)
evidence = {
    "schema": "iroha.norito-bridge.apple-slice-evidence.v1",
    "context": context,
    "slice": {
        "id": slice_id,
        "target_triple": target_triple,
        "profile": profile,
        "sdk_name": sdk_name,
        "sdk_version": sdk_version,
        "deployment_target": deployment_target,
    },
    "library": {
        "native_bridge_abi_version": 22,
        "file_name": library.name,
        "sha256": hashlib.sha256(library_bytes).hexdigest(),
        "size": len(library_bytes),
        "architectures": actual_architectures,
        "global_defined_symbols_sha256": hashlib.sha256(symbol_bytes).hexdigest(),
        "global_defined_symbol_count": len(symbols),
        "required_symbol_inventory_sha256": hashlib.sha256(required_bytes).hexdigest(),
        "forbidden_symbol_inventory_sha256": hashlib.sha256(forbidden_bytes).hexdigest(),
    },
}
evidence_path = bundle / "slice-evidence.json"
descriptor = os.open(
    evidence_path,
    os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
    0o600,
)
with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
    json.dump(evidence, handle, sort_keys=True, separators=(",", ":"))
    handle.write("\n")
    handle.flush()
    os.fsync(handle.fileno())
for path in (library, evidence_path, bundle, root):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
PY
}

assemble_slice_bundles() {
  local staged_root="$STAGE_DIR/cargo-libraries"
  run_isolated_python - \
    "$ASSEMBLE_SLICE_ROOT" "$MATRIX_CONTEXT_PATH" "$staged_root" \
    "$LIPO_BINARY" "$NM_BINARY" "$XCODE_DEVELOPER_DIR" \
    "$APPLE_ARTIFACT_VALIDATOR" <<'PY_ASSEMBLE_SLICES'
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import stat
import subprocess
import sys

root = Path(sys.argv[1])
context_path = Path(sys.argv[2])
staged_root = Path(sys.argv[3])
lipo = sys.argv[4]
nm = sys.argv[5]
developer_dir = sys.argv[6]
validator_path = sys.argv[7]
slices = {
    "ios-arm64": {
        "target_triple": "aarch64-apple-ios",
        "profile": "apple-ios-device",
        "sdk_name": "iphoneos",
        "architecture": "arm64",
    },
    "ios-sim-arm64": {
        "target_triple": "aarch64-apple-ios-sim",
        "profile": "apple-ios-simulator",
        "sdk_name": "iphonesimulator",
        "architecture": "arm64",
    },
    "ios-sim-x64": {
        "target_triple": "x86_64-apple-ios",
        "profile": "apple-ios-simulator",
        "sdk_name": "iphonesimulator",
        "architecture": "x86_64",
    },
    "macos-arm64": {
        "target_triple": "aarch64-apple-darwin",
        "profile": "apple-macos",
        "sdk_name": "macosx",
        "architecture": "arm64",
    },
    "macos-x64": {
        "target_triple": "x86_64-apple-darwin",
        "profile": "apple-macos",
        "sdk_name": "macosx",
        "architecture": "x86_64",
    },
}

def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result

def regular_owned(path: Path, label: str) -> os.stat_result:
    metadata = path.lstat()
    if (
        path.resolve(strict=True) != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o022
    ):
        raise SystemExit(f"{label} is not an owner-controlled regular file")
    return metadata

with context_path.open("r", encoding="utf-8") as handle:
    expected_context = json.load(handle, object_pairs_hook=no_duplicates)
validator = runpy.run_path(validator_path)
if validator.get("REQUIRED_NATIVE_BRIDGE_ABI_VERSION") != 22:
    raise SystemExit("Apple artifact validator does not require exact native ABI 22")
required_symbols = set(validator["EXPECTED_REQUIRED_SYMBOLS"])
forbidden_symbols = set(validator["EXPECTED_FORBIDDEN_SYMBOLS"])
expected_kagemusha = {
    symbol for symbol in required_symbols
    if symbol.startswith("connect_norito_kagemusha_")
}
required_bytes = ("\n".join(sorted(required_symbols)) + "\n").encode("utf-8")
forbidden_bytes = ("\n".join(sorted(forbidden_symbols)) + "\n").encode("utf-8")
if {entry.name for entry in root.iterdir()} != set(slices):
    raise SystemExit("slice assembly root does not contain exactly five canonical bundles")
staged_root.mkdir(mode=0o700)
tool_environment = {
    "HOME": "/tmp",
    "PATH": "/usr/bin:/bin",
    "TMPDIR": "/tmp",
    "LANG": "C.UTF-8",
    "LC_ALL": "C.UTF-8",
    "DEVELOPER_DIR": developer_dir,
}
for slice_id, expected in slices.items():
    bundle = root / slice_id
    bundle_metadata = bundle.lstat()
    if (
        bundle.resolve(strict=True) != bundle
        or stat.S_ISLNK(bundle_metadata.st_mode)
        or not stat.S_ISDIR(bundle_metadata.st_mode)
        or bundle_metadata.st_uid != os.geteuid()
        or bundle_metadata.st_mode & 0o022
        or {entry.name for entry in bundle.iterdir()}
        != {"libconnect_norito_bridge.a", "slice-evidence.json"}
    ):
        raise SystemExit(f"slice bundle has a non-canonical inventory: {slice_id}")
    source = bundle / "libconnect_norito_bridge.a"
    evidence_path = bundle / "slice-evidence.json"
    source_metadata = regular_owned(source, f"slice {slice_id} library")
    evidence_metadata = regular_owned(evidence_path, f"slice {slice_id} evidence")
    if evidence_metadata.st_size > 2 * 1024 * 1024:
        raise SystemExit(f"slice {slice_id} evidence exceeds the closed size limit")
    evidence_descriptor = os.open(
        evidence_path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        evidence_before = os.fstat(evidence_descriptor)
        chunks = []
        while chunk := os.read(evidence_descriptor, 1024 * 1024):
            chunks.append(chunk)
        evidence_after = os.fstat(evidence_descriptor)
    finally:
        os.close(evidence_descriptor)
    evidence_bytes = b"".join(chunks)
    evidence_identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_uid,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    evidence_visible = evidence_path.lstat()
    if (
        evidence_identity(evidence_before) != evidence_identity(evidence_after)
        or (evidence_visible.st_dev, evidence_visible.st_ino)
        != (evidence_after.st_dev, evidence_after.st_ino)
        or len(evidence_bytes) != evidence_after.st_size
    ):
        raise SystemExit(f"slice {slice_id} evidence changed while it was read")
    try:
        evidence = json.loads(
            evidence_bytes.decode("utf-8"), object_pairs_hook=no_duplicates
        )
    except (UnicodeError, ValueError, TypeError) as error:
        raise SystemExit(f"slice {slice_id} evidence is malformed: {error}") from None
    if set(evidence) != {"schema", "context", "slice", "library"}:
        raise SystemExit(f"slice {slice_id} evidence has a non-canonical schema")
    if evidence["schema"] != "iroha.norito-bridge.apple-slice-evidence.v1":
        raise SystemExit(f"slice {slice_id} evidence schema is unsupported")
    if evidence["context"] != expected_context:
        raise SystemExit(f"slice {slice_id} evidence does not match this assembly context")
    expected_slice = {
        "id": slice_id,
        "target_triple": expected["target_triple"],
        "profile": expected["profile"],
        "sdk_name": expected["sdk_name"],
        "sdk_version": expected_context["apple"]["sdk_versions"][expected["sdk_name"]],
        "deployment_target": expected_context["apple"]["deployment_targets"][expected["sdk_name"]],
    }
    if evidence["slice"] != expected_slice:
        raise SystemExit(f"slice {slice_id} evidence has the wrong target identity")
    library_evidence = evidence["library"]
    if set(library_evidence) != {
        "native_bridge_abi_version",
        "file_name",
        "sha256",
        "size",
        "architectures",
        "global_defined_symbols_sha256",
        "global_defined_symbol_count",
        "required_symbol_inventory_sha256",
        "forbidden_symbol_inventory_sha256",
    } or (
        library_evidence["native_bridge_abi_version"] != 22
        or library_evidence["file_name"] != source.name
        or library_evidence["required_symbol_inventory_sha256"]
        != hashlib.sha256(required_bytes).hexdigest()
        or library_evidence["forbidden_symbol_inventory_sha256"]
        != hashlib.sha256(forbidden_bytes).hexdigest()
    ):
        raise SystemExit(f"slice {slice_id} library evidence is non-canonical")

    destination_dir = staged_root / expected["target_triple"]
    destination_dir.mkdir(mode=0o700)
    destination = destination_dir / "libconnect_norito_bridge.a"
    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    source_descriptor = os.open(source, source_flags)
    destination_descriptor = os.open(
        destination,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        before = os.fstat(source_descriptor)
        digest = hashlib.sha256()
        observed = 0
        with os.fdopen(os.dup(source_descriptor), "rb", closefd=True) as reader:
            with os.fdopen(os.dup(destination_descriptor), "wb", closefd=True) as writer:
                while chunk := reader.read(1024 * 1024):
                    digest.update(chunk)
                    observed += len(chunk)
                    writer.write(chunk)
                writer.flush()
                os.fsync(writer.fileno())
        after = os.fstat(source_descriptor)
    finally:
        os.close(destination_descriptor)
        os.close(source_descriptor)
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    visible = source.lstat()
    if (
        identity(before) != identity(after)
        or (visible.st_dev, visible.st_ino) != (after.st_dev, after.st_ino)
        or observed != after.st_size
        or library_evidence["sha256"] != digest.hexdigest()
        or library_evidence["size"] != observed
    ):
        raise SystemExit(f"slice {slice_id} library bytes do not match closed evidence")
    architectures = subprocess.run(
        [lipo, "-archs", str(destination)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=tool_environment,
    ).stdout.split()
    if architectures != [expected["architecture"]] or library_evidence["architectures"] != architectures:
        raise SystemExit(f"slice {slice_id} has the wrong architecture")
    symbols_output = subprocess.run(
        [nm, "-gUj", str(destination)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=tool_environment,
    ).stdout.decode("utf-8", "strict")
    symbols = sorted(
        {
            line.strip().removeprefix("_")
            for line in symbols_output.splitlines()
            if line.strip()
        }
    )
    symbol_bytes = (("\n".join(symbols) + "\n") if symbols else "").encode("utf-8")
    missing = sorted(required_symbols - set(symbols))
    forbidden = sorted(forbidden_symbols & set(symbols))
    actual_kagemusha = {
        symbol for symbol in symbols
        if symbol.startswith("connect_norito_kagemusha_")
    }
    if missing:
        raise SystemExit(
            f"slice {slice_id} is missing ABI22 required exports: "
            + ", ".join(missing)
        )
    if forbidden:
        raise SystemExit(
            f"slice {slice_id} contains forbidden exports: "
            + ", ".join(forbidden)
        )
    if actual_kagemusha != expected_kagemusha:
        raise SystemExit(f"slice {slice_id} Kagemusha export inventory is not exact")
    if (
        library_evidence["global_defined_symbols_sha256"]
        != hashlib.sha256(symbol_bytes).hexdigest()
        or library_evidence["global_defined_symbol_count"] != len(symbols)
    ):
        raise SystemExit(f"slice {slice_id} symbols do not match closed evidence")
PY_ASSEMBLE_SLICES
}

if [[ "$MATRIX_MODE" != ordinary ]]; then
  write_matrix_context
fi

if [[ "$MATRIX_MODE" == produce ]]; then
  slice_configuration "$PRODUCE_SLICE_ID" || exit 1
  build_one_apple_slice "$PRODUCE_SLICE_ID"
  PRODUCED_LIBRARY="$(stage_cargo_library \
    "$SLICE_TARGET_TRIPLE" "$SLICE_LABEL" \
    "$CARGO_TARGET_DIR/$SLICE_TARGET_TRIPLE")"
  assert_bridge_source_seal "Apple slice evidence production"
  write_slice_bundle "$PRODUCED_LIBRARY"
  assert_bridge_source_seal "the completed $PRODUCE_SLICE_ID evidence bundle"
  rm -rf "$STAGE_DIR"
  echo "[+] Produced authenticated Apple slice bundle: $SLICE_OUTPUT_ROOT/$PRODUCE_SLICE_ID" >&2
  exit 0
fi

if [[ "$MATRIX_MODE" == assemble ]]; then
  assert_bridge_source_seal "Apple slice assembly preflight"
  assemble_slice_bundles
  assert_bridge_source_seal "Apple slice evidence revalidation"
else
  echo "[+] Building five Rust static libraries sequentially in fixed Cargo targets" >&2
  echo "    Targets: $DEVICE_TRIPLE, $SIM_ARM_TRIPLE, $SIM_X64_TRIPLE, $MACOS_ARM_TRIPLE, $MACOS_X64_TRIPLE" >&2
  build_one_apple_slice ios-arm64
  build_one_apple_slice ios-sim-arm64
  build_one_apple_slice ios-sim-x64
  build_one_apple_slice macos-arm64
  build_one_apple_slice macos-x64
fi

if [[ "$MATRIX_MODE" == assemble ]]; then
  LIB_DEV="$STAGE_DIR/cargo-libraries/$DEVICE_TRIPLE/lib${LIB_CRATE_NAME}.a"
  LIB_SIM_ARM="$STAGE_DIR/cargo-libraries/$SIM_ARM_TRIPLE/lib${LIB_CRATE_NAME}.a"
  LIB_SIM_X64="$STAGE_DIR/cargo-libraries/$SIM_X64_TRIPLE/lib${LIB_CRATE_NAME}.a"
  LIB_MAC_ARM="$STAGE_DIR/cargo-libraries/$MACOS_ARM_TRIPLE/lib${LIB_CRATE_NAME}.a"
  LIB_MAC_X64="$STAGE_DIR/cargo-libraries/$MACOS_X64_TRIPLE/lib${LIB_CRATE_NAME}.a"
else
  LIB_DEV=$(stage_cargo_library "$DEVICE_TRIPLE" "iOS device" "$CARGO_TARGET_DIR")
  LIB_SIM_ARM=$(stage_cargo_library "$SIM_ARM_TRIPLE" "arm64 simulator" "$CARGO_TARGET_DIR")
  LIB_SIM_X64=$(stage_cargo_library "$SIM_X64_TRIPLE" "x86_64 simulator" "$CARGO_TARGET_DIR")
  LIB_MAC_ARM=$(stage_cargo_library "$MACOS_ARM_TRIPLE" "arm64 macOS" "$CARGO_TARGET_DIR")
  LIB_MAC_X64=$(stage_cargo_library "$MACOS_X64_TRIPLE" "x86_64 macOS" "$CARGO_TARGET_DIR")
fi

assert_bridge_source_seal "Apple slice staging"

if [[ ! -f "$LIB_DEV" || ! -f "$LIB_SIM_ARM" || ! -f "$LIB_SIM_X64" \
    || ! -f "$LIB_MAC_ARM" || ! -f "$LIB_MAC_X64" ]]; then
  echo "[-] Missing authenticated Apple slice libraries" >&2
  exit 1
fi

PUBLISH_ROOT="$(mktemp -d "$OUT_DIR/.NoritoBridge.publish.XXXXXX")"
PUBLISH_XCFRAMEWORK="$PUBLISH_ROOT/${FRAMEWORK_NAME}.xcframework"
PUBLISH_MANIFEST="$PUBLISH_XCFRAMEWORK/${FRAMEWORK_NAME}.artifacts.json"
PUBLISH_MANIFEST_LINK="$PUBLISH_ROOT/${FRAMEWORK_NAME}.artifacts.json"
FINAL_XCFRAMEWORK="$OUT_DIR/${FRAMEWORK_NAME}.xcframework"
FINAL_MANIFEST="$OUT_DIR/${FRAMEWORK_NAME}.artifacts.json"
CANONICAL_MANIFEST_RELATIVE_TARGET="${FRAMEWORK_NAME}.xcframework/${FRAMEWORK_NAME}.artifacts.json"

SIM_UNI="$STAGE_DIR/${FRAMEWORK_NAME}-sim-universal.a"
MAC_UNI="$STAGE_DIR/${FRAMEWORK_NAME}-macos-universal.a"
echo "[+] Creating simulator universal static library" >&2
env -i \
  HOME="$USER_HOME_DIR" \
  PATH="${LIPO_BINARY%/*}:/usr/bin:/bin" \
  TMPDIR="$MOBILE_TMPDIR" \
  LANG=C.UTF-8 \
  LC_ALL=C.UTF-8 \
  DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
  "$LIPO_BINARY" -create -output "$SIM_UNI" "$LIB_SIM_ARM" "$LIB_SIM_X64"

echo "[+] Creating macOS universal static library" >&2
env -i \
  HOME="$USER_HOME_DIR" \
  PATH="${LIPO_BINARY%/*}:/usr/bin:/bin" \
  TMPDIR="$MOBILE_TMPDIR" \
  LANG=C.UTF-8 \
  LC_ALL=C.UTF-8 \
  DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
  "$LIPO_BINARY" -create -output "$MAC_UNI" "$LIB_MAC_ARM" "$LIB_MAC_X64"

echo "[+] Staging XCFramework slices" >&2
HEADERS_DEV="$STAGE_DIR/device-headers"
HEADERS_SIM="$STAGE_DIR/simulator-headers"
HEADERS_MAC="$STAGE_DIR/macos-headers"
LIB_DEV_STAGED="$STAGE_DIR/device/${STATIC_LIB_NAME}"
LIB_SIM_STAGED="$STAGE_DIR/simulator/${STATIC_LIB_NAME}"
LIB_MAC_STAGED="$STAGE_DIR/macos/${STATIC_LIB_NAME}"

mkdir -p "$HEADERS_DEV" "$HEADERS_SIM" "$HEADERS_MAC" "$(dirname "$LIB_DEV_STAGED")" "$(dirname "$LIB_SIM_STAGED")" "$(dirname "$LIB_MAC_STAGED")"

# Package only task-owned staging copies. The caller's Cargo target remains intact.
mv "$LIB_DEV" "$LIB_DEV_STAGED"
mv "$SIM_UNI" "$LIB_SIM_STAGED"
mv "$MAC_UNI" "$LIB_MAC_STAGED"
rm -f "$LIB_SIM_ARM" "$LIB_SIM_X64"

# Copy headers for static-library slices. xcodebuild copies this directory as the
# slice's Headers bundle, so the modulemap lives at Headers/module.modulemap.
cp "$INC_DIR/connect_norito_bridge.h" "$HEADERS_DEV/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$HEADERS_DEV/NoritoBridge.h"
cp "$CRATE_DIR/module.modulemap.template" "$HEADERS_DEV/module.modulemap"
cp "$INC_DIR/connect_norito_bridge.h" "$HEADERS_SIM/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$HEADERS_SIM/NoritoBridge.h"
cp "$CRATE_DIR/module.modulemap.template" "$HEADERS_SIM/module.modulemap"
cp "$INC_DIR/connect_norito_bridge.h" "$HEADERS_MAC/connect_norito_bridge.h"
cp "$INC_DIR/NoritoBridge.h" "$HEADERS_MAC/NoritoBridge.h"
cp "$CRATE_DIR/module.modulemap.template" "$HEADERS_MAC/module.modulemap"

echo "[+] Creating XCFramework" >&2
if env -i \
  HOME="$USER_HOME_DIR" \
  PATH=/usr/bin:/bin \
  TMPDIR="$MOBILE_TMPDIR" \
  LANG=C.UTF-8 \
  LC_ALL=C.UTF-8 \
  DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
  "$XCODEBUILD_BINARY" -create-xcframework \
  -library "$LIB_DEV_STAGED" -headers "$HEADERS_DEV" \
  -library "$LIB_SIM_STAGED" -headers "$HEADERS_SIM" \
  -library "$LIB_MAC_STAGED" -headers "$HEADERS_MAC" \
  -output "$PUBLISH_XCFRAMEWORK"; then
  :
else
  xcodebuild_status=$?
  echo "[-] xcodebuild failed with status $xcodebuild_status; refusing to publish NoritoBridge" >&2
  exit "$xcodebuild_status"
fi

assert_bridge_source_seal "XCFramework packaging"

echo "[+] XCFramework staged: $PUBLISH_XCFRAMEWORK" >&2
if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" ]]; then
  touch "$PUBLISH_XCFRAMEWORK/.privacy-production-enabled"
fi

IOS_BIN="$PUBLISH_XCFRAMEWORK/ios-arm64/${STATIC_LIB_NAME}"
SIM_BIN="$PUBLISH_XCFRAMEWORK/ios-arm64_x86_64-simulator/${STATIC_LIB_NAME}"
MAC_BIN="$PUBLISH_XCFRAMEWORK/macos-arm64_x86_64/${STATIC_LIB_NAME}"
if [[ ! -f "$IOS_BIN" || ! -f "$SIM_BIN" || ! -f "$MAC_BIN" ]]; then
  echo "[-] Missing XCFramework binaries needed to emit NoritoBridge.artifacts.json" >&2
  exit 1
fi

IOS_HASH=$(shasum -a 256 "$IOS_BIN" | awk '{print $1}')
SIM_HASH=$(shasum -a 256 "$SIM_BIN" | awk '{print $1}')
MAC_HASH=$(shasum -a 256 "$MAC_BIN" | awk '{print $1}')
HEADER_HASH=$(shasum -a 256 "$INC_DIR/connect_norito_bridge.h" | awk '{print $1}')
BRIDGE_ABI_VERSION="$(run_isolated_python - \
  "$INC_DIR/connect_norito_bridge.h" \
  "$CRATE_DIR/src/lib.rs" \
  "$ROOT_DIR/crates/iroha_data_model/src/privacy/protocol.rs" <<'PY'
from pathlib import Path
import re
import sys

header, bridge_source, protocol = (Path(value) for value in sys.argv[1:])
header_abis = re.findall(
    r"^#define[ \t]+CONNECT_NORITO_BRIDGE_ABI_VERSION[ \t]+([0-9]+)[ \t]*$",
    header.read_text(encoding="utf-8"),
    re.MULTILINE,
)
bridge_aliases = re.findall(
    r"^const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = "
    r"(PRIVACY_BRIDGE_ABI_VERSION_V1);$",
    bridge_source.read_text(encoding="utf-8"),
    re.MULTILINE,
)
protocol_abis = re.findall(
    r"^pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = ([0-9]+);$",
    protocol.read_text(encoding="utf-8"),
    re.MULTILINE,
)
if header_abis != ["22"]:
    raise SystemExit("authoritative NoritoBridge public header ABI is not exact 22")
if bridge_aliases != ["PRIVACY_BRIDGE_ABI_VERSION_V1"]:
    raise SystemExit("NoritoBridge Rust ABI alias is not exact")
if protocol_abis != header_abis:
    raise SystemExit("privacy protocol ABI differs from the public NoritoBridge header")
print(header_abis[0])
PY
)" || exit 1
# The mobile registry binds the Kagemusha ABI-22/V4 artifact family carried by
# the independently versioned native bridge ABI above.
KAGEMUSHA_ARTIFACT_ABI_VERSION=22
SOURCE_COMMIT="$SOURCE_COMMIT_START"
SOURCE_TREE_DIRTY=false
if [[ -n "$SOURCE_STATUS_START" ]]; then
  SOURCE_TREE_DIRTY=true
fi
SOURCE_FINGERPRINT="$SOURCE_FINGERPRINT_START"
PRIVACY_PRODUCTION_JSON=false
CARGO_FEATURES_JSON='[]'
if [[ "$PRIVACY_PRODUCTION_ENABLED" == "1" ]]; then
  PRIVACY_PRODUCTION_JSON=true
  CARGO_FEATURES_JSON='["privacy-production-enabled"]'
fi

cat > "$PUBLISH_MANIFEST" <<EOF
{
  "version": "$BRIDGE_VERSION",
  "native_bridge_abi_version": $BRIDGE_ABI_VERSION,
  "privacy_production_enabled": $PRIVACY_PRODUCTION_JSON,
  "cargo_features": $CARGO_FEATURES_JSON,
  "build_environment": {
    "schema": "iroha.mobile-native-build-environment.v1",
    "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
    "hermetic_runner_sha256": "$HERMETIC_RUNNER_SHA256",
    "environment_profiles": {
      "apple-ios-device": [
        "CARGO",
        "CARGO_BUILD_JOBS",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "DEVELOPER_DIR",
        "HOME",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "LANG",
        "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC",
        "PATH",
        "RUSTC",
        "RUSTC_BOOTSTRAP",
        "RUSTDOC",
        "RUSTUP_HOME",
        "SDKROOT",
        "TMPDIR"
      ],
      "apple-ios-simulator": [
        "CARGO",
        "CARGO_BUILD_JOBS",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "DEVELOPER_DIR",
        "HOME",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "IPHONESIMULATOR_DEPLOYMENT_TARGET",
        "LANG",
        "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC",
        "PATH",
        "RUSTC",
        "RUSTC_BOOTSTRAP",
        "RUSTDOC",
        "RUSTUP_HOME",
        "SDKROOT",
        "TMPDIR"
      ],
      "apple-macos": [
        "CARGO",
        "CARGO_BUILD_JOBS",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "DEVELOPER_DIR",
        "HOME",
        "LANG",
        "LC_ALL",
        "MACOSX_DEPLOYMENT_TARGET",
        "NORITO_SKIP_BINDINGS_SYNC",
        "PATH",
        "RUSTC",
        "RUSTC_BOOTSTRAP",
        "RUSTDOC",
        "RUSTUP_HOME",
        "SDKROOT",
        "TMPDIR"
      ]
    },
    "cargo_build_jobs": 1,
    "rust_toolchain_channel": "$PINNED_RUST_TOOLCHAIN",
    "cargo_release": "$CARGO_RELEASE",
    "cargo_commit_hash": "$CARGO_COMMIT_HASH",
    "cargo_binary_sha256": "$CARGO_BINARY_SHA256",
    "rustc_release": "$RUSTC_RELEASE",
    "rustc_commit_hash": "$RUSTC_COMMIT_HASH",
    "rustc_binary_sha256": "$RUSTC_BINARY_SHA256",
    "rustdoc_release": "$RUSTDOC_RELEASE",
    "rustdoc_commit_hash": "$RUSTDOC_COMMIT_HASH",
    "rustdoc_binary_sha256": "$RUSTDOC_BINARY_SHA256",
    "python_version": "$PYTHON_VERSION",
    "python_binary_sha256": "$PYTHON_BINARY_SHA256",
    "git_version": "$GIT_VERSION",
    "git_binary_sha256": "$GIT_BINARY_SHA256",
    "rustup_version": "$RUSTUP_VERSION",
    "rustup_binary_sha256": "$RUSTUP_BINARY_SHA256",
    "xcode_version": "$XCODE_VERSION",
    "xcode_build_version": "$XCODE_BUILD_VERSION",
    "iphoneos_sdk_version": "$IPHONEOS_SDK_VERSION",
    "iphonesimulator_sdk_version": "$IPHONESIMULATOR_SDK_VERSION",
    "macosx_sdk_version": "$MACOSX_SDK_VERSION",
    "iphoneos_deployment_target": "$IPHONEOS_DEPLOYMENT_TARGET",
    "iphonesimulator_deployment_target": "$IPHONESIMULATOR_DEPLOYMENT_TARGET",
    "macosx_deployment_target": "$MACOSX_DEPLOYMENT_TARGET"
  },
  "source_commit": "$SOURCE_COMMIT",
  "source_tree_dirty": $SOURCE_TREE_DIRTY,
  "source_fingerprint_sha256": "$SOURCE_FINGERPRINT",
  "cargo_lock_sha256": "$CARGO_LOCK_SHA256_START",
  "bridge_header_sha256": "$HEADER_HASH",
  "required_symbols": [
    "connect_norito_bridge_abi_version",
    "connect_norito_free",
    "connect_norito_chain_discriminant_scope_enter",
    "connect_norito_chain_discriminant_scope_exit",
    "connect_norito_encode_transfer_signed_transaction",
    "connect_norito_encode_transfer_instruction_box",
    "connect_norito_detached_transaction_scaffold_inspect_v1",
    "connect_norito_detached_transaction_scaffold_finalize_ed25519_v1",
    "connect_norito_canonical_json_blake3_v1",
    "connect_norito_encode_account_onboarding_plan_body_v1",
    "connect_norito_alias_instruction_round_trip_v1",
    "connect_norito_offline_cash_payment_request_canonicalize_v1",
    "connect_norito_offline_cash_payment_canonicalize_v1",
    "connect_norito_offline_cash_payment_canonicalize_for_session_v1",
    "connect_norito_offline_cash_acknowledgement_canonicalize_v1",
    "connect_norito_offline_cash_peer_encode_payment_request_v1",
    "connect_norito_offline_cash_peer_decode_payment_request_v1",
    "connect_norito_offline_cash_peer_encode_payment_v1",
    "connect_norito_offline_cash_peer_decode_payment_v1",
    "connect_norito_offline_cash_peer_encode_acknowledgement_v1",
    "connect_norito_offline_cash_peer_decode_acknowledgement_v1",
    "connect_norito_offline_cash_release_probe_v1",
    "connect_norito_offline_cash_artifact_begin_v1",
    "connect_norito_offline_cash_artifact_write_v1",
    "connect_norito_offline_cash_artifact_finalize_v1",
    "connect_norito_offline_cash_artifact_cancel_v1",
    "connect_norito_offline_cash_artifact_set_install_v1",
    "connect_norito_offline_cash_artifact_set_uninstall_v1",
    "connect_norito_offline_cash_verification_session_open_v1",
    "connect_norito_offline_cash_verification_session_open_bound_v1",
    "connect_norito_offline_cash_verification_session_verify_payment_v1",
    "connect_norito_offline_cash_verification_session_verify_acknowledgement_v1",
    "connect_norito_offline_cash_verification_session_state_v1",
    "connect_norito_offline_cash_verification_session_close_v1",
    "connect_norito_offline_cash_wallet_runtime_session_open_v1",
    "connect_norito_offline_cash_wallet_runtime_session_status_v1",
    "connect_norito_offline_cash_wallet_runtime_session_attempt_v1",
    "connect_norito_offline_cash_wallet_runtime_session_close_v1",
    "connect_norito_offline_cash_wallet_session_open_v1",
    "connect_norito_offline_cash_wallet_session_open_bound_v1",
    "connect_norito_offline_cash_wallet_session_accept_payment_v1",
    "connect_norito_offline_cash_wallet_session_accept_acknowledgement_v1",
    "connect_norito_offline_cash_wallet_session_state_v1",
    "connect_norito_offline_cash_wallet_session_close_v1",
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
    "connect_norito_sorafs_reference_validate_bundle_json",
    "connect_norito_sorafs_reference_validate_governance_json",
    "connect_norito_sorafs_reference_validate_governance_dag_block_json",
    "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json",
    "connect_norito_validation_fee_current_policy_proof_request_v1",
    "connect_norito_validation_fee_current_policy_proof_verify_v1",
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
    "connect_norito_kagemusha_topup_finality_verify_v4",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
    "connect_norito_kagemusha_output_membership_frontier_build_v4",
    "connect_norito_kagemusha_output_membership_paths_derive_v4",
    "connect_norito_kagemusha_recursive_spend_branch_validate_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_topup_v4",
    "connect_norito_kagemusha_project_top_up_submission_request_v4",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_project_redeem_submission_request_v4",
    "connect_norito_kagemusha_validate_operation_status_v4",
    "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
    "connect_norito_kagemusha_secret_free_buffer",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_recipient_receive_offer_create_v2",
    "connect_norito_kagemusha_recipient_receive_offer_project_v2",
    "connect_norito_kagemusha_recipient_receive_offer_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
    "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_split_change_prepare_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v4"
  ],
  "forbidden_symbols": [
    "connect_norito_get_chain_discriminant",
    "connect_norito_set_chain_discriminant",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
    "connect_norito_kagemusha_recipient_lineage_query_create_v2",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v2",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "iroha_privacy_capabilities_v1",
    "iroha_privacy_validate_capabilities_v1",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "CONNECT_NORITO_OFFLINE_CASH_TESTNET_DEVICE_EMULATOR_DO_NOT_SHIP_V1",
    "connect_norito_offline_cash_device_capabilities_v1",
    "connect_norito_offline_cash_device_execute_v1",
    "Java_org_hyperledger_iroha_sdk_offline_OfflineCashDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeCapabilitiesV1",
    "Java_org_hyperledger_iroha_sdk_offline_OfflineCashDeviceLifecycleBridgeV1_00024NativeEndpoint_nativeExecuteV1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2",
    "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientLineageQueryV2",
    "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientLineageQueryV2",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRegistrationLineageV2",
    "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRegistrationLineageV2"
  ],
  "kagemusha_mobile_artifact_roles": [
    {
      "role": "native_bridge",
      "purpose": "typed Norito codecs and privacy proof execution",
      "circuit_id": null,
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "xcframework",
      "delivery": "bridge_embedded",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "transfer_proving_key",
      "purpose": "prove exact confidential top-up and offline split transitions",
      "circuit_id": "confidential-transfer-v2",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "halo2_ipa_proving_key",
      "delivery": "bridge_embedded",
      "production_ready": $PRIVACY_PRODUCTION_JSON,
      "required_by": ["topup", "peer_send"]
    },
    {
      "role": "transfer_verifier_record",
      "purpose": "verify top-up and offline split evidence at an active height",
      "circuit_id": "confidential-transfer-v2",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "norito_verifying_key_record",
      "delivery": "torii_readiness_snapshot",
      "required_by": ["topup", "peer_send", "peer_receive"]
    },
    {
      "role": "unshield_proving_key",
      "purpose": "prove full or partial offline-to-online redemption",
      "circuit_id": "confidential-unshield-v3",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "halo2_ipa_proving_key",
      "delivery": "bridge_embedded",
      "production_ready": $PRIVACY_PRODUCTION_JSON,
      "required_by": ["redemption"]
    },
    {
      "role": "unshield_verifier_record",
      "purpose": "verify proof-bound public credit and optional offline change",
      "circuit_id": "confidential-unshield-v3",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "norito_verifying_key_record",
      "delivery": "torii_readiness_snapshot",
      "required_by": ["redemption"]
    },
    {
      "role": "step_eq_params_ipa",
      "purpose": "step_eq_params_ipa",
      "file_name": "step-eq.params-ipa.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_eq_proving_key",
      "purpose": "step_eq_proving_key",
      "file_name": "step-eq.proving-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "redemption"]
    },
    {
      "role": "step_eq_verifying_key",
      "purpose": "step_eq_verifying_key",
      "file_name": "step-eq.verifying-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_eq_bootstrap_witness",
      "purpose": "step_eq_bootstrap_witness",
      "file_name": "step-eq.bootstrap-witness.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-eq-compact-layout-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_ep_params_ipa",
      "purpose": "step_ep_params_ipa",
      "file_name": "step-ep.params-ipa.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_ep_proving_key",
      "purpose": "step_ep_proving_key",
      "file_name": "step-ep.proving-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "redemption"]
    },
    {
      "role": "step_ep_verifying_key",
      "purpose": "step_ep_verifying_key",
      "file_name": "step-ep.verifying-key.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "step_ep_bootstrap_witness",
      "purpose": "step_ep_bootstrap_witness",
      "file_name": "step-ep.bootstrap-witness.krv4",
      "circuit_id": "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "KagemushaRecursiveSpendPastaCycleArtifactsV4",
      "delivery": "content_addressed_external",
      "required_by": ["topup", "peer_send", "peer_receive", "redemption"]
    },
    {
      "role": "topup_finality_roster",
      "purpose": "topup_finality_roster",
      "circuit_id": "kagemusha-topup-finality-qc-merkle-v2",
      "abi": $KAGEMUSHA_ARTIFACT_ABI_VERSION,
      "artifact_type": "iroha_data_model::offline::model::KagemushaTopUpFinalityRosterArtifactV2",
      "delivery": "content_addressed_external",
      "required_by": ["topup"]
    }
  ],
  "hashes": {
    "ios-arm64": "$IOS_HASH",
    "ios-arm64_x86_64-simulator": "$SIM_HASH",
    "macos-arm64_x86_64": "$MAC_HASH"
  }
}
EOF
echo "[+] Wrote staged artifact manifest: $PUBLISH_MANIFEST" >&2
ln -s "$CANONICAL_MANIFEST_RELATIVE_TARGET" "$PUBLISH_MANIFEST_LINK"
PUBLISH_PROSPECTIVE_LOADER="$PUBLISH_ROOT/.NoritoBridge.prospective.NativeBridge.swift"
SWIFT_PIN_PREIMAGE_SHA256="$(
  sha256_file "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
)"
env -i \
  HOME="$USER_HOME_DIR" \
  PATH="${PYTHON_BINARY%/*}:${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${RUSTDOC_BINARY%/*}:${GIT_BINARY%/*}:/usr/bin:/bin" \
  TMPDIR="$MOBILE_TMPDIR" \
  LANG=C.UTF-8 \
  LC_ALL=C.UTF-8 \
  NORITO_BRIDGE_SEAL_HOME="$USER_HOME_DIR" \
  NORITO_BRIDGE_SEAL_CARGO_HOME="$MOBILE_CARGO_HOME" \
  NORITO_BRIDGE_SEAL_RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
  NORITO_BRIDGE_SEAL_TMPDIR="$MOBILE_TMPDIR" \
  NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
  NORITO_BRIDGE_SEAL_CARGO="$CARGO_BINARY" \
  NORITO_BRIDGE_SEAL_RUSTC="$RUSTC_BINARY" \
  NORITO_BRIDGE_SEAL_RUSTDOC="$RUSTDOC_BINARY" \
  NORITO_BRIDGE_SEAL_RUSTUP="$RUSTUP_BINARY" \
  NORITO_BRIDGE_SEAL_DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
  "$PYTHON_BINARY" -I -S -B \
  "$ROOT_DIR/scripts/update_norito_bridge_swift_pins.py" \
  --root "$ROOT_DIR" \
  --artifact-dir "$PUBLISH_ROOT" \
  --lockfile-path "$CARGO_LOCKFILE" \
  --output "$PUBLISH_PROSPECTIVE_LOADER" \
  --expected-preimage-sha256 "$SWIFT_PIN_PREIMAGE_SHA256"

run_isolated_python - \
  "$PUBLISH_XCFRAMEWORK" "$PUBLISH_MANIFEST" \
  "$PUBLISH_MANIFEST_LINK" "$CANONICAL_MANIFEST_RELATIVE_TARGET" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import plistlib
import stat
import sys


def object_without_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


xcframework = Path(sys.argv[1])
manifest_path = Path(sys.argv[2])
manifest_link = Path(sys.argv[3])
manifest_link_target = sys.argv[4]
expected_slices = {
    "ios-arm64": {
        "architectures": ["arm64"],
        "platform": "ios",
        "variant": None,
    },
    "ios-arm64_x86_64-simulator": {
        "architectures": ["arm64", "x86_64"],
        "platform": "ios",
        "variant": "simulator",
    },
    "macos-arm64_x86_64": {
        "architectures": ["arm64", "x86_64"],
        "platform": "macos",
        "variant": None,
    },
}
library_name = "libNoritoBridge.a"

if xcframework.is_symlink() or not xcframework.is_dir():
    raise SystemExit("staged NoritoBridge XCFramework root is not a regular directory")
for root, directories, files in os.walk(xcframework, followlinks=False):
    for name in [*directories, *files]:
        path = Path(root) / name
        if stat.S_ISLNK(path.lstat().st_mode):
            raise SystemExit(f"staged NoritoBridge contains a symlink: {path}")

with manifest_path.open("r", encoding="utf-8") as handle:
    manifest = json.load(handle, object_pairs_hook=object_without_duplicates)
if manifest.get("native_bridge_abi_version") != 22:
    raise SystemExit("staged NoritoBridge manifest does not bind exact ABI 22")
hashes = manifest.get("hashes")
if not isinstance(hashes, dict) or set(hashes) != set(expected_slices):
    raise SystemExit("staged NoritoBridge manifest has a non-canonical slice inventory")
if not manifest_link.is_symlink() or os.readlink(manifest_link) != manifest_link_target:
    raise SystemExit("staged NoritoBridge public manifest link is not canonical")

info_path = xcframework / "Info.plist"
with info_path.open("rb") as handle:
    info = plistlib.load(handle)
libraries = info.get("AvailableLibraries")
if (
    not isinstance(libraries, list)
    or len(libraries) != len(expected_slices)
    or any(not isinstance(library, dict) for library in libraries)
):
    raise SystemExit("staged NoritoBridge Info.plist has a non-canonical slice list")
metadata = {}
for library in libraries:
    identifier = library.get("LibraryIdentifier")
    if identifier in metadata:
        raise SystemExit(f"duplicate staged NoritoBridge slice: {identifier!r}")
    metadata[identifier] = library
if set(metadata) != set(expected_slices):
    raise SystemExit("staged NoritoBridge Info.plist has a non-canonical slice inventory")
if info.get("CFBundlePackageType") != "XFWK":
    raise SystemExit("staged NoritoBridge Info.plist has the wrong package type")
if info.get("XCFrameworkFormatVersion") != "1.0":
    raise SystemExit("staged NoritoBridge Info.plist has the wrong format version")

for identifier, expected in expected_slices.items():
    library = metadata[identifier]
    if library.get("LibraryPath") != library_name:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical LibraryPath"
        )
    if "BinaryPath" in library and library["BinaryPath"] != library_name:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a conflicting BinaryPath"
        )
    if library.get("HeadersPath") != "Headers":
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical HeadersPath"
        )
    if library.get("SupportedArchitectures") != expected["architectures"]:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has non-canonical architectures"
        )
    if library.get("SupportedPlatform") != expected["platform"]:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical platform"
        )
    if expected["variant"] is None:
        if "SupportedPlatformVariant" in library:
            raise SystemExit(
                f"staged NoritoBridge slice {identifier} has an unexpected variant"
            )
    elif library.get("SupportedPlatformVariant") != expected["variant"]:
        raise SystemExit(
            f"staged NoritoBridge slice {identifier} has a non-canonical variant"
        )

    binary = xcframework / identifier / library_name
    headers = xcframework / identifier / "Headers"
    modulemap = headers / "module.modulemap"
    if "module NoritoBridge" not in modulemap.read_text(encoding="utf-8"):
        raise SystemExit(f"staged NoritoBridge module map is invalid: {modulemap}")
    digest = hashlib.sha256()
    with binary.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    actual_hash = digest.hexdigest()
    if hashes[identifier] != actual_hash:
        raise SystemExit(f"staged NoritoBridge hash mismatch for {identifier}")

expected_top_level = {
    "Info.plist",
    "NoritoBridge.artifacts.json",
    *expected_slices,
}
privacy_marker = xcframework / ".privacy-production-enabled"
if manifest.get("privacy_production_enabled") is True:
    expected_top_level.add(privacy_marker.name)
elif manifest.get("privacy_production_enabled") is not False:
    raise SystemExit("staged NoritoBridge manifest has a non-boolean privacy mode")
test_only_marker = xcframework / ".test-only-prebuilt-slices"
if "test_only_prebuilt_slices" in manifest or test_only_marker.exists():
    raise SystemExit("release staged NoritoBridge contains test-only prebuilt slices")
if {entry.name for entry in xcframework.iterdir()} != expected_top_level:
    raise SystemExit("staged NoritoBridge has unexpected top-level artifacts")

expected_slice_entries = {"Headers", library_name}
expected_header_entries = {
    "NoritoBridge.h",
    "connect_norito_bridge.h",
    "module.modulemap",
}
for identifier in expected_slices:
    slice_path = xcframework / identifier
    headers_path = slice_path / "Headers"
    if {entry.name for entry in slice_path.iterdir()} != expected_slice_entries:
        raise SystemExit(f"staged NoritoBridge slice {identifier} has unexpected files")
    if {entry.name for entry in headers_path.iterdir()} != expected_header_entries:
        raise SystemExit(f"staged NoritoBridge slice {identifier} has unexpected headers")

def fsync_path(path):
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


directories_to_sync = []
for root, directories, files in os.walk(xcframework, topdown=True):
    root_path = Path(root)
    directories_to_sync.append(root_path)
    for name in files:
        fsync_path(root_path / name)
for directory in reversed(directories_to_sync):
    fsync_path(directory)
fsync_path(xcframework.parent)
PY

run_isolated_python \
  "$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py" \
  --root "$ROOT_DIR" \
  --lockfile-path "$CARGO_LOCKFILE" \
  --xcframework "$PUBLISH_XCFRAMEWORK" \
  --manifest "$PUBLISH_MANIFEST" \
  --manifest-link "$PUBLISH_MANIFEST_LINK" \
  --expected-link-target "$CANONICAL_MANIFEST_RELATIVE_TARGET" \
  --swift-loader "$PUBLISH_PROSPECTIVE_LOADER"

assert_bridge_source_seal "staged artifact validation"

if [[ "$ALLOW_DIRTY_SOURCE" == "1" ]]; then
  MOBILE_SDK_ALLOW_DIRTY_SOURCE=1 \
    MOBILE_SDK_APPLE_ARTIFACT_DIR="$PUBLISH_ROOT" \
    MOBILE_SDK_STAGED_BUILD_VALIDATION=1 \
    MOBILE_SDK_PROSPECTIVE_SWIFT_LOADER_PATH="$PUBLISH_PROSPECTIVE_LOADER" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
else
  MOBILE_SDK_APPLE_ARTIFACT_DIR="$PUBLISH_ROOT" \
    MOBILE_SDK_STAGED_BUILD_VALIDATION=1 \
    MOBILE_SDK_PROSPECTIVE_SWIFT_LOADER_PATH="$PUBLISH_PROSPECTIVE_LOADER" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
fi

assert_bridge_source_seal "pre-publication artifact verification"

echo "[+] Removing task-owned staging intermediates before publication" >&2
rm -rf "$STAGE_DIR"

assert_bridge_source_seal "the XCFramework publication exchange"
run_isolated_python - \
  "$PUBLISH_XCFRAMEWORK" "$FINAL_XCFRAMEWORK" "$FINAL_MANIFEST" \
  "$CANONICAL_MANIFEST_RELATIVE_TARGET" <<'PY'
import ctypes
import hashlib
import json
import os
from pathlib import Path
import stat
import sys


RENAME_EXCHANGE = 0x00000002
libc = ctypes.CDLL(None, use_errno=True)
SLICE_PATHS = {
    "ios-arm64": "libNoritoBridge.a",
    "ios-arm64_x86_64-simulator": "libNoritoBridge.a",
    "macos-arm64_x86_64": "libNoritoBridge.a",
}


def object_without_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def fsync_path(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def validate_manifest_bytes(framework: Path, manifest_bytes: bytes) -> None:
    try:
        manifest = json.loads(
            manifest_bytes.decode("utf-8"),
            object_pairs_hook=object_without_duplicates,
        )
    except (UnicodeError, ValueError, TypeError) as error:
        raise RuntimeError("live NoritoBridge manifest is malformed") from error
    hashes = manifest.get("hashes")
    if not isinstance(hashes, dict) or set(hashes) != set(SLICE_PATHS):
        raise RuntimeError("live NoritoBridge manifest has a non-canonical hash inventory")
    for identifier, library_name in SLICE_PATHS.items():
        binary = framework / identifier / library_name
        if (
            not binary.is_file()
            or binary.is_symlink()
            or (framework / identifier).is_symlink()
        ):
            raise RuntimeError(f"live NoritoBridge slice is not regular: {binary}")
        digest = hashlib.sha256()
        with binary.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        if hashes[identifier] != digest.hexdigest():
            raise RuntimeError(f"live NoritoBridge hash mismatch for {identifier}")


def install_public_manifest_link(link: Path, target: str) -> None:
    temporary = link.parent / f".NoritoBridge.artifacts.link.{os.getpid()}"
    if os.path.lexists(temporary):
        raise RuntimeError(f"refusing unexpected manifest-link temporary: {temporary}")
    try:
        os.symlink(target, temporary)
        os.replace(temporary, link)
        fsync_path(link.parent)
    finally:
        if os.path.lexists(temporary):
            temporary.unlink()


def prepare_stable_manifest_link(
    final_framework: Path,
    final_manifest: Path,
    relative_target: str,
) -> None:
    framework_exists = os.path.lexists(final_framework)
    manifest_exists = os.path.lexists(final_manifest)

    if framework_exists:
        if final_framework.is_symlink() or not final_framework.is_dir():
            raise RuntimeError(
                f"refusing unexpected live XCFramework path: {final_framework}"
            )
        embedded = final_framework / "NoritoBridge.artifacts.json"
        if (
            not manifest_exists
            or not final_manifest.is_symlink()
            or os.readlink(final_manifest) != relative_target
        ):
            raise RuntimeError(
                "live XCFramework requires its canonical public manifest link"
            )
        if embedded.is_symlink() or not embedded.is_file():
            raise RuntimeError("canonical public manifest link has no regular target")
        contents = embedded.read_bytes()
        validate_manifest_bytes(final_framework, contents)
        return

    if manifest_exists:
        if (
            not final_manifest.is_symlink()
            or os.readlink(final_manifest) != relative_target
        ):
            raise RuntimeError(
                "refusing an orphaned non-canonical public NoritoBridge manifest"
            )
    else:
        install_public_manifest_link(final_manifest, relative_target)


def exchange(left: Path, right: Path) -> None:
    encoded_left = os.fsencode(left)
    encoded_right = os.fsencode(right)
    if hasattr(libc, "renamex_np"):
        renamex_np = libc.renamex_np
        renamex_np.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        renamex_np.restype = ctypes.c_int
        result = renamex_np(encoded_left, encoded_right, RENAME_EXCHANGE)
    elif hasattr(libc, "renameat2"):
        renameat2 = libc.renameat2
        renameat2.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        renameat2.restype = ctypes.c_int
        result = renameat2(
            -100, encoded_left, -100, encoded_right, RENAME_EXCHANGE
        )
    else:
        raise OSError("this host lacks an atomic path-exchange primitive")
    if result != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), f"{left} <-> {right}")


def publish(staged: Path, final: Path) -> None:
    if os.path.lexists(final):
        exchange(staged, final)
    else:
        os.rename(staged, final)
    fsync_path(staged.parent)
    fsync_path(final.parent)


staged_xcframework = Path(sys.argv[1])
final_xcframework = Path(sys.argv[2])
final_manifest = Path(sys.argv[3])
relative_target = sys.argv[4]

prepare_stable_manifest_link(
    final_xcframework,
    final_manifest,
    relative_target,
)
publish(staged_xcframework, final_xcframework)
PY

assert_bridge_source_seal "the published XCFramework"

echo "[+] Atomically published XCFramework and canonical manifest: $FINAL_XCFRAMEWORK" >&2
echo "[+] Public manifest link: $FINAL_MANIFEST -> $CANONICAL_MANIFEST_RELATIVE_TARGET" >&2

if [[ -n "$ARCHIVE_OUTPUT" ]]; then
  ARCHIVE_OWNER="$ROOT_DIR/scripts/archive_norito_xcframework.py"
  if [[ ! -f "$ARCHIVE_OWNER" || -L "$ARCHIVE_OWNER" ]]; then
    echo "[-] Deterministic NoritoBridge archive owner is unavailable: $ARCHIVE_OWNER" >&2
    exit 1
  fi
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${PYTHON_BINARY%/*}:${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${RUSTDOC_BINARY%/*}:${GIT_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" \
    NORITO_BRIDGE_OUTPUT_LOCK_FD="$NORITO_BRIDGE_OUTPUT_LOCK_FD" \
    NORITO_BRIDGE_SEAL_HOME="$USER_HOME_DIR" \
    NORITO_BRIDGE_SEAL_CARGO_HOME="$MOBILE_CARGO_HOME" \
    NORITO_BRIDGE_SEAL_RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
    NORITO_BRIDGE_SEAL_TMPDIR="$MOBILE_TMPDIR" \
    NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    NORITO_BRIDGE_SEAL_CARGO="$CARGO_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTC="$RUSTC_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTDOC="$RUSTDOC_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTUP="$RUSTUP_BINARY" \
    NORITO_BRIDGE_SEAL_DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
    "$PYTHON_BINARY" -I -S -B "$ARCHIVE_OWNER" \
      --xcframework "$FINAL_XCFRAMEWORK" \
      --output "$ARCHIVE_OUTPUT" \
      --scratch-dir "$BUILD_DIR" \
      --lockfile-path "$CARGO_LOCKFILE"
  assert_bridge_source_seal "the archive publication"
  echo "[+] Deterministic XCFramework archive: $ARCHIVE_OUTPUT" >&2
fi

assert_bridge_source_seal "the completed Apple artifact build"
