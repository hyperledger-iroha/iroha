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
- Include mismatch state, drop counters, Highest/Locked QC hashes, and pacemaker backpressure deferrals in `/v1/sumeragi/status` JSON (`gossip_fallback_total`, `block_created_dropped_by_lock_total`, `block_created_hint_mismatch_total`, `block_created_proposal_mismatch_total`, `pacemaker_backpressure_deferrals_total`, QC `subject_block_hash`).

**Status:** Implemented via the gauges `sumeragi_membership_view_hash`, `sumeragi_membership_height`, `sumeragi_membership_view`, `sumeragi_membership_epoch`, and the new `/v1/sumeragi/status.membership` JSON object. These surfaces complement the mismatch counters by exposing the deterministic roster hash and its (height, view, epoch) context per peer.

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
- Detect content type using magic-byte sniffing (fall back to declared MIME). Reject unknown or disallowed formats.
- Enforce decompression/expansion limits before storing on disk. For compressed archives, limit total expanded size and depth of nested archives.
- Execute risky parsers (e.g., PDF, image metadata) in a sandbox (seccomp profile or WASI guest) with read-only inputs and strict CPU/memory limits.
- Sanitize outbound exports: optionally strip active content (scripts/macros) or block unsupported formats when exporting attachments.
- Record provenance metadata (hashes, sniffed type, sanitizer result) alongside stored attachments.

**Observability**
- Prometheus counters:
  - `torii_attachment_reject_total{reason}` (reasons: `type`, `expansion`, `sandbox`, `checksum`).
  - `torii_attachment_sanitize_ms` histogram for sandbox execution time.
- Structured logs at INFO on rejection with attachment ID and reason; DEBUG logs for sandbox diagnostics.

**Deployment considerations**
- Configuration should allow per-format enable/disable and expansion thresholds.
- Fallback path must remain deterministic across nodes; attach sanitized output or reject uniformly.
- Provide integration tests with representative fixture files.
