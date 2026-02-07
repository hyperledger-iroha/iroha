---
lang: kk
direction: ltr
source: docs/source/torii/kaigi_telemetry_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd38b2ed3a941969d403e2ba50d28da325f7bd85966267002a2d77ede6c8e04
source_last_modified: "2026-01-28T17:11:30.745278+00:00"
translation_last_reviewed: 2026-02-07
---

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
| `/v1/kaigi/relays` | GET | `app_api` + `telemetry` | Lists registered relays with their domain, bandwidth class, HPKE fingerprint, and latest health sample. Supports `address_format=ih58\|compressed` (IH58 preferred; compressed (`sora`) is second-best Sora-only) to control how `relay_id` literals are rendered. | `KaigiRelaySummaryListDto` |
| `/v1/kaigi/relays/{relay_id}` | GET | `app_api` + `telemetry` | Returns metadata for a single relay, including base64 HPKE key material, latest health report metadata, and per-domain counters. `address_format=…` mirrors the list handler and also controls the `reported_by` literal. | `KaigiRelayDetailDto` |
| `/v1/kaigi/relays/health` | GET | `app_api` + `telemetry` | Aggregated relay health totals across all domains plus per-domain metrics. | `KaigiRelayHealthSnapshotDto` |
| `/v1/kaigi/relays/events` | GET (SSE) | `app_api` + `telemetry` | Server-Sent Events stream emitting relay registration and health update notifications. | SSE events with JSON payloads (see below) |

> **Address formatting (`ADDR-5`):** Both the list and single-relay endpoints
> accept an optional `address_format` query parameter. The value defaults to
> the preferred `ih58` and may be set to `compressed` to emit second-best
> `sora…` literals in the
> `relay_id` and `reported_by` fields, matching the other Torii `address_format`
> surfaces and the metrics counters that back the Local-8 cutover dashboards.

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

### Rate Limiting and Access Control

- Handlers reuse `limits::rate_limit_key` with route-specific labels
  (`"v1/kaigi/relays"`, `"v1/kaigi/relays/health"`, etc.).
- All routes leverage `MaybeTelemetry`; requests are rejected with
  `TelemetryProfileRestricted` when the active profile disables metrics.
- Endpoints follow the same authentication/routing rules as other `app_api`
  surfaces (API token where configured, CIDR allow-list bypass).

### Metrics Backing

The handlers consume the Kaigi counters exported by
`iroha_telemetry::metrics::Metrics`:

- `kaigi_relay_registered_total`
- `kaigi_relay_manifest_updates_total`
- `kaigi_relay_failover_total`
- `kaigi_relay_health_reports_total`
- `kaigi_relay_health_state`

`prometheus::core::Collector` is used to gather consistent snapshots prior to
rendering the DTOs.

### Tests & Verification

- REST coverage: `crates/iroha_torii/tests/kaigi_endpoints.rs` exercises list,
  detail, and health responses using an in-memory state fixture.
- SSE coverage: `convert_kaigi_event` filters are indirectly exercised by the
  integration test (relay events accepted, non-Kaigi events ignored).
- Router wiring: existing router smoke tests continue to assert feature-gated
  route registration; additional parity checks will be added as SDKs adopt the
  surface.

### Follow-up

- Runtime currently emits registration and health updates. Manifest/failover
  events can be added to the SSE stream once upstream producers expose them.
- Observability dashboards should be updated to reference the new JSON
  endpoints in addition to the Prometheus scrape targets.
