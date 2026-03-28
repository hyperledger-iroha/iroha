---
lang: ar
direction: rtl
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8337416254dfc062c40d691f6b35f7ee5818a1071279142bff75a74b75c0a802
source_last_modified: "2026-03-27T19:05:03.382221+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

# Iroha v2 نموذج البيانات - الغوص العميق

يشرح هذا المستند الهياكل والمعرفات والسمات والبروتوكولات التي تشكل نموذج بيانات Iroha v2، كما تم تنفيذها في صندوق `iroha_data_model` واستخدامها عبر مساحة العمل. ومن المفترض أن يكون مرجعًا دقيقًا يمكنك مراجعته واقتراح التحديثات عليه.

## النطاق والأسس

- الغرض: توفير أنواع أساسية لكائنات المجال (المجالات والحسابات والأصول وNFTs والأدوار والأذونات والأقران) وتعليمات تغيير الحالة (ISI) والاستعلامات والمشغلات والمعاملات والكتل والمعلمات.
- التسلسل: تستمد كافة الأنواع العامة برامج الترميز Norito (`norito::codec::{Encode, Decode}`) والمخطط (`iroha_schema::IntoSchema`). يتم استخدام JSON بشكل انتقائي (على سبيل المثال، لحمولات HTTP و`Json`) خلف إشارات الميزات.
- ملاحظة IVM: يتم تعطيل بعض عمليات التحقق من صحة وقت إلغاء التسلسل عند استهداف الجهاز الظاهري Iroha (IVM)، حيث يقوم المضيف بإجراء التحقق من الصحة قبل استدعاء العقود (راجع مستندات الصناديق في `src/lib.rs`).
- بوابات FFI: يتم شرح بعض الأنواع بشكل مشروط لـ FFI عبر `iroha_ffi` خلف `ffi_export`/`ffi_import` لتجنب الحمل الزائد عندما لا تكون هناك حاجة إلى FFI.

## السمات الأساسية والمساعدين- `Identifiable`: للكيانات `Id` و`fn id(&self) -> &Self::Id` المستقر. يجب أن يتم اشتقاقها باستخدام `IdEqOrdHash` لسهولة الخريطة/المجموعة.
- `Registrable`/`Registered`: تستخدم العديد من الكيانات (على سبيل المثال، `Domain`، `AssetDefinition`، `Role`) نمط الإنشاء. يربط `Registered` نوع وقت التشغيل بنوع منشئ خفيف الوزن (`With`) مناسب لمعاملات التسجيل.
- `HasMetadata`: الوصول الموحد إلى خريطة المفتاح/القيمة `Metadata`.
- `IntoKeyValue`: مساعد تقسيم التخزين لتخزين `Key` (المعرف) و`Value` (البيانات) بشكل منفصل لتقليل التكرار.
- `Owned<T>`/`Ref<'world, K, V>`: أغلفة خفيفة الوزن تستخدم في المخازن ومرشحات الاستعلام لتجنب النسخ غير الضرورية.

## الأسماء والمعرفات- `Name`: معرف نصي صالح. لا يسمح بالمسافات البيضاء والأحرف المحجوزة `@`، `#`، `$` (المستخدمة في المعرفات المركبة). قابلة للإنشاء عبر `FromStr` مع التحقق من الصحة. تتم تسوية الأسماء إلى Unicode NFC عند التحليل (يتم التعامل مع التهجئة المكافئة قانونيًا على أنها متطابقة ومخزنة). الاسم الخاص `genesis` محجوز (تم تحديده بشكل غير حساس لحالة الأحرف).
- `IdBox`: مظروف من النوع الإجمالي لأي معرف معتمد (`DomainId`، `AccountId`، `AssetDefinitionId`، `AssetId`، `NftId`، `PeerId`، `TriggerId`، `RoleId`، `Permission`، `CustomParameterId`). مفيد للتدفقات العامة وترميز Norito كنوع واحد.
- `ChainId`: معرف سلسلة غير شفاف يستخدم لحماية إعادة التشغيل في المعاملات.نماذج سلسلة من المعرفات (قابلة للتعثر مع `Display`/`FromStr`):
- `DomainId`: `name` (على سبيل المثال، `wonderland`).
- `AccountId`: معرف الحساب الأساسي بدون نطاق المشفر عبر `AccountAddress` كـ I105 فقط. يجب أن تكون مدخلات المحلل اللغوي الصارمة هي I105 الأساسية؛ يتم رفض لاحقات المجال (`@domain`)، والحرفية للاسم المستعار للحساب، وإدخال المحلل اللغوي السداسي الأساسي، وحمولات `norito:` القديمة، ونماذج المحلل اللغوي للحساب `uaid:`/`opaque:`. تستخدم الأسماء المستعارة للحسابات على السلسلة `name@domain.dataspace` أو `name@dataspace` وتتوافق مع قيم `AccountId` الأساسية.
- `AssetDefinitionId`: عنوان Base58 الأساسي غير البادئ على وحدات البايت الأساسية لتعريف الأصول. هذا هو معرف الأصول العامة. تستخدم الأسماء المستعارة للأصول على السلسلة `name#domain.dataspace` أو `name#dataspace` وتحل فقط مع معرف الأصل Base58 الأساسي هذا.
- `AssetId`: معرف الأصول العامة في نموذج Base58 الأساسي. يتم تحويل الأسماء المستعارة للأصول مثل `name#dataspace` أو `name#domain.dataspace` إلى `AssetId`. قد تعرض مقتنيات دفتر الأستاذ الداخلي أيضًا حقول `asset + account + optional dataspace` المقسمة عند الحاجة، ولكن هذا الشكل المركب ليس هو `AssetId` العام.
- `NftId`: `nft$domain` (على سبيل المثال، `rose$garden`).
- `PeerId`: `public_key` (تتم المساواة بين الأقران عن طريق المفتاح العام).

## الكيانات

### المجال
- `DomainId { name: Name }` – اسم فريد.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- المنشئ: `NewDomain` مع `with_logo`، `with_metadata`، ثم `Registrable::build(authority)` يعين `owned_by`.

### الحساب
- `AccountId` هي هوية الحساب الأساسية بدون مجال والتي يتم مفتاحها بواسطة وحدة التحكم والمشفرة كـ I105 الأساسية.
- يحمل `ScopedAccountId { account: AccountId, domain: DomainId }` سياق المجال الصريح فقط عندما يكون العرض محدد النطاق مطلوبًا.
- `Account { id, metadata, label?, uaid?, linked_domains? }` — `label` هو اسم مستعار ثابت اختياري تستخدمه سجلات إعادة المفتاح، ويحمل `uaid` نطاق Nexus الاختياري [معرف الحساب العالمي] (./universal_accounts_guide.md)، و`linked_domains` هو حالة فهرس مشتقة وليس جزءًا من الهوية الكنسية.
- بناة:
  - `NewAccount` عبر `Account::new(scoped_id)` يحقق تسجيلًا صريحًا مرتبطًا بالمجال وبالتالي يتطلب `ScopedAccountId`.
  - يقوم `NewAccount` عبر `Account::new_domainless(id)` بتسجيل موضوع الحساب العالمي فقط بدون مجال مرتبط.
- نموذج الاسم المستعار:
  - لا تشتمل هوية الحساب Canonical مطلقًا على مجال أو مقطع مساحة بيانات.
  - الأسماء المستعارة للحساب هي روابط منفصلة لتسمية الحساب/SNS موضوعة أعلى `AccountId`.
  - الأسماء المستعارة المؤهلة للمجال مثل `merchant@hbl.sbp` تحمل كلاً من المجال ومساحة البيانات في ربط الاسم المستعار.
  - الأسماء المستعارة لجذر مساحة البيانات مثل `merchant@sbp` تحمل مساحة البيانات فقط وبالتالي تقترن بشكل طبيعي مع `Account::new_domainless(...)`.
  - يجب أن تقوم الاختبارات والتركيبات بزرع `AccountId` العالمي أولاً، ثم إضافة روابط المجال وعقود إيجار الاسم المستعار وأذونات الاسم المستعار بشكل منفصل بدلاً من تشفير افتراضات المجال في هوية الحساب نفسها.

### تعريفات الأصول والأصول
- `AssetDefinitionId { aid_bytes: [u8; 16] }` معروض نصيًا كعنوان Base58 غير مسبوق مع الإصدار والمجموع الاختباري.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `name` مطلوب نص عرض ذو واجهة بشرية ويجب ألا يحتوي على `#`/`@`.
  - `alias` اختياري ويجب أن يكون واحدًا مما يلي:
    -`<name>#<domain>.<dataspace>`
    -`<name>#<dataspace>`
    مع الجزء الأيسر المطابق تمامًا لـ `AssetDefinition.name`.
  - يتم تخزين حالة تأجير الاسم المستعار بشكل مخوّل في سجل ربط الاسم المستعار المستمر؛ يتم اشتقاق الحقل `alias` المضمن عند إعادة قراءة التعريفات من خلال واجهات برمجة التطبيقات الأساسية/Torii.
  - قد تتضمن استجابات تعريف الأصول Torii `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`، حيث يكون `status` واحدًا من `permanent`، أو `leased_active`، أو `leased_grace`، أو `expired_pending_cleanup`.
  - يستخدم حل الاسم المستعار أحدث طابع زمني للكتلة المخصصة بدلاً من ساعة حائط العقدة. بمجرد مرور `grace_until_ms`، تتوقف محددات الاسم المستعار عن الحل فورًا حتى لو لم تقم عملية التنظيف بإزالة الارتباط القديم بعد؛ قد تستمر قراءات التعريف المباشر في الإبلاغ عن الارتباط العالق كـ `expired_pending_cleanup`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - الإنشاءات: `AssetDefinition::new(id, spec)` أو `numeric(id)` الملائم؛ مطلوب `name` ويجب تعيينه عبر `.with_name(...)`.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` مع `AssetEntry`/`AssetValue` سهل التخزين.- `AssetBalanceScope`: `Global` للأرصدة غير المقيدة و`Dataspace(DataSpaceId)` للأرصدة المقيدة بمساحة البيانات.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` مكشوف لواجهات برمجة التطبيقات الموجزة.

#

## إن إف تي
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (المحتوى عبارة عن بيانات تعريف مفتاح/قيمة عشوائية).
- المنشئ: `NewNft` عبر `Nft::new(id, content)`.

#

## الأدوار والأذونات
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` مع المنشئ `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` - يجب أن يتوافق `name` ومخطط الحمولة مع `ExecutorDataModel` النشط (انظر أدناه).

#

## أقرانهم
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` وشكل السلسلة `public_key@address` القابل للتحليل.

#

## أساسيات التشفير (الميزة `sm`)
- `Sm2PublicKey` و`Sm2Signature`: النقاط المتوافقة مع SEC1 وتوقيعات `r∥s` ذات العرض الثابت لـ SM2. يتحقق المنشئون من صحة عضوية المنحنى والمعرفات المميزة؛ يعكس ترميز Norito التمثيل الأساسي الذي يستخدمه `iroha_crypto`.
- `Sm3Hash`: `[u8; 32]` النوع الجديد الذي يمثل ملخص GM/T 0004، المستخدم في البيانات والقياس عن بعد واستجابات syscall.
- `Sm4Key`: غلاف مفاتيح متماثل 128 بت مشترك بين مكالمات النظام المضيفة وتركيبات نموذج البيانات.
توجد هذه الأنواع جنبًا إلى جنب مع عناصر Ed25519/BLS/ML-DSA الأولية الحالية وتصبح جزءًا من المخطط العام بمجرد إنشاء مساحة العمل باستخدام `--features sm`.

### المشغلات والأحداث
- `TriggerId { name: Name }` و`Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` أو `Exactly(u32)`؛ وشملت المرافق طلب واستنفاد.
  - السلامة: لا يمكن استخدام `TriggerCompleted` كمرشح للإجراء (يتم التحقق من صحته أثناء (إلغاء) التسلسل).
- `EventBox`: نوع المجموع لخط الأنابيب، ودفعة خط الأنابيب، والبيانات، والوقت، ومشغل التنفيذ، وأحداث المشغل المكتملة؛ يعكس `EventFilterBox` ذلك بالنسبة للاشتراكات وعوامل التصفية.

## المعلمات والتكوين

- عائلات معلمات النظام (جميع `Default`ed، وتحمل الحروف، وتحويلها إلى تعدادات فردية):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  -`BlockParameters { max_transactions: NonZeroU64 }`.
  -`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  -`SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` يجمع كل العائلات و`custom: BTreeMap<CustomParameterId, CustomParameter>`.
- التعدادات ذات المعلمة الواحدة: `SumeragiParameter`، `BlockParameter`، `TransactionParameter`، `SmartContractParameter` للتحديثات والتكرارات المشابهة للفرق.
- المعلمات المخصصة: محددة من قبل المنفذ، ويتم حملها كـ `Json`، ويتم تعريفها بواسطة `CustomParameterId` (a `Name`).

## ISI (Iroha تعليمات خاصة)- السمة الأساسية: `Instruction` مع `dyn_encode`، و`as_any`، ومعرف ثابت لكل نوع `id()` (الاسم الافتراضي هو اسم النوع المحدد). جميع التعليمات هي `Send + Sync + 'static`.
- `InstructionBox`: غلاف `Box<dyn Instruction>` المملوك مع النسخ/المعادل/ord الذي يتم تنفيذه عبر معرف النوع + البايتات المشفرة.
- يتم تنظيم عائلات التعليمات المدمجة تحت:
  - `mint_burn`، و`transfer`، و`register`، ومجموعة `transparent` من المساعدين.
  - اكتب التعدادات لتدفقات التعريف: `InstructionType`، والمجاميع المعبأة مثل `SetKeyValueBox` (domain/account/asset_def/nft/trigger).
- الأخطاء: نموذج الأخطاء الغني ضمن `isi::error` (أخطاء نوع التقييم، العثور على الأخطاء، قابلية التعدين، الرياضيات، المعلمات غير الصالحة، التكرار، الثوابت).
- تسجيل التعليمات: يقوم الماكرو `instruction_registry!{ ... }` بإنشاء سجل فك تشفير وقت التشغيل مرتبطًا بنوع الاسم. يتم استخدامه بواسطة استنساخ `InstructionBox` وNorito serde لتحقيق التسلسل الديناميكي (إلغاء). إذا لم يتم تعيين أي سجل بشكل صريح عبر `set_instruction_registry(...)`، فسيتم تثبيت السجل الافتراضي المضمن مع ISI الأساسي بالكامل بتكاسل عند الاستخدام الأول للحفاظ على الثنائيات قوية.

## المعاملات- `Executable`: إما `Instructions(ConstVec<InstructionBox>)` أو `Ivm(IvmBytecode)`. يتم إجراء تسلسل `IvmBytecode` كـ base64 (نوع جديد شفاف فوق `Vec<u8>`).
- `TransactionBuilder`: إنشاء حمولة معاملة باستخدام `chain`، و`authority`، و`creation_time_ms`، و`time_to_live_ms` الاختياري و`nonce`، و`metadata`، و `Executable`.
  - المساعدون: `with_instructions`، `with_bytecode`، `with_executable`، `with_metadata`، `set_nonce`، `set_ttl`، `set_creation_time`، `sign`.
- `SignedTransaction` (الإصدار مع `iroha_version`): يحمل `TransactionSignature` والحمولة؛ يوفر التجزئة والتحقق من التوقيع.
- المداخل والنتائج:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` مع مساعدات التجزئة.
  - `ExecutionStep(ConstVec<InstructionBox>)`: دفعة واحدة مرتبة من التعليمات في المعاملة.

## كتل- يحتوي `SignedBlock` (الإصدار) على ما يلي:
  - `signatures: BTreeSet<BlockSignature>` (من المدققين)،
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`،
  - `result: BlockResult` (حالة التنفيذ الثانوية) التي تحتوي على `time_triggers`، وأشجار Merkle للإدخال/النتيجة، و`transaction_results`، و`fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- المرافق: `presigned`، `set_transaction_results(...)`، `set_transaction_results_with_transcripts(...)`، `header()`، `signatures()`، `hash()`، `add_signature`، `replace_signatures`.
- جذور Merkle: يتم الالتزام بنقاط دخول المعاملات والنتائج عبر أشجار Merkle؛ النتيجة يتم وضع جذر Merkle في رأس الكتلة.
- تعرض إثباتات تضمين الكتلة (`BlockProofs`) كلاً من إثباتات Merkle للإدخال/النتيجة وخريطة `fastpq_transcripts` حتى يتمكن المثبتون خارج السلسلة من جلب دلتا النقل المرتبطة بتجزئة المعاملة.
- رسائل `ExecWitness` (التي يتم بثها عبر Torii والمدعومة بالإجماع) تتضمن الآن كلاً من `fastpq_transcripts` و`fastpq_batches: Vec<FastpqTransitionBatch>` الجاهز للإثبات مع `public_inputs` المضمن (dsid، فتحة، جذور، perm_root، tx_set_hash)، حتى يتمكن المثبتون الخارجيون من استيعاب صفوف FASTPQ الأساسية دون إعادة تشفير النصوص.

## الاستعلامات- نكهتين:
  - المفرد: تنفيذ `SingularQuery<Output>` (على سبيل المثال، `FindParameters`، `FindExecutorDataModel`).
  - قابل للتكرار: تنفيذ `Query<Item>` (على سبيل المثال، `FindAccounts`، `FindAssets`، `FindDomains`، وما إلى ذلك).
- النماذج الممحاة بالنوع:
  - `QueryBox<T>` عبارة عن `Query<Item = T>` محاصر وممسوح مع Norito serde مدعوم بسجل عالمي.
  - `QueryWithFilter<T> { query, predicate, selector }` يقرن استعلامًا بمسند/محدد DSL؛ يتحول إلى استعلام قابل للتكرار تم محوه عبر `From`.
- التسجيل والترميز:
  - يقوم `query_registry!{ ... }` بإنشاء سجل عالمي لتعيين أنواع الاستعلام الملموسة للمنشئين حسب اسم النوع لفك التشفير الديناميكي.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` و`QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` هو نوع مجموع على ناقلات متجانسة (على سبيل المثال، `Vec<Account>`، `Vec<Name>`، `Vec<AssetDefinition>`، `Vec<BlockHeader>`)، بالإضافة إلى مساعدات الصفوف والامتداد لترقيم الصفحات بكفاءة.
- DSL: تم تنفيذه في `query::dsl` مع سمات الإسقاط (`HasProjection<PredicateMarker>` / `SelectorMarker`) للمسندات والمحددات التي تم التحقق منها في وقت الترجمة. تعرض ميزة `fast_dsl` متغيرًا أخف إذا لزم الأمر.

## المنفذ والقابلية للتوسعة- `Executor { bytecode: IvmBytecode }`: حزمة التعليمات البرمجية التي ينفذها المدقق.
- يعلن `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` عن المجال المحدد من قبل المنفذ:
  - معلمات التكوين المخصصة،
  - معرفات التعليمات المخصصة،
  - معرفات رمز الإذن،
  - مخطط JSON يصف الأنواع المخصصة لأدوات العميل.
- توجد نماذج التخصيص ضمن `data_model/samples/executor_custom_data_model` مما يوضح:
  - رمز إذن مخصص عبر اشتقاق `iroha_executor_data_model::permission::Permission`،
  - تم تعريف المعلمة المخصصة كنوع قابل للتحويل إلى `CustomParameter`،
  - تعليمات مخصصة متسلسلة في `CustomInstruction` للتنفيذ.

#

## تعليمات مخصصة (ISI المعرفة من قبل المنفذ)- النوع: `isi::CustomInstruction { payload: Json }` مع معرف السلك الثابت `"iroha.custom"`.
- الغرض: مظروف للتعليمات الخاصة بالمنفذ في الشبكات الخاصة/شبكات الاتحاد أو للنماذج الأولية، دون تفرع نموذج البيانات العامة.
- سلوك المنفذ الافتراضي: لا ينفذ المنفذ المضمن في `iroha_core` `CustomInstruction` وسيصاب بالذعر إذا تمت مواجهته. يجب أن يقوم المنفذ المخصص بخفض `InstructionBox` إلى `CustomInstruction` وتفسير الحمولة النافعة على كافة أدوات التحقق من الصحة بشكل حتمي.
- Norito: يتم التشفير/فك التشفير عبر `norito::codec::{Encode, Decode}` مع تضمين المخطط؛ يتم إجراء تسلسل للحمولة `Json` بشكل حتمي. تكون الرحلات ذهابًا وإيابًا مستقرة طالما أن سجل التعليمات يتضمن `CustomInstruction` (وهو جزء من السجل الافتراضي).
- IVM: يتم تجميع Kotodama إلى الكود الثانوي IVM (`.to`) وهو المسار الموصى به لمنطق التطبيق. استخدم `CustomInstruction` فقط للامتدادات على مستوى المنفذ والتي لا يمكن التعبير عنها بعد في Kotodama. ضمان الحتمية والثنائيات المنفذة المتطابقة عبر الأقران.
- ليس للشبكات العامة: لا تستخدم للسلاسل العامة حيث يخاطر المنفذون غير المتجانسون بشوك الإجماع. تفضل اقتراح ISI المضمن الجديد عند الحاجة إلى ميزات النظام الأساسي.

## البيانات الوصفية- `Metadata(BTreeMap<Name, Json>)`: مخزن المفتاح/القيمة المرفق بكيانات متعددة (`Domain`، `Account`، `AssetDefinition`، `Nft`، المشغلات، والمعاملات).
- واجهة برمجة التطبيقات: `contains`، و`iter`، و`get`، و`insert`، و(مع `transparent_api`) `remove`.

## الميزات والحتمية

- ميزات التحكم في واجهات برمجة التطبيقات الاختيارية (`std`، `json`، `transparent_api`، `ffi_export`، `ffi_import`، `fast_dsl`، `http`، `fault_injection`).
- الحتمية: تستخدم جميع عمليات التسلسل ترميز Norito لتكون محمولة عبر الأجهزة. الرمز الثانوي IVM هو عبارة عن فقاعة بايت غير شفافة؛ يجب ألا يقدم التنفيذ تخفيضات غير حتمية. يقوم المضيف بالتحقق من صحة المعاملات ويوفر المدخلات إلى IVM بشكل حتمي.

#

## واجهة برمجة التطبيقات الشفافة (`transparent_api`)- الغرض: الكشف عن الوصول الكامل والقابل للتغيير إلى بنيات/تعدادات `#[model]` للمكونات الداخلية مثل Torii والمنفذين واختبارات التكامل. بدونها، تكون هذه العناصر غير شفافة عن قصد، لذا لا ترى حزم SDK الخارجية سوى المنشئات الآمنة والحمولات المشفرة.
- الميكانيكا: يعيد الماكرو `iroha_data_model_derive::model` كتابة كل حقل عام باستخدام `#[cfg(feature = "transparent_api")] pub` ويحتفظ بنسخة خاصة للإنشاء الافتراضي. يؤدي تمكين الميزة إلى قلب تلك cfgs، لذا فإن تدمير `Account`، و`Domain`، و`Asset`، وما إلى ذلك، يصبح قانونيًا خارج الوحدات النمطية المحددة الخاصة بها.
- الكشف عن السطح: يقوم الصندوق بتصدير ثابت `TRANSPARENT_API: bool` (الذي تم إنشاؤه إما إلى `transparent_api.rs` أو `non_transparent_api.rs`). يمكن للكود المصب التحقق من هذه العلامة والفرع عندما يحتاج إلى الرجوع إلى المساعدين غير الشفافين.
- التمكين: أضف `features = ["transparent_api"]` إلى التبعية في `Cargo.toml`. تقوم صناديق مساحة العمل التي تحتاج إلى إسقاط JSON (على سبيل المثال، `iroha_torii`) بإعادة توجيه العلامة تلقائيًا، ولكن يجب على مستهلكي الطرف الثالث إيقاف تشغيلها ما لم يتحكموا في النشر ويقبلوا سطح واجهة برمجة التطبيقات الأوسع.

## أمثلة سريعة

أنشئ نطاقًا وحسابًا، وحدد أحد الأصول، ثم أنشئ معاملة باستخدام التعليمات:

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
let asset_def_id = AssetDefinitionId::new(
    "wonderland".parse().unwrap(),
    "usd".parse().unwrap(),
);
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

الاستعلام عن الحسابات والأصول باستخدام DSL:

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

استخدم الرمز الثانوي للعقد الذكي IVM:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

معرف تعريف الأصل / المرجع السريع للاسم المستعار (CLI + Torii):

```bash
# Register an asset definition with a canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to the canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```مذكرة الهجرة:
- لا يتم قبول معرفات تعريف الأصول `name#domain` القديمة في الإصدار 1.
- تستخدم محددات الأصول العامة تنسيقًا واحدًا لتعريف الأصول فقط: معرفات Base58 الأساسية. تظل الأسماء المستعارة محددات اختيارية، ولكنها تتوافق مع نفس المعرف الأساسي.
- تعالج عمليات البحث عن الأصول العامة الأرصدة المملوكة باستخدام `asset + account + optional scope`؛ تعد القيم الحرفية `AssetId` المشفرة الخام تمثيلًا داخليًا وليست جزءًا من سطح محدد Torii/CLI.
- يقبل `POST /v1/assets/definitions/query` و`GET /v1/assets/definitions` عوامل تصفية/فرز تعريف الأصول عبر `alias_binding.status`، و`alias_binding.lease_expiry_ms`، و`alias_binding.grace_until_ms`، و`alias_binding.bound_at_ms` بالإضافة إلى `id`، `name`، و`alias`، و`metadata.*`.

## الإصدار

- `SignedTransaction`، و`SignedBlock`، و`SignedQuery` هي بنيات أساسية مشفرة بـ Norito. يقوم كل منهم بتنفيذ `iroha_version::Version` لبادئة الحمولة الخاصة بهم بإصدار ABI الحالي (حاليًا `1`) عند تشفيرها عبر `EncodeVersioned`.

## ملاحظات المراجعة / التحديثات المحتملة

- الاستعلام عن DSL: فكر في توثيق مجموعة فرعية مستقرة تواجه المستخدم وأمثلة للمرشحات/المحددات الشائعة.
- عائلات التعليمات: قم بتوسيع المستندات العامة التي تسرد متغيرات ISI المضمنة التي تم الكشف عنها بواسطة `mint_burn`، `register`، `transfer`.

---
إذا كان أي جزء يحتاج إلى مزيد من التعمق (على سبيل المثال، كتالوج ISI الكامل، أو قائمة تسجيل الاستعلام الكاملة، أو حقول رأس الكتلة)، فأخبرني وسأقوم بتوسيع هذه الأقسام وفقًا لذلك.