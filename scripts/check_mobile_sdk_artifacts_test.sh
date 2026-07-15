#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_SCRIPT="$SCRIPT_DIR/check_mobile_sdk_artifacts.sh"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

fail() {
  printf '[mobile-sdk-artifacts-test] ERROR: %s\n' "$*" >&2
  exit 1
}

command -v zip >/dev/null 2>&1 || fail "zip command is required"

test_build_source_seal() {
  local root="$TMP_DIR/source-seal"
  mkdir -p "$root/scripts" "$root/crates/connect_norito_bridge/src" \
    "$root/crates/unrelated/src"
  cp "$SCRIPT_DIR/build_norito_xcframework.sh" \
    "$root/scripts/build_norito_xcframework.sh"
  cp "$SCRIPT_DIR/norito_bridge_source_seal.py" \
    "$root/scripts/norito_bridge_source_seal.py"
  printf '[workspace]\nmembers = ["crates/connect_norito_bridge", "crates/unrelated"]\nresolver = "2"\n' \
    >"$root/Cargo.toml"
  printf '[package]\nname = "connect_norito_bridge"\nversion = "0.1.0"\nedition = "2024"\n\n[features]\nprivacy-production-enabled = []\n' \
    >"$root/crates/connect_norito_bridge/Cargo.toml"
  printf 'pub fn bridge_fixture() {}\n' \
    >"$root/crates/connect_norito_bridge/src/lib.rs"
  printf '[package]\nname = "unrelated"\nversion = "0.1.0"\nedition = "2024"\n' \
    >"$root/crates/unrelated/Cargo.toml"
  printf 'pub fn unrelated_fixture() {}\n' >"$root/crates/unrelated/src/lib.rs"
  cargo generate-lockfile --manifest-path "$root/Cargo.toml" -q
  git -C "$root" init -q
  git -C "$root" add .
  git -C "$root" -c user.name=test -c user.email=test@example.invalid \
    commit -qm source-seal-fixture

  NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY=1 \
    bash "$root/scripts/build_norito_xcframework.sh"

  # A workspace package outside the bridge dependency closure must not make
  # otherwise identical native slices appear mixed-source.
  NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY=1 \
    NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE=crates/unrelated/src/lib.rs \
    bash "$root/scripts/build_norito_xcframework.sh"

  local output
  if output="$(NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY=1 \
      NORITO_BRIDGE_SOURCE_SEAL_TEST_MUTATE=Cargo.toml \
      bash "$root/scripts/build_norito_xcframework.sh" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected source seal to reject an in-build source mutation"
  fi
  case "$output" in
    *"refusing mixed-source Apple slices"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "source seal mutation failure was not explicit"
      ;;
  esac
}

test_build_source_seal

make_aar() {
  local archive="$1"
  shift
  local stage
  local entry

  stage="$(mktemp -d "$TMP_DIR/aar.XXXXXX")"
  for entry in "$@"; do
    mkdir -p "$stage/$(dirname "$entry")"
    if [[ "$entry" == *.jar ]]; then
      local jar_stage
      jar_stage="$(mktemp -d "$TMP_DIR/jar.XXXXXX")"
      printf 'fixture\n' >"$jar_stage/fixture.txt"
      (cd "$jar_stage" && zip -qr "$stage/$entry" .)
    else
      printf 'fixture\n' >"$stage/$entry"
    fi
  done
  (cd "$stage" && zip -qr "$archive" .)
}

make_jar() {
  local archive="$1"
  local stage
  stage="$(mktemp -d "$TMP_DIR/jar.XXXXXX")"
  printf 'fixture\n' >"$stage/fixture.txt"
  (cd "$stage" && zip -qr "$archive" .)
}

make_gradle_file() {
  local path="$1"
  local artifact_id="$2"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<GRADLE
plugins {
    \`maven-publish\`
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"

publishing {
    publications {
        create<MavenPublication>("release") {
            artifactId = "$artifact_id"
        }
    }
}
GRADLE
}

make_fixture() {
  local root="$1"
  local hash_a="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  local hash_b="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
  local hash_c="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  local slice

  mkdir -p "$root/IrohaSwift/Sources/IrohaSwift"
  cat >"$root/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4.swift" <<'SWIFT'
enum KagemushaRecursiveSpendV4Fixture {
    static func ensureProofBackendAvailableV4() {}
    static func initSpendV4() {}
    static func appendSpendV4() {}
    static func verifySpendV4() {}
    static func buildRedeemV4() {}

    static let symbols = [
        "connect_norito_kagemusha_recursive_spend_capabilities_v4",
        "connect_norito_kagemusha_recursive_spend_init_v4",
        "connect_norito_kagemusha_recursive_spend_append_v4",
        "connect_norito_kagemusha_recursive_spend_verify_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_v4",
    ]
}
SWIFT
  python3 - "$CHECK_SCRIPT" "$root" <<'PY'
from pathlib import Path
import re
import sys

check_script = Path(sys.argv[1]).read_text(encoding="utf-8")
root = Path(sys.argv[2])

def shell_array(name: str) -> list[str]:
    match = re.search(rf"^{name}=\(\n(.*?)^\)$", check_script, re.MULTILINE | re.DOTALL)
    if match is None:
        raise SystemExit(f"missing fixture array {name}")
    return [
        line.strip()
        for line in match.group(1).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]

c_symbols = shell_array("KAGEMUSHA_C_SYMBOLS")
jni_methods = shell_array("KAGEMUSHA_JNI_METHODS")
native_lifecycle = (
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
)
swift = root / "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
swift.write_text(
    "\n".join(
        [
            *(f'let symbol_{index} = "{symbol}"' for index, symbol in enumerate(c_symbols)),
            *(f"func {method}() {{}}" for method in native_lifecycle),
        ]
    )
    + "\n",
    encoding="utf-8",
)

bridge_dir = root / "crates/connect_norito_bridge"
(bridge_dir / "src").mkdir(parents=True, exist_ok=True)
(bridge_dir / "include").mkdir(parents=True, exist_ok=True)
(bridge_dir / "include/connect_norito_bridge.h").write_text(
    "\n".join(f"int {symbol}(void);" for symbol in c_symbols) + "\n",
    encoding="utf-8",
)
rust_lines = [
    "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 20;",
    *(f'pub unsafe extern "C" fn {symbol}() {{}}' for symbol in c_symbols),
]
for namespace in (
    "org_hyperledger_iroha_sdk_offline",
    "org_hyperledger_iroha_android_offline",
):
    rust_lines.extend(
        f"fn Java_{namespace}_KagemushaRecursiveSpendProver_{method}() {{}}"
        for method in jni_methods
    )
(bridge_dir / "src/lib.rs").write_text("\n".join(rust_lines) + "\n", encoding="utf-8")

kotlin = root / (
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/"
    "KagemushaRecursiveSpendProver.kt"
)
kotlin.parent.mkdir(parents=True, exist_ok=True)
kotlin.write_text(
    "\n".join(
        [
            *(f"fun {method}() {{}}" for method in (
                "initSpendV4", "appendSpendV4", "verifySpendV4", "buildRedeemV4"
            )),
            *(f"private external fun {method}()" for method in jni_methods),
        ]
    )
    + "\n",
    encoding="utf-8",
)
java = root / (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/"
    "KagemushaRecursiveSpendProver.java"
)
java.parent.mkdir(parents=True, exist_ok=True)
java.write_text(
    "\n".join(
        [
            *(f"public static void {method}() {{}}" for method in (
                "initSpendV4", "appendSpendV4", "verifySpendV4", "buildRedeemV4"
            )),
            *(f"private static native void {method}();" for method in jni_methods),
        ]
    )
    + "\n",
    encoding="utf-8",
)
PY
  cat >"$root/IrohaSwift/Package.swift" <<'SWIFT'
// swift-tools-version:5.9
import PackageDescription

let bridgeRelativePath = "../dist/NoritoBridge.xcframework"

let package = Package(
    name: "IrohaSwift",
    platforms: [
        .iOS(.v15)
    ],
    targets: [
        .binaryTarget(
            name: "NoritoBridge",
            path: bridgeRelativePath
        )
    ]
)
SWIFT

  mkdir -p "$root/dist/NoritoBridge.xcframework"
  cat >"$root/dist/NoritoBridge.xcframework/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>AvailableLibraries</key>
  <array>
    <dict><key>LibraryIdentifier</key><string>ios-arm64</string></dict>
    <dict><key>LibraryIdentifier</key><string>ios-arm64_x86_64-simulator</string></dict>
    <dict><key>LibraryIdentifier</key><string>macos-arm64</string></dict>
  </array>
</dict>
</plist>
PLIST

  for slice in ios-arm64 ios-arm64_x86_64-simulator macos-arm64; do
    mkdir -p "$root/dist/NoritoBridge.xcframework/$slice/Headers"
    printf 'fake static library for %s\n' "$slice" >"$root/dist/NoritoBridge.xcframework/$slice/libNoritoBridge.a"
    printf 'void norito_%s(void);\n' "$slice" >"$root/dist/NoritoBridge.xcframework/$slice/Headers/NoritoBridge.h"
    printf 'uint32_t connect_norito_bridge_abi_version(void);\n' >"$root/dist/NoritoBridge.xcframework/$slice/Headers/connect_norito_bridge.h"
    printf 'module NoritoBridge {}\n' >"$root/dist/NoritoBridge.xcframework/$slice/Headers/module.modulemap"
  done

  hash_a="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a" | awk '{print $1}')"
  hash_b="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/libNoritoBridge.a" | awk '{print $1}')"
  hash_c="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/macos-arm64/libNoritoBridge.a" | awk '{print $1}')"
  local header_hash
  header_hash="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64/Headers/connect_norito_bridge.h" | awk '{print $1}')"

  cat >"$root/dist/NoritoBridge.artifacts.json" <<JSON
{
  "version": "1.0.0",
  "native_bridge_abi_version": 20,
  "privacy_production_enabled": false,
  "source_commit": "0000000000000000000000000000000000000000",
  "source_tree_dirty": false,
  "source_fingerprint_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
  "bridge_header_sha256": "$header_hash",
  "required_symbols": [
    "connect_norito_bridge_abi_version",
    "connect_norito_free",
    "connect_norito_encode_transfer_signed_transaction",
    "connect_norito_encode_transfer_instruction_box",
    "connect_norito_encode_validation_fee_transfer_signed_transaction",
    "connect_norito_detached_transaction_scaffold_inspect_v1",
    "connect_norito_detached_transaction_scaffold_finalize_ed25519_v1",
    "connect_norito_canonical_json_blake3_v1",
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
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v4"
  ],
  "hashes": {
    "ios-arm64": "$hash_a",
    "ios-arm64_x86_64-simulator": "$hash_b",
    "macos-arm64": "$hash_c"
  }
}
JSON

  mkdir -p "$root/kotlin/client-android/src/main"
  cat >"$root/kotlin/settings.gradle.kts" <<'SETTINGS'
rootProject.name = "iroha_kotlin_sdk"
include(":core-jvm")
include(":client-android")
SETTINGS
  make_gradle_file "$root/kotlin/core-jvm/build.gradle.kts" "core-jvm"
  make_gradle_file "$root/kotlin/client-android/build.gradle.kts" "client-android"
  printf '<manifest />\n' >"$root/kotlin/client-android/src/main/AndroidManifest.xml"
}

append_candidate_lab_source() {
  local root="$1"
  python3 - "$CHECK_SCRIPT" "$root" <<'PY'
from pathlib import Path
import re
import sys

check_script = Path(sys.argv[1]).read_text(encoding="utf-8")
root = Path(sys.argv[2])


def shell_array(name):
    match = re.search(
        rf"^{name}=\(\n(.*?)^\)$", check_script, re.MULTILINE | re.DOTALL
    )
    if match is None:
        raise SystemExit(f"missing fixture array {name}")
    return [
        line.strip()
        for line in match.group(1).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]


feature = "kagemusha-candidate-evidence-lab"
marker = "KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2"
marker_symbol = "CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2"
symbols = shell_array("KAGEMUSHA_CANDIDATE_LAB_C_SYMBOLS")
declarations = []
for symbol in symbols:
    declarations.extend(
        [
            f'#[cfg(feature = "{feature}")]',
            "#[unsafe(no_mangle)]",
            f'pub unsafe extern "C" fn {symbol}() {{}}',
            "",
        ]
    )
declarations.extend(
    [
        f'#[cfg(feature = "{feature}")]',
        "pub const KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_MARKER_V2: &str =",
        f'    "{marker}";',
        "",
        f'#[cfg(feature = "{feature}")]',
        "#[used]",
        "#[unsafe(no_mangle)]",
        f"pub static {marker_symbol}: [u8;",
        "    KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_MARKER_V2.len()] =",
        f'    *b"{marker}";',
        "",
        "#[cfg(all(",
        f'    feature = "{feature}",',
        "    any(",
        '        target_os = "android",',
        '        target_os = "linux",',
        '        target_os = "macos",',
        '        target_os = "windows"',
        "    )",
        "))]",
        "#[allow(clippy::missing_safety_doc)]",
        "#[unsafe(no_mangle)]",
        'pub unsafe extern "system" fn '
        "Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_"
        "KagemushaCandidateLabNative_nativeBridgeAbiVersion() {}",
        "",
    ]
)
source = root / "crates/connect_norito_bridge/src/lib.rs"
source.write_text(
    source.read_text(encoding="utf-8") + "\n".join(declarations),
    encoding="utf-8",
)
cargo = root / "crates/connect_norito_bridge/Cargo.toml"
cargo.write_text(
    "[features]\n"
    f'{feature} = ["iroha_core/{feature}"]\n',
    encoding="utf-8",
)
PY
}

append_candidate_lab_header() {
  local root="$1"
  python3 - "$CHECK_SCRIPT" "$root" <<'PY'
from pathlib import Path
import re
import sys

check_script = Path(sys.argv[1]).read_text(encoding="utf-8")
root = Path(sys.argv[2])
match = re.search(
    r"^KAGEMUSHA_CANDIDATE_LAB_C_SYMBOLS=\(\n(.*?)^\)$",
    check_script,
    re.MULTILINE | re.DOTALL,
)
if match is None:
    raise SystemExit("missing candidate-lab fixture array")
symbols = [
    line.strip()
    for line in match.group(1).splitlines()
    if line.strip() and not line.lstrip().startswith("#")
]
block = [
    "#ifdef CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB",
    "extern const uint8_t "
    "CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2[];",
    *(f"int32_t {symbol}(void);" for symbol in symbols),
    "#endif",
    "",
]
header = root / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
header.write_text(
    header.read_text(encoding="utf-8") + "\n".join(block),
    encoding="utf-8",
)
PY
}

run_expect_pass() {
  local root="$1"
  shift
  local output
  if ! output="$(MOBILE_SDK_SKIP_BINARY_INSPECTION=1 bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected validation to pass for $root"
  fi
}

run_expect_fail() {
  local root="$1"
  local expected="$2"
  shift 2
  local output
  if output="$(MOBILE_SDK_SKIP_BINARY_INSPECTION=1 bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected validation to fail for $root"
  fi
  case "$output" in
    *"$expected"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected failure containing: $expected"
      ;;
  esac
}

make_apple_inspection_tools() {
  local tools="$1"
  mkdir -p "$tools"
  cat >"$tools/lipo" <<'SH'
#!/usr/bin/env bash
case "${*: -1}" in
  *ios-arm64_x86_64-simulator*) printf 'arm64 x86_64\n' ;;
  *) printf 'arm64\n' ;;
esac
SH
  cat >"$tools/nm" <<'SH'
#!/usr/bin/env bash
binary="${*: -1}"
root="${binary%%/dist/*}"
python3 - "$root/dist/NoritoBridge.artifacts.json" <<'PY'
import json
import os
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    manifest = json.load(handle)
for symbol in manifest["required_symbols"]:
    print("_" + symbol)
if os.environ.get("MOBILE_SDK_TEST_EXTRA_KAGEMUSHA") == "1":
    print("_connect_norito_kagemusha_unexpected_v2")
PY
SH
  chmod +x "$tools/lipo" "$tools/nm"
}

make_android_inspection_tools() {
  local tools="$1"
  mkdir -p "$tools"
  cat >"$tools/llvm-nm" <<'SH'
#!/usr/bin/env bash
python3 - "${MOBILE_SDK_TEST_CHECK_SCRIPT:?}" <<'PY'
import os
import re
import sys

text = open(sys.argv[1], "r", encoding="utf-8").read()

def shell_array(name):
    match = re.search(rf"^{name}=\(\n(.*?)^\)$", text, re.MULTILINE | re.DOTALL)
    if match is None:
        raise SystemExit(f"missing fixture array {name}")
    return [
        line.strip()
        for line in match.group(1).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]

print("connect_norito_bridge_abi_version")
for symbol in shell_array("KAGEMUSHA_C_SYMBOLS"):
    print(symbol)
for namespace in (
    "org_hyperledger_iroha_sdk_offline",
    "org_hyperledger_iroha_android_offline",
):
    for method in shell_array("KAGEMUSHA_JNI_METHODS"):
        print(f"Java_{namespace}_KagemushaRecursiveSpendProver_{method}")
if os.environ.get("MOBILE_SDK_TEST_EXTRA_ANDROID_KAGEMUSHA") == "1":
    print("connect_norito_kagemusha_recursive_spend_init_v3")
PY
SH
  cat >"$tools/file" <<'SH'
#!/usr/bin/env bash
if [[ "${MOBILE_SDK_TEST_ANDROID_UNSTRIPPED:-0}" == "1" ]]; then
  printf 'ELF 64-bit LSB shared object, dynamically linked, not stripped\n'
else
  printf 'ELF 64-bit LSB shared object, dynamically linked, stripped\n'
fi
SH
  chmod +x "$tools/llvm-nm" "$tools/file"
}

run_expect_binary_fail() {
  local root="$1"
  local expected="$2"
  local tools="$3"
  local output
  if output="$(PATH="$tools:$PATH" MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_EXTRA_KAGEMUSHA=1 bash "$CHECK_SCRIPT" "$root" --apple-only 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected strict binary validation to fail for $root"
  fi
  case "$output" in
    *"$expected"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected strict binary failure containing: $expected"
      ;;
  esac
}

run_expect_android_binary_pass() {
  local root="$1"
  local tools="$2"
  local output
  if ! output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_ANDROID_NM="$tools/llvm-nm" \
      MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      bash "$CHECK_SCRIPT" "$root" --android-only --require-built-android 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected strict Android binary validation to pass for $root"
  fi
}

run_expect_android_binary_fail() {
  local root="$1"
  local expected="$2"
  local tools="$3"
  local output
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_ANDROID_NM="$tools/llvm-nm" \
      MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_EXTRA_ANDROID_KAGEMUSHA=1 \
      bash "$CHECK_SCRIPT" "$root" --android-only --require-built-android 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected strict Android binary validation to fail for $root"
  fi
  case "$output" in
    *"$expected"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected strict Android binary failure containing: $expected"
      ;;
  esac
}

run_expect_android_unstripped_fail() {
  local root="$1"
  local tools="$2"
  local output
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_ANDROID_NM="$tools/llvm-nm" \
      MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_ANDROID_UNSTRIPPED=1 \
      bash "$CHECK_SCRIPT" "$root" --android-only --require-built-android 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected strict Android binary validation to reject an unstripped bridge"
  fi
  case "$output" in
    *"native bridge is not canonically stripped"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected strict Android unstripped-binary failure"
      ;;
  esac
}

fixture="$TMP_DIR/valid"
make_fixture "$fixture"
run_expect_pass "$fixture"

sentinel_only_source="$TMP_DIR/sentinel-only-source"
make_fixture "$sentinel_only_source"
cat >>"$sentinel_only_source/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
const RETIRED_INPUT_SENTINEL: &str = "reject-kagemusha-v3";
const TOPUP_SHIELD_CIRCUIT_ID: &str = "topup-shield-v3";
RUST
run_expect_pass "$sentinel_only_source"

feature_gated_candidate_lab_source="$TMP_DIR/feature-gated-candidate-lab-source"
make_fixture "$feature_gated_candidate_lab_source"
append_candidate_lab_source "$feature_gated_candidate_lab_source"
run_expect_pass "$feature_gated_candidate_lab_source"

candidate_lab_source_without_export="$TMP_DIR/candidate-lab-source-without-export"
make_fixture "$candidate_lab_source_without_export"
append_candidate_lab_source "$candidate_lab_source_without_export"
python3 - "$candidate_lab_source_without_export" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
name = "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4"
old = (
    '#[cfg(feature = "kagemusha-candidate-evidence-lab")]\n'
    "#[unsafe(no_mangle)]\n"
    f'pub unsafe extern "C" fn {name}'
)
replacement = (
    '#[cfg(feature = "kagemusha-candidate-evidence-lab")]\n'
    f"unsafe fn {name}"
)
if text.count(old) != 1:
    raise SystemExit("missing exact candidate-lab export fixture")
source.write_text(text.replace(old, replacement, 1), encoding="utf-8")
PY
run_expect_fail \
  "$candidate_lab_source_without_export" \
  "candidate-lab Rust/C export inventory is not exact"

unguarded_candidate_lab_source="$TMP_DIR/unguarded-candidate-lab-source"
make_fixture "$unguarded_candidate_lab_source"
append_candidate_lab_source "$unguarded_candidate_lab_source"
python3 - "$unguarded_candidate_lab_source" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
name = "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4"
old = (
    '#[cfg(feature = "kagemusha-candidate-evidence-lab")]\n'
    "#[unsafe(no_mangle)]\n"
    f'pub unsafe extern "C" fn {name}'
)
replacement = "#[unsafe(no_mangle)]\n" f'pub unsafe extern "C" fn {name}'
if text.count(old) != 1:
    raise SystemExit("missing exact candidate-lab guard fixture")
source.write_text(text.replace(old, replacement, 1), encoding="utf-8")
PY
run_expect_fail \
  "$unguarded_candidate_lab_source" \
  "candidate-lab Rust export is not directly guarded by its exact feature"

feature_gated_candidate_lab_header="$TMP_DIR/feature-gated-candidate-lab-header"
make_fixture "$feature_gated_candidate_lab_header"
append_candidate_lab_header "$feature_gated_candidate_lab_header"
run_expect_pass "$feature_gated_candidate_lab_header"

escaped_candidate_lab_header="$TMP_DIR/escaped-candidate-lab-header"
cp -R "$feature_gated_candidate_lab_header" "$escaped_candidate_lab_header"
printf '%s\n' \
  'int32_t connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4(void);' \
  >>"$escaped_candidate_lab_header/crates/connect_norito_bridge/include/connect_norito_bridge.h"
run_expect_fail \
  "$escaped_candidate_lab_header" \
  "candidate-lab header declaration escaped its guard"

enabled_candidate_lab_header="$TMP_DIR/enabled-candidate-lab-header"
cp -R "$feature_gated_candidate_lab_header" "$enabled_candidate_lab_header"
sed -i.bak '1i\
#define CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB 1
' "$enabled_candidate_lab_header/crates/connect_norito_bridge/include/connect_norito_bridge.h"
rm -f "$enabled_candidate_lab_header/crates/connect_norito_bridge/include/connect_norito_bridge.h.bak"
run_expect_fail \
  "$enabled_candidate_lab_header" \
  "bridge header must not enable the candidate-lab macro"

extra_candidate_lab_source="$TMP_DIR/extra-candidate-lab-source"
cp -R "$feature_gated_candidate_lab_source" "$extra_candidate_lab_source"
cat >>"$extra_candidate_lab_source/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_candidate_lab_rogue_v4() {}
RUST
run_expect_fail \
  "$extra_candidate_lab_source" \
  "candidate-lab Rust function inventory is not exact"

extra_candidate_lab_header="$TMP_DIR/extra-candidate-lab-header"
cp -R "$feature_gated_candidate_lab_header" "$extra_candidate_lab_header"
python3 - "$extra_candidate_lab_header" <<'PY'
from pathlib import Path
import sys

header = Path(sys.argv[1]) / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
text = header.read_text(encoding="utf-8")
old = "\n#endif\n"
new = (
    "\nint32_t connect_norito_kagemusha_recursive_spend_candidate_lab_rogue_v4(void);"
    "\n#endif\n"
)
if text.count(old) != 1:
    raise SystemExit("missing exact candidate-lab header guard fixture")
header.write_text(text.replace(old, new, 1), encoding="utf-8")
PY
run_expect_fail \
  "$extra_candidate_lab_header" \
  "candidate-lab header inventory is not exact"

duplicate_candidate_lab_source="$TMP_DIR/duplicate-candidate-lab-source"
cp -R "$feature_gated_candidate_lab_source" "$duplicate_candidate_lab_source"
cat >>"$duplicate_candidate_lab_source/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
#[cfg(not(feature = "kagemusha-candidate-evidence-lab"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4() {}
RUST
run_expect_fail \
  "$duplicate_candidate_lab_source" \
  "non_single_occurrence"

commented_candidate_lab_source="$TMP_DIR/commented-candidate-lab-source"
cp -R "$candidate_lab_source_without_export" "$commented_candidate_lab_source"
cat >>"$commented_candidate_lab_source/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
/*
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4() {}
*/
RUST
run_expect_fail \
  "$commented_candidate_lab_source" \
  "candidate-lab Rust/C export inventory is not exact"

raw_string_candidate_lab_source="$TMP_DIR/raw-string-candidate-lab-source"
cp -R "$candidate_lab_source_without_export" "$raw_string_candidate_lab_source"
cat >>"$raw_string_candidate_lab_source/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
const FAKE_CANDIDATE_EXPORT: &str = r#"
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4() {}
"#;
RUST
run_expect_fail \
  "$raw_string_candidate_lab_source" \
  "candidate-lab Rust/C export inventory is not exact"

commented_candidate_lab_header="$TMP_DIR/commented-candidate-lab-header"
cp -R "$feature_gated_candidate_lab_header" "$commented_candidate_lab_header"
python3 - "$commented_candidate_lab_header" <<'PY'
from pathlib import Path
import sys

header = Path(sys.argv[1]) / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
text = header.read_text(encoding="utf-8")
name = "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4"
declaration = f"int32_t {name}(void);"
if text.count(declaration) != 1:
    raise SystemExit("missing exact candidate-lab header fixture")
text = text.replace(declaration, "", 1)
text = text.replace("\n#endif\n", f"\n/* {declaration} */\n#endif\n", 1)
header.write_text(text, encoding="utf-8")
PY
run_expect_fail \
  "$commented_candidate_lab_header" \
  "candidate-lab header inventory is not exact"

commented_candidate_marker_header="$TMP_DIR/commented-candidate-marker-header"
cp -R "$feature_gated_candidate_lab_header" "$commented_candidate_marker_header"
python3 - "$commented_candidate_marker_header" <<'PY'
from pathlib import Path
import sys

header = Path(sys.argv[1]) / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
text = header.read_text(encoding="utf-8")
marker = (
    "extern const uint8_t "
    "CONNECT_NORITO_KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2[];"
)
if text.count(marker) != 1:
    raise SystemExit("missing exact candidate-lab marker fixture")
text = text.replace(marker, marker.replace("V2", "V2_DRIFTED"), 1)
text = text.replace("\n#endif\n", f"\n/* {marker} */\n#endif\n", 1)
header.write_text(text, encoding="utf-8")
PY
run_expect_fail \
  "$commented_candidate_marker_header" \
  "candidate-lab header guard lacks its exact do-not-ship marker"

default_candidate_lab_feature="$TMP_DIR/default-candidate-lab-feature"
cp -R "$feature_gated_candidate_lab_source" "$default_candidate_lab_feature"
sed -i.bak '/^\[features\]$/a\
default = ["kagemusha-candidate-evidence-lab"]
' "$default_candidate_lab_feature/crates/connect_norito_bridge/Cargo.toml"
rm -f "$default_candidate_lab_feature/crates/connect_norito_bridge/Cargo.toml.bak"
run_expect_fail \
  "$default_candidate_lab_feature" \
  "candidate-lab Cargo feature is enabled directly or transitively by default"

transitive_default_candidate_lab_feature="$TMP_DIR/transitive-default-candidate-lab-feature"
cp -R "$feature_gated_candidate_lab_source" "$transitive_default_candidate_lab_feature"
sed -i.bak '/^\[features\]$/a\
default = ["candidate-lab-alias"]\
candidate-lab-alias = ["kagemusha-candidate-evidence-lab"]
' "$transitive_default_candidate_lab_feature/crates/connect_norito_bridge/Cargo.toml"
rm -f "$transitive_default_candidate_lab_feature/crates/connect_norito_bridge/Cargo.toml.bak"
run_expect_fail \
  "$transitive_default_candidate_lab_feature" \
  "candidate-lab Cargo feature is enabled directly or transitively by default"

drifted_candidate_lab_feature="$TMP_DIR/drifted-candidate-lab-feature"
cp -R "$feature_gated_candidate_lab_source" "$drifted_candidate_lab_feature"
sed -i.bak \
  's#iroha_core/kagemusha-candidate-evidence-lab#iroha_core/unexpected-lab-feature#' \
  "$drifted_candidate_lab_feature/crates/connect_norito_bridge/Cargo.toml"
rm -f "$drifted_candidate_lab_feature/crates/connect_norito_bridge/Cargo.toml.bak"
run_expect_fail \
  "$drifted_candidate_lab_feature" \
  "candidate-lab Cargo feature delegation is not exact"

unguarded_candidate_lab_marker="$TMP_DIR/unguarded-candidate-lab-marker"
cp -R "$feature_gated_candidate_lab_source" "$unguarded_candidate_lab_marker"
python3 - "$unguarded_candidate_lab_marker" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
old = '#[cfg(feature = "kagemusha-candidate-evidence-lab")]\n#[used]\n'
if text.count(old) != 1:
    raise SystemExit("missing exact candidate-lab link marker fixture")
source.write_text(text.replace(old, "#[used]\n", 1), encoding="utf-8")
PY
run_expect_fail \
  "$unguarded_candidate_lab_marker" \
  "candidate-lab Rust link marker is not one exact guarded no-mangle static"

unguarded_candidate_lab_jni="$TMP_DIR/unguarded-candidate-lab-jni"
cp -R "$feature_gated_candidate_lab_source" "$unguarded_candidate_lab_jni"
python3 - "$unguarded_candidate_lab_jni" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
guard = '''#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
'''
if text.count(guard) != 1:
    raise SystemExit("missing exact candidate-lab JNI guard fixture")
source.write_text(text.replace(guard, "", 1), encoding="utf-8")
PY
run_expect_fail \
  "$unguarded_candidate_lab_jni" \
  "candidate-lab JNI export lacks its exact conjunctive feature guard"

retired_header_surface="$TMP_DIR/retired-header-surface"
make_fixture "$retired_header_surface"
printf '%s\n' 'int connect_norito_kagemusha_recursive_spend_init_v3(void);' \
  >>"$retired_header_surface/crates/connect_norito_bridge/include/connect_norito_bridge.h"
run_expect_fail "$retired_header_surface" "bridge header exposes retired or unexpected Kagemusha declarations"

retired_swift_binding="$TMP_DIR/retired-swift-binding"
make_fixture "$retired_swift_binding"
printf '%s\n' 'let retired = "connect_norito_kagemusha_recursive_spend_init_v3"' \
  >>"$retired_swift_binding/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
run_expect_fail "$retired_swift_binding" "Swift Kagemusha native symbol inventory is not exact ABI-20/V4"

retired_swift_surface="$TMP_DIR/retired-swift-surface"
make_fixture "$retired_swift_surface"
cat >>"$retired_swift_surface/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4.swift" <<'SWIFT'
public enum RetiredKagemushaRecursiveSpend {
    public static func initSpend() {}
}
SWIFT
run_expect_fail "$retired_swift_surface" "Swift SDK retains an unversioned retired lifecycle wrapper"

retired_bridge_source="$TMP_DIR/retired-bridge-source"
make_fixture "$retired_bridge_source"
mkdir -p "$retired_bridge_source/crates/connect_norito_bridge/src"
cat >"$retired_bridge_source/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 20;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_init_v3() {}
RUST
run_expect_fail "$retired_bridge_source" "retired or unexpected Kagemusha C exports"

retired_kotlin_native="$TMP_DIR/retired-kotlin-native"
make_fixture "$retired_kotlin_native"
printf '%s\n' 'private external fun nativeArtifactBindingV3()' \
  >>"$retired_kotlin_native/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
run_expect_fail "$retired_kotlin_native" "native method inventory is not exact ABI-20/V4" --android-only

retired_java_native="$TMP_DIR/retired-java-native"
make_fixture "$retired_java_native"
printf '%s\n' 'private static native void nativeArtifactBindingV3();' \
  >>"$retired_java_native/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
run_expect_fail "$retired_java_native" "native method inventory is not exact ABI-20/V4" --android-only

retired_rust_jni="$TMP_DIR/retired-rust-jni"
make_fixture "$retired_rust_jni"
printf '%s\n' \
  'fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactBindingV3() {}' \
  >>"$retired_rust_jni/crates/connect_norito_bridge/src/lib.rs"
run_expect_fail "$retired_rust_jni" "Rust bridge exposes retired or unexpected Kagemusha JNI exports" --android-only

wrong_bridge_abi="$TMP_DIR/wrong-bridge-abi"
make_fixture "$wrong_bridge_abi"
sed -i.bak 's/"native_bridge_abi_version": 20/"native_bridge_abi_version": 19/' \
  "$wrong_bridge_abi/dist/NoritoBridge.artifacts.json"
rm -f "$wrong_bridge_abi/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$wrong_bridge_abi" "exact first-release NoritoBridge ABI 20"

enabled_privacy="$TMP_DIR/enabled-privacy"
make_fixture "$enabled_privacy"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  "$enabled_privacy/dist/NoritoBridge.artifacts.json"
rm -f "$enabled_privacy/dist/NoritoBridge.artifacts.json.bak"
touch "$enabled_privacy/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_pass "$enabled_privacy"

enabled_without_marker="$TMP_DIR/enabled-without-marker"
make_fixture "$enabled_without_marker"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  "$enabled_without_marker/dist/NoritoBridge.artifacts.json"
rm -f "$enabled_without_marker/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$enabled_without_marker" "missing privacy-production-enabled XCFramework marker"

default_with_marker="$TMP_DIR/default-with-marker"
make_fixture "$default_with_marker"
touch "$default_with_marker/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail "$default_with_marker" "default privacy artifact must not carry the privacy-production-enabled XCFramework marker"

invalid_privacy_state="$TMP_DIR/invalid-privacy-state"
make_fixture "$invalid_privacy_state"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": "false"/' \
  "$invalid_privacy_state/dist/NoritoBridge.artifacts.json"
rm -f "$invalid_privacy_state/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$invalid_privacy_state" "must contain exactly one boolean privacy_production_enabled field"

dirty_source_manifest="$TMP_DIR/dirty-source-manifest"
make_fixture "$dirty_source_manifest"
sed -i.bak 's/"source_tree_dirty": false/"source_tree_dirty": true/' \
  "$dirty_source_manifest/dist/NoritoBridge.artifacts.json"
rm -f "$dirty_source_manifest/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$dirty_source_manifest" "release artifact must be built from a clean source tree"
MOBILE_SDK_ALLOW_DIRTY_SOURCE=1 run_expect_pass "$dirty_source_manifest"

duplicate_mixed_privacy_state="$TMP_DIR/duplicate-mixed-privacy-state"
make_fixture "$duplicate_mixed_privacy_state"
sed -i.bak '/"privacy_production_enabled": false/a\
  "privacy_production_enabled": "false",' \
  "$duplicate_mixed_privacy_state/dist/NoritoBridge.artifacts.json"
rm -f "$duplicate_mixed_privacy_state/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$duplicate_mixed_privacy_state" "must contain exactly one boolean privacy_production_enabled field"

apple_only_without_android="$TMP_DIR/apple-only-without-android"
make_fixture "$apple_only_without_android"
rm -rf "$apple_only_without_android/kotlin"
run_expect_pass "$apple_only_without_android" --apple-only

android_only_without_apple="$TMP_DIR/android-only-without-apple"
make_fixture "$android_only_without_apple"
rm -rf "$android_only_without_apple/IrohaSwift" "$android_only_without_apple/dist"
run_expect_pass "$android_only_without_apple" --android-only

missing_ios="$TMP_DIR/missing-ios"
make_fixture "$missing_ios"
rm -rf "$missing_ios/dist/NoritoBridge.xcframework/ios-arm64"
run_expect_fail "$missing_ios" "missing XCFramework slice directory"

missing_header="$TMP_DIR/missing-header"
make_fixture "$missing_header"
rm -f "$missing_header/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/Headers/NoritoBridge.h"
run_expect_fail "$missing_header" "missing XCFramework slice header"

missing_hash="$TMP_DIR/missing-hash"
make_fixture "$missing_hash"
cat >"$missing_hash/dist/NoritoBridge.artifacts.json" <<'JSON'
{
  "version": "1.0.0",
  "privacy_production_enabled": false,
  "hashes": {
    "ios-arm64": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "macos-arm64": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
  }
}
JSON
run_expect_fail "$missing_hash" "NoritoBridge artifact manifest hash for ios-arm64_x86_64-simulator"

hash_mismatch="$TMP_DIR/hash-mismatch"
make_fixture "$hash_mismatch"
printf 'tampered\n' >>"$hash_mismatch/dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a"
run_expect_fail "$hash_mismatch" "NoritoBridge artifact hash mismatch for ios-arm64"

candidate_marker_apple="$TMP_DIR/candidate-marker-apple"
make_fixture "$candidate_marker_apple"
printf '%s\n' 'KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2' \
  >>"$candidate_marker_apple/dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a"
python3 - "$candidate_marker_apple" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

root = Path(sys.argv[1])
manifest_path = root / "dist/NoritoBridge.artifacts.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
binary = root / "dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a"
manifest["hashes"]["ios-arm64"] = hashlib.sha256(binary.read_bytes()).hexdigest()
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
run_expect_fail "$candidate_marker_apple" "contains a non-shipping Kagemusha candidate-lab marker or symbol"

header_mismatch="$TMP_DIR/header-mismatch"
make_fixture "$header_mismatch"
printf 'void unexpected(void);\n' >>"$header_mismatch/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/Headers/connect_norito_bridge.h"
run_expect_fail "$header_mismatch" "NoritoBridge bridge header differs in ios-arm64_x86_64-simulator"

extra_binary_symbol="$TMP_DIR/extra-binary-symbol"
make_fixture "$extra_binary_symbol"
inspection_tools="$TMP_DIR/inspection-tools"
make_apple_inspection_tools "$inspection_tools"
run_expect_binary_fail \
  "$extra_binary_symbol" \
  "Kagemusha export inventory is not exact" \
  "$inspection_tools"

symbol_inventory_mismatch="$TMP_DIR/symbol-inventory-mismatch"
make_fixture "$symbol_inventory_mismatch"
sed -i.bak \
  's/connect_norito_canonical_json_blake3_v1/unexpected_symbol/' \
  "$symbol_inventory_mismatch/dist/NoritoBridge.artifacts.json"
rm -f "$symbol_inventory_mismatch/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$symbol_inventory_mismatch" "required symbol inventory is missing or non-canonical"

extra_manifest_symbol="$TMP_DIR/extra-manifest-symbol"
make_fixture "$extra_manifest_symbol"
sed -i.bak '/"connect_norito_kagemusha_recursive_spend_init_v4",/i\
    "connect_norito_kagemusha_unexpected_v2",' \
  "$extra_manifest_symbol/dist/NoritoBridge.artifacts.json"
rm -f "$extra_manifest_symbol/dist/NoritoBridge.artifacts.json.bak"
run_expect_fail "$extra_manifest_symbol" "required symbol inventory is missing or non-canonical"

missing_android_publication="$TMP_DIR/missing-android-publication"
make_fixture "$missing_android_publication"
cat >"$missing_android_publication/kotlin/client-android/build.gradle.kts" <<'GRADLE'
plugins {
}

group = "org.hyperledger.iroha.sdk"
version = "0.1-SNAPSHOT"
GRADLE
run_expect_fail "$missing_android_publication" "client-android maven-publish plugin"

missing_android_outputs="$TMP_DIR/missing-android-outputs"
make_fixture "$missing_android_outputs"
run_expect_fail "$missing_android_outputs" "missing core-jvm built jar" --require-built-android

missing_client_android_aar="$TMP_DIR/missing-client-android-aar"
make_fixture "$missing_client_android_aar"
mkdir -p "$missing_client_android_aar/kotlin/core-jvm/build/libs"
make_jar "$missing_client_android_aar/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
run_expect_fail "$missing_client_android_aar" "missing client-android release aar" --require-built-android

missing_client_native_source="$TMP_DIR/missing-client-native-source"
make_fixture "$missing_client_native_source"
mkdir -p \
  "$missing_client_native_source/kotlin/core-jvm/build/libs" \
  "$missing_client_native_source/kotlin/client-android/build/outputs/aar"
make_jar "$missing_client_native_source/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
make_aar \
  "$missing_client_native_source/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
  "AndroidManifest.xml" \
  "classes.jar" \
  "jni/arm64-v8a/libconnect_norito_bridge.so" \
  "jni/x86_64/libconnect_norito_bridge.so"
run_expect_fail "$missing_client_native_source" "missing client-android arm64-v8a native bridge library" --require-built-android

missing_client_native_aar_entry="$TMP_DIR/missing-client-native-aar-entry"
make_fixture "$missing_client_native_aar_entry"
mkdir -p \
  "$missing_client_native_aar_entry/kotlin/core-jvm/build/libs" \
  "$missing_client_native_aar_entry/kotlin/client-android/build/outputs/aar" \
  "$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/arm64-v8a" \
  "$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/x86_64"
make_jar "$missing_client_native_aar_entry/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
printf 'so\n' >"$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so"
printf 'so\n' >"$missing_client_native_aar_entry/kotlin/client-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so"
make_aar \
  "$missing_client_native_aar_entry/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
  "AndroidManifest.xml" \
  "classes.jar" \
  "jni/arm64-v8a/libconnect_norito_bridge.so"
run_expect_fail "$missing_client_native_aar_entry" "client-android release aar missing ZIP entry jni/x86_64/libconnect_norito_bridge.so" --require-built-android

with_android_outputs="$TMP_DIR/with-android-outputs"
make_fixture "$with_android_outputs"
mkdir -p \
  "$with_android_outputs/kotlin/core-jvm/build/libs" \
  "$with_android_outputs/kotlin/client-android/build/outputs/aar" \
  "$with_android_outputs/kotlin/client-android/src/main/jniLibs/arm64-v8a" \
  "$with_android_outputs/kotlin/client-android/src/main/jniLibs/x86_64"
make_jar "$with_android_outputs/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
printf 'fixture\n' >"$with_android_outputs/kotlin/client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so"
printf 'fixture\n' >"$with_android_outputs/kotlin/client-android/src/main/jniLibs/x86_64/libconnect_norito_bridge.so"
make_aar \
  "$with_android_outputs/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
  "AndroidManifest.xml" \
  "classes.jar" \
  "jni/arm64-v8a/libconnect_norito_bridge.so" \
  "jni/x86_64/libconnect_norito_bridge.so"
run_expect_pass "$with_android_outputs" --require-built-android

candidate_marker_android="$TMP_DIR/candidate-marker-android"
cp -R "$with_android_outputs" "$candidate_marker_android"
printf '%s\n' 'KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2' \
  >>"$candidate_marker_android/kotlin/client-android/src/main/jniLibs/arm64-v8a/libconnect_norito_bridge.so"
run_expect_fail \
  "$candidate_marker_android" \
  "contains a non-shipping Kagemusha candidate-lab marker or symbol" \
  --require-built-android

candidate_marker_archive="$TMP_DIR/candidate-marker-archive"
cp -R "$with_android_outputs" "$candidate_marker_archive"
archive_marker_stage="$TMP_DIR/candidate-marker-archive-entry"
mkdir -p "$archive_marker_stage"
printf '%s\n' 'kagemusha_recursive_spend_candidate_lab_init_v4' \
  >"$archive_marker_stage/candidate-lab-marker.txt"
(
  cd "$archive_marker_stage"
  zip -q \
    "$candidate_marker_archive/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
    candidate-lab-marker.txt
)
run_expect_fail \
  "$candidate_marker_archive" \
  "contains a non-shipping Kagemusha candidate-lab marker or symbol" \
  --require-built-android

android_inspection_tools="$TMP_DIR/android-inspection-tools"
make_android_inspection_tools "$android_inspection_tools"
run_expect_android_binary_pass "$with_android_outputs" "$android_inspection_tools"
run_expect_android_unstripped_fail "$with_android_outputs" "$android_inspection_tools"
run_expect_android_binary_fail \
  "$with_android_outputs" \
  "exposes retired or unexpected Kagemusha symbols" \
  "$android_inspection_tools"
rm -rf "$with_android_outputs/IrohaSwift" "$with_android_outputs/dist"
run_expect_pass "$with_android_outputs" --android-only --require-built-android

echo "[mobile-sdk-artifacts-test] all checks passed"
