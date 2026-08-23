make_android_outputs() {
  local root="$1"
  local mode="${2:-default}"
  local omitted_aar_abi="${3:-}"
  local aar="$root/kotlin/client-android/build/outputs/aar/client-android-release.aar"

  mkdir -p \
    "$root/kotlin/core-jvm/build/libs" \
    "$root/kotlin/client-android/build/outputs/aar"
  make_jar "$root/kotlin/core-jvm/build/libs/core-jvm-0.1-SNAPSHOT.jar"
  make_aar "$aar" "AndroidManifest.xml" "classes.jar"
  "$TEST_PYTHON_BINARY" -I -S -B - "$root" "$aar" "$mode" "$omitted_aar_abi" <<'PY'
import hashlib
import json
from pathlib import Path
import struct
import sys
import zipfile

root = Path(sys.argv[1])
aar = Path(sys.argv[2])
mode = sys.argv[3]
omitted_aar_abi = sys.argv[4]
if mode not in {"default", "production"}:
    raise SystemExit(f"invalid Android fixture mode: {mode}")
production = mode == "production"
abis = ("arm64-v8a", "x86_64")
library_name = "libconnect_norito_bridge.so"
libraries = {}
generated_libraries = {}


def stripped_elf(machine, marker):
    ident = b"\x7fELF" + bytes((2, 1, 1, 0)) + bytes(8)
    header = struct.pack(
        "<HHIQQQIHHHHHH",
        3,
        machine,
        1,
        0,
        64,
        120,
        0,
        64,
        56,
        1,
        64,
        2,
        1,
    )
    program_header = struct.pack("<IIQQQQQQ", 2, 4, 248, 0, 0, 16, 16, 8)
    null_section = bytes(64)
    string_section = struct.pack(
        "<IIQQQQIIQQ", 1, 3, 0, 0, 248, 11, 0, 0, 1, 0
    )
    return (
        ident
        + header
        + program_header
        + null_section
        + string_section
        + b"\x00.shstrtab\x00"
        + marker
    )


for abi in abis:
    machine = 183 if abi == "arm64-v8a" else 62
    marker = f"fixture-{mode}-{abi}\n".encode("ascii")
    payload = stripped_elf(machine, marker)
    raw_payload = b"raw-cargo-ndk\n" + payload
    path = root / (
        "kotlin/client-android/build/generated/jniLibs/"
        f"{mode}/{abi}/{library_name}"
    )
    raw_path = root / (
        "kotlin/client-android/build/native/cargo-ndk/"
        f"{mode}/{abi}/{library_name}"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    raw_path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    raw_path.write_bytes(raw_payload)
    digest = hashlib.sha256(payload).hexdigest()
    raw_digest = hashlib.sha256(raw_payload).hexdigest()
    generated_libraries[abi] = path
    libraries[abi] = {
        "aar_path": f"jni/{abi}/{library_name}",
        "bytes": len(payload),
        "raw_bytes": len(raw_payload),
        "raw_sha256": raw_digest,
        "sha256": digest,
    }

manifest = {
    "schema": "iroha.android-native-build-provenance.v1",
    "native_bridge_abi_version": 22,
    "build_profile": "release",
    "cargo_locked": True,
    "privacy_production_enabled": production,
    "cargo_features": ["privacy-production-enabled"] if production else [],
    "build_environment": {
        "schema": "iroha.mobile-native-build-environment.v1",
        "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
        "hermetic_runner_sha256": hashlib.sha256(
            (root / "scripts/run_mobile_hermetic_command.py").read_bytes()
        ).hexdigest(),
        "environment_profile": "android-cargo",
        "environment_allowlist": [
            "ANDROID_NDK_HOME",
            "ANDROID_NDK_ROOT",
            "CARGO",
            "CARGO_BUILD_JOBS",
            "CARGO_HOME",
            "CARGO_INCREMENTAL",
            "CARGO_NET_OFFLINE",
            "CARGO_TARGET_DIR",
            "HOME",
            "LANG",
            "LC_ALL",
            "NORITO_SKIP_BINDINGS_SYNC",
            "PATH",
            "RUSTC",
            "RUSTC_BOOTSTRAP",
            "RUSTDOC",
            "RUSTUP_HOME",
            "TMPDIR",
        ],
        "cargo_build_jobs": 1,
        "rust_toolchain_channel": "1.93.1",
        "cargo_release": "1.93.1",
        "cargo_commit_hash": "2" * 40,
        "cargo_binary_sha256": "2" * 64,
        "rustc_release": "1.93.1",
        "rustc_commit_hash": "3" * 40,
        "rustc_binary_sha256": "3" * 64,
        "rustdoc_release": "1.93.1",
        "rustdoc_commit_hash": "3" * 40,
        "rustdoc_binary_sha256": "9" * 64,
        "cargo_ndk_version": "4.1.2",
        "cargo_ndk_binary_sha256": "4" * 64,
        "python_version": "3.12.9",
        "python_binary_sha256": "5" * 64,
        "git_version": "2.43.0",
        "git_binary_sha256": "6" * 64,
        "rustup_version": "1.28.2",
        "rustup_binary_sha256": "7" * 64,
        "android_ndk_revision": "28.0.12674087",
        "android_ndk_source_properties_sha256": "8" * 64,
    },
    "source_commit": "0" * 40,
    "source_tree_dirty": False,
    "source_fingerprint_sha256": "c" * 64,
    "cargo_lock_sha256": "a" * 64,
    "android_ndk_revision": "28.0.12674087",
    "strip_tool_sha256": "b" * 64,
    "libraries": libraries,
}
manifest_bytes = (json.dumps(manifest, indent=2, sort_keys=False) + "\n").encode("utf-8")
manifest_path = root / (
    "kotlin/client-android/build/generated/nativeProvenance/"
    f"{mode}/iroha/native-build-provenance-v1.json"
)
manifest_path.parent.mkdir(parents=True, exist_ok=True)
manifest_path.write_bytes(manifest_bytes)

with zipfile.ZipFile(aar, "a", compression=zipfile.ZIP_DEFLATED) as archive:
    archive.writestr("assets/iroha/native-build-provenance-v1.json", manifest_bytes)
    for abi, path in generated_libraries.items():
        if abi != omitted_aar_abi:
            archive.write(path, f"jni/{abi}/{library_name}")
PY
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
  local cargo_lock_hash
  local hermetic_runner_hash
  local kagemusha_roles_json
  local slice

  kagemusha_roles_json="$(
    "$TEST_PYTHON_BINARY" -I -S -B - \
      "$SCRIPT_DIR/validate_norito_bridge_xcframework.py" <<'PY'
import importlib.util
import json
from pathlib import Path
import sys

path = Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("mobile_sdk_fixture_validator", path)
if spec is None or spec.loader is None:
    raise SystemExit("unable to load mobile SDK fixture validator")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
print(json.dumps(module.expected_kagemusha_roles(False), separators=(",", ":")))
PY
  )"

  mkdir -p "$root/scripts"
  cp "$SCRIPT_DIR/norito_bridge_source_seal.py" \
    "$root/scripts/norito_bridge_source_seal.py"
  cp "$SCRIPT_DIR/validate_norito_bridge_xcframework.py" \
    "$root/scripts/validate_norito_bridge_xcframework.py"
  cp "$SCRIPT_DIR/run_mobile_hermetic_command.py" \
    "$root/scripts/run_mobile_hermetic_command.py"
  hermetic_runner_hash="$(
    shasum -a 256 "$root/scripts/run_mobile_hermetic_command.py" | awk '{print $1}'
  )"
  mkdir -p "$root/IrohaSwift/Sources/IrohaSwift"
  cat >"$root/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4.swift" <<'SWIFT'
enum KagemushaRecursiveSpendV4Fixture {
    static func ensureProofBackendAvailableV4() {}
    static func initSpendV4() {}
    static func appendSpendV4() {}
    static func verifySpendV4() {}
    static func buildRedeemV4() {}
    static func prepareRedemptionChangeV4() {}

    static let symbols = [
        "connect_norito_kagemusha_recursive_spend_capabilities_v4",
        "connect_norito_kagemusha_recursive_spend_init_v4",
        "connect_norito_kagemusha_recursive_spend_append_v4",
        "connect_norito_kagemusha_recursive_spend_verify_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_v4",
    ]
}
SWIFT
  "$TEST_PYTHON_BINARY" -I -S -B - "$CHECK_SCRIPT" "$root" <<'PY'
from pathlib import Path
import hashlib
import json
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
    "kagemushaRecursiveSpendRedemptionChangePrepareV4",
    "kagemushaRecursiveSpendVerifyV4",
)
swift = root / "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
swift.write_text(
    "\n".join(
        [
            *(f'let symbol_{index} = "{symbol}"' for index, symbol in enumerate(c_symbols)),
            *(f"func {method}() {{}}" for method in native_lifecycle
              if method != "kagemushaRecursiveSpendRedemptionChangePrepareV4"),
            "func kagemushaRecursiveSpendRedemptionChangePrepareV4() {",
            '    let secureFree = "connect_norito_kagemusha_secret_free_buffer"',
            "    copyKagemushaNativeSecretArchiveOutput()",
            "}",
        ]
    )
    + "\n",
    encoding="utf-8",
)

bridge_dir = root / "crates/connect_norito_bridge"
(bridge_dir / "src").mkdir(parents=True, exist_ok=True)
(bridge_dir / "include").mkdir(parents=True, exist_ok=True)
(bridge_dir / "include/connect_norito_bridge.h").write_text(
    "#define CONNECT_NORITO_BRIDGE_ABI_VERSION 22\n"
    + "\n".join(f"int {symbol}(void);" for symbol in c_symbols)
    + "\n",
    encoding="utf-8",
)
lifecycle_symbols = {
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
}
rust_lines = [
    "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = PRIVACY_BRIDGE_ABI_VERSION_V1;",
    *(
        f'pub unsafe extern "C" fn {symbol}() {{}}'
        for symbol in c_symbols
        if symbol not in lifecycle_symbols
    ),
    r'''macro_rules! kagemusha_recursive_spend_lifecycle_exports {
    (
        resolver = $resolver:path;
        verify_precheck = $verify_precheck:literal;
        init $(#[$init_attribute:meta])* => $init_name:ident, $init_worker:literal;
        append $(#[$append_attribute:meta])* => $append_name:ident, $append_worker:literal;
        verify $(#[$verify_attribute:meta])* => $verify_name:ident, $verify_worker:literal;
        redeem $(#[$redeem_attribute:meta])* => $redeem_name:ident, $redeem_worker:literal;
    ) => {
        $(#[$init_attribute])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $init_name() {}
        $(#[$append_attribute])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $append_name() {}
        $(#[$verify_attribute])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $verify_name() {}
        $(#[$redeem_attribute])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $redeem_name() {}
    };
}
kagemusha_recursive_spend_lifecycle_exports! {
    resolver = require_kagemusha_recursive_spend_artifact_binding_v4;
    verify_precheck = true;
    init => connect_norito_kagemusha_recursive_spend_init_v4, "krv4-init";
    append => connect_norito_kagemusha_recursive_spend_append_v4, "krv4-append";
    verify => connect_norito_kagemusha_recursive_spend_verify_v4, "krv4-verify";
    redeem => connect_norito_kagemusha_recursive_spend_redeem_v4, "krv4-redeem";
}''',
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
protocol = root / "crates/iroha_data_model/src/privacy/protocol.rs"
protocol.parent.mkdir(parents=True, exist_ok=True)
protocol.write_text(
    "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;\n",
    encoding="utf-8",
)

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
  cp "$SCRIPT_DIR/../crates/connect_norito_bridge/include/NoritoBridge.h" \
    "$root/crates/connect_norito_bridge/include/NoritoBridge.h"
  cp "$SCRIPT_DIR/../crates/connect_norito_bridge/module.modulemap.template" \
    "$root/crates/connect_norito_bridge/module.modulemap.template"
  cat >"$root/IrohaSwift/Package.swift" <<'SWIFT'
// swift-tools-version:5.9
import Foundation
import PackageDescription

let bridgeRelativePath = "../dist/NoritoBridge.xcframework"
let configuredArtifactDirectory = ProcessInfo.processInfo.environment[
    "MOBILE_SDK_APPLE_ARTIFACT_DIR"
]
let bridgeTargetPath = configuredArtifactDirectory == nil
    ? bridgeRelativePath
    : configuredArtifactDirectory! + "/NoritoBridge.xcframework"

let package = Package(
    name: "IrohaSwift",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    targets: [
        .binaryTarget(
            name: "NoritoBridge",
            path: bridgeTargetPath
        )
    ]
)
SWIFT
  printf '{"pins":[],"version":3}\n' >"$root/IrohaSwift/Package.resolved"

  printf '# mobile SDK checker fixture lock\n' >"$root/Cargo.lock"
  cargo_lock_hash="$(shasum -a 256 "$root/Cargo.lock" | awk '{print $1}')"
  mkdir -p "$root/dist/NoritoBridge.xcframework"
  cat >"$root/dist/NoritoBridge.xcframework/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>AvailableLibraries</key>
  <array>
    <dict>
      <key>LibraryIdentifier</key><string>ios-arm64</string>
      <key>LibraryPath</key><string>libNoritoBridge.a</string>
      <key>HeadersPath</key><string>Headers</string>
      <key>SupportedArchitectures</key><array><string>arm64</string></array>
      <key>SupportedPlatform</key><string>ios</string>
    </dict>
    <dict>
      <key>LibraryIdentifier</key><string>ios-arm64_x86_64-simulator</string>
      <key>LibraryPath</key><string>libNoritoBridge.a</string>
      <key>HeadersPath</key><string>Headers</string>
      <key>SupportedArchitectures</key><array><string>arm64</string><string>x86_64</string></array>
      <key>SupportedPlatform</key><string>ios</string>
      <key>SupportedPlatformVariant</key><string>simulator</string>
    </dict>
    <dict>
      <key>LibraryIdentifier</key><string>macos-arm64_x86_64</string>
      <key>LibraryPath</key><string>libNoritoBridge.a</string>
      <key>HeadersPath</key><string>Headers</string>
      <key>SupportedArchitectures</key><array><string>arm64</string><string>x86_64</string></array>
      <key>SupportedPlatform</key><string>macos</string>
    </dict>
  </array>
  <key>CFBundlePackageType</key><string>XFWK</string>
  <key>XCFrameworkFormatVersion</key><string>1.0</string>
</dict>
</plist>
PLIST

  for slice in ios-arm64 ios-arm64_x86_64-simulator macos-arm64_x86_64; do
    mkdir -p "$root/dist/NoritoBridge.xcframework/$slice/Headers"
    printf 'fake static library for %s\n' "$slice" >"$root/dist/NoritoBridge.xcframework/$slice/libNoritoBridge.a"
    cp "$root/crates/connect_norito_bridge/include/NoritoBridge.h" \
      "$root/dist/NoritoBridge.xcframework/$slice/Headers/NoritoBridge.h"
    cp "$root/crates/connect_norito_bridge/include/connect_norito_bridge.h" \
      "$root/dist/NoritoBridge.xcframework/$slice/Headers/connect_norito_bridge.h"
    cp "$root/crates/connect_norito_bridge/module.modulemap.template" \
      "$root/dist/NoritoBridge.xcframework/$slice/Headers/module.modulemap"
  done

  hash_a="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64/libNoritoBridge.a" | awk '{print $1}')"
  hash_b="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64_x86_64-simulator/libNoritoBridge.a" | awk '{print $1}')"
  hash_c="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/macos-arm64_x86_64/libNoritoBridge.a" | awk '{print $1}')"
  local header_hash
  header_hash="$(shasum -a 256 "$root/dist/NoritoBridge.xcframework/ios-arm64/Headers/connect_norito_bridge.h" | awk '{print $1}')"
  cat >"$root/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift" <<SWIFT
enum NoritoBridgeLoader {
    static let expectedVersion = "1.0.0"
    private static let expectedHashes: [String: String] = [
        "macos-arm64_x86_64": "$hash_c",
        "ios-arm64": "$hash_a",
        "ios-arm64_x86_64-simulator": "$hash_b"
    ]
    private static let requiredSymbols = [
        "connect_norito_bridge_abi_version",
        "connect_norito_free",
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
        "connect_norito_sorafs_reference_validate_bundle_json",
        "connect_norito_sorafs_reference_validate_governance_json",
        "connect_norito_sorafs_reference_validate_governance_dag_block_json",
        "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json"
    ]
}
SWIFT

  cat >"$root/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" <<JSON
{
  "version": "1.0.0",
  "native_bridge_abi_version": 22,
  "privacy_production_enabled": false,
  "cargo_features": [],
  "build_environment": {
    "schema": "iroha.mobile-native-build-environment.v1",
    "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
    "hermetic_runner_sha256": "$hermetic_runner_hash",
    "environment_profiles": {
      "apple-ios-device": [
        "CARGO", "CARGO_BUILD_JOBS", "CARGO_HOME", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR", "DEVELOPER_DIR", "HOME",
        "IPHONEOS_DEPLOYMENT_TARGET", "LANG", "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC", "PATH", "RUSTC", "RUSTC_BOOTSTRAP", "RUSTDOC",
        "RUSTUP_HOME", "SDKROOT", "TMPDIR"
      ],
      "apple-ios-simulator": [
        "CARGO", "CARGO_BUILD_JOBS", "CARGO_HOME", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR", "DEVELOPER_DIR", "HOME",
        "IPHONEOS_DEPLOYMENT_TARGET", "IPHONESIMULATOR_DEPLOYMENT_TARGET",
        "LANG", "LC_ALL", "NORITO_SKIP_BINDINGS_SYNC", "PATH", "RUSTC",
        "RUSTC_BOOTSTRAP", "RUSTDOC", "RUSTUP_HOME", "SDKROOT", "TMPDIR"
      ],
      "apple-macos": [
        "CARGO", "CARGO_BUILD_JOBS", "CARGO_HOME", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR", "DEVELOPER_DIR", "HOME", "LANG", "LC_ALL",
        "MACOSX_DEPLOYMENT_TARGET", "NORITO_SKIP_BINDINGS_SYNC", "PATH",
        "RUSTC", "RUSTC_BOOTSTRAP", "RUSTDOC", "RUSTUP_HOME", "SDKROOT", "TMPDIR"
      ]
    },
    "cargo_build_jobs": 1,
    "rust_toolchain_channel": "1.93.1",
    "cargo_release": "1.93.1",
    "cargo_commit_hash": "2222222222222222222222222222222222222222",
    "cargo_binary_sha256": "2222222222222222222222222222222222222222222222222222222222222222",
    "rustc_release": "1.93.1",
    "rustc_commit_hash": "3333333333333333333333333333333333333333",
    "rustc_binary_sha256": "3333333333333333333333333333333333333333333333333333333333333333",
    "rustdoc_release": "1.93.1",
    "rustdoc_commit_hash": "3333333333333333333333333333333333333333",
    "rustdoc_binary_sha256": "7777777777777777777777777777777777777777777777777777777777777777",
    "python_version": "3.12.9",
    "python_binary_sha256": "4444444444444444444444444444444444444444444444444444444444444444",
    "git_version": "2.43.0",
    "git_binary_sha256": "5555555555555555555555555555555555555555555555555555555555555555",
    "rustup_version": "1.28.2",
    "rustup_binary_sha256": "6666666666666666666666666666666666666666666666666666666666666666",
    "xcode_version": "16.2",
    "xcode_build_version": "16C5032a",
    "iphoneos_sdk_version": "18.2",
    "iphonesimulator_sdk_version": "18.2",
    "macosx_sdk_version": "15.2",
    "iphoneos_deployment_target": "15.0",
    "iphonesimulator_deployment_target": "15.0",
    "macosx_deployment_target": "12.0"
  },
  "source_commit": "0000000000000000000000000000000000000000",
  "source_tree_dirty": false,
  "source_fingerprint_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
  "cargo_lock_sha256": "$cargo_lock_hash",
  "bridge_header_sha256": "$header_hash",
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
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
    "connect_norito_kagemusha_secret_free_buffer",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_recipient_lineage_query_create_v2",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v2",
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
    "connect_norito_kagemusha_request_authorization_create_v2",
    "iroha_privacy_capabilities_v1",
    "iroha_privacy_validate_capabilities_v1",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2",
    "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2"
  ],
  "kagemusha_mobile_artifact_roles": $kagemusha_roles_json,
  "hashes": {
    "ios-arm64": "$hash_a",
    "ios-arm64_x86_64-simulator": "$hash_b",
    "macos-arm64_x86_64": "$hash_c"
  }
}
JSON
  ln -s \
    "NoritoBridge.xcframework/NoritoBridge.artifacts.json" \
    "$root/dist/NoritoBridge.artifacts.json"

  mkdir -p "$root/kotlin/client-android/src/main"
  cat >"$root/kotlin/settings.gradle.kts" <<'SETTINGS'
rootProject.name = "iroha_kotlin_sdk"
include(":core-jvm")
include(":client-android")
SETTINGS
  make_gradle_file "$root/kotlin/core-jvm/build.gradle.kts" "core-jvm"
  make_gradle_file "$root/kotlin/client-android/build.gradle.kts" "client-android"
  printf '<manifest />\n' >"$root/kotlin/client-android/src/main/AndroidManifest.xml"

  local keymint_source="$root/java/iroha_android/android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaAndroidKeyMint.java"
  mkdir -p "$(dirname "$keymint_source")"
  cp "$SCRIPT_DIR/../java/iroha_android/android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaAndroidKeyMint.java" \
    "$keymint_source"
}
