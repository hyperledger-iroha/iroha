---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-spec
title: المواصفات التقنية لسورا نيكسس
description: مرآة كاملة لـ `docs/source/nexus.md` تغطي معمارية وقيود التصميم لدفتر Iroha 3 (Sora Nexus).
---

:::note المصدر الرسمي
تعكس هذه الصفحة `docs/source/nexus.md`. حافظ على النسختين متطابقتين حتى يصل تراكم الترجمات إلى البوابة.
:::

#! Iroha 3 - Sora Nexus Ledger: المواصفات التقنية للتصميم

يقترح هذا المستند معمارية Sora Nexus Ledger لـ Iroha 3، مع تطوير Iroha 2 إلى دفتر عالمي موحد منطقيا ومنظم حول Data Spaces (DS). توفر Data Spaces نطاقات خصوصية قوية ("private data spaces") ومشاركة مفتوحة ("public data spaces"). يحافظ التصميم على composability عبر الدفتر العالمي مع ضمان عزل صارم وسرية بيانات private-DS، ويقدم توسيع توفر البيانات عبر ترميز المحو في Kura (block storage) و WSV (World State View).

يبني نفس المستودع كلا من Iroha 2 (شبكات مستضافة ذاتيا) و Iroha 3 (SORA Nexus). يعتمد التنفيذ على Iroha Virtual Machine (IVM) المشتركة وسلسلة ادوات Kotodama، لذا تبقى العقود وقطع الـ bytecode قابلة للنقل بين النشر الذاتي والدفتر العالمي لنكسس.

الاهداف
- دفتر منطقي عالمي واحد مكون من العديد من المدققين المتعاونين و Data Spaces.
- Data Spaces خاصة للتشغيل المقيّد (مثل CBDC) مع بقاء البيانات داخل DS الخاص.
- Data Spaces عامة بمشاركة مفتوحة ووصول دون اذن على غرار Ethereum.
- عقود ذكية قابلة للتركيب عبر Data Spaces، مع صلاحيات صريحة للوصول إلى اصول private-DS.
- عزل الاداء بحيث لا تؤثر الحركة العامة على معاملات private-DS الداخلية.
- توفر بيانات على نطاق واسع: Kura و WSV بترميز محو لدعم بيانات شبه غير محدودة مع بقاء بيانات private-DS خاصة.

غير الاهداف (المرحلة الاولى)
- تعريف اقتصاديات التوكن او حوافز المدققين؛ سياسات scheduling و staking قابلة للتوصيل.
- إدخال اصدار ABI جديد؛ التغييرات تستهدف ABI v1 مع امتدادات syscalls و pointer-ABI وفق سياسة IVM.

المصطلحات
- Nexus Ledger: الدفتر المنطقي العالمي المتشكل من تركيب كتل Data Space (DS) في تاريخ مرتب واحد والتزام حالة.
- Data Space (DS): نطاق تنفيذ وتخزين محدود بمدققين خاصين وحوكمة وفئة خصوصية وسياسة DA وحصص وسياسة رسوم. فئتان: public DS و private DS.
- Private Data Space: مدققون باذونات وتحكم وصول؛ بيانات المعاملة والحالة لا تغادر DS. يتم تثبيت الالتزامات/البيانات الوصفية فقط عالميا.
- Public Data Space: مشاركة بدون اذن؛ البيانات الكاملة والحالة متاحة للجميع.
- Data Space Manifest (DS Manifest): manifest مشفّر عبر Norito يعلن معلمات DS (validators/QC keys، فئة الخصوصية، سياسة ISI، معلمات DA، الاحتفاظ، الحصص، سياسة ZK، الرسوم). يتم تثبيت hash الـ manifest على سلسلة nexus. ما لم يتم تجاوزه، تستخدم شهادات quorum الخاصة بـ DS مخطط توقيع ML-DSA-87 (فئة Dilithium5) كافتراضي post-quantum.
- Space Directory: عقد دليل عالمي على السلسلة يتتبع manifests DS والنسخ واحداث الحوكمة/الدوران لاغراض الاستدلال والتدقيق.
- DSID: معرف فريد عالميا لـ Data Space. يستخدم لعمل namespacing لكل الكائنات والمراجع.
- Anchor: التزام تشفيري من كتلة/راس DS يضم الى سلسلة nexus لربط تاريخ DS بالدفتر العالمي.
- Kura: تخزين كتل Iroha. ممتد هنا بتخزين blobs بترميز محو والتزامات.
- WSV: Iroha World State View. ممتد هنا بمقاطع حالة ذات نسخ ولقطات ومرمزة بالمحو.
- IVM: Iroha Virtual Machine لتنفيذ العقود الذكية (Kotodama bytecode `.to`).
  - AIR: Algebraic Intermediate Representation. تمثيل جبري للحساب من اجل ادلة نمط STARK يصف التنفيذ كسلاسل تعتمد على الحقول مع قيود انتقال وحدود.

نموذج Data Spaces
- الهوية: `DataSpaceId (DSID)` يعرّف DS ويضع namespacing لكل شيء. يمكن انشاء DS على درجتين:
  - Domain-DS: `ds::domain::<domain_name>` - تنفيذ وحالة ضمن نطاق domain.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - تنفيذ وحالة ضمن تعريف اصل واحد.
  الشكلان يتعايشان؛ ويمكن للمعاملات لمس عدة DSID بشكل ذري.
- دورة حياة manifest: تسجيل إنشاء DS والتحديثات (تدوير المفاتيح، تغييرات السياسة) والإنهاء في Space Directory. كل اثر DS لكل slot يشير إلى أحدث hash للـ manifest.
- الفئات: Public DS (مشاركة مفتوحة، DA عامة) و Private DS (مقيد، DA سرية). سياسات هجينة ممكنة عبر flags في manifest.
- السياسات لكل DS: صلاحيات ISI، معلمات DA `(k,m)`، تشفير، احتفاظ، حصص (حصة tx الدنيا/العليا لكل كتلة)، سياسة اثبات ZK/تفاؤلي، رسوم.
- الحوكمة: العضوية والدوران في DS يحددان في قسم الحوكمة بالـ manifest (مقترحات على السلسلة، multisig، او حوكمة خارجية مثبتة بمعاملات nexus و attestations).

Manifests القدرات و UAID
- الحسابات العالمية: يحصل كل مشارك على UAID حتمي (`UniversalAccountId` في `crates/iroha_data_model/src/nexus/manifest.rs`) يغطي جميع dataspaces. تربط manifests القدرات (`AssetPermissionManifest`) الـ UAID بـ dataspace محدد وفترات تفعيل/انتهاء وقائمة مرتبة من قواعد allow/deny `ManifestEntry` التي تقيد `dataspace` و `program_id` و `method` و `asset` وادوار AMX اختيارية. قواعد deny تفوز دائما؛ ويصدر المقيّم `ManifestVerdict::Denied` مع سبب تدقيق او منح `Allowed` مع بيانات allowance المطابقة.
- Allowances: يحمل كل allow bucket حتمي `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) مع `max_amount` اختياري. تستعمل hosts و SDK نفس حمولة Norito، لذا يكون التنفيذ متطابقا عبر العتاد و SDKs.
- Telemetry تدقيق: يبث Space Directory حدث `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) عند تغير حالة manifest. السطح الجديد `SpaceDirectoryEventFilter` يمكّن مشتركين Torii/data-event من مراقبة تحديثات UAID manifest والابطال وقرارات deny-wins دون اعداد مخصص.

لاجل ادلة تشغيل من طرف لطرف وملاحظات ترحيل SDK وقوائم نشر manifests، حافظ على هذا القسم متوافقا مع Universal Account Guide (`docs/source/universal_accounts_guide.md`). ابق الوثيقتين متطابقتين عند تغير سياسة UAID او الادوات.

المعمارية عالية المستوى
1) طبقة التركيب العالمية (Nexus Chain)
- تحافظ على ترتيب قانوني واحد لكتل Nexus كل 1 ثانية لتثبيت معاملات ذرية تمتد عبر Data Spaces (DS). كل معاملة ملتزمة تحدث world state الموحد (متجه roots لكل DS).
- تحتوي بيانات وصفية دنيا مع proofs/QCs مجمعة لضمان composability والنهائية وكشف الاحتيال (DSIDs المتأثرة، roots لكل DS قبل/بعد، التزامات DA، proofs صلاحية لكل DS، وشهادة quorum للـ DS باستخدام ML-DSA-87). لا تتضمن بيانات خاصة.
- الاجماع: لجنة BFT عالمية بPipeline من 22 عقدة (3f+1 مع f=7) مختارة عبر VRF/stake لكل حقبة من مجموعة حتى ~200k مدقق محتمل. لجنة nexus ترتب المعاملات وتؤكد الكتلة خلال 1s.

2) طبقة Data Space (Public/Private)
- تنفذ مقاطع المعاملات لكل DS، وتحدث WSV المحلي، وتنتج artifacts صلاحية لكل كتلة (proofs مجمعة لكل DS والتزامات DA) تتجمع في كتلة Nexus ذات 1s.
- Private DS تشفر البيانات في السكون والحركة بين المدققين المصرح لهم؛ ولا يغادر DS سوى الالتزامات و proofs صلاحية PQ.
- Public DS تصدر اجسام البيانات الكاملة (via DA) و proofs صلاحية PQ.

3) معاملات ذرية عبر Data Spaces (AMX)
- النموذج: كل معاملة مستخدم قد تمس عدة DS (مثل domain DS و asset DS). تلتزم ذرّيا في كتلة Nexus واحدة او تلغى؛ لا آثار جزئية.
- Prepare-Commit خلال 1s: لكل معاملة مرشحة، تنفذ DS المعنية بالتوازي على نفس snapshot (roots DS ببداية slot) وتنتج proofs صلاحية PQ لكل DS (FASTPQ-ISI) والتزامات DA. يلتزم comite nexus فقط اذا تحققت كل proofs المطلوبة ووصلت شهادات DA في الوقت (هدف <=300 ms)؛ والا تعاد جدولة المعاملة للـ slot التالي.
- الاتساق: مجموعات القراءة/الكتابة مصرح بها؛ يتم كشف التعارض عند الالتزام مقابل roots بداية slot. التنفيذ المتفائل بدون اقفال لكل DS يتجنب التعطيل العالمي؛ الذرية تفرض بقاعدة التزام nexus (الكل او لا شيء عبر DS).
- الخصوصية: تصدر private DS فقط proofs/commitments مرتبطة بـ roots قبل/بعد. لا تخرج بيانات خاصة خام.

4) توفر البيانات (DA) بترميز محو
- يخزن Kura اجسام الكتل ولقطات WSV كـ blobs بترميز محو. يتم توزيع blobs العامة على نطاق واسع؛ اما blobs الخاصة فتخزن فقط داخل مدققي private-DS مع chunks مشفرة.
- يتم تسجيل التزامات DA في artifacts الخاصة بـ DS وفي كتل Nexus، مما يمكّن من ضمانات sampling والاسترجاع دون كشف المحتوى الخاص.

هيكل الكتل والالتزام
- Artifact اثبات Data Space (لكل slot 1s ولكل DS)
  - الحقول: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - تصدر private-DS artifacts بدون اجسام بيانات؛ وpublic DS تسمح باسترجاع الاجسام عبر DA.

- Nexus Block (بمعدل 1s)
  - الحقول: block_number, parent_hash, slot_time, tx_list (معاملات ذرية cross-DS مع DSIDs), ds_artifacts[], nexus_qc.
  - الوظيفة: تثبيت جميع المعاملات الذرية التي تتحقق artifacts المطلوبة لها؛ تحديث متجه roots العالمي لكل DS في خطوة واحدة.

الاجماع و scheduling
- اجماع Nexus Chain: BFT عالمي متسلسل (فئة Sumeragi) مع لجنة 22 عقدة (3f+1 مع f=7) تستهدف كتل 1s ونهائية 1s. يتم اختيار اللجنة عبر VRF/stake لكل حقبة من ~200k مرشح؛ الدوران يحافظ على اللامركزية ومقاومة الرقابة.
- اجماع Data Space: يشغل كل DS BFT خاص به لانتاج artifacts لكل slot (proofs، التزامات DA، DS QC). يتم تحجيم لجان lane-relay الى `3f+1` باستخدام `fault_tolerance` للـ dataspace ويتم اختيارها بشكل حتمي لكل حقبة من مجموعة المدققين باستخدام VRF seed مرتبط بـ `(dataspace_id, lane_id)`. Private DS مقيدة؛ Public DS تسمح بالحيوية المفتوحة مع سياسات anti-Sybil. اللجنة العالمية nexus ثابتة.
- Scheduling المعاملات: يرسل المستخدمون معاملات ذرية مع DSIDs ومجموعات القراءة/الكتابة. تنفذ DS بالتوازي داخل slot؛ وتضم لجنة nexus المعاملة في كتلة 1s اذا تحقق جميع artifacts ووصلت شهادات DA في الوقت (<=300 ms).
- عزل الاداء: لكل DS mempools وتنفيذ مستقل. تقيد quotas لكل DS عدد المعاملات التي تلمس DS في كل كتلة لتجنب head-of-line blocking وحماية زمن private DS.

نموذج البيانات و namespacing
- DS-Qualified IDs: كل الكيانات (domains، accounts، assets، roles) مؤهلة بـ `dsid`. مثال: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global References: المرجع العالمي عبارة عن tuple `(dsid, object_id, version_hint)` ويمكن وضعه on-chain في طبقة nexus او في اوصاف AMX للاستخدام cross-DS.
- Norito Serialization: جميع رسائل cross-DS (اوصاف AMX، proofs) تستخدم Norito codecs. لا استخدام لـ serde في مسارات الانتاج.

العقود الذكية وامتدادات IVM
- سياق التنفيذ: اضافة `dsid` الى سياق تنفيذ IVM. عقود Kotodama تنفذ دائما ضمن Data Space محدد.
- primitive ذرية عبر DS:
  - `amx_begin()` / `amx_commit()` تحدد معاملة ذرية multi-DS في IVM host.
  - `amx_touch(dsid, key)` تعلن نية القراءة/الكتابة لكشف التعارض مقابل roots snapshot للـ slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (مسموح فقط اذا سمحت السياسة وكان handle صالحا)
- Asset handles والرسوم:
  - عمليات الاصول مصرح بها عبر سياسات ISI/role للـ DS؛ الرسوم تدفع في رمز gas الخاص بـ DS. يمكن اضافة capability tokens وسياسات اكثر ثراء (multi-approver, rate-limits, geofencing) لاحقا دون تغيير النموذج الذري.
- الحتمية: كل syscalls الجديدة نقية وحتمية عند المدخلات ومجموعات القراءة/الكتابة AMX المعلنة. لا تأثيرات خفية للوقت او البيئة.

اثباتات الصلاحية post-quantum (ISI معممة)
- FASTPQ-ISI (PQ بدون trusted setup): حجة hash-based تعمم تصميم transfer لكل عائلات ISI مع استهداف اثبات دون الثانية لدفعات بحجم 20k على عتاد فئة GPU.
  - ملف التشغيل:
    - عقد الانتاج تنشئ prover عبر `fastpq_prover::Prover::canonical` الذي يهيئ دائما backend الانتاج؛ تمت ازالة mock الحتمي. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) و `irohad --fastpq-execution-mode` يتيحان تثبيت تنفيذ CPU/GPU بشكل حتمي بينما يسجل observer hook ثلاثيات requested/resolved/backend للتدقيقات. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetization:
  - KV-Update AIR: يعامل WSV كخريطة key-value نوعية ملتزمة عبر Poseidon2-SMT. كل ISI يتوسع الى مجموعة صغيرة من صفوف read-check-write على المفاتيح (accounts, assets, roles, domains, metadata, supply).
  - قيود محكومة بالـ opcode: جدول AIR واحد مع اعمدة selector يفرض قواعد كل ISI (conservation, monotonic counters, permissions, range checks, تحديثات metadata محدودة).
  - Lookup arguments: جداول شفافة ملتزمة بالهاش لصلاحيات/ادوار وprecisions ومعلمات سياسة تتجنب قيود bitwise الثقيلة.
- التزامات وتحديثات الحالة:
  - Aggregated SMT Proof: جميع المفاتيح المتأثرة (pre/post) تثبت مقابل `old_root`/`new_root` باستخدام frontier مضغوط مع siblings مكررة تمت ازالتها.
  - Invariants: يتم فرض ثوابت عالمية (مثل اجمالي supply لكل اصل) عبر مساواة multiset بين صفوف التأثير والعدادات المتعقبة.
- نظام الاثبات:
  - التزامات متعددة الحدود بأسلوب FRI (DEEP-FRI) مع arity عالية (8/16) و blow-up 8-16؛ hashes Poseidon2؛ transcript Fiat-Shamir مع SHA-2/3.
  - Recursion اختيارية: تجميع محلي داخل DS لضغط micro-batches الى اثبات واحد لكل slot عند الحاجة.
- النطاق والامثلة:
  - الاصول: transfer, mint, burn, register/unregister asset definitions, set precision (مقيد), set metadata.
  - الحسابات/المجالات: create/remove, set key/threshold, add/remove signatories (حالة فقط؛ فحص التواقيع يثبت من مدققي DS وليس داخل AIR).
  - الادوار/الصلاحيات (ISI): grant/revoke roles و permissions؛ يتم فرضها عبر جداول lookup و checks سياسة monotonic.
  - العقود/AMX: علامات begin/commit للـ AMX، capability mint/revoke اذا كانت مفعلة؛ تثبت كتحولات حالة وعدادات سياسة.
- فحوصات خارج AIR للحفاظ على الكمون:
  - التواقيع والتشفير الثقيل (مثل تواقيع ML-DSA للمستخدمين) يتحقق منها مدققو DS ويثبتونها في DS QC؛ اثبات الصلاحية يغطي فقط اتساق الحالة والامتثال للسياسات. هذا يبقي proofs PQ وسريعة.
- اهداف الاداء (تقريبية، CPU 32 نواة + GPU حديثة):
  - 20k ISI مختلطة مع لمس مفاتيح صغير (<=8 keys/ISI): ~0.4-0.9 s اثبات، ~150-450 KB اثبات، ~5-15 ms تحقق.
  - ISI اثقل: micro-batch (مثلا 10x2k) + recursion للحفاظ على <1 s لكل slot.
- تهيئة DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (التواقيع تتحقق عبر DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (افتراضي؛ البدائل يجب اعلانها صراحة)
- Fallbacks:
  - ISI المعقدة/المخصصة يمكنها استخدام STARK عام (`zk.policy = "stark_fri_general"`) مع اثبات مؤجل ونهائية 1s عبر attestation QC + slashing على ادلة غير صحيحة.
  - خيارات غير PQ (مثل Plonk مع KZG) تتطلب trusted setup ولم تعد مدعومة في build الافتراضي.

AIR Primer (لـ Nexus)
- Trace التنفيذ: مصفوفة بعرض (اعمدة سجلات) وطول (خطوات). كل صف هو خطوة منطقية في معالجة ISI؛ الاعمدة تحمل قيم pre/post وselectors وflags.
- القيود:
  - قيود الانتقال: تفرض علاقات صف-الى-صف (مثلا post_balance = pre_balance - amount لصف خصم عند `sel_transfer = 1`).
  - قيود الحدود: تربط public I/O (old_root/new_root, counters) بالصف الاول/الاخير.
  - Lookups/permutations: تضمن العضوية والمساواة متعددة المجموعات مقابل جداول ملتزمة (permissions, asset params) دون دوائر bit-heavy.
- الالتزام والتحقق:
  - يقوم prover بالالتزام بالتراسات عبر ترميزات hash ويبني متعددات حدود منخفضة الدرجة صالحة اذا تحققت القيود.
  - يتحقق verifier من الدرجة المنخفضة عبر FRI (hash-based, post-quantum) مع فتحات Merkle قليلة؛ التكلفة لوغاريتمية بعدد الخطوات.
- مثال (Transfer): تتضمن السجلات pre_balance, amount, post_balance, nonce, selectors. تفرض القيود عدم السلبية/المدى، الحفظ، ورتابة nonce، بينما تربط multi-proof SMT مجمعة الاوراق pre/post بـ roots old/new.

تطور ABI و syscalls (ABI v1)
- Syscalls لاضافتها (اسماء توضيحية):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- انواع pointer-ABI لاضافتها:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- تحديثات مطلوبة:
  - اضافة الى `ivm::syscalls::abi_syscall_list()` (مع الحفاظ على الترتيب)، مع gating حسب السياسة.
  - ربط الارقام غير المعروفة بـ `VMError::UnknownSyscall` في hosts.
  - تحديث الاختبارات: syscall list golden، ABI hash، pointer type ID goldens، واختبارات السياسة.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

نموذج الخصوصية
- احتواء البيانات الخاصة: اجسام المعاملات وفروق الحالة ولقطات WSV الخاصة بـ private DS لا تغادر مجموعة المدققين الخاصة.
- التعرض العام: يتم تصدير headers والتزامات DA و proofs صلاحية PQ فقط.
- ادلة ZK اختيارية: يمكن لـ private DS انتاج ادلة ZK (مثل كفاية الرصيد او امتثال السياسة) لتمكين عمليات cross-DS دون كشف الحالة الداخلية.
- التحكم بالوصول: يتم فرض التخويل عبر سياسات ISI/role داخل DS. capability tokens اختيارية ويمكن اضافتها لاحقا.

عزل الاداء و QoS
- اجماع ومجموعات انتظار وتخزين منفصلة لكل DS.
- quotas لـ scheduling على مستوى DS في nexus لتقييد زمن ادراج anchors وتجنب head-of-line blocking.
- ميزانيات موارد العقود لكل DS (compute/memory/IO) يفرضها host IVM. التنافس في public DS لا يمكنه استهلاك ميزانيات private DS.
- استدعاءات cross-DS غير متزامنة تتجنب انتظار متزامن طويل داخل تنفيذ private-DS.

توفر البيانات وتصميم التخزين
1) ترميز المحو
- استخدام Reed-Solomon منهجي (مثلا GF(2^16)) لترميز المحو على مستوى blob لكتل Kura ولقطات WSV: معلمات `(k, m)` مع `n = k + m` shards.
- المعلمات الافتراضية (المقترحة، public DS): `k=32, m=16` (n=48) لاستعادة حتى 16 shard مفقود مع توسع ~1.5x. للـ private DS: `k=16, m=8` (n=24) ضمن المجموعة المصرح بها. كلاهما قابل للتهيئة عبر DS Manifest.
- Blobs العامة: shards موزعة عبر عقد DA/مدققين عديدة مع فحوص توفر بالعينات. التزامات DA في headers تسمح light clients بالتحقق.
- Blobs الخاصة: shards مشفرة وموزعة فقط بين مدققي private-DS (او حراس معينين). السلسلة العالمية تحمل فقط التزامات DA (دون مواقع shards او المفاتيح).

2) الالتزامات والـ sampling
- لكل blob: احسب Merkle root فوق shards وادرجه في `*_da_commitment`. حافظ على PQ بتجنب الالتزامات البيضوية.
- DA Attesters: attesters اقليميون يتم اختيارهم عبر VRF (مثلا 64 لكل منطقة) يصدرون شهادة ML-DSA-87 تؤكد sampling الناجح. الهدف لزمن attestation <=300 ms. لجنة nexus تتحقق من الشهادات بدلا من سحب shards.

3) تكامل Kura
- الكتل تخزن اجسام المعاملات كـ blobs بترميز محو مع التزامات Merkle.
- headers تحمل التزامات blobs؛ يمكن استرجاع الاجسام عبر شبكة DA للـ public DS وبقنوات خاصة للـ private DS.

4) تكامل WSV
- Snapshots WSV: بشكل دوري يتم checkpoint لحالة DS في snapshots مجزأة ومشفرة بالمحو مع التزامات مسجلة في headers. بين اللقطات يتم الحفاظ على change logs. لقطات public توزع على نطاق واسع، ولقطات private تبقى داخل مدققين خاصين.
- Proof-Carrying Access: يمكن للعقود تقديم (او طلب) ادلة حالة (Merkle/Verkle) مثبتة عبر التزامات snapshot. يمكن لـ private DS تقديم شهادات zero-knowledge بدلا من ادلة خام.

5) الاحتفاظ و pruning
- بدون pruning للـ public DS: الاحتفاظ بكل اجسام Kura ولقطات WSV عبر DA (توسع افقي). يمكن لـ private DS تحديد احتفاظ داخلي، لكن الالتزامات المصدرة تظل غير قابلة للتغيير. طبقة nexus تحتفظ بكل Nexus Blocks والتزامات artifacts لـ DS.

الشبكات وادوار العقد
- المدققون العالميون: يشاركون في اجماع nexus، يتحققون من Nexus Blocks و DS artifacts، ويجرون فحوص DA لـ public DS.
- مدققو Data Space: يشغلون اجماع DS، ينفذون العقود، يديرون Kura/WSV المحلي، ويتولون DA لـ DS الخاصة بهم.
- عقد DA (اختياري): تخزن/تنشر blobs عامة وتدعم sampling. في private DS، عقد DA تكون متجاورة مع المدققين او حراس موثوقين.

تحسينات واعتبارات على مستوى النظام
- فصل sequencing/mempool: اعتماد mempool DAG (مثل Narwhal) يغذي BFT متسلسل في طبقة nexus لتقليل الكمون وزيادة throughput دون تغيير النموذج المنطقي.
- حصص DS والعدالة: حصص لكل DS في كل كتلة وحدود وزن لتجنب head-of-line blocking وضمان كمون متوقع لـ private DS.
- اقرار DS (PQ): شهادات quorum للـ DS تستخدم ML-DSA-87 (فئة Dilithium5) افتراضيا. هذا post-quantum واكبر من تواقيع EC لكنه مقبول بمعدل QC واحد لكل slot. يمكن لـ DS اختيار ML-DSA-65/44 الاصغر او تواقيع EC اذا اعلن في DS Manifest؛ ويوصى public DS بالاحتفاظ بـ ML-DSA-87.
- DA attesters: للـ public DS، استخدم attesters اقليميين مختارين بـ VRF يصدرون شهادات DA. لجنة nexus تتحقق من الشهادات بدلا من sampling الخام؛ private DS تحتفظ بشهاداتها الداخلية.
- Recursion واثباتات الحقبة: يمكن تجميع عدة micro-batches داخل DS في اثبات واحد لكل slot/epoch للحفاظ على حجم الاثبات وزمن التحقق مستقرا تحت الحمل العالي.
- Lane scaling (عند الحاجة): اذا اصبحت اللجنة العالمية عنق زجاجة، قدم K lanes تسلسل متوازية مع دمج حتمي. هذا يحافظ على ترتيب عالمي واحد مع توسع افقي.
- تسريع حتمي: توفير kernels SIMD/CUDA مع feature flags للـ hashing/FFT مع fallback CPU مطابق للبت للحفاظ على الحتمية عبر العتاد.
- عتبات تفعيل lanes (اقتراح): تفعيل 2-4 lanes اذا (a) تجاوزت نهائية p95 مدة 1.2 s لاكثر من 3 دقائق متتالية، او (b) تجاوز اشغال الكتلة 85% لاكثر من 5 دقائق، او (c) تطلب معدل المعاملات الداخل >1.2x من سعة الكتلة بشكل مستدام. تقوم lanes بتجميع المعاملات بحتمية حسب hash للـ DSID ثم تدمج في كتلة nexus.

الرسوم والاقتصاد (افتراضات اولية)
- وحدة الغاز: token غاز لكل DS مع قياس compute/IO؛ الرسوم تدفع بعملة الغاز الاصلية للـ DS. التحويل بين DS مسؤولية التطبيق.
- اولوية الادراج: round-robin عبر DS مع حصص لكل DS للحفاظ على العدالة وSLOs 1s؛ داخل DS يمكن للمزايدة على الرسوم ان تفك التعادل.
- مستقبلا: يمكن استكشاف سوق رسوم عالمي او سياسات تقلل MEV دون تغيير الذرية او تصميم ادلة PQ.

سير عمل cross-Data-Space (مثال)
1) يرسل مستخدم معاملة AMX تمس public DS P و private DS S: نقل الاصل X من S الى المستفيد B الذي حسابه في P.
2) داخل slot، ينفذ P و S مقطعهم على snapshot. تتحقق S من التفويض والتوفر وتحدث حالتها الداخلية وتنتج اثبات صلاحية PQ والتزام DA (بدون تسريب بيانات خاصة). يحضر P تحديث الحالة المقابل (مثل mint/burn/locking في P حسب السياسة) واثباته.
3) تتحقق لجنة nexus من كلا اثباتي DS وشهادات DA؛ اذا تحقق كلاهما خلال slot يتم التزام المعاملة ذرياً في كتلة Nexus ذات 1s وتحديث roots لكل DS في متجه world state العالمي.
4) اذا كان اي اثبات او شهادة DA مفقودا او غير صالح، تلغى المعاملة (بدون اثر) ويمكن للعميل اعادة ارسالها للـ slot التالي. لا تغادر بيانات S الخاصة في اي خطوة.

- اعتبارات الامان
- تنفيذ حتمي: تبقى syscalls في IVM حتمية؛ النتائج عبر DS يقودها AMX commit والنهائية، وليس الوقت او توقيت الشبكة.
- التحكم بالوصول: صلاحيات ISI في private DS تقيد من يمكنه ارسال المعاملات وما العمليات المسموحة. capability tokens تشفر حقوقا دقيقة لاستخدام cross-DS.
- السرية: تشفير من الطرف للطرف لبيانات private-DS، shards بترميز محو مخزنة فقط لدى الاعضاء المصرح لهم، وادلة ZK اختيارية لاثباتات خارجية.
- مقاومة DoS: العزل على مستويات mempool/consensus/storage يمنع ازدحام public من تعطيل تقدم private DS.

تغييرات على مكونات Iroha
- iroha_data_model: ادخال `DataSpaceId` و IDs مؤهلة بـ DS و AMX descriptors (مجموعات قراءة/كتابة) وانواع proofs/التزامات DA. تسلسل Norito فقط.
- ivm: اضافة syscalls و pointer-ABI types لـ AMX (`amx_begin`, `amx_commit`, `amx_touch`) و proofs DA؛ تحديث اختبارات/توثيق ABI وفقا لسياسة v1.
