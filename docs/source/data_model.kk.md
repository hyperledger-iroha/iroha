---
lang: kk
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 Деректер үлгісі – Терең сүңгу

Бұл құжат `iroha_data_model` жәшігінде енгізілген және жұмыс кеңістігінде пайдаланылатын Iroha v2 деректер үлгісін құрайтын құрылымдарды, идентификаторларды, белгілерді және протоколдарды түсіндіреді. Бұл сіз қарап шығуға және жаңартуларды ұсына алатын нақты анықтама болуы керек.

## Қолдану аясы мен негіздері

- Мақсаты: домен нысандары (домендер, тіркелгілер, активтер, NFTs, рөлдер, рұқсаттар, тең дәрежелер), күйді өзгертетін нұсқаулар (ISI), сұраулар, триггерлер, транзакциялар, блоктар және параметрлер үшін канондық түрлерді қамтамасыз ету.
- Серияландыру: барлық жалпы түрлер Norito кодектерін (`norito::codec::{Encode, Decode}`) және схемасын (`iroha_schema::IntoSchema`) шығарады. JSON таңдаулы түрде (мысалы, HTTP және `Json` пайдалы жүктемелері үшін) мүмкіндік жалауларының артында пайдаланылады.
- IVM ескертпе: Iroha виртуалды машинасына (IVM) бағытталған кезде белгілі сериясыздандыру уақытының валидациялары өшіріледі, себебі хост келісім-шарттарды шақырмас бұрын тексеруді орындайды (`src/lib.rs` ішіндегі жәшік құжаттарын қараңыз).
- FFI қақпалары: FFI қажет болмаған кезде үстеме шығындарды болдырмау үшін `ffi_export`/`ffi_import` артында `iroha_ffi` арқылы FFI үшін кейбір түрлер шартты түрде түсіндіріледі.

## Негізгі қасиеттер мен көмекшілер- `Identifiable`: Нысандарда тұрақты `Id` және `fn id(&self) -> &Self::Id` бар. Карта/жиынтық достық үшін `IdEqOrdHash` арқылы алынуы керек.
- `Registrable`/`Registered`: Көптеген нысандар (мысалы, `Domain`, `AssetDefinition`, `Role`) құрастырушы үлгісін пайдаланады. `Registered` орындау уақыты түрін тіркеу транзакциялары үшін қолайлы жеңіл құрастырушы түріне (`With`) байланыстырады.
- `HasMetadata`: `Metadata` кілтіне/мәніне бірыңғай рұқсат.
- `IntoKeyValue`: қайталануды азайту үшін `Key` (ID) және `Value` (деректер) бөлек сақтауға арналған сақтауды бөлу көмекшісі.
- `Owned<T>`/`Ref<'world, K, V>`: қажетсіз көшірмелерді болдырмау үшін қоймалар мен сұрау сүзгілерінде қолданылатын жеңіл орауыштар.

## Аттар мен идентификаторлар- `Name`: жарамды мәтін идентификаторы. Бос орынға және `@`, `#`, `$` (құрама идентификаторларда пайдаланылады) сақталған таңбаларға рұқсат бермейді. Валидациясы бар `FromStr` арқылы құрастырылады. Атаулар талдау кезінде Юникод NFC стандартына қалыпқа келтіріледі (канондық эквивалентті емлелер бірдей болып саналады және құрастырылған түрде сақталады). `genesis` арнайы атауы сақталған (регистрді ескермей белгіленеді).
- `IdBox`: кез келген қолдау көрсетілетін идентификаторға арналған жиынтық түрдегі конверт (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Жалпы ағындар мен Norito кодтауы жалғыз түр ретінде пайдалы.
- `ChainId`: транзакцияларда қайта ойнатудан қорғау үшін пайдаланылатын мөлдір емес тізбек идентификаторы.Идентификаторлардың жолдық пішіндері (`Display`/`FromStr` арқылы екі рет айналуға болады):
- `DomainId`: `name` (мысалы, `wonderland`).
- `AccountId`: `AccountAddress` арқылы тек I105 ретінде кодталған канондық доменсіз тіркелгі идентификаторы. Талдаушы кірістері канондық I105 болуы керек; домен жұрнақтары (`@domain`), канондық I105 литералдары, лақап ат литералдары, канондық он алтылық талдаушы кірісі, бұрынғы `norito:` пайдалы жүктемелері және `uaid:`/`opaque:` есептік жазбасы қабылданбайды.
- `AssetDefinitionId`: канондық `aid:<32-lower-hex-no-dash>` (UUID-v4 байт).
- `AssetId`: канондық кодталған литерал `norito:<hex>` (бұрынғы мәтіндік пішіндерге бірінші шығарылымда қолдау көрсетілмейді).
- `NftId`: `nft$domain` (мысалы, `rose$garden`).
- `PeerId`: `public_key` (тең теңдік ашық кілт арқылы жүзеге асырылады).

## Нысандар

### Домен
- `DomainId { name: Name }` – бірегей атау.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Builder: `NewDomain` `with_logo`, `with_metadata`, содан кейін `Registrable::build(authority)` `owned_by` жинақтары.### Тіркелгі
- `AccountId` - контроллер арқылы кілттелген және канондық I105 ретінде кодталған канондық доменсіз тіркелгі идентификаторы.
- `ScopedAccountId { account: AccountId, domain: DomainId }` ауқымды көрініс қажет болған жағдайда ғана анық домен мәтінмәнін тасымалдайды.
- `Account { id, metadata, label?, uaid? }` — `label` қайта кілт жазбалары пайдаланатын қосымша тұрақты бүркеншік ат, `uaid` кең ауқымда қосымша Nexus [Әмбебап тіркелгі идентификаторы](Kotodama) бар.
- Құрылысшы: `NewAccount` `Account::new(id)` арқылы; тіркеу нақты `ScopedAccountId` доменін қажет етеді және әдепкіден біреуін шығармайды.

### Актив анықтамалары және активтер
- `AssetDefinitionId { aid_bytes: [u8; 16] }` мәтіндік түрде `aid:<32-hex-no-dash>` ретінде көрсетіледі.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `name` адамға арналған дисплей мәтіні қажет және құрамында `#`/`@` болмауы керек.
  - `alias` міндетті емес және мыналардың бірі болуы керек:
    - `<name>#<domain>@<dataspace>`
    - `<name>#<dataspace>`
    сол сегментімен `AssetDefinition.name` дәл сәйкес келеді.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Құрылысшылар: `AssetDefinition::new(id, spec)` немесе ыңғайлылық `numeric(id)`; `name` қажет және `.with_name(...)` арқылы орнатылуы керек.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }`, сақтауға ыңғайлы `AssetEntry`/`AssetValue`.
- `AssetBalanceScope`: шектеусіз баланстар үшін `Global` және деректер кеңістігі шектелген баланстар үшін `Dataspace(DataSpaceId)`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` жиынтық API интерфейстері үшін ашық.### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (мазмұн - ерікті кілт/мән метадеректері).
- Құрылысшы: `NewNft` `Nft::new(id, content)` арқылы.

### Рөлдер мен рұқсаттар
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` `NewRole { inner: Role, grant_to: AccountId }` құрылысшымен.
- `Permission { name: Ident, payload: Json }` – `name` және пайдалы жүктеме схемасы белсенді `ExecutorDataModel` (төменде қараңыз) сәйкес келуі керек.

### Құрдастар
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` және талданатын `public_key@address` жол пішіні.

### Криптографиялық примитивтер (`sm` мүмкіндігі)
- `Sm2PublicKey` және `Sm2Signature`: SM2 үшін SEC1-үйлесімді нүктелер және бекітілген ені `r∥s` қолтаңбалары. Конструкторлар қисық мүшелік пен ажыратушы идентификаторларды тексереді; Norito кодтауы `iroha_crypto` пайдаланатын канондық көріністі көрсетеді.
- `Sm3Hash`: GM/T 0004 дайджестін білдіретін `[u8; 32]` жаңа түрі, манифесттерде, телеметрияда және жүйе жауаптарында қолданылады.
- `Sm4Key`: 128 биттік симметриялы кілт ораушысы хост жүйесі қоңыраулары мен деректер үлгісінің құрылғылары арасында ортақ.
Бұл түрлер бар Ed25519/BLS/ML-DSA примитивтерімен қатар орналасады және жұмыс кеңістігі `--features sm` көмегімен салынғаннан кейін жалпы схеманың бөлігі болады.### Триггерлер мен оқиғалар
- `TriggerId { name: Name }` және `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` немесе `Exactly(u32)`; тапсырыс беру және сарқылу утилиталары кіреді.
  - Қауіпсіздік: `TriggerCompleted` әрекет сүзгісі ретінде пайдаланылмайды (серияландыру кезінде расталған).
- `EventBox`: конвейер, конвейер-партиясы, деректер, уақыт, орындау-триггер және триггер-аяқталған оқиғалар үшін қосынды түрі; `EventFilterBox` жазылымдар мен триггер сүзгілерін көрсетеді.

## Параметрлер және конфигурация

- Жүйе параметрлерінің отбасылары (барлық `Default`ed, алушыларды тасымалдау және жеке сандарға түрлендіру):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` барлық отбасыларды және `custom: BTreeMap<CustomParameterId, CustomParameter>` топтарын топтайды.
- Бір параметрді сандар: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` дифференциалды жаңартулар мен итерация үшін.
- Пайдаланушы параметрлері: орындаушы анықтаған, `Json` ретінде тасымалданған, `CustomParameterId` (a `Name`) арқылы анықталған.

## ISI (Iroha Арнайы нұсқаулар)- Негізгі қасиет: `Instruction`, `dyn_encode`, `as_any` және әр түрге арналған тұрақты идентификатор `id()` (әдепкі бойынша бетон түрінің атауы). Барлық нұсқаулар `Send + Sync + 'static`.
- `InstructionBox`: ID түрі + кодталған байттар арқылы жүзеге асырылатын клон/экв/орд бар `Box<dyn Instruction>` орамасы.
- Кіріктірілген оқу отбасылары келесідей ұйымдастырылады:
  - `mint_burn`, `transfer`, `register` және `transparent` көмекшілер жинағы.
  - Мета ағындары үшін нөмірлерді теріңіз: `InstructionType`, `SetKeyValueBox` (domain/account/asset_def/nft/trigger) сияқты қорапшалар.
- Қателер: `isi::error` астындағы бай қате үлгісі (бағалау түріндегі қателер, қателерді табу, есептеу мүмкіндігі, математика, жарамсыз параметрлер, қайталау, инварианттар).
- Нұсқаулар тізілімі: `instruction_registry!{ ... }` макросы түр атауы бойынша кілттелген орындау уақытының декодтау тізбесін құрады. `InstructionBox` клонында және Norito серде динамикалық (де) сериялауға қол жеткізу үшін пайдаланылады. Ешбір тізбе `set_instruction_registry(...)` арқылы анық орнатылмаған болса, екілік файлдардың сенімді болуы үшін бірінші рет пайдаланған кезде барлық негізгі ISI бар кірістірілген әдепкі тізбе жалқаулықпен орнатылады.

## транзакциялар- `Executable`: `Instructions(ConstVec<InstructionBox>)` немесе `Ivm(IvmBytecode)`. `IvmBytecode` base64 (`Vec<u8>` үстіндегі мөлдір жаңа тип) ретінде серияланады.
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, қосымша `time_to_live_ms` және `nonce`, `nonce`, `time_to_live_ms`, `chain`, `authority`, `creation_time_ms` және транзакцияның пайдалы жүктемесін құрастырады `Executable`.
  - Көмекшілер: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Norito
- `SignedTransaction` (`iroha_version` нұсқасымен): `TransactionSignature` және пайдалы жүкті тасымалдайды; хэштеу және қолтаңбаны тексеруді қамтамасыз етеді.
- Кіру нүктелері және нәтижелер:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` хэштеу көмекшілері бар.
  - `ExecutionStep(ConstVec<InstructionBox>)`: транзакциядағы нұсқаулардың бір реттелген партиясы.

## Блоктар- `SignedBlock` (нұсқасы) инкапсулалар:
  - `signatures: BTreeSet<BlockSignature>` (валидаторлардан),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (екінші орындалу күйі) құрамында `time_triggers`, кіріс/нәтиже Merkle ағаштары, `transaction_results` және `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Утилиталар: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, Kotodama.
- Merkle тамыры: транзакцияның кіру нүктелері мен нәтижелері Merkle ағаштары арқылы жасалады; нәтиже Merkle түбірі блок тақырыбына орналастырылады.
- Блокты қосу дәлелдері (`BlockProofs`) енгізу/нәтиже Merkle дәлелдерін де, `fastpq_transcripts` картасын да көрсетеді, осылайша тізбектен тыс провайдерлер транзакция хэшімен байланысты тасымалдау дельталарын ала алады.
- `ExecWitness` хабарлары (Torii арқылы таратылады және консенсус өсектеріне сүйенеді) енді `fastpq_transcripts` және кірістірілген Kotodama түбірлері бар `fastpq_batches: Vec<FastpqTransitionBatch>` және проверге дайын `fastpq_batches: Vec<FastpqTransitionBatch>`, perm_root, tx_set_hash), сондықтан сыртқы дәлелдеушілер транскрипттерді қайта кодтаусыз канондық FASTPQ жолдарын қабылдай алады.

## Сұраулар- Екі дәм:
  - Сингулярлы: `SingularQuery<Output>` іске қосыңыз (мысалы, `FindParameters`, `FindExecutorDataModel`).
  - Қайталанатын: `Query<Item>` іске қосыңыз (мысалы, `FindAccounts`, `FindAssets`, `FindDomains` және т.б.).
- Түрі өшірілген пішіндер:
  - `QueryBox<T>` - жаһандық тізіліммен қамтамасыз етілген Norito сердесі бар қораптағы, өшірілген `Query<Item = T>`.
  - `QueryWithFilter<T> { query, predicate, selector }` сұрауды DSL предикаты/селекторымен жұптайды; `From` арқылы өшірілген қайталанатын сұрауға түрлендіреді.
- Тізілім және кодектер:
  - `query_registry!{ ... }` динамикалық декодтау үшін түр атауы бойынша конструкторларға нақты сұрау түрлерін салыстыратын жаһандық тізілімді құрады.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` және `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` – біртекті векторлар (мысалы, `Vec<Account>`, `Vec<Name>`, `Vec<BlockHeader>`, `Vec<BlockHeader>`), плюс кортеж және кеңейту анықтамалары үшін қосынды түрі.
- DSL: `query::dsl` жүйесінде компиляция уақыты тексерілетін предикаттар мен селекторларға арналған проекция белгілерімен (`HasProjection<PredicateMarker>` / `SelectorMarker`) енгізілген. `fast_dsl` мүмкіндігі қажет болса, жеңілірек нұсқаны көрсетеді.

## Орындаушы және кеңейту- `Executor { bytecode: IvmBytecode }`: валидатор орындайтын код жинағы.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` орындаушы анықтаған доменді жариялайды:
  - теңшелетін конфигурация параметрлері,
  - Теңшелетін нұсқаулық идентификаторлары,
  - Рұқсат белгісінің идентификаторлары,
  - Клиент құралына арналған пайдаланушы түрлерін сипаттайтын JSON схемасы.
- Теңшеу үлгілері `data_model/samples/executor_custom_data_model` астында бар, мынаны көрсетеді:
  - `iroha_executor_data_model::permission::Permission` туындысы арқылы реттелетін рұқсат белгісі,
  - `CustomParameter` түрлендірілетін түр ретінде анықталған теңшелетін параметр,
  - Орындау үшін `CustomInstruction` ішіне серияланған теңшелетін нұсқаулар.

### CustomInstruction (орындаушы анықтайтын ISI)- Түрі: `"iroha.custom"` тұрақты сым идентификаторы бар `isi::CustomInstruction { payload: Json }`.
- Мақсаты: жеке/консорциум желілеріндегі орындаушыға арналған нұсқауларға арналған конверт немесе жалпы деректер үлгісін бұзбай прототиптеу үшін.
- Әдепкі орындаушының әрекеті: `iroha_core` ішіндегі кірістірілген орындаушы `CustomInstruction` орындамайды және кездескен жағдайда үрейленеді. Теңшелетін орындаушы `InstructionBox` параметрін `CustomInstruction` дейін төмендетуі және барлық валидаторлардағы пайдалы жүктемені анықтауы керек.
- Norito: схемасы қосылған `norito::codec::{Encode, Decode}` арқылы кодтайды/декодтайды; `Json` пайдалы жүктемесі детерминирленген түрде серияланады. Нұсқаулар тізілімінде `CustomInstruction` (ол әдепкі тізілімнің бөлігі болып табылады) болғанша, айналма сапарлар тұрақты болады.
- IVM: Kotodama IVM байт кодына (`.to`) құрастырады және қолданба логикасы үшін ұсынылған жол болып табылады. `CustomInstruction` әлі Kotodama түрінде көрсетілмейтін орындаушы деңгейіндегі кеңейтімдер үшін ғана пайдаланыңыз. Детерминизмді және әріптестер арасында бірдей орындаушының екілік файлдарын қамтамасыз етіңіз.
- Қоғамдық желілер үшін емес: біртекті емес орындаушылар консенсус шанышқыларына қауіп төндіретін қоғамдық желілер үшін пайдаланбаңыз. Платформа мүмкіндіктері қажет болғанда жаңа кірістірілген ISI ағынын ұсыныңыз.

## Метадеректер- `Metadata(BTreeMap<Name, Json>)`: бірнеше нысандарға тіркелген кілт/мән қоймасы (`Domain`, `Account`, `AssetDefinition`, `Nft`, триггерлер және транзакциялар).
- API: `contains`, `iter`, `get`, `insert`, және (`transparent_api` бар) `remove`.

## Ерекшеліктер және детерминизм

- Мүмкіндіктер қосымша API интерфейстерін басқарады (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `fast_dsl`, I10800 `fault_injection`).
- Детерминизм: барлық сериялау аппараттық құрал арқылы тасымалдануы үшін Norito кодтауын пайдаланады. IVM байт коды мөлдір емес байт блобы; орындау детерминирленген емес қысқартуларды енгізбеуі керек. Хост транзакцияларды тексереді және IVM деректерін анықтаушы түрде қамтамасыз етеді.

### Transparent API (`transparent_api`)- Мақсаты: Torii, орындаушылар және біріктіру сынақтары сияқты ішкі құрамдастарға арналған `#[model]` құрылымдарына/сандарына толық, өзгермелі қатынасты көрсетеді. Онсыз бұл элементтер әдейі мөлдір емес, сондықтан сыртқы SDK тек қауіпсіз конструкторлар мен кодталған пайдалы жүктемелерді көреді.
- Механика: `iroha_data_model_derive::model` макросы әрбір жалпы өрісті `#[cfg(feature = "transparent_api")] pub` көмегімен қайта жазады және әдепкі құрастыру үшін жеке көшірмені сақтайды. Мүмкіндікті қосу сол cfg файлдарын аударады, сондықтан `Account`, `Domain`, `Asset`, т.б. құрылымын бұзу олардың анықтау модульдерінен тыс заңды болады.
- Беттік анықтау: жәшік `TRANSPARENT_API: bool` тұрақтысын экспорттайды (`transparent_api.rs` немесе `non_transparent_api.rs` түрінде жасалады). Төменгі ағын коды бұл жалаушаны тексере алады және ол мөлдір емес көмекшілерге оралу қажет болғанда тармақты алады.
- Қосу: `Cargo.toml` ішіндегі тәуелділікке `features = ["transparent_api"]` қосыңыз. JSON проекциясын қажет ететін жұмыс кеңістігі жәшіктері (мысалы, `iroha_torii`) жалаушаны автоматты түрде жібереді, бірақ үшінші тарап тұтынушылары орналастыруды бақылап, кеңірек API бетін қабылдамайынша, оны өшіру керек.

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

`aid` / бүркеншік аттың жылдам анықтамасы (CLI + Torii):

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
```Көшіру жазбасы:
- Ескі `name#domain` актив анықтамасының идентификаторлары v1 нұсқасында қабылданбайды.
- Жалға беру/жазу/тасымалдау үшін актив идентификаторлары канондық `norito:<hex>` болып қалады; оларды құру:
  - `iroha tools encode asset-id --definition aid:... --account <i105>`
  - немесе `--alias <name>#<domain>@<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## Нұсқа жасау

- `SignedTransaction`, `SignedBlock` және `SignedQuery` канондық Norito кодталған құрылымдар. Әрқайсысы `EncodeVersioned` арқылы кодталған кезде пайдалы жүктемені ағымдағы ABI нұсқасымен (қазіргі `1`) префикстеу үшін `iroha_version::Version` қолданады.

## Шолу ескертпелері / Потенциалды жаңартулар

- DSL сұрауы: пайдаланушыға арналған тұрақты ішкі жиынды құжаттауды және жалпы сүзгілер/таңдаушылар үшін мысалдарды қарастырыңыз.
- Нұсқаулар отбасылары: `mint_burn`, `register`, `transfer` арқылы ашылған кірістірілген ISI нұсқаларының тізімін көрсететін жалпыға қолжетімді құжаттарды кеңейтіңіз.

---
Егер қандай да бір бөлікке көбірек тереңдік қажет болса (мысалы, толық ISI каталогы, толық сұраулар тізілімінің тізімі немесе блок тақырыбы өрістері), маған хабарлаңыз, мен бұл бөлімдерді сәйкесінше кеңейтемін.