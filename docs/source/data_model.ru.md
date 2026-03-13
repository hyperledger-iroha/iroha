---
lang: ru
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha Модель данных v2 – глубокое погружение

В этом документе описываются структуры, идентификаторы, характеристики и протоколы, которые формируют модель данных Iroha v2, реализованную в крейте `iroha_data_model` и используемую в рабочей области. Он предназначен для точного справочника, который вы можете просмотреть и предложить обновления.

## Область применения и основы

- Цель: предоставить канонические типы для объектов домена (домены, учетные записи, активы, NFT, роли, разрешения, одноранговые узлы), инструкции по изменению состояния (ISI), запросы, триггеры, транзакции, блоки и параметры.
- Сериализация: все общедоступные типы наследуют кодеки Norito (`norito::codec::{Encode, Decode}`) и схему (`iroha_schema::IntoSchema`). JSON используется выборочно (например, для полезных данных HTTP и `Json`) за флагами функций.
- Примечание IVM: некоторые проверки во время десериализации отключены при настройке виртуальной машины Iroha (IVM), поскольку хост выполняет проверку перед вызовом контрактов (см. документацию по ящикам в `src/lib.rs`).
- Ворота FFI: некоторые типы условно аннотированы для FFI через `iroha_ffi` после `ffi_export`/`ffi_import`, чтобы избежать накладных расходов, когда FFI не требуется.

## Основные черты и помощники- `Identifiable`: объекты имеют стабильные `Id` и `fn id(&self) -> &Self::Id`. Должен быть получен с помощью `IdEqOrdHash` для удобства отображения/набора.
- `Registrable`/`Registered`: многие объекты (например, `Domain`, `AssetDefinition`, `Role`) используют шаблон построителя. `Registered` связывает тип среды выполнения с упрощенным типом компоновщика (`With`), подходящим для транзакций регистрации.
- `HasMetadata`: унифицированный доступ к карте ключ/значение `Metadata`.
- `IntoKeyValue`: помощник разделения хранилища для хранения `Key` (ID) и `Value` (данные) отдельно для уменьшения дублирования.
- `Owned<T>`/`Ref<'world, K, V>`: облегченные оболочки, используемые в хранилищах и фильтрах запросов, чтобы избежать ненужных копий.

## Имена и идентификаторы- `Name`: действительный текстовый идентификатор. Запрещает пробелы и зарезервированные символы `@`, `#`, `$` (используются в составных идентификаторах). Можно построить через `FromStr` с проверкой. При анализе имена нормализуются до Unicode NFC (канонически эквивалентные варианты написания считаются идентичными и сохраняются в составе). Специальное имя `genesis` зарезервировано (проверяется без учета регистра).
- `IdBox`: конверт типа суммы для любого поддерживаемого идентификатора (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Полезно для общих потоков и кодирования Norito как одного типа.
- `ChainId`: непрозрачный идентификатор цепочки, используемый для защиты от повтора в транзакциях.Строковые формы идентификаторов (возможны двусторонние действия с `Display`/`FromStr`):
- `DomainId`: `name` (например, `wonderland`).
- `AccountId`: канонический идентификатор учетной записи без домена, закодированный с помощью `AccountAddress` только как I105. Входные данные парсера должны быть каноническими I105; суффиксы домена (`@domain`), канонические литералы I105, литералы псевдонимов, канонические входные данные шестнадцатеричного анализатора, устаревшие полезные нагрузки `norito:` и формы анализатора учетных записей `uaid:`/`opaque:` отклоняются.
- `AssetDefinitionId`: канонический `aid:<32-lower-hex-no-dash>` (байты UUID-v4).
- `AssetId`: канонический литерал `norito:<hex>` (устаревшие текстовые формы не поддерживаются в первом выпуске).
- `NftId`: `nft$domain` (например, `rose$garden`).
- `PeerId`: `public_key` (равенство одноранговых узлов осуществляется по открытому ключу).

## Сущности

### Домен
- `DomainId { name: Name }` – уникальное имя.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Строитель: `NewDomain` с `with_logo`, `with_metadata`, затем `Registrable::build(authority)` устанавливает `owned_by`.### Аккаунт
- `AccountId` — это канонический идентификатор учетной записи без домена, заданный контроллером и закодированный как канонический I105.
- `ScopedAccountId { account: AccountId, domain: DomainId }` содержит явный контекст домена только там, где требуется ограниченное представление.
- `Account { id, metadata, label?, uaid? }` — `label` — это необязательный стабильный псевдоним, используемый записями смены ключей, `uaid` содержит необязательный общий для Nexus [универсальный идентификатор учетной записи] (./universal_accounts_guide.md).
- Строитель: `NewAccount` через `Account::new(id)`; для регистрации требуется явный домен `ScopedAccountId`, который не выводится из значений по умолчанию.

### Определения и активы активов
- `AssetDefinitionId { aid_bytes: [u8; 16] }` отображается в текстовом виде как `aid:<32-hex-no-dash>`.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `name` — это текст, отображаемый человеком, который не должен содержать `#`/`@`.
  - `alias` является необязательным и должен быть одним из:
    - `<name>#<domain>@<dataspace>`
    - `<name>#<dataspace>`
    левый сегмент точно соответствует `AssetDefinition.name`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Строители: `AssetDefinition::new(id, spec)` или удобство `numeric(id)`; `name` требуется и должен быть установлен через `.with_name(...)`.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` с удобным для хранения `AssetEntry`/`AssetValue`.
- `AssetBalanceScope`: `Global` для неограниченных балансов и `Dataspace(DataSpaceId)` для балансов с ограниченным пространством данных.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` доступен для сводных API.### NFT
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (содержимое представляет собой произвольные метаданные «ключ/значение»).
- Строитель: `NewNft` через `Nft::new(id, content)`.

### Роли и разрешения
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` со сборщиком `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – `name` и схема полезной нагрузки должны совпадать с активным `ExecutorDataModel` (см. ниже).

### Коллеги
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` и анализируемая строковая форма `public_key@address`.

### Криптографические примитивы (функция `sm`)
- `Sm2PublicKey` и `Sm2Signature`: точки, соответствующие SEC1, и подписи `r∥s` фиксированной ширины для SM2. Конструкторы проверяют принадлежность кривых и различают идентификаторы; Кодировка Norito отражает каноническое представление, используемое `iroha_crypto`.
- `Sm3Hash`: новый тип `[u8; 32]`, представляющий дайджест GM/T 0004, используемый в манифестах, телеметрии и ответах на системные вызовы.
- `Sm4Key`: 128-битная оболочка симметричного ключа, используемая совместно системными вызовами хоста и устройствами модели данных.
Эти типы располагаются рядом с существующими примитивами Ed25519/BLS/ML-DSA и становятся частью общедоступной схемы после создания рабочей области с помощью `--features sm`.### Триггеры и события
- `TriggerId { name: Name }` и `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` или `Exactly(u32)`; Утилиты заказа и истощения включены.
  - Безопасность: `TriggerCompleted` нельзя использовать в качестве фильтра действия (проверено во время (де)сериализации).
- `EventBox`: тип суммы для конвейера, конвейерного пакета, данных, времени, событий запуска и завершения триггера; `EventFilterBox` отражает это для подписок и фильтров запуска.

## Параметры и конфигурация

- Семейства системных параметров (все `Default`ed, переносы геттеров и преобразование в отдельные перечисления):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` группирует все семейства, а `custom: BTreeMap<CustomParameterId, CustomParameter>`.
— Перечисления с одним параметром: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` для обновлений и итераций, подобных различиям.
- Пользовательские параметры: определяются исполнителем, обозначаются как `Json`, идентифицируются как `CustomParameterId` (`Name`).

## ISI (Iroha Специальные инструкции)- Основная черта: `Instruction` с `dyn_encode`, `as_any` и стабильным идентификатором каждого типа `id()` (по умолчанию используется имя конкретного типа). Все инструкции `Send + Sync + 'static`.
- `InstructionBox`: принадлежащая `Box<dyn Instruction>` оболочка с клоном/eq/ord, реализованная через идентификатор типа + закодированные байты.
- Семейства встроенных инструкций организованы по следующим категориям:
  - `mint_burn`, `transfer`, `register` и пакет помощников `transparent`.
  — Введите перечисления для метапотоков: `InstructionType`, упакованные суммы, например `SetKeyValueBox` (домен/аккаунт/asset_def/nft/trigger).
- Ошибки: расширенная модель ошибок под `isi::error` (ошибки типа оценки, ошибки поиска, возможность чеканки, математика, недопустимые параметры, повторение, инварианты).
- Реестр инструкций: макрос `instruction_registry!{ ... }` создает реестр декодирования во время выполнения с ключом по имени типа. Используется клоном `InstructionBox` и сердом Norito для достижения динамической (де)сериализации. Если реестр не был явно настроен через `set_instruction_registry(...)`, встроенный реестр по умолчанию со всеми основными ISI лениво устанавливается при первом использовании, чтобы обеспечить надежность двоичных файлов.

## Транзакции- `Executable`: либо `Instructions(ConstVec<InstructionBox>)`, либо `Ivm(IvmBytecode)`. `IvmBytecode` сериализуется как base64 (прозрачный новый тип вместо `Vec<u8>`).
- `TransactionBuilder`: создает полезную нагрузку транзакции с `chain`, `authority`, `creation_time_ms`, дополнительными `time_to_live_ms` и `nonce`, `metadata` и `Executable`.
  - Помощники: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_creation_time`, `sign`.
- `SignedTransaction` (версия `iroha_version`): несет `TransactionSignature` и полезную нагрузку; обеспечивает хеширование и проверку подписи.
- Точки входа и результаты:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` с помощниками хеширования.
  - `ExecutionStep(ConstVec<InstructionBox>)`: один упорядоченный пакет инструкций в транзакции.

## Блоки- `SignedBlock` (версия) инкапсулирует:
  - `signatures: BTreeSet<BlockSignature>` (от валидаторов),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (вторичное состояние выполнения), содержащее `time_triggers`, деревья Меркла записи/результата, `transaction_results` и `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Утилиты: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Корни Меркла: точки входа и результаты транзакций фиксируются через деревья Меркла; результат Корень Меркла помещается в заголовок блока.
- Доказательства включения блоков (`BlockProofs`) предоставляют как входные/результатные доказательства Меркла, так и карту `fastpq_transcripts`, поэтому проверяющие вне цепочки могут получить дельты передачи, связанные с хэшем транзакции.
- Сообщения `ExecWitness` (передаваемые через Torii и связанные с консенсусными сплетнями) теперь включают как `fastpq_transcripts`, так и готовый к проверке `fastpq_batches: Vec<FastpqTransitionBatch>` со встроенным `public_inputs` (dsid, slot, roots, perm_root, tx_set_hash), поэтому внешние средства проверки могут принимать канонические строки FASTPQ без перекодирования транскриптов.

## Запросы- Два вкуса:
  - Единственное число: реализовать `SingularQuery<Output>` (например, `FindParameters`, `FindExecutorDataModel`).
  - Итерируемый: реализовать `Query<Item>` (например, `FindAccounts`, `FindAssets`, `FindDomains` и т. д.).
- Тип-стертые формы:
  - `QueryBox<T>` — это упакованный, стертый `Query<Item = T>` с сердом Norito, поддерживаемый глобальным реестром.
  - `QueryWithFilter<T> { query, predicate, selector }` объединяет запрос с предикатом/селектором DSL; преобразуется в стертый итерируемый запрос через `From`.
- Реестр и кодеки:
  - `query_registry!{ ... }` создает глобальный реестр, сопоставляющий конкретные типы запросов с конструкторами по имени типа для динамического декодирования.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` и `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` — это тип суммы по однородным векторам (например, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), а также кортежи и помощники расширения для эффективной нумерации страниц.
- DSL: реализован в `query::dsl` с признаками проекции (`HasProjection<PredicateMarker>` / `SelectorMarker`) для предикатов и селекторов, проверяемых во время компиляции. Функция `fast_dsl` при необходимости предоставляет более легкий вариант.

## Исполнитель и расширяемость- `Executor { bytecode: IvmBytecode }`: пакет кода, исполняемый валидатором.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` объявляет домен, определенный исполнителем:
  - Пользовательские параметры конфигурации,
  - Пользовательские идентификаторы инструкций,
  - Идентификаторы токенов разрешений,
  — Схема JSON, описывающая пользовательские типы для клиентских инструментов.
- Под номером `data_model/samples/executor_custom_data_model` существуют образцы настройки, демонстрирующие:
  - Пользовательский токен разрешения через получение `iroha_executor_data_model::permission::Permission`,
  - Пользовательский параметр, определенный как тип, конвертируемый в `CustomParameter`,
  - Пользовательские инструкции сериализуются в `CustomInstruction` для выполнения.

### CustomInstruction (ISI, определяемый исполнителем)- Тип: `isi::CustomInstruction { payload: Json }` со стабильным идентификатором провода `"iroha.custom"`.
- Назначение: конверт для инструкций, специфичных для исполнителя, в частных сетях/сетях консорциума или для прототипирования без разветвления общедоступной модели данных.
- Поведение исполнителя по умолчанию: встроенный исполнитель в `iroha_core` не выполняет `CustomInstruction` и в случае его обнаружения будет паниковать. Пользовательский исполнитель должен преобразовать `InstructionBox` в `CustomInstruction` и детерминированно интерпретировать полезную нагрузку на всех валидаторах.
- Norito: кодирует/декодирует через `norito::codec::{Encode, Decode}` с включенной схемой; Полезная нагрузка `Json` сериализуется детерминированно. Обмены туда и обратно стабильны, пока реестр инструкций включает `CustomInstruction` (он является частью реестра по умолчанию).
- IVM: Kotodama компилируется в байт-код IVM (`.to`) и является рекомендуемым путем для логики приложения. Используйте `CustomInstruction` только для расширений уровня исполнителя, которые еще не могут быть выражены в Kotodama. Обеспечьте детерминизм и идентичные двоичные файлы исполнителя на всех узлах.
- Не для публичных сетей: не используйте для публичных цепочек, где разнородные исполнители рискуют создать консенсусные разветвления. Если вам нужны функции платформы, предпочтительнее предлагать новый встроенный восходящий ISI.

## Метаданные- `Metadata(BTreeMap<Name, Json>)`: хранилище ключей/значений, прикрепленное к нескольким объектам (`Domain`, `Account`, `AssetDefinition`, `Nft`, триггерам и транзакциям).
- API: `contains`, `iter`, `get`, `insert` и (с `transparent_api`) `remove`.

## Особенности и детерминизм

- Функции управления дополнительными API (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `http`, `fault_injection`).
- Детерминизм: при любой сериализации используется кодировка Norito для переносимости между оборудованием. Байт-код IVM представляет собой непрозрачный байтовый объект; исполнение не должно приводить к недетерминированным сокращениям. Хост проверяет транзакции и детерминированно подает входные данные в IVM.

### Прозрачный API (`transparent_api`)- Цель: предоставляет полный изменяемый доступ к структурам/перечислениям `#[model]` для внутренних компонентов, таких как Torii, исполнителей и интеграционных тестов. Без него эти элементы намеренно непрозрачны, поэтому внешние SDK видят только безопасные конструкторы и закодированные полезные данные.
- Механика: макрос `iroha_data_model_derive::model` перезаписывает каждое общедоступное поле с помощью `#[cfg(feature = "transparent_api")] pub` и сохраняет частную копию для сборки по умолчанию. Включение этой функции переворачивает эти cfgs, поэтому деструктуризация `Account`, `Domain`, `Asset` и т. д. становится законной за пределами их определяющих модулей.
- Обнаружение поверхности: контейнер экспортирует константу `TRANSPARENT_API: bool` (сгенерированную в `transparent_api.rs` или `non_transparent_api.rs`). Нижестоящий код может проверять этот флаг и выполнять разветвление, когда ему необходимо вернуться к непрозрачным помощникам.
- Включение: добавьте `features = ["transparent_api"]` в зависимость в `Cargo.toml`. Ячейки рабочей области, которым требуется проекция JSON (например, `iroha_torii`), автоматически пересылают флаг, но сторонние потребители должны отключать его, если только они не контролируют развертывание и не принимают более широкую поверхность API.

## Быстрые примеры

Создайте домен и учетную запись, определите актив и создайте транзакцию с инструкциями:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "aid:2f17c72466f84a4bb8a8e24884fdcd2f".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

Запрос учетных записей и активов с помощью DSL:

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

Используйте байт-код смарт-контракта IVM:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

`aid` / краткий справочник псевдонимов (CLI + Torii):

```bash
# Register an asset definition with canonical aid + explicit name + alias
iroha ledger asset definition register \
  --id aid:2f17c72466f84a4bb8a8e24884fdcd2f \
  --name pkr \
  --alias pkr#ubl@sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id aid:550e8400e29b41d4a7164466554400dd \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl@sbp \
  --account sorauﾛ1P... \
  --quantity 500

# Resolve alias to canonical aid via Torii
curl -sS http://127.0.0.1:8080/v2/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl@sbp"}'
```Примечание по миграции:
— Старые идентификаторы определения актива `name#domain` не принимаются в версии 1.
— Идентификаторы активов для выпуска/сжигания/передачи остаются каноническими `norito:<hex>`; создайте их с помощью:
  - `iroha tools encode asset-id --definition aid:... --account <i105>`
  - или `--alias <name>#<domain>@<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## Управление версиями

- `SignedTransaction`, `SignedBlock` и `SignedQuery` являются каноническими структурами, закодированными в Norito. Каждый из них реализует `iroha_version::Version` для префикса своей полезной нагрузки текущей версией ABI (в настоящее время `1`) при кодировании через `EncodeVersioned`.

## Примечания к обзору/потенциальные обновления

- Query DSL: рассмотрите возможность документирования стабильного подмножества, ориентированного на пользователя, и примеров для общих фильтров/селекторов.
- Семейства инструкций: разверните общедоступные документы со списком встроенных вариантов ISI, представленных `mint_burn`, `register`, `transfer`.

---
Если какая-либо часть требует большей глубины (например, полный каталог ISI, полный список реестра запросов или поля заголовков блоков), дайте мне знать, и я соответствующим образом расширим эти разделы.