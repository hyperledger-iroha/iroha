---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-primitives
titre : Primitifs pos-quantiques de SoraNet
sidebar_label : Primitivos PQ
description : Visa général de la caisse `soranet_pq` et de la poignée de main des assistants SoraNet ML-KEM/ML-DSA.
---

:::note Fonte canonica
Cette page espelha `docs/source/soranet/pq_primitives.md`. Mantenha ambas comme copies synchronisées.
:::

La caisse `soranet_pq` contient les blocs pos-quantiques de tout ce qui concerne le relais, le client et les composants d'outillage de la confiance SoraNet. Il s'agit des suites Kyber (ML-KEM) et Dilithium (ML-DSA) prises en charge par PQClean et des assistants supplémentaires de HKDF et RNG hedged amigaves au protocole pour que toutes les surfaces partagent les implémentations identiques.

## O que je viens d'em `soranet_pq`

- **ML-KEM-512/768/1024 :** agit de manière déterministe sur les chaves, aide à l'encapsulation et à la décapsulation avec la propagation des erreurs à temps constant.
- **ML-DSA-44/65/87 :** assinatura/verificacao separadas para transcricoes com separacao de dominio.
- **HKDF avec rotulo :** `derive_labeled_hkdf` ajoute un espace de noms à chaque dérivé de l'état de poignée de main (`DH/es`, `KEM/1`, ...) pour que les hybrides soient transcrits sans colis.
- **Aleatoriedade hedged :** `hedged_chacha20_rng` graines déterminées par entropie de SO et zéro de l'état intermédiaire à télécharger.

Tous les éléments séparés sont placés dans les conteneurs `Zeroizing` et CI exerce les liaisons PQClean sur toutes les plates-formes prises en charge.

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

## Côme consommer

1. **Ajouter une dépendance** aux forums de caisses pour l'espace de travail :

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selecione a suite correta** nos pontos de chamada. Pour le travail initial de poignée de main hybride, utilisez `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **Dériver les chaînes avec les rotules.** Utilisez `HkdfDomain::soranet("KEM/1")` (et similaires) pour que l'enchaînement des transcriptions soit déterministe entre les nœuds.

4. **Utilisez le RNG couvert** pour surveiller les solutions de secours :

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

La poignée de main centrale de SoraNet et les assistants de blindage du CID (`iroha_crypto::soranet`) peuvent être utilisés directement, ce qui signifie que les caisses en aval sont mises en œuvre par des liaisons de liaison PQClean avec leur propriétaire.

## Checklist de validation

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Auditez les exemples d'utilisation dans le fichier README (`crates/soranet_pq/README.md`)
- Actualiser le document de conception pour la poignée de main de SoraNet lorsque les hybrides choisissent