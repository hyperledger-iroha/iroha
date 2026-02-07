---
lang: ba
direction: ltr
source: docs/source/sorafs_gateway_dns_design_pre_read.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 548a01a269d5b25438c037ec4c0a7a5512163aa4abfe1e8d1cda1e01f3814b68
source_last_modified: "2025-12-29T18:16:36.144051+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway & DNS Design Kickoff Pre-read
summary: Briefing deck for the 2025-03-03 kickoff covering deterministic DNS, gateway hardening, GAR enforcement, and the SF-5a conformance harness.
---

# SoraFS Gateway & DNS Design Kickoff (2025-03-03) — Pre-read

This memo frames the gateway/DNS design kickoff requested in `roadmap.md`.
Stakeholders should review the scope alignment, GAR enforcement guardrails,
and conformance test expectations before the live discussion so we can spend
the session on decision-making rather than status syncing.

## Meeting Logistics
- **Date / time:** 2025-03-03 @ 16:00 UTC (60 minutes). Adjust as needed once
  the full attendee list confirms availability.
- **Facilitator:** Networking TL (agenda owner).
- **Expected participants:** Networking TLs, Ops leads, Storage Team reps,
  Tooling WG, Governance liaison, QA Guild (conformance harness maintainers),
  and Docs/DevRel observers for downstream playbooks.
- **Pre-read bundle:** This note plus the normative artefacts listed under
  [Reference Package](#reference-package).
- **Success criteria:** Agreement on scope, hand-offs, and milestone ordering
  so implementation can begin the moment SF-2 exits rollout review.

## Objectives for the Kickoff
1. Lock the shared vision for deterministic SoraDNS naming (SF-4) and the
   trustless gateway service (SF-5), including sequencing and owner chart.
2. Ratify the GAR enforcement MVP so gateway policy violations remain
   deterministic and observable across all operators.
3. Validate the SF-5a conformance harness coverage plan and confirm the QA
   Guild’s resourcing for replay, negative, and load suites.
4. Capture the integration plan for Torii, admission manifests, and operator
  runbooks so Docs/DevRel can stage the updates alongside the code changes.

## Context & Dependencies
- **Upstream inputs:** `sorafs_manifest` provider adverts (SF-2), chunker
  fixtures (SF-1b), and admission policy tooling (SF-2b/2c) are the canonical
  data sources for gateway policy and deterministic naming.
- **Downstream consumers:** `sorafs-node`, SDK orchestrators (SF-6b), gateway
  operator tooling, and the Docusaurus portal (`docs/portal/`) depend on the
  decisions from this kickoff.
- **Policy alignment:** GAR manifests and telemetry envelopes already wire
  through `sorafs_manifest::gateway` and `iroha_torii` (see references below);
  the kickoff must decide how DNS automation and gateway deployments ingest the
  same artefacts without bespoke per-operator overrides.

## Scope Snapshot

### Deterministic DNS & Naming (SF-4/SF-4a)
- Canonical host derivation from manifests, including namespace and capability
  labels (e.g., `cid.gateway.sora` vs `anon.gateway.sora`).
- Alias proof caching and TTL policy (`roadmap.md` SF-4a) that keeps proofs
  deterministic and prevents stale GAR envelopes from circulating.
- DNS automation expectations: ACME DNS-01 integration for wildcard issuance,
  GAR-aware canonical TXT records, and audit logging for proof rotations.
- Governance hooks for bundling DNS changes with GAR manifest updates so the
  operator self-certification flow stays reproducible.

### Gateway Service Hardening (SF-5)
- Trustless HTTP profile alignment (see `docs/source/sorafs_gateway_profile.md`)
  covering CAR streams, refusal semantics, and telemetry headers.
- Direct-mode operation to respect manifest capability checks while SoraNet
  defaults ramp (refer to `docs/source/sorafs_gateway_direct_mode.md`).
- Rate limiting and denylist hooks tied to Norito manifests so governance can
  review every policy decision with signed artefacts.
- Deployment automation baseline (`docs/source/sorafs_gateway_deployment_handbook.md`)
  to ensure every gateway exposes the same surface: `torii_sorafs_gar_violations_total`
  metrics, `Sora-TLS-State` telemetry, and GAR attestation bundles.

## GAR Enforcement Scope (Kickoff Draft)
Current status:
- GAR envelope parsing, signature verification, and canonical host matching are
  implemented in `crates/sorafs_manifest/src/gateway.rs`.
- Telemetry and alerting for GAR violations land in
  `crates/iroha_torii/src/sorafs/api.rs` and `crates/iroha_telemetry/src/metrics.rs`.
- TLS automation documents enumerate GAR artefacts and renewal flows
  (`docs/source/sorafs_gateway_tls_automation.md`).

Discussion items:
1. **Runtime policy engine:** finalise whether gateways link directly against
   `sorafs_manifest::gateway::Evaluator` or consume a Norito-signed policy cache
   refreshed via Torii.
2. **Configuration surface:** confirm the `iroha_config` knobs that expose
   GAR enforcement (host patterns, admission fallbacks, telemetry sinks) and
   map them to operator overrides without weakening determinism.
3. **Violation escalation:** agree on the minimum viable automation for routing
   `torii_sorafs_gar_violations_total` alerts into governance, including the
   JSON payload schema operators must attach to incident tickets.
4. **Audit artefacts:** settle on the archive structure for GAR attestations
   (`manifest_signatures.json`, operator envelope, conformance output) so Docs
   can incorporate it into the gateway self-cert workflow.

## Conformance Harness Scope Check (SF-5a)
Reference implementation plan: `docs/source/sorafs_gateway_conformance.md`.

Key confirmations needed during kickoff:
- Replay suite must gate _every_ manifest in the canonical fixtures bundle and
  is invoked via `ci/check_sorafs_gateway_conformance.sh` (already scripted).
- Negative coverage includes unsupported chunkers, malformed proofs, admission
  mismatches, downgrade attempts, and GAR refusal scenarios; QA Guild to affirm
  ownership of ongoing fixture curation.
- Load harness target: ≥1,000 concurrent streams (success and refusal mix) with
  deterministic latency thresholds and telemetry capture that feeds into the
  signed attestation output.
- Attestation pipeline: ensure the Norito envelope flow described in
  `sorafs_gateway_conformance.md` aligns with governance requirements and
  document how operators submit reports via `sorafs-gateway-cert`.

## Decision Points for the Kickoff
1. Pick the canonical DNS automation stack (Terraform + RFC2136 signer vs Torii
   managed) and define the escrow process for GAR/DNS artefacts.
2. Approve the GAR policy engine integration approach and associated config
   surfaces.
3. Validate the conformance harness acceptance criteria, sign-off owners, and
   CI entry-points.
4. Sequence milestones: DNS automation MVP, gateway policy enforcement, public
   conformance run, operator self-cert launch.

## Open Questions to Resolve
- Do we require ECH support in the MVP, or can it remain behind the SF-5b
  milestone already delivered in TLS automation?
- What additional telemetry or governance hooks are needed for anonymous /
  blinded-CID access (ties into SNNet-2 roadmap items)?
- Are further legal/compliance sign-offs needed for GAR enforcement logs before
  we publish operator templates?
- Which environments (devnet, staging, prod) must adopt the deterministic DNS
  flow before operators can onboard to the gateway beta?

## Reference Package
- `docs/source/sorafs_gateway_profile.md`
- `docs/source/sorafs_gateway_conformance.md`
- `docs/source/sorafs_gateway_direct_mode.md`
- `docs/source/sorafs_gateway_deployment_handbook.md`
- `docs/source/sorafs_gateway_tls_automation.md`
- `docs/source/sorafs_gateway_dns_design_runbook.md`
- `crates/sorafs_manifest/src/gateway.rs`
- `crates/iroha_torii/src/sorafs/api.rs`
- `crates/iroha_telemetry/src/metrics.rs`
- `roadmap.md` sections SF-4/SF-5 and Near-Term Execution (Next 6 Weeks)

## Next Actions Before the Meeting
- Circulate this pre-read to Networking, Ops, Storage, QA, Governance, and
  Docs stakeholders; collect agenda additions by 2025-02-28.
- Produce a draft attendee list and Slack invite for the kickoff; confirm Ops
  and DNS automation leads are available.
- Prepare annotated diagrams (DNS flow + gateway policy paths) for the meeting
  deck—draft slides can live alongside this note in `docs/source/images/`.
- Gather existing GAR violation metrics from staging (`torii_sorafs_gar_violations_total`)
  to ground the discussion in current behaviour.
- Walk through the automation rehearsal described in
  `docs/source/sorafs_gateway_dns_design_runbook.md`, archive the artefacts
  under `artifacts/sorafs_gateway_dns/<run>/`, and link the bundle in the
  meeting invite so participants can inspect evidence ahead of time.

Once the kickoff concludes, update `status.md` and `roadmap.md` with the agreed
milestones and owner assignments.
