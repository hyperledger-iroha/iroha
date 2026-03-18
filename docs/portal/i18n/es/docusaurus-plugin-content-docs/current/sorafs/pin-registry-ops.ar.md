---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: عمليات Registro de PIN
sidebar_label: Registro de PIN
descripción: Registro de PIN de مراقبة وفرز SoraFS y للتكرار SLA.
---

:::nota المصدر المعتمد
Nombre `docs/source/sorafs/runbooks/pin_registry_ops.md`. حافظ على النسختين متزامنتين حتى تقاعد وثائق Sphinx القديمة.
:::

## نظرة عامة

Utilice el registro de PIN y el registro de PIN SoraFS y establezca los requisitos de seguridad (SLA). Utilice el `iroha_torii` y el Prometheus y el `torii_sorafs_*`. يقوم Torii بأخذ عينات من حالة registro كل 30 ثانية في الخلفية, لذا تبقى لوحات المعلومات محدثة حتى عند عدم قيام المشغلين باستدعاء نقاط النهاية `/v1/sorafs/pin/*`. استورد لوحة التحكم المنقحة (`docs/source/grafana_sorafs_pin_registry.json`) للحصول على تخطيط Grafana جاهز الاستخدام يطابق الأقسام أدناه مباشرة.

## مرجع المقاييس| المقياس | Etiquetas | الوصف |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | مخزون manifiesta على السلسلة بحسب حالة دورة الحياة. |
| `torii_sorafs_registry_aliases_total` | — | Hay alias de manifiesto del registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | تراكم أوامر التكرار مقسما حسب الحالة. |
| `torii_sorafs_replication_backlog_total` | — | Calibre مرجعي يعكس أوامر `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA: `met` Haga clic en el enlace del enlace `missed` + Haga clic en el enlace `pending` يعكس الأوامر المعلقة. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | كمون الإكمال المجمع (عدد الايبوكات بين الاصدار والاكتمال). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | نوافذ الهامش للاوامر المعلقة (المهلة ناقص ايبوك الاصدار). |

Los medidores están disponibles en una instantánea, en una instantánea, en una pantalla `1m` y en otra.

## لوحة Grafana

تحتوي لوحة التحكم JSON على سبعة مخططات تغطي مسارات عمل المشغلين. يتم سرد الاستعلامات ادناه كمرجع سريع اذا كنت تفضل بناء مخططات مخصصة.1. **دورة حياة manifiestos** – `torii_sorafs_registry_manifests_total` (مجموعة حسب `status`).
2. **اتجاه كتالوج alias** – `torii_sorafs_registry_aliases_total`.
3. **طابور الاوامر حسب الحالة** – `torii_sorafs_registry_orders_total` (مجموعة حسب `status`).
4. **Atrasos مقابل الاوامر المنتهية** – يجمع `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` لاظهار التشبع.
5. **نسبة نجاح SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **الكمون مقابل هامش المهلة** – قم بدمج `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` مع `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Grafana `min_over_time` `min_over_time` `min_over_time` `min_over_time` `min_over_time` المثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **الاوامر الفائتة (duración 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## عتبات التنبيه

- **نجاح SLA  0**
  - Idioma: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - الاجراء: افحص manifiesta la pérdida de proveedores.
- **p95 للاكتمال > متوسط هامش المهلة**
  - Idioma: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - الاجراء: تحقق ان proveedores يلتزمون قبل المهلة؛ فكر في اعادة الاسناد.

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

## سير عمل الفرز1. **تحديد السبب**
   - اذا زادت اخفاقات SLA بينما بقي التراكم منخفضا، ركز على اداء proveedores (فشل PoR، الاكتمال المتاخر).
   - اذا زاد التراكم مع اخفاقات مستقرة، افحص القبول (`/v1/sorafs/pin/*`) لتاكيد manifiesta التي تنتظر موافقة المجلس.
2. **التحقق من حالة proveedores**
   - شغّل `iroha app sorafs providers list` وتاكد ان القدرات المعلن عنها تطابق متطلبات التكرار.
   - Medidores de calibre `torii_sorafs_capacity_*` para GiB y PoR.
3. **اعادة اسناد التكرار**
   - اصدر اوامر جديدة عبر `sorafs_manifest_stub capacity replication-order` عندما ينخفض هامش التراكم (`stat="avg"`) عن 5 ايبوكات (تغليف manifest/CAR يستخدم `iroha app sorafs toolkit pack`).
   - اخطر الحوكمة اذا كانت alias تفتقر لربط manifiesto نشط (انخفاض غير متوقع في `torii_sorafs_registry_aliases_total`).
4. **توثيق النتيجة**
   - سجل ملاحظات الحادث في سجل عمليات SoraFS مع الطوابع الزمنية وdigests المتاثرة.
   - حدّث هذا الدليل عند ظهور اوضاع فشل جديدة او لوحات جديدة.

## خطة الاطلاق

اتبع هذا الاجراء المرحلي عند تفعيل او تشديد سياسة cache للـ alias في الانتاج:1. **تحضير الاعدادات**
   - حدّث `torii.sorafs_alias_cache` في `iroha_config` (usuario -> real) باستخدام TTLs y السماح المتفق عليها: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`. القيم الافتراضية تطابق السياسة في `docs/source/sorafs_alias_policy.md`.
   - Para instalar SDK y enlaces de Rust / NAPI / Python (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` con enlaces Rust / NAPI / Python) están disponibles.
2. **Ejecución en seco de la puesta en escena**
   - انشر تغيير الاعدادات على عنقود puesta en escena يعكس طوبولوجيا الانتاج.
   - `cargo xtask sorafs-pin-fixtures` incluye accesorios y alias de ida y vuelta. اي عدم تطابق يعني drift في المنبع يجب معالجته اولا.
   - Los puntos finales `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` contienen ventanas nuevas y de actualización, caducadas y caducadas. Utilice HTTP y encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y archivos JSON.
3. **التفعيل في الانتاج**
   - اطرح الاعدادات الجديدة في نافذة التغيير القياسية. Utilice Torii para configurar las puertas de enlace/SDK del SDK y configurarlos para su instalación.
   - استورد `docs/source/grafana_sorafs_pin_registry.json` الى Grafana (او حدّث اللوحات الموجودة) وثبت لوحات تحديث caché للـ alias في مساحة عمل NOC.
4. **التحقق بعد النشر**- Las conexiones `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` duran 30 días. يجب ان ترتبط القمم في منحنيات `error`/`expired` بنوافذ التحديث؛ النمو غير المتوقع يعني ان على المشغلين فحص ادلة alias وصحة proveedores قبل المتابعة.
   - تاكد ان سجلات العميل تعرض نفس قرارات السياسة (los SDK que están obsoletos o caducados). غياب تحذيرات العميل يشير الى خطا في الاعداد.
5. **Retroceso**
   - Nombre del alias y nombre del nombre de usuario `refresh_window` y `positive_ttl` الاعداد ثم اعد النشر. ابق `hard_expiry` كما هو حتى تظل الادلة القديمة فعلا مرفوضة.
   - عد الى الاعداد السابق باستعادة snapshot السابق من `iroha_config` اذا استمرت التليمتري في اظهار اعداد `error` مرتفعة، ثم افتح حادثة لتتبع تاخير توليد alias.

## مواد ذات صلة

- `docs/source/sorafs/pin_registry_plan.md` — خارطة طريق التنفيذ وسياق الحوكمة.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — عمليات عامل التخزين، تكمل هذا الدليل.