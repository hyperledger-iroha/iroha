---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-spec
title: Техническая спецификация Sora Nexus
description: Полное отражение `docs/source/nexus.md`, охватывающее архитектуру и ограничения проектирования для Iroha 3 (Sora Nexus).
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus.md`. Держите обе копии синхронизированными, пока переводческий бэклог не попадет в портал.
:::

#! Iroha 3 - Sora Nexus Ledger: Техническая спецификация

Этот документ предлагает архитектуру Sora Nexus Ledger для Iroha 3, развивая Iroha 2 в единый глобальный логически унифицированный ledger, организованный вокруг Data Spaces (DS). Data Spaces предоставляют сильные домены приватности ("private data spaces") и открытую участие ("public data spaces"). Дизайн сохраняет composability глобального ledger при строгой изоляции и конфиденциальности данных private-DS и вводит масштабирование доступности данных через стирающее кодирование в Kura (block storage) и WSV (World State View).

Один и тот же репозиторий собирает Iroha 2 (self-hosted сети) и Iroha 3 (SORA Nexus). Исполнение обеспечивается общей Iroha Virtual Machine (IVM) и toolchain Kotodama, поэтому контракты и артефакты байткода остаются переносимыми между self-hosted развертываниями и глобальным ledger Nexus.

Цели
- Один глобальный логический ledger, составленный из множества кооперативных валидаторов и Data Spaces.
- Private Data Spaces для permissioned операций (например, CBDC), где данные никогда не покидают private DS.
- Public Data Spaces с открытым участием, permissionless доступом в стиле Ethereum.
- Composable смарт-контракты между Data Spaces при явных разрешениях на доступ к private-DS активам.
- Изоляция производительности, чтобы public активность не ухудшала private-DS внутренние транзакции.
- Доступность данных в масштабе: erasure-coded Kura и WSV для поддержки практически неограниченных данных при сохранении приватности private-DS.

Не цели (начальная фаза)
- Определение токеномики или стимулов валидаторов; политики scheduling и staking должны оставаться подключаемыми.
- Введение новой версии ABI; изменения нацелены на ABI v1 с явными расширениями syscalls и pointer-ABI по политике IVM.

Термины
- Nexus Ledger: Глобальный логический ledger, сформированный композицией блоков Data Space (DS) в единую упорядоченную историю и commitment состояния.
- Data Space (DS): Ограниченный домен исполнения и хранения со своими валидаторами, governance, классом приватности, DA политикой, квотами и политикой комиссий. Существуют два класса: public DS и private DS.
- Private Data Space: Permissioned валидаторы и контроль доступа; данные транзакций и состояния никогда не выходят за DS. Глобально якорятся только commitments/metadata.
- Public Data Space: Permissionless участие; полные данные и состояние публичны.
- Data Space Manifest (DS Manifest): Norito-encoded manifest, декларирующий параметры DS (validators/QC keys, класс приватности, ISI политика, DA параметры, retention, quotas, ZK политика, комиссии). Hash manifest anchoring на nexus chain. Если не указано иначе, DS quorum certificates используют ML-DSA-87 (Dilithium5-класс) как дефолтный пост-квантовый подпись.
- Space Directory: Глобальный on-chain directory контракт, отслеживающий DS manifests, версии и governance/rotation события для разрешимости и аудитов.
- DSID: Глобально уникальный идентификатор Data Space. Используется для namespacing всех объектов и ссылок.
- Anchor: Криптографический commitment от DS блока/заголовка, включаемый в nexus chain, чтобы связать историю DS с глобальным ledger.
- Kura: Хранилище блоков Iroha. Здесь расширяется erasure-coded blob storage и commitments.
- WSV: Iroha World State View. Здесь расширяется версионированными, snapshot-capable, erasure-coded сегментами состояния.
- IVM: Iroha Virtual Machine для исполнения смарт-контрактов (Kotodama bytecode `.to`).
  - AIR: Algebraic Intermediate Representation. Алгебраическое представление вычислений для STARK-подобных доказательств, описывающее исполнение как field-based трассы с переходными и граничными ограничениями.

Модель Data Spaces
- Идентичность: `DataSpaceId (DSID)` идентифицирует DS и namespace-ит все. DS могут быть инстанцированы в двух гранулярностях:
  - Domain-DS: `ds::domain::<domain_name>` - исполнение и состояние, ограниченные доменом.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - исполнение и состояние, ограниченные одной дефиницией актива.
  Обе формы сосуществуют; транзакции могут атомарно касаться нескольких DSID.
- Жизненный цикл manifest: создание DS, обновления (ротация ключей, изменения политики) и вывод из эксплуатации записываются в Space Directory. Каждый per-slot DS артефакт ссылается на последний manifest hash.
- Классы: Public DS (открытое участие, публичная DA) и Private DS (permissioned, конфиденциальная DA). Гибридные политики возможны через флаги manifest.
- Политики на DS: ISI permissions, DA параметры `(k,m)`, шифрование, retention, квоты (min/max доля tx на блок), ZK/optimistic proof политика, комиссии.
- Governance: членство DS и ротация валидаторов определяются секцией governance manifest (on-chain предложения, multisig или внешняя governance, заякоренная транзакциями nexus и attestations).

Capability manifests и UAID
- Universal accounts: каждый участник получает детерминированный UAID (`UniversalAccountId` в `crates/iroha_data_model/src/nexus/manifest.rs`), который охватывает все dataspaces. Capability manifests (`AssetPermissionManifest`) связывают UAID с конкретным dataspace, epoch активации/истечения и упорядоченным списком allow/deny `ManifestEntry` правил, ограничивающих `dataspace`, `program_id`, `method`, `asset` и опциональные AMX роли. Deny правила всегда побеждают; evaluator возвращает `ManifestVerdict::Denied` с причиной аудита или `Allowed` grant с соответствующей allowance metadata.
- Allowances: каждая allow запись несет детерминированные `AllowanceWindow` buckets (`PerSlot`, `PerMinute`, `PerDay`) и опциональный `max_amount`. Hosts и SDK потребляют один и тот же Norito payload, поэтому enforcement одинаков на разном железе и SDK.
- Audit telemetry: Space Directory транслирует `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) при изменении состояния manifest. Новая поверхность `SpaceDirectoryEventFilter` позволяет Torii/data-event подписчикам отслеживать обновления UAID manifests, ревокации и deny-wins решения без кастомного plumbing.

Для end-to-end операционных доказательств, SDK migration notes и checklist публикации manifest синхронизируйте этот раздел с Universal Account Guide (`docs/source/universal_accounts_guide.md`). Держите оба документа в соответствии при изменениях политики или инструментов UAID.

Архитектура верхнего уровня
1) Глобальный композиционный слой (Nexus Chain)
- Поддерживает единый canonical порядок 1-секундных Nexus Blocks, которые финализируют атомарные транзакции, затрагивающие один или более Data Spaces (DS). Каждая committed транзакция обновляет единый глобальный world state (вектор per-DS roots).
- Содержит минимальные метаданные плюс агрегированные proofs/QCs для composability, finality и fraud detection (затронутые DSIDs, per-DS state roots до/после, DA commitments, per-DS validity proofs и DS quorum certificate с ML-DSA-87). Приватные данные не включаются.
- Консенсус: единый глобальный pipelined BFT committee размера 22 (3f+1 с f=7), выбранный по epoch VRF/stake из пула до ~200k кандидатов. Nexus committee упорядочивает транзакции и финализирует блок за 1s.

2) Слой Data Space (Public/Private)
- Исполняет per-DS фрагменты глобальных транзакций, обновляет локальный DS WSV и производит per-block validity artifacts (aggregated per-DS proofs и DA commitments), которые сворачиваются в 1-секундный Nexus Block.
- Private DS шифруют данные at-rest и in-flight среди авторизованных валидаторов; наружу выходят только commitments и PQ validity proofs.
- Public DS экспортируют полные data bodies (via DA) и PQ validity proofs.

3) Атомарные cross-Data-Space транзакции (AMX)
- Модель: каждая пользовательская транзакция может касаться нескольких DS (например, domain DS и один или несколько asset DS). Она коммитится атомарно в одном Nexus Block или откатывается; частичных эффектов нет.
- Prepare-Commit за 1s: для каждой кандидатной транзакции затронутые DS параллельно исполняются на одном snapshot (DS roots начала слота) и выдают per-DS PQ validity proofs (FASTPQ-ISI) и DA commitments. Nexus committee коммитит транзакцию только если все требуемые DS proofs проверяются и DA certificates приходят вовремя (цель <=300 ms); иначе транзакция переносится на следующий слот.
- Consistency: read-write наборы объявляются; конфликт детектируется при commit против start-of-slot roots. Optimistic lock-free исполнение по DS избегает глобальных stall; atomicity обеспечивается правилом nexus commit (all-or-nothing по DS).
- Privacy: private DS экспортируют только proofs/commitments, привязанные к pre/post DS roots. Никакие сырые private данные не покидают DS.

4) Data Availability (DA) с erasure coding
- Kura хранит block bodies и WSV snapshots как erasure-coded blobs. Публичные blobs широко shard-ятся; private blobs хранятся только внутри private-DS validators, с зашифрованными chunks.
- DA commitments записываются и в DS artifacts, и в Nexus Blocks, позволяя sampling и recovery guarantees без раскрытия приватного содержимого.

Структура блока и commit
- Data Space Proof Artifact (на слот 1s, на DS)
  - Поля: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS экспортируют artifacts без data bodies; public DS позволяют получение body через DA.

- Nexus Block (cadence 1s)
  - Поля: block_number, parent_hash, slot_time, tx_list (атомарные cross-DS транзакции с DSIDs), ds_artifacts[], nexus_qc.
  - Функция: финализирует все атомарные транзакции, чьи DS artifacts проверяются; обновляет глобальный вектор DS roots одним шагом.

Консенсус и scheduling
- Nexus Chain Consensus: единый глобальный pipelined BFT (Sumeragi-class) с комитетом 22 (3f+1, f=7) для блоков 1s и finality 1s. Комитет выбирается по epochs через VRF/stake из ~200k кандидатов; ротация поддерживает децентрализацию и цензуроустойчивость.
- Data Space Consensus: каждый DS запускает свой BFT для per-slot artifacts (proofs, DA commitments, DS QC). Lane-relay committees размером `3f+1` используют `fault_tolerance` dataspace и детерминированно сэмплируются по epoch из пула валидаторов dataspace с VRF seed, привязанным к `(dataspace_id, lane_id)`. Private DS - permissioned; public DS - open liveness с anti-Sybil политиками. Глобальный nexus committee неизменен.
- Transaction Scheduling: пользователи отправляют атомарные транзакции с DSIDs и read-write наборами. DS исполняют параллельно в слот; nexus committee включает транзакцию в 1s блок, если все DS artifacts проверены и DA certificates приходят вовремя (<=300 ms).
- Performance Isolation: у каждого DS отдельные mempools и исполнение. Per-DS quotas ограничивают число транзакций на блок для данного DS, предотвращая head-of-line blocking и защищая latency private DS.

Data model и namespacing
- DS-qualified IDs: все сущности (domains, accounts, assets, roles) квалифицированы `dsid`. Пример: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global References: глобальная ссылка - кортеж `(dsid, object_id, version_hint)` и может быть размещена on-chain в nexus layer или в AMX descriptors для cross-DS.
- Norito Serialization: все cross-DS сообщения (AMX descriptors, proofs) используют Norito codecs. Serde в продакшене не применяется.

Smart Contracts и расширения IVM
- Execution Context: добавить `dsid` в контекст исполнения IVM. Kotodama контракты всегда исполняются в конкретном Data Space.
- Atomic Cross-DS Primitives:
  - `amx_begin()` / `amx_commit()` ограничивают атомарную multi-DS транзакцию в IVM host.
  - `amx_touch(dsid, key)` объявляет read/write intent для conflict detection против snapshot roots слота.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (операция разрешена только при политическом разрешении и валидном handle)
- Asset Handles and Fees:
  - Операции активов авторизуются политиками ISI/role DS; комиссии оплачиваются в gas токене DS. Optional capability tokens и richer policy (multi-approver, rate-limits, geofencing) можно добавить позже без изменения атомарной модели.
- Determinism: все новые syscalls чистые и детерминированные при данных входах и заявленных AMX read/write наборах. Нет скрытых эффектов времени или окружения.

Post-Quantum Validity Proofs (Generalized ISIs)
- FASTPQ-ISI (PQ, no trusted setup): hash-based аргумент, обобщающий transfer дизайн на все ISI, с целью субсекундного proving для batch масштаба 20k на GPU-классе железа.
  - Operational profile:
    - Production nodes строят prover через `fastpq_prover::Prover::canonical`, который теперь всегда инициализирует production backend; детерминированный mock удален. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) и `irohad --fastpq-execution-mode` позволяют операторам детерминированно фиксировать CPU/GPU исполнение, а observer hook фиксирует requested/resolved/backend тройки для fleet audits. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmetization:
  - KV-Update AIR: трактует WSV как типизированную key-value карту, зафиксированную через Poseidon2-SMT. Каждый ISI разворачивается в небольшой набор read-check-write строк по ключам (accounts, assets, roles, domains, metadata, supply).
  - Opcode-gated constraints: единая AIR таблица с selector columns enforce-ит правила по ISI (conservation, monotonic counters, permissions, range checks, bounded metadata updates).
  - Lookup arguments: прозрачные hash-committed таблицы для permissions/roles, asset precisions и policy параметров избегают тяжелых bitwise ограничений.
- State commitments and updates:
  - Aggregated SMT Proof: все затронутые ключи (pre/post) доказываются против `old_root`/`new_root` с компрессированным frontier и deduped siblings.
  - Invariants: глобальные инварианты (например, total supply per asset) enforce-ятся через multiset equality между effect rows и tracked counters.
- Proof system:
  - FRI-style polynomial commitments (DEEP-FRI) с высокой arity (8/16) и blow-up 8-16; Poseidon2 hashes; Fiat-Shamir transcript с SHA-2/3.
  - Optional recursion: DS-local recursive aggregation для сжатия micro-batches в одну proof на слот при необходимости.
- Scope и примеры:
  - Assets: transfer, mint, burn, register/unregister asset definitions, set precision (bounded), set metadata.
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories (state-only; проверки подписи аттестуются DS validators, не доказываются в AIR).
  - Roles/Permissions (ISI): grant/revoke roles и permissions; enforced lookup tables и monotonic policy checks.
  - Contracts/AMX: AMX begin/commit markers, capability mint/revoke при включении; доказываются как state transitions и policy counters.
- Out-of-AIR checks для сохранения latency:
  - Подписи и тяжелая криптография (например, ML-DSA user signatures) проверяются DS validators и аттестуются в DS QC; validity proof покрывает только state consistency и policy compliance. Это оставляет proofs PQ и быстрыми.
- Performance targets (иллюстративно, 32-core CPU + современная GPU):
  - 20k mixed ISIs с небольшим key-touch (<=8 keys/ISI): ~0.4-0.9 s prove, ~150-450 KB proof, ~5-15 ms verify.
  - Более тяжелые ISI: micro-batch (например, 10x2k) + recursion, чтобы держать <1 s на слот.
- DS Manifest конфигурация:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (подписи проверяются DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (по умолчанию; альтернативы должны быть явно объявлены)
- Fallbacks:
  - Сложные/кастомные ISI могут использовать общий STARK (`zk.policy = "stark_fri_general"`) с deferred proof и 1s finality через QC аттестацию + slashing на неверных доказательствах.
  - Non-PQ варианты (например, Plonk с KZG) требуют trusted setup и больше не поддерживаются в дефолтной сборке.

AIR Primer (для Nexus)
- Execution trace: матрица с шириной (register columns) и длиной (steps). Каждая строка - логический шаг ISI; столбцы держат pre/post значения, selector и flags.
- Constraints:
  - Transition constraints: enforce отношения между строками (например, post_balance = pre_balance - amount для debit строки при `sel_transfer = 1`).
  - Boundary constraints: привязывают public I/O (old_root/new_root, counters) к первой/последней строкам.
  - Lookups/permutations: гарантируют membership и multiset equality против committed таблиц (permissions, asset params) без тяжелых битовых схем.
- Commitment and verification:
  - Prover коммитит трассы hash-based кодировками и строит low-degree polynomials, валидные только при соблюдении ограничений.
  - Verifier проверяет low-degree через FRI (hash-based, post-quantum) с несколькими Merkle открытиями; стоимость логарифмична по steps.
- Example (Transfer): регистры включают pre_balance, amount, post_balance, nonce и selectors. Ограничения enforce неотрицательность/диапазон, сохранение и монотонность nonce, а aggregated SMT multi-proof связывает pre/post leaves с old/new roots.

Эволюция ABI и syscalls (ABI v1)
- Syscalls к добавлению (иллюстративные имена):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Pointer-ABI типы к добавлению:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Требуемые обновления:
  - Добавить в `ivm::syscalls::abi_syscall_list()` (сохранять порядок), gate по политике.
  - Мапить неизвестные номера в `VMError::UnknownSyscall` в hosts.
  - Обновить тесты: syscall list golden, ABI hash, pointer type ID goldens и policy tests.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Модель приватности
- Private Data Containment: тела транзакций, диффы состояния и WSV snapshots для private DS никогда не покидают private validator subset.
- Public Exposure: экспортируются только headers, DA commitments и PQ validity proofs.
- Optional ZK Proofs: private DS могут выпускать ZK proofs (например, достаточный баланс, соблюдение политики), позволяя cross-DS действия без раскрытия внутреннего состояния.
- Access Control: авторизация enforced политиками ISI/role внутри DS. Capability tokens опциональны и могут быть добавлены позже.

Изоляция производительности и QoS
- Отдельные consensus, mempools и storage на DS.
- Nexus scheduling quotas per DS ограничивают время включения anchors и предотвращают head-of-line blocking.
- Контрактные resource budgets per DS (compute/memory/IO) enforced IVM host-ом. Public-DS contention не может съедать private-DS бюджеты.
- Асинхронные cross-DS вызовы избегают длинных синхронных ожиданий внутри private-DS исполнения.

Data Availability и storage design
1) Erasure Coding
- Использовать systematic Reed-Solomon (например, GF(2^16)) для blob-level erasure coding Kura blocks и WSV snapshots: параметры `(k, m)` с `n = k + m` shards.
- Дефолтные параметры (public DS): `k=32, m=16` (n=48), восстановление до 16 потерянных shards при ~1.5x expansion. Для private DS: `k=16, m=8` (n=24) внутри permissioned набора. Оба конфигурируются per DS Manifest.
- Public blobs: shards распределены по множеству DA nodes/validators с sampling-based availability checks. DA commitments в headers позволяют light clients проверять.
- Private blobs: shards зашифрованы и распределены только внутри private-DS validators (или назначенных custodians). Глобальная цепочка несет только DA commitments (без shard locations или ключей).

2) Commitments и sampling
- Для каждого blob: вычислить Merkle root по shards и включить в `*_da_commitment`. Сохранять PQ, избегая elliptic-curve commitments.
- DA Attesters: VRF-sampled региональные attesters (например, 64 на регион) выдают ML-DSA-87 сертификат об успешном sampling shards. Целевая DA attestation latency <=300 ms. Nexus committee валидирует сертификаты вместо вытягивания shards.

3) Kura Integration
- Blocks хранят transaction bodies как erasure-coded blobs с Merkle commitments.
- Headers несут blob commitments; bodies извлекаются через DA сеть для public DS и через private каналы для private DS.

4) WSV Integration
- WSV Snapshotting: периодически checkpoint DS состояния в chunked, erasure-coded snapshots с commitments в headers. Между snapshots поддерживаются change logs. Public snapshots широко shard-ятся; private snapshots остаются в private validators.
- Proof-Carrying Access: контракты могут предоставлять (или запрашивать) state proofs (Merkle/Verkle), anchored snapshot commitments. Private DS могут выдавать zero-knowledge attestations вместо raw proofs.

5) Retention и pruning
- Нет pruning для public DS: хранить все Kura bodies и WSV snapshots через DA (горизонтальное масштабирование). Private DS могут определить внутреннюю retention, но экспортированные commitments остаются неизменными. Nexus layer хранит все Nexus Blocks и DS artifact commitments.

Сеть и роли узлов
- Global Validators: участвуют в nexus consensus, валидируют Nexus Blocks и DS artifacts, выполняют DA checks для public DS.
- Data Space Validators: запускают DS consensus, исполняют контракты, управляют локальным Kura/WSV, обрабатывают DA для своего DS.
- DA Nodes (опционально): хранят/публикуют public blobs, помогают sampling. Для private DS DA nodes co-located с validators или доверенными custodians.

Системные улучшения и соображения
- Sequencing/mempool decoupling: принять DAG mempool (например, Narwhal-стиль), питающий pipelined BFT на nexus layer, чтобы снизить latency и повысить throughput без изменения логической модели.
- DS quotas и fairness: per-DS per-block quotas и weight caps предотвращают head-of-line blocking и обеспечивают предсказуемую latency для private DS.
- DS attestation (PQ): DS quorum certificates используют ML-DSA-87 (Dilithium5-класс) по умолчанию. Это пост-квантово и больше, чем EC подписи, но приемлемо для 1 QC на слот. DS могут явно выбрать ML-DSA-65/44 (меньше) или EC подписи, если объявлено в DS Manifest; public DS настоятельно рекомендуется сохранять ML-DSA-87.
- DA attesters: для public DS использовать VRF-sampled региональных attesters, выдающих DA certificates. Nexus committee валидирует сертификаты вместо raw shard sampling; private DS держат DA attestations внутренними.
- Recursion и epoch proofs: опционально агрегировать несколько micro-batches в DS в одно recursive proof на слот/эпоху для стабильного размера proofs и времени verify при высокой нагрузке.
- Lane scaling (если нужно): если единый глобальный committee становится узким местом, ввести K параллельных sequencing lanes с детерминированным merge. Это сохраняет единый глобальный порядок и масштабируется горизонтально.
- Deterministic acceleration: предоставить SIMD/CUDA kernels под feature flags для hashing/FFT с bit-exact CPU fallback, чтобы сохранить детерминизм на разном железе.
- Lane activation thresholds (proposal): включать 2-4 lanes, если (a) p95 finality превышает 1.2 s более 3 минут подряд, или (b) per-block occupancy превышает 85% более 5 минут, или (c) входящий tx rate требует >1.2x block capacity устойчиво. Lanes детерминированно bucket-ят транзакции по DSID hash и merge-ятся в nexus block.

Fees и экономика (начальные дефолты)
- Gas unit: per-DS gas token с метered compute/IO; fees оплачиваются в нативном gas asset DS. Конверсия между DS - ответственность приложения.
- Inclusion priority: round-robin по DS с per-DS quotas для fairness и 1s SLO; внутри DS fee bidding может разруливать.
- Future: возможны глобальный fee market или MEV-minimizing политики без изменения atomicity или PQ proof дизайна.

Cross-Data-Space workflow (пример)
1) Пользователь отправляет AMX транзакцию, затрагивающую public DS P и private DS S: переместить asset X из S к beneficiary B, чей аккаунт в P.
2) Внутри слота P и S исполняют свои фрагменты на snapshot. S проверяет authorization и availability, обновляет внутреннее состояние и формирует PQ validity proof и DA commitment (без утечки private data). P готовит соответствующее обновление состояния (например, mint/burn/locking в P по политике) и свое proof.
3) Nexus committee проверяет обе DS proofs и DA certificates; если обе валидны в слоте, транзакция commit-ится атомарно в 1s Nexus Block, обновляя оба DS roots в глобальном world state векторе.
4) Если какой-либо proof или DA certificate отсутствует/невалиден, транзакция abort-ится (без эффектов), и клиент может переслать на следующий слот. Никакие private данные S не покидают на любом шаге.

- Security Considerations
- Deterministic Execution: IVM syscalls остаются детерминированными; cross-DS результаты определяются AMX commit и finality, а не wall-clock или сетевыми таймингами.
- Access Control: ISI permissions в private DS ограничивают, кто может отправлять транзакции и какие операции разрешены. Capability tokens кодируют fine-grained права для cross-DS.
- Confidentiality: end-to-end шифрование для private-DS данных, erasure-coded shards хранятся только у авторизованных участников, опциональные ZK proofs для внешних attestations.
- DoS Resistance: изоляция на уровнях mempool/consensus/storage предотвращает влияние public congestion на прогресс private DS.

Изменения в компонентах Iroha
- iroha_data_model: добавить `DataSpaceId`, DS-qualified IDs, AMX descriptors (read/write sets), типы proof/DA commitments. Только Norito сериализация.
- ivm: добавить syscalls и pointer-ABI types для AMX (`amx_begin`, `amx_commit`, `amx_touch`) и DA proofs; обновить ABI tests/docs по политике v1.
