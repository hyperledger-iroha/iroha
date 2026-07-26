---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Référence du codec Norito

Norito et la caméra canonique de sérialisation Iroha. Aujourd'hui, les messages sont diffusés en ligne, la charge utile en discothèque et l'API entre les composants utilisant Norito pour que nos octets soient identiques lorsque nous utilisons un matériel différent. Cette page reprend en tant que parties principales et propose une spécification complète sur `norito.md`.

## Base de mise en page

| Composants | Proposé | Fonte |
| --- | --- | --- |
| **En-tête** | Charges utiles Enquadra avec hachage magique/version/schéma, CRC64, longueur et étiquette de compression ; v1 demande `VERSION_MINOR = 0x00` et valide les drapeaux d'en-tête contre le mascara pris en charge (par défaut `0x00`). | `norito::header` - ver `norito.md` ("En-tête et drapeaux", Raiz do Repositorio) |
| **En-tête sem de charge utile** | Codification déterministe des valeurs utilisée pour le hachage/comparaison. O transporte on-wire semper usa en-tête ; octets sem en-tête sao apenas internos. | `norito::codec::{Encode, Decode}` |
| **Compression** | Zstd facultatif (et accélération GPU expérimentale) sélectionné via l'octet de compression de l'en-tête. | `norito.md`, "Négociation de compression" |

L'enregistrement des indicateurs de mise en page (packed-struct, packed-seq, field bitset, compact lengths) figure dans `norito::header::flags`. V1 drapeaux américains `0x00` par padrao mas aceita header flags explicitas dentro da mascara suportada; bits desconhecidos sao rejeitados. `norito::header::Flags` et manteau pour inspection interne et versions futures.## Prise en charge d'un dérivé

`norito_derive` dérive `Encode`, `Decode`, `IntoSchema` et les assistants JSON. Conférences principales :

- Dérive Geram Caminhos AoS e emballé ; v1 USA layout AoS par padrao (drapeaux `0x00`) et les drapeaux d'en-tête sont moins choisis pour les variantes emballées. Implémentation dans `crates/norito_derive/src/derive_struct.rs`.
- Les ressources relatives à la mise en page (`packed-struct`, `packed-seq`, `compact-len`) telles que l'opt-in via les indicateurs d'en-tête et doivent être codifiées/décodifiées de manière cohérente entre pairs.
- Aides JSON (`norito::json`) pour la détermination JSON appliquée à Norito pour les API ouvertes. Utilisez `norito::json::{to_json_pretty, from_json}` - nunca `serde_json`.

## Multicodec et tableaux d'identification

Norito possède ses attributs de multicodec dans `norito::multicodec`. Un tableau de référence (hachages, types de mots-clés, descripteurs de charge utile) et un contenu sur `multicodec.md` dans le référentiel. Quand un nouvel identifiant et un ajout :

1. Actualisez `norito::multicodec::registry`.
2. Estenda a tabela em `multicodec.md`.
3. Régénérez les liaisons en aval (Python/Java) pour utiliser la carte.

## Régénérer les documents et les luminaires

Pour le portail qui héberge un CV en gros, utilisez comme polices Markdown en amont comme police de vérité :

- **Spécification** : `norito.md`
- **Table multicodec** : `multicodec.md`
- **Références** : `crates/norito/benches/`
- **Tests d'or** : `crates/norito/tests/`Lorsque l'automate Docusaurus entrera dans l'espace, le portail sera actualisé via un script de synchronisation (rasté sur `docs/portal/scripts/`) qui extraira les données de ces archives. Ate la, mantenha esta pagina alinhada manualmente semper que a spec mudar.