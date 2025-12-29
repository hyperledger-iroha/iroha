# Q1 2026 Clarification Prompts

These ready-to-send LLM prompts target the open roadmap items flagged for
Q1 2026. Copy the relevant block into the coordination thread, swap the
bracketed placeholders, and attach any local diffs or logs before routing to
@mtakemiya.

## Kaigi Privacy Phase 3 — Relay Overlay & Governance Hooks

*(Completed: governance allowlists, health reporting, telemetry, and failover tooling have landed; no follow-up needed.)*

## NPoS Sumeragi — Restart & Randomness Acceptance Gates

*(Completed: restart liveness and pacemaker telemetry coverage landed; RBC cold-start recovery and downtime resume tests now backstop Milestone A3. See `integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart` and `integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`. VRF acceptance for Milestone A4 shipped alongside the telemetry/runbook updates referenced in `status.md`.)*

## Dual Iroha 2/3 Release Track — Build & Packaging Decisions

````markdown
We are scoping the **dual build matrix for Iroha 2 vs. Iroha 3** and need your decisions before wiring CI and release automation.

**Current implementation snapshot**
- Roadmap items 1364–1375 outline the pending tasks; no shared pipeline exists yet.
- Release manifests currently assume a single artifact family.
- Operator docs (`docs/source/release_procedure.org`) mention only the legacy flow.

**Questions for you**
1. How should we name and label the dual artifacts so Sora Nexus reliably picks Iroha 3 while other networks default to Iroha 2?
2. Are there configuration knobs we must expose via `iroha_config` to help operators verify they have the correct build?
3. What signing and publication targets should we treat as authoritative for each build flavor (container registry, package buckets, checksum manifests)?

Once we have these answers we can finalise the CI plan and update the operator documentation.
````
