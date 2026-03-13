---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رقم التعريف الشخصي للتسجيل
العنوان: Operacoes do Pin Registry
Sidebar_label: عمليات التشغيل تقوم بتسجيل الدبوس
الوصف: مراقبة وفرز بيانات Pin Registry لـ SoraFS ومقاييس النسخ المتماثل لـ SLA.
---

:::ملاحظة فونتي كانونيكا
إسبيلها `docs/source/sorafs/runbooks/pin_registry_ops.md`. استمر في العمل كآيات متزامنة أكلت مستندات من قطيع أبو الهول.
:::

## فيساو جيرال

هذا المستند الخاص بدليل التشغيل هو عبارة عن أداة مراقبة وفرز من خلال سجل Pin لـ SoraFS وتوافقاته مع مستوى الخدمة (SLA) المتماثلة. كما هي المقاييس الأصلية في `iroha_torii` ويتم تصديرها عبر Prometheus sob أو مساحة الاسم `torii_sorafs_*`. Torii يتم تسجيله أو حالته في فاصل زمني مدته 30 ثانية في الخطة الثانية، مما يسمح بتحسين لوحات المعلومات حتى عندما يقوم المشغل باستشارة نقاط النهاية `/v2/sorafs/pin/*`. قم باستيراد لوحة المعلومات المعالجة (`docs/source/grafana_sorafs_pin_registry.json`) لتخطيط Grafana قريبًا لاستخدامها في التخطيط مباشرة لبضع ثوان.

## مرجع المقاييس| متريكا | التسميات | وصف |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | مخزون البيانات على السلسلة من أجل حالة الحياة. |
| `torii_sorafs_registry_aliases_total` | - | إرسال الأسماء المستعارة للبيانات المسجلة غير المسجلة. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | تراكم أوامر النسخ المتماثل حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | - | مقياس الراحة الذي يعرض الأوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | مراقبة SLA: `met` تحتوي على أوامر موجزة في الموعد النهائي، `missed` تجمع الاستنتاجات المتأخرة + انتهاء الصلاحية، `pending` تحدد الأوامر المعلقة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia aggregada de conclusao (مراحل بين الإرسال والنتيجة). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de ordens pendentes (الموعد النهائي أقل من epoca de emissao). |

تعمل جميع المقاييس على إعادة تشغيل كل لقطة، بما في ذلك لوحات المعلومات، والتي يمكن ضبطها على الإيقاع `1m` أو بسرعة أكبر.

## لوحة المعلومات تفعل Grafanaتشتمل لوحة معلومات JSON على مجموعة من الألم تتضمن تدفقات عمل المشغلين. كما أن الاستشارات الإضافية تخدم كمرجع سريع للقضية الصوتية المفضلة، فهي عبارة عن رسومات بيانية تنهدات متوسطة.

1. **سلسلة بيانات الحياة** - `torii_sorafs_registry_manifests_total` (تم تجميعها بواسطة `status`).
2. **اتجاه كتالوج الاسم المستعار** - `torii_sorafs_registry_aliases_total`.
3. **حالة الأوامر حسب الحالة** - `torii_sorafs_registry_orders_total` (مجموعة حسب `status`).
4. **التراكم مقابل أوامر انتهاء الصلاحية** - اجمع بين `torii_sorafs_replication_backlog_total` و`torii_sorafs_registry_orders_total{status="expired"}` لعرض التشبع.
5. **نجاح نجاح جيش تحرير السودان** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia vs folga do الموعد النهائي** - sobrepoe `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم المحولات Grafana لإضافة مشاهد `min_over_time` عندما تحتاج إلى قطع كامل للقطعة، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **أوامر الخسارة (التصنيف 1 ساعة)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## حدود التنبيه- **نجاح SLA  0**
  - الحد: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - أكاو: قوائم التفتيش الحكومية لتأكيد توقف مقدمي الخدمة.
- **ص 95 من الخلاصة > جميع وسائل الموعد النهائي**
  - الحد: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acao: التحقق من مقدمي الخدمات الذين يلتزمون بالمواعيد النهائية مسبقًا؛ النظر في reatribuicoes.

### Regras de Prometheus de exemplo

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
          summary: "SLA de replicacao SoraFS abaixo da meta"
          description: "A razao de sucesso do SLA ficou abaixo de 95% por 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicacao SoraFS acima do limiar"
          description: "Ordens de replicacao pendentes excederam o budget configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordens de replicacao SoraFS expiradas"
          description: "Pelo menos uma ordem de replicacao expirou nos ultimos cinco minutos."
```

## فلوكسو دي ترياجيم1. **تحديد السبب**
   - إذا كانت أخطاء اتفاقية مستوى الخدمة (SLA) تزيد من بقاء الأعمال المتراكمة منخفضة، فلا تتطلب من مقدمي الخدمة (خطأ في الأداء، استنتاجات متأخرة).
   - إذا نشأ تراكم الأعمال المتراكمة مع المفقودين، فافحص القبول (`/v2/sorafs/pin/*`) لتأكيد بيانات الضمان التي تم تقديمها للمشورة.
2. **موفرو حالة التحقق من الصحة**
   - قم بتنفيذ `iroha app sorafs providers list` وتحقق من أن القدرات المعلنة تتوافق مع متطلبات النسخ المتماثل.
   - التحقق من المقاييس `torii_sorafs_capacity_*` لتأكيد توفير GiB ونجاح PoR.
3. ** رياتريبوير ريبلاكاو **
   - قم بإصدار أوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عند إكمال العمل المتراكم (`stat="avg"`) بعد 5 حلقات (أو تعبئة البيان/CAR باستخدام `iroha app sorafs toolkit pack`).
   - إعلام بالتحكم في الأسماء المستعارة التي تحتوي على روابط بيانية (quedas inesperadas em `torii_sorafs_registry_aliases_total`).
4. ** نتيجة توثيقية **
   - قم بتسجيل ملاحظات الأحداث في سجل عمليات SoraFS مع الطوابع الزمنية وملخصات البيانات الصادرة.
   - قم بتنشيط دليل التشغيل هذا مع وضعيات التشغيل الجديدة أو لوحات المعلومات التي تم تقديمها.

## خطة الطرح

قم بإجراء هذا الإجراء من خلال تأهيل أو تحمل سياسة ذاكرة التخزين المؤقت للأسماء المستعارة المنتجة:1. ** إعداد التكوين **
   - تحديث `torii.sorafs_alias_cache` في `iroha_config` (المستخدم -> الفعلي) مع TTLs والملفات المجانية المسجلة: `positive_ttl`، `refresh_window`، `hard_expiry`، `negative_ttl`، `revocation_ttl`، `rotation_max_age`، `successor_grace` و`governance_grace`. تتوافق الإعدادات الافتراضية مع السياسة في `docs/source/sorafs_alias_policy.md`.
   - بالنسبة لحزم SDK، قم بتوزيع القيم نفسها من خلال مجموعات التكوين الخاصة بك (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في روابط Rust / NAPI / Python) بحيث يتوافق العميل مع البوابة.
2. ** التدريج الجاف **
   - زرع تعديل التكوين في مجموعة التدريج التي تهدف إلى طوبولوجيا الإنتاج.
   - نفذ `cargo xtask sorafs-pin-fixtures` لتأكيد أن التركيبات الكنسيونية ذات الأسماء المستعارة ستساعد في فك التشفير والذهاب ذهابًا وإيابًا؛ يتضمن أي عدم تطابق الانجراف نحو المنبع الذي يجب أن يكون الحل الأول.
   - ممارسة نقاط النهاية `/v2/sorafs/pin/{digest}` و`/v2/sorafs/aliases` مع اختبار cobrindo كاسوس جديدة، نافذة التحديث، منتهية الصلاحية ومنتهية الصلاحية. رموز HTTP صالحة، والرؤوس (`Sora-Proof-Status`، `Retry-After`، `Warning`) ومجالات جسم JSON مقابل دليل التشغيل هذا.
3. **التمكن من الإنتاج**
   - زرع تكوين جديد في حشوة الزرع. قم بالتطبيق أولاً على Torii ثم قم بإنشاء بوابات/خدمات SDK حتى تتمكن من تأكيد سجلاتنا السياسية الجديدة.- استيراد `docs/source/grafana_sorafs_pin_registry.json` إلى Grafana (أو تحديث لوحات المعلومات الموجودة) وإصلاح آلام التحديث في ذاكرة التخزين المؤقت للاسم المستعار في مساحة عمل NOC.
4. **التحقق من النشر**
   - شاشة `torii_sorafs_alias_cache_refresh_total` و`torii_sorafs_alias_cache_age_seconds` لمدة 30 دقيقة. يجب أن ترتبط الصور المنحنية `error`/`expired` بملفات التحديث؛ يشير التزايد غير المكتمل إلى أن المشغلين يجب عليهم التحقق من الأسماء المستعارة والاتصال بمقدمي الخدمة قبل الاستمرار.
   - تأكد من أن السجلات التي يتم إرسالها إلى العميل تظهر كرسائل قرارات سياسية (تصدر حزم SDK لنظام التشغيل أخطاء عند إثبات أنها قديمة أو منتهية الصلاحية). تشير تحذيرات العميل إلى عدم صحة التكوين.
5. **الاحتياطي**
   - إذا تم إرسال الاسم المستعار وحذف قائمة التحديث مع التردد، قم بإرخاء السياسة مؤقتًا وزيادة `refresh_window` و`positive_ttl` في التكوين، بعد إعادة الزرع. حافظ على `hard_expiry` سليمًا حتى تثبت أنه قديم بالفعل في نفس الوقت الذي تم تجديده فيه.
   - قم بالرجوع إلى التكوين السابق للمطعم أو اللقطة السابقة لـ `iroha_config` إذا استمر القياس عن بعد في عرض عدوى `error` المرتفعة، ثم قم بمتابعة حادثة لمسح البيانات باستخدام الاسم المستعار.

## المواد ذات الصلة

- `docs/source/sorafs/pin_registry_plan.md` - خارطة طريق التنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - عمليات التشغيل الخاصة بالتخزين، وقواعد اللعب المكملة لهذا التسجيل.