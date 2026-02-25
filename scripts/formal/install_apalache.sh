#!/bin/bash
set -euo pipefail

version="${1:-0.52.2}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
install_root="${APALACHE_INSTALL_ROOT:-$root_dir/target/apalache/toolchains}"
install_dir="$install_root/v$version"
archive_name="apalache-${version}.tgz"
release_base="https://github.com/apalache-mc/apalache/releases/download/v${version}"
archive_url="${release_base}/${archive_name}"
checksums_url="${release_base}/sha256sum.txt"

hash_file_cmd() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return 0
  fi
  shasum -a 256 "$file" | awk '{print $1}'
}

if [[ -x "$install_dir/bin/apalache-mc" ]]; then
  echo "[apalache] already installed at $install_dir"
  echo "[apalache] version v$version is ready"
  exit 0
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "[apalache] downloading checksum file: $checksums_url"
curl -fsSL "$checksums_url" -o "$tmp_dir/sha256sum.txt"
expected_sha256="$(awk '$2 == "'"$archive_name"'" {print $1}' "$tmp_dir/sha256sum.txt")"

if [[ -z "$expected_sha256" ]]; then
  echo "error: checksum for '$archive_name' not found in release manifest" >&2
  exit 1
fi

echo "[apalache] downloading archive: $archive_url"
curl -fsSL "$archive_url" -o "$tmp_dir/$archive_name"
actual_sha256="$(hash_file_cmd "$tmp_dir/$archive_name")"

if [[ "$actual_sha256" != "$expected_sha256" ]]; then
  echo "error: checksum mismatch for '$archive_name'" >&2
  echo "expected: $expected_sha256" >&2
  echo "actual:   $actual_sha256" >&2
  exit 1
fi

echo "[apalache] checksum verified ($actual_sha256)"
tar -xzf "$tmp_dir/$archive_name" -C "$tmp_dir"

src_dir="$tmp_dir/apalache-$version"
if [[ ! -x "$src_dir/bin/apalache-mc" ]]; then
  echo "error: extracted archive missing executable '$src_dir/bin/apalache-mc'" >&2
  exit 1
fi

mkdir -p "$install_root"
rm -rf "$install_dir"
mkdir -p "$install_dir"
cp -R "$src_dir"/. "$install_dir"/
chmod +x "$install_dir/bin/apalache-mc"

echo "[apalache] installed to $install_dir"
echo "[apalache] version v$version is ready"
echo "[apalache] runner default path now resolves to:"
echo "  $install_dir/bin/apalache-mc"
