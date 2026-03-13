---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة التسجيل
العنوان: Plano de Implementacao do Pin Registry do SoraFS
Sidebar_label: Plano do Pin Registry
الوصف: خطة تنفيذ SF-4 cobrindo لآلة حالة التسجيل، وإعداد Torii، والأدوات وقابلية المراقبة.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/pin_registry_plan.md`. الحفاظ على العمل كنسخ متزامنة أثناء وجود مستند دائم.
:::

# خطة تنفيذ سجل Pin لـ SoraFS (SF-4)

يدخل SF-4 إلى عقد Pin Registry وخدمات المساعدة في التخزين
تنازلات البيان، وفرض السياسة على Pin وإظهار واجهات برمجة التطبيقات لـ Torii،
البوابات والأوركسترادور. هذا المستند واسع النطاق أو مخطط التحقق من الصحة com
مهام تنفيذ الخرسانة، وتطبيق المنطق على السلسلة، والخدمات التي تقوم بها
المضيف والتركيبات ومتطلبات التشغيل.

##اسكوبو1. **آلة حالة التسجيل**: السجلات المحددة لـ Norito للبيانات،
   الأسماء المستعارة، والسلاسل اللاحقة، وفترات الاحتفاظ، وامتدادات الحوكمة.
2. **تنفيذ العقود**: العمليات الحتمية الخام لسلسلة الحياة
   دبابيس دوس (`ReplicationOrder`، `Precommit`، `Completion`، الإخلاء).
3. **واجهة الخدمة**: نقاط النهاية gRPC/REST المدعومة بالتسجيل Torii
   تستهلك أدوات تطوير البرامج (SDK) لنظام التشغيل، بما في ذلك الصفحة والمصادقة.
4. **الأدوات والتركيبات**: مساعدات CLI، واختبارات الاختبار، والتوثيق للمساعدة
   البيانات والأسماء المستعارة والمغلفات الحكومية المتزامنة.
5. **القياس عن بعد والعمليات**: المقاييس والتنبيهات ودفاتر التشغيل لجميع عمليات التسجيل.

## موديلو دي دادوس

### السجلات المركزية (Norito)| هيكل | وصف | كامبوس |
|--------|----------|--------|
| `PinRecordV1` | مدخل الكنسي للبيان. | `manifest_cid`، `chunk_plan_digest`، `por_root`، `profile_handle`، `approved_at`، `retention_epoch`، `pin_policy`، `successor_of`، `governance_envelope_hash`. |
| `AliasBindingV1` | الاسم المستعار Mapeia -> CID de Manifest. | `alias`، `manifest_cid`، `bound_at`، `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات لمقدمي الخدمات لإصلاح البيان. | `order_id`، `manifest_cid`، `providers`، `redundancy`، `deadline`، `policy_hash`. |
| `ReplicationReceiptV1` | تأكيد القيام بالموفر. | `order_id`، `provider_id`، `status`، `timestamp`، `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة من سياسة الحكم. | `min_replicas`، `max_retention_epochs`، `allowed_profiles`، `pin_fee_basis_points`. |

مرجع التنفيذ: veja `crates/sorafs_manifest/src/pin_registry.rs` لنظام التشغيل
يظهر هذا الملف Norito في Rust والمساعدون في التحقق من صحة هذه السجلات. أ
التحقق من الصحة أو أدوات البيان (البحث عن التسجيل المقسم، بوابة سياسة الدبوس)
وفقًا للعقد، مثل الواجهات Torii وCLI التي تشارك الثوابت المتطابقة.

طرفاس:
- تم الانتهاء من الأسئلة Norito في `crates/sorafs_manifest/src/pin_registry.rs`.
- تغيير الكود (Rust + Outros SDKs) باستخدام وحدات الماكرو Norito.
- قم بتحديث المستند (`sorafs_architecture_rfc.md`) عندما يتم تجميده.

## تنفيذ العقود| طريفة | الاستجابة(هو) | نوتاس |
|--------|-----------------|-------|
| قم بتنفيذ تخزين التسجيل (sled/sqlite/off-chain) أو نموذج العقد الذكي. | البنية التحتية الأساسية / فريق العقد الذكي | من أجل حتمية التجزئة، تجنب الاستمرار في التقلب. |
| نقاط الإدخال: `submit_manifest`، `approve_manifest`، `bind_alias`، `issue_replication_order`، `complete_replication`، `evict_manifest`. | الأشعة تحت الحمراء الأساسية | أبروفيتار `ManifestValidator` من خطة التحقق. ربط الاسم المستعار الحالي عبر `RegisterPinManifest` (DTO do Torii) أثناء `bind_alias` مخصص للتخطيط التالي لتحديث النجاحات. |
| انتقالات الحالة: أهمية النجاح (البيان أ -> ب)، فترات الاحتفاظ، توحيد الاسم المستعار. | مجلس الحكم / البنية الأساسية | توحيد الأسماء المستعارة وحدود الاحتفاظ وفحوصات الموافقة/سحب الأسلاف موجودة في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ اكتشاف النجاح متعدد القفزات ومسك الدفاتر الدائم في الفتح. |
| المعلمات الحاكمة: حمل `ManifestPolicyV1` من التكوين/حالة الإدارة؛ السماح بالتجديد عبر أحداث الحكم. | مجلس الحكم | Fornecer CLI لتحديث السياسة. |
| إرسال الأحداث: إرسال الأحداث Norito للقياس عن بعد (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | إمكانية الملاحظة | تعريف حدث الأحداث + التسجيل. |الخصية:
- نقطة دخول الخصيتين الوحدويتين (positivo + rejeicao).
- اختبارات الملكية لمجموعة من النجاح (sem ciclos، epocas monotonicamente crescentes).
- يظهر Fuzz de validacao gerando خيارات غير محدودة (الحدود).

## واجهة الخدمة (Integracao Torii/SDK)

| مكون | طريفة | الاستجابة(هو) |
|------------|-------|-----------------|
| سيرفيكو Torii | Expor `/v2/sorafs/pin` (إرسال)، `/v2/sorafs/pin/{cid}` (بحث)، `/v2/sorafs/aliases` (قائمة/ربط)، `/v2/sorafs/replication` (الطلبات/الإيصالات). Fornecer paginacao + تصفية. | الشبكات TL / الأشعة تحت الحمراء الأساسية |
| اتستاكاو | قم بتضمين الارتفاع/تجزئة التسجيل في الاستجابة؛ إضافة نموذج التحقق Norito لمجموعات SDK المستهلكة. | الأشعة تحت الحمراء الأساسية |
| سطر الأوامر | المرسل `sorafs_manifest_stub` أو CLI الجديد `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | الأدوات مجموعة العمل |
| SDK | إنشاء روابط العميل (Rust/Go/TS) من المستوى Norito؛ إضافة الخصيتين التكاملية. | فرق SDK |

العمليات:
- إضافة ذاكرة التخزين المؤقت/ETag لنقاط النهاية GET.
- الحد من معدل Fornecer / المصادقة يتسق مع ما يفعله السياسيون Torii.

## المباريات وCI- دليل التركيبات: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` لقطات مخزنة تمت مهاجمتها من البيان/الاسم المستعار/تم تجديد الطلب بواسطة `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot e falha se hover diffs، مع الحفاظ على تركيبات CI alinhados.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`) تمارس تدفقًا أفضل من خلال استعادة الاسم المستعار المكرر، وحماية المصادقة/الاحتفاظ بالاسم المستعار، ومقابض القطع غير المتوافقة، والتحقق من صحة النسخ المتماثلة، وخطأ حماية النجاح (الطول) desconhecidos/preaprovados/retirados/autorreferencias); شاهد الحالة `register_manifest_rejects_*` لتفاصيل التغطية.
- الخصيتين الوحدويتين الآن تشملان التحقق من الاسم المستعار، وحماية الاحتفاظ وشيكات الوريث في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`؛ اكتشاف النجاح متعدد القفزات عند تشغيل الآلة.
- JSON الذهبي للأحداث المستخدمة في خطوط أنابيب المراقبة.

## القياس عن بعد وإمكانية المراقبة

المقاييس (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- مقياس عن بعد موجود من قبل مقدمي الخدمة (`torii_sorafs_capacity_*`، `torii_sorafs_fee_projection_nanos`) متاح دائمًا للوحات المعلومات من طرف إلى طرف.

السجلات:
- دفق الأحداث Norito التي تم تصميمها لمحاكمات الاستماع (القتلة؟).

التنبيهات:
- أوامر النسخ المعلقة تتجاوز جيش تحرير السودان.
- انتهاء الصلاحية من الاسم المستعار إلى الحد الأقصى.
- Violacoes de retencao (manifest nao renovado antes de expirar).لوحات المعلومات:
- O JSON do Grafana `docs/source/grafana_sorafs_pin_registry.json` راستريا كاملة لسلسلة الحياة من البيانات، تغطية الاسم المستعار، إشباع الأعمال المتراكمة، تجزئة SLA، تراكبات زمن الاستجابة مقابل slack وضرائب أوامر الخسارة للمراجعة عند الطلب.

## Runbooks و documentacao

- تحديث `docs/source/sorafs/migration_ledger.md` لتضمين تحديث حالة التسجيل.
- دليل المشغل: `docs/source/sorafs/runbooks/pin_registry_ops.md` (تم نشره) يتضمن مقاييس وتنبيهات ونشر ونسخ احتياطي وتدفقات استرداد.
- دليل الإدارة: الكشف عن المعلمات السياسية، وسير العمل في الموافقة، ومعالجة النزاعات.
- الصفحات المرجعية لواجهة برمجة التطبيقات (API) لكل نقطة نهاية (docs Docusaurus).

## التبعيات والتسلسل

1. إكمال تعريفات خطة التحقق (تكامل بيان المصادقة).
2. تم الانتهاء من الملف Norito + الإعدادات السياسية الافتراضية.
3. تنفيذ عقد + خدمة، توصيل القياس عن بعد.
4. تجديد التركيبات وأجنحة القضبان المتكاملة.
5. قم بتحديث المستندات/دفاتر التشغيل وتمييز عناصر خريطة الطريق بشكل كامل.

يجب أن تشير كل قائمة مرجعية من SF-4 إلى هذه الخطة عند التقدم.
قم بإعداد REST قبل إدخال نقاط النهاية في القائمة مع التحقق:- `GET /v2/sorafs/pin` و `GET /v2/sorafs/pin/{digest}` بيانات إعادة التدوير com
  روابط الأسماء المستعارة وأوامر النسخ وكائن المصادقة المشتق من ذلك
  التجزئة تفعل كتلة ultimo.
- `GET /v2/sorafs/aliases` و `GET /v2/sorafs/replication` معرض أو كتالوج دي
  الاسم المستعار ativo e o backlog de أوامر النسخ المتماثل مع الصفحات المتسقة e
  مرشحات الحالة.

تغليف CLI هو essas chamadas (`iroha app sorafs pin list`، `pin show`، `alias list`،
`replication list`) لكي يقوم المشغلون بأتمتة القاعات
قم بالتسجيل في واجهات برمجة التطبيقات (APIs) على المستوى الأساسي.