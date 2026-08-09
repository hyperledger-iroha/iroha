#!/usr/bin/env bash
set -euo pipefail

# v1.8.0 is a rolling pre-release whose asset is overwritten by upstream
# master builds. Use the immutable stable release so this checksum remains
# reproducible.
readonly VERSION="1.7.4"
readonly JAR_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly URL="https://github.com/tlaplus/tlaplus/releases/download/v${VERSION}/tla2tools.jar"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly INSTALL_ROOT="${TLA2TOOLS_INSTALL_ROOT:?TLA2TOOLS_INSTALL_ROOT must be an explicitly authorized external directory}"
readonly INSTALL_DIR="${INSTALL_ROOT}/${VERSION}"
readonly JAR="${INSTALL_DIR}/tla2tools.jar"

source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"
require_external_private_directory \
  "$REPO_ROOT" "$INSTALL_ROOT" "TLA2Tools install" || exit $?

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
