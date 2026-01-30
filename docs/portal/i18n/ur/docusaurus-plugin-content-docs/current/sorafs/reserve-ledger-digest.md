---
id: reserve-ledger-digest
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


Reserve+Rent پالیسی (روڈ میپ آئٹم **SFM-6**) اب `sorafs reserve` کے CLI helpers اور
`scripts/telemetry/reserve_ledger_digest.py` مترجم فراہم کرتی ہے تاکہ خزانہ رنز
deterministic rent/reserve transfers جاری کر سکیں۔ یہ صفحہ
`docs/source/sorafs_reserve_rent_plan.md` میں بیان کردہ workflow کی عکاسی کرتا ہے اور
بتاتا ہے کہ نئے transfer feed کو Grafana + Alertmanager میں کیسے جوڑا جائے تاکہ اقتصادی
اور گورننس ریویورز ہر billing cycle کو آڈٹ کر سکیں۔

## اینڈ ٹو اینڈ ورک فلو

1. **Quote + ledger projection**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account ih58... \
     --treasury-account ih58... \
     --reserve-account ih58... \
     --asset-definition xor#sora \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   ledger helper ایک `ledger_projection` بلاک منسلک کرتا ہے (rent due، reserve shortfall،
   top-up delta، underwriting booleans) اور Norito `Transfer` ISIs شامل کرتا ہے جو XOR کو
   treasury اور reserve اکاؤنٹس کے درمیان منتقل کرنے کے لیے درکار ہیں۔

2. **Digest تیار کریں + Prometheus/NDJSON آؤٹ پٹس**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   digest helper micro-XOR totals کو XOR میں normalize کرتا ہے، یہ ریکارڈ کرتا ہے کہ
   projection underwriting پر پورا اترتا ہے یا نہیں، اور transfer feed metrics
   `sorafs_reserve_ledger_transfer_xor` اور `sorafs_reserve_ledger_instruction_total` جاری
   کرتا ہے۔ جب کئی ledgers پروسیس کرنا ہوں (مثلا providers کا batch)، `--ledger`/`--label`
   جوڑے دہرائیں اور helper ایک واحد NDJSON/Prometheus فائل لکھتا ہے جس میں ہر digest ہوتا ہے
   تاکہ dashboards پورا cycle بغیر bespoke glue کے ingest کر سکیں۔ `--out-prom` فائل
   node-exporter textfile collector کے لیے ہے - `.prom` کو exporter کے watched directory میں
   رکھیں یا اسے اس telemetry bucket میں اپلوڈ کریں جسے Reserve dashboard job استعمال کرتا ہے -
   جبکہ `--ndjson-out` انہی payloads کو data pipelines میں feed کرتا ہے۔

3. **Artefacts + evidence شائع کریں**
   - digests کو `artifacts/sorafs_reserve/ledger/<provider>/` میں رکھیں اور اپنے ہفتہ وار
     economic report سے Markdown خلاصے کو لنک کریں۔
   - JSON digest کو rent burn-down کے ساتھ منسلک کریں (تاکہ auditors حساب دوبارہ چلا سکیں)
     اور checksum کو governance evidence packet میں شامل کریں۔
   - اگر digest top-up یا underwriting breach دکھائے تو alert IDs
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) کا حوالہ دیں
     اور نوٹ کریں کہ کون سے transfer ISIs لاگو ہوئے۔

## Metrics → dashboards → alerts

| Source metric | Grafana panel | Alert / policy hook | Notes |
|--------------|---------------|---------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” in `dashboards/grafana/sorafs_capacity_health.json` | weekly treasury digest feed کریں؛ reserve flow میں spikes `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) میں propagate ہوتے ہیں۔ |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (same dashboard) | ledger digest کے ساتھ جوڑیں تاکہ ثابت ہو سکے کہ billed storage XOR transfers سے میل کھاتا ہے۔ |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + status cards in `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` تب فائر ہوتا ہے جب `requires_top_up=1` ہو؛ `SoraFSReserveLedgerUnderwritingBreach` تب فائر ہوتا ہے جب `meets_underwriting=0` ہو۔ |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown” اور coverage cards in `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, اور `SoraFSReserveLedgerTopUpTransferMissing` اس وقت وارن کرتے ہیں جب transfer feed غائب یا zero ہو جبکہ rent/top-up درکار ہو؛ coverage cards انہی حالات میں 0% پر گر جاتے ہیں۔ |

جب rent cycle مکمل ہو جائے تو Prometheus/NDJSON snapshots ریفریش کریں، تصدیق کریں کہ Grafana panels
نیا `label` اٹھا رہے ہیں، اور screenshots + Alertmanager IDs کو rent governance packet کے ساتھ
منسلک کریں۔ اس سے ثابت ہوتا ہے کہ CLI projection، telemetry، اور governance artefacts **اسی**
transfer feed سے نکلتے ہیں اور roadmap کے economics dashboards کو Reserve+Rent automation کے ساتھ
aligned رکھتے ہیں۔ coverage cards کو 100% (یا 1.0) دکھانا چاہیے اور نئی alerts تب clear ہو جانی
چاہیے جب rent اور reserve top-up transfers digest میں موجود ہوں۔
