<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Référence du codec Norito

Norito est la couche de sérialisation canonique d'Iroha. Chaque message on-wire, payload sur disque et API inter-composants utilise Norito afin que les nœuds s'accordent sur des octets identiques même lorsqu'ils tournent sur du matériel différent. Cette page résume les éléments clés et renvoie vers la spécification complète dans `norito.md`.

## Disposition de base

| Composant | Objectif | Source |
| --- | --- | --- |
| **Header** | Encapsule les payloads avec magic/version/schema hash, CRC64, longueur et tag de compression; v1 exige `VERSION_MINOR = 0x00` et valide les flags d'en-tête contre le masque supporté (par défaut `0x00`). | `norito::header` — voir `norito.md` (“Header & Flags”, racine du dépôt) |
| **Payload nu** | Encodage déterministe des valeurs utilisé pour le hashing/comparaison. Le transport on-wire utilise toujours un header; les octets nus sont internes uniquement. | `norito::codec::{Encode, Decode}` |
| **Compression** | Zstd optionnel (et accélération GPU expérimentale) sélectionné via l'octet de compression du header. | `norito.md`, “Compression negotiation” |

Le registre des flags de layout (packed-struct, packed-seq, varint offsets, compact lengths) vit dans `norito::header::flags`. La v1 utilise par défaut les flags `0x00` mais accepte des flags explicites dans le masque supporté; les bits inconnus sont rejetés. `norito::header::Flags` est conservé pour l'inspection interne et les versions futures.

## Support des derives

`norito_derive` fournit les derives `Encode`, `Decode`, `IntoSchema` et les helpers JSON. Conventions clés:

- Les derives génèrent des chemins AoS et packed; v1 utilise le layout AoS par défaut (flags `0x00`) sauf si les flags d'en-tête optent pour des variantes packed. L'implémentation se trouve dans `crates/norito_derive/src/derive_struct.rs`.
- Les fonctionnalités qui affectent le layout (`packed-struct`, `packed-seq`, `compact-len`) sont opt-in via les flags d'en-tête et doivent être encodées/décodées de manière cohérente entre pairs.
- Les helpers JSON (`norito::json`) fournissent un JSON déterministe adossé à Norito pour les API publiques. Utilisez `norito::json::{to_json_pretty, from_json}` — jamais `serde_json`.

## Multicodec et tables d'identifiants

Norito conserve ses affectations multicodec dans `norito::multicodec`. La table de référence (hashes, types de clés, descripteurs de payload) est maintenue dans `multicodec.md` à la racine du dépôt. Lorsqu'un nouvel identifiant est ajouté:

1. Mettez à jour `norito::multicodec::registry`.
2. Étendez la table dans `multicodec.md`.
3. Régénérez les bindings downstream (Python/Java) s'ils consomment la map.

## Regénérer les docs et fixtures

Avec le portail qui héberge actuellement un résumé en prose, utilisez les sources Markdown amont comme source de vérité:

- **Spec**: `norito.md`
- **Table multicodec**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Quand l'automatisation Docusaurus sera en ligne, le portail sera mis à jour via un script de sync (suivi dans `docs/portal/scripts/`) qui extrait les données depuis ces fichiers. D'ici là, gardez cette page alignée manuellement à chaque changement de spec.
