#!/usr/bin/env bash
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

# Kotlin owns argument validation and attestation verification. This launcher
# builds the JVM tool and passes the caller's arguments without interpretation.
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
"$REPO_ROOT/kotlin/gradlew" -p "$REPO_ROOT/kotlin" \
  :tools:installDist --console=plain >&2

ATTESTATION_BUILD_DIR="$REPO_ROOT/kotlin/tools/build"
if [[ -n "${MOBILE_SDK_ANDROID_ARTIFACT_DIR:-}" ]]; then
  # Gradle validates this reviewed external artifact root before installDist.
  ATTESTATION_BUILD_DIR="$MOBILE_SDK_ANDROID_ARTIFACT_DIR/gradle-build/iroha_kotlin_sdk/tools"
fi
exec "$ATTESTATION_BUILD_DIR/install/iroha-attestation/bin/iroha-attestation" "$@"
