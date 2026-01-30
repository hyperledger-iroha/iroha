---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/lane-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-lane-model
title: نموذج lanes في Nexus
description: تصنيف منطقي للـ lanes، هندسة التهيئة، وقواعد دمج world-state لـ Sora Nexus.
---

# نموذج lanes في Nexus وتقسيم WSV

> **الحالة:** مخرج NX-1 - تصنيف lanes، هندسة التهيئة، وتخطيط التخزين جاهزة للتنفيذ.  
> **المالكون:** Nexus Core WG، Governance WG  
> **مرجع roadmap:** NX-1 في `roadmap.md`

تعكس هذه الصفحة موجز `docs/source/nexus_lanes.md` القانوني كي يتمكن مشغلو Sora Nexus ومالكو SDK والمراجعون من قراءة ارشادات lanes دون الغوص في شجرة mono-repo. تحافظ الهندسة المستهدفة على حتمية world state مع السماح لمساحات البيانات (lanes) الفردية بتشغيل مجموعات مدققين عامة او خاصة باعمال معزولة.

## المفاهيم

- **Lane:** shard منطقي من ledger Nexus مع مجموعة مدققين خاصة به وbacklog تنفيذ. معرف بـ `LaneId` ثابت.
- **Data Space:** حاوية حوكمة تجمع lane او اكثر تشترك في سياسات الامتثال والتوجيه والتسوية.
- **Lane Manifest:** بيانات وصفية تتحكم بها الحوكمة تصف المدققين وسياسة DA ورمز الغاز وقواعد التسوية واذونات التوجيه.
- **Global Commitment:** proof تصدرها lane تلخص جذور حالة جديدة وبيانات التسوية ونقل cross-lane اختياري. حلقة NPoS العالمية ترتب commitments.

## تصنيف lanes

تصف انواع lanes قانونيا رؤيتها وسطح الحوكمة وhooks التسوية. هندسة التهيئة (`LaneConfig`) تلتقط هذه السمات كي تتمكن العقد وSDKs والادوات من فهم التخطيط دون منطق مخصص.

| نوع lane | الرؤية | عضوية المدققين | تعريض WSV | الحوكمة الافتراضية | سياسة التسوية | الاستخدام المعتاد |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | عام | Permissionless (global stake) | نسخة حالة كاملة | SORA Parliament | `xor_global` | دفتر عام اساسي |
| `public_custom` | عام | Permissionless او stake-gated | نسخة حالة كاملة | وحدة مرجحة بال stake | `xor_lane_weighted` | تطبيقات عامة عالية السعة |
| `private_permissioned` | مقيد | مجموعة مدققين ثابتة (معتمدة من الحوكمة) | Commitments و proofs | Federated council | `xor_hosted_custody` | CBDC، اعمال كونسورتيوم |
| `hybrid_confidential` | مقيد | عضوية مختلطة؛ تغلف ZK proofs | Commitments + افصاح انتقائي | وحدة نقود قابلة للبرمجة | `xor_dual_fund` | نقود قابلة للبرمجة مع حفظ الخصوصية |

يجب على كل نوع lane التصريح بما يلي:

- Alias للداتاسبيس - تجميع مقروء للبشر يربط سياسات الامتثال.
- Governance handle - معرف يحل عبر `Nexus.governance.modules`.
- Settlement handle - معرف يستهلكه settlement router لخصم مخازن XOR.
- Metadata تيليمتري اختيارية (وصف، جهة اتصال، مجال عمل) تظهر عبر `/status` وdashboards.

## هندسة تهيئة lanes (`LaneConfig`)

`LaneConfig` هي هندسة runtime المشتقة من catalog lanes المعتمد. لا تستبدل manifests الحوكمة؛ بل توفر معرفات تخزين حتمية وتلميحات تيليمتري لكل lane مهيأة.

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

- `LaneConfig::from_catalog` يعيد حساب الهندسة عند تحميل التهيئة (`State::set_nexus`).
- تتحول aliases الى slugs بحروف صغيرة؛ تتكاثف الاحرف غير الابجدية الرقمية المتتالية الى `_`. اذا نتج slug فارغ نعود الى `lane{id}`.
- `shard_id` مشتق من مفتاح metadata `da_shard_id` (الافتراضي `lane_id`) ويقود journal مؤشر shard المثبت للحفاظ على replay حتمي ل DA عبر restarts/resharding.
- تضمن prefixes المفاتيح بقاء نطاقات مفاتيح WSV لكل lane منفصلة حتى عند مشاركة نفس backend.
- اسماء مقاطع Kura حتمية عبر hosts؛ يمكن للمدققين تدقيق ادلة المقاطع وmanifests دون ادوات مخصصة.
- مقاطع الدمج (`lane_{id:03}_merge`) تخزن اخر roots ل merge-hint وcommitments الحالة العالمية لتلك lane.

## تقسيم world-state

- world state المنطقي لـ Nexus هو اتحاد مساحات الحالة لكل lane. lanes العامة تحفظ حالة كاملة؛ lanes الخاصة/confidential تصدر roots Merkle/commitment الى merge ledger.
- تخزين MV يسبق كل مفتاح ببادئة 4 بايت من `LaneConfigEntry::key_prefix`, منتجا مفاتيح مثل `[00 00 00 01] ++ PackedKey`.
- الجداول المشتركة (accounts, assets, triggers, سجلات الحوكمة) تخزن الادخالات مجمعة حسب بادئة lane، مما يبقي range scans حتمية.
- تعكس metadata الـ merge-ledger نفس التخطيط: كل lane تكتب roots merge-hint وroots الحالة العالمية المخفضة الى `lane_{id:03}_merge`، ما يسمح بالاحتفاظ او الازالة الموجهة عندما تتقاعد lane.
- فهارس cross-lane (account aliases, asset registries, governance manifests) تخزن بادئات lane صريحة كي يتمكن المشغلون من مطابقة الادخالات بسرعة.
- **سياسة الاحتفاظ** - lanes العامة تحتفظ باجسام الكتل كاملة؛ lanes ذات commitments فقط يمكنها ضغط الاجسام الاقدم بعد checkpoints لان commitments هي المرجع. lanes confidential تحتفظ بسجلات مشفرة في مقاطع مخصصة كي لا تعيق workloads اخرى.
- **Tooling** - يجب على ادوات الصيانة (`kagami`, اوامر admin في CLI) الرجوع الى namespace ذو slug عند اظهار metrics او تسميات Prometheus او ارشفة مقاطع Kura.

## Routing وAPIs

- تقبل نقاط النهاية Torii REST/gRPC قيمة `lane_id` اختيارية؛ غيابها يعني `lane_default`.
- تعرض SDKs محددات lane وتطابق aliases الودية مع `LaneId` باستخدام catalog lanes.
- تعمل قواعد routing على catalog المعتمد ويمكنها اختيار lane وdataspace معا. يوفر `LaneConfig` aliases مناسبة للتيليمتري في dashboards والlogs.

## Settlement والرسوم

- كل lane تدفع رسوم XOR لمجموعة المدققين العالمية. يمكن لل lanes تحصيل رموز gas محلية لكن يجب ان تودع ما يعادل XOR مع commitments.
- proofs التسوية تشمل المبلغ وmetadata التحويل ودليل escrow (مثلا تحويل الى vault الرسوم العالمية).
- settlement router الموحد (NX-3) يخصم buffers باستخدام نفس بادئات lane، بحيث تتطابق تيليمتري التسوية مع هندسة التخزين.

## Governance

- تعلن lanes وحدة الحوكمة عبر catalog. تحمل `LaneConfigEntry` alias وslug الاصليين لابقاء تيليمتري ومسارات التدقيق مقروءة.
- يوزع Nexus registry manifests موقعة للـ lane تشمل `LaneId` وربط dataspace وgovernance handle وsettlement handle وmetadata.
- تستمر hooks الترقية runtime في فرض سياسات الحوكمة (`gov_upgrade_id` افتراضيا) وتسجيل diffs عبر telemetry bridge (احداث `nexus.config.diff`).

## Telemetry وstatus

- يكشف `/status` عن aliases لل lanes وربط dataspaces وhandles الحوكمة وملفات التسوية، مشتقة من catalog و`LaneConfig`.
- تعرض مقاييس scheduler (`nexus_scheduler_lane_teu_*`) aliases/slugs ليتمكن المشغلون من ربط backlog وضغط TEU بسرعة.
- `nexus_lane_configured_total` يحصي عدد ادخالات lane المشتقة ويعاد حسابه عند تغير التهيئة. تبعث التيليمتري diffs موقعة عندما تتغير هندسة lanes.
- تتضمن gauges backlog للداتاسبيس metadata alias/description لمساعدة المشغلين على ربط ضغط الطوابير بالمجالات التجارية.

## التهيئة وانواع Norito

- `LaneCatalog`, `LaneConfig`, و`DataSpaceCatalog` تعيش في `iroha_data_model::nexus` وتوفر هياكل متوافقة مع Norito للـ manifests وSDKs.
- `LaneConfig` يعيش في `iroha_config::parameters::actual::Nexus` ويشتق تلقائيا من catalog؛ لا يحتاج الى Norito encoding لانه helper داخلي runtime.
- تظل تهيئة المستخدم (`iroha_config::parameters::user::Nexus`) تقبل واصفات lane وdataspace التصريحية؛ يقوم التحليل الان باشتقاق الهندسة ورفض aliases غير صالحة او IDs lanes مكررة.

## العمل المتبقي

- دمج تحديثات settlement router (NX-3) مع الهندسة الجديدة كي توسم خصومات وايصالات buffers XOR بslug lane.
- توسيع tooling الادارية لسرد column families وضغط lanes المتقاعدة وفحص سجلات الكتل لكل lane باستخدام namespace ذي slug.
- انهاء خوارزمية الدمج (ordering, pruning, conflict detection) وارفاق fixtures رجعية لاعادة التشغيل cross-lane.
- اضافة hooks امتثال لـ whitelists/blacklists وسياسات النقود القابلة للبرمجة (متابعة تحت NX-12).

---

*ستواصل هذه الصفحة تتبع متابعات NX-1 مع وصول NX-2 حتى NX-18. يرجى اظهار الاسئلة المفتوحة في `roadmap.md` او tracker الحوكمة ليبقى البوابة متوافقة مع الوثائق القانونية.*
