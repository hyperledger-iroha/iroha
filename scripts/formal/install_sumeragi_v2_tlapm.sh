#!/usr/bin/env bash
set -euo pipefail

readonly TLAPM_VERSION="1.6.0-pre"
readonly TLAPM_COMMIT="763bf3c1826d77a4cf206f43d5aa16775da1da33"
readonly RELEASE_BASE="https://github.com/tlaplus/tlapm/releases/download/${TLAPM_VERSION}"
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
    readonly PLATFORM="x86_64-linux-gnu"
    readonly ARCHIVE_SHA256="28db02bafd7c899befb696a66812e19a6d2704688f78668cc127cbe4951de8d2"
    ;;
  Darwin-arm64)
    readonly PLATFORM="arm64-darwin"
    readonly ARCHIVE_SHA256="1dddf866712a826f513124a035a7b53278f28c0fc01c749dacb2901f6445cdd2"
    ;;
  *)
    echo "unsupported TLAPM host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

readonly ARCHIVE="tlapm-${TLAPM_VERSION}-${PLATFORM}.tar.gz"
readonly URL="${RELEASE_BASE}/${ARCHIVE}"
readonly INSTALL_ROOT="${TLAPM_INSTALL_ROOT:-${REPO_ROOT}/target/tlapm/toolchains}"
readonly INSTALL_DIR="${INSTALL_ROOT}/${TLAPM_COMMIT}/${PLATFORM}"
readonly TLAPM_BIN="${INSTALL_DIR}/tlapm/bin/tlapm"

verify_install() {
  [[ -x "$TLAPM_BIN" ]] || return 1
  "$TLAPM_BIN" --version 2>&1 | grep -Fq "${TLAPM_COMMIT:0:7}"
}

if verify_install; then
  echo "[tlapm] pinned ${TLAPM_COMMIT} already installed at ${INSTALL_DIR}"
  exit 0
fi

command -v curl >/dev/null 2>&1 || {
  echo "curl is required to install pinned TLAPM" >&2
  exit 1
}
command -v tar >/dev/null 2>&1 || {
  echo "tar is required to install pinned TLAPM" >&2
  exit 1
}

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-tlapm.XXXXXX")"
trap 'rm -rf -- "$tmp_dir"' EXIT

echo "[tlapm] downloading immutable bytes for commit ${TLAPM_COMMIT}"
curl --proto '=https' --tlsv1.2 --fail --location --retry 3 \
  --output "${tmp_dir}/${ARCHIVE}" "$URL"
actual_sha256="$(hash_file "${tmp_dir}/${ARCHIVE}")"
if [[ "$actual_sha256" != "$ARCHIVE_SHA256" ]]; then
  echo "TLAPM archive checksum mismatch" >&2
  echo "expected: ${ARCHIVE_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

mkdir -p "${tmp_dir}/extract"
tar -xzf "${tmp_dir}/${ARCHIVE}" -C "${tmp_dir}/extract"
[[ -x "${tmp_dir}/extract/tlapm/bin/tlapm" ]] || {
  echo "pinned TLAPM archive has an unexpected layout" >&2
  exit 1
}

mkdir -p "$INSTALL_ROOT"
rm -rf -- "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
cp -R "${tmp_dir}/extract"/. "$INSTALL_DIR"/
printf '%s\n' "$ARCHIVE_SHA256" > "${INSTALL_DIR}/archive.sha256"

if ! verify_install; then
  echo "installed TLAPM does not identify commit ${TLAPM_COMMIT}" >&2
  exit 1
fi
echo "[tlapm] installed pinned ${TLAPM_COMMIT} at ${INSTALL_DIR}"
