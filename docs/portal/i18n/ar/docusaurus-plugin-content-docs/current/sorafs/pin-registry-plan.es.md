---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تنفيذ Pin Registry لـ SoraFS
Sidebar_label: خطة التسجيل Pin
الوصف: خطة تنفيذ SF-4 التي تغطي حالة آلة التسجيل والواجهة Torii والأدوات وقابلية المراقبة.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/pin_registry_plan.md`. احتفظ بنسخ متزامنة أثناء تنشيط المستندات المخزنة.
:::

# خطة تنفيذ Pin Registry لـ SoraFS (SF-4)

يقوم SF-4 بإدراج عقد Pin Registry وخدمات تخزين الدعم
تسويات البيان، إضافة سياسات الدبوس وتوسيع واجهات برمجة التطبيقات إلى Torii، والبوابات
و Orquestadores. هذه الوثيقة موسعة لخطة التحقق من الصحة مع خطط العمل
التنفيذ الملموس، استكشاف المنطق على السلسلة، خدمات المضيف،
التركيبات ومتطلبات التشغيل.

## الكانس1. **آلة حالة التسجيل**: السجلات المحددة لـ Norito للبيانات،
   الأسماء المستعارة، والسلاسل اللاحقة، وفترات الاحتفاظ، وبيانات التعريف الحكومية.
2. **تنفيذ العقد**: عمليات التحديد الخام لدائرة الحياة
   دي دبابيس (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **واجهة الخدمة**: نقاط النهاية gRPC/REST التي يتم الرد عليها من خلال السجل الذي تستخدمه
   Torii ومجموعات SDK، بما في ذلك الصفحة والشهادة.
4. **الأدوات والتركيبات**: مساعدات CLI، وناقلات الاختبار، والوثائق للصيانة
   البيانات والأسماء المستعارة والمغلفات المتزامنة.
5. **القياس عن بعد والعمليات**: المقاييس والتنبيهات ودفاتر التشغيل لسلامة التسجيل.

## نموذج البيانات

### السجلات المركزية (Norito)| البنية التحتية | الوصف | كامبوس |
|------------|-------------|--------|
| `PinRecordV1` | مدخل الكنسي للبيان. | `manifest_cid`، `chunk_plan_digest`، `por_root`، `profile_handle`، `approved_at`، `retention_epoch`، `pin_policy`، `successor_of`، `governance_envelope_hash`. |
| `AliasBindingV1` | الاسم المستعار Mapea -> بيان CID. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات لكي يقوم مقدمو الخدمة بتثبيت البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | سبب تلقي المزود. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة من سياسة الحكومة. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

مرجع التنفيذ: ver `crates/sorafs_manifest/src/pin_registry.rs` para los
يتم ظهور Norito في Rust ومساعدي التحقق من الصحة الذين يستجيبون لهذه السجلات. لا
التحقق من صحة مرجع أدوات البيان (البحث عن سجل المقسم، بوابة سياسة الدبوس)
بالنسبة للعقد، فإن الواجهات Torii وCLI تقارن الثوابت المتطابقة.

تاريس:
- الانتهاء من الخيارات Norito و`crates/sorafs_manifest/src/pin_registry.rs`.
- إنشاء كود (Rust + otros SDKs) باستخدام وحدات الماكرو Norito.
- قم بتحديث المستند (`sorafs_architecture_rfc.md`) مرة واحدة عندما تكون هذه القوائم موجودة.## تنفيذ العقد

| تاريا | المسؤول (المسؤولون) | نوتاس |
|------|----------------|------|
| تنفيذ نظام التسجيل (sled/sqlite/off-chain) أو نموذج العقد الذكي. | البنية التحتية الأساسية / فريق العقد الذكي | إثبات تحديد التجزئة، وتجنب النقطة العائمة. |
| نقاط الإدخال: `submit_manifest`، `approve_manifest`، `bind_alias`، `issue_replication_order`، `complete_replication`، `evict_manifest`. | الأشعة تحت الحمراء الأساسية | Aprovechar `ManifestValidator` من خطة التحقق من الصحة. يتدفق الرابط الاسم المستعار الآن عبر `RegisterPinManifest` (DTO de Torii) mientras `bind_alias` المخصص للتحديثات اللاحقة. |
| انتقالات الحالة: تحفيز المتابعة (البيان أ -> ب)، فترات الاحتفاظ، توحيد الاسم المستعار. | مجلس الحكم / البنية الأساسية | اسم مستعار وحدود الاحتفاظ والتحقق من الموافقة/إرجاع الأسبقية موجود في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ اكتشاف Sucesion Multi-Hop وإمكانية النسخ المتماثل مفتوحة. |
| معلمات الإدارة: الشحن `ManifestPolicyV1` من التكوين/حالة الإدارة؛ السماح بالتحديث عبر أحداث الحكومة. | مجلس الحكم | إثبات CLI للتحديث السياسي. |
| بث الأحداث: يصدر الأحداث Norito للقياس عن بعد (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | إمكانية الملاحظة | تعريف حدث الأحداث + التسجيل. |برويباس:
- اختبارات الوحدويين لكل نقطة دخول (إيجابية + ريكازو).
- اختبارات الخصائص لسلسلة النجاح (بدون حلقات، فترات رتيبة تتزايد).
- يظهر Fuzz de validacion generando aleatorios (acotados).

## واجهة الخدمة (Integracion Torii/SDK)

| مكون | تاريا | المسؤول (المسؤولون) |
|-----------|-------|----------------|
| خدمة Torii | Exponer `/v2/sorafs/pin` (إرسال)، `/v2/sorafs/pin/{cid}` (بحث)، `/v2/sorafs/aliases` (قائمة/ربط)، `/v2/sorafs/replication` (الطلبات/الإيصالات). إثبات الصفحة + الفلترة. | الشبكات TL / الأشعة تحت الحمراء الأساسية |
| أستاسيون | تضمين ارتفاع/تجزئة التسجيل في الاستجابة؛ تجميع بنية الشهادة Norito المستهلكة لمجموعات SDK. | الأشعة تحت الحمراء الأساسية |
| سطر الأوامر | الموسع `sorafs_manifest_stub` أو CLI جديد `sorafs_pin` مع `pin submit`، `alias bind`، `order issue`، `registry export`. | الأدوات مجموعة العمل |
| SDK | إنشاء روابط العميل (Rust/Go/TS) من المستوي Norito; Agregar اختبارات التكامل. | فرق SDK |

العمليات:
- قم بجمع سعة ذاكرة التخزين المؤقت/ETag لنقاط النهاية GET.
- إثبات تحديد المعدل / المصادقة على اتساق السياسات مع Torii.

## المباريات وCI- دليل التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يحمي اللقطات الثابتة من البيان/الاسم المستعار/الطلب المُجدد بواسطة `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` تقوم بإعادة إنشاء اللقطة وإيقافها إذا كانت هناك اختلافات، والحفاظ على تركيبات CI المنفصلة.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) لإخراج التدفق من نفس الاسم المستعار المكرر، وحماية/الاحتفاظ بالاسم المستعار، ومقابض القطع المتحللة، والتحقق من صحة محتوى النسخ المتماثلة، وفشل حماية النسخ (النقاط) desconocidos/preaprobados/retirados/autorreferencias); شاهد الحالات `register_manifest_rejects_*` لتفاصيل التغطية.
- الاختبارات الموحدة الآن بعد التحقق من صحة الأسماء المستعارة، وحماية الاحتفاظ، والتحقق من المتابعة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ The Detection of Sucesion Multi-Hop When the Machine Status.
- JSON الذهبي للأحداث المستخدمة من خلال خطوط أنابيب المراقبة.

## القياس عن بعد وإمكانية المراقبة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- يوجد مزودو خدمات القياس عن بعد (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) متواصلون على لوحات المعلومات من البداية إلى النهاية.

السجلات:
- دفق الأحداث Norito الهيكلية لمراجعي الإدارة (الشركات؟).التنبيهات:
- أوامر النسخ المتماثلة التي تتجاوز جيش تحرير السودان (SLA).
- انتهاء صلاحية الاسم المستعار للظل.
- Violaciones de retencion (بيان عدم التجديد قبل انتهاء الصلاحية).

لوحات المعلومات:
- JSON من Grafana `docs/source/grafana_sorafs_pin_registry.json` يوزع إجمالي دورة حياة البيانات، وتغطية الاسم المستعار، وتشبع الأعمال المتراكمة، ونسبة SLA، وتراكبات زمن الاستجابة مقابل الركود، وتكاليف الأوامر المفقودة للمراجعة عند الطلب.

## كتب التشغيل والوثائق

- تحديث `docs/source/sorafs/migration_ledger.md` لإضافة تحديثات حالة التسجيل.
- دليل المشغلين: `docs/source/sorafs/runbooks/pin_registry_ops.md` (الإعلان) عن طريق قياس المقاييس والتنبيهات والنشر والنسخ الاحتياطي وتدفقات الاسترداد.
- دليل الإدارة: وصف المعلمات السياسية، وسير العمل، وإدارة النزاعات.
- الصفحات المرجعية لواجهة برمجة التطبيقات (API) لكل نقطة نهاية (Docusaurus docs).

## التبعيات والتسلسل

1. أكمل خطط التحقق من الصحة (تكامل البيان).
2. تم الانتهاء من الملف Norito + الإعدادات السياسية الافتراضية.
3. تنفيذ العقود + الخدمات، الاتصال بالقياس عن بعد.
4. تجديد التركيبات، وتصحيح مجموعات التكامل.
5. قم بتحديث المستندات/دفاتر التشغيل وتمييز عناصر خريطة الطريق بشكل كامل.

يجب الرجوع إلى كل قائمة مرجعية لـ SF-4 لهذه الخطة عند ظهور تقدم.
يتم إدخال REST الآن إلى نقاط نهاية القائمة مع التحقق:- `GET /v2/sorafs/pin` و `GET /v2/sorafs/pin/{digest}` يشرح البيانات
  روابط الأسماء المستعارة وأوامر النسخ وكائن الإقرار المشتق من
  تجزئة الكتلة الأخيرة.
- `GET /v2/sorafs/aliases` و `GET /v2/sorafs/replication` يعرض الكتالوج
  الاسم المستعار النشط وتراكم أوامر النسخ المتماثل مع صفحات متسقة
  مرشحات الحالة.

يقوم La CLI بتنشيط هذه المكالمات (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) حتى يتمكن المشغلون من أتمتة جلسات الاستماع
التسجيل بدون واجهات برمجة التطبيقات (APIs) على مستوى منخفض.