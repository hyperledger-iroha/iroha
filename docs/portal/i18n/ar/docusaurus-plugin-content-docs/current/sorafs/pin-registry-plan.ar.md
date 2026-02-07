---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تنفيذ Pin Registry في SoraFS
Sidebar_label: خطة تسجيل الرقم السري
الوصف: خطة تنفيذ SF-4 التي تغطي الحالات للسجل والواجهة Torii والتولنغ والرصد.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/pin_registry_plan.md`. حافظ على النسختين متزامنتين ما دامت الوثائق القديمة.
:::

# خطة تنفيذ Pin Registry في SoraFS (SF-4)

يؤيد SF-4 عقد Pin Registry والمساندة التي تخزن البيان،
وتفرض سياسات pin، وتكشف واجهات API لـ Torii والبوابات ودوات يمكن.
يوسف يعتمد هذا النموذج على خطة التحقق بمهام التنفيذية لتغطية المنطق اللطيف على السلسلة،
المنزلية، والخدمات والـ Installations، والمتطلبات التشغيلية.

## النطاق

1. **سجل حالات الآلة**: السجلات Norito للبيانات والأسماء المستعارة والسلاسل الخلفية
   إصلاحات وبيانات ال تور الوصفية.
2. **تنفيذ العقد**: عمليات CRUD حتمية لدورة حياة دبوس (`ReplicationOrder`, `Precommit`,
   `Completion`، الإخلاء).
3. ** خدمة الواجهة **: نقاط نهاية gRPC/REST مدعومة بالـ التسجيل تستهلكها Torii وSDKs،
   ومن الترقيم والاتستاشن.
4. **التولنغ والـ التركيبات**: مساعدات CLI ومتجهات الاختبار والوثائق للحفاظ على تزامن
   يظهر والأسماء المستعارة والمغلفات الخاصة بال تور.
5. **التليمتري بداية التشغيل**: معايير وتنبيهات وسجلات التشغيل.

## نموذج البيانات

###تطلب الامرة (Norito)| المعرفة | الوصف | بمعنى |
|--------|-------|--------|
| `PinRecordV1` | مانيفست الإدخال كنسي. | `manifest_cid`، `chunk_plan_digest`، `por_root`، `profile_handle`، `approved_at`، `retention_epoch`، `pin_policy`، `successor_of`، `governance_envelope_hash`. |
| `AliasBindingV1` | ربط الاسم المستعار -> CID الخاص بالـ البيان. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات للمنظمين المانيفست. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | قرار المحكم. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة إلى ال تور. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

مرجع التنفيذ: مرجع `crates/sorafs_manifest/src/pin_registry.rs` لخططات Norito في Rust
ومساعدات التحقق التي تدعم هذه السجلات. التحقق من صحة الأدوات الخاصة بالـ البيان
(بحث عن ــ Chunker Registration وpin Policy Gating) دائمًا ان العقد وواجهات Torii وCLI
تتقاسم نفس الثوابت.

:
- انهاء مخططات Norito في `crates/sorafs_manifest/src/pin_registry.rs`.
- توليد مشاريع (Rust + SDKs وغيرها) باستخدام ماكرو Norito.
- تحديث السجلات (`sorafs_architecture_rfc.md`) بعد تثبيت التثبيت.

## تنفيذ العقد| | المالك/المالكون | مذكرة |
|--------|------------------|----------|
| تنفيذ تخزين التسجيل (sled/sqlite/off-chain) او وحدة عقد ذكي. | البنية التحتية الأساسية / فريق العقد الذكي | توفير التجزئة حتمي حدث مشاركة العائمة. |
| نقاط الدخول: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | الأشعة تحت الحمراء الأساسية | استخدام `ManifestValidator` من خطة التحقق. الاسم المستعار للاتصال يمر الان عبر `RegisterPinManifest` (DTO من Torii) بينما يبقى `bind_alias` مخططا لتحديثات لاحقة. |
| انتقالات الحالة: فرض التعاقب (البيان أ -> ب)، عصور الإصلاح، تفرد الاسم المستعار. | مجلس الحكم / البنية الأساسية | تفرد الاسم المستعار وحدود الإصلاح والفحوصات المعتمدة/سحب السلف موجود في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ استعادة التتابع المتعدد للقفزات ودفاتر التكرار ما يمكن استخدامه. |
| المعلمات الحكومية: تحميل `ManifestPolicyV1` من config/حالة التورم؛ مما أدى إلى التحديث عبر احداث ال تور. | مجلس الحكم | توفير CLI لتحديثات السياسة. |
| اصدار الاحداث: اصدار احدث Norito للتليمتري (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | إمكانية الملاحظة | تعريف مخطط الاحداث + التسجيل. |

الموضوع:
- درجة وحدة لكل نقطة دخول (ايجابي + الرفض).
- خصائص خصائص سلسلة التتابع (بدون دورة، عصور معززة).
- Fuzz لخلق عبر توليد بيانات غير مباشرة (مقيدة).

## واجهة خدمة (تكامل Torii/SDK)| المكون | | المالك/المالكون |
|--------|--------|-----------------|
| خدمة Torii | كشف `/v1/sorafs/pin` (إرسال)، `/v1/sorafs/pin/{cid}` (بحث)، `/v1/sorafs/aliases` (قائمة/ربط)، `/v1/sorafs/replication` (الطلبات/الإيصالات). توفير ترقيم + ترشيح. | الشبكات TL / الأشعة تحت الحمراء الأساسية |
| الاتستاشن | تضمين ارتفاع/هاش التسجيل في الاستجابات؛ إضافة بناء Norito للاتستاشن تستهلكها SDKs. | الأشعة تحت الحمراء الأساسية |
| سطر الأوامر | النهائي `sorafs_manifest_stub` او CLI جديد `sorafs_pin` مع `pin submit`, `alias bind`, `order issue`, `registry export`. | الأدوات مجموعة العمل |
| SDK | توليد الارتباطات من أجل (Rust/Go/TS) من مخطط Norito؛ إضافة إلى تكامل. | فرق SDK |

العمليات:
- إضافة ذاكرة التخزين المؤقت/ETag لنقاط نهاية الطبقة GET.
- توفير تحديد المعدل / المصادقة بما يتناسب مع السياسات Torii.

## المباريات و CI- دليل المباريات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` يخزن لقطات موقعة لـ البيان/الاسم المستعار/الأمر يعاد توليدها عبر `cargo run -p iroha_core --example gen_pin_snapshot`.
- خطوة CI: `ci/check_sorafs_fixtures.sh` يمكن إضافة اللقطة وفشل عند وجود الإكتشافات، لتحافظ على تماهي التركيبات الخاصة بـ CI.
- تكامل (`crates/iroha_core/tests/pin_registry.rs`) تغطية المسار السعيد مع رفض الاسم المستعار المكرر، وحمايات تعتمد/احتفاظ بالاسم المستعار، والمقابض غير متطابقة لـchunker، والتحقق من عدد النسخ، وفشل حمايات التتابع ( مؤشرات مجهولة/موافَق عليها مسبقاً/مسحوبة/ذاتية الاشارة)؛ مراجعة حالات `register_manifest_rejects_*` لتفاصيل التغطية.
- الوحدة تغطي الان التحقق من الاسم المستعار وحمايات الإصلاح والفحوصات السابقة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ كشف التتابع الجديد قفزات متعددة عند وصول الحالات.
- JSON ذهبي للاحداث المستخدمة في خطوط الرصد.

## الرصد والرصد

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- تليمتري المزود الحالي (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) يظل ضمن النطاق للوحـات end-to-end.

سجل:
- تيار احداث Norito منظم لدقيقات التصفح (موقع؟).

التنبيهات:
- اوامر تعدى SLA.
- انتهاء صلاحية الاسم المستعار أقل من العتبة.
- أضرار الأضرار (بيان لم يجدد قبل الانتهاء).

معلومات اللوحات:
- ملف Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` يتتبع اجمالي دورة البيانات، تغطية الأسماء المستعارة، تشبع backlog، نسبة SLA، تراكب latency مقابل slack، اشتراكات الاوامر الفاشلة للمراجعة أثناء النوبة.

## الدفاتر والوثائق- تحديث `docs/source/sorafs/migration_ledger.md` لضمين تحديثات حالة التسجيل.
- دليل التشغيل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (منشور حاليا) إعدادات المعايير والتنبيه والنشر والنسخ الاحتياطي واستعادة الخدمة.
- دليل الـ تور: وصف معلمات السياسة وسير العمل الاعتماد ولا يمكن.
- صفحات مرجع API لكل نقطة نهاية (Docusaurus).

## الاعتماديات والسلسلة

1. أكمل مهام خطة التحقق (مدمج ManifestValidator).
2. انهاء مخطط Norito + القيم الافتراضية.
3. تنفيذ العقد + خدمة وربط الليمتري.
4. إعادة توليد التركيبات وتشغيلها.
5. تحديث الوثائق/دفاتر التشغيل العلامة القانونية على عناصر خارطة الطريق.

يجب ان تشاهد كل قائمة تحقق ضمن SF-4 الى هذه البناء عند تسجيل التقدم.
واجهة REST تتطلب الان نهاية نقاط قائمة مع اتستاشن:

- `GET /v1/sorafs/pin` و `GET /v1/sorafs/pin/{digest}` يتجلى مع
  ربط الأسماء المستعارة واوامر التكرار وكائن اتستاشن مشغل من هاش اخر كتلة.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` ليزران كتالوج الاسم المستعار
  يجب وتراكم اوامر التكرار بترقيم ثابت ومرشحات الحالة.

تغلف CLI هذه الاستدعاءات (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) حتى يبدأون من اتمتة تدقيقات التسجيل بدون لمس
واجهات API المستوى المنخفض.