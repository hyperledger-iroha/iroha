---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-primitives
title: Primitives post-quantiques SoraNet
sidebar_label: Primitives PQ
description: Vue d'ensemble du crate `soranet_pq` et de la maniere dont le handshake SoraNet consomme les helpers ML-KEM/ML-DSA.
---

:::note Source canonique
:::

Le crate `soranet_pq` contient les briques post-quantiques sur lesquelles reposent tous les relays, clients et composants de tooling SoraNet. Il encapsule les suites Kyber (ML-KEM) et Dilithium (ML-DSA) adossees a PQClean et ajoute des helpers HKDF et RNG hedged adaptes au protocole afin que toutes les surfaces partagent des implementations identiques.

## Ce qui est livre dans `soranet_pq`

- **ML-KEM-512/768/1024:** generation deterministe de cles, encapsulation et decapsulation avec propagation d'erreurs en temps constant.
- **ML-DSA-44/65/87:** signature/verif detachee avec transcriptions a separation de domaine.
- **HKDF etiquete:** `derive_labeled_hkdf` applique un namespace a chaque derivation via l'etape du handshake (`DH/es`, `KEM/1`, ...) afin que les transcriptions hybrides restent sans collision.
- **Aleatoire hedged:** `hedged_chacha20_rng` combine des seeds deterministes avec l'entropie du systeme et zeroise l'etat intermediaire a la destruction.

Tous les secrets vivent dans des conteneurs `Zeroizing` et CI exerce les bindings PQClean sur toutes les plateformes supportees.

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

## Comment l'utiliser

1. **Ajoutez la dependance** aux crates en dehors de la racine du workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selectionnez la suite correcte** aux points d'appel. Pour le travail initial du handshake hybride, utilisez `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **Derivez les cles avec labels.** Utilisez `HkdfDomain::soranet("KEM/1")` (et equivalents) pour que l'enchainement des transcriptions reste deterministe entre les noeuds.

4. **Utilisez le RNG hedged** pour echantillonner les secrets de repli:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Le handshake central de SoraNet et les helpers de blindage de CID (`iroha_crypto::soranet`) utilisent directement ces utilitaires, ce qui signifie que les crates downstream heritent des memes implementations sans lier les bindings PQClean eux-memes.

## Checklist de validation

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Auditez les exemples d'usage du README (`crates/soranet_pq/README.md`)
- Mettez a jour le document de conception du handshake SoraNet lorsque les hybrides arriveront
