---
lang: mn
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Өгөгдлийн загвар ба ISI — Хэрэгжилтээс үүдэлтэй техникийн үзүүлэлт

Энэхүү тодорхойлолт нь дизайныг хянахад туслах зорилгоор `iroha_data_model` болон `iroha_core` дээрх одоогийн хэрэгжилтээс урвуу байдлаар хийгдсэн болно. Ар талд байгаа замууд нь эрх бүхий кодыг заадаг.

## Хамрах хүрээ
- Каноник байгууллагууд (домэйн, данс, хөрөнгө, NFT, үүрэг, зөвшөөрөл, үе тэнгийнхэн, триггер) болон тэдгээрийн танигчийг тодорхойлдог.
- Төлөв өөрчлөгдөх зааварчилгаа (ISI): төрөл, параметр, урьдчилсан нөхцөл, төлөвийн шилжилт, ялгарсан үйл явдал, алдааны нөхцөл зэргийг тайлбарлана.
- Параметрийн удирдлага, гүйлгээ, зааврын цуваачлалыг нэгтгэн харуулав.

Детерминизм: Бүх зааварчилгааны семантик нь техник хангамжаас хамааралгүй цэвэр төлөвийн шилжилтүүд юм. Цуваалалт нь Norito ашигладаг; VM байт код нь IVM-г ашигладаг бөгөөд гинжин хэлхээнд ажиллахаас өмнө хост талдаа баталгаажуулдаг.

---

## Байгууллага ба танигч
ID-ууд нь `Display`/`FromStr` хоёр талын аялалтай тогтвортой мөрийн маягтуудтай. Нэрийн дүрэм нь хоосон зай болон нөөцлөгдсөн `@ # $` тэмдэгтүүдийг хориглодог.- `Name` — баталгаажуулсан текст танигч. Дүрэм: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Домэйн: `{ id, logo, metadata, owned_by }`. Барилгачид: `NewDomain`. Код: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — каноник хаягуудыг `AccountAddress` (IH58 / `sora…` шахсан / зургаан өнцөгт) ашиглан үйлдвэрлэдэг ба Torii `AccountAddress::parse_any`-ээр дамжуулан оролтыг хэвийн болгодог. IH58 бол илүүд үздэг дансны формат юм; `sora…` хэлбэр нь зөвхөн Sora-д зориулагдсан UX-ийн хувьд хоёрдугаарт ордог. Танил `alias@domain` мөр нь зөвхөн чиглүүлэлтийн нэрээр хадгалагдана. Данс: `{ id, metadata }`. Код: `crates/iroha_data_model/src/account.rs`.
- Дансны элсэлтийн бодлого — домайнууд Norito-JSON `AccountAdmissionPolicy` мета өгөгдлийн түлхүүр `iroha:account_admission_policy` дор хадгалах замаар далд данс үүсгэхийг хянадаг. Түлхүүр байхгүй үед `iroha:default_account_admission_policy` гинжин түвшний захиалгат параметр нь анхдагчаар хангадаг; Энэ нь бас байхгүй үед хатуу өгөгдмөл нь `ImplicitReceive` (анхны хувилбар). Бодлогын шошгууд нь `mode` (`ExplicitOnly` эсвэл `ImplicitReceive`) дээр нэмэх нь гүйлгээ бүрийн сонголт (өгөгдмөл `16`) болон блок үүсгэх дээд хязгаар, нэмэлт `implicit_creation_fee` данс (sink) Хөрөнгийн тодорхойлолт бүрт `min_initial_amounts` ба нэмэлт `default_role_on_create` (`AccountCreated`-ийн дараа олгоно, байхгүй бол `DefaultRoleError` татгалзана). Genesis сонголт хийх боломжгүй; идэвхгүй/хүчингүй бодлого нь `InstructionExecutionError::AccountAdmission`-тай үл мэдэгдэх дансны төлбөрийн баримтын маягийн зааврыг үгүйсгэдэг. `AccountCreated`-ээс өмнө `iroha:created_via="implicit"` мета өгөгдлийн далд дансны тамга; өгөгдмөл дүрүүд нь дараагийн `AccountRoleGranted`-ийг ялгаруулдаг бөгөөд гүйцэтгэгч эзэмшигчийн үндсэн дүрмүүд нь шинэ дансанд нэмэлт үүрэггүйгээр өөрийн хөрөнгө/NFT-ийг зарцуулах боломжийг олгодог. Код: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Тодорхойлолт: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Код: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId` — Домэйн таарч байвал `asset#domain#account` эсвэл `asset##account`, `account` нь каноник `AccountId` мөр (IH58 илүү тохиромжтой). Хөрөнгө: `{ id, value: Numeric }`. Код: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`.
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. Код: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Үүрэг: `{ id, permissions: BTreeSet<Permission> }` барилгачин `NewRole { inner: Role, grant_to }`. Код: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Код: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — үе тэнгийн таних тэмдэг (нийтийн түлхүүр) болон хаяг. Код: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Триггер: `{ id, action }`. Үйлдэл: `{ executable, repeats, authority, filter, metadata }`. Код: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` оруулга/засах тэмдэгтэй. Код: `crates/iroha_data_model/src/metadata.rs`.
- Захиалгын загвар (програмын давхарга): төлөвлөгөөнүүд нь `subscription_plan` мета өгөгдөл бүхий `AssetDefinition` оруулгууд; захиалга нь `subscription` мета өгөгдөл бүхий `Nft` бичлэгүүд; Төлбөр тооцоог захиалгын NFT-д хамаарах цаг хугацааны триггерүүд гүйцэтгэдэг. `docs/source/subscriptions_api.md` болон `crates/iroha_data_model/src/subscription.rs`-г үзнэ үү.
- **Криптографийн командууд** (`sm` онцлог):- `Sm2PublicKey` / `Sm2Signature` каноник SEC1 цэгийг + SM2-д зориулсан тогтмол өргөнтэй `r∥s` кодчилолыг тусгана. Бүтээгчид муруй гишүүнчлэл болон ялгах ID семантик (`DEFAULT_DISTID`)-ийг мөрддөг бол баталгаажуулалт нь алдаатай эсвэл өндөр хүрээний скаляруудыг үгүйсгэдэг. Код: `crates/iroha_crypto/src/sm.rs` ба `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` нь GM/T 0004 дижестийг Norito-цуваачлах боломжтой `[u8; 32]` шинэ төрөл болгон харуулж байна. Код: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` нь 128 битийн SM4 түлхүүрүүдийг төлөөлдөг бөгөөд хост систем болон өгөгдлийн загварын бэхэлгээний хооронд хуваалцдаг. Код: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Эдгээр төрлүүд нь одоо байгаа Ed25519/BLS/ML-DSA командуудтай зэрэгцэн суудаг бөгөөд `sm` функцийг идэвхжүүлсний дараа өгөгдлийн загвар хэрэглэгчдэд (Torii, SDK, үүслийн хэрэгсэл) ашиглах боломжтой.

Чухал шинж чанарууд: `Identifiable`, `Registered`/`Registrable` (барилгачин загвар), `HasMetadata`, `IntoKeyValue`. Код: `crates/iroha_data_model/src/lib.rs`.

Үйл явдал: Байгууллага бүр мутаци дээр ялгардаг үйл явдлуудтай байдаг (үүсгэх/устгах/эзэмшигч өөрчлөгдсөн/мета өгөгдлийг өөрчилсөн гэх мэт). Код: `crates/iroha_data_model/src/events/`.

---

## Параметрүүд (Гинжний тохиргоо)
- Гэр бүл: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, дээр нь `custom: BTreeMap`.
- Ялгааны дан тоолол: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Агрегатор: `Parameters`. Код: `crates/iroha_data_model/src/parameter/system.rs`.

Параметрүүдийг тохируулах (ISI): `SetParameter(Parameter)` харгалзах талбарыг шинэчилж, `ConfigurationEvent::Changed` ялгаруулдаг. Код: `crates/iroha_data_model/src/isi/transparent.rs`, `crates/iroha_core/src/smartcontracts/isi/world.rs` дахь гүйцэтгэгч.

---

## Зааварчилгаа цуврал болгох ба бүртгэл
- Үндсэн шинж чанар: `Instruction: Send + Sync + 'static`, `dyn_encode()`, `as_any()`, тогтвортой `id()` (өгөгдмөл нь бетоны төрлийн нэрээр).
- `InstructionBox`: `Box<dyn Instruction>` боодол. Clone/Eq/Ord `(type_id, encoded_bytes)` дээр ажилладаг тул тэгш байдал нь утгаараа байна.
- `InstructionBox`-д зориулсан Norito serde нь `(String wire_id, Vec<u8> payload)` гэж цуваа (утас ID байхгүй бол `type_name` руу буцна). Цуваа салгах нь бүтээгчдэд дэлхийн `InstructionRegistry` зураглалын танигчийг ашигладаг. Өгөгдмөл бүртгэл нь бүх суурилагдсан ISI-г агуулдаг. Код: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: Төрөл, семантик, алдаа
Гүйцэтгэлийг `iroha_core::smartcontracts::isi` дээр `Execute for <Instruction>`-ээр гүйцэтгэдэг. Нийтийн нөлөө, урьдчилсан нөхцөл, ялгарсан үйл явдал, алдаануудыг доор жагсаав.

### Бүртгүүлэх / Бүртгэлээс хасах
Төрөл: `Register<T: Registered>` ба `Unregister<T: Identifiable>`, `RegisterBox`/`UnregisterBox` нийлбэрүүд нь бетонон зорилтуудыг хамардаг.

- Register Peer: дэлхийн үе тэнгийн багцад оруулдаг.
  - Урьдчилсан нөхцөл: аль хэдийн байхгүй байх ёстой.
  - Үйл явдал: `PeerEvent::Added`.
  - Алдаа: Давхардсан тохиолдолд `Repetition(Register, PeerId)`; Хайлт дээр `FindError`. Код: `core/.../isi/world.rs`.

- Домэйн бүртгүүлэх: `NewDomain`-аас `owned_by = authority`-ээр бүтээгдсэн. Зөвшөөрөгүй: `genesis` домэйн.
  - Урьдчилсан нөхцөл: домэйн байхгүй байх; `genesis` биш.
  - Үйл явдал: `DomainEvent::Created`.
  - Алдаа: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Код: `core/.../isi/world.rs`.- Бүртгэлийн бүртгэл: `genesis` домэйнд зөвшөөрөгдөөгүй `NewAccount`-с бүтээгдсэн; `genesis` бүртгэлийг бүртгэх боломжгүй.
  - Урьдчилсан нөхцөл: домэйн байх ёстой; данс байхгүй байх; генезийн домэйнд биш.
  - Үйл явдал: `DomainEvent::Account(AccountEvent::Created)`.
  - Алдаа: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Код: `core/.../isi/domain.rs`.

- AssetDefinition-г бүртгүүлэх: бүтээгчээс бүтээх; `owned_by = authority` багц.
  - Урьдчилсан нөхцөл: байхгүй гэсэн тодорхойлолт; домэйн байдаг.
  - Үйл явдал: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Алдаа: `Repetition(Register, AssetDefinitionId)`. Код: `core/.../isi/domain.rs`.

- NFT-г бүртгүүлэх: Барилгачнаас бүтээх; `owned_by = authority` багц.
  - Урьдчилсан нөхцөл: NFT байхгүй байх; домэйн байдаг.
  - Үйл явдал: `DomainEvent::Nft(NftEvent::Created)`.
  - Алдаа: `Repetition(Register, NftId)`. Код: `core/.../isi/nft.rs`.

- Бүртгэлийн үүрэг: `NewRole { inner, grant_to }`-ээс бүтээгдсэн (анхны эзэмшигч нь дансны үүргийн зураглалаар бүртгэгдсэн), `inner: Role` хадгалдаг.
  - Урьдчилсан нөхцөл: үүрэг байхгүй байх.
  - Үйл явдал: `RoleEvent::Created`.
  - Алдаа: `Repetition(Register, RoleId)`. Код: `core/.../isi/world.rs`.

- Бүртгэлийн триггер: гохыг шүүлтүүрийн төрлөөр тохируулсан тохирох триггерт хадгална.
  - Урьдчилсан нөхцөл: Шүүлтүүрийг ашиглах боломжгүй бол `action.repeats` `Exactly(1)` (өөрөөр бол `MathError::Overflow`) байх ёстой. Давхардсан үнэмлэхийг хориглоно.
  - Үйл явдал: `TriggerEvent::Created(TriggerId)`.
  - Алдаа: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` хувиргалт/баталгаажуулалтын алдаа. Код: `core/.../isi/triggers/mod.rs`.

- Register Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger: зорилтыг арилгана; устгах үйл явдлуудыг ялгаруулдаг. Нэмэлт шаталсан зайлуулах:
  - Домэйн бүртгэлээс хасах: домэйн дэх бүх бүртгэл, тэдгээрийн үүрэг, зөвшөөрөл, tx дарааллын тоолуур, дансны шошго, UAID холболтыг устгана; тэдний хөрөнгийг (мөн хөрөнгийн мета өгөгдлийг) устгадаг; домэйн дэх бүх хөрөнгийн тодорхойлолтыг устгадаг; домэйн дэх NFT болон хасагдсан акаунтуудын эзэмшиж буй аливаа NFT-г устгах; эрх мэдлийн домайн таарч байгаа триггерүүдийг устгана. Үйл явдал: `DomainEvent::Deleted`, мөн зүйл бүрийн устгалын үйл явдлууд. Алдаа: байхгүй бол `FindError::Domain`. Код: `core/.../isi/world.rs`.
  - Бүртгэлээс хасах: Бүртгэлийн зөвшөөрөл, үүрэг, tx дарааллын тоолуур, дансны шошгоны зураглал, UAID холболтыг устгана; дансны эзэмшиж буй хөрөнгийг (мөн хөрөнгийн мета өгөгдлийг) устгадаг; дансны эзэмшиж буй NFT-г устгах; тухайн дансны эрх мэдэл бүхий өдөөгчийг устгадаг. Үйл явдал: `AccountEvent::Deleted`, мөн хасагдсан NFT бүрт `NftEvent::Deleted`. Алдаа: байхгүй бол `FindError::Account`. Код: `core/.../isi/domain.rs`.
  - Unregister AssetDefinition: тухайн тодорхойлолтын бүх хөрөнгө болон тэдгээрийн нэг хөрөнгийн мета өгөгдлийг устгана. Үйл явдал: `AssetDefinitionEvent::Deleted` болон `AssetEvent::Deleted` нэг хөрөнгө. Алдаа: `FindError::AssetDefinition`. Код: `core/.../isi/domain.rs`.
  - NFT-г бүртгэлээс хасах: NFT-г устгана. Үйл явдал: `NftEvent::Deleted`. Алдаа: `FindError::Nft`. Код: `core/.../isi/nft.rs`.
  - Бүртгэлээс хасах: эхлээд бүх дансны үүргийг цуцална; дараа нь дүрийг арилгана. Үйл явдал: `RoleEvent::Deleted`. Алдаа: `FindError::Role`. Код: `core/.../isi/world.rs`.
  - Бүртгэлээс хасах триггер: хэрэв байгаа бол гохыг арилгана; давхардсан бүртгэлээс хасалт `Repetition(Unregister, TriggerId)` гарна. Үйл явдал: `TriggerEvent::Deleted`. Код: `core/.../isi/triggers/mod.rs`.

### Гаа / Түлэнхийн
Төрөл: `Mint<O, D: Identifiable>` ба `Burn<O, D: Identifiable>`, хайрцагласан `MintBox`/`BurnBox`.- Хөрөнгө (Тоон) гаа/шатаах: үлдэгдэл болон тодорхойлолтын `total_quantity`-ийг тохируулна.
  - Урьдчилсан нөхцөл: `Numeric` утга нь `AssetDefinition.spec()` шаардлагыг хангасан байх ёстой; `mintable` зөвшөөрөгдсөн гаа:
    - `Infinitely`: үргэлж зөвшөөрнө.
    - `Once`: яг нэг удаа зөвшөөрсөн; Эхний гаа нь `mintable`-ийг `Not` руу эргүүлж, `AssetDefinitionEvent::MintabilityChanged`, дээр нь аудит хийх боломжтой нарийвчилсан `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` ялгаруулдаг.
    - `Limited(n)`: `n` нэмэлт гаа үйл ажиллагааг зөвшөөрдөг. Амжилттай гаа бүр тоолуурыг бууруулдаг; тэг хүрэх үед тодорхойлолт нь `Not` болж хувирч, дээрхтэй ижил `MintabilityChanged` үйл явдлуудыг ялгаруулдаг.
    - `Not`: алдаа `MintabilityError::MintUnmintable`.
  - Төрийн өөрчлөлт: гаа дээр байхгүй бол хөрөнгө үүсгэнэ; Шатаахад үлдэгдэл тэг болсон тохиолдолд хөрөнгийн оруулгыг арилгана.
  - Үйл явдал: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (`Once` эсвэл `Limited(n)` тэтгэмжээ дуусгах үед).
  - Алдаа: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Код: `core/.../isi/asset.rs`.

- Триггерийн давталтуудыг гаа/шатаах: гох тоолох `action.repeats` өөрчлөлт.
  - Урьдчилсан нөхцөл: гаа дээр шүүлтүүр нь гаатай байх ёстой; арифметик хэт их/дутуу урсах ёсгүй.
  - Үйл явдал: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Алдаа: хүчингүй гаа дээр `MathError::Overflow`; Хэрэв байхгүй бол `FindError::Trigger`. Код: `core/.../isi/triggers/mod.rs`.

### Дамжуулах
Төрөл: `Transfer<S: Identifiable, O, D: Identifiable>`, хайрцагласан `TransferBox`.

- Хөрөнгө (Тоон): `AssetId` эх үүсвэрээс хасаж, `AssetId` хүрэх газарт нэмнэ (ижил тодорхойлолт, өөр данс). Тэглэгдсэн эх үүсвэрийг устгах.
  - Урьдчилсан нөхцөл: эх үүсвэр байгаа; утга нь `spec`-ийг хангасан.
  - Үйл явдал: `AssetEvent::Removed` (эх сурвалж), `AssetEvent::Added` (очих газар).
  - Алдаа: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Код: `core/.../isi/asset.rs`.

- Домэйн эзэмшил: `Domain.owned_by`-г очих данс болгон өөрчилнө.
  - Урьдчилсан нөхцөл: хоёр данс байгаа; домэйн байдаг.
  - Үйл явдал: `DomainEvent::OwnerChanged`.
  - Алдаа: `FindError::Account/Domain`. Код: `core/.../isi/domain.rs`.

- AssetDefinition эзэмшил: `AssetDefinition.owned_by`-г очих данс руу өөрчилнө.
  - Урьдчилсан нөхцөл: хоёр данс байгаа; тодорхойлолт байдаг; эх сурвалж одоогоор үүнийг эзэмших ёстой.
  - Үйл явдал: `AssetDefinitionEvent::OwnerChanged`.
  - Алдаа: `FindError::Account/AssetDefinition`. Код: `core/.../isi/account.rs`.

- NFT эзэмшил: `Nft.owned_by`-г очих данс руу өөрчилнө.
  - Урьдчилсан нөхцөл: хоёр данс байгаа; NFT байдаг; эх сурвалж одоогоор үүнийг эзэмших ёстой.
  - Үйл явдал: `NftEvent::OwnerChanged`.
  - Алдаа: эх сурвалж нь NFT-г эзэмшдэггүй бол `FindError::Account/Nft`, `InvariantViolation`. Код: `core/.../isi/nft.rs`.

### Мета өгөгдөл: Түлхүүр утгыг тохируулах/устгах
Төрөл: `SetKeyValue<T>` ба `RemoveKeyValue<T>`, `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Хайрцагласан тоонуудыг өгсөн.

- Set: оруулах буюу орлуулах `Metadata[key] = Json(value)`.
- Устгах: түлхүүрийг арилгах; байхгүй бол алдаа гарна.
- Үйл явдал: `<Target>Event::MetadataInserted` / `MetadataRemoved` хуучин/шинэ утгуудтай.
- Алдаа: зорилт байхгүй бол `FindError::<Target>`; `FindError::MetadataKey` арилгах түлхүүр дутуу байна. Код: `crates/iroha_data_model/src/isi/transparent.rs` ба зорилт болгон гүйцэтгэгч.### Зөвшөөрөл ба үүрэг: Зөвшөөрөх / хүчингүй болгох
Төрөл: `Grant<O, D>` ба `Revoke<O, D>`, `Permission`/`Role`-аас `Account`, Norito-аас Norito.

- Бүртгэлд зөвшөөрөл олгох: урьд өмнө нь байхгүй бол `Permission`-г нэмнэ. Үйл явдал: `AccountEvent::PermissionAdded`. Алдаа: Хэрэв давхардсан бол `Repetition(Grant, Permission)`. Код: `core/.../isi/account.rs`.
- Бүртгэлээс зөвшөөрөл цуцлах: байгаа бол устгана. Үйл явдал: `AccountEvent::PermissionRemoved`. Алдаа: байхгүй бол `FindError::Permission`. Код: `core/.../isi/account.rs`.
- Бүртгэлд үүрэг өгөх: байхгүй бол `(account, role)` зураглалыг оруулна. Үйл явдал: `AccountEvent::RoleGranted`. Алдаа: `Repetition(Grant, RoleId)`. Код: `core/.../isi/account.rs`.
- Бүртгэлээс үүрэг ролийг хүчингүй болгох: хэрэв байгаа бол зураглалыг устгана. Үйл явдал: `AccountEvent::RoleRevoked`. Алдаа: байхгүй бол `FindError::Role`. Код: `core/.../isi/account.rs`.
- Үүрэг гүйцэтгэх зөвшөөрөл олгох: зөвшөөрөл нэмснээр дүрийг дахин бүтээнэ. Үйл явдал: `RoleEvent::PermissionAdded`. Алдаа: `Repetition(Grant, Permission)`. Код: `core/.../isi/world.rs`.
- Дүрээс зөвшөөрлийг цуцлах: зөвшөөрөлгүйгээр дүрийг дахин бүтээдэг. Үйл явдал: `RoleEvent::PermissionRemoved`. Алдаа: байхгүй бол `FindError::Permission`. Код: `core/.../isi/world.rs`.

### Өдөөгч: Гүйцэтгэх
Төрөл: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Зан төлөв: гох дэд системд `ExecuteTriggerEvent { trigger_id, authority, args }` дараалалд оруулдаг. Гараар гүйцэтгэхийг зөвхөн дуудлагын триггерүүдэд зөвшөөрдөг (`ExecuteTrigger` шүүлтүүр); шүүлтүүр таарч байх ёстой бөгөөд дуудагч нь гох үйлдэл эрх мэдэлтэй байх эсвэл тухайн эрх мэдлийн `CanExecuteTrigger`-г барих ёстой. Хэрэглэгчийн өгсөн гүйцэтгэгч идэвхтэй байх үед триггерийн гүйцэтгэлийг ажлын цагийн гүйцэтгэгч баталгаажуулж, гүйлгээний гүйцэтгэгчийн түлшний төсвийг зарцуулдаг (суурь `executor.fuel` болон нэмэлт мета өгөгдөл `additional_fuel`).
- Алдаа: бүртгэгдээгүй бол `FindError::Trigger`; Хэрэв эрх бүхий бус хүн дуудсан бол `InvariantViolation`. Код: `core/.../isi/triggers/mod.rs` (мөн `core/.../smartcontracts/isi/mod.rs` дээрх туршилтууд).

### Шинэчлэх ба бүртгэл хийх
- `Upgrade { executor }`: өгөгдсөн `Executor` байт кодыг ашиглан гүйцэтгэгчийг шилжүүлж, гүйцэтгэгч болон түүний өгөгдлийн загварыг шинэчилж, `ExecutorEvent::Upgraded` ялгаруулдаг. Алдаа: шилжилт хөдөлгөөний бүтэлгүйтлийн үед `InvalidParameterError::SmartContract` гэж ороосон. Код: `core/.../isi/world.rs`.
- `Log { level, msg }`: өгөгдсөн түвшний зангилааны бүртгэлийг гаргадаг; төлөв өөрчлөгдөхгүй. Код: `core/.../isi/world.rs`.

### Алдааны загвар
Нийтлэг дугтуй: `InstructionExecutionError` үнэлгээний алдаа, асуулгын алдаа, хөрвүүлэлт, нэгж олдоогүй, давталт, үнэлэмж, математик, хүчингүй параметр, өөрчлөгдөөгүй зөрчлийн хувилбаруудтай. Тооллогууд болон туслахууд `pub mod error` доор `crates/iroha_data_model/src/isi/mod.rs` байна.

---## Гүйлгээ ба гүйцэтгэх файлууд
- `Executable`: `Instructions(ConstVec<InstructionBox>)` эсвэл `Ivm(IvmBytecode)`; байт кодыг base64 болгон цуваа болгодог. Код: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: мета өгөгдөл, `chain_id`, `authority`, `creation_time_ms`, нэмэлт I103200, Norito, мета өгөгдөл бүхий гүйцэтгэгдэх файлыг бүтээж, тэмдэглэж, багцлана. `nonce`. Код: `crates/iroha_data_model/src/transaction/`.
- Ажиллаж байх үед `iroha_core` нь `Execute for InstructionBox`-ээр дамжуулан `InstructionBox` багцуудыг гүйцэтгэж, тохирох `*Box` эсвэл тодорхой заавар болгон бууруулна. Код: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Ажиллах цагийн гүйцэтгэгч баталгаажуулалтын төсөв (хэрэглэгчийн өгсөн гүйцэтгэгч): үндсэн `executor.fuel` параметрүүдээс гадна гүйлгээний доторх заавар/триггер баталгаажуулалтаар хуваалцсан нэмэлт гүйлгээний мета өгөгдөл `additional_fuel` (`u64`).

---

## Инвариант ба тэмдэглэл (туршилт, хамгаалалтаас)
- Эхлэл хамгаалалт: `genesis` домэйн эсвэл `genesis` домэйн дахь бүртгэлийг бүртгэх боломжгүй; `genesis` бүртгэлийг бүртгэх боломжгүй. Код/тест: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Тоон хөрөнгө нь `NumericSpec` шаардлагыг хангасан байх ёстой. Spec үл нийцэх гарц `TypeError::AssetNumericSpec`.
- Цутгах чадвар: `Once` нь нэг гаа хэрэглэхийг зөвшөөрдөг бөгөөд дараа нь `Not` руу шилждэг; `Limited(n)` `Not` руу шилжихээсээ өмнө яг `n` гаа хийхийг зөвшөөрдөг. `Infinitely` дээр мөнгө хийхийг хориглох оролдлого нь `MintabilityError::ForbidMintOnMintable`-ийг үүсгэж, `Limited(0)`-г тохируулснаар `MintabilityError::InvalidMintabilityTokens` гарна.
- Мета өгөгдлийн үйлдлүүд нь түлхүүр юм; байхгүй түлхүүрийг арилгах нь алдаа юм.
- Триггер шүүлтүүрүүд нь ашиглах боломжгүй байж болно; дараа нь `Register<Trigger>` нь зөвхөн `Exactly(1)` давталтыг зөвшөөрдөг.
- `__enabled` (bool) хаалганы гүйцэтгэлийг идэвхжүүлэх мета өгөгдлийн түлхүүр; өгөгдмөл тохиргоог идэвхжүүлээгүй, идэвхгүй болгосон өдөөгчийг өгөгдөл/цаг/дуудлагын зам дээр алгасах болно.
- Детерминизм: бүх арифметик нь шалгасан үйлдлүүдийг ашигладаг; under/overflow нь бичсэн математикийн алдааг буцаана; тэг үлдэгдэл хөрөнгийн оруулгуудыг хасах (далд төлөв байхгүй).

---## Практик жишээнүүд
- Цутгах, шилжүүлэх:
  - `Mint::asset_numeric(10, asset_id)` → техникийн үзүүлэлт/зөвшөөрөгдсөн тохиолдолд 10 нэмнэ; үйл явдал: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 шилжих; хасах/нэмэх үйл явдлууд.
- Мета өгөгдлийн шинэчлэлтүүд:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → дээш; `RemoveKeyValue::account(...)`-ээр дамжуулан устгах.
- Үүрэг/зөвшөөрлийн удирдлага:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)`, тэдгээрийн `Revoke` аналогууд.
- Триггерийн амьдралын мөчлөг:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` шүүлтүүрээр унасан эсэхийг шалгах; `ExecuteTrigger::new(id).with_args(&args)` тохируулсан эрх мэдэлтэй тохирч байх ёстой.
  - `__enabled` мета өгөгдлийн түлхүүрийг `false` болгон тохируулснаар триггерийг идэвхгүй болгож болно (идэвхжүүлсэн өгөгдмөл байхгүй); `SetKeyValue::trigger` эсвэл IVM `set_trigger_enabled` системээр дамжуулан сэлгэх.
  - Ачааллын үед триггерийн хадгалалтыг засдаг: давхардсан id, таарахгүй id болон дутуу байт кодыг иш татсан триггерийг хассан; байт кодын лавлагааны тоог дахин тооцоолно.
  - Гүйцэтгэх үед триггерийн IVM байт код дутуу байвал гохыг устгаж, гүйцэтгэлийг бүтэлгүйтлийн үр дагавартай ажиллагаагүй гэж үзнэ.
  - Дууссан өдөөгчийг нэн даруй арилгадаг; Хэрэв гүйцэтгэлийн явцад дууссан оруулгатай тулгарвал түүнийг тайрч, алга болсонд тооцно.
- Параметр шинэчлэх:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` шинэчлэгдэж, `ConfigurationEvent::Changed` ялгаруулдаг.

---

## Мөшгих чадвар (сонгосон эх сурвалж)
 - Өгөгдлийн загварын цөм: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI тодорхойлолт ба бүртгэл: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI гүйцэтгэл: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Үйл явдал: `crates/iroha_data_model/src/events/**`.
 - Гүйлгээ: `crates/iroha_data_model/src/transaction/**`.

Хэрэв та энэ үзүүлэлтийг API/ зан үйлийн хүснэгт болгон өргөжүүлэх эсвэл тодорхой үйл явдал/алдаа бүртэй хөндлөн холбохыг хүсвэл энэ үгийг хэлээрэй, би үүнийг өргөтгөх болно.