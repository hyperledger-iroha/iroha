---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قواعد اللعبة التي تمارسها العمليات
العنوان: دليل تشغيل العمليات SoraFS
Sidebar_label: دليل التشغيل
الوصف: إرشادات التوجيه المسبق للآلات لتدريبات الفوضى لمشغل SoraFS.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة الدليل التشغيلي الموجود ضمن `docs/source/sorafs_ops_playbook.md`. احرص على أن يتم رحيل مجموعة أبو الهول بالكامل.
:::

## المراجع الرئيسية

- أصول بخير: راجع اللوحات Grafana ضمن `dashboards/grafana/` وقواعد تنبيهات Prometheus في `dashboards/alerts/`.
- كتالوج المعايير: `docs/source/sorafs_observability_plan.md`.
- أسطح تليمترية المُنسِّق: `docs/source/sorafs_orchestrator_plan.md`.

## مرتبة التصعيد

| بمعنى | أمثلة على المحفزات | المناوب الأساسي | احتياطي | تعليقات |
|----------|--------------------|-----------------|---------|---------|
| ص1 | عالمي للبوابة، فشل فشل PoR أكبر من 5% (15 دقيقة)، فشل التكرار يكاد يكون كل 10 مرات | تخزين SRE | إمكانية الملاحظة TL | أشرك مجلس تور إذا تجاوز 30 دقيقة. |
| ص2 | كامل SLO لتأخر تأخر البوابة، قفزة إعادة المحاولة في المُنسِّق دون تأثير SLA | إمكانية الملاحظة TL | تخزين SRE | واصل قاطع ولكن توقف المانيفستات الجديدة. |
| ص3 | تنبيهات غير حرجة (تقادم المانيفيست، السعة 80–90%) | فرز التبرعات | نقابة العمليات | عالج خلال يوم العمل التالي. |

##أخبار الشبكة / تدهور المتوفرة

**الكشف**

- التنبيهات: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- لوحة المتابعة: `dashboards/grafana/sorafs_gateway_overview.json`.

**الإجراءات الفورية**1. أكّد النطاق (مقياس واحد مقابل الأسطول) عبر لوحة معدل الطلبات.
2. تحويل توجيه Torii إلى المتحكمين الأصحاء (إذا كان متعدد المتحكمين) عبر التغيير `sorafs_gateway_route_weights` في إعدادات العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. إذا تأثر جميع المتحكمين، ففعل بديل “الجلب المباشر” لعملاء CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- تحقّق من استهلاك رموز الدفق مقابل `sorafs_gateway_stream_token_limit`.
- فحص سجلات البوابة لأخطاء TLS أو أخطاء التقييم.
- شغّل `scripts/telemetry/run_schema_diff.sh` للتأكد من أنك ستصدّره البوابة يطابق النسخة المصنّعة.

**الكمبيوتر**

- خطة تشغيل البوابة المتأثرة فقط؛ تجنّب إعادة التدفق الكامل للطاقم إلا في حالة فشل عدة منظمين.
- ارفع قواعد البيانات بنسبة 10–15% بدقة إذا تم ضمان التشبع.
- إعادة تشغيل الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) بشكل افتراضي.

**ما بعد الحادث**

- سجل بعد الوفاة من فئة P1 باستخدام `docs/source/sorafs/postmortem_template.md`.
- جدول تدريب فوضى لاحقاً إذا كانت هناك أضرار على العمل يدوياً.

## ارتفاع الثقة في فشل الإثبات (PoR / PoTR)

**الكشف**

- التنبيهات: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- لوحة المتابعة: `dashboards/grafana/sorafs_proof_integrity.json`.
- التليمترية: `torii_sorafs_proof_stream_events_total` و الأحداث `sorafs.fetch.error` مع `provider_reason=corrupt_proof`.

**الإجراءات الفورية**

1. جمّد مقبولات المانيفست الجديدة عبر تعليم سجل المانيفست (`docs/source/sorafs/manifest_pipeline.md`).
2. أخطر الأعضاء لإيقاف الحوافز للمنظمات المتأثرة.

**الفرز**

- إعادة النظر في قائمة تحديات PoR مقابل `sorafs_node_replication_backlog_total`.
- تحقّق من مسار التحقق من الإثبات (`crates/sorafs_node/src/potr.rs`) للعمليات المنشورة منذ وقت طويل.
- قارن إصدارات البرامج الثابتة للمنظمين مع سجل التشغيلين.**الكمبيوتر**

- قم بإعادة تشغيل PoR باستخدام `sorafs_cli proof stream` مع أحدث مانيفيست.
- إذا كان البرنامج الإخفاقات، أزِل المتحكم من المجموعة العضوية عبر تحديث سجل ال تور وإجبار المُنسِّق على تحديث لوحات النتائج.

**ما بعد الحادث**

- نفي سيناريو فوضى PoR قبل نشر الإنتاج التالي.
- دوّن الدروس في قالب ما بعد الوفاة وتحديد قائمة تحديد تأهيل المحاسبين.

## تأخر التكرار / تراكم التراكم

**الكشف**

- التنبيهات: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. استورد
  `dashboards/alerts/sorafs_capacity_rules.yml` وشغّل
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  قبل المصادقة كي تعكس Alertmanager العتبات الموثقة.
- لوحة المتابعة: `dashboards/grafana/sorafs_capacity_health.json`.
- المقاييس: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**الإجراءات الفورية**

1. تحقّق من نطاق التراكم (مقياس فردي أو الأطوال) وأوقف مهام التكرار غير الأساسية.
2. إذا كان التراكم معزولًا، أعد إسناد الطلبات الجديدة مؤقتًا إلى محاسبين بدلاء عبر جدول التكرار.

**الفرز**

- تم بحث فحص تليمتري المُنسِق عن دفعات إعادة المحاولة التي قد تؤدي إلى تضخم التراكم.
- تأكد من أن تهدف إلى تحقيق النتائج (`sorafs_node_capacity_utilisation_percent`).
- التغيرات الطارئة على الأحداث الأخيرة (تحديثات تعريف القطع، تراجع التجدد).

**الكمبيوتر**

- شغّل `sorafs_cli` مع خيار `--rebalance` إعادة توزيع المحتوى.
- قم بتوسييع عمال التكرار بصريًا للتأثير.
- فعّل تحديث المانيفست مرة أخرى لـ نوافذ TTL.

**ما بعد الحادث**

- خطة تدريب لمشاركة ضعف تشبع المتحكمين.
- حدث وثائق SLA الخاصة بالتكرار في `docs/source/sorafs_node_client_protocol.md`.

##تغيرات التغيير- **ربع سنوي**: محاكاة مجمعة لانقطاع البوابة + عاصفة عودة هدف المُنسِّق.
- **نصف سنوي**: فشل حقن PoR/PoTR عبر منظمين اثنين مع الفشل.
- **تحقق شهري سريع**: سيناريو تأخر التكرار باستخدام مانيفستات التدريج.
- تتبع التدريبات في سجل التشغيل (`ops/drill-log.md`) عبر:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- تحقّق من السجل قبل الالتزامات عبر:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- استخدم `--status scheduled` التقنيات القادمة، و`pass`/`fail` للجولات المكتملة، و`follow-up` عندما تستمر بنود.
- غيّر الوجه باستخدام `--log` للتشغيل التجريبي أو التحقق التجريبي؛ بدونه استمر السكربت في تحديث `ops/drill-log.md`.

## قالب ما بعد الحادث

استخدم `docs/source/sorafs/postmortem_template.md` لكل حادثة P1/P2 وللاستعادات اللاحقة لتدريبات الفوضى. يتتبع الخط الزمني، وقياس التأثير، والعوامل، للتأكد من التصحيح، وما بعد ذلك.