#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/android_keystore_attestation.sh --bundle-dir <path> --trust-root <root.pem> [options]

Verify an Android Keystore attestation bundle using the Iroha Android attestation harness.

Required:
  --bundle-dir <path>      Directory containing chain.pem/alias.txt/challenge.hex (see docs).
  --trust-root <path>      Trusted root certificate (PEM/DER). Repeat for additional roots.

Optional:
  --trust-root-dir <path>  Directory containing trusted roots (PEM/DER/CRT). Repeat as needed.
  --trust-root-bundle <zip>  ZIP archive containing trusted roots. Repeat as needed.
  --chain <path>           Explicit attestation chain file (PEM/DER). Overrides bundle-dir lookup.
  --alias <alias>          Override alias value (defaults to alias.txt or a placeholder).
  --challenge-hex <hex>    Hex-encoded challenge (overrides challenge.hex).
  --challenge-file <path>  Read hex challenge from the provided file.
  --require-strongbox      Enforce StrongBox attestation.
  --output <path>          Write JSON summary to <path>.
  --help                   Show this message.

The script compiles the Iroha Android library with the Norito Java sources and executes the
attestation harness. See docs/source/sdk/android/readiness/android_strongbox_device_matrix.md
for collection guidance and required firmware levels.
EOF
}

if [[ $# -eq 0 ]]; then
  usage >&2
  exit 1
fi

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ANDROID_ROOT="${REPO_ROOT}/java/iroha_android"
NORITO_ROOT="${ANDROID_ROOT}/../norito_java/src/main/java"

BUNDLE_DIR=""
CHAIN_FILE=""
ALIAS_OVERRIDE=""
CHALLENGE_HEX=""
CHALLENGE_FILE=""
OUTPUT=""
REQUIRE_STRONGBOX=0
declare -a TRUST_ROOTS
declare -a TRUST_ROOT_DIRS
declare -a TRUST_ROOT_BUNDLES

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundle-dir)
      BUNDLE_DIR="$2"
      shift 2
      ;;
    --chain)
      CHAIN_FILE="$2"
      shift 2
      ;;
    --alias)
      ALIAS_OVERRIDE="$2"
      shift 2
      ;;
    --challenge-hex)
      CHALLENGE_HEX="$2"
      shift 2
      ;;
    --challenge-file)
      CHALLENGE_FILE="$2"
      shift 2
      ;;
    --trust-root)
      TRUST_ROOTS+=("$2")
      shift 2
      ;;
    --trust-root-dir)
      TRUST_ROOT_DIRS+=("$2")
      shift 2
      ;;
    --trust-root-bundle)
      TRUST_ROOT_BUNDLES+=("$2")
      shift 2
      ;;
    --output)
      OUTPUT="$2"
      shift 2
      ;;
    --require-strongbox)
      REQUIRE_STRONGBOX=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$BUNDLE_DIR" && -z "$CHAIN_FILE" ]]; then
  echo "Either --bundle-dir or --chain must be supplied." >&2
  usage >&2
  exit 1
fi

if [[ ${#TRUST_ROOTS[@]} -eq 0 && ${#TRUST_ROOT_DIRS[@]} -eq 0 && ${#TRUST_ROOT_BUNDLES[@]} -eq 0 ]]; then
  echo "At least one --trust-root, --trust-root-dir, or --trust-root-bundle must be supplied." >&2
  usage >&2
  exit 1
fi

if [[ -n "$BUNDLE_DIR" && ! -d "$BUNDLE_DIR" ]]; then
  echo "Bundle directory not found: $BUNDLE_DIR" >&2
  exit 1
fi

if [[ -n "$CHAIN_FILE" && ! -f "$CHAIN_FILE" ]]; then
  echo "Chain file not found: $CHAIN_FILE" >&2
  exit 1
fi

if [[ -n "$CHALLENGE_FILE" && ! -f "$CHALLENGE_FILE" ]]; then
  echo "Challenge file not found: $CHALLENGE_FILE" >&2
  exit 1
fi

if ((${#TRUST_ROOTS[@]} > 0)); then
  for root in "${TRUST_ROOTS[@]}"; do
    if [[ ! -f "$root" ]]; then
      echo "Trust root not found: $root" >&2
      exit 1
    fi
  done
fi

if ((${#TRUST_ROOT_DIRS[@]} > 0)); then
  for dir in "${TRUST_ROOT_DIRS[@]}"; do
    if [[ ! -d "$dir" ]]; then
      echo "Trust root directory not found: $dir" >&2
      exit 1
    fi
  done
fi

if ((${#TRUST_ROOT_BUNDLES[@]} > 0)); then
  for bundle in "${TRUST_ROOT_BUNDLES[@]}"; do
    if [[ ! -f "$bundle" ]]; then
      echo "Trust root bundle not found: $bundle" >&2
      exit 1
    fi
  done
fi

ensure_java_tool() {
  local tool=$1
  if command -v "$tool" >/dev/null 2>&1; then
    return 0
  fi
  if [[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/$tool" ]]; then
    PATH="$JAVA_HOME/bin:$PATH"
    export PATH
    return 0
  fi
  if [[ "$tool" == "java" ]]; then
    local candidate
    candidate=$( /usr/libexec/java_home 2>/dev/null || true )
    if [[ -n "$candidate" && -x "$candidate/bin/java" ]]; then
      PATH="$candidate/bin:$PATH"
      export PATH
      export JAVA_HOME="${JAVA_HOME:-$candidate}"
      return 0
    fi
  fi
  return 1
}

if ! ensure_java_tool javac; then
  echo "javac not found; install JDK 21+ or set JAVA_HOME" >&2
  exit 1
fi

if ! ensure_java_tool java; then
  echo "java runtime not found; install JDK 21+ or set JAVA_HOME" >&2
  exit 1
fi

BUILD_DIR=$(mktemp -d "${TMPDIR:-/tmp}/iroha-android-attestation.XXXXXX")
trap 'rm -rf "$BUILD_DIR"' EXIT

CLASSES="$BUILD_DIR/classes"
mkdir -p "$CLASSES"

if [[ ! -d "$NORITO_ROOT" ]]; then
  echo "Expected norito_java sources at $NORITO_ROOT" >&2
  exit 1
fi

MAIN_SOURCES=$(find "$ANDROID_ROOT/src/main/java" -name '*.java')
NORITO_SOURCES=$(find "$NORITO_ROOT" -name '*.java')

JAVAC_FLAGS=("-d" "$CLASSES")
if javac --release 21 -version >/dev/null 2>&1; then
  JAVAC_FLAGS=("--release" "21" "-d" "$CLASSES")
fi

javac "${JAVAC_FLAGS[@]}" $NORITO_SOURCES $MAIN_SOURCES

COMMAND=("java" "-cp" "$CLASSES" "org.hyperledger.iroha.android.tools.AndroidKeystoreAttestationHarness")

if [[ -n "$BUNDLE_DIR" ]]; then
  COMMAND+=("--bundle-dir" "$(cd "$BUNDLE_DIR" && pwd)")
fi

if [[ -n "$CHAIN_FILE" ]]; then
  COMMAND+=("--chain" "$(cd "$(dirname "$CHAIN_FILE")" && pwd)/$(basename "$CHAIN_FILE")")
fi

if ((${#TRUST_ROOTS[@]} > 0)); then
  for root in "${TRUST_ROOTS[@]}"; do
    COMMAND+=("--trust-root" "$(cd "$(dirname "$root")" && pwd)/$(basename "$root")")
  done
fi

if ((${#TRUST_ROOT_DIRS[@]} > 0)); then
  for dir in "${TRUST_ROOT_DIRS[@]}"; do
    COMMAND+=("--trust-root-dir" "$(cd "$dir" && pwd)")
  done
fi

if ((${#TRUST_ROOT_BUNDLES[@]} > 0)); then
  for bundle in "${TRUST_ROOT_BUNDLES[@]}"; do
    COMMAND+=("--trust-root-bundle" "$(cd "$(dirname "$bundle")" && pwd)/$(basename "$bundle")")
  done
fi

if [[ -n "$ALIAS_OVERRIDE" ]]; then
  COMMAND+=("--alias" "$ALIAS_OVERRIDE")
fi

if [[ -n "$CHALLENGE_HEX" ]]; then
  COMMAND+=("--challenge-hex" "$CHALLENGE_HEX")
elif [[ -n "$CHALLENGE_FILE" ]]; then
  COMMAND+=("--challenge-file" "$(cd "$(dirname "$CHALLENGE_FILE")" && pwd)/$(basename "$CHALLENGE_FILE")")
fi

if [[ $REQUIRE_STRONGBOX -eq 1 ]]; then
  COMMAND+=("--require-strongbox")
fi

if [[ -n "$OUTPUT" ]]; then
  OUTPUT_ABS="$(cd "$(dirname "$OUTPUT")" && pwd)/$(basename "$OUTPUT")"
  COMMAND+=("--output" "$OUTPUT_ABS")
fi

"${COMMAND[@]}"
