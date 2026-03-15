---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رقم التعريف الشخصي للتسجيل
العنوان: عمليات التسجيل Pin
Sidebar_label: عمليات التسجيل
الوصف: مراقبة وتسجيل Pin Registry SoraFS ومقاييس النسخ المتماثل لـ SLA.
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/sorafs/runbooks/pin_registry_ops.md`. استمر في العمل على الإصدارات المتزامنة حتى يتم سحب الوثائق المتوارثة من Sphinx.
:::

## السيرة الذاتية

هذا دليل التشغيل هو مستند يقوم بمراقبة وتسجيل Pin Registry لـ SoraFS ونسخ مستوى الخدمة (SLA) الخاص به. يتم تقديم المقاييس من `iroha_torii` ويتم تصديرها عبر Prometheus من مساحة الاسم `torii_sorafs_*`. يعرض Torii حالة التسجيل في فاصل زمني مدته 30 ثانية في المخطط الثاني، حتى يتم تحديث لوحات المعلومات حتى عندما لا يقوم المشغل باستشارة نقاط النهاية `/v1/sorafs/pin/*`. قم باستيراد لوحة المعلومات الأنيقة (`docs/source/grafana_sorafs_pin_registry.json`) لتخطيط قائمة Grafana لاستخدامها في تعيين الأقسام التالية مباشرة.

## مرجع المقاييس| متريكا | التسميات | الوصف |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | مخزون البيانات على السلسلة من أجل حالة حلقة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | يحتوي على الأسماء المستعارة لبيانات الأنشطة المسجلة في السجل. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | تراكم أوامر النسخ مقسمة حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | مقياس الراحة الذي يعكس الأوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | مراقبة جيش تحرير السودان: `met` عدد الأوامر المكتملة في الموعد النهائي، `missed` يجمع عدد مكتمل متأخر + انتهاء الصلاحية، `pending` يعيد الأوامر المعلقة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | زمن الوصول المتوافق مع اللمسات النهائية (بين الإصدار والمكتمل). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Ventanas de Holgura de órdenes pendientes (الموعد النهائي أقل من عصر الانبعاثات). |

يتم تحسين جميع المقاييس في كل سحب لقطة، حيث يجب أن تتحرك لوحات المعلومات بالإيقاع `1m` أو بشكل أسرع.

## لوحة القيادة Grafanaتشتمل لوحة معلومات JSON على ست لوحات تغطي تدفقات عمل المشغلين. يتم سرد الاستشارات أدناه للمراجعة السريعة إذا كنت تفضل إنشاء رسومات متوسطة الحجم.

1. **سلسلة بيانات الحياة** – `torii_sorafs_registry_manifests_total` (تم تجميعها بواسطة `status`).
2. ** اتجاه الكتالوج الاسم المستعار ** – `torii_sorafs_registry_aliases_total`.
3. **ترتيبات الحالة** – `torii_sorafs_registry_orders_total` (مجمعة حسب `status`).
4. **التراكم مقابل أوامر انتهاء الصلاحية** – اجمع بين `torii_sorafs_replication_backlog_total` و`torii_sorafs_registry_orders_total{status="expired"}` لعرض التشبع.
5. **نسبة نجاح جيش تحرير السودان** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. ** زمن التأخر مقابل الموعد النهائي ** – superpone `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` و `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. تستخدم تحويلات Grafana لإضافة مشاهد `min_over_time` عندما تحتاج إلى قطعة الخبز المطلقة، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Órdenes Fallidas (تاسا 1 ساعة)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## مظلات التنبيه- **إنهاء SLA  0**
  - الظل: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - العمل: فحص بيانات الإدارة لتأكيد توقف مقدمي الخدمة.
- **ص95 كاملة > مدة الموعد النهائي**
  - الظل: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الإجراء: التحقق من التزام مقدمي الخدمة بالتخطيط قبل الموعد النهائي؛ النظر في إعادة النظر.

### قوانين المثال Prometheus

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
          summary: "SLA de replicación de SoraFS por debajo del objetivo"
          description: "El ratio de éxito del SLA se mantuvo por debajo de 95% durante 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicación de SoraFS por encima del umbral"
          description: "Las órdenes pendientes excedieron el presupuesto de backlog configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Órdenes de replicación de SoraFS expiradas"
          description: "Al menos una orden de replicación expiró en los últimos cinco minutos."
```

## تدفق الثلاثة1. **تحديد السبب**
   - إذا كانت أخطاء اتفاقية مستوى الخدمة (SLA) تتأخر أثناء تأخير الأعمال المتراكمة، فركز على أداء مقدمي الخدمة (فشل الأداء، التأخير الكامل).
   - إذا نشأ تراكم الأعمال مع مؤسسات مفقودة، فافحص القبول (`/v1/sorafs/pin/*`) لتأكيد البيانات على أمل الحصول على المشورة.
2. **التحقق من حالة مقدمي الخدمة**
   - قم بتشغيل `iroha app sorafs providers list` وتحقق من أن السعات المعلنة تفي بمتطلبات النسخ المتماثل.
   - قم بمراجعة المقاييس `torii_sorafs_capacity_*` لتأكيد توفير GiB ونجاح PoR.
3. ** إعادة تصميم النسخة المتماثلة **
   - قم بإصدار أوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عند ملء القائمة المتراكمة (`stat="avg"`) لمدة 5 سنوات (تغليف البيان/CAR باستخدام `iroha app sorafs toolkit pack`).
   - إشعار للتحكم في ما إذا كانت الأسماء المستعارة تحتوي على روابط بيانات النشاط (الرؤوس غير المكتملة من `torii_sorafs_registry_aliases_total`).
4. ** نتيجة توثيقية **
   - قم بتسجيل ملاحظات الحادث في سجل عمليات SoraFS مع الطوابع الزمنية وملخصات البيان المؤثر.
   - تحديث دليل التشغيل هذا عند ظهور أوضاع جديدة للفشل أو لوحات المعلومات.

## خطة الهبوط

اتبع هذا الإجراء من أجل البدء في تأهيل أو تحمل سياسة ذاكرة التخزين المؤقت للأسماء المستعارة في الإنتاج:1. **إعداد التكوين**
   - تحديث `torii.sorafs_alias_cache` و`iroha_config` (المستخدم -> الفعلي) مع TTL والنوافذ المدعومة: `positive_ttl`، `refresh_window`، `hard_expiry`، `negative_ttl`، `revocation_ttl`، `rotation_max_age`، `successor_grace` و`governance_grace`. تتزامن الإعدادات الافتراضية مع السياسة في `docs/source/sorafs_alias_policy.md`.
   - بالنسبة لحزم SDK، قم بتوزيع نفس القيم عبر طبقات التكوين الخاصة بك (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في روابط Rust / NAPI / Python) حتى يتزامن تطبيق العميل مع البوابة.
2. **التشغيل الجاف والتدريج**
   - عرض تغيير التكوين في مجموعة التدريج التي تعكس طوبولوجيا الإنتاج.
   - قم بتشغيل `cargo xtask sorafs-pin-fixtures` لتأكيد أن التركيبات القانونية ذات الأسماء المستعارة تم فك تشفيرها وإدخالها ذهابًا وإيابًا؛ أي عدم تطابق يتضمن الانجراف عندما يأتي ما يجب حله أولاً.
   - استخدم نقاط النهاية `/v1/sorafs/pin/{digest}` و`/v1/sorafs/aliases` مع اختبارات عقلانية تحمي الحالات الجديدة ونافذة التحديث والمنتهية الصلاحية والمنتهية الصلاحية. التحقق من صحة رموز HTTP والرؤوس (`Sora-Proof-Status` و`Retry-After` و`Warning`) ومجالات الجسم JSON مقابل دليل التشغيل هذا.
3. **التمكن من الإنتاج**
   - عرض التكوين الجديد على نافذة التغيير القياسية. تم التطبيق الأول على Torii وقم بإعادة تشغيل البوابات/الخدمات SDK عندما تؤكد العقدة السياسة الجديدة في السجلات.- استيراد `docs/source/grafana_sorafs_pin_registry.json` وGrafana (تحديث لوحات المعلومات الموجودة) وإعادة تحديث لوحات ذاكرة التخزين المؤقت للاسم المستعار لمساحة عمل NOC.
4. **التحقق بعد النشر**
   - الشاشة `torii_sorafs_alias_cache_refresh_total` و `torii_sorafs_alias_cache_age_seconds` لمدة 30 دقيقة. الصور المنحنية `error`/`expired` مرتبطة بفتحات التحديث؛ تشير الزيادة غير المتوقعة إلى أن المشغلين يجب عليهم فحص الأسماء المستعارة واستقبال مقدمي الخدمة قبل الاستمرار.
   - تأكد من أن سجلات عمل العميل تعرض القرارات السياسية الخاطئة (تظهر حزم SDK أخطاء عندما تكون التجربة قديمة أو منتهية الصلاحية). تشير تحذيرات العميل إلى تكوين سيء.
5. **الاحتياطي**
   - إذا عاد الانبعاث الاسم المستعار وانقطعت نافذة التحديث مع التردد، فسيتم إعادة ضبط السياسة بشكل مؤقت على `refresh_window` و`positive_ttl` في التكوين، ثم قم بإيقاف تشغيلها. حافظ على `hard_expiry` سليمًا بحيث تصبح الاختبارات قديمة بالفعل.
   - قم بإعادة التكوين إلى استعادة اللقطة السابقة لـ `iroha_config` إذا كان القياس عن بعد يعرض محتويات `error` مرتفعة، ثم حدث حادث لتراجعات عكسية في جيل الأسماء المستعارة.

## المواد ذات الصلة- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الإدارة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين، المكملة لدليل التسجيل هذا.