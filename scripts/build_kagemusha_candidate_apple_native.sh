#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
Usage:
  scripts/build_kagemusha_candidate_apple_native.sh \
    --candidate-record /absolute/path/candidate-v4.norito \
    --source-commit <40-lowercase-hex> \
    --source-tree-sha256 <64-lowercase-hex> \
    --reviewed-source-closure /absolute/path/reviewed-source-closure-v1.json \
    --reviewed-source-closure-sha256 <64-lowercase-hex> \
    --target-dir /absolute/private/cargo-target \
    --output-dir /absolute/private/output

Builds a marker-bearing, device-only iOS XCFramework from the exact reviewed
dirty source closure. This profile is solely for physical Taira-testnet
candidate evidence. It contains no simulator slice and must never ship.
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
CANDIDATE_RECORD=""
SOURCE_COMMIT=""
SOURCE_TREE_SHA256=""
REVIEWED_SOURCE_CLOSURE=""
REVIEWED_SOURCE_CLOSURE_SHA256=""
TARGET_DIR=""
OUTPUT_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --candidate-record)
      CANDIDATE_RECORD="${2:-}"
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
    --reviewed-source-closure)
      REVIEWED_SOURCE_CLOSURE="${2:-}"
      shift 2
      ;;
    --reviewed-source-closure-sha256)
      REVIEWED_SOURCE_CLOSURE_SHA256="${2:-}"
      shift 2
      ;;
    --target-dir)
      TARGET_DIR="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[kagemusha-apple-native] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

for path_value in "$CANDIDATE_RECORD" "$REVIEWED_SOURCE_CLOSURE" "$TARGET_DIR" "$OUTPUT_DIR"; do
  if [[ "$path_value" != /* ]]; then
    echo "[kagemusha-apple-native] ERROR: every input/output path must be absolute" >&2
    exit 64
  fi
done
if [[ ! "$SOURCE_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
  echo "[kagemusha-apple-native] ERROR: --source-commit must be lowercase git hex" >&2
  exit 64
fi
for digest in "$SOURCE_TREE_SHA256" "$REVIEWED_SOURCE_CLOSURE_SHA256"; do
  if [[ ! "$digest" =~ ^[0-9a-f]{64}$ || "$digest" == "$(printf '0%.0s' {1..64})" ]]; then
    echo "[kagemusha-apple-native] ERROR: source digests must be nonzero lowercase SHA-256" >&2
    exit 64
  fi
done
for input in "$CANDIDATE_RECORD" "$REVIEWED_SOURCE_CLOSURE"; do
  if [[ ! -f "$input" || -L "$input" ]]; then
    echo "[kagemusha-apple-native] ERROR: input must be a non-symlink regular file: $input" >&2
    exit 66
  fi
done
if [[ "$TARGET_DIR" == "$ROOT_DIR" || "$OUTPUT_DIR" == "$ROOT_DIR" ]] \
  || [[ "$ROOT_DIR" == "$TARGET_DIR"/* || "$ROOT_DIR" == "$OUTPUT_DIR"/* ]] \
  || [[ "$TARGET_DIR" == "$ROOT_DIR"/* || "$OUTPUT_DIR" == "$ROOT_DIR"/* ]]; then
  echo "[kagemusha-apple-native] ERROR: build/output roots must be disjoint from the repository" >&2
  exit 64
fi
if [[ -e "$OUTPUT_DIR" ]]; then
  echo "[kagemusha-apple-native] ERROR: refusing to overwrite output: $OUTPUT_DIR" >&2
  exit 73
fi
if [[ -e "$TARGET_DIR" ]]; then
  echo "[kagemusha-apple-native] ERROR: Cargo target directory must be new: $TARGET_DIR" >&2
  exit 73
fi
OUTPUT_PARENT="${OUTPUT_DIR%/*}"
TARGET_PARENT="${TARGET_DIR%/*}"
if [[ ! -d "$OUTPUT_PARENT" || -L "$OUTPUT_PARENT" \
  || ! -d "$TARGET_PARENT" || -L "$TARGET_PARENT" ]]; then
  echo "[kagemusha-apple-native] ERROR: build/output parents must be real existing directories" >&2
  exit 66
fi

PYTHON3_BINARY="$(command -v python3 2>/dev/null || true)"
GIT_BINARY="$(command -v git 2>/dev/null || true)"
CARGO_BINARY="$(command -v cargo 2>/dev/null || true)"
RUSTC_BINARY="$(command -v rustc 2>/dev/null || true)"
RUSTUP_BINARY="$(command -v rustup 2>/dev/null || true)"
XCRUN_BINARY="/usr/bin/xcrun"
XCODEBUILD_BINARY="/usr/bin/xcodebuild"
NM_BINARY="/usr/bin/nm"
for tool in \
  "$PYTHON3_BINARY" "$GIT_BINARY" "$CARGO_BINARY" "$RUSTC_BINARY" \
  "$RUSTUP_BINARY" "$XCRUN_BINARY" "$XCODEBUILD_BINARY" "$NM_BINARY"
do
  if [[ ! -x "$tool" ]]; then
    echo "[kagemusha-apple-native] ERROR: required executable is unavailable: $tool" >&2
    exit 69
  fi
done

SOURCE_SEAL="$ROOT_DIR/scripts/kagemusha_source_tree_seal.py"
HEADER_SOURCE="$ROOT_DIR/crates/connect_norito_bridge/include/connect_norito_bridge.h"
for input in "$SOURCE_SEAL" "$HEADER_SOURCE"; do
  if [[ ! -f "$input" || -L "$input" ]]; then
    echo "[kagemusha-apple-native] ERROR: build input is unavailable: $input" >&2
    exit 66
  fi
done

PINNED_TOOLCHAIN="$(
  sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' \
    "$ROOT_DIR/rust-toolchain.toml"
)"
if [[ "$PINNED_TOOLCHAIN" != "1.93.1" ]]; then
  echo "[kagemusha-apple-native] ERROR: exact Rust 1.93.1 is required" >&2
  exit 69
fi
if ! "$RUSTUP_BINARY" target list --toolchain "$PINNED_TOOLCHAIN" --installed \
  | /usr/bin/grep -Fxq aarch64-apple-ios
then
  echo "[kagemusha-apple-native] ERROR: aarch64-apple-ios target is not installed" >&2
  exit 69
fi

source_identity() {
  "$PYTHON3_BINARY" -I "$SOURCE_SEAL" identity \
    --root "$ROOT_DIR" \
    --reviewed-source-closure "$REVIEWED_SOURCE_CLOSURE" \
    --reviewed-source-closure-sha256 "$REVIEWED_SOURCE_CLOSURE_SHA256"
}

IDENTITY_BEFORE="$(source_identity)"
"$PYTHON3_BINARY" -I - "$IDENTITY_BEFORE" "$SOURCE_COMMIT" "$SOURCE_TREE_SHA256" \
  "$REVIEWED_SOURCE_CLOSURE_SHA256" <<'PY'
import json
import sys

value = json.loads(sys.argv[1])
expected = {
    "source_commit": sys.argv[2],
    "source_tree_sha256": sys.argv[3],
    "reviewed_source_closure_descriptor_sha256": sys.argv[4],
    "source_repo_dirty": True,
}
for key, wanted in expected.items():
    if value.get(key) != wanted:
        raise SystemExit(f"reviewed source identity mismatch: {key}")
PY

if [[ "$("$GIT_BINARY" -C "$ROOT_DIR" rev-parse HEAD)" != "$SOURCE_COMMIT" ]]; then
  echo "[kagemusha-apple-native] ERROR: checkout HEAD differs from reviewed commit" >&2
  exit 65
fi

BUILD_SESSION="$(mktemp -d "${TMPDIR:-/tmp}/kagemusha-apple-native.XXXXXX")"
cleanup() {
  rm -rf -- "$BUILD_SESSION"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -- "$TARGET_DIR"
chmod 0700 "$TARGET_DIR"
SDKROOT="$("$XCRUN_BINARY" --sdk iphoneos --show-sdk-path)"
SDK_VERSION="$("$XCRUN_BINARY" --sdk iphoneos --show-sdk-version)"
XCODE_VERSION="$("$XCODEBUILD_BINARY" -version)"
CARGO_VERSION="$("$CARGO_BINARY" --version --verbose)"
RUSTC_VERSION="$("$RUSTC_BINARY" --version --verbose)"

echo "[kagemusha-apple-native] building physical-iOS candidate bridge" >&2
(
  cd "$ROOT_DIR"
  env \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_OFFLINE=true \
    CARGO_TARGET_DIR="$TARGET_DIR" \
    IPHONEOS_DEPLOYMENT_TARGET=15.0 \
    NORITO_SKIP_BINDINGS_SYNC=1 \
    SDKROOT="$SDKROOT" \
    "$CARGO_BINARY" +"$PINNED_TOOLCHAIN" build \
      --locked \
      --offline \
      --release \
      --jobs 1 \
      --target aarch64-apple-ios \
      -p connect_norito_bridge \
      --no-default-features \
      --features kagemusha-candidate-evidence-lab
)

BUILT_LIBRARY="$TARGET_DIR/aarch64-apple-ios/release/libconnect_norito_bridge.a"
if [[ ! -f "$BUILT_LIBRARY" || -L "$BUILT_LIBRARY" ]]; then
  echo "[kagemusha-apple-native] ERROR: Cargo did not produce the device static library" >&2
  exit 1
fi
SYMBOLS="$BUILD_SESSION/symbols.txt"
"$NM_BINARY" -gU "$BUILT_LIBRARY" >"$SYMBOLS"
for symbol in \
  _connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1 \
  _connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1 \
  _CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2
do
  if ! /usr/bin/grep -Fq "$symbol" "$SYMBOLS"; then
    echo "[kagemusha-apple-native] ERROR: required candidate symbol missing: $symbol" >&2
    exit 1
  fi
done

IDENTITY_AFTER="$(source_identity)"
if [[ "$IDENTITY_AFTER" != "$IDENTITY_BEFORE" ]]; then
  echo "[kagemusha-apple-native] ERROR: reviewed source closure changed during build" >&2
  exit 1
fi

XCFRAMEWORK="$OUTPUT_DIR/NoritoBridgeCandidateLab.xcframework"
SLICE="$XCFRAMEWORK/ios-arm64"
HEADERS="$SLICE/Headers"
mkdir -- "$OUTPUT_DIR"
mkdir -p -- "$HEADERS"
chmod 0700 "$OUTPUT_DIR" "$XCFRAMEWORK" "$SLICE" "$HEADERS"
cp -- "$BUILT_LIBRARY" "$SLICE/libNoritoBridgeCandidateLab.a"
cp -- "$HEADER_SOURCE" "$HEADERS/connect_norito_bridge_base.h"
"$PYTHON3_BINARY" -I - "$HEADERS/connect_norito_bridge.h" <<'PY'
from pathlib import Path
import sys

Path(sys.argv[1]).write_text(
    "#ifndef CONNECT_NORITO_BRIDGE_CANDIDATE_LAB_WRAPPER_H\\n"
    "#define CONNECT_NORITO_BRIDGE_CANDIDATE_LAB_WRAPPER_H\\n"
    "#define CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB 1\\n"
    '#include "connect_norito_bridge_base.h"\\n'
    "#endif\\n",
    encoding="ascii",
)
PY
"$PYTHON3_BINARY" -I - "$HEADERS/module.modulemap" <<'PY'
from pathlib import Path
import sys

Path(sys.argv[1]).write_text(
    'module NoritoBridgeCandidateLab {\\n'
    '  header "connect_norito_bridge.h"\\n'
    '  export *\\n'
    '}\\n',
    encoding="ascii",
)
PY
"$PYTHON3_BINARY" -I - "$XCFRAMEWORK/Info.plist" <<'PY'
from pathlib import Path
import plistlib
import sys

value = {
    "AvailableLibraries": [{
        "LibraryIdentifier": "ios-arm64",
        "LibraryPath": "libNoritoBridgeCandidateLab.a",
        "HeadersPath": "Headers",
        "SupportedArchitectures": ["arm64"],
        "SupportedPlatform": "ios",
    }],
    "CFBundlePackageType": "XFWK",
    "XCFrameworkFormatVersion": "1.0",
}
with Path(sys.argv[1]).open("wb") as handle:
    plistlib.dump(value, handle, fmt=plistlib.FMT_XML, sort_keys=True)
PY
printf '%s\n' "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2" \
  >"$XCFRAMEWORK/.kagemusha-candidate-evidence-lab-do-not-ship-v2"

"$PYTHON3_BINARY" -I - \
  "$OUTPUT_DIR/NoritoBridgeCandidateLab.artifacts.json" \
  "$XCFRAMEWORK" "$CANDIDATE_RECORD" "$SOURCE_COMMIT" "$SOURCE_TREE_SHA256" \
  "$REVIEWED_SOURCE_CLOSURE_SHA256" "$SDK_VERSION" "$XCODE_VERSION" \
  "$CARGO_VERSION" "$RUSTC_VERSION" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()

output = Path(sys.argv[1])
framework = Path(sys.argv[2])
library = framework / "ios-arm64/libNoritoBridgeCandidateLab.a"
base_header = framework / "ios-arm64/Headers/connect_norito_bridge_base.h"
wrapper_header = framework / "ios-arm64/Headers/connect_norito_bridge.h"
modulemap = framework / "ios-arm64/Headers/module.modulemap"
info = framework / "Info.plist"
marker = framework / ".kagemusha-candidate-evidence-lab-do-not-ship-v2"
candidate = Path(sys.argv[3])
manifest = {
    "schema": "iroha.kagemusha.apple_candidate_native_build.v1",
    "version": 1,
    "profile": "physical-ios-candidate-evidence-lab",
    "do_not_ship_marker": "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
    "candidate_feature_enabled": True,
    "production_capability_enabled": False,
    "bridge_abi_version": 21,
    "target_triple": "aarch64-apple-ios",
    "architectures": ["arm64"],
    "simulator_slice_present": False,
    "minimum_ios_version": "15.0",
    "candidate_record_sha256": digest(candidate),
    "source_commit": sys.argv[4],
    "source_tree_sha256": sys.argv[5],
    "source_repo_dirty": True,
    "reviewed_source_closure_descriptor_sha256": sys.argv[6],
    "iphoneos_sdk_version": sys.argv[7],
    "xcode_version": sys.argv[8],
    "cargo_version_verbose": sys.argv[9],
    "rustc_version_verbose": sys.argv[10],
    "required_symbols": [
        "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1",
        "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1",
        "CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2",
    ],
    "files": {
        "NoritoBridgeCandidateLab.xcframework/Info.plist": digest(info),
        "NoritoBridgeCandidateLab.xcframework/.kagemusha-candidate-evidence-lab-do-not-ship-v2":
            digest(marker),
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/libNoritoBridgeCandidateLab.a":
            digest(library),
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/connect_norito_bridge.h":
            digest(wrapper_header),
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/connect_norito_bridge_base.h":
            digest(base_header),
        "NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/module.modulemap":
            digest(modulemap),
    },
}
output.write_text(
    json.dumps(manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n",
    encoding="ascii",
)
PY

find "$OUTPUT_DIR" -type d -exec chmod 0700 {} +
find "$OUTPUT_DIR" -type f -exec chmod 0400 {} +
echo "[kagemusha-apple-native] candidate-only XCFramework: $XCFRAMEWORK"
