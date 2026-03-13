---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: SoraFS Pin Registry نفاذی منصوبہ
Sidebar_label: Pin Registry منصوبہ
الوصف: نفاذية SF-4 منصوبة لآلة تسجيل الحالة، واجهة Torii، والأدوات وإمكانية المراقبة.
---

:::ملاحظة مستند ماخذ
هذه هي الصفحة `docs/source/sorafs/pin_registry_plan.md`. عندما تتاح لك إمكانية الوصول إلى جهات الاتصال الفعالة، يمكنك الاتصال بنا.
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

SF-4 Pin Registry ورابط التسجيل يوضح الالتزامات الواضحة المهمة،
تظهر سياسات الدبوس بشكل فعال، وTorii، والبوابات والمنسقين لواجهات برمجة التطبيقات.
لقد وضعت خطة التحقق من الصحة والتي تتضمن مهام التنفيذ بشكل فعال، وهي عبارة عن منطق متصل بالسلسلة،
تشمل خدمات الجانب المضيف والتركيبات والعمليات المطلوبة.

##ديرہ کار1. **جهاز حالة التسجيل**: السجلات المحددة بواسطة Norito تظهر البيانات، الأسماء المستعارة، السلاسل اللاحقة،
   فترات الاحتفاظ، والبيانات الوصفية للحوكمة.
2. **شبكة نفاذ**: دورة حياة الدبوس لعمليات CRUD الحتمية (`ReplicationOrder`،
   `Precommit`، `Completion`، الإخلاء).
3. **الواجهة الخارجية**: يتم استخدام تسجيل نقاط نهاية gRPC/REST وTorii وSDK،
   إن ترقيم الصفحات والتصديق يشملان كل شيء.
4. **الأدوات والتركيبات**: مساعدو واجهة سطر الأوامر (CLI)، وموجهات الاختبار، والوثائق، والبيانات، والأسماء المستعارة،
   مظاريف الحوكمة مظاريف الحكم.
5. **القياس عن بعد والعمليات**: سجل صحيح للمقاييس والتنبيهات ودفاتر التشغيل.

## ڈیٹا ماڈ

### دفتر ريكارز (Norito)| هيكل | وضاحت | فيليز |
|--------|-------|-------|
| `PinRecordV1` | إدخال البيان الكنسي. | `manifest_cid`، `chunk_plan_digest`، `por_root`، `profile_handle`، `approved_at`، `retention_epoch`، `pin_policy`، `successor_of`، `governance_envelope_hash`. |
| `AliasBindingV1` | الاسم المستعار -> تعيين CID الظاهر. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | يقوم مقدمو الخدمة بإظهار رقم التعريف الشخصي. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | إقرار المزود. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | صورة لسياسة الحوكمة. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

مرجع التنفيذ: `crates/sorafs_manifest/src/pin_registry.rs` مخططات Rust Norito
ومساعدي التحقق من الصحة موجودون. أدوات بيان التحقق من الصحة (البحث عن السجل المقسم، بوابة سياسة الدبوس)
هذه هي الواجهات المركزية وواجهات Torii وثوابت CLI الأكثر تنوعًا.

المهام:
- `crates/sorafs_manifest/src/pin_registry.rs` مكمل لمخططات Norito.
- وحدات الماكرو Norito تنشئ كودًا (Rust + SDKs أخرى).
- المخططات من خلال المستندات التالية (`sorafs_architecture_rfc.md`) متاحة.

## کنٹریکٹ نفاذ| كام | مالک/مالکان | أخبار |
|-----|------------|------|
| تخزين التسجيل (sled/sqlite/off-chain) أو وحدة العقود الذكية. | البنية التحتية الأساسية / فريق العقد الذكي | التجزئة الحتمية فرام كري، النقطة العائمة سے بچيں. |
| نقاط الإدخال: `submit_manifest`، `approve_manifest`، `bind_alias`، `issue_replication_order`، `complete_replication`، `evict_manifest`. | الأشعة تحت الحمراء الأساسية | استخدام خطة التحقق من الصحة `ManifestValidator`. الاسم المستعار ملزم اب `RegisterPinManifest` (Torii DTO) سے گزرت ہے جبکہصوص `bind_alias` ہيندہ پیٹس لیے منصوبہ بند ہے. |
| انتقالات الحالة: الخلافة (البيان أ -> ب)، فترات الاحتفاظ، والاسم المستعار التفرد نافذ کریں۔ | مجلس الحكم / البنية الأساسية | تفرد الاسم المستعار، وحدود الاحتفاظ، والموافقة السابقة/فحوصات التقاعد اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ہیں؛ اكتشاف التتابع متعدد القفزات ومسك دفاتر النسخ المتماثل |
| المعلمات المحكومة: `ManifestPolicyV1` حالة التكوين/الإدارة؛ تم السماح بأحداث الحوكمة. | مجلس الحكم | تحديثات السياسة لها علاقة بـ CLI. |
| انبعاث الحدث: القياس عن بعد لأحداث Norito (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) جاري. | إمكانية الملاحظة | مخطط الحدث + التسجيل ممتع. |الاختبار:
- ہر نقطة الدخول کے لیے اختبارات الوحدة (إيجابية + رفض).
- سلسلة التعاقب کے لیے اختبارات الخاصية (بدون دورات، عصور رتيبة).
- البيانات العشوائية (المحدودة) سے التحقق من صحة الزغب.

## واجهة سروس (Torii/SDK الجوائز)

| جزو | كام | مالک/مالکان |
|------|-----|-------------|
| خدمة Torii | `/v2/sorafs/pin` (إرسال)، `/v2/sorafs/pin/{cid}` (بحث)، `/v2/sorafs/aliases` (قائمة/ربط)، `/v2/sorafs/replication` (الطلبات/الإيصالات) ترقيم الصفحات + التصفية. | الشبكات TL / الأشعة تحت الحمراء الأساسية |
| تصديق | تتضمن الاستجابات ارتفاع التسجيل/التجزئة؛ تتضمن بنية شهادة Norito استخدام أدوات تطوير البرمجيات (SDKs) للقراءة. | الأشعة تحت الحمراء الأساسية |
| سطر الأوامر | `sorafs_manifest_stub` جديد أو جديد `sorafs_pin` CLI مستخدم `pin submit`, `alias bind`, `order issue`, `registry export`. | الأدوات مجموعة العمل |
| SDK | مخطط Norito وارتباطات العميل (Rust/Go/TS) تولد ملفات؛ اختبارات التكامل تشمل کریں۔ | فرق SDK |

العمليات:
- الحصول على نقاط النهاية من أجل ذاكرة التخزين المؤقت/طبقة ETag.
- تتوافق سياسات Torii مع تحديد المعدل / المصادقة.

## المباريات وCI- دليل التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يحتوي على البيان الموقع/الاسم المستعار/لقطات الطلب.
- خطوة CI: `ci/check_sorafs_fixtures.sh` لقطة إعادة إنشاء لقطة ہے وفرق ہونے پر فشل کرتا ہے تى CI تركيبات محاذاة.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) المسار السعيد رفض الاسم المستعار المكرر، حراس الموافقة/الاحتفاظ بالاسم المستعار، مقابض القطع غير المتطابقة، التحقق من صحة عدد النسخ المتماثلة، وفشل حماية التعاقب (غير معروف/موافق عليه مسبقًا/متقاعد/مؤشرات ذاتية) وصف لحالات `register_manifest_rejects_*`.
- اختبارات الوحدة اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` تتضمن التحقق من صحة الاسم المستعار، ووحدات حماية الاحتفاظ، والفحوصات اللاحقة؛ اكتشاف التتابع متعدد القفزات
- خطوط أنابيب إمكانية المراقبة لأحداث JSON الذهبية۔

## القياس عن بعد وإمكانية الملاحظة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- القياس عن بعد الخاص بموفر الخدمة (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) موجود من خلال لوحات معلومات شاملة للنطاق.

السجلات:
- عمليات تدقيق الإدارة کے لیے تدفق حدث Norito منظم (موقع؟).

التنبيهات:
- SLA عدد كبير من أوامر النسخ المتماثل المعلقة.
- عتبة انتهاء الصلاحية الاسم المستعار سے کم.
- مخالفات الاحتفاظ (بيان التجديد وقت سے پہلے نہ ہو).لوحات المعلومات:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` إجماليات دورة حياة البيان، وتغطية الاسم المستعار، وتشبع الأعمال المتراكمة، ونسبة SLA، ووقت الاستجابة مقابل تراكبات الركود، ومعدلات الطلبات الفائتة ومراجعة عند الطلب.

## كتب التشغيل والوثائق

- `docs/source/sorafs/migration_ledger.md` تتضمن تحديثات حالة التسجيل معلومات أساسية.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ابتشاع شاعہ) المقاييس، والتنبيه، والنشر، والنسخ الاحتياطي، وتدفقات الاسترداد.
- دليل الحوكمة: معايير السياسة، سير عمل الموافقة، التعامل مع النزاعات.
- الصفحات المرجعية لنقطة النهاية لواجهة برمجة التطبيقات (Docusaurus docs).

## التبعيات والتسلسل

1. مهام خطة التحقق من الصحة مکمل کریں (تكامل ManifestValidator).
2. مخطط Norito + إعدادات السياسة الافتراضية.
3. العقد + خدمة التمويل وسلك القياس عن بعد.
4. تعمل التركيبات على تجديد أجنحة التكامل والتكامل.
5. تعد المستندات/دفاتر التشغيل عناصر أساسية وخريطة الطريق للعلامة التجارية.

SF-4 هي قائمة مرجعية تم تصميمها خصيصًا للتحول إلى ما هو أبعد من ذلك.
واجهة REST هي نقاط نهاية القائمة المعتمدة:

- يعرض `GET /v2/sorafs/pin` و`GET /v2/sorafs/pin/{digest}` بطاقة الاتصال الهاتفي
  الارتباطات المستعارة، وأوامر النسخ المتماثل، وأكثر تجزئة كتلة سے ما يختار كائن التصديق شامل ہے۔
- `GET /v2/sorafs/aliases` و `GET /v2/sorafs/replication` كتالوج الاسم المستعار النشط و
  يتم الاحتفاظ بتراكم أوامر النسخ المتماثل مع ترقيم الصفحات المتسق ومرشحات الحالة.CLI تستدعي کو التفاف کرتی ہے (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) يقوم المشغلون بواجهات برمجة التطبيقات (APIs) الشاملة بتجميع عمليات تدقيق التسجيل الخاصة بهم.