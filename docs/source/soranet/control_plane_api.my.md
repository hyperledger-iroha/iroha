---
lang: my
direction: ltr
source: docs/source/soranet/control_plane_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c790b41665774db4fd21f871fbc9bd8c70f588f7f10dcc139d9e9514c330150
source_last_modified: "2025-12-29T18:16:36.185146+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet Control Plane Seeds (SNNet-15D)

Roadmap item **SNNet-15D – Developer & operator surfaces** starts with a
control-plane contract shared by the API, CLI, SDKs, and console. This seed
captures the initial OpenAPI surface, RBAC catalogue, and the CLI/SDK scaffolds
needed to unblock integration work while the data plane and UX mature.

## Scope

This control plane covers:

- Organisation/project lifecycle and service tokens.
- Domains, DNS zones/records, TLS/ECH automation.
- Cache profiles, purge/rate-limit requests, constant-rate class hints.
- WAF policy packs and moderation tokens/receipts.
- Analytics queries/exports and billing meters/statements.
- Full-fidelity audit feeds for GAR/legal evidence and rollback rehearsals.

All payloads are Norito JSON; do not introduce alternative encodings or serde
dependencies. SDKs should use the Norito helpers already present in the
workspace.

## API Schema (OpenAPI 3.1)

The seed OpenAPI document lives at `docs/source/soranet/control_plane_openapi.yaml`
and defines:

- Tenanted base: `X-SN-Org` (required) and optional `X-SN-Project` headers.
- Bearer JWT auth with service-token claim support and explicit scope lists.
- Core resource models: `Org`, `Project`, `Domain`, `DnsZone/RecordSet`,
  `CacheProfile`, `PurgeRequest`, `RateLimitRule`, `WafRuleSet`,
  `AnalyticsQuery/Result`, `BillingStatement`, `AuditEvent`.
- Idempotency via `Idempotency-Key` headers on mutating endpoints.
- RBAC annotations (`x-rbac-scopes`) that align with the catalogue below.

Early consumers should pin to `version: 0.1.0` and treat new fields as additive.
Breaking changes require a new versioned base path.

## RBAC Catalogue

`docs/source/soranet/control_plane_rbac.yaml` lists scopes, roles, and audit
expectations:

- Roles include `org-owner`, `platform-admin`, `domain-admin`, `security-ops`,
  `billing-admin`, and `support-observer`.
- Scopes map to the OpenAPI operations (`domain:write`, `dns:write`,
  `cache:write`, `waf:write`, `analytics:query/export`, `billing:read`,
  `moderation:write/read`, `org:admin/billing/audit/reader`).
- Audit exports must retain 90 days by default and support JSONL/NDJSON/Parquet
  feeds for GAR and security review. Every event records actor, scopes, action,
  target, request id, IP/UA, and result.
Per-tenant policy evaluation now has a minimal guard in `control-plane-mock`
(`X-SN-Org` header required); extend scope enforcement alongside the real
service skeleton by loading `control_plane_rbac.yaml` directly.

## CLI Scaffold (`soranet`)

The CLI will be a thin Rust wrapper (reusing the Norito client stack in
`crates/iroha_cli`) with a persistent profile store and service-token helper.
Initial command map:

- `soranet login --org <id> [--project <id>]` – exchange credentials for a JWT
  and cache it with expiry; supports service tokens.
- `soranet org list|create`
- `soranet project list|create`
- `soranet domain list|add|pause|resume` (options for TLS/ECH modes)
- `soranet dns zone list|import|export` and `dns record upsert`
- `soranet cache profile list|set` and `cache purge --kind path|tag|cid`
- `soranet rate-limit set` and `waf policy apply`
- `soranet analytics query --window 5m|1h|1d --metric <slug> [--cursor ...]`
- `soranet billing meters|statement --period YYYY-MM`
- `soranet audit tail --from <cursor> [--format jsonl|table]`

Each command emits Norito JSON with a deterministic field order suitable for
fixtures. CLI docs will live under `docs/source/soranet/control_plane_cli.md`
once the binary scaffolding lands.

## SDK Scaffolds

All SDKs consume `control_plane_openapi.yaml` with language-appropriate Norito
wrappers and the RBAC scope list baked into client-side validation.

- **JavaScript/TypeScript:** add `ControlPlaneClient` to
  `javascript/iroha_js/src/` that wraps a configurable fetch, exposes strongly
  typed methods per OpenAPI operation, and rejects requests that violate the
  RBAC scope map before issuing network calls.
- The JS and Rust seeds are snapshotted via SHA-256 hashes
  (`javascript/iroha_js/test/controlPlaneSnapshot.test.js`,
  `control-plane-mock` tests); add a Go snapshot harness when the Go SDK lands.
- **Go:** introduce a `controlplane` package (under `sdk/examples` until the
  module is published) that provides context-aware clients with org/project
  headers, idempotency key injection, and retry/backoff helpers.
- **Rust:** extend the `iroha` client surface with a `control_plane` module that
  shares the Norito JSON serializer and supports service-token signing for
  automation tasks; keep transport pluggable so integration tests can use a
  mock server.

OpenAPI snapshot tests are already wired into JS; add Go/Rust snapshot harnesses
as soon as their scaffolds land to prevent contract drift.

## Next Steps

1. Implement the control-plane service skeleton that consumes the OpenAPI +
   RBAC seeds and surfaces mock responses for SDK/CLI integration tests.
2. Add CLI/SDK snapshot fixtures that prove Norito payload parity across
   languages.
3. Publish UI wireframes and accessibility checklist for the console
   onboarding/TLS-ECH wizard and analytics dashboards; see
   `docs/source/soranet/control_plane_console.md`.
