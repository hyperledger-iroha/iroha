## Kaigi Relay Telemetry API (TORII-APP-6)

Status: Implemented 2026-04-05  
Owners: Kaigi Team, Torii Platform, Observability  
Roadmap reference: TORII-APP-6 — Kaigi relay telemetry API

This document describes the live Kaigi relay telemetry surface exposed by Torii.
The implementation shipped in `crates/iroha_torii` v2.0.0-rc.2.0 behind the
`app_api` + `telemetry` feature gates. Responses are Norito-backed and mirror
the Prometheus metrics emitted by `iroha_telemetry::metrics::Metrics`.

### Endpoints

| Route | Method | Feature Gate | Description | Response |
|-------|--------|--------------|-------------|----------|
| `/v1/kaigi/relays` | GET | `app_api` + `telemetry` | Operator-signed expensive read listing registered relays with their domain, bandwidth class, HPKE fingerprint, and latest health sample. Emits canonical I105 relay identifiers. | `KaigiRelaySummaryListDto` |
| `/v1/kaigi/relays/{relay_id}` | GET | `app_api` + `telemetry` | Operator-signed bounded lookup for one relay, including base64 HPKE key material, latest health report metadata, and per-domain counters. `reported_by` always uses canonical I105 output, matching the Torii hard-cut account-literal contract. | `KaigiRelayDetailDto` |
| `/v1/kaigi/relays/health` | GET | `app_api` + `telemetry` | Operator-signed expensive aggregation of relay health totals and per-domain metrics. | `KaigiRelayHealthSnapshotDto` |
| `/v1/kaigi/relays/events` | GET (SSE) | `app_api` + `telemetry` | Server-Sent Events stream emitting relay registration and health update notifications. | SSE events with JSON payloads (see below) |

> **Account literals (`ADDR-5`):** The list and single-relay endpoints always return canonical I105 in `relay_id` and `reported_by`, matching the Torii hard-cut account-literal contract and the metrics counters backing Local-8 cutover dashboards.

### Response Schemas

```rust
/// Summary response used by `/v1/kaigi/relays`.
pub struct KaigiRelaySummaryDto {
    pub relay_id: String,
    pub domain: String,
    pub bandwidth_class: u8,
    pub hpke_fingerprint_hex: String,
    pub status: Option<KaigiRelayHealthStatus>,
    pub reported_at_ms: Option<u64>,
}

pub struct KaigiRelaySummaryListDto {
    pub total: u64,
    pub items: Vec<KaigiRelaySummaryDto>,
}

/// Detailed relay view returned by `/v1/kaigi/relays/{relay_id}`.
pub struct KaigiRelayDetailDto {
    pub relay: KaigiRelaySummaryDto,
    pub hpke_public_key_b64: String,
    pub reported_call: Option<KaigiId>,
    pub reported_by: Option<String>,
    pub notes: Option<String>,
    pub metrics: Option<KaigiRelayDomainMetricsDto>,
}

pub struct KaigiRelayDomainMetricsDto {
    pub domain: String,
    pub registrations_total: u64,
    pub manifest_updates_total: u64,
    pub failovers_total: u64,
    pub health_reports_total: u64,
}

/// Health snapshot returned by `/v1/kaigi/relays/health`.
pub struct KaigiRelayHealthSnapshotDto {
    pub healthy_total: u64,
    pub degraded_total: u64,
    pub unavailable_total: u64,
    pub reports_total: u64,
    pub registrations_total: u64,
    pub failovers_total: u64,
    pub domains: Vec<KaigiRelayDomainMetricsDto>,
}
```

All DTOs derive Norito and JSON traits (`norito::derive`, `crate::json_macros`)
so responses stay canonical across transports.

### SSE Payload Shape

`/v1/kaigi/relays/events` reuses Torii's broadcast channel and emits JSON
objects with the following structure (one per SSE event):

- **Registration events** (`kind == "registration"`):
  ```json
  {
    "kind": "registration",
    "domain": "<domain-name>",
    "relay_id": "<account-id>",
    "bandwidth_class": 5,
    "hpke_fingerprint_hex": "<64 hex chars>"
  }
  ```
- **Health events** (`kind == "health"`):
  ```json
  {
    "kind": "health",
    "domain": "<domain-name>",
    "relay_id": "<account-id>",
    "status": "healthy" | "degraded" | "unavailable",
    "reported_at_ms": 1702560000000,
    "call": { "domain": "<domain>", "name": "<call-name>" }
  }
  ```

Query parameters allow optional filtering by `domain`, `relay`, and `kind`
(`registration` or `health`). Unsupported events are dropped with an SSE
comment (`"ignored"`), and filter mismatches yield `"filtered"` comments.

### Diagnostic authentication and bounds

- The three snapshot reads require all four fresh operator-signature headers.
  The signature binds the immutable, exact `NetworkId`, `GET`, encoded path,
  sorted query, empty body hash, timestamp, and nonce. API tokens, legacy auth
  headers, and caller-precomputed operator headers are not substitutes.
- SDK callers dispatch each signed request once with redirects and automatic
  retries disabled. A retry must be a newly signed logical request.
- List and health are catalogued as `ExpensiveCompute`; detail is `ReadOnly`.
  Authentication runs before any relay-world scan or response materialization.
- List and health fail closed with `422` after
  `KAIGI_RELAY_DIAGNOSTIC_MAX_RELAYS` relay records. They use a single bounded
  pass rather than collecting the full registry or every Prometheus label
  series. Detail derives the metadata key and performs direct per-domain lookup.
- Call-signal history uses the core bounded committed-transaction visitor,
  validates `offset + limit` against the canonical fetch budget, retains only
  the requested heap window, and caps retained page JSON at
  `KAIGI_CALL_SIGNALS_MAX_RETAINED_BYTES`.
- The SSE route retains its separate streaming protocol and is not converted to
  snapshot-style operator request signing by this contract.
- All routes still reject with `TelemetryProfileRestricted` when the active
  profile disables metrics.

### Metrics Backing

The handlers consume the Kaigi counters exported by
`iroha_telemetry::metrics::Metrics`:

- `kaigi_relay_registered_total`
- `kaigi_relay_manifest_updates_total`
- `kaigi_relay_manifest_updates_by_domain_total`
- `kaigi_relay_failover_total`
- `kaigi_relay_failovers_by_domain_total`
- `kaigi_relay_health_reports_total`
- `kaigi_relay_health_reports_by_domain_total`
- `kaigi_relay_health_state`

The `*_by_domain_total` counters are updated beside their dimensioned source
counters and expose exactly one label, `domain`. Snapshot handlers read only
those aggregates for the bounded set of active relay domains; they never
collect the unbounded action, call, or status label families. Canonical
on-chain relay feedback supplies health status, avoiding an unbounded clone of
the Prometheus label registry.

### Tests & Verification

- REST coverage: `crates/iroha_torii/tests/kaigi_endpoints.rs` exercises list,
  detail, and health responses using an in-memory state fixture.
- SSE coverage: `convert_kaigi_event` filters are indirectly exercised by the
  integration test (relay events accepted, non-Kaigi events ignored).
- Router/auth coverage verifies missing, wrong-network, wrong-path, and
  wrong-query signatures fail before the handlers run. Python and JavaScript
  transport tests verify generated exact-target signatures and one-shot
  dispatch.

### Follow-up

- Runtime currently emits registration and health updates. Manifest/failover
  events can be added to the SSE stream once upstream producers expose them.
- Observability dashboards should be updated to reference the new JSON
  endpoints in addition to the Prometheus scrape targets.
