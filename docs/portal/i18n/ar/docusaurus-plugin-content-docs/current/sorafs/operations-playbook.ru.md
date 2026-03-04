---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قواعد اللعبة التي تمارسها العمليات
العنوان: عملية الكمبيوتر SoraFS
Sidebar_label: عملية اللعب
الوصف: إعادة صياغة الأحداث وإجراءات الحفر الفوضوي للمشغل SoraFS.
---

:::note Канонический источник
هذا الجزء ينشر الرانبوك، ويدعم `docs/source/sorafs_ops_playbook.md`. بعد أن تكون النسخ متزامنة، حول وثائق أبو الهول لن تكون كاملة الهجرة.
:::

## الكلمات المفتاحية

- المراقبة النشطة: استخدم لوحات الاتصال Grafana في `dashboards/grafana/` والتنبيهات الصحيحة Prometheus في `dashboards/alerts/`.
- مقياس الكتالوج: `docs/source/sorafs_observability_plan.md`.
-جهاز القياس عن بعد الشامل: `docs/source/sorafs_orchestrator_plan.md`.

## MATRICA scalacatioi

| الأولوية | الأمثلة الأولية للتحفيز | أساسي عند الطلب | الاحتياطي | مساعدة |
|-----------|-------------------|-----------------|--------|------------|
| ص1 | بوابة الدعم العالمية، نسبة أداء أفضل > 5% (15 دقيقة)، التكرار المتراكم يصل إلى 10 دقائق | تخزين SRE | إمكانية الملاحظة TL | قم بإدراج نصائح حول الحوكمة، إذا تم نقلها 30 دقيقة. |
| ص2 | تضييق نطاق SLO الإقليمي عبر بوابة زمن الوصول، بالإضافة إلى إعادة محاولة المنظم بدون ارتباط باتفاقية مستوى الخدمة | إمكانية الملاحظة TL | تخزين SRE | قم بتمديد عملية الطرح ولكن قم بإلغاء حظر البيانات الجديدة. |
| ص3 | تنبيهات غير واضحة (يظهر الجمود بنسبة 80-90%) | فرز المدخول | نقابة العمليات | نفذ في يوم العمل التالي. |## بوابة الإلغاء / التدهور

**التوعية**

- التنبيهات: `SoraFSGatewayAvailabilityDrop`، `SoraFSGatewayLatencySlo`.
-العداد: `dashboards/grafana/sorafs_gateway_overview.json`.

**النشاط التجاري**

1. قم بتأكيد الطلب (مزود واحد أو كل قطعة أرض) من خلال لوحة معدل الطلب.
2. قم بإلغاء تحديد Torii لموفري الخدمات الخارجيين (في حالة وجود موفر متعدد)، وإلغاء `sorafs_gateway_route_weights` في تكوين العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. إذا كان جميع العملاء من الموردين، قم بتضمين خيار "الجلب المباشر" الاحتياطي لعملاء CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- التحقق من استخدام رمز الدفق المميز `sorafs_gateway_stream_token_limit`.
- عرض بوابة السجل لـ TLS أو القبول.
- قم بتثبيت `scripts/telemetry/run_schema_diff.sh` لتتمكن من تعديل مخطط بوابة التصدير الذي يتوافق مع الإصدار الحالي.

**العلاجات البديلة**

- قم بتمرير بوابة العملية الآمنة فقط؛ قم باختيار النقل إلى كل فئة، إذا لم يتم توفير عدد قليل من المزودين.
- قم حاليًا بزيادة الحد الأقصى لرمز الدفق بنسبة 10-15%، إذا تم إعادة ضبطه.
- إعادة تشغيل الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) بعد الاستقرار.

**ما بعد الحادث**

- قم بتكوين ما بعد النقل P1 باستخدام `docs/source/sorafs/postmortem_template.md`.
- قم بتخطيط تدريبات الفوضى التالية إذا تم إجراء الإصلاح على نظام جيد.

## إثبات الخروج الكامل (PoR / PoTR)

**التوعية**

- التنبيهات: `SoraFSProofFailureSpike`، `SoraFSPoTRDeadlineMiss`.
-العداد: `dashboards/grafana/sorafs_proof_integrity.json`.
- قياس المسافة: `torii_sorafs_proof_stream_events_total` والاشتراك `sorafs.fetch.error` مع `provider_reason=corrupt_proof`.**النشاط التجاري**

1. قم بشراء بيانات جديدة، وراجع بيانات المسجل (`docs/source/sorafs/manifest_pipeline.md`).
2. مراقبة حوكمة الحوافز الحالية لمقدمي الخدمات.

**الفرز**

- التحقق من صحة المشرفين على تحدي PoR `sorafs_node_replication_backlog_total`.
- التحقق من إثبات اختبار خط الأنابيب (`crates/sorafs_node/src/potr.rs`) لعمليات النشر اللاحقة.
- يتم تثبيت الإصدارات الأصلية من البرامج الثابتة بواسطة مشغلي الشبكة.

**العلاجات البديلة**

- قم بإعادة تشغيل PoR من خلال `sorafs_cli proof stream` مع البيان التالي.
- إذا كانت البراهين ثابتة، باستثناء مقدم الخدمة من العمل النشط من خلال حوكمة المراقبة ولوحات النتائج الرئيسية أوركيستراتورا.

**ما بعد الحادث**

- قم بتثبيت سيناريوهات نقل الطاقة PoR إلى التوزيع التالي.
- قم بتأكيد الاختيارات في الفصل بعد الوفاة واطلع على مؤهلات مقدم الخدمة في القائمة المرجعية.

## زيادة النسخ المتماثلة / تراكم بسيط

**التوعية**

- التنبيهات: `SoraFSReplicationBacklogGrowing`، `SoraFSCapacityPressure`. استيراد
  `dashboards/alerts/sorafs_capacity_rules.yml` ومفيد
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  من أجل التشجيع على قيام Alertmanager بإنهاء الإجراءات التوثيقية.
-العداد: `dashboards/grafana/sorafs_capacity_health.json`.
- المقاييس: `sorafs_node_replication_backlog_total`، `sorafs_node_manifest_refresh_age_seconds`.

**النشاط التجاري**

1. قم بمنع تراكم البيانات الكبيرة (موفر واحد أو كل أسطول) واحتفظ بالنسخ المتماثلة غير الضرورية.
2. إذا كانت الأعمال المتراكمة محلية، فستقوم دائمًا بإعادة تقديم إعانات جديدة للمزودين البديلين من خلال النسخ المتماثلة للجدولة.**الفرز**

- التحقق من مدير القياس عن بعد لجميع المحاولات التي يمكن أن تؤدي إلى تراكم الأعمال المتراكمة.
- تأكد من أن الإرتفاع عالي الجودة (`sorafs_node_capacity_utilisation_percent`).
- التحقق من التكوينات التالية (تغيير ملف تعريف القطعة، وإثباتات الإيقاع).

**العلاجات البديلة**

- أدخل `sorafs_cli` مع الخيار `--rebalance` لإعادة تدوير المحتوى.
- إمكانية استخدام عمال النسخ المتماثل بشكل أفقي للمزود المعتمد.
- بدء تحديث البيان لتمكين TTL.

**ما بعد الحادث**

- قم بتخطيط تدريبات القدرة، مع التركيز على الحفر من قبل مقدمي الخدمة.
- احصل على وثائق نسخ SLA المتماثلة في `docs/source/sorafs_node_client_protocol.md`.

## هاوس-دريلوف الدورية

- **مجاني**: بوابة المحاكاة الحديثة + إعادة محاولة مدير العاصفة.
- **اليومين في العام**: حقنة PoR/PoTR لمزودين متجددين.
- **التحقق من البقعة الجميلة**: نسخ سيناريوهات مكررة باستخدام بيانات التدريج.
- قم بإنهاء التدريبات في سجل دليل التشغيل الرئيسي (`ops/drill-log.md`) من خلال:

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

- التحقق من السجل قبل الالتزامات باستخدام:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- استخدم `--status scheduled` للمثاقب الصلبة، و`pass`/`fail` للمثاقب النهائية و`follow-up`، إذا отались отклытые действия.
- أدخل الأسماء عبر `--log` للتشغيل الجاف أو المدقق التلقائي؛ بدون هذا البرنامج النصي، قم بالتعرف على `ops/drill-log.md`.

## شابلون بعد الوفاة

استخدم `docs/source/sorafs/postmortem_template.md` لكل من النقطة P1/P2 وللاستكشاف الاسترجاعي. تقوم شابلون بإعادة الجدول الزمني, جمع النقاط, العوامل, تصحيح المسار والفوائد اللاحقة التحقق من الصحة.