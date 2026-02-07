---
lang: kk
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Деректер үлгісі – Терең сүңгу

Бұл құжат `iroha_data_model` жәшігінде енгізілген және жұмыс кеңістігінде пайдаланылатын Iroha v2 деректер үлгісін құрайтын құрылымдарды, идентификаторларды, белгілерді және протоколдарды түсіндіреді. Бұл сіз қарап шығуға және жаңартуларды ұсына алатын нақты анықтама болуы керек.

## Қолдану аясы мен негіздері

- Мақсаты: домен нысандары (домендер, тіркелгілер, активтер, NFTs, рөлдер, рұқсаттар, тең дәрежелер), күйді өзгертетін нұсқаулар (ISI), сұраулар, триггерлер, транзакциялар, блоктар және параметрлер үшін канондық түрлерді қамтамасыз ету.
- Серияландыру: барлық жалпы түрлер Norito кодектерін (`norito::codec::{Encode, Decode}`) және схемасын (`iroha_schema::IntoSchema`) шығарады. JSON таңдаулы түрде (мысалы, HTTP және `Json` пайдалы жүктемелері үшін) мүмкіндік жалауларының артында пайдаланылады.
- IVM ескертпе: Iroha виртуалды машинасына (IVM) бағытталған кезде белгілі сериясыздандыру уақытының валидациялары өшіріледі, себебі хост келісім-шарттарды шақырмас бұрын тексеруді орындайды (Norito ішіндегі жәшік құжаттарын қараңыз).
- FFI қақпалары: FFI қажет болмаған кезде үстеме шығындарды болдырмау үшін `ffi_export`/`ffi_import` артында `iroha_ffi` арқылы FFI үшін кейбір түрлер шартты түрде түсіндіріледі.

## Негізгі қасиеттер мен көмекшілер

- `Identifiable`: Нысандарда тұрақты `Id` және `fn id(&self) -> &Self::Id` бар. Карта/жиынтық достық үшін `IdEqOrdHash` арқылы алынуы керек.
- `Registrable`/`Registered`: Көптеген нысандар (мысалы, `Domain`, `AssetDefinition`, `Role`) құрастырушы үлгісін пайдаланады. `Registered` орындау уақыты түрін тіркеу транзакциялары үшін қолайлы жеңіл құрастырушы түріне (`With`) байланыстырады.
- `HasMetadata`: `Metadata` кілтіне/мәніне бірыңғай рұқсат.
- `IntoKeyValue`: қайталануды азайту үшін `Key` (ID) және `Value` (деректер) бөлек сақтауға арналған сақтауды бөлу көмекшісі.
- `Owned<T>`/`Ref<'world, K, V>`: қажетсіз көшірмелерді болдырмау үшін қоймалар мен сұрау сүзгілерінде қолданылатын жеңіл орамдар.

## Аттар мен идентификаторлар

- `Name`: жарамды мәтін идентификаторы. Бос орынға және `@`, `#`, `$` (құрама идентификаторларда пайдаланылады) сақталған таңбаларға рұқсат бермейді. Валидациясы бар `FromStr` арқылы құрастырылады. Атаулар талдау кезінде Юникод NFC стандартына қалыпқа келтіріледі (канондық эквивалентті емлелер бірдей болып саналады және құрастырылған түрде сақталады). `genesis` арнайы атауы сақталған (регистрді ескермей белгіленеді).
- `IdBox`: Қолдау көрсетілетін кез келген идентификаторға арналған жиынтық түрдегі конверт (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama, Kotodama `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Жалпы ағындар мен Norito кодтауы жалғыз түр ретінде пайдалы.
- `ChainId`: транзакцияларда қайта ойнатудан қорғау үшін пайдаланылатын мөлдір емес тізбек идентификаторы.Идентификаторлардың жолдық пішіндері (`Display`/`FromStr` арқылы екі рет айналуға болады):
- `DomainId`: `name` (мысалы, `wonderland`).
- `AccountId`: IH58, Sora қысылған (`sora…`) және канондық он алтылық кодектер (`AccountAddress::to_ih58`, I1000030, Kotodama, `AccountAddress` арқылы кодталған канондық идентификатор. `canonical_hex`, `parse_any`). IH58 - қолайлы тіркелгі пішімі; `sora…` пішіні тек Sora UX үшін екінші ең жақсы. Адамға қолайлы `alias@domain` маршрутизаторының бүркеншік аты UX үшін сақталған, бірақ енді беделді идентификатор ретінде қарастырылмайды. Torii кіріс жолдарын `AccountAddress::parse_any` арқылы қалыпқа келтіреді. Тіркелгі идентификаторлары бір кілтті де, мультисигті контроллерді де қолдайды.
- `AssetDefinitionId`: `asset#domain` (мысалы, `xor#soramitsu`).
- `AssetId`: `asset#domain#account` немесе анықтау домені тіркелгі доменіне тең болса, `asset##account` стенографиясы, мұнда `account` канондық `AccountId` жолы (IH58 қолайлы).
- `NftId`: `nft$domain` (мысалы, `rose$garden`).
- `PeerId`: `public_key` (тең теңдік ашық кілт арқылы жүзеге асырылады).

## Нысандар

### Домен
- `DomainId { name: Name }` – бірегей атау.
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Builder: `NewDomain` `with_logo`, `with_metadata`, содан кейін `Registrable::build(authority)` `owned_by` жинақтары.

### Тіркелгі
- `AccountId { domain: DomainId, controller: AccountController }` (контроллер = бір кілт немесе мультисиг саясаты).
- `Account { id, metadata, label?, uaid? }` — `label` — қайта енгізу жазбалары пайдаланатын қосымша тұрақты бүркеншік ат, `uaid` кең ауқымда қосымша Nexus [Әмбебап тіркелгі идентификаторы](Kotodama бағдарламасымен Norito) бар.
- Құрылысшы: `Account::new(id)` арқылы `NewAccount`; `HasMetadata` құрылысшы мен ұйым үшін.

### Актив анықтамалары және активтер
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Құрылысшылар: `AssetDefinition::new(id, spec)` немесе ыңғайлылық `numeric(id)`; `metadata`, `mintable`, `owned_by` үшін реттеуіштер.
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }`, сақтауға ыңғайлы `AssetEntry`/`AssetValue`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` жиынтық API интерфейстері үшін ашық.

### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (мазмұн - ерікті кілт/мән метадеректері).
- Құрылысшы: `Nft::new(id, content)` арқылы `NewNft`.

### Рөлдер мен рұқсаттар
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` `NewRole { inner: Role, grant_to: AccountId }` құрылысшымен.
- `Permission { name: Ident, payload: Json }` – `name` және пайдалы жүктеме схемасы белсенді `ExecutorDataModel` (төменде қараңыз) сәйкес келуі керек.

### Құрдастар
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` және талданатын `public_key@address` жол пішіні.### Криптографиялық примитивтер (`sm` мүмкіндігі)
- `Sm2PublicKey` және `Sm2Signature`: SM2 үшін SEC1-үйлесімді нүктелер және бекітілген ені `r∥s` қолтаңбалары. Конструкторлар қисық мүшелік пен ажыратушы идентификаторларды тексереді; Norito кодтауы `iroha_crypto` пайдаланатын канондық көріністі көрсетеді.
- `Sm3Hash`: GM/T 0004 дайджестін білдіретін `[u8; 32]` жаңа түрі, манифесттерде, телеметрияда және жүйе жауаптарында қолданылады.
- `Sm4Key`: 128 биттік симметриялы кілт ораушысы хост жүйесі қоңыраулары мен деректер үлгісінің құрылғылары арасында ортақ.
Бұл түрлер бар Ed25519/BLS/ML-DSA примитивтерімен қатар орналасады және жұмыс кеңістігі `--features sm` көмегімен салынғаннан кейін жалпы схеманың бөлігі болады.

### Триггерлер мен оқиғалар
- `TriggerId { name: Name }` және `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` немесе `Exactly(u32)`; тапсырыс беру және сарқылу утилиталары кіреді.
  - Қауіпсіздік: `TriggerCompleted` әрекет сүзгісі ретінде пайдаланылмайды (серияландыру кезінде расталған).
- `EventBox`: құбыр желісі, конвейер-партиясы, деректер, уақыт, орындау-триггер және триггер-аяқталған оқиғаларға арналған қосынды түрі; `EventFilterBox` жазылымдар мен триггер сүзгілерін көрсетеді.

## Параметрлер және конфигурация

- Жүйе параметрлерінің отбасылары (барлық `Default`ed, алушыларды тасымалдау және жеке сандарға түрлендіру):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` барлық отбасыларды және `custom: BTreeMap<CustomParameterId, CustomParameter>` топтарын топтайды.
- Бір параметрді сандар: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` дифференциалды жаңартулар мен итерация үшін.
- Пайдаланушы параметрлері: орындаушы анықтаған, `Json` ретінде тасымалданған, `CustomParameterId` (a `Name`) арқылы анықталған.

## ISI (Iroha Арнайы нұсқаулар)

- Негізгі қасиет: `Instruction`, `dyn_encode`, `as_any` және әр түрге арналған тұрақты идентификатор `id()` (әдепкі бойынша бетон түрінің атауы). Барлық нұсқаулар `Send + Sync + 'static`.
- `InstructionBox`: ID түрі + кодталған байттар арқылы жүзеге асырылатын клон/экв/орд бар `Box<dyn Instruction>` орамасы.
- Кіріктірілген оқу отбасылары келесідей ұйымдастырылады:
  - `mint_burn`, `transfer`, `register` және `transparent` көмекшілер жинағы.
  - Мета ағындары үшін нөмірлерді теріңіз: `InstructionType`, `SetKeyValueBox` (domain/account/asset_def/nft/trigger) сияқты қорапшалар.
- Қателер: `isi::error` астындағы бай қате үлгісі (бағалау түріндегі қателер, қателерді табу, есептеу мүмкіндігі, математика, жарамсыз параметрлер, қайталау, инварианттар).
- Нұсқаулар тізілімі: `instruction_registry!{ ... }` макросы түр атауы бойынша кілттелген орындау уақытының декодтау тізбесін құрады. `InstructionBox` клонында және Norito серде динамикалық (де) сериялауға қол жеткізу үшін пайдаланылады. Ешбір тізбе `set_instruction_registry(...)` арқылы анық орнатылмаған болса, екілік файлдардың сенімді болуы үшін бірінші рет пайдаланған кезде барлық негізгі ISI бар кірістірілген әдепкі тізбе жалқаулықпен орнатылады.

## транзакциялар- `Executable`: `Instructions(ConstVec<InstructionBox>)` немесе `Ivm(IvmBytecode)`. `IvmBytecode` base64 (`Vec<u8>` үстіндегі мөлдір жаңа тип) ретінде серияланады.
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, қосымша `time_to_live_ms` және `nonce`, `nonce`, `nonce`, I1010, `authority`, `creation_time_ms` арқылы транзакцияның пайдалы жүктемесін құрастырады `Executable`.
  - Көмекшілер: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Kotodama.
- `SignedTransaction` (`iroha_version` нұсқасымен): `TransactionSignature` және пайдалы жүкті тасымалдайды; хэштеу және қолтаңбаны тексеруді қамтамасыз етеді.
- Кіру нүктелері және нәтижелер:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` хэштеу көмекшілері бар.
  - `ExecutionStep(ConstVec<InstructionBox>)`: транзакциядағы нұсқаулардың бір реттелген партиясы.

## Блоктар

- `SignedBlock` (нұсқасы) инкапсулалар:
  - `signatures: BTreeSet<BlockSignature>` (валидаторлардан),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (екінші орындалу күйі) құрамында `time_triggers`, кіріс/нәтиже Merkle ағаштары, `transaction_results` және `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Утилиталар: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, I123NI00X00.
- Merkle тамыры: транзакцияның кіру нүктелері мен нәтижелері Merkle ағаштары арқылы жасалады; нәтиже Merkle түбірі блок тақырыбына орналастырылады.
- Блокты қосу дәлелдері (`BlockProofs`) енгізу/нәтиже Merkle дәлелдерін де, `fastpq_transcripts` картасын да көрсетеді, осылайша тізбектен тыс провайдерлер транзакция хэшімен байланысты тасымалдау дельталарын ала алады.
- `ExecWitness` хабарлары (Torii арқылы таратылады және консенсус өсектеріне негізделген) енді `fastpq_transcripts` және кірістірілген `fastpq_batches: Vec<FastpqTransitionBatch>` түбірлері бар `fastpq_batches: Vec<FastpqTransitionBatch>` (Kotodama, slotlar, perm_root, tx_set_hash), сондықтан сыртқы дәлелдеушілер транскрипттерді қайта кодтаусыз канондық FASTPQ жолдарын қабылдай алады.

## Сұраулар

- Екі дәм:
  - Сингулярлы: `SingularQuery<Output>` іске қосыңыз (мысалы, `FindParameters`, `FindExecutorDataModel`).
  - Қайталанатын: `Query<Item>` іске қосыңыз (мысалы, `FindAccounts`, `FindAssets`, `FindDomains` және т.б.).
- Түрі бойынша өшірілген пішіндер:
  - `QueryBox<T>` - жаһандық тізіліммен қамтамасыз етілген Norito сердесі бар қораптағы, өшірілген `Query<Item = T>`.
  - `QueryWithFilter<T> { query, predicate, selector }` сұрауды DSL предикаты/селекторымен жұптайды; `From` арқылы өшірілген қайталанатын сұрауға түрлендіреді.
- Тізілім және кодектер:
  - `query_registry!{ ... }` динамикалық декодтау үшін түр атауы бойынша конструкторларға нақты сұрау түрлерін салыстыратын жаһандық тізілімді құрады.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` және `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` - біртекті векторлар (мысалы, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), плюс кортеж және кеңейту анықтамалары үшін қосынды түрі.
- DSL: `query::dsl` жүйесінде компиляция уақыты бойынша тексерілетін предикаттар мен селекторлар үшін проекция белгілерімен (`HasProjection<PredicateMarker>` / `SelectorMarker`) енгізілген. `fast_dsl` мүмкіндігі қажет болса, жеңілірек нұсқаны көрсетеді.

## Орындаушы және кеңейту мүмкіндігі- `Executor { bytecode: IvmBytecode }`: валидатор орындайтын код жинағы.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` орындаушы анықтаған доменді жариялайды:
  - теңшелетін конфигурация параметрлері,
  - Теңшелетін нұсқаулық идентификаторлары,
  - Рұқсат белгісінің идентификаторлары,
  - Клиент құралына арналған пайдаланушы түрлерін сипаттайтын JSON схемасы.
- Теңшеу үлгілері `data_model/samples/executor_custom_data_model` астында бар, ол көрсетеді:
  - `iroha_executor_data_model::permission::Permission` туындысы арқылы реттелетін рұқсат белгісі,
  - `CustomParameter` түрлендірілетін түр ретінде анықталған теңшелетін параметр,
  - Орындау үшін `CustomInstruction` ішіне серияланған теңшелетін нұсқаулар.

### CustomInstruction (орындаушы анықтайтын ISI)

- Түрі: `"iroha.custom"` тұрақты сым идентификаторы бар `isi::CustomInstruction { payload: Json }`.
- Мақсаты: жеке/консорциум желілеріндегі орындаушыға арналған нұсқауларға арналған конверт немесе жалпы деректер үлгісін бұзбай прототиптеу үшін.
- Әдепкі орындаушының әрекеті: `iroha_core` ішіндегі кірістірілген орындаушы `CustomInstruction` орындамайды және кездескен жағдайда үрейленеді. Теңшелетін орындаушы `InstructionBox` нұсқасын `CustomInstruction` дейін төмендетуі және барлық валидаторлардағы пайдалы жүктемені анықтауы керек.
- Norito: схемасы қосылған `norito::codec::{Encode, Decode}` арқылы кодтайды/декодтайды; `Json` пайдалы жүктемесі детерминирленген түрде серияланады. Нұсқаулар тізілімінде `CustomInstruction` (ол әдепкі тізілімнің бөлігі болып табылады) болғанша, айналма сапарлар тұрақты болады.
- IVM: Kotodama IVM байт кодына (`.to`) құрастырады және қолданба логикасы үшін ұсынылған жол болып табылады. `CustomInstruction` әлі Kotodama түрінде көрсетілмейтін орындаушы деңгейіндегі кеңейтімдер үшін ғана пайдаланыңыз. Детерминизмді және әріптестер арасында бірдей орындаушының екілік файлдарын қамтамасыз етіңіз.
- Қоғамдық желілер үшін емес: біртекті емес орындаушылар консенсус шанышқыларына қауіп төндіретін қоғамдық желілер үшін пайдаланбаңыз. Платформа мүмкіндіктері қажет болғанда жаңа кірістірілген ISI ағынын ұсыныңыз.

## Метадеректер

- `Metadata(BTreeMap<Name, Json>)`: бірнеше нысандарға тіркелген кілт/құн қоймасы (`Domain`, `Account`, `AssetDefinition`, `Nft`, триггерлер және транзакциялар).
- API: `contains`, `iter`, `get`, `insert`, және (`transparent_api` бар) `remove`.

## Ерекшеліктер және детерминизм

- Мүмкіндіктер қосымша API интерфейстерін басқарады (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `fast_dsl`, I0030X `fault_injection`).
- Детерминизм: барлық сериялау аппараттық құрал арқылы тасымалдануы үшін Norito кодтауын пайдаланады. IVM байт коды мөлдір емес байт блобы; орындау детерминирленген емес қысқартуларды енгізбеуі керек. Хост транзакцияларды тексереді және анықтаушы түрде IVM кірістерін береді.

### Transparent API (`transparent_api`)- Мақсаты: Torii, орындаушылар және біріктіру сынақтары сияқты ішкі құрамдастарға арналған `#[model]` құрылымдарына/сандарына толық, өзгермелі қатынасты көрсетеді. Онсыз бұл элементтер әдейі мөлдір емес, сондықтан сыртқы SDK тек қауіпсіз конструкторлар мен кодталған пайдалы жүктемелерді көреді.
- Механика: `iroha_data_model_derive::model` макросы әрбір жалпы өрісті `#[cfg(feature = "transparent_api")] pub` көмегімен қайта жазады және әдепкі құрастыру үшін жеке көшірмені сақтайды. Мүмкіндікті қосу сол cfg файлдарын аударады, сондықтан `Account`, `Domain`, `Asset`, т.б. құрылымын бұзу олардың анықтау модульдерінен тыс заңды болады.
- Беттік анықтау: жәшік `TRANSPARENT_API: bool` тұрақтысын экспорттайды (`transparent_api.rs` немесе `non_transparent_api.rs` түрінде жасалады). Төменгі ағын коды бұл жалаушаны тексере алады және ол мөлдір емес көмекшілерге оралу қажет болғанда тармақты алады.
- Қосу: `Cargo.toml` ішіндегі тәуелділікке `features = ["transparent_api"]` қосыңыз. JSON проекциясын қажет ететін жұмыс кеңістігінің жәшіктері (мысалы, `iroha_torii`) жалаушаны автоматты түрде жібереді, бірақ үшінші тарап тұтынушылары орналастыруды бақылап, кеңірек API бетін қабылдамайынша, оны өшіру керек.

## Жылдам мысалдар

Домен мен тіркелгіні жасаңыз, активті анықтаңыз және нұсқаулармен транзакция жасаңыз:

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

Есептік жазбалар мен активтерді DSL арқылы сұрау:

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

IVM смарт келісімшарт байт кодын пайдаланыңыз:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## Нұсқа жасау

- `SignedTransaction`, `SignedBlock` және `SignedQuery` канондық Norito кодталған құрылымдар. Әрқайсысы `EncodeVersioned` арқылы кодталған кезде пайдалы жүктемесін ағымдағы ABI нұсқасымен (қазіргі `1`) префикстеу үшін `iroha_version::Version` қолданады.

## Шолу ескертпелері / Потенциалды жаңартулар

- DSL сұрауы: пайдаланушыға арналған тұрақты ішкі жиынды құжаттауды және жалпы сүзгілер/таңдаушылар үшін мысалдарды қарастырыңыз.
- Нұсқаулар отбасылары: `mint_burn`, `register`, `transfer` арқылы ашылған кірістірілген ISI нұсқаларының тізімін көрсететін жалпыға қолжетімді құжаттарды кеңейтіңіз.

---
Егер қандай да бір бөлікке көбірек тереңдік қажет болса (мысалы, толық ISI каталогы, толық сұраулар тізілімінің тізімі немесе блок тақырыбы өрістері), маған хабарлаңыз, мен бұл бөлімдерді сәйкесінше кеңейтемін.