#!/usr/bin/env bash
# Resolve one canonical, working Java runtime for the Sumeragi v2 formal gates.

set -euo pipefail

if (($# > 1)); then
  echo "usage: $0 [java-executable]" >&2
  exit 2
fi

canonical_path() {
  python3 -c \
    'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
    "$1"
}

working_java() {
  local candidate="$1"
  local executable
  if [[ "$candidate" == */* ]]; then
    executable="$candidate"
  else
    executable="$(command -v "$candidate" 2>/dev/null)" || return 1
  fi
  [[ -f "$executable" && -x "$executable" ]] || return 1
  "$executable" -version >/dev/null 2>&1 || return 1
  canonical_path "$executable"
}

if (($# == 1)); then
  if resolved="$(working_java "$1")"; then
    printf '%s\n' "$resolved"
    exit 0
  fi
  echo "configured Java runtime is not a working executable: $1" >&2
  exit 1
fi

candidates=()
if [[ -n "${JAVA_HOME:-}" ]]; then
  candidates+=("${JAVA_HOME}/bin/java")
fi
candidates+=(
  java
)

if [[ -x /usr/libexec/java_home ]]; then
  for version in 21 17; do
    if java_home="$(/usr/libexec/java_home -v "$version" 2>/dev/null)"; then
      candidates+=("${java_home}/bin/java")
    fi
  done
fi

# Stable package-manager links avoid binding discovery to a particular Cellar
# patch directory. The selected canonical binary path and bytes are recorded in
# the release evidence after resolution.
candidates+=(
  /opt/homebrew/opt/openjdk@21/bin/java
  /usr/local/opt/openjdk@21/bin/java
  /opt/homebrew/opt/openjdk@17/bin/java
  /usr/local/opt/openjdk@17/bin/java
  /opt/homebrew/opt/openjdk/bin/java
  /usr/local/opt/openjdk/bin/java
  /usr/lib/jvm/java-21-openjdk/bin/java
  /usr/lib/jvm/java-21-openjdk-amd64/bin/java
  /usr/lib/jvm/java-21-openjdk-arm64/bin/java
  /usr/lib/jvm/java-17-openjdk/bin/java
  /usr/lib/jvm/java-17-openjdk-amd64/bin/java
  /usr/lib/jvm/java-17-openjdk-arm64/bin/java
  /usr/lib/jvm/default-java/bin/java
)

for candidate in "${candidates[@]}"; do
  if resolved="$(working_java "$candidate")"; then
    printf '%s\n' "$resolved"
    exit 0
  fi
done

echo "no working Java runtime was found; set JAVA_BIN or JAVA_HOME" >&2
exit 1
