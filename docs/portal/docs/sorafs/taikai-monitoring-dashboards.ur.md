---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6049b1c4fb42bbfbeaa7fa8f3549c5b7beac1a3e8baec45c0c0ce52f0c3baa2e
source_last_modified: "2025-11-14T09:52:13.533271+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Taikai مانیٹرنگ ڈیش بورڈز
description: viewer/cache Grafana boards کا پورٹل خلاصہ جو SN13-C evidence کو سپورٹ کرتے ہیں۔
---

Taikai routing-manifest (TRM) readiness دو Grafana boards اور ان کے companion
alerts پر منحصر ہے۔ یہ صفحہ
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`, اور
`dashboards/alerts/taikai_viewer_rules.yml` کے highlights کو mirror کرتا ہے تاکہ
reviewers repo clone کیے بغیر فالو کر سکیں۔

## Viewer dashboard (`taikai_viewer.json`)

- **Live edge اور latency:** panels p95/p99 latency histograms
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) کو
  cluster/stream کے حساب سے دکھاتے ہیں۔ p99 > 900 ms یا drift > 1.5 s پر نظر رکھیں
  (alert `TaikaiLiveEdgeDrift` trigger ہوتا ہے)۔
- **Segment errors:** `taikai_ingest_segment_errors_total{reason}` کو break out کر کے
  decode failures، lineage replay attempts، یا manifest mismatches ظاہر کرتا ہے۔
  جب یہ panel “warning” band سے اوپر جائے تو SN13-C incidents کے ساتھ screenshots
  attach کریں۔
- **Viewer اور CEK health:** `taikai_viewer_*` metrics سے panels CEK rotation age،
  PQ guard mix، rebuffer counts، اور alert roll-ups ٹریک کرتے ہیں۔ CEK panel اس
  rotation SLA کو enforce کرتا ہے جسے governance نئے aliases approve کرنے سے پہلے
  ریویو کرتی ہے۔
- **Alias telemetry snapshot:** `/status → telemetry.taikai_alias_rotations` table
  board پر موجود ہے تاکہ operators governance evidence attach کرنے سے پہلے
  manifest digests confirm کر سکیں۔

## Cache dashboard (`taikai_cache.json`)

- **Tier pressure:** panels `sorafs_taikai_cache_{hot,warm,cold}_occupancy` اور
  `sorafs_taikai_cache_promotions_total` chart کرتے ہیں۔ ان سے دیکھیں کہ TRM
  rotation کسی specific tier کو overload تو نہیں کر رہی۔
- **QoS denials:** `sorafs_taikai_qos_denied_total` تب surface ہوتا ہے جب cache
  pressure throttling مجبور کرے؛ جب rate صفر سے ہٹے تو drill log annotate کریں۔
- **Egress utilisation:** تصدیق میں مدد کرتا ہے کہ SoraFS exits Taikai viewers کے
  ساتھ رہتے ہیں جب CMAF windows rotate ہوتے ہیں۔

## Alerts اور evidence capture

- Paging rules `dashboards/alerts/taikai_viewer_rules.yml` میں موجود ہیں اور اوپر
  والے panels سے one-to-one map ہوتے ہیں (`TaikaiLiveEdgeDrift`,
  `TaikaiIngestFailure`, `TaikaiCekRotationLag`, proof-health warnings). یقینی بنائیں
  کہ ہر production cluster انہیں Alertmanager سے wire کرے۔
- drills کے دوران لی گئی snapshots/screenshots کو
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` میں spool files اور `/status` JSON کے
  ساتھ اسٹور کریں۔ `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  استعمال کر کے execution کو shared drill log میں append کریں۔
- جب dashboards تبدیل ہوں تو JSON فائل کا SHA-256 digest portal PR description میں
  شامل کریں تاکہ auditors managed Grafana folder کو repo version سے match کر سکیں۔

## Evidence bundle checklist

SN13-C reviews توقع کرتی ہیں کہ ہر drill یا incident Taikai anchor runbook میں درج
وہی artefacts ship کرے۔ bundle کو governance review کے لیے تیار رکھنے کے لیے انہیں
نیچے والے order میں capture کریں:

1. تازہ ترین `taikai-anchor-request-*.json`, `taikai-trm-state-*.json`, اور
   `taikai-lineage-*.json` فائلیں `config.da_ingest.manifest_store_dir/taikai/` سے
   copy کریں۔ یہ spool artefacts ثابت کرتے ہیں کہ کون سا routing manifest (TRM) اور
   lineage window فعال تھا۔ helper
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   spool files copy کرے گا، hashes emit کرے گا، اور optional طور پر summary sign کرے گا۔
2. `/v1/status` output کو `.telemetry.taikai_alias_rotations[]` پر filter کر کے
   spool files کے ساتھ اسٹور کریں۔ reviewers `manifest_digest_hex` اور window bounds
   کو copied spool state کے ساتھ compare کرتے ہیں۔
3. اوپر دی گئی metrics کے لیے Prometheus snapshots export کریں اور viewer/cache dashboards
   کے screenshots لیں جن میں relevant cluster/stream filters نظر آئیں۔ raw JSON/CSV
   اور screenshots کو artefact folder میں ڈالیں۔
4. Alertmanager incident IDs (اگر ہوں) شامل کریں جو
   `dashboards/alerts/taikai_viewer_rules.yml` کی rules کو reference کریں اور نوٹ کریں
   کہ condition clear ہونے پر auto-close ہوا یا نہیں۔

ہر چیز کو `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` کے تحت رکھیں تاکہ drill
audits اور SN13-C governance reviews ایک single archive نکال سکیں۔

## Drill cadence اور logging

- Taikai anchor drill ہر ماہ کے پہلے منگل کو 15:00 UTC پر چلائیں۔ یہ schedule
  SN13 governance sync سے پہلے evidence کو تازہ رکھتا ہے۔
- اوپر والے artefacts capture کرنے کے بعد execution کو shared ledger میں append کریں
  `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor` کے ذریعے۔ یہ helper
  `docs/source/sorafs/runbooks-index.md` کے لیے درکار JSON entry emit کرتا ہے۔
- archived artefacts کو runbook index entry میں link کریں اور کسی بھی failed alerts
  یا dashboard regressions کو 48 گھنٹوں میں Media Platform WG/SRE چینل کے ذریعے
  escalate کریں۔
- drill summary screenshot set (latency, drift, errors, CEK rotation, cache pressure)
  کو spool bundle کے ساتھ رکھیں تاکہ operators بالکل دکھا سکیں کہ rehearsal کے دوران
  dashboards نے کیسے برتاؤ کیا۔

مکمل Sev 1 procedure اور evidence checklist کے لیے
[Taikai Anchor Runbook](./taikai-anchor-runbook.md) دیکھیں۔ یہ صفحہ صرف وہ
dashboard-specific guidance capture کرتا ہے جو SN13-C کو 🈺 سے نکلنے سے پہلے درکار ہے۔
