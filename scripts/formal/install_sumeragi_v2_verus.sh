#!/usr/bin/env bash
set -euo pipefail

readonly VERSION="0.2026.05.31.5dd6d83"
readonly RELEASE_BASE="https://github.com/verus-lang/verus/releases/download/release/${VERSION}"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    readonly PLATFORM="x86-linux"
    readonly EXTRACTED_DIR="verus-x86-linux"
    readonly ARCHIVE_SHA256="d234121e38718860e00edaadd2807278f720f9715f8d6c90e22f5d606be92cf1"
    ;;
  Darwin-arm64)
    readonly PLATFORM="arm64-macos"
    readonly EXTRACTED_DIR="verus-arm64-macos"
    readonly ARCHIVE_SHA256="7dc6c255a58d1432ac05c5576554ae110782e291357bdd0aaad440c32f351ce3"
    ;;
  Darwin-x86_64)
    readonly PLATFORM="x86-macos"
    readonly EXTRACTED_DIR="verus-x86-macos"
    readonly ARCHIVE_SHA256="d9177554b9e045d0b5462aad245477cce24d91f22bf4e09e336a31b493bfe4d7"
    ;;
  *)
    echo "unsupported Verus host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

readonly ARCHIVE="verus-${VERSION}-${PLATFORM}.zip"
readonly URL="${RELEASE_BASE}/${ARCHIVE}"
readonly INSTALL_ROOT="${VERUS_INSTALL_ROOT:?VERUS_INSTALL_ROOT must be an explicitly authorized external directory}"
readonly INSTALL_DIR="${INSTALL_ROOT}/${VERSION}/${PLATFORM}"

source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"
require_external_private_directory \
  "$REPO_ROOT" "$INSTALL_ROOT" "Verus install" || exit $?

verify_install() {
  [[ -x "${INSTALL_DIR}/verus" && -x "${INSTALL_DIR}/cargo-verus" ]] || return 1
  "${INSTALL_DIR}/verus" --version 2>&1 | grep -Fq "$VERSION"
}

if verify_install; then
  echo "[verus] pinned ${VERSION} already installed at ${INSTALL_DIR}"
  exit 0
fi

command -v unzip >/dev/null 2>&1 || {
  echo "unzip is required to install pinned Verus" >&2
  exit 1
}
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-verus.XXXXXX")"
trap 'rm -rf -- "$tmp_dir"' EXIT

curl --proto '=https' --tlsv1.2 --fail --location --retry 3 \
  --output "${tmp_dir}/${ARCHIVE}" "$URL"
actual_sha256="$(hash_file "${tmp_dir}/${ARCHIVE}")"
if [[ "$actual_sha256" != "$ARCHIVE_SHA256" ]]; then
  echo "Verus archive checksum mismatch" >&2
  echo "expected: ${ARCHIVE_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

unzip -q "${tmp_dir}/${ARCHIVE}" -d "${tmp_dir}/extract"
source_dir="${tmp_dir}/extract/${EXTRACTED_DIR}"
[[ -f "${source_dir}/verus" && -f "${source_dir}/cargo-verus" ]] || {
  echo "pinned Verus archive has an unexpected layout" >&2
  exit 1
}

mkdir -p "$INSTALL_ROOT"
rm -rf -- "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
cp -R "${source_dir}"/. "$INSTALL_DIR"/
chmod +x "${INSTALL_DIR}/verus" "${INSTALL_DIR}/cargo-verus"
printf '%s\n' "$ARCHIVE_SHA256" > "${INSTALL_DIR}/archive.sha256"

if ! verify_install; then
  echo "installed Verus does not identify version ${VERSION}" >&2
  exit 1
fi
echo "[verus] installed pinned ${VERSION} at ${INSTALL_DIR}"
