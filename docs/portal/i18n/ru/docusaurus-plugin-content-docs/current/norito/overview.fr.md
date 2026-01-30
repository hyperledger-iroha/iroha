---
lang: ru
direction: ltr
source: docs/portal/docs/norito/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Vue d'ensemble de Norito

Norito est la couche de serialisation binaire utilisee dans tout Iroha : elle definit comment les structures de donnees sont encodees sur le fil, persistees sur disque et echangees entre contrats et hotes. Chaque crate du workspace s'appuie sur Norito plutot que sur `serde` afin que des pairs sur du materiel different produisent des octets identiques.

Cet apercu resume les elements cles et renvoie aux references canoniques.

## Architecture en un coup d'oeil

- **En-tete + payload** - Chaque message Norito commence par un en-tete de negociation de features (flags, checksum) suivi du payload brut. Les layouts packes et la compression sont negocies via les bits de l'en-tete.
- **Encodage deterministe** - `norito::codec::{Encode, Decode}` implementent l'encodage nu. Le meme layout est reutilise lors de l'enrobage des payloads dans des en-tetes afin que le hachage et la signature restent deterministes.
- **Schema + derives** - `norito_derive` genere des implementations `Encode`, `Decode` et `IntoSchema`. Les structs/sequences packes sont actives par defaut et documentes dans `norito.md`.
- **Registre multicodec** - Les identifiants pour les hashes, types de cle et descripteurs de payload vivent dans `norito::multicodec`. La table de reference est maintenue dans `multicodec.md`.

## Outils

| Tache | Commande / API | Notes |
| --- | --- | --- |
| Inspecter l'en-tete/sections | `ivm_tool inspect <file>.to` | Affiche la version ABI, les flags et les entrypoints. |
| Encoder/decoder en Rust | `norito::codec::{Encode, Decode}` | Implemente pour tous les types principaux du data model. |
| Interop JSON | `norito::json::{to_json_pretty, from_json}` | JSON deterministe adosse aux valeurs Norito. |
| Generer docs/specs | `norito.md`, `multicodec.md` | Documentation source de verite a la racine du repo. |

## Workflow de developpement

1. **Ajouter les derives** - Preferez `#[derive(Encode, Decode, IntoSchema)]` pour les nouvelles structures de donnees. Evitez les serialiseurs ecrits a la main sauf necessite absolue.
2. **Valider les layouts packes** - Utilisez `cargo test -p norito` (et la matrice de features packes dans `scripts/run_norito_feature_matrix.sh`) pour garantir que les nouveaux layouts restent stables.
3. **Regenerer les docs** - Quand l'encodage change, mettez a jour `norito.md` et la table multicodec, puis rafraichissez les pages du portail (`/reference/norito-codec` et cet apercu).
4. **Garder les tests Norito-first** - Les tests d'integration doivent utiliser les helpers JSON Norito au lieu de `serde_json` afin d'exercer les memes chemins que la production.

## Liens rapides

- Specification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Attributions multicodec : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matrice de features : `scripts/run_norito_feature_matrix.sh`
- Exemples de layout packe : `crates/norito/tests/`

Associez cet apercu au guide de demarrage rapide (`/norito/getting-started`) pour un parcours pratique de compilation et d'execution de bytecode utilisant des payloads Norito.
