#!/usr/bin/env bash

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1

if ! command -v cargo >/dev/null 2>&1; then
  echo "ERROR: cargo not found in PATH; install Rust toolchain before running the SM OpenSSL smoke check." >&2
  exit 1
fi

if ! command -v pkg-config >/dev/null 2>&1; then
  echo "SKIP: pkg-config not found; install pkg-config and OpenSSL >= 3.0.0 development headers to run the SM OpenSSL smoke check." >&2
  exit 2
fi

if ! pkg-config --exists 'openssl >= 3.0.0'; then
  echo "SKIP: OpenSSL >= 3.0.0 development files not detected via pkg-config; skipping SM OpenSSL smoke check." >&2
  exit 2
fi

CRATE_MANIFEST="crates/iroha_crypto/Cargo.toml"

echo "+ cargo check --locked --manifest-path ${CRATE_MANIFEST} --features \"sm sm-ffi-openssl\" $*"
cargo check --locked --manifest-path "${CRATE_MANIFEST}" --features "sm sm-ffi-openssl" "$@"

echo "+ cargo test --locked --manifest-path ${CRATE_MANIFEST} --features \"sm sm-ffi-openssl\" --test iroha_crypto_group_01 sm_openssl_smoke:: $* -- --nocapture"
cargo test --locked --manifest-path "${CRATE_MANIFEST}" --features "sm sm-ffi-openssl" \
  --test iroha_crypto_group_01 "$@" sm_openssl_smoke:: -- --nocapture
