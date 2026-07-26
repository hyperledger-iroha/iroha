---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Référence du codec Norito

Norito est la couche de sérialisation canonique d'Iroha. Chaque message on-wire, payload sur disque et API inter-composants utilisent Norito afin que les nuds s'accordent sur des octets identiques même lorsqu'ils tournent sur du matériel différent. Cette page reprend les éléments clés et renvoie vers la spécification complète dans `norito.md`.

## Disposition de base

| Composant | Objectif | Source |
| --- | --- | --- |
| **En-tête** | Encapsule les payloads avec magic/version/schema hash, CRC64, longueur et tag de compression ; v1 exige `VERSION_MINOR = 0x00` et valide les drapeaux d'en-tête contre le masque supporte (par défaut `0x00`). | `norito::header` - voir `norito.md` ("Header & Flags", racine du dépôt) |
| **Charge utile nu** | Encodage déterministe des valeurs utilisé pour le hachage/comparaison. Le transport on-wire utilise toujours un en-tête; les octets nus sont uniquement internes. | `norito::codec::{Encode, Decode}` |
| **Compression** | Sélection optionnelle Zstd (et accélération GPU expérimentale) via l'octet de compression du header. | `norito.md`, "Négociation de compression" |Le registre des flags de layout (packed-struct, packed-seq, field bitset, compact lengths) figure dans `norito::header::flags`. La v1 utilise par défaut les flags `0x00` mais accepte les flags explicites dans le masque supporté ; les bits inconnus sont rejetés. `norito::header::Flags` est conservé pour l'inspection interne et les versions futures.

## Support des dérivés

`norito_derive` fournit les dérivés `Encode`, `Decode`, `IntoSchema` et les helpers JSON. Clés de conventions :

- Les dérives génèrent des chemins AoS et emballés ; v1 utilise le layout AoS par défaut (flags `0x00`) sauf si les flags d'en-tête optent pour des variantes packagées. L'implémentation se trouve dans `crates/norito_derive/src/derive_struct.rs`.
- Les fonctionnalités qui recherchent le layout (`packed-struct`, `packed-seq`, `compact-len`) sont opt-in via les flags d'en-tête et doivent être encodées/décodées de manière cohérente entre paires.
- Les helpers JSON (`norito::json`) fournissent un JSON déterministe adosse à Norito pour les API publiques. Utilisez `norito::json::{to_json_pretty, from_json}` - jamais `serde_json`.

## Multicodec et tables d'identifiants

Norito conserve ses affectations multicodec dans `norito::multicodec`. La table de référence (hashes, types de clés, descripteurs de payload) est effectuée dans `multicodec.md` à la racine du dépôt. Lorsqu'un nouvel identifiant est ajouté :1. Mettez à jour `norito::multicodec::registry`.
2. Etendez la table dans `multicodec.md`.
3. Régénérez les liaisons en aval (Python/Java) s'ils consomment la carte.

## Régénérer les docs et les luminaires

Avec le portail qui héberge actuellement un CV en prose, utilisez les sources Markdown comme source de vérité :

- **Spécification** : `norito.md`
- **Table multicodec** : `multicodec.md`
- **Références** : `crates/norito/benches/`
- **Tests d'or** : `crates/norito/tests/`

Quand l'automatisation Docusaurus sera en ligne, le portail sera mis à jour via un script de sync (suivi dans `docs/portal/scripts/`) qui extrait les données depuis ces fichiers. D'ici la, gardez cette page alignée manuellement à chaque changement de spécification.