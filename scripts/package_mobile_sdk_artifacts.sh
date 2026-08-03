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
      echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PYTHON_BINARY must be an absolute canonical regular executable" >&2
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
      echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PYTHON_BINARY must already name its canonical executable" >&2
      return 1
    fi
    printf '%s\n' "$canonical"
    return 0
  done

  if [[ -n "$override" ]]; then
    echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PYTHON_BINARY must be an isolated Python 3.12 executable" >&2
  else
    echo "[mobile-sdk-package] ERROR: a trusted absolute Python 3.12 executable is required" >&2
  fi
  return 1
}

PYTHON_BINARY="$(resolve_trusted_python312)" || exit 1
ORIGINAL_ARGUMENTS=("$@")

run_isolated_python() {
  env -i \
    HOME=/tmp \
    PATH=/usr/bin:/bin \
    TMPDIR=/tmp \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$PYTHON_BINARY" -I -S -B "$@"
}

usage() {
  cat <<'USAGE'
Usage:
  scripts/package_mobile_sdk_artifacts.sh [--root <repo-root>] [--version <version>] [--apple] [--android]

Packages built mobile SDK artifacts into an explicit external cache directory:
  --apple    Package NoritoBridge.xcframework and its artifact manifest.
  --android  Package Kotlin core/client/offline-wallet Android release outputs,
             generated native bridge bytes, and their embedded provenance.

MOBILE_SDK_APPLE_ARTIFACT_DIR is required for Apple packaging and must select an
external Apple artifact directory.
MOBILE_SDK_ANDROID_ARTIFACT_DIR is required for Android release packaging and
must identify the canonical external Gradle/artifact root.
MOBILE_SDK_PACKAGE_OUT_DIR is required and selects a dedicated external package directory
whose final path component contains "mobile-sdk".
MOBILE_SDK_PYTHON_BINARY may select an absolute, already-canonical regular
Python 3.12 executable when the fixed Homebrew/system locators are unavailable.
Source-authenticated packaging invokes scripts/check_mobile_sdk_artifacts.sh
and therefore also requires exact Rust 1.93.1 RUSTC/RUSTDOC plus one canonical
writable external CARGO_TARGET_DIR. Only the repository-root Cargo.lock is
accepted.
Apple packaging additionally requires an explicit canonical SOURCE_DATE_EPOCH;
the deterministic archive owner normalizes every ZIP entry to that instant.

When neither --apple nor --android is passed, both platforms are packaged.
USAGE
}

ROOT_ARG=""
VERSION=""
PACKAGE_APPLE=0
PACKAGE_ANDROID=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-package] ERROR: --root requires a value" >&2
        exit 64
      fi
      ROOT_ARG="$1"
      ;;
    --root=*)
      ROOT_ARG="${1#*=}"
      ;;
    --version)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-package] ERROR: --version requires a value" >&2
        exit 64
      fi
      VERSION="$1"
      ;;
    --version=*)
      VERSION="${1#*=}"
      ;;
    --apple)
      PACKAGE_APPLE=1
      ;;
    --android)
      PACKAGE_ANDROID=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[mobile-sdk-package] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
  shift
done

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
ROOT_DIR="$(cd "$ROOT_ARG" && pwd -P)"
APPLE_ARCHIVE_OWNER="$ROOT_DIR/scripts/archive_norito_xcframework.py"
APPLE_ARTIFACT_DIR="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-}"

if [[ "$PACKAGE_APPLE" == "0" && "$PACKAGE_ANDROID" == "0" ]]; then
  PACKAGE_APPLE=1
  PACKAGE_ANDROID=1
fi

if [[ "$PACKAGE_APPLE" == "1" && -z "$APPLE_ARTIFACT_DIR" ]]; then
  echo "[mobile-sdk-package] ERROR: Apple packaging requires an explicit external MOBILE_SDK_APPLE_ARTIFACT_DIR" >&2
  exit 66
fi
if [[ -z "${MOBILE_SDK_PACKAGE_OUT_DIR:-}" ]]; then
  echo "[mobile-sdk-package] ERROR: MOBILE_SDK_PACKAGE_OUT_DIR is required and must be external" >&2
  exit 65
fi
OUT_DIR="$MOBILE_SDK_PACKAGE_OUT_DIR"
if [[ "$OUT_DIR" != /* ]]; then
  echo "[mobile-sdk-package] ERROR: package output must be an absolute external path: $OUT_DIR" >&2
  exit 65
fi
case "$OUT_DIR/" in
  *"/../"*|*"/./"*|*"//"*)
    echo "[mobile-sdk-package] ERROR: package output path must be canonical: $OUT_DIR" >&2
    exit 65
    ;;
esac
OUT_BASENAME="${OUT_DIR##*/}"
case "$OUT_BASENAME" in
  *mobile-sdk*) ;;
  *)
    echo "[mobile-sdk-package] ERROR: package output must be a dedicated mobile-sdk directory: $OUT_DIR" >&2
    exit 65
    ;;
esac
OUT_PARENT="${OUT_DIR%/*}"
if [[ ! -d "$OUT_PARENT" || -L "$OUT_PARENT" ]]; then
  echo "[mobile-sdk-package] ERROR: package output parent must be an existing non-symbolic directory: $OUT_PARENT" >&2
  exit 65
fi
RAW_OUT_PARENT="$OUT_PARENT"
OUT_PARENT="$(cd "$RAW_OUT_PARENT" && pwd -P)"
if [[ "$OUT_PARENT" != "$RAW_OUT_PARENT" ]]; then
  echo "[mobile-sdk-package] ERROR: package output parent must not traverse symbolic links: $RAW_OUT_PARENT" >&2
  exit 65
fi
OUT_DIR="$OUT_PARENT/$OUT_BASENAME"
if [[ -L "$OUT_DIR" ]]; then
  echo "[mobile-sdk-package] ERROR: package output must not be a symbolic link: $OUT_DIR" >&2
  exit 65
fi
case "$OUT_DIR" in
  /|"$ROOT_DIR"|"$APPLE_ARTIFACT_DIR")
    echo "[mobile-sdk-package] ERROR: refusing broad package output path: $OUT_DIR" >&2
    exit 65
    ;;
esac
case "$OUT_DIR/" in
  "$ROOT_DIR/"*)
    echo "[mobile-sdk-package] ERROR: package output must be outside the Iroha source tree" >&2
    exit 65
    ;;
esac

PACKAGE_SCRIPT="$ROOT_DIR/scripts/package_mobile_sdk_artifacts.sh"
PACKAGE_LOCK_RUNNER="$ROOT_DIR/scripts/exec_with_file_lock.py"
PACKAGE_LOCK="$OUT_PARENT/.${OUT_BASENAME}.publish.lockfile"
PACKAGE_LOCK_ENV="MOBILE_SDK_PACKAGE_LOCK_FDS"
APPLE_SOURCE_LOCK=""
PACKAGE_LOCK_PATHS=("$PACKAGE_LOCK")
if [[ "$PACKAGE_APPLE" == "1" ]]; then
  if [[ ! -d "$APPLE_ARTIFACT_DIR" || -L "$APPLE_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Apple artifact directory must be an existing non-symbolic directory: $APPLE_ARTIFACT_DIR" >&2
    exit 66
  fi
  canonical_apple_artifact_dir="$(cd "$APPLE_ARTIFACT_DIR" && pwd -P)"
  if [[ "$canonical_apple_artifact_dir" != "$APPLE_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Apple artifact directory must be canonical: $APPLE_ARTIFACT_DIR" >&2
    exit 66
  fi
  case "$APPLE_ARTIFACT_DIR/" in
    "$ROOT_DIR/"*)
      echo "[mobile-sdk-package] ERROR: Apple artifact directory must be outside the Iroha source tree" >&2
      exit 66
      ;;
  esac
  APPLE_SOURCE_LOCK="$APPLE_ARTIFACT_DIR/.NoritoBridge.publish.lockfile"
  PACKAGE_LOCK_PATHS+=("$APPLE_SOURCE_LOCK")
fi
if [[ ! -f "$PACKAGE_SCRIPT" || -L "$PACKAGE_SCRIPT" \
  || "$(cd "$(dirname "$PACKAGE_SCRIPT")" && pwd -P)/$(basename "$PACKAGE_SCRIPT")" != "$PACKAGE_SCRIPT" ]]; then
  echo "[mobile-sdk-package] ERROR: package owner must be a canonical non-symbolic regular file: $PACKAGE_SCRIPT" >&2
  exit 65
fi
if [[ ! -f "$PACKAGE_LOCK_RUNNER" || -L "$PACKAGE_LOCK_RUNNER" \
  || "$(cd "$(dirname "$PACKAGE_LOCK_RUNNER")" && pwd -P)/$(basename "$PACKAGE_LOCK_RUNNER")" != "$PACKAGE_LOCK_RUNNER" ]]; then
  echo "[mobile-sdk-package] ERROR: package lock runner must be a canonical non-symbolic regular file: $PACKAGE_LOCK_RUNNER" >&2
  exit 65
fi

if [[ -z "${MOBILE_SDK_PACKAGE_LOCK_FDS:-}" ]]; then
  export MOBILE_SDK_PYTHON_BINARY="$PYTHON_BINARY"
  exec "$PYTHON_BINARY" -I -S -B "$PACKAGE_LOCK_RUNNER" \
    "$PACKAGE_LOCK_ENV" "${PACKAGE_LOCK_PATHS[@]}" -- \
    /bin/bash "$PACKAGE_SCRIPT" "${ORIGINAL_ARGUMENTS[@]}"
fi

authenticate_package_lock() {
  local authenticated
  authenticated="$(run_isolated_python - \
    "${MOBILE_SDK_PACKAGE_LOCK_FDS:-}" "$PACKAGE_LOCK" "$APPLE_SOURCE_LOCK" <<'PY'
import fcntl
import os
from pathlib import Path
import re
import stat
import sys

raw_descriptors = sys.argv[1]
package_lock = Path(sys.argv[2])
apple_lock = Path(sys.argv[3]) if sys.argv[3] else None
raw_parts = raw_descriptors.split(",")
if not raw_parts or any(re.fullmatch(r"[1-9][0-9]*", raw) is None for raw in raw_parts):
    raise SystemExit("[mobile-sdk-package] ERROR: inherited package lock descriptors are not canonical")
descriptors = [int(raw, 10) for raw in raw_parts]
if any(descriptor < 3 or str(descriptor) != raw for descriptor, raw in zip(descriptors, raw_parts)):
    raise SystemExit("[mobile-sdk-package] ERROR: inherited package lock descriptors are not canonical")
lock_paths = sorted(
    [package_lock] + ([apple_lock] if apple_lock is not None else []),
    key=os.fspath,
)
if len(descriptors) != len(lock_paths):
    raise SystemExit("[mobile-sdk-package] ERROR: inherited package lock descriptor count is invalid")
descriptor_by_path = dict(zip(lock_paths, descriptors))
for lock_path, descriptor in zip(lock_paths, descriptors):
    try:
        descriptor_metadata = os.fstat(descriptor)
        path_metadata = lock_path.lstat()
    except OSError as error:
        raise SystemExit(f"[mobile-sdk-package] ERROR: unable to authenticate package lock: {error}")
    if (
        not stat.S_ISREG(descriptor_metadata.st_mode)
        or not stat.S_ISREG(path_metadata.st_mode)
        or stat.S_ISLNK(path_metadata.st_mode)
        or descriptor_metadata.st_dev != path_metadata.st_dev
        or descriptor_metadata.st_ino != path_metadata.st_ino
        or descriptor_metadata.st_nlink != 1
        or path_metadata.st_nlink != 1
        or descriptor_metadata.st_uid != os.geteuid()
        or path_metadata.st_uid != os.geteuid()
    ):
        raise SystemExit("[mobile-sdk-package] ERROR: inherited package lock does not match its authenticated path")
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        raise SystemExit(f"[mobile-sdk-package] ERROR: inherited package lock is not held: {error}")
print(descriptor_by_path[package_lock], end=" ")
print(descriptor_by_path[apple_lock] if apple_lock is not None else "-")
PY
)"
  read -r PACKAGE_LOCK_FD APPLE_SOURCE_LOCK_FD <<<"$authenticated"
  if [[ "$PACKAGE_APPLE" == "1" ]]; then
    if [[ ! "$APPLE_SOURCE_LOCK_FD" =~ ^[1-9][0-9]*$ ]]; then
      echo "[mobile-sdk-package] ERROR: authenticated Apple source lock descriptor is invalid" >&2
      exit 70
    fi
    export NORITO_BRIDGE_OUTPUT_LOCK_FD="$APPLE_SOURCE_LOCK_FD"
  else
    unset NORITO_BRIDGE_OUTPUT_LOCK_FD
  fi
}

authenticate_package_lock

ANDROID_ARTIFACT_DIR="${MOBILE_SDK_ANDROID_ARTIFACT_DIR:-}"
ANDROID_KOTLIN_BUILD_ROOT=""
ANDROID_MAVEN_REPO_DIR=""
if [[ "$PACKAGE_ANDROID" == "1" ]]; then
  if [[ -z "$ANDROID_ARTIFACT_DIR" || "$ANDROID_ARTIFACT_DIR" != /* \
    || ! -d "$ANDROID_ARTIFACT_DIR" || -L "$ANDROID_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Android release packaging requires an absolute non-symbolic MOBILE_SDK_ANDROID_ARTIFACT_DIR" >&2
    exit 66
  fi
  canonical_android_artifact_dir="$(cd "$ANDROID_ARTIFACT_DIR" && pwd -P)"
  if [[ "$canonical_android_artifact_dir" != "$ANDROID_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: MOBILE_SDK_ANDROID_ARTIFACT_DIR must be canonical" >&2
    exit 66
  fi
  case "$ANDROID_ARTIFACT_DIR/" in
    "$ROOT_DIR/"*)
      echo "[mobile-sdk-package] ERROR: MOBILE_SDK_ANDROID_ARTIFACT_DIR must be outside the Iroha source tree" >&2
      exit 66
      ;;
  esac
  ANDROID_KOTLIN_BUILD_ROOT="$ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk"
  ANDROID_MAVEN_REPO_DIR="${MOBILE_SDK_ANDROID_MAVEN_REPO_DIR:-$ANDROID_ARTIFACT_DIR/maven}"
fi

if [[ -z "$VERSION" ]]; then
  VERSION_SOURCE="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  if [[ -f "$VERSION_SOURCE" ]]; then
    if command -v rg >/dev/null 2>&1; then
      VERSION="$(rg -n "expectedVersion" "$VERSION_SOURCE" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
    else
      VERSION="$(grep -m1 "expectedVersion" "$VERSION_SOURCE" | sed -E 's/.*"([^"]+)".*/\1/')"
    fi
  fi
fi

if [[ -z "$VERSION" ]]; then
  VERSION="$(git -C "$ROOT_DIR" describe --tags --always --dirty 2>/dev/null || true)"
fi

if [[ -z "$VERSION" ]]; then
  echo "[mobile-sdk-package] ERROR: unable to determine artifact version" >&2
  exit 65
fi

VERSION="${VERSION//\//-}"
if [[ ! "$VERSION" =~ ^[A-Za-z0-9._+-]+$ ]]; then
  echo "[mobile-sdk-package] ERROR: version contains unsupported filename characters: $VERSION" >&2
  exit 65
fi

MODE_LABEL="all"
if [[ "$PACKAGE_APPLE" == "1" && "$PACKAGE_ANDROID" == "0" ]]; then
  MODE_LABEL="apple"
elif [[ "$PACKAGE_APPLE" == "0" && "$PACKAGE_ANDROID" == "1" ]]; then
  MODE_LABEL="android"
fi

FINAL_OUT_DIR="$OUT_DIR"
if [[ -e "$FINAL_OUT_DIR" && ( ! -d "$FINAL_OUT_DIR" || -L "$FINAL_OUT_DIR" ) ]]; then
  echo "[mobile-sdk-package] ERROR: existing package output must be a non-symbolic directory: $FINAL_OUT_DIR" >&2
  exit 65
fi
FINAL_OUT_BASELINE="$(run_isolated_python - "$FINAL_OUT_DIR" <<'PY'
from pathlib import Path
import stat
import sys

path = Path(sys.argv[1])
try:
    metadata = path.lstat()
except FileNotFoundError:
    print("absent")
else:
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit("[mobile-sdk-package] ERROR: package output is not a non-symbolic directory")
    print(
        f"directory:{metadata.st_dev}:{metadata.st_ino}:"
        f"{metadata.st_mode}:{metadata.st_mtime_ns}"
    )
PY
)"
if [[ "$FINAL_OUT_BASELINE" != "absent" ]]; then
  echo "[mobile-sdk-package] ERROR: first-release package output must not already exist: $FINAL_OUT_DIR" >&2
  exit 65
fi
PACKAGE_STAGE_DIR="$(mktemp -d "$OUT_PARENT/.${OUT_BASENAME}.publish.XXXXXXXX")"
PACKAGE_STAGE_BASELINE="$(run_isolated_python - "$PACKAGE_STAGE_DIR" <<'PY'
from pathlib import Path
import stat
import sys

path = Path(sys.argv[1])
metadata = path.lstat()
if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
    raise SystemExit("[mobile-sdk-package] ERROR: package stage is not canonical")
print(
    f"directory:{metadata.st_dev}:{metadata.st_ino}:"
    f"{metadata.st_mode}"
)
PY
)"
cleanup_package_stage() {
  local status=$?
  if [[ -n "${PACKAGE_STAGE_DIR:-}" ]]; then
    case "$PACKAGE_STAGE_DIR" in
      "$OUT_PARENT/.${OUT_BASENAME}.publish."*)
        echo "[mobile-sdk-package] retained failed package stage: $PACKAGE_STAGE_DIR" >&2
        ;;
      *)
        echo "[mobile-sdk-package] ERROR: refusing to clean unexpected package stage: $PACKAGE_STAGE_DIR" >&2
        status=70
        ;;
    esac
  fi
  exit "$status"
}
trap cleanup_package_stage EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
OUT_DIR="$PACKAGE_STAGE_DIR"

CHECKSUMS="$OUT_DIR/SHA256SUMS-${MODE_LABEL}-${VERSION}.txt"
MANIFEST="$OUT_DIR/mobile-sdk-${MODE_LABEL}-${VERSION}.artifacts.json"
ARTIFACT_RECORDS=()

hash_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  else
    run_isolated_python - "$path" <<'PY'
import hashlib
import sys

hasher = hashlib.sha256()
with open(sys.argv[1], "rb") as handle:
    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        hasher.update(chunk)
print(hasher.hexdigest())
PY
  fi
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ ! -f "$path" ]]; then
    echo "[mobile-sdk-package] ERROR: missing $label: $path" >&2
    exit 66
  fi
}

require_dir() {
  local path="$1"
  local label="$2"
  if [[ ! -d "$path" ]]; then
    echo "[mobile-sdk-package] ERROR: missing $label: $path" >&2
    exit 66
  fi
}

single_match() {
  local pattern="$1"
  local label="$2"
  local matches=()
  local match

  while IFS= read -r match; do
    matches+=("$match")
  done < <(compgen -G "$pattern" || true)

  if [[ "${#matches[@]}" -ne 1 ]]; then
    echo "[mobile-sdk-package] ERROR: expected exactly one $label for pattern $pattern, found ${#matches[@]}" >&2
    exit 66
  fi

  printf '%s' "${matches[0]}"
}

resolve_core_jar() {
  local stripped_version="${VERSION#v}"
  local candidate

  for candidate in \
    "$ANDROID_KOTLIN_BUILD_ROOT/core-jvm/libs/core-jvm-${VERSION}.jar" \
    "$ANDROID_KOTLIN_BUILD_ROOT/core-jvm/libs/core-jvm-${stripped_version}.jar"; do
    if [[ -f "$candidate" ]]; then
      printf '%s' "$candidate"
      return
    fi
  done

  single_match "$ANDROID_KOTLIN_BUILD_ROOT/core-jvm/libs/core-jvm-*.jar" "core-jvm built jar"
}

resolve_android_native_mode() {
  local aar="$1"
  run_isolated_python - "$aar" <<'PY'
import json
import sys
import zipfile

entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(sys.argv[1]) as archive:
    manifest = json.loads(archive.read(entry))
production = manifest.get("privacy_production_enabled")
if type(production) is not bool:
    raise SystemExit("native provenance privacy_production_enabled is not boolean")
print("production" if production else "default")
PY
}

record_artifact() {
  local path="$1"
  local kind="$2"
  local name rel sha bytes

  require_file "$path" "$kind artifact"
  name="$(basename "$path")"
  if [[ "$path" == "$OUT_DIR/"* ]]; then
    rel="${path#"$OUT_DIR/"}"
  else
    rel="${path#"$ROOT_DIR/"}"
  fi
  sha="$(hash_file "$path")"
  bytes="$(wc -c < "$path" | tr -d '[:space:]')"
  printf '%s  %s\n' "$sha" "$rel" >> "$CHECKSUMS"
  ARTIFACT_RECORDS+=("    {\"kind\":\"$kind\",\"name\":\"$name\",\"path\":\"$rel\",\"sha256\":\"$sha\",\"bytes\":$bytes}")
}

copy_android_artifact() {
  local src="$1"
  local dest="$2"
  local stage="$3"
  local stage_checksums="$4"
  local sha

  require_file "$src" "Android SDK package input"
  mkdir -p "$(dirname "$stage/$dest")"
  cp "$src" "$stage/$dest"
  sha="$(hash_file "$stage/$dest")"
  printf '%s  %s\n' "$sha" "$dest" >> "$stage_checksums"
}

write_manifest() {
  local index count
  count="${#ARTIFACT_RECORDS[@]}"
  if [[ "$count" -eq 0 ]]; then
    echo "[mobile-sdk-package] ERROR: no artifacts were packaged" >&2
    exit 70
  fi

  {
    printf '{\n'
    printf '  "version": "%s",\n' "$VERSION"
    printf '  "mode": "%s",\n' "$MODE_LABEL"
    printf '  "artifacts": [\n'
    for index in "${!ARTIFACT_RECORDS[@]}"; do
      if [[ "$index" -gt 0 ]]; then
        printf ',\n'
      fi
      printf '%s' "${ARTIFACT_RECORDS[$index]}"
    done
    printf '\n  ]\n'
    printf '}\n'
  } > "$MANIFEST"

  record_artifact "$MANIFEST" "manifest"
}

prepare_apple_archive_provenance_environment() {
  local user_home developer_dir authenticated
  local cargo_home rustup_home temporary_dir cargo_target
  local cargo_binary rustc_binary rustdoc_binary rustup_binary

  user_home="${NORITO_BRIDGE_SEAL_HOME:-$(run_isolated_python -c '
import os
from pathlib import Path
import pwd
print(Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True))
')}"
  cargo_home="${NORITO_BRIDGE_SEAL_CARGO_HOME:-$user_home/.cargo}"
  rustup_home="${NORITO_BRIDGE_SEAL_RUSTUP_HOME:-$user_home/.rustup}"
  temporary_dir="${NORITO_BRIDGE_SEAL_TMPDIR:-/tmp}"
  cargo_target="${NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR:-${CARGO_TARGET_DIR:-}}"
  rustc_binary="${NORITO_BRIDGE_SEAL_RUSTC:-${RUSTC:-}}"
  rustdoc_binary="${NORITO_BRIDGE_SEAL_RUSTDOC:-${RUSTDOC:-}}"
  if [[ -n "${NORITO_BRIDGE_SEAL_CARGO:-}" ]]; then
    cargo_binary="$NORITO_BRIDGE_SEAL_CARGO"
  elif [[ -n "$rustc_binary" ]]; then
    cargo_binary="${rustc_binary%/*}/cargo"
  else
    cargo_binary=""
  fi
  rustup_binary="${NORITO_BRIDGE_SEAL_RUSTUP:-$user_home/.cargo/bin/rustup}"
  developer_dir="${NORITO_BRIDGE_SEAL_DEVELOPER_DIR:-${NORITO_BRIDGE_DEVELOPER_DIR:-}}"
  if [[ -z "$developer_dir" ]]; then
    if [[ ! -x /usr/bin/xcode-select ]]; then
      echo "[mobile-sdk-package] ERROR: Xcode developer directory is required for Apple archive provenance" >&2
      exit 66
    fi
    developer_dir="$(/usr/bin/xcode-select -p)"
  fi

  authenticated="$(run_isolated_python - \
    "$user_home" "$cargo_home" "$rustup_home" "$temporary_dir" \
    "$cargo_target" "$cargo_binary" "$rustc_binary" "$rustdoc_binary" \
    "$rustup_binary" "$developer_dir" <<'PY'
import os
from pathlib import Path
import stat
import sys

raw_directories = [Path(value) for value in sys.argv[1:6]]
raw_tools = [Path(value) for value in sys.argv[6:10]]
raw_developer = Path(sys.argv[10])
if any(not os.fspath(path) for path in [*raw_directories, *raw_tools, raw_developer]):
    raise SystemExit("Apple archive provenance environment is incomplete")


def canonical_directory(path: Path, label: str) -> Path:
    if not path.is_absolute():
        raise SystemExit(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
        metadata = resolved.lstat()
    except OSError as error:
        raise SystemExit(f"{label} is unavailable: {error}") from None
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit(f"{label} must resolve to a non-symbolic directory")
    return resolved


def canonical_tool(path: Path, label: str) -> Path:
    if not path.is_absolute():
        raise SystemExit(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
        metadata = resolved.lstat()
    except OSError as error:
        raise SystemExit(f"{label} is unavailable: {error}") from None
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or not os.access(resolved, os.X_OK)
    ):
        raise SystemExit(f"{label} must resolve to a non-symbolic executable")
    return resolved


directories = [
    canonical_directory(path, label)
    for path, label in zip(
        raw_directories,
        (
            "source-seal home",
            "source-seal Cargo home",
            "source-seal rustup home",
            "source-seal temporary directory",
            "source-seal Cargo target",
        ),
        strict=True,
    )
]
tools = [
    canonical_tool(path, label)
    for path, label in zip(
        raw_tools,
        (
            "source-seal Cargo",
            "source-seal rustc",
            "source-seal rustdoc",
            "source-seal rustup",
        ),
        strict=True,
    )
]
developer = canonical_directory(raw_developer, "Xcode developer directory")
print("\t".join(os.fspath(path) for path in [*directories, *tools, developer]))
PY
)" || exit 1
  IFS=$'\t' read -r \
    ARCHIVE_SEAL_HOME ARCHIVE_SEAL_CARGO_HOME ARCHIVE_SEAL_RUSTUP_HOME \
    ARCHIVE_SEAL_TMPDIR ARCHIVE_SEAL_CARGO_TARGET_DIR \
    ARCHIVE_SEAL_CARGO ARCHIVE_SEAL_RUSTC ARCHIVE_SEAL_RUSTDOC \
    ARCHIVE_SEAL_RUSTUP ARCHIVE_SEAL_DEVELOPER_DIR <<<"$authenticated"
}

package_apple() {
  local artifact_root
  local xcframework
  local bridge_manifest
  local apple_zip="$OUT_DIR/NoritoBridge-${VERSION}.xcframework.zip"
  local versioned_manifest="$OUT_DIR/NoritoBridge-${VERSION}.artifacts.json"

  if [[ ! -f "$APPLE_ARCHIVE_OWNER" || -L "$APPLE_ARCHIVE_OWNER" ]]; then
    echo "[mobile-sdk-package] ERROR: deterministic Apple archive owner is unavailable: $APPLE_ARCHIVE_OWNER" >&2
    exit 66
  fi
  require_dir "$APPLE_ARTIFACT_DIR" "Apple artifact directory"
  if [[ -L "$APPLE_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Apple artifact directory must not be a symbolic link: $APPLE_ARTIFACT_DIR" >&2
    exit 66
  fi
  artifact_root="$(cd "$APPLE_ARTIFACT_DIR" && pwd -P)"
  if [[ "$artifact_root" != "$APPLE_ARTIFACT_DIR" ]]; then
    echo "[mobile-sdk-package] ERROR: Apple artifact directory must be canonical: $APPLE_ARTIFACT_DIR" >&2
    exit 66
  fi
  xcframework="$artifact_root/NoritoBridge.xcframework"
  bridge_manifest="$artifact_root/NoritoBridge.artifacts.json"

  prepare_apple_archive_provenance_environment

  MOBILE_SDK_APPLE_ARTIFACT_DIR="$artifact_root" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --apple-only
  require_dir "$xcframework" "NoritoBridge XCFramework"
  require_file "$bridge_manifest" "NoritoBridge artifact manifest"

  env -i \
    HOME="$ARCHIVE_SEAL_HOME" \
    PATH="${PYTHON_BINARY%/*}:${ARCHIVE_SEAL_CARGO%/*}:${ARCHIVE_SEAL_RUSTC%/*}:${ARCHIVE_SEAL_RUSTDOC%/*}:/usr/bin:/bin" \
    TMPDIR="$ARCHIVE_SEAL_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-}" \
    NORITO_BRIDGE_OUTPUT_LOCK_FD="$APPLE_SOURCE_LOCK_FD" \
    NORITO_BRIDGE_SEAL_HOME="$ARCHIVE_SEAL_HOME" \
    NORITO_BRIDGE_SEAL_CARGO_HOME="$ARCHIVE_SEAL_CARGO_HOME" \
    NORITO_BRIDGE_SEAL_RUSTUP_HOME="$ARCHIVE_SEAL_RUSTUP_HOME" \
    NORITO_BRIDGE_SEAL_TMPDIR="$ARCHIVE_SEAL_TMPDIR" \
    NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$ARCHIVE_SEAL_CARGO_TARGET_DIR" \
    NORITO_BRIDGE_SEAL_CARGO="$ARCHIVE_SEAL_CARGO" \
    NORITO_BRIDGE_SEAL_RUSTC="$ARCHIVE_SEAL_RUSTC" \
    NORITO_BRIDGE_SEAL_RUSTDOC="$ARCHIVE_SEAL_RUSTDOC" \
    NORITO_BRIDGE_SEAL_RUSTUP="$ARCHIVE_SEAL_RUSTUP" \
    NORITO_BRIDGE_SEAL_DEVELOPER_DIR="$ARCHIVE_SEAL_DEVELOPER_DIR" \
    "$PYTHON_BINARY" -I -S -B "$APPLE_ARCHIVE_OWNER" \
      --xcframework "$xcframework" \
      --output "$apple_zip" \
      --scratch-dir "$OUT_PARENT"
  run_isolated_python - "$apple_zip" "$versioned_manifest" <<'PY'
import os
from pathlib import Path
import sys
import tempfile
import zipfile

archive_path = Path(sys.argv[1])
output = Path(sys.argv[2])
entry = "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
with zipfile.ZipFile(archive_path) as archive:
    payload = archive.read(entry)
descriptor, temporary_name = tempfile.mkstemp(
    prefix=f".{output.name}.",
    suffix=".tmp",
    dir=output.parent,
)
temporary = Path(temporary_name)
try:
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(payload)
        handle.flush()
        os.fchmod(handle.fileno(), 0o644)
        os.fsync(handle.fileno())
    os.replace(temporary, output)
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    directory_fd = os.open(output.parent, flags)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
except BaseException:
    temporary.unlink(missing_ok=True)
    raise
PY

  record_artifact "$apple_zip" "apple-xcframework"
  record_artifact "$versioned_manifest" "apple-manifest"
}

package_android() {
  local stage_container
  stage_container="$(mktemp -d "$OUT_PARENT/.iroha-mobile-sdk-android-${VERSION}.stage.XXXXXXXX")"
  local stage="$stage_container/iroha-mobile-sdk-android-${VERSION}"
  local stage_checksums="$stage/SHA256SUMS.txt"
  local android_zip="$OUT_DIR/iroha-mobile-sdk-android-${VERSION}.zip"
  local maven_repo="$ANDROID_MAVEN_REPO_DIR"
  local client_build_root="$ANDROID_KOTLIN_BUILD_ROOT/client-android"
  local client_aar="$client_build_root/outputs/aar/client-android-release.aar"
  local core_jar
  local native_mode
  local generated_native_root
  local generated_native_provenance
  local rel

  MOBILE_SDK_ANDROID_ARTIFACT_DIR="$ANDROID_ARTIFACT_DIR" \
    bash "$ROOT_DIR/scripts/check_mobile_sdk_artifacts.sh" --root "$ROOT_DIR" --android-only --require-built-android
  native_mode="$(resolve_android_native_mode "$client_aar")"
  generated_native_root="$client_build_root/generated/jniLibs/$native_mode"
  generated_native_provenance="$client_build_root/generated/nativeProvenance/$native_mode/iroha/native-build-provenance-v1.json"
  rm -rf "$stage" "$android_zip"
  mkdir -p "$stage"
  : > "$stage_checksums"

  core_jar="$(resolve_core_jar)"
  copy_android_artifact \
    "$core_jar" \
    "core-jvm/$(basename "$core_jar")" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$client_aar" \
    "client-android/client-android-release.aar" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$generated_native_root/arm64-v8a/libconnect_norito_bridge.so" \
    "native/arm64-v8a/libconnect_norito_bridge.so" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$generated_native_root/x86_64/libconnect_norito_bridge.so" \
    "native/x86_64/libconnect_norito_bridge.so" \
    "$stage" \
    "$stage_checksums"
  copy_android_artifact \
    "$generated_native_provenance" \
    "native/native-build-provenance-v1.json" \
    "$stage" \
    "$stage_checksums"

  if [[ -d "$maven_repo" ]]; then
    while IFS= read -r rel; do
      rel="${rel#./}"
      copy_android_artifact "$maven_repo/$rel" "maven/$rel" "$stage" "$stage_checksums"
    done < <(cd "$maven_repo" && find . -type f | sort)
  fi

  (cd "$stage_container" && zip -qr "$android_zip" "$(basename "$stage")")
  echo "[mobile-sdk-package] retained Android package stage: $stage_container" >&2
  record_artifact "$android_zip" "android-sdk"
}

publish_package_stage() {
  authenticate_package_lock
  run_isolated_python - \
    "$PACKAGE_STAGE_DIR" "$PACKAGE_STAGE_BASELINE" \
    "$FINAL_OUT_DIR" "$FINAL_OUT_BASELINE" \
    "$PACKAGE_LOCK" "$PACKAGE_LOCK_FD" <<'PY'
import ctypes
import errno
import fcntl
import os
from pathlib import Path
import stat
import sys

stage = Path(sys.argv[1])
expected_stage_identity = sys.argv[2]
final = Path(sys.argv[3])
expected_final_identity = sys.argv[4]
lock_path = Path(sys.argv[5])
raw_lock_descriptor = sys.argv[6]


def fail(message: str) -> None:
    raise SystemExit(f"[mobile-sdk-package] ERROR: {message}")


def directory_identity(path: Path) -> str:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return "absent"
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        fail(f"package output is not a non-symbolic directory: {path}")
    return (
        f"directory:{metadata.st_dev}:{metadata.st_ino}:"
        f"{metadata.st_mode}:{metadata.st_mtime_ns}"
    )


def directory_inode_identity(path: Path) -> str:
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        return "absent"
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        fail(f"package stage is not a non-symbolic directory: {path}")
    return f"directory:{metadata.st_dev}:{metadata.st_ino}:{metadata.st_mode}"


def open_directory(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    return os.open(path, flags)


def fsync_directory(path: Path) -> None:
    descriptor = open_directory(path)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def fsync_tree(root: Path) -> None:
    root_metadata = root.lstat()
    if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(root_metadata.st_mode):
        fail(f"package stage is not a non-symbolic directory: {root}")
    for current_raw, directory_names, file_names in os.walk(
        root,
        topdown=False,
        followlinks=False,
    ):
        current = Path(current_raw)
        for name in sorted(file_names, key=os.fsencode):
            path = current / name
            before = path.lstat()
            if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
                fail(f"unsupported package output entry: {path}")
            if before.st_nlink != 1:
                fail(f"hard-linked package output file is forbidden: {path}")
            flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
            descriptor = os.open(path, flags)
            try:
                opened = os.fstat(descriptor)
                if (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_size) != (
                    before.st_dev,
                    before.st_ino,
                    before.st_mode,
                    before.st_size,
                ):
                    fail(f"package output changed while being synchronized: {path}")
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            after = path.lstat()
            if (
                after.st_dev,
                after.st_ino,
                after.st_mode,
                after.st_size,
                after.st_mtime_ns,
            ) != (
                before.st_dev,
                before.st_ino,
                before.st_mode,
                before.st_size,
                before.st_mtime_ns,
            ):
                fail(f"package output changed while being synchronized: {path}")
        for name in sorted(directory_names, key=os.fsencode):
            path = current / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                fail(f"unsupported package output entry: {path}")
        fsync_directory(current)


def authenticate_lock() -> None:
    if not raw_lock_descriptor.isdecimal():
        fail("package publication lock descriptor is not canonical")
    descriptor = int(raw_lock_descriptor, 10)
    if descriptor < 3 or str(descriptor) != raw_lock_descriptor:
        fail("package publication lock descriptor is not canonical")
    try:
        descriptor_metadata = os.fstat(descriptor)
        path_metadata = lock_path.lstat()
    except OSError as error:
        fail(f"unable to authenticate package publication lock: {error}")
    if (
        not stat.S_ISREG(descriptor_metadata.st_mode)
        or not stat.S_ISREG(path_metadata.st_mode)
        or stat.S_ISLNK(path_metadata.st_mode)
        or (descriptor_metadata.st_dev, descriptor_metadata.st_ino)
        != (path_metadata.st_dev, path_metadata.st_ino)
        or descriptor_metadata.st_nlink != 1
        or path_metadata.st_nlink != 1
        or descriptor_metadata.st_uid != os.geteuid()
        or path_metadata.st_uid != os.geteuid()
    ):
        fail("package publication lock no longer matches its authenticated path")
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        fail(f"package publication lock is not held: {error}")


def rename_with_flag(source: Path, destination: Path, flag: int) -> None:
    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin":
        function = getattr(library, "renameatx_np", None)
        at_fdcwd = -2
    elif sys.platform.startswith("linux"):
        function = getattr(library, "renameat2", None)
        at_fdcwd = -100
    else:
        function = None
        at_fdcwd = 0
    if function is None:
        fail("host does not provide atomic package directory publication")
    function.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    function.restype = ctypes.c_int
    result = function(
        at_fdcwd,
        os.fsencode(source),
        at_fdcwd,
        os.fsencode(destination),
        flag,
    )
    if result != 0:
        error_number = ctypes.get_errno()
        raise OSError(error_number, os.strerror(error_number), os.fspath(destination))


if not stage.is_absolute() or not final.is_absolute() or stage.parent != final.parent:
    fail("package stage and destination must be absolute siblings")
if stage.parent.resolve(strict=True) != stage.parent:
    fail(f"package publication parent must be canonical: {stage.parent}")
expected_prefix = f".{final.name}.publish."
if not stage.name.startswith(expected_prefix):
    fail(f"package stage has an unexpected name: {stage}")
if directory_inode_identity(stage) != expected_stage_identity:
    fail(f"package stage identity changed before publication: {stage}")
if expected_final_identity != "absent":
    fail("first-release package destination must be absent")
if directory_identity(final) != expected_final_identity:
    fail("package destination changed while the package was being assembled")

authenticate_lock()
fsync_tree(stage)
if directory_inode_identity(stage) != expected_stage_identity:
    fail("package stage identity changed while it was being synchronized")
if directory_identity(final) != expected_final_identity:
    fail("package destination changed while the package was being synchronized")
authenticate_lock()

no_replace_flag = 0x4 if sys.platform == "darwin" else 0x1
rename_with_flag(stage, final, no_replace_flag)
if directory_inode_identity(final) != expected_stage_identity:
    fail("published package does not match the authenticated stage inode")
fsync_directory(final.parent)
PY
  PACKAGE_STAGE_DIR=""
  OUT_DIR="$FINAL_OUT_DIR"
  CHECKSUMS="$OUT_DIR/$(basename "$CHECKSUMS")"
  MANIFEST="$OUT_DIR/$(basename "$MANIFEST")"
}

: > "$CHECKSUMS"

if [[ "$PACKAGE_APPLE" == "1" ]]; then
  package_apple
fi

if [[ "$PACKAGE_ANDROID" == "1" ]]; then
  package_android
fi

write_manifest
publish_package_stage

echo "[mobile-sdk-package] wrote artifacts to $OUT_DIR"
echo "[mobile-sdk-package] wrote checksums to $CHECKSUMS"
echo "[mobile-sdk-package] wrote manifest to $MANIFEST"
