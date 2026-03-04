---
lang: ur
direction: rtl
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_elastic_lane.md -->

# Elastic Lane Provisioning ٹول کٹ (NX-7)

> **روڈ میپ آئٹم:** NX-7 — Elastic lane provisioning tooling  
> **اسٹیٹس:** tooling مکمل — manifests، catalog snippets، Norito payloads اور smoke tests بناتا ہے،
> اور load-test bundle helper اب slot latency gating + evidence manifests کو جوڑتا ہے تاکہ validator
> load runs بغیر bespoke scripting کے شائع ہو سکیں۔

یہ گائیڈ `scripts/nexus_lane_bootstrap.sh` helper کو بیان کرتی ہے جو lane manifests، lane/dataspace
catalog snippets اور rollout evidence کو خودکار بناتا ہے۔ مقصد یہ ہے کہ نئی Nexus lanes (public یا
private) بنانا آسان ہو، بغیر کئی فائلیں ہاتھ سے ایڈٹ کئے یا catalog geometry دوبارہ اخذ کئے۔

## 1. پیشگی تقاضے

1. lane alias، dataspace، validator set، fault tolerance (`f`) اور settlement policy کے لئے گورننس
   منظوری۔
2. validators کی حتمی فہرست (account IDs) اور protected namespaces کی فہرست۔
3. node configuration repo تک رسائی تاکہ generated snippets شامل کئے جا سکیں۔
4. lane manifests registry کے لئے paths (دیکھیں `nexus.registry.manifest_directory` اور
   `cache_directory`).
5. lane کے لئے telemetry/PagerDuty contacts تاکہ alerts فوری طور پر wire کئے جا سکیں۔

## 2. Lane artefacts بنائیں

Helper کو repo root سے چلائیں:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

اہم flags:

- `--lane-id` نئے entry کے index سے match ہونا چاہئے جو `nexus.lane_catalog` میں ہے۔
- `--dataspace-alias` اور `--dataspace-id/hash` dataspace catalog entry کو کنٹرول کرتے ہیں (اگر
  چھوڑ دیں تو default lane id استعمال ہوتا ہے).
- `--validator` کو repeat کیا جا سکتا ہے یا `--validators-file` سے لیا جا سکتا ہے۔
- `--route-instruction` / `--route-account` routing rules تیار کر کے دیتے ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) runbook contacts ریکارڈ کرتے
  ہیں تاکہ dashboards درست owners دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` manifest میں runtime-upgrade hook شامل کرتے
  ہیں جب lane کو extended operator controls درکار ہوں۔
- `--encode-space-directory` خودکار طور پر `cargo xtask space-directory encode` چلاتا ہے۔ اگر `.to`
  فائل کہیں اور رکھنی ہو تو `--space-directory-out` استعمال کریں۔

اسکرپٹ `--output-dir` میں تین artefacts بناتا ہے (default: موجودہ ڈائریکٹری)، اور encoding فعال
ہونے پر چوتھا artefact بھی بناتا ہے:

1. `<slug>.manifest.json` — lane manifest جس میں validator quorum، protected namespaces اور اختیاری
   runtime-upgrade hook metadata شامل ہے۔
2. `<slug>.catalog.toml` — TOML snippet جس میں `[[nexus.lane_catalog]]`، `[[nexus.dataspace_catalog]]`
   اور مطلوبہ routing rules شامل ہیں۔ `fault_tolerance` کو dataspace entry میں ضرور سیٹ کریں تاکہ
   lane-relay committee (`3f+1`) صحیح سائز ہو۔
3. `<slug>.summary.json` — audit summary جو geometry (slug, segments, metadata) اور rollout steps
   بیان کرتا ہے، اور `cargo xtask space-directory encode` کا درست کمانڈ بھی دیتا ہے
   (`space_directory_encode.command`)۔ onboarding ticket کے ساتھ اس JSON کو evidence کے طور پر لگائیں۔
4. `<slug>.manifest.to` — `--encode-space-directory` کے ساتھ بنتا ہے؛ Torii کے
   `iroha app space-directory manifest publish` flow کے لئے تیار۔

`--dry-run` سے JSON/snippets کی preview بغیر فائل لکھے ہو جاتی ہے، اور `--force` سے existing artefacts
overwrite ہو جاتے ہیں۔

## 3. تبدیلیاں لاگو کریں

1. manifest JSON کو configured `nexus.registry.manifest_directory` میں کاپی کریں (اور cache directory
   میں بھی اگر registry remote bundles کو mirror کرتا ہے). اگر manifests version control میں ہوں
   تو فائل commit کریں۔
2. catalog snippet کو `config/config.toml` (یا `config.d/*.toml`) میں add کریں۔ یقینی بنائیں کہ
   `nexus.lane_count` کم از کم `lane_id + 1` ہو، اور وہ `nexus.routing_policy.rules` اپ ڈیٹ کریں جو
   نئی lane کی طرف اشارہ کریں۔
3. اگر `--encode-space-directory` نہیں چلایا تو encode کریں اور manifest کو Space Directory میں
   publish کریں؛ summary میں دیا گیا کمانڈ `space_directory_encode.command` استعمال کریں۔ یہ
   `.manifest.to` payload بناتا ہے جو Torii کو چاہئے اور audit evidence بھی ریکارڈ کرتا ہے؛
   `iroha app space-directory manifest publish` کے ذریعے submit کریں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور trace output کو rollout
   ticket میں archive کریں۔ یہ ثابت کرتا ہے کہ نئی geometry slug/kura segments سے match کرتی ہے۔
5. manifest/catalog deploy ہونے کے بعد lane کے validators کو restart کریں۔ summary JSON کو ticket میں
   رکھیں تاکہ مستقبل کے audits میں کام آئے۔

## 4. Registry distribution bundle بنائیں

manifest، catalog snippet اور summary تیار ہونے پر validators کیلئے bundle بنائیں۔ نیا bundler
manifests کو اس layout میں کاپی کرتا ہے جو `nexus.registry.manifest_directory` / `cache_directory`
کو چاہئے، governance catalog overlay بناتا ہے تاکہ modules بغیر main config بدلے swap ہو سکیں، اور
اختیاری طور پر bundle archive بھی کرتا ہے:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

نتائج:

1. `manifests/<slug>.manifest.json` — `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` — `nexus.registry.cache_directory` میں رکھیں تاکہ governance
   modules override یا replace ہوں (`--module ...` cached catalog کو override کرتا ہے)۔ یہ NX-2 کے
   لئے pluggable module path ہے: module definition بدلیں، bundler دوبارہ چلائیں، اور cache overlay
   distribute کریں، `config.toml` کو چھیڑے بغیر۔
3. `summary.json` — ہر manifest کے SHA-256 / Blake2b digests اور overlay metadata شامل کرتا ہے۔
4. اختیاری `registry_bundle.tar.*` — Secure Copy / artifact storage کیلئے تیار۔

اگر deployment bundles کو air-gapped hosts پر mirror کرتا ہے تو پورا output directory (یا tarball)
sync کریں۔ online nodes manifest directory کو براہ راست mount کر سکتے ہیں جبکہ offline nodes tarball
extract کر کے manifests + cache overlay کو configured paths میں کاپی کرتے ہیں۔

## 5. Validator smoke tests

Torii restart کے بعد smoke helper چلائیں تاکہ lane `manifest_ready=true` رپورٹ کرے، metrics میں
expected lane count نظر آئے، اور sealed gauge صاف ہو۔ جن lanes کیلئے manifest ضروری ہے انہیں
`manifest_path` غیر خالی ہونا چاہئے — helper evidence نہ ملنے پر فوراً fail کرے گا تاکہ NX-7 change
controls میں signed bundle references شامل ہوں:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Self-signed ماحول کیلئے `--insecure` شامل کریں۔ lane غائب، sealed، یا metrics/telemetry میں انحراف
ہو تو script non-zero exit دیتا ہے۔ `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`
اور `--max-headroom-events` سے height/finality/backlog/headroom telemetry کو operational حدود میں
رکھیں۔ `--max-slot-p95/--max-slot-p99` (اور `--min-slot-samples`) کے ساتھ NX-18 slot-duration SLO
helper میں براہ راست enforce کریں۔ `--allow-missing-lane-metrics` صرف تب استعمال کریں جب staging
clusters نے gauges expose نہ کئے ہوں (production میں defaults برقرار رکھیں).

Helper اب scheduler load-test telemetry بھی enforce کرتا ہے۔ `--min-teu-capacity` سے ہر lane کا
`nexus_scheduler_lane_teu_capacity` ثابت کریں، `--max-teu-slot-commit-ratio` سے slot utilization gate
کریں ( `nexus_scheduler_lane_teu_slot_committed` کو capacity کے ساتھ compare کرتا ہے)، اور
`--max-teu-deferrals` اور `--max-must-serve-truncations` سے deferral/truncation counters صفر رکھیں۔
یہ knobs NX-7 کے "deeper validator load tests" کو repeatable CLI check بناتے ہیں: helper اس وقت fail
ہوتا ہے جب lane PQ/TEU کام defer کرے یا per-slot TEU headroom سے تجاوز کرے، اور CLI per-lane summary
پرنٹ کرتا ہے تاکہ evidence میں CI کے برابر اعداد شامل ہوں۔

Air-gapped validation (یا CI) کیلئے live node کے بجائے captured Torii response replay کریں:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` کے fixtures bootstrap helper کے artefacts کو mirror کرتے ہیں تاکہ نئے manifests
بغیر bespoke scripting کے lint ہو سکیں۔ CI `ci/check_nexus_lane_smoke.sh` اور
`ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) کے ذریعے یہی flow چلاتی
ہے تاکہ NX-7 smoke helper published payload format کے مطابق رہے اور bundle digests/overlays
reproducible ہوں۔

جب lane rename ہو تو `nexus.lane.topology` telemetry events capture کریں (مثلاً
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) اور انہیں smoke helper میں
feed کریں۔ نیا `--telemetry-file/--from-telemetry` flag newline-delimited log قبول کرتا ہے اور
`--require-alias-migration old:new` یہ assert کرتا ہے کہ `alias_migrated` event نے rename ریکارڈ کیا:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` fixture میں canonical rename sample شامل ہے تاکہ CI telemetry parsing
کو live node کے بغیر verify کر سکے۔

## 6. Validator load tests (NX-7 evidence)

Roadmap **NX-7** کے تحت lane operators کو production ready کرنے سے پہلے reproducible validator load run
capture کرنا ہوگا۔ مقصد یہ ہے کہ lane کو اتنا stress کیا جائے کہ slot duration، settlement backlog،
DA quorum، oracle، scheduler headroom اور TEU metrics exercise ہوں، اور پھر نتائج اس طرح archive ہوں
کہ auditors bespoke tooling کے بغیر replay کر سکیں۔ نیا `scripts/nexus_lane_load_test.py` helper smoke
checks، slot-duration gating اور slot bundle manifest کو ایک artefact set میں جوڑتا ہے تاکہ load runs
براہ راست governance tickets میں publish ہو سکیں۔

### 6.1 Workload preparation

1. Run directory بنائیں اور lane کے لئے canonical fixtures capture کریں:

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   Fixtures `fixtures/nexus/lane_commitments/*.json` کی mirror ہیں اور workload generator کو
   deterministic seed دیتی ہیں (seed کو `artifacts/.../README.md` میں ریکارڈ کریں).
2. Run سے پہلے lane baseline کریں:

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   stdout/stderr کو run directory میں رکھیں تاکہ smoke thresholds audit ہو سکیں۔
3. Telemetry log capture کریں جو بعد میں `--telemetry-file` اور `validate_nexus_telemetry_pack.py`
   کے لئے استعمال ہوگا:

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. Lane workload شروع کریں (k6 profile، replay harness، یا federation ingestion tests) اور workload
   seed + slot range نوٹ کریں؛ metadata section 6.3 میں telemetry manifest validator استعمال کرتا ہے۔

5. Load run evidence کو نئے helper سے پیک کریں۔ status/metrics/telemetry payloads، lane aliases اور
   alias migration events فراہم کریں۔ helper `smoke.log`, `slot_summary.json`, slot bundle manifest
   اور `load_test_manifest.json` بناتا ہے تاکہ governance review کیلئے سب کچھ ایک ساتھ ہو:

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   یہ کمانڈ اسی guide کے DA quorum، oracle، settlement buffer، TEU اور slot-duration gates نافذ کرتی
   ہے اور ایک attachable manifest بناتی ہے۔

### 6.2 Instrumented run

Workload کے دوران:

1. Torii status + metrics snapshot کریں:

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. Slot-duration quantiles نکالیں اور summary archive کریں:

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. Lane-governance snapshot کو JSON + Parquet میں export کریں:

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   JSON/Parquet snapshot اب TEU utilization، scheduler trigger levels، RBC chunk/byte counters اور
   transaction graph stats فی lane ریکارڈ کرتا ہے تاکہ rollout evidence backlog اور execution pressure
   دونوں دکھائے۔

4. Load peak پر smoke helper دوبارہ چلائیں (output `smoke_during.log` میں لکھیں) اور workload ختم
   ہونے پر دوبارہ چلائیں (`smoke_after.log`).

### 6.3 Telemetry pack اور governance manifest

Run directory میں telemetry pack (`prometheus.tgz`, OTLP stream, structured logs, harness outputs)
ہونا چاہئے۔ pack validate کریں اور governance کی مطلوبہ metadata stamp کریں:

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

آخر میں captured telemetry log attach کریں اور اگر lane rename ہو تو alias migration evidence لازم
کریں:

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

Governance ticket کیلئے درج ذیل artefacts archive کریں:

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + pack contents (`prometheus.tgz`, `otlp.ndjson`, وغیرہ)
- `nexus.lane.topology.ndjson` (یا relevant telemetry slice)

اب یہ run Space Directory manifests اور governance trackers میں NX-7 کے canonical load test کے طور
پر reference کیا جا سکتا ہے۔

## 7. Telemetry اور governance follow-ups

- lane dashboards (`dashboards/grafana/nexus_lanes.json` اور متعلقہ overlays) کو نئے lane id اور
  metadata سے اپ ڈیٹ کریں۔ generated keys (`contact`, `channel`, `runbook`, وغیرہ) labels بھرنا آسان
  بناتے ہیں۔
- نئی lane کیلئے PagerDuty/Alertmanager rules admission سے پہلے wire کریں۔ `summary.json`
  `docs/source/nexus_operations.md` کی checklist کو mirror کرتا ہے۔
- validator set live ہونے کے بعد Space Directory میں manifest bundle register کریں۔ helper کے بنائے
  ہوئے manifest JSON کو governance runbook کے مطابق sign کر کے استعمال کریں۔
- `docs/source/sora_nexus_operator_onboarding.md` کے مطابق smoke tests (FindNetworkStatus، Torii
  reachability) چلائیں اور evidence کو اوپر والے artefact set سے capture کریں۔

## 8. Dry-run مثال

Artefacts بغیر فائل لکھے preview کرنے کیلئے:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --dry-run
```

یہ کمانڈ JSON summary اور TOML snippet stdout پر پرنٹ کرتی ہے، جس سے planning کے دوران تیز iteration
ممکن ہوتی ہے۔

---

مزید context کیلئے دیکھیں:

- `docs/source/nexus_operations.md` — operational checklist اور telemetry requirements۔
- `docs/source/sora_nexus_operator_onboarding.md` — تفصیلی onboarding flow جو helper کو refer کرتا ہے۔
- `docs/source/nexus_lanes.md` — lanes geometry, slugs, اور tool کے استعمال کردہ storage layout۔

</div>
