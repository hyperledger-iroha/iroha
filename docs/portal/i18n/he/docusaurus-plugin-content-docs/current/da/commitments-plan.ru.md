---
lang: he
direction: rtl
source: docs/portal/docs/da/commitments-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/commitments_plan.md`. Держите обе версии
:::

# План коммитментов זמינות נתונים Sora Nexus (DA-3)

_Черновик: 2026-03-25 — Владельцы: Core Protocol WG / צוות חוזה חכם / צוות אחסון_

DA-3 расширяет формат блока Nexus так, чтобы каждая ליין встраивала
детерминированные записи, описывающие כתמים, принятые DA-2. В этом документе
зафиксированы канонические структуры данных, хуки блокового пайплайна,
лайт-клиентские доказательства и поверхности Torii/RPC, которые должны появиться
до того, как валидаторы смогут полагаться на DA-коммитменты при admission или
проверках управления. Все payloadы Norito-кодированы; ללא SCALE ו-JSON אד-הוק.

## Цели

- Нести коммитменты на blob (שורש נתח + חשיש מניפסט + опциональный KZG
  התחייבות) внутри каждого блока Nexus, чтобы пиры могли реконструировать
  זמינות אופטימלית ללא שימוש באחסון מחוץ לפנקס החשבונות.
- Дать детерминированные הוכחות חברות, чтобы light clients могли проверить,
  что Manifest hash финализирован в конкретном блоке.
- Экспортировать Torii запросы (`/v2/da/commitments/*`) והוכחות, позволяющие
  ממסרים, ערכות פיתוח התוכנה וניהול אוטומציה פנו לזמינות ללא רכישה
  блоков.
- Сохранить канонический `SignedBlockWire` מעטפה, пропуская новые структуры
  ראה Norito כותרת מטא נתונים ו-hash בלוק גזירה.

## Обзор области работ

1. **מודל נתונים Дополнения** в `iroha_data_model::da::commitment` плюс изменения
   כותרת בלוק в `iroha_data_model::block`.
2. **ווי מנהלים** чтобы `iroha_core` ingest-ил DA קבלות, эмитированные
   Torii (`crates/iroha_core/src/queue.rs` ו-`crates/iroha_core/src/block.rs`).
3. **התמדה/אינדקסים** чтобы WSV быстро отвечал на שאילתות מחויבות
   (`iroha_core/src/wsv/mod.rs`).
4. **Torii תוספות RPC** לרשימה/שאילתה/הוכחת נקודות קצה под
   `/v2/da/commitments`.
5. **מבחני אינטגרציה + מתקנים** для проверки wire layout и proof flow в
   `integration_tests/tests/da/commitments.rs`.

## 1. מודל נתונים של Дополнения

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` переиспользует 48-байтовую точку из `iroha_crypto::kzg`.
  При отсутствии используем только מרקל הוכחות.
- `proof_scheme` берется из каталога נתיבי; נתיבי מרקל отклоняют מטעני KZG,
  а `kzg_bls12_381` נתיבים требуют ненулевых התחייבויות KZG. Torii сейчас
  производит только התחייבויות מרקל и отклоняет מסלולים с конфигурацией KZG.
- `KzgCommitment` переиспользует 48-байтовую точку из `iroha_crypto::kzg`.
  При отсутствии на Merkle lanes используем только מרקל הוכחות.
- `proof_digest` מזוקק DA-5 PDP/PoTR интеграцию, чтобы запись содержала
  דגימה расписание, использованное для поддержания כתמים.

### 1.2 כותרת בלוק Расширение

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

חבילת Хэш входит как в hash блока, так и в metadata `SignedBlockWire`. Когда
накладных расходов.הערת יישום: `BlockPayload` и прозрачный `BlockBuilder` теперь имеют
מגדירים/מצטרים `da_commitments` (см. `BlockBuilder::set_da_commitments` и
`SignedBlock::set_da_commitments`), так что מארח могут прикрепить
חבילת предварительно собранный до запечатывания блока. Все helper-конструкторы
оставляют поле `None` пока Torii не начнет передавать реальные חבילות.

### 1.3 קידוד חוט

- כותרת `SignedBlockWire::canonical_wire()` ל-Norito לכותרת
  `DaCommitmentBundle` сразу после списка транзакций. Версионный byte `0x01`.
- חבילות `SignedBlockWire::decode_wire()` отклоняет с неизвестной `version`,
  в соответствии с Norito политикой из `norito.md`.
- גזירת Hash обновляется только в `block::Hasher`; קל לקוחות, которые
  декодируют существующий פורמט תיל, автоматически получают новое поле, потому
  что Norito header объявляет его наличие.

## 2. Поток выпуска блоков

1. Torii DA לצרוך финализирует `DaIngestReceipt` и публикует его во внутреннюю
   очередь (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` собирает все קבלות с совпадающим `lane_id` для блока в
   построении, дедуплицируя по `(lane_id, client_blob_id, manifest_hash)`.
3. Перед запечатыванием Builder сортирует התחייבויות по `(lane_id, epoch,
   sequence)` для детерминированного hash, кодирует חבילה Norito codec и
   обновляет `da_commitments_hash`.
4. צרור Полный сохраняется в WSV и эмитируется вместе с блоком в
   `SignedBlockWire`.

Если создание блока проваливается, קבלות остаются в очереди для следующей
попытки; Builder записывает последний включенный `sequence` по каждой ליין,
чтобы предотвратить שידור חוזר атаки.

## 3. RPC и משטח שאילתה

Torii предоставляет три נקודת קצה:

| מסלול | שיטה | מטען | הערות |
|-------|--------|--------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (טווח-фильтр по מסלול/תקופה/רצף, עימוד) | Возвращает `DaCommitmentPage` с ספירה כוללת, התחייבויות и hash блока. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (נתיב + מניפסט hash или кортеж `(epoch, sequence)`). | Отвечает `DaCommitmentProof` (רשומה + נתיב מרקל + חשיש בלוקא). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | עוזר חסר מדינה, הכללה пересчитывающий hash блока и проверяющий; זמין עבור SDKs ללא תשלום עבור `iroha_crypto`. |

Все מטענים находятся в `iroha_data_model::da::commitment`. Torii роутеры
מטפלים מנוטריים рядом с существующими DA בולעת נקודות קצה, чтобы переиспользовать
token/mTLS политики.

## 4. הוכחות הכללה и לקוחות קלים

- Производитель блока строит бинарное Merkle дерево по сериализованному списку
  `DaCommitmentRecord`. Корень подает `da_commitments_hash`.
- `DaCommitmentProof` упаковывает целевой record и вектор `(sibling_hash,
  עמדה)` чтобы верификаторы смогли восстановить корень. הוכחה включает hash
  כותרת блока и подписанный, чтобы light clients могли проверить סופיות.
- CLI helpers (`iroha_cli app da prove-commitment`) оборачивают цикл בקשת הוכחה/
  בדוק ומצא את Norito/hex вывод для операторов.

## 5. אחסון и индексированиеהתחייבויות WSV хранит в отдельной משפחת העמודות с ключом `manifest_hash`.
Вторичные индексы покрывают `(lane_id, epoch)` ו-`(lane_id, sequence)`, чтобы
запросы не сканировали полные חבילות. Каждый record хранит высоту блока, в
котором он был запечатан, что позволяет להדביק узлам быстро восстанавливать
индекс из בלוק יומן.

## 6. טלמטריה и יכולת תצפית

- `torii_da_commitments_total` увеличивается, когда блок запечатывает минимум
  שיא один.
- `torii_da_commitment_queue_depth` отслеживает קבלות, כרוך
  (נתיב).
- Grafana לוח המחוונים `dashboards/grafana/da_commitments.json` визуализирует
  включение в блок, глубину очереди ותפוקת הוכחה לשחרור аудита DA-3
  שער.

## 7. Стратегия тестирования

1. **בדיקות יחידה** לקידוד/פענוח `DaCommitmentBundle` и обновлений
   לחסום גזירת חשיש.
2. **אביזרי זהב** в `fixtures/da/commitments/` с каноническими חבילת בתים
   והוכחות מרקל.
3. **מבחני אינטגרציה** с двумя валидаторами, בליעת כתמי דגימה и проверка
   חבילת согласованности ושאילתה/הוכחה ответов.
4. **בדיקות אור-לקוח** в `integration_tests/tests/da/commitments.rs` (חלודה),
   вызывающие `/prove` и проверяющие הוכחה без обращения к Torii.
5. **CLI עשן** скрипт `scripts/da/check_commitments.sh` для воспроизводимости
   операторского כלי עבודה.

## 8. השקת План

| שלב | תיאור | קריטריוני יציאה |
|-------|-------------|------------|
| P0 - מיזוג מודל נתונים | Принять `DaCommitmentRecord`, כותרת קוד בלוק או Norito קודקים. | מכשירי `cargo test -p iroha_data_model` зеленый с новыми. |
| P1 - חיווט ליבה/WSV | Протянуть תור + לוגי בונה בלוקים, сохранить индексы ו открыть מטפלי RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` הוכחה לצרור. |
| P2 - כלי עבודה למפעיל | הצג CLI helpers, Grafana לוח המחוונים ומסמכי אישור הוכחה. | `iroha_cli app da prove-commitment` работает на devnet; לוח המחוונים показывает live данные. |
| P3 - שער ממשל | Включить בלוק מאמת, требующий התחייבויות DA לנתיבים, отмеченных в `iroha_config::nexus`. | הזנת סטטוס + עדכון מפת הדרכים помечают DA-3 как завершенный. |

## Открытые вопросы

1. **ברירת המחדל של KZG נגד מרקל** — Нужно ли для маленьких blobs всегда пропускать
   התחייבויות KZG, чтобы уменьшить размер блока? Предложение: оставить
   `kzg_commitment` опциональным и gating через `iroha_config::da.enable_kzg`.
2. **פערים ברצף** — Разрешать ли разрывы последовательности? Текущий план
   отклоняет פערים, если ממשל не включит `allow_sequence_skips` для
   שידור חוזר של экстренного.
3. **מטמון קליינט קל** — Команда SDK запросила легкий מטמון SQLite עבור הוכחות;
   дальнейшая работа под DA-8.

Ответы на эти вопросы в יישום יחסי ציבור переведут DA-3 из статуса "טיוטה"
(этот документ) в "בביצוע" после старта кодовых работ.