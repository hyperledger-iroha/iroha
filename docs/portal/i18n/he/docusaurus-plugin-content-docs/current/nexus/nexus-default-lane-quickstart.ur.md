---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-default-lane-quickstart
כותרת: מסלול ברירת מחדל.
sidebar_label: default lane کوئیک اسٹارٹ
description: Nexus کے default lane fallback کو configure اور verify کریں تاکہ Torii اور SDKs public lanes میں lane_id omit کر سکیں۔
---

:::הערה מקור קנוני
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ جب تک localization sweep پورٹل تک نہیں پہنچتی، دونوں کاپیوں کو aligned رکھیں۔
:::

# default lane کوئیک اسٹارٹ (NX-5)

> **הקשר של מפת דרכים:** NX-5 - שילוב ברירת מחדל של נתיב ציבורי. runtime اب `nexus.routing_policy.default_lane` fallback ظاہر کرتا ہے تاکہ Torii REST/gRPC endpoints اور ہر SDK اس وقت `lane_id` محفوظ طریقے سے omit کر سکیں جب ٹریفک canonical public lane سے تعلق رکھتا ہو۔ תרגיל התנהגות לקוח מקצה לקצה.

## דרישות מוקדמות

- `irohad` کا Sora/Nexus build ( `irohad --sora --config ...` چلائیں ).
- configuration repository تک رسائی تاکہ `nexus.*` sections edit کیے جا سکیں۔
- `iroha_cli` جو target cluster سے بات کرنے کے لئے configured ہو۔
- Torii `/status` payload inspect کرنے کے لئے `curl`/`jq` (یا equivalent).

## 1. lane اور dataspace catalog بیان کریں

network پر موجود ہونے والے lanes اور dataspaces کو declare کریں۔ نیچے والا snippet (`defaults/nexus/config.toml` سے) تین public lanes اور matching dataspace aliases register کرتا ہے:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

ہر `index` منفرد اور contiguous ہونا چاہیے۔ ערכים של מזהי מרחב נתונים של 64 סיביות اوپر والے مثالیں وضاحت کے لئے lane indexes کے برابر numeric values ​​استعمال کرتی ہیں۔

## 2. routing defaults اور optional overrides سیٹ کریں

`nexus.routing_policy` سیکشن fallback lane کو control کرتا ہے اور مخصوص instructions یا account prefixes کے لئے routing override کرنے دیتا ہے۔ اگر کوئی rule match نہ کرے تو scheduler ٹرانزیکشن کو configured `default_lane` اور `default_dataspace` پر route کرتا ہے۔ Router logic `crates/iroha_core/src/queue/router.rs` میں ہے اور Torii REST/gRPC surfaces پر پالیسی شفاف انداز میں apply کرتا ہے۔

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. پالیسی کے ساتھ node boot کریں

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

node startup کے دوران derived routing policy لاگ کرتا ہے۔ کوئی بھی validation errors (missing indexes، duplicated aliases، invalid dataspace ids) gossip شروع ہونے سے پہلے سامنے آ جاتے ہیں۔

## 4. lane governance state کنفرم کریں

node online ہونے کے بعد، CLI helper استعمال کریں تاکہ default lane sealed (manifest loaded) اور traffic کے لئے ready ہو۔ Summary view ہر lane کے لئے ایک row پرنٹ کرتا ہے:

```bash
iroha_cli app nexus lane-report --summary
```

פלט לדוגמה:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

اگر default lane `sealed` دکھائے تو external traffic allow کرنے سے پہلے lane governance runbook فالو کریں۔ `--fail-on-sealed` flag CI کے لئے مفید ہے۔

## 5. Torii מצב מטענים בדוק`/status` response routing policy اور فی-lane scheduler snapshot دونوں expose کرتا ہے۔ `curl`/`jq` ברירות מחדל מוגדרות הגדרות ברירות מחדל. 2:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

פלט לדוגמה:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

lane `0` کے لئے live scheduler counters دیکھنے کے لئے:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

یہ کنفرم کرتا ہے کہ TEU snapshot، alias metadata، اور manifest flags configuration کے ساتھ align ہیں۔ یہی payload Grafana panels کے lane-ingest dashboard میں استعمال ہوتا ہے۔

## 6. client defaults exercise کریں

- **Rust/CLI.** `iroha_cli` اور Rust client crate `lane_id` field کو omit کرتے ہیں جب آپ `--lane-id` / `LaneSelector` pass نہیں کرتے۔ اس لئے queue router `default_lane` پر fallback کرتا ہے۔ Explicit `--lane-id`/`--dataspace-id` flags صرف non-default lane کو target کرتے وقت استعمال کریں۔
- **JS/Swift/Android.** تازہ SDK releases `laneId`/`lane_id` کو optional مانتے ہیں اور `/status` میں اعلان کردہ value پر fallback کرتے ہیں۔ Routing policy کو staging اور production میں sync رکھیں تاکہ mobile apps کو emergency reconfigurations نہ کرنی پڑیں۔
- **Pipeline/SSE tests.** transaction event filters `tx_lane_id == <u32>` predicates قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`). `/v2/pipeline/events/transactions` کو اس filter کے ساتھ subscribe کریں تاکہ یہ ثابت ہو کہ explicit lane کے بغیر بھیجی گئی writes fallback lane id کے تحت پہنچتی ہیں۔

## 7. התבוננות או משילות ווים

- `/status` `nexus_lane_governance_sealed_total` اور `nexus_lane_governance_sealed_aliases` بھی publish کرتا ہے تاکہ Alertmanager warn کر سکے جب کوئی lane اپنا manifest کھو دے۔ ان alerts کو devnets میں بھی enabled رکھیں۔
- scheduler telemetry map اور lane governance dashboard (`dashboards/grafana/nexus_lanes.json`) catalog کے alias/slug fields expect کرتے ہیں۔ اگر آپ alias rename کریں تو متعلقہ Kura directories کو relabel کریں تاکہ auditors deterministic paths رکھ سکیں (NX-1 کے تحت track ہوتا ہے)۔
- default lanes کے لئے parliament approvals میں rollback plan شامل ہونا چاہیے۔ manifest hash اور governance evidence کو اس quickstart کے ساتھ اپنے operator runbook میں record کریں تاکہ future rotations مطلوبہ state کا اندازہ نہ لگائیں۔