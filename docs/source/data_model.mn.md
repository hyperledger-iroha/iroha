---
lang: mn
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 Өгөгдлийн загвар – Гүн шумбах

Энэхүү баримт бичигт `iroha_data_model` хайрцагт хэрэгжиж, ажлын талбарт ашигладаг Iroha v2 өгөгдлийн загварыг бүрдүүлдэг бүтэц, танигч, шинж чанар, протоколуудыг тайлбарласан болно. Энэ нь таны хянаж, шинэчлэлт санал болгох боломжтой нарийн лавлагаа байх зорилготой юм.

## Хамрах хүрээ ба суурь

- Зорилго: Домэйн объект (домэйн, данс, хөрөнгө, NFT, үүрэг, зөвшөөрөл, үе тэнгийнхэн), төлөвийг өөрчлөх заавар (ISI), асуулга, триггер, гүйлгээ, блок, параметрүүдийг каноник төрлөөр хангах.
- Цувралчлал: Бүх нийтийн төрлүүд нь Norito кодлогч (`norito::codec::{Encode, Decode}`) ба схем (`iroha_schema::IntoSchema`)-аас гаралтай. JSON-г онцлог шинж тэмдгийн ард сонгон ашигладаг (жишээ нь: HTTP болон `Json` ачааллын хувьд).
- IVM тэмдэглэл: Гэрээг дуудахын өмнө хост баталгаажуулалт хийдэг тул Iroha виртуал машиныг (IVM) чиглүүлэх үед зарим цуваа тайлах хугацааны баталгаажуулалтыг идэвхгүй болгосон (`src/lib.rs` дээрх хайрцагны баримтуудыг үзнэ үү).
- FFI хаалга: Зарим төрлүүдийг FFI шаардлагагүй үед нэмэлт ачааллаас зайлсхийхийн тулд `iroha_ffi`-ээр дамжуулан `ffi_export`/`ffi_import`-ийн ард FFI-д зориулж нөхцөлт тэмдэглэгээ хийдэг.

## Үндсэн шинж чанарууд ба туслахууд- `Identifiable`: Байгууллагууд тогтвортой `Id` болон `fn id(&self) -> &Self::Id` байна. Газрын зураг/багтаамжтай байхын тулд `IdEqOrdHash`-ээс гаргаж авсан байх ёстой.
- `Registrable`/`Registered`: Олон байгууллага (жишээ нь, `Domain`, `AssetDefinition`, `Role`) бүтээгчийн загварыг ашигладаг. `Registered` нь ажиллах цагийн төрлийг бүртгэлийн гүйлгээнд тохиромжтой, хөнгөн бүтээгч төрөлтэй (`With`) холбодог.
- `HasMetadata`: `Metadata` газрын зургийн түлхүүр/утгад нэгдсэн хандалт.
- `IntoKeyValue`: Давхардлыг багасгахын тулд `Key` (ID) болон `Value` (өгөгдөл) -ийг тусад нь хадгалахад зориулсан хадгалах санг хуваах туслах.
- `Owned<T>`/`Ref<'world, K, V>`: Шаардлагагүй хуулбараас зайлсхийхийн тулд хадгалах сан, асуулгын шүүлтүүрт ашигладаг хөнгөн боодол.

## Нэр ба танигч- `Name`: Хүчин төгөлдөр текст танигч. Хоосон зай болон нөөцлөгдсөн тэмдэгтүүдийг `@`, `#`, `$` (нийлмэл ID-д ашигладаг) зөвшөөрөхгүй. Баталгаажуулалттай `FromStr`-ээр бүтээгдэх боломжтой. Нэрүүдийг задлан шинжилж байхдаа Юникод NFC болгон хэвийн болгосон (каноникийн хувьд ижил төстэй үсгийн алдааг ижил гэж үзэж, зохиосон хэлбэрээр хадгалдаг). `genesis` тусгай нэр хадгалагдсан (том үсгээр тэмдэглэсэн).
- `IdBox`: Ямар ч дэмжигдсэн ID-д зориулсан нийлбэр төрлийн дугтуй (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Ерөнхий урсгал болон Norito кодчилолд нэг төрлийн хувьд хэрэгтэй.
- `ChainId`: Гүйлгээнд дахин тоглуулах хамгаалалтад ашигладаг тунгалаг бус хэлхээ танигч.ID-н стринг маягтууд (`Display`/`FromStr`-ээр хоёр тийш эргэх боломжтой):
- `DomainId`: `name` (жишээ нь, `wonderland`).
- `AccountId`: `AccountAddress`-ээр зөвхөн I105 гэж кодлогдсон домэйнгүй каноник данс танигч. Шинжилгээний оролт нь каноник I105 байх ёстой; домэйны дагавар (`@domain`), каноник I105 литерал, өөр нэрийн литерал, каноник зургаан талт задлагч оролт, хуучин `norito:` ачааллыг болон `uaid:`/`opaque:` хаягийг задалсан.
- `AssetDefinitionId`: каноник `unprefixed Base58 address with versioning and checksum` (UUID-v4 байт).
- `AssetId`: каноник кодлогдсон literal `<asset-definition-id>#<account-id>` (эхний хувилбар дээр хуучин текст хэлбэрийг дэмждэггүй).
- `NftId`: `nft$domain` (жишээ нь, `rose$garden`).
- `PeerId`: `public_key` (үе тэнгийн тэгш байдал нь нийтийн түлхүүрээр байдаг).

## Аж ахуйн нэгж

### Домэйн
- `DomainId { name: Name }` – өвөрмөц нэр.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Builder: `NewDomain` нь `with_logo`, `with_metadata`, дараа нь `Registrable::build(authority)` багцууд `owned_by`.### Данс
- `AccountId` нь хянагчаар тохируулсан, каноник I105 гэж кодлогдсон домэйнгүй дансны таниулбар юм.
- `ScopedAccountId { account: AccountId, domain: DomainId }` нь зөвхөн хамрах хүрээг харах шаардлагатай тохиолдолд тодорхой домэйн контекстийг агуулна.
- `Account { id, metadata, label?, uaid? }` — `label` нь дахин түлхүүр бичлэгт ашигладаг нэмэлт тогтвортой нэр юм, `uaid` нь нэмэлт Nexus өргөн [Түгээмэл дансны ID](Kotodama) юм.
- Барилгачин: `Account::new(id)`-ээр дамжуулан `NewAccount`; Бүртгэлийн хувьд тодорхой `ScopedAccountId` домэйн шаардлагатай бөгөөд анхдагчаас нэгийг нь гаргахгүй.

### Хөрөнгийн тодорхойлолт ба хөрөнгө
- `AssetDefinitionId { aid_bytes: [u8; 16] }` текстийн хувьд `unprefixed Base58 address` хэлбэрээр ил гарсан.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` нь хүний ​​нүүрэн дээр харуулсан дэлгэцийн текст шаардлагатай бөгөөд `#`/`@` агуулаагүй байх ёстой.
  - `alias` сонголттой бөгөөд дараахын аль нэг нь байх ёстой.
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    зүүн сегмент нь яг таарч байгаа `AssetDefinition.name`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Барилгачид: `AssetDefinition::new(id, spec)` эсвэл ая тухтай байдал `numeric(id)`; `name` шаардлагатай бөгөөд `.with_name(...)`-ээр тохируулсан байх ёстой.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }`, хадгалахад тохиромжтой `AssetEntry`/`AssetValue`.
- `AssetBalanceScope`: Хязгааргүй үлдэгдлийн хувьд `Global`, өгөгдлийн орон зайгаар хязгаарлагдсан үлдэгдлийн хувьд `Dataspace(DataSpaceId)`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` хураангуй API-д нээлттэй.### NFT
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (агуулга нь дурын түлхүүр/утга мета өгөгдөл).
- Барилгачин: `Nft::new(id, content)`-ээр дамжуулан `NewNft`.

### Үүрэг ба зөвшөөрөл
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` барилгачин `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – `name` болон ачааллын схем нь идэвхтэй `ExecutorDataModel`-тэй таарч байх ёстой (доороос харна уу).

### Үе тэнгийнхэн
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` болон задлан шинжилж болох `public_key@address` мөр хэлбэр.

### Криптографийн командууд (`sm` онцлог)
- `Sm2PublicKey` ба `Sm2Signature`: SM2-д зориулсан SEC1-д нийцсэн цэгүүд ба тогтмол өргөнтэй `r∥s` гарын үсэг. Барилгачид муруй гишүүнчлэл болон ялгах ID-г баталгаажуулдаг; Norito кодчилол нь `iroha_crypto`-ийн ашигладаг каноник дүрслэлийг тусгадаг.
- `Sm3Hash`: `[u8; 32]` шинэ төрөл нь GM/T 0004 дижестийг төлөөлж, манифест, телеметр болон системийн хариу үйлдэлд ашигладаг.
- `Sm4Key`: 128 битийн тэгш хэмтэй түлхүүрийн боолтыг хост систем болон дата загварын бэхэлгээний хооронд хуваалцдаг.
Эдгээр төрлүүд нь одоо байгаа Ed25519/BLS/ML-DSA командуудтай зэрэгцэн суудаг бөгөөд ажлын талбарыг `--features sm`-ээр бүтээсний дараа нийтийн схемийн нэг хэсэг болно.### Өдөөгч болон үйл явдал
- `TriggerId { name: Name }` болон `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` эсвэл `Exactly(u32)`; захиалга болон хомсдолын хэрэгслүүд багтсан.
  - Аюулгүй байдал: `TriggerCompleted`-ийг үйлдлийн шүүлтүүр болгон ашиглах боломжгүй (цувралжуулалтын үед баталгаажуулсан).
- `EventBox`: дамжуулах хоолой, дамжуулах хоолой-багц, өгөгдөл, хугацаа, гүйцэтгэх-триггер, триггер дууссан үйл явдлын нийлбэр төрөл; `EventFilterBox` нь захиалга болон триггер шүүлтүүрт зориулагдсан.

## Параметр ба тохиргоо

- Системийн параметрийн бүлгүүд (бүх `Default`ed, хүлээн авагчийг зөөвөрлөх ба бие даасан тоолол руу хөрвүүлэх):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` бүх гэр бүл болон `custom: BTreeMap<CustomParameterId, CustomParameter>`-ийг бүлэглэнэ.
- Нэг параметрийн тоолол: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` дифференциалтай төстэй шинэчлэлтүүд болон давталтуудад зориулагдсан.
- Захиалгат параметрүүд: гүйцэтгэгчээр тодорхойлогддог, `Json` гэж авч явдаг, `CustomParameterId` (a `Name`)-ээр тодорхойлогддог.

## ISI (Iroha Тусгай заавар)- Үндсэн шинж чанар: `Instruction` нь `dyn_encode`, `as_any`, мөн төрөл бүрийн тогтвортой танигч `id()` (өгөгдмөл нь бетоны төрлийн нэр). Бүх заавар нь `Send + Sync + 'static`.
- `InstructionBox`: Clon/eq/ord бүхий `Box<dyn Instruction>` боодол нь ID төрлийн + кодлогдсон байтаар хэрэгждэг.
- Баригдсан зааврын гэр бүлүүдийг дараахь дор зохион байгуулдаг.
  - `mint_burn`, `transfer`, `register`, `transparent` туслагчийн багц.
  - Мета урсгалын тоонуудыг бичнэ үү: `InstructionType`, `SetKeyValueBox` (domain/account/asset_def/nft/trigger) гэх мэт хайрцагласан нийлбэрүүд.
- Алдаа: `isi::error`-ийн дагуу баялаг алдааны загвар (үнэлгээний төрлийн алдаа, олох алдаа, алдаа, математик, хүчингүй параметр, давталт, өөрчлөгддөггүй).
- Зааврын бүртгэл: `instruction_registry!{ ... }` макро нь төрлийн нэрээр түлхүүрлэгдсэн ажиллах цагийн код тайлах бүртгэлийг бүтээдэг. `InstructionBox` клон болон Norito серде нь динамик (де) цуврал болгоход ашигладаг. Хэрэв `set_instruction_registry(...)`-ээр дамжуулан ямар ч бүртгэлийг тодорхой тохируулаагүй бол хоёртын файлыг тогтвортой байлгахын тулд бүх үндсэн ISI бүхий анхдагч бүртгэлийг анх ашиглахдаа залхуугаар суулгадаг.

## Гүйлгээ- `Executable`: `Instructions(ConstVec<InstructionBox>)` эсвэл `Ivm(IvmBytecode)`. `IvmBytecode` base64 (`Vec<u8>` дээр ил тод шинэ төрөл) болгон цуваа болгодог.
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, нэмэлт `time_to_live_ms`, `nonce`, `nonce`, I018X, `chain`, `authority`, гүйлгээний ачаалал үүсгэдэг. `Executable`.
  - Туслах хүмүүс: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Kotodama.
- `SignedTransaction` (`iroha_version` хувилбартай): `TransactionSignature` болон даацыг зөөдөг; хэш болон гарын үсгийн баталгаажуулалтыг хангадаг.
- Нэвтрэх цэг ба үр дүн:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` хэш туслагчтай.
  - `ExecutionStep(ConstVec<InstructionBox>)`: гүйлгээний зааврын нэг захиалгат багц.

## Блок- `SignedBlock` (хувилбартай) капсулууд:
  - `signatures: BTreeSet<BlockSignature>` (баталгаажуулагчаас),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (хоёрдогч гүйцэтгэлийн төлөв) `time_triggers`, оролт/үр дүн Merkle мод, `transaction_results`, `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` зэргийг агуулсан.
- Хэрэглээ: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, Kotodama.
- Merkle roots: гүйлгээний нэвтрэх цэгүүд болон үр дүнг Merkle модоор дамжуулан хийдэг; үр дүн Merkle root-ийг блокийн толгой хэсэгт байрлуулна.
- Блок оруулах нотлох баримтууд (`BlockProofs`) оролт/үр дүнгийн Merkle баталгаа болон `fastpq_transcripts` газрын зургийг хоёуланг нь ил гаргадаг тул сүлжээнээс гадуурх мэргэжилтнүүд гүйлгээний хэштэй холбоотой шилжүүлгийн дельтануудыг татаж авах боломжтой.
- `ExecWitness` мессежүүд (Torii-ээр дамжуулж, зөвшилцсөн хов жив дээр тулгуурласан) одоо `fastpq_transcripts` ба `fastpq_batches: Vec<FastpqTransitionBatch>`, суулгагдсан root I102000, slot-той `fastpq_batches: Vec<FastpqTransitionBatch>`, perm_root, tx_set_hash), тиймээс гадны судлаачид транскриптийг дахин кодлохгүйгээр каноник FASTPQ мөрүүдийг залгих боломжтой.

## Асуулт- Хоёр амт:
  - Ганц тоо: хэрэгжүүлэх `SingularQuery<Output>` (жишээ нь, `FindParameters`, `FindExecutorDataModel`).
  - Давтагдах боломжтой: `Query<Item>` (жишээ нь, `FindAccounts`, `FindAssets`, `FindDomains` гэх мэт) хэрэгжүүлэх.
- Төрөл арилгасан маягтууд:
  - `QueryBox<T>` нь дэлхийн бүртгэлээр баталгаажсан Norito сердтэй, хайрцаглагдсан, устгагдсан `Query<Item = T>` юм.
  - `QueryWithFilter<T> { query, predicate, selector }` хүсэлтийг DSL предикат/сонгогчтой хослуулдаг; `From`-ээр дамжуулан устгасан давтагдах хүсэлт болгон хувиргадаг.
- Бүртгэл ба кодлогч:
  - `query_registry!{ ... }` нь динамик кодыг тайлахын тулд бүтээгчдэд төрөл бүрийн нэрээр тодорхой асуулгын төрлүүдийг дүрслэх дэлхийн бүртгэлийг бүтээдэг.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` болон `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` нь нэгэн төрлийн векторуудын нийлбэр төрөл юм (жишээ нь, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), нэмсэн tuple болон өргөтгөлийн туслахад зориулагдсан.
- DSL: `query::dsl`-д эмхэтгэх хугацаанд шалгагдсан предикат болон сонгогчид проекцын шинж чанартай (`HasProjection<PredicateMarker>` / `SelectorMarker`) хэрэгжсэн. `fast_dsl` функц нь шаардлагатай бол илүү хөнгөн хувилбарыг гаргадаг.

## Гүйцэтгэгч ба өргөтгөх боломж- `Executor { bytecode: IvmBytecode }`: баталгаажуулагчийн гүйцэтгэсэн кодын багц.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` нь гүйцэтгэгчээр тодорхойлсон домэйныг зарлаж байна:
  - Тусгай тохиргооны параметрүүд,
  - Зааварчилгаа таниулагч,
  - Зөвшөөрлийн токен танигч,
  - Үйлчлүүлэгчийн хэрэгсэлд зориулсан захиалгат төрлүүдийг тодорхойлсон JSON схем.
- Тохируулах дээжүүд `data_model/samples/executor_custom_data_model`-ийн дагуу байгаа бөгөөд үүнийг харуулж байна:
  - `iroha_executor_data_model::permission::Permission` деривээр дамжуулан тусгай зөвшөөрлийн токен,
  - Захиалгат параметрийг `CustomParameter` болгон хувиргах төрөл гэж тодорхойлсон,
  - Захиалгат зааварчилгааг гүйцэтгэхийн тулд `CustomInstruction` руу цувуулсан.

### CustomInstruction (гүйцэтгэгчээр тодорхойлсон ISI)- Төрөл: `isi::CustomInstruction { payload: Json }`, тогтвортой утас ID `"iroha.custom"`.
- Зорилго: хувийн/консорциумын сүлжээн дэх гүйцэтгэгчийн тусгай зааварчилгааны дугтуй эсвэл олон нийтийн мэдээллийн загварыг салаагүйгээр загвар гаргах.
- Өгөгдмөл гүйцэтгэгчийн үйлдэл: `iroha_core`-д суурилуулсан гүйцэтгэгч нь `CustomInstruction`-г ажиллуулдаггүй бөгөөд хэрэв таарвал сандрах болно. Захиалгат гүйцэтгэгч нь `InstructionBox`-г `CustomInstruction` болгон бууруулж, бүх баталгаажуулагч дээрх ачааллыг тодорхой тайлбарлах ёстой.
- Norito: схемийг агуулсан `norito::codec::{Encode, Decode}`-ээр дамжуулан кодчилдог/тайлдаг; `Json` ачааллыг тодорхойлон цуваа болгосон. Зааврын бүртгэлд `CustomInstruction` (энэ нь өгөгдмөл бүртгэлийн нэг хэсэг) орсон тохиолдолд хоёр талын аялал тогтвортой байна.
- IVM: Kotodama нь IVM байт кодыг (`.to`) хөрвүүлдэг бөгөөд програмын логикт санал болгож буй зам юм. `CustomInstruction`-г зөвхөн Kotodama хэлээр илэрхийлэх боломжгүй гүйцэтгэгч түвшний өргөтгөлүүдэд ашиглаарай. Үе тэнгийнхэндээ детерминизм болон ижил гүйцэтгэгч хоёртын файлуудыг баталгаажуулах.
- Олон нийтийн сүлжээнд зориулагдаагүй: нэг төрлийн бус гүйцэтгэгчид зөвшилцөх эрсдэлтэй тохиолдолд нийтийн сүлжээнүүдэд бүү ашигла. Танд платформын функц хэрэгтэй үед шинэ суурилуулсан ISI-г санал болгохыг илүүд үзээрэй.

## Мета өгөгдөл- `Metadata(BTreeMap<Name, Json>)`: олон байгууллага (`Domain`, `Account`, `AssetDefinition`, `Nft`, триггер болон гүйлгээ)-д хавсаргасан түлхүүр/үнэ цэнийн хадгалалт.
- API: `contains`, `iter`, `get`, `insert`, (`transparent_api`-тэй) `remove`.

## Онцлогууд ба детерминизм

- Нэмэлт API-г (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `fast_dsl`, I10800X, I10800) хянадаг `fault_injection`).
- Детерминизм: Бүх цуваа техник хангамжид зөөврийн байхын тулд Norito кодчилолыг ашигладаг. IVM байт код нь тунгалаг биш байт блб; гүйцэтгэл нь тодорхой бус бууралтыг нэвтрүүлэх ёсгүй. Хост нь гүйлгээг баталгаажуулж, IVM-д тодорхой байдлаар оролтыг нийлүүлдэг.

### Ил тод API (`transparent_api`)- Зорилго: Torii, гүйцэтгэгчид болон интеграцийн тест зэрэг дотоод бүрэлдэхүүн хэсгүүдийн `#[model]` бүтэц/тооцоонд бүрэн, өөрчлөгдөх боломжтой хандалтыг ил гаргах. Үүнгүйгээр эдгээр зүйлүүд нь зориудаар тунгалаг байх тул гадаад SDK нь зөвхөн аюулгүй бүтээгчид болон кодлогдсон ачааллыг хардаг.
- Механик: `iroha_data_model_derive::model` макро нь нийтийн талбар бүрийг `#[cfg(feature = "transparent_api")] pub`-ээр дахин бичиж, анхдагч бүтээхэд зориулж хувийн хуулбарыг хадгалдаг. Энэ функцийг идэвхжүүлснээр тэдгээр cfg-г эргүүлэх тул `Account`, `Domain`, `Asset` гэх мэтийг устгах нь тэдгээрийн тодорхойлох модулиас гадуур хууль ёсны болно.
- Гадаргууг илрүүлэх: хайрцаг нь `TRANSPARENT_API: bool` тогтмолыг (`transparent_api.rs` эсвэл `non_transparent_api.rs` болгон үүсгэсэн) экспортлодог. Доод урсгалын код нь тунгалаг бус туслахууд руу буцах шаардлагатай үед энэ тугийг шалгаж, салбарлах боломжтой.
- Идэвхжүүлж байна: `Cargo.toml`-ийн хамааралд `features = ["transparent_api"]` нэмнэ. JSON проекц (жишээ нь, `iroha_torii`) шаардлагатай ажлын талбарын хайрцагнууд нь тугийг автоматаар дамжуулдаг боловч гуравдагч талын хэрэглэгчид суулгацыг хянаж, илүү өргөн API гадаргууг хүлээн зөвшөөрөхгүй бол үүнийг унтрааж байх ёстой.

## Түргэн жишээ

Домэйн болон акаунт үүсгэж, хөрөнгийг тодорхойлж, зааварчилгаа бүхий гүйлгээг үүсгэ:

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
let asset_def_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap();
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

DSL-ээр данс болон хөрөнгийг асууна уу:

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

IVM ухаалаг гэрээний байт кодыг ашиглана уу:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / бусад нэрийн шуурхай лавлагаа (CLI + Torii):

```bash
# Register an asset definition with canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauﾛ1P... \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```Шилжилтийн тэмдэглэл:
- Хуучин `name#domain` хөрөнгийн тодорхойлолтыг v1-д хүлээн зөвшөөрдөггүй.
- Цутгах/шатаах/шилжүүлэх хөрөнгийн ID-ууд `<asset-definition-id>#<account-id>` каноник хэвээр байна; тэдгээрийг бүтээх:
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - эсвэл `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## Хувилбар

- `SignedTransaction`, `SignedBlock`, `SignedQuery` нь Norito кодлогдсон каноник бүтэц юм. Тус бүр нь `EncodeVersioned`-ээр кодлогдсон үед одоогийн ABI хувилбарт (одоогоор `1`) ачаагаа угтвар болгохын тулд `iroha_version::Version`-ийг хэрэгжүүлдэг.

## Хяналтын тэмдэглэл / Боломжит шинэчлэлтүүд

- DSL-ийн асуулга: хэрэглэгчдэд чиглэсэн тогтвортой дэд багц болон нийтлэг шүүлтүүр/сонгогчдын жишээг баримтжуулах талаар бодож үзээрэй.
- Зааварчилгааны бүлгүүд: `mint_burn`, `register`, `transfer`-д илэрсэн ISI-ийн суулгасан хувилбаруудыг жагсаасан олон нийтийн баримт бичгийг өргөжүүлнэ.

---
Хэрэв аль нэг хэсэгт илүү гүнзгийрүүлэх шаардлагатай бол (жишээ нь, бүрэн ISI каталог, бүрэн асуулгын бүртгэлийн жагсаалт эсвэл блок толгойн талбарууд) надад мэдэгдээрэй, би тэдгээр хэсгүүдийг зохих ёсоор нь өргөтгөх болно.