---
lang: kk
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 деректер үлгісі және ISI — енгізуден алынған спецификация

Бұл спецификация дизайнды тексеруге көмектесу үшін `iroha_data_model` және `iroha_core` бойынша ағымдағы енгізуден кері жобаланған. Кері жолдардағы жолдар беделді кодты көрсетеді.

## Ауқым
- Канондық нысандарды (домендер, тіркелгілер, активтер, NFTs, рөлдер, рұқсаттар, әріптестер, триггерлер) және олардың идентификаторларын анықтайды.
- Күйді өзгертетін нұсқауларды (ISI) сипаттайды: түрлер, параметрлер, алғы шарттар, күй ауысулары, шығарылған оқиғалар және қате жағдайлары.
- Параметрлерді басқаруды, транзакцияларды және нұсқауларды сериялауды қорытындылайды.

Детерминизм: Барлық нұсқау семантикасы аппараттық құралға тәуелді мінез-құлықсыз таза күйдегі ауысулар болып табылады. Серияландыру Norito пайдаланады; VM байт коды IVM пайдаланады және тізбектегі орындалу алдында хост тарапынан тексеріледі.

---

## Нысандар мен идентификаторлар
Идентификаторларда `Display`/`FromStr` айналу жолы бар тұрақты жол пішіндері бар. Атау ережелері бос орынға және сақталған `@ # $` таңбаларына тыйым салады.- `Name` — расталған мәтіндік идентификатор. Ережелер: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Домен: `{ id, logo, metadata, owned_by }`. Құрылысшылар: `NewDomain`. Код: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — канондық мекенжайлар `AccountAddress` (IH58 / `sora…` қысылған / алтылық) арқылы шығарылады және Torii `AccountAddress::parse_encoded` арқылы кірістерді қалыпқа келтіреді. IH58 - қолайлы тіркелгі пішімі; `sora…` пішіні тек Sora UX үшін екінші ең жақсы. Таныс `alias` (rejected legacy form) жолы тек маршруттау бүркеншік аты ретінде сақталады. Есептік жазба: `{ id, metadata }`. Код: `crates/iroha_data_model/src/account.rs`.
- Есептік жазбаны қабылдау саясаты — домендер `iroha:account_admission_policy` метадеректер кілті астында Norito-JSON `AccountAdmissionPolicy` сақтау арқылы жасырын тіркелгі жасауды басқарады. Кілт жоқ кезде, `iroha:default_account_admission_policy` тізбек деңгейіндегі теңшелетін параметр әдепкі мәнді береді; ол да болмаған кезде, қатты әдепкі мән `ImplicitReceive` (бірінші шығарылым) болып табылады. Саясат тегтері `mode` (`ExplicitOnly` немесе `ImplicitReceive`) плюс әр транзакция үшін қосымша (әдепкі `16`) және әр блокты жасау шектеулері, қосымша `implicit_creation_fee` (қосымша `implicit_creation_fee` тіркелгісі немесе sink), Актив анықтамасы үшін `min_initial_amounts` және қосымша `default_role_on_create` (`AccountCreated` кейін беріледі, жоқ болса `DefaultRoleError` қабылдамайды). Genesis қосыла алмайды; өшірілген/жарамсыз саясаттар `InstructionExecutionError::AccountAdmission` бар белгісіз тіркелгілерге арналған түбіртек стиліндегі нұсқауларды қабылдамайды. `AccountCreated` алдындағы `iroha:created_via="implicit"` метадеректерінің жасырын тіркелгі мөрі; әдепкі рөлдер `AccountRoleGranted` қосымшасын шығарады және орындаушы иесінің негізгі ережелері жаңа тіркелгіге қосымша рөлдерсіз өз активтерін/NFTs жұмсауға мүмкіндік береді. Код: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Анықтама: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Код: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Код: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Рөл: `{ id, permissions: BTreeSet<Permission> }` құрылысшымен `NewRole { inner: Role, grant_to }`. Код: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Код: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — теңді сәйкестендіру (ашық кілт) және мекенжай. Код: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Триггер: `{ id, action }`. Әрекет: `{ executable, repeats, authority, filter, metadata }`. Код: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` тексерілген кірістіру/алып тастау. Код: `crates/iroha_data_model/src/metadata.rs`.
- Жазылым үлгісі (қолданбалы деңгей): жоспарлар `subscription_plan` метадеректері бар `AssetDefinition` жазбалары; жазылымдар `subscription` метадеректері бар `Nft` жазбалары; шот ұсыну жазылым NFTs сілтеме жасайтын уақыт триггерлерімен орындалады. `docs/source/subscriptions_api.md` және `crates/iroha_data_model/src/subscription.rs` қараңыз.
- **Криптографиялық примитивтер** (`sm` мүмкіндігі):- `Sm2PublicKey` / `Sm2Signature` канондық SEC1 нүктесін + SM2 үшін бекітілген ені `r∥s` кодтауын көрсетеді. Конструкторлар қисық мүшелік пен ерекшеленетін идентификатор семантикасын (`DEFAULT_DISTID`) қамтамасыз етеді, ал тексеру дұрыс емес немесе жоғары ауқымды скалярларды қабылдамайды. Код: `crates/iroha_crypto/src/sm.rs` және `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` GM/T 0004 дайджестін Norito сериялы `[u8; 32]` жаңа түрі ретінде манифесттерде немесе телеметрияда хэштер пайда болған жерде пайдаланылады. Код: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` 128 биттік SM4 кілттерін білдіреді және хост жүйесі қоңыраулары мен деректер үлгісінің құрылғылары арасында ортақ пайдаланылады. Код: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Бұл түрлер бұрыннан бар Ed25519/BLS/ML-DSA примитивтерімен қатар орналасады және `sm` мүмкіндігі қосылғаннан кейін деректер үлгісін тұтынушыларға (Torii, SDK, генезис құралы) қолжетімді болады.

Маңызды белгілер: `Identifiable`, `Registered`/`Registrable` (құрастырушы үлгісі), `HasMetadata`, `IntoKeyValue`. Код: `crates/iroha_data_model/src/lib.rs`.

Оқиғалар: Әрбір нысанда мутацияларда шығарылатын оқиғалар бар (жасау/жою/иесі өзгертілді/метадеректер өзгертілді және т.б.). Код: `crates/iroha_data_model/src/events/`.

---

## Параметрлер (тізбек конфигурациясы)
- Отбасы: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, плюс `custom: BTreeMap`.
- Айырмашылықтар үшін жалғыз сандар: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Агрегатор: `Parameters`. Код: `crates/iroha_data_model/src/parameter/system.rs`.

Параметрлерді орнату (ISI): `SetParameter(Parameter)` сәйкес өрісті жаңартады және `ConfigurationEvent::Changed` шығарады. Код: `crates/iroha_data_model/src/isi/transparent.rs`, `crates/iroha_core/src/smartcontracts/isi/world.rs` ішіндегі орындаушы.

---

## Нұсқауларды сериялау және тізілім
- Негізгі қасиет: `Instruction: Send + Sync + 'static`, `dyn_encode()`, `as_any()`, тұрақты `id()` (әдепкі бойынша бетон түрінің атауы).
- `InstructionBox`: `Box<dyn Instruction>` орауыш. Clone/Eq/Ord `(type_id, encoded_bytes)` жұмыс істейді, сондықтан теңдік мән бойынша болады.
- Norito сериясы `InstructionBox` үшін `(String wire_id, Vec<u8> payload)` ретінде серияланады (егер сым идентификаторы болмаса, `type_name` түріне қайта түседі). Сериясыздандыру конструкторларға ғаламдық `InstructionRegistry` салыстыру идентификаторларын пайдаланады. Әдепкі тізілім барлық кірістірілген ISI қамтиды. Код: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: түрлері, семантикасы, қателері
Орындау `iroha_core::smartcontracts::isi` ішінде `Execute for <Instruction>` арқылы жүзеге асырылады. Төменде жалпыға ортақ әсерлер, алғышарттар, шығарылған оқиғалар және қателер тізімі берілген.

### Тіркеу / Тіркеуден шығару
Түрлері: `Register<T: Registered>` және `Unregister<T: Identifiable>`, нақты нысандарды қамтитын `RegisterBox`/`UnregisterBox` қосынды түрлерімен.

- Register Peer: әлемдік әріптестер жинағына кірістіреді.
  - Алғышарттар: бұрыннан бар болмауы керек.
  - Оқиғалар: `PeerEvent::Added`.
  - Қателер: `Repetition(Register, PeerId)` егер қайталанса; І18NI00000144X іздеуде. Код: `core/.../isi/world.rs`.

- Доменді тіркеу: `NewDomain` бастап `owned_by = authority` көмегімен құрастырылады. Рұқсат етілмеген: `genesis` домені.
  - Алғышарттар: доменнің болмауы; емес `genesis`.
  - Оқиғалар: `DomainEvent::Created`.
  - Қателер: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Код: `core/.../isi/world.rs`.- Тіркелгі тіркелгісі: `genesis` доменінде рұқсат етілмеген `NewAccount` құрастырады; `genesis` тіркелгісін тіркеу мүмкін емес.
  - Алғышарттар: домен болуы керек; шоттың болмауы; генезис доменінде емес.
  - Оқиғалар: `DomainEvent::Account(AccountEvent::Created)`.
  - Қателер: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Код: `core/.../isi/domain.rs`.

- Register AssetDefinition: құрылысшыдан құрастырады; `owned_by = authority` жинағы.
  - Алғышарттар: жоқ болуды анықтау; домен бар.
  - Оқиғалар: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Қателер: `Repetition(Register, AssetDefinitionId)`. Код: `core/.../isi/domain.rs`.

- NFT-ті тіркеу: құрылысшыдан құрастыру; `owned_by = authority` жинағы.
  - Алғышарттар: NFT болмауы; домен бар.
  - Оқиғалар: `DomainEvent::Nft(NftEvent::Created)`.
  - Қателер: `Repetition(Register, NftId)`. Код: `core/.../isi/nft.rs`.

- Тіркеу рөлі: `NewRole { inner, grant_to }` құрастырады (бірінші иесі тіркелгі рөлін салыстыру арқылы жазылған), `inner: Role` сақтайды.
  - Алғышарттар: рөлдің жоқтығы.
  - Оқиғалар: `RoleEvent::Created`.
  - Қателер: `Repetition(Register, RoleId)`. Код: `core/.../isi/world.rs`.

- Триггерді тіркеу: триггерді сүзгі түрі бойынша орнатылған сәйкес триггерде сақтайды.
  - Алдын ала шарттар: сүзгі қолданылмайтын болса, `action.repeats` `Exactly(1)` болуы керек (әйтпесе `MathError::Overflow`). Қайталанатын идентификаторларға тыйым салынады.
  - Оқиғалар: `TriggerEvent::Created(TriggerId)`.
  - Қателер: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` түрлендіру/тексеру сәтсіздігі. Код: `core/.../isi/triggers/mod.rs`.

- Unregister Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger: мақсатты жояды; жою оқиғаларын шығарады. Қосымша каскадты жоюлар:
  - Доменді тіркеуден шығару: домендегі барлық тіркелгілерді, олардың рөлдерін, рұқсаттарын, tx ретті есептегіштерін, тіркелгі белгілерін және UAID байланыстыруларын жояды; олардың активтерін (және әр актив метадеректерін) жояды; домендегі барлық актив анықтамаларын жояды; домендегі NFT-терді және жойылған тіркелгілерге тиесілі кез келген NFT-терді жояды; өкілеттік доменіне сәйкес келетін триггерлерді жояды. Оқиғалар: `DomainEvent::Deleted`, сонымен қатар әрбір элементті жою оқиғалары. Қателер: `FindError::Domain` егер жоқ болса. Код: `core/.../isi/world.rs`.
  - Есептік жазбаны тіркеуден шығару: есептік жазбаның рұқсаттарын, рөлдерін, tx реттілігі есептегішін, тіркелгі белгісін салыстыруды және UAID байланыстыруларын жояды; есептік жазбаға тиесілі активтерді (және әрбір актив метадеректерін) жояды; есептік жазбаға тиесілі NFT-терді жояды; өкілеттігі сол тіркелгі болып табылатын триггерлерді жояды. Оқиғалар: `AccountEvent::Deleted`, плюс жойылған NFT үшін `NftEvent::Deleted`. Қателер: `FindError::Account` егер жоқ болса. Код: `core/.../isi/domain.rs`.
  - AssetDefinition тіркелімін жою: осы анықтаманың барлық активтерін және олардың әрбір актив метадеректерін жояды. Оқиғалар: әр актив үшін `AssetDefinitionEvent::Deleted` және `AssetEvent::Deleted`. Қателер: `FindError::AssetDefinition`. Код: `core/.../isi/domain.rs`.
  - NFT тіркеуден шығару: NFT жойылады. Оқиғалар: `NftEvent::Deleted`. Қателер: `FindError::Nft`. Код: `core/.../isi/nft.rs`.
  - Рөлді тіркеуден шығару: алдымен барлық тіркелгілерден рөлді жояды; содан кейін рөлді алып тастайды. Оқиғалар: `RoleEvent::Deleted`. Қателер: `FindError::Role`. Код: `core/.../isi/world.rs`.
  - Триггерді тіркеуден шығару: егер бар болса, триггерді жояды; қайталанатын тіркеуден шығару кірістері `Repetition(Unregister, TriggerId)`. Оқиғалар: `TriggerEvent::Deleted`. Код: `core/.../isi/triggers/mod.rs`.

### Жалбыз / Күйік
Түрлері: `Mint<O, D: Identifiable>` және `Burn<O, D: Identifiable>`, қорапта `MintBox`/`BurnBox`.- Актив (сандық) жалбыз/жазу: баланстар мен анықтаманың `total_quantity` мәнін реттейді.
  - Алғышарттар: `Numeric` мәні `AssetDefinition.spec()` сәйкес келуі керек; `mintable` рұқсат берген ақша:
    - `Infinitely`: әрқашан рұқсат етіледі.
    - `Once`: бір рет рұқсат етіледі; бірінші жалбыз `mintable` - `Not` және `AssetDefinitionEvent::MintabilityChanged` шығарады, сонымен қатар тексерілу мүмкіндігі үшін егжей-тегжейлі `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`.
    - `Limited(n)`: `n` қосымша ақша операцияларына мүмкіндік береді. Әрбір сәтті жалбыз есептегішті азайтады; ол нөлге жеткенде анықтама `Not` түріне ауысады және жоғарыдағыдай `MintabilityChanged` оқиғаларын шығарады.
    - `Not`: қате `MintabilityError::MintUnmintable`.
  - Күй өзгерістері: теңгеде жоқ болса, активті жасайды; күйген кезде баланс нөлге тең болса, актив жазбасын жояды.
  - Оқиғалар: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (`Once` немесе `Limited(n)` рұқсатын таусылғанда).
  - Қателер: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Код: `core/.../isi/asset.rs`.

- Триггердің қайталануы жалбыз/жану: триггер үшін `action.repeats` өзгерістері.
  - Алғышарттар: жалбызда сүзгі соғатын болуы керек; арифметика толып кетпеуі керек.
  - Оқиғалар: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Қателер: `MathError::Overflow` жарамсыз монетада; Егер жоқ болса, `FindError::Trigger`. Код: `core/.../isi/triggers/mod.rs`.

### Тасымалдау
Түрлері: `Transfer<S: Identifiable, O, D: Identifiable>`, қорапта `TransferBox`.

- Актив (Сандық): `AssetId` көзінен алып тастаңыз, `AssetId` тағайындалған жерге қосыңыз (бірдей анықтама, басқа тіркелгі). Нөлдік бастапқы активті жою.
  - Алғышарттар: бастапқы актив бар; мән `spec` сәйкес келеді.
  - Оқиғалар: `AssetEvent::Removed` (көзі), `AssetEvent::Added` (тағайындалған орын).
  - Қателер: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Код: `core/.../isi/asset.rs`.

- Доменді иелену: `Domain.owned_by` тағайындалған тіркелгіге өзгертеді.
  - Алғышарттар: екі есептік жазба да бар; домен бар.
  - Оқиғалар: `DomainEvent::OwnerChanged`.
  - Қателер: `FindError::Account/Domain`. Код: `core/.../isi/domain.rs`.

- AssetDefinition иелігі: `AssetDefinition.owned_by` тағайындалған тіркелгіге өзгертеді.
  - Алғышарттар: екі шот бар; анықтамасы бар; дереккөз қазіргі уақытта оған ие болуы керек.
  - Оқиғалар: `AssetDefinitionEvent::OwnerChanged`.
  - Қателер: `FindError::Account/AssetDefinition`. Код: `core/.../isi/account.rs`.

- NFT иелігі: `Nft.owned_by` тағайындалған тіркелгіге өзгертеді.
  - Алғышарттар: екі шот бар; NFT бар; дереккөз қазіргі уақытта оған ие болуы керек.
  - Оқиғалар: `NftEvent::OwnerChanged`.
  - Қателер: `FindError::Account/Nft`, `InvariantViolation`, егер дереккөз NFT-ге ие болмаса. Код: `core/.../isi/nft.rs`.

### Метадеректер: Кілт-мәнді орнату/жою
Түрлері: `SetKeyValue<T>` және `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` бар `RemoveKeyValue<T>`. Қораптағы сандар берілген.

- Орнату: `Metadata[key] = Json(value)` кірістіреді немесе ауыстырады.
- Remove: кілтті алып тастайды; жоқ болса қате.
- Оқиғалар: `<Target>Event::MetadataInserted` / `MetadataRemoved` ескі/жаңа мәндерімен.
- Қателер: `FindError::<Target>`, егер мақсат жоқ болса; Жоюға арналған кілт жоқ `FindError::MetadataKey`. Код: `crates/iroha_data_model/src/isi/transparent.rs` және әр мақсатқа орындаушы нұсқауы.### Рұқсаттар мен рөлдер: Беру/қайтарып алу
Түрлері: `Grant<O, D>` және `Revoke<O, D>`, `Permission`/`Role` және `Account` және Norito. Norito.

- Есептік жазбаға рұқсат беру: бұрыннан тән болмаса, `Permission` қосады. Оқиғалар: `AccountEvent::PermissionAdded`. Қателер: `Repetition(Grant, Permission)` егер қайталанса. Код: `core/.../isi/account.rs`.
- Тіркелгіден рұқсатты жою: бар болса, жояды. Оқиғалар: `AccountEvent::PermissionRemoved`. Қателер: `FindError::Permission`, егер жоқ болса. Код: `core/.../isi/account.rs`.
- Тіркелгіге рөл беру: жоқ болса, `(account, role)` салыстыруды кірістіреді. Оқиғалар: `AccountEvent::RoleGranted`. Қателер: `Repetition(Grant, RoleId)`. Код: `core/.../isi/account.rs`.
- Тіркелгіден рөлді жою: бар болса, салыстыруды жояды. Оқиғалар: `AccountEvent::RoleRevoked`. Қателер: `FindError::Role`, егер жоқ болса. Код: `core/.../isi/account.rs`.
- Рөлге рұқсат беру: рұқсат қосылған рөлді қайта құрады. Оқиғалар: `RoleEvent::PermissionAdded`. Қателер: `Repetition(Grant, Permission)`. Код: `core/.../isi/world.rs`.
- Рөлден рұқсатты қайтарып алу: рөлді рұқсатсыз қайта жасайды. Оқиғалар: `RoleEvent::PermissionRemoved`. Қателер: `FindError::Permission`, егер жоқ болса. Код: `core/.../isi/world.rs`.

### Триггерлер: Орындау
Түрі: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Мінез: триггер ішкі жүйесі үшін `ExecuteTriggerEvent { trigger_id, authority, args }` кезекке қояды. Қолмен орындау тек шақыру триггерлері үшін рұқсат етіледі (`ExecuteTrigger` сүзгісі); сүзгі сәйкес келуі керек және қоңырау шалушы триггер әрекетінің уәкілетті органы болуы керек немесе сол өкілеттік үшін `CanExecuteTrigger` ұстауы керек. Пайдаланушы ұсынатын орындаушы белсенді болғанда, триггерді орындау орындалу уақытының орындаушысы арқылы тексеріледі және транзакцияның орындаушысының отын бюджетін тұтынады (негізгі `executor.fuel` және қосымша метадеректер `additional_fuel`).
- Қателер: `FindError::Trigger`, егер тіркелмеген болса; `InvariantViolation`, егер уәкілетті емес орган шақырса. Код: `core/.../isi/triggers/mod.rs` (және `core/.../smartcontracts/isi/mod.rs` жүйесіндегі сынақтар).

### Жаңарту және тіркеу
- `Upgrade { executor }`: берілген `Executor` байт кодын пайдаланып орындаушыны тасымалдайды, орындаушыны және оның деректер үлгісін жаңартады, `ExecutorEvent::Upgraded` шығарады. Қателер: тасымалдау сәтсіздігі кезінде `InvalidParameterError::SmartContract` ретінде оралған. Код: `core/.../isi/world.rs`.
- `Log { level, msg }`: берілген деңгеймен түйін журналын шығарады; күй өзгермейді. Код: `core/.../isi/world.rs`.

### Қате үлгісі
Жалпы конверт: `InstructionExecutionError` бағалау қателеріне, сұрау сәтсіздіктеріне, түрлендірулерге, нысан табылмады, қайталануға, қолданылу мүмкіндігіне, математикаға, жарамсыз параметрге және инвариантты бұзуға арналған нұсқалары бар. Тізімдер мен көмекшілер `crates/iroha_data_model/src/isi/mod.rs` ішінде `pub mod error` астында.

---## Транзакциялар және орындалатын файлдар
- `Executable`: `Instructions(ConstVec<InstructionBox>)` немесе `Ivm(IvmBytecode)`; байт коды base64 ретінде серияланады. Код: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: метадеректермен, `chain_id`, `authority`, `creation_time_ms`, `creation_time_ms`, қосымша I003000, метадеректермен бірге орындалатын файлды құрастырады, белгілейді және бумалайды. `nonce`. Код: `crates/iroha_data_model/src/transaction/`.
- Орындалу уақытында `iroha_core` сәйкес `*Box` немесе нақты нұсқауларға дейін төмендете отырып, `InstructionBox` топтамаларын `Execute for InstructionBox` арқылы орындайды. Код: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Орындау уақытын орындаушыны тексеру бюджеті (пайдаланушы қамтамасыз ететін орындаушы): параметрлерден алынған базалық `executor.fuel` және транзакция ішіндегі нұсқаулық/триггер тексерулері арқылы ортақ `additional_fuel` (`u64`) қосымша транзакция метадеректері.

---

## Инварианттар мен ескертпелер (сынақтар мен қорғаушылардан)
- Жаратылыс қорғаулары: `genesis` доменін немесе `genesis` доменіндегі тіркелгілерді тіркей алмайды; `genesis` тіркелгісін тіркеу мүмкін емес. Код/тесттер: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Сандық активтер өздерінің `NumericSpec` шарттарын беру/беру/жағу кезінде қанағаттандыруы керек; спецификация сәйкес келмеуі `TypeError::AssetNumericSpec`.
- Теңге қабілеттілігі: `Once` бір жалбызды жасауға мүмкіндік береді, содан кейін `Not` түріне ауысады; `Limited(n)` `Not` параметріне аударар алдында дәл `n` теңгелерге рұқсат береді. `Infinitely` құрылғысында соғуға тыйым салу әрекеттері `MintabilityError::ForbidMintOnMintable` тудырады, ал `Limited(0)` конфигурациялау `MintabilityError::InvalidMintabilityTokens` береді.
- Метадеректер операциялары негізгі-дәл; жоқ кілтті жою қате болып табылады.
- Триггер сүзгілері қолданылмайтын болуы мүмкін; онда `Register<Trigger>` тек `Exactly(1)` қайталауға рұқсат береді.
- триггер метадеректер кілті `__enabled` (bool) қақпаларының орындалуы; жоқ әдепкілер қосулы, ал өшірілген триггерлер деректер/уақыт/шақыру жолдары бойынша өткізіп жіберіледі.
- Детерминизм: барлық арифметика тексерілген амалдарды қолданады; under/overflow терілген математикалық қателерді қайтарады; нөлдік қалдықтар актив жазбаларын тастайды (жасырын күй жоқ).

---## Практикалық мысалдар
- соғу және аудару:
  - `Mint::asset_numeric(10, asset_id)` → спецификация/айналу мүмкіндігі рұқсат етілсе, 10 қосады; оқиғалар: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 жылжытады; жою/қосу оқиғалары.
- Метадеректер жаңартулары:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → жоғары; `RemoveKeyValue::account(...)` арқылы жою.
- Рөл/рұқсаттарды басқару:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` және олардың `Revoke` аналогтары.
- Триггердің өмірлік циклі:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`, сүзгі арқылы ұтымдылығын тексеру; `ExecuteTrigger::new(id).with_args(&args)` конфигурацияланған өкілеттікке сәйкес болуы керек.
  - `__enabled` метадеректер кілтін `false` мәніне орнату арқылы триггерлерді өшіруге болады (қосылған әдепкі мәндер жоқ); `SetKeyValue::trigger` немесе IVM `set_trigger_enabled` жүйе қоңырауы арқылы ауыстырыңыз.
  - Жүктеме кезінде триггер жады жөнделеді: қайталанатын идентификаторлар, сәйкес келмейтін идентификаторлар және жетіспейтін байт-кодқа сілтеме жасайтын триггерлер жойылады; байт-код сілтемелерінің саны қайта есептеледі.
  - Орындау уақытында триггердің IVM байт коды жоқ болса, триггер жойылады және орындалу сәтсіздік нәтижесімен жұмыс істемейтін ретінде қарастырылады.
  - Таусылған триггерлер дереу жойылады; егер орындау кезінде таусылған жазба кездессе, ол кесіледі және жоқ болып есептеледі.
- Параметрді жаңарту:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` жаңартылады және `ConfigurationEvent::Changed` шығарады.

---

## Бақылау (таңдалған көздер)
 - Деректер үлгісінің ядросы: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI анықтамалары және тізілімі: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI орындалуы: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Оқиғалар: `crates/iroha_data_model/src/events/**`.
 - Транзакциялар: `crates/iroha_data_model/src/transaction/**`.

Бұл спецификацияны көрсетілген API/мінез-құлық кестесіне кеңейтуді немесе әрбір нақты оқиғаға/қатеге байланыстыруды қаласаңыз, сөзді айтыңыз, мен оны кеңейтемін.