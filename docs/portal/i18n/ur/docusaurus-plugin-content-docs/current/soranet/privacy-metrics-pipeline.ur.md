---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: privacy-metrics-pipeline
title: SoraNet پرائیویسی میٹرکس پائپ لائن (SNNet-8)
sidebar_label: پرائیویسی میٹرکس پائپ لائن
description: SoraNet کے relays اور orchestrators کے لئے پرائیویسی محفوظ telemetry جمع کرنا۔
---

:::note مستند ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ پرانے docs کے ختم ہونے تک دونوں نقول ہم وقت رکھیں۔
:::

# SoraNet پرائیویسی میٹرکس پائپ لائن

SNNet-8 relay runtime کے لئے پرائیویسی کو مدنظر رکھنے والی telemetry سطح متعارف کراتا ہے۔ relay اب handshake اور circuit واقعات کو ایک منٹ کے buckets میں جمع کرتا ہے اور صرف موٹے Prometheus counters برآمد کرتا ہے، جس سے انفرادی circuits unlinkable رہتے ہیں جبکہ آپریٹرز کو قابلِ عمل بصیرت ملتی ہے۔

## Aggregator کا خلاصہ

- runtime کی implementation `tools/soranet-relay/src/privacy.rs` میں `PrivacyAggregator` کے طور پر موجود ہے۔
- buckets کو wall-clock منٹ کے حساب سے key کیا جاتا ہے (`bucket_secs`, default 60 seconds) اور انہیں ایک محدود ring (`max_completed_buckets`, default 120) میں رکھا جاتا ہے۔ collector shares اپنا محدود backlog رکھتے ہیں (`max_share_lag_buckets`, default 12) تاکہ پرانے Prio windows suppressed buckets کے طور پر flush ہوں اور memory leak یا stuck collectors چھپ نہ جائیں۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` میں map ہوتا ہے اور tuning knobs (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`) ظاہر کرتا ہے۔ production runtime defaults رکھتا ہے جبکہ SNNet-8a secure aggregation thresholds متعارف کراتا ہے۔
- runtime ماڈیولز typed helpers کے ذریعے events ریکارڈ کرتے ہیں: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, اور `record_gar_category`۔

## Relay admin endpoint

آپریٹرز `GET /privacy/events` کے ذریعے relay کے admin listener کو poll کر کے raw observations لے سکتے ہیں۔ یہ endpoint newline-delimited JSON (`application/x-ndjson`) واپس کرتا ہے جس میں `SoranetPrivacyEventV1` payloads ہوتے ہیں جو اندرونی `PrivacyEventBuffer` سے mirror ہوتے ہیں۔ buffer تازہ ترین events کو `privacy.event_buffer_capacity` entries تک رکھتا ہے (default 4096) اور read پر drain ہو جاتا ہے، اس لئے scrapers کو gaps سے بچنے کیلئے کافی بار poll کرنا چاہئے۔ events وہی handshake، throttle، verified bandwidth، active circuit، اور GAR signals کور کرتے ہیں جو Prometheus counters کو طاقت دیتے ہیں، تاکہ downstream collectors پرائیویسی محفوظ breadcrumbs محفوظ کر سکیں یا secure aggregation workflows کو feed کریں۔

## Relay configuration

آپریٹرز relay configuration فائل کے `privacy` سیکشن کے ذریعے پرائیویسی telemetry cadence ایڈجسٹ کرتے ہیں:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Field defaults SNNet-8 spec کے مطابق ہیں اور load کے وقت validate ہوتے ہیں:

| Field | Description | Default |
|-------|-------------|---------|
| `bucket_secs` | ہر aggregation window کی چوڑائی (seconds)۔ | `60` |
| `min_handshakes` | کم از کم contributor count جس کے بعد bucket counters emit کر سکتا ہے۔ | `12` |
| `flush_delay_buckets` | flush کی کوشش سے پہلے مکمل buckets کی تعداد۔ | `1` |
| `force_flush_buckets` | suppressed bucket emit کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | محفوظ شدہ bucket backlog (unbounded memory سے بچاتا ہے)۔ | `120` |
| `max_share_lag_buckets` | suppression سے پہلے collector shares کی retention window۔ | `12` |
| `expected_shares` | combine کرنے سے پہلے درکار Prio collector shares۔ | `2` |
| `event_buffer_capacity` | admin stream کیلئے NDJSON event backlog۔ | `4096` |

`force_flush_buckets` کو `flush_delay_buckets` سے کم کرنا، thresholds کو صفر کرنا، یا retention guard کو بند کرنا اب validation fail کرتا ہے تاکہ ایسی deployments سے بچا جائے جو per-relay telemetry leak کریں۔

`event_buffer_capacity` کی حد `/admin/privacy/events` کو بھی bound کرتی ہے، تاکہ scrapers غیر معینہ مدت تک پیچھے نہ رہ جائیں۔

## Prio collector shares

SNNet-8a dual collectors تعینات کرتا ہے جو secret-shared Prio buckets emit کرتے ہیں۔ orchestrator اب `/privacy/events` NDJSON stream کو `SoranetPrivacyEventV1` entries اور `SoranetPrivacyPrioShareV1` shares دونوں کیلئے parse کرتا ہے اور انہیں `SoranetSecureAggregator::ingest_prio_share` میں forward کرتا ہے۔ buckets اس وقت emit ہوتے ہیں جب `PrivacyBucketConfig::expected_shares` contributions آ جائیں، جو relay behavior کی عکاسی ہے۔ shares کو bucket alignment اور histogram shape کیلئے validate کیا جاتا ہے، پھر انہیں `SoranetPrivacyBucketMetricsV1` میں combine کیا جاتا ہے۔ اگر combined handshake count `min_contributors` سے نیچے چلا جائے تو bucket کو `suppressed` کے طور پر export کیا جاتا ہے، جو in-relay aggregator behavior جیسا ہے۔ suppressed windows اب `suppression_reason` label emit کرتے ہیں تاکہ آپریٹرز `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, اور `forced_flush_window_elapsed` کے درمیان فرق کر سکیں جب telemetry gaps کی تشخیص کر رہے ہوں۔ `collector_window_elapsed` کی وجہ تب بھی fire ہوتی ہے جب Prio shares `max_share_lag_buckets` سے زیادہ دیر رہیں، جس سے stuck collectors نظر آتے ہیں اور stale accumulators memory میں نہیں رہتے۔

## Torii ingestion endpoints

Torii اب دو telemetry-gated HTTP endpoints فراہم کرتا ہے تاکہ relays اور collectors کسی bespoke transport کے بغیر observations forward کر سکیں:

- `POST /v2/soranet/privacy/event` ایک `RecordSoranetPrivacyEventDto` payload قبول کرتا ہے۔ body میں `SoranetPrivacyEventV1` کے ساتھ ایک optional `source` label شامل ہوتا ہے۔ Torii request کو فعال telemetry profile کے مطابق validate کرتا ہے، event ریکارڈ کرتا ہے، اور HTTP `202 Accepted` کے ساتھ ایک Norito JSON envelope واپس کرتا ہے جس میں computed bucket window (`bucket_start_unix`, `bucket_duration_secs`) اور relay mode ہوتا ہے۔
- `POST /v2/soranet/privacy/share` ایک `RecordSoranetPrivacyShareDto` payload قبول کرتا ہے۔ body میں `SoranetPrivacyPrioShareV1` اور optional `forwarded_by` hint ہوتا ہے تاکہ آپریٹرز collector flows کا audit کر سکیں۔ کامیاب submissions HTTP `202 Accepted` کے ساتھ Norito JSON envelope واپس کرتی ہیں جو collector، bucket window، اور suppression hint کو summarize کرتا ہے؛ validation failures telemetry `Conversion` response سے map ہوتے ہیں تاکہ collectors کے درمیان deterministic error handling برقرار رہے۔ orchestrator کا event loop اب relay polling کے دوران یہ shares emit کرتا ہے، جس سے Torii کا Prio accumulator on-relay buckets کے ساتھ sync رہتا ہے۔

دونوں endpoints telemetry profile کی پابندی کرتے ہیں: metrics disabled ہونے پر `503 Service Unavailable` واپس کرتے ہیں۔ clients Norito binary (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) bodies بھیج سکتے ہیں؛ server standard Torii extractors کے ذریعے format خود بخود negotiate کرتا ہے۔

## Prometheus metrics

ہر exported bucket میں `mode` (`entry`, `middle`, `exit`) اور `bucket_start` labels ہوتے ہیں۔ درج ذیل metric families emit کی جاتی ہیں:

| Metric | Description |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | handshake taxonomy جس میں `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` شامل ہے۔ |
| `soranet_privacy_throttles_total{scope}` | throttle counters جن میں `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ہے۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | throttled handshakes سے آنے والی cooldown مدتوں کا مجموعہ۔ |
| `soranet_privacy_verified_bytes_total` | blinded measurement proofs سے verified bandwidth۔ |
| `soranet_privacy_active_circuits_{avg,max}` | ہر bucket میں active circuits کا اوسط اور زیادہ سے زیادہ۔ |
| `soranet_privacy_rtt_millis{percentile}` | RTT percentile estimates (`p50`, `p90`, `p99`)۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | hashed Governance Action Report counters جو category digest سے key ہوتے ہیں۔ |
| `soranet_privacy_bucket_suppressed` | وہ buckets جو contributor threshold پورا نہ ہونے کی وجہ سے روکے گئے۔ |
| `soranet_privacy_pending_collectors{mode}` | collector share accumulators جو combine ہونے کے منتظر ہیں، relay mode کے حساب سے گروپ کیے گئے۔ |
| `soranet_privacy_suppression_total{reason}` | suppressed bucket counters جن میں `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` شامل ہیں تاکہ dashboards privacy gaps کو نسبت دے سکیں۔ |
| `soranet_privacy_snapshot_suppression_ratio` | آخری drain کا suppressed/drained ratio (0-1)، alert budgets کیلئے مفید۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب poll کا UNIX timestamp (collector-idle alert کو چلانے کیلئے)۔ |
| `soranet_privacy_collector_enabled` | gauge جو اس وقت `0` ہو جاتا ہے جب privacy collector بند ہو یا start ہونے میں ناکام ہو (collector-disabled alert کیلئے)۔ |
| `soranet_privacy_poll_errors_total{provider}` | polling failures جو relay alias کے حساب سے گروپ ہوتے ہیں (decode errors، HTTP failures، یا غیر متوقع status codes پر بڑھتے ہیں)۔ |

جن buckets میں observations نہیں ہوتیں وہ خاموش رہتے ہیں، جس سے dashboards صاف رہتے ہیں اور zero-filled windows نہیں بنتیں۔

## Operational guidance

1. **Dashboards** - اوپر والی metrics کو `mode` اور `window_start` کے حساب سے گروپ کر کے چارٹ کریں۔ missing windows کو highlight کریں تاکہ collector یا relay مسائل سامنے آئیں۔ `soranet_privacy_suppression_total{reason}` استعمال کریں تاکہ gaps کی triage میں contributor shortfall اور collector-driven suppression کے درمیان فرق ہو سکے۔ Grafana asset اب ایک dedicated **"Suppression Reasons (5m)"** panel فراہم کرتا ہے جو ان counters سے چلتا ہے، ساتھ ہی ایک **"Suppressed Bucket %"** stat جو `sum(soranet_privacy_bucket_suppressed) / count(...)` فی selection حساب کرتا ہے تاکہ آپریٹرز budget breaches ایک نظر میں دیکھ سکیں۔ **"Collector Share Backlog"** series (`soranet_privacy_pending_collectors`) اور **"Snapshot Suppression Ratio"** stat stuck collectors اور automated runs کے دوران budget drift کو نمایاں کرتے ہیں۔
2. **Alerting** - privacy-safe counters سے alarms چلائیں: PoW reject spikes، cooldown frequency، RTT drift، اور capacity rejects۔ چونکہ ہر bucket کے اندر counters monotonic ہوتے ہیں، سادہ rate-based rules اچھا کام کرتے ہیں۔
3. **Incident response** - پہلے aggregated data پر انحصار کریں۔ جب گہرے debugging کی ضرورت ہو تو relays سے bucket snapshots replay کروائیں یا blinded measurement proofs دیکھیں، raw traffic logs جمع نہ کریں۔
4. **Retention** - `max_completed_buckets` سے تجاوز سے بچنے کیلئے کافی بار scrape کریں۔ exporters کو Prometheus output کو canonical source سمجھنا چاہئے اور forward کرنے کے بعد local buckets حذف کر دینے چاہئیں۔

## Suppression analytics اور automated runs

SNNet-8 کی قبولیت اس بات پر منحصر ہے کہ automated collectors صحت مند رہیں اور suppression پالیسی حدود کے اندر رہے (ہر relay کیلئے کسی بھی 30 منٹ کی window میں ≤10% buckets)۔ اس gate کو پورا کرنے کیلئے درکار tooling اب repo کے ساتھ آتا ہے؛ آپریٹرز کو اسے اپنی ہفتہ وار rituals میں شامل کرنا ہوگا۔ نئے Grafana suppression panels نیچے دیے گئے PromQL snippets کو منعکس کرتے ہیں، جس سے on-call ٹیموں کو live visibility ملتی ہے اس سے پہلے کہ انہیں manual queries کی ضرورت پڑے۔

### Suppression review کیلئے PromQL recipes

آپریٹرز کو درج ذیل PromQL helpers ساتھ رکھنے چاہئیں؛ دونوں shared Grafana dashboard (`dashboards/grafana/soranet_privacy_metrics.json`) اور Alertmanager rules میں referenced ہیں:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

ratio output استعمال کریں تاکہ **"Suppressed Bucket %"** stat پالیسی budget سے نیچے رہے؛ spike detector کو Alertmanager سے جوڑیں تاکہ contributors کی تعداد غیر متوقع طور پر کم ہونے پر فوری فیڈبیک ملے۔

### Offline bucket report CLI

workspace میں `cargo xtask soranet-privacy-report` ایک وقتی NDJSON captures کیلئے دستیاب ہے۔ اسے ایک یا زیادہ relay admin exports پر point کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

یہ helper capture کو `SoranetSecureAggregator` کے ذریعے stream کرتا ہے، stdout پر suppression summary پرنٹ کرتا ہے، اور اختیاری طور پر `--json-out <path|->` کے ذریعے structured JSON report لکھتا ہے۔ یہ live collector والے knobs (`--bucket-secs`, `--min-contributors`, `--expected-shares`, وغیرہ) کو honour کرتا ہے، جس سے آپریٹرز issues کی triage میں مختلف thresholds کے ساتھ historical captures replay کر سکتے ہیں۔ SNNet-8 suppression analytics gate کو audit کے قابل رکھنے کیلئے JSON کو Grafana screenshots کے ساتھ attach کریں۔

### پہلی automated run checklist

Governance اب بھی ثبوت چاہتا ہے کہ پہلی automation run suppression budget میں رہی۔ helper اب `--max-suppression-ratio <0-1>` قبول کرتا ہے تاکہ CI یا آپریٹرز اس وقت fail fast کر سکیں جب suppressed buckets allowed window سے بڑھ جائیں (default 10%) یا جب ابھی کوئی buckets موجود نہ ہوں۔ تجویز کردہ flow:

1. relay admin endpoint(s) اور orchestrator کے `/v2/soranet/privacy/event|share` stream سے NDJSON export کر کے `artifacts/sorafs_privacy/<relay>.ndjson` میں محفوظ کریں۔
2. policy budget کے ساتھ helper چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   کمانڈ observed ratio پرنٹ کرتی ہے اور non-zero exit کرتی ہے جب budget exceed ہو **یا** جب buckets ابھی تیار نہ ہوں، جس سے اشارہ ملتا ہے کہ telemetry ابھی run کیلئے پیدا نہیں ہوئی۔ live metrics کو دکھانا چاہئے کہ `soranet_privacy_pending_collectors` صفر کی طرف drain ہو رہا ہے اور `soranet_privacy_snapshot_suppression_ratio` اسی budget سے نیچے رہتا ہے جب run چل رہا ہو۔
3. transport default بدلنے سے پہلے JSON output اور CLI log کو SNNet-8 evidence bundle کے ساتھ archive کریں تاکہ reviewers وہی artifacts دوبارہ چلا سکیں۔

## Next Steps (SNNet-8a)

- dual Prio collectors کو integrate کریں اور ان کے share ingestion کو runtime سے جوڑیں تاکہ relays اور collectors consistent `SoranetPrivacyBucketMetricsV1` payloads emit کریں۔ *(Done — `crates/sorafs_orchestrator/src/lib.rs` میں `ingest_privacy_payload` اور متعلقہ tests دیکھیں۔)*
- shared Prometheus dashboard JSON اور alert rules شائع کریں جو suppression gaps، collector health، اور anonymity brownouts کو cover کریں۔ *(Done — `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` اور validation fixtures دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ differential-privacy calibration artifacts تیار کریں، جن میں reproducible notebooks اور governance digests شامل ہوں۔ *(Done — notebook اور artifacts `scripts/telemetry/run_privacy_dp.py` سے بنتے ہیں؛ CI wrapper `scripts/telemetry/run_privacy_dp_notebook.sh` notebook کو `.github/workflows/release-pipeline.yml` workflow کے ذریعے چلاتا ہے؛ governance digest `docs/source/status/soranet_privacy_dp_digest.md` میں فائل کیا گیا ہے۔)*

موجودہ release SNNet-8 کی بنیاد فراہم کرتی ہے: deterministic، privacy-safe telemetry جو براہ راست موجودہ Prometheus scrapers اور dashboards میں فٹ ہوتی ہے۔ differential privacy calibration artifacts موجود ہیں، release pipeline workflow notebook outputs کو تازہ رکھتا ہے، اور باقی کام پہلی automated run کی monitoring اور suppression alert analytics کو مزید بڑھانے پر مرکوز ہے۔
