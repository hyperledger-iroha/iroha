#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
SOURCE_ROOTS=(
  "$ROOT/kotlin/core-jvm/src/main"
  "$ROOT/kotlin/client-android/src/main"
  "$ROOT/kotlin/offline-wallet-android/src/main"
)
EXISTING_ROOTS=()
for source_root in "${SOURCE_ROOTS[@]}"; do
  if [[ -d "$source_root" ]]; then
    EXISTING_ROOTS+=("$source_root")
  fi
done

if [[ ${#EXISTING_ROOTS[@]} -eq 0 ]]; then
  echo "Kotlin production source roots are missing." >&2
  exit 1
fi

REFLECTION_PATTERN='java[.]lang[.]reflect|kotlin[.]reflect|Class[.]forName[[:space:]]*[(]|ReflectiveOperationException|[.]getDeclared(Method|Constructor|Field)[[:space:]]*[(]|[.]get(Method|Constructor|Field)[[:space:]]*[(]'

if rg -n \
  --glob '*.kt' \
  --glob '*.java' \
  "$REFLECTION_PATTERN" \
  "${EXISTING_ROOTS[@]}"; then
  echo "Reflection is forbidden in Kotlin SDK production sources." >&2
  exit 1
fi

echo "Kotlin SDK production sources are reflection-free."
