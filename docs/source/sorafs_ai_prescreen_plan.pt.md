---
lang: pt
direction: ltr
source: docs/source/sorafs_ai_prescreen_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c814d9430985ebfc67274454334437a4c7f78d6a33e46553d8d0601c168da308
source_last_modified: "2026-01-03T18:07:57.617568+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS AI Pre-screening & Quarantine
summary: SFM-4a implementation status for moderation reproducibility, honey-audit tooling, and remaining quarantine service gates.
---

# SoraFS AI Pre-screening & Quarantine

## Status

SFM-4a defines deterministic AI pre-screening before public SoraFS gateway
publication. The repository currently ships the reproducibility, corpus, local
model-registry admission/checkpointing, local deterministic screening-result
and pending-quarantine evidence checkpointing, gateway-policy, honey-audit, and
observability foundations. It does not yet ship the persistent production
model-registry service, production AI runner service, committee runner,
encrypted quarantine object store, operator review panel, or release workflow
as runnable services.

Implemented locally:

- `crates/iroha_data_model/src/sorafs/moderation.rs` defines
  `ModerationReproManifestV1`, `ModerationReproBodyV1`,
  `ModerationModelFingerprintV1`, `ModerationThresholdsV1`,
  `ModerationReproSignatureV1`, and `AdversarialCorpusManifestV1` with Norito
  encode/decode support and validators for schema version, model coverage,
  duplicate signers, and governance signatures.
- `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  validates governance-signed reproducibility manifests.
- `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  validates adversarial corpus manifests before gateway adoption.
- `sorafs_node::NodeHandle` admits validated moderation reproducibility
  manifests and adversarial corpus manifests into a local model registry,
  rejects conflicting manifest ids, keys corpus manifests by canonical Norito
  BLAKE3 digest, returns deterministic registry snapshots for operator
  plumbing, and checkpoints the registry under the SoraFS data directory when
  storage is enabled.
- Torii exposes the local registry through
  `GET /v1/sorafs/moderation/model-registry?limit=N`,
  `POST /v1/sorafs/moderation/model-registry/repro-manifests`, and
  `POST /v1/sorafs/moderation/model-registry/corpora`; admission requests carry
  base64 canonical Norito manifests and require X-Iroha canonical app
  authentication.
- `sorafs_node::NodeHandle` records deterministic local screening-result
  evidence with BLAKE3 record digests, creates pending local quarantine records
  for `quarantine` and `escalate` verdicts, exports/restores validated
  duplicate-checked snapshots, and checkpoints them under
  `moderation-screening/screening-snapshot.to` when SoraFS storage is enabled.
- Torii exposes the local screening/quarantine evidence surface through
  canonical-authenticated `POST /v1/sorafs/moderation/screening-results`,
  bounded `GET /v1/sorafs/moderation/screening-results?limit=N`, and bounded
  `GET /v1/sorafs/moderation/quarantine?limit=N`.
- `sorafs_cli moderation honey-audit` probes configured gateways with
  denylisted digests and emits JSON/Markdown evidence for policy enforcement.
- `docs/examples/ai_moderation_calibration_manifest_202602.json`,
  `docs/examples/ai_moderation_calibration_scorecard_202602.json`, and
  `docs/examples/ai_moderation_perceptual_registry_202602.json` provide the
  committed calibration and registry fixtures.
- `docs/source/sorafs/reports/ai_moderation_calibration_202602.md` records the
  calibration report, with localized mirrors.
- Gateway Authorization Records support moderation directives and slugs through
  `GarModerationDirectiveV1`, `GarModerationAction`, GAR v2 policy parsing, and
  Torii gateway policy checks.
- Torii CID lookup can report local moderation hits, while gateway fetch policy
  still blocks serving when required moderation directives are absent.
- `dashboards/grafana/ministry_moderation_overview.json` and
  `dashboards/alerts/ministry_moderation_rules.yml` provide the moderation
  ingest, latency, drift, and manifest-health monitoring story.

Not shipped locally:

- A persistent production `ai_model_registry` service.
- A deterministic `sorafs_ai_runner` HTTP/gRPC service.
- An `ai_committee_service` that executes models and emits screening verdicts.
- Encrypted quarantine object storage and review/release queue state.
- `list-quarantine`, `review`, or `release` CLI commands.
- A moderation operator web panel.
- End-to-end ingest -> quarantine -> appeal -> transparency workflow services.

## Target Architecture

The production service remains a staged rollout target:

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Model registry | Stores model artifacts, reproducibility manifests, calibration datasets, and hashes. | Local Torii admission/readback plus node snapshot/checkpoint foundation exists; persistent registry service not shipped. |
| AI runner | Executes approved models deterministically and emits model scores. | Runner contract documented; service not shipped. |
| Committee orchestrator | Aggregates model outputs and yields `pass`, `quarantine`, or `escalate`. | Threshold schema, calibration report, and local screening-result admission exist; orchestrator not shipped. |
| Quarantine store | Stores flagged content and metadata under moderation access controls. | Local pending-quarantine evidence records exist; encrypted object storage and review/release service are not shipped. |
| Moderation bridge | Hands escalations to appeal and transparency workflows. | Appeal finance CLI exists separately; bridge service not shipped. |

## Data Model

The shipped reproducibility manifest records the runner and model fingerprints
that gateways should require before adopting a moderation committee:

```norito
struct ModerationReproManifestV1 {
    body: ModerationReproBodyV1,
    signatures: Vec<ModerationReproSignatureV1>,
}

struct ModerationReproBodyV1 {
    schema_version: u16,
    manifest_id: [u8; 16],
    manifest_digest: Digest32,
    runner_hash: Digest32,
    runtime_version: String,
    issued_at_unix: u64,
    seed_material: ModerationSeedMaterialV1,
    thresholds: ModerationThresholdsV1,
    models: Vec<ModerationModelFingerprintV1>,
    notes: Option<String>,
}

struct ModerationThresholdsV1 {
    quarantine: u16,
    escalate: u16,
}
```

The adversarial corpus manifest records digest families and variants used for
honey-probe and regression testing. These structures are validation artifacts;
they do not execute models or store quarantined payloads.

The local screening checkpoint stores metadata and digests for gateway or
runner-supplied screening outcomes. It does not execute models or persist the
quarantined content bytes; `quarantine` and `escalate` verdicts only enqueue
pending local evidence records until the encrypted object store and review
workflow ship.

## Gateway Policy Integration

GAR v2 carries moderation directives:

- `allow`, `warn`, `quarantine`, and `block` moderation actions.
- Moderation slugs that must be present on gateway requests before content is
  served.
- Torii gateway policy checks that fail closed with `moderation_required` when
  content requires moderation evidence and the request lacks accepted slugs.

Honey-audit tooling exercises this policy path by probing gateway providers with
known denied digests. Operators should archive the emitted JSON/Markdown reports
with governance evidence for the active moderation cohort.

## Operator CLI

Shipped commands:

```bash
cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation validate-repro --manifest docs/examples/ai_moderation_calibration_manifest_202602.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation validate-corpus --manifest docs/examples/ai_moderation_perceptual_registry_202602.json

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  moderation honey-audit \
  --manifest-id=<hex32> \
  --honey=<digest_hex> \
  --provider name=<alias>,provider-id=<hex32>,base-url=<url>,stream-token=<base64>
```

Do not document `list-quarantine`, `review`, or `release` as shipped commands
until the moderation panel and quarantine state service exist.

## Observability

Implemented local observability:

- Calibration and corpus evidence fixtures in `docs/examples/`.
- Calibration report under `docs/source/sorafs/reports/`.
- Grafana dashboard `dashboards/grafana/ministry_moderation_overview.json`.
- Prometheus alert rules and tests under `dashboards/alerts/`.

Reserved production metrics:

- `sorafs_ai_screening_requests_total{verdict}`.
- `sorafs_ai_screening_latency_seconds_bucket`.
- `sorafs_ai_model_score_bucket{model}`.
- `sorafs_ai_quarantine_backlog`.
- `sorafs_ai_manifest_version_current`.

Dashboards may keep reserved panels for production rollout, but release
evidence must distinguish shipped validation/honey-audit tooling and local
screening/quarantine evidence checkpoints from a live AI runner and production
quarantine workflow.

## Remaining Production Gates

- Implement the deterministic runner service with locked model artifacts,
  fixed seeds, and no outbound network except approved artifact sources.
- Implement committee orchestration and threshold aggregation over
  governance-approved manifests.
- Promote local screening/quarantine evidence into the production runner and
  committee workflow with live governance evidence.
- Ship encrypted quarantine storage, review/release queue state, and role-based
  operator access for the quarantined payloads.
- Add operator panel and CLI commands for queue listing, review, release, and
  appeal handoff.
- Append quarantine/escalation evidence to the Governance DAG and transparency
  ledgers.
- Add end-to-end tests covering ingest, quarantine, review, release, appeal, and
  transparency publication.
- Update the portal and OpenAPI/operator docs only after the above commands and
  services exist.

## Rollout Status

Completed local foundations:

- Define governance-signed reproducibility manifests and validators.
- Define adversarial corpus manifests and validators.
- Admit validated reproducibility and corpus manifests into a local model
  registry snapshot and Norito checkpoint.
- Expose canonical-authenticated Torii endpoints for local model registry
  admission and bounded readback.
- Persist deterministic local screening-result and pending-quarantine evidence
  snapshots, with Torii admission/readback endpoints for screening results and
  quarantine records.
- Provide moderation validation CLI commands.
- Provide honey-audit gateway probing.
- Provide calibration fixtures, report, dashboards, and alert rules.
- Wire GAR moderation directives into gateway policy checks.

Remaining rollout work is service implementation and live evidence, not the
local manifest/corpus validators, model-registry admission/checkpoint
foundation, Torii registry endpoints, local screening/quarantine evidence
checkpointing and readback, honey-audit tool, GAR policy plumbing, or
observability fixtures.
