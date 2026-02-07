---
lang: ar
direction: rtl
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:33:51.650362+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 نموذج البيانات وISI — المواصفات المشتقة من التنفيذ

تم إجراء هندسة عكسية لهذه المواصفات من التنفيذ الحالي عبر `iroha_data_model` و`iroha_core` للمساعدة في مراجعة التصميم. تشير المسارات الموجودة في العلامات الخلفية إلى الكود الرسمي.

## النطاق
- يحدد الكيانات الأساسية (المجالات والحسابات والأصول وNFTs والأدوار والأذونات والأقران والمشغلات) ومعرفاتها.
- يصف تعليمات تغيير الحالة (ISI): الأنواع، والمعلمات، والشروط المسبقة، وانتقالات الحالة، والأحداث المنبعثة، وشروط الخطأ.
- يلخص إدارة المعلمات والمعاملات وتسلسل التعليمات.

الحتمية: جميع دلالات التعليمات هي انتقالات حالة خالصة دون سلوك يعتمد على الأجهزة. يستخدم التسلسل Norito؛ يستخدم VM bytecode IVM ويتم التحقق من صحته من جانب المضيف قبل التنفيذ على السلسلة.

---

## الكيانات والمعرفات
تحتوي المعرفات على نماذج سلسلة مستقرة مع `Display`/`FromStr` ذهابًا وإيابًا. تمنع قواعد الاسم المسافات البيضاء وأحرف `@ # $` المحجوزة.- `Name` - معرف نصي تم التحقق منه. القواعد: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. المجال: `{ id, logo, metadata, owned_by }`. الإنشاءات: `NewDomain`. الكود: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` - يتم إنتاج العناوين الأساسية عبر `AccountAddress` (IH58 / `sora…` مضغوط / سداسي عشري) ويقوم Torii بتسوية المدخلات من خلال `AccountAddress::parse_any`. IH58 هو تنسيق الحساب المفضل؛ يعد نموذج `sora…` هو ثاني أفضل نموذج لـ Sora-only UX. يتم الاحتفاظ بسلسلة `alias@domain` المألوفة كاسم مستعار للتوجيه فقط. الحساب: `{ id, metadata }`. الكود: `crates/iroha_data_model/src/account.rs`.
- سياسة قبول الحساب — تتحكم النطاقات في إنشاء الحساب الضمني عن طريق تخزين Norito-JSON `AccountAdmissionPolicy` ضمن مفتاح البيانات الوصفية `iroha:account_admission_policy`. عند غياب المفتاح، توفر المعلمة المخصصة على مستوى السلسلة `iroha:default_account_admission_policy` الإعداد الافتراضي؛ عندما يكون ذلك غائبًا أيضًا، يكون الإعداد الافتراضي الثابت هو `ImplicitReceive` (الإصدار الأول). علامات السياسة `mode` (`ExplicitOnly` أو `ImplicitReceive`) بالإضافة إلى اختياري لكل معاملة (الافتراضي `16`) والحد الأقصى لإنشاء كل كتلة، و`implicit_creation_fee` اختياري (حساب الحرق أو الغرق)، و`min_initial_amounts` لكل تعريف أصل، و `default_role_on_create` اختياري (يتم منحه بعد `AccountCreated`، ويتم رفضه مع `DefaultRoleError` إذا كان مفقودًا). لا يمكن لـ Genesis الاشتراك؛ ترفض السياسات المعطلة/غير الصالحة تعليمات نمط الاستلام للحسابات غير المعروفة باستخدام `InstructionExecutionError::AccountAdmission`. تقوم الحسابات الضمنية بختم بيانات التعريف `iroha:created_via="implicit"` قبل `AccountCreated`؛ تنبعث الأدوار الافتراضية من `AccountRoleGranted` للمتابعة، وتسمح قواعد خط الأساس لمالك المنفذ للحساب الجديد بإنفاق أصوله/NFTs الخاصة به دون أدوار إضافية. الرمز: `crates/iroha_data_model/src/account/admission.rs`، `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. التعريف: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. الكود: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId` — `asset#domain#account` أو `asset##account` إذا كانت المجالات متطابقة، حيث `account` هي سلسلة `AccountId` الأساسية (يفضل IH58). الأصول: `{ id, value: Numeric }`. الرمز: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`.
- `NftId` — `nft$domain`. إن إف تي: `{ id, content: Metadata, owned_by }`. الكود: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. الدور: `{ id, permissions: BTreeSet<Permission> }` مع المنشئ `NewRole { inner: Role, grant_to }`. الكود: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. الكود: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` - هوية النظير (المفتاح العام) والعنوان. الكود: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. المشغل: `{ id, action }`. الإجراء: `{ executable, repeats, authority, filter, metadata }`. الرمز: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` مع تحديد الإدخال/الإزالة. الكود: `crates/iroha_data_model/src/metadata.rs`.
- نمط الاشتراك (طبقة التطبيق): الخطط عبارة عن إدخالات `AssetDefinition` مع البيانات التعريفية `subscription_plan`؛ الاشتراكات هي سجلات `Nft` مع بيانات التعريف `subscription`؛ يتم تنفيذ الفوترة حسب مشغلات الوقت التي تشير إلى NFTs الخاصة بالاشتراك. راجع `docs/source/subscriptions_api.md` و`crates/iroha_data_model/src/subscription.rs`.
- **أساسيات التشفير** (الميزة `sm`):- `Sm2PublicKey` / `Sm2Signature` يعكس نقطة SEC1 الأساسية + ترميز `r∥s` ذو العرض الثابت لـ SM2. يفرض المنشئون عضوية المنحنى ودلالات المعرف المميزة (`DEFAULT_DISTID`)، بينما يرفض التحقق الكميات المشوهة أو عالية النطاق. الرمز: `crates/iroha_crypto/src/sm.rs` و`crates/iroha_data_model/src/crypto/mod.rs`.
  - يعرض `Sm3Hash` ملخص GM/T 0004 باعتباره النوع الجديد Norito القابل للتسلسل `[u8; 32]` المستخدم أينما تظهر التجزئة في البيانات أو القياس عن بعد. الكود: `crates/iroha_data_model/src/crypto/hash.rs`.
  - يمثل `Sm4Key` مفاتيح SM4 ذات 128 بت ويتم مشاركتها بين مكالمات النظام المضيفة وتركيبات نماذج البيانات. الكود: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  توجد هذه الأنواع جنبًا إلى جنب مع عناصر Ed25519/BLS/ML-DSA الأولية الحالية وتكون متاحة لمستهلكي نماذج البيانات (Torii، SDKs، أدوات التكوين) بمجرد تمكين ميزة `sm`.

السمات المهمة: `Identifiable`، `Registered`/`Registrable` (نمط المنشئ)، `HasMetadata`، `IntoKeyValue`. الكود: `crates/iroha_data_model/src/lib.rs`.

الأحداث: كل كيان لديه أحداث منبعثة عند حدوث طفرات (إنشاء/حذف/تغيير المالك/تغيير بيانات التعريف، وما إلى ذلك). الكود: `crates/iroha_data_model/src/events/`.

---

## المعلمات (تكوين السلسلة)
- العائلات: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`، `BlockParameters { max_transactions }`، `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`، `SmartContractParameters { fuel, memory, execution_depth }`، بالإضافة إلى `custom: BTreeMap`.
- التعدادات الفردية للفرق: `SumeragiParameter`، `BlockParameter`، `TransactionParameter`، `SmartContractParameter`. المجمع: `Parameters`. الكود: `crates/iroha_data_model/src/parameter/system.rs`.

معلمات الإعداد (ISI): يقوم `SetParameter(Parameter)` بتحديث الحقل المقابل ويصدر `ConfigurationEvent::Changed`. الكود: `crates/iroha_data_model/src/isi/transparent.rs`، المنفذ في `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## تسلسل التعليمات والتسجيل
- السمة الأساسية: `Instruction: Send + Sync + 'static` مع `dyn_encode()`، `as_any()`، `id()` المستقر (الاسم الافتراضي هو اسم النوع الملموس).
- `InstructionBox`: غلاف `Box<dyn Instruction>`. يعمل Clone/Eq/Ord على `(type_id, encoded_bytes)` لذا تكون المساواة من حيث القيمة.
- يتم إجراء تسلسل Norito لـ `InstructionBox` كـ `(String wire_id, Vec<u8> payload)` (يعود إلى `type_name` في حالة عدم وجود معرف سلكي). تستخدم عملية إلغاء التسلسل معرفات التعيين `InstructionRegistry` العالمية للمنشئين. يتضمن التسجيل الافتراضي كافة ISI المضمنة. الكود: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: الأنواع، الدلالات، الأخطاء
يتم تنفيذ التنفيذ عبر `Execute for <Instruction>` في `iroha_core::smartcontracts::isi`. يسرد أدناه التأثيرات العامة والشروط المسبقة والأحداث المنبعثة والأخطاء.

### تسجيل / إلغاء التسجيل
الأنواع: `Register<T: Registered>` و`Unregister<T: Identifiable>`، مع أنواع المجموع `RegisterBox`/`UnregisterBox` التي تغطي الأهداف الملموسة.

- تسجيل النظير: يتم إدراجه في مجموعة أقران العالم.
  - الشروط المسبقة: يجب ألا تكون موجودة بالفعل.
  - الأحداث: `PeerEvent::Added`.
  - الأخطاء: `Repetition(Register, PeerId)` إذا كانت مكررة؛ `FindError` في عمليات البحث. الكود: `core/.../isi/world.rs`.

- تسجيل المجال: يتم إنشاؤه من `NewDomain` مع `owned_by = authority`. غير مسموح به: مجال `genesis`.
  - الشروط المسبقة: عدم وجود المجال؛ ليس `genesis`.
  - الأحداث: `DomainEvent::Created`.
  - الأخطاء: `Repetition(Register, DomainId)`، `InvariantViolation("Not allowed to register genesis domain")`. الكود: `core/.../isi/world.rs`.- تسجيل الحساب: تم ​​إنشاؤه من `NewAccount`، وهو غير مسموح به في المجال `genesis`؛ لا يمكن تسجيل حساب `genesis`.
  - الشروط المسبقة: يجب أن يكون المجال موجودا؛ عدم وجود الحساب؛ ليس في مجال التكوين.
  - الأحداث: `DomainEvent::Account(AccountEvent::Created)`.
  - الأخطاء: `Repetition(Register, AccountId)`، `InvariantViolation("Not allowed to register account in genesis domain")`. الكود: `core/.../isi/domain.rs`.

- تسجيل تعريف الأصول: يبني من المنشئ؛ مجموعات `owned_by = authority`.
  - الشروط المسبقة: عدم وجود التعريف؛ المجال موجود.
  - الأحداث: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - الأخطاء: `Repetition(Register, AssetDefinitionId)`. الكود: `core/.../isi/domain.rs`.

- سجل NFT: يبني من منشئ؛ مجموعات `owned_by = authority`.
  - الشروط المسبقة: عدم وجود NFT؛ المجال موجود.
  - الأحداث: `DomainEvent::Nft(NftEvent::Created)`.
  - الأخطاء: `Repetition(Register, NftId)`. الكود: `core/.../isi/nft.rs`.

- تسجيل الدور: تم إنشاؤه من `NewRole { inner, grant_to }` (تم تسجيل المالك الأول عبر تعيين دور الحساب)، ويخزن `inner: Role`.
  - الشروط المسبقة: عدم وجود الدور.
  - الأحداث: `RoleEvent::Created`.
  - الأخطاء: `Repetition(Register, RoleId)`. الكود: `core/.../isi/world.rs`.

- تسجيل المشغل: يقوم بتخزين المشغل في المشغل المناسب الذي تم تعيينه حسب نوع الفلتر.
  - الشروط المسبقة: إذا كان المرشح غير قابل للتعدين، فيجب أن يكون `action.repeats` هو `Exactly(1)` (وإلا فإن `MathError::Overflow`). معرفات مكررة محظورة.
  - الأحداث: `TriggerEvent::Created(TriggerId)`.
  - الأخطاء: `Repetition(Register, TriggerId)`، `InvalidParameterError::SmartContract(..)` عند فشل التحويل/التحقق من الصحة. الكود: `core/.../isi/triggers/mod.rs`.

- إلغاء تسجيل النظير/المجال/الحساب/AssetDefinition/NFT/Role/Trigger: إزالة الهدف؛ تنبعث أحداث الحذف. عمليات الإزالة المتتالية الإضافية:
  - إلغاء تسجيل المجال: إزالة كافة الحسابات في المجال، وأدوارها، وأذوناتها، وعدادات تسلسل الإرسال، وتسميات الحساب، وروابط UAID؛ يحذف أصولهم (والبيانات التعريفية لكل أصل)؛ يزيل كافة تعريفات الأصول في المجال؛ يحذف NFTs في المجال وأي NFTs مملوكة للحسابات المحذوفة؛ يزيل المشغلات التي يتطابق معها مجال السلطة. الأحداث: `DomainEvent::Deleted`، بالإضافة إلى أحداث الحذف لكل عنصر. الأخطاء: `FindError::Domain` إذا كان مفقودًا. الكود: `core/.../isi/world.rs`.
  - إلغاء تسجيل الحساب: إزالة أذونات الحساب، والأدوار، وعداد تسلسل الإرسال، وتعيين تسمية الحساب، وروابط UAID؛ حذف الأصول المملوكة للحساب (والبيانات التعريفية لكل أصل)؛ حذف NFTs المملوكة للحساب؛ يزيل المشغلات التي تكون سلطتها هي هذا الحساب. الأحداث: `AccountEvent::Deleted`، بالإضافة إلى `NftEvent::Deleted` لكل NFT تمت إزالته. الأخطاء: `FindError::Account` إذا كان مفقودًا. الكود: `core/.../isi/domain.rs`.
  - إلغاء تسجيل AssetDefinition: يحذف جميع أصول هذا التعريف وبيانات التعريف الخاصة بكل أصل. الأحداث: `AssetDefinitionEvent::Deleted` و`AssetEvent::Deleted` لكل أصل. الأخطاء: `FindError::AssetDefinition`. الكود: `core/.../isi/domain.rs`.
  - إلغاء تسجيل NFT: يزيل NFT. الأحداث: `NftEvent::Deleted`. الأخطاء: `FindError::Nft`. الكود: `core/.../isi/nft.rs`.
  - إلغاء تسجيل الدور: يلغي الدور من جميع الحسابات أولا؛ ثم يزيل الدور. الأحداث: `RoleEvent::Deleted`. الأخطاء: `FindError::Role`. الكود: `core/.../isi/world.rs`.
  - إلغاء تسجيل المشغل: إزالة المشغل إذا كان موجودًا؛ يؤدي إلغاء التسجيل المكرر إلى `Repetition(Unregister, TriggerId)`. الأحداث: `TriggerEvent::Deleted`. الكود: `core/.../isi/triggers/mod.rs`.

### نعناع / حرق
الأنواع: `Mint<O, D: Identifiable>` و`Burn<O, D: Identifiable>`، في صندوق كـ `MintBox`/`BurnBox`.- الأصول (الرقمية) النعناع/الحرق: ضبط الأرصدة والتعريف `total_quantity`.
  - الشروط المسبقة: يجب أن تفي قيمة `Numeric` بـ `AssetDefinition.spec()`؛ النعناع المسموح به بواسطة `mintable`:
    - `Infinitely`: مسموح به دائمًا.
    - `Once`: مسموح به مرة واحدة بالضبط؛ تقلب أول قطعة نعناع `mintable` إلى `Not` وتنبعث منها `AssetDefinitionEvent::MintabilityChanged`، بالإضافة إلى `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` التفصيلية لقابلية التدقيق.
    - `Limited(n)`: يسمح بعمليات النعناع الإضافية لـ `n`. كل نعناع ناجح ينقص العداد؛ عندما يصل إلى الصفر، ينقلب التعريف إلى `Not` ويصدر نفس أحداث `MintabilityChanged` كما هو مذكور أعلاه.
    - `Not`: الخطأ `MintabilityError::MintUnmintable`.
  - تغييرات الحالة: إنشاء أصل إذا كان مفقودًا في النعناع؛ يزيل إدخال الأصول إذا أصبح الرصيد صفرًا عند الحرق.
  - الأحداث: `AssetEvent::Added`/`AssetEvent::Removed`، `AssetDefinitionEvent::MintabilityChanged` (عندما يستنفد `Once` أو `Limited(n)` الحد المسموح به).
  - الأخطاء: `TypeError::AssetNumericSpec(Mismatch)`، `MathError::Overflow`/`NotEnoughQuantity`. الرمز: `core/.../isi/asset.rs`.

- تكرارات الزناد بالنعناع/الحرق: يتغير عدد `action.repeats` للمشغل.
  - الشروط المسبقة: في حالة النعناع، ​​يجب أن يكون الفلتر قابلاً للسك؛ يجب ألا يتجاوز الحساب/التجاوز.
  - الأحداث: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - الأخطاء: `MathError::Overflow` على النعناع غير صالح؛ `FindError::Trigger` إذا كان مفقودًا. الكود: `core/.../isi/triggers/mod.rs`.

### نقل
الأنواع: `Transfer<S: Identifiable, O, D: Identifiable>`، محاصر كـ `TransferBox`.

- الأصل (رقمي): اطرح من المصدر `AssetId`، وأضف إلى الوجهة `AssetId` (نفس التعريف، حساب مختلف). حذف أصل المصدر الصفري.
  - الشروط المسبقة: الأصل المصدر موجود؛ القيمة ترضي `spec`.
  - الأحداث: `AssetEvent::Removed` (المصدر)، `AssetEvent::Added` (الوجهة).
  - الأخطاء: `FindError::Asset`، `TypeError::AssetNumericSpec`، `MathError::NotEnoughQuantity/Overflow`. الكود: `core/.../isi/asset.rs`.

- ملكية المجال: تغير `Domain.owned_by` إلى حساب الوجهة.
  - الشروط المسبقة: كلا الحسابين موجودان؛ المجال موجود.
  - الأحداث: `DomainEvent::OwnerChanged`.
  - الأخطاء: `FindError::Account/Domain`. الكود: `core/.../isi/domain.rs`.

- ملكية AssetDefinition: التغييرات `AssetDefinition.owned_by` إلى حساب الوجهة.
  - الشروط المسبقة: كلا الحسابين موجودان؛ التعريف موجود؛ يجب أن يمتلكه المصدر حاليًا.
  - الأحداث: `AssetDefinitionEvent::OwnerChanged`.
  - الأخطاء: `FindError::Account/AssetDefinition`. الكود: `core/.../isi/account.rs`.

- ملكية NFT: تغير `Nft.owned_by` إلى حساب الوجهة.
  - الشروط المسبقة: كلا الحسابين موجودان؛ NFT موجود؛ يجب أن يمتلكه المصدر حاليًا.
  - الأحداث: `NftEvent::OwnerChanged`.
  - الأخطاء: `FindError::Account/Nft`، `InvariantViolation` إذا كان المصدر لا يملك NFT. الكود: `core/.../isi/nft.rs`.

### البيانات الوصفية: ضبط/إزالة قيمة المفتاح
الأنواع: `SetKeyValue<T>` و`RemoveKeyValue<T>` مع `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. التعدادات محاصر المقدمة.

- تعيين: إدراج `Metadata[key] = Json(value)` أو استبداله.
- إزالة: إزالة المفتاح؛ خطأ إذا كان في عداد المفقودين.
- الأحداث: `<Target>Event::MetadataInserted` / `MetadataRemoved` بالقيم القديمة/الجديدة.
- الأخطاء: `FindError::<Target>` إذا كان الهدف غير موجود؛ `FindError::MetadataKey` على المفتاح المفقود للإزالة. الكود: `crates/iroha_data_model/src/isi/transparent.rs` والمنفذ يتضمن كل هدف.### الأذونات والأدوار: منح / إلغاء
الأنواع: `Grant<O, D>` و`Revoke<O, D>`، مع التعدادات المعبأة لـ `Permission`/`Role` إلى/من `Account`، و`Permission` إلى/من `Role`.

- منح الإذن للحساب: يضيف `Permission` ما لم يكن متأصلًا بالفعل. الأحداث: `AccountEvent::PermissionAdded`. الأخطاء: `Repetition(Grant, Permission)` إذا كانت مكررة. الرمز: `core/.../isi/account.rs`.
- إلغاء الإذن من الحساب: تتم إزالته إذا كان موجودًا. الأحداث: `AccountEvent::PermissionRemoved`. الأخطاء: `FindError::Permission` في حالة الغياب. الكود: `core/.../isi/account.rs`.
- منح الدور للحساب: يُدرج تعيين `(account, role)` في حالة عدم وجوده. الأحداث: `AccountEvent::RoleGranted`. الأخطاء: `Repetition(Grant, RoleId)`. الرمز: `core/.../isi/account.rs`.
- إبطال الدور من الحساب: إزالة التعيين إذا كان موجودًا. الأحداث: `AccountEvent::RoleRevoked`. الأخطاء: `FindError::Role` في حالة الغياب. الرمز: `core/.../isi/account.rs`.
- منح الإذن للدور: إعادة بناء الدور مع إضافة الإذن. الأحداث: `RoleEvent::PermissionAdded`. الأخطاء: `Repetition(Grant, Permission)`. الكود: `core/.../isi/world.rs`.
- إبطال الإذن من الدور: إعادة بناء الدور دون هذا الإذن. الأحداث: `RoleEvent::PermissionRemoved`. الأخطاء: `FindError::Permission` في حالة الغياب. الكود: `core/.../isi/world.rs`.

### المشغلات: تنفيذ
النوع: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- السلوك: يدرج `ExecuteTriggerEvent { trigger_id, authority, args }` لنظام التشغيل الفرعي. يُسمح بالتنفيذ اليدوي فقط لمشغلات النداءات الجانبية (مرشح `ExecuteTrigger`)؛ يجب أن يتطابق عامل التصفية ويجب أن يكون المتصل هو سلطة إجراء التشغيل أو يحمل `CanExecuteTrigger` لتلك السلطة. عندما يكون المنفذ المقدم من المستخدم نشطًا، يتم التحقق من صحة تنفيذ التشغيل بواسطة منفذ وقت التشغيل ويستهلك ميزانية وقود منفذ المعاملة (الأساس `executor.fuel` بالإضافة إلى بيانات التعريف الاختيارية `additional_fuel`).
- الأخطاء: `FindError::Trigger` إذا لم تكن مسجلة؛ `InvariantViolation` إذا تم استدعاؤه من قبل جهة غير مخولة. الرمز: `core/.../isi/triggers/mod.rs` (والاختبارات في `core/.../smartcontracts/isi/mod.rs`).

### الترقية والتسجيل
- `Upgrade { executor }`: لترحيل المنفذ باستخدام الرمز الثانوي `Executor` المقدم، وتحديث المنفذ ونموذج البيانات الخاص به، وإصدار `ExecutorEvent::Upgraded`. الأخطاء: ملفوفة كـ `InvalidParameterError::SmartContract` عند فشل الترحيل. الكود: `core/.../isi/world.rs`.
- `Log { level, msg }`: يصدر سجل العقدة بالمستوى المحدد؛ لا تغييرات الدولة. الكود: `core/.../isi/world.rs`.

### نموذج الخطأ
المغلف الشائع: `InstructionExecutionError` مع متغيرات لأخطاء التقييم، وفشل الاستعلام، والتحويلات، ولم يتم العثور على الكيان، والتكرار، وقابلية التعدين، والرياضيات، والمعلمة غير الصالحة، والانتهاك الثابت. التعدادات والمساعدات موجودة في `crates/iroha_data_model/src/isi/mod.rs` ضمن `pub mod error`.

---## المعاملات والملفات التنفيذية
- `Executable`: إما `Instructions(ConstVec<InstructionBox>)` أو `Ivm(IvmBytecode)`؛ يتم إجراء تسلسل bytecode كـ base64. الرمز: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: إنشاء ملف قابل للتنفيذ مع بيانات التعريف وتوقيعه وحزمه، `chain_id`، `authority`، `creation_time_ms`، `ttl_ms` الاختياري، و `nonce`. الكود: `crates/iroha_data_model/src/transaction/`.
- في وقت التشغيل، يقوم `iroha_core` بتنفيذ دفعات `InstructionBox` عبر `Execute for InstructionBox`، مع الانتقال إلى `*Box` المناسب أو التعليمات الملموسة. الكود: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- ميزانية التحقق من صحة المنفذ في وقت التشغيل (المنفذ المقدم من المستخدم): قاعدة `executor.fuel` من المعلمات بالإضافة إلى بيانات تعريف المعاملة الاختيارية `additional_fuel` (`u64`)، المشتركة عبر عمليات التحقق من صحة التعليمات/المشغلات داخل المعاملة.

---

## الثوابت والملاحظات (من الاختبارات والحراس)
- حماية التكوين: لا يمكن تسجيل المجال `genesis` أو الحسابات في المجال `genesis`؛ لا يمكن تسجيل حساب `genesis`. الكود/الاختبارات: `core/.../isi/world.rs`، `core/.../smartcontracts/isi/mod.rs`.
- يجب أن تستوفي الأصول الرقمية `NumericSpec` عند النعناع/النقل/النسخ؛ يؤدي عدم تطابق المواصفات إلى `TypeError::AssetNumericSpec`.
- قابلية التعدين: `Once` يسمح بالنعناع الواحد ثم يقلب إلى `Not`؛ يسمح `Limited(n)` بالضبط بالنعناع `n` قبل التقليب إلى `Not`. تؤدي محاولات منع سك العملة على `Infinitely` إلى حدوث `MintabilityError::ForbidMintOnMintable`، وينتج عن تكوين `Limited(0)` `MintabilityError::InvalidMintabilityTokens`.
- عمليات البيانات الوصفية دقيقة للغاية؛ تعتبر إزالة مفتاح غير موجود خطأً.
- يمكن أن تكون مرشحات التشغيل غير قابلة للسك؛ ثم يسمح `Register<Trigger>` بتكرار `Exactly(1)` فقط.
- تشغيل بوابات البيانات التعريفية بمفتاح `__enabled` (منطقي)؛ يتم تخطي الإعدادات الافتراضية المفقودة إلى التمكين، ويتم تخطي المشغلات المعطلة عبر مسارات البيانات/الوقت/المكالمات.
- الحتمية: جميع العمليات الحسابية تستخدم العمليات المحددة؛ يُرجع Under/overflow الأخطاء الحسابية المكتوبة؛ أرصدة صفرية تسقط إدخالات الأصول (لا توجد حالة مخفية).

---## أمثلة عملية
- السك والنقل:
  - `Mint::asset_numeric(10, asset_id)` → يضيف 10 إذا سمحت المواصفات/قابلية التعدين؛ الأحداث: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → التحركات 5؛ أحداث للإزالة/الإضافة.
- تحديثات البيانات الوصفية:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert؛ الإزالة عبر `RemoveKeyValue::account(...)`.
- إدارة الدور/الإذن:
  - `Grant::account_role(role_id, account)`، و`Grant::role_permission(perm, role)`، ونظيراتها `Revoke`.
- دورة حياة الزناد:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` مع التحقق من قابلية التعدين ضمنيًا بواسطة الفلتر؛ يجب أن يتطابق `ExecuteTrigger::new(id).with_args(&args)` مع السلطة التي تم تكوينها.
  - يمكن تعطيل المشغلات عن طريق تعيين مفتاح بيانات التعريف `__enabled` إلى `false` (فقد الإعدادات الافتراضية ممكّنة)؛ قم بالتبديل عبر `SetKeyValue::trigger` أو IVM `set_trigger_enabled` syscall.
  - يتم إصلاح تخزين المشغلات عند التحميل: يتم إسقاط المعرفات المكررة والمعرفات غير المتطابقة والمشغلات التي تشير إلى الكود الثانوي المفقود؛ يتم إعادة حساب أعداد مرجع البايت كود.
  - إذا كان الرمز الثانوي IVM الخاص بالمشغل مفقودًا في وقت التنفيذ، تتم إزالة المشغل ويتم التعامل مع التنفيذ على أنه عدم تنفيذ مع نتيجة الفشل.
  - تتم إزالة المشغلات المستنفدة على الفور؛ إذا تمت مصادفة إدخال مستنفد أثناء التنفيذ، فسيتم تقليمه ومعاملته على أنه مفقود.
- تحديث المعلمة:
  - يتم تحديث `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` ويصدر `ConfigurationEvent::Changed`.

---

## إمكانية التتبع (مصادر مختارة)
 - نواة نموذج البيانات: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - تعريفات ISI والتسجيل: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - تنفيذ ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - الأحداث: `crates/iroha_data_model/src/events/**`.
 - المعاملات: `crates/iroha_data_model/src/transaction/**`.

إذا كنت تريد توسيع هذه المواصفات إلى جدول سلوك/واجهة برمجة تطبيقات معروضة أو ربطها بكل حدث/خطأ ملموس، قل الكلمة وسأقوم بتوسيعها.