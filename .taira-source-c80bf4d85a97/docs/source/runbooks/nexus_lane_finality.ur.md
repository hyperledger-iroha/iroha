---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: ur
direction: rtl
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# Nexus لین فائنالٹی اور اوریکل رن بُک

**اسٹیٹس:** Active — NX-18 ڈیش بورڈ/رن بُک ڈیلیوریبل کو پورا کرتا ہے۔  
**سامعین:** Core Consensus WG، SRE/Telemetry، Release Engineering، آن کال لیڈز۔  
**اسکوپ:** سلاٹ دورانیہ، DA quorum، اوریکل اور سیٹلمنٹ بفر کے SLOs کو کور کرتا ہے
جو 1 s فائنالٹی وعدہ نافذ کرتے ہیں۔ `dashboards/grafana/nexus_lanes.json` اور
`scripts/telemetry/` کے ٹیلی میٹری ہیلپرز کے ساتھ استعمال کریں۔

## ڈیش بورڈز

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — “Nexus Lane Finality & Oracles” بورڈ شائع کرتا ہے۔ پینلز:
  - `histogram_quantile()` on `iroha_slot_duration_ms` (p50/p95/p99) اور تازہ ترین sample gauge۔
  - `iroha_da_quorum_ratio` اور `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` سے DA churn نمایاں۔
  - اوریکل سطحیں: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, `iroha_oracle_haircut_basis_points`۔
  - سیٹلمنٹ بفر پینل (`iroha_settlement_buffer_xor`) جو `LaneBlockCommitment` receipts سے لائیو lane ڈیبٹس دکھاتا ہے۔
- **Alert rules** — `ans3.md` کی Slot/DA SLO شقیں استعمال کرتی ہیں۔ Page جب:
  - slot-duration p95 > 1000 ms دو مسلسل 5 m ونڈوز میں،
  - DA quorum ratio < 0.95 یا `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`,
  - oracle staleness > 90 s یا TWAP window ≠ 60 s،
  - settlement buffer < 25 % (soft) / 10 % (hard) جب میٹرک لائیو ہو۔

## میٹرک چیٹ شیٹ

| میٹرک | ہدف / الرٹ | نوٹس |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard), 950 ms warning | ڈیش بورڈ پینل استعمال کریں یا `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) کو chaos رن کے Prometheus export پر چلائیں۔ |
| `iroha_slot_duration_ms_latest` | تازہ ترین slot؛ اگر > 1100 ms ہو تو تحقیق کریں، چاہے quantiles ٹھیک ہوں۔ | انسیڈنٹ میں ویلیو ایکسپورٹ کریں۔ |
| `iroha_da_quorum_ratio` | 30 m رولنگ ونڈو میں ≥ 0.95۔ | block commits کے دوران DA reschedules سے اخذ۔ |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | chaos rehearsals کے باہر 0 رہنا چاہیے۔ | مسلسل اضافہ `missing-availability warning` شمار کریں۔ |

ہر reschedule Torii pipeline میں `kind = "missing-availability warning"` warning بھی ٹرگر کرتا ہے۔ میٹرک اسپائیک کے ساتھ یہ ایونٹس کیپچر کریں تاکہ متاثرہ بلاک ہیڈر، retry کوشش اور requeue کاؤنٹرز واضح ہوں بغیر validator logs کھنگالے۔【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 s۔ 75 s پر الرٹ۔ | 60 s TWAP feeds کی staleness ظاہر کرتا ہے۔ |
| `iroha_oracle_twap_window_seconds` | بالکل 60 s ± 5 s tolerances۔ | اختلاف کا مطلب غلط کنفیگ ہے۔ |
| `iroha_oracle_haircut_basis_points` | lane liquidity tier (0/25/75 bps) سے match۔ | غیر متوقع اضافہ ہو تو escalate کریں۔ |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. 10% سے نیچے XOR‑only نافذ۔ | پینل lane/dataspace کے حساب سے micro‑XOR debits دکھاتا ہے؛ router policy بدلنے سے پہلے export کریں۔ |

## رسپانس پلے بُک

### Slot-duration breach
1. ڈیش بورڈ + `promql` سے تصدیق کریں (p95/p99)۔  
2. `scripts/telemetry/check_slot_duration.py --json-out <path>` آؤٹ پٹ (اور میٹرکس snapshot) کیپچر کریں تاکہ CXO reviewers 1 s گیٹ کی تصدیق کر سکیں۔  
3. RCA inputs دیکھیں: mempool queue depth، DA reschedules، IVM traces۔  
4. انسیڈنٹ فائل کریں، Grafana اسکرین شاٹ شامل کریں، اور regression برقرار رہے تو chaos drill شیڈول کریں۔

### DA quorum degradation
1. `iroha_da_quorum_ratio` اور reschedule counter چیک کریں؛ `missing-availability warning` logs کے ساتھ correlate کریں۔  
2. ratio <0.95 ہو تو failing attesters pin کریں، sampling parameters بڑھائیں، یا XOR‑only mode پر جائیں۔  
3. `scripts/telemetry/check_nexus_audit_outcome.py` کو routed‑trace rehearsals کے دوران چلائیں تاکہ `nexus.audit.outcome` remediation کے بعد بھی پاس ہونا ثابت ہو۔  
4. DA receipt bundles کو انسیڈنٹ ٹکٹ کے ساتھ archive کریں۔

### Oracle staleness / haircut drift
1. پینلز 5–8 سے price, staleness, TWAP window اور haircut چیک کریں۔  
2. staleness >90 s پر: oracle feed restart یا failover کریں، پھر chaos harness دوبارہ چلائیں۔  
3. haircut mismatch پر: liquidity profile config اور حالیہ governance تبدیلیاں دیکھیں؛ treasury کو مطلع کریں اگر swap lines کی ضرورت ہو۔

### Settlement buffer alerts
1. `iroha_settlement_buffer_xor` (اور nightly receipts) سے headroom تصدیق کریں قبل از policy تبدیلی۔  
2. threshold breach ہونے پر:
   - **Soft breach (<25 %)**: treasury شامل کریں، swap lines پر غور کریں، اور الرٹ لاگ کریں۔  
   - **Hard breach (<10 %)**: XOR‑only enforcement، subsidised lanes انکار، `ops/drill-log.md` میں دستاویز کریں۔  
3. repo/reverse‑repo لیورز کے لیے `docs/source/settlement_router.md` دیکھیں۔

## ایویڈنس اور آٹومیشن

- **CI** — `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` اور `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` کو RC acceptance workflow میں جوڑیں تاکہ ہر RC سلاٹ ڈوریشن سمری اور DA/oracle/buffer گیٹ نتائج میٹرکس snapshot کے ساتھ دے۔ یہ ہیلپر پہلے ہی `ci/check_nexus_lane_smoke.sh` سے invoke ہوتا ہے۔  
- **Dashboard parity** — `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` چلائیں تاکہ پبلش بورڈ staging/prod exports سے میچ ہو۔  
- **Trace artefacts** — TRACE rehearsals یا NX-18 chaos drills کے دوران `scripts/telemetry/check_nexus_audit_outcome.py` چلائیں تاکہ تازہ `nexus.audit.outcome` payload آرکائیو ہو (`docs/examples/nexus_audit_outcomes/`)۔ Grafana اسکرین شاٹس کے ساتھ drill log میں شامل کریں۔
- **Slot evidence bundling** — summary JSON بنانے کے بعد `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` چلائیں تاکہ `slot_bundle_manifest.json` دونوں artefacts کے SHA‑256 digests محفوظ کرے۔ RC evidence bundle کے ساتھ ڈائریکٹری ویسے ہی اپ لوڈ کریں۔ ریلیز پائپ لائن یہ خود چلاتی ہے ( `--skip-nexus-lane-smoke` سے اسکیپ) اور `artifacts/nx18/` کو ریلیز آؤٹ پٹ میں کاپی کرتی ہے۔

## مینٹیننس چیک لسٹ

- `dashboards/grafana/nexus_lanes.json` کو ہر اسکیمہ تبدیلی کے بعد Grafana exports کے ساتھ sync رکھیں؛ NX-18 حوالہ کے ساتھ commit messages میں درج کریں۔  
- نئی میٹرکس (مثلاً settlement buffer gauges) یا تھریش ہولڈز آئیں تو رن بُک اپ ڈیٹ کریں۔  
- ہر chaos rehearsal (slot latency، DA jitter، oracle stall، buffer depletion) کو
  `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>` سے ریکارڈ کریں۔

اس رن بُک کی پیروی NX-18 کی مطلوبہ “operator dashboards/runbooks” ایویڈنس فراہم کرتی ہے اور Nexus GA سے پہلے finality SLO کو enforceable رکھتی ہے۔

</div>
