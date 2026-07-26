---
lang: ar
direction: rtl
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-11-08T16:26:57.335679+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_operations.md -->

# دليل عمليات Nexus (NX-14)

**رابط خارطة الطريق:** NX-14 — توثيق Nexus ودلائل تشغيل المشغلين
**الحالة:** مسودة 2026-03-24 — متوافقة مع `docs/source/nexus_overview.md` و
تدفق الانضمام في `docs/source/sora_nexus_operator_onboarding.md`.
**الجمهور المستهدف:** مشغلو الشبكة، مهندسو SRE/المناوبة، منسقو الحوكمة.

يلخص هذا الدليل دورة الحياة التشغيلية لعقد Sora Nexus (Iroha 3).
لا يستبدل المواصفة التفصيلية (`docs/source/nexus.md`) او ادلة كل lane
(مثل `docs/source/cbdc_lane_playbook.md`)، لكنه يجمع قوائم التحقق العملية،
ومخططات التلِمتري، ومتطلبات الاثبات التي يجب استيفاؤها قبل قبول او ترقية العقدة.

## 1. دورة الحياة التشغيلية

| المرحلة | قائمة التحقق | الدليل |
|---------|--------------|--------|
| **فحص مسبق** | التحقق من تجزئات/تواقيع الاصول، تأكيد `profile = "iroha3"`، وتجهيز قوالب الاعداد. | مخرجات `scripts/select_release_profile.py`، سجل checksum، وحزمة manifest موقعة. |
| **مواءمة الكتالوج** | تحديث كتالوج lane + dataspace في `[nexus]`، سياسة التوجيه، وحدود DA لتطابق manifest الصادر عن المجلس. | مخرجات `irohad --sora --config ... --trace-config` محفوظة مع التذكرة. |
| **اختبار سريع وقطع** | تشغيل `irohad --sora --config ... --trace-config`، تنفيذ اختبار CLI سريع (مثل `FindNetworkStatus`)، التحقق من نقاط نهاية التلِمتري، ثم طلب القبول. | سجل smoke-test + تأكيد silence في Alertmanager. |
| **حالة مستقرة** | مراقبة لوحات القياس والتنبيهات، تدوير المفاتيح وفق cadence الحوكمة، والحفاظ على اتساق configs + runbooks مع مراجعات manifest. | محاضر مراجعة ربع سنوية، لقطات لوحات مترابطة، ومعرفات تذاكر التدوير. |

تعليمات الانضمام التفصيلية (بما في ذلك استبدال المفاتيح، امثلة سياسة التوجيه، والتحقق من release profile)
موجودة في `docs/source/sora_nexus_operator_onboarding.md`. ارجع اليها عند تغير تنسيقات الاصول او السكربتات.

## 2. ادارة التغييرات ومفاتيح الحوكمة

1. **تحديثات الاصدار**
   - متابعة الاعلانات في `status.md` و `roadmap.md`.
   - كل PR اصدار يجب ان يرفق قائمة التحقق المكتملة من
     `docs/source/sora_nexus_operator_onboarding.md`.
2. **تغييرات lane manifest**
   - تنشر الحوكمة حزم manifest موقعة عبر Space Directory.
   - يتحقق المشغلون من التواقيع، ويحدثون ادخالات الكتالوج، ويؤرشفون
     manifests في `docs/source/project_tracker/nexus_config_deltas/`.
3. **فوارق الاعداد**
   - اي تغيير في `config/config.toml` يتطلب تذكرة تشير الى lane ID والاسم المستعار للـ dataspace.
   - الاحتفاظ بنسخة منقحة من الاعداد الفعلي داخل التذكرة عند الانضمام او الترقية.
4. **تمارين الرجوع**
   - اجراء تمارين رجوع ربع سنوية (ايقاف العقدة، استعادة الحزمة السابقة، اعادة تطبيق الاعداد، واعادة smoke).
     تسجيل النتائج في `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقات الامتثال**
   - يجب على المسارات الخاصة/CBDC الحصول على موافقة الامتثال قبل تغيير سياسة DA او
     مفاتيح تنقيح التلِمتري. ارجع الى `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. التلِمتري وتغطية SLO

يتم اصدار لوحات القياس وقواعد التنبيه تحت `dashboards/` ويتم توثيقها في
`docs/source/nexus_telemetry_remediation_plan.md`. يجب على المشغلين:

- ربط اهداف PagerDuty/المناوبة بملف `dashboards/alerts/nexus_audit_rules.yml`
  وقواعد صحة lane في `dashboards/alerts/torii_norito_rpc_rules.yml`
  (يغطي نقل Torii/Norito).
- نشر لوحات Grafana التالية في بوابة العمليات:
  - `nexus_lanes.json` (ارتفاع lane، backlog، وتكافؤ DA).
  - `nexus_settlement.json` (كمون settlement، فروقات الخزينة).
  - `android_operator_console.json` / لوحات SDK عندما تعتمد lane على
    تلِمتري الهواتف.
- ابقاء مصدري OTEL متماشين مع `docs/source/torii/norito_rpc_telemetry.md` عندما
  يكون نقل Torii الثنائي مفعلا.
- تنفيذ قائمة معالجة التلِمتري على الاقل ربع سنويا (القسم 5 في
  `docs/source/nexus_telemetry_remediation_plan.md`) وارفاق النموذج المكتمل
  بمحاضر مراجعة العمليات.

### المقاييس الرئيسية

| المقياس | الوصف | عتبة التنبيه |
|--------|-------|--------------|
| `nexus_lane_height{lane_id}` | ارتفاع الرأس لكل lane؛ يكشف توقف المدققين. | تنبيه اذا لم ترتفع لمدة 3 فتحات متتالية. |
| `nexus_da_backlog_chunks{lane_id}` | مقاطع DA غير المعالجة لكل lane. | تنبيه فوق الحد المضبوط (الافتراضي: 64 للعام، 8 للخاص). |
| `nexus_settlement_latency_seconds{lane_id}` | الزمن بين commit للـ lane و settlement العالمي. | تنبيه >900ms P99 (عام) او >1200ms (خاص). |
| `torii_request_failures_total{scheme="norito_rpc"}` | عدد اخطاء Norito RPC. | تنبيه اذا كانت نسبة اخطاء 5 دقائق >2%. |
| `telemetry_redaction_override_total` | عمليات override لتنقيح التلِمتري. | تنبيه فوري (Sev 2) ويتطلب تذكرة امتثال. |

## 4. الاستجابة للحوادث

| الشدة | التعريف | الاجراءات المطلوبة |
|-------|---------|--------------------|
| **Sev 1** | خرق عزل data-space، توقف settlement لاكثر من 15 دقيقة، او فساد تصويت الحوكمة. | تنبيه Nexus Primary + Release Engineering + Compliance. تجميد قبول lanes، جمع المقاييس/السجلات، نشر اتصالات الحادث خلال 60 دقيقة، وتقديم RCA خلال 5 ايام عمل. |
| **Sev 2** | backlog lane يتجاوز SLA، نقطة عمياء للتلِمتري لاكثر من 30 دقيقة، فشل rollout لملف manifest. | تنبيه Nexus Primary + SRE، تخفيف خلال 4 ساعات، وتسجيل مهام المتابعة خلال يومي عمل. |
| **Sev 3** | تراجعات غير معطلة (انحراف الوثائق، تنبيه خاطئ). | تسجيل في المتتبع، وجدولة الاصلاح خلال السبريت. |

يجب ان تتضمن تذاكر الحوادث:

1. معرفات lane/data-space المتاثرة و hashes للـ manifest.
2. خط زمني (UTC) مع الكشف، التخفيف، الاستعادة، والاتصالات.
3. مقاييس/لقطات تدعم الكشف.
4. مهام متابعة (مع الملاك/التواريخ) وما اذا كان يلزم تحديث الاتمتة/runbooks.

## 5. الادلة ومسار التدقيق

- **ارشفة الاصول:** حفظ bundles و manifests و telemetry exports تحت
  `artifacts/nexus/<lane>/<date>/`.
- **لقطات الاعداد:** `config.toml` منقح مع مخرجات `trace-config` لكل اصدار.
- **الربط بالحوكمة:** محاضر المجلس والقرارات الموقعة المشار اليها في تذكرة الانضمام او الحادث.
- **تصدير التلِمتري:** لقطات اسبوعية من مقاطع Prometheus TSDB المتعلقة بالـ lane،
  مرفقة بمشاركة التدقيق لمدة لا تقل عن 12 شهرا.
- **اصدارات الدليل:** اي تغيير كبير في هذا الملف يجب ان يتضمن ادخال changelog في
  `docs/source/project_tracker/nexus_config_deltas/README.md` حتى يتمكن المدققون
  من تتبع توقيت تغير المتطلبات.

## 6. موارد ذات صلة

- `docs/source/nexus_overview.md` — ملخص معماري عالي المستوى.
- `docs/source/nexus.md` — المواصفة التقنية الكاملة.
- `docs/source/nexus_lanes.md` — هندسة lanes.
- `docs/source/nexus_transition_notes.md` — خارطة طريق الهجرة.
- `docs/source/cbdc_lane_playbook.md` — سياسات CBDC الخاصة.
- `docs/source/sora_nexus_operator_onboarding.md` — تدفق الاصدار/الانضمام.
- `docs/source/nexus_telemetry_remediation_plan.md` — حواجز التلِمتري.

حافظ على تحديث هذه المراجع عند تقدم البند NX-14 او عند ادخال فئات lanes جديدة
او قواعد تلِمتري جديدة او مفاتيح حوكمة جديدة.

</div>
