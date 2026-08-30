#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK_SCRIPT="$SCRIPT_DIR/check_mobile_sdk_artifacts.sh"
PACKAGE_SCRIPT="$SCRIPT_DIR/package_mobile_sdk_artifacts.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mobile-sdk-artifacts-test.XXXXXXXX")"
TMP_DIR="$(cd "$TMP_DIR" && pwd -P)"
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
    && [[ "$("$trusted_python" -I -S -B -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null)" == "3.12" ]]; then
    TEST_PYTHON_BINARY="$trusted_python"
    break
  fi
done
[[ -n "$TEST_PYTHON_BINARY" ]] || {
  printf '[mobile-sdk-artifacts-test] ERROR: pinned Python 3.12 is required\n' >&2
  exit 1
}
TEST_PYTHON_BINARY="$("$TEST_PYTHON_BINARY" -I -S -B -c \
  'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
  "$TEST_PYTHON_BINARY")"
export MOBILE_SDK_PYTHON_BINARY="$TEST_PYTHON_BINARY"
export MOBILE_SDK_TEST_PYTHON_BINARY="$TEST_PYTHON_BINARY"

cleanup() {
  printf '[mobile-sdk-artifacts-test] retained fixture root: %s\n' "$TMP_DIR" >&2
}
trap cleanup EXIT

fail() {
  printf '[mobile-sdk-artifacts-test] ERROR: %s\n' "$*" >&2
  exit 1
}

command -v zip >/dev/null 2>&1 || fail "zip command is required"

# shellcheck source=tests/mobile_sdk_build_source_seal_test.sh
source "$SCRIPT_DIR/tests/mobile_sdk_build_source_seal_test.sh"

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
    "$TEST_PYTHON_BINARY" -I -S -B "$SCRIPT_DIR/run_mobile_hermetic_command.py" \
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
  "$TEST_PYTHON_BINARY" -I -S -B - "$output" <<'PY'
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
      "$TEST_PYTHON_BINARY" -I -S -B "$SCRIPT_DIR/run_mobile_hermetic_command.py" \
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
    "$TEST_PYTHON_BINARY" -I -S -B -c \
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

# shellcheck source=tests/mobile_sdk_artifact_fixture_helpers.sh
source "$SCRIPT_DIR/tests/mobile_sdk_artifact_fixture_helpers.sh"
append_candidate_lab_source() {
  local root="$1"
  "$TEST_PYTHON_BINARY" -I -S -B - "$CHECK_SCRIPT" "$root" <<'PY'
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
jni_symbols = shell_array("KAGEMUSHA_CANDIDATE_LAB_JNI_SYMBOLS")
generated_lifecycle = {
    "connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4",
}
declarations = []
for symbol in symbols:
    if symbol in generated_lifecycle:
        continue
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
        "kagemusha_recursive_spend_lifecycle_exports! {",
        "    resolver = require_kagemusha_candidate_evidence_lab_artifact_binding_v4;",
        "    verify_precheck = false;",
        "    init => connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4, \"krv4-lab-init\";",
        "    append => connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4, \"krv4-lab-app\";",
        "    verify => connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4, \"krv4-lab-ver\";",
        "    redeem => connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4, \"krv4-lab-red\";",
        "}",
        "",
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
        "#[allow(clippy::missing_safety_doc, clippy::too_many_arguments)]",
        "mod kagemusha_candidate_lab_jni {",
        "    macro_rules! candidate_lab_jni_export {",
        "        ($name:ident() -> $return_type:ty $body:block) => {",
        "            #[unsafe(no_mangle)]",
        '            pub unsafe extern "system" fn $name() -> $return_type $body',
        "        };",
        "    }",
        "    macro_rules! candidate_lab_jni_forwarders {",
        "        ($($name:ident() -> $return_type:ty => $delegate:path;)*) => {$(",
        "            #[unsafe(no_mangle)]",
        '            pub unsafe extern "system" fn $name() -> $return_type { $delegate() }',
        "        )*};",
        "    }",
    ]
)
for name in jni_symbols[:2]:
    declarations.extend(
        [
            "    candidate_lab_jni_export! {",
            f"        {name}() -> () {{}}",
            "    }",
        ]
    )
declarations.append("    candidate_lab_jni_forwarders! {")
declarations.extend(
    f"        {name}() -> () => candidate_lab_delegate;"
    for name in jni_symbols[2:]
)
declarations.extend(
    [
        "    }",
        "}",
        "#[cfg(all(",
        f'    feature = "{feature}",',
        "    any(",
        '        target_os = "android",',
        '        target_os = "linux",',
        '        target_os = "macos",',
        '        target_os = "windows"',
        "    )",
        "))]",
        "pub use kagemusha_candidate_lab_jni::*;",
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
  "$TEST_PYTHON_BINARY" -I -S -B - "$CHECK_SCRIPT" "$root" <<'PY'
from pathlib import Path
import hashlib
import json
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
header_bytes = header.read_bytes()
xcframework = root / "dist/NoritoBridge.xcframework"
for slice_name in (
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64_x86_64",
):
    (xcframework / slice_name / "Headers/connect_norito_bridge.h").write_bytes(
        header_bytes
    )
manifest_path = xcframework / "NoritoBridge.artifacts.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
manifest["bridge_header_sha256"] = hashlib.sha256(header_bytes).hexdigest()
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
}

run_expect_pass() {
  local root="$1"
  shift
  local output
  if ! output="$(PATH="$INSPECTION_TOOLS:$PATH" \
      MOBILE_SDK_ANDROID_NM="$INSPECTION_TOOLS/llvm-nm" \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected validation to pass for $root"
  fi
}

run_expect_single_apple_nm_projection() {
  local root="$1"
  local count_file="$TMP_DIR/apple-nm-invocations"
  local output
  : >"$count_file"
  if ! output="$(PATH="$INSPECTION_TOOLS:$PATH" \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_NM_COUNT_FILE="$count_file" \
      bash "$CHECK_SCRIPT" "$root" --apple-only 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected single-projection Apple validation to pass for $root"
  fi
  if [[ "$(wc -l <"$count_file" | tr -d '[:space:]')" != "3" \
    || "$(sort -u "$count_file" | wc -l | tr -d '[:space:]')" != "3" ]]; then
    fail "Apple validation must invoke nm exactly once for each of three slices"
  fi
}

run_expect_fail() {
  local root="$1"
  local expected="$2"
  shift 2
  local output
  if output="$(PATH="$INSPECTION_TOOLS:$PATH" \
      MOBILE_SDK_ANDROID_NM="$INSPECTION_TOOLS/llvm-nm" \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      bash "$CHECK_SCRIPT" "$root" "$@" 2>&1)"; then
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
  *ios-arm64_x86_64-simulator*|*macos-arm64_x86_64*) printf 'arm64 x86_64\n' ;;
  *) printf 'arm64\n' ;;
esac
SH
  cat >"$tools/nm" <<'SH'
#!/usr/bin/env bash
case "${1:-}" in
  -gUj|-gj) ;;
  *) exit 97 ;;
esac
nm_mode="$1"
binary="${*: -1}"
if [[ -n "${MOBILE_SDK_TEST_NM_COUNT_FILE:-}" ]]; then
  printf '%s\n' "$binary" >>"$MOBILE_SDK_TEST_NM_COUNT_FILE"
fi
"${MOBILE_SDK_TEST_PYTHON_BINARY:?}" -I -S -B - \
  "${MOBILE_SDK_TEST_CHECK_SCRIPT:?}" "$binary" "$nm_mode" <<'PY'
import os
import re
import sys

text = open(sys.argv[1], "r", encoding="utf-8").read()

def shell_array(name):
    match = re.search(
        rf"^{name}=\(\n(.*?)^\)$", text, re.MULTILINE | re.DOTALL
    )
    if match is None:
        raise SystemExit(f"missing fixture array {name}")
    return [
        line.strip()
        for line in match.group(1).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]

for name in (
    "REQUIRED_BRIDGE_SYMBOLS",
    "OFFLINE_DEVICE_POLICY_C_SYMBOLS",
    "SORAFS_APPEAL_FINANCE_C_SYMBOLS",
    "PRIVACY_ABI_C_SYMBOLS",
    "KAGEMUSHA_C_SYMBOLS",
):
    for symbol in shell_array(name):
        if (
            symbol == os.environ.get("MOBILE_SDK_TEST_REFERENCE_ONLY_SYMBOL")
            and "U" in sys.argv[3]
        ):
            continue
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
"${MOBILE_SDK_TEST_PYTHON_BINARY:?}" -I -S -B - \
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
for symbol in shell_array("OFFLINE_DEVICE_POLICY_C_SYMBOLS"):
    emit(symbol)
for symbol in shell_array("SORAFS_APPEAL_FINANCE_C_SYMBOLS"):
    emit(symbol)
for symbol in shell_array("PRIVACY_ABI_C_SYMBOLS"):
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
for symbol in shell_array("PRIVACY_COMPILED_PROFILE_JNI_SYMBOLS"):
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
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
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

run_expect_reference_only_binary_fail() {
  local root="$1"
  local symbol="$2"
  local tools="$3"
  local output
  if output="$(PATH="$tools:$PATH" \
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_REFERENCE_ONLY_SYMBOL="$symbol" \
      bash "$CHECK_SCRIPT" "$root" --apple-only 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected Apple validation to reject reference-only symbol $symbol"
  fi
  case "$output" in
    *"missing required symbol $symbol"*) ;;
    *)
      printf '%s\n' "$output" >&2
      fail "expected reference-only symbol rejection for $symbol"
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
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
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
      MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
      MOBILE_SDK_TEST_OMIT_ANDROID_SYMBOL="$symbol" \
      bash "$CHECK_SCRIPT" "$root" --android-only --require-built-android 2>&1)"; then
    printf '%s\n' "$output" >&2
    fail "expected Android validation to reject missing symbol $symbol"
  fi
  case "$output" in
    *"bridge is missing ABI-21/V4 proof plus eligibility-envelope symbols through the ABI-22 native bridge:"*"$symbol"*) ;;
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

INSPECTION_TOOLS="$TMP_DIR/mandatory-inspection-tools"
make_apple_inspection_tools "$INSPECTION_TOOLS"
make_android_inspection_tools "$INSPECTION_TOOLS"

fixture="$TMP_DIR/valid"
make_fixture "$fixture"
run_expect_pass "$fixture"
run_expect_single_apple_nm_projection "$fixture"

retired_binary_bypass_output="$(
  MOBILE_SDK_SKIP_BINARY_INSPECTION=1 \
    bash "$CHECK_SCRIPT" "$fixture" --apple-only 2>&1 || true
)"
case "$retired_binary_bypass_output" in
  *"is retired; binary inspection is mandatory"*) ;;
  *) fail "retired binary-inspection bypass was not rejected" ;;
esac

missing_package_resolution="$TMP_DIR/missing-package-resolution"
make_fixture "$missing_package_resolution"
rm "$missing_package_resolution/IrohaSwift/Package.resolved"
run_expect_fail \
  "$missing_package_resolution" \
  "missing Swift package resolution lock" \
  --apple-only

symlinked_package_resolution="$TMP_DIR/symlinked-package-resolution"
make_fixture "$symlinked_package_resolution"
mv "$symlinked_package_resolution/IrohaSwift/Package.resolved" \
  "$symlinked_package_resolution/IrohaSwift/Package.resolved.real"
ln -s Package.resolved.real \
  "$symlinked_package_resolution/IrohaSwift/Package.resolved"
run_expect_fail \
  "$symlinked_package_resolution" \
  "Swift package resolution lock must be a non-symbolic regular file" \
  --apple-only

substituted_abi_alias="$TMP_DIR/substituted-abi-alias"
make_fixture "$substituted_abi_alias"
sed -i.bak 's/= PRIVACY_BRIDGE_ABI_VERSION_V1;/= SUBSTITUTE_ABI_VERSION;/' \
  "$substituted_abi_alias/crates/connect_norito_bridge/src/lib.rs"
rm "$substituted_abi_alias/crates/connect_norito_bridge/src/lib.rs.bak"
run_expect_fail \
  "$substituted_abi_alias" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

fallback_abi_definition="$TMP_DIR/fallback-abi-definition"
make_fixture "$fallback_abi_definition"
printf '%s\n' 'const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 21;' \
  >>"$fallback_abi_definition/crates/connect_norito_bridge/src/lib.rs"
run_expect_fail \
  "$fallback_abi_definition" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

missing_canonical_abi="$TMP_DIR/missing-canonical-abi"
make_fixture "$missing_canonical_abi"
: >"$missing_canonical_abi/crates/iroha_data_model/src/privacy/protocol.rs"
run_expect_fail \
  "$missing_canonical_abi" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

nonnumeric_canonical_abi="$TMP_DIR/nonnumeric-canonical-abi"
make_fixture "$nonnumeric_canonical_abi"
sed -i.bak 's/= 22;/= ABI_TWENTY_TWO;/' \
  "$nonnumeric_canonical_abi/crates/iroha_data_model/src/privacy/protocol.rs"
rm "$nonnumeric_canonical_abi/crates/iroha_data_model/src/privacy/protocol.rs.bak"
run_expect_fail \
  "$nonnumeric_canonical_abi" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

canonical_abi_drift="$TMP_DIR/canonical-abi-drift"
make_fixture "$canonical_abi_drift"
sed -i.bak 's/= 22;/= 21;/' \
  "$canonical_abi_drift/crates/iroha_data_model/src/privacy/protocol.rs"
rm "$canonical_abi_drift/crates/iroha_data_model/src/privacy/protocol.rs.bak"
run_expect_fail \
  "$canonical_abi_drift" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

header_abi_drift="$TMP_DIR/header-abi-drift"
make_fixture "$header_abi_drift"
sed -i.bak 's/ABI_VERSION 22/ABI_VERSION 21/' \
  "$header_abi_drift/crates/connect_norito_bridge/include/connect_norito_bridge.h"
rm "$header_abi_drift/crates/connect_norito_bridge/include/connect_norito_bridge.h.bak"
run_expect_fail \
  "$header_abi_drift" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

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

balanced_lexical_noise="$TMP_DIR/balanced-lexical-noise"
make_fixture "$balanced_lexical_noise"
cat >>"$balanced_lexical_noise/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
const MACRO_DECOY: &str = r###"
kagemusha_recursive_spend_lifecycle_exports! {
    verify => connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4,
}
candidate_lab_jni_export! {
    Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeRogueV4() {}
}
} { /* not Rust structure */
"###;
/* outer { /* nested } */ still comment } */
const CODE_BRACES: (char, char, u8, u8) = ('{', '}', b'{', b'}');
RUST
run_expect_pass "$balanced_lexical_noise"

current_macro_generated_source="$TMP_DIR/current-macro-generated-source"
make_fixture "$current_macro_generated_source"
cp "$SCRIPT_DIR/../crates/connect_norito_bridge/src/lib.rs" \
  "$current_macro_generated_source/crates/connect_norito_bridge/src/lib.rs"
cp "$SCRIPT_DIR/../crates/connect_norito_bridge/Cargo.toml" \
  "$current_macro_generated_source/crates/connect_norito_bridge/Cargo.toml"
cp "$SCRIPT_DIR/../crates/connect_norito_bridge/include/connect_norito_bridge.h" \
  "$current_macro_generated_source/crates/connect_norito_bridge/include/connect_norito_bridge.h"
mkdir -p "$current_macro_generated_source/crates/iroha_data_model/src/privacy"
cp "$SCRIPT_DIR/../crates/iroha_data_model/src/privacy/protocol.rs" \
  "$current_macro_generated_source/crates/iroha_data_model/src/privacy/protocol.rs"
"$TEST_PYTHON_BINARY" -I -S -B - "$current_macro_generated_source" <<'PY'
from pathlib import Path
import hashlib
import json
import sys

root = Path(sys.argv[1])
header = root / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
header_bytes = header.read_bytes()
xcframework = root / "dist/NoritoBridge.xcframework"
for slice_name in (
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64_x86_64",
):
    (xcframework / slice_name / "Headers/connect_norito_bridge.h").write_bytes(
        header_bytes
    )
manifest_path = xcframework / "NoritoBridge.artifacts.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
manifest["bridge_header_sha256"] = hashlib.sha256(header_bytes).hexdigest()
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
run_expect_pass "$current_macro_generated_source" --apple-only

feature_gated_candidate_lab_source="$TMP_DIR/feature-gated-candidate-lab-source"
make_fixture "$feature_gated_candidate_lab_source"
append_candidate_lab_source "$feature_gated_candidate_lab_source"
run_expect_pass "$feature_gated_candidate_lab_source"

missing_generated_production_lifecycle="$TMP_DIR/missing-generated-production-lifecycle"
make_fixture "$missing_generated_production_lifecycle"
"$TEST_PYTHON_BINARY" -I -S -B - "$missing_generated_production_lifecycle" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
line = '    verify => connect_norito_kagemusha_recursive_spend_verify_v4, "krv4-verify";\n'
if text.count(line) != 1:
    raise SystemExit("missing exact production lifecycle fixture")
source.write_text(text.replace(line, "", 1), encoding="utf-8")
PY
run_expect_fail \
  "$missing_generated_production_lifecycle" \
  "Kagemusha lifecycle invocation is not exact"

dual_generated_direct_lifecycle="$TMP_DIR/dual-generated-direct-lifecycle"
make_fixture "$dual_generated_direct_lifecycle"
cat >>"$dual_generated_direct_lifecycle/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_init_v4() {}
RUST
run_expect_fail \
  "$dual_generated_direct_lifecycle" \
  "Kagemusha shipping C export source identities are not single-occurrence"

unguarded_generated_candidate_lifecycle="$TMP_DIR/unguarded-generated-candidate-lifecycle"
cp -R "$feature_gated_candidate_lab_source" "$unguarded_generated_candidate_lifecycle"
"$TEST_PYTHON_BINARY" -I -S -B - "$unguarded_generated_candidate_lifecycle" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
old = (
    '#[cfg(feature = "kagemusha-candidate-evidence-lab")]\n'
    "kagemusha_recursive_spend_lifecycle_exports! {\n"
    "    resolver = require_kagemusha_candidate_evidence_lab_artifact_binding_v4;"
)
replacement = old.split("\n", 1)[1]
if text.count(old) != 1:
    raise SystemExit("missing exact candidate lifecycle guard fixture")
source.write_text(text.replace(old, replacement, 1), encoding="utf-8")
PY
run_expect_fail \
  "$unguarded_generated_candidate_lifecycle" \
  "candidate-lab lifecycle invocation lacks its exact feature guard"

drifted_lifecycle_generator="$TMP_DIR/drifted-lifecycle-generator"
make_fixture "$drifted_lifecycle_generator"
"$TEST_PYTHON_BINARY" -I -S -B - "$drifted_lifecycle_generator" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
old = '''        $(#[$verify_attribute])*
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $verify_name() {}
'''
replacement = '''        $(#[$verify_attribute])*
        pub unsafe extern "C" fn $verify_name() {}
'''
if text.count(old) != 1:
    raise SystemExit("missing exact generated verify export fixture")
source.write_text(text.replace(old, replacement, 1), encoding="utf-8")
PY
run_expect_fail \
  "$drifted_lifecycle_generator" \
  "Kagemusha lifecycle generator has a non-canonical no-mangle verify export"

candidate_lab_source_without_export="$TMP_DIR/candidate-lab-source-without-export"
make_fixture "$candidate_lab_source_without_export"
append_candidate_lab_source "$candidate_lab_source_without_export"
"$TEST_PYTHON_BINARY" -I -S -B - "$candidate_lab_source_without_export" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$unguarded_candidate_lab_source" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$extra_candidate_lab_header" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$commented_candidate_lab_header" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$commented_candidate_marker_header" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$unguarded_candidate_lab_marker" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$unguarded_candidate_lab_jni" <<'PY'
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
#[allow(clippy::missing_safety_doc, clippy::too_many_arguments)]
mod kagemusha_candidate_lab_jni {
'''
if text.count(guard) != 1:
    raise SystemExit("missing exact candidate-lab JNI guard fixture")
replacement = '''#[allow(clippy::missing_safety_doc, clippy::too_many_arguments)]
mod kagemusha_candidate_lab_jni {
'''
source.write_text(text.replace(guard, replacement, 1), encoding="utf-8")
PY
run_expect_fail \
  "$unguarded_candidate_lab_jni" \
  "candidate-lab JNI module lacks its exact conjunctive feature guard"

missing_candidate_lab_jni="$TMP_DIR/missing-candidate-lab-jni"
cp -R "$feature_gated_candidate_lab_source" "$missing_candidate_lab_jni"
"$TEST_PYTHON_BINARY" -I -S -B - "$missing_candidate_lab_jni" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
line = (
    "        Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_"
    "KagemushaCandidateLabNative_nativeRedeemV4() -> () => candidate_lab_delegate;\n"
)
if text.count(line) != 1:
    raise SystemExit("missing exact forwarded JNI fixture")
source.write_text(text.replace(line, "", 1), encoding="utf-8")
PY
run_expect_fail \
  "$missing_candidate_lab_jni" \
  "candidate-lab forwarded JNI invocation inventory is not exact"

rogue_candidate_lab_jni="$TMP_DIR/rogue-candidate-lab-jni"
cp -R "$feature_gated_candidate_lab_source" "$rogue_candidate_lab_jni"
"$TEST_PYTHON_BINARY" -I -S -B - "$rogue_candidate_lab_jni" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
name = (
    "Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_"
    "KagemushaCandidateLabNative_nativeRedeemV4"
)
rogue = name.replace("nativeRedeemV4", "nativeRogueV4")
if text.count(name) != 1:
    raise SystemExit("missing exact JNI identity fixture")
source.write_text(text.replace(name, rogue, 1), encoding="utf-8")
PY
run_expect_fail \
  "$rogue_candidate_lab_jni" \
  "candidate-lab forwarded JNI invocation inventory is not exact"

escaped_candidate_lab_jni="$TMP_DIR/escaped-candidate-lab-jni"
cp -R "$feature_gated_candidate_lab_source" "$escaped_candidate_lab_jni"
cat >>"$escaped_candidate_lab_jni/crates/connect_norito_bridge/src/lib.rs" <<'RUST'
fn escaped_candidate_lab_jni_identity() {
    Java_org_hyperledger_iroha_sdk_kagemusha_candidate_lab_KagemushaCandidateLabNative_nativeBridgeAbiVersion();
}
RUST
run_expect_fail \
  "$escaped_candidate_lab_jni" \
  "candidate-lab JNI identity appears outside its canonical macro invocation"

drifted_candidate_lab_jni_generator="$TMP_DIR/drifted-candidate-lab-jni-generator"
cp -R "$feature_gated_candidate_lab_source" "$drifted_candidate_lab_jni_generator"
"$TEST_PYTHON_BINARY" -I -S -B - "$drifted_candidate_lab_jni_generator" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
old = '''    macro_rules! candidate_lab_jni_forwarders {
        ($($name:ident() -> $return_type:ty => $delegate:path;)*) => {$(
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $name() -> $return_type { $delegate() }
'''
replacement = '''    macro_rules! candidate_lab_jni_forwarders {
        ($($name:ident() -> $return_type:ty => $delegate:path;)*) => {$(
            pub unsafe extern "system" fn $name() -> $return_type { $delegate() }
'''
if text.count(old) != 1:
    raise SystemExit("missing exact candidate JNI generator fixture")
source.write_text(text.replace(old, replacement, 1), encoding="utf-8")
PY
run_expect_fail \
  "$drifted_candidate_lab_jni_generator" \
  "candidate-lab JNI generator candidate_lab_jni_forwarders does not emit one exact no-mangle export"

retired_header_surface="$TMP_DIR/retired-header-surface"
make_fixture "$retired_header_surface"
printf '%s\n' 'int connect_norito_kagemusha_recursive_spend_init_v3(void);' \
  >>"$retired_header_surface/crates/connect_norito_bridge/include/connect_norito_bridge.h"
run_expect_fail "$retired_header_surface" "bridge header exposes retired or unexpected Kagemusha declarations"

retired_swift_binding="$TMP_DIR/retired-swift-binding"
make_fixture "$retired_swift_binding"
printf '%s\n' 'let retired = "connect_norito_kagemusha_recursive_spend_init_v3"' \
  >>"$retired_swift_binding/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
run_expect_fail "$retired_swift_binding" "Swift Kagemusha native symbol inventory is not the exact ABI-21/V4 proof plus eligibility-envelope surface through the ABI-22 native bridge"

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
const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 22;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_init_v3() {}
RUST
run_expect_fail "$retired_bridge_source" "retired or unexpected Kagemusha C exports"

wrong_public_header_abi="$TMP_DIR/wrong-public-header-abi"
make_fixture "$wrong_public_header_abi"
"$TEST_PYTHON_BINARY" -I -S -B - "$wrong_public_header_abi" <<'PY'
from pathlib import Path
import sys

header = Path(sys.argv[1]) / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
text = header.read_text(encoding="utf-8")
header.write_text(
    text.replace("CONNECT_NORITO_BRIDGE_ABI_VERSION 22", "CONNECT_NORITO_BRIDGE_ABI_VERSION 21", 1),
    encoding="utf-8",
)
PY
run_expect_fail \
  "$wrong_public_header_abi" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

wrong_rust_abi_alias="$TMP_DIR/wrong-rust-abi-alias"
make_fixture "$wrong_rust_abi_alias"
"$TEST_PYTHON_BINARY" -I -S -B - "$wrong_rust_abi_alias" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]) / "crates/connect_norito_bridge/src/lib.rs"
text = source.read_text(encoding="utf-8")
source.write_text(
    text.replace("PRIVACY_BRIDGE_ABI_VERSION_V1;", "21;", 1),
    encoding="utf-8",
)
PY
run_expect_fail \
  "$wrong_rust_abi_alias" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

wrong_protocol_abi="$TMP_DIR/wrong-protocol-abi"
make_fixture "$wrong_protocol_abi"
"$TEST_PYTHON_BINARY" -I -S -B - "$wrong_protocol_abi" <<'PY'
from pathlib import Path
import sys

protocol = Path(sys.argv[1]) / "crates/iroha_data_model/src/privacy/protocol.rs"
protocol.write_text(
    "pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 21;\n",
    encoding="utf-8",
)
PY
run_expect_fail \
  "$wrong_protocol_abi" \
  "bridge ABI must be exact public-header 22 with the canonical Rust alias"

retired_kotlin_native="$TMP_DIR/retired-kotlin-native"
make_fixture "$retired_kotlin_native"
printf '%s\n' 'private external fun nativeArtifactBindingV3()' \
  >>"$retired_kotlin_native/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt"
run_expect_fail "$retired_kotlin_native" "native method inventory is not the exact ABI-21/V4 proof plus eligibility-envelope surface through the ABI-22 native bridge" --android-only

retired_java_native="$TMP_DIR/retired-java-native"
make_fixture "$retired_java_native"
printf '%s\n' 'private static native void nativeArtifactBindingV3();' \
  >>"$retired_java_native/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java"
run_expect_fail "$retired_java_native" "native method inventory is not the exact ABI-21/V4 proof plus eligibility-envelope surface through the ABI-22 native bridge" --android-only

retired_rust_jni="$TMP_DIR/retired-rust-jni"
make_fixture "$retired_rust_jni"
printf '%s\n' \
  'fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactBindingV3() {}' \
  >>"$retired_rust_jni/crates/connect_norito_bridge/src/lib.rs"
run_expect_fail "$retired_rust_jni" "Rust bridge exposes retired or unexpected Kagemusha JNI exports" --android-only

wrong_bridge_abi="$TMP_DIR/wrong-bridge-abi"
make_fixture "$wrong_bridge_abi"
sed -i.bak 's/"native_bridge_abi_version": 22/"native_bridge_abi_version": 21/' \
  "$wrong_bridge_abi/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$wrong_bridge_abi/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$wrong_bridge_abi" "exact first-release NoritoBridge ABI 22"

tampered_apple_cargo_lock="$TMP_DIR/tampered-apple-cargo-lock"
make_fixture "$tampered_apple_cargo_lock"
printf '# changed after artifact creation\n' >>"$tampered_apple_cargo_lock/Cargo.lock"
run_expect_fail \
  "$tampered_apple_cargo_lock" \
  "artifact workspace Cargo lock digest does not match checkout"

malformed_apple_cargo_lock_hash="$TMP_DIR/malformed-apple-cargo-lock-hash"
make_fixture "$malformed_apple_cargo_lock_hash"
sed -i.bak \
  's/"build_cargo_lock_sha256": "[0-9a-f]*"/"build_cargo_lock_sha256": "ABC"/' \
  "$malformed_apple_cargo_lock_hash/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f \
  "$malformed_apple_cargo_lock_hash/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail \
  "$malformed_apple_cargo_lock_hash" \
  "NoritoBridge build Cargo lock hash"

symlinked_apple_cargo_lock="$TMP_DIR/symlinked-apple-cargo-lock"
make_fixture "$symlinked_apple_cargo_lock"
mv "$symlinked_apple_cargo_lock/Cargo.lock" \
  "$symlinked_apple_cargo_lock/Cargo.lock.real"
ln -s Cargo.lock.real "$symlinked_apple_cargo_lock/Cargo.lock"
run_expect_fail \
  "$symlinked_apple_cargo_lock" \
  "selected Apple Cargo lock is not an absolute canonical non-symbolic regular file"

tampered_apple_build_environment="$TMP_DIR/tampered-apple-build-environment"
make_fixture "$tampered_apple_build_environment"
"$TEST_PYTHON_BINARY" -I -S -B - "$tampered_apple_build_environment" <<'PY'
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

for python_case in missing null 3.11.9 3.13.0 malformed; do
  invalid_apple_python="$TMP_DIR/invalid-apple-python-${python_case//./-}"
  make_fixture "$invalid_apple_python"
  "$TEST_PYTHON_BINARY" -I -S -B - "$invalid_apple_python" "$python_case" <<'PY'
import json
from pathlib import Path
import sys

manifest = Path(sys.argv[1]) / "dist/NoritoBridge.artifacts.json"
payload = json.loads(manifest.read_text(encoding="utf-8"))
case = sys.argv[2]
if case == "missing":
    del payload["build_environment"]["python_version"]
elif case == "null":
    payload["build_environment"]["python_version"] = None
elif case == "malformed":
    payload["build_environment"]["python_version"] = "3.12.x"
else:
    payload["build_environment"]["python_version"] = case
manifest.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
  run_expect_fail \
    "$invalid_apple_python" \
    "artifact build_environment is missing, malformed, or not hermetic" \
    --apple-only
done

enabled_privacy="$TMP_DIR/enabled-privacy"
make_fixture "$enabled_privacy"
"$TEST_PYTHON_BINARY" -I -S -B - \
  "$enabled_privacy" "$SCRIPT_DIR/validate_norito_bridge_xcframework.py" <<'PY'
import importlib.util
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
validator_path = Path(sys.argv[2])
spec = importlib.util.spec_from_file_location(
    "mobile_sdk_enabled_privacy_validator", validator_path
)
if spec is None or spec.loader is None:
    raise SystemExit("unable to load enabled-privacy fixture validator")
validator = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = validator
spec.loader.exec_module(validator)
manifest_path = root / "dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
manifest["privacy_production_enabled"] = True
manifest["cargo_features"] = ["privacy-production-enabled"]
manifest["kagemusha_mobile_artifact_roles"] = validator.expected_kagemusha_roles(True)
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
PY
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

non_universal_macos="$TMP_DIR/non-universal-macos"
make_fixture "$non_universal_macos"
"$TEST_PYTHON_BINARY" -I -S - \
  "$non_universal_macos/dist/NoritoBridge.xcframework/Info.plist" <<'PY'
from pathlib import Path
import plistlib
import sys


path = Path(sys.argv[1])
with path.open("rb") as handle:
    payload = plistlib.load(handle)
for library in payload["AvailableLibraries"]:
    if library["LibraryIdentifier"] == "macos-arm64_x86_64":
        library["SupportedArchitectures"] = ["arm64"]
with path.open("wb") as handle:
    plistlib.dump(payload, handle)
PY
run_expect_fail \
  "$non_universal_macos" \
  "NoritoBridge Info.plist does not declare the canonical universal Apple slices"

test_only_prebuilt_manifest="$TMP_DIR/test-only-prebuilt-manifest"
make_fixture "$test_only_prebuilt_manifest"
sed -i.bak '/"version": "1.0.0",/a\
  "test_only_prebuilt_slices": false,' \
  "$test_only_prebuilt_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$test_only_prebuilt_manifest/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail \
  "$test_only_prebuilt_manifest" \
  "release artifact manifest must not contain test_only_prebuilt_slices"

test_only_prebuilt_marker="$TMP_DIR/test-only-prebuilt-marker"
make_fixture "$test_only_prebuilt_marker"
touch "$test_only_prebuilt_marker/dist/NoritoBridge.xcframework/.test-only-prebuilt-slices"
run_expect_fail \
  "$test_only_prebuilt_marker" \
  "release artifact must not carry the test-only prebuilt-slices marker"

missing_hash="$TMP_DIR/missing-hash"
make_fixture "$missing_hash"
cat >"$missing_hash/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json" <<'JSON'
{
  "version": "1.0.0",
  "privacy_production_enabled": false,
  "cargo_features": [],
  "hashes": {
    "ios-arm64": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "macos-arm64_x86_64": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
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
"$TEST_PYTHON_BINARY" -I -S -B - "$candidate_marker_apple" <<'PY'
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
run_expect_reference_only_binary_fail \
  "$extra_binary_symbol" \
  "connect_norito_bridge_abi_version" \
  "$inspection_tools"
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
  "macos-arm64_x86_64" \
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
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "iroha_privacy_capabilities_v1" \
  "ios-arm64" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "iroha_privacy_validate_capabilities_v1" \
  "macos-arm64_x86_64" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "iroha_privacy_proof_request_v1" \
  "ios-arm64_x86_64-simulator" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "iroha_privacy_build_proof_v1" \
  "ios-arm64" \
  "$inspection_tools"
run_expect_apple_forbidden_binary_fail \
  "$extra_binary_symbol" \
  "iroha_privacy_verify_proof_v1" \
  "macos-arm64_x86_64" \
  "$inspection_tools"

symbol_inventory_mismatch="$TMP_DIR/symbol-inventory-mismatch"
make_fixture "$symbol_inventory_mismatch"
sed -i.bak \
  's/connect_norito_canonical_json_blake3_v1/unexpected_symbol/' \
  "$symbol_inventory_mismatch/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$symbol_inventory_mismatch/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail "$symbol_inventory_mismatch" "required symbol inventory is missing or non-canonical"

forbidden_symbol_inventory_mismatch="$TMP_DIR/forbidden-symbol-inventory-mismatch"
make_fixture "$forbidden_symbol_inventory_mismatch"
sed -i.bak \
  's/iroha_privacy_verify_proof_v1/unexpected_forbidden_symbol/' \
  "$forbidden_symbol_inventory_mismatch/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json"
rm -f "$forbidden_symbol_inventory_mismatch/dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json.bak"
run_expect_fail \
  "$forbidden_symbol_inventory_mismatch" \
  "forbidden symbol inventory is missing or non-canonical"

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
  "release aar missing ZIP entry jni/x86_64/libconnect_norito_bridge.so" \
  --require-built-android

with_android_outputs="$TMP_DIR/with-android-outputs"
make_fixture "$with_android_outputs"
make_android_outputs "$with_android_outputs"
run_expect_pass "$with_android_outputs" --require-built-android

tampered_android_build_environment="$TMP_DIR/tampered-android-build-environment"
cp -R "$with_android_outputs" "$tampered_android_build_environment"
"$TEST_PYTHON_BINARY" -I -S -B - "$tampered_android_build_environment" <<'PY'
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

for android_environment_case in jobs rustdoc python; do
  invalid_android_environment="$TMP_DIR/invalid-android-environment-$android_environment_case"
  cp -R "$with_android_outputs" "$invalid_android_environment"
  "$TEST_PYTHON_BINARY" -I -S -B - \
    "$invalid_android_environment" "$android_environment_case" <<'PY'
import json
from pathlib import Path
import sys
import zipfile

root = Path(sys.argv[1])
case = sys.argv[2]
manifest = root / (
    "kotlin/client-android/build/generated/nativeProvenance/default/"
    "iroha/native-build-provenance-v1.json"
)
payload = json.loads(manifest.read_text(encoding="utf-8"))
if case == "jobs":
    payload["build_environment"]["cargo_build_jobs"] = 2
elif case == "rustdoc":
    payload["build_environment"]["rustdoc_commit_hash"] = "f" * 40
else:
    payload["build_environment"]["python_version"] = "3.11.9"
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
  if [[ "$android_environment_case" == "python" ]]; then
    expected_android_environment_failure="python_version must be exact Python 3.12"
  else
    expected_android_environment_failure="native provenance build_environment is not canonical"
  fi
  run_expect_fail \
    "$invalid_android_environment" \
    "$expected_android_environment_failure" \
    --android-only --require-built-android
done

packaged_android_outputs="$TMP_DIR/packaged-android-outputs"
cp -R "$with_android_outputs" "$packaged_android_outputs"
mkdir -p "$packaged_android_outputs/scripts"
cp "$CHECK_SCRIPT" \
  "$packaged_android_outputs/scripts/check_mobile_sdk_artifacts.owner.sh"
cat >"$packaged_android_outputs/scripts/check_mobile_sdk_artifacts.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
PATH="${MOBILE_SDK_TEST_INSPECTION_TOOLS:?}:/usr/bin:/bin"
export PATH
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec /bin/bash "$SCRIPT_DIR/check_mobile_sdk_artifacts.owner.sh" "$@"
SH
chmod 0700 "$packaged_android_outputs/scripts/check_mobile_sdk_artifacts.sh"
cp "$PACKAGE_SCRIPT" "$packaged_android_outputs/scripts/package_mobile_sdk_artifacts.sh"
cp "$SCRIPT_DIR/exec_with_file_lock.py" \
  "$packaged_android_outputs/scripts/exec_with_file_lock.py"
rejected_package_parent="$TMP_DIR/rejected-android-package"
mkdir -p "$rejected_package_parent"
if PATH="$INSPECTION_TOOLS:$PATH" \
  MOBILE_SDK_ANDROID_NM="$INSPECTION_TOOLS/llvm-nm" \
  MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
  MOBILE_SDK_TEST_INSPECTION_TOOLS="$INSPECTION_TOOLS" \
  MOBILE_SDK_PACKAGE_OUT_DIR="$rejected_package_parent/mobile-sdk" \
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
packaged_android_package_parent="$TMP_DIR/packaged-android-package"
mkdir -p "$packaged_android_package_parent"
packaged_android_package="$packaged_android_package_parent/mobile-sdk"
PATH="$INSPECTION_TOOLS:$PATH" \
  MOBILE_SDK_ANDROID_NM="$INSPECTION_TOOLS/llvm-nm" \
  MOBILE_SDK_TEST_CHECK_SCRIPT="$CHECK_SCRIPT" \
  MOBILE_SDK_TEST_INSPECTION_TOOLS="$INSPECTION_TOOLS" \
  MOBILE_SDK_ANDROID_ARTIFACT_DIR="$packaged_android_artifacts" \
  MOBILE_SDK_PACKAGE_OUT_DIR="$packaged_android_package" \
  bash "$packaged_android_outputs/scripts/package_mobile_sdk_artifacts.sh" \
  --root "$packaged_android_outputs" \
  --android \
  --version 1.0.0 >/dev/null
"$TEST_PYTHON_BINARY" -I -S -B - "$packaged_android_package" <<'PY'
import io
from pathlib import Path
import sys
import zipfile

package_root = Path(sys.argv[1])
archive_path = package_root / "iroha-mobile-sdk-android-1.0.0.zip"
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
"$TEST_PYTHON_BINARY" -I -S -B - "$tampered_android_raw" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$tampered_android_provenance" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$duplicate_android_provenance" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$missing_android_source_fingerprint" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$invalid_android_source_fingerprint" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$dirty_android_provenance" <<'PY'
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
"$TEST_PYTHON_BINARY" -I -S -B - "$invalid_android_production_features" <<'PY'
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
  "iroha_privacy_capabilities_v1" \
  "arm64-v8a" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "iroha_privacy_validate_capabilities_v1" \
  "x86_64" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "iroha_privacy_proof_request_v1" \
  "arm64-v8a" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "iroha_privacy_build_proof_v1" \
  "x86_64" \
  "$android_inspection_tools"
run_expect_android_forbidden_binary_fail \
  "$with_android_outputs" \
  "iroha_privacy_verify_proof_v1" \
  "arm64-v8a" \
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
  "iroha_privacy_compiled_profile_catalog_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_validate_compiled_profile_catalog_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_exact12_fixture_bundle_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_validate_exact12_fixture_bundle_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_inspect_signed_exact12_action_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_transaction_details_prepare_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_transaction_details_finalize_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_transaction_details_project_result_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_offline_device_registration_result_project_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_action_receipt_prepare_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_action_receipt_finalize_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_action_receipt_project_result_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_state_query_prepare_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_state_query_finalize_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_authenticated_state_query_project_result_v1" \
  "$android_inspection_tools"
run_expect_android_missing_symbol_fail \
  "$with_android_outputs" \
  "iroha_privacy_free_buffer" \
  "$android_inspection_tools"
for policy_or_envelope_symbol in \
  connect_norito_offline_device_policy_proof_request_v1 \
  connect_norito_offline_device_policy_proof_verify_v1 \
  connect_norito_offline_device_eligibility_request_v1 \
  connect_norito_offline_device_eligibility_response_verify_v1 \
  connect_norito_offline_device_attestation_policy_view_verify_v1 \
  connect_norito_offline_device_eligibility_credential_verify_v1 \
  connect_norito_offline_device_eligibility_peer_certificate_verify_v1 \
  connect_norito_kagemusha_eligibility_payment_prepare_v1 \
  connect_norito_kagemusha_eligibility_payment_signing_bytes_v1 \
  connect_norito_kagemusha_eligibility_payment_finalize_v1 \
  connect_norito_kagemusha_eligibility_payment_validate_static_v1 \
  connect_norito_kagemusha_eligibility_payment_validate_first_delivery_v1 \
  connect_norito_kagemusha_eligibility_payment_validate_first_delivery_finalized_v1; do
  run_expect_android_missing_symbol_fail \
    "$with_android_outputs" \
    "$policy_or_envelope_symbol" \
    "$android_inspection_tools"
done
for privacy_jni_symbol in \
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion \
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog \
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog \
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle \
  Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle \
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion \
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog \
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog \
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle \
  Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle \
  Java_org_hyperledger_iroha_sdk_client_AuthenticatedTransactionDetailsNativeBridge_nativeProjectExactOfflineDeviceRegistrationResultV1 \
  Java_org_hyperledger_iroha_android_client_AuthenticatedTransactionDetailsNativeBridge_nativeProjectExactOfflineDeviceRegistrationResultV1; do
  run_expect_android_missing_symbol_fail \
    "$with_android_outputs" \
    "$privacy_jni_symbol" \
    "$android_inspection_tools"
done
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
