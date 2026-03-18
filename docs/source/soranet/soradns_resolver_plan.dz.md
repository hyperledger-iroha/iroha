---
lang: dz
direction: ltr
source: docs/source/soranet/soradns_resolver_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eaab88eefec935b942923c5a405d92a30c3cfdc4096a3cc1880cabdeee51c6a8
source_last_modified: "2026-01-05T09:28:12.088581+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraDNS Resolver & Authoritative Plan (SNNet-15B)

Roadmap item **SNNet-15B – SoraDNS resolver & authoritative layer** extends the
SoraGlobal gateway program with a privacy-first recursive resolver, managed
authoritative DNS surfaces, and auditable telemetry so browser and SDK traffic
can rely on deterministic name resolution. This plan summarises the scope,
interfaces, and evidence required before the resolver enters the SNNet-15
stage-gate review.

## Objectives

1. Ship a dual-stack recursive resolver that supports DoH/DoT/DoQ with an
   **ODoH preview** and enforces DNSSEC, QNAME minimisation, ECS opt-in, and
   `serve-stale`.
2. Integrate **Zone Root Hash (ZRH)** verification so resolver responses embed
   the Merkle-proof metadata expected by governance and the SoraFS gateway
   ingestion pipeline.
3. Provide lightweight authoritative coverage for managed zones
   (`*.sora.link`, gateway aliases, portal vanity domains) with **SVCB/ECH**
   automation and deterministic NS delegation.
4. Expose a hardened admin surface (gRPC + REST) for zone/record lifecycle,
   Remote Policy Tokens (RPT) attestation, incident runbooks, and telemetry
   queries.
5. Deliver monitoring assets (Prometheus + Grafana) and compliance tests that
   prove the resolver enforces the privacy policy before SNNet-15B can unblock
   the global CDN milestone.

## Architecture

```
        +------------------+       +-----------------+       +---------------------+
        |  Ingress layer   |  -->  | Recursive Core  |  -->  |  ZRH notariser +    |
        |  (DoH/DoT/DoQ,   |       | (cache + policy)|       |  authoritative shim |
        |   ODoH relay)    |       +-----------------+       +---------------------+
        +---------+--------+
                  |
         Admin / Telemetry plane
```

### Protocol ingress

- **Protocol map:** DoH (HTTP/2), DoT (ALPN `dot`), and DoQ (QUIC) share the
  same enforcement path while **ODoH preview** uses a dedicated listener pair
  (relay + target) with deterministic relay keys published alongside the PoP
  descriptor. Each listener enforces mTLS between edge PoPs and the core
  resolver to prevent spoofed traffic, and enforces `resolver.policy.h2_only`
  or `resolver.policy.h3_only` when incident playbooks pin a transport.
- **ODoH path:** ingress decrypts the outer tunnel, attaches an ephemeral
  relay-id, and forwards the blinded inner request to the recursive core over
  QUIC. Responses include a **Verification Receipt** (VR) that binds the relay
  key, blinded client id, and ZRH digest used for the answer so telemetry and
  GAR review can prove the preview path preserved obliviousness.
- **Rate limiting:** token-bucket shapers per client certificate, `CF-Ray`, and
  ASN buckets as described in the SNNet-15 gateway RFC. ODoH previews inherit
  the same buckets and attach a `preview_bucket` tag for dashboards.
- **Telemetry:** `soranet_dns_ingress_requests_total{protocol=...,tls_cipher=...,path=...}`
  and structured access logs (JSON) that redact query names unless the caller
  requests ECS opt-in (see §Privacy Controls). The ODoH path also surfaces
  `soranet_dns_odoh_relays_active` and `soranet_dns_odoh_receipts_total` per
  relay-id.

### Recursive core

- Built on top of the Norito-aware resolver runtime (Rust) with a watchdog that
  rejects behaviour outside the privacy envelope (for example, disabled QNAME
  minimisation).
- **Cache tiers:** in-memory SFQ cache (primary) plus RocksDB-backed cold cache
  for `serve-stale` windows. Stale answers include explicit metadata so SDKs
  and dashboards can highlight fallback behaviour.
- **Policy hooks:** pluggable resolver pipeline that evaluates:
  - DNSSEC validation (RFC 4035) with out-of-band trust-anchor updates.
  - QNAME minimisation per RFC 9156 enabled by default; per-zone overrides are
    recorded in the admin API and emitted as telemetry.
  - ECS only when the ERC request flag is set and the caller has an allow-listed
    profile. ECS answers embed `ecs_scope` metadata for post-processing.
  - Response rewriting for managed Sora domains (for example, toggling between
    IPv4-only vs dual-stack PoPs during incidents).

### ZRH notariser & authoritative shim

- Resolver responses that match managed Sora zones embed the latest Zone Root
  Hash and Merkle proof, allowing clients to verify the DNS tree against the
  SoraFS admission manifest. Proofs are signed with the same Norito payload used
  by the SoraFS publisher so governance can compare resolver output and the
  on-chain anchor.
- Lightweight authoritative instances handle:
  - `*.sora.link` gateway aliases and `docs-preview` vanity names.
  - Automated HTTPS/SVCB/ECH records: the admin API accepts a `tls_profile`
    descriptor (ALPN list, ECH config, fallback endpoints) and renders the
    corresponding record set.
  - Managed NS delegation for tenants that do not run their own authority.

#### Verification receipts & proof chain

1. **Anchor:** SoraFS publisher posts the authoritative bundle (zonefile +
   GAR) and captures the ZRH digest; the digest is recorded in the VR header
   template stored alongside the PoP descriptor.
2. **Proof ingest:** Operators upload the Merkle proof via
   `POST /v1/zones/{zone}/authoritative-proof`; the resolver notariser stores
   it under `artifacts/soradns/proofs/<zone>/` with the VR template.
3. **Response binding:** For any managed zone answer, the resolver emits the
   ZRH digest, VR signature, and (for ODoH) the blinded relay-id so SDKs can
   validate the response offline. The VR payload mirrors the
   `soradns_registry_rfc` structure and is expected to be redacted only when
   the caller has not opted into ECS.
4. **Audit replay:** `scripts/soradns_validation.py ds-validate` uses the
   sample zones to assert DS/DNSKEY coverage matches the expected digest
   lengths, while the EDNS matrix helper captures resolver behaviour expected
   by GAR reviewers.

## Privacy Controls

| Control | Behaviour |
|---------|-----------|
| QNAME minimisation | Enabled by default, per-zone overrides recorded in telemetry; admin API exposes current state per zone. |
| ECS | Disabled unless the client sends `SoraDNS-ECS: opt-in` and presents a token bound to the preview program or operator audit. ECS scope is `0/0` for IPv6 and `0` for IPv4 unless overridden by policy. |
| Serve-stale | Enabled with a 30-minute ceiling; responses include `serve_stale=true` metadata and emit `soranet_dns_serve_stale_total` metrics. |
| Logging | Query names hashed with BLAKE2b using the `soradns.log_salt_epoch`, rotated quarterly. ECS opt-ins record the cleartext name alongside a signed waiver in the request metadata. |
| ODoH relay | Supports Oblivious DoH by accepting encrypted payloads over HTTPS and forwarding them to the recursive core using blinded client IDs. Relays log only the target resolver certificate fingerprint and byte counts. |

## Admin & Telemetry API

The admin plane exposes both gRPC (`soranet.soradns.v1`) and REST/JSON
endpoints so operators can automate rollouts without shelling into the PoP.

### Example REST payloads

```json
POST /v1/zones
{
  "zone": "docs-preview.sora.link.",
  "ttl_seconds": 300,
  "dnssec_mode": "managed",
  "qname_minimisation": "default",
  "records": [
    {"type": "A", "name": "docs-preview", "value": "203.0.113.10"},
    {"type": "HTTPS", "name": "docs-preview", "priority": 1,
     "target": ".", "alpn": ["h2","h3"], "ech_config_id": "alpha-2026-02"}
  ]
}
```

- `POST /v1/zones/{zone}/authoritative-proof`: uploads the Merkle proof bundle
  for the ZRH notariser.
- `GET /v1/resolver/policy`: returns the current privacy posture
  (salt epoch, ECS config, QNAME policy).
- `POST /v1/resolver/waivers`: records ECS/QNAME overrides with signed RPT
  attestations.
- `PATCH /v1/resolver/policy`: toggles enforcement knobs (QNAME minimisation,
  ECS opt-in defaults, serve-stale ceiling, ODoH preview enablement) and
  records the Sigstore client identity for audit exports.
- `GET /v1/telemetry/metrics`: JSON view of the Prometheus snapshot used by
  the dashboards so GAR reviewers can pull metrics without scraping Prometheus.
- `GET /v1/telemetry/qps`: streaming endpoint that emits per-protocol counters,
  cache hit rates, DNSSEC failure counts, BFD state per upstream resolver, and
  latency buckets.

All admin calls require Sigstore-backed mTLS (same CA as SoraFS operators) and
are logged in `docs/source/sorafs_gateway_dns_design_runbook.md` as part of the
operator evidence bundle.

## Observability & Compliance

- **Metrics:** exported via Prometheus with dashboards committed to
  `dashboards/grafana/soradns_resolver.json` (new). Key metrics include:
  - `soranet_dns_queries_total{protocol=...,resolver=...}`
  - `soranet_dns_dnssec_failures_total`
  - `soranet_dns_qname_minimisation_mode` (gauge)
  - `soranet_dns_ecs_opt_in_total`
  - `soranet_dns_zrh_verification_failures_total`
  - `soranet_dns_authoritative_publish_total{zone=...}`
  - `soradns_resolver_dns_queries_total` / `soradns_resolver_dns_failures_total`
    / `soradns_resolver_validation_failures_total` at the resolver edge,
    mirroring the health JSON so governance can scrape proof-age/TTL gauges
    alongside request and validation-failure counters without Prometheus. The
    `/healthz` payload surfaced by the prototype now reports these counters for
    compliance dashboards.
- **Logs:** structured NDJSON entries with hashed query names, ECS flags, policy
  decisions, and RPKI cache status.
- **Alerts:** `dashboards/alerts/soradns_policy_rules.yml` defines:
  - DNSSEC failure rate > 0.5 % over 5 min.
  - Missing ZRH proof for managed zones.
  - Serve-stale ratio > 20 % outside maintenance windows.
  - ODoH relay queue saturation.
- **Evidence:** weekly `status.md` entry summarising QPS, privacy incidents, and
  proof updates. `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` now
  links to the resolver telemetry snapshot so GAR reviewers can audit the same
  data.

## Compliance & Testing Harness

   (`python scripts/soradns_validation.py edns-matrix`) to exercise OPT codes,
   buffer sizes, and TC bit behaviour. Capture results under
   `artifacts/soradns/edns/<date>/matrix.json` with the chosen resolver,
   protocol map, and buffer sizes recorded alongside the response flags.
2. **DNSSEC chain validation:** nightly job fetches DS/key pairs for Sora zones
   and proves validation both through the resolver and directly via `ldns-verify`.
3. **RPKI/Zone Root Hash:** integration test publishes a dummy zone to SoraFS,
   processes the proof via the admin API, and ensures client responses carry the
   expected `zrh_digest`. Record fixtures in
   `fixtures/soradns/zrh_roundtrip/`.
4. **Privacy smoke suite:** automated tests toggle ECS opt-ins, QNAME override
   requests, and ODoH relay flows to prove the resolver emits the expected
   telemetry and strips identifiers when required.
5. **Sample zones & DS coverage:** the sample zone catalog
   (`fixtures/soradns/managed_zones.sample.json`) drives
   `python scripts/soradns_validation.py ds-validate` to flag digest length or
   DS/DNSKEY coverage gaps, emitting `artifacts/soradns/ds_validation.json`
   for GAR review.

Automation: `cargo xtask soranet-pop-template --input fixtures/soranet_pop/sjc01.json --resolver-config artifacts/soradns/resolver.toml --edns-out artifacts/soradns/edns/matrix.json --ds-out artifacts/soradns/ds_validation.json` renders the FRR + resolver templates and runs the EDNS/DS helpers in one shot (uses `scripts/soradns_validation.py` under the hood). Use `--edns-resolver` to point at a staging resolver before attaching the JSON outputs to GAR packets.

## Rollout Checklist

| Step | Description | Evidence |
|------|-------------|----------|
| PoP bootstrap | Generate FRR configs (`cargo xtask soranet-pop-template`) and resolver manifests for each PoP. | `deploy/soranet/frr/` repo + `artifacts/soradns/pop/<name>/resolver.toml`. |
| Staging bake | Run recursive/authoritative nodes in staging, attach Grafana dashboard, and exercise EDNS/DNSSEC suites. | `docs/source/soranet/reports/soradns_staging_bake.md`. |
| GAR review | Submit ZRH proofs and privacy telemetry to GAR reviewers; attach NDJSON logs + dashboards. | `docs/source/soranet/gar_compliance_playbook.md` appendix. |
| Operator training | Deliver runbook briefing using `docs/source/sorafs_gateway_dns_design_runbook.md` plus this plan; capture attendance/quiz metrics. | `docs/source/sorafs_gateway_dns_design_attendance.md`. |
 | Production enablement | Flip traffic via the gateway CDN change tickets, enable alerts, and log the adoption date in `status.md`. | `docs/source/project_tracker/soradns_rollout/2026Q4.md`. |

## Next Actions

1. Promote `fixtures/soradns/managed_zones.sample.json` to a formal JSON schema
   and keep the sample catalog regenerated whenever PoP descriptors change.
2. Extend `cargo xtask soranet-pop-template` with a `--render-resolver`
   subcommand and a hook that invokes `python scripts/soradns_validation.py
   edns-matrix` so PoP manifests ship with fresh resolver evidence.
3. Wire the telemetry exporter into the shared observability stack and surface
   the ODoH/VR gauges in Grafana once the first staging bake exercises the new
   receipt flow.

Documenting these expectations keeps SNNet-15B accountable and gives operators
a single reference for policy, configuration, and evidence before the SoraDNS
resolver becomes a gating dependency for the SoraGlobal gateway CDN.
