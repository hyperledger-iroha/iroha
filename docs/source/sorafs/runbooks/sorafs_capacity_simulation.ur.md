<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/source/sorafs/runbooks/sorafs_capacity_simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a74e1cb5abc86822ff9d24b9ce42a6567d964cbc01ca4c619b49ca6d239101da
source_last_modified: "2025-11-05T18:02:08.787799+00:00"
translation_last_reviewed: 2025-12-28
---

# SoraFS کپیسٹی سمیولیشن رَن بُک

یہ رن بُک وضاحت کرتی ہے کہ SF-2c کپیسٹی مارکیٹ پلیس سمیولیشن ٹول کِٹ کیسے چلائیں اور حاصل شدہ میٹرکس کیسے دیکھیں۔ مقصد یہ ہے کہ `docs/examples/sorafs_capacity_simulation/` کے reproducible fixtures کے ذریعے quota مذاکرات، failover ہینڈلنگ اور slashing remediation کو end‑to‑end ویلیڈیٹ کیا جائے۔ کپیسٹی payloads اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں؛ manifest/CAR پیکجنگ کے لیے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI آرٹی فیکٹس تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

یہ اسکرپٹ `sorafs_manifest_stub capacity` کو چلाकर deterministic Norito payloads، base64 encodings، Torii request bodies اور JSON summaries تیار کرتا ہے برائے:

- quota مذاکرات والے منظرنامے میں شریک تین providers کی declarations۔
- ایک replication order جو staged manifest کو providers میں تقسیم کرتا ہے۔
- telemetry snapshots جو pre‑outage baseline، outage window اور failover recovery کو گرفت میں لیتے ہیں۔
- simulated outage کے بعد slashing درخواست کرنے والا dispute payload۔

آرٹی فیکٹس `./artifacts` میں لکھے جاتے ہیں (یا پہلے آرگومنٹ میں دیا گیا راستہ)۔ انسانی فہم کے لیے `_summary.json` فائلیں دیکھیں۔

## 2. نتائج جمع کریں اور metrics جاری کریں

```bash
./analyze.py --artifacts ./artifacts
```

analysis script تیار کرتا ہے:

- `capacity_simulation_report.json` — مجموعی allocations، failover deltas اور dispute metadata۔
- `capacity_simulation.prom` — Prometheus textfile metrics (`sorafs_simulation_*`) جو node-exporter textfile collector یا standalone Prometheus scrape job کے ذریعے امپورٹ ہو سکتے ہیں۔

`prometheus.yml` scrape configuration کی مثال:

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

textfile collector کو generated `.prom` کی طرف پوائنٹ کریں (node-exporter کے لیے اسے `--collector.textfile.directory` میں کاپی کریں)۔

## 3. Grafana ڈیش بورڈ امپورٹ کریں

1. Grafana میں `dashboards/grafana/sorafs_capacity_simulation.json` امپورٹ کریں۔
2. `Prometheus` datasource input کو اوپر دی گئی scrape configuration سے جوڑیں۔
3. پینلز چیک کریں:
   - **Quota Allocation (GiB)** ہر provider کے committed/assigned balance دکھاتا ہے۔
   - **Failover Trigger** outage metrics لوڈ ہونے پر *Failover Active* ہو جاتا ہے۔
   - **Uptime Drop During Outage** provider `alpha` کے فیصدی نقصان کو دکھاتا ہے۔
   - **Requested Slash Percentage** dispute fixture سے نکلا remediation ratio دکھاتا ہے۔

## 4. متوقع چیکس

- `sorafs_simulation_quota_total_gib{scope="assigned"}` کی قدر 600 رہتی ہے جب تک committed total ≥600 رہے۔
- `sorafs_simulation_failover_triggered` قدر `1` دیتا ہے اور replacement provider metric میں `beta` نمایاں ہوتا ہے۔
- `sorafs_simulation_slash_requested` provider `alpha` کے لیے `0.15` (15% slash) رپورٹ کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں تاکہ تصدیق ہو سکے کہ fixtures CLI schema کے ساتھ اب بھی validate ہوتے ہیں۔
