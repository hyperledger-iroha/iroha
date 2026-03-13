---
lang: ur
direction: rtl
source: docs/source/torii_query_cursor_modes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b3c105f09e4aae0d0b6df76ab3a1bfb2173db1c5cc9bce8764a69a449f4da68
source_last_modified: "2026-01-03T18:07:57.386553+00:00"
translation_last_reviewed: 2026-01-30
---

## Torii Query Cursor Modes (Snapshot Lane)

This page documents how server-side queries select cursor behavior when executing over a snapshot of state, and how clients can override it per request.

### Overview

Iroha runs read-only queries against a captured `StateView` snapshot for determinism. Iterable queries can operate in two modes:

- ephemeral: returns only the first batch. The server does not keep a cursor. Clients page by reissuing a new Start request with updated pagination parameters.
- stored: returns the first batch and a server cursor. Clients can continue the same snapshot using the cursor.

Mode selection is configurable and can be overridden per request.

### Configuration

- `pipeline.query_default_cursor_mode`: default cursor mode for server-facing iterable queries. Values: `ephemeral` (default) or `stored`.
- `pipeline.query_stored_min_gas_units`: minimum gas units required to use stored mode (0 disables the requirement). When > 0, stored mode requires the client to provide sufficient `gas_units` in the request.

### Per-request Overrides

The `/query` endpoint accepts optional query-string parameters (reserved for mode control):

- `cursor_mode`: `ephemeral` | `stored`
- `gas_units`: integer; required when `pipeline.query_stored_min_gas_units > 0` and `cursor_mode=stored`. When insufficient, the server rejects the request with a validation error.
- Stored `Continue` requests embed the gas budget in the Norito payload via `ForwardCursor.gas_budget` so the server can re-validate stored cursors.

Notes:
- If `cursor_mode` is omitted, the server uses the default from `pipeline.query_default_cursor_mode`.
- In ephemeral mode, the server always returns `cursor=null`. Continue requests are rejected.

### Telemetry

When telemetry is enabled (`telemetry_enabled=true`):

- `torii_query_snapshot_requests_total{mode}`: total count of snapshot-lane queries by mode.
- `torii_query_snapshot_first_batch_ms{mode}`: first-batch latency histogram by mode (ms).
- `torii_query_snapshot_gas_consumed_units_total{mode}`: cumulative gas units reported by clients for stored mode (when a minimum is configured).
- Core telemetry publishes complementary lane metrics such as `query_snapshot_lane_first_batch_ms{mode}` and `query_snapshot_lane_remaining_items{mode}` for operators who monitor the pipeline directly.

### Determinism and Snapshot Semantics

- All queries execute against a captured `StateView`; live state changes made after capture do not affect an in-flight stored-cursor continuation.
- Ephemeral mode materializes the first batch and returns it. Clients must paginate by issuing new Start requests with updated pagination.

### Examples (Conceptual)

1) Ephemeral default, no override:

POST /v2/query
Body: Norito-encoded Start request (predicate/selector/params)
Response: first batch; `cursor=null`

2) Override to stored with gas units:

POST /v2/query?cursor_mode=stored&gas_units=100
Body: Norito-encoded Start request
Response: first batch; `cursor` present for continuation

If `pipeline.query_stored_min_gas_units=200`, the above is rejected with NotPermitted.

---

For the canonical list of Torii endpoints, see the Reference section. This page covers only mode selection and behavior for snapshot-lane query execution.

