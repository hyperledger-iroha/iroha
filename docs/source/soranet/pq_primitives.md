---
title: SoraNet Post-Quantum Primitives
summary: Overview of the soranet_pq crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
---

The `soranet_pq` crate packages the post-quantum cryptography building blocks
that every SoraNet relay, client, and tooling component depends on. It provides
ML-KEM and ML-DSA parameter-set helpers, plus protocol-friendly HKDF and
hedged RNG utilities.

## What ships in `soranet_pq`

- **ML-KEM-512/768/1024:** key generation, encapsulation, and decapsulation
  helpers that require explicit hedged randomness or fallible `_from_os`
  convenience helpers.
- **ML-DSA-44/65/87:** detached signing and verification helpers wired for
  domain-separated transcripts.
- **Labelled HKDF:** `derive_labeled_hkdf` namespaces every derivation with the
  handshake stage (`DH/es`, `KEM/1`, …) so hybrid transcripts stay collision-free.
- **Hedged randomness:** `hedged_chacha20_rng` blends deterministic seed inputs
  with live OS entropy when available, reports the entropy status, and zeroizes
  intermediate state on drop.

All secrets are wrapped in `Zeroizing` containers. Tests cover every suite so
CI exercises the PQClean bindings on each supported platform.

```rust
use soranet_pq::{
    decapsulate_mlkem, derive_labeled_hkdf, encapsulate_mlkem_from_os,
    generate_mlkem_keypair_from_os, HkdfDomain, HkdfSuite, MlKemSuite,
};

let kem = generate_mlkem_keypair_from_os(MlKemSuite::MlKem768).unwrap();
let (client_secret, ciphertext) = encapsulate_mlkem_from_os(MlKemSuite::MlKem768, kem.public_key()).unwrap();
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

4. **Use the hedged RNG** when deterministic seed material is already available:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(
       HedgedRngSeed::from_entropy([0u8; 32]),
       b"snnet16",
   );
   ```

The core SoraNet handshake and CID blinding helpers (`iroha_crypto::soranet`)
now consume these utilities directly, so downstream crates inherit the same
implementations without linking PQClean bindings themselves.

The current backend still contains TODO-marked pqcrypto calls where the 0.1.x
bindings do not expose seeded ML-KEM/ML-DSA derand hooks. The public API now
threads hedged RNG state through those boundaries so the local pure-Rust scalar
backend can replace those calls without changing call sites again.

## Validation checklist

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audit the README usage samples (`crates/soranet_pq/README.md`)
- Update the SoraNet handshake design doc once hybrids land.
