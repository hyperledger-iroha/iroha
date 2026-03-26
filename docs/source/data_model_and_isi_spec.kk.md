---
lang: kk
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
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
Идентификаторларда `Display`/`FromStr` айналу жолымен тұрақты жол пішіндері бар. Атау ережелері бос орынға және сақталған `@ # $` таңбаларына тыйым салады.- `Name` — расталған мәтіндік идентификатор. Ережелер: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Домен: `{ id, logo, metadata, owned_by }`. Құрылысшылар: `NewDomain`. Код: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — канондық мекенжайлар `AccountAddress` (I105 / он алтылық) арқылы шығарылады және Torii `AccountAddress::parse_encoded` арқылы кірістерді қалыпқа келтіреді. I105 - қолайлы тіркелгі пішімі; I105 пішіні тек Sora UX үшін арналған. Таныс `alias` (қабылданбаған бұрынғы пішін) жолы тек бағыттау бүркеншік аты ретінде сақталады. Есептік жазба: `{ id, metadata }`. Код: `crates/iroha_data_model/src/account.rs`.- Тіркелгіні қабылдау саясаты — домендер `iroha:account_admission_policy` метадеректер кілті астында Norito-JSON `AccountAdmissionPolicy` сақтау арқылы жасырын тіркелгі жасауды басқарады. Кілт жоқ кезде, `iroha:default_account_admission_policy` тізбек деңгейіндегі теңшелетін параметр әдепкі мәнді береді; ол да болмаған кезде, қатты әдепкі мән `ImplicitReceive` (бірінші шығарылым) болып табылады. Саясат тегтері `mode` (`ExplicitOnly` немесе `ImplicitReceive`) және қосымша әр транзакция (әдепкі `16`) және әр блокты жасау шектері, қосымша `implicit_creation_fee` тіркелгісі (немесе sink), Актив анықтамасы үшін `min_initial_amounts` және қосымша `default_role_on_create` (`AccountCreated` кейін беріледі, жоқ болса `DefaultRoleError` қабылдамайды). Genesis қосыла алмайды; өшірілген/жарамсыз саясаттар `InstructionExecutionError::AccountAdmission` бар белгісіз тіркелгілерге арналған түбіртек стиліндегі нұсқауларды қабылдамайды. `AccountCreated` алдындағы `iroha:created_via="implicit"` метадеректерінің жасырын тіркелгі мөрі; әдепкі рөлдер `AccountRoleGranted` қосымшасын шығарады және орындаушы иесінің негізгі ережелері жаңа тіркелгіге қосымша рөлдерсіз өз активтерін/NFTs жұмсауға мүмкіндік береді. Код: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — канондық `unprefixed Base58 address with versioning and checksum` (UUID-v4 байт). Анықтама: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. `alias` литералдары `<name>#<domain>.<dataspace>` немесе `<name>#<dataspace>` болуы керек, `<name>` актив анықтамасының атауына тең. Код: `crates/iroha_data_model/src/asset/definition.rs`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`. Alias selectors resolve against the latest committed block creation time and stop resolving after grace even before sweep removes stale bindings.
- `AssetId`: канондық кодталған литерал `<asset-definition-id>#<i105-account-id>` (бұрынғы мәтіндік пішіндерге бірінші шығарылымда қолдау көрсетілмейді).- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Код: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Рөл: `{ id, permissions: BTreeSet<Permission> }` `NewRole { inner: Role, grant_to }` құрастырушымен. Код: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Код: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — теңді сәйкестендіру (ашық кілт) және мекенжай. Код: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Триггер: `{ id, action }`. Әрекет: `{ executable, repeats, authority, filter, metadata }`. Код: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` тексерілген кірістіру/алып тастау. Код: `crates/iroha_data_model/src/metadata.rs`.
- Жазылым үлгісі (қолданбалы деңгей): жоспарлар `subscription_plan` метадеректері бар `AssetDefinition` жазбалары; жазылымдар `subscription` метадеректері бар `Nft` жазбалары; шот ұсыну жазылым NFTs сілтеме жасайтын уақыт триггерлерімен орындалады. `docs/source/subscriptions_api.md` және `crates/iroha_data_model/src/subscription.rs` қараңыз.
- **Криптографиялық примитивтер** (`sm` мүмкіндігі):
  - `Sm2PublicKey` / `Sm2Signature` канондық SEC1 нүктесін + SM2 үшін бекітілген ені `r∥s` кодтауын көрсетеді. Конструкторлар қисық мүшелік пен ажыратушы идентификатор семантикасын (`DEFAULT_DISTID`) қамтамасыз етеді, ал тексеру дұрыс емес немесе жоғары ауқымды скалярларды қабылдамайды. Код: `crates/iroha_crypto/src/sm.rs` және `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` GM/T 0004 дайджестін Norito сериялы `[u8; 32]` жаңа түрі ретінде манифесттерде немесе телеметрияда хэштер пайда болған жерде пайдаланылады. Код: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` 128 биттік SM4 кілттерін білдіреді және хост жүйесі қоңыраулары мен деректер үлгісінің құрылғылары арасында ортақ пайдаланылады. Код: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Бұл түрлер бұрыннан бар Ed25519/BLS/ML-DSA примитивтерімен қатар орналасады және `sm` мүмкіндігі қосылғаннан кейін деректер үлгісін тұтынушыларға (Torii, SDK, генезис құралы) қолжетімді болады.
- Деректер кеңістігінен алынған қатынас қоймалары (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, жолақты релелік төтенше жағдайды қайта анықтау тізілімі) және деректер кеңістігінің мақсатты тіркелгісі рұқсаттары дүкендер) `State::set_nexus(...)` жүйесінде деректер кеңістігі белсенді `dataspace_catalog` ішінен жойылып, орындалу уақыты каталогының жаңартуларынан кейін ескірген деректер кеңістігі сілтемелеріне жол бермейді. Жолақ ауқымы бар DA/релелік кэштер (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) жолақ өшірілгенде немесе басқа деректер кеңістігіне қайта тағайындалғанда кесіледі. Space Directory ISI (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) сондай-ақ `dataspace` белсенді каталогқа қарсы растайды және `InvalidParameter` арқылы белгісіз идентификаторларды қабылдамайды.

Маңызды белгілер: `Identifiable`, `Registered`/`Registrable` (құрастырушы үлгісі), `HasMetadata`, `IntoKeyValue`. Код: `crates/iroha_data_model/src/lib.rs`.

Оқиғалар: Әрбір нысанда мутацияларда шығарылатын оқиғалар бар (жасау/жою/иесі өзгертілді/метадеректер өзгертілді және т.б.). Код: `crates/iroha_data_model/src/events/`.

---## Параметрлер (тізбек конфигурациясы)
- Отбасылар: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, плюс `custom: BTreeMap`.
- Айырмашылықтар үшін жалғыз сандар: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Агрегатор: `Parameters`. Код: `crates/iroha_data_model/src/parameter/system.rs`.

Параметрлерді орнату (ISI): `SetParameter(Parameter)` сәйкес өрісті жаңартады және `ConfigurationEvent::Changed` шығарады. Код: `crates/iroha_data_model/src/isi/transparent.rs`, `crates/iroha_core/src/smartcontracts/isi/world.rs` ішіндегі орындаушы.

---

## Нұсқауларды сериялау және тізілім
- Негізгі қасиет: `Instruction: Send + Sync + 'static`, `dyn_encode()`, `as_any()`, тұрақты `id()` (әдепкі бойынша бетон түрінің атауы).
- `InstructionBox`: `Box<dyn Instruction>` орауыш. Clone/Eq/Ord `(type_id, encoded_bytes)` жұмыс істейді, сондықтан теңдік мән бойынша болады.
- Norito сериясы `InstructionBox` үшін `(String wire_id, Vec<u8> payload)` ретінде серияланады (егер сым идентификаторы болмаса, `type_name` қалпына түседі). Сериясыздандыру конструкторларға ғаламдық `InstructionRegistry` салыстыру идентификаторларын пайдаланады. Әдепкі тізілім барлық кірістірілген ISI қамтиды. Код: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: түрлері, семантикасы, қателері
Орындау `iroha_core::smartcontracts::isi` ішінде `Execute for <Instruction>` арқылы жүзеге асырылады. Төменде жалпыға ортақ әсерлер, алғышарттар, шығарылған оқиғалар және қателер тізімі берілген.

### Тіркеу / Тіркеуден шығару
Түрлері: `Register<T: Registered>` және `Unregister<T: Identifiable>`, нақты мақсаттарды қамтитын `RegisterBox`/`UnregisterBox` қосынды түрлерімен.- Register Peer: әлемдік әріптестер жинағына кірістіреді.
  - Алғышарттар: бұрыннан бар болмауы керек.
  - Оқиғалар: `PeerEvent::Added`.
  - Қателер: `Repetition(Register, PeerId)` егер қайталанса; `FindError` іздеуде. Код: `core/.../isi/world.rs`.

- Доменді тіркеу: `NewDomain` бастап `owned_by = authority` көмегімен құрастырылады. Рұқсат етілмеген: `genesis` домені.
  - Алғышарттар: доменнің болмауы; емес `genesis`.
  - Оқиғалар: `DomainEvent::Created`.
  - Қателер: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Код: `core/.../isi/world.rs`.

- Тіркелгі тіркелгісі: `genesis` доменінде рұқсат етілмеген `NewAccount` құрастырады; `genesis` тіркелгісін тіркеу мүмкін емес.
  - Алғышарттар: домен болуы керек; шоттың болмауы; генезис доменінде емес.
  - Оқиғалар: `DomainEvent::Account(AccountEvent::Created)`.
  - Қателер: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Код: `core/.../isi/domain.rs`.

- Register AssetDefinition: құрылысшыдан құрастырады; `owned_by = authority` жинағы.
  - Алғышарттар: жоқ болуды анықтау; домен бар; `name` қажет, кесілгеннен кейін бос болмауы керек және құрамында `#`/`@` болмауы керек.
  - Оқиғалар: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Қателер: `Repetition(Register, AssetDefinitionId)`. Код: `core/.../isi/domain.rs`.

- NFT-ті тіркеу: құрылысшыдан құрастыру; `owned_by = authority` жинағы.
  - Алғышарттар: NFT болмауы; домен бар.
  - Оқиғалар: `DomainEvent::Nft(NftEvent::Created)`.
  - Қателер: `Repetition(Register, NftId)`. Код: `core/.../isi/nft.rs`.- Тіркеу рөлі: `NewRole { inner, grant_to }` құрастырады (бірінші иесі тіркелгі рөлін салыстыру арқылы жазылған), `inner: Role` сақтайды.
  - Алғышарттар: рөлдің жоқтығы.
  - Оқиғалар: `RoleEvent::Created`.
  - Қателер: `Repetition(Register, RoleId)`. Код: `core/.../isi/world.rs`.

- Триггерді тіркеу: триггерді сүзгі түрі бойынша орнатылған сәйкес триггерде сақтайды.
  - Алдын ала шарттар: сүзгі қолданылмайтын болса, `action.repeats` `Exactly(1)` (әйтпесе `MathError::Overflow`) болуы керек. Қайталанатын идентификаторларға тыйым салынады.
  - Оқиғалар: `TriggerEvent::Created(TriggerId)`.
  - Қателер: түрлендіру/тексеру қателеріндегі `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)`. Код: `core/.../isi/triggers/mod.rs`.- Unregister Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger: мақсатты жояды; жою оқиғаларын шығарады. Қосымша каскадты жоюлар:- Доменді тіркеуден шығару: домен нысанын және оның таңдаушы/мақұлдау саясаты күйін жояды; домендегі актив анықтамаларын (және сол анықтамалар арқылы кілттелген құпия `zk_assets` жанама күйін), сол анықтамалардың активтерін (және әрбір актив метадеректерін), домендегі NFTтерді және домен ауқымындағы тіркелгі белгісі/бүркеншік ат болжамдарын жояды. Ол сондай-ақ жойылған доменнен сақталған тіркелгілердің байланысын жояды және жойылған доменге немесе онымен бірге жойылған ресурстарға (домен рұқсаттары, жойылған анықтамалар үшін актив анықтамасы/актив рұқсаттары және жойылған NFT идентификаторлары үшін NFT рұқсаттары) сілтеме жасайтын тіркелгі/рөл ауқымындағы рұқсат жазбаларын жояды. Доменді жою жаһандық `AccountId`, оның tx-тізбегі/UAID күйі, шетелдік актив немесе NFT иелігі, триггер өкілеттігі немесе сақталған тіркелгіге нұсқайтын басқа сыртқы аудит/конфигурация сілтемелерін жоймайды. Қорғау жолақтары: домендегі кез келген актив анықтамасына репо келісімі, есеп айырысу кітабы, жалпыға ортақ сыйақы/талап, офлайн төлем/аудару, есеп айырысу репосының дефолттары (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), басқару конфигурацияланған дауыс беру/вирустық репо шарты арқылы сілтеме жасалса, бас тартады. актив анықтамасы сілтемелері, oracle-economics конфигурацияланған сыйақы/қиғаш сызық/даулы-облигация активтері анықтамасы сілтемелері немесе Nexus алымы/ставкасы актив анықтамасы анықтамалары (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Оқиғалар: `DomainEvent::Deleted`, сонымен қатар әр элементті жоюжойылған домен ауқымындағы ресурстарға арналған оқиғалар туралы. Қателер: `FindError::Domain` егер жоқ болса; `InvariantViolation` сақталған актив анықтамасының анықтамалық қайшылықтары туралы. Код: `core/.../isi/world.rs`.- Есептік жазбаны тіркеуден шығару: есептік жазбаның рұқсаттарын, рөлдерін, tx реттілігі есептегішін, тіркелгі белгісін салыстыруды және UAID байланыстыруларын жояды; есептік жазбаға тиесілі активтерді (және әрбір актив метадеректерін) жояды; есептік жазбаға тиесілі NFT-терді жояды; авторлығы сол тіркелгі болып табылатын триггерлерді жояды; жойылған тіркелгіге сілтеме жасайтын тіркелгі/рөл ауқымындағы рұқсат жазбаларын, жойылған меншікті NFT идентификаторлары үшін тіркелгі/рөл ауқымындағы NFT мақсат рұқсаттарын және жойылған триггерлер үшін тіркелгі/рөл ауқымындағы триггер мақсатты рұқсаттарын кеседі. Қорғау тақталары: есептік жазба әлі де доменге ие болса, актив анықтамасы, SoraFS провайдерімен байланыстыру, белсенді азаматтық жазба, жалпыға қолжетімді ставка/сыйлық күйі (соның ішінде есептік жазба талапкер немесе сыйақы активінің иесі ретінде көрінетін сыйақы талап ету кілттері), белсенді oracle күйін (соның ішінде twitter-оның oracle-провайдер-провайдерлерін, oracle-провайдерлерін қоса) қабылдамайды. немесе oracle-economics конфигурацияланған сыйақы/қиғаш тіркелгі сілтемелері), белсенді Nexus алымы/ставкасы тіркелгі сілтемелері (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; канондық код ретінде талданған), белсенді доменсіз тіркелгі идентификаторында қайта жіберілген және қате жіберілген репо-келісім күйі, белсенді есеп айырысу журналы күйі, белсенді офлайн жеңілдік/аудару немесе офлайн үкімнің күшін жою күйі, белсенді актив анықтамаларына арналған белсенді офлайн эскроу-шот конфигурациясының сілтемелері (`settlement.offline.escrow_accounts`), белсенді басқару күйі (ұсыныс/кезеңді бекіту)als/locks/slashes/council/парламент тізімдері, ұсыныс парламентінің суреттері, орындау уақытын жаңарту ұсынушы жазбалары, басқару конфигурацияланған эскроу/slash-receiver/вирустық пул тіркелгісінің сілтемелері, басқару SoraFS телеметрия сілтемелері I1090NI / submitter арқылы `gov.sorafs_telemetry.per_provider_submitters` немесе `gov.sorafs_provider_owners` арқылы басқару конфигурацияланған SoraFS провайдер-иесінің сілтемелері, конфигурацияланған мазмұнды жариялауға рұқсат беру тізімі тіркелгі сілтемелері (`content.publish_allow_accounts`), белсенді әлеуметтік эскроу-жіберуші күйі, белсенді мазмұн-күй-жіберуші, белсенді күй-жіберуші, DA-ның иесі. белсенді жолақ релесі апаттық валидаторды қайта анықтау күйі немесе белсенді SoraFS пин-тізілім эмитенті/байланыстырушы жазбалары (пин манифесттері, манифест бүркеншік аттары, репликация тапсырыстары). Оқиғалар: `AccountEvent::Deleted`, плюс жойылған NFT үшін `NftEvent::Deleted`. Қателер: `FindError::Account` егер жоқ болса; `InvariantViolation` меншігіндегі жетім балалар. Код: `core/.../isi/domain.rs`.- AssetDefinition тіркелімін жою: осы анықтаманың барлық активтерін және олардың әрбір актив метадеректерін жояды және осы анықтамамен кілттелген құпия `zk_assets` жанама күйін жояды; сонымен қатар жойылған актив анықтамасына немесе оның актив даналарына сілтеме жасайтын сәйкес `settlement.offline.escrow_accounts` жазбасын және тіркелгі/рөл ауқымындағы рұқсат жазбаларын кеседі. Күзет жолақтары: анықтамаға репо келісімі, есеп айырысу кітабы, жалпыға ортақ сыйақы/талап, офлайн төлем/аудару күйі, есеп айырысу репо дефолттары (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), басқару конфигурацияланған дауыс беру/азаматтық-құқықтық қатынас арқылы сілтеме жасалған кезде бас тартады. актив анықтамасы сілтемелері, oracle-economics конфигурацияланған сыйақы/қиғаш сызық/даулы-облигация активтері анықтамасы сілтемелері немесе Nexus алымы/ставкасы актив анықтамасы анықтамалары (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Оқиғалар: әр актив үшін `AssetDefinitionEvent::Deleted` және `AssetEvent::Deleted`. Қателер: сілтеме қайшылықтары бойынша `FindError::AssetDefinition`, `InvariantViolation`. Код: `core/.../isi/domain.rs`.
  - NFT-ті тіркеуден шығару: NFT-ті жояды және жойылған NFT-ке сілтеме жасайтын тіркелгі/рөл ауқымындағы рұқсат жазбаларын кеседі. Оқиғалар: `NftEvent::Deleted`. Қателер: `FindError::Nft`. Код: `core/.../isi/nft.rs`.
  - Рөлді тіркеуден шығару: алдымен барлық тіркелгілерден рөлді жояды; содан кейін рөлді алып тастайды. Оқиғалар: `RoleEvent::Deleted`. Қателер: `FindError::Role`. Код: `core/.../isi/world.rs`.- Триггерді тіркеуден шығару: бар болса, триггерді жояды және жойылған триггерге сілтеме жасайтын тіркелгі/рөл ауқымындағы рұқсат жазбаларын қысқартады; қайталанатын тіркеуден шығару кірістері `Repetition(Unregister, TriggerId)`. Оқиғалар: `TriggerEvent::Deleted`. Код: `core/.../isi/triggers/mod.rs`.

### Жалбыз / Күйік
Түрлері: `Mint<O, D: Identifiable>` және `Burn<O, D: Identifiable>`, қорапта `MintBox`/`BurnBox`.

- Актив (сандық) жалбыз/жазу: баланстар мен анықтаманың `total_quantity` мәнін реттейді.
  - Алғышарттар: `Numeric` мәні `AssetDefinition.spec()` сәйкес келуі керек; `mintable` рұқсат берген ақша:
    - `Infinitely`: әрқашан рұқсат етіледі.
    - `Once`: бір рет рұқсат етіледі; бірінші жалбыз `mintable`-ті `Not`-ке аударады және `AssetDefinitionEvent::MintabilityChanged` шығарады, сонымен қатар тексерілу мүмкіндігі үшін егжей-тегжейлі `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`.
    - `Limited(n)`: `n` қосымша ақша операцияларына мүмкіндік береді. Әрбір сәтті жалбыз есептегішті азайтады; ол нөлге жеткенде анықтама `Not` түріне ауысады және жоғарыдағыдай `MintabilityChanged` оқиғаларын шығарады.
    - `Not`: қате `MintabilityError::MintUnmintable`.
  - Күй өзгерістері: теңгеде жоқ болса, активті жасайды; күйген кезде баланс нөлге тең болса, актив жазбасын жояды.
  - Оқиғалар: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (`Once` немесе `Limited(n)` рұқсатын таусылғанда).
  - Қателер: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Код: `core/.../isi/asset.rs`.- Триггердің қайталануы жалбыз/жану: триггер үшін `action.repeats` өзгерістері.
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
  - Алғышарттар: екі есептік жазба да бар; анықтамасы бар; дереккөз қазіргі уақытта оған ие болуы керек; өкілеттік бастапқы тіркелгі, бастапқы домен иесі немесе актив анықтамасы-домен иесі болуы керек.
  - Оқиғалар: `AssetDefinitionEvent::OwnerChanged`.
  - Қателер: `FindError::Account/AssetDefinition`. Код: `core/.../isi/account.rs`.- NFT иелігі: `Nft.owned_by` тағайындалған тіркелгіге өзгертеді.
  - Алғышарттар: екі есептік жазба да бар; NFT бар; дереккөз қазіргі уақытта оған ие болуы керек; уәкілетті орган бастапқы тіркелгі, бастапқы домен иесі, NFT доменінің иесі немесе сол NFT үшін `CanTransferNft` ұстауы керек.
  - Оқиғалар: `NftEvent::OwnerChanged`.
  - Қателер: `FindError::Account/Nft`, `InvariantViolation`, егер көз NFT-ге ие болмаса. Код: `core/.../isi/nft.rs`.

### Метадеректер: Кілт-мәнді орнату/жою
Түрлері: `SetKeyValue<T>` және `RemoveKeyValue<T>` және `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Қораптағы сандар берілген.

- Орнату: `Metadata[key] = Json(value)` кірістіреді немесе ауыстырады.
- Remove: кілтті алып тастайды; жоқ болса қате.
- Оқиғалар: `<Target>Event::MetadataInserted` / `MetadataRemoved` ескі/жаңа мәндері бар.
- Қателер: `FindError::<Target>`, егер мақсат жоқ болса; Жоюға арналған кілт жоқ `FindError::MetadataKey`. Код: `crates/iroha_data_model/src/isi/transparent.rs` және әр мақсатқа орындаушы нұсқауы.

### Рұқсаттар мен рөлдер: Беру/қайтару
Түрлері: `Grant<O, D>` және `Revoke<O, D>`, `Permission`/`Role` және `Account` және Norito. `TriggerEvent::Created(TriggerId)` және Norito.- Есептік жазбаға рұқсат беру: бұрыннан тән болмаса, `Permission` қосады. Оқиғалар: `AccountEvent::PermissionAdded`. Қателер: `Repetition(Grant, Permission)` егер қайталанса. Код: `core/.../isi/account.rs`.
- Тіркелгіден рұқсатты жою: бар болса, жояды. Оқиғалар: `AccountEvent::PermissionRemoved`. Қателер: `FindError::Permission`, егер жоқ болса. Код: `core/.../isi/account.rs`.
- Тіркелгіге рөл беру: жоқ болса, `(account, role)` салыстыруды кірістіреді. Оқиғалар: `AccountEvent::RoleGranted`. Қателер: `Repetition(Grant, RoleId)`. Код: `core/.../isi/account.rs`.
- Тіркелгіден рөлді жою: бар болса, салыстыруды жояды. Оқиғалар: `AccountEvent::RoleRevoked`. Қателер: `FindError::Role`, егер жоқ болса. Код: `core/.../isi/account.rs`.
- Рөлге рұқсат беру: рұқсат қосылған рөлді қайта құрады. Оқиғалар: `RoleEvent::PermissionAdded`. Қателер: `Repetition(Grant, Permission)`. Код: `core/.../isi/world.rs`.
- Рөлден рұқсатты қайтарып алу: рөлді рұқсатсыз қайта жасайды. Оқиғалар: `RoleEvent::PermissionRemoved`. Қателер: `FindError::Permission`, егер жоқ болса. Код: `core/.../isi/world.rs`.### Триггерлер: Орындау
Түрі: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Мінез: триггер ішкі жүйесі үшін `ExecuteTriggerEvent { trigger_id, authority, args }` кезекке қояды. Қолмен орындау тек шақыру триггерлері үшін рұқсат етіледі (`ExecuteTrigger` сүзгісі); сүзгі сәйкес келуі керек және қоңырау шалушы триггер әрекетінің уәкілетті органы болуы керек немесе сол өкілеттік үшін `CanExecuteTrigger` ұстауы керек. Пайдаланушы ұсынатын орындаушы белсенді болғанда, триггерді орындау орындалу уақытын орындаушы арқылы тексеріледі және транзакцияны орындаушының отын бюджетін тұтынады (негізгі `executor.fuel` және қосымша метадеректер `additional_fuel`).
- Қателер: `FindError::Trigger`, егер тіркелмеген болса; `InvariantViolation`, егер уәкілетті емес орган шақырса. Код: `core/.../isi/triggers/mod.rs` (және `core/.../smartcontracts/isi/mod.rs` жүйесіндегі сынақтар).

### Жаңарту және тіркеу
- `Upgrade { executor }`: берілген `Executor` байт кодын пайдаланып орындаушыны тасымалдайды, орындаушыны және оның деректер үлгісін жаңартады, `ExecutorEvent::Upgraded` шығарады. Қателер: тасымалдау сәтсіздігі кезінде `InvalidParameterError::SmartContract` ретінде оралған. Код: `core/.../isi/world.rs`.
- `Log { level, msg }`: берілген деңгеймен түйін журналын шығарады; күй өзгермейді. Код: `core/.../isi/world.rs`.

### Қате үлгісі
Жалпы конверт: `InstructionExecutionError` бағалау қателеріне, сұрау сәтсіздіктеріне, түрлендірулерге, табылмады нысан, қайталануға, қолданылу мүмкіндігіне, математикаға, жарамсыз параметрге және инвариантты бұзуға арналған нұсқалары бар. Тізімдер мен көмекшілер `crates/iroha_data_model/src/isi/mod.rs` ішінде `pub mod error` астында.

---## Транзакциялар және орындалатын файлдар
- `Executable`: `Instructions(ConstVec<InstructionBox>)` немесе `Ivm(IvmBytecode)`; байт коды base64 ретінде серияланады. Код: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: метадеректермен, `chain_id`, `authority`, `creation_time_ms`, `creation_time_ms`, қосымша I10300, метадеректері бар орындалатын файлды құрастырады, белгілейді және бумалайды. `nonce`. Код: `crates/iroha_data_model/src/transaction/`.
- Орындалу уақытында `iroha_core` `InstructionBox` топтамаларын `Execute for InstructionBox` арқылы сәйкес `*Box` немесе нақты нұсқауларға дейін төмендетеді. Код: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Орындау уақытын орындаушыны тексеру бюджеті (пайдаланушы қамтамасыз ететін орындаушы): параметрлерден алынған базалық `executor.fuel` және транзакция ішіндегі нұсқаулық/триггер тексерулері арқылы ортақ `additional_fuel` (`u64`) қосымша транзакция метадеректері.

---## Инварианттар мен ескертпелер (сынақтар мен қорғаушылардан)
- Жаратылыс қорғаулары: `genesis` доменін немесе `genesis` доменіндегі тіркелгілерді тіркей алмайды; `genesis` тіркелгісін тіркеу мүмкін емес. Код/тесттер: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Сандық активтер өздерінің `NumericSpec` шарттарын беру/беру/жағу кезінде қанағаттандыруы керек; спецификацияның сәйкессіздігі `TypeError::AssetNumericSpec` береді.
- Теңге қабілеттілігі: `Once` бір жалбызды жасауға мүмкіндік береді, содан кейін `Not` түріне ауысады; `Limited(n)` `Not` параметріне аударар алдында дәл `n` теңгеге рұқсат береді. `Infinitely` құрылғысында ақша шығаруға тыйым салу әрекеттері `MintabilityError::ForbidMintOnMintable` тудырады, ал `Limited(0)` конфигурациялау `MintabilityError::InvalidMintabilityTokens` береді.
- Метадеректер операциялары негізгі-дәл; жоқ кілтті жою қате болып табылады.
- Триггер сүзгілері қолданылмайтын болуы мүмкін; онда `Register<Trigger>` тек `Exactly(1)` қайталауға рұқсат береді.
- триггер метадеректер кілті `__enabled` (bool) қақпаларының орындалуы; жоқ әдепкілер қосулы, ал өшірілген триггерлер деректер/уақыт/шақыру жолдары бойынша өткізіп жіберіледі.
- Детерминизм: барлық арифметика тексерілген амалдарды қолданады; under/overflow терілген математикалық қателерді қайтарады; нөлдік қалдықтар актив жазбаларын тастайды (жасырын күй жоқ).

---## Практикалық мысалдар
- соғу және аудару:
  - `Mint::asset_numeric(10, asset_id)` → спецификация/айналу мүмкіндігі рұқсат етілсе, 10 қосады; оқиғалар: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 жылжытады; жою/қосу оқиғалары.
- Метадеректер жаңартулары:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → жоғарғы; `RemoveKeyValue::account(...)` арқылы жою.
- Рөл/рұқсаттарды басқару:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` және олардың `Revoke` аналогтары.
- Триггердің өмірлік циклі:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`, сүзгі арқылы ұтымдылығын тексеру; `ExecuteTrigger::new(id).with_args(&args)` конфигурацияланған өкілеттікке сәйкес болуы керек.
  - `__enabled` метадеректер кілтін `false` мәніне орнату арқылы триггерлерді өшіруге болады (қосылған әдепкі мәндер жоқ); `SetKeyValue::trigger` немесе IVM `set_trigger_enabled` жүйе қоңырауы арқылы ауыстырыңыз.
  - Жүктеме кезінде триггер жады жөнделеді: қайталанатын идентификаторлар, сәйкес келмейтін идентификаторлар және жетіспейтін байт-кодқа сілтеме жасайтын триггерлер жойылады; байт-код сілтемелерінің саны қайта есептеледі.
  - Орындау уақытында триггердің IVM байт коды жоқ болса, триггер жойылады және орындалу сәтсіздік нәтижесімен жұмыс істемейтін ретінде қарастырылады.
  - Таусылған триггерлер дереу жойылады; егер орындау кезінде таусылған жазба кездессе, ол кесіледі және жоқ болып есептеледі.
- Параметрді жаңарту:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` жаңартылады және `ConfigurationEvent::Changed` шығарады.CLI / Torii asset-definition id + бүркеншік ат мысалдары:
- Канондық көмек + нақты атау + ұзын бүркеншік атпен тіркелу:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- Канондық көмек + нақты атау + қысқа бүркеншік атпен тіркелу:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- Бүркеншік атпен жалбыз + тіркелгі құрамдастары:
  - `iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- Канондық көмекке бүркеншік аттарды шешіңіз:
  - JSON `{ "alias": "pkr#ubl.sbp" }` бар `POST /v1/assets/aliases/resolve`

Көшіру жазбасы:
- `name#domain` мәтіндік актив анықтамасының идентификаторларына бірінші шығарылымда әдейі қолдау көрсетілмейді.
- Жалға беру/жаю/тасымалдау шекараларындағы актив идентификаторлары канондық `<asset-definition-id>#<i105-account-id>` болып қалады; `iroha tools encode asset-id` `--definition <base58-asset-definition-id>` немесе `--alias ...` плюс `--account` арқылы пайдаланыңыз.

---

## Бақылау (таңдалған көздер)
 - Деректер үлгісінің ядросы: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI анықтамалары және тізілімі: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI орындалуы: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Оқиғалар: `crates/iroha_data_model/src/events/**`.
 - Транзакциялар: `crates/iroha_data_model/src/transaction/**`.

Бұл спецификацияны көрсетілген API/мінез-құлық кестесіне кеңейтуді немесе әрбір нақты оқиғаға/қатеге байланыстыруды қаласаңыз, сөзді айтыңыз, мен оны кеңейтемін.