---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vue d'ensemble de Norito

Norito est la couche de sérialisation binaire utilisée dans tout Iroha : elle définit comment les structures de données sont encodées sur le fil, persistantes sur disque et échangées entre contrats et hôtes. Chaque caisse du workspace s'appuie sur Norito plutôt que sur `serde` afin que des paires sur du matériel différent produisent des octets identiques.

Cet apercu reprend les éléments clés et renvoie aux références canoniques.

## Architecture en un coup d'œil

- **En-tête + payload** - Chaque message Norito commence par une en-tête de négociation de fonctionnalités (drapeaux, checksum) suivi du payload brut. Les packs de mise en page et la compression sont négociés via les bits de l'en-tête.
- **Encodage déterministe** - `norito::codec::{Encode, Decode}` implémente l'encodage nu. Le même layout est réutilisé lors de l'enrobage des payloads dans des en-têtes afin que le hachage et la signature restent déterministes.
- **Schema + dérives** - `norito_derive` génère des implémentations `Encode`, `Decode` et `IntoSchema`. Les packs structs/sequences sont actifs par défaut et documentés dans `norito.md`.
- **Registre multicodec** - Les identifiants pour les hashes, types de clé et descripteurs de payload vivent dans `norito::multicodec`. La table de référence est utilisée dans `multicodec.md`.

##Outils| Taché | Commandes / API | Remarques |
| --- | --- | --- |
| Inspecteur en tête/sections | `ivm_tool inspect <file>.to` | Affiche la version ABI, les flags et les points d'entrée. |
| Encodeur/décodeur en Rust | `norito::codec::{Encode, Decode}` | Implémente pour tous les types principaux du modèle de données. |
| Interopérabilité JSON | `norito::json::{to_json_pretty, from_json}` | JSON déterministe adosse aux valeurs Norito. |
| Générer des documents/spécifications | `norito.md`, `multicodec.md` | Documentation source de vérité à la racine du repo. |

## Workflow de développement

1. **Ajouter les dérives** - Préférez `#[derive(Encode, Decode, IntoSchema)]` pour les nouvelles structures de données. Evitez les sérialiseurs écrits à la main sauf nécessité absolue.
2. **Valider les packs de layouts** - Utilisez `cargo test -p norito` (et la matrice de packs de fonctionnalités dans `scripts/run_norito_feature_matrix.sh`) pour garantir que les nouveaux layouts restent stables.
3. **Regenerer les docs** - Quand l'encodage change, mettez à jour `norito.md` et la table multicodec, puis rafraichissez les pages du portail (`/reference/norito-codec` et cet apercu).
4. **Garder les tests Norito-first** - Les tests d'intégration doivent utiliser les helpers JSON Norito au lieu de `serde_json` afin d'exercer les mèmes chemins que la production.

## Liens rapides- Spécification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Attributions multicodec : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matrice de fonctionnalités : `scripts/run_norito_feature_matrix.sh`
- Exemples de layout pack : `crates/norito/tests/`

Associez cet apercu au guide de démarrage rapide (`/norito/getting-started`) pour un parcours pratique de compilation et d'exécution de bytecode utilisant des payloads Norito.