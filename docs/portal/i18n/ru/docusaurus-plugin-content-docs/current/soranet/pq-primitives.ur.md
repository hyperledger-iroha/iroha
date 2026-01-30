---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-primitives
title: SoraNet Post-Quantum Primitives
sidebar_label: PQ Primitives
description: `soranet_pq` crate کا overview اور یہ کہ SoraNet handshake ML-KEM/ML-DSA helpers کو کیسے استعمال کرتا ہے۔
---

:::note Canonical Source
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں کاپیاں sync رکھیں۔
:::

`soranet_pq` crate ان post-quantum building blocks پر مشتمل ہے جن پر ہر SoraNet relay، client اور tooling component انحصار کرتا ہے۔ یہ PQClean-backed Kyber (ML-KEM) اور Dilithium (ML-DSA) suites کو wrap کرتا ہے اور protocol-friendly HKDF اور hedged RNG helpers فراہم کرتا ہے تاکہ تمام surfaces ایک جیسی implementations share کریں۔

## `soranet_pq` میں کیا شامل ہے

- **ML-KEM-512/768/1024:** deterministic key generation، encapsulation اور decapsulation helpers کے ساتھ constant-time error propagation.
- **ML-DSA-44/65/87:** detached signing/verification جو domain-separated transcripts کے لئے wired ہے.
- **Labelled HKDF:** `derive_labeled_hkdf` ہر derivation کو handshake stage (`DH/es`, `KEM/1`, ...) کے ساتھ namespace دیتا ہے تاکہ hybrid transcripts collision-free رہیں.
- **Hedged randomness:** `hedged_chacha20_rng` deterministic seeds کو live OS entropy کے ساتھ blend کرتا ہے اور drop پر intermediate state کو zeroize کرتا ہے.

تمام secrets `Zeroizing` containers کے اندر رہتے ہیں اور CI تمام supported platforms پر PQClean bindings کو exercise کرتی ہے۔

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

## استعمال کیسے کریں

1. **Dependency شامل کریں** ان crates میں جو workspace root سے باہر ہوں:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **درست suite منتخب کریں** call sites پر۔ ابتدائی hybrid handshake کے لئے `MlKemSuite::MlKem768` اور `MlDsaSuite::MlDsa65` استعمال کریں۔

3. **Labels کے ساتھ keys derive کریں۔** `HkdfDomain::soranet("KEM/1")` (اور اس جیسے) استعمال کریں تاکہ transcript chaining nodes کے درمیان deterministic رہے۔

4. **Hedged RNG استعمال کریں** جب fallback secrets sample کر رہے ہوں:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet کا core handshake اور CID blinding helpers (`iroha_crypto::soranet`) ان utilities کو براہ راست استعمال کرتے ہیں، جس کا مطلب ہے کہ downstream crates بغیر PQClean bindings link کئے وہی implementations inherit کرتے ہیں۔

## Validation checklist

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README usage samples کا audit کریں (`crates/soranet_pq/README.md`)
- hybrids آنے کے بعد SoraNet handshake design doc اپ ڈیٹ کریں
