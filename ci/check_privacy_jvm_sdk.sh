#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_JVM_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
JAVA_HOME_OVERRIDE="${PRIVACY_JVM_SDK_JAVA_HOME:-}"
PYTHON_BIN="${PRIVACY_JVM_PYTHON_BIN:-python3}"
ABI21_ARTIFACT_CHECKER="${ROOT_DIR}/scripts/check_native_sdk_abi21_artifact.py"
JAVA_OUT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-privacy-java-sdk-test.XXXXXX")"
trap 'rm -rf "${JAVA_OUT}"' EXIT

if [[ -z "${PRIVACY_JVM_NATIVE_ARTIFACT:-}" || \
  -z "${PRIVACY_JVM_NATIVE_MANIFEST:-}" ]]; then
  echo "error: authenticated JVM privacy native artifact and manifest are required" >&2
  exit 1
fi
if ! NATIVE_DIRECTORY="$(
  cd "$(dirname "${PRIVACY_JVM_NATIVE_ARTIFACT}")" && pwd -P
)"; then
  echo "error: JVM privacy native artifact directory is unavailable" >&2
  exit 1
fi
if [[ "${PRIVACY_JVM_NATIVE_ARTIFACT}" != \
    "${NATIVE_DIRECTORY}/libconnect_norito_bridge.so" || \
  "${PRIVACY_JVM_NATIVE_MANIFEST}" != \
    "${NATIVE_DIRECTORY}/native-sdk-abi21.json" ]]; then
  echo "error: JVM privacy native paths are not canonical Linux ABI-21 paths" >&2
  exit 1
fi
if [[ "${IROHA_NATIVE_LIBRARY_PATH:-}" != "${NATIVE_DIRECTORY}" ]]; then
  echo "error: IROHA_NATIVE_LIBRARY_PATH must select only the authenticated JVM privacy bridge" >&2
  exit 1
fi
if [[ "${LD_LIBRARY_PATH:-}" != "${NATIVE_DIRECTORY}" ]]; then
  echo "error: LD_LIBRARY_PATH must select only the authenticated JVM privacy bridge" >&2
  exit 1
fi

"${PYTHON_BIN}" -I -B "${ABI21_ARTIFACT_CHECKER}" verify \
  --artifact "${PRIVACY_JVM_NATIVE_ARTIFACT}" \
  --manifest "${PRIVACY_JVM_NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}"

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
java -ea -Djava.library.path="${NATIVE_DIRECTORY}" -cp "${JAVA_OUT}:${PRIVACY_CORE_JVM_JAR}" \
  org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest
java -ea -Djava.library.path="${NATIVE_DIRECTORY}" -cp "${JAVA_OUT}:${PRIVACY_CORE_JVM_JAR}" \
  org.hyperledger.iroha.android.model.instructions.VerifyingKeyInstructionUtilsTests
