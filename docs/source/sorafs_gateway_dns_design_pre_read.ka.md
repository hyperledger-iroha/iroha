---
lang: ka
direction: ltr
source: docs/source/sorafs_gateway_dns_design_pre_read.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 548a01a269d5b25438c037ec4c0a7a5512163aa4abfe1e8d1cda1e01f3814b68
source_last_modified: "2025-12-29T18:16:36.144051+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway & DNS Kickoff Outcome Brief
summary: Close-out record for the 2025-03-03 kickoff covering deterministic DNS, gateway hardening, GAR enforcement, and SF-5a/SF-5b hand-offs.
---

# SoraFS Gateway & DNS Design Kickoff (2025-03-03) - Outcome Brief

This memo now records the gateway/DNS kickoff outcome requested in
`roadmap.md`. It stays at the original pre-read path because the agenda,
attendance tracker, and runbook link here as the canonical briefing bundle.
Use it as the close-out reference for scope, owners, evidence, and follow-up
tracks rather than as a request for a future meeting.

## Kickoff Record

- **Date / time:** 2025-03-03 @ 16:00 UTC (60 minutes), completed.
- **Facilitator:** Networking TL (agenda owner).
- **Participants:** Networking TLs, Ops leads, Storage Team reps, Tooling WG,
  Governance liaison/delegate, QA Guild, Torii Platform, Security Engineering,
  and Docs/DevRel observers.
- **Evidence bundle:** See
  `docs/source/sorafs_gateway_dns_design_runbook.md` section 9 and
  `docs/source/sorafs_gateway_dns_design_minutes.md`.
- **Success criteria:** Scope, hand-offs, owner assignments, and milestone
  ordering were recorded so implementation and operator documentation could
  proceed from the same evidence set.

## Delivered Outcomes

1. Locked the shared approach for deterministic SoraDNS naming (SF-4) and the
   trustless gateway service (SF-5), including the owner chart in
   `docs/source/sorafs_gateway_dns_design_attendance.md`.
2. Ratified GAR enforcement around the shared manifest/gateway evaluator,
   telemetry, and signed evidence artifacts so policy violations remain
   deterministic and observable across operators.
3. Confirmed SF-5a conformance expectations: fixture replay, negative coverage,
   load evidence, and signed attestation envelopes stay under the gateway
   conformance plan and CI harness.
4. Aligned Torii, admission manifests, DNS automation, TLS/ECH automation, and
   operator runbooks so Docs/DevRel can keep public guidance tied to shipped
   tooling.

## Context & Dependencies

- **Upstream inputs:** `sorafs_manifest` provider adverts (SF-2), chunker
  fixtures (SF-1b), and admission policy tooling (SF-2b/2c) remain the
  canonical data sources for gateway policy and deterministic naming.
- **Downstream consumers:** `sorafs-node`, SDK orchestrators (SF-6b), gateway
  operator tooling, and the Docusaurus portal (`docs/portal/`) consume the
  resulting host plans, GAR artifacts, proof headers, and self-cert bundles.
- **Policy alignment:** GAR manifests and telemetry envelopes wire through
  `sorafs_manifest::gateway` and `iroha_torii`; DNS automation and gateway
  deployments must ingest those artifacts rather than inventing per-operator
  policy schemas.

## Scope Snapshot

### Deterministic DNS & Naming (SF-4/SF-4a)

- Canonical host derivation from manifests, including namespace and capability
  labels such as `cid.gateway.sora` and `anon.gateway.sora`.
- Alias proof caching and TTL policy (`roadmap.md` SF-4a) that keeps proofs
  deterministic and prevents stale GAR envelopes from circulating.
- DNS automation uses the checked-in `cargo xtask soradns-hosts`,
  `soradns-gar-template`, `soradns-binding-template`,
  `soradns-directory-release`, and `soradns-acme-plan` helpers for host
  patterns, GAR templates, binding headers, directory releases, and ACME SAN
  evidence.
- Governance hooks bundle DNS changes with GAR manifest updates so operator
  self-certification remains reproducible.

### Gateway Service Hardening (SF-5/SF-5b)

- Trustless HTTP profile alignment stays in
  `docs/source/sorafs_gateway_profile.md`, covering CAR streams, refusal
  semantics, and telemetry headers.
- Direct-mode operation follows
  `docs/source/sorafs_gateway_direct_mode.md` while SoraNet defaults ramp.
- Rate limiting and denylist hooks remain tied to Norito manifests so
  governance can review policy decisions with signed artifacts.
- Deployment automation is documented in
  `docs/source/sorafs_gateway_deployment_handbook.md` and the TLS/ECH operator
  guide. SF-5b TLS/ECH automation is no longer a future milestone for this
  kickoff record; operators use `scripts/sorafs_gateway_self_cert.sh` and
  `cargo xtask sorafs-gateway-probe` for evidence capture.

## GAR Enforcement Scope

Current status:

- GAR envelope parsing, signature verification, and canonical host matching are
  implemented in `crates/sorafs_manifest/src/gateway.rs`.
- Telemetry and alerting for GAR violations land in
  `crates/iroha_torii/src/sorafs/api.rs` and
  `crates/iroha_telemetry/src/metrics.rs`.
- TLS automation documents enumerate GAR artifacts, ECH behavior, renewal
  flows, and fallback procedures in
  `docs/source/sorafs_gateway_tls_automation.md`.

Resolved decisions:

1. **Runtime policy engine:** gateways use the shared
   `sorafs_manifest::gateway` policy surface and signed Norito artifacts for
   hand-off; no separate operator-local policy schema is introduced.
2. **Configuration surface:** production behavior must come from
   `iroha_config`/operator config bundles, with GAR host patterns, admission
   fallbacks, telemetry sinks, and TLS/ECH settings documented in the gateway
   operator guides.
3. **Violation escalation:** `torii_sorafs_gar_violations_total` and probe
   reports are the escalation inputs for governance/on-call routing, with JSON
   evidence attached to incident tickets.
4. **Audit artifacts:** self-cert archives include manifest signatures, GAR
   envelopes, operator metadata, conformance output, TLS/ECH probe reports, and
   hashes listed in the kickoff evidence bundle.

## Conformance Harness Record (SF-5a)

Reference implementation plan: `docs/source/sorafs_gateway_conformance.md`.

Confirmed harness coverage:

- Replay suites gate canonical fixture manifests through
  `ci/check_sorafs_gateway_conformance.sh`.
- Negative coverage includes unsupported chunkers, malformed proofs, admission
  mismatches, downgrade attempts, and GAR refusal scenarios.
- Load evidence remains part of the acceptance package, with deterministic
  latency thresholds and telemetry capture feeding the signed attestation
  output.
- Attestation verification is handled by the gateway conformance tooling and
  the `cargo xtask sorafs-gateway-attest --verify` path.

## Recorded Decisions

1. Use the SoraDNS `xtask` helpers plus signed RAD/directory evidence as the
   canonical DNS automation baseline.
2. Keep GAR policy decisions in the shared manifest/gateway evaluator and
   signed artifacts consumed by Torii and operators.
3. Treat TLS/ECH as an SF-5b operator-guide concern that is delivered for this
   kickoff scope, including documented fallback and drill procedures.
4. Require self-cert, gateway probe, conformance, and telemetry evidence before
   public operator self-certification.

## Follow-up Tracks

- Anonymous and blinded-CID access still depends on SNNet-2 governance and
  telemetry decisions.
- Legal/compliance retention requirements are tracked in the TLS/ECH guide,
  GAR receipt docs, and governance evidence bundles.
- Devnet, staging, and production rollout evidence must be appended to
  `status.md` and the relevant operator runbooks when each environment cuts
  over.
- Any conformance or probe failure should be filed against the SF-5a workstream
  and linked from the attendance/action tracker.

## Reference Package

- `docs/source/sorafs_gateway_profile.md`
- `docs/source/sorafs_gateway_conformance.md`
- `docs/source/sorafs_gateway_direct_mode.md`
- `docs/source/sorafs_gateway_deployment_handbook.md`
- `docs/source/sorafs_gateway_tls_automation.md`
- `docs/source/sorafs_gateway_dns_design_runbook.md`
- `docs/source/sorafs_gateway_dns_design_minutes.md`
- `crates/sorafs_manifest/src/gateway.rs`
- `crates/iroha_torii/src/sorafs/api.rs`
- `crates/iroha_telemetry/src/metrics.rs`
- `roadmap.md` sections SF-4/SF-5 and Near-Term Execution

## Maintained Runbooks & Evidence

- Use `docs/source/sorafs_gateway_dns_design_runbook.md` for rehearsal,
  evidence upload, command reference, and mock-run snapshot details.
- Keep attendance and owner assignments in
  `docs/source/sorafs_gateway_dns_design_attendance.md`.
- Keep close-out decisions in
  `docs/source/sorafs_gateway_dns_design_minutes.md`.
- Update this outcome brief only when kickoff decisions or current shipped
  tooling change; do not reintroduce pre-meeting logistics here.
