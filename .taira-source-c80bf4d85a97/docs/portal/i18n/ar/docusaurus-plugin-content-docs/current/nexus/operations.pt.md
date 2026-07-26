---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الترابط
العنوان: Runbook de Operacoes Nexus
الوصف: استئناف سريع للاستخدام في مجال تدفق العمل للمشغل Nexus، espelhando `docs/source/nexus_operations.md`.
---

استخدم هذه الصفحة كمرجع سريع لـ `docs/source/nexus_operations.md`. يتم إنشاء قائمة التحقق التشغيلية، ويتم تشغيلها من قبل طواقم الإدارة ومتطلبات تغطية القياس عن بعد التي يجب على المشغلين Nexus اتباعها.

## قائمة ciclo de vida

| ايتابا | اكويس | ايفيدنسيا |
|-------|--------|----------|
| قبل فو | تحقق من التجزئات/عمليات التحرير، وتأكد من `profile = "iroha3"` وقم بإعداد قوالب التكوين. | صيدا `scripts/select_release_profile.py`، سجل المجموع الاختباري، حزمة البيانات التي تم اغتيالها. |
| الانضمام إلى الكتالوج | قم بتحديث الكتالوج `[nexus]`، وسياسة التناوب وحدود DA المتوافقة مع البيان الصادر عن المجلس، ثم التقاط `--trace-config`. | صيدا `irohad --sora --config ... --trace-config` يتم تخزينها مع تذكرة الصعود. |
| الدخان والقطع | قم بتنفيذ `irohad --sora --config ... --trace-config`، أو قم بالقيادة أو الدخان إلى CLI (`FindNetworkStatus`)، وتحقق من عمليات التصدير للقياس عن بعد وطلب القبول. | سجل اختبار الدخان + تأكيد مدير التنبيه. |
| إستادو إستافيل | مراقبة لوحات المعلومات/التنبيهات، وتدوير الأقراص المتوافقة مع إيقاع الإدارة ومزامنة التكوينات/دفاتر التشغيل عندما يتم تغيير البيانات. | دقائق المراجعة الثلاثية، وإلتقاطات لوحات المعلومات، ومعرفات التذاكر الدوارة. |

تظل تفاصيل الإعداد (استبدال العناصر وقوالب التدوير وتصاريح إصدار الملف) دائمة في `docs/source/sora_nexus_operator_onboarding.md`.

## جيستاو دي مودانكا

1. **تحديث الإصدار** - الإعلان المصاحب في `status.md`/`roadmap.md`; قم بإرفاق قائمة التحقق من الإعداد لكل إصدار من العلاقات العامة.
2. **تعديلات بيان المسار** - تحقق من الحزم التي تم الاستيلاء عليها من دليل الفضاء واحفظها في `docs/source/project_tracker/nexus_config_deltas/`.
3. **دلتا التكوين** - كل التغييرات في `config/config.toml` تتطلب الرجوع إلى تذكرة المسار/مساحة البيانات. احفظ نسخة تم تعديلها من فعالية التكوين عند دخولك أو تحديثك.
4. **تدريبات التراجع** - اتبع إجراءات الإيقاف/الاستعادة/الدخان الثلاثية الأبعاد؛ قم بتسجيل النتائج على `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقات الامتثال** - يجب على الممرات الخاصة/CBDC أن تتجنب الامتثال قبل تغيير السياسة في DA أو مقابض القياس عن بعد (الإصدار `docs/source/cbdc_lane_playbook.md`).

## القياس عن بعد وSLOs

- لوحات المعلومات: `dashboards/grafana/nexus_lanes.json`، `nexus_settlement.json`، المزيد من وجهات النظر المحددة لـ SDK (على سبيل المثال، `android_operator_console.json`).
- التنبيهات: `dashboards/alerts/nexus_audit_rules.yml` وأنظمة النقل Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- مقاييس مراقب:
  - `nexus_lane_height{lane_id}` - تنبيه لصفر تقدم في ثلاث فتحات.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه حول حدود الممر (padrao 64 public / 8 Private).
  - `nexus_settlement_latency_seconds{lane_id}` - تنبيه عندما يتجاوز P99 900 مللي ثانية (عام) أو 1200 مللي ثانية (خاص).
  - `torii_request_failures_total{scheme="norito_rpc"}` - يتم تنبيه فئة الخطأ لمدة 5 دقائق بنسبة أكبر من 2%.
  - `telemetry_redaction_override_total` - Sev 2 immediato؛ ضمان تجاوز تذاكر الامتثال الخاصة بـ Tenham.
- قم بتنفيذ قائمة التحقق من إصلاح القياس عن بعد رقم [Nexus خطة إصلاح القياس عن بعد](./nexus-telemetry-remediation) بأقل من ثلاثة أشهر وأرفق الصيغة المسبقة بملاحظات المراجعة التشغيلية.

## ماتريز دي الحوادث

| قطع | التعريف | الرد |
|----------|-----------|----------|
| سيف 1 | انتهاك عزل مساحة البيانات، أو تسوية التسوية لمدة تزيد عن 15 دقيقة، أو إتلاف صوت الإدارة. | Acione Nexus Primary + Release Engineering + Compliance، congele admissao، colete artefatos، public comunicados <= 60 min، RCA <= 5 dias uteis. |
| سيف 2 | تعطل SLA في الممرات المتراكمة، لمدة تزيد عن 30 دقيقة، وبدء تشغيل البيان. | Acione Nexus Primary + SRE، تخفيف <= 4 ساعات، تسجيل المتابعات في 2 يومًا. |
| سيف 3 | مشتق nao bloqueante (المستندات، التنبيهات). | قم بتسجيل أي متتبع وقم بجدول التصحيح داخل Sprint. |

تحتوي تذاكر الأحداث على معرفات مسجل للمسار/مساحة البيانات، وتجزئات البيان، والجدول الزمني، ومقاييس/سجلات الدعم، والمسارات/مالكي المتابعة.

## ملف الأدلة

- حزم أرمازين/بيانات/صادرات القياس عن بعد في `artifacts/nexus/<lane>/<date>/`.
- Mantenha configs redigidas + saya de `--trace-config` لكل إصدار.
- تفاصيل دقيقة للمشورة + القرارات التي تم اتخاذها عند تعديل التكوين أو البيان.
- الاحتفاظ بلقطات Prometheus ذات الصلة بمقاييس Nexus لمدة 12 شهرًا.
- سجل سجلات التشغيل في `docs/source/project_tracker/nexus_config_deltas/README.md` ليتمكن المدققون من الوثوق به عندما يكونون مسؤولين عن ذلك.

## مادة ذات صلة

- فيساو عام: [نظرة عامة على Nexus](./nexus-overview)
- المواصفات: [Nexus المواصفات](./nexus-spec)
- هندسة الممرات: [Nexus نموذج الممر](./nexus-lane-model)
- حشوات النقل والتحويل: [Nexus ملاحظات الانتقال](./nexus-transition-notes)
- تأهيل المشغلين: [تجهيز مشغل Sora Nexus](./nexus-operator-onboarding)
- علاج القياس عن بعد: [Nexus خطة معالجة القياس عن بعد](./nexus-telemetry-remediation)