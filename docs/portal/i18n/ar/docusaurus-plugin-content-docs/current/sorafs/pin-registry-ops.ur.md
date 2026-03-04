---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رقم التعريف الشخصي للتسجيل
العنوان: عمليات تسجيل الدبوس
Sidebar_label: عمليات تسجيل الدبوس
الوصف: SoraFS دبوس التسجيل والنسخ المتماثل SLA حركات ونطاقات.
---

:::ملاحظة مستند ماخذ
هذه هي الصفحة `docs/source/sorafs/runbooks/pin_registry_ops.md`. بعد أن لم تعد سجلات أبو الهول تتشكل بعد الآن.
:::

##جائزہ

يحتوي هذا الدليل على دليل تشغيل SoraFS وهو عبارة عن سجل الدبوس والنسخ المتماثل لسلسلات الويول إيغرمنتس (SLA) وهو جديد. تم تحديث العلامة التجارية `iroha_torii` وPrometheus إلى مساحة الاسم `torii_sorafs_*` من Microsoft. Torii سجل منظر الصورة الذي تم إجراؤه في 30 ثانية فقط وهو ما يجعل كل ما عليك فعله هو الحصول على هذه الصورة الرائعة لا يوجد أي رصيد لنقاط النهاية `/v1/sorafs/pin/*`. يحتوي الإطار بالكامل (`docs/source/grafana_sorafs_pin_registry.json`) على منفذ بطاقة Grafana وهو عبارة عن تخطيط إطار واحد متطور لتصوير الفيديو بالكامل.

## ميٹرک حوالہ| ميرك | التسميات | وضاحت |
| ----- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | وهذا يتجلى كنوع آخر من التصنيف العلمي وفقًا لذلك. |
| `torii_sorafs_registry_aliases_total` | — | تم استخدام عدد كبير من الأسماء المستعارة الواضحة في سجل التسجيل. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | أوامر النسخ المتماثل هي أمر متراكم. |
| `torii_sorafs_replication_backlog_total` | — | قم بقياس أوامر `pending` التي ستصدرها. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | اتفاقية مستوى الخدمة: `met` وقت تنفيذ الأوامر المكتملة، `missed` مكتملة المدة + عمليات انتهاء الصلاحية المجمعة، `pending` أوامر التعديل ظهور كرتا . |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | زمن الوصول للإكمال إجمالي (إصدار وإكمال عصور طبية). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | الأوامر المعلقة کی فترات الركود (الموعد النهائي مطروحًا منه الحقبة الصادرة)۔ |

كل المقاييس هي سحب اللقطة للموقع، وهي عبارة عن بوردة مثل `1m` أو ضبط الإيقاع لعينة ما.

## Grafana ڈيش بورڈتعمل كل من لوحات JSON على إنشاء عمل إبداعي للبطاقة. إذا حدث لك مرض ما، فما عليك سوى أن تصبح حوالة فورية درجة.

1. **دورة حياة البيان** – `torii_sorafs_registry_manifests_total` (`status` المجموعة المطابقة).
2. **اتجاه كتالوج الاسم المستعار** - `torii_sorafs_registry_aliases_total`.
3. **قائمة انتظار الطلبات حسب الحالة** – `torii_sorafs_registry_orders_total` (`status` المجموعة المطابقة).
4. **الطلبات المتراكمة مقابل الطلبات منتهية الصلاحية** – `torii_sorafs_replication_backlog_total` و`torii_sorafs_registry_orders_total{status="expired"}` وهو ما يمثل تشبعًا كاملاً.
5. **نسبة نجاح اتفاقية مستوى الخدمة** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **زمن الوصول مقابل تأخير الموعد النهائي** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` و`torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` في البداية. كيفية استخدام تحويلات Grafana إلى `min_over_time` طرق عرض شاملة، مثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** الطلبات الفائتة (معدل الساعة) ** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه

- **نجاح SLA  0**
  - العتبة: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الإجراء: تُظهر الحوكمة أن مقدمي الخدمات يقومون بتصنيع منتجات جديدة.
- **الإكمال صفحة 95 > متوسط الموعد النهائي المتأخر**
  - العتبة: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الإجراء: التحقق من المواعيد النهائية لمقدمي الخدمة قبل الالتزام بالمواعيد النهائية؛ إعادة التعيينات على أساس غور.

### مثال Prometheus متطلبات```yaml
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
          summary: "SoraFS replication SLA ہدف سے کم"
          description: "SLA کامیابی کا تناسب 15 منٹ تک 95% سے کم رہا۔"

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog حد سے اوپر"
          description: "زیر التواء replication orders مقررہ backlog بجٹ سے بڑھ گئے۔"

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders ختم ہو گئیں"
          description: "پچھلے پانچ منٹ میں کم از کم ایک replication order ختم ہوئی۔"
```

## سير عمل الفرز

1. **الأمر سيان**
   - إذا أخطأت اتفاقية مستوى الخدمة (SLA) كثيرًا وتراكمت أعمالك، فقد يؤدي ذلك إلى توقف مقدمي الخدمة عن العمل (فشل PoR، والإكمال المتأخر).
   - إذا كانت الأعمال المتراكمة والأخطاء تستحق القبول (`/v1/sorafs/pin/*` بشكل واضح) فإن هذا الإجراء يتجلى في رؤية المستشار المنتظر.
2. **مقدمو الخدمات ملتزمون**
   - `iroha app sorafs providers list` تم طلب وتسجيل إعلان صلاحيت النسخ المتماثل بطلب أكثر من مليون دولار.
   - مقاييس `torii_sorafs_capacity_*` تعمل على توفير نجاح في اختبار GiB وPoR.
3. ** النسخ المتماثل إعادة التعيين **
   - فترة الركود المتراكمة (`stat="avg"`) 5 عصور ستستمر `sorafs_manifest_stub capacity replication-order` في تنفيذ أوامر جارية (البيان/تغليف CAR `iroha app sorafs toolkit pack` يستخدم ككرتا)).
   - إذا كانت الأسماء المستعارة فعالة في الارتباطات الواضحة فلن تتمكن من الحوكمة بشكل واضح (`torii_sorafs_registry_aliases_total` ستتأخر لاحقًا).
4. **منتجات البناء**
   - يحتوي سجل عمليات SoraFS على طوابع زمنية وملخصات واضحة بالإضافة إلى ملاحظات الحوادث في الجزء السفلي.
   - إذا كانت هناك أوضاع فشل جديدة أو لوحات معلومات، فيمكنك استخدام دليل التشغيل.

## خطة الطرح

يعمل المشروع على سياسة ذاكرة التخزين المؤقت للاسم المستعار التي تعمل على تنشيط أو تقليل وقت الحرب من خلال عمليات صنع مختلفة:1. **سلسلة التكوين**
   - `iroha_config` `torii.sorafs_alias_cache` (المستخدم -> الفعلي) الذي يتوافق مع TTLs ونوافذ النعمة التي تم تسجيلها: `positive_ttl`، `refresh_window`، `hard_expiry`، `negative_ttl`، `revocation_ttl`، `rotation_max_age`، `successor_grace`، `governance_grace`. الإعدادات الافتراضية `docs/source/sorafs_alias_policy.md` هي باليس متعددة الاستخدامات.
   - تعمل مجموعات SDK على زيادة حجم طبقات التكوين (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` روابط Rust / NAPI / Python) بالإضافة إلى بوابة إنفاذ العميل.
2. ** التدريج للتشغيل الجاف **
   - يتم تغيير التكوين إلى مجموعة مرحلية لنشر طوبولوجيا إنتاج الطاقة.
   - `cargo xtask sorafs-pin-fixtures` يقوم بفك تشفير تركيبات الاسم المستعار المتعارف عليه ذهابًا وإيابًا؛ عدم تطابق الانجراف ضد المنبع دکھاتا وهو حق صحيح.
   - نقاط النهاية `/v1/sorafs/pin/{digest}` و`/v1/sorafs/aliases` هي البراهين الاصطناعية التي لا تزال جديدة ونافذة التحديث ومنتهية الصلاحية ومنتهية الصلاحية. رموز حالة HTTP، والرؤوس (`Sora-Proof-Status`، `Retry-After`، `Warning`) وحقول نص JSON الموجودة في دليل التشغيل تتوافق مع التحقق من الصحة.
3. **الإنتاج فعال**
   - نافذة التغيير القياسية تلعب دورًا جديدًا في التكوين. بعد Torii في اللعبة، يتم تشغيل العقدة الجديدة التي يتم فحصها بعد إعادة تشغيل خدمات البوابات/SDK.
   - `docs/source/grafana_sorafs_pin_registry.json` وGrafana مزود ببطاقة بريدية (أو لوحات معلومات موجودة) ولوحات تحديث ذاكرة التخزين المؤقت الاسم المستعار التي تثبت مساحة عمل NOC.4. **التحقق بعد النشر**
   - 30 مليونًا `torii_sorafs_alias_cache_refresh_total` و`torii_sorafs_alias_cache_age_seconds` بطريقة مختلفة. `error`/`expired` منحنيات تحتوي على مسامير لتحديث النوافذ وترتبط بما هو مطلوب؛ تم لاحقًا إضافة عدد من المشغلين والأدلة المستعارة ومقدمي الخدمات الصحية.
   - يتم النظر في السجلات من جانب العميل وقرارات السياسة (أدوات تطوير البرامج (SDKs قديمة أو منتهية الصلاحية) دليل على الأخطاء). تحذيرات العميل لا تسبب أخطاء في تكوين المعلومات.
5. **الاحتياطي**
   - إذا تم إصدار الاسم المستعار وتحديث شريط النافذة، فسيتم إعادة تشغيل `refresh_window` و`positive_ttl` على جهاز العرض بالكامل، ثم إعادة النشر. `hard_expiry` تم إعادة إنتاج هذه البراهين القديمة الحقيقية.
   - إذا كان `error` يحسب القياس عن بعد، فسيتم حذف `iroha_config` من لقطة اللقطة أثناء تكوين اللقطة، كما يؤدي إنشاء تأخيرات في الاسم المستعار إلى وقوع حادث.

## ما يتعلقہ مادة

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الحوكمة ۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين، وهي عبارة عن دليل تشغيل التسجيل الخاص به.