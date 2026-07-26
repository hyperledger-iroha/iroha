---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Objet Norito

Norito — Version binaire de la série, utilisée avec l'Iroha : pour l'exploitation, les structures doivent être configurées En fait, il s'occupe du disque et prend en charge les contrats et les hôtes. Lorsque Crate dans l'espace de travail fonctionne sur Norito vers `serde`, vous pouvez utiliser l'option d'identification des batteries.

C'est ce qui résume les choses clés et l'étude du matériel canonique.

## Architecture dans les bureaux

- **Ajouter + charge utile** – La mise à jour Norito indique les fonctionnalités de gestion de la charge utile (drapeaux, somme de contrôle) pour la charge utile suivante. Les vêtements et les vêtements sont bien rangés à cause des petits morceaux de papier.
- **Codification de base** – `norito::codec::{Encode, Decode}` réalise la codification de base. Cette disposition permet de gérer les charges utiles dans les locaux, de les utiliser et de les détecter.
- **Схема + dérive** – `norito_derive` génère la réalisation de `Encode`, `Decode` et `IntoSchema`. Les structures et les fonctionnalités avancées sont décrites dans l'article et l'article `norito.md`.
- **Utilisation multicodec** – Les identifiants sont disponibles, les types clés et la charge utile disponible dans `norito::multicodec`. La table des autorités est disponible dans `multicodec.md`.

## Instruments| Задача | Commande / API | Première |
| --- | --- | --- |
| Vérifier l'emplacement/le service | `ivm_tool inspect <file>.to` | Trouvez la version ABI, les drapeaux et les points d'entrée. |
| Coder/décoder dans Rust | `norito::codec::{Encode, Decode}` | Réalisé pour tous les types de modèles de données. |
| Interopérabilité JSON | `norito::json::{to_json_pretty, from_json}` | La définition de JSON est la suivante: Norito. |
| Générer des documents/spécifications | `norito.md`, `multicodec.md` | Documentation de l'histoire dans le répertoire de films. |

## Processus de démarrage

1. **Dérivez les dérivés** – Sélectionnez `#[derive(Encode, Decode, IntoSchema)]` pour les nouvelles structures. N'hésitez pas à acheter des sérialiseurs, sinon ce n'est absolument pas nécessaire.
2. **Provérifier les mises en page supplémentaires** – Utilisez `cargo test -p norito` (et les fonctionnalités emballées dans `scripts/run_norito_feature_matrix.sh`), pour savoir quelles nouvelles mises en page sont disponibles стабильными.
3. **Générer des documents** – Pour créer des menus, ouvrez `norito.md` et la table multicodec, puis ouvrez les pages du port (`/reference/norito-codec`). et c'est ce qui se passe).
4. **Tests Norito-first** – Les tests d'intégration peuvent utiliser l'aide JSON Norito avec `serde_json`, чтобы проходить те же пути, что и продакшн.

## Быстрые ссылки

- Spécification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Nom du multicodec : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Caractéristiques des matrices de script : `scripts/run_norito_feature_matrix.sh`
- Exemples de mises en page emballées : `crates/norito/tests/`Сочетайте этот обзор руководством быстрого старта (`/norito/getting-started`) pour la préparation pratique de la compilation et de la mise en service du code de batterie, использующего charges utiles Norito.