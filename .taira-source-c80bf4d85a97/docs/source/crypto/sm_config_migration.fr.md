---
lang: fr
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T15:38:30.660147+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Migration de configuration SM

# Migration de configuration SM

Le déploiement de l'ensemble des fonctionnalités SM2/SM3/SM4 nécessite plus que la compilation avec le
Indicateur de fonctionnalité `sm`. Les nœuds contrôlent les fonctionnalités derrière les couches
Profils `iroha_config` et attendez-vous à ce que le manifeste Genesis contienne la correspondance
valeurs par défaut. Cette note capture le flux de travail recommandé lors de la promotion d'un
réseau existant de « Ed25519 uniquement » à « compatible SM ».

## 1. Vérifiez le profil de build

- Compiler les binaires avec `--features sm` ; ajoutez `sm-ffi-openssl` uniquement lorsque vous
  prévoyez d'exercer le chemin de prévisualisation OpenSSL/Tongsuo. Construit sans le `sm`
  la fonctionnalité rejette les signatures `sm2` lors de l'admission même si la configuration l'active
  eux.
- Confirmer que CI publie les artefacts `sm` et que toutes les étapes de validation (`cargo
  test -p iroha_crypto --features sm`, dispositifs d'intégration, suites fuzz) pass
  sur les binaires exacts que vous avez l'intention de déployer.

## 2. Remplacements de configuration de couche

`iroha_config` applique trois niveaux : `defaults` → `user` → `actual`. Expédier le SM
remplacements dans le profil `actual` que les opérateurs distribuent aux validateurs et
laissez `user` sur Ed25519 uniquement afin que les valeurs par défaut du développeur restent inchangées.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

Copiez le même bloc dans le manifeste `defaults/genesis` via `kagami genesis
générer…` (add `--allowed-signing sm2 --default-hash sm3-256` si vous en avez besoin
remplace) afin que le bloc `parameters` et les métadonnées injectées soient en accord avec le
configuration d'exécution. Les pairs refusent de démarrer lorsque le manifeste et la configuration
les instantanés divergent.

## 3. Régénérer les manifestes Genesis

- Exécutez `kagami genesis generate --consensus-mode <mode>` pour chaque
  environnement et validez le JSON mis à jour avec les remplacements TOML.
- Signez le manifeste (`kagami genesis sign …`) et distribuez la charge utile `.nrt`.
  Les nœuds qui démarrent à partir d'un manifeste JSON non signé dérivent le crypto d'exécution
  configuration directement à partir du fichier, toujours soumise à la même cohérence
  chèques.

## 4. Valider avant le trafic

- Provisionnez un cluster de préparation avec les nouveaux binaires et la nouvelle configuration, puis vérifiez :
  - `/status` expose `crypto.sm_helpers_available = true` une fois les homologues redémarrés.
  - L'admission Torii rejette toujours les signatures SM2 alors que `sm2` est absent de
    `allowed_signing` et accepte les lots mixtes Ed25519/SM2 lorsque la liste
    inclut les deux algorithmes.
  - Matériel clé aller-retour `iroha_cli tools crypto sm2 export …` amorcé via le nouveau
    valeurs par défaut.
- Exécuter les scripts smoke d'intégration qui couvrent les signatures déterministes SM2 et
  Hachage SM3 pour confirmer la cohérence hôte/VM.

## 5. Plan de restauration- Documenter l'inversion : supprimer `sm2` de `allowed_signing` et restaurer
  `default_hash = "blake2b-256"`. Poussez le changement via le même `actual`
  pipeline de profil afin que chaque validateur se retourne de manière monotone.
- Conserver les manifestes SM sur disque ; pairs qui voient une configuration et une genèse incompatibles
  les données refusent de démarrer, ce qui protège contre les restaurations partielles.
- Si l'aperçu OpenSSL/Tongsuo est impliqué, incluez les étapes de désactivation
  `crypto.enable_sm_openssl_preview` et suppression des objets partagés du
  environnement d'exécution.

## Matériel de référence

- [`docs/genesis.md`](../../genesis.md) – structure du manifeste de genèse et
  le bloc `crypto`.
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  aperçu des sections `iroha_config` et des valeurs par défaut.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – fin à
  liste de contrôle de l'opérateur final pour l'expédition de la cryptographie SM.