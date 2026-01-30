---
lang: ru
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ur
direction: rtl
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-11-10T16:27:31.384538+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: اے آئی موڈریشن رنر کی وضاحت
summary: وزارتِ اطلاعات (MINFO-1) ڈیلیوریبل کے لئے deterministic موڈریشن کمیٹی کا ڈیزائن۔
---

# اے آئی موڈریشن رنر کی وضاحت

یہ وضاحت **MINFO-1 — Establish AI moderation baseline** کے ڈاکیومنٹیشن حصے کو پورا کرتی ہے۔ یہ وزارتِ اطلاعات کے موڈریشن سروس کے لئے deterministic execution contract بیان کرتی ہے تاکہ ہر gateway اپیل اور شفافیت کے فلو (SFM-4/SFM-4b) سے پہلے ایک جیسے pipelines چلا سکے۔ یہاں بیان کیا گیا تمام رویّہ normative ہے جب تک اسے واضح طور پر informational نہ کہا جائے۔

## 1. اہداف اور دائرہ کار
- ایک ایسا قابلِ تکرار موڈریشن کمیٹی فراہم کرنا جو gateway مواد (objects, manifests, metadata, audio) کو heterogeneous ماڈلز کے ذریعے evaluate کرے۔
- آپریٹرز کے درمیان deterministic execution یقینی بنانا: مقررہ opset، seeded tokenisation، محدود precision، اور versioned artefacts۔
- audit-ready artefacts تیار کرنا: manifests، scorecards، calibration evidence، اور transparency digests جو governance DAG میں شائع کیے جا سکیں۔
- ایسی telemetry مہیا کرنا تاکہ SRE drift، false positives، اور downtime کا پتہ لگا سکیں بغیر raw user data جمع کیے۔

## 2. Deterministic execution contract
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) جسے AVX2 disabled اور `--enable-extended-minimal-build` کے ساتھ compile کیا گیا ہے تاکہ opcode set فکس رہے۔ CUDA/Metal runtimes پروڈکشن میں واضح طور پر ممنوع ہیں۔
- **Opset:** `opset=17`۔ نئے opsets کو target کرنے والے ماڈلز کو admission سے پہلے down-convert اور validate کرنا لازم ہے۔
- **Seed derivation:** ہر evaluation `BLAKE3(content_digest || manifest_id || run_nonce)` سے RNG seed اخذ کرتی ہے جہاں `run_nonce` governance-approved manifest سے آتا ہے۔ seeds تمام stochastic components (beam search، dropout toggles) کو feed کرتی ہیں تاکہ نتائج bit-for-bit reproducible ہوں۔
- **Threading:** ہر ماڈل کے لیے ایک worker۔ runner orchestrator shared-state race conditions سے بچنے کے لیے concurrency کو coordinate کرتا ہے۔ BLAS libraries single-threaded موڈ میں چلتی ہیں۔
- **Numerics:** FP16 accumulation ممنوع ہے۔ FP32 intermediates استعمال کریں اور aggregation سے پہلے outputs کو چار decimal places تک clamp کریں۔

## 3. کمیٹی کی تشکیل
Baseline کمیٹی تین model families پر مشتمل ہے۔ Governance مزید ماڈلز شامل کر سکتی ہے، مگر minimum quorum پورا رہنا چاہیے۔

| Family | Baseline Model | Purpose |
|--------|----------------|---------|
| Vision | OpenCLIP ViT-H/14 (safety fine-tuned) | بصری contraband، تشدد، CSAM indicators detect کرتا ہے۔ |
| Multimodal | LLaVA-1.6 34B Safety | متن + تصویر interactions، contextual cues، اور harassment کو capture کرتا ہے۔ |
| Perceptual | pHash + aHash + NeuralHash-lite ensemble | known bad material کی near-duplicate detection اور recall۔ |

ہر model entry یہ بتاتی ہے:
- `model_id` (UUID)
- `artifact_digest` (OCI image کا BLAKE3-256)
- `weights_digest` (ONNX یا merged safetensors blob کا BLAKE3-256)
- `opset` (لازماً `17`)
- `weight` (committee weight، default `1.0`)
- `critical_labels` (labels کا set جو فوراً `Escalate` trigger کرے)
- `max_eval_ms` (deterministic watchdogs کے لئے guardrail)

## 4. Norito manifests اور نتائج

### 4.1 کمیٹی manifest
```norito
struct AiModerationManifestV1 {
    manifest_id: Uuid,
    issued_at: Timestamp,
    runner_hash: Digest32,
    runtime_version: String,
    models: Vec<AiModerationModelV1>,
    calibration_dataset: DatasetReferenceV1,
    calibration_hash: Digest32,
    thresholds: AiModerationThresholdsV1,
    run_nonce: Digest32,
    governance_signature: Signature,
}

struct AiModerationModelV1 {
    model_id: Uuid,
    family: AiModerationFamilyV1, // vision | multimodal | perceptual | audio
    artifact_digest: Digest32,
    weights_digest: Digest32,
    opset: u8,
    weight: f32,
    critical_labels: Vec<String>,
    max_eval_ms: u32,
}
```

### 4.2 Evaluation result
```norito
struct AiModerationResultV1 {
    manifest_id: Uuid,
    request_id: Uuid,
    content_digest: Digest32,
    content_uri: String,
    content_class: ModerationContentClassV1, // manifest | chunk | metadata | audio
    model_scores: Vec<AiModerationModelScoreV1>,
    combined_score: f32,
    verdict: ModerationVerdictV1, // pass | quarantine | escalate
    executed_at: Timestamp,
    execution_ms: u32,
    runner_hash: Digest32,
    annotations: Option<Vec<String>>,
}

struct AiModerationModelScoreV1 {
    model_id: Uuid,
    score: f32,
    threshold: f32,
    confidence: f32,
    label: Option<String>,
}
```

Runner کو deterministic `AiModerationDigestV1` (serialized result پر BLAKE3) emit کرنا لازم ہے تاکہ transparency logs بن سکیں، اور جب verdict `pass` نہ ہو تو نتائج کو moderation ledger میں append کیا جائے۔

### 4.3 Adversarial corpus manifest

Gateway operators اب ایک companion manifest ingest کرتے ہیں جو calibration runs سے حاصل کردہ perceptual hash/embedding “families” کو enumerate کرتا ہے:

```norito
struct AdversarialCorpusManifestV1 {
    schema_version: u16,                // must equal 1
    issued_at_unix: u64,
    cohort_label: Option<String>,       // e.g. "2026-Q1"
    families: Vec<AdversarialPerceptualFamilyV1>,
}

struct AdversarialPerceptualFamilyV1 {
    family_id: Uuid,
    description: String,
    variants: Vec<AdversarialPerceptualVariantV1>,
}

struct AdversarialPerceptualVariantV1 {
    variant_id: Uuid,
    attack_vector: String,
    reference_cid_b64: Option<String>,
    perceptual_hash: Option<Digest32>,   // Goldilocks hash, BLAKE3 domain separated
    hamming_radius: u8,                  // ≤ 32
    embedding_digest: Option<Digest32>,  // BLAKE3 of quantised embedding vector
    notes: Option<String>,
}
```

Schema `crates/iroha_data_model/src/sorafs/moderation.rs` میں موجود ہے اور `AdversarialCorpusManifestV1::validate()` کے ذریعے validate ہوتا ہے۔ یہ manifest gateway denylist loader کو `perceptual_family` entries بھرنے دیتا ہے جو individual bytes کے بجائے پورے near-duplicate clusters کو block کرتی ہیں۔ ایک runnable fixture (`docs/examples/ai_moderation_perceptual_registry_202602.json`) متوقع layout دکھاتا ہے اور نمونہ gateway denylist کو براہ راست feed کرتا ہے۔

## 5. Execution pipeline
1. Governance DAG سے `AiModerationManifestV1` لوڈ کریں۔ اگر `runner_hash` یا `runtime_version` deployed binary سے mismatch ہوں تو reject کریں۔
2. OCI digest کے ذریعے model artifacts حاصل کریں، لوڈ کرنے سے پہلے digests verify کریں۔
3. Content type کے مطابق evaluation batches بنائیں؛ order `(content_digest, manifest_id)` ہونا چاہیے تاکہ deterministic aggregation ہو۔
4. ہر model کو derived seed کے ساتھ چلائیں۔ perceptual hashes کے لئے ensemble کو majority vote سے combine کریں → score `[0,1]` میں۔
5. `combined_score` میں weighted clipped ratio کے ذریعے aggregation کریں:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` بنائیں:
   - اگر کوئی `critical_labels` fire کرے یا `combined ≥ thresholds.escalate` ہو تو `escalate`.
   - اگر `thresholds.quarantine` سے اوپر مگر `escalate` سے نیچے ہو تو `quarantine`.
   - ورنہ `pass`.
7. `AiModerationResultV1` کو persist کریں اور downstream processes enqueue کریں:
   - Quarantine service (اگر verdict escalate/quarantine ہو)
   - Transparency log writer (`ModerationLedgerV1`)
   - Telemetry exporter

## 6. Calibration اور evaluation
- **Datasets:** baseline calibration policy team کی منظوری والے mixed corpus سے ہوتی ہے۔ `calibration_dataset` میں reference درج ہوتا ہے۔
- **Metrics:** ہر model اور combined verdict کے لئے Brier score، Expected Calibration Error (ECE) اور AUROC نکالیں۔ ماہانہ recalibration میں `Brier ≤ 0.18` اور `ECE ≤ 0.05` برقرار رکھنا لازم ہے۔ نتائج SoraFS reports tree میں محفوظ ہوں (مثلاً [February 2026 calibration](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Schedule:** ماہانہ recalibration (پہلا Monday)۔ drift alerts fire ہونے پر emergency recalibration کی اجازت ہے۔
- **Process:** calibration set پر deterministic evaluation pipeline چلائیں، `thresholds` regenerate کریں، manifest update کریں، اور governance vote کے لئے changes stage کریں۔

## 7. Packaging اور deployment
- OCI images `docker buildx bake -f docker/ai_moderation.hcl` کے ذریعے بنائیں۔
- Images میں شامل ہیں:
  - Locked Python env (`poetry.lock`) یا Rust binary `Cargo.lock`.
  - `models/` directory جس میں hashed ONNX weights ہوں۔
  - Entry point `run_moderation.py` (یا Rust equivalent) جو HTTP/gRPC API expose کرے۔
- Artifacts کو `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>` پر publish کریں۔
- Runner binary `sorafs_ai_runner` crate کا حصہ ہے۔ build pipeline manifest hash کو binary میں embed کرتا ہے ( `/v1/info` سے expose ہوتا ہے )۔

## 8. Telemetry اور observability
- Prometheus metrics:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Logs: JSON lines جن میں `request_id`, `manifest_id`, `verdict` اور stored result کا digest ہو۔ raw scores کو logs میں دو decimal places تک redact کیا جاتا ہے۔
- Dashboards `dashboards/grafana/ministry_moderation_overview.json` میں محفوظ ہیں (پہلی calibration report کے ساتھ publish ہوتے ہیں)۔
- Alert thresholds:
  - Missing ingestion (`moderation_requests_total` 10 منٹ تک stalled رہے)۔
  - Drift detection (average model score delta >20% بمقابلہ rolling 7-day mean)۔
  - False-positive backlog (quarantine queue > 50 items for >30 minutes)۔

## 9. Governance اور change control
- Manifests کے لئے dual signatures درکار ہیں: وزارت کے council member + moderation SRE lead۔ signatures `AiModerationManifestV1.governance_signature` میں درج ہوتی ہیں۔
- Changes `ModerationManifestChangeProposalV1` کے ذریعے Torii میں جاتے ہیں۔ hashes governance DAG میں درج ہوتے ہیں؛ deployment proposal enact ہونے تک block رہتا ہے۔
- Runner binaries میں `runner_hash` embed ہوتا ہے؛ hashes mismatch ہوں تو CI deployment refuse کرتا ہے۔
- Transparency: ہفتہ وار `ModerationScorecardV1` جو volume، verdict mix اور appeal outcomes summarize کرتا ہے۔ Sora Parliament portal پر publish ہوتا ہے۔

## 10. Security اور privacy
- Content digests میں BLAKE3 استعمال ہوتا ہے۔ raw payloads quarantine کے باہر کبھی persist نہیں ہوتے۔
- Quarantine تک رسائی Just-In-Time approvals سے مشروط ہے؛ تمام access log ہوتا ہے۔
- Runner untrusted content کو sandbox کرتا ہے، 512 MiB memory limits اور 120s wall-clock guards نافذ کرتا ہے۔
- Differential privacy یہاں لاگو نہیں؛ gateways quarantine + audit workflows پر بھروسہ کرتے ہیں۔ Redaction policies gateway compliance plan (`docs/source/sorafs_gateway_compliance_plan.md`; portal copy pending) کے مطابق ہیں۔

## 11. Calibration publication (2026-02)
- **Manifest:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  governance-signed `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), dataset reference
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, runner hash
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` اور
  2026-02 calibration thresholds (`quarantine = 0.42`, `escalate = 0.78`) ریکارڈ کرتا ہے۔
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  اور readable report
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  ہر model کے لئے Brier، ECE، AUROC اور verdict mix capture کرتے ہیں۔ combined metrics نے targets meet کیے (`Brier = 0.126`, `ECE = 0.034`).
- **Dashboards & alerts:** `dashboards/grafana/ministry_moderation_overview.json`
  اور `dashboards/alerts/ministry_moderation_rules.yml` (regression tests کے ساتھ
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) rollout کے لئے مطلوب ingest/latency/drift monitoring فراہم کرتے ہیں۔

## 12. Reproducibility schema & validator (MINFO-1b)
- Canonical Norito types اب SoraFS schema کے ساتھ `crates/iroha_data_model/src/sorafs/moderation.rs` میں ہیں۔ `ModerationReproManifestV1`/`ModerationReproBodyV1` structures manifest UUID، runner hash، model digests، threshold set، اور seed material capture کرتے ہیں۔
  `ModerationReproManifestV1::validate` schema version (`MODERATION_REPRO_MANIFEST_VERSION_V1`) enforce کرتا ہے، ہر manifest میں کم از کم ایک model اور signer کی موجودگی یقینی بناتا ہے، اور ہر `SignatureOf<ModerationReproBodyV1>` verify کرتا ہے قبل اس کے کہ machine-readable summary دے۔
- Operators shared validator کو
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  سے invoke کر سکتے ہیں ( `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` میں implement )۔ CLI
  `docs/examples/ai_moderation_calibration_manifest_202602.json` کے JSON artifacts یا raw Norito encoding دونوں قبول کرتی ہے اور validation کے بعد model/signature counts اور manifest timestamp print کرتی ہے۔
- Gateways اور automation اسی helper سے جڑ کر reproducibility manifests کو deterministic طور پر reject کر سکتے ہیں جب schema drift ہو، digests missing ہوں، یا signatures fail ہوں۔
- Adversarial corpus bundles بھی یہی pattern follow کرتے ہیں:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1` parse کرتا ہے، schema version enforce کرتا ہے، اور وہ manifests رد کرتا ہے جو families، variants یا fingerprint metadata چھوڑ دیں۔ کامیاب runs issued-at timestamp، cohort label اور family/variant counts emit کرتی ہیں تاکہ operators section 4.3 میں بیان کردہ gateway denylist entries کو اپ ڈیٹ کرنے سے پہلے evidence pin کر سکیں۔

## 13. Open follow-ups
- 2026-03-02 کے بعد ماہانہ recalibration windows Section 6 کے طریقہ کار پر چلیں گے؛ `ai-moderation-calibration-<YYYYMM>.md` کو updated manifest/scorecard bundles کے ساتھ SoraFS reports tree میں publish کریں۔
- MINFO-1b اور MINFO-1c (reproducibility manifest validators + adversarial corpus registry) roadmap میں الگ سے track ہوتے رہیں گے۔
