---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Référence du codec Norito

Norito est la capacité de sérialisation canonique de Iroha. Chaque message en ligne, charge utile en discothèque et API entre les composants utilise Norito pour que les nœuds soient connectés en octets identiques, y compris lors de l'exécution d'un matériel distinct. Cette page reprend les pièces mobiles et indique la spécification complète en `norito.md`.

## Base Diseno

| Composants | Proposé | Source |
| --- | --- | --- |
| **Encabezado** | Encapsuler les charges utiles avec le hachage magique/version/schéma, CRC64, longitude et étiquette de compression ; v1 nécessite `VERSION_MINOR = 0x00` et valide les drapeaux de l'encabezado contre le mascara porté (par défaut `0x00`). | `norito::header` - ver `norito.md` ("En-tête et drapeaux", source du référentiel) |
| **Charge utile sans encabezado** | Codification déterministe des valeurs utilisée pour le hachage/comparaison. El transporte on-wire is usa encabezado; los bytes sin encabezado son solo internos. | `norito::codec::{Encode, Decode}` |
| **Compression** | Zstd optionnel (et accélération GPU expérimentale) sélectionné via l'octet de compression de l'encabezado. | `norito.md`, "Négociation de compression" |Le registre des indicateurs de mise en page (packed-struct, packed-seq, field bitset, compact lengths) se trouve dans `norito::header::flags`. V1 usa flags `0x00` par défaut mais accepté les flags explicitos dentro du mascara soportada ; los bits desconocidos se rechazan. `norito::header::Flags` est conservé pour l'inspection interne et les versions futures.

## Support de dérive

`norito_derive` dérive `Encode`, `Decode`, `IntoSchema` et les assistants JSON. Conventions clés :

- Los dérive des routes génériques AoS y emballées ; v1 utilise la mise en page AoS par défaut (drapeaux `0x00`) alors que les drapeaux encadrés optent pour des variantes emballées. La mise en œuvre est vive en `crates/norito_derive/src/derive_struct.rs`.
- Les fonctions qui affectent la mise en page (`packed-struct`, `packed-seq`, `compact-len`) sont opt-in via des drapeaux encochés et doivent être codifiées/décodifiées de manière cohérente entre pairs.
- Les assistants JSON (`norito::json`) ont prouvé que JSON était déterminé par Norito pour les API ouvertes. États-Unis `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec et tableaux d'identification

Norito conserve vos attributions multicodec et `norito::multicodec`. Le tableau de référence (hachages, types de clés, descripteurs de charge utile) est conservé en `multicodec.md` dans la racine du référentiel. Lorsque vous obtenez un nouvel identifiant :1. Actualisation `norito::multicodec::registry`.
2. Étendre la table en `multicodec.md`.
3. Régénérez les liaisons en aval (Python/Java) si vous utilisez la carte.

## Régénérer les documents et les luminaires

Avec le portail alojando por ahora un CV en prosa, utilisez les sources Markdown en amont comme source de vérité :

- **Spécification** : `norito.md`
- **Tableau multicodec** : `multicodec.md`
- **Références** : `crates/norito/benches/`
- **Tests d'or** : `crates/norito/tests/`

Lorsque l'automatisation de Docusaurus entre en production, le portail actualise un script de synchronisation (suivi en `docs/portal/scripts/`) qui extrae les données de ces archives. Alors, gardez cette page alignée manuellement à chaque fois que vous modifiez les spécifications.