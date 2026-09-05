#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check_mobile_sdk_artifacts.sh [--root <repo-root>] [--apple-only|--android-only] [--require-built-android] [--allow-dirty-source]

Validate the sole first-release mobile SDK surface:
  - exact KAGEMUSHA V1 C/header exports;
  - source-complete Swift, Kotlin, and mirrored Java V1 codecs/transports;
  - source-authenticated NoritoBridge XCFramework manifest and slices; and
  - optional built Android jars/AARs with both qualified native ABIs.
USAGE
}

ROOT_ARG=""
CHECK_APPLE=1
CHECK_ANDROID=1
REQUIRE_ANDROID_OUTPUTS="${MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS:-0}"
ALLOW_DIRTY_SOURCE="${MOBILE_SDK_ALLOW_DIRTY_SOURCE:-0}"

if [[ -n "${MOBILE_SDK_SKIP_BINARY_INSPECTION+x}" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_SKIP_BINARY_INSPECTION is retired; binary inspection is mandatory" >&2
  exit 64
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      shift
      [[ $# -gt 0 ]] || { echo "[mobile-sdk-artifacts] --root requires a value" >&2; exit 64; }
      ROOT_ARG="$1"
      ;;
    --root=*) ROOT_ARG="${1#*=}" ;;
    --apple-only) CHECK_APPLE=1; CHECK_ANDROID=0 ;;
    --android-only) CHECK_APPLE=0; CHECK_ANDROID=1 ;;
    --require-built-android) REQUIRE_ANDROID_OUTPUTS=1 ;;
    --allow-dirty-source) ALLOW_DIRTY_SOURCE=1 ;;
    --help|-h) usage; exit 0 ;;
    *)
      if [[ -z "$ROOT_ARG" ]]; then
        ROOT_ARG="$1"
      else
        echo "[mobile-sdk-artifacts] unexpected argument: $1" >&2
        usage >&2
        exit 64
      fi
      ;;
  esac
  shift
done

[[ "$REQUIRE_ANDROID_OUTPUTS" == "0" || "$REQUIRE_ANDROID_OUTPUTS" == "1" ]] || {
  echo "[mobile-sdk-artifacts] MOBILE_SDK_REQUIRE_ANDROID_OUTPUTS must be 0 or 1" >&2
  exit 64
}
[[ "$ALLOW_DIRTY_SOURCE" == "0" || "$ALLOW_DIRTY_SOURCE" == "1" ]] || {
  echo "[mobile-sdk-artifacts] MOBILE_SDK_ALLOW_DIRTY_SOURCE must be 0 or 1" >&2
  exit 64
}

if [[ -z "$ROOT_ARG" ]]; then
  ROOT_ARG="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
[[ -d "$ROOT_ARG" ]] || { echo "[mobile-sdk-artifacts] repository root is missing: $ROOT_ARG" >&2; exit 66; }
ROOT_DIR="$(cd "$ROOT_ARG" && pwd -P)"
if [[ -n "${MOBILE_SDK_APPLE_CARGO_LOCK_PATH+x}" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_APPLE_CARGO_LOCK_PATH is not part of the first-release artifact contract" >&2
  exit 64
fi

resolve_trusted_python312() {
  local candidate canonical
  local override="${MOBILE_SDK_PYTHON_BINARY:-}"
  local candidates=()

  if [[ -n "$override" ]]; then
    if [[ "$override" != /* || ! -f "$override" || -L "$override" || ! -x "$override" ]]; then
      echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_PYTHON_BINARY must be an absolute canonical regular executable" >&2
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
      echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_PYTHON_BINARY must already name its canonical executable" >&2
      return 1
    fi
    printf '%s\n' "$canonical"
    return 0
  done

  if [[ -n "$override" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_PYTHON_BINARY must be an isolated Python 3.12 executable" >&2
  else
    echo "[mobile-sdk-artifacts] ERROR: a trusted absolute Python 3.12 executable is required" >&2
  fi
  return 1
}

CHECK_PYTHON_BINARY="$(resolve_trusted_python312)" || exit 69
CHECK_USER_HOME_DIR="$("$CHECK_PYTHON_BINARY" -I -S -B -c \
  'import os,pathlib,pwd; print(pathlib.Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True))' \
  2>/dev/null || true)"
if [[ -z "$CHECK_USER_HOME_DIR" ]]; then
  CHECK_USER_HOME_DIR="$("$CHECK_PYTHON_BINARY" -I -S -B -c \
    'import pathlib; print(pathlib.Path.home().resolve(strict=True))')"
fi
CHECK_TMPDIR="/tmp"
for trusted_path in "$CHECK_PYTHON_BINARY" "$CHECK_USER_HOME_DIR" "$CHECK_TMPDIR"; do
  if [[ "$trusted_path" != /* ]]; then
    echo "[mobile-sdk-artifacts] ERROR: verifier runtime path is not absolute: $trusted_path" >&2
    exit 69
  fi
done
if [[ ! -f "$CHECK_PYTHON_BINARY" || -L "$CHECK_PYTHON_BINARY" \
  || ! -x "$CHECK_PYTHON_BINARY" || ! -d "$CHECK_USER_HOME_DIR" \
  || ! -d "$CHECK_TMPDIR" ]]; then
  echo "[mobile-sdk-artifacts] ERROR: verifier runtime paths are not canonical regular files/directories" >&2
  exit 69
fi

run_isolated_checker_python() {
  env -i \
    HOME="$CHECK_USER_HOME_DIR" \
    PATH="${CHECK_PYTHON_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$CHECK_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    "$CHECK_PYTHON_BINARY" -I -S -B "$@"
}

SOURCE_SEAL_TOOLS_INITIALIZED=0
SOURCE_SEAL_CARGO_BINARY=""
SOURCE_SEAL_RUSTC_BINARY=""
SOURCE_SEAL_RUSTDOC_BINARY=""
SOURCE_SEAL_RUSTUP_BINARY=""
SOURCE_SEAL_CARGO_HOME="$CHECK_USER_HOME_DIR/.cargo"
SOURCE_SEAL_RUSTUP_HOME="$CHECK_USER_HOME_DIR/.rustup"
SOURCE_SEAL_CARGO_TARGET_DIR=""
SOURCE_SEAL_DEVELOPER_DIR=""
CHECK_RUSTUP_OVERRIDE=""

if [[ -n "${MOBILE_SDK_RUSTUP_BINARY+x}" ]]; then
  CHECK_RUSTUP_OVERRIDE="$MOBILE_SDK_RUSTUP_BINARY"
  if [[ -z "$CHECK_RUSTUP_OVERRIDE" || "$CHECK_RUSTUP_OVERRIDE" != /* ]] \
    || ! canonical_rustup_binary="$(run_isolated_checker_python -c \
      'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
      "$CHECK_RUSTUP_OVERRIDE")" \
    || [[ "$canonical_rustup_binary" != "$CHECK_RUSTUP_OVERRIDE" ]] \
    || [[ ! -f "$CHECK_RUSTUP_OVERRIDE" || -L "$CHECK_RUSTUP_OVERRIDE" \
      || ! -x "$CHECK_RUSTUP_OVERRIDE" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: MOBILE_SDK_RUSTUP_BINARY must be an absolute canonical non-symbolic executable" >&2
    exit 69
  fi
fi

initialize_source_seal_tools() {
  local actual_toolchain developer_dir
  if [[ "$SOURCE_SEAL_TOOLS_INITIALIZED" == "1" ]]; then
    return
  fi
  if [[ -n "${MOBILE_SDK_RUSTUP_BINARY+x}" ]]; then
    SOURCE_SEAL_RUSTUP_BINARY="$CHECK_RUSTUP_OVERRIDE"
  else
    SOURCE_SEAL_RUSTUP_BINARY="$CHECK_USER_HOME_DIR/.cargo/bin/rustup"
  fi
  if [[ ! -f "$SOURCE_SEAL_RUSTUP_BINARY" || -L "$SOURCE_SEAL_RUSTUP_BINARY" \
    || ! -x "$SOURCE_SEAL_RUSTUP_BINARY" || ! -x /usr/bin/git ]]; then
    echo "[mobile-sdk-artifacts] ERROR: pinned rustup and Git are required for source authentication" >&2
    return 1
  fi
  actual_toolchain="$(
    /usr/bin/sed -nE \
      's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' \
      "$ROOT_DIR/rust-toolchain.toml"
  )"
  if [[ "$actual_toolchain" != "1.93.1" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: source authentication requires exact Rust 1.93.1" >&2
    return 1
  fi
  if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: source authentication requires an explicit CARGO_TARGET_DIR" >&2
    return 1
  fi
  if ! SOURCE_SEAL_CARGO_TARGET_DIR="$(run_isolated_checker_python - \
      "$CARGO_TARGET_DIR" "$ROOT_DIR" <<'PY'
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
source_root = Path(sys.argv[2])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit(1)
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(1) from None
if (
    resolved != candidate
    or stat.S_ISLNK(metadata.st_mode)
    or not stat.S_ISDIR(metadata.st_mode)
    or not os.access(candidate, os.R_OK | os.W_OK | os.X_OK)
    or candidate == source_root
    or source_root in candidate.parents
):
    raise SystemExit(1)
print(candidate)
PY
  )"; then
    echo "[mobile-sdk-artifacts] ERROR: CARGO_TARGET_DIR must be a writable, non-symbolic canonical directory outside the Iroha source tree" >&2
    return 1
  fi
  SOURCE_SEAL_CARGO_BINARY="$(
    env -i \
      HOME="$CHECK_USER_HOME_DIR" \
      PATH="${SOURCE_SEAL_RUSTUP_BINARY%/*}:/usr/bin:/bin" \
      CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
      RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
      TMPDIR="$CHECK_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$SOURCE_SEAL_RUSTUP_BINARY" which --toolchain 1.93.1 cargo
  )" || return 1
  SOURCE_SEAL_RUSTC_BINARY="$(
    env -i \
      HOME="$CHECK_USER_HOME_DIR" \
      PATH="${SOURCE_SEAL_RUSTUP_BINARY%/*}:/usr/bin:/bin" \
      CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
      RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
      TMPDIR="$CHECK_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$SOURCE_SEAL_RUSTUP_BINARY" which --toolchain 1.93.1 rustc
  )" || return 1
  SOURCE_SEAL_RUSTDOC_BINARY="$(
    env -i \
      HOME="$CHECK_USER_HOME_DIR" \
      PATH="${SOURCE_SEAL_RUSTUP_BINARY%/*}:/usr/bin:/bin" \
      CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
      RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
      TMPDIR="$CHECK_TMPDIR" \
      LANG=C.UTF-8 \
      LC_ALL=C.UTF-8 \
      "$SOURCE_SEAL_RUSTUP_BINARY" which --toolchain 1.93.1 rustdoc
  )" || return 1
  SOURCE_SEAL_CARGO_BINARY="$(run_isolated_checker_python -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$SOURCE_SEAL_CARGO_BINARY")"
  SOURCE_SEAL_RUSTC_BINARY="$(run_isolated_checker_python -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$SOURCE_SEAL_RUSTC_BINARY")"
  SOURCE_SEAL_RUSTDOC_BINARY="$(run_isolated_checker_python -c \
    'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
    "$SOURCE_SEAL_RUSTDOC_BINARY")"
  if [[ ! -x "$SOURCE_SEAL_CARGO_BINARY" || ! -x "$SOURCE_SEAL_RUSTC_BINARY" \
    || ! -x "$SOURCE_SEAL_RUSTDOC_BINARY" ]]; then
    echo "[mobile-sdk-artifacts] ERROR: exact Rust 1.93.1 Cargo/rustc/rustdoc are unavailable" >&2
    return 1
  fi
  developer_dir="${NORITO_BRIDGE_SEAL_DEVELOPER_DIR:-${NORITO_BRIDGE_DEVELOPER_DIR:-}}"
  if [[ -z "$developer_dir" ]]; then
    if [[ ! -x /usr/bin/xcode-select ]]; then
      echo "[mobile-sdk-artifacts] ERROR: Xcode developer directory is required for Apple source authentication" >&2
      return 1
    fi
    developer_dir="$(/usr/bin/xcode-select -p)" || return 1
  fi
  if ! SOURCE_SEAL_DEVELOPER_DIR="$(run_isolated_checker_python - \
      "$developer_dir" <<'PY'
import os
from pathlib import Path
import stat
import sys

candidate = Path(sys.argv[1])
if not candidate.is_absolute() or candidate != Path(os.path.abspath(candidate)):
    raise SystemExit(1)
try:
    metadata = candidate.lstat()
    resolved = candidate.resolve(strict=True)
except OSError:
    raise SystemExit(1) from None
if resolved != candidate or stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
    raise SystemExit(1)
print(candidate)
PY
  )"; then
    echo "[mobile-sdk-artifacts] ERROR: Xcode developer directory must be canonical and non-symbolic" >&2
    return 1
  fi
  SOURCE_SEAL_TOOLS_INITIALIZED=1
}

run_source_authenticated_python() {
  initialize_source_seal_tools || return 1
  env -i \
    HOME="$CHECK_USER_HOME_DIR" \
    PATH="${CHECK_PYTHON_BINARY%/*}:${SOURCE_SEAL_CARGO_BINARY%/*}:${SOURCE_SEAL_RUSTC_BINARY%/*}:${SOURCE_SEAL_RUSTDOC_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$CHECK_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    NORITO_BRIDGE_SEAL_HOME="$CHECK_USER_HOME_DIR" \
    NORITO_BRIDGE_SEAL_CARGO_HOME="$SOURCE_SEAL_CARGO_HOME" \
    NORITO_BRIDGE_SEAL_RUSTUP_HOME="$SOURCE_SEAL_RUSTUP_HOME" \
    NORITO_BRIDGE_SEAL_TMPDIR="$CHECK_TMPDIR" \
    NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$SOURCE_SEAL_CARGO_TARGET_DIR" \
    NORITO_BRIDGE_SEAL_CARGO="$SOURCE_SEAL_CARGO_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTC="$SOURCE_SEAL_RUSTC_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTDOC="$SOURCE_SEAL_RUSTDOC_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTUP="$SOURCE_SEAL_RUSTUP_BINARY" \
    NORITO_BRIDGE_SEAL_DEVELOPER_DIR="$SOURCE_SEAL_DEVELOPER_DIR" \
    "$CHECK_PYTHON_BINARY" -I -S -B "$@"
}

FAILURES=0
fail() {
  echo "[mobile-sdk-artifacts] ERROR: $*" >&2
  FAILURES=1
}

require_file() {
  local path="$1"
  local label="$2"
  [[ -f "$path" && ! -L "$path" ]] || fail "missing $label: $path"
}

require_literal() {
  local path="$1"
  local literal="$2"
  local label="$3"
  if [[ ! -f "$path" ]] || ! grep -Fq -- "$literal" "$path"; then
    fail "$label is not exact in $path"
  fi
}

KAGEMUSHA_C_SYMBOLS=(
  connect_norito_kagemusha_v1_payment_request_validate
  connect_norito_kagemusha_v1_payment_validate
  connect_norito_kagemusha_v1_acknowledgement_validate
  connect_norito_kagemusha_v1_complete_exchange_validate
  connect_norito_kagemusha_v1_mint_authorization_validate
  connect_norito_kagemusha_v1_mint_credit_validate
  connect_norito_kagemusha_v1_mint_credit_against_authorization_validate
  connect_norito_kagemusha_v1_redemption_voucher_validate
  connect_norito_kagemusha_v1_payment_request_text_validate
  connect_norito_kagemusha_v1_payment_text_validate
  connect_norito_kagemusha_v1_acknowledgement_text_validate
  connect_norito_kagemusha_v1_complete_exchange_text_validate
  connect_norito_kagemusha_v1_mint_authorization_text_validate
  connect_norito_kagemusha_v1_mint_credit_text_validate
  connect_norito_kagemusha_v1_mint_credit_against_authorization_text_validate
  connect_norito_kagemusha_v1_redemption_voucher_text_validate
  connect_norito_kagemusha_device_mint_stage_command_v1_validate
  connect_norito_kagemusha_device_mint_stage_result_v1_validate
  connect_norito_kagemusha_contract_vector_v1
  connect_norito_kagemusha_core_coordinator_contract_v1
  connect_norito_kagemusha_core_coordinator_open_v1
  connect_norito_kagemusha_core_coordinator_invoke_v1
  connect_norito_kagemusha_device_capabilities_v1
  connect_norito_kagemusha_device_execute_v1
  connect_norito_kagemusha_device_response_authenticator_v1_verify
)

REQUIRED_PROTOCOL_C_SYMBOLS=(
  connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1
)
RETIRED_AUDITOR_CAPSULE_VERIFY_PARTS=(
  connect_norito_private_settlement_auditor_capsule_response
  verify
  v1
)
RETIRED_PROTOCOL_C_SYMBOLS=(
  "${RETIRED_AUDITOR_CAPSULE_VERIFY_PARTS[0]}_${RETIRED_AUDITOR_CAPSULE_VERIFY_PARTS[1]}_${RETIRED_AUDITOR_CAPSULE_VERIFY_PARTS[2]}"
)
RETIRED_KAGEMUSHA_PARTS=(cash offline)
RETIRED_KAGEMUSHA_C_PREFIX="connect_norito_${RETIRED_KAGEMUSHA_PARTS[1]}_${RETIRED_KAGEMUSHA_PARTS[0]}_"

check_source_contract() {
  local gate="$ROOT_DIR/ci/check_connect_norito_bridge_header.sh"
  require_file "$gate" "NoritoBridge header parity gate"
  if [[ -f "$gate" ]] && ! bash "$gate"; then
    fail "NoritoBridge C/Rust export parity failed"
  fi

  require_file "$ROOT_DIR/fixtures/offline/kagemusha_v1.json" "shared KAGEMUSHA V1 fixture"
  require_file "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/KagemushaWireV1.swift" "Swift KAGEMUSHA V1 codec"
  require_file "$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/KagemushaDeviceLifecycleBridgeV1.swift" "Swift hardware lifecycle bridge"
  require_file "$ROOT_DIR/kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaWireV1.kt" "Kotlin KAGEMUSHA V1 codec"
  require_file "$ROOT_DIR/kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaDeviceLifecycleBridgeV1.kt" "Kotlin hardware lifecycle bridge"
  require_file "$ROOT_DIR/java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaWireV1.java" "Java KAGEMUSHA V1 codec"
}

check_binary_symbols() {
  local binary="$1"
  local label="$2"
  local nm_mode="$3"
  local symbols
  if ! command -v nm >/dev/null 2>&1; then
    fail "nm is required to inspect $label"
    return
  fi
  if [[ "$nm_mode" == "apple" ]]; then
    symbols="$(nm -gUj "$binary" 2>/dev/null || true)"
  else
    symbols="$(nm -D --defined-only "$binary" 2>/dev/null | awk '{print $NF}' || true)"
  fi
  [[ -n "$symbols" ]] || { fail "$label has no inspectable exported symbols"; return; }
  local symbol
  for symbol in "${KAGEMUSHA_C_SYMBOLS[@]}"; do
    if ! grep -Eq "^_?${symbol}$" <<<"$symbols"; then
      fail "$label is missing $symbol"
    fi
  done
  for symbol in "${REQUIRED_PROTOCOL_C_SYMBOLS[@]}"; do
    if ! grep -Eq "^_?${symbol}$" <<<"$symbols"; then
      fail "$label is missing $symbol"
    fi
  done
  for symbol in "${RETIRED_PROTOCOL_C_SYMBOLS[@]}"; do
    if grep -Eq "^_?${symbol}$" <<<"$symbols"; then
      fail "$label exposes retired protocol symbol $symbol"
    fi
  done
  if grep -Eq "^_?${RETIRED_KAGEMUSHA_C_PREFIX}" <<<"$symbols"; then
    fail "$label exposes a retired KAGEMUSHA C namespace"
  fi
  local observed_kagemusha expected_kagemusha
  observed_kagemusha="$(grep -E '^_?connect_norito_kagemusha_' <<<"$symbols" | sed 's/^_//' | sort -u || true)"
  expected_kagemusha="$(printf '%s\n' "${KAGEMUSHA_C_SYMBOLS[@]}" | sort -u)"
  if [[ "$observed_kagemusha" != "$expected_kagemusha" ]]; then
    fail "$label KAGEMUSHA export inventory is not exact"
  fi
}

check_apple() {
  local artifact_root="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-$ROOT_DIR/dist}"
  [[ "$artifact_root" == /* ]] || artifact_root="$ROOT_DIR/$artifact_root"
  local xcframework="$artifact_root/NoritoBridge.xcframework"
  local manifest="$xcframework/NoritoBridge.artifacts.json"
  local manifest_link="$artifact_root/NoritoBridge.artifacts.json"
  local loader="$ROOT_DIR/IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
  if [[ "${MOBILE_SDK_STAGED_BUILD_VALIDATION:-0}" == "1" ]]; then
    loader="${MOBILE_SDK_PROSPECTIVE_SWIFT_LOADER_PATH:-}"
  fi

  require_file "$ROOT_DIR/IrohaSwift/Package.swift" "Swift package manifest"
  require_literal "$ROOT_DIR/IrohaSwift/Package.swift" 'name: "NoritoBridge"' "Swift binary target"
  require_file "$loader" "Swift native bridge hash owner"
  require_file "$manifest" "embedded NoritoBridge manifest"
  [[ -L "$manifest_link" ]] || fail "public NoritoBridge manifest must be a relative symlink"

  if [[ -f "$manifest" && -f "$loader" ]]; then
    local validation=(
      "$ROOT_DIR/scripts/validate_norito_bridge_xcframework.py"
      --root "$ROOT_DIR"
      --xcframework "$xcframework"
      --manifest "$manifest"
      --manifest-link "$manifest_link"
      --expected-link-target "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
      --swift-loader "$loader"
      --verify-repository-provenance
    )
    if [[ "$ALLOW_DIRTY_SOURCE" == "1" ]]; then
      validation+=(--allow-dirty-source)
    fi
    if ! run_source_authenticated_python "${validation[@]}"; then
      fail "strict NoritoBridge XCFramework validation failed"
    fi
  fi

  if [[ "$(uname -s)" == "Darwin" && -d "$xcframework" ]]; then
    local slice
    for slice in ios-arm64 ios-arm64_x86_64-simulator macos-arm64_x86_64; do
      local binary="$xcframework/$slice/libNoritoBridge.a"
      require_file "$binary" "NoritoBridge $slice library"
      [[ -f "$binary" ]] && check_binary_symbols "$binary" "NoritoBridge $slice" apple
    done
  fi
}

check_android() {
  local settings="$ROOT_DIR/kotlin/settings.gradle.kts"
  require_file "$settings" "Kotlin settings"
  require_literal "$settings" 'include(":core-jvm")' "Kotlin core-jvm module"
  require_literal "$settings" 'include(":client-android")' "Kotlin client-android module"
  require_file "$ROOT_DIR/kotlin/core-jvm/build.gradle.kts" "Kotlin core-jvm build"
  require_file "$ROOT_DIR/kotlin/client-android/build.gradle.kts" "Kotlin client-android build"

  if [[ "$REQUIRE_ANDROID_OUTPUTS" != "1" ]]; then
    return 0
  fi
  local jar
  jar="$(find "$ROOT_DIR/kotlin/core-jvm/build/libs" -maxdepth 1 -type f -name 'core-jvm-*.jar' -print -quit 2>/dev/null || true)"
  [[ -n "$jar" ]] || fail "core-jvm built jar is missing"
  local aar="$ROOT_DIR/kotlin/client-android/build/outputs/aar/client-android-release.aar"
  require_file "$aar" "client-android release AAR"
  [[ -f "$aar" ]] || return
  command -v unzip >/dev/null 2>&1 || { fail "unzip is required for Android artifact validation"; return; }
  local entry
  for entry in \
    AndroidManifest.xml \
    classes.jar \
    assets/iroha/native-build-provenance-v1.json \
    jni/arm64-v8a/libconnect_norito_bridge.so \
    jni/x86_64/libconnect_norito_bridge.so; do
    if ! unzip -Z1 "$aar" | grep -Fxq -- "$entry"; then
      fail "client-android release AAR is missing $entry"
    fi
  done

  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-mobile-sdk.XXXXXX")"
  trap 'rm -rf "$tmp"' RETURN
  local abi
  for abi in arm64-v8a x86_64; do
    local archive_entry="jni/$abi/libconnect_norito_bridge.so"
    if unzip -p "$aar" "$archive_entry" >"$tmp/$abi.so"; then
      check_binary_symbols "$tmp/$abi.so" "client-android $abi bridge" elf
    else
      fail "unable to extract client-android $abi bridge"
    fi
  done
}

check_source_contract
if [[ "$CHECK_APPLE" == "1" ]]; then
  check_apple
fi
if [[ "$CHECK_ANDROID" == "1" ]]; then
  check_android
fi

if [[ "$FAILURES" -ne 0 ]]; then
  echo "[mobile-sdk-artifacts] validation failed for $ROOT_DIR" >&2
  exit 1
fi
echo "[mobile-sdk-artifacts] validation passed for $ROOT_DIR"
