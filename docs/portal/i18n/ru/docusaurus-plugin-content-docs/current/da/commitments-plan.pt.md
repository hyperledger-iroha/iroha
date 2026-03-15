---
lang: ru
direction: ltr
source: docs/portal/docs/da/commitments-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание Fonte canonica
Эспелья `docs/source/da/commitments_plan.md`. Мантенья как дуа-версоэс
:::

# План компрометации доступности данных от Sora Nexus (DA-3)

_Регидо: 25 марта 2026 г. -- Ответы: Рабочая группа по базовому протоколу / группа смарт-контрактов / группа хранения_

DA-3 расширен в формате блока Nexus для включения всех регистров
детерминированные, которые определяют капли, полученные из DA-2. Esta nota captura как
канонические инструкции, крючки для конвейеров блоков, как подтверждения
Клиенту необходимо указать Torii/RPC, который точно будет указан перед этим.
Валидаторы могут подтвердить наши компромиссы на время допуска или проверки
губернатора. Все полезные данные закодированы в Norito; объявление в формате SCALE или JSON
хок.

## Объективос

- Carregar compromissos por blob (корневой фрагмент + хэш манифеста + обязательство KZG
  необязательно) dentro de cada bloco Nexus для того, чтобы сверстники могли восстановить свое состояние
  Консультант по доступности хранилища для реестра.
- Форнесер обеспечивает детерминированное членство для того, чтобы клиенты ушли
  проверяем, что хэш манифеста завершен в блоке.
- Экспортные консультации Torii (`/v2/da/commitments/*`) и подтверждение того, что разрешено использовать реле,
  SDK и автоматическая проверка доступности могут воспроизводиться каждый блок.
- Конверт `SignedBlockWire` canonico ao enfiar as novas estruturas
  Пело заголовок метаданных Norito и производный хеш-блок.

## Панорама Эскопо

1. **Добавьтесь к модели данных** в `iroha_data_model::da::commitment`, затем внесите изменения.
   заголовок блока в `iroha_data_model::block`.
2. **Hooks do executor** для `iroha_core` приема квитанций DA, выданных для
   Torii (`crates/iroha_core/src/queue.rs` и `crates/iroha_core/src/block.rs`).
3. **Сохранение/индексы** для ответа WSV на консультации по компрометациям.
   быстрый (`iroha_core/src/wsv/mod.rs`).
4. **Adicoes RPC em Torii** для конечных точек списка/консультации/проверки
   `/v2/da/commitments`.
5. **Испытания на интеграцию + крепления**, проверка расположения проводов и проверка флюса.
   эм `integration_tests/tests/da/commitments.rs`.

## 1. Модель данных Adicoes ao

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

- `KzgCommitment` повторно использует 48 байт, использованных в `iroha_crypto::kzg`.
  Когда вы встретитесь, нажмите на кнопку «Меркле апенас».
- `proof_scheme` — производный каталог дорожек; полосы движения Merkle rejeitam полезные грузы KZG
  enquanto переулки `kzg_bls12_381` exigem обязательства KZG nao ноль. Torii в реальности
  так что производите компромиссные решения Merkle и используйте настройки дорожек с KZG.
- `KzgCommitment` повторно использует 48 байт, использованных в `iroha_crypto::kzg`.
  Когда вы выходите на полосу Меркле, нужно, чтобы Меркле был открыт.
- `proof_digest` перед интеграцией DA-5 PDP/PoTR для основной записи
  Перечислите расписание отбора проб, используемое для хранения больших объемов живых организмов.

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
```

Хеш-пакет войдет в блок без хеша-блока метаданных
`SignedBlockWire`. Когда блокировался персонал DA, или кампо фика `None` дляПримечание к внедрению: `BlockPayload` и `BlockBuilder` прозрачная версия назад
сеттеры/геттеры expoem `da_commitments` (версия `BlockBuilder::set_da_commitments`
e `SignedBlock::set_da_commitments`), entao Hosts Podem Anexar um Bundle
preconstruido antes de selar um bloco. Все помощники deixam o Campo em `None`
ел те Torii энкадные пакеты, которые есть в наличии.

### 1.3 Кодирование провода

- `SignedBlockWire::canonical_wire()` добавление к заголовку Norito для пункта
  `DaCommitmentBundle` немедленно прочтите список существующих транзакций. О
  байт версии и `0x01`.
- `SignedBlockWire::decode_wire()` rejeita связки cujo `version` seja
  deconhecido, alinhado a politica Norito, описанная в `norito.md`.
- Атуализированное получение хэша, существующего в `block::Hasher`; клиенты
  остается декодифицированным или проводным форматом, существующим в новом формате или новом кампо
  автоматически появляется заголовок Norito, объявляющий о своем появлении.

## 2. Поток производства блоков

1. Завершение обработки Torii с `DaIngestReceipt` и публикация файла.
   внутренняя (`iroha_core::gossiper::QueueMessage::DaReceipt`).
2. `PendingBlocks` содержит все квитанции, связанные с `lane_id`, соответствует блоку
   В конструкции выполнена дедупликация для `(lane_id, client_blob_id, manifest_hash)`.
3. Pouco antes de selar, o builder ordena os compromissos por `(lane_id, epoch,
   последовательность)` параметры детерминированного хэша, кодирования или пакета с помощью кодека
   Norito, и настроена `da_commitments_hash`.
4. Полный комплект и вооруженный WSV и объединенный блок с блоком
   `SignedBlockWire`.

Если блокировка заблокирована, квитанции остаются неизменными для того, чтобы приблизиться к ней.

предварительный захват; o регистрация застройщика o ultimo `sequence`, включенная в полосу движения
чтобы избежать повторных атак.

## 3. Поверхностные RPC и консультации

Torii выставляет конечные точки трех:

| Рота | Метод | Полезная нагрузка | Заметки |
|------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (диапазонный фильтр по полосе/эпохе/последовательности, страницам) | Retorna `DaCommitmentPage` с общим количеством, компромиссами и хеш-де-блоком. |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (дорожка + хеш манифеста или тупла `(epoch, sequence)`). | Ответьте на `DaCommitmentProof` (запись + адрес Меркла + хеш-де-блок). |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | Помощник без гражданства, который выполняет расчет хэша блока и проверки включения; использовать SDK, которые можно связать непосредственно с `iroha_crypto`. |

Todos os payloads vivem sob `iroha_data_model::da::commitment`. Маршрутизаторы ОС
Torii монтирует обработчики ОС и конечные точки приема существующих данных для
повторно использовать политику токена/mTLS.

## 4. Условия включения и уровни клиентов- O produtor de blocos constroi uma arvore Merkle binaria sobre a lista
  сериализация `DaCommitmentRecord`. Raiz alimenta `da_commitments_hash`.
- `DaCommitmentProof` empacota o Record Alvo mais um vetor de
  `(sibling_hash, position)` для проверки, чтобы восстановить результат. Как
  Провас тамбем включает или хеширует блок и заголовок, присвоенный для клиентов
  делает валидным окончательность.
- Помощники CLI (`iroha_cli app da prove-commitment`) включают цикл
  запросить/проверить электронную почту, указанную как Norito/hex для операций.

## 5. Хранение и индексация

O WSV Armazena Compromissos em uma Column Family dedicada com chave
`manifest_hash`. Индексы secundarios cobrem `(lane_id, epoch)` e
`(lane_id, sequence)`, чтобы проконсультироваться по поводу полных пакетов varrer. Када
записать растрейю в альтернативный блок, который или сам, позволив нам наверстать упущенное
Реконструировать или быстро индексировать часть журнала блокировки.

## 6. Телеметрия и наблюдение

- `torii_da_commitments_total` приращение, сколько блоков было в меносах
  запись.
- `torii_da_commitment_queue_depth` растрея квитанции aguardando Bundle (пор
  переулок).
- O приборная панель Grafana `dashboards/grafana/da_commitments.json` визуализация a
  включая блоки, глубину потока и пропускную способность для них
  Ворота выпуска DA-3 могут быть проверены или соблюдены.

## 7. Стратегия яичек

1. **Унитарные тесты** для кодирования/декодирования `DaCommitmentBundle` e
   автоматическое получение хэш-блока.
2. **Светильники золотые** sob `fixtures/da/commitments/` capturando bytes canonicos
   сделай связку и провас Меркле.
3. **Интеграционные тесты** с валидаторами, встроенными образцами и примерами.
   verificando que ambos os nos concordam no conteudo do Bundle e nas respostas
   де запрос/доказательство.
4. **Тесты уровня клиента** в `integration_tests/tests/da/commitments.rs`
   (Rust) que chamam `/prove` и проверенный образец с Torii.
5. **Smoke CLI** с `scripts/da/check_commitments.sh` параметрами инструментов
   Operadores воспроизводит.

## 8. План развертывания

| Фаза | Описание | Критерий выбора |
|------|-----------|-------------------|
| P0 — Объединить модель данных | Интегрируйте `DaCommitmentRecord`, инициализируйте заголовок блока и кодеки Norito. | `cargo test -p iroha_data_model` светильники verde com novas. |
| P1 — жила проводки/WSV | Логическая логика файла + построитель блоков, сохранение индексов и обработчики экспорта RPC. | `cargo test -p iroha_core`, `integration_tests/tests/da/commitments.rs` проходят с утверждениями пакета доказательств. |
| P2 - Инструменты для эксплуатации | Включите вспомогательный интерфейс командной строки, панель управления Grafana и обновления документов проверки доказательств. | `iroha_cli app da prove-commitment` работает против разработчиков; o приборная панель Mostra Dados ao Vivo. |
| P3 – Ворота управления | Навык проверки блоков, требующих компромиссов на полосах движения в `iroha_config::nexus`. | Введение статуса + обновление дорожной карты для модели DA-3 как ЗАВЕРШЕНО. |

## Пергунтас абертас1. **KZG против дефолтов Меркла** – Дедемос вечных обязательств KZG в больших объемах
   Пекенос, чтобы уменьшить или уменьшить блок? Пропоста: мантер `kzg_commitment`
   дополнительный электронный шлюз через `iroha_config::da.enable_kzg`.
2. **Пробелы в последовательности** — Разрешены ли полосы для порядка? O plano atual rejeita пробелы
   Залп включается активный режим `allow_sequence_skips` для повтора аварийной ситуации.
3. **Кэш легкого клиента** — во время SDK уровня кэша SQLite для проверки;
   подвеска на DA-8.

Ответчик пытается принять участие в инициативе по реализации проекта DA-3 от RASCUNHO (эте
документ) для EM ANDAMENTO, когда вы работаете с кодами Comecar.