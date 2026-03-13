---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تنفيذ رقم التعريف الشخصي لـ SoraFS
Sidebar_label: Plan du Pin Registry
الوصف: خطة التنفيذ SF-4 تغطي آلة التسجيل والواجهة Torii والأدوات وإمكانية الملاحظة.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/pin_registry_plan.md`. قم بمزامنة نسختين حتى تظل الوثائق الموروثة نشطة.
:::

# خطة تنفيذ Pin Registry لـ SoraFS (SF-4)

SF-4 يحرر عقد Pin Registry والخدمات التي يتم دعمها من المخزون
التزامات البيان، تطبيق سياسات التثبيت وكشفها
واجهة برمجة التطبيقات (API) إلى Torii والبوابات المساعدة والمنسقين المساعدين. هذه الوثيقة étend لو الخطة دي
التحقق من صحة تقنيات التنفيذ الخرسانية يغطي المنطق
على السلسلة والخدمات السريعة والتركيبات والمتطلبات التشغيلية.

## بورتيه1. **جهاز تسجيل البيانات**: يقوم بتسجيل Norito للبيانات، والأسماء المستعارة،
   سلاسل الخلافة، وفترات الاحتفاظ، وفترات الحوكمة.
2. **تنفيذ العقد**: عمليات CRUD تحدد دورة الحياة
   الدبابيس (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **واجهة الخدمة**: نقاط النهاية gRPC/REST مدعومة بالسجل المستهلك وفقًا لـ Torii
   ومجموعات SDK، مع ترقيم الصفحات والشهادة.
4. **الأدوات والتركيبات**: مساعدات CLI، ناقلات الاختبار والوثائق
   حراسة البيانات والأسماء المستعارة ومغلفات الإدارة بشكل متزامن.
5. **القياس عن بعد والعمليات**: المقاييس والتنبيهات ودفاتر التشغيل لسلامة التسجيل.

## نموذج البيانات

### مبادئ التسجيل (Norito)| هيكل | الوصف | الأبطال |
|--------|-----------|--------|
| `PinRecordV1` | المدخل الكنسي للبيان. | `manifest_cid`، `chunk_plan_digest`، `por_root`، `profile_handle`، `approved_at`، `retention_epoch`، `pin_policy`، `successor_of`، `governance_envelope_hash`. |
| `AliasBindingV1` | الاسم المستعار Mappe -> بيان CID. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات حول كيفية قيام مقدمي الخدمة بتحديد البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du Provider. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة من سياسة الحكم. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

مرجع التنفيذ: انظر `crates/sorafs_manifest/src/pin_registry.rs` للملفات
المخططات Norito في Rust والمساعدون في التحقق من الصحة الذين يدعمون هذه التسجيلات.
يعكس التحقق من صحة بيان الأدوات (البحث في سجل القطعة، وبوابة سياسة الدبوس)
من أجل التعاقد، الواجهات Torii وCLI جزء من الثوابت المتطابقة.

اللمس :
- إنهاء المخططات Norito في `crates/sorafs_manifest/src/pin_registry.rs`.
- قم بإنشاء الكود (Rust + SDKs الأخرى) عبر وحدات الماكرو Norito.
- قم بمراجعة التوثيق (`sorafs_architecture_rfc.md`) ثم ضع المخططات في مكانها.##تنفيذ العقد| تاش | المالك (المالكون) | ملاحظات |
|-------|----------|-------|
| قم بتنفيذ مخزن التسجيل (sled/sqlite/off-chain) أو وحدة العقد الذكي. | البنية التحتية الأساسية / فريق العقد الذكي | قم بتوفير تحديد تجزئة لتجنب التعويم. |
| نقاط الإدخال: `submit_manifest`، `approve_manifest`، `bind_alias`، `issue_replication_order`، `complete_replication`، `evict_manifest`. | الأشعة تحت الحمراء الأساسية | اضغط على `ManifestValidator` من خطة التحقق من الصحة. يتم تمرير الاسم المستعار الملزم بشكل مستمر إلى `RegisterPinManifest` (DTO Torii يعرض) بينما `bind_alias` تم تجميده مسبقًا للأحداث المتعاقبة. |
| انتقالات الحالة: فرض الخلافة (البيان أ -> ب)، فترات الاحتفاظ، توحيد الأسماء المستعارة. | مجلس الحكم / البنية الأساسية | تظل وحدة الأسماء المستعارة وحدود الاحتفاظ وعمليات التحقق من الموافقة/سحب الأسلاف حية في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ؛ يتم الكشف عن اكتشاف التتابع متعدد القفزات ومسك الدفاتر للنسخ المتماثل. |
| المعلمات الحاكمة : شاحن `ManifestPolicyV1` من خلال التكوين/حالة الحكم ؛ السماح للأحداث اليومية عبر أحداث الحكم. | مجلس الحكم | Fournir une CLI لأحداث السياسة. |
| بث الأحداث : قم بتشغيل الأحداث Norito للقياس عن بعد (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | إمكانية الملاحظة | تحديد مخطط الأحداث + التسجيل. |الاختبارات :
- اختبارات الوحدويين لنقطة دخول الشاكي (positif + rejet).
- اختبارات الملكية لسلسلة الخلافة (فترة الدورات، العصور الرتيبة).
- Fuzz de validation en générant des البيانات البديلة (bornés ).

## واجهة الخدمة (Intégration Torii/SDK)

| مركب | تاش | المالك (المالكون) |
|-----------|------|----------|
| الخدمة Torii | Exposer `/v2/sorafs/pin` (إرسال)، `/v2/sorafs/pin/{cid}` (بحث)، `/v2/sorafs/aliases` (قائمة/ربط)، `/v2/sorafs/replication` (الطلبات/الإيصالات). ترقيم الصفحات + الترشيح. | الشبكات TL / الأشعة تحت الحمراء الأساسية |
| تصديق | قم بتضمين أعلى/تجزئة التسجيل في الردود؛ أضف بنية المصادقة Norito المستهلكة بواسطة مجموعات SDK. | الأشعة تحت الحمراء الأساسية |
| سطر الأوامر | انتهى `sorafs_manifest_stub` أو CLI جديد `sorafs_pin` مع `pin submit`، `alias bind`، `order issue`، `registry export`. | الأدوات مجموعة العمل |
| SDK | إنشاء روابط العميل (Rust/Go/TS) من خلال المخطط Norito ; إضافة اختبارات التكامل. | فرق SDK |

العمليات :
- أضف طبقة من ذاكرة التخزين المؤقت/ETag لنقاط النهاية GET.
- تحديد معدل Fournir / auth cohérents avec les Politiques Torii.

## تركيبات وCI- ملف التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` مخزون اللقطات الموقعة من البيان/الاسم المستعار/الطلب المُعاد إنشاؤه عبر `cargo run -p iroha_core --example gen_pin_snapshot`.
- شريط CI : `ci/check_sorafs_fixtures.sh` ينشئ اللقطة ويلتقطها في حالة الاختلاف، مع محاذاة تركيبات CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تشمل المسار السعيد بالإضافة إلى رفض الأسماء المستعارة المكررة، ووحدات حماية الموافقة/الاحتفاظ، ومقابض القطع غير المتوافقة، والتحقق من صحة حساب النسخ المتماثلة، وفحوصات حماية التسلسل (المؤشرات) inconnus/pre-approvés/retirés/auto-référencés) ; انظر إلى الحالة `register_manifest_rejects_*` للحصول على تفاصيل الغطاء.
- تغطي الاختبارات الوحدوية التحقق من صحة الأسماء المستعارة ووحدات الاحتفاظ وفحوصات النجاح في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ اكتشاف الخلافة متعددة القفزات في آلة الحالات.
- JSON الذهبي للأحداث المستخدمة من خلال خطوط الأنابيب القابلة للمراقبة.

## التحكم عن بعد وقابلية الملاحظة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- موفر خدمة القياس عن بعد الموجود (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) موجود في نطاق لوحات المعلومات من البداية إلى النهاية.

السجلات :
- هيكل تدفق الأحداث Norito لعمليات تدقيق الحوكمة (التوقيعات؟).التنبيهات :
- Ordres de réplication en attente dépassant le SLA.
- انتهاء الاسم المستعار < seuil.
- انتهاكات الاحتجاز (بيان غير متجدد قبل انتهاء الصلاحية).

لوحات المعلومات :
- يتناسب JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` مع كامل دورة حياة البيانات، والغطاء المستعار، وتشبع الأعمال المتراكمة، ونسبة SLA، وتراكبات الكمون مقابل الركود، ومجموعات الأوامر المفقودة للعرض عند الطلب.

## أدلة التشغيل والوثائق

- Mettre à jour `docs/source/sorafs/migration_ledger.md` لتضمين بيانات حالة التسجيل في اليوم.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور من قبل) يغطي المقاييس، والتنبيه، والنشر، والحفظ، وتدفق التكرار.
- دليل الحوكمة: تحديد إعدادات السياسة، وسير العمل بالموافقة، وإدارة الدعاوى القضائية.
- الصفحات المرجعية API لكل نقطة نهاية (docs Docusaurus).

## التبعيات والتسلسل

1. قم بإنهاء لمسات خطة التحقق من الصحة (بيان التكامل).
2. قم بإنهاء المخطط Norito + الإعدادات السياسية الافتراضية.
3. قم بتنفيذ العقد + الخدمة، ثم انتقل إلى الاتصال عن بعد.
4. قم بإعادة تركيب التركيبات وتنفيذ مجموعات التكامل.
5. قم بالاطلاع على المستندات/دفاتر التشغيل وحدد عناصر خريطة الطريق كاملة.

كل عنصر من قائمة التحقق SF-4، يرجى الرجوع إلى هذه الخطة عند تسجيل التقدم.
تشهد واجهة REST livre المضطربة لنقاط نهاية القائمة :- `GET /v2/sorafs/pin` و`GET /v2/sorafs/pin/{digest}` يراجع البيانات مع
  روابط الأسماء المستعارة وأوامر النسخ وكائن المصادقة المشتق من التجزئة
  كتلة دو ديرنير.
- `GET /v2/sorafs/aliases` و`GET /v2/sorafs/replication` يعرض الكتالوج
  الاسم المستعار النشط وتراكم أوامر النسخ مع صفحة متماسكة
  ومرشحات الحالة.

La CLI تغليف ces appels (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) للسماح لمشغلي عمليات التدقيق التلقائية
التسجيل بدون لمس aux APIs ذو المستوى الأساسي.