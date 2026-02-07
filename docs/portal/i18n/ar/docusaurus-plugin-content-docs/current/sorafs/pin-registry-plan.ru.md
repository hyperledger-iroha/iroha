---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: خطة تحقيق رقم التعريف الشخصي للتسجيل SoraFS
Sidebar_label: خطة تسجيل الدبوس
الوصف: خطة تحقيق SF-4، تسجيل الجهاز المميز، المرحلة Torii، الأدوات والمراقبة.
---

:::note Канонический источник
يتم إرسال هذا الشريط إلى `docs/source/sorafs/pin_registry_plan.md`. قم بالنسخ المتزامن بعد تفعيل التوثيق التالي.
:::

# خطة تحقيق Pin Registry SoraFS (SF-4)

SF-4 ينشر عقد التسجيل وخدمات الدعم التي يتم تقديمها
بيان الالتزام، تثبيت دبوس السياسة وتقديم واجهة برمجة التطبيقات لـ Torii،
المقاطع الموسيقية والمنسقين. هذه الوثيقة تتضمن خطة التحقق من صحة الخرسانة
تحقيق المزيد, تعزيز المنطق على السلسلة, خدمات المضيفين,
التركيبات والتشغيل.

## Область1. **التسجيل الأساسي للجهاز**: اكتب Norito للبيانات والأسماء المستعارة،
   المزايا العامة، عصر الحداثة والإدارة التحويلية.
2. **عقد التحقق**: تحديد عمليات CRUD للحياة
   دبوس цикла (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **الأسلوب الخدمي**: نقاط نهاية gRPC/REST، وتشغيل التسجيل، و
   يتضمن Torii وSDK، بما في ذلك الصفحات والشهادة.
4. **الأدوات والتركيبات**: مساعدو واجهة سطر الأوامر (CLI) ومتجهات الاختبارات والوثائق
   بيانات المزامنة والأسماء المستعارة ومظاريف الإدارة.
5. **قياس المسافة والعمليات**: المقاييس والتنبيهات وسجلات التشغيل لإغلاق التسجيل.

## نموذج البيانات

### السجل الأساسي (Norito)| الهيكل | الوصف | بوليا |
|----------|---------|------|
| `PinRecordV1` | kanonicheская запись بيان. | `manifest_cid`، `chunk_plan_digest`، `por_root`، `profile_handle`، `approved_at`، `retention_epoch`، `pin_policy`، `successor_of`، `governance_envelope_hash`. |
| `AliasBindingV1` | Сопоставляет الاسم المستعار -> بيان إدارة البحث الجنائي. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات لمقدمي الخدمة لفك البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | مقدم الطلب. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | الإدارة السياسية البسيطة. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

طريقة التنفيذ: سم. `crates/sorafs_manifest/src/pin_registry.rs` للنظام Norito
في الصدأ والمساعدين في التحقق من ذلك، يتم سردها في البداية. التحقق من الصحة
ابحث عن أدوات البيان (تسجيل مقسم البحث، بوابة سياسة الدبوس)، لذلك
العقد، تم إنشاء الواجهات Torii وCLI بثوابت متطابقة.

المزيد:
- قم بإنهاء المخططات Norito إلى `crates/sorafs_manifest/src/pin_registry.rs`.
- إنشاء كود (Rust + SDK آخر) باستخدام وحدة الماكرو Norito.
- التعرف على الوثائق (`sorafs_architecture_rfc.md`) بعد نظام الاتصال.

## عقد التجسيد| زادا | إجابة | مساعدة |
|--------|--------------|----------|
| تحقيق ذاكرة التسجيل (sled/sqlite/off-chain) أو نموذج العقد الذكي. | البنية التحتية الأساسية / فريق العقد الذكي | يجب تحديد التجزئة، وحذف النقطة العائمة. |
| نقاط الإدخال: `submit_manifest`، `approve_manifest`، `bind_alias`، `issue_replication_order`، `complete_replication`، `evict_manifest`. | الأشعة تحت الحمراء الأساسية | استخدم `ManifestValidator` من التحقق من صحة الخطة. يتم تنفيذ الاسم المستعار الملزم من خلال `RegisterPinManifest` (DTO Torii)، ثم يتم وضع `bind_alias` النموذجي في الخطة أحدث التطورات. |
| Пеreоды состояния: обеспечивать пемственность (manifest A -> B)، epоhi charaneniya، اسم مستعار فريد. | مجلس الحكم / البنية الأساسية | الاسم المستعار الفريد، حدود السعة والتحقق من الجودة/الحياة السابقة للحياة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ يتم الكشف عن العديد من المزايا والتكرارات الكبيرة. |
| معلمات التحكم: قم بإلغاء تأمين `ManifestPolicyV1` من إدارة التكوين/الإعدادات؛ التخلص من الإدمان من خلال إدارة الرعاية الاجتماعية. | مجلس الحكم | يقترح CLI للسياسة السياسية. |
| الرسالة الخاصة: شراء البيانات Norito لأجهزة القياس عن بعد (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | إمكانية الملاحظة | قم بطرح مخطط الاشتراك + التسجيل. |الاختبار:
- اختبارات لكل نقطة دخول (إيجابية + سيناريوهات مميزة).
- اختبارات الملكية للأشياء المشروطة (باستثناء السيكلوف، الرتابة الرتيبة).
- التحقق من صحة الزغب من خلال بيانات الإنشاءات (مع الامتيازات).

## النظام الخدمي (التكامل Torii/SDK)

| المكون | زادا | إجابة |
|-----------|-------|---------------|
| خدمة Torii | Экспонировать `/v1/sorafs/pin` (إرسال)، `/v1/sorafs/pin/{cid}` (بحث)، `/v1/sorafs/aliases` (قائمة/ربط)، `/v1/sorafs/replication` (الطلبات/الإيصالات). قم بإلغاء ترقيم الصفحات + التصفية. | الشبكات TL / الأشعة تحت الحمراء الأساسية |
| شهادة | قم بإلغاء تسجيل الدخول/التسجيل في الإجابات; إضافة هيكل التصديق Norito، مطلوب SDK. | الأشعة تحت الحمراء الأساسية |
| سطر الأوامر | قم بتغيير `sorafs_manifest_stub` أو CLI الجديد `sorafs_pin` إلى `pin submit`، `alias bind`، `order issue`، `registry export`. | الأدوات مجموعة العمل |
| SDK | إنشاء روابط العملاء (Rust/Go/TS) من الأنظمة Norito; إضافة اختبارات التكامل. | فرق SDK |

العمليات:
- إضافة سلوي ذاكرة التخزين المؤقت/ETag للحصول على نقاط النهاية.
- اقتراح تحديد المعدل / المصادقة في السياسة Torii.

## المباريات وCI- تركيبات الكتالوج: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` charnit подписанные snapshots Manifest/alias/order، يتم تحويلها من خلال `cargo run -p iroha_core --example gen_pin_snapshot`.
- CI CI: `ci/check_sorafs_fixtures.sh` ينقل لقطة ويلتقط الاختلافات، ويدير تزامن تركيبات CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تظهر المسار السعيد بالإضافة إلى الخروج عند الاسم المستعار للدبلجة، الاسم المستعار للموافقة/الشرف، غير قابل للتخصيص مقابض القطع، والتحقق من نسخة طبق الأصل وإخراج الحراس المتفوقين (غير معروف/متاح/متنوع/ساموسيلكي)؛ سم. اضغط على `register_manifest_rejects_*` لتفاصيل الشاشة.
- تقوم الاختبارات بفحص التحقق من الاسم المستعار وحرس الحراس والتحقق من صحة الاسم المستعار؛ هناك العديد من الإخطارات التي تشير إلى أن الماكينة جاهزة تمامًا.
- Golden JSON للأشخاص الذين يستخدمون أجهزة الكمبيوتر الشخصية.

## القياس عن بعد والمراقبة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- جهاز قياس الاتصال الخاص بموفر الخدمة (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) موجود في منطقة للوحة البيانات من طرف إلى طرف.

الشعارات:
- الهيكل الهيكلي للوحدة Norito لإدارة مدققي الحسابات (التقديم؟).

التنبيهات:
- بسبب النسخ المتماثل في الخدمة، SLA السابق.
- إنشاء اسم مستعار جديد.
- ضيق التنفس (البيان لا يمتد إلى النشأة).لوحة القيادة:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` يتتبع إجماليات بيانات الدورة الحيوية، والاسم المستعار للطباعة، والتراكم المتراكم، ونسبة SLA، وتراكب الكمون مقابل الركود، والتكلفة العبارات المقترحة للمراجعة عند الطلب.

## دفاتر التشغيل والوثائق

- قم بتثبيت `docs/source/sorafs/migration_ledger.md` لإدراج حالة التسجيل.
- مشغل التشغيل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (متاح حاليًا) مزود بمقاييس وتنبيهات وتحديثات ونسخ احتياطي ودعم.
- Руководство по управлению: описать parameters politicies, Workflow одобения, обработку sporov.
- واجهة برمجة التطبيقات الخاصة بكل نقطة نهاية (Docusaurus docs).

## المعرفة والتقدم

1. الانتهاء من التحقق من صحة الخطة (التكامل ManifestValidator).
2. الانتهاء من النظام Norito + الإعدادات الافتراضية.
3. تحقيق العقد + الخدمة، إضافة إلى القياس عن بعد.
4. إعادة تنظيم التركيبات ودمج جناح التكامل.
5. قم بتنزيل المستندات/دفاتر التشغيل وحذف النقاط الدقيقة في خارطة الطريق بشكل أفضل.

يتم اختيار كل نقطة من نقاط SF-4 من خلال هذه الخطة من خلال التقدم المحرز.
يتم وضع مرحلة REST مع قائمة نقاط النهاية المعتمدة:- `GET /v1/sorafs/pin` و `GET /v1/sorafs/pin/{digest}` يظهران مع
  ارتباطات الاسم المستعار، وتكرار الأوامر، والشهادات الموضوعية، المنتجة من
  هذه هي الكتلة التالية.
- `GET /v1/sorafs/aliases` و `GET /v1/sorafs/replication` نشاط النشر
  الاسم المستعار للكتالوج والمتراكمة بسبب النسخ المتماثل مع صفحات متسقة و
  حالة الترشيح.

تستجيب سطر الأوامر لهذا الصوت (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`)، لكي يتمكن المشغلون من أتمتة تسجيل التدقيق
بدون الخضوع لواجهة برمجة التطبيقات غير المرغوب فيها.