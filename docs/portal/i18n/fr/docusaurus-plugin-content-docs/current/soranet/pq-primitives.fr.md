---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-primitives
titre : Primitives post-quantiques SoraNet
sidebar_label : Primitives PQ
description : Vue d'ensemble du crate `soranet_pq` et de la manière dont le handshake SoraNet utilise les helpers ML-KEM/ML-DSA.
---

:::note Source canonique
:::

Le crate `soranet_pq` contient les briques post-quantiques sur lesquelles reposent tous les relais, clients et composants de l'outillage SoraNet. Il encapsule les suites Kyber (ML-KEM) et Dilithium (ML-DSA) adopte PQClean et ajoute des helpers HKDF et RNG hedged adaptés au protocole afin que toutes les surfaces partagent des implémentations identiques.

## Ce qui est livre dans `soranet_pq`

- **ML-KEM-512/768/1024:** génération déterministe de clés, encapsulation et décapsulation avec propagation d'erreurs en temps constant.
- **ML-DSA-44/65/87 :** signature/vérif détaché avec transcriptions à séparation de domaine.
- **HKDF etiquete:** `derive_labeled_hkdf` applique un espace de noms à chaque dérivation via l'étape du handshake (`DH/es`, `KEM/1`, ...) afin que les transcriptions hybrides restent sans collision.
- **Aleatoire hedged:** `hedged_chacha20_rng` combine des graines déterministes avec l'entropie du système et remet à zéro l'état intermédiaire à la destruction.

Tous les secrets vivent dans des conteneurs `Zeroizing` et CI exercent les liaisons PQClean sur toutes les plateformes supportées.

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

## Commenter l'utiliser

1. **Ajoutez la dépendance** aux caisses en dehors de la racine du workspace :

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Sélectionnez la suite correcte** aux points d'appel. Pour le travail initial du handshake hybride, utilisez `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **Dérivez les clés avec labels.** Utilisez `HkdfDomain::soranet("KEM/1")` (et équivalents) pour que l'enchaînement des transcriptions reste déterministe entre les nœuds.

4. **Utilisez le RNG hedged** pour enchanter les secrets de réponse :

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Le handshake central de SoraNet et les helpers de blindage de CID (`iroha_crypto::soranet`) utilisent directement ces utilitaires, ce qui signifie que les crates en aval héritent des implémentations de mèmes sans lier les liaisons PQClean eux-mèmes.

## Checklist de validation

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Auditez les exemples d'utilisation du README (`crates/soranet_pq/README.md`)
- Mettre à jour le document de conception du handshake SoraNet lorsque les hybrides arrivent