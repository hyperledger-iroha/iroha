---
lang: ru
direction: ltr
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Код `docs/source/da/commitments_plan.md`. Он сказал, что он Сэнсэй и Сэнсэй.
الوثائق القديمة.
:::

# خطة تعهدات توفر البيانات في Sora Nexus (DA-3)

Дата: 25 марта 2026 г. -- Информация: Рабочая группа по базовому протоколу / группа по смарт-контрактам / группа по хранению_

В DA-3 используется код Nexus, который находится на переулке Стокгольм, где есть blobs المقبولة
В DA-2. Он выступил в роли президента США в Вашингтоне, а затем в Вашингтоне.
Для этого необходимо установить Torii/RPC и подключиться к серверу.
Он сказал, что Д.А. جميع الحمولات مشفرة
بـ Norito; МАСШТАБ в формате JSON.

## الاهداف

- Создание большого двоичного объекта (корень фрагмента + хеш манифеста + код KZG)
  Проверьте Nexus, чтобы получить доступ к информации о доступности.
  Это было сделано в честь Дня святого Валентина.
- Дэниел Пинсон проинформировал о хеш-значении манифеста.
  Он был в Кейптауне.
- Код Torii (`/v1/da/commitments/*`) для подключения реле и SDK.
  Доступность всегда доступна в ближайшее время.
- على ظرف `SignedBlockWire` عبر تمرير البنى الجديدة من خلال
  Установите флажок Norito для создания хеш-кода.

## نظرة عامة على النطاق

1. **Запуск приложения** для `iroha_data_model::da::commitment` при запуске.
   Установите флажок `iroha_data_model::block`.
2. **Крючки** حتى يقوم `iroha_core` Получение квитанций الخاصة بـ DA
   Сообщение от Torii (`crates/iroha_core/src/queue.rs` и
   `crates/iroha_core/src/block.rs`).
3. **Сохраняющиеся/индексы** Создаются в WSV в зависимости от типа файла.
   (`iroha_core/src/wsv/mod.rs`).
4. **Запустите RPC для Torii** для создания списка/запроса/подтверждения `/v1/da/commitments`.
5. **Оборудование + светильники** Проверьте расположение проводов и доказательство.
   `integration_tests/tests/da/commitments.rs`.

## 1. اضافات نموذج البيانات

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

- `KzgCommitment` был отправлен на 48-й день недели.
  `iroha_crypto::kzg`. Он был Стивом Мерклом.
- `proof_scheme` на полосах движения; переулки в Меркле ترفض حمولات KZG,
  بينما переулки `kzg_bls12_381` تتطلب تعهدات KZG غير صفرية. Torii حاليا ينتج
  تعهدات Merkle в Нью-Йорке, в KZG.
- `KzgCommitment` был отправлен на 48-й день недели.
  `iroha_crypto::kzg`. Он был назван в честь Лейнса Меркла и его отца Меркла.
- `proof_digest` для DA-5 PDP/PoTR, установленного в DA-5 PDP/PoTR.
  На экране появляются капли.

### 1.2 Обновление версии

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

Хэш был создан для хэша, созданного в 18NI00000045X. عندما لا

Коды: `BlockPayload` и `BlockBuilder`.
сеттеры/геттеры لـ `da_commitments` (راجع `BlockBuilder::set_da_commitments`
و`SignedBlock::set_da_commitments`) Лей Тэмх принимает участие в турнире ارفاة مسبقة
Это было сделано для того, чтобы сделать это. جميع الدوال المساعدة تترك الحقل `None` حتى يمر Torii
حزم حقيقية.

### 1,3-дюймовый провод- `SignedBlockWire::canonical_wire()` для Norito
  `DaCommitmentBundle` был отключен от сети. بايت الاصدار هو
  `0x01`.
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` معروفة, بما
  Установите флажок Norito для `norito.md`.
- Создан хеш-код для `block::Hasher`; العملاء الخفيفون الذين
  Формат проводного соединения.
  Тэхен Уэйд.

## 2. تدفق انتاج الكتل

1. Выполните прием Torii DA `DaIngestReceipt` и нажмите
   Установите флажок (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. Получите `PendingBlocks` для получения квитанций `lane_id` для получения дополнительной информации.
   Это приложение `(lane_id, client_blob_id, manifest_hash)`.
3. Владелец дома, строитель Бангкока, `(lane_id, epoch, sequence)`
   Хеш-код создан в 18NT00000007X,
   `da_commitments_hash`.
4. Установите флажок для WSV на сервере `SignedBlockWire`.

Вы можете получить квитанции о получении денег в случае необходимости.
تالية؛ ويسجل builder اخر `sequence` تم تضمينه لكل Lane لتجنب هجمات replay.

## 3. Управление RPC

Torii определяет конечные точки:

| المسار | طريقة | حمولة | ملاحظات |
|--------|---------|---------|---------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (отображение дорожки/эпохи/последовательности, нумерации страниц) | Загрузите `DaCommitmentPage` и создайте хеш-код. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (дорожка + хеш манифеста или кортеж `(epoch, sequence)`). | يعيد `DaCommitmentProof` (запись + код Меркла + хеш-код). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Получение хэша без сохранения состояния без сохранения состояния Пакеты SDK доступны для использования в `iroha_crypto`. |

На сайте `iroha_data_model::da::commitment`. يقوم Torii بتركيب
обработчики, конечные точки, принимающие токены/mTLS.

## 4. Дэниел Уайт Уилсон

- Написано Мерклом Меркле в فوق قائمة `DaCommitmentRecord`
  المسلسلة. Приложение `da_commitments_hash`.
- `DaCommitmentProof` для `(sibling_hash, position)`.
  Лоуэн Уинстон Джонс Сити. تتضمن البراهين ايضا hash الكتلة والترويسة
  Он сказал, что это будет окончательное решение.
- Запустите CLI (`iroha_cli app da prove-commitment`) для получения дополнительной информации/загрузки.
  Установите флажок Norito/hex.

## 5. Устранение неполадок

Используется WSV для семейства столбцов `manifest_hash`. تغطي
Установите флажок `(lane_id, epoch)` и `(lane_id, sequence)` в режиме онлайн.
Это не так. Он был свидетелем того, как Спенсер вошел в историю.
Наверняка он наверстывает упущенное из-за Скелли.

## 6. القياس والرصد

- `torii_da_commitments_total` был создан в 2007 году.
- `torii_da_commitment_queue_depth` квитанции المنتظرة للتجميع (Lكل переулок).
- لوحة Grafana `dashboards/grafana/da_commitments.json` تعرض ادراج الكتل وعمق
  Производительность и пропускная способность были достигнуты в Нью-Йорке, где был установлен DA-3 в Сан-Франциско.

## 7. Обратная связь1. **Обработка** لترميز/فك ترميز `DaCommitmentBundle` с помощью хеш-кода
   Хорошо.
2. **Светильники золотые** تحت `fixtures/da/commitments/` تلتقط bytes الحزمة
   Его Величество Меркл.
3. **Вечеринка**, созданная в 1990 году, и BLOB-объекты, созданные в прошлом месяце.
   Он сказал, что он был в центре внимания и в сфере здравоохранения.
4. **Настройка ** для `integration_tests/tests/da/commitments.rs`
   (Ржавчина) Создан `/prove` и установлен в Torii.
5. **Smoke CLI** Код `scripts/da/check_commitments.sh`
   قابلة لاعادة الانتاج.

## 8. خطة الاطلاق

| عرحلة | الوصف | معيار الخروج |
|---------|-------|--------------|
| P0 - Свободный день | Установите `DaCommitmentRecord` и установите флажок Norito. | `cargo test -p iroha_data_model` — это светильники جديدة. |
| P1 — ядро ​​Core/WSV | Создан набор + построитель блоков, встроенный процессор и обработчики RPC. | `cargo test -p iroha_core` и `integration_tests/tests/da/commitments.rs` — это доказательство пакета. |
| P2 - Свободное время | Помощники для CLI включают Grafana и помогают найти доказательства. | `iroha_cli app da prove-commitment` на сайте devnet; Позвоните ему. |
| P3 - Информационный бюллетень | Он был создан в DA в полосах движения `iroha_config::nexus`. | Статус создания и дорожная карта для проекта DA-3. |

## اسئلة مفتوحة

1. **KZG по сравнению с дефолтными значениями Merkle** – в зависимости от размера BLOB-объектов KZG.
   حجم الكتلة؟ Название: `kzg_commitment`.
   `iroha_config::da.enable_kzg`.
2. **Пробелы в последовательности** – الخطة الحالية ترفض الفجوات الا
   Установите флажок `allow_sequence_skips`.
3. **Кэш легкого клиента** – использование SDK для SQLite и поддержка SQLite. متابعة
   Доступен для DA-8.

Он был создан для PRs в рамках проекта DA-3 в штате (Вашингтон)
Он был убит Биллом Бёрном.