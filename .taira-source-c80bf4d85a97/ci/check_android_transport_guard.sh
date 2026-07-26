#!/usr/bin/env bash
# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$ROOT/java/iroha_android/android"
ALLOW_JAVA_HTTP="${ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP:-0}"
AAR_PATH="${ANDROID_TRANSPORT_GUARD_AAR:-}"
CLASSES_DIR="${ANDROID_TRANSPORT_GUARD_CLASSES_DIR:-}"
DEFAULT_CLASSES_JAR="$ANDROID_DIR/build/intermediates/aar_main_jar/release/classes.jar"
DEFAULT_AAR="$ANDROID_DIR/build/outputs/aar/android-release.aar"
CLASSES_JAR="${1:-$DEFAULT_CLASSES_JAR}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/android-transport-guard.XXXXXX")"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

if [[ ! -d "$ANDROID_DIR" ]]; then
  echo "Android module not found at $ANDROID_DIR" >&2
  exit 1
fi

if command -v rg >/dev/null 2>&1; then
  scan_cmd=(rg -n 'java\.net\.http')
else
  scan_cmd=(grep -R -n 'java\.net\.http')
fi

echo "Checking Android main sources for java.net.http usage..."
if "${scan_cmd[@]}" "$ANDROID_DIR/src/main/java"; then
  echo "Found java.net.http references in Android main sources; transports must rely on OkHttp only." >&2
  exit 1
fi
echo "No java.net.http references found in Android main sources."

binary_target=""
binary_kind="classes jar"

if [[ -z "$AAR_PATH" && -z "$CLASSES_DIR" && ! -f "$CLASSES_JAR" && -f "$DEFAULT_AAR" ]]; then
  AAR_PATH="$DEFAULT_AAR"
fi

if [[ -n "$AAR_PATH" ]]; then
  if [[ ! -f "$AAR_PATH" ]]; then
    echo "error: ANDROID_TRANSPORT_GUARD_AAR points to missing file: $AAR_PATH" >&2
    exit 1
  fi
  echo "Extracting classes.jar from AAR: $AAR_PATH"
  if ! unzip -p "$AAR_PATH" classes.jar >"$WORK_DIR/classes.jar"; then
    echo "error: failed to extract classes.jar from $AAR_PATH" >&2
    exit 1
  fi
  binary_target="$WORK_DIR/classes.jar"
  binary_kind="AAR classes jar"
elif [[ -n "$CLASSES_DIR" && -d "$CLASSES_DIR" ]]; then
  binary_target="$CLASSES_DIR"
  binary_kind="compiled classes directory"
elif [[ -f "$CLASSES_JAR" ]]; then
  binary_target="$CLASSES_JAR"
else
  echo "warning: no compiled artifact found (checked ${CLASSES_JAR} and ANDROID_TRANSPORT_GUARD_CLASSES_DIR);"
  echo "         source-level checks ran, but binary dependency scan was skipped."
  exit 0
fi

if ! command -v jdeps >/dev/null 2>&1; then
  echo "Warning: jdeps not available; skipping binary dependency scan." >&2
  exit 0
fi

echo "Scanning ${binary_kind} at ${binary_target} for java.net.http dependencies..."
if jdeps --multi-release 21 -recursive "$binary_target" | grep -q "java.net.http"; then
  if [[ "$ALLOW_JAVA_HTTP" == "1" ]]; then
    echo "java.net.http detected but allowed via ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP=1"
  else
    echo "java.net.http detected in Android artifact; JVM-only transports must not be packaged." >&2
    exit 1
  fi
else
  echo "No java.net.http dependency detected in Android artifact."
fi

has_forbidden_class() {
  local target=$1
  local class_name=$2
  if [[ -d "$target" ]]; then
    find "$target" -type f -name "${class_name}.class" -print -quit | grep -q .
  else
    jar tf "$target" | grep -q "org/hyperledger/iroha/android/client/${class_name}.class"
  fi
}

if has_forbidden_class "$binary_target" "JavaHttpExecutor"; then
  if [[ "$ALLOW_JAVA_HTTP" == "1" ]]; then
    echo "JavaHttpExecutor present in Android artifact but allowed via ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP=1"
  else
    echo "JavaHttpExecutor present in Android artifact; JVM executor must not ship in the Android AAR." >&2
    exit 1
  fi
else
  echo "JavaHttpExecutor not found in Android artifact."
fi

if has_forbidden_class "$binary_target" "JavaHttpExecutorFactory"; then
  if [[ "$ALLOW_JAVA_HTTP" == "1" ]]; then
    echo "JavaHttpExecutorFactory present in Android artifact but allowed via ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP=1"
  else
    echo "JavaHttpExecutorFactory present in Android artifact; JVM executor must not ship in the Android AAR." >&2
    exit 1
  fi
else
  echo "JavaHttpExecutorFactory not found in Android artifact."
fi

if has_forbidden_class "$binary_target" "JdkWebSocketConnector"; then
  if [[ "$ALLOW_JAVA_HTTP" == "1" ]]; then
    echo "JdkWebSocketConnector present in Android artifact but allowed via ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP=1"
  else
    echo "JdkWebSocketConnector present in Android artifact; JVM websocket connector must not ship in the Android AAR." >&2
    exit 1
  fi
else
  echo "JdkWebSocketConnector not found in Android artifact."
fi

if has_forbidden_class "$binary_target" "JavaTransportWebSocket"; then
  if [[ "$ALLOW_JAVA_HTTP" == "1" ]]; then
    echo "JavaTransportWebSocket present in Android artifact but allowed via ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP=1"
  else
    echo "JavaTransportWebSocket present in Android artifact; JVM websocket transport must not ship in the Android AAR." >&2
    exit 1
  fi
else
  echo "JavaTransportWebSocket not found in Android artifact."
fi

echo "Android transport guard completed successfully."
