# RustCrypto: Advanced Encryption Standard (AES)

## Iroha patch note

This directory contains crates.io `aes` 0.9.3, RustCrypto/block-ciphers commit
`c1534361e7549e29a16c3505f45a04c92d26b62a`, path `aes`. The original registry
archive SHA-256 is
`35f0f96ce78e38c3dc6d8948aa8163d06385be74000f3c7a95bf1eef35d3ea32`.
All files under `src/backends/`, including fixslicing, key expansion and CPU
intrinsics, are byte-identical to that release.

Repository changes:

- `hardware` is a positive, default Cargo feature. Without it only the portable
  backend is compiled, and top-level cipher names alias the portable types.
  The upstream `aes_backend="soft"` configuration also selects this behavior.
- Upstream's public autodetection wrappers moved unchanged into
  `src/autodetect.rs`, with their imports adjusted for the module boundary.
  `src/backends.rs` and hazmat dispatch require `hardware` before compiling or
  selecting CPU intrinsics. Runtime hardware detection and fallback are retained.
- `src/soft.rs` exposes `aes::soft::{Aes128,Aes192,Aes256}` and their `Enc`/`Dec`
  variants using thin cipher 0.5 trait wrappers over the upstream soft backend.
  They remain portable under Cargo feature unification. The `zeroize` feature
  erases their complete key schedules on drop using upstream's flat-type erasure.
- `tests/portable.rs` retains all seven FIPS-197, random-block parity and feature
  selection tests, with current cipher traits and arrays. It also exercises
  portable encryptor conversions and checks the zeroize-on-drop contracts.
- Upstream's normalized manifest is retained with these features and the extra
  test. The nightly benchmark target is excluded from stable all-target checks.
  Registry-only metadata and the upstream standalone lockfile are omitted.

Consumers on cipher 0.5 use `BlockCipherEncrypt`, `BlockCipherDecrypt` and
`Array`. A consumer that must remain portable explicitly names `aes::soft::*`.
The patch applies to the 0.9.3 resolution; older transitive AES versions do not
inherit this package's feature policy.


[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]
[![Build Status][build-image]][build-link]
[![Downloads][downloads-image]][crate-link]
[![HAZMAT][hazmat-image]][hazmat-link]

Pure Rust implementation of the [Advanced Encryption Standard (AES)][1].

This crate implements the low-level AES block function, and is intended
for use for implementing higher-level constructions *only*. It is NOT
intended for direct use in applications.

<img src="https://raw.githubusercontent.com/RustCrypto/media/85f62bb/img/block-ciphers/aes-round.svg" width="480px">

## Security

### ⚠️ Warning: [Hazmat!][hazmat-link]

This crate does not ensure ciphertexts are authentic (i.e. by using a MAC to
verify ciphertext integrity), which can lead to serious vulnerabilities
if used incorrectly!

To avoid this, use an [AEAD][2] mode based on AES, such as [AES-GCM][3] or [AES-GCM-SIV][4].
See the [RustCrypto/AEADs][5] repository for more information.

USE AT YOUR OWN RISK!

### Notes

This crate has received one [security audit by NCC Group][6], with no significant
findings. We would like to thank [MobileCoin][7] for funding the audit.

All implementations contained in the crate are designed to execute in constant
time, either by relying on hardware intrinsics (i.e. AES-NI on x86/x86_64), or
using a portable implementation based on bitslicing.

## License

Licensed under either of:

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[crate-image]: https://img.shields.io/crates/v/aes.svg
[crate-link]: https://crates.io/crates/aes
[docs-image]: https://docs.rs/aes/badge.svg
[docs-link]: https://docs.rs/aes
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.89+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260039-block-ciphers
[build-image]: https://github.com/RustCrypto/block-ciphers/actions/workflows/aes.yml/badge.svg?branch=master
[build-link]: https://github.com/RustCrypto/block-ciphers/actions/workflows/aes.yml
[downloads-image]: https://img.shields.io/crates/d/aes.svg
[hazmat-image]: https://img.shields.io/badge/crypto-hazmat%E2%9A%A0-red.svg
[hazmat-link]: https://github.com/RustCrypto/meta/blob/master/HAZMAT.md

[//]: # (general links)

[1]: https://en.wikipedia.org/wiki/Advanced_Encryption_Standard
[2]: https://en.wikipedia.org/wiki/Authenticated_encryption
[3]: https://github.com/RustCrypto/AEADs/tree/master/aes-gcm
[4]: https://github.com/RustCrypto/AEADs/tree/master/aes-gcm-siv
[5]: https://github.com/RustCrypto/AEADs
[6]: https://web.archive.org/web/20240108154854/https://research.nccgroup.com/wp-content/uploads/2020/02/NCC_Group_MobileCoin_RustCrypto_AESGCM_ChaCha20Poly1305_Implementation_Review_2020-02-12_v1.0.pdf
[7]: https://www.mobilecoin.com/
