---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-primitives
titre : Postквантовые примитивы SoraNet
sidebar_label : primes PQ
description : Pour la caisse `soranet_pq` et aussi, pour la récupération de SoraNet, nous utilisons les assistants ML-KEM/ML-DSA.
---

:::note Канонический источник
Cette page indique `docs/source/soranet/pq_primitives.md`. Si vous souhaitez créer des copies de synchronisation, vous ne pourrez pas créer de documents d'exploitation.
:::

Crate `soranet_pq` prend en charge les blocs de construction post-réseau, pour l'exploitation de tous les relais, clients et composants d'outillage SoraNet. En travaillant sur Kyber (ML-KEM) et Dilithium (ML-DSA) sur la base PQClean et en travaillant sur le protocole helpers HKDF et hedged RNG, qui sont nos plus puissants разделяли идентичные реализации.

## Ce que vous envoyez dans `soranet_pq`

- **ML-KEM-512/768/1024 :** Détermination de la clé de génération, aides à l'enregistrement et à la décapsulation pour la sauvegarde du bloc d'alimentation constant время.
- **ML-DSA-44/65/87 :** отделенная подпись/проверка, привязанная к транскриптам с разделением домена.
- **Помеченный HKDF :** `derive_labeled_hkdf` ajoute un espace de noms pour que vous puissiez visiter les stades de la région (`DH/es`, `KEM/1`, ...), чтобы гибридные транскрипты не сталкивались.
- ** Couverture de couverture :** `hedged_chacha20_rng` permet de déterminer les graines avec l'introprie de l'OS et de fournir une solution prometteuse pour ce produit. освобождении.

Tous les secrets trouvés dans le conteneur `Zeroizing`, un CI installe les liaisons PQClean sur toutes les plates-formes disponibles.

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

## Comment utiliser

1. ** Ajoutez une touche ** dans l'espace de travail de crates :

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Выберите правильный набор** в местах вызова. Pour que les robots d'établissement de liaison hybrides utilisent `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **Utilisez les clés des métaux.** Utilisez le `HkdfDomain::soranet("KEM/1")` (et le mode de fonctionnement) pour que les transcrits définissent le mode de détection. узлами.

4. **Utilisez le RNG couvert** pour utiliser les secrets :

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

L'utilisation de SoraNet et les assistants de gestion du CID (`iroha_crypto::soranet`) utilisent ces utilitaires, qui permettent aux caisses en aval de vous aider à réaliser votre réalisation. Sans aucun lien possible avec les liaisons PQClean, il est possible de le faire.

## Liste des preuves

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- Vérifiez les exemples d'utilisation dans le fichier README (`crates/soranet_pq/README.md`)
- Ouvrir le document sur la conception de SoraNet après la mise en œuvre des hybrides