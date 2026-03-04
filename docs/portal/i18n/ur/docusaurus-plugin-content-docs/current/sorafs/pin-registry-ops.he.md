---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06e5db00023aee18678eea6f1480133fbb500f55df0d2a12d7f412867ced8b65
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: pin-registry-ops
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx دستاویزات ریٹائر نہ ہوں دونوں ورژنز کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ runbook بیان کرتا ہے کہ SoraFS کے Pin Registry اور replication کے سروس لیول ایگریمنٹس (SLA) کی نگرانی اور ٹرائج کیسے کیا جائے۔ میٹرکس `iroha_torii` سے آتی ہیں اور Prometheus کے ذریعے `torii_sorafs_*` namespace میں ایکسپورٹ ہوتی ہیں۔ Torii پس منظر میں registry اسٹیٹ کو ہر 30 سیکنڈ پر سیمپل کرتا ہے، اس لیے ڈیش بورڈز اپ ٹو ڈیٹ رہتے ہیں چاہے کوئی آپریٹر `/v1/sorafs/pin/*` endpoints کو پول نہ کر رہا ہو۔ تیار شدہ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) امپورٹ کریں تاکہ Grafana کا ایک تیار layout ملے جو نیچے کے حصوں سے براہ راست میپ ہوتا ہے۔

## میٹرک حوالہ

| میٹرک | Labels | وضاحت |
| ----- | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | آن چین manifests کا انوینٹری لائف سائیکل اسٹیٹ کے مطابق۔ |
| `torii_sorafs_registry_aliases_total` | — | registry میں ریکارڈ شدہ فعال manifest aliases کی تعداد۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | replication orders کا backlog اسٹیٹس کے لحاظ سے۔ |
| `torii_sorafs_replication_backlog_total` | — | سہولت کے لیے gauge جو `pending` orders کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA اکاؤنٹنگ: `met` وقت پر مکمل ہونے والے orders کو گنتا ہے، `missed` دیر سے مکمل ہونا + expirations جمع کرتا ہے، `pending` زیر التواء orders کو ظاہر کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | completion latency مجموعی طور پر (issuance اور completion کے درمیان epochs)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | pending orders کی slack windows (deadline minus issued epoch)۔ |

تمام gauges ہر snapshot pull پر ری سیٹ ہوتے ہیں، اس لیے ڈیش بورڈز کو `1m` یا اس سے تیز cadence پر sample کرنا چاہیے۔

## Grafana ڈیش بورڈ

ڈیش بورڈ JSON میں سات پینلز ہیں جو آپریٹر ورک فلو کور کرتے ہیں۔ اگر آپ اپنی مرضی کے چارٹس بنانا چاہیں تو نیچے سوالات بطور فوری حوالہ درج ہیں۔

1. **Manifest lifecycle** – `torii_sorafs_registry_manifests_total` (`status` کے مطابق group).
2. **Alias catalogue trend** – `torii_sorafs_registry_aliases_total`.
3. **Order queue by status** – `torii_sorafs_registry_orders_total` (`status` کے مطابق group).
4. **Backlog vs expired orders** – `torii_sorafs_replication_backlog_total` اور `torii_sorafs_registry_orders_total{status="expired"}` کو ملا کر saturation دکھاتا ہے۔
5. **SLA success ratio** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latency vs deadline slack** – `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` اور `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}` کو اوورلے کریں۔ جب absolute slack floor چاہیے ہو تو Grafana transformations سے `min_over_time` views شامل کریں، مثال:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Missed orders (1h rate)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Alert thresholds

- **SLA success < 0.95 for 15 min**
  - Threshold: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - Action: SRE پیج کریں؛ replication backlog triage شروع کریں۔
- **Pending backlog above 10**
  - Threshold: `torii_sorafs_replication_backlog_total > 10` 10 منٹ تک برقرار
  - Action: providers کی دستیابی اور Torii capacity scheduler چیک کریں۔
- **Expired orders > 0**
  - Threshold: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Action: governance manifests دیکھیں تاکہ providers churn کی تصدیق ہو۔
- **Completion p95 > deadline slack avg**
  - Threshold: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Action: تصدیق کریں کہ providers deadlines سے پہلے commit کر رہے ہیں؛ reassignments پر غور کریں۔

### مثال Prometheus قواعد

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

## Triage workflow

1. **وجہ کی شناخت**
   - اگر SLA misses بڑھیں اور backlog کم رہے تو providers کارکردگی پر توجہ دیں (PoR failures، late completions)۔
   - اگر backlog بڑھے اور misses مستحکم ہوں تو admission (`/v1/sorafs/pin/*`) چیک کریں تاکہ manifests جو کونسل کی منظوری کے منتظر ہیں واضح ہوں۔
2. **Providers کی حالت کی توثیق**
   - `iroha app sorafs providers list` چلائیں اور دیکھیں کہ اعلان کردہ صلاحیتیں replication تقاضوں سے میل کھاتی ہیں۔
   - `torii_sorafs_capacity_*` gauges چیک کریں تاکہ provisioned GiB اور PoR success کی تصدیق ہو۔
3. **Replication کی reassign**
   - جب backlog slack (`stat="avg"`) 5 epochs سے نیچے جائے تو `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے orders جاری کریں (manifest/CAR packaging `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - اگر aliases کے پاس فعال manifest bindings نہ ہوں تو governance کو مطلع کریں (`torii_sorafs_registry_aliases_total` میں غیر متوقع کمی)۔
4. **نتیجہ دستاویزی بنائیں**
   - SoraFS operations log میں timestamps اور متاثرہ manifest digests کے ساتھ incident notes درج کریں۔
   - اگر نئے failure modes یا dashboards آئیں تو اس runbook کو اپڈیٹ کریں۔

## Rollout plan

پروڈکشن میں alias cache policy کو فعال یا سخت کرتے وقت یہ مرحلہ وار طریقہ اختیار کریں:

1. **Configuration تیار کریں**
   - `iroha_config` میں `torii.sorafs_alias_cache` (user -> actual) کو متفقہ TTLs اور grace windows کے ساتھ اپڈیٹ کریں: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace`, `governance_grace`۔ defaults `docs/source/sorafs_alias_policy.md` کی پالیسی سے ملتے ہیں۔
   - SDKs کے لیے یہی اقدار ان کی configuration layers میں فراہم کریں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` Rust / NAPI / Python bindings میں) تاکہ client enforcement gateway سے میل کھائے۔
2. **Staging میں dry-run**
   - config تبدیلی کو staging کلسٹر پر deploy کریں جو production topology کی نقل کرے۔
   - `cargo xtask sorafs-pin-fixtures` چلائیں تاکہ canonical alias fixtures اب بھی decode اور round-trip ہوں؛ کوئی mismatch upstream drift دکھاتا ہے جسے پہلے درست کرنا ہوگا۔
   - `/v1/sorafs/pin/{digest}` اور `/v1/sorafs/aliases` endpoints کو synthetic proofs کے ساتھ آزمائیں جو fresh، refresh-window، expired اور hard-expired کیسز کور کریں۔ HTTP status codes، headers (`Sora-Proof-Status`, `Retry-After`, `Warning`) اور JSON body fields کو اس runbook کے مطابق validate کریں۔
3. **Production میں فعال کریں**
   - standard change window میں نئی config رول آؤٹ کریں۔ پہلے Torii پر لاگو کریں، پھر node لاگز میں نئی پالیسی کی تصدیق کے بعد gateways/SDK services کو restart کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana میں امپورٹ کریں (یا موجودہ dashboards اپڈیٹ کریں) اور alias cache refresh panels کو NOC workspace میں pin کریں۔
4. **Post-deployment verification**
   - 30 منٹ تک `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` مانیٹر کریں۔ `error`/`expired` curves میں spikes کو refresh windows کے ساتھ correlate ہونا چاہیے؛ غیر متوقع اضافہ ہو تو operators کو alias proofs اور providers کی صحت چیک کرنی چاہیے۔
   - Client-side logs میں وہی policy decisions نظر آئیں (SDKs stale یا expired proof پر errors دکھائیں گے)۔ client warnings کا نہ آنا غلط configuration کی علامت ہے۔
5. **Fallback**
   - اگر alias issuance پیچھے رہ جائے اور refresh window بار بار ٹرگر ہو تو `refresh_window` اور `positive_ttl` بڑھا کر پالیسی عارضی طور پر نرم کریں، پھر redeploy کریں۔ `hard_expiry` کو برقرار رکھیں تاکہ واقعی stale proofs رد ہوتے رہیں۔
   - اگر telemetry میں `error` counts بلند رہیں تو پچھلا `iroha_config` snapshot بحال کر کے پچھلی configuration پر واپس جائیں، پھر alias generation delays کی تفتیش کے لیے incident کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` — implementation roadmap اور governance context۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — storage worker operations، اس registry playbook کو مکمل کرتا ہے۔
