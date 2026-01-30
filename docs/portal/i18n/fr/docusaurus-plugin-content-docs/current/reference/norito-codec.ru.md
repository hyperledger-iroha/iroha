---
lang: fr
direction: ltr
source: docs/portal/docs/reference/norito-codec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Справочник кодека Norito

Norito — канонический слой сериализации Iroha. Каждое on-wire сообщение, on-disk payload и межкомпонентный API используют Norito, чтобы узлы соглашались на идентичные байты даже при разном оборудовании. Эта страница резюмирует ключевые элементы и указывает на полную спецификацию в `norito.md`.

## Базовая компоновка

| Компонент | Назначение | Источник |
| --- | --- | --- |
| **Header** | Обрамляет payloads magic/version/schema hash, CRC64, длиной и тегом сжатия; v1 требует `VERSION_MINOR = 0x00` и проверяет header flags относительно поддерживаемой маски (по умолчанию `0x00`). | `norito::header` — см. `norito.md` ("Header & Flags", корень репозитория) |
| **Bare payload** | Детерминированное кодирование значений для hashing/сравнения. On-wire транспорт всегда использует header; bare байты — только внутренние. | `norito::codec::{Encode, Decode}` |
| **Compression** | Опциональный Zstd (и экспериментальное GPU-ускорение), выбираемый байтом сжатия в header. | `norito.md`, “Compression negotiation” |

Реестр layout flags (packed-struct, packed-seq, field bitset, compact lengths) находится в `norito::header::flags`. V1 по умолчанию использует flags `0x00`, но принимает явные flags в пределах поддерживаемой маски; неизвестные биты отклоняются. `norito::header::Flags` сохраняется для внутренней инспекции и будущих версий.

## Поддержка derive

`norito_derive` поставляет derive `Encode`, `Decode`, `IntoSchema` и JSON helpers. Ключевые соглашения:

- Derive генерируют как AoS, так и packed code paths; v1 по умолчанию использует AoS layout (flags `0x00`), если header flags не включают packed варианты. Реализация находится в `crates/norito_derive/src/derive_struct.rs`.
- Функции, влияющие на layout (`packed-struct`, `packed-seq`, `compact-len`), включаются opt-in через header flags и должны кодироваться/декодироваться согласованно между peers.
- JSON helpers (`norito::json`) предоставляют детерминированный Norito-backed JSON для публичных API. Используйте `norito::json::{to_json_pretty, from_json}` — никогда `serde_json`.

## Multicodec и таблицы идентификаторов

Norito хранит назначения multicodec в `norito::multicodec`. Референсная таблица (hashes, типы ключей, дескрипторы payload) поддерживается в `multicodec.md` в корне репозитория. При добавлении нового идентификатора:

1. Обновите `norito::multicodec::registry`.
2. Расширьте таблицу в `multicodec.md`.
3. Перегенерируйте downstream bindings (Python/Java), если они используют эту карту.

## Перегенерация docs и fixtures

Пока портал размещает лишь прозаическое резюме, используйте upstream Markdown источники как источник истины:

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Когда автоматизация Docusaurus будет включена, портал обновится через sync script (отслеживается в `docs/portal/scripts/`), который извлекает данные из этих файлов. До тех пор держите эту страницу вручную синхронизированной при каждом изменении спецификации.
