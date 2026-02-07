---
lang: ba
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Мәғлүмәттәр моделе һәм ISI — тормошҡа ашырыу‐алыу Spec

Был спецификация `iroha_data_model` һәм `iroha_core` аша ғәмәлдәге тормошҡа ашырыуҙан кире инженерлыҡ итеүҙән ярҙам итеү өсөн проектлауҙы тикшерергә ярҙам итә. Артҡа юлдар авторитетлы кодҡа ишаралай.

## Масштаб
- Канонлы субъекттарҙы (домендар, иҫәп, активтар, НФТ, ролдәр, рөхсәт, тиҫтерҙәр, триггерҙар) һәм уларҙы идентификаторҙарын билдәләй.
- Дәүләт үҙгәртә күрһәтмәләрен һүрәтләй (ISI): төрҙәре, параметрҙары, алдан шарттар, дәүләт күсеүҙәре, ваҡиғалар сығарылған ваҡиғалар, һәм хата шарттары.
- Параметр менән идара итеү, транзакциялар һәм инструкция сериализацияһы дөйөмләштерелә.

Детерминизм: Бөтә инструкция семантикаһы – аппаратһыҙ тәртипһеҙ саф хәл күсеүҙәре. Сериализация Norito ҡулланыла; VM baytecode IVM ҡуллана һәм раҫланған хост-яҡында элек‐сылбыр башҡарыу.

---

## субъекттар һәм идентификаторҙар
IDs тотороҡло струнный формалары менән `Display`/`FromStr` туры. Исем ҡағиҙәләре тыйыу аҡ шаршау һәм запас `@ # $` символдары.- `Name` — раҫланған текст идентификаторы. Ҡағиҙәләр: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Домен: `{ id, logo, metadata, owned_by }`. Төҙөүселәр: `NewDomain`. Код: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — канон адрестары `AccountAddress` (IH58 / `sora…` ҡыҫылған / алты) һәм `AccountAddress::parse_any` аша Torii нормализацияһы аша етештерелә. IH58 - өҫтөнлөклө иҫәп форматында; `sora…` формаһы Сора-тик UX өсөн икенсе-иң яҡшы. Таныш `alias@domain` ептәре тик маршрутлаштырыу псевдонимы булараҡ ғына һаҡлана. Иҫәп: `{ id, metadata }`. Код: `crates/iroha_data_model/src/account.rs`.
- Иҫәп ҡабул итеү сәйәсәте — домендар контролдә тота йәшерен иҫәп-хисап булдырыу аша Norito-JSON `executor.fuel` метамағлүмәттәр асҡысы буйынса Norito. Ҡасан асҡыс юҡ, сылбыр кимәлендә ҡулланыусылар параметры `iroha:default_account_admission_policy` ғәҙәттән тыш хәл итә; ҡасан, тип, шулай уҡ юҡ, ҡаты ғәҙәттәгесә `ImplicitReceive` (беренсе релиз). Сәйәсәт тегтары `mode` (`ExplicitOnly` йәки `ImplicitReceive`) плюс опциональ пер-транзакция (дефолт `16`) һәм бер блок булдырыу ҡапҡастары, өҫтәмә `implicit_creation_fee` (яныу йәки раковина иҫәбенә). `min_initial_amounts` бер актив билдәләмәһе, һәм өҫтәмә `default_role_on_create` (гранировать һуң `AccountCreated`, `DefaultRoleError` менән кире ҡаға, әгәр юғалған). Genesis ҡабул итә алмай; инвалид/дөрөҫ булмаған сәйәсәт `InstructionExecutionError::AccountAdmission` менән билдәһеҙ иҫәптәр өсөн квитанция стилендәге күрһәтмәләрҙе кире ҡаға. Implict иҫәп-хисап марка метамағлүмәттәр `iroha:created_via="implicit"` `AccountCreated` тиклем; ролдәр стандарт сығарыу `AccountRoleGranted`, һәм башҡарыусы хужаһы-база ҡағиҙәләре яңы иҫәп үҙ активтарын сарыф итә/NFTs өҫтәмә ролдәрһеҙ. Код: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Билдәләү: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Код: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId` — `asset#domain#account` йәки `asset##account`, әгәр домендар тап килһә, унда `account` канонлы `AccountId` стринг (IH58 өҫтөнлөк). Актив: `{ id, value: Numeric }`. Код: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`.
- `NftId` — `nft$domain`. НФТ: `{ id, content: Metadata, owned_by }`. Код: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Роль: `{ id, permissions: BTreeSet<Permission> }` төҙөүсе `NewRole { inner: Role, grant_to }` менән. Код: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Код: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — тиҫтерҙәренең үҙенсәлеге (йәмәғәт асҡысы) һәм адресы. Код: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Триггер: `{ id, action }`. Эш: `{ executable, repeats, authority, filter, metadata }`. Код: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` тикшерелгән вставка/юйыу менән. Код: `crates/iroha_data_model/src/metadata.rs`.
- Яҙылыу ҡалыбы (ҡушымта ҡатламы): пландар `AssetDefinition` `subscription_plan` метамағлүмәттәр менән; яҙылыуҙары `Nft` рекордтары менән `subscription` метамағлүмәттәр менән; биллинг ваҡыт менән башҡарыла триггерҙар һылтанма яҙылыу NFTs. Ҡара: `docs/source/subscriptions_api.md` һәм `crates/iroha_data_model/src/subscription.rs`.
- **Криптографик примитивтар** (функция `sm`):- `Sm2PublicKey` / `Sm2Signature` канонлы SEC1 балл көҙгө + SM2 өсөн кодлаусы `r∥s` стационар киңлеге. Конструкторҙар ҡойроҡҡа ағзалыҡты һәм ID семантикаһын айырыуҙы тота (`DEFAULT_DISTID`), шул уҡ ваҡытта тикшерелгән дөрөҫ формалашҡан йәки юғары диапазонлы скалярҙарҙы кире ҡаға. Код: `crates/iroha_crypto/src/sm.rs` һәм Torii.
  - `Sm3Hash` X GM/T 0004 Norito-сериализацияланған `[u8; 32]` яңы типтағы матдә булараҡ ҡулланыла, унда хештар нәфис йәки телеметрия барлыҡҡа килә. Код: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` 128 битлы SM4 асҡыстарын күрһәтә һәм хост-сискаллдар һәм мәғлүмәттәр-модель ҡорамалдары араһында бүленгән. Код: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Был типтағылар ғәмәлдәге Ed25519/BLS/ML-DSA примитивтары менән бергә ултыра һәм мәғлүмәттәр-модель ҡулланыусылар өсөн мөмкин (Torii, SDKs, генез инструменттары) бер тапҡыр `sm` функцияһы өҫтөндә эшләй.

Мөһим һыҙаттар: `Identifiable`, `Registered`/`Registrable` (төҙөүсе ҡалып), `HasMetadata`, `IntoKeyValue`. Код: `crates/iroha_data_model/src/lib.rs`.

Ваҡиғалар: Һәр субъект мутациялар өҫтөндә сығарылған ваҡиғалар бар (хәл/юйыу/хужа үҙгәргән/метамашина үҙгәрҙе, һ.б.). Код: `crates/iroha_data_model/src/events/`.

---

## Параметрҙар (Сылбырлы конфигурация)
- Ғаиләләр: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }` X, `BlockParameters { max_transactions }`, Norito, `SmartContractParameters { fuel, memory, execution_depth }`, өҫтәүенә `FromStr`.
- Диффтар өсөн бер үк ваҡытта: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Агрегатор: `Parameters`. Код: `crates/iroha_data_model/src/parameter/system.rs`.

Параметрҙар ҡуйыу (ISI): `SetParameter(Parameter)` тейешле яланды яңырта һәм `ConfigurationEvent::Changed` сығарыла. Код: `crates/iroha_data_model/src/isi/transparent.rs`, башҡарыусы `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Инструкция сериализацияһы һәм теркәүе
- Ядро һыҙаты: `Instruction: Send + Sync + 'static` менән `dyn_encode()`, `as_any()`, тотороҡло `id()` (бетон тип исеменә тиклем ғәҙәттәгесә).
- `InstructionBox`: `Box<dyn Instruction>` X. Клон/Eq/Ord `(type_id, encoded_bytes)`-та эшләй, шуға күрә тигеҙлек ҡиммәте буйынса.
- Norito серде өсөн `InstructionBox` сериялы `(String wire_id, Vec<u8> payload)` (ҡайтып инә `type_name`, әгәр сым ID). Десериализация глобаль `InstructionRegistry` карта төҙөү идентификаторҙарын конструкторҙарға ҡуллана. Ғәҙәттәгесә, бөтә төҙөлгән ИСИ-ла бөтә төҙөлгән. Код: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ИСИ: Типтар, Семантика, Хаталар
Башҡарыу `Execute for <Instruction>` аша тормошҡа ашырыла `iroha_core::smartcontracts::isi`. Түбәндә йәмәғәт эффекттары, алдан шарттар, ваҡиғалар сығарылған ваҡиғалар һәм хаталар исемлеге.

### Реестр / Регистр
Типтары: `Register<T: Registered>` һәм `Unregister<T: Identifiable>`, бетон маҡсаттарын ҡаплаған `RegisterBox`/`UnregisterBox` типтары менән.

- Тиҫтерҙәре теркәү: донъя тиҫтерҙәренә индереүҙәр ҡуйылған.
  - Алдан шарттар: инде булырға тейеш түгел.
  - Ваҡиғалар: `PeerEvent::Added`.
  - Хаталар: `Repetition(Register, PeerId)` әгәр дубликаты; `FindError` эҙләүҙәрҙә. Код: `core/.../isi/world.rs`.

- Домен регистры: `NewDomain` XX менән `owned_by = authority` менән төҙөй. Отказ: `genesis` домены.
  - Алдан шарттар: домен булмауы; түгел, `genesis`.
  - Ваҡиғалар: `DomainEvent::Created`.
  - Хаталар: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Код: `core/.../isi/world.rs`.- Теркәү иҫәбе: `NewAccount`-тан төҙөлгән, Torii доменында рөхсәт ителмәгән; `genesis` иҫәп яҙмаһын теркәп булмай.
  - Алдан шарттар: домен булырға тейеш; иҫәп-хисап булмаған; генез доменында түгел.
  - Ваҡиғалар: `DomainEvent::Account(AccountEvent::Created)`.
  - Хаталар: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Код: `core/.../isi/domain.rs`.

- Теркәү AssetDefinition: төҙөүсенән төҙөлә; `owned_by = authority` ҡуя.
  - Алдан шарттар: билдәләмәһе юҡлыҡ; домен бар.
  - Ваҡиғалар: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Хаталар: `Repetition(Register, AssetDefinitionId)`. Код: `core/.../isi/domain.rs`.

- НФТ-ға теркәү: төҙөүсенән төҙөлә; `owned_by = authority` й.
  - Алдан шарттар: НФТ-ның булмауы; домен бар.
  - Ваҡиғалар: `DomainEvent::Nft(NftEvent::Created)`X.
  - Хаталар: `Repetition(Register, NftId)`. Код: `core/.../isi/nft.rs`.

- Rehister Role: `NewRole { inner, grant_to }` X (беренсе хужа иҫәп яҙмаһы аша теркәлгән), `inner: Role` магазиндарында төҙөлә.
  - Алдан шарттар: ролдәр юҡлығы.
  - Ваҡиғалар: `RoleEvent::Created`.
  - Хаталар: `Repetition(Register, RoleId)`. Код: `core/.../isi/world.rs`.

- Теркәү Trigger: һаҡлау триггер тейешле триггер йыйылмаһы фильтр төрө буйынса.
  - Алдан шарт: Әгәр фильтр түгел, тип, `action.repeats` `Exactly(1)` булырға тейеш (башҡаса `MathError::Overflow`). Дубликат идентификаторҙары тыйылған.
  - Ваҡиғалар: `TriggerEvent::Created(TriggerId)`.
  - Хаталар: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` X, конверсия/валидация етешһеҙлектәре буйынса. Код: `core/.../isi/triggers/mod.rs`.

- Теркәүһеҙ тиҫтерҙәре/Домен/Иҫәп яҙмаһы/AssetDefinition/NFT/Роль/Триггер: маҡсатты бөтөрә; юйыу ваҡиғаларын сығара. Өҫтәмә каскадлауҙы бөтөрөү:
  - теркәлмәгән домен: домендағы бөтә иҫәптәрҙе, ролдәрен, рөхсәттәрен, tx-эҙмә-эҙмә-эҙмә-эҙмә-эҙләүселәрҙе, иҫәп-хисап билдәләре һәм UAID бәйләүҙәре; уларҙы активтарын юйып (һәм бер актив метамағлүмәт); домендағы бөтә активтарҙы билдәләүҙәрҙе бөтөрә; домендағы НФТ-ларҙы һәм юйыла, уларҙы юйылған иҫәп яҙмалары милкендәге теләһә ниндәй НФТ; влас домены тап килгән триггерҙарҙы алып ташлай. Ваҡиғалар: `DomainEvent::Deleted`, плюс пер-пункт юйыу ваҡиғалары. Хаталар: `FindError::Domain`, әгәр юғалһа. Код: `core/.../isi/world.rs`.
  - Теркәүһеҙ иҫәп: иҫәп яҙмаһын юйып’рөхсәт, ролдәр, tx-эҙмә-эҙлеклелек иҫәпләүсе, иҫәп-хисап ярлыҡ, һәм UAID бәйләүҙәр; иҫәп яҙмаһына ҡараған активтарҙы (һәм перактив метамағлүмәттәре буйынса) юйып ташлай; иҫәп-хисапҡа ҡараған НФТ-ларҙы юя; триггерҙарҙы алып ташлай, кем авторитеты шул иҫәп. Ваҡиғалар: `AccountEvent::Deleted`, өҫтәүенә `NftEvent::Deleted` бер NFT алыу. Хаталар: `FindError::Account`, әгәр юғалған. Код: `core/.../isi/domain.rs`.
  - теркәлмәгән AssetDefinition: был билдәләмәнең бөтә активтарын һәм уларҙы пер-актив метамағлүмәттәрҙе юя. Ваҡиғалар: `AssetDefinitionEvent::Deleted` һәм `AssetEvent::Deleted` бер актив. Хаталар: `FindError::AssetDefinition`. Код: `core/.../isi/domain.rs`.
  - Теркәүһеҙ НФТ: НФТ-ны бөтөрә. Ваҡиғалар: `NftEvent::Deleted`. Хаталар: `FindError::Nft`. Код: `core/.../isi/nft.rs`.
  - Теркәүһеҙ роль: бөтә иҫәптәрҙән ролде тәүҙә тартып ала; һуңынан ролде алып ташлай. Ваҡиғалар: Torii. Хаталар: `FindError::Role`. Код: `core/.../isi/world.rs`.
  - теркәлмәгән Триггер: әгәр бар икән, триггерҙы бөтөрә; дубликаты теркәлмәгән `Repetition(Unregister, TriggerId)`X. Ваҡиғалар: `TriggerEvent::Deleted`. Код: `core/.../isi/triggers/mod.rs`.

### Минт / Янған
Типтар: `Mint<O, D: Identifiable>` һәм `Burn<O, D: Identifiable>`, `MintBox`/`BurnBox` тип йәшниктәр.- Актив (һанлы) мәтрүшкә/яндырыу: баланс һәм билдәләмәһен көйләй’`total_quantity`.
  - Алдан шарттар: `Numeric` ҡиммәте `AssetDefinition.spec()` X ҡәнәғәтләндерергә тейеш; мәтрүшкә рөхсәт `mintable`:
    - `Infinitely` X: һәр ваҡыт рөхсәт.
    - `Once`: теүәл бер тапҡыр рөхсәт ителә; `mintable` беренсе тапҡыр `Not` һәм `AssetDefinitionEvent::MintabilityChanged` сығарыла, өҫтәүенә аудит үткәреү өсөн ентекле `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`.
    - `Limited(n)`: `n` өҫтәмә мәтрүшкә операцияларын рөхсәт итә. Һәр уңышлы мәтрүшкә прилавокты кәметә; ҡасан ул нулгә еткән аныҡлау `Not` тиклем әйләндерә һәм шул уҡ `MintabilityChanged` ваҡиғаларҙы өҫтәге кеүек сығара.
    - `Not`: хатаһы `MintabilityError::MintUnmintable`.
  - Дәүләт үҙгәрештәре: мәтрүшкә юғалһа, актив булдыра; активтар индереүҙе алып ташлай, әгәр баланс нуль булып янып.
  - Ваҡиғалар: `crates/iroha_data_model/src/domain.rs`/Norito, Norito ( `Once` йәки `Limited(n)`-ҡа ҡарағанда, уның пособиеһын сығара).
  - Хаталар: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`X. Код: IVM.

- Триггер ҡабатлауҙары мәтрүшкә/яндырыу: үҙгәрештәр `action.repeats` иҫәп өсөн триггер.
  - Алдан шарттар: мәтрүшкә, фильтрҙа мәтрүшкә булырға тейеш; арифметика ташып торорға тейеш түгел/аҫҡы ағым.
  - Ваҡиғалар: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Хаталар: `MathError::Overflow` дөрөҫ булмаған мәтрүшкә буйынса; `FindError::Trigger`, әгәр юғалһа. Код: `core/.../isi/triggers/mod.rs`.

### Күсерергә
Типтар: `Transfer<S: Identifiable, O, D: Identifiable>`, `TransferBox` тип boxed.

- Актив (һанлы): `AssetId` сығанаҡтан алыу, `AssetId`-та урын өҫтәү (бер үк билдәләмә, төрлө иҫәп). Нулле сығанаҡ активын юйырға.
  - Алдан шарттар: сығанаҡ активы бар; ҡиммәте `spec`-ты ҡәнәғәтләндерә.
  - Ваҡиғалар: `AssetEvent::Removed` (сығанаҡ), `AssetEvent::Added` (төшөү).
  - Хаталар: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Код: `core/.../isi/asset.rs`.

- Доменға милекселек: `Domain.owned_by` X-ҡа үҙгәреш.
  - Алдан шарттар: ике иҫәп яҙмаһы ла бар; домен бар.
  - Ваҡиғалар: `DomainEvent::OwnerChanged`.
  - Хаталар: `FindError::Account/Domain`. Код: `core/.../isi/domain.rs`.

- AssetDefinition милекселеге: `AssetDefinition.owned_by` үҙгәрештәре өсөн тәғәйенләнеш иҫәбенә.
  - Алдан шарттар: ике иҫәп яҙмаһы ла бар; аныҡлау бар; сығанаҡ әлеге ваҡытта уның хужаһы булырға тейеш.
  - Ваҡиғалар: `AssetDefinitionEvent::OwnerChanged`.
  - Хаталар: `FindError::Account/AssetDefinition`. Код: `core/.../isi/account.rs`.

- НФТ милекселеге: `Nft.owned_by` XX үҙгәрештәр.
  - Алдан шарттар: ике иҫәп яҙмаһы ла бар; НФТ бар; сығанаҡ әлеге ваҡытта уның хужаһы булырға тейеш.
  - Ваҡиғалар: `NftEvent::OwnerChanged`.
  - Хаталар: `FindError::Account/Nft`, `InvariantViolation`, әгәр сығанаҡ НФТ-ға эйә булмаһа. Код: `core/.../isi/nft.rs`.

### Metatata: Ҡуйыу/Асҡыс-ҡиммәте сығарыу
Типтары: `SetKeyValue<T>` һәм `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` менән `RemoveKeyValue<T>`. Боксированный анументтар бирелгән.

- Комплект: индереү йәки алмаштыра `Metadata[key] = Json(value)`.
- Сығарылыш: асҡысты алып ташлай; хата, әгәр юғалған.
- Ваҡиғалар: `<Target>Event::MetadataInserted` / `MetadataRemoved` иҫке/яңы ҡиммәттәр менән.
- Хаталар: `FindError::<Target>`, әгәр маҡсат бар түгел; `FindError::MetadataKey` юғалған асҡыс буйынса сығарыу өсөн. Код: `crates/iroha_data_model/src/isi/transparent.rs` һәм башҡарыусы импульс бер маҡсат.### Рөхсәт һәм ролдәр: Грант / Ҡатыштырыу
Типтары: `Grant<O, D>` һәм `Revoke<O, D>`, `Permission`/`Role` өсөн боксированный enums менән/`Account`, һәм Iroha тиклем/2000 йылдарҙа. `Role`.

- Грант рөхсәт иҫәбенә: `Permission` өҫтәй, әгәр ҙә инде үҙенә генә хас. Ваҡиғалар: `AccountEvent::PermissionAdded`. Хаталар: `Repetition(Grant, Permission)` әгәр дубликаты. Код: `core/.../isi/account.rs`.
- Иҫәп яҙмаһынан рөхсәт алыу: әгәр бар икән, юҡҡа сығарыла. Ваҡиғалар: `AccountEvent::PermissionRemoved`. Хаталар: `FindError::Permission`, әгәр юҡ. Код: `core/.../isi/account.rs`.
- Грант роле буйынса иҫәп: `(account, role)` карта төҙөү, әгәр юҡ. Ваҡиғалар: `AccountEvent::RoleGranted`. Хаталар: `Repetition(Grant, RoleId)`. Код: `core/.../isi/account.rs`.
- Иҫәп яҙмаһынан ролде кире ҡағыу: әгәр бар икән, картаға төшөрөүҙе бөтөрә. Ваҡиғалар: `AccountEvent::RoleRevoked`. Хаталар: `FindError::Role`, әгәр юҡ. Код: `core/.../isi/account.rs`.
- Грант Рөхсәткә рөхсәт: рөхсәт өҫтәлгән ролен тергеҙә. Ваҡиғалар: `RoleEvent::PermissionAdded`. Хаталар: `Repetition(Grant, Permission)`. Код: `core/.../isi/world.rs`.
- Рольдән рөхсәт алыу: был рөхсәтһеҙ роль тергеҙә. Ваҡиғалар: `RoleEvent::PermissionRemoved`. Хаталар: `FindError::Permission`, әгәр юҡ. Код: `core/.../isi/world.rs`.

### Триггер:
Тип: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- тәртибе: `ExecuteTriggerEvent { trigger_id, authority, args }` триггер подсистемаһы өсөн ҡарай. Ҡул менән башҡарыу тик шылтыратыу өсөн генә рөхсәт ителә триггерҙар (`ExecuteTrigger` фильтр); фильтр тура килергә тейеш һәм шылтыратыусы булырға тейеш триггер ғәмәлдәре йәки үткәрергә `CanExecuteTrigger` был вәкәләт өсөн. Ҡасан ҡулланыусылар менән тәьмин иткән башҡарыусы әүҙем, триггер башҡарыу раҫлана эшләүсе һәм ҡулланыу транзакция’s башҡарыусы яғыулыҡ бюджеты (база Norito плюс өҫтәмә метамағлүмәттәр Norito).
- Хаталар: `FindError::Trigger`, әгәр теркәлмәһә, әгәр; `InvariantViolation`, әгәр ҙә саҡырылмаһа, августа. Код: `core/.../isi/triggers/mod.rs` (һәм `core/.../smartcontracts/isi/mod.rs`-та һынауҙар).

### Яңыртыу һәм журнал
- `Upgrade { executor }`: башҡарыусыны миграция ҡулланып, тәьмин ителгән `Executor` байтекод, яңыртыуҙар башҡарыусы һәм уның мәғлүмәттәре моделе, `ExecutorEvent::Upgraded` сығара. Хаталар: `InvalidParameterError::SmartContract` тип уралған миграция етешһеҙлектәре буйынса. Код: `core/.../isi/world.rs`.
- `Log { level, msg }`: бирелгән кимәл менән төйөн журналы сығара; дәүләт үҙгәрештәре юҡ. Код: `core/.../isi/world.rs`.

### Хата моделе
Дөйөм конверт: `InstructionExecutionError` варианттары менән баһалау хаталары, эҙләү етешһеҙлектәре, конверсия, субъект табылмаған, ҡабатлау, mathing, математика, дөрөҫ булмаған параметр, һәм инвариант боҙоу. Һандар һәм ярҙамсылар `crates/iroha_data_model/src/isi/mod.rs` `pub mod error` буйынса.

---## Транзакциялар һәм башҡарыусылар
- `Executable`: йәки `Instructions(ConstVec<InstructionBox>)` йәки `Ivm(IvmBytecode)` X; baytecode serialize base64. Код: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: конструкциялар, билдәләр һәм пакеттар менән башҡарыла торған метамағлүмәттәр, `chain_id`, `authority`, Iroha, `ttl_ms`, һәм `nonce`. Код: `crates/iroha_data_model/src/transaction/`.
- Йөрөү ваҡытында `iroha_core` `InstructionBox` партияһы `Execute for InstructionBox` аша башҡарыла, тейешле `*Box` йәки бетон инструкцияһына төшөнкөлөккә бирелә. Код: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Йүгереп ваҡыт башҡарыусы валидация бюджеты (ҡулланыусы менән тәьмин ителгән башҡарыусы): база `ImplicitReceive` параметрҙарҙан плюс өҫтәмә транзакция метамағлүмәттәр `mode` (`ExplicitOnly`), транзакция сиктәрендә инструкция/триггер раҫлауҙары буйынса бүленгән.

---

## инварианттар һәм иҫкәрмәләр (тест һәм һаҡсыларҙан)
- Genesis һаҡлау: `ImplicitReceive` доменын йәки иҫәп яҙмаларын `genesis` доменында теркәй алмай; `implicit_creation_fee` иҫәбен теркәп булмай. Код/тестар: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Һанлы активтар үҙҙәренең `NumericSpec` мәтрүшкә/трансфер/яныуҙа ҡәнәғәтләндерергә тейеш; спец тап килмәүе `TypeError::AssetNumericSpec` бирә.
- Минтабильность: `Once` бер мәтрүшкә мөмкинлек бирә, ә һуңынан `Not`X тиклем әйләндерә; `Limited(n)` `n` мәтрүшкәләрен `Not` тиклем әйләндергәнсе теүәл мөмкинлек бирә. `Infinitely`-та `MintabilityError::ForbidMintOnMintable`-та тыйыуҙы тыйыуға ынтылыштар һәм `Limited(0)` `MintabilityError::InvalidMintabilityTokens` етештереүсәнлеген конфигурациялау.
- Метадата операциялары төп теүәл; булмаған асҡысты бөтөрөү — хата.
- Триггер фильтрҙары булмауы мөмкин түгел; Һуңынан `Register<Trigger>` ғына рөхсәт `Exactly(1)` ҡабатлай.
- Триггер метамағлүмәттәр асҡысы `__enabled` (бол) ҡапҡаларын башҡарыу; 1990 йылдарҙа был йүнәлештәге эштәрҙең иң мөһимдәренең береһе булып тора, ә инвалид триггерҙар мәғлүмәттәр/ваҡыт/шылтыратыу юлдары буйынса үткәрелә.
- Детерминизм: бөтә арифметик ҡулланыу тикшерелгән операциялар; аҫтында/өҫкә ағып ҡайтара тип математика хаталары; нуль баланстары тамсы активтар яҙмалары (йәшерен хәле юҡ).

---## Практик миҫалдар
- Штамм һәм күсерергә:
  - `Mint::asset_numeric(10, asset_id)` → 10 өҫтәй, әгәр ҙә спец/mintability рөхсәт ителә; ваҡиғалар: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 хәрәкәттәр; ваҡиғалар өсөн бөтөрөү/өҫтәү.
- Метадата яңыртыуҙары:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; `RemoveKeyValue::account(...)` аша сығарыу.
- Роль/рөхсәт менән идара итеү:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)`, һәм уларҙы `Revoke` коллегалары.
- Триггер тормош циклы:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` менән фильтр менән аңлатыла; `ExecuteTrigger::new(id).with_args(&args)` конфигурацияланған власть тап килергә тейеш.
  - Triggers өҙөлгән була ала, метамағлүмәттәр асҡысы `__enabled` `false` тиклем (туҡландырыу өсөн ғәҙәттәгесә); `SetKeyValue::trigger` йәки IVM X`set_trigger_enabled` syscall аша toggle.
  - Триггер һаҡлау йөк өҫтөндә ремонтлана: дубликаттар ids, тап килмәгән ids, һәм триггерҙар һылтанма юҡ butecode ташлана; байткод белешмәләр һаны яңынан иҫәпләнә.
  - Әгәр ҙә триггер IVM байтекод башҡарылған ваҡытта юҡ, триггер алып ташлана һәм башҡарылыу етешһеҙлек һөҙөмтәһе менән оп булмаған тип ҡарала.
  - Һыулы триггерҙарҙы шунда уҡ сығарыла; әгәр ҙә атҡарылған ваҡытта ярлыланған яҙма осраһа, ул ҡырҡылған һәм юҡ тип ҡабул ителә.
- Параметр яңыртыу:
  - IVM яңыртыуҙары һәм `ConfigurationEvent::Changed` сығара.

---

## Эҙләүсәнлек (һайланған сығанаҡтар)
 - Мәғлүмәттәр моделе ядроһы: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ИСИ билдәләмәләре һәм реестры: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ИСИ-ны үтәү: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`X.
 - Ваҡиғалар: `crates/iroha_data_model/src/events/**`.
 - Транзакциялар: `crates/iroha_data_model/src/transaction/**`.

Әгәр һеҙ был спец киңәйгән рендерланған API/тәртип таблицаһы йәки кросс‐һәр бетон ваҡиға/хата, һүҙ әйтергә һәм мин уны оҙайтыу.