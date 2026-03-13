---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رقم التعريف الشخصي للتسجيل
العنوان: عمليات Pin Registry
Sidebar_label: عمليات تسجيل Pin
الوصف: مراقبة وفرز Pin Registry في SoraFS ومعايير SLA للتكرار.
---

:::ملحوظة المصدر مؤهل
احترام `docs/source/sorafs/runbooks/pin_registry_ops.md`. حافظ على النسختين متزامنتين حتى تقاعد وثائق سفنكس القديمة.
:::

## نظرة عامة

يوثق هذا الدليل كيفية مراقبة وفرز Pin Registry في SoraFS واتفاقيات مستوى الخدمة (SLA) للتكرار. تنبع المعايير من `iroha_torii` وتصدر عبر Prometheus تحت مساحة الاسم `torii_sorafs_*`. يقوم Torii بأخذ عينة من حالة التسجيل كل 30 ثانية في الخلفية، لذا تعيش لوحات المعلومات محدثة حتى عند عدم السماح بتصرفات باستدعاء نقاط النهاية `/v2/sorafs/pin/*`. استراد لوحة التحكم المنقحة (`docs/source/grafana_sorafs_pin_registry.json`) للحصول على تخطيط Grafana جاهزية يطبق الأقسام أدناه مباشرة.

## مرجع المقاييس| المقياس | التسميات | الوصف |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | بيان المخزون على السفرة حسب حالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | عدد الأسماء المستعارة للـ المانيفست في التسجيل. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | عدة مرات التكرار مقسمة حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | مقياس مرجعي للضوء `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | محاسبة SLA: `met` تحسب المكتملة ضمن المهلة، `missed` جمع الإكمال المتاخر + الانتهاء، `pending` تعكس الإشارة الواضحة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | كمون الإكمال المجمع (عدد الايبوكات بين الاصدار والاكتمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | نافذة الهامش للاوامر المعلقة (المهلة ناقص ايبوك الاصدار). |

جميع المقاييس يعاد ضبطها عند كل سحب للـ snapshot، لذلك يجب ان تكون لوحات المعلومات على التلطيف `1m` او اسرع.

## لوحة Grafana

تحتوي على لوحة تحكم JSON على سبعة مخططات تغطي مسارات عمل المشغلين. يتم سرد الإشارات ادناه كمرجع سريع اذا كنت تفضل بناء مخططات مخصصة.1. **بيانات دورة حياة** – `torii_sorafs_registry_manifests_total` (مجموعة حسب `status`).
2. **اتجاه كتالوج الاسم المستعار** – `torii_sorafs_registry_aliases_total`.
3. **طابور اوامر حسب الحالة** – `torii_sorafs_registry_orders_total` (مجموعة حسب الطلب `status`).
4. **Backlog مقابل الاوامر المنتهية** – يجمع `torii_sorafs_replication_backlog_total` و `torii_sorafs_registry_orders_total{status="expired"}` لا تجارة التتجار.
5. **نسبة نجاح اتفاقية مستوى الخدمة** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **الكمون مقابل المسام المهلة** – قم بدمج `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` مع `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم تحويلات Grafana لاضافة عروض `min_over_time` عندما تحتاج الحد الادنى المطلق لهامش، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **الاوامر الفائتة (معدل 1ح)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

##عتبات التنبيه

- **نجاح SLA  0**
  - العتبة: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الاجراء: فحص البيانات الخاصة بالتوكيد لتاكيد churn لدى مقدمي الخدمة.
- **p95 للاكتمال > متوسط المدى المهلة**
  - العتبة: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الاجراء: تحقق ان مقدمي الخدمة يلتزمون قبل المهلة؛ فكر في إعادة الاسناد.

### امثلة متطلبات Prometheus

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

## سير عمل الفرز1. **تحديد السبب**
   - إذا كانت احتياجاتك أقل من SLA بينما تبقى التراكم منخفضا، ركز على اداء مقدمي الخدمة (فشل PoR، الاكتمال المتاخر).
   - اذا زاد التراكم مع اخفاقات العمر، افحص القبول (`/v2/sorafs/pin/*`) لتاكيد البيانات التي تنتظر موافقة المجلس.
2. **التحقق من مقدمي الحالة**
   - شغّل `iroha app sorafs providers list` وتأكد ان الفانتازيا عنها تطابق التكرار.
   - مقياس المراجعة `torii_sorafs_capacity_*` لتاكيد GiB المجهزة ونجاح PoR.
3. **عادة اسناد التكرار**
   - اصدر اوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عندما ينخفض التراكم (`stat="avg"`) عن 5 ايبوكات (تغليف المانيفست/CAR يستخدم `iroha app sorafs toolkit pack`).
   - اخطر الـ تورا اذا كانت الأسماء المستعارة جيدة لربط البيان النشط (انخفاض غير متوقع في `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - سجل الأحداث في سجل العمليات SoraFS مع الطوابع الزمنية والملخصات المتاثرة.
   - تحديد هذا الدليل عند ظهور فشل جديد او لوحات جديدة.

##خطة الاطلاق

اتبع هذا الاجراء المرحلي عند تفعيل او تشديد لاتخاذ ذاكرة التخزين المؤقت للـ الاسم المستعار في الإنتاج:1. ** تحضير الاعدادات **
   - تحديث `torii.sorafs_alias_cache` في `iroha_config` (المستخدم -> الفعلي) باستخدام TTLs ونوافذ الدوثر المتفق عليها: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`، `successor_grace`، `governance_grace`. القيم المناسبة تتوافق مع السياسة في `docs/source/sorafs_alias_policy.md`.
   - بالنسبة لـ SDKs، وزع نفس القيم عبر طبقات الاعداد (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في Rust / NAPI / Python Bins) حتى يتوافق مع العميل العميل.
2. **التشغيل الجاف في التدريج**
   - نشر تغيير الاعدادات على التدريج يعكس الانتاج الطوبولوجي.
   - شغّل `cargo xtask sorafs-pin-fixtures` لتاكيد ان التركيبات الكنسية للـ alias ما لتتمكن من فك التشفير خيار ذهابًا وإيابًا؛ اي لا تتطابق يعني الانجراف في المنبع معالجه اولا.
   - اختبر endpoints `/v2/sorafs/pin/{digest}` و `/v2/sorafs/aliases` معادلة تركيبية تغطي حالات طازجة و Refresh-window و منتهية الصلاحية و Hard-Expired. تحقق من اكواد HTTP و الرؤوس (`Sora-Proof-Status`, `Retry-After`, `Warning`) وحقول جسم JSON تجاه هذا الدليل.
3. **التفعيل في الإنتاج**
   - طرح الاعدادات الجديدة في نافذة الغد. طبرقها على Torii أولا، ثم تشغيل بوابات/خدمات SDK بمجرد ان تسجل السياسة الجديدة في تسجيل.
   - استرد `docs/source/grafana_sorafs_pin_registry.json` الى Grafana (او تحديث اللوحات الموجودة) وثبت لوحات تحديث ذاكرة التخزين المؤقت للـ alias في مساحة عمل NOC.
4. **التحقق بعد النشر**- راقب `torii_sorafs_alias_cache_refresh_total` و `torii_sorafs_alias_cache_age_seconds` لمدة 30 دقيقة. يجب ان ترتبط القمم في منحنيات `error`/`expired` بنوافذ الحركة؛ النمو غير المصنَّع يعني أن يتم تشغيله لفحص مقدمي خدمات الاسم المستعار المعادل قبل المتابعة.
   - تأكد من أن العملاء تعرضوا لسياسة السياسة (ستظهر أخطاء SDKs عندما يكون الدليل قديمًا أو منتهية الصلاحية). غياب تحذيرات العميل يشير الى خطا في الاعداد.
5. **الاحتياطي**
   - اذا تاخر اصدار الاسم المستعار و تكرار نافذة التحديث، خفف السياسة المؤقتة `refresh_window` و `positive_ttl` في الاعداد ثم اعد النشر. ابق `hard_expiry` كما هو حتى ينظر إلى الممثلة القديمة حاليا مرفوضة.
   - عد الى الاعداد السابق باستعادة اللقطة السابقة من `iroha_config` اذا الاعداد التميري في اإعدادت اعدادت `error` مرتفعة، ثم قم بفتح حادثة لتتبع تاخير توليد الاسم المستعار.

## مادة ذات صلة

- `docs/source/sorafs/pin_registry_plan.md` — خريطة طريق التنفيذ وسياق الـ تور.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين، تكمل هذا الدليل.