---
lang: ar
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-ar
slug: /sorafs/pin-registry-ops-ar
---

:::ملاحظة المصدر الكنسي
المرايا `docs/source/sorafs/runbooks/pin_registry_ops.md`. حافظ على محاذاة كلا الإصدارين عبر الإصدارات.
:::

## نظرة عامة

يوثق دليل التشغيل هذا كيفية مراقبة وفرز سجل الدبوس SoraFS واتفاقيات مستوى خدمة النسخ المتماثل (SLAs) الخاصة به. تنشأ المقاييس من `iroha_torii` ويتم تصديرها عبر Prometheus ضمن مساحة الاسم `torii_sorafs_*`. يقوم Torii باختبار حالة التسجيل في فاصل زمني مدته 30 ثانية في الخلفية، بحيث تظل لوحات المعلومات محدثة حتى في حالة عدم قيام أي عامل باستقصاء نقاط نهاية `/v2/sorafs/pin/*`. قم باستيراد لوحة المعلومات المنسقة (`docs/source/grafana_sorafs_pin_registry.json`) للحصول على تخطيط Grafana الجاهز للاستخدام والذي يتم تعيينه مباشرة إلى الأقسام أدناه.

## مرجع متري

| متري | التسميات | الوصف |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | مخزون البيان على السلسلة حسب حالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | عدد الأسماء المستعارة النشطة المسجلة في السجل. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | تراكم ترتيب النسخ المتماثل مجزأة حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | مقياس الراحة يعكس أوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | محاسبة SLA: يحسب `met` الطلبات المكتملة خلال الموعد النهائي، ويجمع `missed` عمليات الإكمال المتأخرة + انتهاء الصلاحية، ويعكس `pending` الطلبات المعلقة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | زمن الوصول المجمع للإكمال (الفترات بين الإصدار والإكمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | فترات الركود للطلبات المعلقة (الموعد النهائي مطروحًا منه فترة الإصدار). |

تتم إعادة ضبط جميع أجهزة القياس عند كل عملية سحب لقطة، لذا يجب أن تأخذ لوحات المعلومات عينات عند إيقاع `1m` أو أسرع.

## Grafana لوحة القيادة

تأتي لوحة المعلومات JSON مع سبع لوحات تغطي سير عمل المشغل. تم إدراج الاستعلامات أدناه كمرجع سريع إذا كنت تفضل إنشاء مخططات مخصصة.

1. **دورة حياة البيان** – `torii_sorafs_registry_manifests_total` (مجمعة حسب `status`).
2. ** اتجاه كتالوج الاسم المستعار ** - `torii_sorafs_registry_aliases_total`.
3. **قائمة انتظار الطلبات حسب الحالة** – `torii_sorafs_registry_orders_total` (مجمعة حسب `status`).
4. **الطلبات المتراكمة مقابل الطلبات منتهية الصلاحية** - يجمع بين `torii_sorafs_replication_backlog_total` و`torii_sorafs_registry_orders_total{status="expired"}` لتشبع السطح.
5. **نسبة نجاح اتفاقية مستوى الخدمة** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **زمن الوصول مقابل فترة تأخير الموعد النهائي** – التراكب `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` و`torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم تحويلات Grafana لإضافة طرق عرض `min_over_time` عندما تحتاج إلى الحد الأدنى المطلق من فترة السماح، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** الطلبات الفائتة (معدل الساعة) ** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه- **نجاح SLA  0**
  - العتبة: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الإجراء: فحص بيانات الإدارة للتأكد من توقف المزود.
- **الإكمال صفحة 95 > متوسط الموعد النهائي المتأخر**
  - العتبة: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الإجراء: التحقق من التزام مقدمي الخدمة قبل المواعيد النهائية؛ النظر في إصدار عمليات إعادة التعيين.

### مثال لقواعد Prometheus

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## سير عمل الفرز

1. **تحديد السبب**
   - إذا أخطأت اتفاقية مستوى الخدمة (SLA) الارتفاع بينما ظل حجم الأعمال المتراكمة منخفضًا، فركز على أداء الموفر (فشل PoR، والإكمال المتأخر).
   - إذا زاد حجم الأعمال المتراكمة مع حدوث أخطاء ثابتة، قم بفحص القبول (`/v2/sorafs/pin/*`) لتأكيد البيانات التي تنتظر موافقة المجلس.
2. **التحقق من حالة المزود**
   - قم بتشغيل `iroha app sorafs providers list` وتحقق من تطابق القدرات المعلن عنها مع متطلبات النسخ المتماثل.
   - تحقق من أجهزة القياس `torii_sorafs_capacity_*` لتأكيد نجاح GiB وPoR المقدم.
3. ** إعادة تعيين النسخ المتماثل **
   - قم بإصدار أوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عندما تنخفض فترة ركود الأعمال المتراكمة (`stat="avg"`) إلى أقل من 5 فترات (يستخدم البيان/تغليف CAR `iroha app sorafs toolkit pack`).
   - قم بإخطار الإدارة إذا كانت الأسماء المستعارة تفتقر إلى روابط البيان النشطة (`torii_sorafs_registry_aliases_total` يسقط بشكل غير متوقع).
4. **نتيجة الوثيقة**
   - سجل ملاحظات الحوادث في سجل عمليات SoraFS مع الطوابع الزمنية وملخصات البيان المتأثرة.
   - قم بتحديث دليل التشغيل هذا في حالة تقديم أوضاع فشل أو لوحات معلومات جديدة.

## خطة الطرح

اتبع هذا الإجراء المرحلي عند تمكين أو تشديد سياسة ذاكرة التخزين المؤقت للاسم المستعار في الإنتاج:1. ** إعداد التكوين **
   - تحديث `torii.sorafs_alias_cache` في `iroha_config` (المستخدم → الفعلي) باستخدام TTLs ونوافذ السماح المتفق عليها: `positive_ttl`، `refresh_window`، `hard_expiry`، `negative_ttl`، `revocation_ttl`، `rotation_max_age`، و`successor_grace`، و`governance_grace`. تتطابق الإعدادات الافتراضية مع السياسة الموجودة في `docs/source/sorafs_alias_policy.md`.
   - بالنسبة لحزم SDK، قم بتوزيع نفس القيم من خلال طبقات التكوين الخاصة بها (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في روابط Rust / NAPI / Python) بحيث يتطابق تطبيق العميل مع البوابة.
2. **التشغيل الجاف في التدريج**
   - نشر تغيير التكوين إلى مجموعة مرحلية تعكس طوبولوجيا الإنتاج.
   - قم بتشغيل `cargo xtask sorafs-pin-fixtures` للتأكد من أن تركيبات الاسم المستعار المتعارف عليها لا تزال يتم فك تشفيرها ورحلة الذهاب والإياب؛ وأي عدم تطابق يعني ضمنًا انحرافًا واضحًا عند المنبع يجب معالجته أولاً.
   - قم بتمرين نقطتي النهاية `/v2/sorafs/pin/{digest}` و`/v2/sorafs/aliases` باستخدام البراهين الاصطناعية التي تغطي الحالات الجديدة ونوافذ التحديث والحالات منتهية الصلاحية والحالات منتهية الصلاحية. تحقق من صحة رموز حالة HTTP والرؤوس (`Sora-Proof-Status`، `Retry-After`، `Warning`)، وحقول نص JSON مقابل دليل التشغيل هذا.
3. **التمكين في الإنتاج**
   - طرح التكوين الجديد عبر نافذة التغيير القياسية. قم بتطبيقه على Torii أولاً، ثم أعد تشغيل خدمات البوابات/SDK بمجرد أن تؤكد العقدة السياسة الجديدة في السجلات.
   - قم باستيراد `docs/source/grafana_sorafs_pin_registry.json` إلى Grafana (أو قم بتحديث لوحات المعلومات الموجودة) وقم بتثبيت لوحات تحديث ذاكرة التخزين المؤقت للاسم المستعار في مساحة عمل NOC.
4. **التحقق بعد النشر**
   - راقب `torii_sorafs_alias_cache_refresh_total` و`torii_sorafs_alias_cache_age_seconds` لمدة 30 دقيقة. يجب أن ترتبط الارتفاعات في منحنيات `error`/`expired` بنوافذ تحديث السياسة؛ النمو غير المتوقع يعني أنه يجب على المشغلين فحص أدلة الأسماء المستعارة وصحة المزود قبل المتابعة.
   - تأكد من أن السجلات من جانب العميل تعرض نفس قرارات السياسة (ستعرض حزم SDK الأخطاء عندما يكون الدليل قديمًا أو منتهية الصلاحية). يشير غياب تحذيرات العميل إلى وجود خطأ في التكوين.
5. **الاحتياطي**
   - إذا تأخر إصدار الاسم المستعار وتعطلت نافذة التحديث بشكل متكرر، فقم بتخفيف السياسة مؤقتًا عن طريق زيادة `refresh_window` و`positive_ttl` في التكوين، ثم أعد النشر. احتفظ بـ `hard_expiry` سليمًا حتى تظل البراهين القديمة مرفوضة.
   - ارجع إلى التكوين السابق عن طريق استعادة لقطة `iroha_config` السابقة إذا استمر القياس عن بعد في إظهار أعداد `error` المرتفعة، ثم افتح حادثة لتتبع تأخيرات إنشاء الاسم المستعار.

## المواد ذات الصلة

- `docs/source/sorafs/pin_registry_plan.md` - خارطة طريق التنفيذ وسياق الإدارة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - عمليات عامل التخزين، تكمل قواعد تشغيل التسجيل هذه.
