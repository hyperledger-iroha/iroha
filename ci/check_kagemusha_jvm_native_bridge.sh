#!/usr/bin/env bash
set -euo pipefail
umask 077
PATH=/usr/bin:/bin
export PATH

SCRIPT_DIR="$(cd "${BASH_SOURCE[0]%/*}" && pwd -P)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
SOURCE_SEAL="$ROOT_DIR/scripts/norito_bridge_source_seal.py"
ABI21_ARTIFACT_CHECKER="$ROOT_DIR/scripts/check_native_sdk_abi21_artifact.py"
HERMETIC_RUNNER="$ROOT_DIR/scripts/run_mobile_hermetic_command.py"
PINNED_TOOLCHAIN="1.93.1"
REQUIRED_NATIVE_ASSERTION="A freshly built connect_norito_bridge ABI 21 artifact-streaming library is required"

fail() {
  printf '[kagemusha-jvm-native] ERROR: %s\n' "$*" >&2
  exit 1
}

PYTHON_RESOLUTION_ONLY=0
if (( $# > 1 )); then
  fail "unexpected arguments"
elif (( $# == 1 )); then
  [[ "$1" == "--resolve-python312-for-test" ]] || fail "unknown argument: $1"
  PYTHON_RESOLUTION_ONLY=1
fi

for required_file in \
  "$SOURCE_SEAL" \
  "$ABI21_ARTIFACT_CHECKER" \
  "$HERMETIC_RUNNER" \
  "$ROOT_DIR/rust-toolchain.toml" \
  "$ROOT_DIR/kotlin/gradlew" \
  "$ROOT_DIR/java/iroha_android/gradlew"; do
  [[ -f "$required_file" && ! -L "$required_file" ]] \
    || fail "required regular file is unavailable: $required_file"
done

actual_toolchain="$(
  /usr/bin/sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)"[[:space:]]*$/\1/p' \
    "$ROOT_DIR/rust-toolchain.toml"
)"
[[ "$actual_toolchain" == "$PINNED_TOOLCHAIN" ]] \
  || fail "rust-toolchain.toml must pin exact Rust $PINNED_TOOLCHAIN"

resolve_trusted_python312() {
  local candidate canonical
  local override="${MOBILE_SDK_PYTHON_BINARY:-}"
  local candidates=()

  if [[ -n "$override" ]]; then
    if [[ "$override" != /* || ! -f "$override" || -L "$override" || ! -x "$override" ]]; then
      printf '[kagemusha-jvm-native] ERROR: MOBILE_SDK_PYTHON_BINARY must be an absolute canonical regular executable\n' >&2
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
      /usr/bin/env -i \
        HOME=/tmp \
        PATH=/usr/bin:/bin \
        TMPDIR=/tmp \
        LANG=C.UTF-8 \
        LC_ALL=C.UTF-8 \
        "$candidate" -I -S -c '
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
      printf '[kagemusha-jvm-native] ERROR: MOBILE_SDK_PYTHON_BINARY must already name its canonical executable\n' >&2
      return 1
    fi
    printf '%s\n' "$canonical"
    return 0
  done

  if [[ -n "$override" ]]; then
    printf '[kagemusha-jvm-native] ERROR: MOBILE_SDK_PYTHON_BINARY must be an isolated Python 3.12 executable\n' >&2
  else
    printf '[kagemusha-jvm-native] ERROR: a trusted absolute Python 3.12 executable is required\n' >&2
  fi
  return 1
}

PYTHON_BINARY="$(resolve_trusted_python312)" || exit 1
if [[ "$PYTHON_RESOLUTION_ONLY" == "1" ]]; then
  printf '%s\n' "$PYTHON_BINARY"
  exit 0
fi
USER_HOME_DIR="$("$PYTHON_BINARY" -I -S -c 'import os,pwd; print(pwd.getpwuid(os.getuid()).pw_dir)')"
USER_HOME_DIR="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$USER_HOME_DIR")"
GIT_BINARY="/usr/bin/git"
RUSTUP_BINARY="$USER_HOME_DIR/.cargo/bin/rustup"
MOBILE_CARGO_HOME="$USER_HOME_DIR/.cargo"
MOBILE_RUSTUP_HOME="$USER_HOME_DIR/.rustup"
MOBILE_GRADLE_HOME="$USER_HOME_DIR/.gradle"
MOBILE_TMPDIR="/tmp"
for directory in "$USER_HOME_DIR" "$MOBILE_CARGO_HOME" "$MOBILE_RUSTUP_HOME" "$MOBILE_TMPDIR"; do
  [[ "$directory" == /* ]] || fail "mobile build directories must be absolute: $directory"
done
for tool in "$PYTHON_BINARY" "$GIT_BINARY" "$RUSTUP_BINARY"; do
  [[ -f "$tool" && ! -L "$tool" && -x "$tool" ]] \
    || fail "pinned build tool is not a regular executable: $tool"
done

HOST_OS="$(/usr/bin/uname -s)"
if [[ "$HOST_OS" == "Darwin" ]]; then
  XCODE_DEVELOPER_DIR="$(
    /usr/bin/env -i HOME="$USER_HOME_DIR" PATH=/usr/bin:/bin TMPDIR=/tmp \
      LANG=C.UTF-8 LC_ALL=C.UTF-8 /usr/bin/xcode-select -p
  )"
  NM_BINARY="$(
    /usr/bin/env -i HOME="$USER_HOME_DIR" PATH=/usr/bin:/bin TMPDIR=/tmp \
      LANG=C.UTF-8 LC_ALL=C.UTF-8 DEVELOPER_DIR="$XCODE_DEVELOPER_DIR" \
      /usr/bin/xcrun --find nm
  )"
  JAVA_HOME_DIR="$(
    /usr/bin/env -i HOME="$USER_HOME_DIR" PATH=/usr/bin:/bin TMPDIR=/tmp \
      LANG=C.UTF-8 LC_ALL=C.UTF-8 /usr/libexec/java_home -v 21
  )"
else
  NM_BINARY="/usr/bin/nm"
  JAVA_HOME_DIR="${NORITO_MOBILE_JAVA_HOME:-}"
  [[ -n "$JAVA_HOME_DIR" ]] \
    || fail "NORITO_MOBILE_JAVA_HOME must pin the setup-java JDK on non-macOS hosts"
fi
NM_BINARY="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$NM_BINARY")"
JAVA_HOME_DIR="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$JAVA_HOME_DIR")"
JAVA_BINARY="$JAVA_HOME_DIR/bin/java"
for tool in "$NM_BINARY" "$JAVA_BINARY"; do
  [[ -f "$tool" && ! -L "$tool" && -x "$tool" ]] \
    || fail "pinned native/JVM tool is not a regular executable: $tool"
done
if [[ -n "${NORITO_MOBILE_ANDROID_HOME:-}" ]]; then
  MOBILE_ANDROID_HOME="$NORITO_MOBILE_ANDROID_HOME"
elif [[ "$HOST_OS" == "Darwin" ]]; then
  MOBILE_ANDROID_HOME="$USER_HOME_DIR/Library/Android/sdk"
else
  fail "NORITO_MOBILE_ANDROID_HOME must pin the setup-android SDK on non-macOS hosts"
fi
MOBILE_ANDROID_HOME="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$MOBILE_ANDROID_HOME")"
[[ -d "$MOBILE_ANDROID_HOME" && ! -L "$MOBILE_ANDROID_HOME" ]] \
  || fail "pinned Android SDK root is unavailable: $MOBILE_ANDROID_HOME"

RUSTUP_ENV=(
  HOME="$USER_HOME_DIR"
  PATH="${RUSTUP_BINARY%/*}:/usr/bin:/bin"
  RUSTUP_HOME="$MOBILE_RUSTUP_HOME"
  CARGO_HOME="$MOBILE_CARGO_HOME"
  TMPDIR="$MOBILE_TMPDIR"
  LANG=C.UTF-8
  LC_ALL=C.UTF-8
)
CARGO_BINARY="$(
  /usr/bin/env -i "${RUSTUP_ENV[@]}" \
    "$RUSTUP_BINARY" which --toolchain "$PINNED_TOOLCHAIN" cargo
)"
RUSTC_BINARY="$(
  /usr/bin/env -i "${RUSTUP_ENV[@]}" \
    "$RUSTUP_BINARY" which --toolchain "$PINNED_TOOLCHAIN" rustc
)"
CARGO_BINARY="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$CARGO_BINARY")"
RUSTC_BINARY="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$RUSTC_BINARY")"
[[ -x "$CARGO_BINARY" && -x "$RUSTC_BINARY" ]] \
  || fail "pinned Cargo and rustc executables are unavailable"

cargo_identity="$(/usr/bin/env -i "${RUSTUP_ENV[@]}" "$CARGO_BINARY" --version --verbose)"
rustc_identity="$(/usr/bin/env -i "${RUSTUP_ENV[@]}" "$RUSTC_BINARY" --version --verbose)"
cargo_release="$(/usr/bin/sed -n 's/^release: //p' <<<"$cargo_identity")"
rustc_release="$(/usr/bin/sed -n 's/^release: //p' <<<"$rustc_identity")"
cargo_commit="$(/usr/bin/sed -n 's/^commit-hash: //p' <<<"$cargo_identity")"
rustc_commit="$(/usr/bin/sed -n 's/^commit-hash: //p' <<<"$rustc_identity")"
[[ "$cargo_release" == "$PINNED_TOOLCHAIN" && "$rustc_release" == "$PINNED_TOOLCHAIN" ]] \
  || fail "resolved Cargo/rustc do not match exact Rust $PINNED_TOOLCHAIN"
[[ "$cargo_commit" =~ ^[0-9a-f]{40}$ && "$rustc_commit" =~ ^[0-9a-f]{40}$ ]] \
  || fail "resolved Cargo/rustc commits are not canonical"
java_version="$(
  /usr/bin/env -i HOME="$USER_HOME_DIR" PATH="$JAVA_HOME_DIR/bin:/usr/bin:/bin" \
    TMPDIR=/tmp LANG=C.UTF-8 LC_ALL=C.UTF-8 JAVA_HOME="$JAVA_HOME_DIR" \
    "$JAVA_BINARY" -version 2>&1 \
    | /usr/bin/sed -nE '1s/.*version "([0-9]+)([^"]*)".*/\1/p'
)"
[[ "$java_version" == "21" ]] || fail "fresh JNI gate requires exact Java major version 21"

tool_sha256() {
  /usr/bin/env -i HOME="$USER_HOME_DIR" PATH="${PYTHON_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR=/tmp LANG=C.UTF-8 LC_ALL=C.UTF-8 \
    "$PYTHON_BINARY" -I -S - "$1" <<'PY'
from pathlib import Path
import hashlib
import sys

digest = hashlib.sha256()
with Path(sys.argv[1]).open("rb") as handle:
    while chunk := handle.read(1024 * 1024):
        digest.update(chunk)
print(digest.hexdigest())
PY
}

PYTHON_SHA256_START="$(tool_sha256 "$PYTHON_BINARY")"
GIT_SHA256_START="$(tool_sha256 "$GIT_BINARY")"
RUSTUP_SHA256_START="$(tool_sha256 "$RUSTUP_BINARY")"
CARGO_SHA256_START="$(tool_sha256 "$CARGO_BINARY")"
RUSTC_SHA256_START="$(tool_sha256 "$RUSTC_BINARY")"
NM_SHA256_START="$(tool_sha256 "$NM_BINARY")"
JAVA_SHA256_START="$(tool_sha256 "$JAVA_BINARY")"

if [[ "${KAGEMUSHA_JVM_NATIVE_TOOL_RESOLUTION_ONLY:-0}" == "1" ]]; then
  printf '%s\n' \
    "$PYTHON_BINARY" "$GIT_BINARY" "$RUSTUP_BINARY" "$CARGO_BINARY" \
    "$RUSTC_BINARY" "$NM_BINARY" "$JAVA_BINARY"
  exit 0
fi

BUILD_SESSION="$(/usr/bin/mktemp -d "${MOBILE_TMPDIR%/}/kagemusha-jvm-native.XXXXXX")"
cleanup() {
  rm -rf -- "$BUILD_SESSION"
}
trap cleanup EXIT HUP INT TERM
EMPTY_NATIVE_DIR="$BUILD_SESSION/no-native"
CARGO_TARGET_DIR="$BUILD_SESSION/cargo-target"
mkdir -p "$EMPTY_NATIVE_DIR" "$CARGO_TARGET_DIR"

run_adversarial_environment_self_test() {
  local fake_bin="$BUILD_SESSION/hostile-bin"
  local fake_home="$BUILD_SESSION/hostile-home"
  local marker="$BUILD_SESSION/hostile-tool-invoked"
  local probe="$BUILD_SESSION/environment-probe"
  local cargo_probe_output="$BUILD_SESSION/cargo-environment.txt"
  local gradle_probe_output="$BUILD_SESSION/gradle-environment.txt"
  mkdir -p "$fake_bin" "$fake_home"
  for tool_name in python3 git rustup cargo rustc nm java; do
    printf '#!/bin/sh\nprintf "%%s\\n" "%s0" >>"%s"\nexit 97\n' '$' "$marker" \
      >"$fake_bin/$tool_name"
    chmod 0700 "$fake_bin/$tool_name"
  done
  printf '%s\n' \
    "#!$PYTHON_BINARY" \
    'import os' \
    'from pathlib import Path' \
    'import sys' \
    'Path(sys.argv[1]).write_text("".join(f"{key}={value}\n" for key, value in sorted(os.environ.items())), encoding="utf-8")' \
    >"$probe"
  chmod 0700 "$probe"

  /usr/bin/env -i \
    PATH="$fake_bin" \
    HOME="$fake_home" \
    TMPDIR="$fake_home" \
    JAVA_HOME="$fake_home/fake-jdk" \
    GRADLE_USER_HOME="$fake_home/fake-gradle" \
    GRADLE_OPTS="-Duser.home=$fake_home/forged" \
    JAVA_TOOL_OPTIONS="-Djava.library.path=$fake_home/forged" \
    RUSTFLAGS="-C link-arg=forged" \
    CARGO_ENCODED_RUSTFLAGS="forged" \
    RUSTC_WRAPPER="$fake_bin/rustc-wrapper" \
    RUSTC_WORKSPACE_WRAPPER="$fake_bin/workspace-wrapper" \
    NORITO_MOBILE_JAVA_HOME="$JAVA_HOME_DIR" \
    NORITO_MOBILE_ANDROID_HOME="$MOBILE_ANDROID_HOME" \
    MOBILE_SDK_PYTHON_BINARY="$PYTHON_BINARY" \
    KAGEMUSHA_JVM_NATIVE_TOOL_RESOLUTION_ONLY=1 \
    /bin/bash "$ROOT_DIR/ci/check_kagemusha_jvm_native_bridge.sh" \
    >"$BUILD_SESSION/tool-resolution.txt"
  [[ ! -e "$marker" ]] || fail "hostile PATH tool was invoked during pinned tool resolution"

  /usr/bin/env \
    RUSTFLAGS="-C link-arg=forged" \
    CARGO_ENCODED_RUSTFLAGS="forged" \
    RUSTC_WRAPPER="$fake_bin/rustc-wrapper" \
    RUSTC_WORKSPACE_WRAPPER="$fake_bin/workspace-wrapper" \
    CC="$fake_bin/cc" \
    SDKROOT="$fake_home/forged-sdk" \
    "$PYTHON_BINARY" -I -S "$HERMETIC_RUNNER" \
      --profile host-cargo \
      --set "CARGO=$CARGO_BINARY" \
      --set "CARGO_HOME=$MOBILE_CARGO_HOME" \
      --set "CARGO_INCREMENTAL=0" \
      --set "CARGO_NET_OFFLINE=true" \
      --set "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" \
      --set "HOME=$USER_HOME_DIR" \
      --set "LANG=C.UTF-8" \
      --set "LC_ALL=C.UTF-8" \
      --set "NORITO_SKIP_BINDINGS_SYNC=1" \
      --set "PATH=${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:/usr/bin:/bin" \
      --set "RUSTC=$RUSTC_BINARY" \
      --set "RUSTUP_HOME=$MOBILE_RUSTUP_HOME" \
      --set "TMPDIR=$MOBILE_TMPDIR" \
      -- "$probe" "$cargo_probe_output"

  /usr/bin/env \
    JAVA_HOME="$fake_home/fake-jdk" \
    GRADLE_USER_HOME="$fake_home/fake-gradle" \
    GRADLE_OPTS="-Duser.home=$fake_home/forged" \
    JAVA_TOOL_OPTIONS="-Djava.library.path=$fake_home/forged" \
    _JAVA_OPTIONS="-Djava.io.tmpdir=$fake_home/forged" \
    JDK_JAVA_OPTIONS="-Duser.language=forged" \
    "$PYTHON_BINARY" -I -S "$HERMETIC_RUNNER" \
      --profile gradle-jvm \
      --set "ANDROID_HOME=$MOBILE_ANDROID_HOME" \
      --set "ANDROID_SDK_ROOT=$MOBILE_ANDROID_HOME" \
      --set "DYLD_LIBRARY_PATH=$EMPTY_NATIVE_DIR" \
      --set "GRADLE_USER_HOME=$MOBILE_GRADLE_HOME" \
      --set "HOME=$USER_HOME_DIR" \
      --set "IROHA_NATIVE_LIBRARY_PATH=$EMPTY_NATIVE_DIR" \
      --set "IROHA_REQUIRE_KAGEMUSHA_NATIVE=1" \
      --set "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION=1" \
      --set "JAVA_HOME=$JAVA_HOME_DIR" \
      --set "LANG=C.UTF-8" \
      --set "LC_ALL=C.UTF-8" \
      --set "LD_LIBRARY_PATH=$EMPTY_NATIVE_DIR" \
      --set "PATH=$JAVA_HOME_DIR/bin:/usr/bin:/bin" \
      --set "TMPDIR=$MOBILE_TMPDIR" \
      -- "$probe" "$gradle_probe_output"

  "$PYTHON_BINARY" -I -S - "$cargo_probe_output" "$gradle_probe_output" <<'PY'
from pathlib import Path
import sys


def environment(path):
    result = {}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        name, value = line.split("=", 1)
        if name in result:
            raise SystemExit(f"duplicate environment variable from probe: {name}")
        result[name] = value
    return result


cargo = environment(sys.argv[1])
gradle = environment(sys.argv[2])
# macOS injects this process-local CoreFoundation encoding marker after exec;
# it is not inherited from the caller and has no compiler/tool selection role.
cargo.pop("__CF_USER_TEXT_ENCODING", None)
gradle.pop("__CF_USER_TEXT_ENCODING", None)
expected_cargo = {
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
expected_gradle = {
    "ANDROID_HOME",
    "ANDROID_SDK_ROOT",
    "DYLD_LIBRARY_PATH",
    "GRADLE_USER_HOME",
    "HOME",
    "IROHA_NATIVE_LIBRARY_PATH",
    "IROHA_REQUIRE_KAGEMUSHA_NATIVE",
    "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION",
    "JAVA_HOME",
    "LANG",
    "LC_ALL",
    "LD_LIBRARY_PATH",
    "PATH",
    "TMPDIR",
}
if set(cargo) != expected_cargo or set(gradle) != expected_gradle:
    raise SystemExit(
        "hermetic environment probe inventory mismatch "
        f"(cargo={sorted(cargo)}, gradle={sorted(gradle)})"
    )
if (
    cargo["CARGO_NET_OFFLINE"] != "true"
    or gradle["IROHA_REQUIRE_KAGEMUSHA_NATIVE"] != "1"
    or gradle["IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"] != "1"
):
    raise SystemExit("hermetic environment probe lost a mandatory fail-closed value")
PY
  printf '[kagemusha-jvm-native] hostile PATH, Cargo flags, wrappers, and JVM flags were scrubbed\n' >&2
}

run_adversarial_environment_self_test

source_seal() {
  env -i \
    HOME="$USER_HOME_DIR" \
    PATH="${PYTHON_BINARY%/*}:${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:${GIT_BINARY%/*}:/usr/bin:/bin" \
    TMPDIR="$MOBILE_TMPDIR" \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    NORITO_BRIDGE_SEAL_HOME="$USER_HOME_DIR" \
    NORITO_BRIDGE_SEAL_CARGO_HOME="$MOBILE_CARGO_HOME" \
    NORITO_BRIDGE_SEAL_RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
    NORITO_BRIDGE_SEAL_TMPDIR="$MOBILE_TMPDIR" \
    NORITO_BRIDGE_SEAL_CARGO="$CARGO_BINARY" \
    NORITO_BRIDGE_SEAL_RUSTC="$RUSTC_BINARY" \
    "$PYTHON_BINARY" -I -S "$SOURCE_SEAL" "$@"
}

SOURCE_SNAPSHOT="$BUILD_SESSION/source-seal-v1.json"
source_seal snapshot --root "$ROOT_DIR" --platform android >"$SOURCE_SNAPSHOT"

run_exact_gradle() {
  local working_directory="$1"
  local native_directory="$2"
  shift 2
  (
    cd "$working_directory"
    "$PYTHON_BINARY" -I -S "$HERMETIC_RUNNER" \
      --profile gradle-jvm \
      --set "ANDROID_HOME=$MOBILE_ANDROID_HOME" \
      --set "ANDROID_SDK_ROOT=$MOBILE_ANDROID_HOME" \
      --set "DYLD_LIBRARY_PATH=$native_directory" \
      --set "GRADLE_USER_HOME=$MOBILE_GRADLE_HOME" \
      --set "HOME=$USER_HOME_DIR" \
      --set "IROHA_NATIVE_LIBRARY_PATH=$native_directory" \
      --set "IROHA_REQUIRE_KAGEMUSHA_NATIVE=1" \
      --set "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION=1" \
      --set "JAVA_HOME=$JAVA_HOME_DIR" \
      --set "LANG=C.UTF-8" \
      --set "LC_ALL=C.UTF-8" \
      --set "LD_LIBRARY_PATH=$native_directory" \
      --set "PATH=$JAVA_HOME_DIR/bin:/usr/bin:/bin" \
      --set "TMPDIR=$MOBILE_TMPDIR" \
      -- "$@"
  )
}

run_expected_missing_native_failure() {
  local label="$1"
  local working_directory="$2"
  local result_directory="$3"
  shift 3
  local log_file="$BUILD_SESSION/${label}.log"
  rm -rf -- "$result_directory"
  if run_exact_gradle \
      "$working_directory" "$EMPTY_NATIVE_DIR" "$@" >"$log_file" 2>&1; then
    fail "$label passed even though no native bridge was available"
  fi
  if ! grep -R -F -q "$REQUIRED_NATIVE_ASSERTION" "$log_file" "$result_directory" 2>/dev/null; then
    sed -n '1,240p' "$log_file" >&2
    fail "$label did not fail at the mandatory native-bridge assertion"
  fi
  printf '[kagemusha-jvm-native] %s correctly rejected native absence\n' "$label" >&2
}

run_expected_missing_native_failure \
  kotlin-missing-native \
  "$ROOT_DIR/kotlin" \
  "$ROOT_DIR/kotlin/core-jvm/build/test-results/test" \
  "$ROOT_DIR/kotlin/gradlew" --no-daemon --rerun-tasks :core-jvm:test \
    --tests org.hyperledger.iroha.sdk.offline.IrohaPeerTransportV1Test \
    --console=plain
run_expected_missing_native_failure \
  java-missing-native \
  "$ROOT_DIR/java/iroha_android" \
  "$ROOT_DIR/java/iroha_android/core/build/test-results/test" \
  "$ROOT_DIR/java/iroha_android/gradlew" --no-daemon --rerun-tasks :core:test \
    --tests org.hyperledger.iroha.android.offline.IrohaPeerKagemushaAdapterV1Tests \
    --console=plain

HOST_TRIPLE="$("$RUSTC_BINARY" --version --verbose | sed -n 's/^host: //p')"
[[ "$HOST_TRIPLE" =~ ^[A-Za-z0-9_.-]+$ ]] || fail "rustc returned a non-canonical host triple"
CARGO_PATH="${CARGO_BINARY%/*}:${RUSTC_BINARY%/*}:/usr/bin:/bin"
printf '[kagemusha-jvm-native] building fresh host ABI-21 bridge for %s\n' "$HOST_TRIPLE" >&2
"$PYTHON_BINARY" -I -S "$HERMETIC_RUNNER" \
  --profile host-cargo \
  --set "CARGO=$CARGO_BINARY" \
  --set "CARGO_HOME=$MOBILE_CARGO_HOME" \
  --set "CARGO_INCREMENTAL=0" \
  --set "CARGO_NET_OFFLINE=true" \
  --set "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" \
  --set "HOME=$USER_HOME_DIR" \
  --set "LANG=C.UTF-8" \
  --set "LC_ALL=C.UTF-8" \
  --set "NORITO_SKIP_BINDINGS_SYNC=1" \
  --set "PATH=$CARGO_PATH" \
  --set "RUSTC=$RUSTC_BINARY" \
  --set "RUSTUP_HOME=$MOBILE_RUSTUP_HOME" \
  --set "TMPDIR=$MOBILE_TMPDIR" \
  -- "$CARGO_BINARY" build --locked --offline --target "$HOST_TRIPLE" \
    -p connect_norito_bridge --lib
source_seal verify --root "$ROOT_DIR" --platform android --snapshot "$SOURCE_SNAPSHOT"

case "$HOST_TRIPLE" in
  *-apple-*) NATIVE_LIBRARY="$CARGO_TARGET_DIR/$HOST_TRIPLE/debug/libconnect_norito_bridge.dylib" ;;
  *-windows-*) NATIVE_LIBRARY="$CARGO_TARGET_DIR/$HOST_TRIPLE/debug/connect_norito_bridge.dll" ;;
  *) NATIVE_LIBRARY="$CARGO_TARGET_DIR/$HOST_TRIPLE/debug/libconnect_norito_bridge.so" ;;
esac
[[ -f "$NATIVE_LIBRARY" && ! -L "$NATIVE_LIBRARY" ]] \
  || fail "fresh host bridge library is missing: $NATIVE_LIBRARY"
NATIVE_LIBRARY_DIR="${NATIVE_LIBRARY%/*}"
NATIVE_EVIDENCE="$BUILD_SESSION/c-jni-native-abi21.json"
"$PYTHON_BINARY" -I -S "$ABI21_ARTIFACT_CHECKER" record \
  --artifact "$NATIVE_LIBRARY" \
  --manifest "$NATIVE_EVIDENCE" \
  --source-root "$ROOT_DIR" \
  --sdk c-jni \
  --target "$HOST_TRIPLE"
"$PYTHON_BINARY" -I -S "$ABI21_ARTIFACT_CHECKER" verify \
  --artifact "$NATIVE_LIBRARY" \
  --manifest "$NATIVE_EVIDENCE" \
  --source-root "$ROOT_DIR"
SYMBOLS_FILE="$BUILD_SESSION/native-symbols.txt"
"$NM_BINARY" -g "$NATIVE_LIBRARY" >"$SYMBOLS_FILE"
"$PYTHON_BINARY" -I -S - "$SYMBOLS_FILE" <<'PY'
from pathlib import Path
import re
import sys

text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
symbols = {
    match.group(1).removeprefix("_")
    for match in re.finditer(r"(?:^|[ \t])(_?[A-Za-z][A-Za-z0-9_]*)$", text, re.MULTILINE)
}
required = {
    "connect_norito_bridge_abi_version",
}
for namespace in ("sdk", "android"):
    for method in (
        "nativeArtifactBeginV4",
        "nativeArtifactCancelV4",
        "nativeArtifactFinalizeV4",
        "nativeArtifactWriteV4",
        "nativeBridgeAbiVersion",
        "nativeVerifyRecipientReceiveOfferV2",
    ):
        required.add(
            "Java_org_hyperledger_iroha_"
            f"{namespace}_offline_KagemushaRecursiveSpendProver_{method}"
        )
forbidden = {
    "connect_norito_get_chain_discriminant",
    "connect_norito_set_chain_discriminant",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "Java_org_hyperledger_iroha_sdk_offline_"
    "KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2",
    "Java_org_hyperledger_iroha_android_offline_"
    "KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2",
}
missing = sorted(required - symbols)
present_forbidden = sorted(forbidden & symbols)
if missing or present_forbidden:
    raise SystemExit(
        "fresh host bridge symbol contract failed "
        f"(missing={missing}, forbidden={present_forbidden})"
    )
PY

run_full_suite() {
  local label="$1"
  local working_directory="$2"
  shift 2
  printf '[kagemusha-jvm-native] running complete %s suite with fresh native bridge\n' "$label" >&2
  run_exact_gradle "$working_directory" "$NATIVE_LIBRARY_DIR" "$@"
}

run_full_suite kotlin "$ROOT_DIR/kotlin" \
  "$ROOT_DIR/kotlin/gradlew" --no-daemon --rerun-tasks :core-jvm:test --console=plain
run_full_suite java "$ROOT_DIR/java/iroha_android" \
  "$ROOT_DIR/java/iroha_android/gradlew" --no-daemon --rerun-tasks test --console=plain
source_seal verify --root "$ROOT_DIR" --platform android --snapshot "$SOURCE_SNAPSHOT"
for tool_and_hash in \
  "$PYTHON_BINARY:$PYTHON_SHA256_START" \
  "$GIT_BINARY:$GIT_SHA256_START" \
  "$RUSTUP_BINARY:$RUSTUP_SHA256_START" \
  "$CARGO_BINARY:$CARGO_SHA256_START" \
  "$RUSTC_BINARY:$RUSTC_SHA256_START" \
  "$NM_BINARY:$NM_SHA256_START" \
  "$JAVA_BINARY:$JAVA_SHA256_START"; do
  tool_path="${tool_and_hash%:*}"
  expected_hash="${tool_and_hash##*:}"
  [[ "$(tool_sha256 "$tool_path")" == "$expected_hash" ]] \
    || fail "reviewed tool bytes changed while running the fresh JNI gate: $tool_path"
done

printf '[kagemusha-jvm-native] fresh host bridge and both complete JVM suites passed\n' >&2
