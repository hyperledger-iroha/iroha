---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/operations-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: قواعد اللعبة التي تمارسها العمليات
العنوان: Playbook de Operations de SoraFS
Sidebar_label: دليل العمليات
الوصف: تعليمات الاستجابة للحوادث وإجراءات تدريبات المساعدة لمشغلي SoraFS.
---

:::ملاحظة فوينتي كانونيكا
تعكس هذه الصفحة دليل التشغيل المحتفظ به في `docs/source/sorafs_ops_playbook.md`. حافظ على النسخ المتزامنة حتى يتم نقل مجموعة وثائق أبو الهول بالكامل.
:::

##المراجع المفتاح

- أنشطة المراقبة: راجع لوحات المعلومات Grafana و`dashboards/grafana/` وقواعد التنبيه Prometheus و`dashboards/alerts/`.
- كتالوج المقاييس: `docs/source/sorafs_observability_plan.md`.
- سطوح القياس عن بعد لجهاز orquestador: `docs/source/sorafs_orchestrator_plan.md`.

## ماتريز دي escalamiento| الأولوية | أمثلة على الضياع | مدير تحت الطلب | النسخ الاحتياطي | نوتاس |
|----------|-------------------------------------|--|--------|-------|
| ص1 | إجمالي البوابة العالمية، معدل السقوط > 5% (15 دقيقة)، تراكم النسخ المتماثلة كل 10 دقائق | تخزين SRE | إمكانية الملاحظة TL | قم بإدراج نصيحة الحكومة إذا كان التأثير فائقًا لمدة 30 دقيقة. |
| ص2 | توفير SLO لوقت الاستجابة الإقليمي للبوابة، وهو جزء من عمليات الاحتفاظ بالأداة بدون تأثير SLA | إمكانية الملاحظة TL | تخزين SRE | مواصلة الطرح حتى يظهر حظر جديد. |
| ص3 | تنبيهات بشأن عدم وجود انتقادات (عدم صلاحية البيانات، السعة 80-90%) | فرز المدخول | نقابة العمليات | الحل التالي هو الحل. |

## صندوق البوابة / تدهور القدرة على التوفر

**الكشف**

- التنبيهات: `SoraFSGatewayAvailabilityDrop`، `SoraFSGatewayLatencySlo`.
- لوحة القيادة: `dashboards/grafana/sorafs_gateway_overview.json`.

**الإجراءات الوسيطة**

1. قم بتأكيد الرصيد (المزود الوحيد مقابل الأسطول) عبر لوحة طلبات المساعدة.
2. قم بتغيير إدخال Torii إلى موردين بلا موردين (إذا كان مزودًا متعددًا) وقم بتنشيط `sorafs_gateway_route_weights` في تكوين العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. إذا كان جميع الموردين متأثرين، فيمكنهم استخدام خيار "الجلب المباشر" الاحتياطي لعملاء CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**- قم بمراجعة استخدام رمز الدفق الأمامي إلى `sorafs_gateway_stream_token_limit`.
- فحص سجلات البوابة بحثًا عن أخطاء TLS أو الدخول.
- قم بتشغيل `scripts/telemetry/run_schema_diff.sh` للتأكد من أن الملف المصدر عبر البوابة يتزامن مع الإصدار المنتظر.

**خيارات العلاج**

- استعادة عملية البوابة الفعالة فقط؛ تجنب إعادة توجيه جميع الصواريخ العنقودية التي سقطت على العديد من الموردين.
- زيادة الحد الأقصى من الرموز المميزة للبث بنسبة 10-15% بشكل مؤقت في حالة تأكيد التشبع.
- قم بإعادة إنشاء الشهادة الذاتية (`scripts/sorafs_gateway_self_cert.sh`) من خلال التثبيت.

**ما بعد الحادث**

- قم بالتسجيل بعد الوفاة P1 باستخدام `docs/source/sorafs/postmortem_template.md`.
- قم ببرمجة أداة تهدئة للمتابعة إذا كان العلاج يتطلب تدخلات يدوية.

## بيكو دي فالوس دي بروباس (PoR / PoTR)

**الكشف**

- التنبيهات: `SoraFSProofFailureSpike`، `SoraFSPoTRDeadlineMiss`.
- لوحة القيادة: `dashboards/grafana/sorafs_proof_integrity.json`.
- القياس عن بعد: `torii_sorafs_proof_stream_events_total` والأحداث `sorafs.fetch.error` مع `provider_reason=corrupt_proof`.

**الإجراءات الوسيطة**

1. قم بإيداع البيانات الجديدة من خلال سجل البيانات (`docs/source/sorafs/manifest_pipeline.md`).
2. إخطار الحوكمة بإيقاف حوافز الموردين المتأثرين.

**الفرز**

- قم بمراجعة عمق فجوة التحديات أمام `sorafs_node_replication_backlog_total`.
- التحقق من صحة خط أنابيب الاختبار (`crates/sorafs_node/src/potr.rs`) للإطلاع على أحدث الأخبار.
- مقارنة إصدارات البرامج الثابتة للموردين مع سجل المشغلين.**خيارات العلاج**

- قم بإظهار إعادات PoR باستخدام `sorafs_cli proof stream` بالبيان الأحدث.
- إذا استمرت الاختبارات بشكل متسق، فقم بإزالة مورد المجموعة النشطة من سجل الإدارة واحتفظ بلوحة تسجيل النتائج الخاصة بالـ Orquestador.

**ما بعد الحادث**

- قم بتنفيذ سيناريو الحفر قبل نشر الإنتاج التالي.
- احصل على تدريبات على مصنع ما بعد الوفاة وقم بتحديث قائمة تأهيل الموردين.

## استعادة النسخ المتماثل / زيادة حجم الأعمال المتراكمة

**الكشف**

- التنبيهات: `SoraFSReplicationBacklogGrowing`، `SoraFSCapacityPressure`. هام
  `dashboards/alerts/sorafs_capacity_rules.yml` وقم بالتشغيل
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  قبل الترويج لكي يعرض Alertmanager المظلات الموثقة.
- لوحة القيادة: `dashboards/grafana/sorafs_capacity_health.json`.
- المقاييس: `sorafs_node_replication_backlog_total`، `sorafs_node_manifest_refresh_age_seconds`.

**الإجراءات الوسيطة**

1. التحقق من رصيد الأعمال المتراكمة (المزود الوحيد للأسطول) وإيقاف فترات النسخ غير الضرورية.
2. إذا تم تأجيل التراكم، قم بإعادة تصميم الطلبات الجديدة مؤقتًا إلى موردين بديلين من خلال جدولة النسخ المتماثل.

**الفرز**

- فحص جهاز القياس عن بعد من خلال عمليات إعادة التدوير التي يمكن من خلالها تصعيد الأعمال المتراكمة.
- تأكد من أن أهداف التخزين بها مساحة كافية للرأس (`sorafs_node_capacity_utilisation_percent`).
- مراجعة أحدث تغييرات التكوين (تحديث ملفات تعريف القطعة، وتسلسل الأدلة).**خيارات العلاج**

- قم بتشغيل `sorafs_cli` باستخدام الخيار `--rebalance` لإعادة توزيع المحتوى.
- قم برفع عمال النسخ أفقيًا للمورد المتأثر.
- قم بإلغاء تحديث البيانات لإعادة ضبط النوافذ TTL.

**ما بعد الحادث**

- برنامج تدريبات القدرة المعززة بالسقوط من خلال تشبع الموردين.
- تحديث وثائق النسخ المتماثل SLA على `docs/source/sorafs_node_client_protocol.md`.

## إيقاع تدريبات الاسترخاء

- **Trimestral**: محاكاة مجمعة لسلسلة البوابة + عذاب استعادة الأوركيستادور.
- **نصف سنوي**: حقن المغذيات PoR/PoTR لدى الموردين أثناء التعافي.
- **Checko mensual**: سيناريو إعادة النسخ باستخدام بيانات التدريج.
- قم بتسجيل التدريبات في السجل المشترك (`ops/drill-log.md`) عبر:

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

- التحقق من صحة السجل قبل ارتكاب الأخطاء:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- الولايات المتحدة الأمريكية `--status scheduled` للتدريبات المستقبلية، `pass`/`fail` للتنفيذ الكامل، و`follow-up` عندما يتم فتح الإجراءات.
- قم بكتابة الوجهة باستخدام `--log` لعمليات التشغيل الجافة أو التحقق التلقائي؛ ومع ذلك، يتم تحديث البرنامج النصي `ops/drill-log.md`.

## نبتة ما بعد الوفاةالولايات المتحدة الأمريكية `docs/source/sorafs/postmortem_template.md` لكل حادثة P1/P2 ولمراجعة تدريبات الاسترخاء. تتضمن المجموعة التسلسل الزمني، وقياس التأثير، والعوامل المساهمة، والإجراءات التصحيحية، وخطط التحقق من المتابعة.