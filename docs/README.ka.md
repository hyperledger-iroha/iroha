---
lang: ka
direction: ltr
source: docs/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26e6f90205e98b5db87d442eb7e4e7691cce47e1c33ef3d11c9bfba25269294e
source_last_modified: "2026-01-14T17:53:24.552406+00:00"
translation_last_reviewed: 2026-02-07
---

# Iroha Documentation

日本語版の概要は [`README.ja.md`](./README.ja.md) を参照してください。

The workspace ships two release lines from the same codebase: **Iroha 2** (self-hosted deployments) and
**Iroha 3 / SORA Nexus** (the single global Nexus ledger). Both reuse the same Iroha Virtual Machine (IVM) and
Kotodama toolchain, so contracts and bytecode remain portable between deployment targets. Documentation applies
to both unless otherwise noted.

In the [main Iroha documentation](https://docs.iroha.tech/) you will find:

- [Get Started Guide](https://docs.iroha.tech/get-started/)
- [SDK Tutorials](https://docs.iroha.tech/guide/tutorials/) for Rust, Python, Javascript, and Java/Kotlin
- [API Reference](https://docs.iroha.tech/reference/torii-endpoints.html)

Release-specific whitepapers and specs:

- [Iroha 2 Whitepaper](./source/iroha_2_whitepaper.md) — self-hosted network specification.
- [Iroha 3 (SORA Nexus) Whitepaper](./source/iroha_3_whitepaper.md) — Nexus multi-lane and data-space design.
- [Data Model & ISI Spec (implementation‑derived)](./source/data_model_and_isi_spec.md) — reverse-engineered behavior reference.
- [ZK Envelopes (Norito)](./source/zk_envelopes.md) — native IPA/STARK Norito envelopes and verifier expectations.

## Localization

Japanese (`*.ja.*`), Hebrew (`*.he.*`), Spanish (`*.es.*`), Portuguese
(`*.pt.*`), French (`*.fr.*`), Russian (`*.ru.*`), Arabic (`*.ar.*`), and Urdu
(`*.ur.*`) documentation stubs live next to each English source file. See
[`docs/i18n/README.md`](./i18n/README.md) for details on generating and
maintaining translations, as well as guidance for adding new languages in the
future.

## Tools

In this repository you can find documentation for Iroha 2 tools:

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) macros for configuration structs (see the `config_base` feature)
- [Profiling build steps](./profile_build.md) for identifying slow `iroha_data_model` compilation tasks

## Swift / iOS SDK References

- [Swift SDK overview](./source/sdk/swift/index.md) — pipeline helpers, acceleration toggles, and Connect/WebSocket APIs.
- [Connect quickstart](./connect_swift_ios.md) — SDK-first walkthrough plus the CryptoKit reference.
- [Xcode integration guide](./connect_swift_integration.md) — wiring NoritoBridgeKit/Connect into an app, with ChaChaPoly and frame helpers.
- [SwiftUI demo contributor guide](./norito_demo_contributor.md) — running the iOS demo against a local Torii node, plus acceleration notes.
- Run `make swift-ci` before publishing Swift artifacts or Connect changes; it verifies fixture parity, dashboard feeds, and Buildkite `ci/xcframework-smoke:<lane>:device_tag` metadata.

## Norito (Serialization Codec)

Norito is the workspace serialization codec. We do not use `parity-scale-codec`
(SCALE). Where documentation or benchmarks compare to SCALE, it is only for
context; all production paths use Norito. The `norito::codec::{Encode, Decode}`
APIs provide a headerless ("bare") Norito payload for hashing and wire
efficiency — it is Norito, not SCALE.

Latest state:

- Deterministic encoding/decoding with a fixed header (magic, version, 16‑byte schema, compression, length, CRC64, flags).
- CRC64-XZ checksum with runtime‑selected acceleration:
  - x86_64 PCLMULQDQ (carry‑less multiply) + Barrett reduction, folded over 32‑byte chunks.
  - aarch64 PMULL with matching folding.
  - Slicing‑by‑8 and bitwise fallbacks for portability.
- Encoded length hints implemented by derives and core types to reduce allocations.
- Larger streaming buffers (64 KiB) and incremental CRC update during decode.
- Optional zstd compression; GPU acceleration is feature‑gated and deterministic.
- Adaptive path selection: `norito::to_bytes_auto(&T)` chooses among no
  compression, CPU zstd, or GPU‑offloaded zstd (when compiled and available)
  based on payload size and cached hardware capabilities. Selection only affects
  performance and the header's `compression` byte; payload semantics are unchanged.

See `crates/norito/README.md` for parity tests, benchmarks, and usage examples.

Note: Some subsystem docs (e.g., IVM acceleration and ZK circuits) are evolving. Where functionality is incomplete, the files call out the work that remains and the direction of travel.

Status endpoint encoding notes
- Torii `/status` body uses Norito by default with a headerless ("bare") payload for compactness. Clients should attempt Norito decode first.
- Servers may return JSON when requested; clients fall back to JSON if the `content-type` is `application/json`.
- The wire format is Norito, not SCALE. The `norito::codec::{Encode,Decode}` APIs are used for the bare variant.
