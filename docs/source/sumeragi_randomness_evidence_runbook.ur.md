---
lang: ur
direction: rtl
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/sumeragi_randomness_evidence_runbook.md -->

# Sumeragi Randomness اور Evidence Runbook

یہ گائیڈ Milestone A6 روڈ میپ آئٹم کو پورا کرتا ہے جس میں VRF randomness اور
slashing evidence کیلئے اپ ڈیٹڈ operator procedures درکار تھیں۔ اسے
{doc}`sumeragi` اور {doc}`sumeragi_chaos_performance_runbook` کے ساتھ استعمال کریں
جب بھی آپ نیا validator build stage کریں یا governance کیلئے readiness artefacts
جمع کریں۔


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## دائرہ کار اور ضروریات

- `iroha_cli` ہدف کلسٹر کیلئے configured ہو (دیکھیں `docs/source/cli.md`).
- `curl`/`jq` تاکہ Torii `/status` payload scrape کیا جا سکے۔
- Prometheus تک رسائی (یا snapshot exports) `sumeragi_vrf_*` میٹرکس کیلئے۔
- موجودہ epoch اور roster سے واقفیت تاکہ CLI output کو staking snapshot یا
  governance manifest سے ملایا جا سکے۔

## 1. موڈ سلیکشن اور epoch context کی تصدیق

1. `iroha sumeragi params --summary` چلائیں تاکہ ثابت ہو کہ بائنری نے
   `sumeragi.consensus_mode="npos"` لوڈ کیا ہے اور `k_aggregators`,
   `redundant_send_r`, epoch length، اور VRF commit/reveal offsets ریکارڈ ہوں۔
2. runtime view دیکھیں:

   ```bash
   iroha sumeragi status --summary
   iroha sumeragi collectors --summary
   iroha sumeragi rbc status --summary
   ```

   `status` لائن leader/view tuple، RBC backlog، DA retries، epoch offsets، اور
   pacemaker deferrals پرنٹ کرتی ہے؛ `collectors` collector indices کو peer IDs سے
   map کرتا ہے تاکہ معلوم ہو کہ کون سے validators اس height پر randomness duties
   سنبھال رہے ہیں۔
3. جس epoch کا آڈٹ کرنا ہے اس کا نمبر حاصل کریں:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   اس ویلیو کو (decimal یا `0x` prefix کے ساتھ) نیچے والی VRF کمانڈز کیلئے محفوظ کریں۔

## 2. VRF epochs اور penalties کا snapshot

ہر validator سے persisted VRF ریکارڈز نکالنے کیلئے مخصوص CLI subcommands استعمال کریں:

```bash
iroha sumeragi vrf-epoch --epoch "$EPOCH" --summary
iroha sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha sumeragi vrf-penalties --epoch "$EPOCH" --summary
iroha sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

Summaries بتاتے ہیں کہ epoch finalize ہوا یا نہیں، کتنے participants نے commits/reveals
جمع کرائے، roster length، اور derived seed۔ JSON میں participant list، signer کے حساب سے
penalty status، اور pacemaker کا `seed_hex` شامل ہوتا ہے۔ participant count کو staking roster
سے ملا کر دیکھیں، اور تصدیق کریں کہ penalty arrays chaos testing کے دوران آنے والے alerts
کی عکاسی کرتی ہیں (late reveals `late_reveals` میں، forfeited validators `no_participation` میں)۔

## 3. VRF telemetria اور alerts کی نگرانی

Prometheus roadmap کے مطلوبہ counters expose کرتا ہے:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

ہفتہ وار رپورٹ کیلئے PromQL مثال:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

Readiness drills کے دوران تصدیق کریں کہ:

- `sumeragi_vrf_commits_emitted_total` اور `..._reveals_emitted_total` ہر block میں
  commit/reveal windows کے اندر بڑھتے ہیں۔
- Late-reveal scenarios `sumeragi_vrf_reveals_late_total` ٹرگر کرتے ہیں اور
  `vrf_penalties` JSON میں متعلقہ entry clear کرتے ہیں۔
- `sumeragi_vrf_no_participation_total` صرف تب بڑھتا ہے جب chaos testing میں
  جان بوجھ کر commits روکیں۔

Grafana overview (`docs/source/grafana_sumeragi_overview.json`) میں ہر counter کیلئے panels ہیں؛
ہر run کے بعد screenshots لیں اور انہیں {doc}`sumeragi_chaos_performance_runbook` میں ذکر کردہ
artifact bundle کے ساتھ attach کریں۔

## 4. Evidence ingestion اور streaming

Slashing evidence ہر validator پر جمع ہونا چاہئے اور Torii کو relay ہونا چاہئے۔ CLI helpers
استعمال کریں تاکہ HTTP endpoints کے ساتھ parity دکھائی جا سکے جو
{doc}`torii/sumeragi_evidence_app_api` میں دستاویزی ہیں:

```bash
# Count and list persisted evidence
iroha sumeragi evidence count --summary
iroha sumeragi evidence list --summary --limit 5

# Show JSON for audits
iroha sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

یقینی بنائیں کہ رپورٹ شدہ `total` Grafana widget (`sumeragi_evidence_records_total`) سے
میل کھاتا ہے، اور یہ بھی کہ `sumeragi.npos.reconfig.evidence_horizon_blocks` سے پرانے
records reject ہوتے ہیں (CLI drop reason دکھاتا ہے)۔ alerting کی جانچ کیلئے معروف درست
payload یہیں سے بھیجیں:

```bash
iroha sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex --summary
```

`/v1/events/sse` کو filtered stream کے ساتھ مانیٹر کریں تاکہ ثابت ہو کہ SDKs وہی data دیکھتے ہیں:
{doc}`torii/sumeragi_evidence_app_api` کا Python one-liner استعمال کر کے filter بنائیں اور raw
`data:` frames capture کریں۔ SSE payloads میں evidence kind اور signer ویسا ہی ہونا چاہئے جیسا
CLI output میں آیا تھا۔

## 5. Evidence packaging اور reporting

ہر rehearsal یا release candidate کیلئے:

1. CLI JSON فائلیں (`vrf_epoch_*.json`, `vrf_penalties_*.json`, `evidence_snapshot.json`)
   run کے artifact directory میں محفوظ کریں (وہی root جو chaos/performance scripts استعمال کرتے ہیں)۔
2. اوپر دیے counters کیلئے Prometheus query results یا snapshot exports ریکارڈ کریں۔
3. SSE capture اور alert acknowledgements کو artifacts README کے ساتھ attach کریں۔
4. `status.md` اور
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` کو artifact paths اور
   inspect کیے گئے epoch نمبر کے ساتھ اپ ڈیٹ کریں۔

یہ checklist فالو کرنے سے VRF randomness proofs اور slashing evidence NPoS rollout کے دوران
قابلِ آڈٹ رہتے ہیں اور governance reviewers کو deterministic ٹریس ملتی ہے جو captured metrics
اور CLI snapshots تک جاتی ہے۔

## 6. Troubleshooting signals

- **Mode selection mismatch** — اگر `iroha sumeragi params --summary` میں
  `consensus_mode="permissioned"` آئے یا `k_aggregators` manifest سے مختلف ہو،
  تو captured artefacts حذف کریں، `iroha_config` درست کریں، validator restart کریں،
  اور {doc}`sumeragi` میں بیان کردہ validation flow دوبارہ چلائیں۔
- **Missing commits or reveals** — `sumeragi_vrf_commits_emitted_total` یا
  `sumeragi_vrf_reveals_emitted_total` کی flat series اس بات کی علامت ہے کہ Torii
  VRF frames forward نہیں کر رہا۔ validator logs میں `handle_vrf_*` errors دیکھیں،
  پھر اوپر بیان کردہ POST helpers کے ذریعے payload دستی طور پر بھیجیں۔
- **Unexpected penalties** — جب `sumeragi_vrf_no_participation_total` spike کرے،
  `vrf_penalties_<epoch>.json` فائل میں signer ID چیک کریں اور اسے staking roster سے
  compare کریں۔ chaos drills سے نہ ملنے والی penalties validator clock skew یا Torii
  replay protection کی نشاندہی کرتی ہیں؛ test دوبارہ چلانے سے پہلے peer درست کریں۔
- **Evidence ingestion stalls** — جب `sumeragi_evidence_records_total` رکتا ہوا نظر آئے جبکہ
  chaos tests faults emit کر رہے ہوں، تو `iroha sumeragi evidence count` کئی validators پر چلائیں
  اور `/v1/sumeragi/evidence/count` کو CLI output سے match کریں۔ کسی بھی فرق کا مطلب یہ ہے کہ
  SSE/webhook consumers stale ہو سکتے ہیں، اس لئے معروف fixture دوبارہ submit کریں اور اگر
  counter پھر بھی نہ بڑھے تو Torii maintainers تک معاملہ escalate کریں۔

</div>
