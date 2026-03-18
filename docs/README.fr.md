---
lang: fr
direction: ltr
source: docs/README.md
status: complete
translator: manual
source_hash: 6d050b266fbcc3f0041ec554f89e397e56dc37e5ec3fb093af59e46eb52109e6
source_last_modified: "2025-11-02T04:40:28.809865+00:00"
translation_last_reviewed: 2025-11-14
---

# Documentation d’Iroha

Pour une vue d’ensemble en japonais, consultez [`README.ja.md`](./README.ja.md).

Ce workspace publie deux lignes de version à partir du même code : **Iroha 2**
(déploiements auto‑hébergés) et **Iroha 3 / SORA Nexus** (le grand livre Nexus
global unique). Les deux réutilisent la même Iroha Virtual Machine (IVM) et la
même toolchain Kotodama, de sorte que les contrats et le bytecode restent
portables entre les déploiements. Sauf mention contraire, la documentation
s’applique aux deux lignes.

Dans la [documentation principale d’Iroha](https://docs.iroha.tech/), vous trouverez :

- [Guide de démarrage](https://docs.iroha.tech/get-started/)
- [Tutoriels SDK](https://docs.iroha.tech/guide/tutorials/) pour Rust, Python, JavaScript et Java/Kotlin
- [Référence d’API](https://docs.iroha.tech/reference/torii-endpoints.html)

Livres blancs et spécifications par version :

- [Livre blanc Iroha 2](./source/iroha_2_whitepaper.md) — spécification pour les réseaux auto‑hébergés.
- [Livre blanc Iroha 3 (SORA Nexus)](./source/iroha_3_whitepaper.md) — conception multi‑voies (lanes) et espaces de données Nexus.
- [Modèle de données et spécification ISI (dérivés de l’implémentation)](./source/data_model_and_isi_spec.md) — référence de comportement reconstruite à partir du code.
- [Enveloppes ZK (Norito)](./source/zk_envelopes.md) — enveloppes Norito natives (IPA/STARK) et attentes du vérificateur.

## Localisation

Les stubs de documentation en japonais (`*.ja.*`), hébreu (`*.he.*`), espagnol
(`*.es.*`), portugais (`*.pt.*`), français (`*.fr.*`), russe (`*.ru.*`), arabe
(`*.ar.*`) et ourdou (`*.ur.*`) se trouvent à côté de chaque fichier source
anglais. Voir [`docs/i18n/README.md`](./i18n/README.md) pour les détails sur la
génération et la maintenance des traductions, ainsi que pour l’ajout de nouvelles
langues.

## Outils

Ce dépôt contient la documentation des outils Iroha 2 :

- [Kagami](../crates/iroha_kagami/README.md)
- Macros [`iroha_derive`](../crates/iroha_derive/) pour les structures de configuration (cf. l’option `config_base`)
- [Étapes de build avec profilage](./profile_build.md) pour identifier les points lents de compilation dans `iroha_data_model`

## Références SDK Swift / iOS

- [Vue d’ensemble du SDK Swift](./source/sdk/swift/index.md) : aides de pipeline, bascules d’accélération et APIs Connect/WebSocket.
- [Guide de démarrage Connect](./connect_swift_ios.md) : parcours guidé centré SDK, plus la référence CryptoKit héritée.
- [Guide d’intégration Xcode](./connect_swift_integration.md) : intégration de NoritoBridgeKit/Connect dans une application, avec ChaChaPoly et aides de trame.
- [Guide du contributeur pour la démo SwiftUI](./norito_demo_contributor.md) : exécuter la démo iOS contre un nœud Torii local, avec notes sur l’accélération.
- Exécutez `make swift-ci` avant de publier des artefacts Swift ou des changements de Connect ; cela vérifie la parité des fixtures, les flux de tableaux de bord et les métadonnées `ci/xcframework-smoke:<lane>:device_tag` dans Buildkite.

## Norito (codec de sérialisation)

Norito est le codec de sérialisation du workspace. Nous n’utilisons pas
`parity-scale-codec` (SCALE). Lorsque la documentation ou les benchmarks
comparent à SCALE, c’est uniquement à titre de contexte ; tous les chemins de
production utilisent Norito. Les APIs `norito::codec::{Encode, Decode}`
fournissent une charge utile Norito sans en‑tête (« bare ») pour le hachage et
l’efficacité sur le réseau : c’est toujours Norito, pas SCALE.

État actuel :

- Encodage/décodage déterministe avec un en‑tête fixe (magic, version, schéma
  16 octets, compression, longueur, CRC64, indicateurs).
- Checksum CRC64-XZ avec accélération choisie à l’exécution :
  - PCLMULQDQ sur x86_64 (multiplication sans retenue) + réduction de Barrett sur des blocs de 32 octets.
  - PMULL sur aarch64 avec le même schéma de repli.
  - Variantes slicing‑by‑8 et bit‑à‑bit pour la portabilité.
- Indications de longueur encodée fournies par les derives et les types noyaux pour réduire les allocations.
- Tampons de streaming plus grands (64 KiB) et mise à jour incrémentale du CRC lors du décodage.
- Compression zstd optionnelle ; l’accélération GPU est contrôlée par des features et reste déterministe.
- Sélection de chemin adaptative : `norito::to_bytes_auto(&T)` choisit entre
  sans compression, zstd CPU ou zstd délégué au GPU (lorsqu’il est compilé et
  disponible) en fonction de la taille de la charge utile et des capacités
  matérielles connues. Ce choix n’affecte que les performances et l’octet
  `compression` de l’en‑tête ; la sémantique de la charge utile reste identique.

Voir `crates/norito/README.md` pour les tests de parité, benchmarks et exemples d’utilisation.

Remarque : certains documents de sous‑systèmes (par exemple l’accélération IVM
et les circuits ZK) évoluent encore. Lorsqu’une fonctionnalité n’est pas
complète, les fichiers indiquent explicitement le travail restant et la
direction prévue.

Notes sur l’encodage du point de terminaison `/status`

- Le corps de `/status` dans Torii utilise Norito par défaut avec une charge
  utile sans en‑tête (« bare ») pour plus de compacité. Les clients doivent
  d’abord tenter un décodage Norito.
- Les serveurs peuvent retourner du JSON sur demande ; les clients se replient
  sur JSON si le `content-type` est `application/json`.
- Le format sur le fil est Norito, pas SCALE. Les APIs `norito::codec::{Encode,Decode}` sont utilisées pour la variante sans en‑tête.
