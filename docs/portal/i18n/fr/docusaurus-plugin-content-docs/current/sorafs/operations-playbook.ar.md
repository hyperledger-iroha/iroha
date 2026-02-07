---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : manuel d'opérations
titre : دليل تشغيل عمليات SoraFS
sidebar_label : دليل التشغيل
description: إرشادات الاستجابة للحوادث وإجراءات تدريبات الفوضى لمشغلي SoraFS.
---

:::note المصدر المعتمد
تعكس هذه الصفحة الدليل التشغيلي الموجود ضمن `docs/source/sorafs_ops_playbook.md`. Il s'agit d'une histoire de Sphinx.
:::

## المراجع الرئيسية

- أصول الملاحظة: راجع لوحات Grafana et `dashboards/grafana/` et Prometheus à `dashboards/alerts/`.
- Nom de l'utilisateur : `docs/source/sorafs_observability_plan.md`.
- Nom du produit : `docs/source/sorafs_orchestrator_plan.md`.

## مصفوفة التصعيد

| الأولوية | أمثلة على المحفزات | المناوب الأساسي | الاحتياطي | ملاحظات |
|----------|----------|-----------------|----------|---------|
| P1 | انقطاع عالمي للبوابة، معدل فشل PoR أكبر من 5% (15 دقيقة), تراكم التكرار يتضاعف كل 10 دقائق | Stockage SRE | Observabilité TL | Il faut compter 30 dollars pour le faire. |
| P2 | SLO لزمن تأخر البوابة الإقليمي، قفزة إعادة المحاولة في المُنسِّق دون تأثير SLA | Observabilité TL | Stockage SRE | واصل الإطلاق لكن أوقف المانيفستات الجديدة. |
| P3 | تنبيهات غير حرجة (تقادم المانيفست، السعة 80–90%) | فرز الاستقبال | Guilde des opérations | عالج خلال يوم العمل التالي. |

## انقطاع البوابة / تدهور التوفر

**الكشف**

- Titres : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Nom du produit : `dashboards/grafana/sorafs_gateway_overview.json`.

**إجراءات فورية**1. أكّد النطاق (مزوّد واحد مقابل الأسطول) عبر لوحة معدل الطلبات.
2. Utilisez le modèle Torii pour le modèle Torii avec le modèle `sorafs_gateway_route_weights`. إعدادات العمليات (`docs/source/sorafs_gateway_self_cert.md`).
3. Utilisez la fonction « récupération directe » via CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- تحقّق من استهلاك رموز stream مقابل `sorafs_gateway_stream_token_limit`.
- افحص سجلات البوابة لأخطاء TLS et أخطاء القبول.
- شغّل `scripts/telemetry/run_schema_diff.sh` للتأكد من أن المخطط الذي تصدّره البوابة يطابق الإصدار المتوقع.

**خيارات المعالجة**

- أعد تشغيل عملية البوابة المتأثرة فقط؛ تجنّب إعادة تدوير كامل العنقود إلا إذا فشل عدة مزوّدين.
- ارفع حد رموز stream بنسبة 10–15% مؤقتًا إذا تم تأكيد التشبع.
- أعد تشغيل self-cert (`scripts/sorafs_gateway_self_cert.sh`) بعد الاستقرار.

**ما بعد الحادث**

- L'autopsie est effectuée sur P1 par `docs/source/sorafs/postmortem_template.md`.
- جدولة تدريب فوضى لاحق إذا اعتمدت المعالجة على تدخلات يدوية.

## ارتفاع مفاجئ لفشل الإثبات (PoR / PoTR)

**الكشف**

- Titres : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Nom du produit : `dashboards/grafana/sorafs_proof_integrity.json`.
- Nom : `torii_sorafs_proof_stream_events_total` et `sorafs.fetch.error` ou `provider_reason=corrupt_proof`.

**إجراءات فورية**

1. جمّد قبولات المانيفست الجديدة عبر تعليم سجل المانيفست (`docs/source/sorafs/manifest_pipeline.md`).
2. أخطر الحوكمة لإيقاف الحوافز للمزوّدين المتأثرين.

**الفرز**

- راجع عمق قائمة تحديات PoR مقابل `sorafs_node_replication_backlog_total`.
- تحقّق من مسار التحقق من الإثبات (`crates/sorafs_node/src/potr.rs`) للعمليات المنشورة مؤخرًا.
- قارن إصدارات firmware للمزوّدين مع سجل المشغلين.**خيارات المعالجة**

- فعّل إعادة تشغيل PoR byاستخدام `sorafs_cli proof stream` مع أحدث مانيفست.
- إذا استمرت الإخفاقات، أزِل المزوّد من المجموعة النشطة عبر تحديث سجل الحوكمة وإجبار المُنسِّق على تحديث tableaux de bord.

**ما بعد الحادث**

- نفّذ سيناريو تدريب فوضى PoR قبل نشر الإنتاج التالي.
- دوّن الدروس في قالب post-mortem وحدّث قائمة تحقق تأهيل المزوّدين.

## تأخر التكرار / نمو التراكم

**الكشف**

- Titres : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. استورد
  `dashboards/alerts/sorafs_capacity_rules.yml` et
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  قبل الترقية كي يعكس Alertmanager العتبات الموثقة.
- Nom du produit : `dashboards/grafana/sorafs_capacity_health.json`.
- Nom : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**إجراءات فورية**

1. تحقّق من نطاق التراكم (مزوّد منفرد أو الأسطول) وأوقف مهام التكرار غير الأساسية.
2. إذا كان التراكم معزولًا، أعد إسناد الطلبات الجديدة مؤقتًا إلى مزوّدين بدلاء عبر مجدول التكرار.

**الفرز**

- افحص تليمترية المُنسِّق بحثًا عن دفعات إعادة المحاولة التي قد تؤدي إلى تضخم التراكم.
- تأكد من أن أهداف التخزين لديها هامش كافٍ (`sorafs_node_capacity_utilisation_percent`).
- راجع تغييرات الإعداد الأخيرة (تحديثات ملفات تعريف القطع، وتيرة الإثباتات).

**خيارات المعالجة**

- شغّل `sorafs_cli` مع الخيار `--rebalance` لإعادة توزيع المحتوى.
- قم بتوسيع عمال التكرار أفقيًا للمزوّد المتأثر.
- فعّل تحديث المانيفست لإعادة محاذاة نوافذ TTL.

**ما بعد الحادث**

- جدولة تدريب سعة يركز على فشل تشبع المزوّدين.
- Vérifiez la compatibilité SLA avec `docs/source/sorafs_node_client_protocol.md`.

## وتيرة تدريبات الفوضى- **ربع سنوي**: محاكاة مجمعة لانقطاع البوابة + عاصفة إعادة محاولات المُنسِّق.
- **نصف سنوي**: حقن فشل PoR/PoTR عبر مزوّدين اثنين مع التعافي.
- **تحقق شهري سريع** : سيناريو تأخر التكرار باستخدام مانيفستات mise en scène.
- تتبع التدريبات في سجل التشغيل المشترك (`ops/drill-log.md`) عبر :

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

- تحقّق من السجل قبل التزامات عبر:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- استخدم `--status scheduled` pour les appareils ménagers et `pass`/`fail` pour les appareils ménagers et `follow-up` تبقى بنود مفتوحة.
- غيّر الوجهة باستخدام `--log` للتشغيلات التجريبية أو التحقق الآلي؛ بدونه يستمر السكربت في تحديث `ops/drill-log.md`.

## قالب ما بعد الحادث

استخدم `docs/source/sorafs/postmortem_template.md` pour le connecteur P1/P2 et le connecteur `docs/source/sorafs/postmortem_template.md`. يغطي القالب الخط الزمني، وقياس الأثر، والعوامل المساهمة، والإجراءات التصحيحية، ومهام التحقق اللاحقة.