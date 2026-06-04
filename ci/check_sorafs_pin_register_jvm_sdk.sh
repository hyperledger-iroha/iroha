#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SORAFS_PIN_REGISTER_JVM_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
JAVA_HOME_OVERRIDE="${SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME:-}"
JAVA_OUT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sorafs-pin-register-java.XXXXXX")"
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
    echo "SORAFS_PIN_REGISTER_JVM_SDK_JAVA_HOME must point to a JDK 21 home." >&2
    return 1
  fi
  if [[ -n "${JAVA_HOME:-}" ]]; then
    if is_java_21_home "${JAVA_HOME}"; then
      printf '%s\n' "${JAVA_HOME}"
      return 0
    fi
    echo "JAVA_HOME must point to a JDK 21 home for SoraFS pin-register JVM SDK tests." >&2
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
  echo "JDK 21 is required for SoraFS pin-register JVM SDK tests." >&2
  return 1
}

JAVA_HOME="$(resolve_java_home)"
export JAVA_HOME
export PATH="${JAVA_HOME}/bin:${PATH}"
java -version

cd "${ROOT_DIR}/kotlin"
./gradlew --no-daemon -q :core-jvm:test \
  --tests org.hyperledger.iroha.sdk.core.model.instructions.RegisterPinManifestInstructionTest

cd "${ROOT_DIR}"
javac -d "${JAVA_OUT}" \
  java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/InstructionBox.java \
  java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/InstructionKind.java \
  java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/InstructionTemplate.java \
  java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterPinManifestInstruction.java \
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/testing/SimpleJson.java \
  java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsRegisterPinManifestBuilderTests.java

java -ea -cp "${JAVA_OUT}" \
  org.hyperledger.iroha.android.sorafs.SorafsRegisterPinManifestBuilderTests
