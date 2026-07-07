---
lang: hy
direction: ltr
source: docs/source/sorafs_evidence_viewer_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c532af543f9f4dcd9323f49a2ee1c1c8b88199faae1724c8224b10c170a71fa1
source_last_modified: "2026-07-05T01:58:10.285289+00:00"
translation_last_reviewed: 2026-07-05
source_mtime: 2026-07-05T01:58:10.285289+00:00
---
# Secure Evidence Viewer & Access Logging

## Current Status

SFM-4b3 is not yet shipped as the full browser and streaming moderation
evidence viewer. The repository now contains quarantine-scoped, payload-free
local session manifests and append-only access-event records for encrypted
moderation quarantine objects plus local payload-free daily audit reports that
record `EvidenceAccess` transparency source entries and can be published by a
config-backed Torii runtime scheduler when enabled and a Governance DAG publisher is configured,
alongside adjacent evidence metadata schemas, governance evidence export
helpers, and a Taikai media validation harness. It still does not contain the
browser viewer, streaming backend, short-lived URL signer, watermark engine,
WebAuthn session flow, or deployed rollout evidence required for
moderated juror evidence review. The shared SFM-4b
moderation-panel rollout evidence gate now validates a dedicated
`sorafs.moderation_panel.evidence_viewer_canary.v1` artifact for the viewer
boundary, including role-scoped manifests, short-lived URLs, attested sessions,
strict CSP/offline-mode controls, watermark metadata hashing, append-only access
logs, anomaly events, audit digests, legal-hold binding, Governance DAG and
transparency-ledger export coverage, and payload-free digest preimages for the
session manifest, watermark metadata, access log, legal-hold receipt, and
transparency report. Evidence-viewer canaries also bind `session_count` to the
unique canonical `sessions[].name` inventory, require `attested_session_count`
and `logged_session_count` to match the `sessions[].attested` and
`sessions[].logged` partitions, and reject duplicate session entries before
promotion can report ready. Evidence-viewer canaries also require explicit
audit-log tamper rejection and watermark metadata mismatch rejection before
promotion can report ready. It also rejects raw evidence, signed URLs, session tokens,
response bodies, raw access logs, legal-hold receipt payloads, transparency
report payloads, or watermark secrets.
The moderation-panel rollout summary publishes the payload-free
`valid_evidence_viewer_digest_sets` metadata set for SFM-4b3, and the final
SoraFS aggregate production-readiness gate requires that set to match recognized
`evidence_viewer` artifact fingerprints before reporting ready.
That gate is a promotion blocker for deployed evidence; it does not replace the
missing browser/streaming viewer service.

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
- `crates/sorafs_node` now stores payload-free
  `ModerationEvidenceViewerSessionRecord` and
  `ModerationEvidenceViewerAccessEventRecord` checkpoints for sealed moderation
  quarantine objects. The records bind viewer account, role, purpose,
  attestation digest, watermark metadata digest, evidence digest, legal-hold
  metadata, event kind, request digest, and append-only sequence without
  decrypting or returning evidence bytes.
- Torii exposes the local audit runtime through
  `/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-sessions` and
  `/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-access`. These
  operator-role-gated endpoints reject raw evidence, signed URLs, session
  tokens, response bodies, raw access-log payloads, and watermark secrets.
- `crates/sorafs_node` now derives payload-free
  `ModerationEvidenceViewerAuditReport` daily reports from local session/access
  snapshots. Reports expose only aggregate counts, timestamp bounds, event-kind
  counts, and digest-set hashes for evidence, session manifests, access events,
  request metadata, attestation transcripts, and watermark metadata.
- Torii exposes the local transparency-source exporter through
  `/v1/sorafs/moderation/viewer-audit-reports`. The operator-role-gated
  endpoint records reports as local `EvidenceAccess` transparency source
  entries for later ledger/Governance DAG publication and rejects raw evidence,
  raw access logs, viewer accounts, signed URLs, runtime session tokens, and
  response bodies.
- Torii also exposes
  `/v1/sorafs/moderation/viewer-audit-reports/publish-due`. The
  operator-role-gated endpoint runs one local scheduler tick, uses the
  configured `sorafs.storage.evidence_viewer_audits` cadence when request
  cadence fields are omitted, derives the oldest due payload-free report window
  from local session/access records, records the matching `EvidenceAccess`
  source entry, publishes the matching transparency ledger cycle through the
  configured Governance DAG publisher when present, and suppresses duplicate
  cycle publication.
- Torii now starts a config-backed SFM-4b3 evidence-viewer audit scheduler when
  SoraFS storage is enabled and `sorafs.storage.evidence_viewer_audits` is
  enabled. The background scheduler runs an immediate local tick, catches up
  stale published windows without waiting for another full cadence, records and
  publishes due payload-free `EvidenceAccess` cycles with the default
  `local-daily` scope, omits request-only policy and previous-block metadata,
  and leaves explicit operator replay available through the publish-due route.
- `scripts/check_sorafs_moderation_panel_rollout_evidence.py` validates the
  SFM-4b evidence-viewer canary as a payload-free deployment gate and rejects
  missing access event coverage, long-lived segment URLs, private viewer
  material, and incomplete watermark/access-log controls.
- `scripts/build_sorafs_evidence_viewer_canary.py` builds the payload-free
  `evidence_viewer` canary from reviewed deployment facts, rejects unreviewed
  `--deployment-id` and `--environment` values before checker prevalidation,
  requires every positive control claim explicitly, forces raw
  evidence/session-token/signed URL/watermark-secret/body flags to `false`,
  requires reviewed `moderation-viewer-session-*` `--viewer-session` labels
  whose unique inventory matches `--session-count` and rejects non-production markers,
  emits `role_count`, `security_control_count`, `access_event_kind_count`, and
  `export_target_count` from the reviewed role/control/event/export inventories
  before checker prevalidation, requires explicit audit-log tamper rejection and
  watermark metadata mismatch rejection, rejects duplicate or unknown `--verified-claim`,
  `--role`, `--security-control`, `--access-event-kind`, and `--export-target`
  inputs before any canary JSON is written, validates the generated JSON with the
  same SFM-4b checker contract before writing, and writes atomically. It
  helps operators prepare reviewable canary evidence, but it still does not
  replace the browser viewer, streaming backend, watermark engine, WebAuthn
  session flow, or deployed transparency exporter.

## Target Runtime Shape

The production moderation evidence viewer still needs these services:

| Component | Responsibility |
|-----------|----------------|
| Viewer frontend | Browser UI for jurors, auditors, and legal reviewers with strict CSP and disabled offline mode. |
| Viewer backend | Authenticates sessions, issues short-lived segment URLs, and binds access to case and role scopes. |
| Watermark engine | Generates per-session visual and optional audio watermarks tied to juror pseudonyms and nonces. |
| Access logger | Local payload-free quarantine access records are implemented; frontend instrumentation and deployed service integration still need to feed them. |
| Transparency exporter | Local payload-free daily reports now record `EvidenceAccess` source entries, the operator publish-due route can replay due ticks, and the config-backed Torii scheduler can publish due report cycles to the configured Governance DAG publisher; rollout evidence still needs to prove that deployed path. |

## Required Session Flow

1. The moderation panel service issues a signed session token for a specific case,
   evidence item, role, and viewer pseudonym.
2. The viewer performs device/user attestation before a session key is created.
3. The backend returns short-lived streaming URLs plus watermark metadata.
4. The frontend records playback and viewer interaction events locally and sends
   them to the access logger.
5. The logger appends events to the case audit trail and exports privacy-safe
   digests for transparency reporting. The local SoraFS node can now persist
  payload-free session/access records for sealed quarantine objects, record
  daily payload-free `EvidenceAccess` source entries, and publish due local
  report cycles through the configured Torii
  `sorafs.storage.evidence_viewer_audits` runtime scheduler and Governance DAG publisher, but it does
  not issue streaming URLs or run the browser viewer.

No production route should claim support for `/v1/evidence/session`,
`/v1/evidence/manifest`, `/v1/evidence/log`, or `/v1/evidence/audit` until the
service exists and the authorization model is enforced.

Until those routes exist, evidence-viewer rollout review must use the
payload-free SFM-4b canary artifact rather than captured payloads or response
bodies:

```sh
python3 scripts/build_sorafs_evidence_viewer_canary.py \
  @scripts/examples/sorafs_evidence_viewer_canary.args.example
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
- Connect the browser/streaming service to the local payload-free access logger
  and expand deployed anomaly coverage for download attempts, screenshots,
  session expiry, and attestation failures.
- Add retention, erasure, and legal-hold workflows with signed receipts.
- Operate the configured runtime scheduler in staged and production
  deployments, and collect rollout evidence proving anonymized access reports
  and daily audit digests reach the Governance DAG without payload leakage.
- Make the deployed canary publish only digest evidence for session manifests,
  watermark metadata, access logs, legal-hold receipts, and transparency
  reports; raw access logs, legal-hold receipt bodies, and transparency report
  payloads must never enter rollout archives.
- Use the payload-free `evidence_viewer` canary builder for staged review
  packets so every required role, viewer control, access-event kind, export
  target, digest field, reviewed viewer session label, and positive control
  claim is explicit before the SFM-4b rollout gate runs.
- Add end-to-end security tests for unauthorized access, replay, stale URLs,
  audit-log tampering, and watermark metadata mismatch beyond the payload-free
  canary claims.
- Collect a passing payload-free `evidence_viewer` canary through the SFM-4b
  rollout evidence gate after the viewer service exists.

## Validation

Existing adjacent checks do not prove full evidence-viewer readiness. For now,
use the local runtime tests for payload-free session/access audit records, the
local report/exporter/configured scheduler and publish-due tests for payload-free `EvidenceAccess` source entries,
the SFM-4b rollout gate for payload-free evidence-viewer promotion checks, and
the Taikai harness only for media envelope and telemetry validation:

```sh
cargo test -p sorafs_node moderation_evidence_viewer
cargo test -p iroha_torii moderation_evidence_viewer --features app_api
python3 scripts/build_sorafs_evidence_viewer_canary.py \
  @scripts/examples/sorafs_evidence_viewer_canary.args.example
python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example \
  --require-kind evidence_viewer
cargo test -p sorafs_orchestrator taikai
```

When the full browser/streaming SFM-4b3 viewer is implemented, add dedicated
frontend, backend, and authorization tests before removing the
unshipped-service language from this page.
