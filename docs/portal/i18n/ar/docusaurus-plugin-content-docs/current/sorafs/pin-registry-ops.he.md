---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 80bc7b939264e64466b3fe1395be868f4717df5a811c62e14af2f030540c9862
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-ops
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
تعكس `docs/source/sorafs/runbooks/pin_registry_ops.md`. حافظ على النسختين متزامنتين حتى تقاعد وثائق Sphinx القديمة.
:::

## نظرة عامة

يوثق هذا الدليل كيفية مراقبة وفرز Pin Registry في SoraFS واتفاقيات مستوى الخدمة (SLA) للتكرار. تنبع المقاييس من `iroha_torii` وتصدر عبر Prometheus تحت مساحة الاسم `torii_sorafs_*`. يقوم Torii بأخذ عينات من حالة registry كل 30 ثانية في الخلفية، لذا تبقى لوحات المعلومات محدثة حتى عند عدم قيام المشغلين باستدعاء نقاط النهاية `/v1/sorafs/pin/*`. استورد لوحة التحكم المنقحة (`docs/source/grafana_sorafs_pin_registry.json`) للحصول على تخطيط Grafana جاهز الاستخدام يطابق الأقسام أدناه مباشرة.

## مرجع المقاييس

| المقياس | Labels | الوصف |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | مخزون manifests على السلسلة بحسب حالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | عدد aliases النشطة للـ manifest المسجلة في registry. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | تراكم أوامر التكرار مقسما حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | Gauge مرجعي يعكس أوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | محاسبة SLA: `met` تحسب الأوامر المكتملة ضمن المهلة، `missed` يجمع الإكمال المتاخر + الانتهاء، `pending` يعكس الأوامر المعلقة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | كمون الإكمال المجمع (عدد الايبوكات بين الاصدار والاكتمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | نوافذ الهامش للاوامر المعلقة (المهلة ناقص ايبوك الاصدار). |

جميع gauges يعاد ضبطها عند كل سحب للـ snapshot، لذا يجب ان تكون لوحات المعلومات على وتيرة `1m` او اسرع.

## لوحة Grafana

تحتوي لوحة التحكم JSON على سبعة مخططات تغطي مسارات عمل المشغلين. يتم سرد الاستعلامات ادناه كمرجع سريع اذا كنت تفضل بناء مخططات مخصصة.

1. **دورة حياة manifests** – `torii_sorafs_registry_manifests_total` (مجموعة حسب `status`).
2. **اتجاه كتالوج alias** – `torii_sorafs_registry_aliases_total`.
3. **طابور الاوامر حسب الحالة** – `torii_sorafs_registry_orders_total` (مجموعة حسب `status`).
4. **Backlog مقابل الاوامر المنتهية** – يجمع `torii_sorafs_replication_backlog_total` و `torii_sorafs_registry_orders_total{status="expired"}` لاظهار التشبع.
5. **نسبة نجاح SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **الكمون مقابل هامش المهلة** – قم بدمج `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` مع `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم تحويلات Grafana لاضافة عروض `min_over_time` عندما تحتاج الحد الادنى المطلق للهامش، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **الاوامر الفائتة (معدل 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه

- **نجاح SLA < 0.95 لمدة 15 دقيقة**
  - العتبة: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - الاجراء: استدعاء SRE؛ ابدأ فرز تراكم التكرار.
- **تراكم معلق اعلى من 10**
  - العتبة: `torii_sorafs_replication_backlog_total > 10` مستمرة لمدة 10 دقائق
  - الاجراء: تحقق من توفر providers وجدولة سعة Torii.
- **الاوامر المنتهية > 0**
  - العتبة: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الاجراء: افحص manifests الخاصة بالحوكمة لتاكيد churn لدى providers.
- **p95 للاكتمال > متوسط هامش المهلة**
  - العتبة: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الاجراء: تحقق ان providers يلتزمون قبل المهلة؛ فكر في اعادة الاسناد.

### امثلة قواعد Prometheus

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
          summary: "هبوط SLA لتكرار SoraFS تحت الهدف"
          description: "ظلت نسبة نجاح SLA اقل من 95% لمدة 15 دقيقة."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "تراكم تكرار SoraFS فوق العتبة"
          description: "تجاوزت اوامر التكرار المعلقة ميزانية التراكم المضبوطة."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "انتهاء اوامر تكرار SoraFS"
          description: "انتهى امر تكرار واحد على الاقل في الخمس دقائق الماضية."
```

## سير عمل الفرز

1. **تحديد السبب**
   - اذا زادت اخفاقات SLA بينما بقي التراكم منخفضا، ركز على اداء providers (فشل PoR، الاكتمال المتاخر).
   - اذا زاد التراكم مع اخفاقات مستقرة، افحص القبول (`/v1/sorafs/pin/*`) لتاكيد manifests التي تنتظر موافقة المجلس.
2. **التحقق من حالة providers**
   - شغّل `iroha app sorafs providers list` وتاكد ان القدرات المعلن عنها تطابق متطلبات التكرار.
   - راجع gauges `torii_sorafs_capacity_*` لتاكيد GiB المجهزة ونجاح PoR.
3. **اعادة اسناد التكرار**
   - اصدر اوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عندما ينخفض هامش التراكم (`stat="avg"`) عن 5 ايبوكات (تغليف manifest/CAR يستخدم `iroha app sorafs toolkit pack`).
   - اخطر الحوكمة اذا كانت aliases تفتقر لربط manifest نشط (انخفاض غير متوقع في `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - سجل ملاحظات الحادث في سجل عمليات SoraFS مع الطوابع الزمنية وdigests المتاثرة.
   - حدّث هذا الدليل عند ظهور اوضاع فشل جديدة او لوحات جديدة.

## خطة الاطلاق

اتبع هذا الاجراء المرحلي عند تفعيل او تشديد سياسة cache للـ alias في الانتاج:

1. **تحضير الاعدادات**
   - حدّث `torii.sorafs_alias_cache` في `iroha_config` (user -> actual) باستخدام TTLs ونوافذ السماح المتفق عليها: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. القيم الافتراضية تطابق السياسة في `docs/source/sorafs_alias_policy.md`.
   - بالنسبة لـ SDKs، وزع نفس القيم عبر طبقات الاعداد (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في Rust / NAPI / Python bindings) حتى يطابق تنفيذ العميل البوابة.
2. **Dry-run في staging**
   - انشر تغيير الاعدادات على عنقود staging يعكس طوبولوجيا الانتاج.
   - شغّل `cargo xtask sorafs-pin-fixtures` لتاكيد ان fixtures الكنسية للـ alias ما زالت تفك التشفير وتقوم بعملية round-trip؛ اي عدم تطابق يعني drift في المنبع يجب معالجته اولا.
   - اختبر endpoints `/v1/sorafs/pin/{digest}` و `/v1/sorafs/aliases` بادلة تركيبية تغطي حالات fresh و refresh-window و expired و hard-expired. تحقق من اكواد HTTP و headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) وحقول جسم JSON مقابل هذا الدليل.
3. **التفعيل في الانتاج**
   - اطرح الاعدادات الجديدة في نافذة التغيير القياسية. طبّقها على Torii اولا، ثم اعد تشغيل gateways/خدمات SDK بمجرد ان يؤكد العقدة السياسة الجديدة في السجلات.
   - استورد `docs/source/grafana_sorafs_pin_registry.json` الى Grafana (او حدّث اللوحات الموجودة) وثبت لوحات تحديث cache للـ alias في مساحة عمل NOC.
4. **التحقق بعد النشر**
   - راقب `torii_sorafs_alias_cache_refresh_total` و `torii_sorafs_alias_cache_age_seconds` لمدة 30 دقيقة. يجب ان ترتبط القمم في منحنيات `error`/`expired` بنوافذ التحديث؛ النمو غير المتوقع يعني ان على المشغلين فحص ادلة alias وصحة providers قبل المتابعة.
   - تاكد ان سجلات العميل تعرض نفس قرارات السياسة (ستظهر SDKs اخطاء عندما يكون الدليل stale او expired). غياب تحذيرات العميل يشير الى خطا في الاعداد.
5. **Fallback**
   - اذا تاخر اصدار alias وتكرر تجاوز نافذة التحديث، خفف السياسة مؤقتا بزيادة `refresh_window` و `positive_ttl` في الاعداد ثم اعد النشر. ابق `hard_expiry` كما هو حتى تظل الادلة القديمة فعلا مرفوضة.
   - عد الى الاعداد السابق باستعادة snapshot السابق من `iroha_config` اذا استمرت التليمتري في اظهار اعداد `error` مرتفعة، ثم افتح حادثة لتتبع تاخير توليد alias.

## مواد ذات صلة

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين، تكمل هذا الدليل.
