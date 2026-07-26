#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat <<'USAGE'
Usage:
  scripts/run_kagemusha_candidate_android_lab.sh [--build-only] \
    --candidate-sha256 <hex> \
    --stage-sha256 <hex> \
    --source-commit <git-hex> \
    --source-tree-sha256 <hex> \
    --generation <id> \
    --slot-id <device-lab-slot> \
    [--attestation-slot <slot-directory>] \
    [--trusted-signer-public-key <PEM>] \
    [--apksigner <absolute-path> --apksigner-sha256 <hex>] \
    [--openssl <absolute-path> --openssl-sha256 <hex>] \
    [--android-attestation-trust-root <PEM> \
     --android-attestation-trust-root-sha256 <hex>]... \
    [--android-attestation-revocation-status <JSON> \
     --android-attestation-revocation-status-sha256 <hex>] \
    [--serial <adb-serial>]

The required physical-evidence sequence is:
  1. Run with --build-only. Retain both exact staged APKs and the printed
     32-byte candidate-stage StrongBox challenge.
  2. Use the authorized external device-lab capture to collect fresh StrongBox
     evidence for those bytes, execute/export the complete canonical lifecycle,
     and sign the complete candidate-bound slot.
  3. Run without --build-only and pass that complete signed reference slot plus
     the explicit trusted release-signing and attestation authority inputs.

The full run cryptographically validates the complete signed reference,
rebuilds/revalidates both APKs, executes an independent confirmation of the two
lifecycle processes, compares its complete deterministic semantics with the
reference (only durations may vary), exports confirmation evidence, and removes
only the candidate lab packages on every exit. It is not the original capture.
USAGE
}

fail() {
  echo "[kagemusha-candidate-lab] ERROR: $*" >&2
  exit 1
}

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CANDIDATE_SHA256=""
STAGE_SHA256=""
SOURCE_COMMIT=""
SOURCE_TREE_SHA256=""
GENERATION=""
SLOT_ID=""
ATTESTATION_SLOT=""
TRUSTED_SIGNER_PUBLIC_KEY=""
AUTHORITY_APKSIGNER=""
AUTHORITY_APKSIGNER_SHA256=""
AUTHORITY_OPENSSL=""
AUTHORITY_OPENSSL_SHA256=""
AUTHORITY_REVOCATION_STATUS=""
AUTHORITY_REVOCATION_STATUS_SHA256=""
AUTHORITY_TRUST_ROOTS=()
AUTHORITY_TRUST_ROOT_SHA256=()
SERIAL=""
BUILD_ONLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build-only)
      BUILD_ONLY=true
      shift
      ;;
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
    --generation)
      GENERATION="${2:-}"
      shift 2
      ;;
    --slot-id)
      SLOT_ID="${2:-}"
      shift 2
      ;;
    --attestation-slot)
      ATTESTATION_SLOT="${2:-}"
      shift 2
      ;;
    --trusted-signer-public-key)
      [[ -z "$TRUSTED_SIGNER_PUBLIC_KEY" ]] || fail \
        "--trusted-signer-public-key may be provided exactly once"
      TRUSTED_SIGNER_PUBLIC_KEY="${2:-}"
      shift 2
      ;;
    --apksigner)
      [[ -z "$AUTHORITY_APKSIGNER" ]] || fail "--apksigner may be provided exactly once"
      AUTHORITY_APKSIGNER="${2:-}"
      shift 2
      ;;
    --apksigner-sha256)
      [[ -z "$AUTHORITY_APKSIGNER_SHA256" ]] || fail \
        "--apksigner-sha256 may be provided exactly once"
      AUTHORITY_APKSIGNER_SHA256="${2:-}"
      shift 2
      ;;
    --openssl)
      [[ -z "$AUTHORITY_OPENSSL" ]] || fail "--openssl may be provided exactly once"
      AUTHORITY_OPENSSL="${2:-}"
      shift 2
      ;;
    --openssl-sha256)
      [[ -z "$AUTHORITY_OPENSSL_SHA256" ]] || fail \
        "--openssl-sha256 may be provided exactly once"
      AUTHORITY_OPENSSL_SHA256="${2:-}"
      shift 2
      ;;
    --android-attestation-trust-root)
      AUTHORITY_TRUST_ROOTS+=("${2:-}")
      shift 2
      ;;
    --android-attestation-trust-root-sha256)
      AUTHORITY_TRUST_ROOT_SHA256+=("${2:-}")
      shift 2
      ;;
    --android-attestation-revocation-status)
      [[ -z "$AUTHORITY_REVOCATION_STATUS" ]] || fail \
        "--android-attestation-revocation-status may be provided exactly once"
      AUTHORITY_REVOCATION_STATUS="${2:-}"
      shift 2
      ;;
    --android-attestation-revocation-status-sha256)
      [[ -z "$AUTHORITY_REVOCATION_STATUS_SHA256" ]] || fail \
        "--android-attestation-revocation-status-sha256 may be provided exactly once"
      AUTHORITY_REVOCATION_STATUS_SHA256="${2:-}"
      shift 2
      ;;
    --serial)
      SERIAL="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[kagemusha-candidate-lab] ERROR: unexpected argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

[[ "$CANDIDATE_SHA256" =~ ^[0-9a-f]{64}$ ]] || fail \
  "--candidate-sha256 must be lowercase SHA-256"
[[ "$STAGE_SHA256" =~ ^[0-9a-f]{64}$ ]] || fail \
  "--stage-sha256 must be lowercase SHA-256"
[[ "$SOURCE_COMMIT" =~ ^[0-9a-f]{40}$ ]] || fail \
  "--source-commit must be lowercase git hex"
[[ "$SOURCE_TREE_SHA256" =~ ^[0-9a-f]{64}$ ]] || fail \
  "--source-tree-sha256 must be lowercase SHA-256"
[[ "$GENERATION" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]] || fail \
  "--generation is invalid"
[[ "$SLOT_ID" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]] || fail \
  "--slot-id is invalid"
if [[ -n "$SERIAL" && ( "$SERIAL" == *[[:space:]]* || "$SERIAL" == -* ) ]]; then
  fail "--serial is invalid"
fi
if [[ "$BUILD_ONLY" == true && -n "$ATTESTATION_SLOT" ]]; then
  fail "--build-only must run before and without --attestation-slot"
fi
if [[ "$BUILD_ONLY" == true && -n "$TRUSTED_SIGNER_PUBLIC_KEY" ]]; then
  fail "--build-only does not consume a trusted attestation-slot signer"
fi
if [[ "$BUILD_ONLY" == true && ( -n "$AUTHORITY_APKSIGNER" \
    || -n "$AUTHORITY_APKSIGNER_SHA256" || -n "$AUTHORITY_OPENSSL" \
    || -n "$AUTHORITY_OPENSSL_SHA256" || -n "$AUTHORITY_REVOCATION_STATUS" \
    || -n "$AUTHORITY_REVOCATION_STATUS_SHA256" \
    || ${#AUTHORITY_TRUST_ROOTS[@]} -ne 0 \
    || ${#AUTHORITY_TRUST_ROOT_SHA256[@]} -ne 0 ) ]]; then
  fail "--build-only does not consume attestation authority inputs"
fi
if [[ "$BUILD_ONLY" == false && -z "$ATTESTATION_SLOT" ]]; then
  fail "the full run requires a freshly collected --attestation-slot"
fi
if [[ "$BUILD_ONLY" == false && -z "$TRUSTED_SIGNER_PUBLIC_KEY" ]]; then
  fail "the full run requires --trusted-signer-public-key"
fi
if [[ "$BUILD_ONLY" == false ]]; then
  [[ -n "$AUTHORITY_APKSIGNER" && -n "$AUTHORITY_APKSIGNER_SHA256" ]] || fail \
    "the full run requires --apksigner and --apksigner-sha256"
  [[ -n "$AUTHORITY_OPENSSL" && -n "$AUTHORITY_OPENSSL_SHA256" ]] || fail \
    "the full run requires --openssl and --openssl-sha256"
  [[ -n "$AUTHORITY_REVOCATION_STATUS" \
      && -n "$AUTHORITY_REVOCATION_STATUS_SHA256" ]] || fail \
    "the full run requires a pinned Android attestation revocation-status JSON"
  [[ ${#AUTHORITY_TRUST_ROOTS[@]} -ge 1 \
      && ${#AUTHORITY_TRUST_ROOTS[@]} -eq ${#AUTHORITY_TRUST_ROOT_SHA256[@]} ]] || fail \
    "the full run requires aligned pinned Android attestation trust roots"
fi

GIT_BINARY="$(command -v git 2>/dev/null || true)"
PYTHON3_BINARY="$(command -v python3 2>/dev/null || true)"
SHASUM_BINARY="$(command -v shasum 2>/dev/null || true)"
if [[ "$BUILD_ONLY" == true ]]; then
  OPENSSL_BINARY="$(command -v openssl 2>/dev/null || true)"
else
  OPENSSL_BINARY="$AUTHORITY_OPENSSL"
fi
CARGO_BINARY="$(command -v cargo 2>/dev/null || true)"
RUSTC_BINARY="$(command -v rustc 2>/dev/null || true)"
CARGO_NDK_BINARY="$(command -v cargo-ndk 2>/dev/null || true)"
[[ -x "$GIT_BINARY" && -x "$PYTHON3_BINARY" && -x "$SHASUM_BINARY" ]] || fail \
  "git, python3, and shasum are required"
[[ -x "$CARGO_BINARY" && -x "$RUSTC_BINARY" && -x "$CARGO_NDK_BINARY" ]] || fail \
  "cargo, rustc, and cargo-ndk are required"
[[ -x "$OPENSSL_BINARY" ]] || fail "openssl is required"
if [[ "$BUILD_ONLY" == false ]]; then
  [[ "$ATTESTATION_SLOT" == /* && "$ATTESTATION_SLOT" != */./* \
      && "$ATTESTATION_SLOT" != */../* && "$ATTESTATION_SLOT" != */ \
      && "$ATTESTATION_SLOT" != *//* ]] || fail \
    "--attestation-slot must be one canonical absolute slot path"
  [[ "${ATTESTATION_SLOT##*/}" == "$SLOT_ID" ]] || fail \
    "--attestation-slot basename must exactly match --slot-id"
  [[ -d "$ATTESTATION_SLOT" && ! -L "$ATTESTATION_SLOT" ]] || fail \
    "--attestation-slot must be one regular slot directory"
  [[ "$TRUSTED_SIGNER_PUBLIC_KEY" == /* \
      && "$TRUSTED_SIGNER_PUBLIC_KEY" != */./* \
      && "$TRUSTED_SIGNER_PUBLIC_KEY" != */../* \
      && "$TRUSTED_SIGNER_PUBLIC_KEY" != */ \
      && "$TRUSTED_SIGNER_PUBLIC_KEY" != *//* ]] || fail \
    "--trusted-signer-public-key must be one canonical absolute path"
  [[ -f "$TRUSTED_SIGNER_PUBLIC_KEY" && ! -L "$TRUSTED_SIGNER_PUBLIC_KEY" ]] || fail \
    "--trusted-signer-public-key must be one regular PEM public key"
  for authority_path in \
    "$AUTHORITY_APKSIGNER" \
    "$AUTHORITY_OPENSSL" \
    "$AUTHORITY_REVOCATION_STATUS" \
    "${AUTHORITY_TRUST_ROOTS[@]}"; do
    [[ "$authority_path" == /* && "$authority_path" != */./* \
        && "$authority_path" != */../* && "$authority_path" != */ \
        && "$authority_path" != *//* ]] || fail \
      "attestation authority paths must be canonical and absolute"
    [[ -f "$authority_path" && ! -L "$authority_path" ]] || fail \
      "attestation authority inputs must be regular non-symlink files"
  done
  [[ -x "$AUTHORITY_APKSIGNER" && -x "$AUTHORITY_OPENSSL" ]] || fail \
    "pinned apksigner and openssl authority tools must be executable"
  for authority_digest in \
    "$AUTHORITY_APKSIGNER_SHA256" \
    "$AUTHORITY_OPENSSL_SHA256" \
    "$AUTHORITY_REVOCATION_STATUS_SHA256" \
    "${AUTHORITY_TRUST_ROOT_SHA256[@]}"; do
    [[ "$authority_digest" =~ ^[0-9a-f]{64}$ \
        && "$authority_digest" != "$(printf '0%.0s' {1..64})" ]] || fail \
      "attestation authority digests must be non-zero lowercase SHA-256"
  done
fi

EVIDENCE_ROOT="$ROOT_DIR/artifacts/kagemusha-candidate-evidence/$CANDIDATE_SHA256/$STAGE_SHA256"
STAGE_MANIFEST="$EVIDENCE_ROOT/candidate-stage-manifest-v1.json"
NATIVE_LIBRARY="$EVIDENCE_ROOT/evidence/candidate/lib/arm64-v8a/libconnect_norito_bridge.so"
LAB_APK="$EVIDENCE_ROOT/evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-$CANDIDATE_SHA256-debug.apk"
TEST_APK="$EVIDENCE_ROOT/evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-$CANDIDATE_SHA256-debug-androidTest.apk"
PACKAGE="org.hyperledger.iroha.sdk.kagemusha.candidate.lab"
TEST_PACKAGE="$PACKAGE.test"
RUNNER="androidx.test.runner.AndroidJUnitRunner"
LIFECYCLE_CLASS="$PACKAGE.KagemushaCandidateLifecycleInstrumentedTest"
EXPORT_CLASS="$PACKAGE.KagemushaCandidateArtifactExportInstrumentedTest"
REMOTE_EVIDENCE="/sdcard/Android/data/$PACKAGE/files/evidence"
SOURCE_SEAL="$ROOT_DIR/scripts/kagemusha_source_tree_seal.py"

sha256_file() {
  local output
  output="$("$SHASUM_BINARY" -a 256 "$1")"
  printf '%s\n' "${output%% *}"
}

verify_pinned_file() {
  local path="$1"
  local expected="$2"
  local label="$3"
  [[ "$(sha256_file "$path")" == "$expected" ]] || fail \
    "$label does not match its explicit SHA-256 pin"
}

verify_attestation_authority_inputs() {
  [[ "$BUILD_ONLY" == false ]] || return 0
  verify_pinned_file "$AUTHORITY_APKSIGNER" "$AUTHORITY_APKSIGNER_SHA256" apksigner
  verify_pinned_file "$AUTHORITY_OPENSSL" "$AUTHORITY_OPENSSL_SHA256" openssl
  verify_pinned_file \
    "$AUTHORITY_REVOCATION_STATUS" \
    "$AUTHORITY_REVOCATION_STATUS_SHA256" \
    android-attestation-revocation-status
  local index
  for ((index = 0; index < ${#AUTHORITY_TRUST_ROOTS[@]}; index++)); do
    verify_pinned_file \
      "${AUTHORITY_TRUST_ROOTS[$index]}" \
      "${AUTHORITY_TRUST_ROOT_SHA256[$index]}" \
      "android-attestation-trust-root[$index]"
  done
}

verify_attestation_authority_inputs

source_snapshot() {
  local label="$1"
  local commit fingerprint
  commit="$("$GIT_BINARY" -C "$ROOT_DIR" rev-parse HEAD)"
  fingerprint="$("$PYTHON3_BINARY" -I "$SOURCE_SEAL" fingerprint --root "$ROOT_DIR")"
  [[ "$commit" == "$SOURCE_COMMIT" ]] || fail \
    "$label source commit does not match --source-commit"
  [[ "$fingerprint" == "$SOURCE_TREE_SHA256" ]] || fail \
    "$label full source-tree seal does not match --source-tree-sha256"
  echo "[kagemusha-candidate-lab] source_tree_sha256_${label}=$fingerprint"
}

[[ -f "$STAGE_MANIFEST" && ! -L "$STAGE_MANIFEST" ]] || fail \
  "missing regular candidate stage manifest: $STAGE_MANIFEST"
[[ "$(sha256_file "$STAGE_MANIFEST")" == "$STAGE_SHA256" ]] || fail \
  "candidate stage manifest digest does not match --stage-sha256"
"$PYTHON3_BINARY" -I - "$ROOT_DIR" "$EVIDENCE_ROOT" "$CANDIDATE_SHA256" "$STAGE_SHA256" \
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

source_snapshot before

mkdir -p "$EVIDENCE_ROOT/build"
chmod 0700 "$EVIDENCE_ROOT/build"
BUILD_TMP="$(mktemp -d "$EVIDENCE_ROOT/build/runner.XXXXXX")"
chmod 0700 "$BUILD_TMP"
cleanup_build_tmp() {
  rm -rf "$BUILD_TMP"
}
trap cleanup_build_tmp EXIT

"$ROOT_DIR/scripts/build_kagemusha_candidate_android_native.sh" \
  --candidate-sha256 "$CANDIDATE_SHA256" \
  --stage-sha256 "$STAGE_SHA256" \
  --source-commit "$SOURCE_COMMIT" \
  --source-tree-sha256 "$SOURCE_TREE_SHA256"

PRIVATE_GRADLE_HOME="$BUILD_TMP/gradle-user-home"
mkdir -p "$PRIVATE_GRADLE_HOME/caches" "$PRIVATE_GRADLE_HOME/wrapper"
chmod 0700 "$PRIVATE_GRADLE_HOME" "$PRIVATE_GRADLE_HOME/caches" "$PRIVATE_GRADLE_HOME/wrapper"
SOURCE_GRADLE_HOME="${GRADLE_USER_HOME:-$HOME/.gradle}"
for cache_path in \
  "$SOURCE_GRADLE_HOME"/caches/modules-2 \
  "$SOURCE_GRADLE_HOME"/caches/jars-* \
  "$SOURCE_GRADLE_HOME"/caches/transforms-*; do
  if [[ -e "$cache_path" ]]; then
    ln -s "$cache_path" "$PRIVATE_GRADLE_HOME/caches/$(basename "$cache_path")"
  fi
done
if [[ -d "$SOURCE_GRADLE_HOME/wrapper/dists" ]]; then
  ln -s "$SOURCE_GRADLE_HOME/wrapper/dists" "$PRIVATE_GRADLE_HOME/wrapper/dists"
fi
JAVA_HOME_RESOLVED="${JAVA_HOME:-}"
if [[ -z "$JAVA_HOME_RESOLVED" && -x /usr/libexec/java_home ]]; then
  JAVA_HOME_RESOLVED="$(/usr/libexec/java_home -v 21)"
fi
[[ -x "$JAVA_HOME_RESOLVED/bin/java" ]] || fail "a pinned JDK 21 JAVA_HOME is required"
ANDROID_SDK_RESOLVED="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
[[ -d "$ANDROID_SDK_RESOLVED" ]] || fail "Android SDK root is unavailable"
ADB_BINARY="$ANDROID_SDK_RESOLVED/platform-tools/adb"
[[ -x "$ADB_BINARY" ]] || fail "Android SDK platform-tools adb is required"
ANDROID_NDK_RESOLVED="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
[[ -d "$ANDROID_NDK_RESOLVED" ]] || fail "Android NDK root is unavailable"
GRADLE_SAFE_PATH="$JAVA_HOME_RESOLVED/bin:/usr/bin:/bin:/usr/sbin:/sbin"
GRADLE_ENV=(
  env -i
  HOME="$HOME"
  PATH="$GRADLE_SAFE_PATH"
  TMPDIR="${TMPDIR:-/tmp}"
  LANG="${LANG:-C.UTF-8}"
  JAVA_HOME="$JAVA_HOME_RESOLVED"
  ANDROID_HOME="$ANDROID_SDK_RESOLVED"
  ANDROID_SDK_ROOT="$ANDROID_SDK_RESOLVED"
  GRADLE_USER_HOME="$PRIVATE_GRADLE_HOME"
)
GRADLE_ENV+=(
  ANDROID_NDK_HOME="$ANDROID_NDK_RESOLVED"
  ANDROID_NDK_ROOT="$ANDROID_NDK_RESOLVED"
)

(
  cd "$ROOT_DIR/kotlin"
  "${GRADLE_ENV[@]}" ./gradlew --no-daemon --offline --max-workers=2 \
    -PkagemushaCandidateEvidenceLab=true \
    -PkagemushaCandidateSha256="$CANDIDATE_SHA256" \
    -PkagemushaCandidateStageSha256="$STAGE_SHA256" \
    -PkagemushaCandidateEvidenceRoot="$EVIDENCE_ROOT" \
    -PkagemushaCandidateSourceCommit="$SOURCE_COMMIT" \
    -PkagemushaCandidateSourceTreeSha256="$SOURCE_TREE_SHA256" \
    -PkagemushaCandidateGeneration="$GENERATION" \
    -PkagemushaCandidateSlotId="$SLOT_ID" \
    -PkagemushaCandidateLabNativeLibrary="$NATIVE_LIBRARY" \
    :kagemusha-candidate-evidence-lab:stageCandidateLabApk \
    :kagemusha-candidate-evidence-lab:stageCandidateLabTestApk
)

for apk in "$LAB_APK" "$TEST_APK"; do
  [[ -f "$apk" && ! -L "$apk" ]] || fail "staged APK is absent or not regular: $apk"
done

find_apksigner() {
  "$PYTHON3_BINARY" -I - "$ANDROID_SDK_RESOLVED" <<'PY'
from pathlib import Path
import os
import sys

candidates = sorted(
    candidate
    for candidate in (Path(sys.argv[1]) / "build-tools").glob("*/apksigner")
    if candidate.is_file() and os.access(candidate, os.X_OK)
)
if not candidates:
    raise SystemExit(1)
print(candidates[-1].resolve())
PY
}

if [[ "$BUILD_ONLY" == true ]]; then
  APKSIGNER="$(find_apksigner)" || fail "Android build-tools apksigner is required"
else
  APKSIGNER="$AUTHORITY_APKSIGNER"
fi

write_toolchain_audit() {
  local output="$EVIDENCE_ROOT/evidence/candidate-build-toolchain-v1.json"
  local cargo_ndk ndk_properties
  cargo_ndk="$CARGO_NDK_BINARY"
  ndk_properties="$ANDROID_NDK_RESOLVED/source.properties"
  [[ -f "$cargo_ndk" && -f "$ndk_properties" ]] || fail \
    "cargo-ndk and Android NDK identities are required for the build audit"
  mkdir -p "$EVIDENCE_ROOT/evidence"
  "$PYTHON3_BINARY" -I - "$output" "$STAGE_SHA256" "$SOURCE_COMMIT" "$SOURCE_TREE_SHA256" \
    "$CARGO_BINARY" "$RUSTC_BINARY" "$cargo_ndk" "$ndk_properties" \
    "$ROOT_DIR/kotlin/gradle/wrapper/gradle-wrapper.jar" \
    "$ROOT_DIR/kotlin/gradle/wrapper/gradle-wrapper.properties" \
    "$JAVA_HOME_RESOLVED/bin/java" "$APKSIGNER" "$ADB_BINARY" \
    "$PYTHON3_BINARY" "$GIT_BINARY" "$SHASUM_BINARY" "$OPENSSL_BINARY" \
    "$LAB_APK_SHA256" "$TEST_APK_SHA256" \
    "$LAB_APK_CERT_SHA256" "$TEST_APK_CERT_SHA256" <<'PY'
from pathlib import Path
import ctypes
import errno
import hashlib
import json
import os
import stat
import subprocess
import sys

def digest(path_text):
    result = hashlib.sha256()
    with Path(path_text).resolve().open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            result.update(chunk)
    return result.hexdigest()

def bounded_text(path_text, maximum):
    path = Path(path_text)
    if path.stat().st_size > maximum:
        raise SystemExit(f"bounded text input is too large: {path}")
    with path.open("rb") as handle:
        payload = handle.read(maximum + 1)
    if len(payload) > maximum:
        raise SystemExit(f"bounded text input grew while reading: {path}")
    return payload.decode("utf-8")

def version(command):
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    return (result.stdout + result.stderr).strip()

output = Path(sys.argv[1])
ndk_text = bounded_text(sys.argv[8], 64 * 1024).strip()
wrapper_properties = bounded_text(sys.argv[10], 64 * 1024)
root = Path(sys.argv[9]).resolve().parents[3]
sys.path.insert(0, str(root / "scripts"))
from check_android_device_lab_slot import (
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND,
)
payload = {
    "schema": "iroha.kagemusha.android_candidate_build_toolchain.v1",
    "candidate_stage_manifest_sha256": sys.argv[2],
    "source_commit": sys.argv[3],
    "source_tree_sha256_before": sys.argv[4],
    "source_tree_sha256_after": sys.argv[4],
    "cargo_binary_sha256": digest(sys.argv[5]),
    "cargo_version_verbose": version([sys.argv[5], "--version", "--verbose"]),
    "rustc_binary_sha256": digest(sys.argv[6]),
    "rustc_version_verbose": version([sys.argv[6], "--version", "--verbose"]),
    "cargo_ndk_binary_sha256": digest(sys.argv[7]),
    "cargo_ndk_version": version([sys.argv[7], "--version"]),
    "android_ndk_source_properties_sha256": digest(sys.argv[8]),
    "android_ndk_source_properties": ndk_text,
    "gradle_wrapper_jar_sha256": digest(sys.argv[9]),
    "gradle_wrapper_properties_sha256": digest(sys.argv[10]),
    "gradle_distribution_url": next(
        line.split("=", 1)[1]
        for line in wrapper_properties.splitlines()
        if line.startswith("distributionUrl=")
    ),
    "java_binary_sha256": digest(sys.argv[11]),
    "java_version": version([sys.argv[11], "-version"]),
    "apksigner_binary_sha256": digest(sys.argv[12]),
    "apksigner_version": version([sys.argv[12], "version"]),
    "adb_binary_sha256": digest(sys.argv[13]),
    "adb_version": version([sys.argv[13], "version"]),
    "python_binary_sha256": digest(sys.argv[14]),
    "python_version": version([sys.argv[14], "--version"]),
    "git_binary_sha256": digest(sys.argv[15]),
    "git_version": version([sys.argv[15], "--version"]),
    "shasum_binary_sha256": digest(sys.argv[16]),
    "openssl_binary_sha256": digest(sys.argv[17]),
    "openssl_version": version([sys.argv[17], "version"]),
    "candidate_lab_apk_sha256": sys.argv[18],
    "candidate_lab_test_apk_sha256": sys.argv[19],
    "candidate_lab_apk_signing_certificate_sha256": sys.argv[20],
    "candidate_lab_test_apk_signing_certificate_sha256": sys.argv[21],
    "candidate_lab_apksigner_verified": True,
    "candidate_lab_test_apksigner_verified": True,
    "fresh_native_target": True,
    "private_gradle_user_home": True,
    "gradle_offline": True,
    "raw_test_commands": [
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND,
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND,
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND,
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND,
    ],
}
encoded = (json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()

def validate_existing() -> bool:
    try:
        metadata = output.lstat()
    except FileNotFoundError:
        return False
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise SystemExit("existing build-toolchain audit is not one private regular file")
    with output.open("rb") as handle:
        existing = handle.read(len(encoded) + 1)
    if stat.S_IMODE(metadata.st_mode) != 0o600 or existing != encoded:
        raise SystemExit("refusing to replace a different build-toolchain audit")
    return True

def rename_no_replace(source: Path, destination: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    source_bytes = os.fsencode(source)
    destination_bytes = os.fsencode(destination)
    if sys.platform == "darwin" and hasattr(libc, "renamex_np"):
        result = libc.renamex_np(source_bytes, destination_bytes, 0x00000004)
    elif hasattr(libc, "renameat2"):
        result = libc.renameat2(-100, source_bytes, -100, destination_bytes, 1)
    else:
        raise SystemExit("atomic no-replace audit publication is unsupported")
    if result != 0:
        error = ctypes.get_errno()
        if error == errno.EEXIST:
            if validate_existing():
                return
        raise OSError(error, os.strerror(error), destination)

if not validate_existing():
    temporary = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    try:
        with temporary.open("xb") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, 0o600)
        rename_no_replace(temporary, output)
        directory_fd = os.open(output.parent, os.O_RDONLY)
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
}

write_run_receipt() {
  local mode="$1"
  local trusted_summary="${2:-}"
  local trusted_summary_sha256="${3:-}"
  local trusted_key_sha256="${4:-}"
  local signed_evidence_sha256="${5:-}"
  local app_certificate_sha256="${6:-}"
  local chain_sha256="${7:-}"
  local attestation_root="${8:-}"
  local authority_apksigner_sha256="${9:-}"
  local authority_openssl_sha256="${10:-}"
  local authority_revocation_sha256="${11:-}"
  local authority_trust_root_sha256_csv="${12:-}"
  local confirmation_binding="${13:-}"
  local confirmation_transcript="${14:-}"
  local confirmation_report="${15:-}"
  local confirmation_binding_sha256="${16:-}"
  local confirmation_transcript_sha256="${17:-}"
  local confirmation_report_sha256="${18:-}"
  local output
  if [[ "$mode" == "build_only" ]]; then
    output="$EVIDENCE_ROOT/evidence/candidate-build-only-receipt-v1.json"
  else
    output="$EVIDENCE_ROOT/evidence/candidate-full-run-receipt-v1.json"
  fi
  local toolchain="$EVIDENCE_ROOT/evidence/candidate-build-toolchain-v1.json"
  "$PYTHON3_BINARY" -I - \
    "$ROOT_DIR" "$output" "$mode" "$CANDIDATE_SHA256" "$STAGE_SHA256" \
    "$SOURCE_COMMIT" "$SOURCE_TREE_SHA256" "$GENERATION" "$SLOT_ID" \
    "$NATIVE_LIBRARY" "$(sha256_file "$NATIVE_LIBRARY")" \
    "$LAB_APK" "$LAB_APK_SHA256" "$LAB_APK_CERT_SHA256" \
    "$TEST_APK" "$TEST_APK_SHA256" "$TEST_APK_CERT_SHA256" \
    "$toolchain" "$(sha256_file "$toolchain")" \
    "$CHALLENGE_HEX" "$CHALLENGE_SHA256" "$ADB_BINARY" "$SERIAL" \
    "$app_certificate_sha256" "$chain_sha256" \
    "$trusted_summary" "$trusted_summary_sha256" \
    "$TRUSTED_SIGNER_PUBLIC_KEY" "$trusted_key_sha256" \
    "$signed_evidence_sha256" "$ANDROID_SDK_RESOLVED" \
    "$ANDROID_NDK_RESOLVED" "$JAVA_HOME_RESOLVED/bin/java" "$APKSIGNER" \
    "$attestation_root" "$PYTHON3_BINARY" \
    "$authority_apksigner_sha256" "$authority_openssl_sha256" \
    "$authority_revocation_sha256" "$authority_trust_root_sha256_csv" \
    "$confirmation_binding" "$confirmation_transcript" "$confirmation_report" \
    "$confirmation_binding_sha256" "$confirmation_transcript_sha256" \
    "$confirmation_report_sha256" <<'PY'
from pathlib import Path
import ctypes
import errno
import hashlib
import json
import os
import stat
import sys

(
    root_text, output_text, mode, candidate_sha, stage_sha, source_commit,
    source_tree_sha, generation, slot_id, native_path, native_sha, main_apk_path,
    main_apk_sha, main_apk_cert, test_apk_path, test_apk_sha, test_apk_cert,
    toolchain_path, toolchain_sha, challenge_hex, challenge_sha, adb_path, serial,
    app_cert, chain_sha, trusted_summary_path, trusted_summary_sha,
    trusted_key_path, trusted_key_sha, signed_evidence_sha, android_sdk, android_ndk,
    java_path, apksigner_path, attestation_root, python_path,
    authority_apksigner_sha, authority_openssl_sha, authority_revocation_sha,
    authority_trust_root_sha_csv,
    confirmation_binding_path, confirmation_transcript_path,
    confirmation_report_path,
    confirmation_binding_sha, confirmation_transcript_sha,
    confirmation_report_sha,
) = sys.argv[1:]
root = Path(root_text)
output = Path(output_text)
sys.path.insert(0, str(root / "scripts"))
from check_android_device_lab_slot import (
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND,
)

def artifact_projection(path_text: str, expected_sha256: str) -> dict | None:
    if not path_text and not expected_sha256:
        return None
    if not path_text or not expected_sha256:
        raise SystemExit("confirmation artifact path and digest must be provided together")
    path = Path(path_text)
    metadata = path.lstat()
    if (
        not path.is_absolute()
        or path.resolve(strict=True) != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_mode & 0o077
        or metadata.st_uid not in {0, os.geteuid()}
    ):
        raise SystemExit(f"confirmation artifact is not one regular file: {path.name}")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    digest = hashlib.sha256()
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        identity = (metadata.st_dev, metadata.st_ino)
        if (
            (opened.st_dev, opened.st_ino) != identity
            or not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or opened.st_size != metadata.st_size
        ):
            raise SystemExit(f"confirmation artifact changed while opening: {path.name}")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        final = path.lstat()
        if (
            (final.st_dev, final.st_ino) != identity
            or final.st_size != metadata.st_size
            or final.st_mtime_ns != metadata.st_mtime_ns
            or final.st_ctime_ns != metadata.st_ctime_ns
        ):
            raise SystemExit(f"confirmation artifact changed while reading: {path.name}")
    finally:
        os.close(descriptor)
    actual_sha256 = digest.hexdigest()
    if actual_sha256 != expected_sha256:
        raise SystemExit(f"confirmation artifact digest changed: {path.name}")
    return {
        "path": str(path.relative_to(root)),
        "size_bytes": metadata.st_size,
        "sha256": actual_sha256,
    }

confirmation_binding = artifact_projection(
    confirmation_binding_path, confirmation_binding_sha
)
confirmation_transcript = artifact_projection(
    confirmation_transcript_path, confirmation_transcript_sha
)
confirmation_comparison = artifact_projection(
    confirmation_report_path, confirmation_report_sha
)

adb_prefix = [adb_path]
if serial:
    adb_prefix += ["-s", serial]
instrument_common = [
    "-e", "kagemushaAttestationChallengeHex", challenge_hex,
    "-e", "kagemushaAttestationChallengeSha256", challenge_sha,
    "-e", "kagemushaAttestationCertificateChainSha256", chain_sha,
    "-e", "kagemushaAppSigningCertificateSha256", app_cert,
    "-e", "kagemushaStrongboxAttestation", "true",
    "-e", "kagemushaPhysicalDeviceAttestation", "true",
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab.test/"
    "androidx.test.runner.AndroidJUnitRunner",
]

def instrument(class_name: str) -> list[str]:
    return adb_prefix + [
        "shell", "am", "instrument", "-w", "-r", "-e", "class", class_name,
        *instrument_common,
    ]

native_command = [
    str(root / "scripts/build_kagemusha_candidate_android_native.sh"),
    "--candidate-sha256", candidate_sha,
    "--stage-sha256", stage_sha,
    "--source-commit", source_commit,
    "--source-tree-sha256", source_tree_sha,
]
gradle_command = [
    "./gradlew", "--no-daemon", "--offline", "--max-workers=2",
    "-PkagemushaCandidateEvidenceLab=true",
    f"-PkagemushaCandidateSha256={candidate_sha}",
    f"-PkagemushaCandidateStageSha256={stage_sha}",
    f"-PkagemushaCandidateEvidenceRoot={root / 'artifacts/kagemusha-candidate-evidence' / candidate_sha / stage_sha}",
    f"-PkagemushaCandidateSourceCommit={source_commit}",
    f"-PkagemushaCandidateSourceTreeSha256={source_tree_sha}",
    f"-PkagemushaCandidateGeneration={generation}",
    f"-PkagemushaCandidateSlotId={slot_id}",
    f"-PkagemushaCandidateLabNativeLibrary={native_path}",
    ":kagemusha-candidate-evidence-lab:stageCandidateLabApk",
    ":kagemusha-candidate-evidence-lab:stageCandidateLabTestApk",
]
validated = mode == "full"
validator_command = None
confirmation_command = None
lifecycle_command = None
export_command = None
if validated:
    authority_root_digests = sorted(
        digest for digest in authority_trust_root_sha_csv.split(",") if digest
    )
    validator_command = [
        python_path, "-I", str(root / "scripts/check_android_device_lab_slot.py"),
        "--root", "<private-attestation-root>",
        "--slot", slot_id,
        "--require-slot",
        "--require-kagemusha-production-evidence",
        "--trusted-signer-public-key", "<trusted-signer-public-key>",
        "--apksigner", "<pinned-apksigner>",
        "--apksigner-sha256", authority_apksigner_sha,
        "--openssl", "<pinned-openssl>",
        "--openssl-sha256", authority_openssl_sha,
        "--android-attestation-revocation-status", "<pinned-revocation-status>",
        "--android-attestation-revocation-status-sha256", authority_revocation_sha,
    ]
    for digest in authority_root_digests:
        validator_command += [
            "--android-attestation-trust-root", "<pinned-attestation-trust-root>",
            "--android-attestation-trust-root-sha256", digest,
        ]
    validator_command += ["--json-out", "<private-validator-summary>"]
    confirmation_command = [
        python_path, "-I", str(root / "scripts/check_android_device_lab_slot.py"),
        "--confirmation-reference-slot", "<private-attestation-slot>",
        "--confirmation-binding", "<pulled-candidate-binding>",
        "--confirmation-lifecycle", "<pulled-lifecycle-transcript>",
        "--confirmation-json-out", "<private-confirmation-report>",
        "--trusted-signer-public-key", "<trusted-signer-public-key>",
        "--apksigner", "<pinned-apksigner>",
        "--apksigner-sha256", authority_apksigner_sha,
        "--openssl", "<pinned-openssl>",
        "--openssl-sha256", authority_openssl_sha,
        "--android-attestation-revocation-status", "<pinned-revocation-status>",
        "--android-attestation-revocation-status-sha256", authority_revocation_sha,
    ]
    for digest in authority_root_digests:
        confirmation_command += [
            "--android-attestation-trust-root", "<pinned-attestation-trust-root>",
            "--android-attestation-trust-root-sha256", digest,
        ]
    lifecycle_command = instrument(
        "org.hyperledger.iroha.sdk.kagemusha.candidate.lab."
        "KagemushaCandidateLifecycleInstrumentedTest"
    )
    export_command = instrument(
        "org.hyperledger.iroha.sdk.kagemusha.candidate.lab."
        "KagemushaCandidateArtifactExportInstrumentedTest"
    )

payload = {
    "schema": "iroha.kagemusha.android_candidate_run_receipt.v1",
    "mode": mode,
    "candidate_record_sha256": candidate_sha,
    "candidate_stage_manifest_sha256": stage_sha,
    "source_commit": source_commit,
    "source_tree_sha256_before": source_tree_sha,
    "source_tree_sha256_after": source_tree_sha,
    "generation": generation,
    "slot_id": slot_id,
    "candidate_lab_native_library_path": str(Path(native_path).relative_to(root)),
    "candidate_lab_native_library_sha256": native_sha,
    "candidate_lab_apk_path": str(Path(main_apk_path).relative_to(root)),
    "candidate_lab_apk_sha256": main_apk_sha,
    "candidate_lab_apk_signing_certificate_sha256": main_apk_cert,
    "candidate_lab_test_apk_path": str(Path(test_apk_path).relative_to(root)),
    "candidate_lab_test_apk_sha256": test_apk_sha,
    "candidate_lab_test_apk_signing_certificate_sha256": test_apk_cert,
    "candidate_build_toolchain_path": str(Path(toolchain_path).relative_to(root)),
    "candidate_build_toolchain_sha256": toolchain_sha,
    "attestation_challenge_hex": challenge_hex,
    "attestation_challenge_sha256": challenge_sha,
    "trusted_slot_cryptographically_validated": validated,
    "trusted_slot_validation_summary_path": None,
    "trusted_slot_validation_summary_sha256": trusted_summary_sha or None,
    "trusted_signer_public_key_sha256": trusted_key_sha or None,
    "signed_evidence_artifact_sha256": signed_evidence_sha or None,
    "app_signing_certificate_sha256": app_cert or None,
    "attestation_certificate_chain_sha256": chain_sha or None,
    "authority_input_sha256": {
        "apksigner": authority_apksigner_sha or None,
        "openssl": authority_openssl_sha or None,
        "android_attestation_revocation_status": authority_revocation_sha or None,
        "android_attestation_trust_roots": (
            sorted(digest for digest in authority_trust_root_sha_csv.split(",") if digest)
            if authority_trust_root_sha_csv else []
        ),
    },
    "confirmation_candidate_binding": confirmation_binding,
    "confirmation_lifecycle_transcript": confirmation_transcript,
    "confirmation_semantic_comparison": confirmation_comparison,
    "raw_test_commands": [
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND,
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND,
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND,
        KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND,
    ],
    "executed_commands": {
        "native_build": {"cwd": str(root), "argv": native_command},
        "gradle_build": {"cwd": str(root / "kotlin"), "argv": gradle_command},
        "trusted_slot_validator": (
            {"cwd": str(root), "redacted_argv_template": validator_command}
            if validator_command else None
        ),
        "confirmation_comparator": (
            {"cwd": str(root), "redacted_argv_template": confirmation_command}
            if confirmation_command else None
        ),
        "lifecycle_instrumentation": (
            {"cwd": str(root), "argv": lifecycle_command} if lifecycle_command else None
        ),
        "export_instrumentation": (
            {"cwd": str(root), "argv": export_command} if export_command else None
        ),
    },
}
encoded = (json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode()

def validate_existing() -> bool:
    try:
        metadata = output.lstat()
    except FileNotFoundError:
        return False
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != 0o600
        or output.stat().st_size != len(encoded)
    ):
        raise SystemExit("refusing to replace a different candidate run receipt")
    with output.open("rb") as handle:
        if handle.read(len(encoded) + 1) != encoded:
            raise SystemExit("refusing to replace a different candidate run receipt")
    return True

def rename_no_replace(source: Path, destination: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    source_bytes = os.fsencode(source)
    destination_bytes = os.fsencode(destination)
    if sys.platform == "darwin" and hasattr(libc, "renamex_np"):
        result = libc.renamex_np(source_bytes, destination_bytes, 0x00000004)
    elif hasattr(libc, "renameat2"):
        result = libc.renameat2(-100, source_bytes, -100, destination_bytes, 1)
    else:
        raise SystemExit("atomic no-replace receipt publication is unsupported")
    if result != 0:
        error = ctypes.get_errno()
        if error == errno.EEXIST and validate_existing():
            return
        raise OSError(error, os.strerror(error), destination)

if not validate_existing():
    temporary = output.with_name(f".{output.name}.{os.getpid()}.tmp")
    try:
        with temporary.open("xb") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, 0o600)
        rename_no_replace(temporary, output)
        directory_fd = os.open(output.parent, os.O_RDONLY)
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
}

apk_signer_sha256() {
  local apk="$1"
  local output digests
  output="$("$APKSIGNER" verify --print-certs "$apk")" || fail \
    "apksigner rejected $apk"
  digests="$(printf '%s\n' "$output" | awk -F': ' \
    '/^Signer #[0-9]+ certificate SHA-256 digest:/ {print tolower($2)}')"
  [[ "$(printf '%s\n' "$digests" | sed '/^$/d' | wc -l | tr -d ' ')" == "1" ]] || fail \
    "$apk must have exactly one current signing certificate"
  digests="$(printf '%s' "$digests" | tr -d ':[:space:]')"
  [[ "$digests" =~ ^[0-9a-f]{64}$ && "$digests" != "$(printf '0%.0s' {1..64})" ]] || fail \
    "$apk signing certificate digest is invalid"
  echo "$digests"
}

LAB_APK_SHA256="$(sha256_file "$LAB_APK")"
TEST_APK_SHA256="$(sha256_file "$TEST_APK")"
[[ "$LAB_APK_SHA256" != "$TEST_APK_SHA256" ]] || fail \
  "main and androidTest APKs must be distinct artifacts"
LAB_APK_CERT_SHA256="$(apk_signer_sha256 "$LAB_APK")"
TEST_APK_CERT_SHA256="$(apk_signer_sha256 "$TEST_APK")"
[[ "$LAB_APK_CERT_SHA256" == "$TEST_APK_CERT_SHA256" ]] || fail \
  "main and androidTest APK signing certificates must match"

CHALLENGE_VALUES="$("$PYTHON3_BINARY" -I - "$ROOT_DIR" "$SLOT_ID" "$CANDIDATE_SHA256" \
  "$EVIDENCE_ROOT/evidence/candidate/manifest-v4.norito" "$STAGE_SHA256" \
  "$NATIVE_LIBRARY" "$LAB_APK_SHA256" "$TEST_APK_SHA256" \
  "$SOURCE_COMMIT" "$SOURCE_TREE_SHA256" <<'PY'
from pathlib import Path
import hashlib
import sys

root = Path(sys.argv[1])
sys.path.insert(0, str(root / "scripts"))
from check_android_device_lab_slot import derive_kagemusha_strongbox_challenge_v1

def digest(path_text):
    result = hashlib.sha256()
    with Path(path_text).open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            result.update(chunk)
    return result.hexdigest()

manifest_sha = digest(sys.argv[4])
native_sha = digest(sys.argv[6])
values = {
    "slot_id": sys.argv[2],
    "candidate_record_sha256": sys.argv[3],
    "candidate_manifest_sha256": manifest_sha,
    "candidate_stage_manifest_sha256": sys.argv[5],
    "candidate_lab_native_library_sha256": native_sha,
    "candidate_lab_apk_sha256": sys.argv[7],
    "candidate_lab_test_apk_sha256": sys.argv[8],
    "candidate_source_commit": sys.argv[9],
    "candidate_source_tree_sha256": sys.argv[10],
}
challenge = derive_kagemusha_strongbox_challenge_v1(values)
print(challenge.hex() + "|" + hashlib.sha256(challenge).hexdigest())
PY
)"
IFS='|' read -r CHALLENGE_HEX CHALLENGE_SHA256 <<<"$CHALLENGE_VALUES"
[[ "$CHALLENGE_HEX" =~ ^[0-9a-f]{64}$ ]] || fail "derived challenge is invalid"
[[ "$CHALLENGE_SHA256" =~ ^[0-9a-f]{64}$ ]] || fail "derived challenge digest is invalid"

verify_host_apks() {
  local label="$1"
  [[ "$(sha256_file "$LAB_APK")" == "$LAB_APK_SHA256" ]] || fail \
    "$label main APK substitution detected"
  [[ "$(sha256_file "$TEST_APK")" == "$TEST_APK_SHA256" ]] || fail \
    "$label androidTest APK substitution detected"
  [[ "$(apk_signer_sha256 "$LAB_APK")" == "$LAB_APK_CERT_SHA256" ]] || fail \
    "$label main APK signer substitution detected"
  [[ "$(apk_signer_sha256 "$TEST_APK")" == "$TEST_APK_CERT_SHA256" ]] || fail \
    "$label androidTest APK signer substitution detected"
}

verify_host_apks after_build
source_snapshot after_build
write_toolchain_audit

echo "[kagemusha-candidate-lab] candidate_lab_apk_sha256=$LAB_APK_SHA256"
echo "[kagemusha-candidate-lab] candidate_lab_test_apk_sha256=$TEST_APK_SHA256"
echo "[kagemusha-candidate-lab] candidate_lab_apk_signing_certificate_sha256=$LAB_APK_CERT_SHA256"
echo "[kagemusha-candidate-lab] candidate_lab_test_apk_signing_certificate_sha256=$TEST_APK_CERT_SHA256"
echo "[kagemusha-candidate-lab] strongbox_challenge_hex=$CHALLENGE_HEX"
echo "[kagemusha-candidate-lab] attestation_challenge_sha256=$CHALLENGE_SHA256"

if [[ "$BUILD_ONLY" == true ]]; then
  write_run_receipt build_only
  echo "[kagemusha-candidate-lab] build-only complete; collect a fresh StrongBox attestation now"
  exit 0
fi

ATTESTATION_ROOT="${ATTESTATION_SLOT%/*}"
[[ -n "$ATTESTATION_ROOT" ]] || ATTESTATION_ROOT="/"
TRUSTED_SLOT_SUMMARY="$BUILD_TMP/trusted-attestation-slot-summary.json"
VALIDATOR_SAFE_PATH="${PYTHON3_BINARY%/*}:${OPENSSL_BINARY%/*}:$JAVA_HOME_RESOLVED/bin:${APKSIGNER%/*}:/usr/bin:/bin"
AUTHORITY_VALIDATOR_ARGS=(
  --apksigner "$AUTHORITY_APKSIGNER"
  --apksigner-sha256 "$AUTHORITY_APKSIGNER_SHA256"
  --openssl "$AUTHORITY_OPENSSL"
  --openssl-sha256 "$AUTHORITY_OPENSSL_SHA256"
  --android-attestation-revocation-status "$AUTHORITY_REVOCATION_STATUS"
  --android-attestation-revocation-status-sha256 "$AUTHORITY_REVOCATION_STATUS_SHA256"
)
for ((authority_index = 0; authority_index < ${#AUTHORITY_TRUST_ROOTS[@]}; authority_index++)); do
  AUTHORITY_VALIDATOR_ARGS+=(
    --android-attestation-trust-root "${AUTHORITY_TRUST_ROOTS[$authority_index]}"
    --android-attestation-trust-root-sha256 "${AUTHORITY_TRUST_ROOT_SHA256[$authority_index]}"
  )
done
/usr/bin/env -i \
  HOME="$HOME" \
  PATH="$VALIDATOR_SAFE_PATH" \
  TMPDIR="${TMPDIR:-/tmp}" \
  LANG="${LANG:-C.UTF-8}" \
  JAVA_HOME="$JAVA_HOME_RESOLVED" \
  ANDROID_HOME="$ANDROID_SDK_RESOLVED" \
  ANDROID_SDK_ROOT="$ANDROID_SDK_RESOLVED" \
  "$PYTHON3_BINARY" -I "$ROOT_DIR/scripts/check_android_device_lab_slot.py" \
    --root "$ATTESTATION_ROOT" \
    --slot "$SLOT_ID" \
    --require-slot \
    --require-kagemusha-production-evidence \
    --trusted-signer-public-key "$TRUSTED_SIGNER_PUBLIC_KEY" \
    "${AUTHORITY_VALIDATOR_ARGS[@]}" \
    --json-out "$TRUSTED_SLOT_SUMMARY" || fail \
      "authoritative cryptographic validation rejected the raw attestation slot"
verify_attestation_authority_inputs

ATTESTATION_VALUES="$("$PYTHON3_BINARY" -I - "$TRUSTED_SLOT_SUMMARY" "$SLOT_ID" \
  "$CHALLENGE_SHA256" "$CANDIDATE_SHA256" \
  "$(sha256_file "$EVIDENCE_ROOT/evidence/candidate/manifest-v4.norito")" \
  "$STAGE_SHA256" "$LAB_APK_SHA256" "$TEST_APK_SHA256" \
  "$(sha256_file "$NATIVE_LIBRARY")" "$SOURCE_COMMIT" \
  "$SOURCE_TREE_SHA256" "$LAB_APK_CERT_SHA256" "$TEST_APK_CERT_SHA256" <<'PY'
from pathlib import Path
import json
import re
import sys

summary_path = Path(sys.argv[1])
if summary_path.stat().st_size > 16 * 1024 * 1024:
    raise SystemExit("trusted validator summary exceeds 16 MiB")
with summary_path.open("rb") as handle:
    summary_bytes = handle.read(16 * 1024 * 1024 + 1)
if len(summary_bytes) > 16 * 1024 * 1024:
    raise SystemExit("trusted validator summary grew while reading")
summary = json.loads(summary_bytes)
if summary.get("ok") != 1 or summary.get("failed") != 0:
    raise SystemExit("trusted validator summary does not contain one successful slot")
slots = summary.get("slots")
if not isinstance(slots, list) or len(slots) != 1:
    raise SystemExit("trusted validator summary must contain exactly one slot")
slot = slots[0]
if slot.get("slot") != sys.argv[2] or slot.get("status") != "ok" or slot.get("errors") != []:
    raise SystemExit("trusted validator summary slot identity/status is not exact")
kagemusha = slot.get("kagemusha")
if not isinstance(kagemusha, dict) or kagemusha.get("required") is not True:
    raise SystemExit("trusted validator summary omitted required Kagemusha evidence")
expected = {
    "attestation_challenge_sha256": sys.argv[3],
    "candidate_record_sha256": sys.argv[4],
    "candidate_manifest_sha256": sys.argv[5],
    "candidate_stage_manifest_path": "candidate-stage-manifest-v1.json",
    "candidate_stage_manifest_sha256": sys.argv[6],
    "candidate_lab_apk_sha256": sys.argv[7],
    "candidate_lab_test_apk_sha256": sys.argv[8],
    "candidate_lab_native_library_sha256": sys.argv[9],
    "candidate_source_commit": sys.argv[10],
    "candidate_source_tree_sha256": sys.argv[11],
    "candidate_source_tree_sha256_before": sys.argv[11],
    "candidate_source_tree_sha256_after": sys.argv[11],
    "candidate_lab_apk_signing_certificate_sha256": sys.argv[12],
    "candidate_lab_test_apk_signing_certificate_sha256": sys.argv[13],
    "strongbox_attestation": True,
    "physical_device_attestation": True,
}
for key, value in expected.items():
    if kagemusha.get(key) != value:
        raise SystemExit(f"trusted validator summary {key} differs from the exact candidate run")
outputs = []
for key in (
    "app_signing_certificate_sha256",
    "attestation_certificate_chain_sha256",
    "signed_evidence_signer_public_key_sha256",
    "signed_evidence_artifact_sha256",
):
    value = kagemusha.get(key)
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None or value == "0" * 64:
        raise SystemExit(f"trusted validator summary {key} is invalid")
    outputs.append(value)
print("|".join(outputs))
PY
)"
IFS='|' read -r APP_SIGNING_CERTIFICATE_SHA256 ATTESTATION_CERTIFICATE_CHAIN_SHA256 \
  TRUSTED_SIGNER_PUBLIC_KEY_SHA256 SIGNED_EVIDENCE_ARTIFACT_SHA256 \
  <<<"$ATTESTATION_VALUES"
[[ "$APP_SIGNING_CERTIFICATE_SHA256" != "$LAB_APK_CERT_SHA256" ]] || fail \
  "non-shipping lab APK signer must differ from the attested wallet signer"

ADB=("$ADB_BINARY")
if [[ -n "$SERIAL" ]]; then
  ADB+=( -s "$SERIAL" )
fi
[[ "$("${ADB[@]}" get-state 2>/dev/null || true)" == "device" ]] || fail \
  "one authorized adb device (or exact --serial) is required"
DEVICE_QEMU="$("${ADB[@]}" shell getprop ro.kernel.qemu)"
DEVICE_QEMU="${DEVICE_QEMU//$'\r'/}"
[[ "$DEVICE_QEMU" != "1" ]] || fail \
  "emulator evidence is forbidden"
DEVICE_ABI="$("${ADB[@]}" shell getprop ro.product.cpu.abi)"
DEVICE_ABI="${DEVICE_ABI//$'\r'/}"
[[ "$DEVICE_ABI" == "arm64-v8a" ]] || fail \
  "physical device must use arm64-v8a"

RUN_TMP="$(mktemp -d -t kagemusha-candidate-lab.XXXXXX)"
DEVICE_CLEANED=false

cleanup_device_state() {
  set +e
  "${ADB[@]}" shell am force-stop "$TEST_PACKAGE" >/dev/null 2>&1
  "${ADB[@]}" shell am force-stop "$PACKAGE" >/dev/null 2>&1
  "${ADB[@]}" shell pm clear "$TEST_PACKAGE" >/dev/null 2>&1
  "${ADB[@]}" shell pm clear "$PACKAGE" >/dev/null 2>&1
  "${ADB[@]}" uninstall "$TEST_PACKAGE" >/dev/null 2>&1
  "${ADB[@]}" uninstall "$PACKAGE" >/dev/null 2>&1
  set -e
  DEVICE_CLEANED=true
}

on_exit() {
  local status=$?
  if [[ "$DEVICE_CLEANED" != true ]]; then
    cleanup_device_state
  fi
  rm -rf "$RUN_TMP"
  rm -rf "$BUILD_TMP"
  trap - EXIT
  exit "$status"
}
trap on_exit EXIT

# Remove only stale instances of the two lab packages. Never signal unrelated
# applications, builds, shells, or Codex processes.
cleanup_device_state
DEVICE_CLEANED=false

verify_host_apks immediately_before_install
"${ADB[@]}" install -r -t "$LAB_APK" >/dev/null
"${ADB[@]}" install -r -t "$TEST_APK" >/dev/null

device_apk_sha256() {
  local package_name="$1"
  local paths path output
  paths="$("${ADB[@]}" shell pm path "$package_name")"
  paths="${paths//$'\r'/}"
  [[ -n "$paths" && "$paths" != *$'\n'* ]] || fail \
    "$package_name must resolve to exactly one installed base APK"
  path="${paths#package:}"
  [[ "$path" =~ ^/data/app/[A-Za-z0-9._~+=/-]+/base\.apk$ ]] || fail \
    "$package_name installed APK path is unsafe or unexpected: $path"
  output="$("${ADB[@]}" shell sha256sum "$path")"
  output="${output//$'\r'/}"
  output="${output%% *}"
  [[ "$output" =~ ^[0-9a-f]{64}$ ]] || fail \
    "$package_name installed APK digest output is invalid"
  printf '%s\n' "$output"
}

verify_installed_apks() {
  local label="$1"
  verify_host_apks "$label"
  [[ "$(device_apk_sha256 "$PACKAGE")" == "$LAB_APK_SHA256" ]] || fail \
    "$label installed main APK differs from the retained host APK"
  [[ "$(device_apk_sha256 "$TEST_PACKAGE")" == "$TEST_APK_SHA256" ]] || fail \
    "$label installed androidTest APK differs from the retained host APK"
}

verify_installed_apks immediately_after_install
"${ADB[@]}" shell pm clear "$PACKAGE" >/dev/null

mkdir -p "$EVIDENCE_ROOT/evidence"
LIFECYCLE_COMMAND=(
  "${ADB[@]}" shell am instrument -w -r
  -e class "$LIFECYCLE_CLASS"
  -e kagemushaAttestationChallengeHex "$CHALLENGE_HEX"
  -e kagemushaAttestationChallengeSha256 "$CHALLENGE_SHA256"
  -e kagemushaAttestationCertificateChainSha256 "$ATTESTATION_CERTIFICATE_CHAIN_SHA256"
  -e kagemushaAppSigningCertificateSha256 "$APP_SIGNING_CERTIFICATE_SHA256"
  -e kagemushaStrongboxAttestation true
  -e kagemushaPhysicalDeviceAttestation true
  "$TEST_PACKAGE/$RUNNER"
)
EXPORT_COMMAND=(
  "${ADB[@]}" shell am instrument -w -r
  -e class "$EXPORT_CLASS"
  -e kagemushaAttestationChallengeHex "$CHALLENGE_HEX"
  -e kagemushaAttestationChallengeSha256 "$CHALLENGE_SHA256"
  -e kagemushaAttestationCertificateChainSha256 "$ATTESTATION_CERTIFICATE_CHAIN_SHA256"
  -e kagemushaAppSigningCertificateSha256 "$APP_SIGNING_CERTIFICATE_SHA256"
  -e kagemushaStrongboxAttestation true
  -e kagemushaPhysicalDeviceAttestation true
  "$TEST_PACKAGE/$RUNNER"
)
run_instrumentation() {
  local class_name="$1"
  local log_file="$2"
  shift 2
  local -a command=("$@")
  verify_installed_apks "before_${class_name##*.}"
  printf '[kagemusha-candidate-lab] command:'
  printf ' %q' "${command[@]}"
  printf '\n'
  set +e
  "${command[@]}" | /usr/bin/tee "$log_file"
  local instrument_status=${PIPESTATUS[0]}
  set -e
  [[ $instrument_status -eq 0 ]] || fail "instrumentation command failed: $class_name"
  /usr/bin/grep -q '^OK (1 test)$' "$log_file" || fail \
    "instrumentation failed: $class_name"
  verify_installed_apks "after_${class_name##*.}"
}

run_instrumentation \
  "$LIFECYCLE_CLASS" \
  "$EVIDENCE_ROOT/evidence/candidate-lifecycle-instrumentation.log" \
  "${LIFECYCLE_COMMAND[@]}"
run_instrumentation \
  "$EXPORT_CLASS" \
  "$EVIDENCE_ROOT/evidence/candidate-export-instrumentation.log" \
  "${EXPORT_COMMAND[@]}"

"${ADB[@]}" pull \
  "$REMOTE_EVIDENCE/lifecycle-transcript-v2.json" \
  "$EVIDENCE_ROOT/evidence/lifecycle-transcript-v2.json" >/dev/null
"${ADB[@]}" pull \
  "$REMOTE_EVIDENCE/candidate-binding-v2.json" \
  "$EVIDENCE_ROOT/evidence/candidate-binding-v2.json" >/dev/null

TRANSCRIPT="$EVIDENCE_ROOT/evidence/lifecycle-transcript-v2.json"
BINDING="$EVIDENCE_ROOT/evidence/candidate-binding-v2.json"
for output in "$TRANSCRIPT" "$BINDING"; do
  [[ -s "$output" && ! -L "$output" ]] || fail "device did not export regular evidence: $output"
done
verify_installed_apks after_evidence_pull
verify_host_apks before_confirmation_comparison
verify_attestation_authority_inputs
CONFIRMATION_REPORT="$EVIDENCE_ROOT/evidence/candidate-confirmation-comparison-v1.json"
if [[ -e "$CONFIRMATION_REPORT" || -L "$CONFIRMATION_REPORT" ]]; then
  fail "refusing to replace an existing candidate confirmation comparison report"
fi
/usr/bin/env -i \
  HOME="$HOME" \
  PATH="$VALIDATOR_SAFE_PATH" \
  TMPDIR="${TMPDIR:-/tmp}" \
  LANG="${LANG:-C.UTF-8}" \
  JAVA_HOME="$JAVA_HOME_RESOLVED" \
  ANDROID_HOME="$ANDROID_SDK_RESOLVED" \
  ANDROID_SDK_ROOT="$ANDROID_SDK_RESOLVED" \
  "$PYTHON3_BINARY" -I "$ROOT_DIR/scripts/check_android_device_lab_slot.py" \
    --confirmation-reference-slot "$ATTESTATION_SLOT" \
    --confirmation-binding "$BINDING" \
    --confirmation-lifecycle "$TRANSCRIPT" \
    --confirmation-json-out "$CONFIRMATION_REPORT" \
    --trusted-signer-public-key "$TRUSTED_SIGNER_PUBLIC_KEY" \
    "${AUTHORITY_VALIDATOR_ARGS[@]}" || fail \
      "independent candidate confirmation differs from the authenticated reference"
verify_attestation_authority_inputs

CONFIRMATION_VALUES="$("$PYTHON3_BINARY" -I - \
  "$CONFIRMATION_REPORT" "$BINDING" "$TRANSCRIPT" "$ATTESTATION_SLOT" \
  "$AUTHORITY_APKSIGNER_SHA256" "$AUTHORITY_OPENSSL_SHA256" \
  "$AUTHORITY_REVOCATION_STATUS_SHA256" \
  "$(IFS=,; printf '%s' "${AUTHORITY_TRUST_ROOT_SHA256[*]}")" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import stat
import sys

(
    report_text,
    binding_text,
    transcript_text,
    reference_slot_text,
    apksigner_sha256,
    openssl_sha256,
    revocation_sha256,
    root_sha256_csv,
) = sys.argv[1:]


def strict_object(raw: bytes, label: str) -> dict:
    def object_pairs(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"{label} contains duplicate JSON field {key}")
            result[key] = value
        return result

    def invalid_constant(value):
        raise ValueError(f"{label} contains non-finite number {value}")

    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=object_pairs,
            parse_constant=invalid_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise SystemExit(f"{label} is not strict JSON: {error}") from error
    if not isinstance(value, dict):
        raise SystemExit(f"{label} must be a JSON object")
    return value


def read_private_json(path: Path, label: str) -> tuple[dict, dict]:
    try:
        canonical = path.resolve(strict=True)
        initial = path.lstat()
    except OSError as error:
        raise SystemExit(f"{label} could not be inspected") from error
    if (
        not path.is_absolute()
        or canonical != path
        or stat.S_ISLNK(initial.st_mode)
        or not stat.S_ISREG(initial.st_mode)
        or initial.st_nlink != 1
        or initial.st_uid not in {0, os.geteuid()}
        or initial.st_mode & 0o077
        or initial.st_size <= 0
    ):
        raise SystemExit(f"{label} must be one canonical owner-private regular file")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    chunks = []
    digest = hashlib.sha256()
    try:
        opened = os.fstat(descriptor)
        identity = (initial.st_dev, initial.st_ino)
        if (
            (opened.st_dev, opened.st_ino) != identity
            or not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or opened.st_size != initial.st_size
        ):
            raise SystemExit(f"{label} changed while being opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
            digest.update(chunk)
        final = path.lstat()
        if (
            (final.st_dev, final.st_ino) != identity
            or final.st_size != initial.st_size
            or final.st_mtime_ns != initial.st_mtime_ns
            or final.st_ctime_ns != initial.st_ctime_ns
        ):
            raise SystemExit(f"{label} changed while being read")
    finally:
        os.close(descriptor)
    raw = b"".join(chunks)
    return strict_object(raw, label), {
        "path": str(path),
        "size_bytes": len(raw),
        "sha256": digest.hexdigest(),
    }


report_path = Path(report_text)
binding_path = Path(binding_text)
transcript_path = Path(transcript_text)
reference_slot = Path(reference_slot_text)
report, report_measurement = read_private_json(
    report_path, "candidate confirmation comparison report"
)
if set(report) != {
    "schema", "status", "errors", "artifacts", "comparison", "authority_tools"
}:
    raise SystemExit("candidate confirmation comparison report fields are not exact")
if report.get("schema") != "iroha.android.device_lab.kagemusha.confirmation_comparison.v1":
    raise SystemExit("candidate confirmation comparison report schema is not V1")
if report.get("status") != "ok" or report.get("errors") != []:
    raise SystemExit("candidate confirmation comparison report is not successful")
if report.get("comparison") != {
    "deterministic_fields_equal": True,
    "only_duration_nanos_may_differ": True,
}:
    raise SystemExit("candidate confirmation comparison policy/result is not exact")
expected_authority = {
    "apksigner_sha256": apksigner_sha256,
    "openssl_sha256": openssl_sha256,
    "attestation_trust_root_sha256": sorted(root_sha256_csv.split(",")),
    "attestation_revocation_status_sha256": revocation_sha256,
}
if report.get("authority_tools") != expected_authority:
    raise SystemExit("candidate confirmation report authority projection differs")

artifacts = report.get("artifacts")
if not isinstance(artifacts, dict) or set(artifacts) != {
    "reference_binding",
    "reference_lifecycle",
    "confirmation_binding",
    "confirmation_lifecycle",
}:
    raise SystemExit("candidate confirmation comparison artifact set is not exact")
confirmation_paths = {
    "confirmation_binding": binding_path,
    "confirmation_lifecycle": transcript_path,
}
measurements = {}
for name, record in artifacts.items():
    if not isinstance(record, dict) or set(record) != {"path", "size_bytes", "sha256"}:
        raise SystemExit(f"candidate confirmation report {name} measurement is not exact")
    artifact_path_text = record.get("path")
    if not isinstance(artifact_path_text, str):
        raise SystemExit(f"candidate confirmation report {name} path is invalid")
    artifact_path = Path(artifact_path_text)
    if name in confirmation_paths:
        if artifact_path != confirmation_paths[name]:
            raise SystemExit(f"candidate confirmation report {name} path differs")
    else:
        try:
            artifact_path.relative_to(reference_slot)
        except ValueError as error:
            raise SystemExit(
                f"candidate confirmation report {name} escapes the authenticated reference slot"
            ) from error
    _, measurement = read_private_json(
        artifact_path, f"candidate confirmation report {name} artifact"
    )
    if measurement != record:
        raise SystemExit(f"candidate confirmation report {name} measurement differs")
    measurements[name] = measurement

print(
    "|".join(
        (
            measurements["confirmation_binding"]["sha256"],
            measurements["confirmation_lifecycle"]["sha256"],
            report_measurement["sha256"],
        )
    )
)
PY
)"
IFS='|' read -r CONFIRMATION_BINDING_SHA256 CONFIRMATION_TRANSCRIPT_SHA256 \
  CONFIRMATION_REPORT_SHA256 <<<"$CONFIRMATION_VALUES"
for confirmation_digest in \
  "$CONFIRMATION_BINDING_SHA256" \
  "$CONFIRMATION_TRANSCRIPT_SHA256" \
  "$CONFIRMATION_REPORT_SHA256"; do
  [[ "$confirmation_digest" =~ ^[0-9a-f]{64}$ ]] || fail \
    "authoritative candidate confirmation produced an invalid artifact digest"
done

verify_host_apks after_confirmation_comparison
cleanup_device_state
source_snapshot after
verify_attestation_authority_inputs
TRUSTED_SLOT_SUMMARY_SHA256="$(sha256_file "$TRUSTED_SLOT_SUMMARY")"
AUTHORITY_TRUST_ROOT_SHA256_CSV="$(
  IFS=,
  printf '%s' "${AUTHORITY_TRUST_ROOT_SHA256[*]}"
)"
write_run_receipt \
  full \
  "$TRUSTED_SLOT_SUMMARY" \
  "$TRUSTED_SLOT_SUMMARY_SHA256" \
  "$TRUSTED_SIGNER_PUBLIC_KEY_SHA256" \
  "$SIGNED_EVIDENCE_ARTIFACT_SHA256" \
  "$APP_SIGNING_CERTIFICATE_SHA256" \
  "$ATTESTATION_CERTIFICATE_CHAIN_SHA256" \
  "$ATTESTATION_ROOT" \
  "$AUTHORITY_APKSIGNER_SHA256" \
  "$AUTHORITY_OPENSSL_SHA256" \
  "$AUTHORITY_REVOCATION_STATUS_SHA256" \
  "$AUTHORITY_TRUST_ROOT_SHA256_CSV" \
  "$BINDING" \
  "$TRANSCRIPT" \
  "$CONFIRMATION_REPORT" \
  "$CONFIRMATION_BINDING_SHA256" \
  "$CONFIRMATION_TRANSCRIPT_SHA256" \
  "$CONFIRMATION_REPORT_SHA256"

echo "[kagemusha-candidate-lab] observed physical-device evidence exported under $EVIDENCE_ROOT/evidence"
