# Sumeragi Aggregator Routing

## Overview

This note captures the deterministic collector ("aggregator") routing strategy
used by Sumeragi after the Phase 3 fairness update. Every validator computes the
same collector ordering for a given block height and view. The design eliminates
reliance on ad-hoc randomness and keeps the normal vote fan-out bounded by the
collector list; when collectors are unavailable or quorum stalls, reschedule
rebroadcasts reuse collector targets with a commit-topology fallback.

## Deterministic Selection

* The new `sumeragi::collectors` module exposes
  `deterministic_collectors(topology, mode, k, seed, height, view)` which returns
  a reproducible `Vec<PeerId>` for the `(height, view)` pair.
* Permissioned mode rotates the contiguous tail collector set by
  `height + view`, ensuring each collector becomes primary on a round-robin
  schedule. This preserves the original proxy-tail behaviour while evenly distributing
  load across the tail segment (proxy tail + Set B validators).
* NPoS mode continues to use the per-epoch PRF but the helper now centralises
  the computation so every caller receives the same order. The seed is derived
  from the epoch randomness supplied by `EpochManager`.
* `CollectorPlan` tracks consumption of the ordered targets and records whether
  the gossip fallback was triggered. Telemetry updates
  (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`,
  `sumeragi_gossip_fallback_total`) surface how often fallbacks occur and how
  long redundant fan-out takes.

## Fairness Goals

1. **Reproducibility:** The same validator topology, consensus mode, and
   `(height, view)` tuple must lead to identical primary/secondary collectors on
   every peer. The helper hides topology quirks (proxy tail, Set B validators) so the
   ordering is portable across components and tests.
2. **Rotation:** In permissioned deployments the primary collector rotates every
   block (and also changes after a view bump) preventing a single Set B validator
   from permanently owning aggregation duties. PRF-based NPoS selection already
   provides randomness and is unaffected.
3. **Observability:** Telemetry keeps reporting per-collector assignments and the
   fallback path emits a warning when gossip is engaged so operators can detect
   misbehaving collectors.

## Retry and Gossip Backoff

* Validators keep a `CollectorPlan` in the proposal state; the plan records how
  many collectors have been contacted and whether the redundant fan-out limit
  has been reached.
* Collector plans are keyed by `(height, view)` and are reinitialized whenever
  the subject changes so stale view-change retries do not reuse old collector
  targets.
* Redundant send (`r`) is applied deterministically by advancing through the
  plan. When no collectors are available for the `(height, view)` tuple, votes
  fall back to the full commit topology (excluding self) to avoid deadlock.
* When quorum stalls, the reschedule path rebroadcasts cached votes via the
  collector plan, falling back to the commit topology when collectors are
  empty, local-only, or below quorum. This provides a bounded "gossip" fallback
  without paying full broadcast cost on the steady-state fast path.
* Each drop of a proposal due to the locked commit-certificate gate increments
  `block_created_dropped_by_lock_total`; failed header validation paths raise
  `block_created_hint_mismatch_total` and
  `block_created_proposal_mismatch_total`, helping operators correlate repeated
  fallbacks with leader correctness issues. The `/v1/sumeragi/status` snapshot
  also exports the latest Highest/Locked commit certificate hashes so dashboards
  can correlate drop spikes with specific block hashes.

## Implementation Summary

* New public module `sumeragi::collectors` hosts `CollectorPlan` and
  `deterministic_collectors` so both crate-level and integration tests can verify
  fairness properties without instantiating the full consensus actor.
* `CollectorPlan` lives in the Sumeragi proposal state and is reset when the
  proposal pipeline completes.
* `Sumeragi` builds collector plans via `init_collector_plan` and targets
  collectors when emitting availability/precommit votes. Availability and
  precommit votes fall back to the commit topology when collectors are empty,
  local-only, or below quorum, and rebroadcasts fall back under the same
  conditions.
* Unit and integration tests validate permissioned rotation, PRF determinism and
  backoff state transitions.

## Review Sign-off

* Reviewed-by: Consensus WG
* Reviewed-by: Platform Reliability WG
