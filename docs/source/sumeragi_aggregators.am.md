---
lang: am
direction: ltr
source: docs/source/sumeragi_aggregators.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e35bcd9c4f3879aa207ee9f5cd056f8753659e3aaee366c2125df911fefaaf22
source_last_modified: "2026-02-01T09:15:43.391921+00:00"
translation_last_reviewed: 2026-02-07
---

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
* Permissioned mode uses PRF-based collector selection seeded by the epoch PRF/VRF state.
  The helper derives a deterministic per-`(height, view)` ordering from the canonical
  roster and excludes the leader. When a PRF seed is unavailable, it falls back to a
  wraparound slice starting at `proxy_tail_index()` to preserve deterministic behaviour.
* NPoS mode continues to use the per-epoch PRF, with the helper centralising the
  computation so every caller receives the same order. The seed is derived from the
  epoch randomness supplied by `EpochManager`.
* Validators may also send commit votes to a small parallel topology fanout
  (`collectors.parallel_topology_fanout`) to reduce collector-hop latency without
  full broadcast fan-out.
* `CollectorPlan` tracks consumption of the ordered targets and records whether
  the gossip fallback was triggered. Telemetry updates
  (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`,
  `sumeragi_gossip_fallback_total`) surface how often fallbacks occur and how
  long redundant fan-out takes.

## Fairness Goals

1. **Reproducibility:** The same validator topology, consensus mode, PRF seed,
   and `(height, view)` tuple must lead to identical primary/secondary collectors
   on every peer. The helper hides topology quirks (proxy tail, Set B validators) so the
   ordering is portable across components and tests.
2. **Rotation:** PRF-based selection rotates the primary collector across heights
   and views in both modes, preventing a single Set B validator from permanently
   owning aggregation duties. The deterministic wraparound fallback is used only
   when the PRF seed is unavailable.
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
  empty or local-only. This provides a bounded "gossip" fallback without paying
  full broadcast cost on the steady-state fast path.
* Each drop of a proposal due to the locked QC gate increments
  `block_created_dropped_by_lock_total`; failed header validation paths raise
  `block_created_hint_mismatch_total` and
  `block_created_proposal_mismatch_total`, helping operators correlate repeated
  fallbacks with leader correctness issues. The `/v1/sumeragi/status` snapshot
  also exports the latest Highest/Locked QC hashes so dashboards
  can correlate drop spikes with specific block hashes.

## Implementation Summary

* New public module `sumeragi::collectors` hosts `CollectorPlan` and
  `deterministic_collectors` so both crate-level and integration tests can verify
  fairness properties without instantiating the full consensus actor.
* `CollectorPlan` lives in the Sumeragi proposal state and is reset when the
  proposal pipeline completes.
* `Sumeragi` builds collector plans via `init_collector_plan` and targets
  collectors when emitting availability/precommit votes. Availability and
  precommit votes fall back to the commit topology when collectors are empty or
  local-only, and rebroadcasts fall back under the same conditions.
* Unit and integration tests validate PRF determinism, fallback selection, and
  backoff state transitions.

## Review Sign-off

* Reviewed-by: Consensus WG
* Reviewed-by: Platform Reliability WG
