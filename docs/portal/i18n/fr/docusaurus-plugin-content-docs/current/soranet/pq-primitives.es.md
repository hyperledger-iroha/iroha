---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-primitives
titre : Primitivas poscuanticas de SoraNet
sidebar_label : Primitivas PQ
description : Résumé de la caisse `soranet_pq` et comme la poignée de main de SoraNet consomment les assistants ML-KEM/ML-DSA.
---

:::note Fuente canonica
Cette page reflète `docs/source/soranet/pq_primitives.md`. Manten ambas versionses sincronizadas hasta que el conjunto de documentacion heredado se retire.
:::

La caisse `soranet_pq` contient les blocs possibles dans ceux qui confient tous les relais, clients et composants d'outillage de SoraNet. Enveloppez les suites Kyber (ML-KEM) et Dilithium (ML-DSA) fournies par PQClean et agrégez les aides de HKDF et RNG hedged adaptées au protocole pour que toutes les surfaces partagent des implémentations identiques.

## Que inclut `soranet_pq`

- **ML-KEM-512/768/1024 :** génération déterministe de clés, encapsulation et décapsulation avec propagation d'erreurs en temps constant.
- **ML-DSA-44/65/87 :** firmado/verificación separados para transcripciones con separación de dominio.
- **Étiquette HKDF :** `derive_labeled_hkdf` ajoute un espace de noms à chaque dérivation avec l'étape de la poignée de main (`DH/es`, `KEM/1`, ...) pour que les transcriptions hybrides ne soient pas colisées.
- **Aleatoriedad hedged :** `hedged_chacha20_rng` mezcla semillas déterministicas con entropia del SO et pone a zéro el estado intermedio al liberar recursos.

Tous les secrets se trouvent à l'intérieur des conteneurs `Zeroizing` et CI extrait les liaisons de PQClean sur toutes les plates-formes supportées.

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

1. **Agréger la dépendance** aux caisses qui sont à l'extérieur de la racine de l'espace de travail :

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Sélectionnez la suite correcte** sur les points d'appel. Pour le travail initial de la poignée de main hybride, utilisez `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **Deriva claves con etiquetas.** Usa `HkdfDomain::soranet("KEM/1")` (et équivalents) pour que l'encadenamiento de transcripciones siga siendo déterministico entre nodos.

4. **Utilisez le RNG hedged** pour découvrir les secrets de réponse :

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

La poignée de main centrale de SoraNet et les assistants aveugles du CID (`iroha_crypto::soranet`) consomment directement ces utilitaires, ce qui signifie que les caisses en aval ont les mêmes implémentations sans utiliser les liaisons PQClean pour nous.

## Checklist de validation

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Audit des exemples d'utilisation du README (`crates/soranet_pq/README.md`)
- Actualisation du document de conception de la poignée de main de SoraNet lorsque les hybrides sont utilisés