<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2025-11-02T04:40:39.595018+00:00"
translation_last_reviewed: 2025-12-30
---

# Vue d'ensemble de Norito

Norito est la couche de sérialisation binaire utilisée dans tout Iroha : elle définit comment les structures de données sont encodées sur le fil, persistées sur disque et échangées entre contrats et hôtes. Chaque crate du workspace s'appuie sur Norito plutôt que sur `serde` afin que des pairs sur du matériel différent produisent des octets identiques.

Cet aperçu résume les éléments clés et renvoie aux références canoniques.

## Architecture en un coup d'oeil

- **En-tête + payload** – Chaque message Norito commence par un en-tête de négociation de features (flags, checksum) suivi du payload brut. Les layouts packés et la compression sont négociés via les bits de l'en-tête.
- **Encodage déterministe** – `norito::codec::{Encode, Decode}` implémentent l'encodage nu. Le même layout est réutilisé lors de l'enrobage des payloads dans des en-têtes afin que le hachage et la signature restent déterministes.
- **Schéma + derives** – `norito_derive` génère des implémentations `Encode`, `Decode` et `IntoSchema`. Les structs/séquences packés sont activés par défaut et documentés dans `norito.md`.
- **Registre multicodec** – Les identifiants pour les hashes, types de clé et descripteurs de payload vivent dans `norito::multicodec`. La table de référence est maintenue dans `multicodec.md`.

## Outils

| Tâche | Commande / API | Notes |
| --- | --- | --- |
| Inspecter l'en-tête/sections | `ivm_tool inspect <file>.to` | Affiche la version ABI, les flags et les entrypoints. |
| Encoder/décoder en Rust | `norito::codec::{Encode, Decode}` | Implémenté pour tous les types principaux du data model. |
| Interop JSON | `norito::json::{to_json_pretty, from_json}` | JSON déterministe adossé aux valeurs Norito. |
| Générer docs/specs | `norito.md`, `multicodec.md` | Documentation source de vérité à la racine du repo. |

## Workflow de développement

1. **Ajouter les derives** – Préférez `#[derive(Encode, Decode, IntoSchema)]` pour les nouvelles structures de données. Évitez les sérialiseurs écrits à la main sauf nécessité absolue.
2. **Valider les layouts packés** – Utilisez `cargo test -p norito` (et la matrice de features packés dans `scripts/run_norito_feature_matrix.sh`) pour garantir que les nouveaux layouts restent stables.
3. **Régénérer les docs** – Quand l'encodage change, mettez à jour `norito.md` et la table multicodec, puis rafraîchissez les pages du portail (`/reference/norito-codec` et cet aperçu).
4. **Garder les tests Norito-first** – Les tests d'intégration doivent utiliser les helpers JSON Norito au lieu de `serde_json` afin d'exercer les mêmes chemins que la production.

## Liens rapides

- Spécification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Attributions multicodec : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matrice de features : `scripts/run_norito_feature_matrix.sh`
- Exemples de layout packé : `crates/norito/tests/`

Associez cet aperçu au guide de démarrage rapide (`/norito/getting-started`) pour un parcours pratique de compilation et d'exécution de bytecode utilisant des payloads Norito.
