#!/usr/bin/env bash
set -euo pipefail

readonly VERSION="1.8.0"
readonly JAR_SHA256="33de7da9ce1b7fffb9d1c184021178dbb051747be48504e65c584c423721a32e"
readonly URL="https://github.com/tlaplus/tlaplus/releases/download/v${VERSION}/tla2tools.jar"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly INSTALL_DIR="${TLA2TOOLS_INSTALL_ROOT:-${REPO_ROOT}/target/tla2tools/${VERSION}}"
readonly JAR="${INSTALL_DIR}/tla2tools.jar"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

if [[ -f "$JAR" ]] && [[ "$(hash_file "$JAR")" == "$JAR_SHA256" ]]; then
  echo "[tla2tools] pinned v${VERSION} already installed at ${JAR}"
  exit 0
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-tla2tools.XXXXXX")"
trap 'rm -rf -- "$tmp_dir"' EXIT
curl --proto '=https' --tlsv1.2 --fail --location --retry 3 \
  --output "${tmp_dir}/tla2tools.jar" "$URL"
actual_sha256="$(hash_file "${tmp_dir}/tla2tools.jar")"
if [[ "$actual_sha256" != "$JAR_SHA256" ]]; then
  echo "TLA2Tools archive checksum mismatch" >&2
  echo "expected: ${JAR_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
cp "${tmp_dir}/tla2tools.jar" "$JAR"
echo "[tla2tools] installed pinned v${VERSION} at ${JAR}"
