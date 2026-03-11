---
lang: ur
direction: rtl
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c110536e456d6582c2dd2bd72a71fef25e3f43f7f369b3f1c0ce802564f0dbd
source_last_modified: "2026-01-28T18:33:51.649272+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha V2 ڈیٹا ماڈل - گہری ڈوبکی

یہ دستاویز ان ڈھانچے ، شناخت کاروں ، خصلتوں اور پروٹوکول کی وضاحت کرتی ہے جو Iroha V2 ڈیٹا ماڈل تشکیل دیتے ہیں ، جیسا کہ `iroha_data_model` کریٹ میں نافذ کیا گیا ہے اور کام کی جگہ میں استعمال کیا گیا ہے۔ اس کا مطلب ایک عین مطابق حوالہ ہے جس پر آپ جائزہ لے سکتے ہیں اور اس کی تازہ کاریوں کی تجویز کرسکتے ہیں۔

## دائرہ کار اور بنیادیں

- مقصد: ڈومین آبجیکٹ (ڈومینز ، اکاؤنٹس ، اثاثوں ، این ایف ٹی ، کردار ، اجازت ، ہم عمر) ، ریاست کو تبدیل کرنے والی ہدایات (آئی ایس آئی) ، سوالات ، ٹرگرز ، ٹرانزیکشنز ، بلاکس اور پیرامیٹرز کے لئے کیننیکل اقسام فراہم کریں۔
- سیریلائزیشن: تمام عوامی اقسام Norito کوڈیکس (`norito::codec::{Encode, Decode}`) اور اسکیما (`iroha_schema::IntoSchema`) اخذ کریں۔ فیچر جھنڈوں کے پیچھے JSON منتخب طور پر (جیسے HTTP اور `Json` پے لوڈ کے لئے) استعمال ہوتا ہے۔
- IVM نوٹ: Iroha ورچوئل مشین (IVM) کو نشانہ بناتے وقت کچھ ڈیسیریلائزیشن ٹائم کی توثیق کو غیر فعال کردیا جاتا ہے ، چونکہ میزبان معاہدوں سے پہلے توثیق کرتا ہے (`src/lib.rs` میں کریٹ دستاویزات دیکھیں)۔
- ایف ایف آئی گیٹس: کچھ اقسام `ffi_export`/`ffi_import` کے پیچھے `iroha_ffi` کے ذریعے FFI کے لئے مشروط طور پر تشریح کی جاتی ہیں جب ایف ایف آئی کی ضرورت نہیں ہوتی ہے۔

## بنیادی خصلت اور مددگار

- `Identifiable`: اداروں میں مستحکم `Id` اور `fn id(&self) -> &Self::Id` ہے۔ نقشہ/سیٹ دوستی کے ل I `IdEqOrdHash` کے ساتھ اخذ کیا جانا چاہئے۔
- `Registrable`/`Registered`: بہت ساری ہستیوں (جیسے ، `Domain` ، `AssetDefinition` ، `Role`) بلڈر پیٹرن کا استعمال کریں۔ `Registered` رن ٹائم ٹائپ کو ہلکے وزن والے بلڈر کی قسم (`With`) سے رجسٹریشن لین دین کے ل suitable موزوں کرتا ہے۔
- `HasMetadata`: کلیدی/قدر `Metadata` نقشہ تک متحد رسائی۔
- `IntoKeyValue`: نقل کو کم کرنے کے ل I `Key` (ID) اور `Value` (ڈیٹا) کو الگ الگ اسٹور کرنے کے لئے اسٹوریج اسپلٹ ہیلپر۔
- `Owned<T>`/`Ref<'world, K, V>`: غیر ضروری کاپیاں سے بچنے کے لئے اسٹوریجز اور استفسار فلٹرز میں ہلکا پھلکا ریپر استعمال کیا جاتا ہے۔

## نام اور شناخت کار

- `Name`: درست ٹیکسٹیکل شناخت کنندہ۔ وائٹ اسپیس اور محفوظ حروف `@` ، `#` ، `$` (جامع IDs میں استعمال ہونے والی) کی اجازت نہیں دیتا ہے۔ توثیق کے ساتھ `FromStr` کے ذریعے تعمیری۔ ناموں کو پارس پر یونیکوڈ این ایف سی کے لئے معمول بنا دیا جاتا ہے (کینونک طور پر مساوی ہجے کو یکساں اور ذخیرہ شدہ سمجھا جاتا ہے)۔ خصوصی نام `genesis` محفوظ ہے (کیس-غیر سنجیدگی سے چیک کیا گیا)۔
- `IdBox`: کسی بھی معاون ID (`DomainId` ، `AccountId` ، `AssetDefinitionId` ، `AssetId` ، `NftId` ، I18000067X ، Norito کے لئے ایک مجموعی قسم کا لفافہ `RoleId` ، `Permission` ، `CustomParameterId`)۔ عام بہاؤ اور Norito انکوڈنگ کے لئے ایک ہی قسم کے طور پر مفید ہے۔
- `ChainId`: لین دین میں ری پلے کے تحفظ کے لئے استعمال شدہ مبہم چین شناخت کنندہ۔IDs کے اسٹرنگ فارم (`Display`/`FromStr` کے ساتھ راؤنڈ ٹرپ ایبل):
- `DomainId`: `name` (جیسے ، `wonderland`)۔
- `AccountId`: کیننیکل شناخت کنندہ `AccountAddress` کے ذریعے انکوڈ کیا گیا ، جس میں I105 ، سورہ کمپریسڈ (`i105`) ، اور کینونیکل ہیکس کوڈیکس (`AccountAddress::to_i105` ، `to_i105` ، `to_i105` ، `AccountAddress::to_i105` ، `parse_encoded`)۔ I105 ترجیحی اکاؤنٹ کی شکل ہے۔ `i105` فارم صرف SORA UX کے لئے دوسرا بہترین ہے۔ انسانی دوستانہ روٹنگ عرف `alias` (rejected legacy form) UX کے لئے محفوظ ہے لیکن اب اسے مستند شناخت کنندہ نہیں سمجھا جاتا ہے۔ Torii `AccountAddress::parse_encoded` کے ذریعے آنے والے تار کو معمول پر لاتا ہے۔ اکاؤنٹ IDs سنگل کلیدی اور ملٹی سیگ کنٹرولرز دونوں کی حمایت کرتے ہیں۔
- `AssetDefinitionId`: `asset#domain` (جیسے ، `xor#soramitsu`)۔
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (جیسے ، `rose$garden`)۔
- `PeerId`: `public_key` (ہم مرتبہ مساوات عوامی کلید کے ذریعہ ہے)۔

## اداروں

### ڈومین
- `DomainId { name: Name }` - انوکھا نام۔
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`۔
- بلڈر: Norito کے ساتھ `NewDomain` ، `with_metadata` ، پھر `Registrable::build(authority)` سیٹ `owned_by`۔

### اکاؤنٹ
- `AccountId { domain: DomainId, controller: AccountController }` (کنٹرولر = سنگل کلید یا ملٹی سگ پالیسی)۔
- `Account { id, metadata, label?, uaid? }`- `label` ایک اختیاری مستحکم عرف ہے جو رکی ریکارڈز کے ذریعہ استعمال ہوتا ہے ، `uaid` اختیاری Nexus وسیع [یونیورسل اکاؤنٹ ID] (./universal_accounts_guide.md) رکھتا ہے۔
- بلڈر: `NewAccount` کے ذریعے `Account::new(id)` ؛ `HasMetadata` بلڈر اور ہستی دونوں کے لئے۔

### اثاثہ تعریفیں اور اثاثے
- `AssetDefinitionId { domain: DomainId, name: Name }`۔
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`۔
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`۔
  - بلڈرز: `AssetDefinition::new(id, spec)` یا سہولت `numeric(id)` ؛ `metadata` ، `mintable` ، `owned_by` کے لئے سیٹٹرز۔
- `AssetId { account: AccountId, definition: AssetDefinitionId }`۔
- `Asset { id, value: Numeric }` اسٹوریج دوستانہ `AssetEntry`/`AssetValue` کے ساتھ۔
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` سمری APIs کے لئے بے نقاب۔

### nfts
- `NftId { domain: DomainId, name: Name }`۔
- `Nft { id, content: Metadata, owned_by: AccountId }` (مواد صوابدیدی کلید/قدر میٹا ڈیٹا ہے)۔
- بلڈر: `NewNft` `Nft::new(id, content)` کے ذریعے۔

### کردار اور اجازتیں
- `RoleId { name: Name }`۔
- `Role { id, permissions: BTreeSet<Permission> }` بلڈر `NewRole { inner: Role, grant_to: AccountId }` کے ساتھ۔
- `Permission { name: Ident, payload: Json }` - `name` اور پے لوڈ اسکیما کو فعال `ExecutorDataModel` (نیچے ملاحظہ کریں) کے ساتھ سیدھ میں ہونا چاہئے۔

### ساتھی
- `PeerId { public_key: PublicKey }`۔
- `Peer { address: SocketAddr, id: PeerId }` اور پارسیبل `public_key@address` سٹرنگ فارم۔### کریپٹوگرافک قدیم (خصوصیت `sm`)
-`Sm2PublicKey` اور `Sm2Signature`: SM2 کے لئے SEC1-COMPLIANT پوائنٹس اور فکسڈ چوڑائی `r∥s` دستخط۔ تعمیر کنندگان منحنی خطوط اور امتیازی شناختوں کی توثیق کرتے ہیں۔ Norito انکوڈنگ `iroha_crypto` کے ذریعہ استعمال شدہ کیننیکل نمائندگی کی آئینہ دار ہے۔
- `Sm3Hash`: `[u8; 32]` NewType GM/T 0004 ڈائجسٹ کی نمائندگی کرتا ہے ، جو ظاہر ، ٹیلی میٹری ، اور سیسکل ردعمل میں استعمال ہوتا ہے۔
-`Sm4Key`: میزبان سیسکلز اور ڈیٹا ماڈل فکسچر کے مابین 128 بٹ سڈمٹرک کلیدی ریپر کا اشتراک کیا گیا۔
یہ اقسام موجودہ ED25519/BLS/ML-DSA قدیم کے ساتھ ساتھ بیٹھتی ہیں اور ایک بار جب کام کی جگہ `--features sm` کے ساتھ بنائی جاتی ہے تو عوامی اسکیما کا حصہ بن جاتا ہے۔

### محرکات اور واقعات
- `TriggerId { name: Name }` اور `Trigger { id, action: action::Action }`۔
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`۔
  - `Repeats`: `Indefinitely` یا `Exactly(u32)` ؛ آرڈر اور کمی کی افادیت شامل ہے۔
  - حفاظت: `TriggerCompleted` کو ایکشن کے فلٹر ((DE) سیریلائزیشن کے دوران توثیق) کے طور پر استعمال نہیں کیا جاسکتا۔
-`EventBox`: پائپ لائن ، پائپ لائن بیچ ، ڈیٹا ، وقت ، عملدرآمد ٹرگر ، اور ٹرگر سے مکمل ہونے والے واقعات کے لئے رقم کی قسم۔ `EventFilterBox` آئینہ دار ہے کہ سبسکرپشنز اور ٹرگر فلٹرز کے ل .۔

## پیرامیٹرز اور ترتیب

- سسٹم پیرامیٹر فیملیز (تمام `Default`ed ، گیٹٹر لے کر ، اور انفرادی انوم میں تبدیل ہوجائیں):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`۔
  - `BlockParameters { max_transactions: NonZeroU64 }`۔
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`۔
  - `SmartContractParameters { fuel, memory, execution_depth }`۔
- `Parameters` تمام کنبے اور `custom: BTreeMap<CustomParameterId, CustomParameter>` کو گروپس۔
-سنگل پیرامیٹر اینومس: `SumeragiParameter` ، `BlockParameter` ، `TransactionParameter` ، `SmartContractParameter` مختلف جیسے تازہ کاریوں اور تکرار کے لئے۔
- کسٹم پیرامیٹرز: ایگزیکٹر کی وضاحت ، `Json` کے طور پر لے جانے والی ، `CustomParameterId` (A `Name`) کے ذریعہ شناخت کی گئی ہے۔

## isi (Iroha خصوصی ہدایات)

- کور ٹریٹ: `dyn_encode` ، `as_any` ، اور ایک مستحکم فی ٹائپ شناخت کنندہ `id()` (کنکریٹ قسم کے نام سے پہلے سے طے شدہ) کے ساتھ `Instruction`۔ تمام ہدایات `Send + Sync + 'static` ہیں۔
- `InstructionBox`: کلون/EQ/ORD کے ساتھ `Box<dyn Instruction>` ریپر کی ملکیت ہے جس میں قسم ID + انکوڈڈ بائٹس کے ذریعے نافذ کیا گیا ہے۔
- بلٹ ان انسٹرکشن فیملیز کے تحت منظم کیا گیا ہے:
  - `mint_burn` ، `transfer` ، `register` ، اور مددگاروں کا `transparent` بنڈل۔
  - میٹا فلوز کے لئے اینومس ٹائپ کریں: `InstructionType` ، باکسڈ رقم جیسے `SetKeyValueBox` (ڈومین/اکاؤنٹ/اثاثہ_ڈیف/این ایف ٹی/ٹرگر)۔
- غلطیاں: `isi::error` کے تحت بھرپور غلطی کا ماڈل (تشخیص کی قسم کی غلطیاں ، غلطیاں تلاش کریں ، اشارے ، ریاضی ، غلط پیرامیٹرز ، تکرار ، حملہ آوریاں)۔
- انسٹرکشن رجسٹری: `instruction_registry!{ ... }` میکرو قسم کے نام کے ذریعہ کلید رن ٹائم ڈیکوڈ رجسٹری بناتا ہے۔ متحرک (ڈی ای) سیریلائزیشن کو حاصل کرنے کے لئے `InstructionBox` کلون اور Norito SERDE کے ذریعہ استعمال کیا جاتا ہے۔ اگر `set_instruction_registry(...)` کے ذریعہ کوئی رجسٹری واضح طور پر ترتیب نہیں دی گئی ہے تو ، تمام بنیادی ISI کے ساتھ ایک بلٹ ان ڈیفالٹ رجسٹری بائنریوں کو مضبوط رکھنے کے لئے پہلے استعمال پر لازمی طور پر نصب ہے۔

## لین دین- `Executable`: یا تو `Instructions(ConstVec<InstructionBox>)` یا `Ivm(IvmBytecode)`۔ `IvmBytecode` بیس 64 کے طور پر سیریلائز کرتا ہے (`Vec<u8>` سے زیادہ شفاف NewType)۔
- `TransactionBuilder`: `chain` ، `authority` ، `creation_time_ms` کے ساتھ ٹرانزیکشن پے لوڈ کی تعمیر کرتا ہے ، اختیاری `time_to_live_ms` اور `nonce` ، `metadata` ، اور ایک `name`۔
  - مددگار: `with_instructions` ، `with_bytecode` ، `with_executable` ، `with_metadata` ، `set_nonce` ، `set_ttl` ، `set_creation_time` ، `sign`۔
- `SignedTransaction` (`iroha_version` کے ساتھ ورژن): `TransactionSignature` اور پے لوڈ لے کر جاتا ہے۔ ہیشنگ اور دستخطی توثیق فراہم کرتا ہے۔
- انٹری پوائنٹس اور نتائج:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`۔
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` ہیشنگ مددگاروں کے ساتھ۔
  - `ExecutionStep(ConstVec<InstructionBox>)`: لین دین میں ہدایات کا ایک ہی آرڈرڈ بیچ۔

## بلاکس

- `SignedBlock` (ورژن) encapsulates:
  - `signatures: BTreeSet<BlockSignature>` (توثیق کرنے والوں سے) ،
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }` ،
  - `result: BlockResult` (ثانوی عملدرآمد حالت) جس میں `time_triggers` ، اندراج/نتیجہ مرکل کے درخت ، `transaction_results` ، اور `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` پر مشتمل ہے۔
- افادیت: `presigned` ، `set_transaction_results(...)` ، `set_transaction_results_with_transcripts(...)` ، `header()` ، `signatures()` ، `hash()` ، `add_signature` ، Kotodama
- مرکل کی جڑیں: ٹرانزیکشن انٹری پوائنٹس اور نتائج مرکل کے درختوں کے ذریعہ کیے جاتے ہیں۔ نتیجہ مرکل کی جڑ بلاک ہیڈر میں رکھی گئی ہے۔
- بلاک شمولیت کے ثبوت (`BlockProofs`) دونوں اندراج/نتیجہ مرکل کے ثبوت اور `fastpq_transcripts` نقشہ دونوں کو بے نقاب کریں تاکہ آف چین پرووئر ٹرانزیکشن ہیش سے وابستہ ٹرانسفر ڈیلٹا لاسکیں۔
-`ExecWitness` پیغامات (Torii کے ذریعے اسٹریمڈ اور اتفاق رائے کی گپ شپ پر پگی کی حمایت یافتہ) اب `fastpq_transcripts` اور پروور کے لئے تیار Norito دونوں شامل ہیں ، SOMEDIDED `public_inputs` (DSID ، SLOT ، RITKS) ٹرانسکرپٹس کو دوبارہ انکوڈنگ کے بغیر کیننیکل فاسٹ پی کیو قطاریں لگائیں۔

## سوالات

- دو ذائقے:
  - واحد: `SingularQuery<Output>` کو نافذ کریں (جیسے ، `FindParameters` ، `FindExecutorDataModel`)۔
  - ایٹ ایبل: `Query<Item>` کو نافذ کریں (جیسے ، `FindAccounts` ، `FindAssets` ، `FindDomains` ، وغیرہ)۔
- ٹائپ-ایریڈ فارم:
  - `QueryBox<T>` ایک باکسڈ ہے ، `Query<Item = T>` کو Norito SERDE کے ساتھ ایک عالمی رجسٹری کے تعاون سے تیار کیا گیا ہے۔
  - `QueryWithFilter<T> { query, predicate, selector }` ایک استفسار کو DSL پیش گوئی/سلیکٹر کے ساتھ جوڑتا ہے۔ `From` کے ذریعے مٹا دیئے گئے استفسار میں تبدیل ہوجاتا ہے۔
- رجسٹری اور کوڈیکس:
  - `query_registry!{ ... }` متحرک ڈیکوڈ کے لئے قسم کے نام کے ذریعہ تعمیر کنندگان کو ایک عالمی رجسٹری میپنگ کنکریٹ کے استفسار کی اقسام تیار کرتا ہے۔
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` اور `QueryResponse = Singular(..) | Iterable(QueryOutput)`۔
  - `QueryOutputBatchBox` یکساں ویکٹرز (جیسے ، `Vec<Account>` ، `Vec<Name>` ، `Vec<AssetDefinition>` ، `Vec<BlockHeader>`) ، پلس ٹپل اور ایکسٹینشن مدد کرنے والے موثر انداز میں شامل ہیں۔
-DSL: مرتب کی خصوصیات (`HasProjection<PredicateMarker>` / `SelectorMarker`) کے ساتھ `query::dsl` میں مرتب کردہ وقت کی جانچ پڑتال کی پیش گوئوں اور سلیکٹرز کے لئے نافذ کیا گیا ہے۔ ایک `fast_dsl` خصوصیت اگر ضرورت ہو تو ہلکے مختلف حالتوں کو بے نقاب کرتی ہے۔

## پھانسی اور توسیع- `Executor { bytecode: IvmBytecode }`: توثیق کرنے والے سے چلنے والا کوڈ بنڈل۔
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` نے پھانسی دینے والے سے طے شدہ ڈومین کا اعلان کیا:
  - کسٹم کنفیگریشن پیرامیٹرز ،
  - کسٹم انسٹرکشن شناخت کنندہ ،
  - اجازت ٹوکن شناخت کار ،
  - ایک JSON اسکیما جس میں کلائنٹ ٹولنگ کے لئے کسٹم اقسام کی وضاحت کی گئی ہے۔
- حسب ضرورت نمونے `data_model/samples/executor_custom_data_model` کے تحت موجود ہیں۔
  - `iroha_executor_data_model::permission::Permission` اخذ کے ذریعے کسٹم اجازت ٹوکن ،
  - کسٹم پیرامیٹر کو `CustomParameter` میں تبدیل کرنے والی قسم کے طور پر بیان کیا گیا ہے ،
  - کسٹم ہدایات پر عمل درآمد کے لئے `CustomInstruction` میں سیریلائز کیا گیا۔

### کسٹمر اسٹراکشن (ایگزیکٹر سے متعین آئی ایس آئی)

- قسم: `isi::CustomInstruction { payload: Json }` مستحکم تار ID `"iroha.custom"` کے ساتھ۔
- مقصد: نجی/کنسورشیم نیٹ ورکس میں پھانسی دینے والے مخصوص ہدایات کے لئے یا عوامی ڈیٹا ماڈل کو کانٹا کیے بغیر ، پروٹو ٹائپنگ کے لئے لفافہ۔
- ڈیفالٹ ایگزیکٹر سلوک: `iroha_core` میں بلٹ ان ایگزیکٹر `CustomInstruction` پر عمل درآمد نہیں کرتا ہے اور اگر اس کا سامنا کرنا پڑتا ہے تو گھبرا جائے گا۔ ایک کسٹم ایگزیکٹر کو `InstructionBox` کو `CustomInstruction` پر نیچے لانا چاہئے اور تمام جائزوں پر پے لوڈ کی تعی .ن سے تشریح کی جائے۔
- Norito: اسکیما کے ساتھ `norito::codec::{Encode, Decode}` کے ذریعے انکوڈس/ڈیکوڈس شامل ہیں۔ `Json` پے لوڈ کو عزم کے ساتھ سیریلائز کیا جاتا ہے۔ راؤنڈ ٹرپس اس وقت تک مستحکم ہیں جب تک کہ انسٹرکشن رجسٹری میں `CustomInstruction` شامل ہوتا ہے (یہ پہلے سے طے شدہ رجسٹری کا حصہ ہے)۔
- IVM: Kotodama IVM بائیکوڈ (`.to`) پر مرتب کرتا ہے اور درخواست کی منطق کے لئے تجویز کردہ راستہ ہے۔ ایگزیکٹر لیول ایکسٹینشن کے لئے صرف `CustomInstruction` استعمال کریں جن کا Kotodama میں ابھی تک اظہار نہیں کیا جاسکتا ہے۔ ساتھیوں میں عزم اور ایک جیسے پھانسی دینے والے بائنریز کو یقینی بنائیں۔
- عوامی نیٹ ورکس کے لئے نہیں: عوامی زنجیروں کے لئے استعمال نہ کریں جہاں متضاد پھانسی دینے والے اتفاق رائے کے کانٹے کا خطرہ ہیں۔ جب آپ کو پلیٹ فارم کی خصوصیات کی ضرورت ہو تو نیا بلٹ ان ISI upstream کی تجویز کرنے کو ترجیح دیں۔

## میٹا ڈیٹا

- `Metadata(BTreeMap<Name, Json>)`: کلیدی/ویلیو اسٹور ایک سے زیادہ اداروں سے منسلک (`Domain` ، `Account` ، `AssetDefinition` ، `Nft` ، ٹرگرز ، اور لین دین)۔
- API: `contains` ، `iter` ، `get` ، `insert` ، اور (`transparent_api` کے ساتھ) `remove`۔

## خصوصیات اور عزم

- کنٹرول اختیاری APIs (`std` ، `json` ، `transparent_api` ، `ffi_export` ، `ffi_import` ، `fast_dsl` ، `http` ، Norito)۔
- تعی .ن: تمام سیریلائزیشن Norito انکوڈنگ کا استعمال ہارڈ ویئر میں پورٹیبل ہونے کے لئے کرتی ہے۔ IVM بائیکوڈ ایک مبہم بائٹ بلاب ہے۔ پھانسی کو غیر طے شدہ کمی کو متعارف نہیں کرنا چاہئے۔ میزبان لین دین کی توثیق کرتا ہے اور IVM کو عزم طریقے سے آدانوں کی فراہمی کرتا ہے۔

### شفاف API (`transparent_api`)- مقصد: داخلی اجزاء جیسے Torii ، ایگزیکٹوز ، اور انضمام ٹیسٹ کے لئے Norito ڈھانچے/ENUMS تک مکمل ، تغیر پذیر رسائی کو بے نقاب کرتا ہے۔ اس کے بغیر ، وہ اشیاء جان بوجھ کر مبہم ہیں لہذا بیرونی ایس ڈی کے صرف محفوظ کنسٹرکٹرز اور انکوڈ پے لوڈ کو دیکھتے ہیں۔
- میکانکس: `iroha_data_model_derive::model` میکرو `#[cfg(feature = "transparent_api")] pub` کے ساتھ ہر عوامی فیلڈ کو دوبارہ لکھتا ہے اور پہلے سے طے شدہ تعمیر کے لئے نجی کاپی رکھتا ہے۔ خصوصیت کو چالو کرنے سے ان CFGs کو پلٹ جاتا ہے ، لہذا `Account` ، `Domain` ، `Asset` ، وغیرہ کو تباہ کرنا ان کے متعین ماڈیول سے باہر قانونی بن جاتا ہے۔
- سطح کا پتہ لگانا: کریٹ `TRANSPARENT_API: bool` مستقل برآمد کرتا ہے (`transparent_api.rs` یا `non_transparent_api.rs` میں سے کسی ایک میں پیدا ہوتا ہے)۔ جب بہاو کوڈ اس پرچم اور برانچ کو چیک کرسکتا ہے جب اسے مبہم مددگاروں کے پاس واپس گرنے کی ضرورت ہو۔
- چالو کرنا: `Cargo.toml` میں انحصار میں `features = ["transparent_api"]` شامل کریں۔ ورک اسپیس کریٹس جن کو JSON پروجیکشن کی ضرورت ہوتی ہے (جیسے ، `iroha_torii`) خود بخود پرچم کو آگے بڑھاتے ہیں ، لیکن تیسری پارٹی کے صارفین کو اس وقت تک اس کو دور رکھنا چاہئے جب تک کہ وہ تعیناتی پر قابو نہ رکھیں اور وسیع تر API سطح کو قبول کریں۔

## فوری مثالیں

ایک ڈومین اور اکاؤنٹ بنائیں ، اثاثہ کی وضاحت کریں ، اور ہدایات کے ساتھ لین دین بنائیں:

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

ڈی ایس ایل کے ساتھ سوال اکاؤنٹس اور اثاثے:

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
// Encode and send via Norito; decode on server using the query registry
```

IVM سمارٹ معاہدہ بائیکوڈ استعمال کریں:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## ورژننگ

- `SignedTransaction` ، `SignedBlock` ، اور `SignedQuery` کیننیکل Norito- انکوڈڈ ڈھانچے ہیں۔ `EncodeVersioned` کے ذریعے انکوڈ ہونے پر موجودہ ABI ورژن (فی الحال `1`) کے ساتھ اپنے پے لوڈ کو ماقبل کرنے کے لئے `iroha_version::Version` کو ہر ایک کا اطلاق کرتا ہے۔

## جائزہ نوٹ / ممکنہ تازہ کاریوں

- استفسار ڈی ایس ایل: عام فلٹرز/سلیکٹرز کے لئے مستحکم صارف کا سامنا کرنے والے سب سیٹ اور مثالوں کی دستاویزات پر غور کریں۔
- ہدایات فیملیز: `mint_burn` ، `register` ، `transfer` کے ذریعہ سامنے آنے والی ISI مختلف حالتوں کی فہرست میں عوامی دستاویزات کو بڑھاؤ۔

---
اگر کسی بھی حصے کو زیادہ گہرائی کی ضرورت ہے (جیسے ، مکمل آئی ایس آئی کیٹلاگ ، مکمل استفسار رجسٹری کی فہرست ، یا ہیڈر فیلڈز کو بلاک کریں) ، تو مجھے بتائیں اور میں ان حصوں کو اسی کے مطابق بڑھاؤں گا۔