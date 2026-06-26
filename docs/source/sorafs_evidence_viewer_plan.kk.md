---
lang: kk
direction: ltr
source: docs/source/sorafs_evidence_viewer_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b42ba81039228f2ff2ca3b03f68ea368a06640b884cb0ccaf7686990a23d5ddd
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-06-25
---

# Secure Evidence Viewer & Access Logging

## Current Status

SFM-4b3 is not yet shipped as a moderation evidence viewer. The repository has
adjacent evidence metadata schemas, governance evidence export helpers, and a
Taikai media validation harness, but it does not contain the browser viewer,
streaming backend, watermark engine, WebAuthn session flow, or access-log
service required for moderated juror evidence review. The shared SFM-4b
moderation-panel rollout evidence gate now validates a dedicated
`sorafs.moderation_panel.evidence_viewer_canary.v1` artifact for the viewer
boundary, including role-scoped manifests, short-lived URLs, attested sessions,
strict CSP/offline-mode controls, watermark metadata hashing, append-only access
logs, anomaly events, audit digests, legal-hold binding, Governance DAG and
transparency-ledger export coverage, and payload-free digest preimages for the
session manifest, watermark metadata, access log, legal-hold receipt, and
transparency report. It also rejects raw evidence, signed URLs, session tokens,
response bodies, raw access logs, legal-hold receipt payloads, transparency
report payloads, or watermark secrets.
That gate is a promotion blocker for deployed evidence; it does not replace the
missing viewer service.

## Shipped Adjacent Foundations

- `sorafs_cli proof stream --governance-evidence-dir=DIR` writes proof-stream
  summaries and metadata bundles for governance archival.
- SoraFS repair and capacity schemas carry evidence digests, optional evidence
  URIs, media types, and byte sizes for dispute and repair workflows.
- `crates/sorafs_orchestrator/src/bin/taikai_viewer.rs` validates Taikai
  segment envelopes against CAR archives and emits playback, CEK, PQ-health,
  and alert telemetry. It is a media validation harness, not a moderation
  evidence viewer.
- Taikai viewer metrics and dashboards provide a useful model for stream health
  telemetry, but they do not satisfy moderation evidence access controls.
- `scripts/check_sorafs_moderation_panel_rollout_evidence.py` validates the
  SFM-4b evidence-viewer canary as a payload-free deployment gate and rejects
  missing access event coverage, long-lived segment URLs, private viewer
  material, and incomplete watermark/access-log controls.

## Target Runtime Shape

The production moderation evidence viewer still needs these services:

| Component | Responsibility |
|-----------|----------------|
| Viewer frontend | Browser UI for jurors, auditors, and legal reviewers with strict CSP and disabled offline mode. |
| Viewer backend | Authenticates sessions, issues short-lived segment URLs, and binds access to case and role scopes. |
| Watermark engine | Generates per-session visual and optional audio watermarks tied to juror pseudonyms and nonces. |
| Access logger | Writes append-only view, seek, pause, screenshot, download-attempt, and annotation events. |
| Transparency exporter | Publishes anonymized access reports and daily digests to the Governance DAG. |

## Required Session Flow

1. The moderation panel service issues a signed session token for a specific case,
   evidence item, role, and viewer pseudonym.
2. The viewer performs device/user attestation before a session key is created.
3. The backend returns short-lived streaming URLs plus watermark metadata.
4. The frontend records playback and viewer interaction events locally and sends
   them to the access logger.
5. The logger appends events to the case audit trail and exports privacy-safe
   digests for transparency reporting.

No production route should claim support for `/v1/evidence/session`,
`/v1/evidence/manifest`, `/v1/evidence/log`, or `/v1/evidence/audit` until the
service exists and the authorization model is enforced.

Until those routes exist, evidence-viewer rollout review must use the
payload-free SFM-4b canary artifact rather than captured payloads or response
bodies:

```sh
python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example \
  --require-kind evidence_viewer
```

## Remaining Production Gates

- Build the browser evidence viewer with role-scoped case manifests, trauma
  warnings, watermark overlays, and deterministic rendering support.
- Build the streaming backend, short-lived URL signer, session-key workflow, and
  WebAuthn or equivalent attestation path.
- Implement watermark generation and per-session watermark metadata hashing.
- Implement append-only evidence access logging and anomaly events for download
  attempts, screenshots, session expiry, and attestation failures.
- Add retention, erasure, and legal-hold workflows with signed receipts.
- Export anonymized access reports and daily audit digests to the Governance DAG.
- Make the deployed canary publish only digest evidence for session manifests,
  watermark metadata, access logs, legal-hold receipts, and transparency
  reports; raw access logs, legal-hold receipt bodies, and transparency report
  payloads must never enter rollout archives.
- Add end-to-end security tests for unauthorized access, replay, stale URLs,
  audit-log tampering, and watermark metadata mismatch.
- Collect a passing payload-free `evidence_viewer` canary through the SFM-4b
  rollout evidence gate after the viewer service exists.

## Validation

Existing adjacent checks do not prove evidence-viewer readiness. For now, use
the SFM-4b rollout gate for payload-free evidence-viewer promotion checks and
the Taikai harness only for media envelope and telemetry validation:

```sh
python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example \
  --require-kind evidence_viewer
cargo test -p sorafs_orchestrator taikai
```

When SFM-4b3 is implemented, add dedicated frontend, backend, and authorization
tests before removing the unshipped-service language from this page.
