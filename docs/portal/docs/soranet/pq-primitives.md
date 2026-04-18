---
id: pq-primitives
title: SoraNet Post-Quantum Primitives
sidebar_label: PQ Primitives
description: Overview of the `soranet_pq` crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
---

:::note Canonical Source
:::

The `soranet_pq` crate contains the post-quantum building blocks that every SoraNet
relay, client, and tooling component relies on. It wraps the PQClean-backed
ML-KEM and ML-DSA suites and layers on protocol-friendly HKDF and hedged RNG
helpers so all surfaces share identical implementations.

## What ships in `soranet_pq`

- **ML-KEM-512/768/1024:** key generation, encapsulation, and decapsulation
  helpers that require explicit hedged randomness or fallible `_from_os`
  convenience helpers.
- **ML-DSA-44/65/87:** detached signing/verification wired for
  domain-separated transcripts.
- **Labelled HKDF:** `derive_labeled_hkdf` namespaces every derivation with the
  handshake stage (`DH/es`, `KEM/1`, …) so hybrid transcripts stay collision-free.
- **Hedged randomness:** `hedged_chacha20_rng` blends deterministic seeds
  with live OS entropy and zeroizes intermediate state on drop.

All secrets sit inside `Zeroizing` containers and CI exercises the PQClean
bindings on every supported platform.

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

1. **Add the dependency** to crates that sit outside the workspace root:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Select the correct suite** at call sites. For the initial hybrid handshake
   work, use `MlKemSuite::MlKem768` and `MlDsaSuite::MlDsa65`.

3. **Derive keys with labels.** Use `HkdfDomain::soranet("KEM/1")` (and siblings)
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
pull these utilities directly, which means downstream crates inherit the same
implementations without linking PQClean bindings themselves.

The current backend uses the pqcrypto/PQClean FIPS implementations and calls
the PQClean derandomized hooks directly where the Rust wrappers do not expose
RNG injection. The public API already threads hedged RNG state through those
boundaries, so a local scalar or hardware-accelerated backend can replace the
PQClean calls without changing call sites again.

## Validation checklist

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Audit the README usage samples (`crates/soranet_pq/README.md`)
- Update the SoraNet handshake design doc once hybrids land
