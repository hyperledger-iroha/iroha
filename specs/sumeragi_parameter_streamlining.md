---
title: Sumeragi Parameter Streamlining Spec
status: draft
last_updated: 2026-01-28
---

# Sumeragi Parameter Streamlining Spec

## Summary
Streamline the consensus parameter surface so operators only tune a minimal set of
on-chain knobs while all remaining timeouts and pacing parameters are derived
deterministically. Advanced overrides remain available but are explicitly scoped
and off by default. This spec is for the first public release; no backward
compatibility is required.

## Goals
- Reduce operator-facing knobs to a small, intuitive set.
- Preserve deterministic behavior across nodes and hardware.
- Derive timeouts from on-chain block/commit timing and the genesis minimum
  finality floor.
- Expose effective, derived values for observability.
- Keep advanced overrides available without complicating the default experience.

## Non-goals
- No new consensus modes or protocol changes beyond parameter layout updates.
- No reliance on environment variables for production behavior.

## Canonical parameter surface

### On-chain parameters (minimal, consensus-critical)
These are the only parameters operators are expected to set:
- `SumeragiParameters.block_time_ms`
- `SumeragiParameters.commit_time_ms`
- `SumeragiParameters.min_finality_ms` (new; genesis must set a floor)
- `SumeragiParameters.pacing_factor_bps`
- `SumeragiParameters.max_clock_drift_ms`
- `SumeragiParameters.collectors_k`
- `SumeragiParameters.collectors_redundant_send_r`
- `SumeragiParameters.da_enabled`

### Advanced overrides (operator config only)
These remain configurable but are grouped under an explicit advanced section and
default to "derive/auto":
- Pacemaker: `rtt_floor_multiplier`, `max_backoff`, optional jitter.
- Pacing governor: `sumeragi.advanced.pacing_governor` (window, thresholds, step size, bounds).
- DA: `quorum_timeout_multiplier`, `availability_timeout_multiplier`,
  `availability_timeout_floor`.
- RBC: TTLs, storage caps, chunk fanout caps, pending stash limits.
- Worker budgets and queue capacities (resource tuning only).

All advanced overrides must have deterministic fallback values and should not
be required for normal operation.

## Derivation rules (default behavior)
When advanced overrides are unset (0/None), derive values deterministically:

### Timing floors
- Enforce `commit_time_ms >= block_time_ms >= min_finality_ms`.
- Genesis starts with `block_time_ms = min_finality_ms`,
  `commit_time_ms = min_finality_ms` unless overridden by genesis configuration.
- Apply `pacing_factor_bps` (basis points, 10_000 = 1.0x) after the base clamps,
  then re-clamp `effective_block_time_ms >= min_finality_ms` and
  `effective_commit_time_ms >= effective_block_time_ms`.

### NPoS timeouts
- Default: derive all NPoS timeouts from the effective block time
  (`block_time_ms` scaled by `pacing_factor_bps`).
- If explicit NPoS timeout overrides exist, they must be moved to advanced-only
  configuration. When unset, derive from the effective block time.
- Derivation uses the baseline 1s ratios from the current defaults:
   - `propose`: 0.35 * block_time
   - `prevote`: 0.45 * block_time
   - `precommit`: 0.55 * block_time
   - `exec`: 0.15 * block_time
   - `witness`: 0.15 * block_time
   - `commit`: 0.75 * block_time
   - `da`: 0.65 * block_time
   - `aggregator`: 0.12 * block_time
- Clamp each derived timeout to at least 1ms.

### Commit quorum timeout
- Use the canonical function:
  - DA enabled: `(block_time + 3 * commit_time) * quorum_timeout_multiplier`
  - DA disabled: `max(block_time, commit_time, 2s)`

### Availability timeout (DA mode)
- `max(commit_quorum_timeout, availability_timeout_floor) * availability_timeout_multiplier`

### Pacemaker base interval
- `max(block_time, propose_timeout * rtt_floor_multiplier)` capped by `max_backoff`.

### Deterministic pacing governor (always enabled)
- The governor is always enabled and adjusts `SumeragiParameters.pacing_factor_bps`
  deterministically at block boundaries.
- Evaluate the last `window_blocks` committed headers and compute:
  - View-change pressure = average view-change index delta per block (permille).
  - Commit spacing pressure = average inter-block spacing vs target block time (permille).
- If either ratio exceeds its pressure threshold, increase by `step_up_bps`.
- If both ratios fall below their clear thresholds, decrease by `step_down_bps`.
- Clamp the factor to `[min_factor_bps, max_factor_bps]` (min >= 10_000).
- Skip adjustments when the current block already sets `pacing_factor_bps` explicitly.

### Rebroadcast and idle thresholds
- Keep derived from `block_time_ms` using existing helper functions.

## Default values (first release)

### On-chain defaults (genesis)
- `min_finality_ms`: 100
- `block_time_ms`: 100 (starts at the floor unless genesis overrides)
- `commit_time_ms`: 100 (must satisfy `commit_time_ms >= block_time_ms`)
- `pacing_factor_bps`: 10_000
- `max_clock_drift_ms`: 1_000
- `collectors_k`: 1
- `collectors_redundant_send_r`: 3
- `da_enabled`: true

These defaults replace the current 2s/4s timing defaults so the chain starts at
the floor and converges upward as needed.

### Advanced override defaults (operator config)
These remain unchanged from current defaults and are only used when explicitly
set (non-zero/non-null). Values below reference `iroha_config` defaults:
- Pacemaker:
  - `backoff_multiplier`: 1
  - `rtt_floor_multiplier`: 2
  - `max_backoff_ms`: 10_000
  - `jitter_frac_permille`: 0
- DA:
  - `quorum_timeout_multiplier`: 3
  - `availability_timeout_multiplier`: 2
  - `availability_timeout_floor_ms`: 2_000
- RBC (resource tuning):
  - `chunk_max_bytes`: 256 KiB
  - `session_ttl_ms`: 120_000
  - `pending_ttl_ms`: 120_000
  - `rebroadcast_sessions_per_tick`: 8
  - `payload_chunks_per_tick`: 64
  - `store_max_sessions`: 4_096
  - `store_max_bytes`: 2 GiB
- Pacing governor (always enabled):
  - `enabled`: true
  - `window_blocks`: 20
  - `view_change_pressure_permille`: 200
  - `view_change_clear_permille`: 50
  - `commit_spacing_pressure_permille`: 1_300
  - `commit_spacing_clear_permille`: 1_100
  - `step_up_bps`: 1_000
  - `step_down_bps`: 100
  - `min_factor_bps`: 10_000
  - `max_factor_bps`: 20_000

### Adaptive observability defaults
Adaptive observability remains off by default:
- `enabled`: false
- `qc_latency_alert_ms`: 400
- `da_reschedule_burst`: 2
- `pacemaker_extra_ms`: 100
- `collector_redundant_r`: 3
- `cooldown_ms`: 5_000

## Observability
Expose effective values in `/v1/sumeragi/status`:
- `effective_block_time_ms`
- `effective_commit_time_ms`
- `effective_pacing_factor_bps`
- `effective_min_finality_ms`
- `effective_commit_quorum_timeout_ms`
- `effective_availability_timeout_ms`
- `effective_pacemaker_interval_ms`
- `effective_npos_timeouts` (when in NPoS mode)
- `effective_collectors_k` and `effective_redundant_send_r`

These fields must reflect the derived runtime values, not raw config input.
Configuration events emit `pacing_factor_bps` changes when the governor adjusts it.

## Validation rules
- `min_finality_ms > 0`
- `block_time_ms >= min_finality_ms`
- `commit_time_ms >= block_time_ms`
- If any advanced override is set, it must still respect the minimum finality
  floor where applicable.

## Implementation outline
1. Add `min_finality_ms` to `SumeragiParameters` (Norito + JSON) and defaults.
2. Update genesis fingerprint and validation to include/enforce the floor.
3. Move NPoS timeout overrides into advanced-only config (or remove them).
4. Ensure all derived timeouts clamp to `min_finality_ms`.
5. Surface effective values in telemetry/status.
6. Update docs and config templates to reflect the streamlined surface.
7. Add the deterministic pacing governor (windowed pressure + bounded hysteresis).
