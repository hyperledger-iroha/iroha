---
lang: ur
direction: rtl
source: docs/source/sumeragi_aggregators.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee79dba673794a3dd4f888d3daf39163a827443bb22d413ab4d7f2e252762293
source_last_modified: "2025-12-26T13:17:08.872635+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/sumeragi_aggregators.md -->

# Sumeragi Aggregator Routing

## جائزہ

یہ نوٹ Phase 3 fairness اپ ڈیٹ کے بعد Sumeragi میں استعمال ہونے والی deterministic collector ("aggregator") routing حکمت عملی کو بیان کرتا ہے۔ ہر validator کسی دیے گئے block height اور view کیلئے collectors کی ایک ہی ordering نکالتا ہے۔ ڈیزائن ad hoc randomness پر انحصار ختم کرتا ہے اور votes کا نارمل fan-out collector list کے اندر محدود رکھتا ہے؛ جب collectors دستیاب نہ ہوں یا quorum رک جائے تو reschedule rebroadcasts collector targets دوبارہ استعمال کرتے ہیں اور commit topology پر fallback کرتے ہیں۔

## Deterministic selection

- نیا `sumeragi::collectors` ماڈیول `deterministic_collectors(topology, mode, k, seed, height, view)` فراہم کرتا ہے جو `(height, view)` کیلئے ایک قابلِ تکرار `Vec<PeerId>` واپس کرتا ہے۔
- Permissioned mode میں contiguous tail collector set کو `height + view` کے مطابق rotate کیا جاتا ہے، جس سے ہر collector round-robin شیڈول میں primary بنتا ہے۔ یہ اصل proxy-tail رویہ برقرار رکھتا ہے اور tail segment (proxy tail + Set B validators) پر لوڈ کو یکساں بانٹتا ہے۔
- NPoS mode ہر epoch کیلئے PRF استعمال کرتا رہتا ہے، مگر helper اب computation کو مرکزی بناتا ہے تاکہ ہر caller کو ایک ہی ترتیب ملے۔ seed کو `EpochManager` کی فراہم کردہ epoch randomness سے derive کیا جاتا ہے۔
- `CollectorPlan` ordered targets کے استعمال کو ٹریک کرتا ہے اور یہ ریکارڈ کرتا ہے کہ gossip fallback ٹرگر ہوا یا نہیں۔ Telemetria updates (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`, `sumeragi_gossip_fallback_total`) بتاتے ہیں کہ fallbacks کتنی بار ہوتے ہیں اور redundant fan-out کتنا وقت لیتا ہے۔

## Fairness اہداف

1. **Reproducibility:** ایک ہی validator topology، consensus mode اور `(height, view)` tuple کو ہر peer پر ایک جیسے primary/secondary collectors پیدا کرنے چاہئیں۔ helper topology کی پیچیدگیاں (proxy tail، Set B validators) چھپاتا ہے تاکہ ordering مختلف components اور tests میں portable رہے۔
2. **Rotation:** Permissioned deployments میں primary collector ہر block پر rotate ہوتا ہے (اور view bump کے بعد بھی بدلتا ہے)، جس سے کوئی ایک Set B validator مستقل aggregation ذمہ داری نہیں رکھتا۔ PRF-based NPoS selection پہلے ہی randomness دیتا ہے اور متاثر نہیں ہوتا۔
3. **Observability:** Telemetria ہر collector کی assignment رپورٹ کرتی رہتی ہے اور fallback path gossip لگنے پر warning دیتا ہے تاکہ operators misbehaving collectors کو detect کر سکیں۔

## Retry اور gossip backoff

- Validators proposal state میں `CollectorPlan` رکھتے ہیں؛ پلان ریکارڈ کرتا ہے کہ کتنے collectors سے رابطہ ہوا اور redundant fan-out limit پہنچا یا نہیں۔
- Collector plans کو `(height, view)` سے key کیا جاتا ہے اور subject بدلتے ہی reinitialize کیا جاتا ہے تاکہ stale view-change retries پرانے collector targets دوبارہ استعمال نہ کریں۔
- Redundant send (`r`) deterministically پلان میں آگے بڑھ کر apply ہوتا ہے۔ جب `(height, view)` کیلئے کوئی collector دستیاب نہ ہو تو votes full commit topology (self کو چھوڑ کر) پر fallback کرتے ہیں تاکہ deadlock سے بچا جا سکے۔
- جب quorum رک جائے تو reschedule path cached votes کو collector plan کے ذریعے rebroadcast کرتا ہے، اور collectors خالی ہوں، صرف local ہوں یا quorum سے کم ہوں تو commit topology پر fallback کرتا ہے۔ یہ bounded "gossip" fallback دیتا ہے بغیر steady-state fast path پر مکمل broadcast کی لاگت دیے۔
- Locked commit certificate gate کی وجہ سے ہر proposal drop پر `block_created_dropped_by_lock_total` بڑھتا ہے؛ header validation کی ناکامیوں سے `block_created_hint_mismatch_total` اور `block_created_proposal_mismatch_total` بڑھتے ہیں، جس سے operators repeated fallbacks کو leader correctness مسائل سے جوڑ سکتے ہیں۔ `/v1/sumeragi/status` snapshot تازہ ترین Highest/Locked commit certificate hashes بھی دکھاتا ہے تاکہ dashboards drop spikes کو مخصوص block hashes سے correlate کر سکیں۔

## Implementation summary

- نیا public module `sumeragi::collectors`، `CollectorPlan` اور `deterministic_collectors` کو host کرتا ہے تاکہ crate-level اور integration tests مکمل consensus actor چلائے بغیر fairness properties verify کر سکیں۔
- `CollectorPlan` Sumeragi proposal state میں رہتا ہے اور proposal pipeline مکمل ہونے پر reset ہو جاتا ہے۔
- `Sumeragi` `init_collector_plan` کے ذریعے collector plans بناتا ہے اور availability/precommit votes emit کرتے وقت collectors کو target کرتا ہے۔ Availability اور precommit votes collectors خالی ہوں، صرف local ہوں یا quorum سے کم ہوں تو commit topology پر fallback کرتے ہیں، اور rebroadcasts بھی انہی حالات میں fallback کرتے ہیں۔
- Unit اور integration tests permissioned rotation، PRF determinism اور backoff state transitions کو validate کرتے ہیں۔

## Review sign-off

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG

</div>
