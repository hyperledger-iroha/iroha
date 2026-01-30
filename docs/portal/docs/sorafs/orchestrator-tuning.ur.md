---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 364e6c85da0b765404e4ec6dd245a8fc024ec7d1801b5b0e002b42f5f4e3a989
source_last_modified: "2025-11-18T04:20:50.893014+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: orchestrator-tuning
title: آرکسٹریٹر رول آؤٹ اور ٹیوننگ
sidebar_label: آرکسٹریٹر ٹیوننگ
description: ملٹی سورس آرکسٹریٹر کو GA تک لے جانے کے لیے عملی ڈیفالٹس، ٹیوننگ رہنمائی، اور آڈٹ چیک پوائنٹس۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator_tuning.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن مکمل طور پر ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# آرکسٹریٹر رول آؤٹ اور ٹیوننگ گائیڈ

یہ گائیڈ [کنفیگریشن ریفرنس](orchestrator-config.md) اور
[ملٹی سورس رول آؤٹ رن بک](multi-source-rollout.md) پر مبنی ہے۔ یہ وضاحت کرتی ہے
کہ ہر رول آؤٹ مرحلے میں آرکسٹریٹر کو کیسے ٹیون کیا جائے، scoreboard کے
آرٹیفیکٹس کو کیسے سمجھا جائے، اور ٹریفک بڑھانے سے پہلے کون سے ٹیلیمیٹری سگنلز
لازمی ہیں۔ ان سفارشات کو CLI، SDKs اور آٹومیشن میں یکساں طور پر نافذ کریں تاکہ
ہر نوڈ ایک ہی ڈٹرمنسٹک fetch پالیسی پر عمل کرے۔

## 1. بنیادی پیرامیٹر سیٹس

ایک مشترکہ کنفیگریشن ٹیمپلیٹ سے آغاز کریں اور رول آؤٹ کے ساتھ چند منتخب knobs
کو ایڈجسٹ کریں۔ نیچے جدول عام مراحل کے لیے تجویز کردہ اقدار دکھاتا ہے؛
جو اقدار درج نہیں، وہ `OrchestratorConfig::default()` اور `FetchOptions::default()`
کے ڈیفالٹس پر واپس جاتی ہیں۔

| مرحلہ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | نوٹس |
|-------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|-------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | سخت latency cap اور چھوٹی grace window شور والی ٹیلیمیٹری کو فوراً ظاہر کرتی ہے۔ retries کم رکھیں تاکہ غلط manifests جلد سامنے آئیں۔ |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | پروڈکشن ڈیفالٹس کو منعکس کرتا ہے اور exploratory peers کے لیے جگہ چھوڑتا ہے۔ |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | ڈیفالٹس کے مطابق؛ `telemetry_region` سیٹ کریں تاکہ ڈیش بورڈز کینری ٹریفک الگ دکھا سکیں۔ |
| **General Availability (GA)** | `None` (تمام eligible استعمال کریں) | `4` | `4` | `5000` | `900` | عارضی خرابیوں کو جذب کرنے کے لیے retry اور failure thresholds بڑھائیں جبکہ آڈٹس ڈٹرمنزم برقرار رکھیں۔ |

- `scoreboard.weight_scale` ڈیفالٹ `10_000` پر رہتا ہے جب تک downstream سسٹم کسی اور integer resolution کا تقاضا نہ کرے۔ اسکیل بڑھانے سے provider ordering نہیں بدلتی؛ صرف زیادہ گھنا credit distribution بنتا ہے۔
- مراحل کے درمیان منتقلی میں JSON bundle محفوظ کریں اور `--scoreboard-out` استعمال کریں تاکہ آڈٹ ٹریل میں درست پیرامیٹر سیٹ ریکارڈ ہو۔

## 2. Scoreboard کی ہائیجین

Scoreboard manifest کی ضروریات، provider adverts اور ٹیلیمیٹری کو یکجا کرتا ہے۔
آگے بڑھنے سے پہلے:

1. **ٹیلیمیٹری کی تازگی چیک کریں۔** یقینی بنائیں کہ `--telemetry-json` میں حوالہ شدہ snapshots
   مقررہ grace window کے اندر ریکارڈ ہوئے ہوں۔ `telemetry_grace_secs` سے پرانی entries
   `TelemetryStale { last_updated }` کے ساتھ فیل ہوں گی۔ اسے hard stop سمجھیں اور
   ٹیلیمیٹری ایکسپورٹ اپڈیٹ کیے بغیر آگے نہ بڑھیں۔
2. **Eligibility وجوہات دیکھیں۔** `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`
   کے ذریعے آرٹیفیکٹس محفوظ کریں۔ ہر entry میں `eligibility` بلاک ہوتا ہے جو ناکامی
   کی درست وجہ بتاتا ہے۔ capability mismatch یا expired adverts کو اوور رائیڈ نہ کریں؛
   upstream payload درست کریں۔
3. **Weight ڈیلٹاز کا جائزہ لیں۔** `normalised_weight` کو پچھلے release سے موازنہ کریں۔
   10% سے زیادہ تبدیلیاں جان بوجھ کر advert/telemetry تبدیلیوں سے ہم آہنگ ہونی چاہئیں
   اور rollout log میں نوٹ ہونی چاہئیں۔
4. **آرٹیفیکٹس آرکائیو کریں۔** `scoreboard.persist_path` سیٹ کریں تاکہ ہر رن میں
   فائنل scoreboard snapshot محفوظ ہو۔ اسے release ریکارڈ کے ساتھ manifest اور
   telemetry bundle کے ساتھ جوڑیں۔
5. **Provider mix کا ثبوت ریکارڈ کریں۔** `scoreboard.json` کی metadata اور متعلقہ
   `summary.json` میں `provider_count`, `gateway_provider_count` اور `provider_mix`
   لیبل لازماً ہو تاکہ جائزہ لینے والے ثابت کر سکیں کہ رن `direct-only`, `gateway-only`
   یا `mixed` تھا۔ Gateway captures میں `provider_count=0` اور `provider_mix="gateway-only"`
   ہونا چاہیے، جبکہ mixed رنز میں دونوں سورسز کے لیے غیر صفر counts ضروری ہیں۔
   `cargo xtask sorafs-adoption-check` ان فیلڈز کو نافذ کرتا ہے (اور mismatch پر فیل ہوتا ہے)،
   لہٰذا اسے ہمیشہ `ci/check_sorafs_orchestrator_adoption.sh` یا اپنے capture script کے ساتھ
   چلائیں تاکہ `adoption_report.json` evidence bundle بنے۔ جب Torii gateways شامل ہوں تو
   `gateway_manifest_id`/`gateway_manifest_cid` کو scoreboard metadata میں رکھیں تاکہ adoption
   gate manifest envelope کو captured provider mix سے جوڑ سکے۔

فیلڈز کی تفصیلی تعریف کے لیے
`crates/sorafs_car/src/scoreboard.rs` اور CLI summary structure (جو `sorafs_cli fetch --json-out`
سے نکلتی ہے) دیکھیں۔

## CLI اور SDK فلیگ ریفرنس

`sorafs_cli fetch` (دیکھیں `crates/sorafs_car/src/bin/sorafs_cli.rs`) اور
`iroha_cli app sorafs fetch` wrapper (`crates/iroha_cli/src/commands/sorafs.rs`) ایک ہی
orchestrator configuration surface شیئر کرتے ہیں۔ rollout evidence یا canonical
fixtures replay کرنے کے لیے یہ flags استعمال کریں:

Shared multi-source flag reference (CLI help اور docs کو صرف اسی فائل میں ترمیم کر کے sync رکھیں):

- `--max-peers=<count>` eligible providers کی تعداد محدود کرتا ہے جو scoreboard فلٹر سے گزرتے ہیں۔ خالی چھوڑیں تاکہ تمام eligible providers سے stream ہو، اور `1` صرف تب سیٹ کریں جب جان بوجھ کر single-source fallback آزمانا ہو۔ SDKs کے `maxPeers` knob سے ہم آہنگ (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` `FetchOptions` کے per-chunk retry limit تک فارورڈ ہوتا ہے۔ تجویز کردہ اقدار کے لیے ٹیوننگ گائیڈ کے rollout جدول کو استعمال کریں؛ evidence جمع کرنے والی CLI رنز کو SDK defaults سے میچ ہونا چاہیے۔
- `--telemetry-region=<label>` Prometheus `sorafs_orchestrator_*` سیریز (اور OTLP relays) پر region/env لیبل لگاتا ہے تاکہ ڈیش بورڈز lab, staging, canary اور GA ٹریفک الگ کر سکیں۔
- `--telemetry-json=<path>` scoreboard کے لیے referenced snapshot inject کرتا ہے۔ JSON کو scoreboard کے ساتھ محفوظ کریں تاکہ آڈیٹرز رن replay کر سکیں (اور `cargo xtask sorafs-adoption-check --require-telemetry` یہ ثابت کر سکے کہ کون سا OTLP stream capture کو feed کر رہا تھا)۔
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) bridge observer hooks کو فعال کرتے ہیں۔ جب سیٹ ہو تو orchestrator مقامی Norito/Kaigi proxy کے ذریعے chunks stream کرتا ہے تاکہ browser clients، guard caches اور Kaigi rooms کو وہی receipts ملیں جو Rust emit کرتا ہے۔
- `--scoreboard-out=<path>` (اختیاری `--scoreboard-now=<unix_secs>` کے ساتھ) eligibility snapshot محفوظ کرتا ہے۔ محفوظ شدہ JSON کو ہمیشہ release ticket میں referenced telemetry اور manifest artifacts کے ساتھ جوڑیں۔
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` advert metadata کے اوپر deterministic ایڈجسٹمنٹس لگاتے ہیں۔ ان flags کو صرف rehearsals کے لیے استعمال کریں؛ پروڈکشن downgrades کو governance artifacts کے ذریعے جانا چاہیے تاکہ ہر نوڈ وہی policy bundle نافذ کرے۔
- `--provider-metrics-out` / `--chunk-receipts-out` per-provider health metrics اور chunk receipts محفوظ کرتے ہیں؛ adoption evidence جمع کرتے وقت دونوں artifacts ضرور شامل کریں۔

مثال (شائع شدہ fixture استعمال کرتے ہوئے):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDKs اسی configuration کو `SorafsGatewayFetchOptions` کے ذریعے Rust client
(`crates/iroha/src/client.rs`)، JS bindings (`javascript/iroha_js/src/sorafs.js`) اور
Swift SDK (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`) میں استعمال کرتے ہیں۔
ان helpers کو CLI defaults کے ساتھ lock-step رکھیں تاکہ آپریٹرز automation میں
پالیسیوں کو بغیر bespoke ترجمہ لیئرز کے کاپی کر سکیں۔

## 3. Fetch پالیسی ٹیوننگ

`FetchOptions` retries، concurrency اور verification کو کنٹرول کرتا ہے۔ ٹیوننگ کے وقت:

- **Retries:** `per_chunk_retry_limit` کو `4` سے اوپر بڑھانے سے recovery وقت بڑھتا ہے مگر provider faults چھپ سکتے ہیں۔ بہتر ہے `4` کو ceiling رکھا جائے اور provider rotation پر بھروسہ کیا جائے تاکہ کمزور performers سامنے آئیں۔
- **Failure threshold:** `provider_failure_threshold` طے کرتا ہے کہ provider کب سیشن کے باقی حصے کے لیے disable ہو۔ اس ویلیو کو retry پالیسی سے ہم آہنگ رکھیں: اگر threshold retry budget سے کم ہو تو orchestrator تمام retries ختم ہونے سے پہلے peer کو نکال دیتا ہے۔
- **Concurrency:** `global_parallel_limit` کو `None` رکھیں جب تک مخصوص ماحول advertised ranges کو saturate نہ کر سکے۔ اگر سیٹ کریں تو ویلیو providers کے stream budgets کے مجموعے سے ≤ ہو تاکہ starvation نہ ہو۔
- **Verification toggles:** `verify_lengths` اور `verify_digests` پروڈکشن میں فعال رہنے چاہئیں۔ یہ mixed provider fleets میں determinism کی ضمانت دیتے ہیں؛ انہیں صرف isolated fuzzing environments میں ہی بند کریں۔

## 4. ٹرانسپورٹ اور انانیمٹی اسٹیجنگ

پرائیویسی posture کو ظاہر کرنے کے لیے `rollout_phase`, `anonymity_policy`, اور `transport_policy` استعمال کریں:

- `rollout_phase="snnet-5"` کو ترجیح دیں اور default anonymity policy کو SNNet-5 milestones کے ساتھ چلنے دیں۔ `anonymity_policy_override` صرف تب استعمال کریں جب governance نے signed directive جاری کیا ہو۔
- `transport_policy="soranet-first"` کو baseline رکھیں جب تک SNNet-4/5/5a/5b/6a/7/8/12/13 🈺 ہوں
  (دیکھیں `roadmap.md`). `transport_policy="direct-only"` صرف دستاویزی downgrades یا compliance drills کے لیے استعمال کریں، اور PQ coverage review کے بعد ہی `transport_policy="soranet-strict"` پر جائیں—اس سطح پر صرف classical relays رہیں تو فوری fail ہوگا۔
- `write_mode="pq-only"` صرف اسی وقت نافذ کریں جب ہر write path (SDK، orchestrator، governance tooling) PQ تقاضے پورے کر سکے۔ rollouts کے دوران `write_mode="allow-downgrade"` رکھیں تاکہ ہنگامی ردعمل direct routes پر انحصار کر سکے جبکہ telemetry downgrade کو نشان زد کرے۔
- Guard selection اور circuit staging SoraNet directory پر منحصر ہیں۔ signed `relay_directory` snapshot فراہم کریں اور `guard_set` cache محفوظ کریں تاکہ guard churn طے شدہ retention window میں رہے۔ `sorafs_cli fetch` کے ذریعے لاگ ہونے والا cache fingerprint rollout evidence کا حصہ ہے۔

## 5. Downgrade اور compliance hooks

دو ذیلی نظام دستی مداخلت کے بغیر پالیسی نافذ کرنے میں مدد دیتے ہیں:

- **Downgrade remediation** (`downgrade_remediation`): `handshake_downgrade_total` ایونٹس کی نگرانی کرتا ہے اور `window_secs` کے اندر `threshold` تجاوز ہونے پر local proxy کو `target_mode` پر مجبور کرتا ہے (ڈیفالٹ metadata-only)۔ ڈیفالٹس (`threshold=3`, `window=300`, `cooldown=900`) برقرار رکھیں جب تک انسیڈنٹ ریویوز مختلف پیٹرن نہ بتائیں۔ کسی بھی override کو rollout log میں دستاویز کریں اور `sorafs_proxy_downgrade_state` کو ڈیش بورڈز پر ٹریک کریں۔
- **Compliance policy** (`compliance`): jurisdiction اور manifest carve-outs governance-managed opt-out lists کے ذریعے آتے ہیں۔ کنفیگریشن bundle میں ad-hoc overrides مت ڈالیں؛ اس کے بجائے `governance/compliance/soranet_opt_outs.json` کے لیے signed update طلب کریں اور generated JSON دوبارہ deploy کریں۔

دونوں سسٹمز کے لیے، نتیجہ خیز کنفیگریشن bundle محفوظ کریں اور اسے release evidence میں شامل کریں تاکہ آڈیٹرز downgrade triggers کو ٹریس کر سکیں۔

## 6. ٹیلیمیٹری اور ڈیش بورڈز

رول آؤٹ بڑھانے سے پہلے تصدیق کریں کہ درج ذیل سگنلز target ماحول میں فعال ہیں:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  کینری مکمل ہونے کے بعد صفر ہونا چاہیے۔
- `sorafs_orchestrator_retries_total` اور
  `sorafs_orchestrator_retry_ratio` — کینری کے دوران 10% سے کم پر مستحکم ہوں اور GA کے بعد 5% سے کم رہیں۔
- `sorafs_orchestrator_policy_events_total` — متوقع rollout stage کی تصدیق کرتا ہے (label `stage`) اور `outcome` کے ذریعے brownouts ریکارڈ کرتا ہے۔
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — پالیسی توقعات کے مقابلے میں PQ relay سپلائی ٹریک کرتے ہیں۔
- `telemetry::sorafs.fetch.*` لاگ ٹارگٹس — مشترکہ لاگ ایگریگیٹر پر جائیں اور `status=failed` کے لیے محفوظ تلاشیں ہوں۔

`dashboards/grafana/sorafs_fetch_observability.json` سے canonical Grafana dashboard لوڈ کریں
(پورٹل میں **SoraFS → Fetch Observability** کے تحت ایکسپورٹ شدہ)، تاکہ region/manifest selectors،
provider retry heatmap، chunk latency histograms اور stall counters وہی ہوں جو SRE
burn-ins کے دوران دیکھتا ہے۔ Alertmanager rules کو `dashboards/alerts/sorafs_fetch_rules.yml`
میں وائر کریں اور `scripts/telemetry/test_sorafs_fetch_alerts.sh` سے Prometheus syntax
ویلیڈیٹ کریں (helper `promtool test rules` کو لوکل یا Docker میں چلاتا ہے)۔ Alert hand-offs
کے لیے اسی routing بلاک کی ضرورت ہے جو اسکرپٹ پرنٹ کرتا ہے تاکہ آپریٹرز ثبوت کو rollout
ٹکٹ سے جوڑ سکیں۔

### ٹیلیمیٹری burn-in ورک فلو

Roadmap item **SF-6e** کے تحت 30 دن کی telemetry burn-in درکار ہے اس سے پہلے کہ
ملٹی سورس آرکسٹریٹر GA ڈیفالٹس پر چلا جائے۔ ریپو اسکرپٹس کے ذریعے ہر دن کے لیے
ریپروڈیوس ایبل آرٹیفیکٹس بنائیں:

1. `ci/check_sorafs_orchestrator_adoption.sh` کو burn-in environment knobs کے ساتھ چلائیں۔ مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Helper `fixtures/sorafs_orchestrator/multi_peer_parity_v1` ری پلے کرتا ہے،
   `scoreboard.json`, `summary.json`, `provider_metrics.json`, `chunk_receipts.json`,
   اور `adoption_report.json` کو `artifacts/sorafs_orchestrator/<timestamp>/` میں لکھتا ہے،
   اور `cargo xtask sorafs-adoption-check` کے ذریعے کم از کم eligible providers enforce کرتا ہے۔
2. جب burn-in ویری ایبلز موجود ہوں تو اسکرپٹ `burn_in_note.json` بھی بناتا ہے، جس میں label،
   day index، manifest id، telemetry source اور artefact digests محفوظ ہوتے ہیں۔ اسے rollout
   log میں شامل کریں تاکہ واضح ہو کہ 30 روزہ ونڈو کا کون سا دن کس capture سے پورا ہوا۔
3. اپڈیٹ شدہ Grafana بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) کو staging/production
   workspace میں امپورٹ کریں، burn-in label لگائیں، اور تصدیق کریں کہ ہر پینل ٹیسٹ شدہ
   manifest/region کے نمونے دکھاتا ہے۔
4. جب بھی `dashboards/alerts/sorafs_fetch_rules.yml` بدلے، `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   (یا `promtool test rules …`) چلائیں تاکہ alert routing burn-in metrics کے مطابق ہونا دستاویزی ہو۔
5. ڈیش بورڈ snapshot، alert test output اور `telemetry::sorafs.fetch.*` لاگ ٹیل کو آرکسٹریٹر
   آرٹیفیکٹس کے ساتھ محفوظ کریں تاکہ governance بغیر live systems سے metrics اٹھائے evidence
   ری پلے کر سکے۔

## 7. رول آؤٹ چیک لسٹ

1. CI میں candidate configuration کے ساتھ scoreboards دوبارہ بنائیں اور artifacts کو version control میں رکھیں۔
2. ہر ماحول (lab, staging, canary, production) میں deterministic fixture fetch چلائیں اور `--scoreboard-out` اور `--json-out` artifacts کو rollout ریکارڈ کے ساتھ منسلک کریں۔
3. on-call انجینئر کے ساتھ ٹیلیمیٹری ڈیش بورڈز ریویو کریں، اور یقینی بنائیں کہ اوپر دیے گئے تمام metrics کے live samples موجود ہیں۔
4. حتمی کنفیگریشن پاتھ (عام طور پر `iroha_config` کے ذریعے) اور governance registry کے git commit کو ریکارڈ کریں جو adverts اور compliance کے لیے استعمال ہوا۔
5. rollout tracker اپڈیٹ کریں اور SDK ٹیموں کو نئے ڈیفالٹس سے آگاہ کریں تاکہ کلائنٹ انٹیگریشنز سیدھ میں رہیں۔

اس گائیڈ کی پیروی آرکسٹریٹر ڈیپلائمنٹس کو ڈٹرمنسٹک اور آڈٹ ایبل رکھتی ہے جبکہ
retry budgets، provider capacity، اور privacy posture کو ٹیون کرنے کے لیے واضح
فیڈبیک لوپس فراہم کرتی ہے۔
