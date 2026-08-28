#!/usr/bin/env bash
set -euo pipefail
umask 077
PATH=/usr/bin:/bin
export PATH

SCRIPT_DIR="$(cd "${BASH_SOURCE[0]%/*}" && pwd -P)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd -P)"
SOURCE_SEAL="$ROOT_DIR/scripts/norito_bridge_source_seal.py"
ABI23_ARTIFACT_CHECKER="$ROOT_DIR/scripts/check_native_sdk_abi23_artifact.py"
HERMETIC_RUNNER="$ROOT_DIR/scripts/run_mobile_hermetic_command.py"
LOCALNET_DEPLOYER="$ROOT_DIR/scripts/deploy_localnet.sh"
PINNED_TOOLCHAIN="1.93.1"
REQUIRED_NATIVE_ASSERTION="A freshly built connect_norito_bridge ABI 23 artifact-streaming library is required"
LOCALNET_TEST_CLASS="org.hyperledger.iroha.sdk.client.ZkAssetShieldLocalnetTest"

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
  "$ABI23_ARTIFACT_CHECKER" \
  "$HERMETIC_RUNNER" \
  "$LOCALNET_DEPLOYER" \
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
  if [[ -n "${NORITO_MOBILE_JAVA_HOME:-}" ]]; then
    JAVA_HOME_DIR="$NORITO_MOBILE_JAVA_HOME"
  else
    JAVA_HOME_DIR="$(
      /usr/bin/env -i HOME="$USER_HOME_DIR" PATH=/usr/bin:/bin TMPDIR=/tmp \
        LANG=C.UTF-8 LC_ALL=C.UTF-8 /usr/libexec/java_home -v 21
    )"
  fi
else
  NM_BINARY="/usr/bin/nm"
  JAVA_HOME_DIR="${NORITO_MOBILE_JAVA_HOME:-}"
  [[ -n "$JAVA_HOME_DIR" ]] \
    || fail "NORITO_MOBILE_JAVA_HOME must pin the setup-java JDK on non-macOS hosts"
fi
[[ "$JAVA_HOME_DIR" == /* && -d "$JAVA_HOME_DIR" ]] \
  || fail "NORITO_MOBILE_JAVA_HOME or the macOS Java locator must provide an absolute existing JDK directory"
JAVA_HOME_DIR="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$JAVA_HOME_DIR")"
[[ -d "$JAVA_HOME_DIR" && ! -L "$JAVA_HOME_DIR" ]] \
  || fail "resolved Java home must be a canonical non-symlink JDK directory"
NM_BINARY="$("$PYTHON_BINARY" -I -S -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$NM_BINARY")"
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
LOCALNET_DIR="$BUILD_SESSION/zk-asset-shield-localnet"
LOCALNET_STOPPED=0
EVIDENCE_DIR="${KAGEMUSHA_JVM_NATIVE_EVIDENCE_DIR:-}"

stop_localnet() {
  local pidfiles=()
  if [[ "$LOCALNET_STOPPED" == "1" || ! -d "$LOCALNET_DIR" ]]; then
    LOCALNET_STOPPED=1
    return 0
  fi
  shopt -s nullglob
  pidfiles=("$LOCALNET_DIR"/peer*.pid)
  shopt -u nullglob
  if (( ${#pidfiles[@]} == 0 )); then
    LOCALNET_STOPPED=1
    return 0
  fi
  if [[ ! -f "$LOCALNET_DIR/stop.sh" || -L "$LOCALNET_DIR/stop.sh" ]]; then
    printf '[kagemusha-jvm-native] ERROR: localnet stop script is unavailable; preserving %s\n' \
      "$BUILD_SESSION" >&2
    return 1
  fi
  if ! (cd "$LOCALNET_DIR" && /bin/bash ./stop.sh); then
    printf '[kagemusha-jvm-native] ERROR: localnet stop script failed; preserving %s\n' \
      "$BUILD_SESSION" >&2
    return 1
  fi
  shopt -s nullglob
  pidfiles=("$LOCALNET_DIR"/peer*.pid)
  shopt -u nullglob
  if (( ${#pidfiles[@]} != 0 )); then
    printf '[kagemusha-jvm-native] ERROR: localnet teardown left owned pidfiles; preserving %s\n' \
      "$BUILD_SESSION" >&2
    printf '  %s\n' "${pidfiles[@]}" >&2
    return 1
  fi
  LOCALNET_STOPPED=1
}

cleanup() {
  local status=$?
  local preserve=0
  trap - EXIT HUP INT TERM
  if ! stop_localnet; then
    status=1
    preserve=1
  fi
  if [[ "$preserve" == "0" ]]; then
    rm -rf -- "$BUILD_SESSION"
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
if [[ -n "$EVIDENCE_DIR" ]]; then
  [[ "$EVIDENCE_DIR" == /* ]] \
    || fail "KAGEMUSHA_JVM_NATIVE_EVIDENCE_DIR must be absolute"
  if [[ -e "$EVIDENCE_DIR" ]]; then
    [[ -d "$EVIDENCE_DIR" && ! -L "$EVIDENCE_DIR" ]] \
      || fail "KAGEMUSHA_JVM_NATIVE_EVIDENCE_DIR must be a non-symbolic directory"
  else
    mkdir -p -- "$EVIDENCE_DIR"
  fi
  EVIDENCE_DIR="$(
    "$PYTHON_BINARY" -I -S -c \
      'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' \
      "$EVIDENCE_DIR"
  )"
  case "$EVIDENCE_DIR/" in
    "$BUILD_SESSION/"*) fail "native evidence directory must be outside the ephemeral build session" ;;
  esac
fi
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
    NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
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

run_exact_gradle_localnet() {
  local working_directory="$1"
  local native_directory="$2"
  shift 2
  (
    cd "$working_directory"
    "$PYTHON_BINARY" -I -S "$HERMETIC_RUNNER" \
      --profile gradle-jvm-localnet \
      --set "ANDROID_HOME=$MOBILE_ANDROID_HOME" \
      --set "ANDROID_SDK_ROOT=$MOBILE_ANDROID_HOME" \
      --set "DYLD_LIBRARY_PATH=$native_directory" \
      --set "GRADLE_USER_HOME=$MOBILE_GRADLE_HOME" \
      --set "HOME=$USER_HOME_DIR" \
      --set "IROHA_LOCALNET_DIR=$LOCALNET_DIR" \
      --set "IROHA_LOCALNET_TEST=1" \
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
printf '[kagemusha-jvm-native] building fresh host ABI-23 bridge for %s\n' "$HOST_TRIPLE" >&2
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
  *-apple-*) NATIVE_LIBRARY_NAME="libconnect_norito_bridge.dylib" ;;
  *-windows-*) NATIVE_LIBRARY_NAME="connect_norito_bridge.dll" ;;
  *) NATIVE_LIBRARY_NAME="libconnect_norito_bridge.so" ;;
esac
CARGO_NATIVE_LIBRARY="$CARGO_TARGET_DIR/$HOST_TRIPLE/debug/$NATIVE_LIBRARY_NAME"
[[ -f "$CARGO_NATIVE_LIBRARY" && ! -L "$CARGO_NATIVE_LIBRARY" ]] \
  || fail "fresh host bridge library is missing: $CARGO_NATIVE_LIBRARY"
NATIVE_LIBRARY_DIR="$BUILD_SESSION/native-runtime"
mkdir -m 0700 -- "$NATIVE_LIBRARY_DIR"
NATIVE_LIBRARY="$NATIVE_LIBRARY_DIR/$NATIVE_LIBRARY_NAME"
NATIVE_EVIDENCE="$BUILD_SESSION/c-jni-native-abi23.json"
"$PYTHON_BINARY" -I -S "$ABI23_ARTIFACT_CHECKER" record \
  --artifact "$CARGO_NATIVE_LIBRARY" \
  --stage-artifact "$NATIVE_LIBRARY" \
  --manifest "$NATIVE_EVIDENCE" \
  --source-root "$ROOT_DIR" \
  --sdk c-jni \
  --target "$HOST_TRIPLE"
"$PYTHON_BINARY" -I -S "$ABI23_ARTIFACT_CHECKER" verify \
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
    for method in (
        "nativeBridgeAbiVersion",
        "nativeEncodeRequestV1",
        "nativeVerifyResponseV1",
    ):
        required.add(
            "Java_org_hyperledger_iroha_"
            f"{namespace}_validationfee_ValidationFeeHijiriQuoteBridge_{method}"
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

printf '[kagemusha-jvm-native] building fresh four-peer localnet tools for %s\n' \
  "$HOST_TRIPLE" >&2
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
    -p iroha_kagami -p irohad -p iroha_cli \
    --bin kagami --bin iroha3d --bin iroha
source_seal verify --root "$ROOT_DIR" --platform android --snapshot "$SOURCE_SNAPSHOT"

LOCALNET_BIN_DIR="$CARGO_TARGET_DIR/$HOST_TRIPLE/debug"
KAGAMI_BINARY="$LOCALNET_BIN_DIR/kagami"
IROHAD_BINARY="$LOCALNET_BIN_DIR/iroha3d"
IROHA_CLI_BINARY="$LOCALNET_BIN_DIR/iroha"
for localnet_binary in "$KAGAMI_BINARY" "$IROHAD_BINARY" "$IROHA_CLI_BINARY"; do
  [[ -f "$localnet_binary" && ! -L "$localnet_binary" && -x "$localnet_binary" ]] \
    || fail "fresh localnet binary is unavailable: $localnet_binary"
done

printf '[kagemusha-jvm-native] deploying fresh four-peer confidential localnet\n' >&2
/usr/bin/env -i \
  CARGO_HOME="$MOBILE_CARGO_HOME" \
  CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
  HOME="$USER_HOME_DIR" \
  IROHA_BIN="$IROHA_CLI_BINARY" \
  IROHA_DIR="$ROOT_DIR" \
  IROHAD_BIN="$IROHAD_BINARY" \
  KAGAMI_BIN="$KAGAMI_BINARY" \
  LANG=C.UTF-8 \
  LC_ALL=C.UTF-8 \
  PATH="${PYTHON_BINARY%/*}:$CARGO_PATH" \
  PYTHON_BIN="$PYTHON_BINARY" \
  RUSTUP_HOME="$MOBILE_RUSTUP_HOME" \
  RUST_LOG=warn \
  SKIP_TOOL_BUILD=true \
  TMPDIR="$MOBILE_TMPDIR" \
  /bin/bash "$LOCALNET_DEPLOYER" \
    --out-dir "$LOCALNET_DIR" \
    --peers 4 \
    --no-build \
    --target-dir "$CARGO_TARGET_DIR" \
    --timeout 120 \
    --logger-level warn \
    --kura-blocks-in-memory 32

verify_four_peer_localnet() {
  local configs=()
  local pidfiles=()
  local command_line config_path healthy peer_index pid torii_address
  shopt -s nullglob
  configs=("$LOCALNET_DIR"/peer*.toml)
  pidfiles=("$LOCALNET_DIR"/peer*.pid)
  shopt -u nullglob
  (( ${#configs[@]} == 4 )) \
    || fail "localnet must expose exactly four peer configs; found ${#configs[@]}"
  (( ${#pidfiles[@]} == 4 )) \
    || fail "localnet must expose exactly four live peer pidfiles; found ${#pidfiles[@]}"
  for peer_index in 0 1 2 3; do
    config_path="$LOCALNET_DIR/peer${peer_index}.toml"
    [[ -f "$config_path" && ! -L "$config_path" ]] \
      || fail "localnet peer config is unavailable: $config_path"
    [[ -f "$LOCALNET_DIR/peer${peer_index}.pid" \
      && ! -L "$LOCALNET_DIR/peer${peer_index}.pid" ]] \
      || fail "localnet peer pidfile is unavailable: peer${peer_index}.pid"
    pid="$(<"$LOCALNET_DIR/peer${peer_index}.pid")"
    [[ "$pid" =~ ^[0-9]+$ ]] \
      || fail "localnet peer${peer_index} pid is noncanonical"
    command_line="$(ps -p "$pid" -o command= 2>/dev/null || true)"
    [[ -n "$command_line" ]] \
      || fail "localnet peer${peer_index} is not running"
    if [[ "$command_line" != *"--config $config_path"* \
      && "$command_line" != *"--config=$config_path"* ]]; then
      fail "localnet peer${peer_index} pid does not belong to its generated config"
    fi
    torii_address="$(
      "$PYTHON_BINARY" -I -S -c \
        'import sys,tomllib; value=tomllib.load(open(sys.argv[1], "rb"))["torii"]["address"]; print(value)' \
        "$config_path"
    )"
    [[ "$torii_address" =~ ^127\.0\.0\.1:[1-9][0-9]{0,4}$ ]] \
      || fail "localnet peer${peer_index} Torii address is not canonical loopback"
    healthy=0
    for _ in {1..30}; do
      if /usr/bin/curl -fsS --connect-timeout 2 --max-time 2 \
          "http://${torii_address}/health" >/dev/null; then
        healthy=1
        break
      fi
      /bin/sleep 1
    done
    [[ "$healthy" == "1" ]] \
      || fail "localnet peer${peer_index} Torii health check failed"
  done
}

verify_four_peer_localnet
LOCALNET_STOPPED=0

run_full_suite() {
  local label="$1"
  local working_directory="$2"
  shift 2
  printf '[kagemusha-jvm-native] running complete %s suite with fresh native bridge\n' "$label" >&2
  run_exact_gradle "$working_directory" "$NATIVE_LIBRARY_DIR" "$@"
}

printf '[kagemusha-jvm-native] running complete Kotlin suite against four-peer localnet\n' >&2
run_exact_gradle_localnet "$ROOT_DIR/kotlin" "$NATIVE_LIBRARY_DIR" \
  "$ROOT_DIR/kotlin/gradlew" --no-daemon --rerun-tasks :core-jvm:test --console=plain
stop_localnet || fail "four-peer localnet teardown failed"

KOTLIN_RESULT_DIR="$ROOT_DIR/kotlin/core-jvm/build/test-results/test"
KOTLIN_LOCALNET_JUNIT="$KOTLIN_RESULT_DIR/TEST-${LOCALNET_TEST_CLASS}.xml"
"$PYTHON_BINARY" -I -S - \
  "$KOTLIN_RESULT_DIR" \
  "$KOTLIN_LOCALNET_JUNIT" \
  "$NATIVE_EVIDENCE" \
  "$EVIDENCE_DIR" \
  "$HOST_TRIPLE" <<'PY'
from pathlib import Path
import hashlib
import json
import os
import stat
import sys
import xml.etree.ElementTree as ET

result_dir = Path(sys.argv[1])
target_report = Path(sys.argv[2])
native_manifest_path = Path(sys.argv[3])
evidence_dir = Path(sys.argv[4]) if sys.argv[4] else None
host_target = sys.argv[5]
expected_class = "org.hyperledger.iroha.sdk.client.ZkAssetShieldLocalnetTest"


def regular_bytes(path: Path, label: str, maximum: int) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise SystemExit(f"{label} is unavailable: {path}: {error}") from error
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_size <= 0
        or metadata.st_size > maximum
    ):
        raise SystemExit(f"{label} must be one bounded non-symbolic regular file")
    payload = path.read_bytes()
    if len(payload) != metadata.st_size:
        raise SystemExit(f"{label} changed while it was read")
    return payload


def suite_counts(root: ET.Element, label: str) -> dict[str, int]:
    try:
        counts = {
            key: int(root.attrib.get(key, "0"))
            for key in ("tests", "skipped", "failures", "errors")
        }
    except ValueError as error:
        raise SystemExit(f"{label} contains non-integer JUnit counters") from error
    if any(value < 0 for value in counts.values()):
        raise SystemExit(f"{label} contains negative JUnit counters")
    return counts


target_bytes = regular_bytes(target_report, "Kotlin localnet JUnit report", 4 * 1024 * 1024)
try:
    target_root = ET.fromstring(target_bytes)
except ET.ParseError as error:
    raise SystemExit(f"Kotlin localnet JUnit report is invalid: {error}") from error
if target_root.tag != "testsuite" or target_root.attrib.get("name") != expected_class:
    raise SystemExit("Kotlin localnet JUnit report names the wrong test suite")
target_counts = suite_counts(target_root, "Kotlin localnet JUnit report")
expected_counts = {"tests": 1, "skipped": 0, "failures": 0, "errors": 0}
if target_counts != expected_counts:
    raise SystemExit(
        "Kotlin localnet JUnit counters are not release-ready "
        f"(expected={expected_counts}, actual={target_counts})"
    )
cases = list(target_root.findall("testcase"))
if len(cases) != 1 or cases[0].attrib.get("classname") != expected_class:
    raise SystemExit("Kotlin localnet JUnit must contain exactly one canonical testcase")
if any(cases[0].find(tag) is not None for tag in ("skipped", "failure", "error")):
    raise SystemExit("Kotlin localnet testcase contains a skipped or failed outcome")

reports = sorted(result_dir.glob("TEST-*.xml"))
if not reports or target_report not in reports:
    raise SystemExit("complete Kotlin JUnit inventory is unavailable")
aggregate = {"tests": 0, "skipped": 0, "failures": 0, "errors": 0}
for report in reports:
    report_bytes = regular_bytes(report, "Kotlin JUnit report", 4 * 1024 * 1024)
    try:
        root = ET.fromstring(report_bytes)
    except ET.ParseError as error:
        raise SystemExit(f"Kotlin JUnit report is invalid: {report}: {error}") from error
    if root.tag != "testsuite":
        raise SystemExit(f"Kotlin JUnit report has an unexpected root: {report}")
    counts = suite_counts(root, f"Kotlin JUnit report {report.name}")
    outcome_nodes = {
        key: sum(1 for _ in root.iter(tag))
        for key, tag in (("skipped", "skipped"), ("failures", "failure"), ("errors", "error"))
    }
    if len(root.findall("testcase")) != counts["tests"] or any(
        outcome_nodes[key] != counts[key] for key in outcome_nodes
    ):
        raise SystemExit(f"Kotlin JUnit counters do not match outcome nodes: {report}")
    for key, value in counts.items():
        aggregate[key] += value
if aggregate["skipped"] != 0:
    raise SystemExit(
        "complete Kotlin release suite may not contain skipped tests; "
        f"found {aggregate['skipped']}"
    )
if aggregate["failures"] != 0 or aggregate["errors"] != 0:
    raise SystemExit(f"complete Kotlin release suite contains failed outcomes: {aggregate}")

native_bytes = regular_bytes(native_manifest_path, "native ABI-23 evidence", 64 * 1024)
try:
    native = json.loads(native_bytes)
except (UnicodeDecodeError, json.JSONDecodeError) as error:
    raise SystemExit(f"native ABI-23 evidence is invalid: {error}") from error
if (
    type(native) is not dict
    or native.get("sdk") != "c-jni"
    or native.get("target") != host_target
    or native.get("bridge_abi_version") != 23
    or native.get("source_tree_clean") is not True
):
    raise SystemExit("native ABI-23 evidence does not match the exercised JNI artifact")

if evidence_dir is not None:
    metadata = evidence_dir.lstat()
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise SystemExit("external evidence destination is not a non-symbolic directory")

    def write_exclusive(name: str, payload: bytes) -> None:
        path = evidence_dir / name
        descriptor = os.open(
            path,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        try:
            with os.fdopen(descriptor, "wb", closefd=False) as handle:
                handle.write(payload)
                handle.flush()
                os.fsync(handle.fileno())
        finally:
            os.close(descriptor)

    summary = {
        "aggregate": aggregate,
        "host_target": host_target,
        "junit_sha256": hashlib.sha256(target_bytes).hexdigest(),
        "native_artifact_sha256": native["artifact_sha256"],
        "native_bridge_abi_version": native["bridge_abi_version"],
        "peer_count": 4,
        "schema": "iroha.kotlin-zk-asset-localnet-evidence.v1",
        "source_commit": native["source_commit"],
        "status": "passed",
        "target_suite": {"name": expected_class, **target_counts},
        "teardown_complete": True,
    }
    summary_bytes = (
        json.dumps(summary, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")
    write_exclusive("zk-asset-shield-localnet.junit.xml", target_bytes)
    write_exclusive("c-jni-native-abi23.json", native_bytes)
    write_exclusive("zk-asset-shield-localnet-summary.json", summary_bytes)
PY

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
