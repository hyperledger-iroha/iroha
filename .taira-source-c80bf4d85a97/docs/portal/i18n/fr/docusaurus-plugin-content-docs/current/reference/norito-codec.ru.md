---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Code de commande Norito

Norito — канонический слой сериализации Iroha. Une solution on-wire, une charge utile sur disque et une API mécanique utilisant Norito, que vous pouvez utiliser pour une baie identifiée при разном оборудовании. Cette étape consiste à utiliser des éléments clés et à obtenir les spécifications techniques du `norito.md`.

## Composant de base

| Composant | Назначение | Источник |
| --- | --- | --- |
| **En-tête** | Обрамляет payloads magic/version/schema hash, CRC64, длиной и тегом сжатия ; v1 utilise `VERSION_MINOR = 0x00` et vérifie les drapeaux d'en-tête de manière automatique en utilisant le masque (en utilisant `0x00`). | `norito::header` — см. `norito.md` ("En-tête et drapeaux", répertoire coréen) |
| **Charge utile nue** | Détermination des paramètres de hachage/démarrage. Le transport par fil utilise un en-tête ; nu байты — только внутренние. | `norito::codec::{Encode, Decode}` |
| **Compression** | Le Zstd spécial (et l'utilisation particulière du GPU) apparaît dans l'en-tête. | `norito.md`, « Négociation de compression » |

Les indicateurs de mise en page (packed-struct, packed-seq, field bitset, compact lengths) sont disponibles dans `norito::header::flags`. La version V1 utilise les drapeaux `0x00`, mais elle n'est pas incluse dans les masques précédents ; неизвестные биты отклоняются. `norito::header::Flags` est destiné aux inspections extérieures et aux versions ultérieures.

## Поддержка dériver`norito_derive` permet de dériver les assistants `Encode`, `Decode`, `IntoSchema` et JSON. Voici les principales fonctionnalités :

- Dériver les générateurs d'AoS, ainsi que les chemins de code compressés ; La v1 peut utiliser la disposition AoS (drapeaux `0x00`), si les indicateurs d'en-tête n'incluent pas les variantes compressées. La réalisation est effectuée dans `crates/norito_derive/src/derive_struct.rs`.
- Fonctions, liées à la mise en page (`packed-struct`, `packed-seq`, `compact-len`), incluant l'opt-in avec les drapeaux d'en-tête et les options кодироваться/декодироваться согласованно между pairs.
- Les assistants JSON (`norito::json`) permettent de détecter le JSON soutenu par Norito pour l'API publique. Utilisez `norito::json::{to_json_pretty, from_json}` — sans `serde_json`.

## Multicodec et tables d'identification

Norito fournit un multicodec dans `norito::multicodec`. Le tableau de référence (hachages, types de clés, descripteurs de charge utile) se trouve dans `multicodec.md` dans le référentiel de fichiers. Pour ajouter un nouvel identifiant :

1. Sélectionnez `norito::multicodec::registry`.
2. Sélectionnez la table dans `multicodec.md`.
3. Créez les liaisons en aval (Python/Java), si vous utilisez cette carte.

## Перегенерация docs et luminaires

Pour que le portail vous permette de résoudre votre problème, utilisez les options de Markdown en amont pour créer des fichiers :

- **Spécification** : `norito.md`
- **Table multicodec** : `multicodec.md`
- **Références** : `crates/norito/benches/`
- **Tests d'or** : `crates/norito/tests/`Lorsque l'automatisation Docusaurus est activée, le portail affiche le script de synchronisation (disponible dans `docs/portal/scripts/`), qui est installé данные из этих файлов. Il s'agit donc d'une étape de synchronisation selon les spécifications de configuration.