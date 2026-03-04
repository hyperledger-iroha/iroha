---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-primitives
titre : Primitives post-quantiques SoraNet
sidebar_label : primitives PQ
description : `soranet_pq` crate vue d'ensemble pour SoraNet handshake ML-KEM/ML-DSA helpers
---

:::note Source canonique
یہ صفحہ `docs/source/soranet/pq_primitives.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retiré نہ ہو، دونوں کاپیاں رکھیں۔
:::

Caisse `soranet_pq` pour les blocs de construction post-quantiques et le relais SoraNet, le client et le composant d'outillage pour le client. Les suites Kyber (ML-KEM) et Dilithium (ML-DSA) soutenues par PQClean et les enveloppes encapsulées et les HKDF respectueux du protocole et les assistants RNG couverts pour les surfaces et les surfaces de stockage les implémentations partagent کریں۔

## `soranet_pq` یں کیا شامل ہے

- **ML-KEM-512/768/1024 :** génération de clés déterministes, encapsulation et aides à la décapsulation et propagation d'erreurs en temps constant.
- **ML-DSA-44/65/87 :** signature/vérification détachée et transcriptions séparées par domaine ou filaires.
- **Étiqueté HKDF :** `derive_labeled_hkdf` et étape de dérivation et de prise de contact (`DH/es`, `KEM/1`, ...) pour un espace de noms et des transcriptions hybrides sans collision.
- ** Caractère aléatoire couvert : ** Graines déterministes `hedged_chacha20_rng` et entropie du système d'exploitation en direct, mélange de graines et de chute dans l'état intermédiaire et remise à zéro.

Les conteneurs secrets `Zeroizing` sont disponibles pour les plates-formes prises en charge par CI et les liaisons PQClean et les exercices.

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

1. **Dépendance en cours** pour les caisses et la racine de l'espace de travail en tant que tel :

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **درست suite منتخب کریں** sites d'appels پر۔ Poignée de main hybride pour `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65` pour `MlDsaSuite::MlDsa65`

3. **Les étiquettes et les clés dérivent des nœuds** `HkdfDomain::soranet("KEM/1")` (اور اس جیسے) استعمال کریں تاکہ nœuds de chaînage de transcription pour déterministe رہے۔

4. **Modèle RNG couvert** Un exemple de secrets de secours par exemple :

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet est une poignée de main principale et des assistants aveuglants CID (`iroha_crypto::soranet`) et des utilitaires pour les caisses en aval. Les liaisons PQClean relient les implémentations et les implémentations héritent des liens

## Liste de contrôle de validation

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Exemples d'utilisation README pour audit (`crates/soranet_pq/README.md`)
- hybrides dans le document de conception de poignée de main SoraNet ci-dessous