---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operations
title: دليل تشغيل Nexus
description: ملخص عملي لخطوات تشغيل مشغل Nexus، يعكس `docs/source/nexus_operations.md`.
---

استخدم هذه الصفحة كمرجع سريع موازي لـ `docs/source/nexus_operations.md`. فهي تلخص قائمة التشغيل، ونقاط إدارة التغيير، ومتطلبات تغطية القياس التي يجب على مشغلي Nexus اتباعها.

## قائمة دورة الحياة

| المرحلة | الإجراءات | الأدلة |
|-------|--------|----------|
| ما قبل الإقلاع | تحقق من بصمات/تواقيع الإصدار، أكد `profile = "iroha3"`، وجهز قوالب الإعداد. | مخرجات `scripts/select_release_profile.py`، سجل checksum، حزمة بيانات موقعة. |
| مواءمة الكتالوج | حدّث كتالوج `[nexus]`، سياسة التوجيه، وعتبات DA وفق بيان المجلس، ثم التقط `--trace-config`. | مخرجات `irohad --sora --config ... --trace-config` محفوظة مع تذكرة onboarding. |
| فحص الدخان والتحويل | شغّل `irohad --sora --config ... --trace-config`، نفّذ فحص الدخان في CLI (`FindNetworkStatus`)، تحقق من صادرات القياس واطلب القبول. | سجل فحص الدخان + تأكيد Alertmanager. |
| حالة مستقرة | راقب لوحات/تنبيهات، دوّر المفاتيح حسب وتيرة الحوكمة، وحدث configs/runbooks عند تغير البيانات. | محاضر مراجعة ربع سنوية، لقطات لوحات، أرقام تذاكر التدوير. |

يبقى onboarding التفصيلي (استبدال المفاتيح، قوالب التوجيه، خطوات ملف الإصدار) في `docs/source/sora_nexus_operator_onboarding.md`.

## إدارة التغيير

1. **تحديثات الإصدار** - تتبع الإعلانات في `status.md`/`roadmap.md`؛ أرفق قائمة onboarding مع كل PR للإصدار.
2. **تغييرات بيانات lane** - تحقق من الحزم الموقعة من Space Directory وأرشفها تحت `docs/source/project_tracker/nexus_config_deltas/`.
3. **تغييرات الإعداد** - كل تغيير في `config/config.toml` يحتاج تذكرة تشير إلى lane/data-space. احفظ نسخة منقحة من الإعداد الفعلي عند انضمام أو ترقية العقد.
4. **تمارين الرجوع** - درّب ربع سنوي على إجراءات الإيقاف/الاستعادة/فحص الدخان؛ دوّن النتائج تحت `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **موافقات الامتثال** - يجب أن تحصل lanes الخاصة/CBDC على موافقة امتثال قبل تعديل سياسة DA أو مفاتيح تنقيح القياس (انظر `docs/source/cbdc_lane_playbook.md`).

## القياس و SLOs

- اللوحات: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, بالإضافة إلى لوحات خاصة بالـ SDK (مثل `android_operator_console.json`).
- التنبيهات: `dashboards/alerts/nexus_audit_rules.yml` وقواعد نقل Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- المقاييس التي يجب مراقبتها:
  - `nexus_lane_height{lane_id}` - تنبيه عند عدم التقدم لثلاث خانات.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل lane (الافتراضي 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - تنبيه عندما يتجاوز P99 900 ms (public) أو 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` - تنبيه إذا تجاوز معدل الخطأ خلال 5 دقائق 2%.
  - `telemetry_redaction_override_total` - Sev 2 فوري؛ تأكد من وجود تذاكر امتثال للتجاوزات.
- نفذ قائمة معالجة القياس في [خطة معالجة قياس Nexus](./nexus-telemetry-remediation) على الأقل فصليا وأرفق النموذج المكتمل بملاحظات مراجعة التشغيل.

## مصفوفة الحوادث

| الشدة | التعريف | الاستجابة |
|----------|------------|----------|
| Sev 1 | خرق عزل data-space، توقف التسوية لأكثر من 15 دقيقة، أو فساد تصويت الحوكمة. | نادِ Nexus Primary + Release Engineering + Compliance، جمّد القبول، اجمع الأدلة، انشر تواصل <=60 دقيقة، RCA <=5 أيام عمل. |
| Sev 2 | خرق SLA لتراكم lane، نقطة عمياء في القياس >30 دقيقة، فشل rollout للبيانات. | نادِ Nexus Primary + SRE، عالج <=4 ساعات، سجّل المتابعات خلال يومي عمل. |
| Sev 3 | انحراف غير معطل (docs، تنبيهات). | سجّل في المتتبع، وجدول إصلاح داخل السبرنت. |

يجب أن تسجل تذاكر الحوادث IDs الخاصة بـ lane/data-space المتأثرة، وبصمات البيانات، والخط الزمني، والمقاييس/السجلات الداعمة، ومهام المتابعة والمالكين.

## أرشيف الأدلة

- خزّن الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`.
- احتفظ بالتهيئات المنقحة + مخرجات `--trace-config` لكل إصدار.
- أرفق محاضر المجلس + القرارات الموقعة عند تطبيق تغييرات الإعداد أو البيانات.
- احتفظ بلقطات Prometheus الأسبوعية ذات الصلة بمقاييس Nexus لمدة 12 شهرا.
- دوّن تعديلات الدليل في `docs/source/project_tracker/nexus_config_deltas/README.md` حتى يعرف المدققون متى تغيرت المسؤوليات.

## مواد ذات صلة

- نظرة عامة: [Nexus overview](./nexus-overview)
- المواصفة: [Nexus spec](./nexus-spec)
- هندسة lane: [Nexus lane model](./nexus-lane-model)
- الانتقال وشيمات التوجيه: [Nexus transition notes](./nexus-transition-notes)
- onboarding المشغلين: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- معالجة القياس: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
