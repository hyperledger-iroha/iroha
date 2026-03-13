---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رقم التعريف الشخصي للتسجيل
العنوان: عمليات تسجيل الدبوس
Sidebar_label: عمليات تسجيل الدبوس
الوصف: مراقبة وفرز Pin Registry SoraFS ونسخ SLA المتري.
---

:::note Канонический источник
أخرج `docs/source/sorafs/runbooks/pin_registry_ops.md`. بعد ذلك، الإصدار المتزامن، بعد التوثيق التالي، لن يتم عرض أبو الهول من خلال الاستثناءات.
:::

## ملاحظة

يوضح دليل التشغيل هذا كيفية مراقبة واستخدام Triage Pin Registry SoraFS وإرشاده من الخدمة الحكومية (SLA) للنسخ المتماثل. يتم تثبيت المقاييس من `iroha_torii` ويتم تصديرها من خلال Prometheus إلى مساحة الاسم `torii_sorafs_*`. يقوم Torii بمسح حالة التسجيل خلال 30 ثانية عبر الهاتف، وتعرض لوحات المعلومات الخلفية البيانات الفعلية حتى الآن من المشغلين لا تغلق نقاط النهاية `/v2/sorafs/pin/*`. قم باستيراد لوحة القيادة المتوافقة (`docs/source/grafana_sorafs_pin_registry.json`) للتخطيط الرئيسي Grafana، الذي يعجبك تمامًا.

## مقياس الكلام| متريكا | التسميات | الوصف |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | يظهر الجرد على السلسلة من خلال دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | يتم تسجيل جميع الأسماء المستعارة النشطة في السجل. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | Backlog بسبب التكرارات المقسمة حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | مقياس رائع، خارج `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | التحقق من اتفاقية مستوى الخدمة: `met` تعهدات القراءة والكتابة، والإغلاق في الوقت المحدد، `missed` تجميع الإنهاء + الضمان، `pending` تخلص من التهم غير المرغوب فيها. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | AGregirovannaya latentность заврозения (почи медо выпуском и завраением). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | عذرًا تمامًا للأسباب غير المحددة (الموعد النهائي مطروحًا منه فترة المراجعة). |

يتم الاتصال بجميع المقاييس عند سحب اللقطة، ويجب معالجة لوحات العدادات بجزء `1m` أو غير ذلك.

## لوحة المعلومات Grafanaلوحة معلومات JSON تتصل بجميع اللوحات، مما يعزز مشغلي العمليات. لقد تم الاستعانة بخبرة جيدة إذا كنت تقترح إنشاء رسومات خاصة بك.

1. **بيانات الدورة الشهرية** – `torii_sorafs_registry_manifests_total` (المجموعة على `status`).
2. ** الاسم المستعار لكتالوج الاتجاه ** - `torii_sorafs_registry_aliases_total`.
3. **الإجابة على استفسار الحالة** – `torii_sorafs_registry_orders_total` (المجموعة على `status`).
4. **التراكم مقابل الأسباب الحقيقية** – يتبع `torii_sorafs_replication_backlog_total` و`torii_sorafs_registry_orders_total{status="expired"}` لإعادة البناء.
5. **اتفاقية مستوى الخدمة الممتازة** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **الإخفاء مقابل التأخير حتى الموعد النهائي** – طالع `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` و`torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم التحويل Grafana لتتمكن من تقديم عرض `min_over_time` عندما تحتاج إلى سلك كهربائي مطلق ببساطة، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **الخصومات الترويجية (معدل 1 ساعة)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## التنبيهات الأولية- **اتفاقية مستوى الخدمة الممتازة  0**
  -البرنامج: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الحقيقة: التحقق من بيانات الإدارة للتحقق من مقدمي الخدمات.
- **الإجابة على ص 95 > انقضى الوقت حتى الموعد النهائي**
  -البرنامج: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الحقيقة: التأكد من أن مقدمي الخدمة يتأخرون حتى الموعد النهائي؛ рассмотretь пераспеделение.

### التمهيدي الصحيح Prometheus

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
          summary: "SLA репликации SoraFS ниже целевого"
          description: "Коэффициент успеха SLA оставался ниже 95% в течение 15 минут."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog репликации SoraFS выше порога"
          description: "Ожидающие заказы репликации превысили настроенный бюджет backlog."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Заказы репликации SoraFS истекли"
          description: "По крайней мере один заказ репликации истек за последние пять минут."
```

## فرز سير العمل1. **تقديم السبب**
   - إذا تم شراء اتفاقية مستوى الخدمة (SLA)، فإن الأعمال المتراكمة تظل غير متماسكة، وتتوافق مع موفري الإنتاج (النسب الخاصة بـ PoR، والتحسين المستمر).
   - إذا تراكمت الأعمال المتراكمة خلال العروض المستقرة، تحقق من القبول (`/v2/sorafs/pin/*`)، لتتمكن من التحقق من البيانات، ومتابعة تنفيذ الأوامر.
2. **التحقق من موفري الحالة**
   - قم بتثبيت `iroha app sorafs providers list` وتأكد من أن الإمكانات الرائعة تحتاج إلى نسخ متماثلة.
   - التحقق من أجهزة القياس `torii_sorafs_capacity_*` للتحقق من توفير GiB ونجاح PoR.
3. **تحويل النسخ المتماثل**
   - شراء قروض جديدة من خلال `sorafs_manifest_stub capacity replication-order`، عند الانتهاء من تراكم الأعمال المتراكمة (`stat="avg"`) بعد 5 سنوات (استخدام بيان التعبئة/استخدام السيارة) `iroha app sorafs toolkit pack`).
   - التحقق من الإدارة، إذا لم تتضمن الأسماء المستعارة بيانات الربط النشطة (العنوان الجديد `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - قم بكتابة الأحداث في دفتر اليومية العملية SoraFS مع الطابع الزمني وبيانات الملخص.
   - تعرف على دليل التشغيل هذا، إذا قمت باستخدام أنظمة جديدة للخروج أو لوحات المعلومات.

## إعادة صياغة الخطة

اتبع هذه العملية بعد تضمينها أو استخدام الاسم المستعار السياسي لذاكرة التخزين المؤقت في البيع:1. **الموافقة على التكوين**
   - قم بتثبيت `torii.sorafs_alias_cache` في `iroha_config` (المستخدم -> الفعلي) باستخدام TTL المضمن واعترف بالامتنان: `positive_ttl`، `refresh_window`، `hard_expiry`، `negative_ttl`، `revocation_ttl`، `rotation_max_age`، `successor_grace`، `governance_grace`. التشجيع على التشجيع على السياسة في `docs/source/sorafs_alias_policy.md`.
   - يتم إنشاء أدوات تطوير البرامج (SDK) من خلال قوالب التكوين (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في روابط Rust / NAPI / Python)، بحيث يتم توصيل عملاء الإنفاذ إلى البوابة.
2. ** التدريج الجاف **
   - تكوينات واسعة النطاق في مجموعات التدريج التي تحدد طوبولوجيا البيع.
   - اضغط على `cargo xtask sorafs-pin-fixtures` لتتمكن من فك تشفير التركيبات ذات الاسم المستعار الكنسي بشكل أفضل وتنفيذها ذهابًا وإيابًا؛ أي شيء غير متوقع يشير إلى الانجراف ضد المنبع الذي يجب التخلص منه أولاً.
   - نقاط النهاية المتقدمة `/v2/sorafs/pin/{digest}` و`/v2/sorafs/aliases` مع إثبات تخليقي، مما يوفر حالات جديدة ونافذة تحديث ومنتهية الصلاحية ومنتهية الصلاحية. تحقق من رموز HTTP والرؤوس (`Sora-Proof-Status`، `Retry-After`، `Warning`) وقم بمسح JSON من خلال هذا الدليل التشغيلي.
3. **إدراج في البيع**
   - تغيير التكوينات الجديدة في التحسين القياسي. قم بالبدء في Torii، ثم قم بإعادة تشغيل البوابات/خدمات SDK بعد إعادة صياغة السياسات الجديدة في السجل.- استيراد `docs/source/grafana_sorafs_pin_registry.json` إلى Grafana (أو التعرف على لوحات المعلومات الخاصة بالنظام) وقص اللوحات لتحديث ذاكرة التخزين المؤقت للاسم المستعار بطريقة سهلة شهادة عدم الممانعة.
4. **التحقق بعد التحديث**
   -مراقبة `torii_sorafs_alias_cache_refresh_total` و`torii_sorafs_alias_cache_age_seconds` في غضون 30 دقيقة. ترتبط الصور الموجودة في `error`/`expired` بموافقة التحديث؛ يشير المبدأ الجديد إلى أن المشغلين يقومون بالتحقق من الأسماء المستعارة للإثبات ومقدمي الخدمات قبل التسليم.
   - تأكد من أن سجل العملاء يوضح هذه القرارات السياسية (يمكن لأدوات تطوير البرامج (SDKs) التحقق من الأشياء عند تثبيت الدليل أو وجودها). تشير إجابات العميل المسبق إلى التكوين غير المحدود.
5. **الاحتياطي**
   - إذا تم حذف الاسم المستعار والتحديث الكامل، فقد تم تعديله مؤقتًا، ونفذ `refresh_window` و`positive_ttl` في التكوين، بعد ذلك، يتم استخدام النشرة اللاحقة. يجب تثبيت `hard_expiry` بشكل غير عادي لإلغاء حجب الدليل الفعال.
   - التحقق من التكوينات السابقة، ودعم اللقطة السريعة `iroha_config`، إذا استمر القياس عن بعد في إظهار المزيد الاسم `error`، ثم يتم فتح عنوان URL لإنشاء الأسماء المستعارة.

## مواد صديقة للبيئة

- `docs/source/sorafs/pin_registry_plan.md` — تحقيق البطاقة الحالية وإدارة السياق.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - عامل تخزين العمليات، يكمل سجل قواعد اللعبة هذا.