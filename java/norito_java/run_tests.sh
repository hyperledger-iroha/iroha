#!/usr/bin/env bash
# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

if [[ "${NORITO_JAVA_SKIP_TESTS:-}" == "1" ]]; then
  echo "[norito-java] Skipping JVM parity tests (NORITO_JAVA_SKIP_TESTS=1)." >&2
  exit 0
fi

ROOT=$(cd "$(dirname "$0")" && pwd)
GRADLEW="$ROOT/../iroha_android/gradlew"
if [[ ! -x "$GRADLEW" ]]; then
  echo "Gradle wrapper not found at $GRADLEW" >&2
  exit 1
fi

exec "$GRADLEW" -p "$ROOT" runNoritoTests --console=plain
