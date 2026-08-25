#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_JVM_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
JAVA_HOME_OVERRIDE="${PRIVACY_JVM_SDK_JAVA_HOME:-}"
PYTHON_BIN="${PRIVACY_JVM_SDK_PYTHON_BIN:-python3}"
CARGO_BIN="${PRIVACY_JVM_SDK_CARGO_BIN:-cargo}"
RUSTC_BIN="${PRIVACY_JVM_SDK_RUSTC_BIN:-rustc}"
FROZEN_CARGO_LOCK_SHA256="ccf4acebfe63ad981193b87afd559c195d8a67642d9536b8082f77bbf24a11f0"
TRACKED_ROOT_CARGO_LOCK_SHA256="ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7"
ABI22_CHECKER="${ROOT_DIR}/scripts/check_native_sdk_abi22_artifact.py"
JAVA_OUT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-privacy-java-sdk-test.XXXXXX")"
NATIVE_BUILD_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-privacy-jvm-native.XXXXXX")"

cleanup() {
  local status=$?
  trap - EXIT HUP INT TERM
  rm -rf -- "${JAVA_OUT}" "${NATIVE_BUILD_ROOT}"
  exit "${status}"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

fail() {
  echo "error: $*" >&2
  exit 1
}

sha256_file() {
  "${PYTHON_BIN}" -I -S - "$1" <<'PY'
import hashlib
import pathlib
import sys

digest = hashlib.sha256()
with pathlib.Path(sys.argv[1]).open("rb") as source:
    while chunk := source.read(1024 * 1024):
        digest.update(chunk)
print(digest.hexdigest())
PY
}

SELECTED_CARGO_LOCK="${IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH:-}"
[[ -f "${SELECTED_CARGO_LOCK}" && ! -L "${SELECTED_CARGO_LOCK}" ]] \
  || fail "the authenticated privacy Cargo.lock is unavailable"
[[ "${SELECTED_CARGO_LOCK}" != "${ROOT_DIR}/Cargo.lock" ]] \
  || fail "the privacy release Cargo.lock must remain distinct from the tracked root lock"
[[ "$(sha256_file "${SELECTED_CARGO_LOCK}")" == "${FROZEN_CARGO_LOCK_SHA256}" ]] \
  || fail "the authenticated privacy Cargo.lock does not match the frozen release digest"
[[ -f "${ROOT_DIR}/Cargo.lock" && ! -L "${ROOT_DIR}/Cargo.lock" ]] \
  || fail "the tracked root Cargo.lock is unavailable"
[[ "$(sha256_file "${ROOT_DIR}/Cargo.lock")" == "${TRACKED_ROOT_CARGO_LOCK_SHA256}" ]] \
  || fail "the tracked root Cargo.lock does not match its release authority"

RUSTC_VERSION="$("${RUSTC_BIN}" --version)"
[[ "${RUSTC_VERSION}" == rustc\ 1.93.1\ * ]] \
  || fail "privacy JVM native execution requires exact rustc 1.93.1"
HOST_TRIPLE="$("${RUSTC_BIN}" -vV | sed -n 's/^host: //p')"
[[ "${HOST_TRIPLE}" =~ ^[A-Za-z0-9_.-]+$ ]] \
  || fail "rustc returned a non-canonical host triple"
export NORITO_SKIP_BINDINGS_SYNC=1

if [[ -n "${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR:-}" ]]; then
  BUILD_TARGET_DIR="${IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR}"
  "${CARGO_BIN}" build --locked -p connect_norito_bridge --lib \
    --target-dir "${BUILD_TARGET_DIR}"
  TARGET_ARTIFACT_DIR="${BUILD_TARGET_DIR}/debug"
else
  BUILD_TARGET_DIR="${NATIVE_BUILD_ROOT}/target"
  "${CARGO_BIN}" build --locked -p connect_norito_bridge --lib \
    --target "${HOST_TRIPLE}" \
    --target-dir "${BUILD_TARGET_DIR}"
  TARGET_ARTIFACT_DIR="${BUILD_TARGET_DIR}/${HOST_TRIPLE}/debug"
fi

case "${HOST_TRIPLE}" in
  *-apple-*) NATIVE_LIBRARY="${TARGET_ARTIFACT_DIR}/libconnect_norito_bridge.dylib" ;;
  *-windows-*) NATIVE_LIBRARY="${TARGET_ARTIFACT_DIR}/connect_norito_bridge.dll" ;;
  *) NATIVE_LIBRARY="${TARGET_ARTIFACT_DIR}/libconnect_norito_bridge.so" ;;
esac
[[ -f "${NATIVE_LIBRARY}" && ! -L "${NATIVE_LIBRARY}" ]] \
  || fail "fresh ABI22 privacy JVM bridge is unavailable: ${NATIVE_LIBRARY}"
NATIVE_LIBRARY_DIR="$(cd "$(dirname "${NATIVE_LIBRARY}")" && pwd -P)"
NATIVE_MANIFEST="${NATIVE_BUILD_ROOT}/native-sdk-abi22.json"
CSHARP_NATIVE_MANIFEST="${NATIVE_BUILD_ROOT}/native-sdk-abi22-csharp.json"

# Native evidence binds the clean source tree, including the tracked root lock.
# The distinct frozen privacy-release lock remains selected externally for all
# wrapped Cargo invocations and is authenticated independently above.

"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" record \
  --artifact "${NATIVE_LIBRARY}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --sdk c-jni \
  --target "${HOST_TRIPLE}"
"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
  --artifact "${NATIVE_LIBRARY}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}"
"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" record \
  --artifact "${NATIVE_LIBRARY}" \
  --manifest "${CSHARP_NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --sdk csharp \
  --target "${HOST_TRIPLE}"
"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
  --artifact "${NATIVE_LIBRARY}" \
  --manifest "${CSHARP_NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}"

export IROHA_NATIVE_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}"
export IROHA_REQUIRE_PRIVACY_EXACT12_NATIVE=1
case "${HOST_TRIPLE}" in
  *-apple-*) export DYLD_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}" ;;
  *-windows-*) export PATH="${NATIVE_LIBRARY_DIR}:${PATH}" ;;
  *) export LD_LIBRARY_PATH="${NATIVE_LIBRARY_DIR}" ;;
esac

is_java_21_home() {
  local java_home="$1"
  local version_line
  [[ -x "${java_home}/bin/java" ]] || return 1
  version_line="$("${java_home}/bin/java" -version 2>&1 | head -n 1)"
  [[ "${version_line}" =~ version[[:space:]]+\"21(\.|\") ]]
}

resolve_java_home() {
  if [[ -n "${JAVA_HOME_OVERRIDE}" ]]; then
    if is_java_21_home "${JAVA_HOME_OVERRIDE}"; then
      printf '%s\n' "${JAVA_HOME_OVERRIDE}"
      return 0
    fi
    echo "PRIVACY_JVM_SDK_JAVA_HOME must point to a JDK 21 home." >&2
    return 1
  fi
  if [[ -n "${JAVA_HOME:-}" ]]; then
    if is_java_21_home "${JAVA_HOME}"; then
      printf '%s\n' "${JAVA_HOME}"
      return 0
    fi
    echo "JAVA_HOME must point to a JDK 21 home for privacy JVM SDK tests." >&2
    return 1
  fi
  if command -v /usr/libexec/java_home >/dev/null 2>&1; then
    local macos_java_home
    if macos_java_home="$(/usr/libexec/java_home -v 21 2>/dev/null)" \
      && is_java_21_home "${macos_java_home}"; then
      printf '%s\n' "${macos_java_home}"
      return 0
    fi
  fi
  local candidates=(
    /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
    /usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
    /opt/homebrew/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home
    /usr/local/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home
    /usr/lib/jvm/java-21-openjdk
    /usr/lib/jvm/java-21-openjdk-amd64
  )
  local candidate
  for candidate in "${candidates[@]}"; do
    if is_java_21_home "${candidate}"; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  echo "JDK 21 is required for privacy JVM SDK tests." >&2
  return 1
}

JAVA_HOME="$(resolve_java_home)"
export JAVA_HOME
export PATH="${JAVA_HOME}/bin:${PATH}"
java -version

cd "${ROOT_DIR}/kotlin"
./gradlew --no-daemon -q :core-jvm:jar :core-jvm:test \
  --tests org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest \
  --tests org.hyperledger.iroha.sdk.privacy.PrivacyExact12FixtureCodecV1Test \
  --tests org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTagTest \
  --tests org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescriptionTest \
  --tests org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionBuildersTest \
  --tests org.hyperledger.iroha.sdk.core.model.instructions.ProofAttachmentTest

cd "${ROOT_DIR}/java/iroha_android"
./gradlew --no-daemon -q :core:test \
  --tests org.hyperledger.iroha.android.privacy.PrivacyExact12FixtureCodecV1Tests \
  --tests org.hyperledger.iroha.android.model.instructions.ProofAttachmentModelTests \
  --tests org.hyperledger.iroha.android.norito.ProofAttachmentNoritoTests

cd "${ROOT_DIR}"
PRIVACY_CORE_JVM_VERSION="$(
  awk -F= '$1 == "irohaSdkVersion" { print $2 }' kotlin/gradle.properties
)"
if [[ -z "${PRIVACY_CORE_JVM_VERSION}" ]]; then
  echo "kotlin/gradle.properties does not declare irohaSdkVersion." >&2
  exit 1
fi
PRIVACY_CORE_JVM_JAR="${ROOT_DIR}/kotlin/core-jvm/build/libs/core-jvm-${PRIVACY_CORE_JVM_VERSION}.jar"
if [[ ! -f "${PRIVACY_CORE_JVM_JAR}" ]]; then
  echo "core-jvm dependency was not built at ${PRIVACY_CORE_JVM_JAR}." >&2
  exit 1
fi
javac \
  -cp "${PRIVACY_CORE_JVM_JAR}" \
  -sourcepath "java/iroha_android/src/main/java:java/iroha_android/src/test/java:java/norito_java/src/main/java" \
  -d "${JAVA_OUT}" \
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java \
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java
java -ea -Djava.library.path="${NATIVE_LIBRARY_DIR}" -cp "${JAVA_OUT}:${PRIVACY_CORE_JVM_JAR}" \
  org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest
java -ea -Djava.library.path="${NATIVE_LIBRARY_DIR}" -cp "${JAVA_OUT}:${PRIVACY_CORE_JVM_JAR}" \
  org.hyperledger.iroha.android.model.instructions.VerifyingKeyInstructionUtilsTests

"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
  --artifact "${NATIVE_LIBRARY}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}"
"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
  --artifact "${NATIVE_LIBRARY}" \
  --manifest "${CSHARP_NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}"
[[ "$(sha256_file "${ROOT_DIR}/Cargo.lock")" == "${TRACKED_ROOT_CARGO_LOCK_SHA256}" ]] \
  || fail "tracked root Cargo.lock changed during privacy JVM native execution"

if [[ -n "${PRIVACY_JVM_NATIVE_EXPORT_DIR:-}" ]]; then
  [[ "${PRIVACY_JVM_NATIVE_EXPORT_DIR}" == /* ]] \
    || fail "PRIVACY_JVM_NATIVE_EXPORT_DIR must be absolute"
  [[ ! -e "${PRIVACY_JVM_NATIVE_EXPORT_DIR}" && \
    ! -L "${PRIVACY_JVM_NATIVE_EXPORT_DIR}" ]] \
    || fail "PRIVACY_JVM_NATIVE_EXPORT_DIR must not already exist"
  install -d -m 700 "${PRIVACY_JVM_NATIVE_EXPORT_DIR}"
  EXPORTED_LIBRARY="${PRIVACY_JVM_NATIVE_EXPORT_DIR}/$(basename "${NATIVE_LIBRARY}")"
  install -m 500 "${NATIVE_LIBRARY}" "${EXPORTED_LIBRARY}"
  install -m 400 "${NATIVE_MANIFEST}" \
    "${PRIVACY_JVM_NATIVE_EXPORT_DIR}/native-sdk-abi22-c-jni.json"
  install -m 400 "${CSHARP_NATIVE_MANIFEST}" \
    "${PRIVACY_JVM_NATIVE_EXPORT_DIR}/native-sdk-abi22-csharp.json"
  install -m 400 "${SELECTED_CARGO_LOCK}" \
    "${PRIVACY_JVM_NATIVE_EXPORT_DIR}/Cargo.lock"
  "${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
    --artifact "${EXPORTED_LIBRARY}" \
    --manifest "${PRIVACY_JVM_NATIVE_EXPORT_DIR}/native-sdk-abi22-c-jni.json" \
    --source-root "${ROOT_DIR}"
  "${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
    --artifact "${EXPORTED_LIBRARY}" \
    --manifest "${PRIVACY_JVM_NATIVE_EXPORT_DIR}/native-sdk-abi22-csharp.json" \
    --source-root "${ROOT_DIR}"
fi
