#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_SCRIPT="$SCRIPT_DIR/check_mobile_sdk_artifacts.sh"
PACKAGE_SCRIPT="$SCRIPT_DIR/package_mobile_sdk_artifacts.sh"
TMP_DIR="$(mktemp -d)"
TEST_PYTHON_BINARY=""
for trusted_python in \
  /opt/homebrew/bin/python3.12 \
  /opt/homebrew/opt/python@3.12/bin/python3.12 \
  /usr/local/bin/python3.12 \
  /usr/local/opt/python@3.12/bin/python3.12 \
  /opt/homebrew/bin/python3 \
  /usr/local/bin/python3 \
  /usr/bin/python3; do
  if [[ -x "$trusted_python" ]] \
    && [[ "$("$trusted_python" -I -S -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null)" == "3.12" ]]; then
    TEST_PYTHON_BINARY="$trusted_python"
    break
  fi
done
[[ -n "$TEST_PYTHON_BINARY" ]] || {
  printf '[mobile-sdk-artifacts-test] ERROR: pinned Python 3.12 is required\n' >&2
  exit 1
}
TEST_PYTHON_BINARY="$("$TEST_PYTHON_BINARY" -I -S -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$TEST_PYTHON_BINARY")"
export MOBILE_SDK_PYTHON_BINARY="$TEST_PYTHON_BINARY"
export MOBILE_SDK_TEST_PYTHON_BINARY="$TEST_PYTHON_BINARY"

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
  cp "$SCRIPT_DIR/exec_with_file_lock.py" \
    "$root/scripts/exec_with_file_lock.py"
  cp "$SCRIPT_DIR/norito_bridge_source_seal.py" \
    "$root/scripts/norito_bridge_source_seal.py"
  cp "$SCRIPT_DIR/run_mobile_hermetic_command.py" \
    "$root/scripts/run_mobile_hermetic_command.py"
  printf '[toolchain]\nchannel = "1.93.1"\n' >"$root/rust-toolchain.toml"
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

  local hostile_bin="$root/hostile-bin"
  local hostile_marker="$root/hostile-tool-invoked"
  mkdir -p "$hostile_bin"
  local tool_name
  for tool_name in python3 git rustup cargo rustc nm; do
    printf '#!/bin/sh\nprintf "%%s\\n" "$0" >>"%s"\nexit 97\n' "$hostile_marker" \
      >"$hostile_bin/$tool_name"
    chmod 0700 "$hostile_bin/$tool_name"
  done
  PATH="$hostile_bin" \
    HOME="$root/forged-home" \
    RUSTFLAGS="-C link-arg=forged" \
    CARGO_ENCODED_RUSTFLAGS="forged" \
    RUSTC_WRAPPER="$hostile_bin/rustc-wrapper" \
    RUSTC_WORKSPACE_WRAPPER="$hostile_bin/workspace-wrapper" \
    GIT_DIR="$root/forged-git-dir" \
    GIT_WORK_TREE="$root/forged-work-tree" \
    GIT_INDEX_FILE="$root/forged-index" \
    GIT_CONFIG_GLOBAL="$root/forged-git-config" \
    NORITO_BRIDGE_SOURCE_SEAL_TEST_ONLY=1 \
    /bin/bash "$root/scripts/build_norito_xcframework.sh"
  [[ ! -e "$hostile_marker" ]] \
    || fail "Apple builder trusted a hostile ambient PATH tool"

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

test_hermetic_command_environment() {
  local probe="$TMP_DIR/hermetic-environment-probe"
  local output="$TMP_DIR/hermetic-environment.txt"
  local rejected_output
  printf '%s\n' \
    "#!$TEST_PYTHON_BINARY" \
    'import os' \
    'from pathlib import Path' \
    'import sys' \
    'Path(sys.argv[1]).write_text("".join(f"{key}={value}\n" for key, value in sorted(os.environ.items())), encoding="utf-8")' \
    >"$probe"
  chmod 0700 "$probe"
  RUSTFLAGS="-C link-arg=forged" \
    CARGO_ENCODED_RUSTFLAGS="forged" \
    RUSTC_WRAPPER="$TMP_DIR/forged-wrapper" \
    RUSTC_WORKSPACE_WRAPPER="$TMP_DIR/forged-workspace-wrapper" \
    CC="$TMP_DIR/forged-cc" \
    SDKROOT="$TMP_DIR/forged-sdk" \
    "$TEST_PYTHON_BINARY" -I -S "$SCRIPT_DIR/run_mobile_hermetic_command.py" \
      --profile host-cargo \
      --set "CARGO=/reviewed/cargo" \
      --set "CARGO_HOME=/reviewed/cargo-home" \
      --set "CARGO_INCREMENTAL=0" \
      --set "CARGO_NET_OFFLINE=true" \
      --set "CARGO_TARGET_DIR=/reviewed/cargo-target" \
      --set "HOME=/reviewed/home" \
      --set "LANG=C.UTF-8" \
      --set "LC_ALL=C.UTF-8" \
      --set "NORITO_SKIP_BINDINGS_SYNC=1" \
      --set "PATH=/reviewed/toolchain/bin:/usr/bin:/bin" \
      --set "RUSTC=/reviewed/rustc" \
      --set "RUSTUP_HOME=/reviewed/rustup-home" \
      --set "TMPDIR=/tmp" \
      -- "$probe" "$output"
  "$TEST_PYTHON_BINARY" -I -S - "$output" <<'PY'
from pathlib import Path
import sys

environment = {}
for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    name, value = line.split("=", 1)
    if name in environment:
        raise SystemExit(f"duplicate probe environment key: {name}")
    environment[name] = value
# macOS injects this process-local CoreFoundation encoding marker after exec;
# it is not inherited from the caller and has no compiler/tool selection role.
environment.pop("__CF_USER_TEXT_ENCODING", None)
expected = {
    "CARGO",
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
    "RUSTUP_HOME",
    "TMPDIR",
}
if set(environment) != expected:
    raise SystemExit(f"hermetic Cargo environment is not exact: {sorted(environment)}")
if environment["CARGO_NET_OFFLINE"] != "true":
    raise SystemExit("hermetic Cargo environment is not offline")
PY
  if rejected_output="$(
      "$TEST_PYTHON_BINARY" -I -S "$SCRIPT_DIR/run_mobile_hermetic_command.py" \
        --profile host-cargo \
        --set "RUSTFLAGS=forged" \
        -- "$probe" "$output" 2>&1
    )"; then
    fail "hermetic command accepted an undeclared Rust flag"
  fi
  case "$rejected_output" in
    *"environment inventory is not exact"*) ;;
    *) fail "hermetic command rejected a Rust flag without an explicit inventory error" ;;
  esac
}

test_android_strip_invocation_contract() {
  local build_file="$SCRIPT_DIR/../kotlin/client-android/build.gradle.kts"
  local probe_root="$TMP_DIR/android-strip-invocation"
  local fake_objcopy="$probe_root/llvm-objcopy"
  local fake_strip="$probe_root/llvm-strip"
  local resolved_strip
  local arm_library="$probe_root/arm64-v8a/libconnect_norito_bridge.so"
  local x86_library="$probe_root/x86_64/libconnect_norito_bridge.so"
  local rejected_library="$probe_root/reject-library.so"
  local library

  grep -Fq 'fun canonicalStripCommands(' "$build_file" \
    || fail "Android native build must centralize canonical strip commands"
  grep -Fq 'return libraryPaths.map { libraryPath ->' "$build_file" \
    || fail "Android native build must create one strip command per ABI path"
  grep -Fq ').forEach { stripCommand ->' "$build_file" \
    || fail "Android native build must execute canonical strip commands independently"
  grep -Fq 'commandLine(*stripCommand.toTypedArray())' "$build_file" \
    || fail "Android native build must execute only one canonical strip command at a time"
  if grep -Fq '*outputLibraries.map { it.absolutePath }.toTypedArray()' "$build_file"; then
    fail "Android native build must not batch ABI libraries as llvm-objcopy positionals"
  fi

  mkdir -p "$(dirname "$arm_library")" "$(dirname "$x86_library")"
  cat >"$fake_objcopy" <<'SH'
#!/bin/sh
set -eu
if [ "$#" -ne 2 ] || [ "$1" != "--strip-unneeded" ]; then
  exit 64
fi
case "$2" in
  *reject*)
    exit 65
    ;;
esac
printf 'stripped\n' >>"$2"
SH
  chmod 0700 "$fake_objcopy"
  ln -s llvm-objcopy "$fake_strip"
  resolved_strip="$(
    "$TEST_PYTHON_BINARY" -I -S -c \
      'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
      "$fake_strip"
  )"
  [[ "$resolved_strip" -ef "$fake_objcopy" ]] \
    || fail "fake llvm-strip launcher did not resolve to authenticated llvm-objcopy"

  printf 'arm64-v8a-original\n' >"$arm_library"
  printf 'x86_64-original\n' >"$x86_library"
  for library in "$arm_library" "$x86_library"; do
    "$resolved_strip" --strip-unneeded "$library" \
      || fail "independent Android strip invocation rejected ${library##*/}"
  done
  [[ "$(<"$arm_library")" == $'arm64-v8a-original\nstripped' ]] \
    || fail "independent Android stripping skipped or cross-overwrote arm64-v8a"
  [[ "$(<"$x86_library")" == $'x86_64-original\nstripped' ]] \
    || fail "independent Android stripping skipped or cross-overwrote x86_64"

  if "$resolved_strip" --strip-unneeded "$arm_library" "$x86_library"; then
    fail "strip regression fixture accepted a multi-positional ABI batch"
  fi
  printf 'must-remain\n' >"$rejected_library"
  if "$resolved_strip" --strip-unneeded "$rejected_library"; then
    fail "strip regression fixture accepted an adversarial rejected ABI"
  fi
  [[ "$(<"$rejected_library")" == "must-remain" ]] \
    || fail "failed Android strip invocation mutated a rejected ABI library"
}

if [[ "${MOBILE_SDK_ANDROID_STRIP_INVOCATION_TEST_ONLY:-0}" == "1" ]]; then
  test_android_strip_invocation_contract
  echo "[mobile-sdk-artifacts-test] Android strip invocation contract passed"
  exit 0
fi

if [[ "${MOBILE_SDK_SKIP_SOURCE_SEAL_SELF_TEST:-0}" != "1" ]]; then
  test_build_source_seal
  test_hermetic_command_environment
  test_android_strip_invocation_contract
fi

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
  "$TEST_PYTHON_BINARY" -I -S - "$root" "$aar" "$mode" "$omitted_aar_abi" <<'PY'
import hashlib
import json
from pathlib import Path
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
for abi in abis:
    payload = f"fixture-{mode}-{abi}\n".encode("ascii")
    raw_payload = f"raw-fixture-{mode}-{abi}\n".encode("ascii")
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
    "native_bridge_abi_version": 21,
    "build_profile": "release",
    "cargo_locked": True,
    "privacy_production_enabled": production,
    "cargo_features": ["privacy-production-enabled"] if production else [],
    "build_environment": {
        "schema": "iroha.mobile-native-build-environment.v1",
        "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
        "hermetic_runner_sha256": "1" * 64,
        "environment_profile": "android-cargo",
        "environment_allowlist": [
            "ANDROID_NDK_HOME",
            "ANDROID_NDK_ROOT",
            "CARGO",
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
            "RUSTUP_HOME",
            "TMPDIR",
        ],
        "rust_toolchain_channel": "1.93.1",
        "cargo_release": "1.93.1",
        "cargo_commit_hash": "2" * 40,
        "cargo_binary_sha256": "2" * 64,
        "rustc_release": "1.93.1",
        "rustc_commit_hash": "3" * 40,
        "rustc_binary_sha256": "3" * 64,
        "cargo_ndk_version": "4.1.2",
        "cargo_ndk_binary_sha256": "4" * 64,
        "python_version": "3.11.9",
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
  local slice

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
  "$TEST_PYTHON_BINARY" -I -S - "$CHECK_SCRIPT" "$root" <<'PY'
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
    "\n".join(f"int {symbol}(void);" for symbol in c_symbols) + "\n",
    encoding="utf-8",
)
rust_lines = [
    "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 21;",
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
        .iOS(.v15)
    ],
    targets: [
        .binaryTarget(
            name: "NoritoBridge",
            path: bridgeTargetPath
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
  cat >"$root/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift" <<SWIFT
enum NativeBridgeFixture {
    static let manifestlessFallbackHashes = [
        "ios-arm64": "$hash_a",
        "ios-arm64_x86_64-simulator": "$hash_b",
        "macos-arm64": "$hash_c",
    ]
}
SWIFT

  cat >"$root/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" <<JSON
{
  "version": "1.0.0",
  "native_bridge_abi_version": 21,
  "privacy_production_enabled": false,
  "cargo_features": [],
  "build_environment": {
    "schema": "iroha.mobile-native-build-environment.v1",
    "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
    "hermetic_runner_sha256": "1111111111111111111111111111111111111111111111111111111111111111",
    "environment_profiles": {
      "apple-ios-device": [
        "CARGO", "CARGO_HOME", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR", "DEVELOPER_DIR", "HOME",
        "IPHONEOS_DEPLOYMENT_TARGET", "LANG", "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC", "PATH", "RUSTC", "RUSTUP_HOME",
        "SDKROOT", "TMPDIR"
      ],
      "apple-ios-simulator": [
        "CARGO", "CARGO_HOME", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR", "DEVELOPER_DIR", "HOME",
        "IPHONEOS_DEPLOYMENT_TARGET", "IPHONESIMULATOR_DEPLOYMENT_TARGET",
        "LANG", "LC_ALL", "NORITO_SKIP_BINDINGS_SYNC", "PATH", "RUSTC",
        "RUSTUP_HOME", "SDKROOT", "TMPDIR"
      ],
      "apple-macos": [
        "CARGO", "CARGO_HOME", "CARGO_INCREMENTAL", "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR", "DEVELOPER_DIR", "HOME", "LANG", "LC_ALL",
        "MACOSX_DEPLOYMENT_TARGET", "NORITO_SKIP_BINDINGS_SYNC", "PATH",
        "RUSTC", "RUSTUP_HOME", "SDKROOT", "TMPDIR"
      ]
    },
    "rust_toolchain_channel": "1.93.1",
    "cargo_release": "1.93.1",
    "cargo_commit_hash": "2222222222222222222222222222222222222222",
    "cargo_binary_sha256": "2222222222222222222222222222222222222222222222222222222222222222",
    "rustc_release": "1.93.1",
    "rustc_commit_hash": "3333333333333333333333333333333333333333",
    "rustc_binary_sha256": "3333333333333333333333333333333333333333333333333333333333333333",
    "python_version": "3.11.9",
    "python_binary_sha256": "4444444444444444444444444444444444444444444444444444444444444444",
    "git_version": "2.43.0",
    "git_binary_sha256": "5555555555555555555555555555555555555555555555555555555555555555",
    "rustup_version": "1.28.2",
    "rustup_binary_sha256": "6666666666666666666666666666666666666666666666666666666666666666",
    "xcode_version": "16.2",
    "xcode_build_version": "16C5032a",
    "iphoneos_sdk_version": "18.2",
    "iphonesimulator_sdk_version": "18.2",
    "macosx_sdk_version": "15.2"
  },
  "source_commit": "0000000000000000000000000000000000000000",
  "source_tree_dirty": false,
  "source_fingerprint_sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
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
  "hashes": {
    "ios-arm64": "$hash_a",
    "ios-arm64_x86_64-simulator": "$hash_b",
    "macos-arm64": "$hash_c"
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

append_candidate_lab_source() {
  local root="$1"
  "$TEST_PYTHON_BINARY" -I -S - "$CHECK_SCRIPT" "$root" <<'PY'
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
  "$TEST_PYTHON_BINARY" -I -S - "$CHECK_SCRIPT" "$root" <<'PY'
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
"${MOBILE_SDK_TEST_PYTHON_BINARY:?}" -I -S - \
  "$root/dist/NoritoBridge.artifacts.json" "$binary" <<'PY'
import json
import os
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    manifest = json.load(handle)
for symbol in manifest["required_symbols"]:
    print("_" + symbol)
if os.environ.get("MOBILE_SDK_TEST_EXTRA_KAGEMUSHA") == "1":
    print("_connect_norito_kagemusha_unexpected_v2")
forbidden = os.environ.get("MOBILE_SDK_TEST_FORBIDDEN_SYMBOL")
target = os.environ.get("MOBILE_SDK_TEST_FORBIDDEN_APPLE_SLICE")
if forbidden and (not target or f"/{target}/" in sys.argv[2]):
    print("_" + forbidden)
PY
SH
  chmod +x "$tools/lipo" "$tools/nm"
}

make_android_inspection_tools() {
  local tools="$1"
  mkdir -p "$tools"
  cat >"$tools/llvm-nm" <<'SH'
#!/usr/bin/env bash
binary="${*: -1}"
"${MOBILE_SDK_TEST_PYTHON_BINARY:?}" -I -S - \
  "${MOBILE_SDK_TEST_CHECK_SCRIPT:?}" "$binary" <<'PY'
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

omitted = os.environ.get("MOBILE_SDK_TEST_OMIT_ANDROID_SYMBOL")

def emit(symbol):
    if symbol != omitted:
        print(symbol)

emit("connect_norito_bridge_abi_version")
for symbol in shell_array("KAGEMUSHA_C_SYMBOLS"):
    emit(symbol)
for symbol in shell_array("SORAFS_APPEAL_FINANCE_C_SYMBOLS"):
    emit(symbol)
for namespace in (
    "org_hyperledger_iroha_sdk_offline",
    "org_hyperledger_iroha_android_offline",
):
    for method in shell_array("KAGEMUSHA_JNI_METHODS"):
        emit(f"Java_{namespace}_KagemushaRecursiveSpendProver_{method}")
for symbol in shell_array("VALIDATION_FEE_JNI_SYMBOLS"):
    emit(symbol)
for symbol in shell_array("SORAFS_APPEAL_FINANCE_JNI_SYMBOLS"):
    emit(symbol)
for symbol in shell_array("NATIVE_SIGNER_JNI_CONTRACT_SYMBOLS"):
    emit(symbol)
if os.environ.get("MOBILE_SDK_TEST_EXTRA_ANDROID_KAGEMUSHA") == "1":
    print("connect_norito_kagemusha_recursive_spend_init_v3")
forbidden = os.environ.get("MOBILE_SDK_TEST_FORBIDDEN_SYMBOL")
target = os.environ.get("MOBILE_SDK_TEST_FORBIDDEN_ANDROID_ABI")
if forbidden and (not target or f"/{target}/" in sys.argv[2]):
    print(forbidden)
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

run_expect_apple_forbidden_binary_fail() {
  local root="$1"
  local symbol="$2"
  local slice="$3"
  local tools="$4"
  local output
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_FORBIDDEN_SYMBOL="$symbol" \
      MOBILE_SDK_TEST_FORBIDDEN_APPLE_SLICE="$slice" \
      bash "$CHECK_SCRIPT" "$root" --apple-only 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected Apple validation to reject forbidden symbol $symbol in $slice"
  fi
  case "$output" in
    *"NoritoBridge $slice exports forbidden first-release symbol $symbol"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected Apple forbidden-symbol failure for $symbol in $slice"
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

run_expect_android_missing_symbol_fail() {
  local root="$1"
  local symbol="$2"
  local tools="$3"
  local output
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_ANDROID_NM="$tools/llvm-nm" \
      MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_OMIT_ANDROID_SYMBOL="$symbol" \
      bash "$CHECK_SCRIPT" "$root" --android-only --require-built-android 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected Android validation to reject missing symbol $symbol"
  fi
  case "$output" in
    *"bridge is missing ABI21/V4 symbols:"*"$symbol"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected Android missing-symbol failure for $symbol"
      ;;
  esac
}

run_expect_android_forbidden_binary_fail() {
  local root="$1"
  local symbol="$2"
  local abi="$3"
  local tools="$4"
  local output
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_ANDROID_NM="$tools/llvm-nm" \
      MOBILE_SDK_SKIP_BINARY_INSPECTION=0 \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_FORBIDDEN_SYMBOL="$symbol" \
      MOBILE_SDK_TEST_FORBIDDEN_ANDROID_ABI="$abi" \
      bash "$CHECK_SCRIPT" "$root" --android-only --require-built-android 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected Android validation to reject forbidden symbol $symbol in $abi"
  fi
  case "$output" in
    *"client-android $abi bridge exports forbidden first-release symbol $symbol"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected Android forbidden-symbol failure for $symbol in $abi"
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

custom_apple_artifact_dir="$TMP_DIR/custom-apple-artifact-dir"
make_fixture "$custom_apple_artifact_dir"
mv \
  "$custom_apple_artifact_dir/dist" \
  "$custom_apple_artifact_dir/staged-apple"
MOBILE_SDK_APPLE_ARTIFACT_DIR="$custom_apple_artifact_dir/staged-apple" \
  run_expect_pass "$custom_apple_artifact_dir" --apple-only

regular_public_manifest="$TMP_DIR/regular-public-manifest"
make_fixture "$regular_public_manifest"
rm "$regular_public_manifest/dist/NoritoBridge.artifacts.json"
cp \
  "$regular_public_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" \
  "$regular_public_manifest/dist/NoritoBridge.artifacts.json"
run_expect_fail \
  "$regular_public_manifest" \
  "public NoritoBridge artifact manifest must be a relative symlink"

absolute_public_manifest="$TMP_DIR/absolute-public-manifest"
make_fixture "$absolute_public_manifest"
rm "$absolute_public_manifest/dist/NoritoBridge.artifacts.json"
ln -s \
  "$absolute_public_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" \
  "$absolute_public_manifest/dist/NoritoBridge.artifacts.json"
run_expect_fail \
  "$absolute_public_manifest" \
  "public NoritoBridge artifact manifest has a non-canonical symlink target"

noncanonical_public_manifest="$TMP_DIR/noncanonical-public-manifest"
make_fixture "$noncanonical_public_manifest"
rm "$noncanonical_public_manifest/dist/NoritoBridge.artifacts.json"
ln -s \
  "./NoritoBridge.xcframework/NoritoBridge.artifacts.json" \
  "$noncanonical_public_manifest/dist/NoritoBridge.artifacts.json"
run_expect_fail \
  "$noncanonical_public_manifest" \
  "public NoritoBridge artifact manifest has a non-canonical symlink target"

dangling_public_manifest="$TMP_DIR/dangling-public-manifest"
make_fixture "$dangling_public_manifest"
rm \
  "$dangling_public_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
run_expect_fail \
  "$dangling_public_manifest" \
  "missing embedded NoritoBridge artifact manifest"

mismatched_public_manifest="$TMP_DIR/mismatched-public-manifest"
make_fixture "$mismatched_public_manifest"
cp \
  "$mismatched_public_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" \
  "$mismatched_public_manifest/dist/NoritoBridge.public.artifacts.json"
sed -i.bak 's/"version": "1.0.0"/"version": "mismatched"/' \
  "$mismatched_public_manifest/dist/NoritoBridge.public.artifacts.json"
rm -f "$mismatched_public_manifest/dist/NoritoBridge.public.artifacts.json.bak"
rm "$mismatched_public_manifest/dist/NoritoBridge.artifacts.json"
ln -s \
  "NoritoBridge.public.artifacts.json" \
  "$mismatched_public_manifest/dist/NoritoBridge.artifacts.json"
run_expect_fail \
  "$mismatched_public_manifest" \
  "public NoritoBridge artifact manifest has a non-canonical symlink target"

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
"$TEST_PYTHON_BINARY" -I -S - "$candidate_lab_source_without_export" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S - "$unguarded_candidate_lab_source" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S - "$extra_candidate_lab_header" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S - "$commented_candidate_lab_header" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S - "$commented_candidate_marker_header" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S - "$unguarded_candidate_lab_marker" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S - "$unguarded_candidate_lab_jni" <<'PY'
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
run_expect_fail "$retired_swift_binding" "Swift Kagemusha native symbol inventory is not exact ABI-21/V4"

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
const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 21;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_init_v3() {}
RUST
run_expect_fail "$retired_bridge_source" "retired or unexpected Kagemusha C exports"

retired_kotlin_native="$TMP_DIR/retired-kotlin-native"
make_fixture "$retired_kotlin_native"
printf '%s\n' 'private external fun nativeArtifactBindingV3()' \
  >>"$retired_kotlin_native/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
run_expect_fail "$retired_kotlin_native" "native method inventory is not exact ABI-21/V4" --android-only

retired_java_native="$TMP_DIR/retired-java-native"
make_fixture "$retired_java_native"
printf '%s\n' 'private static native void nativeArtifactBindingV3();' \
  >>"$retired_java_native/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
run_expect_fail "$retired_java_native" "native method inventory is not exact ABI-21/V4" --android-only

retired_rust_jni="$TMP_DIR/retired-rust-jni"
make_fixture "$retired_rust_jni"
printf '%s\n' \
  'fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactBindingV3() {}' \
  >>"$retired_rust_jni/crates/connect_norito_bridge/src/lib.rs"
run_expect_fail "$retired_rust_jni" "Rust bridge exposes retired or unexpected Kagemusha JNI exports" --android-only

wrong_bridge_abi="$TMP_DIR/wrong-bridge-abi"
make_fixture "$wrong_bridge_abi"
sed -i.bak 's/"native_bridge_abi_version": 21/"native_bridge_abi_version": 20/' \
  "$wrong_bridge_abi/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$wrong_bridge_abi/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$wrong_bridge_abi" "exact first-release NoritoBridge ABI 21"

tampered_apple_build_environment="$TMP_DIR/tampered-apple-build-environment"
make_fixture "$tampered_apple_build_environment"
"$TEST_PYTHON_BINARY" -I -S - "$tampered_apple_build_environment" <<'PY'
import json
from pathlib import Path
import sys

manifest = Path(sys.argv[1]) / "dist/NoritoBridge.artifacts.json"
payload = json.loads(manifest.read_text(encoding="utf-8"))
payload["build_environment"]["environment_profiles"]["apple-macos"].append("RUSTFLAGS")
manifest.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
run_expect_fail \
  "$tampered_apple_build_environment" \
  "artifact build_environment is missing, malformed, or not hermetic"

enabled_privacy="$TMP_DIR/enabled-privacy"
make_fixture "$enabled_privacy"
sed -i.bak \
  -e 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  -e 's/"cargo_features": \[\]/"cargo_features": ["privacy-production-enabled"]/' \
  "$enabled_privacy/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$enabled_privacy/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
touch "$enabled_privacy/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_pass "$enabled_privacy"

enabled_without_marker="$TMP_DIR/enabled-without-marker"
make_fixture "$enabled_without_marker"
sed -i.bak \
  -e 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  -e 's/"cargo_features": \[\]/"cargo_features": ["privacy-production-enabled"]/' \
  "$enabled_without_marker/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$enabled_without_marker/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$enabled_without_marker" "missing privacy-production-enabled XCFramework marker"

default_with_marker="$TMP_DIR/default-with-marker"
make_fixture "$default_with_marker"
touch "$default_with_marker/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail "$default_with_marker" "default privacy artifact must not carry the privacy-production-enabled XCFramework marker"

missing_production_cargo_features="$TMP_DIR/missing-production-cargo-features"
make_fixture "$missing_production_cargo_features"
sed -i.bak \
  -e 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  -e '/"cargo_features": \[\],/d' \
  "$missing_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$missing_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
touch "$missing_production_cargo_features/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail \
  "$missing_production_cargo_features" \
  'cargo_features must be exactly ["privacy-production-enabled"]'

empty_production_cargo_features="$TMP_DIR/empty-production-cargo-features"
make_fixture "$empty_production_cargo_features"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  "$empty_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$empty_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
touch "$empty_production_cargo_features/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail \
  "$empty_production_cargo_features" \
  'cargo_features must be exactly ["privacy-production-enabled"]'

extra_production_cargo_features="$TMP_DIR/extra-production-cargo-features"
make_fixture "$extra_production_cargo_features"
sed -i.bak \
  -e 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  -e 's/"cargo_features": \[\]/"cargo_features": ["privacy-production-enabled", "unexpected-feature"]/' \
  "$extra_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$extra_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
touch "$extra_production_cargo_features/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail \
  "$extra_production_cargo_features" \
  'cargo_features must be exactly ["privacy-production-enabled"]'

wrong_production_cargo_features="$TMP_DIR/wrong-production-cargo-features"
make_fixture "$wrong_production_cargo_features"
sed -i.bak \
  -e 's/"privacy_production_enabled": false/"privacy_production_enabled": true/' \
  -e 's/"cargo_features": \[\]/"cargo_features": ["wrong-feature"]/' \
  "$wrong_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$wrong_production_cargo_features/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
touch "$wrong_production_cargo_features/dist/NoritoBridge.xcframework/.privacy-production-enabled"
run_expect_fail \
  "$wrong_production_cargo_features" \
  'cargo_features must be exactly ["privacy-production-enabled"]'

default_with_production_cargo_feature="$TMP_DIR/default-with-production-cargo-feature"
make_fixture "$default_with_production_cargo_feature"
sed -i.bak \
  's/"cargo_features": \[\]/"cargo_features": ["privacy-production-enabled"]/' \
  "$default_with_production_cargo_feature/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$default_with_production_cargo_feature/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail \
  "$default_with_production_cargo_feature" \
  "default NoritoBridge artifact cargo_features must be exactly []"

invalid_privacy_state="$TMP_DIR/invalid-privacy-state"
make_fixture "$invalid_privacy_state"
sed -i.bak 's/"privacy_production_enabled": false/"privacy_production_enabled": "false"/' \
  "$invalid_privacy_state/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$invalid_privacy_state/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$invalid_privacy_state" "must contain exactly one boolean privacy_production_enabled field"

dirty_source_manifest="$TMP_DIR/dirty-source-manifest"
make_fixture "$dirty_source_manifest"
sed -i.bak 's/"source_tree_dirty": false/"source_tree_dirty": true/' \
  "$dirty_source_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$dirty_source_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$dirty_source_manifest" "release artifact must be built from a clean source tree"
MOBILE_SDK_ALLOW_DIRTY_SOURCE=1 run_expect_pass "$dirty_source_manifest"

duplicate_mixed_privacy_state="$TMP_DIR/duplicate-mixed-privacy-state"
make_fixture "$duplicate_mixed_privacy_state"
sed -i.bak '/"privacy_production_enabled": false/a\
  "privacy_production_enabled": "false",' \
  "$duplicate_mixed_privacy_state/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$duplicate_mixed_privacy_state/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
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
cat >"$missing_hash/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" <<'JSON'
{
  "version": "1.0.0",
  "privacy_production_enabled": false,
  "cargo_features": [],
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
"$TEST_PYTHON_BINARY" -I -S - "$candidate_marker_apple" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

root = Path(sys.argv[1])
manifest_path = root / (
    "dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
)
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
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "connect_norito_get_chain_discriminant" \
  "ios-arm64" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "connect_norito_set_chain_discriminant" \
  "macos-arm64" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "connect_norito_kagemusha_recipient_registration_lineage_verify_v1" \
  "ios-arm64_x86_64-simulator" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "connect_norito_kagemusha_request_authorization_create_v2" \
  "ios-arm64" \
  "$inspection_tools"

symbol_inventory_mismatch="$TMP_DIR/symbol-inventory-mismatch"
make_fixture "$symbol_inventory_mismatch"
sed -i.bak \
  's/connect_norito_canonical_json_blake3_v1/unexpected_symbol/' \
  "$symbol_inventory_mismatch/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$symbol_inventory_mismatch/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$symbol_inventory_mismatch" "required symbol inventory is missing or non-canonical"

extra_manifest_symbol="$TMP_DIR/extra-manifest-symbol"
make_fixture "$extra_manifest_symbol"
sed -i.bak '/"connect_norito_kagemusha_recursive_spend_init_v4",/i\
    "connect_norito_kagemusha_unexpected_v2",' \
  "$extra_manifest_symbol/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$extra_manifest_symbol/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
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
run_expect_fail \
  "$missing_client_native_source" \
  "assets/iroha/native-build-provenance-v1.json" \
  --require-built-android

missing_client_native_aar_entry="$TMP_DIR/missing-client-native-aar-entry"
make_fixture "$missing_client_native_aar_entry"
make_android_outputs "$missing_client_native_aar_entry" default x86_64
run_expect_fail \
  "$missing_client_native_aar_entry" \
  "release aar native bridge inventory is not exact" \
  --require-built-android

with_android_outputs="$TMP_DIR/with-android-outputs"
make_fixture "$with_android_outputs"
make_android_outputs "$with_android_outputs"
run_expect_pass "$with_android_outputs" --require-built-android

tampered_android_build_environment="$TMP_DIR/tampered-android-build-environment"
cp -R "$with_android_outputs" "$tampered_android_build_environment"
"$TEST_PYTHON_BINARY" -I -S - "$tampered_android_build_environment" <<'PY'
import json
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
manifest = root / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
payload = json.loads(manifest.read_text(encoding="utf-8"))
payload["build_environment"]["cargo_ndk_version"] = "4.1.3"
manifest_bytes = (json.dumps(payload, indent=2) + "\n").encode("utf-8")
manifest.write_bytes(manifest_bytes)
aar = root / "kotlin/client-android/build/outputs/aar/client-android-release.aar"
entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(aar) as source:
    entries = {info.filename: source.read(info) for info in source.infolist()}
entries[entry] = manifest_bytes
with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name, child in entries.items():
        output.writestr(name, child)
PY
run_expect_fail \
  "$tampered_android_build_environment" \
  "native provenance build_environment is not canonical" \
  --android-only --require-built-android

packaged_android_outputs="$TMP_DIR/packaged-android-outputs"
cp -R "$with_android_outputs" "$packaged_android_outputs"
mkdir -p "$packaged_android_outputs/scripts"
cp "$CHECK_SCRIPT" "$packaged_android_outputs/scripts/check_mobile_sdk_artifacts.sh"
cp "$PACKAGE_SCRIPT" "$packaged_android_outputs/scripts/package_mobile_sdk_artifacts.sh"
if MOBILE_SDK_SKIP_BINARY_INSPECTION=1 \
  bash "$packaged_android_outputs/scripts/package_mobile_sdk_artifacts.sh" \
    --root "$packaged_android_outputs" \
    --android \
    --version 1.0.0 >/dev/null 2>&1; then
  fail "Android release packager accepted source-tree build outputs without an external artifact root"
fi
packaged_android_artifacts="$TMP_DIR/packaged-android-artifacts"
packaged_android_gradle_root="$packaged_android_artifacts/gradle-build/iroha_kotlin_sdk"
mkdir -p \
  "$packaged_android_gradle_root/core-jvm" \
  "$packaged_android_gradle_root/client-android"
packaged_android_artifacts="$(cd "$packaged_android_artifacts" && pwd -P)"
packaged_android_gradle_root="$packaged_android_artifacts/gradle-build/iroha_kotlin_sdk"
cp -R \
  "$with_android_outputs/kotlin/core-jvm/build/." \
  "$packaged_android_gradle_root/core-jvm/"
cp -R \
  "$with_android_outputs/kotlin/client-android/build/." \
  "$packaged_android_gradle_root/client-android/"
MOBILE_SDK_SKIP_BINARY_INSPECTION=1 \
  MOBILE_SDK_ANDROID_ARTIFACT_DIR="$packaged_android_artifacts" \
  bash "$packaged_android_outputs/scripts/package_mobile_sdk_artifacts.sh" \
  --root "$packaged_android_outputs" \
  --android \
  --version 1.0.0 >/dev/null
"$TEST_PYTHON_BINARY" -I -S - "$packaged_android_outputs" <<'PY'
import io
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
archive_path = root / "dist/mobile-sdk/iroha-mobile-sdk-android-1.0.0.zip"
prefix = "iroha-mobile-sdk-android-1.0.0/"
with zipfile.ZipFile(archive_path) as archive:
    names = set(archive.namelist())
    expected = {
        prefix + "native/arm64-v8a/libconnect_norito_bridge.so",
        prefix + "native/x86_64/libconnect_norito_bridge.so",
        prefix + "native/native-build-provenance-v1.json",
        prefix + "client-android/client-android-release.aar",
    }
    if not expected.issubset(names):
        raise SystemExit(f"packaged Android native inventory is incomplete: {expected - names}")
    aar_bytes = archive.read(prefix + "client-android/client-android-release.aar")
    with zipfile.ZipFile(io.BytesIO(aar_bytes)) as aar:
        for abi in ("arm64-v8a", "x86_64"):
            if archive.read(prefix + f"native/{abi}/libconnect_norito_bridge.so") != aar.read(
                f"jni/{abi}/libconnect_norito_bridge.so"
            ):
                raise SystemExit(f"packaged {abi} convenience library differs from AAR")
        if archive.read(prefix + "native/native-build-provenance-v1.json") != aar.read(
            "assets/iroha/native-build-provenance-v1.json"
        ):
            raise SystemExit("packaged native provenance differs from AAR")
PY

candidate_marker_android="$TMP_DIR/candidate-marker-android"
cp -R "$with_android_outputs" "$candidate_marker_android"
printf '%s\n' 'KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_DO_NOT_SHIP_V2' \
  >>"$candidate_marker_android/kotlin/client-android/build/generated/jniLibs/default/arm64-v8a/libconnect_norito_bridge.so"
run_expect_fail \
  "$candidate_marker_android" \
  "native bridge byte count differs from provenance" \
  --require-built-android

tampered_android_raw="$TMP_DIR/tampered-android-raw"
cp -R "$with_android_outputs" "$tampered_android_raw"
"$TEST_PYTHON_BINARY" -I -S - "$tampered_android_raw" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1]) / (
    "kotlin/client-android/build/native/cargo-ndk/default/"
    "arm64-v8a/libconnect_norito_bridge.so"
)
payload = bytearray(path.read_bytes())
payload[0] ^= 1
path.write_bytes(payload)
PY
run_expect_fail \
  "$tampered_android_raw" \
  "raw cargo-ndk native bridge differs from provenance" \
  --android-only --require-built-android

extra_android_raw_library="$TMP_DIR/extra-android-raw-library"
cp -R "$with_android_outputs" "$extra_android_raw_library"
printf 'unrelated cdylib\n' > \
  "$extra_android_raw_library/kotlin/client-android/build/native/cargo-ndk/default/arm64-v8a/libivm_artifact_admission.so"
run_expect_fail \
  "$extra_android_raw_library" \
  "raw cargo-ndk native bridge inventory is not exact" \
  --android-only --require-built-android

extra_android_generated_library="$TMP_DIR/extra-android-generated-library"
cp -R "$with_android_outputs" "$extra_android_generated_library"
printf 'unrelated cdylib\n' > \
  "$extra_android_generated_library/kotlin/client-android/build/generated/jniLibs/default/x86_64/libivm_artifact_admission.so"
run_expect_fail \
  "$extra_android_generated_library" \
  "generated native bridge inventory is not exact" \
  --android-only --require-built-android

extra_android_aar_library="$TMP_DIR/extra-android-aar-library"
cp -R "$with_android_outputs" "$extra_android_aar_library"
extra_android_aar_stage="$TMP_DIR/extra-android-aar-stage"
mkdir -p "$extra_android_aar_stage/jni/arm64-v8a"
printf 'unrelated cdylib\n' > \
  "$extra_android_aar_stage/jni/arm64-v8a/libivm_artifact_admission.so"
(
  cd "$extra_android_aar_stage"
  zip -q \
    "$extra_android_aar_library/kotlin/client-android/build/outputs/aar/client-android-release.aar" \
    jni/arm64-v8a/libivm_artifact_admission.so
)
run_expect_fail \
  "$extra_android_aar_library" \
  "release aar native bridge inventory is not exact" \
  --android-only --require-built-android

production_android_outputs="$TMP_DIR/production-android-outputs"
make_fixture "$production_android_outputs"
make_android_outputs "$production_android_outputs" production
run_expect_pass "$production_android_outputs" --android-only --require-built-android

tampered_android_provenance="$TMP_DIR/tampered-android-provenance"
cp -R "$with_android_outputs" "$tampered_android_provenance"
"$TEST_PYTHON_BINARY" -I -S - "$tampered_android_provenance" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1]) / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
path.write_text(path.read_text(encoding="utf-8") + " ", encoding="utf-8")
PY
run_expect_fail \
  "$tampered_android_provenance" \
  "generated native provenance differs from release aar" \
  --android-only --require-built-android

duplicate_android_provenance="$TMP_DIR/duplicate-android-provenance"
cp -R "$with_android_outputs" "$duplicate_android_provenance"
"$TEST_PYTHON_BINARY" -I -S - "$duplicate_android_provenance" <<'PY'
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
manifest = root / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
payload = manifest.read_bytes().replace(
    b"{\n",
    b'{\n  "schema": "duplicate",\n',
    1,
)
manifest.write_bytes(payload)
aar = root / "kotlin/client-android/build/outputs/aar/client-android-release.aar"
entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(aar) as source:
    entries = {info.filename: source.read(info) for info in source.infolist()}
entries[entry] = payload
with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name, child in entries.items():
        output.writestr(name, child)
PY
run_expect_fail \
  "$duplicate_android_provenance" \
  "native provenance is invalid strict JSON: duplicate JSON member: schema" \
  --android-only --require-built-android

missing_android_source_fingerprint="$TMP_DIR/missing-android-source-fingerprint"
cp -R "$with_android_outputs" "$missing_android_source_fingerprint"
"$TEST_PYTHON_BINARY" -I -S - "$missing_android_source_fingerprint" <<'PY'
import json
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
manifest = root / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
payload = json.loads(manifest.read_text(encoding="utf-8"))
payload.pop("source_fingerprint_sha256")
manifest_bytes = (json.dumps(payload, indent=2) + "\n").encode("utf-8")
manifest.write_bytes(manifest_bytes)
aar = root / "kotlin/client-android/build/outputs/aar/client-android-release.aar"
entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(aar) as source:
    entries = {info.filename: source.read(info) for info in source.infolist()}
entries[entry] = manifest_bytes
with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name, child in entries.items():
        output.writestr(name, child)
PY
run_expect_fail \
  "$missing_android_source_fingerprint" \
  "native provenance field inventory is not exact" \
  --android-only --require-built-android

invalid_android_source_fingerprint="$TMP_DIR/invalid-android-source-fingerprint"
cp -R "$with_android_outputs" "$invalid_android_source_fingerprint"
"$TEST_PYTHON_BINARY" -I -S - "$invalid_android_source_fingerprint" <<'PY'
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
manifest = root / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
payload = manifest.read_bytes().replace(
    b'"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"',
    b'"NOT-A-DIGEST"',
)
manifest.write_bytes(payload)
aar = root / "kotlin/client-android/build/outputs/aar/client-android-release.aar"
entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(aar) as source:
    entries = {info.filename: source.read(info) for info in source.infolist()}
entries[entry] = payload
with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name, child in entries.items():
        output.writestr(name, child)
PY
run_expect_fail \
  "$invalid_android_source_fingerprint" \
  "source_fingerprint_sha256 is not canonical SHA-256" \
  --android-only --require-built-android

dirty_android_provenance="$TMP_DIR/dirty-android-provenance"
cp -R "$with_android_outputs" "$dirty_android_provenance"
"$TEST_PYTHON_BINARY" -I -S - "$dirty_android_provenance" <<'PY'
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
manifest = root / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
payload = manifest.read_bytes().replace(
    b'"source_tree_dirty": false',
    b'"source_tree_dirty": true',
    1,
)
manifest.write_bytes(payload)
aar = root / "kotlin/client-android/build/outputs/aar/client-android-release.aar"
entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(aar) as source:
    entries = {info.filename: source.read(info) for info in source.infolist()}
entries[entry] = payload
with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name, child in entries.items():
        output.writestr(name, child)
PY
run_expect_fail \
  "$dirty_android_provenance" \
  "release artifact must be built from a clean source tree" \
  --android-only --require-built-android
MOBILE_SDK_ALLOW_DIRTY_SOURCE=1 run_expect_pass \
  "$dirty_android_provenance" \
  --android-only --require-built-android

symlink_android_native="$TMP_DIR/symlink-android-native"
cp -R "$with_android_outputs" "$symlink_android_native"
rm -f \
  "$symlink_android_native/kotlin/client-android/build/generated/jniLibs/default/arm64-v8a/libconnect_norito_bridge.so"
ln -s \
  "../../x86_64/libconnect_norito_bridge.so" \
  "$symlink_android_native/kotlin/client-android/build/generated/jniLibs/default/arm64-v8a/libconnect_norito_bridge.so"
run_expect_fail \
  "$symlink_android_native" \
  "generated native bridge contains a non-regular entry" \
  --android-only --require-built-android

invalid_android_production_features="$TMP_DIR/invalid-android-production-features"
cp -R "$production_android_outputs" "$invalid_android_production_features"
"$TEST_PYTHON_BINARY" -I -S - "$invalid_android_production_features" <<'PY'
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
aar = root / "kotlin/client-android/build/outputs/aar/client-android-release.aar"
entry = "assets/iroha/native-build-provenance-v1.json"
with zipfile.ZipFile(aar) as source:
    entries = {info.filename: source.read(info) for info in source.infolist()}
entries[entry] = entries[entry].replace(
    b'[\n    "privacy-production-enabled"\n  ]',
    b"[]",
)
with zipfile.ZipFile(aar, "w", compression=zipfile.ZIP_DEFLATED) as output:
    for name, payload in entries.items():
        output.writestr(name, payload)
PY
run_expect_fail \
  "$invalid_android_production_features" \
  "native provenance cargo_features must be exactly" \
  --android-only --require-built-android

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
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "connect_norito_get_chain_discriminant" \
  "arm64-v8a" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "connect_norito_set_chain_discriminant" \
  "x86_64" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "connect_norito_kagemusha_recipient_registration_lineage_verify_v1" \
  "arm64-v8a" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "connect_norito_kagemusha_request_authorization_create_v2" \
  "x86_64" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2" \
  "arm64-v8a" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2" \
  "x86_64" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeSignerContractRevision" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignerContractRevision" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateAppealFinanceCancelAssetLockJson" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateAppealFinanceCancelAssetLockJson" \
  "$android_inspection_tools"
run_expect_android_unstripped_fail "$with_android_outputs" "$android_inspection_tools"
run_expect_android_binary_fail \
  "$with_android_outputs" \
  "exposes retired or unexpected Kagemusha symbols" \
  "$android_inspection_tools"
rm -rf "$with_android_outputs/IrohaSwift" "$with_android_outputs/dist"
run_expect_pass "$with_android_outputs" --android-only --require-built-android

bash "$SCRIPT_DIR/tests/mobile_sdk_python312_contract.sh"

echo "[mobile-sdk-artifacts-test] all checks passed"
