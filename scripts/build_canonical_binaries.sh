#!/usr/bin/env bash
set -euo pipefail

# Build the canonical daemon, Governance DAG, external signer, and client.
#
# Prerequisites: the repository Rust toolchain and Cargo dependencies. Set
# BUILD_PROFILE to select a non-default Cargo profile (for example `deploy`).
# Keep package defaults so the daemon retains ordinary SIMD/Metal selection.
# The Linux release adds the signed, embedded IVM CUDA path automatically;
# source/bundle admission fails closed until qualified PTX is present.

usage() {
  cat <<'EOF'
Usage: build_canonical_binaries.sh [--target <triple>] [-h|--help]

Build the canonical `iroha3d` daemon, `sorafs_governance_dag`, the Unix-only
`sorafs_external_software_signer`, and `iroha`. Set BUILD_PROFILE to select a
Cargo profile. Windows software-signer packaging is explicitly unsupported.
EOF
}

declare -a build_args=(build --locked)
if [[ -n "${BUILD_PROFILE:-}" ]]; then
  build_args+=(--profile "${BUILD_PROFILE}")
fi

target=""
while (($#)); do
  case "$1" in
    --target)
      [[ $# -ge 2 && -n "$2" && "$2" != --* ]] || { echo '--target requires a triple' >&2; exit 1; }
      target="$2"
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; usage >&2; exit 1 ;;
  esac
done
if [[ -z "$target" ]]; then
  target="$(rustc -vV | awk '/^host: / { print $2 }')"
fi
[[ "$target" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]+$ ]] || { echo 'Invalid Cargo target' >&2; exit 1; }
case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-unknown-linux-musl|aarch64-unknown-linux-musl|x86_64-apple-darwin|aarch64-apple-darwin) ;;
  *-windows-*) echo 'Canonical Unix signer builder does not support Windows; use the reviewed Windows prebuilt producer' >&2; exit 1 ;;
  *) echo 'Unsupported canonical build target' >&2; exit 1 ;;
esac
build_args+=(--target "$target")

echo "Building canonical binaries (iroha, iroha3d, sorafs_governance_dag, external signer)..."
daemon_features="irohad/external-software-signer-bin,iroha_cli/cli"
# Debug development remains driver/toolkit independent without a release bundle.
# Every non-debug Linux artifact is a shipping CUDA candidate and must
# consume the already signed embedded bundle with a reviewed public fingerprint.
if [[ "${BUILD_PROFILE:-dev}" != "dev" && "${BUILD_PROFILE:-dev}" != "debug" ]]; then
  case "$target" in
    *-linux-*)
      [[ "${IVM_CUDA_TRUSTED_KEY_SHA256:-}" =~ ^[0-9a-f]{64}$ && "${IVM_CUDA_TRUSTED_KEY_SHA256:-}" != "$(printf '%064d' 0)" ]] || {
        echo 'Shipping CUDA requires reviewed IVM_CUDA_TRUSTED_KEY_SHA256' >&2; exit 1;
      }
      export IVM_CUDA_PTX_MODE=bundled
      daemon_features+=",irohad/ivm-cuda"
      ;;
    *-apple-darwin) ;;
    *) echo 'Unsupported shipping target OS' >&2; exit 1 ;;
  esac
fi
cargo "${build_args[@]}" -p irohad -p iroha_cli \
  --features "$daemon_features" \
  --bin iroha3d --bin sorafs_governance_dag \
  --bin sorafs_external_software_signer --bin iroha
