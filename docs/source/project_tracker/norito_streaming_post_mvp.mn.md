---
lang: mn
direction: ltr
source: docs/source/project_tracker/norito_streaming_post_mvp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7200db61e0b2b85c6f41be52f90264706f6e973a4f3285061ac2f5c9f5bdd1f8
source_last_modified: "2026-01-05T09:28:12.036625+00:00"
translation_last_reviewed: 2026-02-07
---

<!-- Tracker detailing post-MVP Norito Streaming follow-ups. -->

# Norito Streaming — Post-MVP Backlog

| ID | Task | Owners | Target | Status | Next Actions | Notes |
|----|------|--------|--------|--------|-------------|-------|
| NSC-28b | Enforce ±10 ms A/V sync tolerance using validator telemetry and segment auditors | Streaming Runtime TL, Telemetry Ops | Q2 2026 | 🟡 In planning | Draft telemetry signal spec (jitter/drift buckets) and schedule validator instrumentation review (week of Mar 2). | Spec outline: `docs/source/project_tracker/nsc28b_av_sync_telemetry.md`. Instrument jitter/drift metrics in `StreamingTelemetry`, add gating to `SegmentAuditor`, and extend loopback/impairment tests for deterministic rejection. |
| NSC-30a | Design relay incentive & reputation framework (economics + schema) | Streaming WG, Economics WG | Q2 2026 | 🟡 In planning | Spin up cross-WG workshop (Mar 9) and assign economists to modelling doc outline. | Workshop prep doc: `docs/source/project_tracker/nsc30a_relay_incentive_design.md`. Produce incentive whitepaper, Norito schemas for relay attestations, telemetry fields, and simulation harness covering churn/failure models. |
| NSC-30b | Implement relay scoring and reward pipeline end-to-end | Streaming WG, Torii Team | Q3 2026 | ⚪ Pending NSC-30a | Blocked until NSC-30a design sign-off; prepare Torii schema change proposal draft. | Depends on NSC-30a output; track follow-ups in same doc and Torii backlog once design freezes. |
| NSC-37b | Implement ZK ticket proofs and host-side validation | ZK Working Group, Core Host Team | Q3 2026 | ⚪ Pending NSC-37a | Stand up prototype branch in `iroha_zkp_halo2` and list required host hooks. | Implementation notes will extend `nsc37a_zk_ticket_schema.md` once schema finalises; ensure host validation plan aligns with Core Host roadmap. |
| NSC-42 | Complete block-based codec legal/patent review | Legal & Standards | Q2 2026 | 🟢 Completed | Closed — counsel opinion logged and gating remains opt-in. | Sign-off captured in `docs/source/soranet/nsc-42-legal.md`; CABAC stays behind `ENABLE_CABAC` + `[streaming.codec]`, trellis is still disabled, and bundled rANS requires `ENABLE_RANS_BUNDLES`, with legal posture recorded in NOTICE/PATENTS/EXPORT. |
| NSC-55 | Validate rANS initialization tables and publish patent posture | Codec Team | Q2 2026 | 🟢 Completed | Closed — deterministic tables and evidence bundles are in-tree. | Canonical tables/generator checked in, benches and CSV/report pipelines land per `docs/source/project_tracker/nsc55_rans_validation_plan.md`, providing reproducible rANS posture for CABAC-optional deployments. |
