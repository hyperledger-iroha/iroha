---
lang: ru
direction: ltr
source: docs/portal/docs/da/commitments-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Источник канонический
Reflete `docs/source/da/commitments_plan.md`. Синхронизированные версии Gardez les deux
:::

# План мероприятий Доступность данных Sora Nexus (DA-3)

_Redige: 25 марта 2026 г. -- Ответственные: Рабочая группа по базовому протоколу/команда смарт-контрактов/команда хранения_

DA-3 сохраняет формат блока Nexus для объединения нескольких полос регистрации
детерминированный декривант les blobs принимает паритет DA-2. Файлы для захвата заметок
Канонические структуры, крючки для блоков, превентивные меры
клиент загрузил, и поверхности Torii/RPC, которые дойдут до того, как они появятся
валидаторы могут быть использованы для проверки обязательств DA для чеков
допуск или управление. Все полезные данные кодируются в Norito; па де
МАСШТАБ или специальный JSON.

## Цели

- Porter des Engagement par blob (корень фрагмента + хэш манифеста + обязательство KZG
  optionnel) в блоке Nexus, чтобы можно было восстановить пары
  l'etat d'availability без консультации с бухгалтерской книгой запасов.
- Преимущество членства определяется тем, что клиенты легеры
  verifient qu'un хэш манифеста, который можно завершить в готовом блоке.
- Exposer des requetes Torii (`/v2/da/commitments/*`) и дополнительные возможности
  дополнительные реле, SDK и автоматизация управления аудитом доступности
  блок sans rejouer chaque.
- Консерватор `SignedBlockWire` canonique en transmettant les nouvelles
  структуры через заголовок метаданных Norito и получение хеша блока.

## Вид ансамбля прицела

1. **Модель данных** в `iroha_data_model::da::commitment` plus
   модификации заголовка блока в `iroha_data_model::block`.
2. **Hooks d'executor** для того, чтобы `iroha_core` принимал квитанции DA по номиналу.
   Torii (`crates/iroha_core/src/queue.rs` и `crates/iroha_core/src/block.rs`).
3. **Постоянство/индексы** для того, чтобы WSV быстро отвечал на запросы
   обязательства (`iroha_core/src/wsv/mod.rs`).
4. **Добавлен RPC Torii** для конечных точек списка/лекций/доказательств.
   `/v2/da/commitments`.
5. **Испытания на интеграцию + крепления**, подтверждающие расположение проводов и проверку потока.
   в `integration_tests/tests/da/commitments.rs`.

## 1. Настройка модели данных

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

- `KzgCommitment` повторно использует точку 48 октетов в `iroha_crypto::kzg`.
  Когда его нет, он уникален в retombe sur des preuves Merkle.
- `proof_scheme` извлекает каталог полос; Лес Лейн Меркл rejettent лес
  полезные нагрузки KZG tandis que les полосы движения `kzg_bls12_381` необходимые обязательства KZG
  не нулевые значения. Torii не производит актуализации обязательств, принятых и отмененных
  les Lanes настраиваются в KZG.
- `KzgCommitment` повторно использует точку 48 октетов в `iroha_crypto::kzg`.
  Когда он отсутствовал на переулках Меркле на ретомбе сюр-де-преув Меркле
  уникальность.
- `proof_digest` ожидает интеграции DA-5 PDP/PoTR для записи мема
  Перечислите график выборки, используемый для поддержания больших объемов в жизни.

### 1.2 Расширение заголовка блока

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```Хэш-пакет входит в хеш-блок и в метаданные
`SignedBlockWire`. Когда блок не перевезет па-де-донне, поле отдыха

Примечание по реализации: `BlockPayload` и `BlockBuilder` прозрачный экспонированный.
обслуживание сеттеров/геттеров `da_commitments` (voir
`BlockBuilder::set_da_commitments` и `SignedBlock::set_da_commitments`), донк
хосты могут быть присоединены к предварительно сконструированному пакету перед блоком. Тус
Помощники отпустили чемпионский титул `None`, а Torii не включаются в комплекты
катушки.

### 1.3 Кодировка провода

- `SignedBlockWire::canonical_wire()` подключает заголовок Norito для заливки
  `DaCommitmentBundle` сразу после списка существующих транзакций.
  Байт версии — `0x01`.
- `SignedBlockWire::decode_wire()` отменять пакеты не `version` есть
  Inconnue, en ligne avec la politique Norito decrite dans `norito.md`.
- Les mises a jour de derivation du hash vivent uniquement dans `block::Hasher`;
  les клиенты легеры, которые декодируют формат провода, существующий gagnent le nouveau
  champ autoiquement car le header Norito объявляет о присутствии.

## 2. Поток производства блоков

1. Примите DA Torii, завершите `DaIngestReceipt` и опубликуйте в очереди.
   внутренний (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` собирайте все квитанции, не `lane_id` соответствует блоку.
   при создании и дедупликванте по `(lane_id, client_blob_id,
   манифест_хэш)`.
3. Просто перед запасом, строитель должен выполнить обязательства по `(lane_id, epoch,
   последовательность)` для проверки детерминированного хеша, кодирования пакета с помощью кодека
   Norito и встретился с `da_commitments_hash`.
4. Комплектация полная в WSV и Эмис с блоком в
   `SignedBlockWire`.

Если создание блока повторяется, квитанции остаются в очереди для того, чтобы
prochaine предварительный les rerenne; строитель зарегистрируйтесь позже `sequence`
включая повторные атаки.

## 3. Surface RPC и запросы

Torii выставляет три конечных точки:

| Маршрут | Метод | Полезная нагрузка | Заметки |
|-------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (диапазонный фильтр полоса/эпоха/последовательность, нумерация страниц) | Отзыв `DaCommitmentPage` с общим количеством, обязательствами и хэшем блока. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (дорожка + хэш манифеста или кортеж `(epoch, sequence)`). | Ответить с `DaCommitmentProof` (запись + chemin Merkle + хеш-де-блок). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Помощник без гражданства, который позволяет вычислить хэш-блок и проверить включение; используйте те SDK, которые не могут быть указаны в указании `iroha_crypto`. |

Все полезные нагрузки vivent sous `iroha_data_model::da::commitment`. Лес-маршрутизаторы
Torii устанавливает обработчики для конечных точек приема существующих DA для
повторно использовать токен политики/mTLS.

## 4. Preuves d'inclusion и клиентские права- Le producteur de bloc construit un arbre Merkle binaire sur la liste
  сериализованный номер `DaCommitmentRecord`. Растительное питание `da_commitments_hash`.
- `DaCommitmentProof` включает в себя кабельную запись и векторный рисунок
  `(sibling_hash, position)` Afin que les verificateurs puissent reconstruire la
  расин. Les preuves incluent aussi le hash de bloc et le header Signe pour que
  Клиенты могут получить окончательную проверку.
- Помощники CLI (`iroha_cli app da prove-commitment`) охватывают цикл файлов.
  запрос/проверка и раскрытие вылетов Norito/hex для операторов.

## 5. Хранение и индексация

Le WSV хранит обязательства в одной колонке «Семейный дед», cle par
`manifest_hash`. Вторые индексы соответствуют `(lane_id, epoch)` и др.
`(lane_id, sequence)` Afin que les requetes eviten de Scandes Bundles
комплекты. Chaque Recordsuit la hauteur de bloc qui l'a scelle, перметант aux
Вы можете быстро восстановить индекс в части журнала блоков.

## 6. Телеметрия и наблюдение

- `torii_da_commitments_total` приращение блока блоков в любое время
  запись.
- `torii_da_commitment_queue_depth` подходит для квитанций в комплекте
  (пар-лейн).
- Le приборная панель Grafana `dashboards/grafana/da_commitments.json` визуализировать
  включение блоков, профондер очереди и пропускная способность предварительной очистки
  что ворота выпуска DA-3 могут контролировать поведение.

## 7. Стратегия испытаний

1. **Единые тесты** для кодирования/декодирования `DaCommitmentBundle` и других файлов.
   упускает время получения хэш-блока.
2. **Светильники золотые** sous `fixtures/da/commitments/` захватывающие байты
   canoniques du Bundle et les Preuves Merkle.
3. **Тесты интеграции**, проверяющие два валидатора, поглощающие большие объемы
   протестируйте и проверьте, что два новых участника согласны с содержимым пакета и др.
   ответы на запросы/доказательства.
4. **Тестируется легкий клиент** на `integration_tests/tests/da/commitments.rs`.
   (Ржавчина) qui appellent `/prove` и проверена безопасность без разговоров с Torii.
5. **Smoke CLI** с `scripts/da/check_commitments.sh` для очистки улиц.
   воспроизводимый оператор.

## 8. План развертывания

| Фаза | Описание | Критерии выхода |
|-------|-------------|---------------|
| P0 — Объединение модели данных | Интегратор `DaCommitmentRecord`, не работает заголовок блока и кодеки Norito. | `cargo test -p iroha_data_model` vert с новыми светильниками. |
| P1 — жила проводки/WSV | Логика подключения очереди + построитель блоков, сохранение индексов и раскрытие обработчиков RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` прошли проверку с утверждениями пакета доказательств. |
| P2 - Оператор оснастки | Livrer помогает CLI, приборной панелью Grafana и проводит время с документами по проверке доказательств. | `iroha_cli app da prove-commitment` работает в сети разработчиков; le Dashboard affiche des donnees live. |
| P3 – Ворота управления | Активируйте валидатора блоков, требующих обязательств DA по сигнальным полосам в `iroha_config::nexus`. | Вход в статус + обновление дорожной карты для рынка DA-3 до ОКОНЧАНИЯ. |

## Открытые вопросы1. **KZG против дефолтов Меркла** – не обращайте внимания на обязательства KZG для
   les petits blobs afin de reduire la Taille des Blocs? Предложение: Гардер
   Опция `kzg_commitment` и управление через `iroha_config::da.enable_kzg`.
2. **Пробелы в последовательности** – Autorise-t-on des Lanes вне порядка? План фактического возврата
   les пробелы sauf si la gouvernance active `allow_sequence_skips` для повтора
   срочность.
3. **Кэш легкого клиента** — оснащен SDK для использования кэша SQLite для файлов.
   доказательства; suivi en attente sous DA-8.

Ответьте на эти вопросы в рамках PR-реализации, прошедшей DA-3 de
БРУЙОН (документ) в COURS des que le travail de code, начинается.