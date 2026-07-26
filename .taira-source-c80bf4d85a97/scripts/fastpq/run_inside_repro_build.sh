#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Invoked inside the FASTPQ reproducible build container. Builds requested
# binaries, copies artefacts into the output directory, and emits hashes +
# manifest metadata.

set -euo pipefail

umask 0022
export TZ=UTC

source_epoch="$(git log -1 --format=%ct 2>/dev/null || date +%s)"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-${source_epoch}}"

profile="${FASTPQ_PROFILE:-release}"
bins_csv="${FASTPQ_BINS:-irohad}"
features="${FASTPQ_FEATURES:-}"
output_dir="${FASTPQ_OUTPUT_DIR:-artifacts/fastpq-repro}"

IFS=',' read -r -a bins <<< "${bins_csv}"
if [[ ${#bins[@]} -eq 0 ]]; then
  echo "error: FASTPQ_BINS is empty" >&2
  exit 1
fi

mkdir -p "${output_dir}"

echo "==> Using Rust toolchain: ${FASTPQ_RUST_TOOLCHAIN:-unknown}"
rustup show active-toolchain

echo "==> Fetching dependencies (locked)"
cargo fetch --locked

echo "==> Building binaries (${bins_csv}) with profile=${profile}"
build_args=(build --locked --profile "${profile}")
if [[ -n "${features// /}" ]]; then
  build_args+=(--features "${features}")
fi

for bin in "${bins[@]}"; do
  echo "---- cargo ${build_args[*]} --bin ${bin}"
  mold --run cargo "${build_args[@]}" --bin "${bin}"
done

manifest_path="${output_dir}/manifest.json"
hash_path="${output_dir}/sha256s.txt"
truncate -s 0 "${hash_path}"

bin_lines=()
for bin in "${bins[@]}"; do
  # Prefer native target directory first
  candidate="target/${profile}/${bin}"
  if [[ ! -f "${candidate}" ]]; then
    candidate="$(find target -type f -perm -0100 -name "${bin}" -path "*/${profile}/*" -print -quit)"
  fi
  if [[ -z "${candidate}" || ! -f "${candidate}" ]]; then
    echo "error: unable to locate built binary for '${bin}'" >&2
    exit 1
  fi
  dest="${output_dir}/${bin}"
  cp "${candidate}" "${dest}"
  sha=$(sha256sum "${dest}" | awk '{print $1}')
  echo "${sha}  ${bin}" >> "${hash_path}"
  bin_lines+=("${bin}|${candidate}|${dest}|${sha}")
done

export FASTPQ_BIN_METADATA
FASTPQ_BIN_METADATA=$(printf '%s\n' "${bin_lines[@]}")
export FASTPQ_MANIFEST="${manifest_path}"

python3 <<'PY'
import json
import os
import pathlib
import subprocess

def run(cmd):
    try:
        return subprocess.check_output(cmd, text=True).strip()
    except (FileNotFoundError, subprocess.CalledProcessError):
        return "unavailable"

profile = os.environ.get("FASTPQ_PROFILE", "release")
features = os.environ.get("FASTPQ_FEATURES", "").strip()
manifest_path = pathlib.Path(os.environ["FASTPQ_MANIFEST"])
bin_metadata = os.environ.get("FASTPQ_BIN_METADATA", "").strip().splitlines()

bins = []
for line in bin_metadata:
    if not line:
        continue
    name, source, dest, digest = line.split("|", 3)
    bins.append(
        {
            "name": name,
            "source_path": source,
            "artifact": dest,
            "sha256": digest,
        }
    )

manifest = {
    "profile": profile,
    "features": features,
    "rustc": run(["rustc", "--version", "--verbose"]),
    "cargo": run(["cargo", "--version"]),
    "rust_image": os.environ.get("FASTPQ_RUST_IMAGE"),
    "rust_toolchain": os.environ.get("FASTPQ_RUST_TOOLCHAIN"),
    "cuda_image": os.environ.get("FASTPQ_CUDA_IMAGE"),
    "nvcc": run(["nvcc", "--version"]),
    "git_revision": run(["git", "rev-parse", "HEAD"]),
    "source_date_epoch": os.environ.get("SOURCE_DATE_EPOCH"),
    "bins": bins,
}

manifest.parent.mkdir(parents=True, exist_ok=True)
manifest.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print(f"Wrote {manifest}")
PY

echo "==> Artefacts written to ${output_dir}"
echo "    - Manifest: ${manifest_path}"
echo "    - Hashes:   ${hash_path}"
