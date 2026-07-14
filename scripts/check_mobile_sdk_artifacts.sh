#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_mobile_sdk_artifacts.sh [--root <repo-root>] [--apple-only|--android-only] [--require-built-android] [--allow-dirty-source]

Checks that the Iroha mobile SDK packaging surface is ready for wallet
integration:
  - SwiftPM package manifest and NoritoBridge binary target exist.
  - NoritoBridge.xcframework contains iOS device, iOS simulator, and macOS slices.
  - NoritoBridge.artifacts.json records per-slice SHA-256 hashes and the
    privacy-production feature state, which must match the XCFramework marker.
  - Every manifest hash matches the actual slice, all headers are identical,
    and the manifest ABI/source fingerprint matches the checked-out bridge.
  - Apple archives contain their declared architectures and the complete
    Kagemusha recursive-spend symbol surface.
  - Kotlin/Android SDK modules are included and publishable.

By default Android build outputs are not required. Pass --require-built-android
or set MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS=1 to require jar/aar outputs too.
By default both Apple and Android packaging surfaces are checked. Pass
--apple-only or --android-only when platform artifact builds run in separate CI
jobs.
Dirty bridge inputs are rejected by default. --allow-dirty-source (or
MOBILE_SDK_ALLOW_DIRTY_SOURCE=1) permits a local integration artifact only when
its manifest dirty bit and exact dependency-closure fingerprint match.
USAGE
}

ROOT_ARG=""
REQUIRE_ANDROID_OUTPUTS="${MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS:-0}"
ALLOW_DIRTY_SOURCE="${MOBILE_SDK_ALLOW_DIRTY_SOURCE:-0}"
CHECK_APPLE=1
CHECK_ANDROID=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      if [[ $# -eq 0 ]]; then
        echo "[mobile-sdk-artifacts] ERROR: --root requires a value" >&2
        exit 64
      fi
      ROOT_ARG="$1"
      ;;
    --root=*)
      ROOT_ARG="${1#*=}"
      ;;
    --require-built-android)
      REQUIRE_ANDROID_OUTPUTS=1
      ;;
    --allow-dirty-source)
      ALLOW_DIRTY_SOURCE=1
      ;;
    --apple-only)
      CHECK_APPLE=1
      CHECK_ANDROID=0
      ;;
    --android-only)
      CHECK_APPLE=0
      CHECK_ANDROID=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      if [[ -z "$ROOT_ARG" ]]; then
        ROOT_ARG="$1"
      else
        echo "[mobile-sdk-artifacts] ERROR: unexpected argument: $1" >&2
        usage >&2
        exit 64
      fi
      ;;
  esac
  shift
done

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi

if [[ ! -d "$ROOT_ARG" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: repo root does not exist: $ROOT_ARG" >&2
  exit 66
fi

ROOT_DIR="$(cd "$ROOT_ARG" && pwd)"
FAILURES=0

# The first mobile release is one exact ABI-20/V4 contract. Keep the complete
# Kagemusha C export allow-list here so Apple archives, Android shared objects,
# checked-out Rust, and the checked-in header are all compared against the same
# surface. V2 suffixes below are supporting request/finality primitives used by
# the V4 lifecycle; V1/V2/V3 lifecycle and V3 artifact entrypoints are retired.
KAGEMUSHA_C_SYMBOLS=(
  connect_norito_kagemusha_recursive_spend_capabilities_v4
  connect_norito_kagemusha_topup_finality_verify_v2
  connect_norito_kagemusha_topup_shield_build_unsigned_v2
  connect_norito_kagemusha_recursive_spend_artifact_begin_v4
  connect_norito_kagemusha_recursive_spend_artifact_write_v4
  connect_norito_kagemusha_recursive_spend_artifact_finalize_v4
  connect_norito_kagemusha_recursive_spend_artifact_cancel_v4
  connect_norito_kagemusha_recursive_spend_artifact_set_install_v4
  connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4
  connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4
  connect_norito_kagemusha_recursive_spend_init_v4
  connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2
  connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2
  connect_norito_kagemusha_recursive_spend_topup_v2
  connect_norito_kagemusha_recursive_spend_append_v4
  connect_norito_kagemusha_recursive_spend_verify_v4
  connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2
  connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2
  connect_norito_kagemusha_recursive_spend_redeem_v4
  connect_norito_kagemusha_receiver_key_reference_v2
  connect_norito_kagemusha_recipient_output_derive_v2
  connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2
  connect_norito_kagemusha_recipient_payment_request_create_v2
  connect_norito_kagemusha_recipient_payment_request_verify_v2
  connect_norito_kagemusha_request_authorization_signing_bytes_v2
  connect_norito_kagemusha_request_authorization_create_v2
  connect_norito_kagemusha_receiver_acknowledgement_payload_v2
  connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2
  connect_norito_kagemusha_receiver_acknowledgement_create_v2
  connect_norito_kagemusha_receiver_acknowledgement_verify_v2
  connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2
  connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2
  connect_norito_kagemusha_recursive_spend_bundle_summary_v2
  connect_norito_kagemusha_recursive_spend_build_split_intent_v2
)

REQUIRED_BRIDGE_SYMBOLS=(
  connect_norito_bridge_abi_version
  connect_norito_detached_transaction_scaffold_inspect_v1
  connect_norito_detached_transaction_scaffold_finalize_ed25519_v1
  connect_norito_canonical_json_blake3_v1
  "${KAGEMUSHA_C_SYMBOLS[@]}"
)

# Exact JNI allow-list for each supported Java namespace. As with the C list,
# V2 names retained here are non-lifecycle support primitives consumed by V4.
KAGEMUSHA_JNI_METHODS=(
  nativeAppendSpendV4
  nativeArtifactBeginV4
  nativeArtifactCancelV4
  nativeArtifactFinalizeV4
  nativeArtifactSetInstallV4
  nativeArtifactSetIsInstalledV4
  nativeArtifactSetUninstallV4
  nativeArtifactWriteV4
  nativeBranchClaimsConflictV2
  nativeBridgeAbiVersion
  nativeBuildAppendRequestV4
  nativeBuildInitRequestV4
  nativeBuildOutputMembershipPathsV4
  nativeBuildRedeemRequestV4
  nativeBuildRedeemV4
  nativeBuildVerifyRequestV4
  nativeCreateAcknowledgementV2
  nativeCreateAuthorizationV2
  nativeCreateRecipientRequestV2
  nativeFinalizeRedeemV2
  nativeFinalizeTopUpV2
  nativeInitSpendV4
  nativePastaCycleV4BackendAvailable
  nativePrepareAcknowledgementV2
  nativePrepareAuthorizationV2
  nativePrepareNoteOpeningV2
  nativePrepareRecipientRequestV2
  nativePrepareTopUpV2
  nativeProjectActiveVerifierV2
  nativeProjectOperationStatusV2
  nativeProjectPeerPaymentV2
  nativeProjectReadinessV2
  nativeProjectRecipientRequestV2
  nativeProjectRedeemBuildResultV2
  nativeProjectSplitResultV2
  nativeProjectVerifyResultV2
  nativeVerifyAcknowledgementV2
  nativeVerifyRecipientRequestV2
  nativeVerifySpendV4
)

relpath() {
  local path="$1"
  case "$path" in
    "$ROOT_DIR"/*) printf '%s' "${path#$ROOT_DIR/}" ;;
    *) printf '%s' "$path" ;;
  esac
}

fail() {
  printf '[mobile-sdk-artifacts] ERROR: %s\n' "$*" >&2
  FAILURES=1
}

require_file() {
  local path="$1"
  local label="$2"
  if [[ ! -f "$path" ]]; then
    fail "missing $label: $(relpath "$path")"
  fi
}

require_dir() {
  local path="$1"
  local label="$2"
  if [[ ! -d "$path" ]]; then
    fail "missing $label: $(relpath "$path")"
  fi
}

require_literal() {
  local path="$1"
  local literal="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    fail "cannot inspect missing $label file: $(relpath "$path")"
    return
  fi
  if ! grep -Fq -- "$literal" "$path"; then
    fail "$label not found in $(relpath "$path")"
  fi
}

require_regex() {
  local path="$1"
  local pattern="$2"
  local label="$3"
  if [[ ! -f "$path" ]]; then
    fail "cannot inspect missing $label file: $(relpath "$path")"
    return
  fi
  if ! grep -Eq -- "$pattern" "$path"; then
    fail "$label not found in $(relpath "$path")"
  fi
}

require_glob() {
  local pattern="$1"
  local label="$2"
  local matches=()
  while IFS= read -r match; do
    matches+=("$match")
  done < <(compgen -G "$pattern" || true)
  if [[ ${#matches[@]} -eq 0 ]]; then
    fail "missing $label: $pattern"
  fi
}

require_zip_entry() {
  local archive="$1"
  local entry="$2"
  local label="$3"
  local entries

  if [[ ! -f "$archive" ]]; then
    fail "cannot inspect missing $label: $(relpath "$archive")"
    return
  fi
  if ! command -v unzip >/dev/null 2>&1; then
    fail "unzip is required to inspect $label"
    return
  fi
  if ! entries="$(unzip -Z1 "$archive" 2>/dev/null)"; then
    fail "$label is not a readable ZIP/AAR archive: $(relpath "$archive")"
    return
  fi
  if ! grep -Fxq -- "$entry" <<<"$entries"; then
    fail "$label missing ZIP entry $entry in $(relpath "$archive")"
  fi
}

plist_contains() {
  local plist="$1"
  local needle="$2"
  if [[ ! -f "$plist" ]]; then
    return 1
  fi
  if grep -Fq -- "$needle" "$plist"; then
    return 0
  fi
  if command -v plutil >/dev/null 2>&1 && plutil -p "$plist" 2>/dev/null | grep -Fq -- "$needle"; then
    return 0
  fi
  return 1
}

hash_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    sha256sum "$path" | awk '{print $1}'
  fi
}

hash_zip_entry() {
  local archive="$1"
  local entry="$2"
  if command -v shasum >/dev/null 2>&1; then
    unzip -p "$archive" "$entry" | shasum -a 256 | awk '{print $1}'
  else
    unzip -p "$archive" "$entry" | sha256sum | awk '{print $1}'
  fi
}

manifest_json_value() {
  local manifest="$1"
  local key="$2"
  python3 - "$manifest" "$key" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    value = json.load(handle)
for component in sys.argv[2].split("."):
    value = value[component]
if isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
PY
}

bridge_source_fingerprint() {
  python3 "$ROOT_DIR/scripts/norito_bridge_source_seal.py" \
    fingerprint --root "$ROOT_DIR"
}

check_bridge_source_contract() {
  local bridge_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
  local bridge_header="$ROOT_DIR/crates/connect_norito_bridge/include/connect_norito_bridge.h"

  # Packaged artifacts can be checked outside a source checkout. When source is
  # present, however, refuse to certify a build whose callable Kagemusha ABI is
  # broader or narrower than the exact first-release allow-list.
  if [[ -f "$bridge_source" ]]; then
    if ! python3 - "$bridge_source" "${KAGEMUSHA_C_SYMBOLS[@]}" <<'PY'
import re
import sys

path = sys.argv[1]
expected = set(sys.argv[2:])
text = open(path, "r", encoding="utf-8").read()
abi = re.search(
    r"CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;",
    text,
)
actual = set(re.findall(
    r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+'
    r'(connect_norito_kagemusha_[A-Za-z0-9_]+)\s*\(',
    text,
))
errors = []
if abi is None or abi.group(1) != "20":
    errors.append("bridge source does not declare exact ABI 20")
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    errors.append("missing Kagemusha C exports: " + ", ".join(missing))
if retired_or_extra:
    errors.append("retired or unexpected Kagemusha C exports: " + ", ".join(retired_or_extra))
for error in errors:
    print(f"[mobile-sdk-artifacts] ERROR: {error}", file=sys.stderr)
raise SystemExit(1 if errors else 0)
PY
    then
      FAILURES=1
    fi
  fi

  if [[ -f "$bridge_header" ]]; then
    if ! python3 - "$bridge_header" "${KAGEMUSHA_C_SYMBOLS[@]}" <<'PY'
import re
import sys

path = sys.argv[1]
expected = set(sys.argv[2:])
text = open(path, "r", encoding="utf-8").read()
actual = set(re.findall(
    r'\b(connect_norito_kagemusha_[A-Za-z0-9_]+)\s*\(',
    text,
))
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    print(
        "[mobile-sdk-artifacts] ERROR: bridge header is missing Kagemusha declarations: "
        + ", ".join(missing),
        file=sys.stderr,
    )
if retired_or_extra:
    print(
        "[mobile-sdk-artifacts] ERROR: bridge header exposes retired or unexpected "
        "Kagemusha declarations: " + ", ".join(retired_or_extra),
        file=sys.stderr,
    )
raise SystemExit(1 if missing or retired_or_extra else 0)
PY
    then
      FAILURES=1
    fi
  fi
}

check_swift_kagemusha_source_contract() {
  local source_dir="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift"
  [[ -d "$source_dir" ]] || return

  if ! python3 - "$source_dir" "${KAGEMUSHA_C_SYMBOLS[@]}" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
expected_symbols = set(sys.argv[2:])
files = sorted(root.glob("*.swift"))
text = "\n".join(path.read_text(encoding="utf-8") for path in files)
expected_wrappers = {
    "appendSpendV4",
    "buildRedeemV4",
    "ensureProofBackendAvailableV4",
    "initSpendV4",
    "verifySpendV4",
}
expected_native_lifecycle = {
    "kagemushaRecursiveSpendAppendV4",
    "kagemushaRecursiveSpendArtifactBeginV4",
    "kagemushaRecursiveSpendArtifactCancelV4",
    "kagemushaRecursiveSpendArtifactFinalizeV4",
    "kagemushaRecursiveSpendArtifactSetInstallV4",
    "kagemushaRecursiveSpendArtifactSetIsInstalledV4",
    "kagemushaRecursiveSpendArtifactSetUninstallV4",
    "kagemushaRecursiveSpendArtifactWriteV4",
    "kagemushaRecursiveSpendCapabilitiesV4",
    "kagemushaRecursiveSpendInitV4",
    "kagemushaRecursiveSpendRedeemV4",
    "kagemushaRecursiveSpendVerifyV4",
}
actual_symbols = set(re.findall(
    r'"(connect_norito_kagemusha_[a-z0-9_]+)"',
    text,
))
actual_wrappers = set(re.findall(
    r"\bfunc\s+((?:ensureProofBackendAvailable|initSpend|appendSpend|verifySpend|"
    r"buildRedeem)V[0-9]+)\s*\(",
    text,
))
actual_native_lifecycle = set(re.findall(
    r"\bfunc\s+(kagemushaRecursiveSpend(?:Capabilities|Init|Append|Verify|Redeem|"
    r"Artifact(?:Begin|Write|Finalize|Cancel|SetInstall|SetIsInstalled|SetUninstall))"
    r"V[0-9]+)\s*\(",
    text,
))
inventories = (
    ("native symbol", actual_symbols, expected_symbols),
    ("lifecycle wrapper", actual_wrappers, expected_wrappers),
    ("native lifecycle binding", actual_native_lifecycle, expected_native_lifecycle),
)
errors = []
for label, actual, expected in inventories:
    missing = sorted(expected - actual)
    retired_or_extra = sorted(actual - expected)
    if missing or retired_or_extra:
        errors.append(
            f"Swift Kagemusha {label} inventory is not exact ABI-20/V4 "
            f"(missing={missing}, retired_or_unexpected={retired_or_extra})"
        )
if re.search(r"\bpublic\s+(?:struct|enum|class|typealias|protocol)\s+[A-Za-z0-9_]*V3\b", text):
    errors.append("Swift SDK retains a public retired V3 schema carrier")
if re.search(
    r"\bpublic\s+static\s+func\s+(?:initSpend|appendSpend|verifySpend|buildRedeem)\s*\(",
    text,
):
    errors.append("Swift SDK retains an unversioned retired lifecycle wrapper")
for error in errors:
    print(
        "[mobile-sdk-artifacts] ERROR: " + error,
        file=sys.stderr,
    )
raise SystemExit(1 if errors else 0)
PY
  then
    FAILURES=1
  fi
}

check_android_kagemusha_source_contract() {
  local rust_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
  local kotlin_source="$ROOT_DIR/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
  local java_source="$ROOT_DIR/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
  local namespace
  local expected_jni=()

  if [[ -f "$rust_source" ]]; then
    for namespace in org_hyperledger_iroha_sdk_offline org_hyperledger_iroha_android_offline; do
      local method
      for method in "${KAGEMUSHA_JNI_METHODS[@]}"; do
        expected_jni+=("Java_${namespace}_KagemushaRecursiveSpendProver_${method}")
      done
    done
    if ! python3 - "$rust_source" "${expected_jni[@]}" <<'PY'
import re
import sys

path = sys.argv[1]
expected = set(sys.argv[2:])
text = open(path, "r", encoding="utf-8").read()
actual = set(re.findall(
    r'fn\s+(Java_org_hyperledger_iroha_(?:sdk|android)_offline_'
    r'KagemushaRecursiveSpendProver_[A-Za-z0-9_]+)\s*\(',
    text,
))
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    print(
        "[mobile-sdk-artifacts] ERROR: Rust bridge is missing Kagemusha JNI exports: "
        + ", ".join(missing),
        file=sys.stderr,
    )
if retired_or_extra:
    print(
        "[mobile-sdk-artifacts] ERROR: Rust bridge exposes retired or unexpected "
        "Kagemusha JNI exports: " + ", ".join(retired_or_extra),
        file=sys.stderr,
    )
raise SystemExit(1 if missing or retired_or_extra else 0)
PY
    then
      FAILURES=1
    fi
  fi

  if [[ -f "$kotlin_source" || -f "$java_source" ]]; then
    if ! python3 - "$kotlin_source" "$java_source" -- "${KAGEMUSHA_JNI_METHODS[@]}" <<'PY'
from pathlib import Path
import re
import sys

separator = sys.argv.index("--")
paths = [Path(raw) for raw in sys.argv[1:separator] if Path(raw).is_file()]
expected_native = set(sys.argv[separator + 1:])
expected_wrappers = {"initSpendV4", "appendSpendV4", "verifySpendV4", "buildRedeemV4"}
errors = []
for path in paths:
    text = path.read_text(encoding="utf-8")
    if path.suffix == ".kt":
        actual_native = set(re.findall(
            r"\bprivate\s+external\s+fun\s+(native[A-Za-z0-9_]+)\s*\(",
            text,
        ))
        actual_wrappers = set(re.findall(
            r"\bfun\s+((?:initSpend|appendSpend|verifySpend|buildRedeem)V[0-9]+)\s*\(",
            text,
        ))
    else:
        actual_native = set(re.findall(
            r"\bprivate\s+static\s+native\s+[A-Za-z0-9_<>?,\[\].]+\s+"
            r"(native[A-Za-z0-9_]+)\s*\(",
            text,
        ))
        actual_wrappers = set(re.findall(
            r"\b(?:public\s+)?(?:static\s+)?[A-Za-z0-9_<>?,\[\].]+\s+"
            r"((?:initSpend|appendSpend|verifySpend|buildRedeem)V[0-9]+)\s*\(",
            text,
        ))
    for label, actual, expected in (
        ("native method", actual_native, expected_native),
        ("lifecycle wrapper", actual_wrappers, expected_wrappers),
    ):
        missing = sorted(expected - actual)
        retired_or_extra = sorted(actual - expected)
        if missing or retired_or_extra:
            errors.append(
                f"{path}: {label} inventory is not exact ABI-20/V4 "
                f"(missing={missing}, retired_or_unexpected={retired_or_extra})"
            )
    if re.search(r"\b(?:data\s+class|class|interface|record|enum)\s+[A-Za-z0-9_]*V3\b", text):
        errors.append(f"{path}: public retired V3 schema carrier")
for error in errors:
    print(f"[mobile-sdk-artifacts] ERROR: {error}", file=sys.stderr)
raise SystemExit(1 if errors else 0)
PY
    then
      FAILURES=1
    fi
  fi
}

require_plist_slice() {
  local plist="$1"
  local slice="$2"
  if ! plist_contains "$plist" "$slice"; then
    fail "Info.plist does not list XCFramework slice $slice"
  fi
}

check_swift_package() {
  local package="$ROOT_DIR/IrohaSwift/Package.swift"

  require_file "$package" "Swift package manifest"
  require_dir "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift" "IrohaSwift sources"
  require_literal "$package" 'name: "IrohaSwift"' "IrohaSwift package name"
  require_literal "$package" '.binaryTarget(' "NoritoBridge binary target declaration"
  require_literal "$package" 'name: "NoritoBridge"' "NoritoBridge binary target name"
  require_literal "$package" '.iOS(.v15)' "IrohaSwift iOS platform floor"
  require_literal "$package" 'path: bridgeRelativePath' "NoritoBridge local artifact path"
}

check_xcframework() {
  local xcframework="$ROOT_DIR/dist/NoritoBridge.xcframework"
  local info="$xcframework/Info.plist"
  local manifest="$ROOT_DIR/dist/NoritoBridge.artifacts.json"
  local privacy_marker="$xcframework/.privacy-production-enabled"
  local slices=(ios-arm64 ios-arm64_x86_64-simulator macos-arm64)
  local slice

  require_dir "$xcframework" "NoritoBridge XCFramework"
  require_file "$info" "NoritoBridge XCFramework metadata"

  for slice in "${slices[@]}"; do
    local slice_dir="$xcframework/$slice"
    local headers_dir="$slice_dir/Headers"
    require_plist_slice "$info" "$slice"
    require_dir "$slice_dir" "XCFramework slice directory"
    if [[ -d "$slice_dir" ]]; then
      require_file "$slice_dir/libNoritoBridge.a" "XCFramework slice binary"
      require_dir "$headers_dir" "XCFramework slice headers"
      if [[ -d "$headers_dir" ]]; then
        require_file "$headers_dir/NoritoBridge.h" "XCFramework slice header"
        require_file "$headers_dir/connect_norito_bridge.h" "XCFramework bridge C header"
        require_file "$headers_dir/module.modulemap" "XCFramework module map"
      fi
    fi
  done

  require_file "$manifest" "NoritoBridge artifact manifest"
  if [[ -f "$manifest" ]]; then
    local privacy_keys=()
    local privacy_declarations=()
    local privacy_key
    local privacy_declaration
    local privacy_value
    while IFS= read -r privacy_key; do
      privacy_keys+=("$privacy_key")
    done < <(
      grep -Eo '"privacy_production_enabled"[[:space:]]*:' "$manifest" || true
    )
    while IFS= read -r privacy_declaration; do
      privacy_declarations+=("$privacy_declaration")
    done < <(
      grep -Eo '"privacy_production_enabled"[[:space:]]*:[[:space:]]*(true|false)' \
        "$manifest" || true
    )
    if [[ ${#privacy_keys[@]} -ne 1 || ${#privacy_declarations[@]} -ne 1 ]]; then
      fail "NoritoBridge artifact manifest must contain exactly one boolean privacy_production_enabled field"
    else
      privacy_value="${privacy_declarations[0]##*:}"
      privacy_value="${privacy_value//[[:space:]]/}"
      if [[ "$privacy_value" == "true" ]]; then
        require_file "$privacy_marker" "privacy-production-enabled XCFramework marker"
      elif [[ -e "$privacy_marker" ]]; then
        fail "default privacy artifact must not carry the privacy-production-enabled XCFramework marker"
      fi
    fi
    require_regex "$manifest" '"version"[[:space:]]*:[[:space:]]*"[^"]+"' "NoritoBridge artifact version"
    for slice in "${slices[@]}"; do
      require_regex "$manifest" "\"$slice\"[[:space:]]*:[[:space:]]*\"[[:xdigit:]]{64}\"" "NoritoBridge artifact manifest hash for $slice"
      if [[ -f "$xcframework/$slice/libNoritoBridge.a" ]]; then
        local expected_hash actual_hash
        expected_hash="$(manifest_json_value "$manifest" "hashes.$slice" 2>/dev/null || true)"
        actual_hash="$(hash_file "$xcframework/$slice/libNoritoBridge.a")"
        if [[ "$expected_hash" != "$actual_hash" ]]; then
          fail "NoritoBridge artifact hash mismatch for $slice"
        fi
      fi
    done

    require_regex "$manifest" '"native_bridge_abi_version"[[:space:]]*:[[:space:]]*20([[:space:]]*[,}])' "exact first-release NoritoBridge ABI 20"
    require_regex "$manifest" '"source_commit"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{40}"' "NoritoBridge source commit"
    require_regex "$manifest" '"source_tree_dirty"[[:space:]]*:[[:space:]]*(true|false)' "NoritoBridge source dirty state"
    require_regex "$manifest" '"source_fingerprint_sha256"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{64}"' "NoritoBridge source fingerprint"
    require_regex "$manifest" '"bridge_header_sha256"[[:space:]]*:[[:space:]]*"[[:xdigit:]]{64}"' "NoritoBridge header hash"
    local manifest_dirty
    manifest_dirty="$(manifest_json_value "$manifest" source_tree_dirty 2>/dev/null || true)"
    if [[ "$manifest_dirty" != "false" && "$ALLOW_DIRTY_SOURCE" != "1" ]]; then
      fail "NoritoBridge release artifact must be built from a clean source tree"
    fi
    if ! python3 - "$manifest" "${REQUIRED_BRIDGE_SYMBOLS[@]}" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)
expected = sys.argv[2:]
actual = payload.get("required_symbols")
raise SystemExit(0 if actual == expected else 1)
PY
    then
      fail "NoritoBridge artifact required symbol inventory is missing or non-canonical"
    fi

    local canonical_header="$xcframework/ios-arm64/Headers/connect_norito_bridge.h"
    if [[ -f "$canonical_header" ]]; then
      local manifest_header_hash actual_header_hash
      manifest_header_hash="$(manifest_json_value "$manifest" bridge_header_sha256 2>/dev/null || true)"
      actual_header_hash="$(hash_file "$canonical_header")"
      if [[ "$manifest_header_hash" != "$actual_header_hash" ]]; then
        fail "NoritoBridge artifact header hash mismatch"
      fi
      for slice in "${slices[@]}"; do
        local slice_header="$xcframework/$slice/Headers/connect_norito_bridge.h"
        if [[ -f "$slice_header" && "$(hash_file "$slice_header")" != "$actual_header_hash" ]]; then
          fail "NoritoBridge bridge header differs in $slice"
        fi
      done
    fi

    local bridge_source="$ROOT_DIR/crates/connect_norito_bridge/src/lib.rs"
    if [[ -f "$bridge_source" && -d "$ROOT_DIR/.git" ]]; then
      local source_abi manifest_abi source_commit manifest_commit source_dirty source_fingerprint manifest_fingerprint
      source_abi="$(sed -nE 's/.*CONNECT_NORITO_BRIDGE_ABI_VERSION:[[:space:]]*u32[[:space:]]*=[[:space:]]*([0-9]+).*/\1/p' "$bridge_source" | head -n1)"
      manifest_abi="$(manifest_json_value "$manifest" native_bridge_abi_version 2>/dev/null || true)"
      if [[ "$source_abi" != "20" || "$manifest_abi" != "20" ]]; then
        fail "NoritoBridge artifact and bridge source must both use exact first-release ABI 20"
      fi
      source_commit="$(git -C "$ROOT_DIR" rev-parse HEAD)"
      manifest_commit="$(manifest_json_value "$manifest" source_commit 2>/dev/null || true)"
      if [[ "$manifest_commit" != "$source_commit" ]]; then
        fail "NoritoBridge artifact source commit does not match checkout"
      fi
      source_dirty=false
      if [[ -n "$(python3 "$ROOT_DIR/scripts/norito_bridge_source_seal.py" \
          status --root "$ROOT_DIR")" ]]; then
        source_dirty=true
      fi
      if [[ "$manifest_dirty" != "$source_dirty" ]]; then
        fail "NoritoBridge artifact source dirty state does not match checkout"
      fi
      if [[ "$source_dirty" != "false" && "$ALLOW_DIRTY_SOURCE" != "1" ]]; then
        fail "NoritoBridge release artifact cannot be certified against a dirty checkout"
      fi
      source_fingerprint="$(bridge_source_fingerprint)"
      manifest_fingerprint="$(manifest_json_value "$manifest" source_fingerprint_sha256 2>/dev/null || true)"
      if [[ "$manifest_fingerprint" != "$source_fingerprint" ]]; then
        fail "NoritoBridge artifact source fingerprint does not match checkout"
      fi
    fi

    if [[ "${MOBILE_SDK_SKIP_BINARY_INSPECTION:-0}" != "1" ]]; then
      local index symbol actual_arches
      for index in "${!slices[@]}"; do
        slice="${slices[$index]}"
        local binary="$xcframework/$slice/libNoritoBridge.a"
        [[ -f "$binary" ]] || continue
        if ! command -v lipo >/dev/null 2>&1; then
          fail "lipo is required for strict Apple artifact validation"
          break
        fi
        actual_arches="$(lipo -archs "$binary" 2>/dev/null || true)"
        case "$slice" in
          ios-arm64|macos-arm64)
            if [[ "$actual_arches" != "arm64" ]]; then
              fail "NoritoBridge $slice architectures must be arm64 (found ${actual_arches:-unreadable})"
            fi
            ;;
          ios-arm64_x86_64-simulator)
            if [[ " $actual_arches " != *" arm64 "* \
              || " $actual_arches " != *" x86_64 "* \
              || "$(wc -w <<<"$actual_arches" | tr -d '[:space:]')" != "2" ]]; then
              fail "NoritoBridge $slice architectures must be arm64 and x86_64 (found ${actual_arches:-unreadable})"
            fi
            ;;
        esac
        if ! command -v nm >/dev/null 2>&1; then
          fail "nm is required for strict Apple artifact validation"
          break
        fi
        local symbols
        symbols="$(nm -gj "$binary" 2>/dev/null || true)"
        for symbol in "${REQUIRED_BRIDGE_SYMBOLS[@]}"; do
          if ! grep -Eq "^_?${symbol}$" <<<"$symbols"; then
            fail "NoritoBridge $slice is missing required symbol $symbol"
          fi
        done
        if ! python3 - "$binary" "${KAGEMUSHA_C_SYMBOLS[@]}" <<'PY'
import subprocess
import sys

binary = sys.argv[1]
expected = set(sys.argv[2:])
result = subprocess.run(
    ["nm", "-gj", binary],
    check=False,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
    text=True,
)
actual = {
    line.strip().removeprefix("_")
    for line in result.stdout.splitlines()
    if line.strip().removeprefix("_").startswith("connect_norito_kagemusha_")
}
raise SystemExit(0 if result.returncode == 0 and actual == expected else 1)
PY
        then
          fail "NoritoBridge $slice Kagemusha export inventory is not exact"
        fi
      done
    fi
  fi
}

check_gradle_publication() {
  local module="$1"
  local artifact_id="$2"
  local build_file="$ROOT_DIR/kotlin/$module/build.gradle.kts"

  require_file "$build_file" "$module Gradle build file"
  require_regex "$build_file" 'maven-publish' "$module maven-publish plugin"
  require_regex "$build_file" 'group[[:space:]]*=[[:space:]]*"org\.hyperledger\.iroha\.sdk"' "$module Maven group"
  require_regex "$build_file" 'version[[:space:]]*=[[:space:]]*("[^"]+"|providers\.gradleProperty\("irohaSdkVersion"\))' "$module Maven version"
  require_regex "$build_file" 'create<MavenPublication>\("release"\)' "$module release publication"
  require_regex "$build_file" "artifactId[[:space:]]*=[[:space:]]*\"$artifact_id\"" "$module artifact id"
}

find_android_nm() {
  local candidate
  local ndk_root

  if [[ -n "${MOBILE_SDK_ANDROID_NM:-}" ]]; then
    if [[ -x "$MOBILE_SDK_ANDROID_NM" ]]; then
      printf '%s' "$MOBILE_SDK_ANDROID_NM"
      return 0
    fi
    if command -v "$MOBILE_SDK_ANDROID_NM" >/dev/null 2>&1; then
      command -v "$MOBILE_SDK_ANDROID_NM"
      return 0
    fi
    return 1
  fi
  if command -v llvm-nm >/dev/null 2>&1; then
    command -v llvm-nm
    return 0
  fi
  for ndk_root in "${ANDROID_NDK_HOME:-}" "${ANDROID_NDK_ROOT:-}"; do
    [[ -n "$ndk_root" ]] || continue
    while IFS= read -r candidate; do
      if [[ -x "$candidate" ]]; then
        printf '%s' "$candidate"
        return 0
      fi
    done < <(compgen -G "$ndk_root/toolchains/llvm/prebuilt/*/bin/llvm-nm" || true)
  done
  if [[ -n "${ANDROID_HOME:-}" ]]; then
    while IFS= read -r candidate; do
      if [[ -x "$candidate" ]]; then
        printf '%s' "$candidate"
        return 0
      fi
    done < <(compgen -G "$ANDROID_HOME/ndk/*/toolchains/llvm/prebuilt/*/bin/llvm-nm" || true)
  fi
  if command -v nm >/dev/null 2>&1; then
    command -v nm
    return 0
  fi
  return 1
}

check_android_native_symbols() {
  local binary="$1"
  local abi="$2"
  local nm_tool
  local symbols
  local namespace
  local expected_jni=()

  if ! nm_tool="$(find_android_nm)"; then
    fail "llvm-nm (or MOBILE_SDK_ANDROID_NM) is required to inspect client-android $abi native bridge"
    return
  fi
  if ! symbols="$("$nm_tool" -g --defined-only "$binary" 2>/dev/null)"; then
    if ! symbols="$("$nm_tool" -D -g --defined-only "$binary" 2>/dev/null)"; then
      if ! symbols="$("$nm_tool" -gj "$binary" 2>/dev/null)"; then
        fail "unable to inspect client-android $abi native bridge with $nm_tool"
        return
      fi
    fi
  fi
  for namespace in org_hyperledger_iroha_sdk_offline org_hyperledger_iroha_android_offline; do
    local method
    for method in "${KAGEMUSHA_JNI_METHODS[@]}"; do
      expected_jni+=("Java_${namespace}_KagemushaRecursiveSpendProver_${method}")
    done
  done
  if ! python3 - "$abi" "${KAGEMUSHA_C_SYMBOLS[@]}" -- "${expected_jni[@]}" 3<<<"$symbols" <<'PY'
import os
import sys

abi = sys.argv[1]
separator = sys.argv.index("--")
expected_c = set(sys.argv[2:separator])
expected_jni = set(sys.argv[separator + 1:])
expected = expected_c | expected_jni | {"connect_norito_bridge_abi_version"}
actual = set()
for raw in os.fdopen(3):
    fields = raw.strip().split()
    if not fields:
        continue
    symbol = fields[-1].removeprefix("_")
    if (
        symbol == "connect_norito_bridge_abi_version"
        or symbol.startswith("connect_norito_kagemusha_")
        or (
            symbol.startswith("Java_org_hyperledger_iroha_")
            and "_KagemushaRecursiveSpendProver_" in symbol
        )
    ):
        actual.add(symbol)
missing = sorted(expected - actual)
retired_or_extra = sorted(actual - expected)
if missing:
    print(
        f"[mobile-sdk-artifacts] ERROR: client-android {abi} bridge is missing "
        "ABI20/V4 symbols: " + ", ".join(missing),
        file=sys.stderr,
    )
if retired_or_extra:
    print(
        f"[mobile-sdk-artifacts] ERROR: client-android {abi} bridge exposes retired "
        "or unexpected Kagemusha symbols: " + ", ".join(retired_or_extra),
        file=sys.stderr,
    )
raise SystemExit(1 if missing or retired_or_extra else 0)
PY
  then
    FAILURES=1
  fi
}

check_android_package() {
  local settings="$ROOT_DIR/kotlin/settings.gradle.kts"

  require_file "$settings" "Kotlin settings manifest"
  require_literal "$settings" 'include(":core-jvm")' "core-jvm module include"
  require_literal "$settings" 'include(":client-android")' "client-android module include"

  check_gradle_publication "core-jvm" "core-jvm"
  check_gradle_publication "client-android" "client-android"

  require_file "$ROOT_DIR/kotlin/client-android/src/main/AndroidManifest.xml" "client-android AndroidManifest"

  if [[ "$REQUIRE_ANDROID_OUTPUTS" == "1" ]]; then
    local client_aar="$ROOT_DIR/kotlin/client-android/build/outputs/aar/client-android-release.aar"
    local abi

    require_glob "$ROOT_DIR/kotlin/core-jvm/build/libs/core-jvm-*.jar" "core-jvm built jar"
    require_glob "$client_aar" "client-android release aar"

    require_zip_entry "$client_aar" "AndroidManifest.xml" "client-android release aar"
    require_zip_entry "$client_aar" "classes.jar" "client-android release aar"

    for abi in arm64-v8a x86_64; do
      local source_native="$ROOT_DIR/kotlin/client-android/src/main/jniLibs/$abi/libconnect_norito_bridge.so"
      local aar_entry="jni/$abi/libconnect_norito_bridge.so"
      require_file "$source_native" "client-android $abi native bridge library"
      require_zip_entry "$client_aar" "jni/$abi/libconnect_norito_bridge.so" "client-android release aar"
      if [[ -f "$source_native" && -f "$client_aar" ]] \
          && unzip -Z1 "$client_aar" 2>/dev/null | grep -Fxq -- "$aar_entry"; then
        if [[ "$(hash_file "$source_native")" != "$(hash_zip_entry "$client_aar" "$aar_entry")" ]]; then
          fail "client-android $abi native bridge differs between jniLibs and release aar"
        fi
        if [[ "${MOBILE_SDK_SKIP_BINARY_INSPECTION:-0}" != "1" ]]; then
          check_android_native_symbols "$source_native" "$abi"
        fi
      fi
    done
  fi
}

check_bridge_source_contract

if [[ "$CHECK_APPLE" == "1" ]]; then
  check_swift_kagemusha_source_contract
  check_swift_package
  check_xcframework
fi

if [[ "$CHECK_ANDROID" == "1" ]]; then
  check_android_kagemusha_source_contract
  check_android_package
fi

if [[ "$FAILURES" -ne 0 ]]; then
  echo "[mobile-sdk-artifacts] validation failed for $ROOT_DIR" >&2
  exit 1
fi

echo "[mobile-sdk-artifacts] validation passed for $ROOT_DIR"
