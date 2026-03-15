---
lang: pt
direction: ltr
source: docs/source/sorafs_ai_moderation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 63d74155001c06a1e05f142f2aa73cb74adcd30bde192f5b4724da98c475cad0
source_last_modified: "2026-01-03T18:07:56.908175+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: AI Moderation Runner Specification
summary: Deterministic moderation committee design for the Ministry of Information (MINFO-1) deliverable.
---

# AI Moderation Runner Specification

This specification fulfils the documentation portion of **MINFO-1 — Establish AI
moderation baseline**. It defines the deterministic execution contract for the
Ministry of Information moderation service so every gateway can run identical
pipelines before appeals and transparency flows (SFM-4/SFM-4b). All behaviour
described here is normative unless explicitly marked as informational.

## 1. Goals & Scope
- Provide a reproducible moderation committee that evaluates gateway content
  (objects, manifests, metadata, audio) using heterogeneous models.
- Guarantee deterministic execution across operators: fixed opset, seeded
  tokenisation, bounded precision, and versioned artefacts.
- Produce audit-ready artefacts: manifests, scorecards, calibration evidence,
  and transparency digests suitable for publication in the governance DAG.
- Surface telemetry so SREs can detect drift, false positives, and downtime
  without collecting raw user data.

## 2. Deterministic Execution Contract
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) compiled with AVX2 disabled and
  `--enable-extended-minimal-build` to keep the opcode set fixed. CUDA/Metal
  runtimes are explicitly disallowed in production.
- **Opset:** `opset=17`. Models targeting newer opsets must be down-converted
  and validated before admission.
- **Seed derivation:** Every evaluation derives an RNG seed from
  `BLAKE3(content_digest || manifest_id || run_nonce)` where `run_nonce` comes
  from the governance-approved manifest. Seeds feed all stochastic components
  (beam search, dropout toggles) so results are bit-for-bit reproducible.
- **Threading:** One worker per model. Concurrency is coordinated by the runner
  orchestrator to avoid shared-state race conditions. BLAS libraries operate in
  single-threaded mode.
- **Numerics:** FP16 accumulation is forbidden. Use FP32 intermediates and clamp
  outputs to four decimal places before aggregation.

## 3. Committee Composition
The baseline committee contains three model families. Governance may add
models, but the minimum quorum must remain satisfied.

| Family | Baseline Model | Purpose |
|--------|----------------|---------|
| Vision | OpenCLIP ViT-H/14 (safety fine-tuned) | Detects visual contraband, violence, CSAM indicators. |
| Multimodal | LLaVA-1.6 34B Safety | Captures text + image interactions, contextual cues, harassment. |
| Perceptual | pHash + aHash + NeuralHash-lite ensemble | Fast near-duplicate detection and recall of known bad material. |

Each model entry specifies:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 of OCI image)
- `weights_digest` (BLAKE3-256 of ONNX or merged safetensors blob)
- `opset` (must equal `17`)
- `weight` (committee weight, default `1.0`)
- `critical_labels` (set of labels that immediately trigger `Escalate`)
- `max_eval_ms` (guardrail for deterministic watchdogs)

## 4. Norito Manifests & Results

### 4.1 Committee Manifest
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

### 4.2 Evaluation Result
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

The runner MUST emit a deterministic `AiModerationDigestV1` (BLAKE3 over the
serialized result) for transparency logs and append results to the moderation
ledger when the verdict is not `pass`.

### 4.3 Adversarial Corpus Manifest

Gateway operators now ingest a companion manifest that enumerates perceptual
hash/embedding “families” derived from the calibration runs:

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

The schema lives in `crates/iroha_data_model/src/sorafs/moderation.rs` and is
validated via `AdversarialCorpusManifestV1::validate()`. The manifest allows the
gateway denylist loader to populate `perceptual_family` entries that block
entire near-duplicate clusters instead of individual bytes. A runnable fixture
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) demonstrates
the expected layout and feeds directly into the sample gateway denylist.

## 5. Execution Pipeline
1. Load `AiModerationManifestV1` from the governance DAG. Reject if
   `runner_hash` or `runtime_version` mismatch the deployed binary.
2. Fetch model artefacts via OCI digest, verifying digests before loading.
3. Construct evaluation batches by content type; ordering must sort by
   `(content_digest, manifest_id)` to ensure deterministic aggregation.
4. Execute each model with the derived seed. For perceptual hashes, combine
   the ensemble via majority vote -> score in `[0,1]`.
5. Aggregate scores into `combined_score` using weighted clipped ratio:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. Produce `ModerationVerdictV1`:
   - `escalate` if any `critical_labels` fire or `combined ≥ thresholds.escalate`.
   - `quarantine` if above `thresholds.quarantine` but below `escalate`.
   - `pass` otherwise.
7. Persist `AiModerationResultV1` and enqueue downstream processes:
   - Quarantine service (if verdict escalates/quarantines)
   - Transparency log writer (`ModerationLedgerV1`)
   - Telemetry exporter

## 6. Calibration & Evaluation
- **Datasets:** Baseline calibration uses the mixed corpus curated with policy
  team approval. Reference recorded in `calibration_dataset`.
- **Metrics:** Compute Brier score, Expected Calibration Error (ECE), and AUROC
  per model and combined verdict. Monthly recalibration MUST keep
  `Brier ≤ 0.18` and `ECE ≤ 0.05`. Results stored in
  `docs/source/sorafs/reports/ai_moderation_calibration_<YYYYMM>.md`.
- **Schedule:** Monthly recalibration (first Monday). Emergency recalibration
  allowed if drift alerts fire.
- **Process:** Run deterministic evaluation pipeline on calibration set,
  regenerate `thresholds`, update manifest, stage changes for governance vote.

## 7. Packaging & Deployment
- Build OCI images via `docker buildx bake -f docker/ai_moderation.hcl`.
- Images include:
  - Locked Python env (`poetry.lock`) or Rust binary `Cargo.lock`.
  - `models/` directory with hashed ONNX weights.
  - Entry point `run_moderation.py` (or Rust equivalent) exposing HTTP/gRPC API.
- Publish artefacts to `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- Runner binary ships as part of `sorafs_ai_runner` crate. The build pipeline
  embeds manifest hash in the binary (exposed via `/v2/info`).

## 8. Telemetry & Observability
- Prometheus metrics:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Logs: JSON lines with `request_id`, `manifest_id`, `verdict`, and the digest
  of the stored result. Raw scores are redacted to two decimal places in logs.
- Dashboards stored in `dashboards/grafana/ministry_moderation_overview.json`
  (to be published alongside the first calibration report).
- Alert thresholds:
  - Missing ingestion (`moderation_requests_total` stalled for 10 minutes).
  - Drift detection (average model score delta >20% versus rolling 7-day mean).
  - False-positive backlog (quarantine queue > 50 items for >30 minutes).

## 9. Governance & Change Control
- Manifests require dual signatures: Ministry council member + moderation SRE
  lead. Signatures recorded in `AiModerationManifestV1.governance_signature`.
- Changes follow `ModerationManifestChangeProposalV1` through Torii. Hashes
  entered into the governance DAG; deployment blocked until the proposal is
  enacted.
- Runner binaries embed `runner_hash`; CI refuses deployment if hashes diverge.
- Transparency: weekly `ModerationScorecardV1` summarising volume, verdict mix,
  and appeal outcomes. Published to Sora Parliament portal.

## 10. Security & Privacy
- Content digests use BLAKE3. Raw payloads never persist outside quarantine.
- Access to quarantine requires Just-In-Time approvals; all accesses logged.
- Runner sandboxes untrusted content, enforcing 512 MiB memory limits and 120s
  wall-clock guards.
- Differential privacy is NOT applied here; gateways rely on quarantine + audit
  workflows instead. Redaction policies follow `docs/source/sorafs_gateway_compliance_plan.md`.

## 11. Calibration Publication (2026-02)
- **Manifest:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  records the governance-signed `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), dataset reference
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, runner hash
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`, and the
  2026-02 calibration thresholds (`quarantine = 0.42`, `escalate = 0.78`).
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  plus the human-readable report in
  `docs/source/sorafs/reports/ai_moderation_calibration_202602.md` capture Brier,
  ECE, AUROC, and verdict mix for every model. Combined metrics met the targets
  (`Brier = 0.126`, `ECE = 0.034`).
- **Dashboards & alerts:** `dashboards/grafana/ministry_moderation_overview.json`
  and `dashboards/alerts/ministry_moderation_rules.yml` (with regression tests in
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) provide the
  moderation ingest/latency/drift monitoring story required for rollout.

## 12. Reproducibility schema & validator (MINFO-1b)
- Canonical Norito types now live alongside the rest of the SoraFS schema in
  `crates/iroha_data_model/src/sorafs/moderation.rs`. The
  `ModerationReproManifestV1`/`ModerationReproBodyV1` structs capture the
  manifest UUID, runner hash, model digests, threshold set, and seed material.
  `ModerationReproManifestV1::validate` enforces schema version
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), ensures every manifest carries at
  least one model and signer, and verifies each `SignatureOf<ModerationReproBodyV1>`
  before returning a machine-readable summary.
- Operators can invoke the shared validator via
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (implemented in `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). The CLI
  accepts either the JSON artefacts published under
  `docs/examples/ai_moderation_calibration_manifest_202602.json` or the raw
  Norito encoding and prints the model/signature counts alongside the manifest
  timestamp once validation succeeds.
- Gateways and automation hook into the same helper so reproducibility manifests
  can be rejected deterministically when schemas drift, digests are missing, or
  signatures fail verification.
- Adversarial corpus bundles follow the same pattern:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  parses `AdversarialCorpusManifestV1`, enforces the schema version, and refuses
  manifests that omit families, variants, or fingerprint metadata. Successful
  runs emit the issued-at timestamp, cohort label, and the family/variant counts
  so operators can pin the evidence before updating the gateway denylist entries
  described in Section 4.3.

## 13. Open Follow-Ups
- Monthly recalibration windows after 2026-03-02 continue to follow the
  procedure in Section 6; publish `ai_moderation_calibration_<YYYYMM>.md`
  alongside updated manifest/scorecard bundles.
- MINFO-1b and MINFO-1c (reproducibility manifest validators plus adversarial
  corpus registry) remain tracked separately in the roadmap.

## 14. Governance Wiring & Publication Cadence (MINFO-1c)

The adversarial corpus now has deterministic schemas plus validation tooling,
but publication only counts as complete once governance, cadence, and evidence
storage are wired into the broader SoraFS process. This section codifies that
workflow so every quarterly registry update produces the same artefacts and
sign-offs.

### 14.1 Stakeholders & approvals

- **Research WG** curates new perceptual families/variants, converts them to
  `AdversarialCorpusManifestV1`, and maintains provenance for the raw samples
  referenced by each variant.
- **Ministry council** provides a 2-of-3 governance signature on the manifest,
  confirming the cohort label, issued-at timestamp, and compliance notes.
- **Observability/SRE delegate** co-signs the manifest, attesting that the
  registry aligns with current drift telemetry and that dashboards/alerts (see
  Section&nbsp;8) are green before publication.
- **Documentation lead** mirrors the JSON artefact under
  `docs/examples/ai_moderation_perceptual_registry_<YYYYMM>.json` and registers
  the digest in the transparency log described by
  `sorafs_governance_dag_plan.md`.

All signatures follow the same `SignatureOf<AdversarialCorpusManifestV1>`
structure used by `sorafs_cli moderation validate-corpus`, allowing CI and
gateway automation to block unsigned manifests.

### 14.2 Publication cadence

Quarterly (first Tuesday of Jan/Apr/Jul/Oct, 12:00 UTC) releases ensure gateway
operators can rotate denylists on a predictable schedule without lagging behind
appeals or calibration data. The timeline below must be documented in
`docs/source/sorafs/reports/ai_moderation_calibration_<YYYYMM>.md` alongside the
usual calibration notes:

| Phase | Window | Deliverables |
|-------|--------|--------------|
| Intake freeze | T−14 days | Research WG locks the candidate manifest, archives supporting samples, and posts the draft to the moderation channel. |
| Deterministic dry run | T−7 days | Run `sorafs_cli moderation validate-corpus` against the draft, store the CLI summary in `artifacts/ministry/transparency/<stamp>/corpus_summary.json`, and attach telemetry snapshots proving the new families pass the drift SLOs. |
| Governance vote | T−2 days | Council and Observability delegates sign the manifest, record signatures in the Norito payload, and stage the governance DAG change (per `sorafs_governance_dag_plan.md`). |
| Publication | T±0 | Mirror the JSON + Norito artefacts to SoraFS, update `docs/examples/` with the cohort file name, and circulate the CID + digest to gateway operators. |
| Post-publication check | T+1 day | Observability verifies that denylists imported the new cohort (see Section 14.4) and files a signed note in `docs/source/sorafs/reports/ai_moderation_corpus_<YYYYMM>.md`. |

Emergency updates (e.g., takedown requests) can follow the same checklist with a
shorter intake window, but the governance vote and signed publication artefacts
remain mandatory.

### 14.3 Submission workflow

Use the following deterministic pipeline whenever a new manifest is ready:

1. Stage the candidate JSON under `artifacts/ministry/transparency/<stamp>/`
   (outside the repo) and run:
   ```bash
   cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
     moderation validate-corpus \
     --manifest artifacts/ministry/transparency/2026Q2/corpus_candidate.json
   ```
   Capture the CLI summary output and store it next to the JSON so auditors can
   replay the validation step.
2. Convert the manifest to a Norito payload with the shared tooling inside the
   moderation crate, then collect signatures from the Ministry council and
   SRE/observability delegate. The same bytes (JSON + Norito) are committed under
   `docs/examples/`.
3. Add the new cohort to `docs/examples/ai_moderation_perceptual_registry_<YYYYMM>.json`,
   keeping the previous cohorts in place for historical replay.
4. Publish the CID, digest, and governance metadata using the DAG procedure in
   [sorafs_governance_dag_plan.md](sorafs_governance_dag_plan.md). The signed
   manifest hash is referenced from the transparency log, and the CID is pinned
   via the SoraFS replication policy described in Section&nbsp;8.

### 14.4 Distribution & monitoring

- Gateway teams must refuse manifests whose `issued_at_unix` predates the
  currently adopted cohort and should surface `perceptual_family`/`variant_id`
  pairs in GAR tickets so governance can tie incidents back to published
  registries.
- The moderation observability dashboards
  (`dashboards/grafana/ministry_moderation_overview.json`) now include
  `corpus_cohort_info` and `perceptual_family_hits_total` panels. SRE runbooks
  require capturing a screenshot of those widgets during the T+1 day
  post-publication check.
- Weekly governance digests cite the most recent cohort label, the CID of the
  published manifest, and the intake status for the next release window so the
  Ministry council can confirm cadence compliance without scraping CI logs.
