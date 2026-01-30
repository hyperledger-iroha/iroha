---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20bbb30b4a3af76645c36c59e49a187699c159c9ea1944bd241fbd290311243a
source_last_modified: "2026-01-18T05:31:56+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Reference du codec Norito

Norito est la couche de serialisation canonique d'Iroha. Chaque message on-wire, payload sur disque et API inter-composants utilise Norito afin que les nuds s'accordent sur des octets identiques meme lorsqu'ils tournent sur du materiel different. Cette page resume les elements cles et renvoie vers la specification complete dans `norito.md`.

## Disposition de base

| Composant | Objectif | Source |
| --- | --- | --- |
| **Header** | Encapsule les payloads avec magic/version/schema hash, CRC64, longueur et tag de compression; v1 exige `VERSION_MINOR = 0x00` et valide les flags d'en-tete contre le masque supporte (par defaut `0x00`). | `norito::header` - voir `norito.md` ("Header & Flags", racine du depot) |
| **Payload nu** | Encodage deterministe des valeurs utilise pour le hashing/comparaison. Le transport on-wire utilise toujours un header; les octets nus sont internes uniquement. | `norito::codec::{Encode, Decode}` |
| **Compression** | Zstd optionnel (et acceleration GPU experimentale) selectionne via l'octet de compression du header. | `norito.md`, "Compression negotiation" |

Le registre des flags de layout (packed-struct, packed-seq, field bitset, compact lengths) vit dans `norito::header::flags`. La v1 utilise par defaut les flags `0x00` mais accepte des flags explicites dans le masque supporte; les bits inconnus sont rejetes. `norito::header::Flags` est conserve pour l'inspection interne et les versions futures.

## Support des derives

`norito_derive` fournit les derives `Encode`, `Decode`, `IntoSchema` et les helpers JSON. Conventions cles:

- Les derives generent des chemins AoS et packed; v1 utilise le layout AoS par defaut (flags `0x00`) sauf si les flags d'en-tete optent pour des variantes packed. L'implementation se trouve dans `crates/norito_derive/src/derive_struct.rs`.
- Les fonctionnalites qui affectent le layout (`packed-struct`, `packed-seq`, `compact-len`) sont opt-in via les flags d'en-tete et doivent etre encodees/decodees de maniere coherente entre pairs.
- Les helpers JSON (`norito::json`) fournissent un JSON deterministe adosse a Norito pour les API publiques. Utilisez `norito::json::{to_json_pretty, from_json}` - jamais `serde_json`.

## Multicodec et tables d'identifiants

Norito conserve ses affectations multicodec dans `norito::multicodec`. La table de reference (hashes, types de cles, descripteurs de payload) est maintenue dans `multicodec.md` a la racine du depot. Lorsqu'un nouvel identifiant est ajoute:

1. Mettez a jour `norito::multicodec::registry`.
2. Etendez la table dans `multicodec.md`.
3. Regenerez les bindings downstream (Python/Java) s'ils consomment la map.

## Regenerer les docs et fixtures

Avec le portail qui heberge actuellement un resume en prose, utilisez les sources Markdown amont comme source de verite:

- **Spec**: `norito.md`
- **Table multicodec**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Quand l'automatisation Docusaurus sera en ligne, le portail sera mis a jour via un script de sync (suivi dans `docs/portal/scripts/`) qui extrait les donnees depuis ces fichiers. D'ici la, gardez cette page alignee manuellement a chaque changement de spec.
