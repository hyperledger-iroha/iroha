---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الترابط
العنوان: دليل تشغيل Nexus
الوصف: ملخص عملي لخطوات تشغيل مشغل Nexus، يعكس `docs/source/nexus_operations.md`.
---

استخدم هذه الصفحة كمرجع واسع النطاق لـ `docs/source/nexus_operations.md`. فهي تلخص قائمة التشغيل، ونقاط إدارة التغيير، ومتطلبات كثافة القياس التي يجب على مشغلي Nexus اتباعها.

## قائمة دورة الحياة

| المرحلة | التدابير | الأدلة |
|-------|--------|----------|
| ما قبل الإقلاع | التحقق من البصمات/تواقيع الإصدار، شدد `profile = "iroha3"`، وجهز القالب. | مخرجات `scripts/select_release_profile.py`، سجل المجموع الاختباري، حزمة بيانات الموقع. |
| موامة الكتالوج | تحديد كتالوج `[nexus]`، لإرشادات التوجيه، وعتبات DA وفق بيان المجلس، ثم التقط `--trace-config`. | مخرجات `irohad --sora --config ... --trace-config` محفوظة مع تذكرة onboarding. |
| استمتع بالفحص والتحويل | شغّل `irohad --sora --config ... --trace-config`، نفيذ فحص رقمي في CLI (`FindNetworkStatus`)، فحص من صادرات القياس وطلب القبول. | سجل دخولك + تأكيد Alertmanager. |
| الحالة الراهنة | مراقبة اللوحات/تنبيهات، دوّر المفاتيح راجع لتحسن التورم، وحدث التكوينات/سجلات التشغيل عند تغير البيانات. | محاضر مراجعة ربع سنوية، لقطات، أرقام تذاكر التدوير. |

يبقى onboarding أدواتي (استبدال المفاتيح، أدوات التوجيه، خطوات الملف الإصدار) في `docs/source/sora_nexus_operator_onboarding.md`.

## إدارة الغد

1. **تحديثات الإصدار** - تتبع الإعلانات في `status.md`/`roadmap.md`؛ أرفق قائمة onboarding مع كل إصدار للعلاقات العامة.
2. **تغيير مسار البيانات** - تحقق من الحزم الموقعية من Space Directory وحاول اكتشافها تحت `docs/source/project_tracker/nexus_config_deltas/`.
3. **تغييرات الإعداد** - كل تغيير في `config/config.toml` يحتاج تذكرة تشير إلى المسار/مساحة البيانات. احفظ نسخة منقحة من التنفيذ الفعلي عند الأفراد أو الغناء.
4. **تمارين الرجوع** - درب ربع سنوي على إجراءات الإيقاف/الاستعادة/الفحص النهائي؛ دوّن النتائج تحت `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقة تماما** - يجب أن تحصل على موافقة الممرات الخاصة/CBDC على الموافقة ويرجى تعديلها قبل تعديل DA أو مفاتيح تنقيح القياس (انظر `docs/source/cbdc_lane_playbook.md`).

## القياس و SLOs

- اللوحات: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`، بالإضافة إلى لوحات خاصة بالـ SDK (مثل `android_operator_console.json`).
- التنبيهات: `dashboards/alerts/nexus_audit_rules.yml` وقواعد نقل Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- المعايير التي يجب مراقبتها:
  - `nexus_lane_height{lane_id}` - تنبيه عند عدم التقدم لثلاث خانات.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل حارة (الافتراضي 64 عام / 8 خاص).
  - `nexus_settlement_latency_seconds{lane_id}` - تنبيه عندما يتجاوز P99 900 مللي ثانية (عام) أو 1200 مللي ثانية (خاص).
  - `torii_request_failures_total{scheme="norito_rpc"}` - تنبيه إذا تجاوز معدل الخطأ خلال 5 صباحا 2%.
  -`telemetry_redaction_override_total` -Sev 2 فوري؛ تأكد من وجود تذكرة لعدم التجاوزات.
- تنفيذ قائمة القياس في [خطة قياس Nexus](./nexus-telemetry-remediation) على الأعضاء فصليا وأرفق النموذج المكتمل بملاحظات مراجعة التشغيل.

##صفوفة لتلقي

| الشدة | التعريف | الشكل |
|----------|-----------|----------|
| سيف 1 | انتهت إزالة مساحة البيانات، وتوقفت لعدة دقائق من 15 دقيقة، أو فساد تصويت التصفح. | نادِ Nexus Primary + Release Engineering + Compliance، جمّد تقبل، اجمع الأدلة، نشر تواصل <=60 دقيقة، RCA <=5 أيام عمل. |
| سيف 2 | كاملة SLA لتراكم الممر، نقطة عمياء في القياس >30 دقيقة، فشل الطرح رد. | نادِ Nexus Primary + SRE، عالج <=4 ساعات، سجل المتابعات خلال العمل اليومي. |
| سيف 3 | انحراف غير معطل (docs، تنبيهات). | سجل في المتتبع، وجدول الإصلاح داخل السبرنت. |

لا بد من عدم فقدان المعرفات الخاصة بـ الحارة/مساحة البيانات المتأثرة، وبصمات البيانات، والخط الدقيق، والقياسات/السجلات الأساسية، ومهام المتابعة والمالكين.

## أرشيف الأدلة

- خزين الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`.
-تجهز بالتاتقحة المنية + مخرجات `--trace-config` لكل إصدار.
- أرفق محاضر المجلس + دقة الموقع عند تطبيق الأشكال أو البيانات.
- استخدام بلقطات Prometheus الأسبوعية ذات الصلة بمقاييس Nexus لمدة 12 شهرا.
- دوّن الدليل الدليل في `docs/source/project_tracker/nexus_config_deltas/README.md` حتى يحدد المدقق عند تغيير المسؤوليات.

## مادة ذات صلة

- نظرة عامة: [نظرة عامة على Nexus](./nexus-overview)
- المواصفة: [مواصفات Nexus](./nexus-spec)
- مسار الهندسة: [نموذج المسار Nexus](./nexus-lane-model)
- انتقال وشيمات التوجيه: [Nexus ملاحظات الانتقال](./nexus-transition-notes)
- onboarding لتشغيلين: [Sora Nexus المشغل onboarding](./nexus-operator-onboarding)
-قياس القياس: [Nexus خطة معالجة القياس عن بعد](./nexus-telemetry-remediation)