#!/usr/bin/env bash
# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

if [[ "${NORITO_JAVA_SKIP_TESTS:-}" == "1" ]]; then
  echo "[norito-java] Skipping JVM parity tests (NORITO_JAVA_SKIP_TESTS=1)." >&2
  exit 0
fi

ROOT=$(cd "$(dirname "$0")" && pwd)
TARGET_ROOT="$ROOT/../../target/norito_java"
mkdir -p "$TARGET_ROOT"

if [[ "${NORITO_JAVA_KEEP_BUILD:-}" == "1" ]]; then
  KEEP_BUILD=1
else
  KEEP_BUILD=0
fi

TEMP_DIR=$(mktemp -d "$TARGET_ROOT/run.XXXXXX")
CLASSES="$TEMP_DIR/classes"
mkdir -p "$CLASSES"

cleanup() {
  if [[ "$KEEP_BUILD" -ne 1 ]]; then
    rm -rf "$TEMP_DIR"
  else
    echo "[norito-java] Preserving compiled classes under $CLASSES" >&2
  fi
}
trap cleanup EXIT

ensure_java_tools() {
  local tool=$1
  if command -v "$tool" >/dev/null 2>&1; then
    return 0
  fi
  if [[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/$tool" ]]; then
    PATH="$JAVA_HOME/bin:$PATH"
  fi
  if command -v "$tool" >/dev/null 2>&1; then
    return 0
  fi
  for prefix in /opt/homebrew/opt/openjdk@* /usr/local/opt/openjdk@* /Library/Java/JavaVirtualMachines/*/Contents/Home; do
    if [[ -x "$prefix/bin/$tool" ]]; then
      PATH="$prefix/bin:$PATH"
      export JAVA_HOME="${JAVA_HOME:-$prefix}"
      return 0
    fi
  done
  if [[ "$tool" == "java" ]]; then
    local candidate
    candidate=$( /usr/libexec/java_home 2>/dev/null || true )
    if [[ -n "$candidate" && -x "$candidate/bin/java" ]]; then
      PATH="$candidate/bin:$PATH"
      export JAVA_HOME="${JAVA_HOME:-$candidate}"
      return 0
    fi
  fi
  return 1
}

if ! ensure_java_tools javac; then
  echo "javac not found; install JDK 21+ to run tests" >&2
  exit 1
fi

if ! ensure_java_tools java; then
  echo "java runtime not found; install JDK 21+ or set JAVA_HOME" >&2
  exit 1
fi

MAIN_SOURCES=$(find "$ROOT/src/main/java" -name '*.java')
TEST_SOURCES=$(find "$ROOT/src/test/java" -name '*.java')

if javac --release 21 -version >/dev/null 2>&1; then
  JAVAC_FLAGS=("--release" "21" "-d" "$CLASSES")
else
  echo "JDK 21+ is required to compile norito-java." >&2
  exit 1
fi

javac "${JAVAC_FLAGS[@]}" $MAIN_SOURCES $TEST_SOURCES
java -ea -cp "$CLASSES" org.hyperledger.iroha.norito.NoritoTests
