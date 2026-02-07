---
lang: ru
direction: ltr
source: docs/portal/docs/da/commitments-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::обратите внимание на Фуэнте каноника
Рефлея `docs/source/da/commitments_plan.md`. Mantenga ambas versiones ru
:::

# План компрометации доступности данных Sora Nexus (DA-3)

_Отредактировано: 25 марта 2026 г. -- Ответственные: Рабочая группа по базовому протоколу / группа по смарт-контрактам / группа по хранению_

DA-3 расширяет формат блока Nexus для того, чтобы каждый переулок инкрустировал регистры
определены те параметры, которые были приняты для DA-2. Esta nota captura las
канонические структуры данных, крючки трубопровода блоков, лас-прубас де
Клиенты ligero и las superficies Torii/RPC, которые должны быть удалены до того, как они будут
Валидаторы могут сообщить о компромиссе на время приема или чеков
гобернанса. Все полезные данные закодированы в Norito; без МАСШТАБИРОВАНИЯ и объявления JSON
хок.

## Объективос

- Компромиссы для больших двоичных объектов (корневой фрагмент + хэш манифеста + обязательство KZG).
  необязательно) в каждом блоке Nexus, чтобы можно было восстановить его
  состояние доступности без консультации с хранилищем бухгалтерской книги.
- Докажите, что членские взносы определены для того, чтобы клиенты были проверены.
  что манифест хэша был завершен в блоке данных.
- Проконсультируйтесь с Exponer Torii (`/v1/da/commitments/*`) и проверьте, что разрешено.
  реле, SDK и автоматизация государственного аудита, доступность без воспроизведения
  Када блок.
- Удерживайте конверт `SignedBlockWire` canonico al enhebrar las nuevas
  структуры, проходящие через заголовок метаданных Norito и вывод хэша
  блок.

## Панорама Альканца

1. **Добавление к модели данных** на `iroha_data_model::da::commitment` mas.
   выберите заголовок блока в `iroha_data_model::block`.
2. **Перехватывает исполнитель** для `iroha_core` приема квитанций DA, выданных для
   Torii (`crates/iroha_core/src/queue.rs` и `crates/iroha_core/src/block.rs`).
3. **Сохранение/индексы** для ответа WSV на консультации по компрометации.
   Рапидо (`iroha_core/src/wsv/mod.rs`).
4. **Добавление RPC в Torii** для конечных точек списка/консультаций/выходов на базу.
   `/v1/da/commitments`.
5. **Испытания на интеграцию + крепления** Проверка расположения проводов и трубопровода
   доказательство en `integration_tests/tests/da/commitments.rs`.

## 1. Дополнительные сведения о модели данных

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

- `KzgCommitment` повторно использует место размером 48 байт, использованное в `iroha_crypto::kzg`.
  Когда это произошло, вы увидите одинокие доказательства Меркла.
- `proof_scheme` является производным каталога дорожек; Лас Лейнс Меркл Решазан
  полезная нагрузка KZG, когда полосы движения `kzg_bls12_381` требуют обязательств KZG
  нет серо. Torii фактически соло производит компромиссы Merkle и rechaza переулков
  конфигурации с KZG.
- `KzgCommitment` повторно использует место размером 48 байт, использованное в `iroha_crypto::kzg`.
  Когда это произошло на полосе Меркла, вы встретитесь с Меркле, и это будет единственным доказательством.
- `proof_digest` ожидает интеграции DA-5 PDP/PoTR для записи неправильной записи
  Перечислите график отбора проб, используемый для хранения капель вживую.

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
```Хэш-дель-пакет входит в хэш-дель-блок как в метаданных
`SignedBlockWire`. Cuando un bloque no lleva datas DA el Campo Permanece `None`

Примечание о реализации: `BlockPayload` и прозрачный `BlockBuilder` сейчас.
экспоненты сеттеры/геттеры `da_commitments` (версия `BlockBuilder::set_da_commitments`
y `SignedBlock::set_da_commitments`), поскольку хосты могут быть дополнены пакетом
предварительно сконструировать перед продажей блока. Все помощники конструкторов Деян Эль
Кампо в `None` имеет Torii, включающий реальные пакеты.

### 1.3 Кодирование провода

- `SignedBlockWire::canonical_wire()` объединяет заголовок Norito для пункта
  `DaCommitmentBundle` немедленно после списка транзакций
  существует. Байт версии `0x01`.
- `SignedBlockWire::decode_wire()` восстанавливается в связках, где `version` уже не используется,
  Я расскажу о политике Norito, описав `norito.md`.
- Актуализации вывода хеш-функции соло в `block::Hasher`; Лос
  Клиенты освобождены от декодирования формата провода, существующего в новом кампо
  автоматически сообщает заголовок Norito.

## 2. Процесс производства блоков

1. Прием DA от Torii финализирует `DaIngestReceipt` и публикует в коле
   внутренняя (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` скопировал все квитанции, которые совпадают с `lane_id`.
   блокирование в конструкции, дедупликация для `(lane_id, client_blob_id,
   манифест_хэш)`.
3. Перед продажей строитель приказал пойти на компромисс из-за `(lane_id,
   эпоха, последовательность)` для сохранения определенного хеша, кодификации пакета с
   кодек Norito и актуализируется `da_commitments_hash`.
4. Полный комплект будет помещен в WSV и будет объединен в один блок
   `SignedBlockWire`.

Если вы создадите блок, квитанции останутся навсегда в коле для того, что вам нужно.
siguiente намерение лос Томе; el builder registera el ultimo `sequence` включен
пора переулок, чтобы избежать повторных атак.

## 3. Поверхностный RPC и консультация

Torii экспонирует конечные точки трех:

| Рута | Метод | Полезная нагрузка | Заметки |
|------|--------|---------|-------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (фильтр по рангу/эпохе/последовательности, страницам) | Devuelve `DaCommitmentPage` с полным набором, компромиссами и хеш-де-блоком. |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (дорожка + хеш манифеста или тупла `(epoch, sequence)`). | Ответить с `DaCommitmentProof` (запись + рута Меркла + хеш-де-блок). |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` | Помощник без сохранения состояния, который выполняет вычисление хэша блока и включение валидации; использовать SDK, которые нельзя использовать непосредственно для `iroha_crypto`. |

Все полезные нагрузки живут в bajo `iroha_data_model::da::commitment`. Лос-роутеры де
Torii соединяет обработчики с конечными точками приема существующих DA
повторно использовать политику токена/mTLS.

## 4. Пруэбас-де-инклюзив и клиенты-лигерос- Производитель блоков создает бинарный файл Merkle из списка
  сериализация `DaCommitmentRecord`. La Raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` запечатывает объектную запись в виде вектора
  `(sibling_hash, position)` для восстановления проверяемых данных. Лас
  pruebas tambien включает хэш-де-блок и заголовок, который будет твердым
  clientes ligeros verifiquen окончательность.
- Помощники CLI (`iroha_cli app da prove-commitment`) включают цикл
  запрос/проверка подтверждений и подтверждений Norito/hex para
  операторы.

## 5. Хранение и индексация

Компромиссный WSV в семействе колонн, выделенном с клавой
`manifest_hash`. Los indexes secundarios cubren `(lane_id, epoch)` y
`(lane_id, sequence)`, чтобы получить консультации по полным пакетам.
Cada запись rastrea la altura del bloque que lo slo, разрешено к узлу
быстрое восстановление индекса из блочного журнала.

## 6. Телеметрия и наблюдение

- `torii_da_commitments_total` увеличивается, когда блокируется продажа всех меносов
  запись.
- `torii_da_commitment_queue_depth` квитанции rastrea esperando ser empaquetados
  (Пор Лейн).
- El приборная панель Grafana `dashboards/grafana/da_commitments.json` визуализация ла
  включение в блоки, глубина колы и пропускная способность для того, чтобы
  ворота выпуска DA-3 можно будет проверить в транспортном средстве.

## 7. Стратегия пребаса

1. **Унитарные тесты** для кодирования/декодирования `DaCommitmentBundle` y
   актуализация получения хеша блока.
2. **Светильники золотые** bajo `fixtures/da/commitments/`, которые захватывают байты
   canonicos del Bundle и Pruebas Merkle.
3. **Тесты интеграции**, соответствующие валидаторам,
   Muestra и verificando que ambos nodos concuerdan en el contenido del Bundle y
   las respuestas de Consulta/prueba.
4. **Тестирование свободного клиента** на `integration_tests/tests/da/commitments.rs`
   (Rust) que llaman `/prove` и проверена проверка без хаблара с Torii.
5. **Дым CLI** с `scripts/da/check_commitments.sh` для инструментов управления
   deoperadores воспроизводимы.

## 8. План развертывания

| Фаза | Описание | Критерий успеха |
|------|-------------|--------------------|
| P0 — Объединение моделей данных | Интегрируйте `DaCommitmentRecord`, актуализируйте заголовок блока и кодеки Norito. | `cargo test -p iroha_data_model` в зеленом цвете с новыми светильниками. |
| P1 — Кабельное ядро/WSV | Логическая логика + построитель блоков, постоянные индексы и обработчики экспонеров RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` проходят с утверждениями доказательства пакета. |
| P2 - Инструменты для эксплуатации | Помощники Lanzar по CLI, панель мониторинга Grafana и актуализация документов по проверке доказательств. | `iroha_cli app da prove-commitment` работает против разработчиков; эль приборная панель muestra datas en vivo. |
| P3 — Ворота губернатора | Habilitar el validador de bloques que requiere compromisos DA на торговых полосах в `iroha_config::nexus`. | Сообщение о статусе + обновление дорожной карты для DA-3 как ЗАВЕРШЕНО. |

## Прегунтас абьертас1. **KZG против дефолтных значений Меркла** – мы можем исключить компромиссные решения KZG и большие объемы капитала
   для уменьшения тамано дель блока? Пропуэста: менеджер `kzg_commitment`
   опционально через `iroha_config::da.enable_kzg`.
2. **Пробелы в последовательности** – Разрешены ли полосы для порядка? Эль план фактическая речаза
   залп пробелов, который активен `allow_sequence_skips` для воспроизведения
   чрезвычайная ситуация.
3. **Кэш легкого клиента** — оборудование SDK для работы с кэшем SQLite
   доказательства; seguimiento pendiente bajo DA-8.

Ответчик находится на рассмотрении в связи с запросами на внедрение DA-3 от БОРРАДОР (эте
документ) a EN PROGRESO, когда начинается работа над кодом.