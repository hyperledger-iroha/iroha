SM3 NEON Helper
================

This crate prepares the ARMv8 NEON acceleration path for the SM3 hash used by
`iroha_crypto`. The deterministic API mirrors the SM4 helper so the crypto
module can dispatch to it under the `sm-neon` feature.

Overview
--------

- Targets `aarch64` NEON with runtime detection via `is_aarch64_feature_detected!`.
- `digest(message)` returns `Option<[u8; 32]>`, yielding `None` when NEON is not
  available so callers can fall back to the scalar RustCrypto implementation.
- When the optional `force-neon` feature is enabled (for benchmarking or CI on
  known hardware), `digest` always returns `Some` as long as the target is
  `aarch64`.

Implementation Status
---------------------

- Full SM3 compression loop is implemented with NEON intrinsics, including the
  message schedule expansion and round function parity with the scalar
  RustCrypto backend. Parity tests cover known vectors and random inputs to
  guarantee identical digests before we wire in benchmarking coverage.
- Additional optimisation passes (wider parallelism, benchmarking artefacts)
  are tracked on the roadmap; the helper remains source compatible while those
  refinements land.

Safety
------

- All NEON intrinsics will be contained in internal `unsafe` blocks once the
  acceleration is wired in.
- Callers receive a safe wrapper that either returns an accelerated digest or
  `None`, guaranteeing deterministic fallbacks across heterogeneous nodes.

Building & Testing
------------------

```
cargo check -p sm3-neon
cargo test  -p sm3-neon
cargo test  -p iroha_crypto --features "sm sm-neon sm-neon-force" \
    sm::sm_accel::tests::neon_sm3_digest_matches_scalar_under_force
```

License
-------

Apache License, Version 2.0 (see the workspace `LICENSE` file).
