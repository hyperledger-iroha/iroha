#!/usr/bin/env bash
set -euo pipefail

readonly TLAPM_VERSION="1.6.0-pre"
readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"
readonly RELEASE_ASSET_API_BASE="https://api.github.com/repos/tlaplus/tlapm/releases/assets"
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
    # The rolling release workflow uploaded this object for TLAPM_COMMIT and
    # recorded its ID and digest in GitHub Actions run 29682668751, job
    # 88181482518. Address the object, not the mutable rolling tag and name.
    readonly RELEASE_ASSET_ID="482292328"
    readonly ARCHIVE_SHA256="a686da5dc31892edcd02f25bb14061427e29e16317002d43c5b5be970d1d5daf"
    ;;
  Darwin-arm64)
    readonly PLATFORM="arm64-darwin"
    # The rolling release workflow uploaded this object for TLAPM_COMMIT and
    # recorded its ID and digest in GitHub Actions run 29682668751, job
    # 88181482538. Address the object, not the mutable rolling tag and name.
    readonly RELEASE_ASSET_ID="482297997"
    readonly ARCHIVE_SHA256="3ca4c39613e58b90e46a385ee61e2c7f17375c19854ea1a35e056d6eb902071c"
    ;;
  *)
    echo "unsupported TLAPM host: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

readonly ARCHIVE="tlapm-${TLAPM_VERSION}-${PLATFORM}.tar.gz"
readonly URL="${RELEASE_ASSET_API_BASE}/${RELEASE_ASSET_ID}"
readonly INSTALL_ROOT="${TLAPM_INSTALL_ROOT:-${REPO_ROOT}/target/tlapm/toolchains}"
readonly INSTALL_DIR="${INSTALL_ROOT}/${TLAPM_COMMIT}/${PLATFORM}"
readonly TLAPM_BIN="${INSTALL_DIR}/tlapm/bin/tlapm"

verify_install() {
  [[ -x "$TLAPM_BIN" ]] || return 1
  [[ "$("$TLAPM_BIN" --version 2>&1)" == "${TLAPM_COMMIT:0:7}" ]]
}

if verify_install; then
  echo "[tlapm] pinned ${TLAPM_COMMIT} already installed at ${INSTALL_DIR}"
  exit 0
fi

command -v tar >/dev/null 2>&1 || {
  echo "tar is required to install pinned TLAPM" >&2
  exit 1
}

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-tlapm.XXXXXX")"
trap 'rm -rf -- "$tmp_dir"' EXIT

archive_path="${tmp_dir}/${ARCHIVE}"
if [[ -n "${TLAPM_ARCHIVE_PATH:-}" ]]; then
  [[ -f "$TLAPM_ARCHIVE_PATH" && -r "$TLAPM_ARCHIVE_PATH" ]] || {
    echo "TLAPM_ARCHIVE_PATH must name a readable regular file" >&2
    exit 1
  }
  echo "[tlapm] copying caller-supplied archive for commit ${TLAPM_COMMIT}"
  cp -- "$TLAPM_ARCHIVE_PATH" "$archive_path"
else
  command -v curl >/dev/null 2>&1 || {
    echo "curl is required to download pinned TLAPM" >&2
    exit 1
  }
  echo "[tlapm] downloading release asset ${RELEASE_ASSET_ID} for commit ${TLAPM_COMMIT}"
  if ! curl --proto '=https' --tlsv1.2 --fail --location --retry 3 \
    --header 'Accept: application/octet-stream' \
    --header 'X-GitHub-Api-Version: 2022-11-28' \
    --output "$archive_path" "$URL"; then
    echo "pinned TLAPM release asset ${RELEASE_ASSET_ID} is unavailable" >&2
    echo "the upstream rolling release deletes superseded assets; it is unsafe to use its tag URL" >&2
    echo "set TLAPM_ARCHIVE_PATH to a retained copy with SHA-256 ${ARCHIVE_SHA256}" >&2
    exit 1
  fi
fi
actual_sha256="$(hash_file "$archive_path")"
if [[ "$actual_sha256" != "$ARCHIVE_SHA256" ]]; then
  echo "TLAPM archive checksum mismatch" >&2
  echo "expected: ${ARCHIVE_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
fi

mkdir -p "${tmp_dir}/extract"
tar -xzf "$archive_path" -C "${tmp_dir}/extract"
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
