---
lang: ba
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Мәғлүмәттәр моделе – Тәрән һыу инеү

Был документта Iroha v2 мәғлүмәт моделен тәшкил иткән структуралар, идентификаторҙар, һыҙаттар һәм протоколдар аңлатыла, был Kotodama йәшниктә тормошҡа ашырыла һәм эш урыны буйынса ҡулланыла. Ул аныҡ һылтанма булырға тейеш, һеҙ тикшерергә һәм яңыртыуҙар тәҡдим итә ала.

## Сокус һәм нигеҙҙәр

- Маҡсат: Домен объекттары (домендар, иҫәп, активтар, НФТ, ролдәр, рөхсәттәр, тиҫтерҙәр), дәүләт үҙгәртеп күрһәтмәләре (ISI), эҙләүҙәр, триггерҙар, транзакциялар, блоктар һәм параметрҙар өсөн канон төрҙәрен тәьмин итеү.
- Сериализация: Бөтә йәмәғәт типтары Norito кодектарын (`norito::codec::{Encode, Decode}`) һәм схема (`iroha_schema::IntoSchema`) килеп сыға. JSON һайлап ҡулланыла (мәҫәлән, HTTP һәм `Json` файҙалы йөкләмәләр өсөн) функцияһы флагтар артында.
- IVM иҫкәрмәһе: Ҡайһы бер дезериализация-ваҡыт валидациялары өҙөлгән, ҡасан маҡсатлы Norito виртуаль машина (IVM), сөнки хужа валидация башҡара, килешеп, контракттарҙы саҡырыу алдынан (ҡарағыҙ, йәшник docs `src/lib.rs`).
- FFI ҡапҡалары: Ҡайһы бер төрҙәре шартлы рәүештә аннотацияланған FFI аша `iroha_ffi` артында `ffi_export`/`ffi_import` ҡотолоу өсөн накладной ҡасан FFI кәрәкмәй.

## Ядро һыҙаттары һәм ярҙамсылары

- `Identifiable`: субъекттар тотороҡло `Id` һәм `fn id(&self) -> &Self::Id`. `IdEqOrdHash` менән карта/комплект дуҫлығы өсөн алынған булырға тейеш.
- `Registrable`/`Registered`: Күп субъекттар (мәҫәлән, `Domain`, `AssetDefinition`, `Role`) төҙөүсе өлгөһөн ҡуллана. `Registered` эшләү ваҡыты тибы менән еңел төҙөүсе тип (`With`) теркәү операциялары өсөн яраҡлы.
- `HasMetadata`: `Metadata` картаһына асҡыс/ҡиммәте менән берҙәм инеү.
- `IntoKeyValue`: Һаҡлау ярҙамсыһы ярҙамсыһын һаҡлау өсөн `Key` (ID) һәм `Value` (мәғлүмәттәр) айырым кәметергә дубликация.
- `Owned<T>`/`Ref<'world, K, V>`: Кәрәкмәгән күсермәләрҙән ҡотолоу өсөн һаҡлау һәм эҙләү фильтрҙарында ҡулланылған еңел урауҙар.

## Исемдәр һәм идентификаторҙар

- `Name`: Дөрөҫ текст идентификаторы. Ҡаршылыҡлы аҡ шкала һәм запасланған персонаждар `@`, `#`, `$` X (композит идентификаторҙарҙа ҡулланыла). Конструктив аша `FromStr` валидация менән. Исемдәрҙе анализлауҙа Unicode NFC-ға нормалаштыралар (канон эквивалентлы яҙылыштары бер үк һәм һаҡланған төҙөлгән тип ҡарала). `genesis` махсус исеме һаҡлана (тикшерелгән кейс-һиҙгер булмаған).
- `IdBox`: теләһә ниндәй ярҙам идентификаторы өсөн сумма-тип конверт (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Дөйөм ағымдар һәм Norito өсөн файҙалы кодлау бер типтағы.
- `ChainId`: транзакцияларҙа реплей һаҡлау өсөн ҡулланылған асыҡ булмаған сылбырлы идентификатор.ИД-лар струнный формалары (`Display`/`FromStr` менән түңәрәкләп йөрөү):
- `DomainId`: `name` (мәҫәлән, `wonderland`).
- `AccountId`: IH58, Сора ҡыҫылған (`sora…`) һәм канонлы гекс кодектары (`AccountAddress::to_ih58`, `to_compressed_sora`, `sora…`) аша кодланған канонлы идентификатор. `canonical_hex`, `parse_encoded`). IH58 - өҫтөнлөклө иҫәп форматында; `sora…` формаһы Сора-тик UX өсөн икенсе иң яҡшы. Кешегә уңайлы маршрутлаштырыу псевдонимы `alias@domain` UX өсөн һаҡлана, әммә хәҙер абруйлы идентификатор булараҡ ҡаралмай. Torii `AccountAddress::parse_encoded` аша килгән ептәрҙе нормалләштерә. Иҫәп идентификаторҙары ярҙам итә, ике бер асҡыс һәм мультисиг контроллерҙар.
- `AssetDefinitionId`: `asset#domain` (мәҫәлән, `xor#soramitsu`).
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (мәҫәлән, `rose$garden`X).
- `PeerId`: `public_key` (тиңдәш тигеҙлеге асыҡ асҡыс буйынса).

##

### Домен
- `DomainId { name: Name }` – уникаль исем.
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Төҙөүсе: `NewDomain` менән `with_logo`, `with_metadata`, һуңынан `Registrable::build(authority)` комплекттары `owned_by`.

### Иҫәп яҙмаһы
- `AccountId { domain: DomainId, controller: AccountController }` (контроллер = бер асҡыс йәки мультисиг сәйәсәте).
- `Account { id, metadata, label?, uaid? }` — `label` - был өҫтәмә тотороҡло псевдоним ҡулланылған перек рекордтары, `uaid` опциональ Kotodama-д.
- Төҙөүсе: `NewAccount` аша `Account::new(id)`; `HasMetadata` төҙөүсе һәм субъект өсөн дә.

### Активтар аңлатмалары һәм активтары
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`: `Infinitely` X | `Once` | `Limited(u32)` | `Not`.
  - Төҙөүселәр: `AssetDefinition::new(id, spec)` йәки уңайлыҡтар `numeric(id)`; `metadata` өсөн сеттерҙар, `mintable`, `owned_by`.
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }` менән һаҡлау-дуҫ Kotodama/`AssetValue` менән.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` йыйнаҡ API-лар өсөн фашланған.

### НФТ
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (контент — үҙ теләге менән асҡыс/ҡиммәте метамағлүмәттәр).
- Төҙөүсе: `NewNft` аша `Nft::new(id, content)`.

### ролдәре һәм рөхсәттәре
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` төҙөүсе `NewRole { inner: Role, grant_to: AccountId }` менән.
- `Permission { name: Ident, payload: Json }` – `name` һәм файҙалы йөк схемаһы әүҙем `ExecutorDataModel` менән тура килергә тейеш (аҫта ҡарағыҙ).

### Ҡорҙаштер
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` һәм Parsable `public_key@address` еп формаһы.### Криптографик примитивтар (функцияһы `sm`)
- `Sm2PublicKey` һәм `Sm2Signature`: SEC1-ға ярашлы мәрәйҙәр һәм SM2 өсөн стационар `r∥s` ҡултамғалары. Конструкторҙар ҡойроҡ ағзалығын раҫлай һәм идентификаторҙарҙы айыра; Norito кодлау көҙгө канон күрһәтеү ҡулланылған Kotodama.
- `Sm3Hash`: `[u8; 32]` яңы типтағы GM/T 0004 дигест, манифестарҙа ҡулланыла, телеметрия, һәм syscall яуаптар.
- `Sm4Key`: 128-битлы симметрик асҡыс уратып алынған хост сискалдары һәм мәғлүмәттәр-модель ҡорамалдары араһында бүленгән.
Был төрҙәр ғәмәлдәге Ed25519/BLS/ML-DSA примитивтары менән бер рәттән ултыра һәм йәмәғәт схемаһы өлөшөнә әйләнә, бер тапҡыр эш урыны `--features sm` менән төҙөлгән.

### Триггер һәм ваҡиғалар
- `TriggerId { name: Name }` һәм `Trigger { id, action: action::Action }`.
— `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` йәки `Exactly(u32)`; заказ һәм коммуналь хеҙмәттәр кәметелгән инә.
  - Хәүефһеҙлек: `TriggerCompleted` ғәмәлдең фильтр булараҡ ҡулланыла алмай (де)сериялаштырыу ваҡытында раҫланған).
- `EventBox`: торба, торба-партия, мәғлүмәттәр, ваҡыт, башҡарыу-триггер һәм триггер менән тамамланған ваҡиғалар өсөн сумма тип; `EventFilterBox` көҙгөләр, тип яҙылыу һәм фильтрҙар өсөн триггер.

## Параметрҙар һәм конфигурация

- Система параметры ғаиләләре (бөтәһе лә `Default`ed, герогтерҙарҙы йөрөтөү һәм айырым анумдарға үҙгәртеп ҡороу):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` бөтә ғаиләләр һәм `custom: BTreeMap<CustomParameterId, CustomParameter>` төркөмдәре.
- Бер параметрлы нуптар: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` диффҡа оҡшаш яңыртыуҙар һәм итерацион өсөн.
- Ҡулланыусы параметрҙары: башҡарма билдәләгән, `Json` тип йөрөтә, `CustomParameterId` (`Name`) менән билдәләнгән.

## ИСИ (Iroha Махсус инструкциялар)

- Ядро һыҙаты: `Instruction` менән `dyn_encode`, `as_any`, һәм тотороҡло бер типтағы идентификатор `id()` (бетон типтағы исемгә ғәҙәттәгесә). Бөтә күрһәтмәләр ҙә `Send + Sync + 'static`.
- `InstructionBox`: ID + кодланған байттар аша клон/экв/орд менән `Box<dyn Instruction>` уратып алынған.
- Инструкцияла төҙөлгән ғаиләләр буйынса ойошторола:
  - `mint_burn`, `transfer`, `register`, һәм `transparent` ярҙамсылары өйөмө.
  - Мета ағымдары өсөн тип анумдар: `InstructionType`, `SetKeyValueBox` кеүек боксированный суммалар (домен/иҫәп/актив_деф/нфт/триггер).
- Хаталар: `isi::error` буйынса бай хата моделе (баһалау типтағы хаталар, хаталар, математика, дөрөҫ булмаған параметрҙар, ҡабатлау, инварианттар табырға).
- Инструкция реестры: `instruction_registry!{ ... }` макросы тип исем буйынса эшләү ваҡыты рациональ реестры клавишаһын төҙөй. Ҡулланылған `InstructionBox` клоны һәм Norito серд динамик (де)сериализацияға өлгәшеү өсөн. Әгәр ҙә бер ниндәй ҙә реестр асыҡтан-асыҡ `set_instruction_registry(...)` аша ҡуйылған, бөтә ядро ​​ISI менән төҙөлгән ғәҙәттәгесә реестр ялҡау ғына тәүге ҡулланыуҙа бинарҙарҙы ныҡлы тотоу өсөн ҡуйылған.

## Транзакциялар- Norito. `IvmBytecode` serialize base64 (үтә күренмәле яңы типтағы `Vec<u8>`X).
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms` менән транзакция файҙалы йөкләмәһен төҙөй, факультатив `time_to_live_ms` һәм `nonce`, `metadata` һәм . `Executable`.
  - Ярҙамсы: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, Norito, Norito. `sign`.
- `SignedTransaction` (`iroha_version` менән версияһы): `TransactionSignature` һәм файҙалы йөк ташый; хешинг һәм ҡултамға тикшерелеүен тәьмин итә.
- Яҙыу нөктәләре һәм һөҙөмтәләр:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = хешинг ярҙамсылары менән `Result<DataTriggerSequence, TransactionRejectionReason>`.
  - `ExecutionStep(ConstVec<InstructionBox>)`: транзакцияла бер заказ бирелгән инструкциялар партияһы.

## Блоктар

- `SignedBlock` (версияланған) капсуляр:
  - `signatures: BTreeSet<BlockSignature>` (валиторҙарҙан),
  — `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (икенсел башҡарыу хәле) составында `time_triggers`, инеү/һөҙөмтә меркл ағастары, `transaction_results`, `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- коммуналь хеҙмәттәр: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, Norito, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Меркл тамырҙары: транзакцияға инеү һәм һөҙөмтәләр Меркл ағастары аша эшләнә; һөҙөмтә Меркл тамыры блок башына ҡуйыла.
- Блок инклюзия иҫбатлауҙары (`BlockProofs`) инеү/һөҙөмтәле Меркл иҫбатлауҙары һәм `fastpq_transcripts` картаһы шулай офф-сылбыр проверстары транзакция хеш менән бәйле тапшырыу дельтаһын ала ала.
- `ExecWitness` хәбәрҙәр (Torii аша ағым һәм консенсус ғәйбәтендә сусҡасылыҡ) хәҙер `fastpq_transcripts` һәм иҫбатлаусы `fastpq_batches: Vec<FastpqTransitionBatch>`-тың `public_inputs`- менән иҫбатлаусы `fastpq_batches: Vec<FastpqTransitionBatch>` (dsid, slot , тамырҙары, perm_root, tx_set_hash), шуға күрә тышҡы проверстар FASTPQ рәттәрен яңынан кодлау транскрипцияларһыҙ инә ала.

## Һорауҙар

- Ике тәм:
  - Яңғыҙлыҡ: `SingularQuery<Output>` XX (мәҫәлән, `FindParameters`, `FindExecutorDataModel` X).
  - Iterable: `Query<Item>` X (мәҫәлән, `FindAccounts`, `FindAssets`, `FindDomains` һ.б.).
- Тип-ысланған формалар:
  - `QueryBox<T>` — йәшникле, `Query<Item = T>` юйылған Norito серҙа менән глобаль реестр ярҙамында.
  - `QueryWithFilter<T> { query, predicate, selector }` парҙары эҙләү менән DSL предикат/һайлаусы; `From` XX аша юйылған итераллы эҙләүгә әйләндерә.
- Реестр һәм кодектар:
  - `query_registry!{ ... }` глобаль реестр картаһы бетон эҙләү типтарын конструкторҙарға динамик декод өсөн тип исем буйынса төҙөй.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` һәм `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` — бер төрлө векторҙарға ҡарағанда сумма-тип (мәҫәлән, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), плюс кортеж һәм оҙайтыу өсөн ярҙам итеүселәр һөҙөмтәле.
- DSL: `query::dsl`-та проекция һыҙаттары (`HasProjection<PredicateMarker>`X / `SelectorMarker`) компиляция-ваҡыт тикшерелгән предикаттар һәм селекторҙар өсөн тормошҡа ашырыла. `fast_dsl` функцияһы кәрәк булһа, еңел вариантын фашлай.

## Башҡарыусы һәм киңәйтеүсәнлек- `Executor { bytecode: IvmBytecode }`: валидатор менән башҡарылған код пакеты.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` башҡарыусы билдәләгән доменды иғлан итә:
  - Ҡулланыусылар конфигурацияһы параметрҙары,
  - Ҡулланыусы инструкция идентификаторҙары, .
  - Рөхсәт жетон идентификаторҙары,
  - Клиент инструменттары өсөн ҡулланыусы төрҙәрен һүрәтләгән JSON схемаһы.
- `data_model/samples/executor_custom_data_model` аҫтындағы ҡоролма өлгөләре бар: күрһәтеү:
  - Ҡулланыусылар өсөн рөхсәт жетоны аша `iroha_executor_data_model::permission::Permission` сыға,
  - Ҡулланыусы параметры тип үҙгәртелә `CustomParameter`,
  - `CustomInstruction`-ға сериализацияланған ҡулланыу инструкциялары башҡарыу өсөн.

### Ҡулланыусы (башҡарыусы билдәләгән ИСИ)

- Тип: `isi::CustomInstruction { payload: Json }` тотороҡло сымлы id `"iroha.custom"` XX.
- Маҡсат: шәхси/консорциум селтәрҙәрендә йәки прототиплаштырыу өсөн башҡарыусыға махсус күрһәтмәләр өсөн конверт, йәмәғәт мәғлүмәттәре моделен формулировкаһыҙ.
- Ғәҙәттән тыш башҡарыусы тәртибе: Norito-та төҙөлгән башҡарыусы Norito-ла башҡармай һәм осраһа, паникаға биреләсәк. Ҡулланыусы башҡарыусыһы `InstructionBox`X `CustomInstruction`-ҡа тиклем өҙөлергә һәм файҙалы йөктө бөтә валидаторҙарға детерминистик интерпретацияларға тейеш.
- Norito: `norito::codec::{Encode, Decode}` аша кодекс/декодтар схемаһы менән индерелгән; `Json` файҙалы йөк сериализацияланған детерминистик. Түңәрәк-сәйәхәттәр тотороҡло шул тиклем оҙаҡ, сөнки инструкция реестры `CustomInstruction` инә (ул реестр өлөшө булып тора).
- IVM: Norito байткодҡа (`.to`) һәм ғариза логикаһы өсөн тәҡдим ителгән юл. Тик ҡулланыу `CustomInstruction` өсөн башҡарыусы кимәлендә оҙайтыу, уларҙы әлегә Kotodama экспрессировать мөмкин түгел. Детерминизм һәм бер үк башҡарыусы бинар тиҫтерҙәре буйынса тәьмин итеү.
- Йәмәғәт селтәрҙәре өсөн түгел: йәмәғәт сылбырҙары өсөн ҡулланмай, унда гетероген башҡарыусылар консенсус санкалары хәүефен тыуҙыра. Өҫтөнлөк тәҡдим яңы встроенный ISI өҫкө ағымында, ҡасан һеҙгә кәрәк платформа функциялары.

## Метадата

- `Metadata(BTreeMap<Name, Json>)`: асҡыс/ҡиммәте магазинына беркетелгән бер нисә субъектҡа беркетелгән (`Domain`, `Account`, `AssetDefinition`, `Nft`, триггерҙар, һәм транзакциялар).
- API: `contains`, `iter`, `get`, `insert`, һәм (Norito X) `remove`.

## үҙенсәлектәре һәм детерминизм

- Үҙенсәлектәре менән идара итеү опциональ APIs (`std`, `json`, `transparent_api` X, Kotodama, Kotodama, `fast_dsl`, Norito. Norito).
- Детерминизм: Бөтә сериализация Norito X ҡуллана, аппарат буйынса портатив булырға кодлау. IVM байтекод — асыҡ булмаған байт тап; башҡарыу детерминистик булмаған кәметергә тейеш түгел. Хост транзакциялар һәм IVM детерминистик рәүештә индереүҙәрҙе раҫлай.

### Үтә күренмәле API (`transparent_api`)- Маҡсат: `#[model]`X структурҙары/һандар өсөн тулы, үҙгәрмәй торған инеү мөмкинлеген фашлай, мәҫәлән, Torii, башҡарыусылар һәм интеграция һынауҙары. Унһыҙ, был әйберҙәр аңлы рәүештә асыҡланмаған, шуға күрә тышҡы SDKs тик хәүефһеҙ конструкторҙар һәм кодланған файҙалы йөктәрҙе күрә.
- Механика: `iroha_data_model_derive::model` макросы һәр йәмәғәт яланын `#[cfg(feature = "transparent_api")] pub` менән яңынан яҙа һәм стандарт төҙөү өсөн шәхси күсермәһен һаҡлай. Функцияны индереү был cfgs, шулай итеп, емерек `Account`, `Domain`, Kotodama, һ.б.
- Ер өҫтөн асыҡлау: йәшник `TRANSPARENT_API: bool` константаһын экспортлай (`transparent_api.rs` йәки `non_transparent_api.rs`-ға генерациялана). Аҫҡа ағым кодын тикшерә ала, был флаг һәм тармаҡ, ҡасан ул кәрәк, тип кире төшөп, асыҡ булмаған ярҙамсылар.
- Инвалидлыҡ: `features = ["transparent_api"]` өҫтәүгә бәйлелек `Cargo.toml`. Workspace йәшниктәре, улар кәрәк JSON проекцияһы (мәҫәлән, `iroha_torii`) флаг автоматик рәүештә алға, әммә өсөнсө яҡ ҡулланыусылар уны һаҡларға тейеш, әгәр улар идара итеү һәм ҡабул итеү киң API өҫтө.

## Тиҙ миҫал

Домен һәм иҫәп яҙмаһы булдырыу, активты билдәләү, һәм инструкция менән транзакция төҙөү:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

Һорауҙар буйынса иҫәп-хисап һәм активтар менән DSL:

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

Ҡулланыу IVM аҡыллы килешеп байткод:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## Версиялау

- `SignedTransaction`, `SignedBlock`, һәм `SignedQuery` канонлы Norito-кодланған структурҙар. Һәр береһе `iroha_version::Version` ғәмәлдәге ABI версияһы менән файҙалы йөктәрҙе префикс (әлеге ваҡытта `EncodeVersioned` аша кодланғанда.

## Обзор иҫкәрмәләр / Потенциаль яңыртыу

- Һорау DSL: документлаштырыуҙы ҡарарға тотороҡло ҡулланыусы-йөҙө подмножество һәм миҫалдар өсөн дөйөм фильтрҙар/һайлаусылар.
- Инструкция ғаиләләре: йәмәғәт документтарын киңәйтеү ISI варианттарын исемлеккә индереү ISI `mint_burn`, `register`, `transfer`.

---
Әгәр ниндәй ҙә булһа өлөшө күберәк тәрәнлек кәрәк (мәҫәлән, тулы ISI каталогы, тулы эҙләү реестр исемлеге, йәки блок баш ҡырҙары), миңә хәбәр итегеҙ һәм мин&#8217;ll был бүлектәрҙе тейешле рәүештә оҙайтыу.