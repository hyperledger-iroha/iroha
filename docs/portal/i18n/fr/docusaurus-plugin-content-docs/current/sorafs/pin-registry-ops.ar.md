---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pin-registry-ops
titre : Registre des épingles de عمليات
sidebar_label : Registre des broches de votre choix
description: مراقبة وفرز Pin Registry في SoraFS ومقاييس SLA للتكرار.
---

:::note المصدر المعتمد
Voir `docs/source/sorafs/runbooks/pin_registry_ops.md`. حافظ على النسختين متزامنتين حتى تقاعد وثئق Sphinx القديمة.
:::

## نظرة عامة

Il s'agit d'un registre PIN avec SoraFS et d'un registre de broches (SLA). Utilisez `iroha_torii` et Prometheus pour `torii_sorafs_*`. Il s'agit d'un Torii mis en place pour le registre depuis 30 ans dans le cadre d'une procédure de registre. حتى عند عدم قيام المشغلين باستدعاء نقاط النهاية `/v1/sorafs/pin/*`. استورد لوحة التحكم المنقحة (`docs/source/grafana_sorafs_pin_registry.json`) للحصول على تخطيط Grafana جاهز الاستخدام يطابق الأقسام أدناه مباشرة.

## مرجع المقاييس

| المقياس | Étiquettes | الوصف |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | مخزون manifeste على السلسلة بحسب حالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | Il existe des alias dans le manifeste du registre. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | تراكم أوامر التكرار مقسما حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | Jauge مرجعي يعكس أوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | محاسبة SLA: `met` تحسب الأوامر المكتملة ضمن المهلة، `missed` يجمع الإكمال المتاخر + الانتهاء، `pending` يعكس الأوامر المعلقة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | كمون الإكمال المجمع (عدد الايبوكات بين الاصدار والاكتمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | نوافذ الهامش للاوامر المعلقة (المهلة ناقص ايبوك الاصدار). |

Les jauges sont destinées à la capture d'instantanés, ainsi qu'aux `1m` et اسرع.

## لوحة Grafana

Si vous utilisez JSON, vous avez également besoin de plus de détails. يتم سرد الاستعلامات ادناه كمرجع سريع اذا كنت تفضل بناء مخططات مخصصة.

1. ** دورة حياة manifeste ** – `torii_sorafs_registry_manifests_total` (مجموعة حسب `status`).
2. **اتجاه كتالوج alias** – `torii_sorafs_registry_aliases_total`.
3. **طابور الاوامر حسب الحالة** – `torii_sorafs_registry_orders_total` (مجموعة حسب `status`).
4. **Backlog مقابل الاوامر المنتهية** – يجمع `torii_sorafs_replication_backlog_total` et `torii_sorafs_registry_orders_total{status="expired"}` لاظهار التشبع.
5. **نسبة نجاح SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **الكمون مقابل هامش المهلة** – قم بدمج `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` ou `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. استخدم تحويلات Grafana pour `min_over_time` عندما تحتاج الحد الادنى المطلق للهامش، على سبيل المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **الاوامر الفائتة (معدل 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه- **نجاح SLA < 0,95 pour 15 dollars**
  - Nom : `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Titre : استدعاء SRE؛ ابدأ فرز تراكم التكرار.
- **تراكم معلق اعلى من 10**
  - Nom : `torii_sorafs_replication_backlog_total > 10` pour 10 jours
  - Nom : les fournisseurs de services sont considérés comme Torii.
- **الاوامر المنتهية > 0**
  - Nom : `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Fonctions : les manifestes des fournisseurs de désabonnement des fournisseurs.
- **p95 للاكتمال > متوسط هامش المهلة**
  - Nom : `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الاجراء: تحقق ان fournisseurs يلتزمون قبل المهلة؛ فكر في اعادة الاسناد.

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
   - Les fournisseurs SLA sont également des fournisseurs de services (PoR, الاكتمال المتاخر).
   - اذا زاد التراكم مع اخفاقات مستقرة، افحص القبول (`/v1/sorafs/pin/*`) لتاكيد manifeste le التي تنتظر موافقة المجلس.
2. ** Fournisseurs de services **
   - شغّل `iroha app sorafs providers list` وتاكد ان القدرات المعلن عنها تطابق متطلبات التكرار.
   - Jauges `torii_sorafs_capacity_*` pour GiB et PoR.
3. **اعادة اسناد التكرار**
   - اصدر اوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عندما ينخفض هامش التراكم (`stat="avg"`) à 5 ايبوكات (تغليف manifest/CAR (`iroha app sorafs toolkit pack`).
   - اخطر الحوكمة اذا كانت alias تفتقر لربط manifest نشط (انخفاض غير متوقع في `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - سجل ملاحظات الحادث في سجل عمليات SoraFS مع الطوابع الزمنية وdigests المتاثرة.
   - حدّث هذا الدليل عند ظهور اوضاع فشل جديدة او لوحات جديدة.

## خطة الاطلاق

Vous pouvez également utiliser le cache pour l'alias de l'alias :1. **تحضير الاعدادات**
   - حدّث `torii.sorafs_alias_cache` في `iroha_config` (user -> actual) باستخدام TTLs ونوافذ السماح المتفق عليها : `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. Il s'agit d'un numéro de téléphone `docs/source/sorafs_alias_policy.md`.
   - Utilisez les SDK et les liens vers les liaisons Rust / NAPI / Python (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` pour les liaisons Rust / NAPI / Python). البوابة.
2. **Exécution à sec et mise en scène**
   - انشر تغيير الاعدادات على عنقود mise en scène يعكس طوبولوجيا الانتاج.
   - شغّل `cargo xtask sorafs-pin-fixtures` لتاكيد ان luminaires الكنسية للـ alias ما زالت تفك التشفير وتقوم بعملية aller-retour؛ اي عدم تطابق يعني drift في المنبع يجب معالجته اولا.
   - Les points de terminaison `/v1/sorafs/pin/{digest}` et `/v1/sorafs/aliases` ont des points de terminaison frais, une fenêtre d'actualisation, une date d'expiration et une date d'expiration matérielle. Vous pouvez utiliser les en-têtes HTTP et (`Sora-Proof-Status`, `Retry-After`, `Warning`) et JSON pour les applications.
3. **التفعيل في الانتاج**
   - اطرح الاعدادات الجديدة في نافذة التغيير القياسية. Utilisez Torii pour créer des passerelles/SDK pour les utilisateurs les plus exigeants. السجلات.
   - Utilisez `docs/source/grafana_sorafs_pin_registry.json` pour Grafana (et utilisez le cache pour l'alias du NOC.
4. **التحقق بعد النشر**
   - Prix `torii_sorafs_alias_cache_refresh_total` et `torii_sorafs_alias_cache_age_seconds` pour 30 dollars. يجب ان ترتبط القمم في منحنيات `error`/`expired` بنوافذ التحديث؛ Il s'agit d'un fournisseur d'alias et d'un fournisseur d'alias.
   - Les SDK sont devenus obsolètes et ont expiré. غياب تحذيرات العميل يشير الى خطا في الاعداد.
5. **Retour**
   - اذا تاخر اصدار alias وتكرر تجاوز نافذة التحديث، خفف السياسة مؤقتا بزيادة `refresh_window` et `positive_ttl` في الاعداد ثم اعد النشر. ابق `hard_expiry` كما هو حتى تظل الادلة القديمة فعلا مرفوضة.
   - Vous pouvez créer un instantané avec `iroha_config` pour obtenir un instantané `error`. مرتفعة، ثم افتح حادثة لتتبع تاخير توليد alias.

## مواد ذات صلة

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين، تكمل هذا الدليل.