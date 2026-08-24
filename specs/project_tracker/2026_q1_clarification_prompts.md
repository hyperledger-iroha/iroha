# Q1 2026 Clarification Prompts

These ready-to-send LLM prompts target the open roadmap items flagged for
Q1 2026. Copy the relevant block into the coordination thread, swap the
bracketed placeholders, and attach any local diffs or logs before routing to
@mtakemiya.

## Kaigi Privacy Phase 3 — Relay Overlay & Governance Hooks

*(Completed: governance allowlists, health reporting, telemetry, and failover tooling have landed; no follow-up needed.)*

## NPoS Sumeragi — Restart & Randomness Acceptance Gates

*(Completed: signed DA recovery and pacemaker telemetry coverage landed; authenticated chunk-hold healing and downtime resume tests now backstop Milestone A3. See `integration_tests/tests/sumeragi_da.rs::authenticated_payload_chunk_hold_heals_and_converges_four_peers` and `integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`. VRF acceptance for Milestone A4 shipped alongside the telemetry/runbook updates referenced in `status.md`.)*
