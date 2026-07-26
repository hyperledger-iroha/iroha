---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Référence du codec #Norito

Norito Iroha pour couche de sérialisation canonique Message sur fil, charge utile sur disque et API multi-composants Norito pour les nœuds et le matériel pour les octets متفق رہیں۔ Il s'agit de la spécification `norito.md` et de la spécification `norito.md`.

## Disposition de base

| Composant | Objectif | Source |
| --- | --- | --- |
| **En-tête** | hachage magique/version/schéma, CRC64, longueur, et balise de compression pour les charges utiles et les balises de compression v1 est `VERSION_MINOR = 0x00` et les drapeaux d'en-tête et les masques pris en charge peuvent être validés (par défaut `0x00`). | `norito::header` — `norito.md` ("En-tête et indicateurs", racine du référentiel) دیکھیں |
| **Charge utile nue** | hachage/موازنہ کیلئے codage de valeur déterministe۔ En-tête de transport filaire octets nus صرف اندرونی استعمال کیلئے ہیں۔ | `norito::codec::{Encode, Decode}` |
| **Compression** | Zstd en option (accélération GPU expérimentale) et en-tête et octet de compression | `norito.md`, « Négociation de compression » |registre d'indicateurs de mise en page (packed-struct, packed-seq, champ bitset, longueurs compactes) `norito::header::flags` میں ہے۔ Valeurs par défaut de la version V1, ainsi que les indicateurs `0x00`, plus de masques pris en charge et plus d'indicateurs d'en-tête explicites. bits inconnus `norito::header::Flags` Inspection des versions et versions disponibles

## Obtenir du support

`norito_derive` `Encode`, `Decode`, `IntoSchema` et l'assistant JSON dérive le code source Ces conventions :

- Dérive les chemins de code AoS et compressés pour les utilisateurs Disposition AoS v1 (drapeaux `0x00`) et options par défaut et drapeaux d'en-tête, variantes emballées et option d'adhésion Implémentation `crates/norito_derive/src/derive_struct.rs` میں ہے۔
- Disposition des fonctionnalités de mise en page et de fonctionnalités (`packed-struct`, `packed-seq`, `compact-len`) drapeaux d'en-tête et opt-in pour les pairs et les pairs درمیان codage/décodage cohérent ہونی چاہئیں۔
- Les assistants JSON (`norito::json`) ouvrent des API et des outils JSON déterministes soutenus par Norito. `norito::json::{to_json_pretty, from_json}` استعمال کریں — کبھی `serde_json` نہیں۔

## Tables d'identifiants multicodecs

Norito pour les affectations multicodecs `norito::multicodec` میں رکھتا ہے۔ Table de référence (hachages, types de clés, descripteurs de charge utile) racine du référentiel `multicodec.md` میں برقرار رکھی جاتی ہے۔ جب نیا identifiant شامل ہو:1. `norito::multicodec::registry` اپڈیٹ کریں۔
2. Tableau de bord `multicodec.md`
3. La carte des liaisons en aval (Python/Java) permet de régénérer la carte.

## Docs اور luminaires کو régénérer کرنا

Il s'agit d'un hôte de résumé en prose qui contient des sources Markdown en amont et une source de vérité :

- **Spécification** : `norito.md`
- **Table multicodec** : `multicodec.md`
- **Références** : `crates/norito/benches/`
- **Tests d'or** : `crates/norito/tests/`

Pour Docusaurus automation live, il existe un script de synchronisation et un script de synchronisation (pour `docs/portal/scripts/`, une piste est disponible). گیا ہے) جو ان fichiers سے data کھینچتا ہے۔ تب تک، spec میں تبدیلی کے ساتھ اس صفحہ کو دستی طور پر align رکھیں۔