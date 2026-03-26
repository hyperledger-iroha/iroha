---
lang: ur
direction: rtl
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 ڈیٹا ماڈل – گہرا غوطہ

یہ دستاویز ان ڈھانچے، شناخت کنندگان، خصائص اور پروٹوکولز کی وضاحت کرتی ہے جو Iroha v2 ڈیٹا ماڈل بناتے ہیں، جیسا کہ `iroha_data_model` کریٹ میں لاگو ہوتا ہے اور پوری ورک اسپیس میں استعمال ہوتا ہے۔ اس کا مطلب ایک قطعی حوالہ ہے جس کا آپ جائزہ لے سکتے ہیں اور اپ ڈیٹس تجویز کر سکتے ہیں۔

## دائرہ کار اور بنیادیں۔

- مقصد: ڈومین آبجیکٹس (ڈومینز، اکاؤنٹس، اثاثے، NFTs، کردار، اجازتیں، ہم مرتبہ)، ریاست میں تبدیلی کی ہدایات (ISI)، سوالات، محرکات، لین دین، بلاکس، اور پیرامیٹرز کے لیے کینونیکل اقسام فراہم کریں۔
- سیریلائزیشن: تمام عوامی اقسام Norito کوڈیکس (`norito::codec::{Encode, Decode}`) اور اسکیما (`iroha_schema::IntoSchema`) اخذ کرتی ہیں۔ JSON کو منتخب طور پر استعمال کیا جاتا ہے (مثال کے طور پر، HTTP اور `Json` پے لوڈز کے لیے) خصوصیت کے جھنڈوں کے پیچھے۔
- IVM نوٹ: Iroha ورچوئل مشین (IVM) کو نشانہ بناتے وقت کچھ ڈی سیریلائزیشن وقت کی توثیق کو غیر فعال کر دیا جاتا ہے، کیونکہ میزبان معاہدوں کی درخواست کرنے سے پہلے توثیق کرتا ہے (I008NI01 میں کریٹ دستاویزات دیکھیں)۔
- FFI گیٹس: کچھ قسمیں مشروط طور پر FFI کے لیے `iroha_ffi` کے ذریعے `ffi_export`/`ffi_import` کے پیچھے تشریح کی جاتی ہیں تاکہ FFI کی ضرورت نہ ہونے پر اوور ہیڈ سے بچا جا سکے۔

## بنیادی خصلتیں اور مددگار- `Identifiable`: اداروں کا `Id` اور `fn id(&self) -> &Self::Id` مستحکم ہوتا ہے۔ نقشہ/سیٹ دوستی کے لیے `IdEqOrdHash` کے ساتھ اخذ کیا جانا چاہیے۔
- `Registrable`/`Registered`: بہت سے ادارے (جیسے، `Domain`, `AssetDefinition`, `Role`) ایک بلڈر پیٹرن استعمال کرتے ہیں۔ `Registered` ties the runtime type to a lightweight builder type (`With`) suitable for registration transactions.
- `HasMetadata`: کلید/قدر `Metadata` نقشہ تک متحد رسائی۔
- `IntoKeyValue`: ڈپلیکیشن کو کم کرنے کے لیے `Key` (ID) اور `Value` (ڈیٹا) کو الگ الگ ذخیرہ کرنے کے لیے اسٹوریج اسپلٹ مددگار۔
- `Owned<T>`/`Ref<'world, K, V>`: غیر ضروری کاپیوں سے بچنے کے لیے سٹوریجز اور کوئوری فلٹرز میں استعمال ہونے والے ہلکے وزن کے ریپرز۔

## نام اور شناخت کرنے والے- `Name`: درست متنی شناخت کنندہ۔ خالی جگہ اور محفوظ کردہ حروف `@`, `#`, `$` (مکمل IDs میں استعمال ہونے) کی اجازت نہیں دیتا ہے۔ توثیق کے ساتھ `FromStr` کے ذریعے قابل تعمیر۔ ناموں کو پارس کرنے پر یونیکوڈ NFC میں معمول بنایا جاتا ہے (مثبت طور پر مساوی املا کو ایک جیسی اور ذخیرہ شدہ کمپوزڈ سمجھا جاتا ہے)۔ خصوصی نام `genesis` محفوظ ہے (کیس کے ساتھ غیر حساس طور پر چیک کیا گیا)۔
- `IdBox`: کسی بھی تعاون یافتہ ID کے لیے ایک مجموعہ قسم کا لفافہ `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`)۔ ایک قسم کے طور پر عام بہاؤ اور Norito انکوڈنگ کے لیے مفید ہے۔
- `ChainId`: مبہم چین شناخت کنندہ لین دین میں ری پلے تحفظ کے لیے استعمال ہوتا ہے۔IDs کی اسٹرنگ فارمز (`Display`/`FromStr` کے ساتھ گول ٹرپ ایبل):
- `DomainId`: `name` (e.g., `wonderland`).
- `AccountId`: کیننیکل ڈومین لیس اکاؤنٹ شناخت کنندہ `AccountAddress` کے ذریعے صرف i105 کے طور پر انکوڈ کیا گیا ہے۔ تجزیہ کار ان پٹ کو کینونیکل i105 ہونا چاہیے؛ ڈومین لاحقے (`@domain`)، کیننیکل i105 لٹریلز، عرفی لٹریلز، کینونیکل ہیکس پارسر ان پٹ، لیگیسی `norito:` پے لوڈز، اور `uaid:`/`opaque:` اکاؤنٹس کو دوبارہ ترتیب دیا گیا ہے۔
- `AssetDefinitionId`: کیننیکل `unprefixed Base58 address with versioning and checksum` (UUID-v4 بائٹس)۔
- `AssetId`: کیننیکل انکوڈ شدہ لٹریل `<canonical-base58-asset-definition-id>` (میراثی متنی شکلیں پہلی ریلیز میں تعاون یافتہ نہیں ہیں)۔
- `NftId`: `nft$domain` (جیسے، `rose$garden`)۔
- `PeerId`: `public_key` (ہم مرتبہ کی مساوات عوامی کلید کے ذریعہ ہے)۔

## ادارے

### ڈومین
- `DomainId { name: Name }` - منفرد نام۔
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`۔
- بلڈر: `NewDomain` `with_logo` کے ساتھ، `with_metadata`، پھر `Registrable::build(authority)` `owned_by` سیٹ کرتا ہے۔### اکاؤنٹ
- `AccountId` ایک کینونیکل ڈومین لیس اکاؤنٹ کی شناخت ہے جسے کنٹرولر نے کلید کیا ہے اور کینونیکل i105 کے طور پر انکوڈ کیا ہے۔
- `ScopedAccountId { account: AccountId, domain: DomainId }` صرف واضح ڈومین سیاق و سباق رکھتا ہے جہاں اسکوپڈ ویو کی ضرورت ہوتی ہے۔
- `Account { id, metadata, label?, uaid? }` — `label` ایک اختیاری مستحکم عرف ہے جسے ریکی ریکارڈز کے ذریعے استعمال کیا جاتا ہے، `uaid` اختیاری Nexus وسیع [یونیورسل اکاؤنٹ ID](Kotodama) رکھتا ہے۔
- بلڈر: `NewAccount` بذریعہ `Account::new(id)`؛ رجسٹریشن کے لیے ایک واضح `ScopedAccountId` ڈومین کی ضرورت ہوتی ہے اور ڈیفالٹس سے کسی کا اندازہ نہیں ہوتا ہے۔

### اثاثہ کی تعریفیں اور اثاثے۔
- `AssetDefinitionId { aid_bytes: [u8; 16] }` متنی طور پر `unprefixed Base58 address` کے طور پر ظاہر ہوا۔
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`۔

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` کو انسان کا سامنا کرنے والا ڈسپلے متن درکار ہے اور اس میں `#`/`@` شامل نہیں ہونا چاہیے۔
  - `alias` اختیاری ہے اور ان میں سے ایک ہونا ضروری ہے:
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    بائیں حصے کے ساتھ بالکل مماثل `AssetDefinition.name`۔
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`۔
  - بلڈرز: `AssetDefinition::new(id, spec)` یا سہولت `numeric(id)`؛ `name` درکار ہے اور `.with_name(...)` کے ذریعے سیٹ ہونا ضروری ہے۔
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`۔
- `Asset { id, value: Numeric }` اسٹوریج کے موافق `AssetEntry`/`AssetValue` کے ساتھ۔
- `AssetBalanceScope`: `Global` غیر محدود بیلنس کے لیے اور `Dataspace(DataSpaceId)` ڈیٹا اسپیس سے محدود بیلنس کے لیے۔
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` خلاصہ APIs کے لیے بے نقاب۔### NFTs
- `NftId { domain: DomainId, name: Name }`۔
- `Nft { id, content: Metadata, owned_by: AccountId }` (مواد صوابدیدی کلید/ویلیو میٹا ڈیٹا ہے)۔
- بلڈر: `NewNft` بذریعہ `Nft::new(id, content)`۔

### کردار اور اجازتیں۔
- `RoleId { name: Name }`۔
- بلڈر `NewRole { inner: Role, grant_to: AccountId }` کے ساتھ `Role { id, permissions: BTreeSet<Permission> }`۔
- `Permission { name: Ident, payload: Json }` – `name` اور پے لوڈ اسکیما کو فعال `ExecutorDataModel` کے ساتھ موافق ہونا چاہیے (نیچے دیکھیں)۔

### ساتھی
- `PeerId { public_key: PublicKey }`۔
- `Peer { address: SocketAddr, id: PeerId }` اور قابل تجزیہ `public_key@address` سٹرنگ فارم۔

### کرپٹوگرافک قدیم (خصوصیت `sm`)
- `Sm2PublicKey` اور `Sm2Signature`: SEC1 کے مطابق پوائنٹس اور SM2 کے لیے مقررہ چوڑائی `r∥s` دستخط۔ تعمیر کنندگان وکر کی رکنیت اور امتیازی IDs کی توثیق کرتے ہیں۔ Norito انکوڈنگ `iroha_crypto` کے ذریعے استعمال کی جانے والی کینونیکل نمائندگی کا آئینہ دار ہے۔
- `Sm3Hash`: `[u8; 32]` نئی قسم جو GM/T 0004 ڈائجسٹ کی نمائندگی کرتی ہے، مینی فیسٹس، ٹیلی میٹری، اور سیسکال ردعمل میں استعمال ہوتی ہے۔
- `Sm4Key`: 128 بٹ سمیٹرک کلید ریپر میزبان سیسکالز اور ڈیٹا ماڈل فکسچر کے درمیان اشتراک کیا گیا ہے۔
یہ اقسام موجودہ Ed25519/BLS/ML-DSA پرائمیٹوز کے ساتھ بیٹھتی ہیں اور `--features sm` کے ساتھ ورک اسپیس بننے کے بعد عوامی اسکیما کا حصہ بن جاتی ہیں۔### محرکات اور واقعات
- `TriggerId { name: Name }` اور `Trigger { id, action: action::Action }`۔
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`۔
  - `Repeats`: `Indefinitely` یا `Exactly(u32)`; آرڈرنگ اور ڈیپلیشن یوٹیلیٹیز شامل ہیں۔
  - سیفٹی: `TriggerCompleted` کو ایکشن فلٹر کے طور پر استعمال نہیں کیا جا سکتا (سیریلائزیشن کے دوران توثیق شدہ)۔
- `EventBox`: پائپ لائن، پائپ لائن-بیچ، ڈیٹا، ٹائم، ایگزیکٹ-ٹرگر، اور ٹرگر سے مکمل ہونے والے واقعات کے لیے رقم کی قسم؛ `EventFilterBox` سبسکرپشنز اور ٹرگر فلٹرز کے لیے آئینہ دار ہے۔

## پیرامیٹرز اور کنفیگریشن

- سسٹم پیرامیٹر فیملیز (تمام `Default`ed، کیری گیٹرز، اور انفرادی enums میں تبدیل):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`۔
  - `BlockParameters { max_transactions: NonZeroU64 }`۔
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`۔
  - `SmartContractParameters { fuel, memory, execution_depth }`۔
- `Parameters` تمام خاندانوں اور ایک `custom: BTreeMap<CustomParameterId, CustomParameter>` کو گروپ کرتا ہے۔
- سنگل پیرامیٹر اینمز: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` مختلف اپ ڈیٹس اور تکرار کے لیے۔
- حسب ضرورت پیرامیٹرز: ایگزیکیوٹر کی وضاحت کردہ، `Json` کے طور پر کی جاتی ہے، جس کی شناخت `CustomParameterId` (a `Name`) سے ہوتی ہے۔

## ISI (Iroha خصوصی ہدایات)- بنیادی خصوصیت: `Instruction` `dyn_encode`، `as_any`، اور ایک مستحکم فی قسم شناخت کنندہ `id()` (کنکریٹ قسم کے نام سے پہلے سے طے شدہ)۔ تمام ہدایات `Send + Sync + 'static` ہیں۔
- `InstructionBox`: clone/eq/ord کے ساتھ ملکیت `Box<dyn Instruction>` ریپر ٹائپ ID + انکوڈ شدہ بائٹس کے ذریعے لاگو کیا گیا ہے۔
- بلٹ ان انسٹرکشن فیملیز کو اس کے تحت منظم کیا جاتا ہے:
  - `mint_burn`, `transfer`, `register`، اور `transparent` مددگاروں کا بنڈل۔
  - میٹا فلو کے لیے enums ٹائپ کریں: `InstructionType`، باکسڈ رقم جیسے `SetKeyValueBox` (domain/account/asset_def/nft/trigger)۔
- غلطیاں: `isi::error` کے تحت بھرپور ایرر ماڈل (تشخیص کی قسم کی غلطیاں، غلطیاں تلاش کریں، مائنٹ ایبلٹی، ریاضی، غلط پیرامیٹرز، تکرار، انویریئنٹس)۔
- انسٹرکشن رجسٹری: `instruction_registry!{ ... }` میکرو ایک رن ٹائم ڈی کوڈ رجسٹری بناتا ہے جس کو قسم کے نام سے کلید کیا جاتا ہے۔ متحرک (de) سیریلائزیشن حاصل کرنے کے لیے `InstructionBox` کلون اور Norito serde کے ذریعے استعمال کیا جاتا ہے۔ اگر `set_instruction_registry(...)` کے ذریعے کوئی رجسٹری واضح طور پر سیٹ نہیں کی گئی ہے، تو بائنریز کو مضبوط رکھنے کے لیے پہلے استعمال پر تمام بنیادی ISI کے ساتھ ایک بلٹ ان ڈیفالٹ رجسٹری سستی سے انسٹال کی جاتی ہے۔

## لین دین- `Executable`: یا تو `Instructions(ConstVec<InstructionBox>)` یا `Ivm(IvmBytecode)`۔ `IvmBytecode` بیس64 (`Vec<u8>` پر شفاف نئی قسم) کے طور پر سیریلائز کرتا ہے۔
- `TransactionBuilder`: `chain`، `authority`، `creation_time_ms`، اختیاری `time_to_live_ms` اور Kotodama، اور `chain` کے ساتھ ایک ٹرانزیکشن پے لوڈ بناتا ہے `Executable`۔
  - مددگار: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Kotodama, Kotodama, `with_executable`.
- `SignedTransaction` (`iroha_version` کے ساتھ ورژن): `TransactionSignature` اور پے لوڈ لے جاتا ہے۔ ہیشنگ اور دستخط کی تصدیق فراہم کرتا ہے۔
- داخلے کے مقامات اور نتائج:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`۔
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` ہیشنگ مددگار کے ساتھ۔
  - `ExecutionStep(ConstVec<InstructionBox>)`: لین دین میں ہدایات کا ایک ہی آرڈر شدہ بیچ۔

## بلاکس- `SignedBlock` (ورژنڈ) انکیپسیلیٹس:
  - `signatures: BTreeSet<BlockSignature>` (تصدیق کرنے والوں سے)،
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`،
  - `result: BlockResult` (سیکنڈری ایگزیکیوشن اسٹیٹ) جس میں `time_triggers`، انٹری/نتیجہ مرکل ٹریز، `transaction_results`، اور `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` شامل ہیں۔
- یوٹیلیٹیز: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, `hash()`, Norito,00041
- مرکل کی جڑیں: لین دین کے داخلے کے مقامات اور نتائج مرکل کے درختوں کے ذریعے کیے جاتے ہیں۔ نتیجہ مرکل روٹ کو بلاک ہیڈر میں رکھا گیا ہے۔
- بلاک شمولیت کے ثبوت (`BlockProofs`) دونوں اندراج/نتیجہ مرکل کے ثبوت اور `fastpq_transcripts` نقشہ کو بے نقاب کرتے ہیں تاکہ آف چین پروورز ٹرانزیکشن ہیش سے وابستہ ٹرانسفر ڈیلٹا حاصل کر سکیں۔
- `ExecWitness` پیغامات (Torii کے ذریعے اسٹریم کیے گئے اور متفقہ گپ شپ پر piggy-backed) میں اب `fastpq_transcripts` اور prover-ready `fastpq_batches: Vec<FastpqTransitionBatch>` شامل ہیں perm_root، tx_set_hash)، اس لیے بیرونی پروررز ٹرانسکرپٹس کو دوبارہ انکوڈنگ کیے بغیر کیننیکل FASTPQ قطاریں نگل سکتے ہیں۔

## سوالات- دو ذائقے:
  - واحد: `SingularQuery<Output>` (مثال کے طور پر، `FindParameters`، `FindExecutorDataModel`) کو لاگو کریں۔
  - قابل تکرار: `Query<Item>` لاگو کریں (جیسے، `FindAccounts`، `FindAssets`، `FindDomains`، وغیرہ)۔
- قسم کے مٹائے گئے فارم:
  - `QueryBox<T>` ایک باکسڈ، مٹا ہوا `Query<Item = T>` Norito serde کے ساتھ ہے جسے عالمی رجسٹری کی حمایت حاصل ہے۔
  - `QueryWithFilter<T> { query, predicate, selector }` ایک سوال کو DSL predicate/ سلیکٹر کے ساتھ جوڑتا ہے۔ `From` کے ذریعے مٹائے جانے والے دوبارہ قابل استفسار میں تبدیل ہوتا ہے۔
- رجسٹری اور کوڈیکس:
  - `query_registry!{ ... }` ڈائنامک ڈی کوڈ کے لیے ٹائپ نام کے ذریعے کنسٹرکٹرز کے لیے عالمی رجسٹری میپنگ کنکریٹ استفسار کی اقسام تیار کرتا ہے۔
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` اور `QueryResponse = Singular(..) | Iterable(QueryOutput)`۔
  - `QueryOutputBatchBox` یکساں ویکٹرز (مثال کے طور پر، `Vec<Account>`، `Vec<Name>`، `Vec<AssetDefinition>`، `Vec<BlockHeader>`) پر ایک مجموعہ کی قسم ہے، پلس پیگنیشن ہیلپرز اور ایگزینشن ٹپلس کے لیے۔
- DSL: `query::dsl` میں پروجیکشن ٹریٹس (`HasProjection<PredicateMarker>` / `SelectorMarker`) کے ساتھ مرتب وقت کی جانچ شدہ پیشین گوئیوں اور سلیکٹرز کے لیے لاگو کیا گیا ہے۔ ایک `fast_dsl` خصوصیت اگر ضرورت ہو تو ہلکے قسم کو ظاہر کرتی ہے۔

## ایگزیکیوٹر اور توسیع پذیری۔- `Executor { bytecode: IvmBytecode }`: توثیق کنندہ کے ذریعے عملدرآمد شدہ کوڈ بنڈل۔
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` ایگزیکیوٹر کی وضاحت کردہ ڈومین کا اعلان کرتا ہے:
  - اپنی مرضی کے مطابق ترتیب کے پیرامیٹرز،
  - اپنی مرضی کے مطابق ہدایات کے شناخت کنندگان،
  - اجازت ٹوکن شناخت کنندگان،
  - ایک JSON اسکیما جو کلائنٹ ٹولنگ کے لیے حسب ضرورت اقسام کو بیان کرتا ہے۔
`data_model/samples/executor_custom_data_model` کے تحت حسب ضرورت نمونے موجود ہیں:
  - `iroha_executor_data_model::permission::Permission` اخذ کے ذریعے اپنی مرضی کے مطابق اجازت ٹوکن،
  - کسٹم پیرامیٹر کو `CustomParameter` میں کنورٹیبل قسم کے طور پر بیان کیا گیا،
  - عمل درآمد کے لیے `CustomInstruction` میں حسب ضرورت ہدایات کو سیریلائز کیا گیا۔

### کسٹم انسٹرکشن (ایگزیکیوٹر کی وضاحت کردہ ISI)- قسم: `isi::CustomInstruction { payload: Json }` مستحکم تار id `"iroha.custom"` کے ساتھ۔
- مقصد: پرائیویٹ/کنسورشیم نیٹ ورکس میں ایگزیکیوٹر کے لیے مخصوص ہدایات کے لیے لفافہ یا پروٹوٹائپنگ کے لیے، عوامی ڈیٹا ماڈل کو کانٹے کے بغیر۔
- پہلے سے طے شدہ ایگزیکیوٹر کا رویہ: `iroha_core` میں بلٹ ان ایگزیکیوٹر `CustomInstruction` پر عمل نہیں کرتا اور اگر سامنا ہوا تو گھبرا جائے گا۔ ایک کسٹم ایگزیکیوٹر کو `InstructionBox` کو `CustomInstruction` پر کم کرنا چاہیے اور تمام تصدیق کنندگان پر پے لوڈ کی تعییناتی طور پر تشریح کرنی چاہیے۔
- Norito: اسکیما کے ساتھ `norito::codec::{Encode, Decode}` کے ذریعے انکوڈز/ڈی کوڈز؛ `Json` پے لوڈ کو ترتیب سے ترتیب دیا گیا ہے۔ راؤنڈ ٹرپس اس وقت تک مستحکم رہتے ہیں جب تک انسٹرکشن رجسٹری میں `CustomInstruction` شامل ہو (یہ ڈیفالٹ رجسٹری کا حصہ ہے)۔
- IVM: Kotodama IVM بائیک کوڈ (`.to`) پر مرتب کرتا ہے اور اطلاق کی منطق کے لیے تجویز کردہ راستہ ہے۔ صرف `CustomInstruction` کو ایگزیکیوٹر لیول ایکسٹینشنز کے لیے استعمال کریں جن کا ابھی تک Kotodama میں اظہار نہیں کیا جا سکتا ہے۔ ساتھیوں میں عزم اور یکساں ایگزیکیوٹر بائنریز کو یقینی بنائیں۔
- عوامی نیٹ ورکس کے لیے نہیں: عوامی زنجیروں کے لیے استعمال نہ کریں جہاں متضاد عمل آوروں کو اتفاق رائے کے کانٹے کا خطرہ ہو۔ جب آپ کو پلیٹ فارم کی خصوصیات کی ضرورت ہو تو نئے بلٹ ان ISI upstream تجویز کرنے کو ترجیح دیں۔

## میٹا ڈیٹا- `Metadata(BTreeMap<Name, Json>)`: ایک سے زیادہ اداروں سے منسلک کلید/ویلیو اسٹور (`Domain`, `Account`, `AssetDefinition`, `Nft`، ٹرگرز، اور ٹرانزیکشنز)۔
- API: `contains`, `iter`, `get`, `insert`، اور (`transparent_api` کے ساتھ) `remove`۔

## خصوصیات اور عزم

- خصوصیات کنٹرول اختیاری APIs (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `fast_dsl`, I108NI, I108NI `fault_injection`)۔
- ڈیٹرمنزم: تمام سیریلائزیشن Norito انکوڈنگ کو ہارڈ ویئر میں پورٹیبل ہونے کے لیے استعمال کرتی ہے۔ IVM بائٹ کوڈ ایک مبہم بائٹ بلاب ہے۔ پھانسی کو غیر مقررہ کمیوں کو متعارف نہیں کرانا چاہیے۔ میزبان لین دین کی توثیق کرتا ہے اور IVM کو ان پٹ کی سپلائی کرتا ہے۔

### شفاف API (`transparent_api`)- مقصد: اندرونی اجزاء جیسے Torii، ایگزیکیوٹرز، اور انٹیگریشن ٹیسٹس کے لیے `#[model]` سٹرکٹس/enums تک مکمل، تبدیل ہونے والی رسائی کو ظاہر کرتا ہے۔ اس کے بغیر، وہ آئٹمز جان بوجھ کر مبہم ہیں لہذا بیرونی SDKs صرف محفوظ کنسٹرکٹرز اور انکوڈ شدہ پے لوڈز کو دیکھتے ہیں۔
- میکانکس: `iroha_data_model_derive::model` میکرو ہر عوامی فیلڈ کو `#[cfg(feature = "transparent_api")] pub` کے ساتھ دوبارہ لکھتا ہے اور پہلے سے طے شدہ تعمیر کے لیے ایک نجی کاپی رکھتا ہے۔ خصوصیت کو فعال کرنے سے وہ cfgs پلٹ جاتے ہیں، لہذا `Account`، `Domain`، `Asset`، وغیرہ کو ڈیسٹرکچر کرنا ان کے متعین ماڈیولز سے باہر قانونی ہو جاتا ہے۔
- سطح کا پتہ لگانا: کریٹ ایک `TRANSPARENT_API: bool` مستقل برآمد کرتا ہے (یا تو `transparent_api.rs` یا `non_transparent_api.rs` میں تیار ہوتا ہے)۔ ڈاؤن اسٹریم کوڈ اس جھنڈے اور برانچ کو چیک کر سکتا ہے جب اسے مبہم مددگاروں کے پاس واپس آنے کی ضرورت ہو۔
- فعال کرنا: `features = ["transparent_api"]` کو `Cargo.toml` میں انحصار میں شامل کریں۔ ورک اسپیس کریٹس جن کو JSON پروجیکشن کی ضرورت ہوتی ہے (مثال کے طور پر، `iroha_torii`) جھنڈے کو خود بخود آگے بھیج دیتے ہیں، لیکن فریق ثالث کے صارفین کو اسے بند رکھنا چاہیے جب تک کہ وہ تعیناتی کو کنٹرول نہ کریں اور وسیع تر API کی سطح کو قبول نہ کریں۔

## فوری مثالیں۔

ایک ڈومین اور اکاؤنٹ بنائیں، ایک اثاثہ کی وضاحت کریں، اور ہدایات کے ساتھ ایک لین دین بنائیں:

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

ڈی ایس ایل کے ساتھ اکاؤنٹس اور اثاثوں سے استفسار کریں:

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

IVM سمارٹ کنٹریکٹ بائی کوڈ استعمال کریں:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / عرف فوری حوالہ (CLI + Torii):

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
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```نقل مکانی نوٹ:
- پرانی `name#domain` اثاثہ کی تعریف کی IDs v1 میں قبول نہیں کی جاتی ہیں۔
- ٹکسال/برن/منتقلی کے لیے اثاثہ IDs کیننیکل `<canonical-base58-asset-definition-id>` رہیں۔ ان کے ساتھ بنائیں:
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - یا `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`۔

## ورژن بنانا

- `SignedTransaction`, `SignedBlock`، اور `SignedQuery` کینونیکل Norito-انکوڈ شدہ ڈھانچہ ہیں۔ ہر ایک اپنے پے لوڈ کو موجودہ ABI ورژن (فی الحال `1`) کے ساتھ سابقہ ​​لگانے کے لیے `iroha_version::Version` کو لاگو کرتا ہے جب `EncodeVersioned` کے ذریعے انکوڈ کیا جاتا ہے۔

## جائزہ نوٹس / ممکنہ اپ ڈیٹس

- سوال DSL: ایک مستحکم صارف کا سامنا کرنے والے سب سیٹ اور عام فلٹرز/سلیکٹرز کے لیے مثالیں دستاویز کرنے پر غور کریں۔
- انسٹرکشن فیملیز: `mint_burn`، `register`، `transfer` کے ذریعے سامنے آنے والے بلٹ ان ISI مختلف حالتوں کی فہرست والے عوامی دستاویزات کو پھیلائیں۔

---
اگر کسی حصے کو مزید گہرائی کی ضرورت ہے (جیسے، مکمل ISI کیٹلاگ، مکمل استفسار کی رجسٹری کی فہرست، یا بلاک ہیڈر فیلڈز)، تو مجھے بتائیں اور میں اس کے مطابق ان حصوں کو بڑھا دوں گا۔