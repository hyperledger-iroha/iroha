---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Registro de PIN
sidebar_label: Registro de PIN
descripción: SoraFS کے Registro de PIN y replicación SLA میٹرکس کی نگرانی اور ٹرائج۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx دستاویزات ریٹائر نہ ہوں دونوں ورژنز کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ runbook بیان کرتا ہے کہ SoraFS کے Registro de PIN اور replicación کے سروس لیول ایگریمنٹس (SLA) کی نگرانی اور ٹرائج کیسے کیا جائے۔ Nombre `iroha_torii` Nombre del espacio de nombres `torii_sorafs_*` Nombre del espacio de nombres Prometheus ہیں۔ Torii پس منظر میں registro اسٹیٹ کو ہر 30 سیکنڈ پر سیمپل کرتا ہے، اس لیے ڈیش بورڈز اپ Puntos finales `/v2/sorafs/pin/*` کو پول نہ کر رہا ہو۔ تیار شدہ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) امپورٹ کریں تاکہ Grafana کا ایک تیار diseño ملے جو نیچے کے حصوں سے براہ راست میپ ہوتا ہے۔

## میٹرک حوالہ| میٹرک | Etiquetas | وضاحت |
| ----- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | آن چین manifiesta کا انوینٹری لائف سائیکل اسٹیٹ کے مطابق۔ |
| `torii_sorafs_registry_aliases_total` | — | registro میں ریکارڈ شدہ فعال alias de manifiesto کی تعداد۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | pedidos de replicación کا trabajo pendiente اسٹیٹس کے لحاظ سے۔ |
| `torii_sorafs_replication_backlog_total` | — | سہولت کے لیے calibre جو `pending` pedidos کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA اکاؤنٹنگ: `met` وقت پر مکمل ہونے والے pedidos کو گنتا ہے، `missed` دیر سے مکمل ہونا + vencimientos جمع کرتا ہے، `pending` زیر التواء pedidos کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | latencia de finalización مجموعی طور پر (emisión اور finalización کے درمیان épocas)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Órdenes pendientes کی ventanas de holgura (fecha límite menos época de emisión) ۔ |

تمام indicadores ہر extracción de instantáneas پر ری سیٹ ہوتے ہیں، اس لیے ڈیش بورڈز کو `1m` یا اس سے تیز cadencia پر muestra کرنا چاہیے۔

## Grafana ڈیش بورڈڈیش بورڈ JSON میں سات پینلز ہیں جو آپریٹر ورک فلو کور کرتے ہیں۔ اگر آپ اپنی مرضی کے چارٹس بنانا چاہیں تو نیچے سوالات بطور فوری حوالہ درج ہیں۔

1. **Ciclo de vida del manifiesto** – `torii_sorafs_registry_manifests_total` (`status` کے مطابق grupo).
2. **Tendencia del catálogo de alias** – `torii_sorafs_registry_aliases_total`.
3. **Cola de pedidos por estado** – `torii_sorafs_registry_orders_total` (`status` کے مطابق grupo).
4. **Pedidos pendientes versus pedidos vencidos** – `torii_sorafs_replication_backlog_total` اور `torii_sorafs_registry_orders_total{status="expired"}` کو ملا کر saturación دکھاتا ہے۔
5. **Proporción de éxito de SLA** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia frente a retraso en la fecha límite** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` اور `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` کو اوورلے کریں۔ جب piso absolutamente flojo چاہیے ہو تو Grafana transformaciones سے `min_over_time` vistas شامل کریں، مثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Pedidos perdidos (tarifa de 1h)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Umbrales de alerta

- **Éxito del SLA  0**
  - Umbral: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acción: la gobernanza manifiesta دیکھیں تاکہ proveedores deserción کی تصدیق ہو۔
- **Finalización p95 > promedio de retraso en la fecha límite**
  - Umbral: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acción: تصدیق کریں کہ fechas límite de los proveedores سے پہلے commit کر رہے ہیں؛ reasignaciones پر غور کریں۔

### مثال Prometheus قواعد```yaml
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

## Flujo de trabajo de clasificación

1. **وجہ کی شناخت**
   - اگر SLA pierde بڑھیں اور backlog کم رہے تو proveedores کارکردگی پر توجہ دیں (fallos de PoR, finalizaciones tardías)۔
   - اگر atraso بڑھے اور pierde مستحکم ہوں تو admisión (`/v2/sorafs/pin/*`) چیک کریں تاکہ manifiesta جو کونسل کی منظوری کے منتظر ہیں واضح ہوں۔
2. **Proveedores کی حالت کی توثیق**
   - `iroha app sorafs providers list` چلائیں اور دیکھیں کہ اعلان کردہ صلاحیتیں replicación تقاضوں سے میل کھاتی ہیں۔
   - `torii_sorafs_capacity_*` medidores چیک کریں تاکہ aprovisionado GiB اور PoR éxito کی تصدیق ہو۔
3. **Replicación y reasignación**
   - جب backlog slack (`stat="avg"`) 5 épocas سے نیچے جائے تو `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے pedidos جاری کریں (manifiesto/empaque de CAR `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - اگر alias کے پاس فعال enlaces de manifiesto نہ ہوں تو gobernanza کو مطلع کریں (`torii_sorafs_registry_aliases_total` میں غیر متوقع کمی)۔
4. **نتیجہ دستاویزی بنائیں**
   - Registro de operaciones SoraFS Marcas de tiempo اور متاثرہ resúmenes de manifiesto کے ساتھ notas de incidentes درج کریں۔
   - اگر نئے modos de falla یا paneles آئیں تو اس runbook کو اپڈیٹ کریں۔

## Plan de implementación

پروڈکشن میں alias política de caché کو فعال یا سخت کرتے وقت یہ مرحلہ وار طریقہ اختیار کریں:1. **Configuración تیار کریں**
   - `iroha_config` میں `torii.sorafs_alias_cache` (usuario -> real) کو متفقہ TTLs اور ventanas de gracia کے ساتھ اپڈیٹ کریں: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`۔ valores predeterminados `docs/source/sorafs_alias_policy.md` کی پالیسی سے ملتے ہیں۔
   - SDKs کے لیے یہی اقدار ان کی capas de configuración میں فراہم کریں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` Enlaces Rust / NAPI / Python میں) تاکہ puerta de enlace de cumplimiento del cliente سے میل کھائے۔
2. **Ejecución en seco de la puesta en escena**
   - configuración de puesta en escena, implementación, topología de producción, configuración
   - `cargo xtask sorafs-pin-fixtures` چلائیں تاکہ dispositivos de alias canónicos اب بھی decodificar اور ida y vuelta ہوں؛ کوئی falta de coincidencia deriva ascendente دکھاتا ہے جسے پہلے درست کرنا ہوگا۔
   - `/v2/sorafs/pin/{digest}` اور `/v2/sorafs/aliases` puntos finales کو pruebas sintéticas کے ساتھ آزمائیں جو fresco, actualizar ventana, caducado اور duramente caducado کیسز کور کریں۔ Códigos de estado HTTP, encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y campos del cuerpo JSON, runbook, validar
3. **Producción میں فعال کریں**
   - ventana de cambio estándar میں نئی config رول آؤٹ کریں۔ پہلے Torii پر لاگو کریں، پھر node لاگز میں نئی پالیسی کی تصدیق کے بعد gateways/SDK Services کو reiniciar کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana میں امپورٹ کریں (یا موجودہ paneles de control اپڈیٹ کریں) اور alias caché paneles de actualización کو NOC espacio de trabajo میں pin کریں۔4. **Verificación posterior a la implementación**
   - 30 meses `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` مانیٹر کریں۔ `error`/`expired` curvas میں picos کو actualizar ventanas کے ساتھ correlacionar ہونا چاہیے؛ غیر متوقع اضافہ ہو تو operadores کو pruebas de alias اور proveedores کی صحت چیک کرنی چاہیے۔
   - Registros del lado del cliente میں وہی decisiones de políticas نظر آئیں (SDK obsoletos یا prueba caducada پر errores دکھائیں گے)۔ advertencias del cliente کا نہ آنا غلط configuración کی علامت ہے۔
5. **Retroceso**
   - Emisión de alias پیچھے رہ جائے اور ventana de actualización بار بار ٹرگر ہو تو `refresh_window` اور `positive_ttl` بڑھا کر پالیسی عارضی طور پر نرم کریں، پھر redistribuir کریں۔ `hard_expiry` کو برقرار رکھیں تاکہ واقعی pruebas obsoletas رد ہوتے رہیں۔
   - Telemetría میں `error` cuenta بلند رہیں تو پچھلا `iroha_config` instantánea بحال کر کے پچھلی configuración پر واپس جائیں، پھر retrasos en la generación de alias کی تفتیش کے لیے incidente کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` — hoja de ruta de implementación y contexto de gobernanza۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — operaciones de trabajo de almacenamiento, manual de registro کو مکمل کرتا ہے۔