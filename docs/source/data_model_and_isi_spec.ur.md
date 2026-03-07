---
lang: ur
direction: rtl
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:33:51.650362+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha V2 ڈیٹا ماڈل اور ISI - عمل درآمد - ماخوذ مخصوص

یہ تصریح ریورس ہے - موجودہ عمل سے `iroha_data_model` اور `iroha_core` میں ڈیزائن کے جائزے کی مدد کے لئے انجینئرڈ ہے۔ بیک ٹکس میں راستے مستند کوڈ کی طرف اشارہ کرتے ہیں۔

## دائرہ کار
- کیننیکل ہستیوں (ڈومینز ، اکاؤنٹس ، اثاثوں ، این ایف ٹی ، کردار ، اجازت ، ہم عمر ، محرکات) اور ان کے شناخت کاروں کی وضاحت کرتا ہے۔
- ریاست - تبدیلی کی ہدایات (ISI) کی وضاحت کرتا ہے: اقسام ، پیرامیٹرز ، پیشگی شرطیں ، ریاستی منتقلی ، خارج ہونے والے واقعات ، اور غلطی کی شرائط۔
- پیرامیٹر مینجمنٹ ، لین دین ، ​​اور انسٹرکشن سیریلائزیشن کا خلاصہ پیش کرتا ہے۔

تعی .ن: تمام انسٹرکشن سیمنٹکس خالص ریاستی منتقلی ہیں جو ہارڈ ویئر کے بغیر - منحصر سلوک کے بغیر ہیں۔ سیریلائزیشن Norito کا استعمال کرتی ہے۔ VM بائٹ کوڈ IVM کا استعمال کرتا ہے اور اس کی توثیق کی جاتی ہے۔

---

## ادارے اور شناخت کار
`Display`/`FromStr` راؤنڈ - ٹرپ کے ساتھ IDs کے مستحکم اسٹرنگ فارم ہیں۔ نام کے قواعد وائٹ اسپیس اور محفوظ `@ # $` حروف سے منع کرتے ہیں۔- `Name` - درست ٹیکسٹیکل شناخت کنندہ۔ قواعد: `crates/iroha_data_model/src/name.rs`۔
- `DomainId` - `name`۔ ڈومین: `{ id, logo, metadata, owned_by }`۔ بلڈرز: `NewDomain`۔ کوڈ: `crates/iroha_data_model/src/domain.rs`۔
- `AccountId` - کیننیکل ایڈریس `AccountAddress` (IH58 / `sora…` کمپریسڈ / ہیکس) کے ذریعے تیار کیے جاتے ہیں اور Torii `AccountAddress::parse_encoded` کے ذریعے آدانوں کو معمول بناتا ہے۔ IH58 ترجیحی اکاؤنٹ کی شکل ہے۔ `sora…` فارم صرف SORA UX کے لئے دوسرا بہترین ہے۔ واقف `alias@domain` سٹرنگ صرف روٹنگ عرف کے طور پر برقرار ہے۔ اکاؤنٹ: `{ id, metadata }`۔ کوڈ: `crates/iroha_data_model/src/account.rs`۔
- اکاؤنٹ میں داخلے کی پالیسی- ڈومینز میٹا ڈیٹا کلید `iroha:account_admission_policy` کے تحت Norito-JSON `AccountAdmissionPolicy` کو اسٹور کرکے اکاؤنٹ کی تخلیق کو کنٹرول کرتے ہیں۔ جب کلید غیر حاضر ہے تو ، چین کی سطح کا کسٹم پیرامیٹر `iroha:default_account_admission_policy` پہلے سے طے شدہ فراہم کرتا ہے۔ جب یہ بھی غیر حاضر ہے تو ، سخت ڈیفالٹ `ImplicitReceive` (پہلی ریلیز) ہے۔ پالیسی ٹیگز `mode` (`ExplicitOnly` یا `ImplicitReceive`) کے علاوہ اختیاری فی ٹرانزیکشن (پہلے سے طے شدہ `16`) اور فی بلاک تخلیق کیپس ، ایک اختیاری `implicit_creation_fee` (برن یا سنک اکاؤنٹ) ، I18NIC `default_role_on_create` (`AccountCreated` کے بعد عطا کیا گیا ، اگر غائب ہو تو `DefaultRoleError` کے ساتھ مسترد ہوجاتا ہے)۔ پیدائش آپٹ نہیں کر سکتی۔ غیر فعال/غلط پالیسیاں `InstructionExecutionError::AccountAdmission` کے ساتھ نامعلوم اکاؤنٹس کے لئے رسید طرز کی ہدایات کو مسترد کرتی ہیں۔ `AccountCreated` سے پہلے مضمر اکاؤنٹس اسٹیمپ میٹا ڈیٹا `iroha:created_via="implicit"` ؛ پہلے سے طے شدہ کردار `AccountRoleGranted` کی فالو اپ کا اخراج کرتے ہیں ، اور ایگزیکٹر کے مالک بیس لائن قواعد کو نئے اکاؤنٹ میں اضافی کردار کے بغیر اپنے اثاثوں/NFTs کو خرچ کرنے دیتا ہے۔ کوڈ: `crates/iroha_data_model/src/account/admission.rs` ، `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`۔
- `AssetDefinitionId` - `asset#domain`۔ تعریف: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`۔ کوڈ: `crates/iroha_data_model/src/asset/definition.rs`۔
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` - `nft$domain`۔ NFT: `{ id, content: Metadata, owned_by }`۔ کوڈ: `crates/iroha_data_model/src/nft.rs`۔
- `RoleId` - `name`۔ کردار: `{ id, permissions: BTreeSet<Permission> }` کے ساتھ بلڈر `NewRole { inner: Role, grant_to }`۔ کوڈ: `crates/iroha_data_model/src/role.rs`۔
- `Permission` - `{ name: Ident, payload: Json }`۔ کوڈ: `crates/iroha_data_model/src/permission.rs`۔
- `PeerId`/`Peer` - ہم مرتبہ شناخت (عوامی کلید) اور پتہ۔ کوڈ: `crates/iroha_data_model/src/peer.rs`۔
- `TriggerId` - `name`۔ ٹرگر: `{ id, action }`۔ ایکشن: `{ executable, repeats, authority, filter, metadata }`۔ کوڈ: `crates/iroha_data_model/src/trigger/`۔
- `Metadata` - `BTreeMap<Name, Json>` چیک شدہ داخل/ہٹانے کے ساتھ۔ کوڈ: `crates/iroha_data_model/src/metadata.rs`۔
- سبسکرپشن پیٹرن (ایپلی کیشن پرت): منصوبے `AssetDefinition` `subscription_plan` میٹا ڈیٹا کے ساتھ اندراجات ہیں۔ سبسکرپشن `Nft` ریکارڈ ہیں جس میں `subscription` میٹا ڈیٹا ہے۔ بلنگ کو وقت کے ذریعہ عمل میں لایا جاتا ہے۔ `docs/source/subscriptions_api.md` اور `crates/iroha_data_model/src/subscription.rs` دیکھیں۔
- ** کریپٹوگرافک قدیم ** (خصوصیت `sm`):- `Sm2PublicKey` / `Sm2Signature` Canonical Sec1 پوائنٹ + فکسڈ چوڑائی `r∥s` SM2 کے لئے انکوڈنگ کا آئینہ۔ کنسٹرکٹرز وکر کی رکنیت اور امتیازی ID Semantics (`DEFAULT_DISTID`) نافذ کرتے ہیں ، جبکہ تصدیق خراب یا اعلی رینج اسکیلرز کو مسترد کرتی ہے۔ کوڈ: `crates/iroha_crypto/src/sm.rs` اور `crates/iroha_data_model/src/crypto/mod.rs`۔
  - `Sm3Hash` GM/T 0004 ڈائجسٹ کو Norito-SERIALISABLE `[u8; 32]` NewType کے طور پر بے نقاب کرتا ہے جہاں بھی ہیشس ظاہر یا ٹیلی میٹری میں ظاہر ہوتا ہے۔ کوڈ: `crates/iroha_data_model/src/crypto/hash.rs`۔
  -`Sm4Key` 128 بٹ SM4 کیز کی نمائندگی کرتا ہے اور میزبان سیسکلز اور ڈیٹا ماڈل فکسچر کے مابین مشترکہ ہے۔ کوڈ: `crates/iroha_data_model/src/crypto/symmetric.rs`۔
  یہ قسمیں موجودہ ED25519/BLS/ML-DSA قدیم کے ساتھ ساتھ بیٹھتی ہیں اور ایک بار `sm` کی خصوصیت کو فعال کرنے کے بعد ڈیٹا ماڈل صارفین (Torii ، SDKS ، جینیسس ٹولنگ) کے لئے دستیاب ہیں۔

اہم خصوصیات: `Identifiable` ، `Registered`/`Registrable` (بلڈر پیٹرن) ، `HasMetadata` ، `IntoKeyValue`۔ کوڈ: `crates/iroha_data_model/src/lib.rs`۔

واقعات: ہر ہستی میں تغیرات پر واقعات خارج ہوتے ہیں (تخلیق/حذف کریں/مالک تبدیل/میٹا ڈیٹا تبدیل ، وغیرہ)۔ کوڈ: `crates/iroha_data_model/src/events/`۔

---

## پیرامیٹرز (چین کی تشکیل)
- فیملیز: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }` ، `BlockParameters { max_transactions }` ، `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }` ، `SmartContractParameters { fuel, memory, execution_depth }` ، نیز `custom: BTreeMap`۔
- فرق کے لئے سنگل انومس: `SumeragiParameter` ، `BlockParameter` ، `TransactionParameter` ، `SmartContractParameter`۔ جمع کرنے والا: `Parameters`۔ کوڈ: `crates/iroha_data_model/src/parameter/system.rs`۔

پیرامیٹرز کی ترتیب (ISI): `SetParameter(Parameter)` متعلقہ فیلڈ کو اپ ڈیٹ کرتا ہے اور `ConfigurationEvent::Changed` کو خارج کرتا ہے۔ کوڈ: `crates/iroha_data_model/src/isi/transparent.rs` ، `crates/iroha_core/src/smartcontracts/isi/world.rs` میں ایگزیکٹر۔

---

## ہدایات سیریلائزیشن اور رجسٹری
- کور ٹریٹ: `dyn_encode()` کے ساتھ `Instruction: Send + Sync + 'static` ، `as_any()` ، مستحکم `id()` (کنکریٹ قسم کے نام سے پہلے سے طے شدہ)۔
- `InstructionBox`: `Box<dyn Instruction>` ریپر۔ کلون/EQ/ORD `(type_id, encoded_bytes)` پر کام کرتا ہے لہذا مساوات قدر کے لحاظ سے ہے۔
- Norito SERDE `InstructionBox` سیریلائزز کے طور پر `(String wire_id, Vec<u8> payload)` (اگر کوئی وائر ID نہیں ہے تو `type_name` پر واپس آتا ہے)۔ ڈیسیریلائزیشن کنسٹرکٹرز کو عالمی `InstructionRegistry` میپنگ شناخت کاروں کا استعمال کرتی ہے۔ پہلے سے طے شدہ رجسٹری میں ISI میں تمام تعمیرات شامل ہیں۔ کوڈ: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`۔

---

## ISI: اقسام ، الفاظ ، غلطیاں
پھانسی `Execute for <Instruction>` کے ذریعے `iroha_core::smartcontracts::isi` میں نافذ کی جاتی ہے۔ ذیل میں عوامی اثرات ، پیشگی شرط ، خارج ہونے والے واقعات اور غلطیوں کی فہرست دی گئی ہے۔

### رجسٹر / غیر رجسٹر
اقسام: `Register<T: Registered>` اور `Unregister<T: Identifiable>` ، SUM قسم کے ساتھ `RegisterBox`/`UnregisterBox` کنکریٹ اہداف کا احاطہ کرتا ہے۔

- ہم مرتبہ رجسٹر کریں: عالمی ہم عمروں کے سیٹ میں داخل کریں۔
  - پیشگی شرط: پہلے سے موجود نہیں ہونا چاہئے۔
  - واقعات: `PeerEvent::Added`۔
  - غلطیاں: `Repetition(Register, PeerId)` اگر نقل ؛ `FindError` پر تلاش کریں۔ کوڈ: `core/.../isi/world.rs`۔

- رجسٹر ڈومین: `NewDomain` سے `owned_by = authority` کے ساتھ تعمیر کرتا ہے۔ اجازت نہیں: `genesis` ڈومین۔
  - پیشگی شرائط: ڈومین عدم وجود ؛ `genesis` نہیں۔
  - واقعات: `DomainEvent::Created`۔
  - غلطیاں: `Repetition(Register, DomainId)` ، `InvariantViolation("Not allowed to register genesis domain")`۔ کوڈ: `core/.../isi/world.rs`۔- رجسٹر اکاؤنٹ: `NewAccount` سے تعمیر ، `genesis` ڈومین میں اجازت نہیں۔ `genesis` اکاؤنٹ رجسٹر نہیں ہوسکتا ہے۔
  - پیشگی شرطیں: ڈومین موجود ہونا ضروری ہے۔ اکاؤنٹ عدم موجودگی ؛ پیدائش ڈومین میں نہیں۔
  - واقعات: `DomainEvent::Account(AccountEvent::Created)`۔
  - غلطیاں: `Repetition(Register, AccountId)` ، `InvariantViolation("Not allowed to register account in genesis domain")`۔ کوڈ: `core/.../isi/domain.rs`۔

- اثاثہ بیان کریں: بلڈر سے تعمیر ؛ `owned_by = authority` سیٹ کرتا ہے۔
  - پیشگی شرطیں: تعریف عدم وجود ؛ ڈومین موجود ہے۔
  - واقعات: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`۔
  - غلطیاں: `Repetition(Register, AssetDefinitionId)`۔ کوڈ: `core/.../isi/domain.rs`۔

- این ایف ٹی کو رجسٹر کریں: بلڈر سے تعمیر ؛ `owned_by = authority` سیٹ کرتا ہے۔
  - پیشگی شرائط: NFT عدم وجود ؛ ڈومین موجود ہے۔
  - واقعات: `DomainEvent::Nft(NftEvent::Created)`۔
  - غلطیاں: `Repetition(Register, NftId)`۔ کوڈ: `core/.../isi/nft.rs`۔

- رجسٹر رول: `NewRole { inner, grant_to }` (اکاؤنٹ کے ذریعے ریکارڈ شدہ پہلا مالک - رول میپنگ) سے تعمیر کرتا ہے ، `inner: Role` اسٹور کرتا ہے۔
  - پیشگی شرطیں: کردار عدم وجود۔
  - واقعات: `RoleEvent::Created`۔
  - غلطیاں: `Repetition(Register, RoleId)`۔ کوڈ: `core/.../isi/world.rs`۔

- رجسٹر ٹرگر: ٹرگر کو فلٹر قسم کے ذریعہ مناسب ٹرگر سیٹ میں اسٹور کرتا ہے۔
  - پیشگی شرط: اگر فلٹر ٹکسال نہیں ہے تو ، `action.repeats` `Exactly(1)` (بصورت دیگر `MathError::Overflow`) ہونا چاہئے۔ ڈپلیکیٹ آئی ڈی ممنوع ہیں۔
  - واقعات: `TriggerEvent::Created(TriggerId)`۔
  - غلطیاں: `Repetition(Register, TriggerId)` ، تبادلوں/توثیق کی ناکامیوں پر `InvalidParameterError::SmartContract(..)`۔ کوڈ: `core/.../isi/triggers/mod.rs`۔

- غیر منظم پیر/ڈومین/اکاؤنٹ/اثاثہ ڈیفائنیشن/این ایف ٹی/کردار/ٹرگر: ہدف کو ہٹا دیتا ہے۔ حذف کرنے کے واقعات کو خارج کرتا ہے۔ اضافی کاسکیڈنگ ہٹانے:
  - غیر منظم ڈومین: ڈومین ، ان کے کردار ، اجازت ، TX-ترتیب کاؤنٹرز ، اکاؤنٹ لیبل ، اور UAID پابندوں میں تمام اکاؤنٹس کو ہٹا دیتا ہے۔ ان کے اثاثوں کو حذف کرتا ہے (اور فی ایسٹ میٹا ڈیٹا) ؛ ڈومین میں اثاثوں کی تمام تعریفوں کو ہٹا دیتا ہے۔ ڈومین میں NFTs کو حذف کرتا ہے اور ہٹائے گئے اکاؤنٹس کی ملکیت میں کسی بھی NFTs ؛ ان محرکات کو ہٹاتا ہے جن کے اتھارٹی ڈومین سے میل کھاتا ہے۔ واقعات: `DomainEvent::Deleted` ، نیز فی آئٹم کو حذف کرنے کے واقعات۔ غلطیاں: `FindError::Domain` اگر غائب ہے۔ کوڈ: `core/.../isi/world.rs`۔
  - غیر منظم اکاؤنٹ: اکاؤنٹ کی اجازت ، کردار ، TX-ترتیب کاؤنٹر ، اکاؤنٹ لیبل میپنگ ، اور UAID بائنڈنگز کو ہٹا دیتا ہے۔ اکاؤنٹ کی ملکیت والے اثاثوں کو حذف کرتا ہے (اور فی اثاثہ میٹا ڈیٹا) ؛ اکاؤنٹ کی ملکیت والی NFTs کو حذف کرتا ہے۔ ان محرکات کو ہٹاتا ہے جن کا اختیار وہ اکاؤنٹ ہے۔ واقعات: `AccountEvent::Deleted` ، پلس `NftEvent::Deleted` فی NFT کو ہٹا دیا گیا۔ غلطیاں: `FindError::Account` اگر غائب ہے۔ کوڈ: `core/.../isi/domain.rs`۔
  - غیر منظم اثاثہ ڈیفائنیشن: اس تعریف کے تمام اثاثوں اور ان کے فی ایسٹ میٹا ڈیٹا کو حذف کرتا ہے۔ واقعات: `AssetDefinitionEvent::Deleted` اور `AssetEvent::Deleted` فی اثاثہ۔ غلطیاں: `FindError::AssetDefinition`۔ کوڈ: `core/.../isi/domain.rs`۔
  - غیر منظم NFT: NFT کو ہٹا دیتا ہے۔ واقعات: `NftEvent::Deleted`۔ غلطیاں: `FindError::Nft`۔ کوڈ: `core/.../isi/nft.rs`۔
  - غیر منظم کردار: سب سے پہلے تمام اکاؤنٹس سے کردار کو منسوخ کرتا ہے۔ پھر کردار کو دور کرتا ہے۔ واقعات: `RoleEvent::Deleted`۔ غلطیاں: `FindError::Role`۔ کوڈ: `core/.../isi/world.rs`۔
  - غیر منظم ٹرگر: اگر موجود ہو تو ٹرگر کو ہٹا دیتا ہے۔ ڈپلیکیٹ غیر رجسٹر `Repetition(Unregister, TriggerId)`۔ واقعات: `TriggerEvent::Deleted`۔ کوڈ: `core/.../isi/triggers/mod.rs`۔

### ٹکسال / برن
اقسام: `Mint<O, D: Identifiable>` اور `Burn<O, D: Identifiable>` ، `MintBox`/`BurnBox` کے بطور باکسڈ۔- اثاثہ (عددی) ٹکسال/برن: بیلنس اور تعریف کے `total_quantity` کو ایڈجسٹ کرتا ہے۔
  - پیشگی شرائط: `Numeric` ویلیو کو `AssetDefinition.spec()` کو پورا کرنا ہوگا۔ `mintable` کے ذریعہ ٹکسال کی اجازت:
    - `Infinitely`: ہمیشہ اجازت ہے۔
    - `Once`: بالکل ایک بار اجازت ؛ پہلا ٹکسال پلٹ جاتا ہے `mintable` سے `Not` اور `AssetDefinitionEvent::MintabilityChanged` کو خارج کرتا ہے ، نیز آڈیٹیبلٹی کے لئے ایک تفصیلی `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`۔
    - `Limited(n)`: `n` اضافی ٹکسال کی کارروائیوں کی اجازت دیتا ہے۔ ہر کامیاب ٹکسال کاؤنٹر میں کمی ؛ جب یہ صفر تک پہنچ جاتا ہے تو تعریف `Not` پر پلٹ جاتی ہے اور اسی طرح `MintabilityChanged` واقعات کا اخراج کرتی ہے۔
    - `Not`: غلطی `MintabilityError::MintUnmintable`۔
  - ریاست میں تبدیلیاں: اگر ٹکسال سے محروم ہو تو اثاثہ پیدا کرتا ہے۔ اثاثوں کے اندراج کو ہٹاتا ہے اگر برن پر توازن صفر ہوجاتا ہے۔
  ۔
  - غلطیاں: `TypeError::AssetNumericSpec(Mismatch)` ، `MathError::Overflow`/`NotEnoughQuantity`۔ کوڈ: `core/.../isi/asset.rs`۔

- ٹرگر تکرار ٹکسال/برن: ٹرگر کے لئے `action.repeats` گنتی میں تبدیلی آتی ہے۔
  - پیشگی شرطیں: ٹکسال پر ، فلٹر لازمی طور پر ہونا چاہئے۔ ریاضی کو بہاؤ/انڈر فلو نہیں ہونا چاہئے۔
  - واقعات: `TriggerEvent::Extended`/`TriggerEvent::Shortened`۔
  - غلطیاں: `MathError::Overflow` پر غلط ٹکسال ؛ `FindError::Trigger` اگر غائب ہے۔ کوڈ: `core/.../isi/triggers/mod.rs`۔

### منتقلی
اقسام: `Transfer<S: Identifiable, O, D: Identifiable>` ، `TransferBox` کے بطور باکسڈ۔

- اثاثہ (عددی): ماخذ `AssetId` سے گھٹ ، منزل `AssetId` (ایک ہی تعریف ، مختلف اکاؤنٹ) میں شامل کریں۔ صفر سورس اثاثہ کو حذف کریں۔
  - پیشگی شرطیں: ماخذ اثاثہ موجود ہے۔ قدر `spec` کو مطمئن کرتی ہے۔
  - واقعات: `AssetEvent::Removed` (ماخذ) ، `AssetEvent::Added` (منزل)۔
  - غلطیاں: `FindError::Asset` ، `TypeError::AssetNumericSpec` ، `MathError::NotEnoughQuantity/Overflow`۔ کوڈ: `core/.../isi/asset.rs`۔

- ڈومین کی ملکیت: `Domain.owned_by` کو منزل مقصود میں تبدیل کرتا ہے۔
  - پیشگی شرط: دونوں اکاؤنٹس موجود ہیں۔ ڈومین موجود ہے۔
  - واقعات: `DomainEvent::OwnerChanged`۔
  - غلطیاں: `FindError::Account/Domain`۔ کوڈ: `core/.../isi/domain.rs`۔

- اثاثہ ڈیفینیشن ملکیت: `AssetDefinition.owned_by` کو منزل مقصود کے اکاؤنٹ میں تبدیل کرتا ہے۔
  - پیشگی شرط: دونوں اکاؤنٹس موجود ہیں۔ تعریف موجود ہے ؛ ماخذ کو فی الحال اس کا مالک ہونا چاہئے۔
  - واقعات: `AssetDefinitionEvent::OwnerChanged`۔
  - غلطیاں: `FindError::Account/AssetDefinition`۔ کوڈ: `core/.../isi/account.rs`۔

- NFT ملکیت: `Nft.owned_by` کو منزل مقصود کے اکاؤنٹ میں تبدیل کرتا ہے۔
  - پیشگی شرط: دونوں اکاؤنٹس موجود ہیں۔ NFT موجود ہے ؛ ماخذ کو فی الحال اس کا مالک ہونا چاہئے۔
  - واقعات: `NftEvent::OwnerChanged`۔
  - غلطیاں: `FindError::Account/Nft` ، `InvariantViolation` اگر ماخذ NFT کا مالک نہیں ہے۔ کوڈ: `core/.../isi/nft.rs`۔

### میٹا ڈیٹا: کلید - ویلیو سیٹ/ہٹا دیں
اقسام: `SetKeyValue<T>` اور `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` کے ساتھ۔ باکسڈ اینوم فراہم کی گئی۔

- سیٹ: `Metadata[key] = Json(value)` داخل کرتا ہے یا اس کی جگہ لیتا ہے۔
- ہٹائیں: کلید کو ہٹا دیتا ہے۔ اگر گمشدہ ہے تو غلطی۔
- واقعات: پرانے / نئی اقدار کے ساتھ `<Target>Event::MetadataInserted` / `MetadataRemoved`۔
- غلطیاں: `FindError::<Target>` اگر ہدف موجود نہیں ہے۔ `FindError::MetadataKey` کو ہٹانے کے لئے کلید کی گمشدگی پر۔ کوڈ: `crates/iroha_data_model/src/isi/transparent.rs` اور ایک ہدف پر عمل کنندہ IMPLES.### اجازت اور کردار: گرانٹ / کالعدم
اقسام: `Grant<O, D>` اور `Revoke<O, D>` ، `Permission`/`Role` کے لئے باکسڈ اینومس کے ساتھ/`Account` ، اور `Permission` سے/`Role` سے۔

- اکاؤنٹ میں گرانٹ کی اجازت: `Permission` شامل کرتا ہے جب تک کہ پہلے سے ہی موروثی نہ ہو۔ واقعات: `AccountEvent::PermissionAdded`۔ غلطیاں: `Repetition(Grant, Permission)` اگر نقل ہے۔ کوڈ: `core/.../isi/account.rs`۔
- اکاؤنٹ سے اجازت منسوخ کریں: اگر موجود ہو تو ہٹاتا ہے۔ واقعات: `AccountEvent::PermissionRemoved`۔ غلطیاں: `FindError::Permission` اگر غیر حاضر ہے۔ کوڈ: `core/.../isi/account.rs`۔
- اکاؤنٹ کو گرانٹ رول: `(account, role)` میپنگ داخل کرتا ہے اگر غیر حاضر ہوں۔ واقعات: `AccountEvent::RoleGranted`۔ غلطیاں: `Repetition(Grant, RoleId)`۔ کوڈ: `core/.../isi/account.rs`۔
- اکاؤنٹ سے کردار کو منسوخ کریں: اگر موجود ہو تو نقشہ سازی کو ہٹا دیتا ہے۔ واقعات: `AccountEvent::RoleRevoked`۔ غلطیاں: `FindError::Role` غیر حاضر ہوں۔ کوڈ: `core/.../isi/account.rs`۔
- کردار کو گرانٹ اجازت: اجازت کے ساتھ دوبارہ تعمیراتی کردار کو شامل کیا گیا۔ واقعات: `RoleEvent::PermissionAdded`۔ غلطیاں: `Repetition(Grant, Permission)`۔ کوڈ: `core/.../isi/world.rs`۔
- کردار سے اجازت منسوخ کریں: اس اجازت کے بغیر کردار کی تعمیر نو۔ واقعات: `RoleEvent::PermissionRemoved`۔ غلطیاں: `FindError::Permission` اگر غیر حاضر ہے۔ کوڈ: `core/.../isi/world.rs`۔

### ٹرگرز: عملدرآمد
قسم: `ExecuteTrigger { trigger: TriggerId, args: Json }`۔
- سلوک: ٹرگر سب سسٹم کے لئے `ExecuteTriggerEvent { trigger_id, authority, args }` کو شامل کرتا ہے۔ دستی عملدرآمد کی اجازت صرف بائی کال ٹرگرز (`ExecuteTrigger` فلٹر) کے لئے ہے۔ فلٹر کو مماثل ہونا چاہئے اور کال کرنے والے کو ٹرگر ایکشن اتھارٹی ہونا چاہئے یا اس اتھارٹی کے لئے `CanExecuteTrigger` ہونا چاہئے۔ جب صارف فراہم کردہ ایگزیکٹر فعال ہوتا ہے تو ، ٹرگر پر عمل درآمد رن ٹائم ایگزیکٹر کے ذریعہ کیا جاتا ہے اور ٹرانزیکشن کا ایگزیکٹر فیول بجٹ (بیس `executor.fuel` پلس اختیاری میٹا ڈیٹا `additional_fuel`) استعمال کرتا ہے۔
- غلطیاں: `FindError::Trigger` اگر رجسٹرڈ نہیں ہے۔ `InvariantViolation` اگر غیر مجاز کے ذریعہ کہا جاتا ہے۔ کوڈ: `core/.../isi/triggers/mod.rs` (اور `core/.../smartcontracts/isi/mod.rs` میں ٹیسٹ)۔

### اپ گریڈ اور لاگ ان
- `Upgrade { executor }`: فراہم کردہ `Executor` بائیکوڈ ، اپ ڈیٹس ایگزیکٹر اور اس کے ڈیٹا ماڈل کا استعمال کرتے ہوئے پھانسی دینے والے کو منتقل کرتا ہے ، `ExecutorEvent::Upgraded` کو خارج کرتا ہے۔ غلطیاں: منتقلی کی ناکامی پر `InvalidParameterError::SmartContract` کے طور پر لپیٹ کوڈ: `core/.../isi/world.rs`۔
- `Log { level, msg }`: دیئے گئے سطح کے ساتھ نوڈ لاگ خارج کرتا ہے۔ ریاست میں کوئی تبدیلی نہیں ہے۔ کوڈ: `core/.../isi/world.rs`۔

### غلطی کا ماڈل
کامن لفافہ: `InstructionExecutionError` تشخیصی غلطیوں ، استفسار کی ناکامیوں ، تبادلوں ، ہستی کے لئے مختلف حالتوں کے ساتھ نہیں ملا ، تکرار ، اجزاء ، ریاضی ، غلط پیرامیٹر ، اور ناگوار خلاف ورزی۔ گنتی اور مددگار `crates/iroha_data_model/src/isi/mod.rs` میں `pub mod error` کے تحت ہیں۔

---## لین دین اور عملدرآمد
- `Executable`: یا تو `Instructions(ConstVec<InstructionBox>)` یا `Ivm(IvmBytecode)` ؛ بائیک کوڈ بیس 64 کے طور پر سیریلائز کرتا ہے۔ کوڈ: `crates/iroha_data_model/src/transaction/executable.rs`۔
- `TransactionBuilder`/`SignedTransaction`: تعمیرات ، نشانیاں اور پیکیجز میٹا ڈیٹا کے ساتھ ایک قابل عمل ، `chain_id` ، `authority` ، `creation_time_ms` ، اختیاری `ttl_ms` ، اور I18NII کوڈ: `crates/iroha_data_model/src/transaction/`۔
- رن ٹائم کے وقت ، `iroha_core` `Execute for InstructionBox` کے ذریعے `InstructionBox` بیچز پر عملدرآمد کرتا ہے ، مناسب Norito یا کنکریٹ کی ہدایت پر گرتے ہوئے۔ کوڈ: `crates/iroha_core/src/smartcontracts/isi/mod.rs`۔
- رن ٹائم ایگزیکٹر کی توثیق کا بجٹ (صارف سے فراہم کردہ ایگزیکٹر): پیرامیٹرز سے بیس `executor.fuel` کے علاوہ اختیاری ٹرانزیکشن میٹا ڈیٹا `additional_fuel` (Torii) ، ٹرانزیکشن کے اندر ہدایت/ٹرگر کی توثیق میں مشترکہ ہے۔

---

## حملہ آور اور نوٹ (ٹیسٹ اور گارڈز سے)
- جینیسیس پروٹیکشن: `genesis` ڈومین یا اکاؤنٹس کو `genesis` ڈومین میں رجسٹر نہیں کرسکتے ہیں۔ `genesis` اکاؤنٹ رجسٹر نہیں ہوسکتا ہے۔ کوڈ/ٹیسٹ: `core/.../isi/world.rs` ، `core/.../smartcontracts/isi/mod.rs`۔
- عددی اثاثوں کو اپنے `NumericSpec` کو ٹکسال/منتقلی/جلانے پر پورا کرنا ہوگا۔ مخصوص مماثلت سے `TypeError::AssetNumericSpec` حاصل ہوتا ہے۔
- اشارے: `Once` ایک ہی ٹکسال کی اجازت دیتا ہے اور پھر `Not` پر پلٹ جاتا ہے۔ `Limited(n)` `Not` پر پلٹ جانے سے پہلے بالکل `n` ٹکسال کی اجازت دیتا ہے۔ `Infinitely` پر midting کی وجہ `MintabilityError::ForbidMintOnMintable` ، اور `Limited(0)` کی تشکیل `MintabilityError::InvalidMintabilityTokens` پر ترتیب دینے سے منع کرنے کی کوششیں۔
- میٹا ڈیٹا کی کاروائیاں کلیدی حیثیت رکھتی ہیں۔ کسی غیر موجود کلید کو ہٹانا ایک غلطی ہے۔
- ٹرگر فلٹرز غیر منقولہ ہوسکتے ہیں۔ پھر `Register<Trigger>` صرف `Exactly(1)` کو دہرانے کی اجازت دیتا ہے۔
- ٹرگر میٹا ڈیٹا کلید `__enabled` (BOOL) گیٹس پر عمل درآمد ؛ فعال ، اور معذور ٹرگرس کو ڈیفالٹس سے محروم ، ڈیٹا/وقت/بائی کال کے راستوں میں چھوڑ دیا جاتا ہے۔
- عزم: تمام ریاضی کا استعمال چیک شدہ آپریشنز ؛ انڈر/اوور فلو ریٹرن ٹائپ شدہ ریاضی کی غلطیاں ؛ زیرو بیلنس اثاثہ اندراجات (کوئی پوشیدہ ریاست نہیں) ڈراپ کرتا ہے۔

---## عملی مثالوں
- ٹکسال اور منتقلی:
  - `Mint::asset_numeric(10, asset_id)` → 10 شامل کرتا ہے اگر اسپیک/ٹکسال کے ذریعہ اجازت دی گئی ہو۔ واقعات: `AssetEvent::Added`۔
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → حرکت 5 ؛ ہٹانے/اضافے کے واقعات۔
- میٹا ڈیٹا کی تازہ کاری:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert ؛ `RemoveKeyValue::account(...)` کے ذریعے ہٹانا۔
- کردار/اجازت کا انتظام:
  - `Grant::account_role(role_id, account)` ، `Grant::role_permission(perm, role)` ، اور ان کے `Revoke` ہم منصب۔
- ٹرگر لائف سائیکل:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` فلٹر کے ذریعہ اشارے کی جانچ پڑتال کے ساتھ۔ `ExecuteTrigger::new(id).with_args(&args)` کو تشکیل شدہ اتھارٹی سے ملنا چاہئے۔
  - ٹرگرز کو میٹا ڈیٹا کلید `__enabled` کو `false` میں ترتیب دے کر غیر فعال کیا جاسکتا ہے (فعال کرنے کے لئے ڈیفالٹس غائب) ؛ `SetKeyValue::trigger` یا IVM `set_trigger_enabled` SYSCALL کے ذریعے ٹوگل کریں۔
  - ٹرگر اسٹوریج کی مرمت بوجھ پر کی جاتی ہے: ڈپلیکیٹ آئی ڈی ، مماثل IDs ، اور لاپتہ بائیک کوڈ کا حوالہ دیتے ہوئے ٹرگر گرا دیئے جاتے ہیں۔ بائیک کوڈ ریفرنس کی گنتی کو دوبارہ شامل کیا جاتا ہے۔
  - اگر کسی ٹرگر کا IVM بائیک کوڈ پھانسی کے وقت غائب ہے تو ، ٹرگر کو ہٹا دیا جاتا ہے اور اس پر عمل درآمد کو ناکامی کے نتائج کے ساتھ کوئی آپٹ نہیں سمجھا جاتا ہے۔
  - ختم ہونے والے محرکات کو فوری طور پر ہٹا دیا جاتا ہے۔ اگر پھانسی کے دوران کسی کمی سے اندراج کا سامنا کرنا پڑتا ہے تو اس کو کٹوا اور لاپتہ سمجھا جاتا ہے۔
- پیرامیٹر اپ ڈیٹ:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` `ConfigurationEvent::Changed` کو تازہ کاری اور خارج کرتا ہے۔

---

## ٹریس ایبلٹی (منتخب ذرائع)
 - ڈیٹا ماڈل کور: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`۔
 - ISI تعریفیں اور رجسٹری: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`۔
 - ISI پھانسی: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`۔
 - واقعات: `crates/iroha_data_model/src/events/**`۔
 - لین دین: `crates/iroha_data_model/src/transaction/**`۔

اگر آپ چاہتے ہیں کہ اس قیاس آرائی کو ایک مہی .ا API/طرز عمل کی میز میں وسعت دی جائے یا ہر کنکریٹ واقعہ/غلطی سے منسلک ہو تو ، لفظ کہیں اور میں اس میں توسیع کروں گا۔