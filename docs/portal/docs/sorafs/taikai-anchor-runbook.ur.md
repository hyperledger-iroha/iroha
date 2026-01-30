---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-11-21T18:08:23.480735+00:00"
translation_last_reviewed: 2026-01-30
---

# Taikai اینکر آبزرویبیلٹی رن بک

یہ پورٹل کاپی کینونیکل رن بک کو منعکس کرتی ہے جو
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md) میں ہے۔
اسے SN13-C routing-manifest (TRM) اینکرز کی مشق کے دوران استعمال کریں تاکہ
SoraFS/SoraNet آپریٹرز spool artifacts، Prometheus ٹیلیمیٹری اور گورننس evidence
کو پورٹل پری ویو سے باہر نکلے بغیر باہم جوڑ سکیں۔

## اسکوپ اور مالکان

- **پروگرام:** SN13-C — Taikai manifests اور SoraNS anchors۔
- **مالکان:** Media Platform WG، DA Program، Networking TL، Docs/DevRel۔
- **ہدف:** Sev 1/Sev 2 alerts، telemetry validation، اور evidence capture کے لیے
  ڈٹرمنسٹک playbook فراہم کرنا جب Taikai routing manifests aliases کے ذریعے آگے بڑھیں۔

## Quickstart (Sev 1/Sev 2)

1. **spool artifacts پکڑیں** — تازہ ترین
   `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`, اور
   `taikai-lineage-*.json` فائلز
   `config.da_ingest.manifest_store_dir/taikai/` سے کاپی کریں، workers ری اسٹارٹ کرنے سے پہلے۔
2. **`/status` ٹیلیمیٹری ڈمپ کریں** —
   `telemetry.taikai_alias_rotations` array ریکارڈ کریں تاکہ ثابت ہو کہ کون سی
   manifest window فعال ہے:
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **Dashboards اور alerts چیک کریں** —
   `dashboards/grafana/taikai_viewer.json` لوڈ کریں (cluster + stream filters) اور نوٹ
   کریں کہ آیا `dashboards/alerts/taikai_viewer_rules.yml` میں کوئی rule فائر ہوا
   (`TaikaiLiveEdgeDrift`, `TaikaiIngestFailure`, `TaikaiCekRotationLag`, SoraFS PoR health events)۔
4. **Prometheus دیکھیں** — "Metric reference" میں دی گئی queries چلائیں تاکہ ingest
   latency/drift اور alias rotation counters درست ہوں۔ اگر
   `taikai_trm_alias_rotations_total` کئی windows تک رک جائے یا error counters بڑھیں
   تو escalate کریں۔

## Metric reference

| Metric | مقصد |
| --- | --- |
| `taikai_ingest_segment_latency_ms` | CMAF ingest latency histogram فی cluster/stream (ہدف: p95 < 750 ms، p99 < 900 ms). |
| `taikai_ingest_live_edge_drift_ms` | live-edge drift encoder اور anchor workers کے درمیان (p99 > 1.5 s اگر 10 منٹ تک ہو تو page). |
| `taikai_ingest_segment_errors_total{reason}` | error counters وجہ کے لحاظ سے (`decode`, `manifest_mismatch`, `lineage_replay`, ...). کسی بھی اضافہ پر `TaikaiIngestFailure` فائر ہوتا ہے۔ |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` | جب `/v1/da/ingest` نئے TRM کو قبول کرے تو بڑھتا ہے؛ cadence جانچنے کے لیے `rate()` استعمال کریں۔ |
| `/status → telemetry.taikai_alias_rotations[]` | JSON snapshot جس میں `window_start_sequence`, `window_end_sequence`, `manifest_digest_hex`, `rotations_total`, اور timestamps شامل ہوں تاکہ evidence bundles تیار ہوں۔ |
| `taikai_viewer_*` (rebuffer، CEK rotation age، PQ health، alerts) | viewer KPIs تاکہ CEK rotation + PQ circuits صحت مند رہیں۔ |

### PromQL snippets

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## Dashboards اور alerts

- **Grafana viewer board:** `dashboards/grafana/taikai_viewer.json` — p95/p99 latency،
  live-edge drift، segment errors، CEK rotation age، viewer alerts۔
- **Grafana cache board:** `dashboards/grafana/taikai_cache.json` — hot/warm/cold promotions
  اور QoS denials جب alias windows rotate ہوں۔
- **Alertmanager rules:** `dashboards/alerts/taikai_viewer_rules.yml` — drift paging،
  ingest failure warnings، CEK rotation lag، اور SoraFS PoR health penalties/cooldowns۔
  ہر production cluster کے لیے receivers یقینی بنائیں۔

## Evidence bundle checklist

- Spool artifacts (`taikai-anchor-request-*`, `taikai-trm-state-*`,
  `taikai-lineage-*`).
- `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` چلائیں تاکہ pending/delivered envelopes کی signed JSON inventory بنے اور request/SSM/TRM/lineage فائلز drill bundle میں کاپی ہوں۔ default spool path `storage/da_manifests/taikai` ہے جو `torii.toml` سے آتا ہے۔
- `/status` snapshot جو `telemetry.taikai_alias_rotations` کو کور کرے۔
- واقعہ ونڈو کے لیے Prometheus exports (JSON/CSV)۔
- Grafana screenshots جن میں filters نظر آئیں۔
- Alertmanager IDs جو متعلقہ rule fires کو ریفرنس کریں۔
- `docs/examples/taikai_anchor_lineage_packet.md` کا لنک جو canonical evidence packet بیان کرتا ہے۔

## Dashboard mirroring اور drill cadence

SN13-C requirement پوری کرنے کا مطلب ہے کہ Taikai viewer/cache dashboards پورٹل میں
**بھی** دکھیں اور anchor evidence drill متوقع cadence کے ساتھ چلے۔

1. **Portal mirroring.** جب `dashboards/grafana/taikai_viewer.json` یا
   `dashboards/grafana/taikai_cache.json` بدلے، `sorafs/taikai-monitoring-dashboards`
   (یہ پورٹل) میں deltas خلاصہ کریں اور portal PR کی description میں JSON checksums
   نوٹ کریں۔ نئے panels/thresholds نمایاں کریں تاکہ reviewers managed Grafana فولڈر
   کے ساتھ correlation کر سکیں۔
2. **Monthly drill.**
   - drill ہر ماہ کے پہلے منگل کو 15:00 UTC پر چلائیں تاکہ evidence SN13 governance sync
     سے پہلے پہنچ جائے۔
   - spool artifacts، `/status` telemetry اور Grafana screenshots کو
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` میں رکھیں۔
   - اجرا کو `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` سے log کریں۔
3. **Review & publish.** 48 گھنٹوں کے اندر alerts/false positives کو DA Program + NetOps کے
   ساتھ ریویو کریں، drill log میں follow-ups ریکارڈ کریں، اور governance bucket upload کا لنک
   `docs/source/sorafs/runbooks-index.md` سے دیں۔

اگر dashboards یا drills پیچھے رہ جائیں تو SN13-C 🈺 سے باہر نہیں آ سکتا؛ cadence یا evidence
توقعات بدلیں تو اس سیکشن کو اپڈیٹ رکھیں۔

## Helpful commands

```bash
# Alias rotation telemetry کا snapshot ایک artefact ڈائریکٹری میں
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# مخصوص alias/event کے لیے spool entries کی فہرست
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# spool log سے TRM mismatch reasons دیکھیں
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

Taikai anchoring telemetry، dashboards یا governance evidence requirements بدلیں تو اس پورٹل
کاپی کو کینونیکل رن بک کے ساتھ ہم آہنگ رکھیں۔
