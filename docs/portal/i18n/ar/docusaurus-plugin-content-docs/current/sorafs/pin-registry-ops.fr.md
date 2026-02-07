---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: رقم التعريف الشخصي للتسجيل
العنوان: عمليات du Pin Registry
Sidebar_label: عمليات تسجيل Pin
الوصف: مراقبة واختبار Pin Registry SoraFS ومقاييس SLA للنسخ المتماثل.
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/sorafs/runbooks/pin_registry_ops.md`. تمت مزامنة الإصدارات الثنائية حتى يتم التراجع عن الوثائق التي ورثها أبو الهول.
:::

## عرض الفرقة

يشير كتاب التشغيل هذا إلى مراقبة وتجربة Pin Registry SoraFS وتوافقات مستوى الخدمة (SLA) الخاصة بالنسخ المتماثل. يتم تصدير المقاييس من `iroha_torii` عبر Prometheus إلى مساحة الاسم `torii_sorafs_*`. Torii يقوم بتنشيط حالة التسجيل لمدة 30 ثانية في المخطط الأخير، مع بقاء لوحات المعلومات في نفس الوقت عندما يقوم المشغل باستجواب نقاط النهاية `/v1/sorafs/pin/*`. قم باستيراد لوحة المعلومات المنسقة (`docs/source/grafana_sorafs_pin_registry.json`) للتخطيط Grafana قبل العمل الذي يتوافق مباشرة مع الأقسام ci-dessous.

## مرجع المقاييس| متريك | التسميات | الوصف |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | اختراع البيانات على السلسلة وفقًا لحالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | أسماء الأسماء المستعارة للبيانات النشطة المسجلة في السجل. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \|`completed` \|`expired`) | تراكم أوامر النسخ المجزأ حسب القانون. |
| `torii_sorafs_replication_backlog_total` | — | مقياس السلع يعكس الطلبات `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | توافق اتفاقية مستوى الخدمة: `met` يحسب الأوامر المنتهية في التأخيرات، `missed` يجمع الإكمال المتأخر + انتهاء الصلاحية، `pending` يعكس الأوامر في حالة انتباه. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | زمن الإكمال المتفق عليه (فترة ما بين الإصدار والإكمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Fenêtres de marge des ordres en attente (الموعد النهائي أقل من époque d'émission). |

تتم إعادة تهيئة جميع المقاييس من خلال استخراج كل لقطة، بينما يتم تحديث لوحات المعلومات بإيقاع `1m` أو أكثر سرعة.

## لوحة المعلومات Grafanaتحتوي لوحة المعلومات JSON على لوحات تغطي مشغلي سير العمل. تم إدراج الطلبات بشكل متكرر للإشارة السريعة إلى ما إذا كنت تفضل إنشاء رسومات بالقياس.

1. **دورة حياة البيانات** – `torii_sorafs_registry_manifests_total` (المجموعة على أساس `status`).
2. ** اتجاه الكتالوج المستعار ** – `torii_sorafs_registry_aliases_total`.
3. **ملف الأوامر حسب الوضع** – `torii_sorafs_registry_orders_total` (مجموعة حسب `status`).
4. **التراكم مقابل الأوامر منتهية الصلاحية** – اجمع بين `torii_sorafs_replication_backlog_total` و`torii_sorafs_registry_orders_total{status="expired"}` لقياس التشبع.
5. **نسبة تجديد مستوى الخدمة** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **زمن الوصول مقابل هامش الموعد النهائي** – ضع `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` و`torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم التحويلات Grafana لإضافة رؤية `min_over_time` عندما تحتاج إلى أداة طحن الحافة المطلقة، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Ordres manqués (taux 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## البقاء في حالة تأهب- **نجاح SLA  0**
  - سيويل: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الإجراء: فحص بيانات الإدارة لتأكيد تغيير مقدمي الخدمة.
- **ص95 من الإكمال > آخر شهر من الموعد النهائي**
  - سيويل: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الإجراء : التحقق من صلاحية مقدمي الخدمة قبل المواعيد النهائية ; متصور إعادة التعيينات.

### أمثلة على القواعد Prometheus

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
          summary: "SLA de réplication SoraFS sous la cible"
          description: "Le ratio de succès SLA est resté sous 95% pendant 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de réplication SoraFS au-dessus du seuil"
          description: "Les ordres de réplication en attente ont dépassé le budget de backlog configuré."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordres de réplication SoraFS expirés"
          description: "Au moins un ordre de réplication a expiré au cours des cinq dernières minutes."
```

## سير العمل في الفرز1. **معرف السبب**
   - إذا كانت اختبارات SLA تزيد من فشل الأعمال المتراكمة، ركز على التحليل حول أداء مقدمي الخدمة (فحوصات الأداء، والإكمال المتأخر).
   - إذا تم زيادة التراكم باستخدام إسطبلات الفحص، قم بفحص القبول (`/v1/sorafs/pin/*`) لتأكيد البيانات في انتظار الموافقة على الاستشارة.
2. **التحقق من حالة مقدمي الخدمة**
   - قم بتنفيذ `iroha app sorafs providers list` وتحقق من أن القدرات المعلنة تتوافق مع متطلبات النسخ المتماثل.
   - تحقق من المقاييس `torii_sorafs_capacity_*` لتأكيد توفير GiB ونجاح PoR.
3. **إعادة تعيين النسخ**
   - إنشاء أوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عند نزول حافة الأعمال المتراكمة (`stat="avg"`) خلال 5 فترات (يستخدم بيان التعبئة/CAR `iroha app sorafs toolkit pack`).
   - Notifier la gouvernance si les aliases n'ont pas de links de واضح النشاط (baisse inattendue de `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - أرسل ملاحظات الحادث إلى مجلة العمليات SoraFS مع الطوابع الزمنية وملخصات البيان المعني.
   - يتم تقديم دليل التشغيل هذا في حالة وجود أوضاع فحص جديدة أو لوحات معلومات.

## خطة النشر

تابع هذا الإجراء من خلال خطوات التنشيط أو متابعة سياسة التخزين المؤقت للأسماء المستعارة في الإنتاج:1. **إعداد التكوين**
   - تحديث `torii.sorafs_alias_cache` في `iroha_config` (المستخدم -> الفعلي) مع TTL ونوافذ الرحمة المدمجة: `positive_ttl`، `refresh_window`، `hard_expiry`، `negative_ttl`، `revocation_ttl`، `rotation_max_age`، `successor_grace` و`governance_grace`. تتوافق القيم الافتراضية مع السياسة `docs/source/sorafs_alias_policy.md`.
   - من أجل حزم SDK، قم بنشر القيم المماثلة عبر ألواح التكوين (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` في روابط Rust / NAPI / Python) حتى يتمكن عميل التطبيق من متابعة البوابة.
2. **التشغيل الجاف والتدريج**
   - نشر تغيير التكوين على مجموعة التدريج التي تعكس طوبولوجيا الإنتاج.
   - قم بتنفيذ `cargo xtask sorafs-pin-fixtures` لتأكيد أن التركيبات الكنسيّة المستعارة تم فك ترميزها وخطها طوال الرحلة ذهابًا وإيابًا؛ كل التباعد يعني الانجراف نحو المنبع للتصحيح أولاً.
   - قم بتمرين نقاط النهاية `/v1/sorafs/pin/{digest}` و`/v1/sorafs/aliases` باستخدام مواد صناعية أولية تغطي فترة جديدة ونافذة تحديث ومنتهية الصلاحية ومنتهية الصلاحية. التحقق من رموز HTTP والرؤوس (`Sora-Proof-Status`، `Retry-After`، `Warning`) وأبطال مجموعة JSON ضد دليل التشغيل هذا.
3. **المنشط في الإنتاج**- نشر التكوين الجديد على نافذة التغيير القياسية. عند تطبيق Torii، يمكنك إعادة تشغيل البوابات/الخدمات SDK مرة واحدة لتأكيد السياسة الجديدة في السجلات.
   - المستورد `docs/source/grafana_sorafs_pin_registry.json` في Grafana (أو قم بتحديث لوحات المعلومات الموجودة حاليًا) وقم بتنشيط لوحات التحديث في ذاكرة التخزين المؤقت الاسمية في مساحة العمل NOC.
4. **التحقق بعد النشر**
   - المراقب `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` قلادة 30 دقيقة. يتم ربط الصور الموجودة في القطع `error`/`expired` بنوافذ التحديث؛ يشير التقاطع غير المتوقع إلى أن المشغلين يجب أن يقوموا بفحص إعدادات الأسماء المستعارة وصحة مقدمي الخدمة قبل الاستمرار.
   - تأكد من أن سجلات العميل ستؤدي إلى نفس القرارات السياسية (حزم تطوير البرمجيات (SDK) خالية من الأخطاء عندما تنتهي أو تنتهي صلاحيتها). يشير غياب الإعلانات البسيطة إلى تكوين خاطئ.
5. **الاحتياطي**
   - Si l'émission d'alias prend du retard et que la fenêtre de Refresh séclenché fréquemment, relâcher temporaire la politique en augmentant `refresh_window` et `positive_ttl` dans la config, puis redéployer. Garder `hard_expiry` سليم بحيث يتم رفض الإجراءات المسبقة بشكل دائم.- Revenir à la التكوين السابق في مطعم اللقطة `iroha_config` السابقة إذا استمرت وحدة القياس في عرض حسابات `error` المرتفعة، ثم قم بفتح حادثة لتتبع تأخير إنشاء الأسماء المستعارة.

##Materiaux Liés

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق للتنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عمال التخزين، سجل قواعد اللعبة الكامل هذا.