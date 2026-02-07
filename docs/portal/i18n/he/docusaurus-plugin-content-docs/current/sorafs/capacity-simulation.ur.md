---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: סימולציית קיבולת
title: SoraFS کپیسٹی سمیولیشن رَن بُک
sidebar_label: کپیسٹی سمیولیشن رَن بُک
תיאור: מתקנים ניתנים לשחזור, יצוא Prometheus, לוחות מחוונים של Grafana עם SF-2c מכשירי טלפון ניידים کٹ چلانا۔
---

:::note ماخذِ مستند
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی آئینہ دار ہے۔ جب تک پرانا Sphinx دستاویزی مجموعہ مکمل طور پر منتقل نہیں ہو جاتا دونوں نقول کو ہم آہنگ رکھیں۔
:::

یہ رن بُک وضاحت کرتی ہے کہ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن کِٹ کیسے چلائیں اور حاصل شدہ میٹرکس کیسے دیکھیں۔ یہ `docs/examples/sorafs_capacity_simulation/` میں موجود deterministic fixtures کے ذریعے quota مذاکرات، failover ہینڈلنگ اور slashing remediation کو end‑to‑end ویلیڈیٹ کرتی ہے۔ کپیسٹی payloads اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں؛ manifest/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI آرٹی فیکٹس تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh`، `sorafs_manifest_stub capacity` کو لپیٹ کر Norito payloads، base64 blobs، Torii request bodies اور JSON summaries تیار کرتا ہے برائے:

- quota مذاکرات والے منظرنامے میں شریک تین providers کی declarations۔
- ایک replication order جو staged manifest کو providers میں تقسیم کرتا ہے۔
- pre‑outage baseline، outage interval، اور failover recovery کے telemetry snapshots۔
- simulated outage کے بعد slashing درخواست کرنے والا dispute payload۔

تمام آرٹی فیکٹس `./artifacts` کے تحت جمع ہوتے ہیں (پہلے آرگومنٹ میں مختلف ڈائریکٹری دے کر اووررائیڈ کر سکتے ہیں)۔ انسانی سمجھ کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. מדדים של ערכים גיאוגרפיים

```bash
./analyze.py --artifacts ./artifacts
```

اینالائزر تیار کرتا ہے:

- `capacity_simulation_report.json` - הקצאות מידע, דלתות תקלות, או מטא נתונים של מחלוקות.
- `capacity_simulation.prom` - Prometheus מדדי קבצי טקסט (`sorafs_simulation_*`) או אספן קבצי טקסט של ייצואן צומתים - עבודת גרידה עצמאית.

Prometheus תצורת גרידה דף:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

textfile collector کو `capacity_simulation.prom` کی طرف پوائنٹ کریں (node-exporter استعمال کریں تو اسے `--collector.textfile.directory` والی ڈائریکٹری میں کاپی کریں)۔

## 3. Grafana

1. Grafana میں `dashboards/grafana/sorafs_capacity_simulation.json` امپورٹ کریں۔
2. `Prometheus` datasource ویری ایبل کو اوپر دیے گئے scrape target سے جوڑیں۔
3. תכניות:
   - **Quota Allocation (GiB)** ہر provider کے committed/assigned balances دکھاتا ہے۔
   - **Failover Trigger** outage metrics آنے پر *Failover Active* ہو جاتا ہے۔
   - **Uptime Drop During Outage** provider `alpha` کے لیے فیصدی نقصان دکھاتا ہے۔
   - **Requested Slash Percentage** dispute fixture سے نکلا remediation ratio دکھاتا ہے۔

## 4. متوقع چیکس

- `sorafs_simulation_quota_total_gib{scope="assigned"}` کی قدر `600` رہتی ہے جب تک committed total >=600 رہے۔
- `sorafs_simulation_failover_triggered` قدر `1` دیتا ہے اور replacement provider metric میں `beta` نمایاں ہوتا ہے۔
- `sorafs_simulation_slash_requested` provider `alpha` کے لیے `0.15` (15% slash) رپورٹ کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں تاکہ تصدیق ہو سکے کہ fixtures اب بھی CLI schema کے مطابق ہیں۔