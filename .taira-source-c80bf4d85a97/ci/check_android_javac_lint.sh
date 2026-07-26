#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0
#
# Prototype static-analysis gate for AND6. Compiles the Android SDK sources
# (including the shared Norito adapters) with strict javac lint settings and
# treats any remaining warnings as errors.

set -euo pipefail

ROOT_DIR=$(git rev-parse --show-toplevel)
ANDROID_DIR="$ROOT_DIR/java/iroha_android"
NORITO_DIR="$ROOT_DIR/java/norito_java/src/main/java"
DEFAULT_SUMMARY_DEST="$ROOT_DIR/artifacts/android/lint/jdeps-summary.txt"

if [[ ! -d "$ANDROID_DIR/src" ]]; then
    echo "Android sources not found at $ANDROID_DIR" >&2
    exit 1
fi
if [[ ! -d "$NORITO_DIR" ]]; then
    echo "Norito sources not found at $NORITO_DIR" >&2
    exit 1
fi

if ! command -v javac >/dev/null 2>&1; then
    echo "javac is required to run Android lint; install JDK 21+ or set JAVA_HOME." >&2
    exit 1
fi
if ! command -v jdeps >/dev/null 2>&1; then
    echo "jdeps is required to validate Android dependencies; install JDK 21+ or set JAVA_HOME." >&2
    exit 1
fi

WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/android-lint.XXXXXX")
KEEP_WORKDIR="${ANDROID_LINT_KEEP_WORKDIR:-}"
cleanup() {
    if [[ -z "$KEEP_WORKDIR" ]]; then
        rm -rf "$WORK_DIR"
    else
        echo "info: preserving lint workspace at $WORK_DIR" >&2
    fi
}
trap cleanup EXIT INT TERM

CLASSES_DIR="$WORK_DIR/classes"
mkdir -p "$CLASSES_DIR"

ANDROID_SOURCES=()
while IFS= read -r -d '' file; do
    ANDROID_SOURCES+=("$file")
done < <(find "$ANDROID_DIR/src" -name '*.java' -print0)

NORITO_SOURCES=()
while IFS= read -r -d '' file; do
    NORITO_SOURCES+=("$file")
done < <(find "$NORITO_DIR" -name '*.java' -print0)

if [[ ${#ANDROID_SOURCES[@]} -eq 0 ]]; then
    echo "No Android Java sources discovered under $ANDROID_DIR/src" >&2
    exit 1
fi
if [[ ${#NORITO_SOURCES[@]} -eq 0 ]]; then
    echo "No Norito Java sources discovered under $NORITO_DIR" >&2
    exit 1
fi

JAVAC_FLAGS=("-d" "$CLASSES_DIR")
if javac --release 21 -version >/dev/null 2>&1; then
    JAVAC_FLAGS=(--release 21 "-d" "$CLASSES_DIR")
fi

JAVAC_FLAGS+=(
    "-Xlint:all"
    "-Xlint:-deprecation"
    "-Xlint:-unchecked"
    "-Werror"
)

echo "info: compiling Android SDK sources with javac lint checks…"
javac \
    "${JAVAC_FLAGS[@]}" \
    "${NORITO_SOURCES[@]}" \
    "${ANDROID_SOURCES[@]}"

SUMMARY_FILE="$WORK_DIR/jdeps-summary.txt"
echo "info: analysing compiled classes for forbidden module dependencies…"
jdeps --multi-release 21 -recursive -summary "$CLASSES_DIR" >"$SUMMARY_FILE"

ALLOWED_DEPENDENCY_MODULES=("java.base" "java.net.http" "jdk.httpserver")
DEPENDENCY_MODULES=()
DISALLOWED_MODULES=()

is_allowed_module() {
    local candidate=$1
    for allowed in "${ALLOWED_DEPENDENCY_MODULES[@]}"; do
        if [[ "$allowed" == "$candidate" ]]; then
            return 0
        fi
    done
    return 1
}

module_seen() {
    local candidate=$1
    if [[ ${#DEPENDENCY_MODULES[@]} -eq 0 ]]; then
        return 1
    fi
    for seen in "${DEPENDENCY_MODULES[@]}"; do
        if [[ "$seen" == "$candidate" ]]; then
            return 0
        fi
    done
    return 1
}

while read -r _ arrow target; do
    [[ -z "$target" ]] && continue
    # Lines look like "<source> -> <module>"
    local_module=${target%%,*}
    local_module=${local_module%%:*}
    local_module=$(echo "$local_module" | tr -d '[:space:]')
    [[ -z "$local_module" ]] && continue
    if module_seen "$local_module"; then
        continue
    fi
    DEPENDENCY_MODULES+=("$local_module")
    if ! is_allowed_module "$local_module"; then
        DISALLOWED_MODULES+=("$local_module")
    fi
done <"$SUMMARY_FILE"

if [[ ${#DISALLOWED_MODULES[@]} -gt 0 ]]; then
    printf 'error: Android lint detected dependencies on unsupported modules: %s\n' "${DISALLOWED_MODULES[*]}" >&2
    echo "       Update ALLOWED_DEPENDENCY_MODULES in ci/check_android_javac_lint.sh if this dependency list is expected." >&2
    exit 1
fi

copy_summary_file() {
    local destination=$1
    mkdir -p "$(dirname "$destination")"
    cp "$SUMMARY_FILE" "$destination"
    echo "info: copied module summary to $destination"
}

copy_summary_file "$DEFAULT_SUMMARY_DEST"

if [[ -n "${ANDROID_LINT_SUMMARY_OUT:-}" ]] && [[ "$ANDROID_LINT_SUMMARY_OUT" != "$DEFAULT_SUMMARY_DEST" ]]; then
    copy_summary_file "$ANDROID_LINT_SUMMARY_OUT"
fi

echo "info: dependency modules detected: ${DEPENDENCY_MODULES[*]}"
echo "info: Android javac lint run completed successfully."
