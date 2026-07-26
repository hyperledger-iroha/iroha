---
lang: ar
direction: rtl
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_lanes.md -->

# نموذج مسارات Nexus وتقسيم WSV

> **الحالة:** تسليم NX-1 - تصنيف المسارات وهندسة الاعدادات ومخطط التخزين جاهزة للتنفيذ.  
> **المالكون:** Nexus Core WG, Governance WG  
> **عنصر خارطة الطريق المرتبط:** NX-1

يوثق هذا المستند البنية المستهدفة لطبقة الاجماع متعددة المسارات في Nexus. الهدف هو انتاج حالة عالمية حتمية واحدة مع السماح لمساحات البيانات (المسارات) بتشغيل مجموعات مدققين عامة او خاصة باعباء عمل معزولة.

> **اثباتات cross-lane:** تركز هذه المذكرة على الهندسة والتخزين. التزامات التسوية لكل مسار، وخط relay، واثباتات merge-ledger المطلوبة لخارطة الطريق **NX-4** موضحة في [nexus_cross_lane.md](nexus_cross_lane.md).

## المفاهيم

- **Lane:** شارد منطقي من دفتر Nexus مع مجموعة مدققين خاصة وتراكم تنفيذ مستقل. يتم تعريفه عبر `LaneId` ثابت.
- **Data Space:** حاوية حوكمة تجمع مسارا او اكثر يشاركون سياسات الامتثال والتوجيه والتسوية. كل مساحة بيانات تعلن ايضا `fault_tolerance (f)` المستخدم لتحديد حجم لجان relay للمسار (`3f+1`).
- **Lane Manifest:** بيانات وصفية تحت سيطرة الحوكمة تصف المدققين وسياسة DA ورمز الغاز وقواعد التسوية واذونات التوجيه.
- **Global Commitment:** اثبات يصدره المسار يلخص جذور حالة جديدة وبيانات التسوية وتحويلات cross-lane اختيارية. حلقة NPoS العالمية ترتب الالتزامات.

## تصنيف المسارات

تصف انواع المسارات بشكل قانوني الرؤية وسطح الحوكمة وخطافات التسوية. تلتقط هندسة التهيئة (`LaneConfig`) هذه السمات لكي تتمكن العقد وSDKs وادوات التشغيل من فهم المخطط دون منطق مخصص.

| نوع المسار | الرؤية | عضوية المدققين | عرض WSV | الحوكمة الافتراضية | سياسة التسوية | الاستخدام النموذجي |
|-----------|--------|----------------|---------|--------------------|---------------|--------------------|
| `default_public` | عام | Permissionless (stake عالمي) | نسخة كاملة من الحالة | برلمان SORA | `xor_global` | دفتر عام اساسي |
| `public_custom` | عام | Permissionless او محكوم بالاستيك | نسخة كاملة من الحالة | وحدة موزونة بالاستيك | `xor_lane_weighted` | تطبيقات عامة عالية الانتاجية |
| `private_permissioned` | مقيد | مجموعة مدققين ثابتة (معتمدة من الحوكمة) | Commitments و proofs | مجلس اتحادي | `xor_hosted_custody` | CBDC واحمال كونسورتيوم |
| `hybrid_confidential` | مقيد | عضوية مختلطة؛ تلتف حول اثباتات ZK | Commitments + افصاح انتقائي | وحدة مال قابل للبرمجة | `xor_dual_fund` | مال قابل للبرمجة مع خصوصية |

يجب على جميع انواع المسارات اعلان:

- Alias لمساحة البيانات - تجميع مقروء يربط سياسات الامتثال.
- Handle للحوكمة - معرف يحل عبر `Nexus.governance.modules`.
- Handle للتسوية - معرف يستهلكه settlement router لخصم مخازن XOR.
- بيانات telemetria اختيارية (وصف، جهة اتصال، مجال اعمال) معروضة عبر `/status` ولوحات التحكم.

## هندسة تهيئة المسارات (`LaneConfig`)

`LaneConfig` هي هندسة التشغيل المشتقة من كتالوج المسارات المعتمد. لا تستبدل manifests الحوكمة؛ بل توفر معرفات تخزين حتمية وتلميحات telemetria لكل مسار مهيأ.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` يعيد حساب الهندسة كلما تم تحميل الاعدادات (`State::set_nexus`).
- يتم تنقية aliases الى slugs بحروف صغيرة؛ وتتقلص الاحرف غير الابجدية الرقمية المتتالية الى `_`. اذا نتج slug فارغ نعود الى `lane{id}`.
- تضمن بادئات المفاتيح بقاء نطاقات WSV لكل مسار منفصلة حتى عند مشاركة نفس backend.
- يتم اشتقاق `shard_id` من مفتاح metadata في الكتالوج `da_shard_id` (افتراضيه `lane_id`) ويوجه journal المؤرشف لمؤشر shard للحفاظ على replay DA الحتمي عبر اعادة التشغيل/اعادة التقسيم.
- اسماء مقاطع Kura حتمية بين المضيفين؛ يمكن للمدققين مطابقة الدلائل وmanifests دون ادوات خاصة.
- تحتفظ مقاطع merge (`lane_{id:03}_merge`) باحدث جذور merge-hint والتزامات الحالة العالمية لهذا المسار.
- عند اعادة تسمية alias لمسار بواسطة الحوكمة، تقوم العقد باعادة تسمية ادلة `blocks/lane_{id:03}_{slug}` (ولقطات الطبقات) تلقائيا حتى يرى المدققون دائما slug القانوني دون تنظيف يدوي.

## تقسيم world-state

- الحالة العالمية المنطقية لـ Nexus هي اتحاد مساحات الحالة لكل مسار. المسارات العامة تحفظ الحالة كاملة؛ المسارات الخاصة/السرية تصدر جذور Merkle/commitment الى merge ledger.
- تخزين MV يسبق كل مفتاح ببادئة 4 بايت من `LaneConfigEntry::key_prefix` منتجا مفاتيح مثل `[00 00 00 01] ++ PackedKey`.
- لذا تخزن الجداول المشتركة (الحسابات، الاصول، المحفزات، سجلات الحوكمة) ادخالات مجمعة حسب بادئة المسار، محافظة على عمليات مسح نطاق حتمية.
- تعكس بيانات merge-ledger نفس المخطط: كل مسار يكتب جذور merge-hint وجذور الحالة العالمية المخفضة في `lane_{id:03}_merge`، ما يسمح بالاحتفاظ او الازالة المستهدفة عند تقاعد مسار.
- تخزن الفهارس cross-lane (اسماء الحسابات، سجلات الاصول، manifests الحوكمة) ازواجا صريحة `(LaneId, DataSpaceId)`. تعيش هذه الفهارس في عائلات اعمدة مشتركة لكنها تستخدم بادئة المسار ومعرفات مساحة البيانات الصريحة للحفاظ على lookups حتمية.
- يجمع سير عمل الدمج بين البيانات العامة والالتزامات الخاصة باستخدام tuples `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` المشتقة من ادخالات merge-ledger.

## تقسيم Kura و WSV

- **مقاطع Kura**
  - `lane_{id:03}_{slug}` - مقطع الكتل الاساسي للمسار (كتل، فهارس، ايصالات).
  - `lane_{id:03}_merge` - مقطع merge-ledger الذي يسجل جذور الحالة المخفضة واثار التسوية.
  - المقاطع العامة (ادلة الاجماع، caches telemetria) تبقى مشتركة لانها محايدة للمسارات؛ مفاتيحها لا تتضمن بادئات المسارات.
- يراقب وقت التشغيل تحديثات كتالوج المسارات: يتم تجهيز ادلة الكتل وmerge-ledger تلقائيا للمسارات الجديدة تحت `kura/blocks/` و`kura/merge_ledger/`، بينما تؤرشف المسارات المتقاعدة تحت `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- تعكس لقطات الحالة الطبقية نفس الدورة؛ يكتب كل مسار في `<cold_root>/lanes/lane_{id:03}_{slug}` حيث `<cold_root>` هو `cold_store_root` (او `da_store_root` عندما لا يتم ضبط `cold_store_root`) وتنتقل التقاعدات الى `<cold_root>/retired/lanes/`.
- **بادئات المفاتيح** - يتم دائما اضافة بادئة 4 بايت المحسوبة من `LaneId` قبل مفاتيح MV المشفرة. لا يستخدم hashing خاص بالمضيف، لذا يكون الترتيب متطابقا عبر العقد.
- **مخطط block log** - بيانات الكتل والفهرس والتجزئات متداخلة تحت `kura/blocks/lane_{id:03}_{slug}/`. تستخدم سجلات merge-ledger نفس slug (`kura/merge/lane_{id:03}_{slug}.log`)، مما يبقي تدفقات الاستعادة لكل مسار معزولة.
- **سياسة الاحتفاظ** - المسارات العامة تحتفظ باجسام الكتل كاملة؛ المسارات ذات الالتزامات فقط قد تضغط الاجسام القديمة بعد checkpoints لان الالتزامات هي المرجع. المسارات السرية تحتفظ بسجلات مشفرة في مقاطع مخصصة لتجنب حجب اعمال اخرى.
- **ادوات التشغيل** - `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` يفحص `<store>/blocks` و`<store>/merge_ledger` باستخدام `LaneConfig` المشتق، ويبلغ عن المقاطع النشطة مقابل المتقاعدة، ويؤرشف الادلة/السجلات المتقاعدة تحت `<store>/retired/...` للحفاظ على ادلة حتمية. يجب على ادوات الصيانة (`kagami`, اوامر CLI الادارية) اعادة استخدام namespace ب slug عند عرض metrics او Prometheus labels او ارشفة مقاطع Kura.

## ميزانيات التخزين

- `nexus.storage.max_disk_usage_bytes` يحدد ميزانية القرص الاجمالية التي يجب ان تستهلكها عقد Nexus عبر Kura ولقطات WSV الباردة وتخزين SoraFS و spools البث (SoraNet/SoraVPN).
- `nexus.storage.budget_enforce_interval_blocks` يحدد الفاصل (بعدد الكتل المؤكدة) بين عمليات فحص ميزانية التخزين؛ 0 يعني كل كتلة.
- عند تجاوز الميزانية الكلية، تكون الازالة حتمية: يتم تقليم spools تزويد SoraNet بترتيب المسار النسبي، ثم spools SoraVPN، ثم لقطات tiered-state الباردة من الاقدم الى الاحدث (مع offload الى `da_store_root` عند ضبطه)، ثم مقاطع Kura المتقاعدة، واخيرا يتم تفريغ اجسام الكتل النشطة لـ Kura الى `da_blocks/` لاعادة الترطيب عبر DA عند القراءة.
- `nexus.storage.max_wsv_memory_bytes` يحد طبقة WSV الساخنة عبر تمرير قياس WSV الحتمي في الذاكرة الى `tiered_state.hot_retained_bytes`؛ قد يتجاوز الاحتفاظ المسموح الميزانية مؤقتا، لكن التجاوز مرئي عبر telemetria (`state_tiered_hot_bytes`, `state_tiered_hot_grace_overflow_bytes`).
- `nexus.storage.disk_budget_weights` يقسم ميزانية القرص بين المكونات باستخدام نقاط الاساس (يجب ان تساوي 10,000). تطبق الحدود المشتقة على `kura.max_disk_usage_bytes` و`tiered_state.max_cold_bytes` و`sorafs.storage.max_capacity_bytes` و`streaming.soranet.provision_spool_max_bytes` و`streaming.soravpn.provision_spool_max_bytes`.
- تطبيق ميزانية Kura يجمع بايتات مخزن الكتل عبر مقاطع المسارات النشطة والمتقاعدة ويشمل الكتل في الطابور غير المحفوظة بعد لتجنب تجاوز الميزانية اثناء تاخير الكتابة.
- spools تزويد SoraVPN تستخدم اعدادات `streaming.soravpn` وتحد بشكل مستقل عن spool تزويد SoraNet.
- لا تزال حدود كل مكون سارية: عندما يكون للمكون حد صريح غير صفري، يتم تطبيق الاصغر بين ذلك الحد وميزانية Nexus المشتقة.
- telemetria الميزانيات تستخدم `storage_budget_bytes_used{component=...}` و`storage_budget_bytes_limit{component=...}` للابلاغ عن الاستخدام/الحدود لـ `kura` و`wsv_hot` و`wsv_cold` و`soranet_spool` و`soravpn_spool`؛ ويزداد `storage_budget_exceeded_total{component=...}` عندما ترفض الالية بيانات جديدة وتصدر السجلات تحذيرا للمشغل.
- تضيف telemetria ازالة DA `storage_da_cache_total{component=...,result=hit|miss}` و`storage_da_churn_bytes_total{component=...,direction=evicted|rehydrated}` لتتبع نشاط التخزين المؤقت والبايتات المنقولة لـ `kura` و`wsv_cold`.
- Kura يبلغ نفس المحاسبة المستخدمة اثناء القبول (بايتات على القرص بالاضافة الى الكتل في الطابور، بما في ذلك حمولات merge-ledger عند وجودها)، لذا تعكس المقاييس الضغط الفعلي وليس فقط البايتات المحفوظة.

## التوجيه وواجهات API

- تقبل نقاط Torii REST/gRPC قيمة `lane_id` اختيارية؛ عدم وجودها يعني `lane_default`.
- تعرض SDKs محددات lanes وتربط aliases سهلة بـ `LaneId` باستخدام كتالوج المسارات.
- تعمل قواعد التوجيه على الكتالوج المعتمد وقد تختار المسار ومساحة البيانات معا. توفر `LaneConfig` aliases مناسبة لل telemetria في اللوحات والسجلات.

## التسوية والرسوم

- كل مسار يدفع رسوم XOR لمجموعة المدققين العالمية. يمكن للمسارات تحصيل رموز غاز محلية لكنها يجب ان تضع مكافئا من XOR في escrow مع الالتزامات.
- تشمل اثباتات التسوية المبلغ وبيانات التحويل واثبات escrow (مثلا تحويل الى خزينة الرسوم العالمية).
- يقوم settlement router الموحد (NX-3) بخصم buffers باستخدام نفس بادئات المسار، بحيث تتطابق telemetria التسوية مع هندسة التخزين.

## الحوكمة

- تعلن المسارات وحدة الحوكمة عبر الكتالوج. يحمل `LaneConfigEntry` الاسم والslug الاصليين ليظل سجل telemetria والتدقيق مقروءا.
- يسوق سجل Nexus manifests مسار موقعة تشمل `LaneId` وربط مساحة البيانات وhandle الحوكمة وhandle التسوية وmetadata.
- تستمر hooks ترقية وقت التشغيل في فرض سياسات الحوكمة (`gov_upgrade_id` افتراضيا) وتسجيل diffs عبر جسر telemetria (احداث `nexus.config.diff`).
- تحدد manifests المسار مجموعة مدققي مساحة البيانات للمسارات المدارة؛ والمسارات المنتخبة بالاستيك تستمد مجموعة المدققين من سجلات استيك المسارات العامة.

## telemetria و status

- يعرض `/status` aliases المسارات وربط مساحات البيانات وhandles الحوكمة وملفات التسوية، مشتقة من الكتالوج و`LaneConfig`.
- تعرض مقاييس المجدول (`nexus_scheduler_lane_teu_*`) aliases/slugs للمسارات كي يربط المشغلون backlog وضغط TEU بسرعة.
- `nexus_lane_configured_total` يحسب عدد ادخالات المسار المشتقة ويعاد حسابه عند تغير الاعدادات. تصدر telemetria diffs موقعة عند تغير هندسة المسارات.
- تشمل مقاييس backlog لمساحات البيانات بيانات alias/description لمساعدة المشغلين على ربط ضغط الطوابير بمجالات الاعمال.

## الاعدادات وانواع Norito

- `LaneCatalog`, `LaneConfig`, و`DataSpaceCatalog` تعيش في `iroha_data_model::nexus` وتوفر هياكل متوافقة مع Norito لل manifests وSDKs.
- `LaneConfig` يعيش في `iroha_config::parameters::actual::Nexus` ويشتق تلقائيا من الكتالوج؛ لا يتطلب ترميز Norito لانه مساعد داخلي لوقت التشغيل.
- تستمر اعدادات المستخدم (`iroha_config::parameters::user::Nexus`) بقبول اوصاف lanes وdataspaces التصريحية؛ parsing يستخرج الهندسة ويرفض aliases غير صالحة او معرفات lanes مكررة.
- `DataSpaceMetadata.fault_tolerance` يتحكم بحجم لجنة relay للمسار؛ عضوية اللجنة يتم اختيارها حتميا لكل حقبة من مجموعة مدققي مساحة البيانات باستخدام بذرة VRF المرتبطة بـ `(dataspace_id, lane_id)`.

## اعمال متبقية

- دمج تحديثات settlement router (NX-3) مع الهندسة الجديدة ليتم وسم خصومات XOR buffers والايرادات ب slug المسار.
- انهاء خوارزمية الدمج (الترتيب، pruning، كشف التعارض) وربط fixtures للارتداد cross-lane.
- اضافة hooks للامتثال لقوائم السماح/الحظر وسياسات المال القابل للبرمجة (متابعة ضمن NX-12).

---

*سيتطور هذا المستند مع تقدم مهام NX-2 حتى NX-18. يرجى تسجيل الاسئلة المفتوحة في roadmap او tracker الحوكمة.*

</div>
