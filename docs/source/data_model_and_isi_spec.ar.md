---
lang: ar
direction: rtl
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
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
- `AccountId` - يتم إنتاج العناوين الأساسية عبر `AccountAddress` (I105 / hex) ويقوم Torii بتطبيع المدخلات من خلال `AccountAddress::parse_encoded`. I105 هو تنسيق الحساب المفضل؛ النموذج I105 مخصص لـ Sora فقط UX. يتم الاحتفاظ بسلسلة `alias` (النموذج القديم المرفوض) المألوفة كاسم مستعار للتوجيه فقط. الحساب: `{ id, metadata }`. الكود: `crates/iroha_data_model/src/account.rs`.- سياسة قبول الحساب — تتحكم النطاقات في إنشاء الحساب الضمني عن طريق تخزين Norito-JSON `AccountAdmissionPolicy` ضمن مفتاح البيانات الوصفية `iroha:account_admission_policy`. عند غياب المفتاح، توفر المعلمة المخصصة على مستوى السلسلة `iroha:default_account_admission_policy` الإعداد الافتراضي؛ عندما يكون ذلك غائبًا أيضًا، يكون الإعداد الافتراضي الثابت هو `ImplicitReceive` (الإصدار الأول). علامات السياسة `mode` (`ExplicitOnly` أو `ImplicitReceive`) بالإضافة إلى الاختيارية لكل معاملة (الافتراضي `16`) والحد الأقصى لإنشاء كل كتلة، و`implicit_creation_fee` اختياري (حساب النسخ أو الغرق)، و`min_initial_amounts` لكل تعريف أصل، و `default_role_on_create` اختياري (يتم منحه بعد `AccountCreated`، ويتم رفضه مع `DefaultRoleError` إذا كان مفقودًا). لا يمكن لـ Genesis الاشتراك؛ ترفض السياسات المعطلة/غير الصالحة تعليمات نمط الاستلام للحسابات غير المعروفة باستخدام `InstructionExecutionError::AccountAdmission`. تقوم الحسابات الضمنية بختم بيانات التعريف `iroha:created_via="implicit"` قبل `AccountCreated`؛ تنبعث الأدوار الافتراضية من `AccountRoleGranted` للمتابعة، وتسمح قواعد خط الأساس لمالك المنفذ للحساب الجديد بإنفاق أصوله/NFTs الخاصة به دون أدوار إضافية. الرمز: `crates/iroha_data_model/src/account/admission.rs`، `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` - `unprefixed Base58 address with versioning and checksum` الأساسي (UUID-v4 بايت). التعريف: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. يجب أن تكون القيم الحرفية `alias` هي `<name>#<domain>.<dataspace>` أو `<name>#<dataspace>`، وأن يكون `<name>` مساوٍ لاسم تعريف الأصل. الكود: `crates/iroha_data_model/src/asset/definition.rs`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`. Alias selectors resolve against the latest committed block creation time and stop resolving after grace even before sweep removes stale bindings.
- `AssetId`: `norito:<hex>` الحرفي المشفر بشكل أساسي (النماذج النصية القديمة غير مدعومة في الإصدار الأول).- `NftId` — `nft$domain`. إن إف تي: `{ id, content: Metadata, owned_by }`. الكود: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. الدور: `{ id, permissions: BTreeSet<Permission> }` مع المنشئ `NewRole { inner: Role, grant_to }`. الكود: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. الكود: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — هوية النظير (المفتاح العام) والعنوان. الرمز: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. المشغل: `{ id, action }`. الإجراء: `{ executable, repeats, authority, filter, metadata }`. الكود: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` مع تحديد الإدخال/الإزالة. الكود: `crates/iroha_data_model/src/metadata.rs`.
- نمط الاشتراك (طبقة التطبيق): الخطط عبارة عن إدخالات `AssetDefinition` مع بيانات التعريف `subscription_plan`؛ الاشتراكات هي سجلات `Nft` مع بيانات التعريف `subscription`؛ يتم تنفيذ الفوترة حسب مشغلات الوقت التي تشير إلى NFTs الخاصة بالاشتراك. راجع `docs/source/subscriptions_api.md` و`crates/iroha_data_model/src/subscription.rs`.
- **أساسيات التشفير** (الميزة `sm`):
  - يعكس `Sm2PublicKey` / `Sm2Signature` نقطة SEC1 الأساسية + ترميز `r∥s` ذو العرض الثابت لـ SM2. يفرض المنشئون عضوية المنحنى ودلالات المعرف المميزة (`DEFAULT_DISTID`)، بينما يرفض التحقق الكميات المشوهة أو عالية النطاق. الرمز: `crates/iroha_crypto/src/sm.rs` و`crates/iroha_data_model/src/crypto/mod.rs`.
  - يعرض `Sm3Hash` ملخص GM/T 0004 باعتباره النوع الجديد Norito القابل للتسلسل `[u8; 32]` المستخدم أينما تظهر التجزئة في البيانات أو القياس عن بعد. الكود: `crates/iroha_data_model/src/crypto/hash.rs`.- يمثل `Sm4Key` مفاتيح SM4 ذات 128 بت ويتم مشاركتها بين مكالمات النظام المضيفة وتركيبات نماذج البيانات. الكود: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  توجد هذه الأنواع جنبًا إلى جنب مع عناصر Ed25519/BLS/ML-DSA الأولية الحالية وتكون متاحة لمستهلكي نماذج البيانات (Torii، SDKs، أدوات التكوين) بمجرد تمكين ميزة `sm`.
- يتم تقليم مخازن العلاقات المشتقة من مساحة البيانات (`space_directory_manifests`، `uaid_dataspaces`، `axt_policies`، `axt_replay_ledger`، سجل تجاوز الطوارئ لترحيل المسار) وأذونات هدف مساحة البيانات (`CanPublishSpaceDirectoryManifest{dataspace: ...}` في مخازن أذونات الحساب/الدور) `State::set_nexus(...)` عندما تختفي مساحات البيانات من `dataspace_catalog` النشط، مما يمنع مراجع مساحة البيانات التي لا معنى لها بعد تحديثات كتالوج وقت التشغيل. يتم أيضًا تقليم ذاكرة التخزين المؤقت DA/ترحيل النطاق (`lane_relays`، `da_commitments`، `da_confidential_compute`، `da_pin_intents`) عند تقاعد المسار أو إعادة تعيينه إلى مساحة بيانات مختلفة، لذلك لا يمكن لحالة المسار المحلي أن تتسرب عبر عمليات ترحيل مساحة البيانات. تقوم ISIs الخاصة بدليل الفضاء (`PublishSpaceDirectoryManifest`، `RevokeSpaceDirectoryManifest`، `ExpireSpaceDirectoryManifest`) أيضًا بالتحقق من صحة `dataspace` مقابل الكتالوج النشط وترفض المعرفات غير المعروفة باستخدام `InvalidParameter`.

السمات المهمة: `Identifiable`، `Registered`/`Registrable` (نمط المنشئ)، `HasMetadata`، `IntoKeyValue`. الكود: `crates/iroha_data_model/src/lib.rs`.

الأحداث: كل كيان لديه أحداث منبعثة عند حدوث طفرات (إنشاء/حذف/تغيير المالك/تغيير بيانات التعريف، وما إلى ذلك). الكود: `crates/iroha_data_model/src/events/`.

---## المعلمات (تكوين السلسلة)
- العائلات: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`، `BlockParameters { max_transactions }`، `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`، `SmartContractParameters { fuel, memory, execution_depth }`، بالإضافة إلى `custom: BTreeMap`.
- التعدادات الفردية للفرق: `SumeragiParameter`، `BlockParameter`، `TransactionParameter`، `SmartContractParameter`. المجمع: `Parameters`. الكود: `crates/iroha_data_model/src/parameter/system.rs`.

معلمات الإعداد (ISI): يقوم `SetParameter(Parameter)` بتحديث الحقل المقابل ويصدر `ConfigurationEvent::Changed`. الكود: `crates/iroha_data_model/src/isi/transparent.rs`، المنفذ في `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## تسلسل التعليمات والتسجيل
- السمة الأساسية: `Instruction: Send + Sync + 'static` مع `dyn_encode()`، `as_any()`، `id()` المستقر (الاسم الافتراضي هو اسم النوع الملموس).
- `InstructionBox`: غلاف `Box<dyn Instruction>`. يعمل Clone/Eq/Ord على `(type_id, encoded_bytes)` لذا تكون المساواة من حيث القيمة.
- يتم إجراء تسلسل Norito لـ `InstructionBox` كـ `(String wire_id, Vec<u8> payload)` (يعود إلى `type_name` في حالة عدم وجود معرف سلكي). تستخدم عملية إلغاء التسلسل معرفات التعيين العالمية `InstructionRegistry` للمنشئين. يتضمن التسجيل الافتراضي كافة ISI المضمنة. الكود: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: الأنواع، الدلالات، الأخطاء
يتم تنفيذ التنفيذ عبر `Execute for <Instruction>` في `iroha_core::smartcontracts::isi`. يسرد أدناه التأثيرات العامة والشروط المسبقة والأحداث المنبعثة والأخطاء.

### تسجيل / إلغاء التسجيل
الأنواع: `Register<T: Registered>` و`Unregister<T: Identifiable>`، مع أنواع المجموع `RegisterBox`/`UnregisterBox` التي تغطي الأهداف الملموسة.- تسجيل النظير: يتم إدراجه في مجموعة أقران العالم.
  - الشروط المسبقة: يجب ألا تكون موجودة بالفعل.
  - الأحداث: `PeerEvent::Added`.
  - الأخطاء: `Repetition(Register, PeerId)` إذا كانت مكررة؛ `FindError` في عمليات البحث. الكود: `core/.../isi/world.rs`.

- تسجيل المجال: يتم إنشاؤه من `NewDomain` مع `owned_by = authority`. غير مسموح به: مجال `genesis`.
  - الشروط المسبقة: عدم وجود المجال؛ ليس `genesis`.
  - الأحداث: `DomainEvent::Created`.
  - الأخطاء: `Repetition(Register, DomainId)`، `InvariantViolation("Not allowed to register genesis domain")`. الكود: `core/.../isi/world.rs`.

- تسجيل الحساب: تم ​​إنشاؤه من `NewAccount`، وهو غير مسموح به في المجال `genesis`؛ لا يمكن تسجيل حساب `genesis`.
  - الشروط المسبقة: يجب أن يكون المجال موجودا؛ عدم وجود الحساب؛ ليس في مجال التكوين.
  - الأحداث: `DomainEvent::Account(AccountEvent::Created)`.
  - الأخطاء: `Repetition(Register, AccountId)`، `InvariantViolation("Not allowed to register account in genesis domain")`. الكود: `core/.../isi/domain.rs`.

- تسجيل تعريف الأصول: يبني من المنشئ؛ مجموعات `owned_by = authority`.
  - الشروط المسبقة: عدم وجود التعريف؛ المجال موجود؛ مطلوب `name`، ويجب ألا يكون فارغًا بعد القطع، ويجب ألا يحتوي على `#`/`@`.
  - الأحداث: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - الأخطاء: `Repetition(Register, AssetDefinitionId)`. الكود: `core/.../isi/domain.rs`.

- سجل NFT: يبني من منشئ؛ مجموعات `owned_by = authority`.
  - الشروط المسبقة: عدم وجود NFT؛ المجال موجود.
  - الأحداث: `DomainEvent::Nft(NftEvent::Created)`.
  - الأخطاء: `Repetition(Register, NftId)`. الكود: `core/.../isi/nft.rs`.- تسجيل الدور: تم إنشاؤه من `NewRole { inner, grant_to }` (المالك الأول المسجل عبر تعيين دور الحساب)، ويخزن `inner: Role`.
  - الشروط المسبقة: عدم وجود الدور.
  - الأحداث: `RoleEvent::Created`.
  - الأخطاء: `Repetition(Register, RoleId)`. الكود: `core/.../isi/world.rs`.

- تسجيل المشغل: يقوم بتخزين المشغل في المشغل المناسب الذي تم تعيينه حسب نوع الفلتر.
  - الشروط المسبقة: إذا كان الفلتر غير قابل للتعدين، فيجب أن يكون `action.repeats` هو `Exactly(1)` (وإلا فإن `MathError::Overflow`). معرفات مكررة محظورة.
  - الأحداث: `TriggerEvent::Created(TriggerId)`.
  - الأخطاء: `Repetition(Register, TriggerId)`، `InvalidParameterError::SmartContract(..)` عند فشل التحويل/التحقق من الصحة. الكود: `core/.../isi/triggers/mod.rs`.- إلغاء تسجيل النظير/المجال/الحساب/AssetDefinition/NFT/Role/Trigger: إزالة الهدف؛ تنبعث أحداث الحذف. عمليات الإزالة المتتالية الإضافية:- إلغاء تسجيل النطاق: إزالة كيان المجال بالإضافة إلى حالة سياسة التحديد/المصادقة الخاصة به؛ يحذف تعريفات الأصول في المجال (والحالة الجانبية السرية `zk_assets` المرتبطة بهذه التعريفات)، وأصول تلك التعريفات (والبيانات التعريفية لكل أصل)، ورموز NFT في المجال، وإسقاطات تسمية الحساب/الاسم المستعار على نطاق النطاق. كما أنه يقوم بإلغاء ربط الحسابات الباقية من المجال الذي تمت إزالته ويقلص إدخالات إذن نطاق الحساب/الدور التي تشير إلى المجال المحذوف أو الموارد المحذوفة معه (أذونات المجال، وأذونات تعريف الأصول/الأصول للتعريفات المحذوفة، وأذونات NFT لمعرفات NFT المحذوفة). لا تؤدي إزالة النطاق إلى حذف `AccountId` العالمي أو حالة تسلسل الإرسال/UAID الخاصة به أو الأصول الأجنبية أو ملكية NFT أو سلطة التشغيل أو مراجع التدقيق/التكوين الخارجية الأخرى التي تشير إلى الحساب الباقي. حواجز الحماية: يتم الرفض عندما تتم الإشارة إلى أي تعريف للأصول في المجال من خلال اتفاقية إعادة الشراء، ودفتر الأستاذ للتسوية، ومكافأة/مطالبة المسار العام، والبدل/التحويل دون اتصال بالإنترنت، وافتراضيات إعادة الشراء للتسوية (`settlement.repo.eligible_collateral`، `settlement.repo.collateral_substitution_matrix`)، والتصويت/المواطنة/أهلية البرلمان/مراجع تعريف أصول المكافآت الفيروسية، واقتصاديات أوراكل مراجع تعريف أصول المكافآت/الشرطة المائلة/سندات النزاع التي تم تكوينها، أو مراجع تعريف أصول الرسوم/التخزين Nexus (`nexus.fees.fee_asset_id`، `nexus.staking.stake_asset_id`). الأحداث: `DomainEvent::Deleted`، بالإضافة إلى عمليات الحذف لكل عنصرفي أحداث الموارد التي تمت إزالتها على نطاق المجال. الأخطاء: `FindError::Domain` إذا كان مفقودًا؛ `InvariantViolation` بشأن تعارضات مرجع تعريف الأصول المحتجزة. الكود: `core/.../isi/world.rs`.- إلغاء تسجيل الحساب: إزالة أذونات الحساب، والأدوار، وعداد تسلسل الإرسال، وتعيين تسمية الحساب، وروابط UAID؛ حذف الأصول المملوكة للحساب (والبيانات التعريفية لكل أصل)؛ حذف NFTs المملوكة للحساب؛ يزيل المشغلات التي تكون سلطتها هي هذا الحساب؛ تقليم إدخالات الأذونات على مستوى الحساب/الدور التي تشير إلى الحساب المحذوف، وأذونات هدف NFT على نطاق الحساب/الدور لمعرفات NFT المملوكة التي تمت إزالتها، وأذونات هدف المشغل على نطاق الحساب/الدور للمشغلات التي تمت إزالتها. حواجز الحماية: يتم الرفض إذا كان الحساب لا يزال يمتلك مجالًا، أو تعريف الأصل، أو ربط موفر SoraFS، أو سجل المواطنة النشط، أو حالة المكافأة/الستاكينغ في المسار العام (بما في ذلك مفاتيح المطالبة بالمكافأة حيث يظهر الحساب كمطالب أو مالك أصل المكافأة)، أو حالة أوراكل النشطة (بما في ذلك إدخالات موفر محفوظات تغذية أوراكل، أو سجلات موفر ربط تويتر، أو مراجع حساب المكافأة/الشرطة المائلة التي تم تكوينها بواسطة أوراكل-إيكونوميكس)، نشطة Nexus مراجع حساب الرسوم/التحصيل (`nexus.fees.fee_sink_account_id`، `nexus.staking.stake_escrow_account_id`، `nexus.staking.slash_sink_account_id`؛ تم تحليلها كمعرفات حساب أساسية بدون مجال وتم رفض الإغلاق الفاشل على القيم الحرفية غير الصالحة)، وحالة اتفاقية الريبو النشطة، وحالة دفتر الأستاذ النشط، والبدل/النقل النشط دون اتصال أو دون اتصال حالة إبطال الحكم، ومراجع تكوين حساب الضمان النشطة دون اتصال لتعريفات الأصول النشطة (`settlement.offline.escrow_accounts`)، وحالة الحوكمة النشطة (الموافقة على الاقتراح/المرحلةقوائم als/locks/الشرطات المائلة/المجلس/البرلمان، لقطات البرلمان المقترحة، سجلات مقترحي ترقية وقت التشغيل، مراجع حساب الضمان/مستقبل الشرطة المائلة/مجمع الفيروسات، الحوكمة SoraFS مراجع مُرسل القياس عن بعد عبر `gov.sorafs_telemetry.submitters` / `gov.sorafs_telemetry.per_provider_submitters`، أو الحوكمة التي تم تكوينها SoraFS مراجع مالك الموفر عبر `gov.sorafs_provider_owners`)، أو مراجع حساب القائمة المسموح بها لنشر المحتوى (`content.publish_allow_accounts`)، أو حالة مرسل الضمان الاجتماعي النشطة، أو حالة منشئ حزمة المحتوى النشطة، أو حالة مالك نية دبوس DA النشطة، أو حالة تجاوز مدقق الطوارئ النشط لترحيل المسار، أو SoraFS النشط سجلات مُصدر/موثق سجل الدبوس (بيانات الدبوس، والأسماء المستعارة للبيان، وأوامر النسخ المتماثل). الأحداث: `AccountEvent::Deleted`، بالإضافة إلى `NftEvent::Deleted` لكل NFT تمت إزالته. الأخطاء: `FindError::Account` إذا كان مفقودًا؛ `InvariantViolation` على ملكية الأيتام. الكود: `core/.../isi/domain.rs`.- إلغاء تسجيل AssetDefinition: حذف جميع الأصول الخاصة بهذا التعريف وبيانات التعريف الخاصة بكل أصل، وإزالة الحالة الجانبية السرية `zk_assets` المرتبطة بهذا التعريف؛ يقوم أيضًا بتشذيب إدخال `settlement.offline.escrow_accounts` المطابق وإدخالات الأذونات على مستوى الحساب/الدور التي تشير إلى تعريف الأصل الذي تمت إزالته أو مثيلات الأصل الخاصة به. حواجز الحماية: يتم الرفض عندما لا يزال يتم الإشارة إلى التعريف من خلال اتفاقية الريبو، ودفتر الأستاذ للتسوية، والمكافأة/المطالبة العامة، وحالة البدل/النقل دون اتصال بالإنترنت، وافتراضيات إعادة الشراء للتسوية (`settlement.repo.eligible_collateral`، `settlement.repo.collateral_substitution_matrix`)، والتصويت/المواطنة/أهلية البرلمان/مراجع تعريف أصول المكافآت الفيروسية، وتكوين اقتصاديات Oracle مراجع تعريف أصول المكافآت/الشرطة المائلة/سندات النزاع، أو مراجع تعريف أصول الرسوم/التخزين Nexus (`nexus.fees.fee_asset_id`، `nexus.staking.stake_asset_id`). الأحداث: `AssetDefinitionEvent::Deleted` و`AssetEvent::Deleted` لكل أصل. الأخطاء: `FindError::AssetDefinition`، `InvariantViolation` عند تعارض المراجع. الكود: `core/.../isi/domain.rs`.
  - إلغاء تسجيل NFT: يزيل NFT ويقلص إدخالات الأذونات ذات نطاق الحساب/الدور التي تشير إلى NFT الذي تمت إزالته. الأحداث: `NftEvent::Deleted`. الأخطاء: `FindError::Nft`. الكود: `core/.../isi/nft.rs`.
  - إلغاء تسجيل الدور: يلغي الدور من جميع الحسابات أولا؛ ثم يزيل الدور. الأحداث: `RoleEvent::Deleted`. الأخطاء: `FindError::Role`. الكود: `core/.../isi/world.rs`.- إلغاء تسجيل المشغل: إزالة المشغل إذا كان موجودًا وتقليم إدخالات الأذونات الخاصة بالحساب/الدور التي تشير إلى المشغل الذي تمت إزالته؛ يؤدي إلغاء التسجيل المكرر إلى `Repetition(Unregister, TriggerId)`. الأحداث: `TriggerEvent::Deleted`. الكود: `core/.../isi/triggers/mod.rs`.

### نعناع / حرق
الأنواع: `Mint<O, D: Identifiable>` و`Burn<O, D: Identifiable>`، في صندوق كـ `MintBox`/`BurnBox`.

- الأصول (الرقمية) النعناع/الحرق: ضبط الأرصدة وتعريف `total_quantity`.
  - الشروط المسبقة: يجب أن تفي قيمة `Numeric` بـ `AssetDefinition.spec()`؛ النعناع المسموح به بواسطة `mintable`:
    - `Infinitely`: مسموح به دائمًا.
    - `Once`: مسموح به مرة واحدة بالضبط؛ تقلب أول قطعة نعناع `mintable` إلى `Not` وتصدر `AssetDefinitionEvent::MintabilityChanged`، بالإضافة إلى `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` التفصيلية لقابلية التدقيق.
    - `Limited(n)`: يسمح بعمليات النعناع الإضافية لـ `n`. كل نعناع ناجح ينقص العداد؛ عندما يصل إلى الصفر، ينقلب التعريف إلى `Not` ويصدر نفس أحداث `MintabilityChanged` كما هو مذكور أعلاه.
    - `Not`: الخطأ `MintabilityError::MintUnmintable`.
  - تغييرات الحالة: إنشاء أصل إذا كان مفقودًا في النعناع؛ يزيل إدخال الأصول إذا أصبح الرصيد صفرًا عند الحرق.
  - الأحداث: `AssetEvent::Added`/`AssetEvent::Removed`، `AssetDefinitionEvent::MintabilityChanged` (عندما يستنفد `Once` أو `Limited(n)` الحد المسموح به).
  - الأخطاء: `TypeError::AssetNumericSpec(Mismatch)`، `MathError::Overflow`/`NotEnoughQuantity`. الكود: `core/.../isi/asset.rs`.- تكرارات الزناد بالنعناع/الحرق: يتغير عدد `action.repeats` للمشغل.
  - الشروط المسبقة: في حالة النعناع، ​​يجب أن يكون الفلتر قابلاً للسك؛ يجب ألا يتجاوز الحساب/التجاوز.
  - الأحداث: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - الأخطاء: `MathError::Overflow` على النعناع غير صالح؛ `FindError::Trigger` إذا كان مفقودًا. الكود: `core/.../isi/triggers/mod.rs`.

### نقل
الأنواع: `Transfer<S: Identifiable, O, D: Identifiable>`، محاصر كـ `TransferBox`.

- الأصل (رقمي): اطرح من المصدر `AssetId`، وأضف إلى الوجهة `AssetId` (نفس التعريف، حساب مختلف). حذف أصل المصدر الصفري.
  - الشروط المسبقة: الأصل المصدر موجود؛ القيمة ترضي `spec`.
  - الأحداث: `AssetEvent::Removed` (المصدر)، `AssetEvent::Added` (الوجهة).
  - الأخطاء: `FindError::Asset`، `TypeError::AssetNumericSpec`، `MathError::NotEnoughQuantity/Overflow`. الرمز: `core/.../isi/asset.rs`.

- ملكية المجال: تغير `Domain.owned_by` إلى حساب الوجهة.
  - الشروط المسبقة: كلا الحسابين موجودان؛ المجال موجود.
  - الأحداث: `DomainEvent::OwnerChanged`.
  - الأخطاء: `FindError::Account/Domain`. الرمز: `core/.../isi/domain.rs`.

- ملكية AssetDefinition: التغييرات `AssetDefinition.owned_by` إلى حساب الوجهة.
  - الشروط المسبقة: كلا الحسابين موجودان؛ التعريف موجود؛ يجب أن يمتلكه المصدر حاليًا؛ يجب أن تكون السلطة حساب المصدر، أو مالك نطاق المصدر، أو مالك نطاق تعريف الأصل.
  - الأحداث: `AssetDefinitionEvent::OwnerChanged`.
  - الأخطاء: `FindError::Account/AssetDefinition`. الكود: `core/.../isi/account.rs`.- ملكية NFT: تغير `Nft.owned_by` إلى حساب الوجهة.
  - الشروط المسبقة: كلا الحسابين موجودان؛ NFT موجود؛ يجب أن يمتلكه المصدر حاليًا؛ يجب أن تكون السلطة هي حساب المصدر، أو مالك المجال المصدر، أو مالك مجال NFT، أو تحمل `CanTransferNft` لذلك NFT.
  - الأحداث: `NftEvent::OwnerChanged`.
  - الأخطاء: `FindError::Account/Nft`، `InvariantViolation` إذا كان المصدر لا يملك NFT. الكود: `core/.../isi/nft.rs`.

### البيانات الوصفية: ضبط/إزالة قيمة المفتاح
الأنواع: `SetKeyValue<T>` و`RemoveKeyValue<T>` مع `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. التعدادات محاصر المقدمة.

- تعيين: إدراج `Metadata[key] = Json(value)` أو استبداله.
- إزالة: إزالة المفتاح؛ خطأ إذا كان في عداد المفقودين.
- الأحداث: `<Target>Event::MetadataInserted` / `MetadataRemoved` بالقيم القديمة/الجديدة.
- الأخطاء: `FindError::<Target>` إذا كان الهدف غير موجود؛ `FindError::MetadataKey` على المفتاح المفقود للإزالة. الكود: `crates/iroha_data_model/src/isi/transparent.rs` والمنفذ يتضمن كل هدف.

### الأذونات والأدوار: منح / إلغاء
الأنواع: `Grant<O, D>` و`Revoke<O, D>`، مع التعدادات المعبأة لـ `Permission`/`Role` إلى/من `Account`، و`Permission` إلى/من `Role`.- منح الإذن للحساب: يضيف `Permission` ما لم يكن متأصلًا بالفعل. الأحداث: `AccountEvent::PermissionAdded`. الأخطاء: `Repetition(Grant, Permission)` إذا كانت مكررة. الكود: `core/.../isi/account.rs`.
- إلغاء الإذن من الحساب: تتم إزالته إذا كان موجودًا. الأحداث: `AccountEvent::PermissionRemoved`. الأخطاء: `FindError::Permission` في حالة الغياب. الكود: `core/.../isi/account.rs`.
- منح الدور للحساب: يُدرج تعيين `(account, role)` في حالة عدم وجوده. الأحداث: `AccountEvent::RoleGranted`. الأخطاء: `Repetition(Grant, RoleId)`. الكود: `core/.../isi/account.rs`.
- إبطال الدور من الحساب: إزالة التعيين إذا كان موجودًا. الأحداث: `AccountEvent::RoleRevoked`. الأخطاء: `FindError::Role` في حالة الغياب. الرمز: `core/.../isi/account.rs`.
- منح الإذن للدور: إعادة بناء الدور مع إضافة الإذن. الأحداث: `RoleEvent::PermissionAdded`. الأخطاء: `Repetition(Grant, Permission)`. الكود: `core/.../isi/world.rs`.
- إبطال الإذن من الدور: إعادة بناء الدور دون هذا الإذن. الأحداث: `RoleEvent::PermissionRemoved`. الأخطاء: `FindError::Permission` في حالة الغياب. الرمز: `core/.../isi/world.rs`.### المشغلات: تنفيذ
النوع: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- السلوك: يدرج `ExecuteTriggerEvent { trigger_id, authority, args }` لنظام التشغيل الفرعي. يُسمح بالتنفيذ اليدوي فقط لمشغلات النداءات الجانبية (مرشح `ExecuteTrigger`)؛ يجب أن يتطابق عامل التصفية ويجب أن يكون المتصل هو سلطة إجراء التشغيل أو يحمل `CanExecuteTrigger` لتلك السلطة. عندما يكون المنفذ المقدم من المستخدم نشطًا، يتم التحقق من صحة تنفيذ التشغيل بواسطة منفذ وقت التشغيل ويستهلك ميزانية وقود منفذ المعاملة (الأساس `executor.fuel` بالإضافة إلى بيانات التعريف الاختيارية `additional_fuel`).
- الأخطاء: `FindError::Trigger` إذا لم تكن مسجلة؛ `InvariantViolation` إذا تم استدعاؤه من قبل جهة غير مخولة. الرمز: `core/.../isi/triggers/mod.rs` (والاختبارات في `core/.../smartcontracts/isi/mod.rs`).

### الترقية والتسجيل
- `Upgrade { executor }`: لترحيل المنفذ باستخدام الرمز الثانوي `Executor` المقدم، وتحديث المنفذ ونموذج البيانات الخاص به، وإصدار `ExecutorEvent::Upgraded`. الأخطاء: ملفوفة كـ `InvalidParameterError::SmartContract` عند فشل الترحيل. الرمز: `core/.../isi/world.rs`.
- `Log { level, msg }`: يصدر سجل العقدة بالمستوى المحدد؛ لا تغييرات الدولة. الكود: `core/.../isi/world.rs`.

### نموذج الخطأ
المغلف الشائع: `InstructionExecutionError` مع متغيرات لأخطاء التقييم، وفشل الاستعلام، والتحويلات، ولم يتم العثور على الكيان، والتكرار، وقابلية التعدين، والرياضيات، والمعلمة غير الصالحة، والانتهاك الثابت. التعدادات والمساعدات موجودة في `crates/iroha_data_model/src/isi/mod.rs` ضمن `pub mod error`.

---## المعاملات والملفات التنفيذية
- `Executable`: إما `Instructions(ConstVec<InstructionBox>)` أو `Ivm(IvmBytecode)`؛ يتم إجراء تسلسل bytecode كـ base64. الرمز: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: إنشاء ملف قابل للتنفيذ مع بيانات التعريف وتوقيعه وحزمه، `chain_id`، `authority`، `creation_time_ms`، `ttl_ms` الاختياري، و `nonce`. الرمز: `crates/iroha_data_model/src/transaction/`.
- في وقت التشغيل، يقوم `iroha_core` بتنفيذ دفعات `InstructionBox` عبر `Execute for InstructionBox`، مع الانتقال إلى `*Box` المناسب أو التعليمات الملموسة. الرمز: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- ميزانية التحقق من صحة منفذ التنفيذ في وقت التشغيل (المنفذ المقدم من المستخدم): قاعدة `executor.fuel` من المعلمات بالإضافة إلى بيانات تعريف المعاملة الاختيارية `additional_fuel` (`u64`)، المشتركة عبر عمليات التحقق من صحة التعليمات/المشغلات داخل المعاملة.

---## الثوابت والملاحظات (من الاختبارات والحراس)
- حماية التكوين: لا يمكن تسجيل المجال `genesis` أو الحسابات في المجال `genesis`؛ لا يمكن تسجيل حساب `genesis`. الكود/الاختبارات: `core/.../isi/world.rs`، `core/.../smartcontracts/isi/mod.rs`.
- يجب أن تستوفي الأصول الرقمية `NumericSpec` عند النعناع/النقل/النسخ؛ يؤدي عدم تطابق المواصفات إلى `TypeError::AssetNumericSpec`.
- قابلية التعدين: `Once` يسمح بطبقة نعناع واحدة ثم يقلب إلى `Not`؛ يسمح `Limited(n)` بالضبط بالنعناع `n` قبل التقليب إلى `Not`. تؤدي محاولات منع سك العملة على `Infinitely` إلى حدوث `MintabilityError::ForbidMintOnMintable`، وينتج عن تكوين `Limited(0)` `MintabilityError::InvalidMintabilityTokens`.
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
  - يتم تحديث `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` ويصدر `ConfigurationEvent::Changed`.CLI / Torii asset-definition id + أمثلة على الأسماء المستعارة:
- التسجيل باستخدام المساعدة الأساسية + الاسم الصريح + الاسم المستعار الطويل:
  -`iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- التسجيل باستخدام المساعدة الأساسية + الاسم الصريح + الاسم المستعار القصير:
  -`iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- النعناع بالاسم المستعار + مكونات الحساب:
  -`iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- حل الاسم المستعار للمساعدة الأساسية:
  - `POST /v1/assets/aliases/resolve` مع JSON `{ "alias": "pkr#ubl.sbp" }`

مذكرة الهجرة:
- معرفات تعريف الأصول النصية `name#domain` غير مدعومة عمدًا في الإصدار الأول.
- تظل معرفات الأصول عند حدود النعناع/الحرق/النقل أساسية `norito:<hex>`؛ استخدم `iroha tools encode asset-id` مع `--definition <base58-asset-definition-id>` أو `--alias ...` بالإضافة إلى `--account`.

---

## إمكانية التتبع (مصادر مختارة)
 - نواة نموذج البيانات: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - تعريفات ISI والتسجيل: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - تنفيذ ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - الأحداث: `crates/iroha_data_model/src/events/**`.
 - المعاملات: `crates/iroha_data_model/src/transaction/**`.

إذا كنت تريد توسيع هذه المواصفات إلى جدول سلوك/واجهة برمجة تطبيقات معروضة أو ربطها بكل حدث/خطأ ملموس، قل الكلمة وسأقوم بتوسيعها.