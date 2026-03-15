---
lang: az
direction: ltr
source: docs/source/sorafs_ai_prescreen_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c814d9430985ebfc67274454334437a4c7f78d6a33e46553d8d0601c168da308
source_last_modified: "2025-12-29T18:16:36.132906+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS AI Pre-screening & Quarantine
summary: Final specification for SFM-4a covering reproducible AI committees, manifests, quarantine flows, observability, and rollout.
---

# SoraFS AI Pre-screening & Quarantine

## Goals & Scope
- Deploy deterministic, reproducible AI committees that screen content before it reaches public gateways, flagging or quarantining potentially non-compliant material.
- Ensure model packaging, execution, and calibration are open-weight, auditable, and reproducible, with cryptographic manifests linked to governance.
- Provide quarantine storage, escalation pathways, and operator tooling to handle detections responsibly.
- Integrate screening decisions with gateway compliance (SFM-4) and moderation appeal systems (SFM-4b).

This specification completes **SFM-4a — AI pre-screening & quarantine service**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Model registry (`ai_model_registry`) | Stores model artefacts, manifests, calibration datasets, hashes. | Backed by S3/IPFS; manifests recorded in Governance DAG. |
| AI runner (`sorafs_ai_runner`) | Executes models deterministically (CPU-only) with fixed seeds, returns structured scores. | Rust/ONNX-based service; gRPC/HTTP interface. |
| Committee orchestrator (`ai_committee_service`) | Dispatches content to models, aggregates scores, issues verdicts, logs results. | Stateless pods with deterministic batching. |
| Quarantine store (`quarantine_store`) | Secure storage for flagged content and metadata; handles retention & access controls. | Encrypted object storage + metadata DB. |
| Escalation workflow (`moderation_bridge`) | Feeds flagged items to moderation appeal panel tooling (SFM-4b) and compliance modules. | Emits events via Torii + queue. |

### Data Flow
1. Content (manifests, chunks, metadata) enters screening pipeline prior to gateway publication.
2. Orchestrator fetches active `AiCommitteeManifestV1`, sends content to AI runner, collects `AiScreeningResultV1`.
3. Based on combined score vs thresholds, the orchestrator yields verdict:
   - `Pass` → content proceeds to gateway pipeline.
   - `Quarantine` → content stored in encrypted quarantine bucket, proof token generated, operator notified.
   - `Escalate` → triggers immediate manual review via moderation tooling.
4. Results appended to Governance DAG if quarantine/escalation occurs, preserving audit trail.
5. Operators use dashboards/CLI to review queue, annotate decisions, integrate with appeals.

## Model Packaging & Execution
- **Initial committee models**:
  - Vision: `OpenCLIP ViT-H/14` with moderation fine-tuning.
  - Multimodal: `LLaVA-1.6 34B` safety-tuned.
  - Perceptual hash: `pHash` + `aHash` + custom `neuralhash-lite`.
  - Optional audio pipeline: `Whisper-medium` (transcription only) feeding text classifier.
- **Packaging standards**:
  - Each model packaged in OCI image built with `Dockerfile.ai`:
    - CPU-only (OpenBLAS/MKL) with deterministic seeds.
    - Hash-locked dependencies (`requirements.lock`, `Cargo.lock`).
    - BLAKE3 digest recorded in manifest.
  - Images pushed to `registry.sora.net/sorafs/ai/<model>@sha256:<digest>`.
- **AI runner**:
  - Accepts `AiScreeningRequestV1 { manifest_id, content_uri, content_hash, content_type }`.
  - Executes models sequentially or in deterministic order; seeds derived from content hash.
  - Outputs `ModelScoreV1 { model_name, score, confidence, threshold }`.
  - Runner exposes `/v2/health` and `/v2/metrics`.
- **Committee composition**:
  - Determined by `AiCommitteeManifestV1`.
  - Supports weightings per model and dynamic threshold adjustments.
  - Manifest stored in Governance DAG (payload `AiCommitteeManifestNode`).

## Manifests & Results
- **Reproducibility manifest** (unchanged structure but clarified usage):
  ```norito
  struct AiCommitteeManifestV1 {
      manifest_id: Uuid,
      issued_at: Timestamp,
      models: Vec<AiModelEntryV1>,
      calibration_dataset: DatasetReferenceV1,
      calibration_hash: Digest32,
      runner_hash: Digest32,
      config_version: String,
      governance_signature: Signature,
  }
  ```
- **Screening result**:
  ```norito
  struct AiScreeningResultV1 {
      manifest_id: Uuid,
      content_digest: Digest32,
      content_uri: String,
      request_id: Uuid,
      model_scores: Vec<ModelScoreV1>,
      combined_score: f32,
      verdict: ScreeningVerdictV1, // Pass | Quarantine | Escalate
      executed_at: Timestamp,
      annotations: Option<Vec<String>>,
  }
  ```
- Results stored in Postgres (`ai_screening_results`), with `verdict` indexed for quarantine.
- Quarantine events generate `AiQuarantineEventV1` for Governance DAG.

## Thresholds & Decision Logic
- Combined score computed via weighted max or logistic combination:
  ```
  combined = Σ w_model * clamp(score / threshold, 0, 1) / Σ w_model
  ```
- Default thresholds:
  - `Pass` if combined < 0.5.
  - `Quarantine` if 0.5 ≤ combined < 0.8 (requires manual release).
  - `Escalate` if ≥ 0.8 or any model flagged critical label (e.g., child sexual abuse material).
- Governance-approved configuration stored in `ai_committee_config.toml`.
- Operators can temporarily adjust thresholds (within governance-specified bounds) via change request logged in audit DB.

## Quarantine & Escalation
- **Quarantine storage**:
  - Encrypted bucket `s3://sorafs-quarantine/<date>/<content_digest>` with server-side KMS, access limited to moderation roles.
  - Metadata stored in Postgres `quarantine_items` table referencing `AiScreeningResultV1`.
  - Retention: default 90 days (configurable). Items auto-deleted after release + retention window.
- **Operator tools**:
  - CLI `sorafs moderation list-quarantine`, `... review <content_digest>`, `... release <content_digest>`.
  - Web UI integrated with moderation panel (SFM-4b) for annotation, appeal decisions.
- **Escalation**:
  - `Escalate` verdict pushes item into moderation appeal queue with `priority = critical`.
  - Notification via PagerDuty/Slack for on-call moderation team.
  - Appeal decision recorded as `ModerationDecisionV1` referencing AI result.

## Observability & Alerts
- Metrics:
  - `sorafs_ai_screening_requests_total{verdict}`
  - `sorafs_ai_screening_latency_seconds_bucket`
  - `sorafs_ai_model_score_bucket{model}`
  - `sorafs_ai_quarantine_backlog`
  - `sorafs_ai_manifest_version_current`
- Logs: structured `ai_screening_event` (request_id, manifest_id, verdict, scores).
- Alerts:
  - Screening service down or error rate > 1%.
  - Quarantine backlog > 100 items.
  - Model manifest mismatch or unsigned manifest.
  - Runner latency p95 > 3 s.
  - False positive rate from appeals > threshold (requires manual review).

## Security & Privacy
- All content hashes and manifests hashed with BLAKE3; sensitive data stored encrypted.
- Access control: RBAC; only moderation roles can view quarantined content; operations limited to metadata.
- AI runner uses sandboxed environment (seccomp, AppArmor) with no outbound network except required endpoints.
- Audit logs hashed daily and stored in Governance DAG.
- Differential privacy optional for aggregated metrics released publicly.

## Testing & Reproducibility
- Unit tests for runner determinism (same input → same score).
- Regression tests verifying threshold logic with calibration dataset.
- Integration tests with synthetic content verifying pipeline (ingest → quarantine → release).
- Calibration tests produce reproducibility report comparing new vs previous committee performance.
- Chaos scenarios: disable model, validate fallback (degraded mode uses remaining models + raises alert).
- Fuzz tests for malformed content metadata.

## Rollout Plan
1. Package initial model set with manifests; publish `AiCommitteeManifestV1` via governance.
2. Deploy staging pipeline with synthetic dataset; confirm determinism across runs.
3. Integrate with gateway compliance (blocklist) in passive mode (log-only) for 2 weeks.
4. Enable quarantine in staging; run operator exercises for release workflow.
5. Production rollout:
   - Stage 0: passive monitoring (no blocking) while logging outputs.
   - Stage 1: enable quarantine for high-confidence detections (Critical categories).
   - Stage 2: expand coverage (additional categories), integrate appeals and transparency reporting.
6. Document SOP (`docs/source/sorafs/ai_prescreen_operator.md`), update compliance plan references, and record status update.

## Implementation Checklist
- [x] Define model packaging, manifests, and runner interfaces.
- [x] Specify committee orchestration, thresholds, and governance controls.
- [x] Document quarantine storage, operator tooling, and escalation.
- [x] Capture observability metrics, alerts, and logging.
- [x] Outline security/privacy safeguards.
- [x] Detail testing, reproducibility, and rollout sequence.

With this specification, AI Platform and operations teams can implement the AI pre-screening and quarantine pipeline confidently, ensuring compliant, reproducible moderation support for SoraFS gateways.
