---
lang: ba
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
---

% SM OpenSSL/Tongsuo Provenance Snapshot
% Generated: 2026-01-30

# Environment Summary

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: reports `LibreSSL 3.3.6` (system-provided TLS toolkit on macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: see `sm_iroha_crypto_tree.txt` for the exact Rust dependency stack (`openssl` crate v0.10.74, `openssl-sys` v0.9.x, vendored OpenSSL 3.x sources available via `openssl-src` crate; feature `vendored` enabled in `crates/iroha_crypto/Cargo.toml` for deterministic preview builds).

# Notes

- Local development environment links against LibreSSL headers/libraries; production preview builds must use OpenSSL >= 3.0.0 or Tongsuo 8.x. Replace the system toolkit or set `OPENSSL_DIR`/`PKG_CONFIG_PATH` when generating the final artefact bundle.
- Regenerate this snapshot inside the release build environment to capture the exact OpenSSL/Tongsuo tarball hash (`openssl version -v`, `openssl version -b`, `openssl version -f`) and attach the reproducible build script/checksum. For vendored builds, record the `openssl-src` crate version/commit used by Cargo (visible in `target/debug/build/openssl-sys-*/output`).
- Apple Silicon hosts require `RUSTFLAGS=-Aunsafe-code` when running the OpenSSL smoke harness so the AArch64 SM3/SM4 acceleration stubs compile (the intrinsics are unavailable on macOS). The script `scripts/sm_openssl_smoke.sh` exports this flag before invoking `cargo` to keep CI and local runs consistent.
- Attach upstream source provenance (e.g., `openssl-src-<ver>.tar.gz` SHA256) once the packaging pipeline is pinned; use the same hash in CI artefacts.
