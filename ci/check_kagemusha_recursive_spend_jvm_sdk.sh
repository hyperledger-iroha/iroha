#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_JVM_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
JAVA_HOME_OVERRIDE="${KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME:-}"
JAVA_OUT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-kagemusha-java-sdk-test.XXXXXX")"
trap 'rm -rf "${JAVA_OUT}"' EXIT

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
    echo "KAGEMUSHA_RECURSIVE_SPEND_JVM_JAVA_HOME must point to a JDK 21 home." >&2
    return 1
  fi
  if [[ -n "${JAVA_HOME:-}" ]]; then
    if is_java_21_home "${JAVA_HOME}"; then
      printf '%s\n' "${JAVA_HOME}"
      return 0
    fi
    echo "JAVA_HOME must point to a JDK 21 home for Kagemusha recursive spend JVM SDK tests." >&2
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
  echo "JDK 21 is required for Kagemusha recursive spend JVM SDK tests." >&2
  return 1
}

JAVA_HOME="$(resolve_java_home)"
export JAVA_HOME
export PATH="${JAVA_HOME}/bin:${PATH}"
java -version

cd "${ROOT_DIR}/kotlin"
./gradlew --no-daemon -q :core-jvm:test \
  --tests org.hyperledger.iroha.sdk.address.AccountIdLiteralTest \
  --tests org.hyperledger.iroha.sdk.client.CanonicalRequestSignerTest \
  --tests org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamClientTest \
  --tests org.hyperledger.iroha.sdk.core.model.instructions.ClaimIdentifierWirePayloadEncoderParityTest \
  --tests org.hyperledger.iroha.sdk.core.model.instructions.VerifyingKeyInstructionBuildersTest \
  --tests org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTagTest \
  --tests org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyRecordDescriptionTest \
  --tests org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyStatusTest \
  --tests org.hyperledger.iroha.sdk.crypto.SigningAlgorithmTest \
  --tests org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendRequestCodecsTest \
  --tests org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProverTest \
  --tests org.hyperledger.iroha.sdk.offline.KagemushaInstructionArchivesTest \
  --tests org.hyperledger.iroha.sdk.offline.OfflineCashLifecycleTest \
  --tests org.hyperledger.iroha.sdk.offline.OfflineNoteTest \
  --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test \
  --tests org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridgeTest

cd "${ROOT_DIR}"
javac \
  -sourcepath "java/iroha_android/src/main/java:java/iroha_android/src/test/java:java/norito_java/src/main/java" \
  -d "${JAVA_OUT}" \
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProverTest.java
java -ea -cp "${JAVA_OUT}" \
  org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest

cd "${ROOT_DIR}/java/iroha_android"
ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest,org.hyperledger.iroha.android.offline.OfflineCashLifecycleTest,org.hyperledger.iroha.android.offline.OfflineNoteV2Test,org.hyperledger.iroha.android.offline.OfflineNoteTest,org.hyperledger.iroha.android.privacy.PrivacyNativeBridgeTest,org.hyperledger.iroha.android.tx.TransactionBuilderTests,org.hyperledger.iroha.android.address.AccountIdLiteralTests,org.hyperledger.iroha.android.client.CanonicalRequestSignerTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.model.instructions.ClaimIdentifierWirePayloadEncoderTests,org.hyperledger.iroha.android.client.IdentifierReceiptCanonicalEncoderTests,org.hyperledger.iroha.android.model.instructions.VerifyingKeyInstructionUtilsTests \
  ./gradlew --no-daemon -q :core:test \
  --tests org.hyperledger.iroha.android.GradleHarnessTests \
  --tests org.hyperledger.iroha.android.crypto.SigningAlgorithmTests
