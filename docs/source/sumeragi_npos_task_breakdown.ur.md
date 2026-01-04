---
lang: ur
direction: rtl
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/sumeragi_npos_task_breakdown.md -->

## Sumeragi + NPoS ٹاسک بریک ڈاؤن

یہ نوٹ Phase A روڈ میپ کو چھوٹے انجینئرنگ کاموں میں تقسیم کرتا ہے تاکہ ہم Sumeragi/NPoS کا باقی کام مرحلہ وار مکمل کر سکیں۔ اسٹیٹس نشانیاں یہ convention فالو کرتی ہیں: `✅` مکمل، `⚙️` جاری، `⬜` شروع نہیں ہوا، اور `🧪` ٹیسٹ درکار۔

### A2 - Wire-level میسیج اپنانا
- ✅ Norito `Proposal`/`Vote`/`Qc` ٹائپس کو `BlockMessage` میں نمایاں کیا اور encode/decode round-trips چلائے (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- ✅ پرانے `BlockSigned/BlockCommitted` frames کو gate کیا؛ migration toggle کو ریٹائرمنٹ سے پہلے `false` پر رکھا گیا۔
- ✅ پرانے block messages ٹوگل کرنے والا migration knob ہٹا دیا؛ اب Vote/QC موڈ ہی واحد wire راستہ ہے۔
- ✅ Torii routers، CLI کمانڈز، اور telemetria consumers کو اپ ڈیٹ کیا تاکہ `/v1/sumeragi/*` JSON snapshots کو پرانے block frames پر ترجیح دیں۔
- ✅ Integration coverage نے `/v1/sumeragi/*` endpoints کو صرف Vote/QC pipeline کے ذریعے exercise کیا (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- ✅ فیچر parity اور interop tests مکمل ہونے پر پرانے frames ہٹا دیے گئے۔

### Frame removal پلان
1. ✅ Telemetria اور CI harnesses دونوں پر 72 h کے multi-node soak tests چلائے گئے؛ Torii snapshots نے proposer throughput اور QC formation کو stable دکھایا اور کوئی regression نہ تھی۔
2. ✅ Integration test coverage اب صرف Vote/QC path پر چلتی ہے (`sumeragi_vote_qc_commit.rs`)، جس سے mixed peers پرانے frames کے بغیر consensus تک پہنچتے ہیں۔
3. ✅ Operator docs اور CLI help میں پرانے wire path کا ذکر نہیں رہا؛ troubleshooting guidance اب Vote/QC telemetria کی طرف اشارہ کرتی ہے۔

### A3 - Engine اور pacemaker enforcement
- ✅ `handle_message` میں Lock/HighestQC invariants نافذ کیے گئے (دیکھیں `block_created_header_sanity`).
- ✅ Data-availability tracking delivery ریکارڈ کرتے وقت RBC payload hash کو validate کرتی ہے (`Actor::ensure_block_matches_rbc_payload`) تاکہ mismatched sessions کو delivered نہ سمجھا جائے۔
- ✅ PrecommitQC requirement (`require_precommit_qc`) کو default configs میں wire کیا اور negative tests شامل کیے (default اب `true` ہے؛ tests gated اور opt-out paths کور کرتے ہیں)۔
- ✅ View-wide redundant-send heuristics کو EMA-backed pacemaker controllers سے replace کیا (`aggregator_retry_deadline` اب live EMA سے derive ہوتا ہے اور redundant send deadlines drive کرتا ہے)۔
- ✅ Proposal assembly کو queue backpressure پر gate کیا (`BackpressureGate` اب queue saturated ہونے پر pacemaker روکتا ہے اور status/telemetry کیلئے deferrals ریکارڈ کرتا ہے)۔
- ✅ Availability votes proposal validation کے بعد emit ہوتے ہیں جب DA لازم ہو (local RBC `DELIVER` کا انتظار کیے بغیر)، اور availability evidence کو `AvailabilityQC` کے ذریعے safety proof کے طور پر track کیا جاتا ہے جبکہ commit بغیر انتظار کے جاری رہتا ہے۔ اس سے payload transport اور voting کے بیچ circular waits ختم ہوتے ہیں۔
- ✅ Restart/liveness coverage اب cold-start RBC recovery (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) اور downtime کے بعد pacemaker resume (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`) کو exercise کرتی ہے۔
- ✅ Lock convergence کیلئے deterministic restart/view-change regression tests شامل کیے گئے (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - Collectors اور randomness pipeline
- ✅ Deterministic collector rotation helpers `collectors.rs` میں موجود ہیں۔
- ✅ GA-A4.1 - PRF-backed collector selection اب deterministic seeds اور height/view کو `/status` اور telemetria میں ریکارڈ کرتا ہے؛ VRF refresh hooks commits اور reveals کے بعد context propagate کرتے ہیں۔ مالکان: `@sumeragi-core`۔ Tracker: `project_tracker/npos_sumeragi_phase_a.md` (بند)۔
- ✅ GA-A4.2 - Reveal participation telemetria + CLI inspection commands ظاہر کیے اور Norito manifests اپ ڈیٹ کیے۔ مالکان: `@telemetry-ops`, `@torii-sdk`۔ Tracker: `project_tracker/npos_sumeragi_phase_a.md:6`۔
- ✅ GA-A4.3 - Late-reveal recovery اور zero-participation epoch tests کو `integration_tests/tests/sumeragi_randomness.rs` میں codify کیا (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`)، penalty-clearing telemetria کو exercise کرتے ہوئے۔ مالکان: `@sumeragi-core`۔ Tracker: `project_tracker/npos_sumeragi_phase_a.md:7`۔

### A5 - Joint reconfiguration اور evidence
- ✅ Evidence scaffolding، WSV persistence، اور Norito roundtrips اب double-vote، invalid proposal، invalid QC، اور double exec variants کو deterministic deduplication اور horizon pruning کے ساتھ cover کرتے ہیں (`sumeragi::evidence`)۔
- ✅ GA-A5.1 - Joint-consensus activation (پرانا set commit کرتا ہے، نیا set اگلے block پر activate ہوتا ہے) enforced ہے اور targeted integration coverage شامل ہے۔
- ✅ GA-A5.2 - Slashing/jailing کیلئے governance docs اور CLI flows اپ ڈیٹ کیے گئے، mdBook synchronization tests کے ساتھ جو defaults اور evidence horizon wording کو lock کرتے ہیں۔
- ✅ GA-A5.3 - Negative-path evidence tests (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) اور fuzz fixtures شامل ہوئے اور nightly چلتے ہیں تاکہ Norito roundtrip validation محفوظ رہے۔

### A6 - Tooling، docs، validation
- ✅ RBC telemetria/reporting موجود ہے؛ DA report حقیقی metrics (eviction counters سمیت) پیدا کرتا ہے۔
- ✅ GA-A6.1 - VRF-enabled 4-peer NPoS happy-path test اب CI میں چلتا ہے اور pacemaker/RBC thresholds کو `integration_tests/tests/sumeragi_npos_happy_path.rs` کے ذریعے enforce کرتا ہے۔ مالکان: `@qa-consensus`, `@telemetry-ops`۔ Tracker: `project_tracker/npos_sumeragi_phase_a.md:11`۔
- ✅ GA-A6.2 - NPoS performance baseline (1 s blocks, k=3) کو capture کیا اور `status.md`/operator docs میں reproducible harness seeds + hardware matrix کے ساتھ شائع کیا۔ مالکان: `@performance-lab`, `@telemetry-ops`۔ Report: `docs/source/generated/sumeragi_baseline_report.md`۔ Tracker: `project_tracker/npos_sumeragi_phase_a.md:12`۔ Live run Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) پر `scripts/run_sumeragi_baseline.py` میں درج کمانڈ کے ذریعے ریکارڈ ہوا۔
- ✅ GA-A6.3 - RBC/pacemaker/backpressure instrumentation کیلئے operator troubleshooting guides جاری ہوئیں (`docs/source/telemetry.md:523`); log correlation اب `scripts/sumeragi_backpressure_log_scraper.py` سے ہوتی ہے تاکہ operators pacemaker deferral/missing-availability pairings بغیر manual grep کے نکال سکیں۔ مالکان: `@operator-docs`, `@telemetry-ops`۔ Tracker: `project_tracker/npos_sumeragi_phase_a.md:13`۔
- ✅ RBC store/chunk-loss performance scenarios (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`)، redundant fan-out coverage (`npos_redundant_send_retries_update_metrics`) اور bounded-jitter harness (`npos_pacemaker_jitter_within_band`) شامل کیے گئے تاکہ A6 suite store soft-limit deferrals، deterministic chunk drops، redundant-send telemetria، اور pacemaker jitter bands کو stress میں exercise کرے۔ [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### فوری اگلے اقدامات
1. ✅ Bounded-jitter harness pacemaker jitter metrics کو exercise کرتا ہے (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. ✅ `npos_queue_backpressure_triggers_metrics` میں RBC deferral assertions کو مضبوط کریں، deterministic RBC store pressure کو prime کر کے (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. ✅ `/v1/sumeragi/telemetry` soak کو طویل epochs اور adversarial collectors تک بڑھائیں، snapshots کو Prometheus counters کے ساتھ متعدد heights پر compare کر کے۔ یہ `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors` سے cover ہے۔

اس فہرست کو یہاں ٹریک کرنا `roadmap.md` کو milestones پر فوکس رکھتا ہے جبکہ ٹیم کو ایک live checklist دیتا ہے۔ اپ ڈیٹس آتے ہی اندراجات اپ ڈیٹ کریں (اور تکمیل نشان زد کریں)۔

</div>
