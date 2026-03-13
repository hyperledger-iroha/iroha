---
lang: am
direction: ltr
source: docs/source/sorafs_evidence_viewer_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b7715fd205a4605a9a4ec3a2b90d260eb83035b077f53612e42165a62aa24a5
source_last_modified: "2026-01-05T09:28:12.084134+00:00"
translation_last_reviewed: 2026-02-07
title: Secure Evidence Viewer & Access Logging
summary: Final specification for SFM-4b3 covering secure viewing, watermarking, attestation, logging, observability, and rollout.
---

# Secure Evidence Viewer & Access Logging

## Goals & Scope
- Deliver a secure, watermarking-enabled evidence viewer for moderation jurors and legal reviewers that prevents unauthorized downloads and records comprehensive audit trails.
- Support multiple media types (video, audio, image, PDF, text) with deterministic rendering across platforms.
- Integrate with moderation appeals (SFM-4b), compliance reporting (SFM-4c), and privacy requirements.

This specification completes **SFM-4b3 — Secure evidence viewer tooling**.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Viewer frontend (`evidence_viewer_web`) | Browser app delivering streamed evidence, watermark overlays, and access controls. | React/TypeScript, strict CSP, offline disabled. |
| Viewer backend (`evidence_streamer`) | Streams encrypted segments, issues session keys, handles authentication. | Rust service with WebAuthn + token validation. |
| Watermark engine (`watermarkd`) | Generates per-session overlay assets (text, waveform, audio pulses). | Stateless service invoked per session. |
| Access logger (`evidence_auditd`) | Records view events, anomaly detection, retention enforcement. | Write-ahead log + Postgres/Timescale. |
| Transparency exporter | Packages audit events into Norito payloads for Governance DAG and dashboards. | Weekly/monthly reporting cadence. |

## Session Setup & Authentication
- Juror accesses viewer via portal; obtains session token (`SessionTokenV1`) signed by moderation service.
- Viewer performs WebAuthn assertion (FIDO2) to attest device; backend verifies and issues `SessionKeyV1`.
- Gateway issues signed streaming URLs bound to session key (HMAC) valid for 30 seconds.
- CLI for jurors: `sorafs moderation open-case --case <id>` opens viewer with short-lived deep link.
- Access roles: `juror`, `auditor`, `legal`. Each role has RBAC-driven content scope.

## Rendering & Watermarking
- Video/audio: served as MP4/WebM/HLS segments encrypted with AES-GCM. Decryption via WebAssembly worker using session key.
- Images/PDF: rendered via `<canvas>`; PDF rasterized server-side (PDFium) to deterministic PNG frames.
- Text: displayed with inline redaction support; watermarked backgrounds.
- Watermarks:
  - Overlay contains `juror_token`, `case_id`, timestamp, session nonce; rotates position every 5 seconds.
  - Watermark comprised of text + faint QR code encoding session metadata.
  - Optional audio watermark (inaudible frequencies) for video to fingerprint audio capture.
  - Accessibility: high-contrast mode, audio descriptions unaffected.
- Screenshot detection: monitors `visibilitychange`, `screen.orientation`, and optional OS-specific APIs; triggers `screenshot_detected` audit event.
- Download prevention: disable context menu, set headers (`Content-Disposition:inline`, `X-Content-Type-Options:nosniff`), rely on streaming segments + attested session identifiers.

## Evidence Storage & Access Control
- Evidence stored encrypted (per-case keys). Keys managed via KMS + governance multi-sig.
- Viewer backend retrieves segments, decrypts server-side, re-encrypts with session key (per-chunk).
- Access tokens require case-specific scope; manual overrides recorded.
- Legal hold: content flagged for legal retains retention settings override, disabling auto-deletion.

## Logging & Audit Trail
- Audit event schema:
  ```norito
  struct EvidenceAuditEventV1 {
      event_id: Uuid,
      case_id: Uuid,
      viewer_role: ViewerRoleV1,
      juror_token: String,  // pseudonym
      access_type: EvidenceAccessTypeV1, // view|pause|seek|download_attempt|screenshot|annotation
      timestamp: Timestamp,
      duration_ms: Option<u64>,
      ip_prefix: Option<String>,
      user_agent_hash: Option<Digest32>,
      watermark_nonce: Digest32,
      notes: Option<String>,
  }
  ```
- Events stored in append-only Postgres table, replicated to object storage (daily JSONL). Daily digest hashed and recorded in Governance DAG.
- Transparency exporter groups events per week, producing `EvidenceAccessReportV1` with anonymized counts.
- CLI `sorafs evidence audit --case <id>` (authorized roles) exports events.

## Observability & Alerts
- Metrics:
  - `sorafs_evidence_sessions_total{role}`
  - `sorafs_evidence_bandwidth_bytes_total`
  - `sorafs_evidence_watermark_generation_seconds`
  - `sorafs_evidence_screenshot_detected_total`
  - `sorafs_evidence_download_attempt_total`
  - `sorafs_evidence_session_duration_seconds_bucket`
- Alerts:
  - Screenshot/detection spikes.
  - Session attestation failures.
  - Evidence retrieval latency > 2 s (p95).
  - Audit log replication lag > 10 minutes.
  - Watermark generation failure.

## APIs
- `POST /v2/evidence/session` – initiate session; returns signed URLs + overlay metadata.
- `GET /v2/evidence/manifest/{case_id}` – returns list of evidence items user can access.
- `POST /v2/evidence/acknowledge` – juror acknowledges trauma warning.
- `POST /v2/evidence/annotation` – juror adds annotation (retention 90 days).
- `POST /v2/evidence/log` – fallback logging endpoint if client offline (sends pending events).
- `GET /v2/evidence/audit?case_id=` – authorized audit export.
- Strict auth via mTLS + JWT scopes (`evidence.view`, `evidence.audit`).

## Security & Privacy
- Session tokens tied to hardware attestation; repeated failure locks account (requires Ops unlock).
- Watermark data hashed before storage to protect tokens if logs leak.
- Viewer served from unique subdomain (`evidence.sora.net`) with strict CSP and COOP/COEP headers to prevent cross-origin leakage.
- Evidence access uses short-lived signed URLs; any offline copy unusable without session key.
- Retention scheduler automatically purges data per policy, generating `RetentionReportV1`.
- Pseudonymization: juror tokens reversible only by governance through secure service.

## Testing & Rollout
- Unit tests for watermark overlay, encryption/decryption, audit logging.
- Automated screenshot/capture attempts in CI to test detection and logging.
- Load test streaming performance (target: 1080p video with <5% CPU overhead on juror laptops).
- Security assessments: penetration tests, red-team simulation, watermark removal attempts.
- Rollout:
  1. Build staging environment with synthetic evidence; run juror user testing.
  2. Verify logging/retention pipeline; test transparency export.
  3. Integrate with moderation portal; initial content class (non-sensitive).
  4. Expand to sensitive content after trauma guardrails validated.
  5. Document runbooks (`docs/source/sorafs/evidence_viewer_operator.md`) including support resources.

## Implementation Checklist
- [x] Define session/auth flow and APIs.
- [x] Specify watermarking/rendering techniques for media.
- [x] Document audit logging, retention, and transparency exports.
- [x] Capture observability metrics, alerts, and security requirements.
- [x] Outline testing strategy and phased rollout.

With this specification, the moderation platform can deliver a secure evidence viewing experience that balances juror usability with strict compliance and transparency obligations.
