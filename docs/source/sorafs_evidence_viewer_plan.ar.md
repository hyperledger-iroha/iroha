---
lang: ar
direction: rtl
source: docs/source/sorafs_evidence_viewer_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b7715fd205a4605a9a4ec3a2b90d260eb83035b077f53612e42165a62aa24a5
source_last_modified: "2026-01-04T10:50:53.672367+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Secure Evidence Viewer & Access Logging
summary: SFM-4b3 implementation status for secure evidence viewing; media validation foundations exist, but the moderation evidence viewer service remains unshipped.
---

# Secure Evidence Viewer & Access Logging

## Current Status

SFM-4b3 is not yet shipped as a moderation evidence viewer. The repository has
adjacent evidence metadata schemas, governance evidence export helpers, and a
Taikai media validation harness, but it does not contain the browser viewer,
streaming backend, watermark engine, WebAuthn session flow, or access-log
service required for moderated juror evidence review.

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
- Add end-to-end security tests for unauthorized access, replay, stale URLs,
  audit-log tampering, and watermark metadata mismatch.

## Validation

Existing adjacent checks do not prove evidence-viewer readiness. For now, use
the Taikai harness only for media envelope and telemetry validation:

```sh
cargo test -p sorafs_orchestrator taikai
```

When SFM-4b3 is implemented, add dedicated frontend, backend, and authorization
tests before removing the unshipped-service language from this page.
