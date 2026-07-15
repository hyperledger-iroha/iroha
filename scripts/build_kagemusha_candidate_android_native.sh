#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
Usage:
  scripts/build_kagemusha_candidate_android_native.sh \
    --candidate-sha256 <hex> \
    --stage-sha256 <hex> \
    --source-commit <git-hex> \
    --source-tree-sha256 <hex>

Builds the non-shipping ARM64 Android connect_norito_bridge directly from the
current checkout with `--features kagemusha-candidate-evidence-lab`. The build
is source-sealed before and after cargo-ndk and may write only beneath:
  artifacts/kagemusha-candidate-evidence/<candidate-sha256>/<stage-sha256>
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CANDIDATE_SHA256=""
STAGE_SHA256=""
SOURCE_COMMIT=""
SOURCE_TREE_SHA256=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --candidate-sha256)
      CANDIDATE_SHA256="${2:-}"
      shift 2
      ;;
    --stage-sha256)
      STAGE_SHA256="${2:-}"
      shift 2
      ;;
    --source-commit)
      SOURCE_COMMIT="${2:-}"
      shift 2
      ;;
    --source-tree-sha256)
      SOURCE_TREE_SHA256="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[kagemusha-candidate-native] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

if [[ ! "$CANDIDATE_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "[kagemusha-candidate-native] ERROR: --candidate-sha256 must be lowercase SHA-256" >&2
  exit 64
fi
if [[ ! "$STAGE_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "[kagemusha-candidate-native] ERROR: --stage-sha256 must be lowercase SHA-256" >&2
  exit 64
fi
if [[ ! "$SOURCE_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
  echo "[kagemusha-candidate-native] ERROR: --source-commit must be lowercase git hex" >&2
  exit 64
fi
if [[ ! "$SOURCE_TREE_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "[kagemusha-candidate-native] ERROR: --source-tree-sha256 must be lowercase SHA-256" >&2
  exit 64
fi

CARGO_BINARY="$(command -v cargo 2>/dev/null || true)"
RUSTC_BINARY="$(command -v rustc 2>/dev/null || true)"
CARGO_NDK_BINARY="$(command -v cargo-ndk 2>/dev/null || true)"
PYTHON3_BINARY="$(command -v python3 2>/dev/null || true)"
GIT_BINARY="$(command -v git 2>/dev/null || true)"
SHASUM_BINARY="$(command -v shasum 2>/dev/null || true)"
[[ -x "$CARGO_BINARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: cargo is required" >&2
  exit 69
}
[[ -x "$RUSTC_BINARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: rustc is required" >&2
  exit 69
}
[[ -x "$CARGO_NDK_BINARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: cargo-ndk is required" >&2
  exit 69
}
[[ -x "$PYTHON3_BINARY" && -x "$GIT_BINARY" && -x "$SHASUM_BINARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: python3, git, and shasum are required" >&2
  exit 69
}
if ! "$CARGO_NDK_BINARY" --version >/dev/null 2>&1; then
  echo "[kagemusha-candidate-native] ERROR: cargo-ndk is not executable" >&2
  exit 69
fi
if [[ -z "${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}" ]]; then
  echo "[kagemusha-candidate-native] ERROR: ANDROID_NDK_HOME or ANDROID_NDK_ROOT is required" >&2
  exit 69
fi

EVIDENCE_ROOT="$ROOT_DIR/artifacts/kagemusha-candidate-evidence/$CANDIDATE_SHA256/$STAGE_SHA256"
STAGE_MANIFEST="$EVIDENCE_ROOT/candidate-stage-manifest-v1.json"
CANDIDATE_RECORD="$EVIDENCE_ROOT/evidence/candidate/candidate-v4.norito"
NATIVE_DIRECTORY="$EVIDENCE_ROOT/evidence/candidate/lib/arm64-v8a"
NATIVE_LIBRARY="$NATIVE_DIRECTORY/libconnect_norito_bridge.so"
BUILD_ROOT="$EVIDENCE_ROOT/build/kagemusha-candidate-android-native"
SOURCE_SEAL="$ROOT_DIR/scripts/kagemusha_source_tree_seal.py"

[[ -f "$STAGE_MANIFEST" && ! -L "$STAGE_MANIFEST" ]] || {
  echo "[kagemusha-candidate-native] ERROR: missing regular stage manifest: $STAGE_MANIFEST" >&2
  exit 66
}
[[ -f "$CANDIDATE_RECORD" && ! -L "$CANDIDATE_RECORD" ]] || {
  echo "[kagemusha-candidate-native] ERROR: missing regular candidate record: $CANDIDATE_RECORD" >&2
  exit 66
}

"$PYTHON3_BINARY" - "$STAGE_MANIFEST" "$STAGE_SHA256" "$CANDIDATE_RECORD" "$CANDIDATE_SHA256" <<'PY'
from pathlib import Path
import hashlib
import sys

for path_text, expected, label in (
    (sys.argv[1], sys.argv[2], "candidate stage manifest"),
    (sys.argv[3], sys.argv[4], "candidate record"),
):
    path = Path(path_text)
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
    if digest.hexdigest() != expected:
        raise SystemExit(f"{label} digest does not match its evidence directory")
PY

"$PYTHON3_BINARY" - "$ROOT_DIR" "$EVIDENCE_ROOT" "$CANDIDATE_SHA256" "$STAGE_SHA256" \
  "$SOURCE_COMMIT" "$SOURCE_TREE_SHA256" <<'PY'
from pathlib import Path
import sys

sys.path.insert(0, str(Path(sys.argv[1]) / "scripts"))
from check_android_device_lab_slot import validate_kagemusha_candidate_stage_manifest_v1

validate_kagemusha_candidate_stage_manifest_v1(
    Path(sys.argv[2]),
    candidate_sha256=sys.argv[3],
    stage_sha256=sys.argv[4],
    source_commit=sys.argv[5],
    source_tree_sha256=sys.argv[6],
)
PY

"$PYTHON3_BINARY" - "$STAGE_MANIFEST" "$CARGO_BINARY" "$RUSTC_BINARY" <<'PY'
from pathlib import Path
import hashlib
import json
import subprocess
import sys

manifest = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
validator = manifest["validator"]
for tool, digest_key, version_key in (
    (Path(sys.argv[2]).resolve(), "cargo_binary_sha256", "cargo_version_verbose"),
    (Path(sys.argv[3]).resolve(), "rustc_binary_sha256", "rustc_version_verbose"),
):
    if not tool.is_file():
        raise SystemExit(f"validator tool is not regular: {tool}")
    if hashlib.sha256(tool.read_bytes()).hexdigest() != validator[digest_key]:
        raise SystemExit(f"current {tool.name} binary does not match the stage validator")
    version = subprocess.run(
        [str(tool), "--version", "--verbose"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    if version != validator[version_key]:
        raise SystemExit(f"current {tool.name} version does not match the stage validator")
PY

source_commit() {
  "$GIT_BINARY" -C "$ROOT_DIR" rev-parse HEAD
}

source_fingerprint() {
  "$PYTHON3_BINARY" "$SOURCE_SEAL" fingerprint --root "$ROOT_DIR"
}

COMMIT_BEFORE="$(source_commit)"
FINGERPRINT_BEFORE="$(source_fingerprint)"
if [[ "$COMMIT_BEFORE" != "$SOURCE_COMMIT" ]]; then
  echo "[kagemusha-candidate-native] ERROR: checkout HEAD does not match candidate source_commit" >&2
  exit 65
fi
if [[ "$FINGERPRINT_BEFORE" != "$SOURCE_TREE_SHA256" ]]; then
  echo "[kagemusha-candidate-native] ERROR: checkout full-source-tree seal does not match candidate source_tree_sha256" >&2
  exit 65
fi

mkdir -p "$BUILD_ROOT" "$NATIVE_DIRECTORY"
chmod 0700 "$BUILD_ROOT" "$NATIVE_DIRECTORY"
BUILD_SESSION="$(mktemp -d "$BUILD_ROOT/session.XXXXXX")"
chmod 0700 "$BUILD_SESSION"
STAGE_DIR="$BUILD_SESSION/output"
CARGO_TARGET_DIR="$BUILD_SESSION/cargo-target"
PRIVATE_CARGO_HOME="$BUILD_SESSION/cargo-home"
mkdir -p "$STAGE_DIR" "$CARGO_TARGET_DIR" "$PRIVATE_CARGO_HOME"
chmod 0700 "$STAGE_DIR" "$CARGO_TARGET_DIR" "$PRIVATE_CARGO_HOME"
SOURCE_CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
for cache_name in registry git; do
  if [[ -e "$SOURCE_CARGO_HOME/$cache_name" ]]; then
    ln -s "$SOURCE_CARGO_HOME/$cache_name" "$PRIVATE_CARGO_HOME/$cache_name"
  fi
done
cleanup() {
  rm -rf "$BUILD_SESSION"
}
trap cleanup EXIT

[[ -f "$CARGO_NDK_BINARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: cargo-ndk executable is unavailable" >&2
  exit 69
}
CARGO_NDK_SHA256_BEFORE="$("$SHASUM_BINARY" -a 256 "$CARGO_NDK_BINARY")"
CARGO_NDK_SHA256_BEFORE="${CARGO_NDK_SHA256_BEFORE%% *}"
CARGO_NDK_VERSION_BEFORE="$("$CARGO_NDK_BINARY" --version)"
NDK_ROOT="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
NDK_SOURCE_PROPERTIES="$NDK_ROOT/source.properties"
[[ -f "$NDK_SOURCE_PROPERTIES" && ! -L "$NDK_SOURCE_PROPERTIES" ]] || {
  echo "[kagemusha-candidate-native] ERROR: NDK source.properties is unavailable" >&2
  exit 69
}
NDK_SOURCE_PROPERTIES_SHA256_BEFORE="$("$SHASUM_BINARY" -a 256 "$NDK_SOURCE_PROPERTIES")"
NDK_SOURCE_PROPERTIES_SHA256_BEFORE="${NDK_SOURCE_PROPERTIES_SHA256_BEFORE%% *}"

echo "[kagemusha-candidate-native] building exact current-source ARM64 lab bridge" >&2
(
  cd "$ROOT_DIR"
  env -i \
    HOME="$HOME" \
    PATH="${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${CARGO_NDK_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="${TMPDIR:-/tmp}" \
    LANG="${LANG:-C.UTF-8}" \
    RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" \
    CARGO_HOME="$PRIVATE_CARGO_HOME" \
    CARGO="$CARGO_BINARY" \
    RUSTC="$RUSTC_BINARY" \
    CARGO_NET_OFFLINE=true \
    ANDROID_NDK_HOME="$NDK_ROOT" \
    ANDROID_NDK_ROOT="$NDK_ROOT" \
    CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    NORITO_SKIP_BINDINGS_SYNC=1 \
    "$CARGO_NDK_BINARY" -t arm64-v8a -o "$STAGE_DIR" \
      build --locked --offline --release -p connect_norito_bridge --no-default-features \
      --features kagemusha-candidate-evidence-lab
)

CARGO_NDK_SHA256_AFTER="$("$SHASUM_BINARY" -a 256 "$CARGO_NDK_BINARY")"
CARGO_NDK_SHA256_AFTER="${CARGO_NDK_SHA256_AFTER%% *}"
[[ "$CARGO_NDK_SHA256_AFTER" == "$CARGO_NDK_SHA256_BEFORE" ]] || {
  echo "[kagemusha-candidate-native] ERROR: cargo-ndk changed during the build" >&2
  exit 1
}
[[ "$("$CARGO_NDK_BINARY" --version)" == "$CARGO_NDK_VERSION_BEFORE" ]] || {
  echo "[kagemusha-candidate-native] ERROR: cargo-ndk version changed during the build" >&2
  exit 1
}
NDK_SOURCE_PROPERTIES_SHA256_AFTER="$("$SHASUM_BINARY" -a 256 "$NDK_SOURCE_PROPERTIES")"
NDK_SOURCE_PROPERTIES_SHA256_AFTER="${NDK_SOURCE_PROPERTIES_SHA256_AFTER%% *}"
[[ "$NDK_SOURCE_PROPERTIES_SHA256_AFTER" == \
    "$NDK_SOURCE_PROPERTIES_SHA256_BEFORE" ]] || {
  echo "[kagemusha-candidate-native] ERROR: Android NDK identity changed during the build" >&2
  exit 1
}

BUILT_LIBRARY="$STAGE_DIR/arm64-v8a/libconnect_norito_bridge.so"
[[ -f "$BUILT_LIBRARY" && ! -L "$BUILT_LIBRARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: cargo-ndk did not produce the ARM64 lab bridge" >&2
  exit 1
}

"$PYTHON3_BINARY" - "$BUILT_LIBRARY" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
required = (
    b"KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
    b"Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_",
    b"connect_norito_kagemusha_recursive_spend_candidate_lab_",
)
with path.open("rb") as handle:
    header = handle.read(64)
if len(header) < 20 or header[:4] != b"\x7fELF" or header[4] != 2 or header[5] != 1:
    raise SystemExit("candidate lab bridge is not a 64-bit little-endian ELF")
if int.from_bytes(header[18:20], "little") != 183:
    raise SystemExit("candidate lab bridge is not AArch64")
payload = path.read_bytes()
missing = [needle.decode("ascii") for needle in required if needle not in payload]
if missing:
    raise SystemExit(f"candidate lab bridge is missing feature-only marker/symbols: {missing}")
PY

COMMIT_AFTER="$(source_commit)"
FINGERPRINT_AFTER="$(source_fingerprint)"
if [[ "$COMMIT_AFTER" != "$COMMIT_BEFORE" \
    || "$FINGERPRINT_AFTER" != "$FINGERPRINT_BEFORE" ]]; then
  echo "[kagemusha-candidate-native] ERROR: source changed during the native build" >&2
  exit 1
fi

chmod 0555 "$BUILT_LIBRARY"
BUILT_LIBRARY_SHA256="$("$SHASUM_BINARY" -a 256 "$BUILT_LIBRARY")"
BUILT_LIBRARY_SHA256="${BUILT_LIBRARY_SHA256%% *}"
if [[ -e "$NATIVE_LIBRARY" ]]; then
  if [[ ! -f "$NATIVE_LIBRARY" || -L "$NATIVE_LIBRARY" ]] \
      || ! cmp -s "$BUILT_LIBRARY" "$NATIVE_LIBRARY"; then
    echo "[kagemusha-candidate-native] ERROR: refusing to replace a different candidate lab bridge" >&2
    exit 1
  fi
else
  "$PYTHON3_BINARY" - "$BUILT_LIBRARY" "$NATIVE_LIBRARY" <<'PY'
import ctypes
import errno
import os
from pathlib import Path
import shutil
import sys

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
temporary = destination.with_name(f".{destination.name}.{os.getpid()}.publish")

def rename_no_replace(source_path: Path, destination_path: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    source_bytes = os.fsencode(source_path)
    destination_bytes = os.fsencode(destination_path)
    if sys.platform == "darwin" and hasattr(libc, "renamex_np"):
        result = libc.renamex_np(source_bytes, destination_bytes, 0x00000004)
    elif hasattr(libc, "renameat2"):
        result = libc.renameat2(-100, source_bytes, -100, destination_bytes, 1)
    else:
        raise SystemExit("atomic no-replace publication is unsupported on this host")
    if result != 0:
        error = ctypes.get_errno()
        if error == errno.EEXIST:
            raise SystemExit("refusing a raced candidate lab bridge destination")
        raise OSError(error, os.strerror(error), destination_path)

try:
    with source.open("rb") as input_handle, temporary.open("xb") as output_handle:
        shutil.copyfileobj(input_handle, output_handle, length=1024 * 1024)
        output_handle.flush()
        os.fsync(output_handle.fileno())
    os.chmod(temporary, 0o555)
    rename_no_replace(temporary, destination)
    directory_fd = os.open(destination.parent, os.O_RDONLY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)
finally:
    try:
        temporary.unlink()
    except FileNotFoundError:
        pass
PY
fi
[[ -f "$NATIVE_LIBRARY" && ! -L "$NATIVE_LIBRARY" ]] || {
  echo "[kagemusha-candidate-native] ERROR: published lab bridge is not regular" >&2
  exit 1
}
"$PYTHON3_BINARY" - "$NATIVE_LIBRARY" <<'PY'
from pathlib import Path
import stat
import sys

path = Path(sys.argv[1])
metadata = path.lstat()
if (
    not stat.S_ISREG(metadata.st_mode)
    or metadata.st_nlink != 1
    or stat.S_IMODE(metadata.st_mode) != 0o555
):
    raise SystemExit("published lab bridge must be one mode-0555 non-hard-linked regular file")
PY
NATIVE_LIBRARY_SHA256_AFTER="$("$SHASUM_BINARY" -a 256 "$NATIVE_LIBRARY")"
NATIVE_LIBRARY_SHA256_AFTER="${NATIVE_LIBRARY_SHA256_AFTER%% *}"
[[ "$NATIVE_LIBRARY_SHA256_AFTER" == "$BUILT_LIBRARY_SHA256" ]] || {
  echo "[kagemusha-candidate-native] ERROR: published lab bridge digest changed" >&2
  exit 1
}

echo "[kagemusha-candidate-native] source-sealed lab bridge ready: $NATIVE_LIBRARY"
