---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Résumé de Norito

Norito est la capacité de sérialisation binaire utilisée dans tout Iroha : définir comment codifier les structures de données dans le rouge, être persistant dans la discothèque et être intercambien entre les contrats et les hôtes. Chaque caisse de l'espace de travail dépend de Norito au lieu de `serde` pour que les pairs du matériel différent produisent des octets identiques.

Ceci reprend la synthèse des pièces centrales et s'applique aux références canoniques.

## Architecture d'une vue

- **Cabecera + payload** - Chaque message Norito commence avec une cabecera de négociation de fonctionnalités (drapeaux, somme de contrôle) suivie de la payload sans être impliquée. Les mises en page empaquetées et la compression sont négociées au milieu des bits de la tête.
- **Codification déterministe** - `norito::codec::{Encode, Decode}` implémente la base de codification. La même disposition est réutilisée pour les charges utiles impliquées dans les tâches afin que le hachage et l'entreprise soient déterminés.
- **Esquema + dérive** - `norito_derive` genres implémentations de `Encode`, `Decode` et `IntoSchema`. Les structures/séquences empaquetées sont habilitées par défaut et documentées en `norito.md`.
- **Registro multicodec** - Les identifiants de hachage, les types de clés et les descripteurs de charge utile sont présents dans `norito::multicodec`. La table autorisée est conservée sur `multicodec.md`.

## Outils| Tarée | Commande / API | Notes |
| --- | --- | --- |
| Inspeccionar cabecera/secciones | `ivm_tool inspect <file>.to` | Découvrez la version d'ABI, les drapeaux et les points d'entrée. |
| Codifier/décodifier en Rust | `norito::codec::{Encode, Decode}` | Implémenté pour tous les types de principes principaux du modèle de données. |
| Interopérabilité JSON | `norito::json::{to_json_pretty, from_json}` | JSON déterminé par les valeurs Norito. |
| Générer des documents/spécifications | `norito.md`, `multicodec.md` | Documentation source de vérité dans la racine du dépôt. |

## Flux de travail de desarrollo

1. **Agréger les dérivés** - Préférer `#[derive(Encode, Decode, IntoSchema)]` pour de nouvelles structures de données. Evita serializadores écrivait à la main ce qui était absolument nécessaire.
2. **Valider les mises en page empaquetées** - Utilisez `cargo test -p norito` (et la matrice de fonctionnalités empaquetées en `scripts/run_norito_feature_matrix.sh`) pour garantir que les nouvelles mises en page soient stables.
3. **Régénérer les documents** - Lorsque vous modifiez la codification, actualisez `norito.md` et le tableau multicodec, puis rafraîchissez les pages du portail (`/reference/norito-codec` et ce résumé).
4. **Maintener les essais Norito-first** - Les essais d'intégration doivent utiliser les assistants JSON de Norito à la place de `serde_json` pour exécuter les mêmes itinéraires de production.

## Enlaces rapides- Spécification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Attributions multicodec : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matrice de fonctionnalités : `scripts/run_norito_feature_matrix.sh`
- Exemples de mise en page empaquetado : `crates/norito/tests/`

Accompagnez ce résumé du guide de démarrage rapide (`/norito/getting-started`) pour un enregistrement pratique de compilation et d'exécution du bytecode utilisant les charges utiles Norito.