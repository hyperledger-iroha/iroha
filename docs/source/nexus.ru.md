---
lang: ru
direction: ltr
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

#! Iroha 3 - Sora Nexus Ledger: Техническая спецификация дизайна

Этот документ предлагает архитектуру Sora Nexus Ledger для Iroha 3, развивая Iroha 2 к единой глобальной логически объединенной книге, организованной вокруг Data Spaces (DS). Data Spaces обеспечивают сильные домены приватности ("private data spaces") и открытую участие ("public data spaces"). Дизайн сохраняет компонуемость по всему глобальному леджеру, обеспечивая строгую изоляцию и конфиденциальность для данных частных DS, и вводит масштабирование доступности данных через erasure coding в Kura (хранилище блоков) и WSV (World State View).

Один и тот же репозиторий собирает Iroha 2 (самостоятельно размещаемые сети) и Iroha 3 (SORA Nexus). Исполнение обеспечивается общей Iroha Virtual Machine (IVM) и цепочкой инструментов Kotodama, поэтому контракты и артефакты байткода остаются переносимыми между self-hosted развертываниями и глобальным леджером Nexus.

Цели
- Единый глобальный логический леджер, составленный из множества взаимодействующих валидаторов и Data Spaces.
- Частные Data Spaces для permissioned-операций (например, CBDC), где данные никогда не покидают частный DS.
- Публичные Data Spaces с открытым участием, permissionless-доступом в стиле Ethereum.
- Компонуемые смарт-контракты между Data Spaces при явных разрешениях доступа к активам частных DS.
- Изоляция производительности, чтобы публичная активность не ухудшала внутренние транзакции частных DS.
- Масштабируемая доступность данных: erasure-coded Kura и WSV для фактически неограниченных данных при сохранении приватности частных DS.

Не-цели (начальная фаза)
- Определение токеномики или стимулов валидаторов; планирование и стейкинг подключаемые.
- Введение новой версии ABI или расширение поверхностей syscalls/pointer-ABI; ABI v1 фиксирован и runtime upgrades не меняют ABI хоста.

Терминология
- Nexus Ledger: Глобальный логический леджер, сформированный композицией блоков Data Space (DS) в единую упорядоченную историю и коммит состояния.
- Data Space (DS): Ограниченный домен исполнения и хранения со своими валидаторами, управлением, классом приватности, политикой DA, квотами и политикой fees. Существуют два класса: публичный DS и частный DS.
- Private Data Space: Permissioned-валидаторы и контроль доступа; данные транзакций и состояния никогда не покидают DS. Глобально якорятся только commitments/metadata.
- Public Data Space: Permissionless-участие; полные данные и состояние доступны публично.
- Data Space Manifest (DS Manifest): Norito-энкоденный манифест, описывающий параметры DS (валидаторы/ключи QC, класс приватности, политика ISI, параметры DA, ретеншн, квоты, ZK-политика, fees). Хэш манифеста якорится в цепи nexus. Если не указано иначе, DS quorum certificates используют ML-DSA-87 (класс Dilithium5) как схему постквантовой подписи по умолчанию.
- Space Directory: Глобальный on-chain каталог, отслеживающий манифесты DS, версии и события управления/ротации для резолвинга и аудита.
- DSID: Глобально уникальный идентификатор Data Space. Используется для namespacing всех объектов и ссылок.
- Anchor: Криптографический коммитмент DS блока/хедера, включенный в nexus chain, чтобы связать историю DS с глобальным леджером.
- Kura: Хранилище блоков Iroha. Здесь расширяется erasure-coded blob storage и commitments.
- WSV: World State View Iroha. Здесь расширяется версионированными сегментами состояния, snapshots и erasure coding.
- IVM: Iroha Virtual Machine для исполнения смарт-контрактов (байткод Kotodama `.to`).
 - AIR: Algebraic Intermediate Representation. Алгебраическое представление вычислений для STARK-подобных доказательств, описывающее исполнение как трассы над полем с ограничениями переходов и границ.

Модель Data Spaces
- Идентичность: `DataSpaceId (DSID)` идентифицирует DS и namespacing всего. DS может создаваться в двух гранулярностях:
  - Domain-DS: `ds::domain::<domain_name>` - исполнение и состояние в рамках домена.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - исполнение и состояние для одной дефиниции актива.
  Обе формы сосуществуют; транзакции могут атомарно затрагивать несколько DSID.
- Жизненный цикл манифеста: создание DS, обновления (ротация ключей, смена политики) и вывод фиксируются в Space Directory. Каждый артефакт на слот ссылается на хэш последнего манифеста.
- Классы: публичный DS (открытое участие, публичный DA) и частный DS (permissioned, конфиденциальный DA). Гибридные политики возможны через флаги манифеста.
- Политики на DS: ISI разрешения, параметры DA `(k,m)`, шифрование, ретеншн, квоты (мин/макс доля tx на блок), политика ZK/optimistic proofs, fees.
- Управление: членство DS и ротация валидаторов определяются секцией governance манифеста (on-chain предложения, multisig или внешнее управление, якоримое nexus транзакциями и attestations).

Gossip с учетом dataspace
- Батчи транзакционного gossip теперь несут тег плоскости (public vs restricted), полученный из каталога lanes; restricted батчи отправляются unicast в онлайн-узлы текущей commit topology (с учетом `transaction_gossip_restricted_target_cap`), тогда как public батчи используют `transaction_gossip_public_target_cap` (задайте `null` для broadcast). Выбор целей перетасовывается по каденсу плоскости, заданному `transaction_gossip_public_target_reshuffle_ms` и `transaction_gossip_restricted_target_reshuffle_ms` (по умолчанию: `transaction_gossip_period_ms`). Когда в commit topology нет онлайн-узлов, операторы могут выбрать отказ или пересылку restricted payloads в public overlay через `transaction_gossip_restricted_public_payload` (по умолчанию `refuse`); телеметрия показывает попытки fallback, счетчики forward/drop и политику вместе с выбором целей по dataspace.
- Неизвестные dataspaces переочередяются при включенном `transaction_gossip_drop_unknown_dataspace`; иначе они переходят в restricted targeting, чтобы избежать утечек.
- Валидация на приемной стороне отбрасывает записи, где lanes/dataspaces не совпадают с локальным каталогом, где тег плоскости не соответствует вычисленной видимости dataspace, или где заявленный маршрут не совпадает с локально переопределенным решением маршрутизации.

Capability manifests и UAID
- Универсальные аккаунты: каждый участник получает детерминированный UAID (`UniversalAccountId` в `crates/iroha_data_model/src/nexus/manifest.rs`), покрывающий все dataspaces. Capability manifests (`AssetPermissionManifest`) связывают UAID с конкретным dataspace, epoch активации/истечения и упорядоченным списком allow/deny правил `ManifestEntry`, которые ограничивают `dataspace`, `program_id`, `method`, `asset` и опциональные AMX роли. Deny всегда побеждает; оценщик выдает либо `ManifestVerdict::Denied` с причиной аудита, либо `Allowed` grant с метаданными разрешения.
- Снимки портфеля UAID теперь доступны через `GET /v1/accounts/{uaid}/portfolio` (см. `docs/source/torii/portfolio_api.md`), поддерживаются детерминированным агрегатором в `iroha_core::nexus::portfolio`.
- Allowances: каждое allow-правило содержит детерминированные `AllowanceWindow` buckets (`PerSlot`, `PerMinute`, `PerDay`) плюс опциональный `max_amount`. Hosts и SDKs потребляют один и тот же Norito payload, так что enforcement одинаков на всех аппаратных платформах и SDK.
- Аудитная телеметрия: Space Directory транслирует `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) при изменении состояния манифеста. Новый `SpaceDirectoryEventFilter` позволяет подписчикам Torii/data-event отслеживать обновления UAID, ревокации и решения deny-wins без отдельной plumbing.

### Операции манифестов UAID

Операции Space Directory поставляются в двух формах, чтобы операторы могли выбрать
встроенный CLI (для скриптовых rollout) или прямые отправки через Torii (для
автоматизированного CI/CD). Оба пути применяют разрешение `CanPublishSpaceDirectoryManifest{dataspace}`
в исполнителе (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`)
и записывают события жизненного цикла в world state (`iroha_core::state::space_directory_manifests`).

#### CLI workflow (`iroha app space-directory manifest ...`)

1. **Закодировать JSON манифеста** - преобразовать проекты политики в Norito байты и вывести
   воспроизводимый хэш до ревью:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   Helper принимает `--json` (сырой JSON манифест) или `--manifest` (существующий
   `.to` payload) и повторяет логику в
   `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **Опубликовать/заменить манифесты** - поставить в очередь инструкции
   `PublishSpaceDirectoryManifest` из Norito или JSON источников:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` заполняет `entries[*].notes` для записей, где не было заметок оператора.

3. **Истечь** манифесты по достижении срока или **отозвать**
   UAID по запросу. Оба команды принимают `--uaid uaid:<hex>` или 64-символьный
   hex digest (LSB=1) и числовой dataspace id:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **Сформировать audit bundles** - `manifest audit-bundle` записывает JSON манифеста,
   `.to` payload, хэш, профиль dataspace и машиночитаемые метаданные в
   выходной каталог, чтобы ревьюеры governance могли скачать один архив:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   Bundle встраивает `SpaceDirectoryEvent` hooks из профиля, чтобы подтвердить, что
   dataspace экспонирует обязательные audit webhooks; см. `docs/space-directory.md`
   для формата полей и требований к доказательствам.

#### Torii APIs

Операторы и SDKs могут выполнять те же действия по HTTPS. Torii применяет
те же проверки разрешений и подписывает транзакции от имени переданной
authority (приватные ключи находятся только в памяти внутри безопасного handler Torii):

- `GET /v1/space-directory/uaids/{uaid}` — разрешить текущие привязки dataspace
  для UAID (нормализованные адреса, dataspace ids, program bindings). Добавьте
  `address_format=compressed` для вывода Sora Name Service (IH58 предпочтительно; compressed (`sora`) — второй по предпочтению, только для Sora).
- `GET /v1/space-directory/uaids/{uaid}/portfolio` —
  агрегатор на Norito, зеркалящий `ToriiClient.getUaidPortfolio`, чтобы кошельки
  отображали универсальные holdings без сканирования состояния по dataspaces.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` — получить канонический
  JSON манифест, метаданные жизненного цикла и хэш для аудита.
- `POST /v1/space-directory/manifests` — отправить новые или заменяющие манифесты
  из JSON (`authority`, `private_key`, `manifest`, опциональный `reason`). Torii
  возвращает `202 Accepted`, когда транзакция в очереди.
- `POST /v1/space-directory/manifests/revoke` — поставить в очередь экстренные ревокации
  с UAID, dataspace id, эффективным epoch и опциональным reason (повторяет CLI формат).

JS SDK (`javascript/iroha_js/src/toriiClient.js`) уже оборачивает эти read-сферы
через `ToriiClient.getUaidPortfolio`, `.getUaidBindings` и
`.getUaidManifests`; будущие релизы Swift/Python используют те же REST payloads.
См. `docs/source/torii/portfolio_api.md` для схем request/response и
`docs/space-directory.md` для полного operator playbook.

Недавние обновления SDK/AMX
- **NX-11 (проверка cross-lane relay):** SDK helpers теперь валидируют
  lane relay envelopes из `/v1/sumeragi/status`. Rust клиент поставляет
  `iroha::nexus` helpers для построения/проверки relay proofs и отклонения
  дубликатов `(lane_id, dataspace_id, height)`, Python binding публикует
  `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`, а JS SDK дает
  `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample`, чтобы операторы могли
  проверять cross-lane transfer proofs с согласованными хэшами до пересылки.
  【crates/iroha/src/nexus.rs:1】【python/iroha_python/iroha_python_rs/src/lib.rs:666】【crates/iroha_js_host/src/lib.rs:640】【javascript/iroha_js/src/nexus.js:1】
- **NX-17 (AMX budget guardrails):** `ivm::analysis::enforce_amx_budget` оценивает
  стоимость исполнения на dataspace и группу, используя статический отчет анализа, и
  применяет бюджеты 30 ms / 140 ms, зафиксированные здесь. Helper сообщает
  явные нарушения по бюджету DS и группы, покрыт unit тестами, делая
  AMX slot budget детерминированным для Nexus scheduler и SDK tooling.
  【crates/ivm/src/analysis.rs:142】【crates/ivm/src/analysis.rs:241】

Высокоуровневая архитектура
1) Глобальный слой композиции (Nexus Chain)
- Поддерживает единый канонический порядок 1-секундных Nexus Blocks, которые финализируют атомарные транзакции, затрагивающие один или несколько Data Spaces (DS). Каждая коммитнутая транзакция обновляет глобальное единое world state (вектор корней по DS).
- Содержит минимум метаданных плюс агрегированные proofs/QC для компонуемости, финальности и обнаружения мошенничества (затронутые DSID, корни состояния по DS до/после, DA commitments, proofs валидности по DS и DS quorum certificate с ML-DSA-87). Приватные данные не включаются.
- Консенсус: единый глобальный pipeline BFT комитет из 22 участников (3f+1 при f=7), выбранный из пула до ~200k потенциальных валидаторов через VRF/stake по эпохам. Nexus комитет упорядочивает транзакции и финализирует блок за 1 s.

2) Слой Data Space (Public/Private)
- Исполняет фрагменты глобальных транзакций по DS, обновляет локальный DS WSV и производит артефакты валидности на блок (агрегированные DS proofs и DA commitments), которые складываются в 1-секундный Nexus Block.
- Частные DS шифруют данные в покое и на линии между авторизованными валидаторами; только commitments и PQ proofs валидности выходят из DS.
- Публичные DS экспортируют полные тела данных (через DA) и PQ proofs валидности.

3) Атомарные cross-Data-Space транзакции (AMX)
- Модель: каждая пользовательская транзакция может затрагивать несколько DS (например, domain DS и один или несколько asset DS). Она коммитится атомарно в 1-секундном Nexus Block или отменяется; частичных эффектов нет.
- Prepare-Commit за 1 s: для каждой транзакции затронутые DS исполняются параллельно на одном snapshot (DS roots начала слота) и производят PQ proofs валидности (FASTPQ-ISI) и DA commitments. Nexus комитет коммитит транзакцию только если все DS proofs валидны и DA сертификаты пришли (целевое <=300 ms); иначе транзакция переносится в следующий слот.
- Согласованность: объявляются read-write sets; конфликт детектируется при коммите относительно корней начала слота. Optimistic исполнение без блокировок по DS избегает глобальных стопов; атомарность обеспечивается правилом commit nexus (all-or-nothing между DS).
- Приватность: частные DS экспортируют только proofs/commitments, привязанные к корням DS до/после. Сырые приватные данные не покидают DS.

4) Доступность данных (DA) с erasure coding
- Kura хранит тела блоков и snapshots WSV как erasure-coded blobs. Публичные blobs широко шарятся; частные blobs хранятся только внутри валидаторов частных DS, с зашифрованными chunks.
- DA commitments записываются и в DS артефакты, и в Nexus Blocks, обеспечивая выборку и восстановление без раскрытия приватного содержимого.

Структура блока и коммита
- Артефакт доказательства Data Space (на 1s слот, на DS)
  - Поля: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Частные DS экспортируют артефакты без тел данных; публичные DS позволяют получать тела через DA.

- Nexus Block (каденс 1 s)
  - Поля: block_number, parent_hash, slot_time, tx_list (атомарные cross-DS транзакции с затронутыми DSID), ds_artifacts[], nexus_qc.
  - Функция: финализирует все атомарные транзакции, для которых требуемые DS артефакты валидны; обновляет глобальный вектор DS roots одним шагом.

Консенсус и планирование
- Консенсус Nexus Chain: единый глобальный pipeline BFT (класс Sumeragi) с комитетом 22 узла (3f+1 при f=7), нацеленным на 1 s блоки и 1 s финальность. Члены комитета выбираются по эпохам через VRF/stake из ~200k кандидатов; ротация сохраняет децентрализацию и устойчивость к цензуре.
- Консенсус Data Space: каждый DS выполняет свой BFT среди валидаторов для выпуска артефактов по слотам (proofs, DA commitments, DS QC). Комитеты lane-relay размером `3f+1` рассчитываются по `fault_tolerance` dataspace и детерминированно выбираются на эпоху из пула валидаторов dataspace, используя VRF seed эпохи, связанный с `(dataspace_id, lane_id)`. Частные DS permissioned; публичные DS допускают открытую liveness при анти-Sybil политиках. Глобальный nexus комитет не изменяется.
- Планирование транзакций: пользователи отправляют атомарные транзакции с указанием DSID и read-write sets. DS исполняются параллельно в слоте; nexus комитет включает транзакцию в 1 s блок, если все DS артефакты валидны и DA сертификаты приходят вовремя (<=300 ms).
- Изоляция производительности: каждый DS имеет независимые mempool и исполнение. Квоты per-DS ограничивают число транзакций, затрагивающих DS, на блок, чтобы избежать head-of-line blocking и защищать latency частных DS.

Модель данных и namespacing
- DS-квалифицированные IDs: все сущности (домены, аккаунты, активы, роли) квалифицируются `dsid`. Пример: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Глобальные ссылки: глобальная ссылка - это кортеж `(dsid, object_id, version_hint)` и может размещаться on-chain на уровне nexus или в AMX дескрипторах для cross-DS использования.
- Norito сериализация: все cross-DS сообщения (AMX дескрипторы, proofs) используют Norito кодеки. Serde не применяется в прод путях.

Смарт-контракты и расширения IVM
- Контекст исполнения: добавить `dsid` в контекст исполнения IVM. Контракты Kotodama всегда выполняются внутри конкретного Data Space.
- Atomic Cross-DS примитивы:
  - `amx_begin()` / `amx_commit()` обозначают атомарную multi-DS транзакцию в IVM host.
  - `amx_touch(dsid, key)` объявляет намерение чтения/записи для конфликтной проверки относительно snapshot roots слота.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (операция разрешена только если политика допускает и handle валиден)
- Asset handles и fees:
  - Операции с активами авторизуются ISI/роль политиками DS; fees оплачиваются газ-токеном DS. Опциональные capability tokens и более богатая политика (multi-approver, rate-limits, geofencing) могут быть добавлены позже без изменения атомарной модели.
- Детерминизм: все syscalls чистые и детерминированные при заданных входах и заявленных AMX read/write sets. Никаких скрытых эффектов времени или окружения.

Постквантовые доказательства валидности (обобщенные ISI)
- FASTPQ-ISI (PQ, без trusted setup): kernelized hash-based аргумент, обобщающий transfer дизайн на все семейства ISI и нацеленный на субсекундные доказательства для партий масштаба 20k на GPU-классе.
  - Операционный профиль:
    - Production-ноды строят prover через `fastpq_prover::Prover::canonical`, который теперь всегда инициализирует production backend; детерминированный mock удален.【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode` (config) и `irohad --fastpq-execution-mode` позволяют операторам закрепить CPU/GPU execution детерминированно, при этом observer hook записывает тройки requested/resolved/backend для fleet аудитa.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- Арифметизация:
  - KV-Update AIR: рассматривает WSV как типизированную key-value карту, зафиксированную через Poseidon2-SMT. Каждый ISI разворачивается в небольшой набор read-check-write строк по ключам (accounts, assets, roles, domains, metadata, supply).
  - Opcode-gated constraints: одна AIR таблица с selector колонками обеспечивает правила per-ISI (conservation, monotonic counters, permissions, range checks, bounded metadata updates).
  - Lookup arguments: прозрачные hash-committed таблицы для permissions/roles, asset precision и policy параметров избегают тяжелых битовых constraints.
- Commitments состояния и обновления:
  - Aggregated SMT Proof: все затронутые ключи (pre/post) доказываются против `old_root`/`new_root` с сжатой frontier и дедупом siblings.
  - Invariants: глобальные инварианты (например, total supply per asset) обеспечиваются равенством multisets между effect rows и отслеживаемыми счетчиками.
- Proof system:
  - FRI-style полиномиальные commitments (DEEP-FRI) с высокой арностью (8/16) и blow-up 8-16; Poseidon2 hashes; Fiat-Shamir transcript с SHA-2/3.
  - Опциональная рекурсия: локальная рекурсивная агрегация DS для сжатия micro-batches в одно доказательство на слот при необходимости.
- Scope и примеры:
  - Assets: transfer, mint, burn, register/unregister asset definitions, set precision (bounded), set metadata.
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories (state-only; проверки подписи аттестуются DS валидаторами, не доказываются внутри AIR).
  - Roles/Permissions (ISI): grant/revoke roles и permissions; обеспечиваются lookup таблицами и monotonic policy checks.
  - Contracts/AMX: AMX begin/commit markers, capability mint/revoke если включено; доказываются как state transitions и policy counters.
- Вне-AIR проверки для сохранения латентности:
  - Подписи и тяжелая криптография (например, ML-DSA user signatures) проверяются DS валидаторами и аттестуются в DS QC; proof валидности покрывает только согласованность состояния и соблюдение политики. Это делает proof PQ и быстрыми.
- Целевые показатели производительности (иллюстративно, 32-core CPU + современный GPU):
  - 20k смешанных ISI с малым touch (<=8 ключей/ISI): ~0.4-0.9 s prove, ~150-450 KB proof, ~5-15 ms verify.
  - Более тяжелые ISI (больше ключей/богатые constraints): micro-batch (например, 10x2k) + recursion, чтобы держать per-slot <1 s.
- Конфигурация DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (подписи проверяются DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (по умолчанию; альтернативы должны быть явно объявлены)
- Fallbacks:
  - Сложные/кастомные ISI могут использовать общий STARK (`zk.policy = "stark_fri_general"`) с отложенным доказательством и 1 s финальностью через QC attestation + slashing за неверные proofs.
  - Не-PQ опции (например, Plonk с KZG) требуют trusted setup и больше не поддерживаются в дефолтной сборке.

AIR Primer (для Nexus)
- Execution trace: матрица ширины (колонки регистров) и длины (шаги). Каждая строка - логический шаг ISI; колонки содержат pre/post значения, селекторы и флаги.
- Constraints:
  - Transition constraints: связывают строки (например, post_balance = pre_balance - amount для debit строки при `sel_transfer = 1`).
  - Boundary constraints: связывают публичный I/O (old_root/new_root, counters) с первой/последней строкой.
  - Lookups/permutations: обеспечивают membership и multiset равенства против зафиксированных таблиц (permissions, asset params) без битовых тяжелых схем.
- Commitment и verification:
  - Prover фиксирует trace через hash-энкодинг и строит low-degree полиномы, валидные если constraints выполняются.
  - Verifier проверяет low-degree через FRI (hash-based, post-quantum) с несколькими Merkle openings; стоимость логарифмическая по шагам.
- Пример (Transfer): регистры включают pre_balance, amount, post_balance, nonce и селекторы. Constraints обеспечивают неотрицательность/диапазон, conservation и monotonicity nonce, а агрегированный SMT multi-proof связывает pre/post листья с old/new roots.

Стабильность ABI (ABI v1)
- Поверхность ABI v1 фиксирована; новые syscalls и pointer-ABI типы в этом релизе не добавляются.
- Runtime upgrades должны сохранять `abi_version = 1` с пустыми `added_syscalls`/`added_pointer_types`.
- ABI goldens (список syscalls, хеш ABI, ID pointer type) закреплены и не должны меняться.

Модель приватности
- Private Data Containment: тела транзакций, state diffs и WSV snapshots для частных DS никогда не покидают частный набор валидаторов.
- Public Exposure: экспортируются только headers, DA commitments и PQ validity proofs.
- Опциональные ZK proofs: частные DS могут выпускать ZK proofs (например, достаточный баланс, политика соблюдена) для cross-DS действий без раскрытия внутреннего состояния.
- Контроль доступа: авторизация обеспечивается ISI/role политиками внутри DS. Capability tokens опциональны и могут быть добавлены позже при необходимости.

Изоляция производительности и QoS
- Отдельные consensus, mempools и storage на DS.
- Nexus scheduling quotas per DS ограничивают время включения anchors и избегают head-of-line blocking.
- Бюджеты ресурсов контрактов per DS (compute/memory/IO), enforced IVM host. Публичная нагрузка DS не может потребить бюджеты частных DS.
- Асинхронные cross-DS вызовы избегают длительных синхронных ожиданий внутри частного DS исполнения.

Доступность данных и дизайн хранения
1) Erasure Coding
- Использовать систематический Reed-Solomon (например, GF(2^16)) для erasure coding blobs Kura блоков и WSV snapshots: параметры `(k, m)` с `n = k + m` shards.
- Параметры по умолчанию (предлагаемые, публичные DS): `k=32, m=16` (n=48), восстановление до 16 потерь shard с ~1.5x расширением. Для частных DS: `k=16, m=8` (n=24) внутри permissioned набора. Оба параметра настраиваются в DS Manifest.
- Public Blobs: shards распределяются по многим DA nodes/валидаторам с sampling availability checks. DA commitments в headers позволяют light clients проверять доступность.
- Private Blobs: shards шифруются и распределяются только среди валидаторов частного DS (или назначенных custodians). Глобальная цепь несет только DA commitments (без локаций shards или ключей).

2) Commitments и sampling
- Для каждого blob: вычислить Merkle root по shards и включить в `*_da_commitment`. Оставаться PQ, избегая эллиптических commitments.
- DA Attesters: VRF-sampled региональные attesters (например, 64 на регион) выпускают сертификат ML-DSA-87, подтверждающий успешное sampling shards. Целевая латентность DA attestation <=300 ms. Nexus комитет валидирует сертификаты вместо извлечения shards.

3) Интеграция Kura
- Блоки хранят тела транзакций как erasure-coded blobs с Merkle commitments.
- Хедеры несут blob commitments; тела доступны через DA сеть для публичных DS и через частные каналы для частных DS.

4) Интеграция WSV
- WSV Snapshotting: периодически чекпоинтить состояние DS в chunked erasure-coded snapshots с commitments в headers. Между snapshots вести change logs. Публичные snapshots широко шарятся; частные snapshots остаются у приватных валидаторов.
- Proof-carrying access: контракты могут предоставлять (или запрашивать) state proofs (Merkle/Verkle), привязанные к snapshot commitments. Частные DS могут давать zero-knowledge attestations вместо сырых proofs.

5) Retention и pruning
- Нет pruning для публичных DS: сохранять все тела Kura и WSV snapshots через DA (горизонтальное масштабирование). Частные DS могут задавать внутренний ретеншн, но экспортированные commitments остаются неизменными. Nexus слой хранит все Nexus Blocks и DS artifact commitments.

Сети и роли узлов
- Глобальные валидаторы: участвуют в консенсусе nexus, валидируют Nexus Blocks и DS артефакты, выполняют DA checks для публичных DS.
- Валидаторы Data Space: выполняют DS consensus, исполняют контракты, управляют локальными Kura/WSV, обслуживают DA для своего DS.
- DA Nodes (опционально): хранят/публикуют публичные blobs, облегчают sampling. Для частных DS DA nodes co-located с валидаторами или доверенными custodians.

Системные улучшения и соображения
- Decoupling sequencing/mempool: принять DAG mempool (например, Narwhal-style), питающий pipeline BFT на уровне nexus для снижения латентности и повышения throughput без изменения логической модели.
- DS quotas и fairness: per-DS per-block quotas и weight caps, чтобы избежать head-of-line blocking и обеспечить предсказуемую латентность для частных DS.
- DS attestation (PQ): DS quorum certificates по умолчанию используют ML-DSA-87 (класс Dilithium5). Это постквантово и больше EC signatures, но приемлемо на один QC per slot. DS могут явно выбрать ML-DSA-65/44 (меньше) или EC signatures, если это заявлено в DS Manifest; публичные DS настоятельно рекомендуются сохранять ML-DSA-87.
- DA attesters: для публичных DS использовать VRF-sampled региональных attesters, выпускающих DA сертификаты. Nexus комитет валидирует сертификаты вместо сырого sampling shards; частные DS держат DA attestations внутри.
- Recursion и epoch proofs: опционально агрегировать несколько micro-batches внутри DS в одно рекурсивное proof per slot/epoch, чтобы держать размеры proofs и время verify стабильными под нагрузкой.
- Lane scaling (если нужно): если единый глобальный комитет становится узким местом, вводятся K параллельных sequencing lanes с детерминированным merge. Это сохраняет единый глобальный порядок при горизонтальном масштабировании.
- Deterministic acceleration: предоставить SIMD/CUDA feature-gated kernels для hashing/FFT с bit-exact CPU fallback для детерминизма между hardware.
- Lane activation thresholds (proposal): включать 2-4 lanes, если (a) p95 финальность превышает 1.2 s более 3 минут подряд, или (b) заполненность блока >85% более 5 минут, или (c) входящая tx rate требует >1.2x block capacity на устойчивых уровнях. Lanes детерминированно bucketize транзакции по DSID hash и мержат в nexus блок.

Fees и экономика (начальные дефолты)
- Gas unit: per-DS gas token с измеряемыми compute/IO; fees оплачиваются в нативном gas активе DS. Конвертация между DS - забота приложения.
- Inclusion priority: round-robin между DS с per-DS квотами для fairness и 1 s SLO; внутри DS fee bidding разрешает равенства.
- Будущее: опциональный глобальный fee market или MEV-minimizing политики можно исследовать без изменения атомарности или дизайна PQ proofs.

Cross-Data-Space workflow (пример)
1) Пользователь отправляет AMX транзакцию, затрагивающую публичный DS P и частный DS S: переместить asset X из S к бенефициару B, чей аккаунт находится в P.
2) В пределах слота P и S исполняют свои фрагменты на snapshot слота. S проверяет авторизацию и доступность, обновляет внутреннее состояние и производит PQ validity proof и DA commitment (приватные данные не утекут). P готовит соответствующее обновление состояния (например, mint/burn/locking в P по политике) и свою proof.
3) Nexus комитет проверяет обе DS proofs и DA сертификаты; если оба валидны в пределах слота, транзакция коммитится атомарно в 1 s Nexus Block, обновляя оба DS roots в глобальном world state векторе.
4) Если какая-либо proof или DA сертификат отсутствует/невалиден, транзакция абортируется (без эффектов), и клиент может повторить на следующий слот. Ни на одном шаге приватные данные не покидают S.

- Соображения безопасности
- Детерминированное исполнение: IVM syscalls остаются детерминированными; результаты cross-DS определяются AMX commit и финальностью, а не wall-clock или сетевыми таймингами.
- Контроль доступа: ISI разрешения в частных DS ограничивают, кто может отправлять транзакции и какие операции разрешены. Capability tokens кодируют тонкие права для cross-DS использования.
- Конфиденциальность: end-to-end шифрование для данных частных DS, erasure-coded shards хранятся только среди авторизованных членов, опциональные ZK proofs для внешних attestations.
- Устойчивость к DoS: изоляция на слоях mempool/consensus/storage предотвращает влияние публичной перегрузки на прогресс частных DS.

Изменения в компонентах Iroha
- iroha_data_model: добавить `DataSpaceId`, DS-квалифицированные идентификаторы, AMX дескрипторы (read/write sets), типы proof/DA commitments. Только Norito сериализация.
- ivm: Поверхность ABI v1 фиксирована (без новых syscalls/pointer-ABI типов); AMX/runtime upgrades используют существующие примитивы v1; закрепить ABI goldens.
- iroha_core: реализовать nexus scheduler, Space Directory, AMX routing/validation, DS artifact verification и policy enforcement для DA sampling и quotas.
- Space Directory и manifest loaders: протянуть метаданные FMS endpoints (и другие common-good service descriptors) через парсинг DS manifest, чтобы узлы автоматически находили локальные сервисные endpoints при присоединении к DS.
- kura: blob store с erasure coding, commitments, retrieval APIs с учетом private/public политик.
- WSV: snapshotting, chunking, commitments; proof APIs; интеграция с AMX конфликтным детектом и верификацией.
- irohad: роли узлов, сетевое взаимодействие для DA, membership/authentication частных DS, конфигурация через `iroha_config` (без env toggles в production путях).

Конфигурация и детерминизм
- Все runtime поведение конфигурируется через `iroha_config` и прокидывается через constructors/hosts. Никаких env toggles в production.
- Аппаратное ускорение (SIMD/NEON/METAL/CUDA) опционально и feature-gated; детерминированные fallbacks должны давать идентичный результат на разных hardware.
- - Постквантовый дефолт: все DS должны использовать PQ validity proofs (STARK/FRI) и ML-DSA-87 для DS QC по умолчанию. Альтернативы требуют явного указания в DS Manifest и policy approval.

### Управление жизненным циклом runtime lanes

- **Admin endpoint:** `POST /v1/nexus/lifecycle` (Torii) принимает Norito/JSON тело с `additions` (полные объекты `LaneConfig`) и `retire` (lane ids) для добавления или удаления lanes без рестарта. Запросы gated на `nexus.enabled=true` и используют ту же Nexus configuration/state view, что и очередь.
- **Behaviour:** при успехе узел применяет lifecycle план к WSV/Kura metadata, пересобирает queue routing/limits/manifests и отвечает `{ ok: true, lane_count: <u32> }`. Планы с ошибками валидации (неизвестные retire ids, дубликаты aliases/ids, Nexus отключен) возвращают `400 Bad Request` с `lane_lifecycle_error`.
- **Safety:** handler использует shared state view lock, чтобы избежать гонок с читателями при обновлении каталогов; вызывающие все равно должны сериализовать lifecycle обновления внешне, чтобы избежать конфликтующих планов.
- **Распространение:** Роутинг/лимиты очереди и lane-манифесты пересобираются из обновленного каталога, а воркеры consensus/DA/RBC читают актуальный lane config через снапшоты состояния, поэтому планирование и выбор валидаторов переключаются без рестарта (in-flight работа завершается по прежней конфигурации).
- **Очистка хранилища:** Геометрия Kura и tiered-WSV согласуется (создать/вывести/переименовать), маппинги курсоров DA-шардов синхронизируются/сохраняются, а retired lanes удаляются из кешей lane relay и хранилищ DA commitments/confidential-compute/pin-intent.

Путь миграции (Iroha 2 -> Iroha 3)
1) Ввести dataspace-qualified IDs и nexus block/global state composition в data model; добавить feature flags для режима совместимости Iroha 2 на время перехода.
2) Реализовать Kura/WSV erasure-coding backends за feature flags, сохранив текущие backends как дефолт в ранних фазах.
3) Сохранить поверхность ABI v1 фиксированной; реализовать AMX без новых syscalls/pointer типов и обновить тесты/документы без изменения ABI.
4) Доставить минимальный nexus chain с одним публичным DS и 1 s блоками; затем добавить первый частный DS pilot, экспортирующий только proofs/commitments.
5) Расширить до полного AMX (atomic cross-DS) с DS-local FASTPQ-ISI proofs и DA attesters; включить ML-DSA-87 QCs на всех DS.

Стратегия тестирования
- Unit tests для типов data model, Norito roundtrips, поведения AMX syscalls, encoding/decoding proofs.
- IVM tests для закрепления ABI v1 goldens (список syscalls, хеш ABI, ID pointer type).
- Интеграционные тесты для atomic cross-DS транзакций (positive/negative), целей DA attester latency (<=300 ms) и performance isolation под нагрузкой.
- Security tests для DS QC verification (ML-DSA-87), конфликтов/abort semantics, и предотвращения утечек конфиденциальных shards.

### NX-18 Telemetry & Runbook Assets

- **Grafana board:** `dashboards/grafana/nexus_lanes.json` теперь экспортирует дашборд "Nexus Lane Finality & Oracles", запрошенный NX-18. Панели покрывают `histogram_quantile()` по `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, предупреждения доступности DA (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), gauges цены/устаревания/TWAP/haircut оракулов и живую панель `iroha_settlement_buffer_xor`, чтобы операторы могли доказать 1 s slot, DA и treasury SLO без bespoke запросов.
- **CI gate:** `scripts/telemetry/check_slot_duration.py` парсит Prometheus snapshots, печатает p50/p95/p99 и применяет пороги NX-18 (p95 <= 1000 ms, p99 <= 1100 ms). Сопутствующий `scripts/telemetry/nx18_acceptance.py` проверяет DA quorum, staleness/TWAP/haircuts оракулов, settlement buffers и slot quantiles за один прогон (`--json-out` сохраняет evidence), и оба запускаются внутри `ci/check_nexus_lane_smoke.sh` для RC.
- **Evidence bundler:** `scripts/telemetry/bundle_slot_artifacts.py` копирует metrics snapshot + JSON summary в `artifacts/nx18/` и пишет `slot_bundle_manifest.json` с SHA-256 digest, гарантируя, что каждый RC загрузит ровно те артефакты, что прошли gate NX-18.
- **Release automation:** `scripts/run_release_pipeline.py` теперь вызывает `ci/check_nexus_lane_smoke.sh` (пропуск через `--skip-nexus-lane-smoke`) и копирует `artifacts/nx18/` в release output, чтобы evidence NX-18 шла вместе с bundle/image artifacts без ручного шага.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` документирует on-call workflow (пороги, шаги инцидента, захват evidence, chaos drills), сопровождающий дашборд, закрывая пункт NX-18 "publish operator dashboards/runbooks".
- **Telemetry helpers:** используйте `scripts/telemetry/compare_dashboards.py` для diff экспортируемых дашбордов (предотвращает staging/prod drift) и `scripts/telemetry/check_nexus_audit_outcome.py` во время routed-trace или chaos rehearsal, чтобы каждый NX-18 drill архивировал соответствующий payload `nexus.audit.outcome`.

Открытые вопросы (требуют уточнения)
1) Подписи транзакций: Решение - конечные пользователи свободны выбирать любой алгоритм подписи, объявленный целевым DS (Ed25519, secp256k1, ML-DSA и т.д.). Hosts должны применять flags мультисиг/curve в манифестах, обеспечивать детерминированные fallback и документировать латентность при смешении алгоритмов. Остается: финализировать capability negotiation flow для Torii/SDKs и обновить admission tests.
2) Gas экономика: каждый DS может номинировать gas в локальном токене, тогда как глобальные settlement fees платятся в SORA XOR. Остается: определить стандартный путь конверсии (public-lane DEX vs другие источники ликвидности), hooks бухгалтерии леджера и safeguards для DS, которые субсидируют или ставят нулевую цену.
3) DA attesters: целевое число per region и порог (например, 64 sampled, 43-of-64 ML-DSA-87 signatures) для <=300 ms при сохранении долговечности. Какие регионы обязательно включить с первого дня?
4) DA параметры по умолчанию: предлагаем public DS `k=32, m=16` и private DS `k=16, m=8`. Нужен ли более высокий профиль избыточности (например, `k=30, m=20`) для некоторых классов DS?
5) Granularity DS: домены и активы могут быть DS. Стоит ли поддерживать иерархические DS (domain DS как родитель asset DS) с опциональным наследованием политик или оставить плоско для v1?
6) Тяжелые ISI: для сложных ISI, которые не могут дать субсекундные proofs, следует (a) отвергать, (b) дробить на более мелкие атомарные шаги между блоками, или (c) разрешать отложенное включение с явными флагами?
7) Cross-DS конфликты: достаточно ли read/write set, объявленного клиентом, или host должен автоматически выводить и расширять его для безопасности (ценой большего числа конфликтов)?

Приложение: соответствие политикам репозитория
- Norito используется для всех wire форматов и JSON сериализации через Norito helpers.
- Только ABI v1; без runtime toggles для ABI политики. Поверхности syscalls и pointer-types фиксированы и закреплены golden tests.
- Детерминизм сохраняется между hardware; ускорение опционально и gated.
- Никакого serde в production путях; никакой env-based конфигурации в production.
