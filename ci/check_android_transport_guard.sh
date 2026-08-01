#!/usr/bin/env bash
# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$ROOT/java/iroha_android/android"
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

if [[ -n "${ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP+x}" ]]; then
  echo "error: ANDROID_TRANSPORT_GUARD_ALLOW_JAVA_HTTP is retired; Android artifacts may never contain JVM transports" >&2
  exit 1
fi

if [[ -n "$AAR_PATH" && -n "$CLASSES_DIR" ]]; then
  echo "error: set only one of ANDROID_TRANSPORT_GUARD_AAR or ANDROID_TRANSPORT_GUARD_CLASSES_DIR" >&2
  exit 1
fi

if [[ $# -gt 0 && ( -n "$AAR_PATH" || -n "$CLASSES_DIR" ) ]]; then
  echo "error: the classes.jar argument cannot be combined with an artifact environment selector" >&2
  exit 1
fi

if command -v rg >/dev/null 2>&1; then
  scan_cmd=(rg -n '^[[:space:]]*import[[:space:]]+(static[[:space:]]+)?java\.net\.http([.;])')
else
  scan_cmd=(grep -R -E -n '^[[:space:]]*import[[:space:]]+(static[[:space:]]+)?java\.net\.http([.;])')
fi

echo "Checking Android main sources for java.net.http imports..."
if "${scan_cmd[@]}" "$ANDROID_DIR/src/main/java"; then
  echo "Found java.net.http references in Android main sources; transports must rely on OkHttp only." >&2
  exit 1
fi
echo "No java.net.http imports found in Android main sources."

binary_target=""
binary_kind="classes jar"

if [[ $# -eq 0 && -z "$AAR_PATH" && -z "$CLASSES_DIR" \
  && ! -f "$CLASSES_JAR" && -f "$DEFAULT_AAR" ]]; then
  AAR_PATH="$DEFAULT_AAR"
fi

if [[ -n "$AAR_PATH" ]]; then
  if [[ ! -f "$AAR_PATH" ]]; then
    echo "error: ANDROID_TRANSPORT_GUARD_AAR points to missing file: $AAR_PATH" >&2
    exit 1
  fi
  if ! command -v unzip >/dev/null 2>&1; then
    echo "error: unzip is required to inspect the Android AAR" >&2
    exit 1
  fi
  echo "Extracting classes.jar from AAR: $AAR_PATH"
  if ! unzip -p "$AAR_PATH" classes.jar >"$WORK_DIR/classes.jar" \
    || [[ ! -s "$WORK_DIR/classes.jar" ]]; then
    echo "error: failed to extract classes.jar from $AAR_PATH" >&2
    exit 1
  fi
  binary_target="$WORK_DIR/classes.jar"
  binary_kind="AAR classes jar"
elif [[ -n "$CLASSES_DIR" ]]; then
  if [[ ! -d "$CLASSES_DIR" ]]; then
    echo "error: ANDROID_TRANSPORT_GUARD_CLASSES_DIR points to a missing directory: $CLASSES_DIR" >&2
    exit 1
  fi
  if ! find "$CLASSES_DIR" -type f -name '*.class' -print -quit | grep -q .; then
    echo "error: ANDROID_TRANSPORT_GUARD_CLASSES_DIR contains no compiled classes: $CLASSES_DIR" >&2
    exit 1
  fi
  binary_target="$CLASSES_DIR"
  binary_kind="compiled classes directory"
elif [[ -f "$CLASSES_JAR" ]]; then
  binary_target="$CLASSES_JAR"
else
  echo "error: no compiled Android artifact found (checked $CLASSES_JAR and $DEFAULT_AAR)" >&2
  exit 1
fi

if ! command -v jdeps >/dev/null 2>&1; then
  echo "error: jdeps is required for the Android binary dependency scan" >&2
  exit 1
fi

echo "Scanning ${binary_kind} at ${binary_target} for java.net.http dependencies..."
if ! jdeps --multi-release 21 -recursive "$binary_target" >"$WORK_DIR/jdeps.txt"; then
  echo "error: jdeps could not inspect ${binary_kind} at ${binary_target}" >&2
  exit 1
fi
if grep -q "java.net.http" "$WORK_DIR/jdeps.txt"; then
  echo "java.net.http detected in Android artifact; JVM-only transports must not be packaged." >&2
  exit 1
else
  echo "No java.net.http dependency detected in Android artifact."
fi

archive_entries=""
if [[ ! -d "$binary_target" ]]; then
  if ! command -v jar >/dev/null 2>&1; then
    echo "error: jar is required to inspect Android archive contents" >&2
    exit 1
  fi
  archive_entries="$WORK_DIR/archive-entries.txt"
  if ! jar tf "$binary_target" >"$archive_entries"; then
    echo "error: jar could not inspect ${binary_kind} at ${binary_target}" >&2
    exit 1
  fi
fi

has_forbidden_class() {
  local target=$1
  local class_name=$2
  if [[ -d "$target" ]]; then
    find "$target" -type f -name "${class_name}.class" -print -quit | grep -q .
  else
    grep -Fxq "org/hyperledger/iroha/android/client/${class_name}.class" "$archive_entries"
  fi
}

if has_forbidden_class "$binary_target" "JavaHttpExecutor"; then
  echo "JavaHttpExecutor present in Android artifact; JVM executor must not ship in the Android AAR." >&2
  exit 1
else
  echo "JavaHttpExecutor not found in Android artifact."
fi

if has_forbidden_class "$binary_target" "JavaHttpExecutorFactory"; then
  echo "JavaHttpExecutorFactory present in Android artifact; JVM executor must not ship in the Android AAR." >&2
  exit 1
else
  echo "JavaHttpExecutorFactory not found in Android artifact."
fi

if has_forbidden_class "$binary_target" "JdkWebSocketConnector"; then
  echo "JdkWebSocketConnector present in Android artifact; JVM websocket connector must not ship in the Android AAR." >&2
  exit 1
else
  echo "JdkWebSocketConnector not found in Android artifact."
fi

if has_forbidden_class "$binary_target" "JavaTransportWebSocket"; then
  echo "JavaTransportWebSocket present in Android artifact; JVM websocket transport must not ship in the Android AAR." >&2
  exit 1
else
  echo "JavaTransportWebSocket not found in Android artifact."
fi

echo "Android transport guard completed successfully."
