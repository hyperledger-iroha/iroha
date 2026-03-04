---
lang: zh-hant
direction: ltr
source: docs/source/soranet/pq_primitives.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56007b3ef271e55b48ca6ad434a6bd9157d69512cf8ad80fcb3bdcc41ce513f1
source_last_modified: "2025-12-29T18:16:36.191370+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Post-Quantum Primitives
summary: Overview of the soranet_pq crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
---

The `soranet_pq` crate packages the post-quantum cryptography building blocks
that every SoraNet relay, client, and tooling component depends on. It provides
thin wrappers around the PQClean-backed Kyber (ML-KEM) and Dilithium (ML-DSA)
parameter sets, plus protocol-friendly HKDF and hedged RNG utilities.

## What ships in `soranet_pq`

- **ML-KEM-512/768/1024:** deterministic key generation, encapsulation, and
  decapsulation helpers with constant-time error propagation.
- **ML-DSA-44/65/87:** detached signing and verification helpers wired for
  domain-separated transcripts.
- **Labelled HKDF:** `derive_labeled_hkdf` namespaces every derivation with the
  handshake stage (`DH/es`, `KEM/1`, …) so hybrid transcripts stay collision-free.
- **Hedged randomness:** `hedged_chacha20_rng` blends deterministic seed inputs
  with live OS entropy and zeroizes intermediate state on drop.

All secrets are wrapped in `Zeroizing` containers. Tests cover every suite so
CI exercises the PQClean bindings on each supported platform.

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    derive_labeled_hkdf, HkdfDomain, HkdfSuite,
};

let kem = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (client_secret, ciphertext) = encapsulate_mlkem(MlKemSuite::MlKem768, kem.public_key()).unwrap();
let server_secret = decapsulate_mlkem(MlKemSuite::MlKem768, kem.secret_key(), ciphertext.as_bytes()).unwrap();
assert_eq!(client_secret.as_bytes(), server_secret.as_bytes());

let okm = derive_labeled_hkdf(
    HkdfSuite::Sha3_256,
    None,
    client_secret.as_bytes(),
    HkdfDomain::soranet("KEM/1"),
    b"soranet-transcript",
    32,
).unwrap();
```

## How to consume it

1. **Add the dependency** to crates that live outside the workspace root:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Select the correct suite** at call sites. For the initial hybrid handshake
   work, use `MlKemSuite::MlKem768` and `MlDsaSuite::MlDsa65`.

3. **Derive keys with labels.** Use `HkdfDomain::soranet("KEM/1")` and friends
   so transcript chaining stays deterministic across nodes.

4. **Use the hedged RNG** when sampling fallback secrets:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

The core SoraNet handshake and CID blinding helpers (`iroha_crypto::soranet`)
now consume these utilities directly, so downstream crates inherit the same
implementations without linking PQClean bindings themselves.

## Validation checklist

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audit the README usage samples (`crates/soranet_pq/README.md`)
- Update the SoraNet handshake design doc once hybrids land.
