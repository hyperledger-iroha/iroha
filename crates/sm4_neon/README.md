SM4 NEON Helper
================

This crate hosts the ARMv8 NEON acceleration glue for the SM4 block cipher that
`iroha_crypto` dispatches to behind the `sm-neon` feature. It is currently a
workspace‑only helper and is not published to crates.io.

Overview
--------

- Targets `aarch64` NEON and exposes a tiny safe API:
  - `is_supported()` detects NEON at runtime via `is_aarch64_feature_detected!`
    (or always returns `true` when the optional `force-neon` feature is enabled
    for benchmarking/CI use on known hardware).
  - `encrypt_block` / `decrypt_block` wrap the accelerated core and return
    `Option<[u8; 16]>`, yielding `None` when NEON is unavailable so callers can
    fall back to the scalar RustCrypto implementation.
- Key schedule and round functions are implemented with NEON table lookups that
  mirror the SM4 S‑Box and rotation flow from GM/T 0002‑2012.
- A scalar parity test runs when the crate is compiled for `aarch64` to ensure
  deterministic behaviour against `sm4`’s pure Rust implementation.

Safety
------

- All NEON intrinsics live in `unsafe` blocks. The public API only uses `unsafe`
  internally and guarantees:
  - Inputs are borrowed slices, so callers never construct invalid NEON vectors.
  - The accelerated functions are only invoked when `is_supported()` is true,
    preventing illegal instruction traps.
- Any future changes to the NEON implementation must retain the scalar fallback
  for determinism across heterogeneous consensus nodes. When the `force-neon`
  feature is used, callers must ensure they run on NEON-capable hardware.

Building & Testing
------------------

```
cargo check -p sm4-neon
cargo test  -p iroha_crypto --features "sm sm-neon" \
    sm::sm_accel::tests::neon_force_disable_disables_accel
# Optional: exercise the forced feature flag that bypasses env gating
cargo test  -p iroha_crypto --features "sm sm-neon sm-neon-force" \
    sm::sm_accel::tests::neon_force_feature_enables_accel
```

The dedicated crate tests only run on `aarch64`; the cross‑crate test verifies
that the runtime disable guard falls back to the scalar path.

License
-------

Apache License, Version 2.0 (see the workspace `LICENSE` file).
