---
lang: my
direction: ltr
source: docs/source/project_tracker/nsc30a_relay_incentive_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a631577db239fab5b0f847dfe9e1f66c8939ea3ff88b8168956e1c692dd17ab8
source_last_modified: "2025-12-29T18:16:36.012688+00:00"
translation_last_reviewed: 2026-02-07
---

<!-- Outline for NSC-30a relay incentive design workshop. -->

# NSC-30a — Relay Incentive & Reputation Framework

## Goals
- Define deterministic incentive mechanisms for relay operators (rewards, penalties, staking).
- Produce Norito schemas and storage model for relay performance attestations.
- Supply quantitative models covering churn, correlated failures, and adversarial relays.

## Workshop Agenda (Draft)
1. Review current relay telemetry and control-plane capabilities.
2. Align on objectives: availability SLA, throughput targets, security constraints.
3. Present candidate economic models (staking + reward curve variants).
4. Discuss scoring function proposals (e.g., exponential decay, quadratic penalties).
5. Identify data requirements for simulations and calibration.
6. Assign drafting responsibilities and timeline checkpoints.

## Deliverables
- **Design whitepaper** summarising incentive assumptions, formulas, and governance hooks.
- **Schema draft** covering:
  - `RelayPerformanceRecord` (per epoch metrics, signatures).
  - `RelayScoreUpdate` (score delta, justification).
  - `RelayPenaltyNotice` (slashing triggers).
- **Simulation harness spec**:
  - Datasets: historic traffic, failure scenarios, Monte Carlo seeds.
  - Output metrics: availability %, reward variance, cost-of-attack.
  - Tooling: Python/Julia notebooks + reproducible seeds.
- **Governance integration plan** for reward distribution and dispute resolution.
- **Telemetry augmentation plan** for new Prometheus counters (`relay_availability_ratio`, `relay_penalty_events_total`, etc.).
- **Risk register update** covering Sybil amplification, collusion, and liquidity shocks.

## Action Items
- [ ] Draft workshop agenda and circulate by **2026-02-26**.
- [ ] Confirm attendees (Streaming WG, Economics WG, Governance liaison).
- [ ] Assign modellers to baseline parameter estimation (staked collateral, reward pool).
- [ ] Inventory telemetry gaps and propose additions (uptime, bandwidth, reputation).
- [ ] Draft schema strawman before workshop for quick iteration.
- [ ] Identify follow-up milestones for Torii integration/testing.
- [ ] Prepare candidate reward curves:
  - `reward = base_reward * availability^k`, for `k ∈ {1.5, 2}`.
  - penalty staircase for SLA breaches (e.g., drop to 50% reward after 2 consecutive misses).
  - optional staking multiplier: `multiplier = min(1, stake / target_stake)`.
- [ ] Outline penalty flow (warning → probation → slash) and required governance hooks.

## Dependencies
- Relay telemetry instrumentation from NSC-28b (jitter metrics) to feed scoring.
- Governance roadmap for staking/slashing support.
- Torii/WSV storage constraints for new records.

## Timeline
- Workshop kickoff: **2026-03-09**.
- Draft whitepaper: **2026-04-05**.
- Simulation results + schema freeze: **2026-04-30**.
- Handover to NSC-30b implementation: early **Q3 2026**.

## Reference Materials
- Prior art: Filecoin storage incentives, libp2p GossipSub reputation.
- Economic considerations: churn models from streaming partner data (pull logs from `artifacts/streaming/relay_metrics_2025.json`).
- Existing Norito schemas: `StreamingTelemetry`, `ReceiverReport`.
