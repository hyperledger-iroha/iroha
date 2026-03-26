---
lang: ur
direction: rtl
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 ڈیٹا ماڈل اور ISI — نفاذ سے ماخوذ تفصیلات

ڈیزائن کے جائزے میں مدد کے لیے یہ تصریح `iroha_data_model` اور `iroha_core` پر موجودہ نفاذ سے ریورس انجینئرڈ ہے۔ بیک ٹِکس میں راستے مستند کوڈ کی طرف اشارہ کرتے ہیں۔

## دائرہ کار
- کیننیکل اداروں (ڈومینز، اکاؤنٹس، اثاثوں، NFTs، کرداروں، اجازتوں، ساتھیوں، محرکات) اور ان کے شناخت کنندگان کی وضاحت کرتا ہے۔
- ریاست کو تبدیل کرنے کی ہدایات (ISI): اقسام، پیرامیٹرز، پیشگی شرائط، ریاست کی منتقلی، خارج ہونے والے واقعات، اور خرابی کے حالات کی وضاحت کرتا ہے۔
- پیرامیٹر مینجمنٹ، لین دین، اور ہدایات سیریلائزیشن کا خلاصہ کرتا ہے۔

ڈیٹرمنزم: تمام انسٹرکشن سیمنٹکس ہارڈ ویئر پر منحصر رویے کے بغیر خالص اسٹیٹ ٹرانزیشن ہیں۔ سیریلائزیشن Norito استعمال کرتی ہے۔ VM bytecode IVM استعمال کرتا ہے اور آن چین پر عمل درآمد سے پہلے میزبان کی طرف سے تصدیق شدہ ہے۔

---

## اداروں اور شناخت کنندگان
IDs میں `Display`/`FromStr` راؤنڈ ٹرپ کے ساتھ مستحکم سٹرنگ فارمز ہوتے ہیں۔ نام کے قواعد وائٹ اسپیس اور محفوظ `@ # $` حروف کو منع کرتے ہیں۔- `Name` — توثیق شدہ متنی شناخت کنندہ۔ قواعد: `crates/iroha_data_model/src/name.rs`۔
- `DomainId` — `name`۔ ڈومین: `{ id, logo, metadata, owned_by }`۔ بلڈرز: `NewDomain`۔ کوڈ: `crates/iroha_data_model/src/domain.rs`۔
- `AccountId` — کیننیکل ایڈریس `AccountAddress` (I105/hex) کے ذریعے تیار کیے جاتے ہیں اور Torii `AccountAddress::parse_encoded` کے ذریعے ان پٹ کو معمول بناتا ہے۔ I105 ترجیحی اکاؤنٹ فارمیٹ ہے۔ I105 فارم صرف Sora-UX کے لیے ہے۔ واقف `alias` (مسترد شدہ لیگیسی فارم) اسٹرنگ کو صرف روٹنگ عرف کے طور پر برقرار رکھا گیا ہے۔ اکاؤنٹ: `{ id, metadata }`۔ کوڈ: `crates/iroha_data_model/src/account.rs`۔- اکاؤنٹ میں داخلے کی پالیسی — ڈومینز Norito-JSON `AccountAdmissionPolicy` کو میٹا ڈیٹا کلید `iroha:account_admission_policy` کے تحت اسٹور کرکے مضمر اکاؤنٹ کی تخلیق کو کنٹرول کرتے ہیں۔ جب کلید غائب ہو تو، چین کی سطح کا کسٹم پیرامیٹر `iroha:default_account_admission_policy` ڈیفالٹ فراہم کرتا ہے۔ جب وہ بھی غائب ہو تو، ہارڈ ڈیفالٹ `ImplicitReceive` (پہلی ریلیز) ہے۔ پالیسی ٹیگز `mode` (`ExplicitOnly` یا `ImplicitReceive`) کے علاوہ اختیاری فی ٹرانزیکشن (پہلے سے طے شدہ `16`) اور فی بلاک تخلیق کیپس، ایک اختیاری Norito یا اکاؤنٹس `min_initial_amounts` فی اثاثہ کی تعریف، اور ایک اختیاری `default_role_on_create` (`AccountCreated` کے بعد عطا کیا جاتا ہے، اگر غائب ہو تو `DefaultRoleError` کے ساتھ مسترد کرتا ہے)۔ پیدائش آپٹ ان نہیں کر سکتی۔ غیر فعال/غلط پالیسیاں `InstructionExecutionError::AccountAdmission` والے نامعلوم اکاؤنٹس کے لیے رسید طرز کی ہدایات کو مسترد کرتی ہیں۔ `AccountCreated` سے پہلے مضمر اکاؤنٹس سٹیمپ میٹا ڈیٹا `iroha:created_via="implicit"`؛ پہلے سے طے شدہ رولز فالو اپ `AccountRoleGranted` کا اخراج کرتے ہیں، اور ایگزیکیوٹر کے مالک کے بنیادی اصول نئے اکاؤنٹ کو اضافی کرداروں کے بغیر اپنے اثاثے/NFTs خرچ کرنے دیتے ہیں۔ کوڈ: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`۔
- `AssetDefinitionId` — کیننیکل `unprefixed Base58 address with versioning and checksum` (UUID-v4 بائٹس)۔ تعریف: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`۔ `alias` لٹریلز `<name>#<domain>.<dataspace>` یا `<name>#<dataspace>`، `<name>` کے ساتھ اثاثہ کی تعریف کے نام کے برابر ہونا چاہیے۔ کوڈ: `crates/iroha_data_model/src/asset/definition.rs`۔

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`. Alias selectors resolve against the latest committed block creation time and stop resolving after grace even before sweep removes stale bindings.
- `AssetId`: کیننیکل انکوڈ شدہ لٹریل `<base58-asset-id>#<katakana-i105-account-id>` (میراثی متنی شکلیں پہلی ریلیز میں تعاون یافتہ نہیں ہیں)۔- `NftId` — `nft$domain`۔ NFT: `{ id, content: Metadata, owned_by }`۔ کوڈ: `crates/iroha_data_model/src/nft.rs`۔
- `RoleId` — `name`۔ کردار: بلڈر `NewRole { inner: Role, grant_to }` کے ساتھ `{ id, permissions: BTreeSet<Permission> }`۔ کوڈ: `crates/iroha_data_model/src/role.rs`۔
- `Permission` — `{ name: Ident, payload: Json }`۔ کوڈ: `crates/iroha_data_model/src/permission.rs`۔
- `PeerId`/`Peer` — ہم مرتبہ کی شناخت (عوامی کلید) اور پتہ۔ کوڈ: `crates/iroha_data_model/src/peer.rs`۔
- `TriggerId` — `name`۔ ٹرگر: `{ id, action }`۔ ایکشن: `{ executable, repeats, authority, filter, metadata }`۔ کوڈ: `crates/iroha_data_model/src/trigger/`۔
- `Metadata` — `BTreeMap<Name, Json>` چیک شدہ داخل/ہٹائیں کے ساتھ۔ کوڈ: `crates/iroha_data_model/src/metadata.rs`۔
- سبسکرپشن پیٹرن (ایپلیکیشن لیئر): منصوبے `AssetDefinition` اندراجات ہیں `subscription_plan` میٹا ڈیٹا کے ساتھ؛ سبسکرپشنز `Nft` ریکارڈز ہیں `subscription` میٹا ڈیٹا کے ساتھ؛ سبسکرپشن NFTs کا حوالہ دینے والے ٹائم ٹرگرز کے ذریعے بلنگ کی جاتی ہے۔ دیکھیں `docs/source/subscriptions_api.md` اور `crates/iroha_data_model/src/subscription.rs`۔
- **Cryptographic Primitives** (خصوصیت `sm`):
  - SM2 کے لیے `Sm2PublicKey` / `Sm2Signature` کیننیکل SEC1 پوائنٹ + فکسڈ چوڑائی `r∥s` انکوڈنگ کا عکس۔ کنسٹرکٹرز منحنی رکنیت اور امتیازی ID کے سیمنٹکس (`DEFAULT_DISTID`) کو نافذ کرتے ہیں، جب کہ توثیق خراب شکل والے یا ہائی رینج اسکیلرز کو مسترد کرتی ہے۔ کوڈ: `crates/iroha_crypto/src/sm.rs` اور `crates/iroha_data_model/src/crypto/mod.rs`۔
  - `Sm3Hash` GM/T 0004 ڈائجسٹ کو Norito-سیریلائز ایبل `[u8; 32]` نئی قسم کے طور پر ظاہر کرتا ہے جہاں مینی فیسٹ یا ٹیلی میٹری میں ہیشز ظاہر ہوتے ہیں۔ کوڈ: `crates/iroha_data_model/src/crypto/hash.rs`۔- `Sm4Key` 128-bit SM4 کیز کی نمائندگی کرتا ہے اور میزبان سیسکالز اور ڈیٹا ماڈل فکسچر کے درمیان اشتراک کیا جاتا ہے۔ کوڈ: `crates/iroha_data_model/src/crypto/symmetric.rs`۔
  یہ اقسام موجودہ Ed25519/BLS/ML-DSA پرائمیٹوز کے ساتھ بیٹھتی ہیں اور `sm` خصوصیت کے فعال ہونے کے بعد ڈیٹا ماڈل صارفین (Torii، SDKs، جینیسس ٹولنگ) کے لیے دستیاب ہوتی ہیں۔
- ڈیٹا اسپیس سے حاصل کردہ ریلیشن اسٹورز (`space_directory_manifests`، `uaid_dataspaces`، `axt_policies`، `axt_replay_ledger`، لین ریلے ایمرجنسی اوور رائیڈ رجسٹری) اور ڈیٹا اسپیس ٹارگٹ اکاؤنٹ کی اجازت (0101010101001001 میں اکاؤنٹ اسٹورز) کو `State::set_nexus(...)` پر کاٹ دیا جاتا ہے جب ڈیٹا اسپیس فعال `dataspace_catalog` سے غائب ہوجاتا ہے، رن ٹائم کیٹلاگ اپ ڈیٹس کے بعد باسی ڈیٹا اسپیس حوالہ جات کو روکتا ہے۔ لین کے دائرہ کار والے DA/relay caches (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) کو بھی کاٹ دیا جاتا ہے جب ایک لین ریٹائر ہو جاتی ہے یا کسی مختلف ڈیٹا اسپیس کو دوبارہ تفویض کی جاتی ہے لہذا ڈیٹا اسپیس میں لین-اسپیس کو مائل نہیں کیا جا سکتا۔ اسپیس ڈائرکٹری ISIs (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) بھی فعال کیٹلاگ کے خلاف `dataspace` کی توثیق کرتے ہیں اور `InvalidParameter` کے ساتھ نامعلوم IDs کو مسترد کرتے ہیں۔

اہم خصوصیات: `Identifiable`, `Registered`/`Registrable` (بلڈر پیٹرن), `HasMetadata`, `IntoKeyValue`۔ کوڈ: `crates/iroha_data_model/src/lib.rs`۔

ایونٹس: ہر ہستی میں تغیرات (تخلیق/حذف/مالک کی تبدیلی/میٹا ڈیٹا تبدیل، وغیرہ) پر خارج ہونے والے واقعات ہوتے ہیں۔ کوڈ: `crates/iroha_data_model/src/events/`۔

---## پیرامیٹرز (چین کی ترتیب)
- فیملیز: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, علاوہ `custom: BTreeMap`۔
- فرقوں کے لیے سنگل انوم: `SumeragiParameter`، `BlockParameter`، `TransactionParameter`، `SmartContractParameter`۔ جمع کرنے والا: `Parameters`۔ کوڈ: `crates/iroha_data_model/src/parameter/system.rs`۔

ترتیب کے پیرامیٹرز (ISI): `SetParameter(Parameter)` متعلقہ فیلڈ کو اپ ڈیٹ کرتا ہے اور `ConfigurationEvent::Changed` کو خارج کرتا ہے۔ کوڈ: `crates/iroha_data_model/src/isi/transparent.rs`، `crates/iroha_core/src/smartcontracts/isi/world.rs` میں ایگزیکیوٹر۔

---

## انسٹرکشن سیریلائزیشن اور رجسٹری
- بنیادی خصوصیت: `Instruction: Send + Sync + 'static` کے ساتھ `dyn_encode()`، `as_any()`، مستحکم `id()` (کنکریٹ قسم کے نام سے پہلے سے طے شدہ)۔
- `InstructionBox`: `Box<dyn Instruction>` ریپر۔ Clone/Eq/Ord `(type_id, encoded_bytes)` پر کام کرتا ہے لہذا مساوات قدر کے لحاظ سے ہے۔
- Norito serde `InstructionBox` کے لیے `(String wire_id, Vec<u8> payload)` کے طور پر سیریلائز کرتا ہے (وائر ID نہ ہونے پر `type_name` پر واپس آتا ہے)۔ ڈی سیریلائزیشن کنسٹرکٹرز کے لیے عالمی `InstructionRegistry` میپنگ شناخت کاروں کا استعمال کرتی ہے۔ ڈیفالٹ رجسٹری میں تمام بلٹ ان ISI شامل ہے۔ کوڈ: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`۔

---

## ISI: اقسام، سیمنٹکس، غلطیاں
عملدرآمد `Execute for <Instruction>` کے ذریعے `iroha_core::smartcontracts::isi` میں لاگو کیا جاتا ہے۔ ذیل میں عوامی اثرات، پیشگی شرائط، خارج ہونے والے واقعات، اور غلطیوں کی فہرست دی گئی ہے۔

### رجسٹر / غیر رجسٹر
اقسام: `Register<T: Registered>` اور `Unregister<T: Identifiable>`، `RegisterBox`/`UnregisterBox` کنکریٹ اہداف کا احاطہ کرنے والی مجموعی اقسام کے ساتھ۔- رجسٹر پیر: ورلڈ پیئر سیٹ میں داخل کرتا ہے۔
  - پیشگی شرائط: پہلے سے موجود نہیں ہونا چاہئے۔
  - واقعات: `PeerEvent::Added`۔
  - غلطیاں: `Repetition(Register, PeerId)` اگر نقل ہو؛ تلاش کرنے پر `FindError`۔ کوڈ: `core/.../isi/world.rs`۔

- رجسٹر ڈومین: `NewDomain` سے `owned_by = authority` کے ساتھ بناتا ہے۔ نامنظور: `genesis` ڈومین۔
  - پیشگی شرائط: ڈومین کی عدم موجودگی؛ `genesis` نہیں ہے۔
  - واقعات: `DomainEvent::Created`۔
  - خرابیاں: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`۔ کوڈ: `core/.../isi/world.rs`۔

- اکاؤنٹ رجسٹر کریں: `NewAccount` سے بناتا ہے، `genesis` ڈومین میں نامنظور؛ `genesis` اکاؤنٹ رجسٹر نہیں کیا جا سکتا۔
  - پیشگی شرائط: ڈومین موجود ہونا چاہیے؛ اکاؤنٹ کی عدم موجودگی؛ جینیسس ڈومین میں نہیں۔
  - واقعات: `DomainEvent::Account(AccountEvent::Created)`۔
  - خرابیاں: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`۔ کوڈ: `core/.../isi/domain.rs`۔

- رجسٹر اثاثہ کی تعریف: بلڈر سے بناتا ہے۔ `owned_by = authority` سیٹ کرتا ہے۔
  - پیشگی شرائط: تعریف عدم وجود؛ ڈومین موجود ہے؛ `name` درکار ہے، ٹرم کے بعد غیر خالی ہونا چاہیے، اور `#`/`@` پر مشتمل نہیں ہونا چاہیے۔
  - واقعات: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`۔
  - خرابیاں: `Repetition(Register, AssetDefinitionId)`۔ کوڈ: `core/.../isi/domain.rs`۔

- NFT رجسٹر کریں: بلڈر سے بناتا ہے۔ `owned_by = authority` سیٹ کرتا ہے۔
  - پیشگی شرائط: NFT کا عدم وجود؛ ڈومین موجود ہے۔
  - واقعات: `DomainEvent::Nft(NftEvent::Created)`۔
  - خرابیاں: `Repetition(Register, NftId)`۔ کوڈ: `core/.../isi/nft.rs`۔- رجسٹر رول: `NewRole { inner, grant_to }` سے بناتا ہے (اکاؤنٹ رول میپنگ کے ذریعے ریکارڈ شدہ پہلا مالک)، `inner: Role` اسٹور کرتا ہے۔
  - پیشگی شرائط: کردار کی عدم موجودگی۔
  - واقعات: `RoleEvent::Created`۔
  - خرابیاں: `Repetition(Register, RoleId)`۔ کوڈ: `core/.../isi/world.rs`۔

- رجسٹر ٹرگر: ٹرگر کو فلٹر قسم کے ذریعہ سیٹ کردہ مناسب ٹرگر میں اسٹور کرتا ہے۔
  - پیشگی شرائط: اگر فلٹر قابل نہیں ہے، تو `action.repeats` `Exactly(1)` ہونا چاہیے (ورنہ `MathError::Overflow`)۔ ڈپلیکیٹ آئی ڈیز ممنوع ہیں۔
  - واقعات: `TriggerEvent::Created(TriggerId)`۔
  - غلطیاں: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` تبدیلی/توثیق کی ناکامیوں پر۔ کوڈ: `core/.../isi/triggers/mod.rs`۔- غیر رجسٹر پیر/ڈومین/اکاؤنٹ/اثاثہ تعریف/NFT/کردار/ٹرگر: ہدف کو ہٹاتا ہے۔ حذف کرنے کے واقعات کو خارج کرتا ہے۔ اضافی جھرن کو ہٹانا:- غیر رجسٹرڈ ڈومین: ڈومین ہستی کے علاوہ اس کے سلیکٹر/توثیق کی پالیسی کی حالت کو ہٹاتا ہے۔ ڈومین میں اثاثہ کی تعریفیں حذف کرتا ہے (اور خفیہ `zk_assets` سائیڈ اسٹیٹ جو ان تعریفوں سے کلید ہوتا ہے)، ان تعریفوں کے اثاثے (اور فی اثاثہ میٹا ڈیٹا)، ڈومین میں NFTs، اور ڈومین کے دائرہ کار والے اکاؤنٹ لیبل/عرف پروجیکشنز کو حذف کرتا ہے۔ یہ ہٹائے گئے ڈومین سے زندہ بچ جانے والے اکاؤنٹس کو بھی غیر لنک کرتا ہے اور اکاؤنٹ-/رول اسکوپڈ اجازت کے اندراجات کاٹتا ہے جو ہٹائے گئے ڈومین یا اس کے ساتھ حذف کیے گئے وسائل کا حوالہ دیتے ہیں (ڈومین کی اجازتیں، ہٹائی گئی تعریفوں کے لیے اثاثہ کی تعریف/اثاثہ کی اجازتیں، اور ہٹائی گئی NFT IDs کے لیے NFT اجازتیں)۔ ڈومین کو ہٹانے سے عالمی `AccountId`، اس کی tx-sequence/UAID حالت، غیر ملکی اثاثہ یا NFT ملکیت، ٹرگر اتھارٹی، یا دیگر بیرونی آڈٹ/config حوالہ جات جو بقایا اکاؤنٹ کی طرف اشارہ کرتے ہیں کو حذف نہیں کرتا ہے۔ گارڈ ریلز: مسترد کرتا ہے جب ڈومین میں کسی بھی اثاثہ کی تعریف کا حوالہ ریپو-ایگریمنٹ، سیٹلمنٹ-لیجر، پبلک لین ریوارڈ/کلیم، آف لائن الاؤنس/ٹرانسفر، سیٹلمنٹ ریپو ڈیفالٹس (`settlement.repo.eligible_collateral`، `settlement.repo.collateral_substitution_matrix`)، گورنمنٹ کے ذریعے دیا جاتا ہے۔ ووٹنگ/شہریت/پارلیمنٹ-اہلیت/وائرل-انعام کے اثاثہ کی تعریف کے حوالہ جات، اوریکل-اکنامکس کنفیگرڈ انعام/سلیش/تنازعہ-بانڈ اثاثہ کی تعریف کے حوالہ جات، یا Nexus فیس/اسٹیکنگ اثاثہ کی تعریف کے حوالہ جات (Nexus `nexus.staking.stake_asset_id`)۔ ایونٹس: `DomainEvent::Deleted`، علاوہ فی آئٹم ڈیلیٹہٹائے گئے ڈومین کے دائرہ کار والے وسائل کے واقعات پر۔ غلطیاں: `FindError::Domain` اگر غائب ہو؛ `InvariantViolation` برقرار رکھے ہوئے اثاثہ کی تعریف کے حوالہ تنازعات پر۔ کوڈ: `core/.../isi/world.rs`۔- اکاؤنٹ کو غیر رجسٹر کریں: اکاؤنٹ کی اجازت، کردار، tx-sequence کاؤنٹر، اکاؤنٹ لیبل میپنگ، اور UAID بائنڈنگز کو ہٹاتا ہے۔ اکاؤنٹ کی ملکیت والے اثاثوں کو حذف کرتا ہے (اور فی اثاثہ میٹا ڈیٹا)؛ اکاؤنٹ کی ملکیت والے NFTs کو حذف کرتا ہے۔ ان محرکات کو ہٹاتا ہے جس کا اختیار وہ اکاؤنٹ ہے۔ ہٹائے گئے اکاؤنٹ کا حوالہ دینے والے اکاؤنٹ-/ کردار کے دائرہ کار کی اجازت کے اندراجات کو کاٹتا ہے، ہٹائے گئے NFT IDs کے لیے اکاؤنٹ-/ کردار کے دائرہ کار والے NFT- ہدف کی اجازتیں، اور ہٹائے گئے محرکات کے لیے اکاؤنٹ-/ رول-اسکوپڈ ٹرگر-ٹارگٹ پرمیشنز۔ گارڈ ریلز: مسترد کرتا ہے اگر اکاؤنٹ اب بھی ڈومین کا مالک ہے، اثاثہ کی تعریف، SoraFS فراہم کنندہ بائنڈنگ، فعال شہریت کا ریکارڈ، پبلک لین اسٹیکنگ/ریوارڈ اسٹیٹ (بشمول ریوارڈ کلیم کیز جہاں اکاؤنٹ دعویدار یا ریوارڈ اثاثہ کے مالک کے طور پر ظاہر ہوتا ہے)، فعال اوریکل اسٹیٹ (بشمول فیڈ فراہم کرنے والے) فراہم کنندہ کے ریکارڈز، یا اوریکل-اکنامکس کنفیگرڈ ریوارڈ/سلیش اکاؤنٹ ریفرینسز)، فعال Nexus فیس/اسٹیکنگ اکاؤنٹ ریفرینسز (`nexus.fees.fee_sink_account_id`، `nexus.staking.stake_escrow_account_id`، `nexus.staking.slash_sink_account_id`؛ اکاؤنٹ کے بغیر شناخت شدہ ڈومین کے بطور دوبارہ غلط لٹریلز پر ناکام بند، فعال ریپو-ایگریمنٹ اسٹیٹ، فعال سیٹلمنٹ لیجر اسٹیٹ، فعال آف لائن الاؤنس/منتقلی یا آف لائن فیصلے کی منسوخی کی حالت، فعال اثاثہ کی تعریفوں کے لیے فعال آف لائن ایسکرو اکاؤنٹ کنفیگریشن ریفرینسز (`settlement.offline.escrow_accounts`)، ایکٹو گورننس اسٹیٹ/ایپ پروپوز اسٹیٹals/locks/slashes/council/parliament rosters، تجویز پارلیمنٹ کے اسنیپ شاٹس، رن ٹائم اپ گریڈ پروپوزر ریکارڈز، گورننس کنفیگرڈ ایسکرو/سلیش ریسیور/وائرل پول اکاؤنٹ حوالہ جات، گورننس SoraFS ٹیلی میٹری جمع کنندہ I18NT `gov.sorafs_telemetry.per_provider_submitters`، یا گورننس سے ترتیب شدہ SoraFS فراہم کنندہ کے مالک کے حوالہ جات بذریعہ `gov.sorafs_provider_owners`)، ترتیب شدہ مواد شائع کرنے کی اجازت دینے والی فہرست اکاؤنٹ کے حوالہ جات (`content.publish_allow_accounts`)، فعال سماجی، فعال مواد بھیجنے والا، فعال مواد بھیجنے والا ریاستی پنشن مالک ریاست، ایکٹو لین-ریلے ایمرجنسی ویلیڈیٹر اوور رائیڈ اسٹیٹ، یا ایکٹو SoraFS پن-رجسٹری جاری کنندہ/بائنڈر ریکارڈز (پن مینی فیسٹس، مینی فیسٹ عرف، نقل کے احکامات)۔ ایونٹس: `AccountEvent::Deleted`، علاوہ `NftEvent::Deleted` فی ہٹا دیا گیا NFT۔ غلطیاں: `FindError::Account` اگر غائب ہو؛ ملکیت یتیموں پر `InvariantViolation`۔ کوڈ: `core/.../isi/domain.rs`۔- اثاثہ کی ڈیفینیشن کا اندراج ختم کریں: اس تعریف کے تمام اثاثوں اور ان کے فی اثاثہ میٹا ڈیٹا کو حذف کرتا ہے، اور اس تعریف کے مطابق خفیہ `zk_assets` کو ہٹا دیتا ہے۔ مماثل `settlement.offline.escrow_accounts` اندراج اور اکاؤنٹ-/کردار کے دائرہ کار کی اجازت کے اندراجات کو بھی کاٹتا ہے جو ہٹائے گئے اثاثہ کی تعریف یا اس کے اثاثوں کی مثالوں کا حوالہ دیتے ہیں۔ گارڈ ریلز: مسترد کرتا ہے جب تعریف کا حوالہ ابھی بھی ریپو-ایگریمنٹ، سیٹلمنٹ-لیجر، پبلک لین ریوارڈ/کلیم، آف لائن الاؤنس/ٹرانسفر اسٹیٹ، سیٹلمنٹ ریپو ڈیفالٹس (`settlement.repo.eligible_collateral`، `settlement.repo.collateral_substitution_matrix`)، گورننس کنفیگرڈ ووٹنگ/شہریت/پارلیمنٹ-اہلیت/وائرل-انعام کے اثاثہ کی تعریف کے حوالہ جات، اوریکل-اکنامکس کنفیگرڈ ریوارڈ/سلیش/تنازعہ-بانڈ کے اثاثہ کی تعریف کے حوالہ جات، یا Nexus فیس/اسٹیکنگ اثاثہ کی تعریف کے حوالہ جات (Nexus `nexus.staking.stake_asset_id`)۔ ایونٹس: `AssetDefinitionEvent::Deleted` اور `AssetEvent::Deleted` فی اثاثہ۔ خرابیاں: `FindError::AssetDefinition`، `InvariantViolation` حوالہ کے تنازعات پر۔ کوڈ: `core/.../isi/domain.rs`۔
  - NFT کو غیر رجسٹر کریں: NFT کو ہٹاتا ہے اور اکاؤنٹ-/کردار کے دائرہ کار کی اجازت کے اندراجات کاٹتا ہے جو ہٹائے گئے NFT کا حوالہ دیتے ہیں۔ واقعات: `NftEvent::Deleted`۔ خرابیاں: `FindError::Nft`۔ کوڈ: `core/.../isi/nft.rs`۔
  - غیر رجسٹر رول: پہلے تمام اکاؤنٹس سے کردار کو منسوخ کرتا ہے۔ پھر کردار کو ہٹاتا ہے. واقعات: `RoleEvent::Deleted`۔ خرابیاں: `FindError::Role`۔ کوڈ: `core/.../isi/world.rs`۔- غیر رجسٹر ٹرگر: موجود ہونے پر ٹرگر کو ہٹاتا ہے اور اکاؤنٹ-/رول اسکوپڈ اجازت کے اندراجات کو کاٹتا ہے جو ہٹائے گئے ٹرگر کا حوالہ دیتے ہیں۔ ڈپلیکیٹ غیر رجسٹر کرنے سے `Repetition(Unregister, TriggerId)` حاصل ہوتا ہے۔ واقعات: `TriggerEvent::Deleted`۔ کوڈ: `core/.../isi/triggers/mod.rs`۔

### پودینہ / جلنا
اقسام: `Mint<O, D: Identifiable>` اور `Burn<O, D: Identifiable>`، `MintBox`/`BurnBox` کے بطور باکسڈ۔

- اثاثہ (عددی) ٹکسال/برن: بیلنس اور تعریف کے `total_quantity` کو ایڈجسٹ کرتا ہے۔
  - پیشگی شرائط: `Numeric` قدر `AssetDefinition.spec()` کو پورا کرے؛ ٹکسال `mintable` کی طرف سے اجازت دی گئی:
    - `Infinitely`: ہمیشہ اجازت ہے۔
    - `Once`: بالکل ایک بار اجازت دی گئی۔ پہلا ٹکسال `mintable` سے `Not` پر پلٹتا ہے اور `AssetDefinitionEvent::MintabilityChanged` کا اخراج کرتا ہے، نیز آڈیٹیبلٹی کے لیے ایک تفصیلی `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`۔
    - `Limited(n)`: `n` اضافی ٹکسال آپریشن کی اجازت دیتا ہے۔ ہر کامیاب ٹکسال کاؤنٹر کو کم کرتا ہے۔ جب یہ صفر تک پہنچ جاتا ہے تو تعریف `Not` پر پلٹ جاتی ہے اور اوپر والے `MintabilityChanged` واقعات کو خارج کرتی ہے۔
    - `Not`: غلطی `MintabilityError::MintUnmintable`۔
  - ریاست میں تبدیلیاں: ٹکسال پر غائب ہونے پر اثاثہ بناتا ہے۔ اگر بیلنس جلنے پر صفر ہو جائے تو اثاثہ کے اندراج کو ہٹا دیتا ہے۔
  - واقعات: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (جب `Once` یا `Limited(n)` اپنا الاؤنس ختم کر دیتا ہے)۔
  - خرابیاں: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`۔ کوڈ: `core/.../isi/asset.rs`۔- ٹرگر کی تکرار ٹکسال/برن: ٹرگر کے لیے `action.repeats` شمار کو تبدیل کرتا ہے۔
  - پیشگی شرائط: پودینہ پر، فلٹر ٹکسال کے قابل ہونا چاہیے؛ ریاضی کو اوور فلو/زیر بہاؤ نہیں ہونا چاہیے۔
  - واقعات: `TriggerEvent::Extended`/`TriggerEvent::Shortened`۔
  - غلطیاں: غلط ٹکسال پر `MathError::Overflow`؛ اگر غائب ہو تو `FindError::Trigger`۔ کوڈ: `core/.../isi/triggers/mod.rs`۔

### منتقلی
اقسام: `Transfer<S: Identifiable, O, D: Identifiable>`، `TransferBox` کے طور پر باکسڈ۔

- اثاثہ (عددی): ماخذ `AssetId` سے منہا کریں، منزل `AssetId` میں شامل کریں (ایک ہی تعریف، مختلف اکاؤنٹ)۔ صفر شدہ ماخذ اثاثہ کو حذف کریں۔
  - پیشگی شرائط: ذریعہ اثاثہ موجود ہے؛ قدر `spec` کو مطمئن کرتی ہے۔
  - واقعات: `AssetEvent::Removed` (ماخذ)، `AssetEvent::Added` (منزل)۔
  - خرابیاں: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`۔ کوڈ: `core/.../isi/asset.rs`۔

- ڈومین کی ملکیت: `Domain.owned_by` کو منزل کے اکاؤنٹ میں تبدیل کرتا ہے۔
  - پیشگی شرائط: دونوں اکاؤنٹس موجود ہیں۔ ڈومین موجود ہے۔
  - واقعات: `DomainEvent::OwnerChanged`۔
  - خرابیاں: `FindError::Account/Domain`۔ کوڈ: `core/.../isi/domain.rs`۔

- AssetDefinition کی ملکیت: `AssetDefinition.owned_by` کو منزل کے اکاؤنٹ میں تبدیل کرتا ہے۔
  - پیشگی شرائط: دونوں اکاؤنٹس موجود ہیں۔ تعریف موجود ہے؛ ذریعہ فی الحال اس کا مالک ہونا چاہیے؛ اتھارٹی کو سورس اکاؤنٹ، سورس-ڈومین کا مالک، یا اثاثہ کی تعریف-ڈومین کا مالک ہونا چاہیے۔
  - واقعات: `AssetDefinitionEvent::OwnerChanged`۔
  - خرابیاں: `FindError::Account/AssetDefinition`۔ کوڈ: `core/.../isi/account.rs`۔- NFT ملکیت: `Nft.owned_by` کو منزل کے اکاؤنٹ میں تبدیل کرتا ہے۔
  - پیشگی شرائط: دونوں اکاؤنٹس موجود ہیں۔ NFT موجود ہے؛ ذریعہ فی الحال اس کا مالک ہونا چاہیے؛ اتھارٹی کا سورس اکاؤنٹ، سورس ڈومین کا مالک، NFT-ڈومین کا مالک ہونا چاہیے، یا اس NFT کے لیے `CanTransferNft` ہولڈ ہونا چاہیے۔
  - واقعات: `NftEvent::OwnerChanged`۔
  - خرابیاں: `FindError::Account/Nft`, `InvariantViolation` اگر ذریعہ NFT کا مالک نہیں ہے۔ کوڈ: `core/.../isi/nft.rs`۔

### میٹا ڈیٹا: کلید کی قدر سیٹ/ہٹائیں
اقسام: `SetKeyValue<T>` اور `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` کے ساتھ۔ باکسڈ اینوم فراہم کیے گئے ہیں۔

- سیٹ: داخل کرتا ہے یا `Metadata[key] = Json(value)` کو تبدیل کرتا ہے۔
- ہٹائیں: کلید کو ہٹاتا ہے؛ غلطی اگر غائب ہو.
- ایونٹس: `<Target>Event::MetadataInserted` / `MetadataRemoved` پرانی/نئی اقدار کے ساتھ۔
- خرابیاں: `FindError::<Target>` اگر ہدف موجود نہیں ہے۔ ہٹانے کے لیے غائب کلید پر `FindError::MetadataKey`۔ کوڈ: `crates/iroha_data_model/src/isi/transparent.rs` اور executor impls فی ہدف۔

### اجازتیں اور کردار: گرانٹ/منسوخ
اقسام: `Grant<O, D>` اور `Revoke<O, D>`، `Permission`/`Role` کے لیے/سے `Account`، اور `Account` کے لیے باکسڈ انوم کے ساتھ `Role`۔- اکاؤنٹ کو اجازت دیں: `Permission` شامل کرتا ہے جب تک کہ پہلے سے موروثی نہ ہو۔ واقعات: `AccountEvent::PermissionAdded`۔ خرابیاں: `Repetition(Grant, Permission)` اگر ڈپلیکیٹ ہو۔ کوڈ: `core/.../isi/account.rs`۔
- اکاؤنٹ سے اجازت منسوخ کریں: اگر موجود ہو تو ہٹا دیتا ہے۔ واقعات: `AccountEvent::PermissionRemoved`۔ خرابیاں: اگر غیر حاضر ہوں تو `FindError::Permission`۔ کوڈ: `core/.../isi/account.rs`۔
- اکاؤنٹ کو رول دیں: غیر حاضر ہونے پر `(account, role)` میپنگ داخل کرتا ہے۔ واقعات: `AccountEvent::RoleGranted`۔ خرابیاں: `Repetition(Grant, RoleId)`۔ کوڈ: `core/.../isi/account.rs`۔
- اکاؤنٹ سے رول منسوخ کریں: اگر موجود ہو تو میپنگ کو ہٹا دیتا ہے۔ واقعات: `AccountEvent::RoleRevoked`۔ خرابیاں: اگر غیر حاضر ہوں تو `FindError::Role`۔ کوڈ: `core/.../isi/account.rs`۔
- کردار کی اجازت دیں: اجازت کے ساتھ کردار کو دوبارہ بناتا ہے۔ واقعات: `RoleEvent::PermissionAdded`۔ خرابیاں: `Repetition(Grant, Permission)`۔ کوڈ: `core/.../isi/world.rs`۔
- کردار سے اجازت منسوخ کریں: اس اجازت کے بغیر کردار کو دوبارہ بناتا ہے۔ واقعات: `RoleEvent::PermissionRemoved`۔ خرابیاں: اگر غیر حاضر ہوں تو `FindError::Permission`۔ کوڈ: `core/.../isi/world.rs`۔### محرکات: عمل کریں۔
قسم: `ExecuteTrigger { trigger: TriggerId, args: Json }`۔
- برتاؤ: ٹرگر سب سسٹم کے لیے `ExecuteTriggerEvent { trigger_id, authority, args }` کی قطار لگاتا ہے۔ دستی عمل درآمد کی اجازت صرف بائے کال ٹرگرز (`ExecuteTrigger` فلٹر) کے لیے ہے۔ فلٹر کا مماثل ہونا چاہیے اور کال کرنے والا ٹرگر ایکشن اتھارٹی ہونا چاہیے یا اس اتھارٹی کے لیے `CanExecuteTrigger` ہولڈ ہونا چاہیے۔ جب صارف کی طرف سے فراہم کردہ ایگزیکیوٹر فعال ہوتا ہے، تو ٹرگر ایگزیکیوشن کو رن ٹائم ایگزیکیوٹر کے ذریعے توثیق کیا جاتا ہے اور ٹرانزیکشن کے ایگزیکیوٹر کے فیول بجٹ کو استعمال کرتا ہے (بیس `executor.fuel` کے علاوہ اختیاری میٹا ڈیٹا `additional_fuel`)۔
- غلطیاں: `FindError::Trigger` اگر رجسٹرڈ نہیں ہے۔ `InvariantViolation` اگر غیر اتھارٹی کے ذریعہ بلایا جائے۔ کوڈ: `core/.../isi/triggers/mod.rs` (اور `core/.../smartcontracts/isi/mod.rs` میں ٹیسٹ)۔

### اپ گریڈ کریں اور لاگ ان کریں۔
- `Upgrade { executor }`: فراہم کردہ `Executor` بائٹ کوڈ کا استعمال کرتے ہوئے ایگزیکیوٹر کو منتقل کرتا ہے، ایگزیکیوٹر اور اس کے ڈیٹا ماڈل کو اپ ڈیٹ کرتا ہے، `ExecutorEvent::Upgraded` کا اخراج کرتا ہے۔ خرابیاں: منتقلی کی ناکامی پر `InvalidParameterError::SmartContract` کے طور پر لپیٹ دی گئی۔ کوڈ: `core/.../isi/world.rs`۔
- `Log { level, msg }`: دی گئی سطح کے ساتھ نوڈ لاگ کو خارج کرتا ہے۔ ریاست میں کوئی تبدیلی نہیں. کوڈ: `core/.../isi/world.rs`۔

### ایرر ماڈل
عام لفافہ: `InstructionExecutionError` تشخیص کی غلطیوں، استفسار میں ناکامی، تبادلوں، ہستی کا نہ ملا، تکرار، مائنٹ ایبلٹی، ریاضی، غلط پیرامیٹر، اور غیر متغیر خلاف ورزی کے متغیرات کے ساتھ۔ گنتی اور مددگار `crates/iroha_data_model/src/isi/mod.rs` میں `pub mod error` کے تحت ہیں۔

---## لین دین اور عمل درآمد
- `Executable`: یا تو `Instructions(ConstVec<InstructionBox>)` یا `Ivm(IvmBytecode)`؛ بائیک کوڈ بیس 64 کے طور پر سیریلائز کرتا ہے۔ کوڈ: `crates/iroha_data_model/src/transaction/executable.rs`۔
- `TransactionBuilder`/`SignedTransaction`: میٹا ڈیٹا، `chain_id`، `authority`، `creation_time_ms`، اختیاری `creation_time_ms`، `creation_time_ms`، اختیاری Norito کے ساتھ تعمیرات، نشانیاں اور پیکجز `nonce`۔ کوڈ: `crates/iroha_data_model/src/transaction/`۔
- رن ٹائم پر، `iroha_core` `InstructionBox` بیچز کو `Execute for InstructionBox` کے ذریعے انجام دیتا ہے، مناسب `*Box` یا ٹھوس ہدایات پر نیچے کاسٹ کرتا ہے۔ کوڈ: `crates/iroha_core/src/smartcontracts/isi/mod.rs`۔
- رن ٹائم ایگزیکیوٹر کی توثیق کا بجٹ (صارف کی طرف سے فراہم کردہ ایگزیکیوٹر): پیرامیٹرز سے بنیادی `executor.fuel` اور اختیاری ٹرانزیکشن میٹا ڈیٹا `additional_fuel` (`u64`)، تمام ہدایات/ٹرگر کی توثیق کے اندر اشتراک کردہ۔

---## انویرینٹس اور نوٹس (ٹیسٹ اور گارڈز سے)
- جینیسس پروٹیکشنز: `genesis` ڈومین یا اکاؤنٹس کو `genesis` ڈومین میں رجسٹر نہیں کر سکتا۔ `genesis` اکاؤنٹ رجسٹر نہیں کیا جا سکتا۔ کوڈ/ٹیسٹ: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`۔
- عددی اثاثوں کو منٹ/ٹرانسفر/برن پر ان کے `NumericSpec` کو پورا کرنا چاہیے۔ spec کی مماثلت `TypeError::AssetNumericSpec` حاصل کرتی ہے۔
- Mintability: `Once` ایک ٹکسال کی اجازت دیتا ہے اور پھر `Not` پر پلٹ جاتا ہے۔ `Limited(n)` `Not` پر پلٹنے سے پہلے بالکل `n` منٹس کی اجازت دیتا ہے۔ `Infinitely` پر minting کو منع کرنے کی کوششیں `MintabilityError::ForbidMintOnMintable` کا سبب بنتی ہیں، اور `Limited(0)` کو ترتیب دینے سے `MintabilityError::InvalidMintabilityTokens` حاصل ہوتا ہے۔
- میٹا ڈیٹا آپریشنز کلیدی عین مطابق ہیں؛ غیر موجود کلید کو ہٹانا ایک غلطی ہے۔
- ٹرگر فلٹرز غیر منقطع ہو سکتے ہیں۔ پھر `Register<Trigger>` صرف `Exactly(1)` کو دہرانے کی اجازت دیتا ہے۔
- ٹرگر میٹا ڈیٹا کلید `__enabled` (بول) گیٹس پر عمل درآمد؛ ڈیفالٹس کو فعال کرنے کے لیے غائب ہیں، اور غیر فعال محرکات کو ڈیٹا/وقت/بائی کال کے راستوں میں چھوڑ دیا جاتا ہے۔
- ڈیٹرمنزم: تمام ریاضی چیک شدہ آپریشنز کا استعمال کرتا ہے؛ انڈر/اوور فلو ٹائپ شدہ ریاضی کی غلطیاں واپس کرتا ہے۔ صفر بیلنس ڈراپ اثاثہ اندراجات (کوئی پوشیدہ حالت نہیں)۔

---## عملی مثالیں۔
- ٹکسال اور منتقلی:
  - `Mint::asset_numeric(10, asset_id)` → 10 کا اضافہ کرتا ہے اگر قیاس/منٹیبلٹی کی طرف سے اجازت ہو؛ واقعات: `AssetEvent::Added`۔
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → چالیں 5؛ ہٹانے/اضافی کرنے کے واقعات۔
- میٹا ڈیٹا اپ ڈیٹس:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; `RemoveKeyValue::account(...)` کے ذریعے ہٹانا۔
- کردار/اجازت کا انتظام:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)`، اور ان کے `Revoke` ہم منصب۔
- ٹرگر لائف سائیکل:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` mintability کی جانچ کے ساتھ فلٹر کے ذریعے مضمر ہے۔ `ExecuteTrigger::new(id).with_args(&args)` کو ترتیب شدہ اتھارٹی سے مماثل ہونا چاہیے۔
  - ٹرگرز کو میٹا ڈیٹا کلید `__enabled` سے `false` سیٹ کر کے غیر فعال کیا جا سکتا ہے (فعال کرنے کے لیے ڈیفالٹس غائب ہیں)؛ `SetKeyValue::trigger` یا IVM `set_trigger_enabled` syscall کے ذریعے ٹوگل کریں۔
  - لوڈ ہونے پر ٹرگر اسٹوریج کی مرمت کی جاتی ہے: ڈپلیکیٹ آئی ڈیز، مماثل آئی ڈیز، اور گمشدہ بائی کوڈ کا حوالہ دینے والے محرکات کو چھوڑ دیا جاتا ہے۔ بائیک کوڈ حوالہ شماروں کی دوبارہ گنتی کی جاتی ہے۔
  - اگر عمل درآمد کے وقت ایک ٹرگر کا IVM بائیک کوڈ غائب ہے، تو ٹرگر کو ہٹا دیا جاتا ہے اور ایگزیکیوشن کو ناکامی کے نتیجے کے ساتھ ایک غیر آپشن سمجھا جاتا ہے۔
  - ختم ہونے والے محرکات کو فوری طور پر ہٹا دیا جاتا ہے۔ اگر عمل درآمد کے دوران ایک ختم شدہ اندراج کا سامنا کرنا پڑتا ہے تو اسے کاٹ دیا جاتا ہے اور اسے غائب سمجھا جاتا ہے۔
- پیرامیٹر اپ ڈیٹ:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` اپ ڈیٹ کرتا ہے اور `ConfigurationEvent::Changed` کو خارج کرتا ہے۔CLI / Torii asset-definition id + عرف مثالیں:
- کینونیکل ایڈ + واضح نام + لمبا عرف کے ساتھ رجسٹر کریں:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- کینونیکل ایڈ + واضح نام + مختصر عرف کے ساتھ رجسٹر کریں:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- عرف + اکاؤنٹ کے اجزاء کے لحاظ سے ٹکسال:
  - `iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- کیننیکل امداد کے عرف کو حل کریں:
  - JSON `{ "alias": "pkr#ubl.sbp" }` کے ساتھ `POST /v1/assets/aliases/resolve`

نقل مکانی نوٹ:
- `name#domain` ٹیکسٹول اثاثہ کی تعریف IDs پہلی ریلیز میں جان بوجھ کر غیر تعاون یافتہ ہیں۔
- ٹکسال/برن/ٹرانسفر باؤنڈری پر اثاثہ IDs کیننیکل `<base58-asset-id>#<katakana-i105-account-id>` رہیں۔ `iroha tools encode asset-id` `--definition <base58-asset-definition-id>` یا `--alias ...` پلس `--account` کے ساتھ استعمال کریں۔

---

## ٹریس ایبلٹی (منتخب ذرائع)
 - ڈیٹا ماڈل کور: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`۔
 - ISI کی تعریفیں اور رجسٹری: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`۔
 - ISI عملدرآمد: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`۔
 - واقعات: `crates/iroha_data_model/src/events/**`۔
 - لین دین: `crates/iroha_data_model/src/transaction/**`۔

اگر آپ چاہتے ہیں کہ اس قیاس کو پیش کردہ API/رویے کی میز میں بڑھایا جائے یا ہر ٹھوس واقعہ/خرابی سے کراس لنک کیا جائے، تو لفظ بولیں اور میں اسے بڑھا دوں گا۔