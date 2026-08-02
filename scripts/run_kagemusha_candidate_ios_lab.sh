#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
Usage:
  scripts/run_kagemusha_candidate_ios_lab.sh \
    --device-id <paired-CoreDevice-identifier> \
    --development-team <Apple-team-id> \
    --candidate-record /absolute/path/candidate-v4.norito \
    --candidate-manifest /absolute/path/manifest-v4.norito \
    --topup-finality-roster /absolute/path/roster-v4.norito \
    --artifact-root /absolute/path/eight-artifacts \
    --scenario-root /absolute/path/thirty-three-scenario-files \
    --reviewed-source-closure /absolute/path/reviewed-source-closure-v1.json \
    --native-build-root /absolute/path/apple-native-build-output \
    --evidence-root /absolute/new/path

The device identifier is runtime-only. Raw CoreDevice, provisioning-profile,
XCResult, and xcodebuild records are deleted. Retained evidence contains only
hashed device identifiers and owner-private candidate/run artifacts.
USAGE
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
LAB_SOURCE="$ROOT_DIR/IrohaSwift/KagemushaCandidateEvidenceLab"
INPUT_DIRECTORY_NAME="kagemusha-candidate-input"
OUTPUT_DIRECTORY_NAME="kagemusha-candidate-output"
DEVICE_ID=""
DEVELOPMENT_TEAM=""
CANDIDATE_RECORD=""
CANDIDATE_MANIFEST=""
ROSTER=""
ARTIFACT_ROOT=""
SCENARIO_ROOT=""
REVIEWED_SOURCE_CLOSURE=""
NATIVE_BUILD_ROOT=""
EVIDENCE_ROOT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --device-id)
      DEVICE_ID="${2:-}"
      shift 2
      ;;
    --development-team)
      DEVELOPMENT_TEAM="${2:-}"
      shift 2
      ;;
    --candidate-record)
      CANDIDATE_RECORD="${2:-}"
      shift 2
      ;;
    --candidate-manifest)
      CANDIDATE_MANIFEST="${2:-}"
      shift 2
      ;;
    --topup-finality-roster)
      ROSTER="${2:-}"
      shift 2
      ;;
    --artifact-root)
      ARTIFACT_ROOT="${2:-}"
      shift 2
      ;;
    --scenario-root)
      SCENARIO_ROOT="${2:-}"
      shift 2
      ;;
    --reviewed-source-closure)
      REVIEWED_SOURCE_CLOSURE="${2:-}"
      shift 2
      ;;
    --native-build-root)
      NATIVE_BUILD_ROOT="${2:-}"
      shift 2
      ;;
    --evidence-root)
      EVIDENCE_ROOT="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[kagemusha-ios-lab] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

if [[ -z "$DEVICE_ID" || -z "$DEVELOPMENT_TEAM" ]]; then
  echo "[kagemusha-ios-lab] ERROR: device and development team are required" >&2
  exit 64
fi
if [[ ! "$DEVELOPMENT_TEAM" =~ ^[A-Z0-9]{10}$ ]]; then
  echo "[kagemusha-ios-lab] ERROR: development team must be an exact 10-character ID" >&2
  exit 64
fi
for input in \
  "$CANDIDATE_RECORD" "$CANDIDATE_MANIFEST" "$ROSTER" \
  "$ARTIFACT_ROOT" "$SCENARIO_ROOT" "$REVIEWED_SOURCE_CLOSURE" \
  "$NATIVE_BUILD_ROOT" "$EVIDENCE_ROOT"
do
  if [[ "$input" != /* ]]; then
    echo "[kagemusha-ios-lab] ERROR: every evidence path must be absolute" >&2
    exit 64
  fi
done
if [[ -e "$EVIDENCE_ROOT" ]]; then
  echo "[kagemusha-ios-lab] ERROR: refusing to overwrite evidence root" >&2
  exit 73
fi
EVIDENCE_PARENT="${EVIDENCE_ROOT%/*}"
if [[ ! -d "$EVIDENCE_PARENT" || -L "$EVIDENCE_PARENT" ]]; then
  echo "[kagemusha-ios-lab] ERROR: evidence parent must be a real existing directory" >&2
  exit 66
fi
if [[ "$EVIDENCE_ROOT" == "$ROOT_DIR" || "$EVIDENCE_ROOT" == "$ROOT_DIR"/* ]] \
  || [[ "$ROOT_DIR" == "$EVIDENCE_ROOT"/* ]]; then
  echo "[kagemusha-ios-lab] ERROR: evidence root must be disjoint from the repository" >&2
  exit 64
fi

PYTHON3_BINARY="$(command -v python3 2>/dev/null || true)"
XCODEGEN_BINARY="/opt/homebrew/bin/xcodegen"
XCODEBUILD_BINARY="/usr/bin/xcodebuild"
XCRUN_BINARY="/usr/bin/xcrun"
CODESIGN_BINARY="/usr/bin/codesign"
LIPO_BINARY="/usr/bin/lipo"
for tool in \
  "$PYTHON3_BINARY" "$XCODEGEN_BINARY" "$XCODEBUILD_BINARY" "$XCRUN_BINARY" \
  "$CODESIGN_BINARY" "$LIPO_BINARY"
do
  if [[ ! -x "$tool" ]]; then
    echo "[kagemusha-ios-lab] ERROR: required executable is unavailable: $tool" >&2
    exit 69
  fi
done
if [[ "$("$XCODEGEN_BINARY" --version)" != "Version: 2.46.0" ]]; then
  echo "[kagemusha-ios-lab] ERROR: exact XcodeGen 2.46.0 is required" >&2
  exit 69
fi
if [[ "$("$XCODEBUILD_BINARY" -version | /usr/bin/head -1)" != "Xcode 26.6" ]]; then
  echo "[kagemusha-ios-lab] ERROR: exact Xcode 26.6 is required" >&2
  exit 69
fi

NATIVE_MANIFEST="$NATIVE_BUILD_ROOT/NoritoBridgeCandidateLab.artifacts.json"
SOURCE_XCFRAMEWORK="$NATIVE_BUILD_ROOT/NoritoBridgeCandidateLab.xcframework"
SOURCE_NATIVE_LIBRARY="$SOURCE_XCFRAMEWORK/ios-arm64/libNoritoBridgeCandidateLab.a"
for input in \
  "$CANDIDATE_RECORD" "$CANDIDATE_MANIFEST" "$ROSTER" "$REVIEWED_SOURCE_CLOSURE" \
  "$NATIVE_MANIFEST" "$SOURCE_NATIVE_LIBRARY" "$SOURCE_XCFRAMEWORK/Info.plist" \
  "$SOURCE_XCFRAMEWORK/.kagemusha-candidate-evidence-lab-do-not-ship-v2" \
  "$SOURCE_XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge.h" \
  "$SOURCE_XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge_base.h" \
  "$SOURCE_XCFRAMEWORK/ios-arm64/Headers/module.modulemap"
do
  if [[ ! -f "$input" || -L "$input" ]]; then
    echo "[kagemusha-ios-lab] ERROR: required input is not a non-symlink regular file" >&2
    exit 66
  fi
done
for input in "$ARTIFACT_ROOT" "$SCENARIO_ROOT" "$SOURCE_XCFRAMEWORK"; do
  if [[ ! -d "$input" || -L "$input" ]]; then
    echo "[kagemusha-ios-lab] ERROR: required input is not a real directory" >&2
    exit 66
  fi
done

verify_native_framework() {
  local framework="$1"
  "$PYTHON3_BINARY" -I - "$NATIVE_MANIFEST" "$framework" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import stat
import sys

manifest_path = Path(sys.argv[1])
framework = Path(sys.argv[2])
expected = (
    "Info.plist",
    ".kagemusha-candidate-evidence-lab-do-not-ship-v2",
    "ios-arm64/libNoritoBridgeCandidateLab.a",
    "ios-arm64/Headers/connect_norito_bridge.h",
    "ios-arm64/Headers/connect_norito_bridge_base.h",
    "ios-arm64/Headers/module.modulemap",
)

def pairs(values):
    result = {}
    for key, value in values:
        if key in result:
            raise SystemExit(f"native manifest contains duplicate JSON key: {key}")
        result[key] = value
    return result

def exact_regular(path, maximum):
    value = path.lstat()
    if (
        not stat.S_ISREG(value.st_mode)
        or value.st_nlink != 1
        or value.st_uid != os.geteuid()
        or stat.S_IMODE(value.st_mode) & 0o077
        or value.st_size <= 0
        or value.st_size > maximum
    ):
        raise SystemExit(f"native input is not an owner-private bounded file: {path.name}")
    return value

def digest(path, maximum):
    exact_regular(path, maximum)
    value = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()

framework_value = framework.lstat()
if (
    not stat.S_ISDIR(framework_value.st_mode)
    or framework_value.st_uid != os.geteuid()
    or stat.S_IMODE(framework_value.st_mode) & 0o077
):
    raise SystemExit("native framework root is not owner-private")
exact_regular(manifest_path, 16 * 1024 * 1024)
manifest_bytes = manifest_path.read_bytes()
manifest = json.loads(
    manifest_bytes.decode("ascii"),
    object_pairs_hook=pairs,
    parse_constant=lambda value: (_ for _ in ()).throw(
        ValueError(f"non-finite native manifest value: {value}")
    ),
)
canonical = (
    json.dumps(
        manifest,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode("ascii")
    + b"\n"
)
if manifest_bytes != canonical:
    raise SystemExit("native build manifest is not canonical JSON")
files = manifest.get("files")
expected_manifest_paths = {
    f"NoritoBridgeCandidateLab.xcframework/{relative}" for relative in expected
}
if not isinstance(files, dict) or set(files) != expected_manifest_paths:
    raise SystemExit("native build manifest files map is not exact")

actual_files = set()
actual_directories = set()
for path in framework.rglob("*"):
    relative = path.relative_to(framework).as_posix()
    value = path.lstat()
    if stat.S_ISLNK(value.st_mode):
        raise SystemExit(f"native framework contains a symlink: {relative}")
    if stat.S_ISDIR(value.st_mode):
        if value.st_uid != os.geteuid() or stat.S_IMODE(value.st_mode) & 0o077:
            raise SystemExit(f"native framework directory is not owner-private: {relative}")
        actual_directories.add(relative)
    elif stat.S_ISREG(value.st_mode):
        actual_files.add(relative)
    else:
        raise SystemExit(f"native framework contains a special file: {relative}")
if actual_files != set(expected):
    raise SystemExit("native framework inventory is not exact")
if actual_directories != {"ios-arm64", "ios-arm64/Headers"}:
    raise SystemExit("native framework directory inventory is not exact")
for relative in expected:
    maximum = 5 * 1024 * 1024 * 1024 if relative.endswith(".a") else 16 * 1024 * 1024
    manifest_relative = f"NoritoBridgeCandidateLab.xcframework/{relative}"
    if files.get(manifest_relative) != digest(framework / relative, maximum):
        raise SystemExit(f"native framework digest mismatch: {relative}")
PY
}

verify_native_framework "$SOURCE_XCFRAMEWORK"

TRANSIENT="$(mktemp -d "${TMPDIR:-/tmp}/kagemusha-ios-device-lab.XXXXXX")"
chmod 0700 "$TRANSIENT"
cleanup() {
  if [[ -n "${TRANSIENT:-}" && "$TRANSIENT" == "${TMPDIR:-/tmp}"/kagemusha-ios-device-lab.* ]] \
    && [[ -d "$TRANSIENT" ]]; then
    find "$TRANSIENT" -depth -delete
  fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

RAW_ROOT="$EVIDENCE_ROOT/raw"
RAW_INPUT="$RAW_ROOT/input"
RAW_BUILD="$RAW_ROOT/build"
RAW_RUN="$RAW_ROOT/run"
RAW_OUTPUT="$RAW_ROOT/output"

DEVICE_JSON="$TRANSIENT/devices.json"
DEVICE_HASHED_JSON="$TRANSIENT/device-hashed.json"
"$XCRUN_BINARY" devicectl list devices \
  --timeout 20 \
  --quiet \
  --json-output "$DEVICE_JSON"
"$PYTHON3_BINARY" -I - "$DEVICE_JSON" "$DEVICE_ID" "$DEVICE_HASHED_JSON" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

document = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
selector = sys.argv[2]
matches = []
for device in document.get("result", {}).get("devices", []):
    hardware = device.get("hardwareProperties", {})
    candidates = {
        str(device.get("identifier", "")),
        str(hardware.get("udid", "")),
        str(hardware.get("ecid", "")),
        str(hardware.get("serialNumber", "")),
    }
    if selector in candidates:
        matches.append(device)
if len(matches) != 1:
    raise SystemExit("the requested physical device is not uniquely visible")
device = matches[0]
connection = device.get("connectionProperties", {})
properties = device.get("deviceProperties", {})
hardware = device.get("hardwareProperties", {})
checks = {
    "pairingState": connection.get("pairingState") == "paired",
    "tunnelState": connection.get("tunnelState") == "connected",
    "ddiServicesAvailable": properties.get("ddiServicesAvailable") is True,
    "developerModeStatus": properties.get("developerModeStatus") == "enabled",
    "reality": hardware.get("reality") == "physical",
    "platform": hardware.get("platform") == "iOS",
    "isProductionFused": hardware.get("isProductionFused") is True,
    "productType": hardware.get("productType") == "iPhone18,2",
    "hardwareModel": hardware.get("hardwareModel") == "V54AP",
}
failed = [key for key, valid in checks.items() if not valid]
if failed:
    raise SystemExit(
        "physical iPhone is paired but unavailable for a device test: " + ",".join(failed)
    )
cpu_names = {
    str(value.get("name", ""))
    for value in hardware.get("supportedCPUTypes", [])
    if isinstance(value, dict)
}
if not {"arm64", "arm64e"}.issubset(cpu_names):
    raise SystemExit("physical iPhone CPU identity is not arm64e+arm64")

def digest(value, label):
    text = str(value or "")
    if not text:
        raise SystemExit(f"physical device omitted {label}")
    return hashlib.sha256(text.encode("utf-8")).hexdigest()

result = {
    "schema": "iroha.kagemusha.ios_device_lab.hashed_device.v1",
    "version": 1,
    "device_udid_sha256": digest(hardware.get("udid"), "UDID"),
    "device_ecid_sha256": digest(hardware.get("ecid"), "ECID"),
    "device_serial_sha256": digest(hardware.get("serialNumber"), "serial number"),
    "expected_hardware_model": str(hardware["productType"]),
    "expected_board_config": str(hardware["hardwareModel"]),
    "expected_os_version": str(properties.get("osVersionNumber", "")),
    "expected_os_build": str(properties.get("osBuildUpdate", "")),
    "physical": True,
    "paired": True,
    "developer_mode_enabled": True,
}
if not result["expected_os_version"] or not result["expected_os_build"]:
    raise SystemExit("physical device OS identity is incomplete")
Path(sys.argv[3]).write_text(
    json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="ascii",
)
PY
find "$TRANSIENT" -name devices.json -type f -delete

mkdir -- "$EVIDENCE_ROOT"
mkdir -p \
  "$RAW_INPUT/artifacts" "$RAW_INPUT/scenario" \
  "$RAW_BUILD/NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers" \
  "$RAW_RUN" "$RAW_OUTPUT"
find "$EVIDENCE_ROOT" -type d -exec chmod 0700 {} +

XCFRAMEWORK="$TRANSIENT/NoritoBridgeCandidateLab.xcframework"
mkdir -p -- "$XCFRAMEWORK/ios-arm64/Headers"
chmod 0700 "$XCFRAMEWORK" "$XCFRAMEWORK/ios-arm64" "$XCFRAMEWORK/ios-arm64/Headers"
cp -- "$SOURCE_XCFRAMEWORK/Info.plist" "$XCFRAMEWORK/Info.plist"
cp -- \
  "$SOURCE_XCFRAMEWORK/.kagemusha-candidate-evidence-lab-do-not-ship-v2" \
  "$XCFRAMEWORK/.kagemusha-candidate-evidence-lab-do-not-ship-v2"
cp -- \
  "$SOURCE_XCFRAMEWORK/ios-arm64/libNoritoBridgeCandidateLab.a" \
  "$XCFRAMEWORK/ios-arm64/libNoritoBridgeCandidateLab.a"
cp -- \
  "$SOURCE_XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge.h" \
  "$XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge.h"
cp -- \
  "$SOURCE_XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge_base.h" \
  "$XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge_base.h"
cp -- \
  "$SOURCE_XCFRAMEWORK/ios-arm64/Headers/module.modulemap" \
  "$XCFRAMEWORK/ios-arm64/Headers/module.modulemap"
find "$XCFRAMEWORK" -type f -exec chmod 0400 {} +
NATIVE_LIBRARY="$XCFRAMEWORK/ios-arm64/libNoritoBridgeCandidateLab.a"
verify_native_framework "$XCFRAMEWORK"

"$PYTHON3_BINARY" -I - \
  "$CANDIDATE_RECORD" "$CANDIDATE_MANIFEST" "$ROSTER" "$ARTIFACT_ROOT" \
  "$SCENARIO_ROOT" "$REVIEWED_SOURCE_CLOSURE" "$NATIVE_MANIFEST" \
  "$NATIVE_LIBRARY" "$DEVICE_HASHED_JSON" "$RAW_INPUT/session-v1.json" <<'PY'
from pathlib import Path
import hashlib
import json
import stat
import sys

candidate = Path(sys.argv[1])
candidate_manifest = Path(sys.argv[2])
roster = Path(sys.argv[3])
artifact_root = Path(sys.argv[4])
scenario_root = Path(sys.argv[5])
closure = Path(sys.argv[6])
native_manifest_path = Path(sys.argv[7])
native_library = Path(sys.argv[8])
device = json.loads(Path(sys.argv[9]).read_text(encoding="ascii"))
output = Path(sys.argv[10])

artifact_files = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
scenario_files = (
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    "init-top-up-finality-roster-artifact-v2.norito",
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-operation-id.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-operation-id.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-operation-id.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
)

def exact_regular(path: Path, maximum=5 * 1024 * 1024 * 1024):
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) & 0o077
        or metadata.st_size <= 0
        or metadata.st_size > maximum
    ):
        raise SystemExit(f"input is not owner-private bounded regular file: {path.name}")
    return metadata

def digest(path: Path):
    exact_regular(path)
    value = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()

actual_artifacts = tuple(sorted(path.name for path in artifact_root.iterdir()))
if actual_artifacts != tuple(sorted(artifact_files)):
    raise SystemExit("artifact directory does not contain the exact eight KRV4 files")
actual_scenario = tuple(sorted(path.name for path in scenario_root.iterdir()))
if actual_scenario != tuple(sorted(scenario_files)):
    raise SystemExit("scenario directory does not contain the exact thirty-three files")
for name in artifact_files:
    exact_regular(artifact_root / name)
for name in scenario_files:
    exact_regular(scenario_root / name, 16 * 1024 * 1024)

scenario_hasher = hashlib.sha256()
scenario_hasher.update(b"iroha.kagemusha.android-candidate-scenario-inventory.v1\0")
scenario_hasher.update(len(scenario_files).to_bytes(4, "big"))
for name in sorted(scenario_files):
    payload = scenario_root / name
    relative = f"scenario/{name}".encode("utf-8")
    metadata = exact_regular(payload, 16 * 1024 * 1024)
    scenario_hasher.update(len(relative).to_bytes(4, "big"))
    scenario_hasher.update(relative)
    scenario_hasher.update(metadata.st_size.to_bytes(8, "big"))
    scenario_hasher.update(bytes.fromhex(digest(payload)))

native = json.loads(native_manifest_path.read_text(encoding="ascii"))
expected_native_keys = {
    "schema", "version", "profile", "do_not_ship_marker",
    "candidate_feature_enabled", "production_capability_enabled",
    "bridge_abi_version", "target_triple", "architectures",
    "simulator_slice_present", "minimum_ios_version",
    "candidate_record_sha256", "source_commit", "source_tree_sha256",
    "source_repo_dirty", "reviewed_source_closure_descriptor_sha256",
    "iphoneos_sdk_version", "xcode_version", "cargo_version_verbose",
    "rustc_version_verbose", "required_symbols", "files",
}
if set(native) != expected_native_keys:
    raise SystemExit("native build manifest is not closed")
if (
    native["schema"] != "iroha.kagemusha.apple_candidate_native_build.v1"
    or native["profile"] != "physical-ios-candidate-evidence-lab"
    or native["target_triple"] != "aarch64-apple-ios"
    or native["architectures"] != ["arm64"]
    or native["simulator_slice_present"] is not False
    or native["source_repo_dirty"] is not False
    or native["candidate_record_sha256"] != digest(candidate)
    or native["reviewed_source_closure_descriptor_sha256"] != digest(closure)
):
    raise SystemExit("native build manifest does not bind exact physical candidate inputs")
library_relative = (
    "NoritoBridgeCandidateLab.xcframework/ios-arm64/"
    "libNoritoBridgeCandidateLab.a"
)
if native["files"].get(library_relative) != digest(native_library):
    raise SystemExit("native static library differs from its build manifest")

session = {
    "schema": "iroha.kagemusha.ios_device_lab.session.v1",
    "version": 1,
    "candidate_record_sha256": digest(candidate),
    "candidate_manifest_sha256": digest(candidate_manifest),
    "topup_finality_roster_sha256": digest(roster),
    "scenario_inventory_sha256": scenario_hasher.hexdigest(),
    "native_build_manifest_sha256": digest(native_manifest_path),
    "native_library_sha256": digest(native_library),
    "source_commit": native["source_commit"],
    "source_tree_sha256": native["source_tree_sha256"],
    "source_repo_dirty": False,
    "reviewed_source_closure_descriptor_sha256": digest(closure),
    "device_udid_sha256": device["device_udid_sha256"],
    "device_ecid_sha256": device["device_ecid_sha256"],
    "device_serial_sha256": device["device_serial_sha256"],
    "expected_hardware_model": device["expected_hardware_model"],
    "expected_board_config": device["expected_board_config"],
    "expected_os_version": device["expected_os_version"],
    "expected_os_build": device["expected_os_build"],
}
output.write_text(
    json.dumps(session, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="ascii",
)
PY

cp -- "$CANDIDATE_RECORD" "$RAW_INPUT/candidate-v4.norito"
cp -- "$CANDIDATE_MANIFEST" "$RAW_INPUT/candidate-manifest-v4.norito"
cp -- "$ROSTER" "$RAW_INPUT/topup-finality-roster-v4.norito"
cp -- "$REVIEWED_SOURCE_CLOSURE" "$RAW_INPUT/reviewed-source-closure-v1.json"
cp -- "$NATIVE_MANIFEST" "$RAW_INPUT/native-build-manifest.json"
cp -- "$NATIVE_LIBRARY" "$RAW_BUILD/libNoritoBridgeCandidateLab.a"
cp -- \
  "$XCFRAMEWORK/Info.plist" \
  "$RAW_BUILD/NoritoBridgeCandidateLab.xcframework/Info.plist"
cp -- \
  "$XCFRAMEWORK/.kagemusha-candidate-evidence-lab-do-not-ship-v2" \
  "$RAW_BUILD/NoritoBridgeCandidateLab.xcframework/.kagemusha-candidate-evidence-lab-do-not-ship-v2"
cp -- \
  "$XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge.h" \
  "$RAW_BUILD/NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/connect_norito_bridge.h"
cp -- \
  "$XCFRAMEWORK/ios-arm64/Headers/connect_norito_bridge_base.h" \
  "$RAW_BUILD/NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/connect_norito_bridge_base.h"
cp -- \
  "$XCFRAMEWORK/ios-arm64/Headers/module.modulemap" \
  "$RAW_BUILD/NoritoBridgeCandidateLab.xcframework/ios-arm64/Headers/module.modulemap"
for name in \
  step-eq.params-ipa.krv4 \
  step-eq.proving-key.krv4 \
  step-eq.verifying-key.krv4 \
  step-eq.bootstrap-witness.krv4 \
  step-ep.params-ipa.krv4 \
  step-ep.proving-key.krv4 \
  step-ep.verifying-key.krv4 \
  step-ep.bootstrap-witness.krv4
do
  cp -- "$ARTIFACT_ROOT/$name" "$RAW_INPUT/artifacts/$name"
done
for name in \
  init-top-up-anchor-v4.norito \
  init-top-up-finality-proof-v2.norito \
  init-top-up-finality-roster-artifact-v2.norito \
  init-opening-v2.norito \
  init-output-membership-v4.norito \
  transfer-verifier-commitment-v2.bin \
  append-hop-01-recipient-request-v2.norito \
  append-hop-01-recipient-opening-v2.norito \
  append-hop-01-change-opening-v2.norito \
  append-hop-01-output-membership-v4.norito \
  append-hop-01-operation-id.bin \
  append-hop-01-block-height.txt \
  append-hop-01-verified-at-ms.txt \
  append-hop-02-recipient-request-v2.norito \
  append-hop-02-recipient-opening-v2.norito \
  append-hop-02-change-opening-v2.norito \
  append-hop-02-output-membership-v4.norito \
  append-hop-02-operation-id.bin \
  append-hop-02-block-height.txt \
  append-hop-02-verified-at-ms.txt \
  redeem-recipient-account-id.txt \
  unshield-verifier-commitment-v2.bin \
  redeem-hop-01-operation-id.bin \
  redeem-hop-01-block-height.txt \
  redeem-hop-02-operation-id.bin \
  redeem-hop-02-block-height.txt \
  redeem-sender-change-operation-id.bin \
  redeem-sender-change-block-height.txt \
  duplicate-input-recipient-request-v2.norito \
  duplicate-input-output-membership-v4.norito \
  duplicate-input-operation-id.bin \
  duplicate-input-block-height.txt \
  duplicate-input-verified-at-ms.txt
do
  cp -- "$SCENARIO_ROOT/$name" "$RAW_INPUT/scenario/$name"
done
find "$RAW_ROOT" -type f -exec chmod 0600 {} +

PROJECT_ROOT="$TRANSIENT/project"
DERIVED_DATA="$TRANSIENT/DerivedData"
mkdir -- "$PROJECT_ROOT"
KAGEMUSHA_CANDIDATE_XCFRAMEWORK_PATH="$XCFRAMEWORK" \
  "$XCODEGEN_BINARY" generate \
    --quiet \
    --spec "$LAB_SOURCE/project.yml" \
    --project "$PROJECT_ROOT" \
    --project-root "$LAB_SOURCE"
PROJECT="$PROJECT_ROOT/KagemushaCandidateEvidenceLab.xcodeproj"
BUILD_RESULT="$TRANSIENT/build.xcresult"
"$XCODEBUILD_BINARY" \
  -quiet \
  -project "$PROJECT" \
  -scheme KagemushaCandidateEvidenceLab \
  -destination "platform=iOS,id=$DEVICE_ID" \
  -derivedDataPath "$DERIVED_DATA" \
  -resultBundlePath "$BUILD_RESULT" \
  -parallel-testing-enabled NO \
  DEVELOPMENT_TEAM="$DEVELOPMENT_TEAM" \
  CODE_SIGN_STYLE=Automatic \
  ONLY_ACTIVE_ARCH=YES \
  build-for-testing
verify_native_framework "$XCFRAMEWORK"

APP="$DERIVED_DATA/Build/Products/Debug-iphoneos/KagemushaCandidateEvidenceLabHost.app"
XCTESTRUN="$(
  find "$DERIVED_DATA/Build/Products" -maxdepth 1 -type f -name '*.xctestrun'
)"
if [[ -z "$XCTESTRUN" || "$XCTESTRUN" == *$'\n'* ]]; then
  echo "[kagemusha-ios-lab] ERROR: build did not produce exactly one xctestrun file" >&2
  exit 1
fi
TEST_BUNDLE="$(
  find "$DERIVED_DATA/Build/Products/Debug-iphoneos" \
    -type d -name 'KagemushaCandidateEvidenceLabTests.xctest' -prune
)"
if [[ ! -d "$APP" || -z "$TEST_BUNDLE" || "$TEST_BUNDLE" == *$'\n'* ]] \
  || [[ ! -d "$TEST_BUNDLE" ]]; then
  echo "[kagemusha-ios-lab] ERROR: signed app/test products are unavailable or ambiguous" >&2
  exit 1
fi

"$CODESIGN_BINARY" --verify --deep --strict --verbose=4 "$APP"
"$CODESIGN_BINARY" --verify --strict --verbose=4 "$TEST_BUNDLE"
APP_ENTITLEMENTS="$TRANSIENT/app-entitlements.plist"
TEST_ENTITLEMENTS="$TRANSIENT/test-entitlements.plist"
"$CODESIGN_BINARY" -d --entitlements :- "$APP" >"$APP_ENTITLEMENTS" 2>/dev/null
"$CODESIGN_BINARY" -d --entitlements :- "$TEST_BUNDLE" >"$TEST_ENTITLEMENTS" 2>/dev/null
if [[ ! -s "$APP_ENTITLEMENTS" || ! -s "$TEST_ENTITLEMENTS" ]]; then
  echo "[kagemusha-ios-lab] ERROR: signed app/test entitlements are unavailable" >&2
  exit 1
fi
APP_DETAILS="$TRANSIENT/app-codesign.txt"
TEST_DETAILS="$TRANSIENT/test-codesign.txt"
"$CODESIGN_BINARY" -dvvv "$APP" >"$APP_DETAILS" 2>&1
"$CODESIGN_BINARY" -dvvv "$TEST_BUNDLE" >"$TEST_DETAILS" 2>&1
"$PYTHON3_BINARY" -I - \
  "$APP" "$TEST_BUNDLE" "$APP_ENTITLEMENTS" "$TEST_ENTITLEMENTS" \
  "$APP_DETAILS" "$TEST_DETAILS" "$NATIVE_LIBRARY" "$NATIVE_MANIFEST" \
  "$RAW_BUILD/code-sign-measurements-v1.json" "$DEVELOPMENT_TEAM" <<'PY'
from pathlib import Path
import hashlib
import json
import plistlib
import re
import stat
import sys

app = Path(sys.argv[1])
test = Path(sys.argv[2])
app_entitlements_path = Path(sys.argv[3])
test_entitlements_path = Path(sys.argv[4])
app_details = Path(sys.argv[5]).read_text(errors="strict")
test_details = Path(sys.argv[6]).read_text(errors="strict")
native = Path(sys.argv[7])
native_manifest = Path(sys.argv[8])
output = Path(sys.argv[9])
expected_team = sys.argv[10]

def digest(path: Path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            value.update(chunk)
    return value.hexdigest()

def detail(text, key):
    match = re.search(rf"^{re.escape(key)}=(.+)$", text, re.MULTILINE)
    if not match:
        raise SystemExit(f"code signature omitted {key}")
    return match.group(1).strip()

def bundle_info(bundle: Path):
    with (bundle / "Info.plist").open("rb") as handle:
        return plistlib.load(handle)

def profile_digest(bundle: Path):
    profile = bundle / "embedded.mobileprovision"
    return digest(profile) if profile.is_file() else None

app_info = bundle_info(app)
test_info = bundle_info(test)
app_executable = app / app_info["CFBundleExecutable"]
test_executable = test / test_info["CFBundleExecutable"]
app_team = detail(app_details, "TeamIdentifier")
test_team = detail(test_details, "TeamIdentifier")
if app_team != expected_team or test_team != expected_team:
    raise SystemExit("app/test code signing team differs from requested team")
app_profile = profile_digest(app)
test_profile = profile_digest(test)
if app_profile is None or test_profile is None:
    raise SystemExit("physical app/test bundles must contain provisioning profiles")
value = {
    "schema": "iroha.kagemusha.ios_device_lab.code_sign_measurements.v1",
    "version": 1,
    "app": {
        "bundle_id": app_info["CFBundleIdentifier"],
        "version": app_info["CFBundleShortVersionString"],
        "build": app_info["CFBundleVersion"],
        "identifier": detail(app_details, "Identifier"),
        "team_id": app_team,
        "cdhash": detail(app_details, "CDHash").lower(),
        "executable_sha256": digest(app_executable),
        "entitlements_sha256": digest(app_entitlements_path),
        "provisioning_profile_sha256": app_profile,
    },
    "test": {
        "bundle_id": test_info["CFBundleIdentifier"],
        "identifier": detail(test_details, "Identifier"),
        "team_id": test_team,
        "cdhash": detail(test_details, "CDHash").lower(),
        "executable_sha256": digest(test_executable),
        "entitlements_sha256": digest(test_entitlements_path),
        "provisioning_profile_sha256": test_profile,
    },
    "native": {
        "kind": "static_library_bound_into_signed_test_bundle",
        "sha256": digest(native),
        "build_manifest_sha256": digest(native_manifest),
        "architectures": ["arm64"],
        "simulator_slice_used": False,
    },
}
output.write_text(
    json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n",
    encoding="ascii",
)
PY
chmod 0600 "$RAW_BUILD/code-sign-measurements-v1.json"
if [[ "$("$LIPO_BINARY" -archs "$NATIVE_LIBRARY")" != "arm64" ]]; then
  echo "[kagemusha-ios-lab] ERROR: candidate native library is not device-only arm64" >&2
  exit 1
fi

APP_BUNDLE_ID="org.hyperledger.iroha.kagemusha-candidate-lab.host"
INSTALL_JSON="$TRANSIENT/install.json"
"$XCRUN_BINARY" devicectl device install app \
  --device "$DEVICE_ID" \
  --timeout 120 \
  --quiet \
  --json-output "$INSTALL_JSON" \
  "$APP"
"$XCRUN_BINARY" devicectl device copy to \
  --device "$DEVICE_ID" \
  --source "$RAW_INPUT" \
  --destination "Documents/$INPUT_DIRECTORY_NAME" \
  --domain-type appDataContainer \
  --domain-identifier "$APP_BUNDLE_ID" \
  --remove-existing-content true \
  --timeout 600 \
  --quiet \
  --json-output "$TRANSIENT/stage-input.json"

run_test_phase() {
  local phase="$1"
  local test_name="$2"
  local result_bundle="$TRANSIENT/$phase.xcresult"
  "$XCODEBUILD_BINARY" \
    -quiet \
    -xctestrun "$XCTESTRUN" \
    -destination "platform=iOS,id=$DEVICE_ID" \
    -resultBundlePath "$result_bundle" \
    -parallel-testing-enabled NO \
    -only-testing:"KagemushaCandidateEvidenceLabTests/KagemushaCandidateEvidenceLabTests/$test_name" \
    test-without-building
}

pull_output_file() {
  local file_name="$1"
  "$XCRUN_BINARY" devicectl device copy from \
    --device "$DEVICE_ID" \
    --source "Documents/$OUTPUT_DIRECTORY_NAME/$file_name" \
    --destination "$RAW_OUTPUT/$file_name" \
    --domain-type appDataContainer \
    --domain-identifier "$APP_BUNDLE_ID" \
    --timeout 120 \
    --quiet \
    --json-output "$TRANSIENT/pull-$file_name.json"
  chmod 0600 "$RAW_OUTPUT/$file_name"
}

push_output_file() {
  local file_name="$1"
  "$XCRUN_BINARY" devicectl device copy to \
    --device "$DEVICE_ID" \
    --source "$RAW_OUTPUT/$file_name" \
    --destination "Documents/$OUTPUT_DIRECTORY_NAME/$file_name" \
    --domain-type appDataContainer \
    --domain-identifier "$APP_BUNDLE_ID" \
    --timeout 120 \
    --quiet \
    --json-output "$TRANSIENT/push-$file_name.json"
}

run_test_phase proof testProofPhase
pull_output_file install-identity-v1.bin
pull_output_file checkpoint-v1.norito
pull_output_file proof-launch-receipt-v1.json
for file_name in install-identity-v1.bin checkpoint-v1.norito proof-launch-receipt-v1.json; do
  push_output_file "$file_name"
done
run_test_phase restart testRestartPhase
pull_output_file native-transcript-v1.json
pull_output_file restart-launch-receipt-v1.json

"$PYTHON3_BINARY" -I - \
  "$RAW_OUTPUT/proof-launch-receipt-v1.json" \
  "$RAW_OUTPUT/restart-launch-receipt-v1.json" \
  "$RAW_OUTPUT/native-transcript-v1.json" \
  "$RAW_BUILD/code-sign-measurements-v1.json" \
  "$RAW_RUN/proof-test-result-v1.json" \
  "$RAW_RUN/restart-test-result-v1.json" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

proof_path, restart_path, transcript_path, code_path, proof_out, restart_out = map(Path, sys.argv[1:])

def load(path):
    return json.loads(path.read_text(encoding="utf-8"))

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

proof = load(proof_path)
restart = load(restart_path)
transcript = load(transcript_path)
code = load(code_path)
if proof.get("phase") != "proof" or restart.get("phase") != "restart":
    raise SystemExit("device receipts do not bind the two exact phases")
if proof.get("process_id") == restart.get("process_id"):
    raise SystemExit("device receipts reused one process")
if proof.get("launch_nonce_sha256") == restart.get("launch_nonce_sha256"):
    raise SystemExit("device receipts reused one launch nonce")
if proof.get("device") != restart.get("device"):
    raise SystemExit("device/boot identity changed between launches")
if proof.get("code_identity") != restart.get("code_identity"):
    raise SystemExit("app/test code identity changed between launches")
if proof["code_identity"]["app_executable_sha256"] != code["app"]["executable_sha256"]:
    raise SystemExit("runtime app executable differs from host code-sign measurement")
if proof["code_identity"]["test_executable_sha256"] != code["test"]["executable_sha256"]:
    raise SystemExit("runtime test executable differs from host code-sign measurement")
if restart.get("native_transcript_sha256") != digest(transcript_path):
    raise SystemExit("restart receipt does not bind the native transcript")
for phase, receipt, output in (
    ("proof", proof, proof_out),
    ("restart", restart, restart_out),
):
    value = {
        "schema": "iroha.kagemusha.ios_device_lab.test_result.v1",
        "version": 1,
        "phase": phase,
        "test_status": "passed",
        "test_identifier": (
            "KagemushaCandidateEvidenceLabTests/"
            "KagemushaCandidateEvidenceLabTests/"
            f"test{phase.capitalize()}Phase"
        ),
        "launch_receipt_sha256": digest(
            proof_path if phase == "proof" else restart_path
        ),
        "native_transcript_sha256": (
            None if phase == "proof" else digest(transcript_path)
        ),
    }
    output.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="ascii",
    )
PY
find "$RAW_ROOT" -type f -exec chmod 0600 {} +
find "$RAW_ROOT" -type d -exec chmod 0700 {} +

echo "[kagemusha-ios-lab] raw physical-iPhone evidence is ready: $RAW_ROOT"
echo "[kagemusha-ios-lab] sign it with scripts/sign_kagemusha_candidate_ios_evidence.py"
