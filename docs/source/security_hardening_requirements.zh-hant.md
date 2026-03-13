---
lang: zh-hant
direction: ltr
source: docs/source/security_hardening_requirements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40ef9aad5b28d78988988a96d56333702b37d47660e97b7c5460e9c5f756d1dc
source_last_modified: "2026-01-18T14:14:19.406960+00:00"
translation_last_reviewed: 2026-02-07
---

# Runtime Hardening Requirements

This note captures concrete requirements for closing the open residual risks called out in the threat model. Each section lists scope, functional requirements, observability, and deployment considerations to guide implementation.

## Membership View-Hash Telemetry

**Scope:** Sumeragi consensus nodes must detect and surface mismatches between the locally computed validator roster (height/view) and the roster advertised by peers.

**Requirements**
- Compute a deterministic hash of the validator roster (ordered peer IDs) per `(height, view)` when new consensus parameters are established.
- When a peer advertises consensus parameters or roster hashes that differ from local state, increment a labeled Prometheus counter and emit a structured log message.
- Expose gauges/counters that allow operators to identify persistent mismatches, including labels for `peer_id`, `height`, and `view`.
- Provide a configuration knob for alert thresholds (default trigger: >0 mismatches over a 5-minute window).

**Observability**
- Prometheus metric `sumeragi_membership_mismatch_total{peer,height,view}` (counter).
- Optional gauge `sumeragi_membership_mismatch_active` indicating current peers flagged.
- Include mismatch state, drop counters, Highest/Locked QC hashes, and pacemaker backpressure deferrals in `/v2/sumeragi/status` JSON (`gossip_fallback_total`, `block_created_dropped_by_lock_total`, `block_created_hint_mismatch_total`, `block_created_proposal_mismatch_total`, `pacemaker_backpressure_deferrals_total`, commit certificate `subject_block_hash`).

**Status:** Implemented via the gauges `sumeragi_membership_view_hash`, `sumeragi_membership_height`, `sumeragi_membership_view`, `sumeragi_membership_epoch`, and the new `/v2/sumeragi/status.membership` JSON object. These surfaces complement the mismatch counters by exposing the deterministic roster hash and its (height, view, epoch) context per peer.

**Deployment considerations**
- Metric registration must not break existing telemetry exporters.
- Backfill: upon node restart, counters continue monotonically; active gauge resets to zero.

## Torii Pre-Auth Connection Gating

**Scope:**Torii exposes configuration knobs under `preauth_*` to cap global connections, per-IP slots, rate limits, and bans.  Torii ingress (REST/WebSocket) must reject abusive clients before application-level authentication.

**Requirements**
- Enforce global and per-IP concurrent connection caps configurable via `iroha_config::parameters::actual::Torii`.
- Implement token-bucket handshake rate limiting for TLS/TCP accept and HTTP upgrade (WebSocket), with configurable burst and sustain rates.
- Provide exponential backoff for offending clients; track temporary bans with configurable duration.
- Allow CIDR allowlists that bypass rate limits (for trusted operators or monitoring systems).
- Fail closed: when limits cannot be evaluated (e.g., state exhaustion), deny new connections.

**Observability**
- Prometheus gauges/counters:
  - `torii_pre_auth_reject_total{reason}` (reasons: `ip_cap`, `global_cap`, `rate`, `ban`, `error`).
  - `torii_active_connections_total` with labels `scheme` (http/ws).
- Structured log events at WARN level when a ban is applied.

**Deployment considerations**
- Limits must be hot-reloadable via Torii configuration reload if possible; otherwise document restart requirement.
- Ensure existing tests cover accepted paths; add regression tests for rejections.
- Provide developer override for integration tests (e.g., environment toggle to disable gating).

## Attachment Sanitisation Pipeline

**Scope:** All attachments ingested via Torii (including manifests, proofs, blobs) must undergo format validation to prevent decompression bombs and malicious payloads.

**Requirements**
- Detect content type using magic-byte sniffing and enforce the allowlist from `torii.attachments_allowed_mime_types` before persistence.
- Enforce decompression/expansion limits via `torii.attachments_max_expanded_bytes` and `torii.attachments_max_archive_depth`; only gzip/zstd containers are expanded.
- Run decompression/sanitization in a bounded worker with `torii.attachments_sanitize_timeout_ms`, deterministic byte limits, and a configurable execution mode (`torii.attachments_sanitizer_mode`, default `subprocess`).
- Apply OS-level CPU/memory limits to the sanitizer subprocess (rlimits) to bound resource use.
- Record provenance metadata (hashes, sniffed type, sanitizer verdict) alongside stored attachments.

**Observability**
- Prometheus counters:
  - `torii_attachment_reject_total{reason}` (reasons: `type`, `expansion`, `sandbox`, `checksum`).
  - `torii_attachment_sanitize_ms` histogram for sandbox execution time.
- Structured logs at INFO on rejection with attachment ID and reason; DEBUG logs for sandbox diagnostics.

**Deployment considerations**
- Configuration should allow per-format enable/disable and expansion thresholds.
- Fallback path must remain deterministic across nodes; attach sanitized output or reject uniformly.
- Provide integration tests with representative fixture files.

