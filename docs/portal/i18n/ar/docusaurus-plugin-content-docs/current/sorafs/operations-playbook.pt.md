---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قواعد اللعبة التي تمارسها العمليات
العنوان: Playbook de Operacoes da SoraFS
Sidebar_label: قواعد التشغيل
الوصف: تعليمات الاستجابة للحوادث وإجراءات تدريبات المساعدة لمشغلي SoraFS.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة مخصصة لدليل التشغيل في `docs/source/sorafs_ops_playbook.md`. احتفظ بنسخ متزامنة مما يعني أن مجموعة وثائق أبو الهول ستنتقل تمامًا.
:::

## المراجع تشافي

- ميزات المراقبة: راجع لوحات المعلومات Grafana في `dashboards/grafana/` وأنظمة التنبيه Prometheus في `dashboards/alerts/`.
- كتالوج المقاييس: `docs/source/sorafs_observability_plan.md`.
- سطوح القياس عن بعد للأوركسترادور: `docs/source/sorafs_orchestrator_plan.md`.

## ماتريز دي escalonamento

| الأولوية | أمثلة على الأشياء | عند الطلب الأساسي | النسخ الاحتياطي | نوتاس |
|-----------|----------------------|------------------|--------|-------|
| ص1 | بوابة عالمية كاملة، تصنيف الأداء > 5% (15 دقيقة)، تراكم النسخ المتماثل كل 10 دقائق | تخزين SRE | إمكانية الملاحظة TL | إن اتخاذ قرار بشأن الحوكمة سيؤثر بشكل فائق على 30 دقيقة. |
| ص2 | انتهاك SLO لوقت الاستجابة الإقليمي للبوابة، وإعادة محاولة المنسق بدون تأثير SLA | إمكانية الملاحظة TL | تخزين SRE | استمر في الطرح، وسيظهر المزيد من الحظر الجديد. |
| ص3 | تنبيهات غير نقدية (جامدة البيانات، السعة 80-90%) | فرز المدخول | نقابة العمليات | المحلل لا يوجد استخدام قريب. |## Queda do gate / disponibilidade degradada

**ديتكاو**

- التنبيهات: `SoraFSGatewayAvailabilityDrop`، `SoraFSGatewayLatencySlo`.
- لوحة القيادة: `dashboards/grafana/sorafs_gateway_overview.json`.

**الأشياء الوسيطة**

1. قم بتأكيد التغطية (providor unico vs frota) عبر لوحة تصنيفات المتطلبات.
2. قم بتدوير Torii لمثبتات البرامج السعودية (في حالة وجود عدة موفري) لضبط `sorafs_gateway_route_weights` في تكوين العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. إذا كانت جميع الأدلة متأثرة، يمكنك استخدام خيار \"الجلب المباشر\" الاحتياطي لعملاء CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- التحقق من استخدام الرموز المميزة للتدفق مقابل `sorafs_gateway_stream_token_limit`.
- تقوم سجلات التفتيش بالبوابة للبحث عن أخطاء TLS أو القبول.
- قم بتنفيذ `scripts/telemetry/run_schema_diff.sh` لضمان أن المخطط المُصدَّر من خلال البوابة يتوافق مع النسخة المتوقعة.

**عمليات العلاج**

- إعادة تشغيل عملية البوابة التي تم تشغيلها؛ تجنب تكرار جميع المهام أو المجموعات، حتى لا يتم تقديم العديد من التعديلات.
- زيادة مؤقتة أو حد للرموز المميزة بنسبة 10-15% في حالة التشبع للتأكيد.
- أعد تنفيذ الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) بعد التثبيت.

**موضع الحادث**

- قم بالتسجيل بعد الوفاة P1 باستخدام `docs/source/sorafs/postmortem_template.md`.
- جدول تدريبات المرافقة إذا كان العلاج يعتمد على التدخلات اليدوية.

## بيكو دي فالهاس دي بروفا (PoR / PoTR)

**ديتكاو**- التنبيهات: `SoraFSProofFailureSpike`، `SoraFSPoTRDeadlineMiss`.
- لوحة القيادة: `dashboards/grafana/sorafs_proof_integrity.json`.
- القياس عن بعد: `torii_sorafs_proof_stream_events_total` والأحداث `sorafs.fetch.error` مع `provider_reason=corrupt_proof`.

**الأشياء الوسيطة**

1. قم بإدخال بيانات جديدة مقبولة من خلال تسجيل البيانات أو تسجيلها (`docs/source/sorafs/manifest_pipeline.md`).
2. إخطار الحوكمة بإيقاف الحوافز المقدمة من الشركات.

**الفرز**

- تحقق من عمق ملف التحديات مقابل `sorafs_node_replication_backlog_total`.
- التحقق من صحة خط أنابيب الاختبار (`crates/sorafs_node/src/potr.rs`) لعمليات النشر الأخيرة.
- قارن إصدارات البرامج الثابتة من دوس المثبتين مع سجل المشغلين.

**عمليات العلاج**

- قم بإلغاء عمليات الإعادة باستخدام `sorafs_cli proof stream` مع أحدث بيان.
- في حالة اختبار شكل متسق، قم بإزالة أو إثبات مجموعة النشاط المحدث أو سجل الإدارة والطلب أو تحديث لوحات النتائج الخاصة بالمنسق.

**موضع الحادث**

- تنفيذ سيناريو التدريب على Caos PoR قبل النشر التالي في الإنتاج.
- قم بتسجيل الأشخاص الذين تعلموا في قالب ما بعد الوفاة وتحديث القائمة المرجعية للمؤهلات المقدمة.

## نتيجة النسخ المتماثل / نمو الأعمال المتراكمة

**ديتكاو**

- التنبيهات: `SoraFSReplicationBacklogGrowing`، `SoraFSCapacityPressure`. استيراد
  `dashboards/alerts/sorafs_capacity_rules.yml` يتم تنفيذه
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  قبل الترويج حتى يقوم Alertmanager بإعادة النظر في الحدود الموثقة.
- لوحة القيادة: `dashboards/grafana/sorafs_capacity_health.json`.
- المقاييس: `sorafs_node_replication_backlog_total`، `sorafs_node_manifest_refresh_age_seconds`.

**الأشياء الوسيطة**1. تحقق من إنهاء الأعمال المتراكمة (مقدم خدمة فردي أو مقدم) وإيقاف مهام النسخ غير الأساسية مؤقتًا.
2. في حالة تراكم الأعمال المعزولة، قم بإعادة تقديم الطلبات الجديدة مؤقتًا إلى البدائل عبر جدولة النسخ المتماثلة.

**الفرز**

- فحص جهاز القياس عن بعد للأوركسترا من خلال البحث عن دفعات من المحاولات التي قد تزيد من حجم الأعمال المتراكمة.
- تأكد من أن أهداف التخزين بها مساحة كافية للرأس (`sorafs_node_capacity_utilisation_percent`).
- مراجعة تعديلات التكوين الأخيرة (تحديث ملف تعريف القطعة، وإيقاع البراهين).

**عمليات العلاج**

- قم بتنفيذ `sorafs_cli` مع `--rebalance` لإعادة توزيع المحتوى.
- قم بتوسيع عمال النسخ أفقيًا لإثبات تأثيرهم.
- Dispare تحديث البيان لrealinhar janelas TTL.

**موضع الحادث**

- جدول تدريبات السعة المركزة في عملية إشباع المولدات.
- قم بتحديث مستند SLA المتماثل على `docs/source/sorafs_node_client_protocol.md`.

## إيقاع تدريبات الاسترخاء

- **Trimestral**: محاكاة مجموعة من البوابة الرئيسية + عاصفة إعادة محاولة orquestrador.
- **Semestral**: injecao de falhas PoR/PoTR em doisprovidores com recuperacao.
- **Spot Check Mensal**: سيناريو الإحالة المتماثلة باستخدام بيانات التدريج.
- قم بتسجيل تدريبات نظام التشغيل دون مشاركة السجل (`ops/drill-log.md`) عبر:

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

- التحقق من صحة السجل المسبق للالتزامات عبر:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- استخدم `--status scheduled` للتدريبات المستقبلية، و`pass`/`fail` للتنفيذ الملخص، و`follow-up` عندما تقوم بالفتح.
- استبدال الوجهة بـ `--log` لعمليات التشغيل الجافة أو التحقق التلقائي؛ لم يتم إصدار البرنامج النصي المستمر حتى `ops/drill-log.md`.

## قالب ما بعد الوفاة

استخدم `docs/source/sorafs/postmortem_template.md` لكل حادث P1/P2 وللاطلاع على تدريبات الاسترخاء. يشمل القالب خط الزمن وكمية التأثير والمساهمات والفوائد التصحيحية ومتطلبات التحقق من المرافقة.