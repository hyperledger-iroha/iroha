# Sumeragi goals

Set: 2026-09-25. Overall goal: **Active**. Fix consensus and freeze it: replace the
Sumeragi v2 runtime with the rewritten Sumeragi core and retire every v2 artifact.
Supersedes [the v2 liveness redesign goals](sumeragi_liveness_redesign_goals.md) (L1–L6),
which are retired unfinished: their target was reconciling v2 lifecycle owners, and the
rewrite removes those owners instead.
Required working directory: `/Users/takemiyamakoto/dev/iroha`. Required branch:
`optimizations`.

Protocol contract: [specs/sumeragi.md](sumeragi.md). Implementation:
[`crates/iroha_sumeragi`](../crates/iroha_sumeragi) (sans-IO core and deterministic
simulator). There is one protocol and it is called Sumeragi; no v1/v2 naming survives
the cutover. Why and how it was built: [2026-09-25 record](../docs/history/2026-09-25/sumeragi-rewrite.md).

## Decisions (2026-09-25)

| Decision | Choice |
| --- | --- |
| Voting | Execute before vote. Prepare and Commit votes sign `(instance, height, view, block, result)`; a CommitQC finalizes order and result. |
| Topology | B-Chain overlay per round: leader, set A (first `n − f`), proxy tail (last of set A), set B (standby `f`). Only set A works in the normal case. |
| Collector | Proxy tail aggregates; graded fallback (retransmit, set B via the proxy tail, then broadcast voting) when it or a set-A member fails. |
| Finality | One core instance per chain. Dataspaces and lanes keep DS-local finality; cross-dataspace AMX uses two-phase commit through the global chain (spec §11). |
| Provenance | In-house protocol and code; no external consensus library. |
| Compatibility | None. Fresh wire, storage and genesis; Taira is reset. |
| Scale | 4 validators now, about 20 at launch. Quorum is `n − f` for any `n ≥ 1`. |

## Goals

| Id | Goal | Status | Acceptance |
| --- | --- | --- | --- |
| S1 | Sans-IO core and simulator | Implemented; qualification in progress. Core 6 314 lines; 105 of 105 mutations killed by their named tests; all 36 fault scenarios pass 500 seeds each. The 10 000-seed nightly run and the CI job for the gate do not exist yet. [Evidence](../docs/history/2026-09-25/sumeragi-rewrite.md#evidence). | Spec conformance; every safety and liveness mutation of spec §13.4 killed by its named deterministic test (`scripts/sumeragi_mutation_gate.py`); all fault scenarios pass nightly at 10 000 seeds; core ≤ 8 000 lines (spec §12.6). |
| S2 | Node driver | Open | Driver in `iroha_core` generic over `Net`, `RecordStore`, `BodyStore`, `BlockStore`, `Clock` and `Executor`; the same driver code runs in the simulator (spec §13.5); O2/O3/O5/O8/O9 conformance oracles pass at every I/O completion. |
| S3 | Execution and storage binding | Open | Define `R` (post-state root, transaction-outcome root, event root, scheduled committee); speculative execution chained on certified parents; apply reuses cached post-states; Kura stores blocks with CommitQCs; per-key safety-record files with the installation log and store id (spec §7.4). |
| S4 | Cutover and deletion | Open | Swap at `SumeragiStartArgs::start` (`crates/iroha_core/src/sumeragi/mod.rs`, started from `crates/irohad/src/main.rs`); P2P control/bulk traffic classes; config surface reduced to spec §12.4; status endpoint from `Core::status`. Then delete the v2 runtime and everything that exists only for it (inventory below). |
| S5 | Dataspace and lane instances | Open | Several cores per node, instance-id derivation, per-instance committees and records, O9 isolation (one stalled instance never delays another). |
| S6 | AMX two-phase commit | Open | Begin, prepare/escrow, relay, decision by deadline, settle; foreign-committee tracking and handoff proofs (spec §11); O-AMX oracle in the simulator. |
| S7 | Taira reset and qualification | Open | Fresh genesis and operator runbook (spec §14.5); 24 h soak at n = 4 and n = 22 with 10–30 % loss, delay spikes, `kill -9` and disk-full injection; O-AGR, O-SIGN, O-LIVE and O-PERF computed from node logs. Release gate. |
| S8 | Evidence and committee scheduling | Open | Evidence → penalties; NPoS election schedules committees with the lag-2 rule (spec §10). |
| S9 | Full state root in `R` | Open | `R`'s post-state root commits to the complete World state (a Merkleized state or an incremental full-state accumulator), not only to the witnessed write set, and an event root is added; replaces the deviation recorded in spec Appendix E, E51. |

S2 and S3 come before S4. S5 and S6 build on S4. S7 gates the release.

## Guardrails

- A liveness fix lands only with a simulator seed or deterministic test that fails before it
  and passes after it.
- Every new rule gets a mutation in spec §13.4 and a named test that kills it; the mutation
  gate runs in CI.
- The core stays within the spec §12.6 line budget; a change that adds an owner, fence,
  guard or custody type must remove one.
- No compatibility shims and no parallel implementation after S4.

## v2 deletion inventory (S4)

Measured on 2026-09-25 (`git ls-files`, lines including tests):

| Area | Lines |
| --- | --- |
| `crates/iroha_core/src/sumeragi/` (393 files) | 625 642 |
| `formal/sumeragi_v2/` (919 files) | 356 775 |
| `scripts/*sumeragi*` (169 files) | 308 146 |
| `pytests/**sumeragi*` | 152 924 |
| `integration_tests/**sumeragi*` | 22 059 |
| `specs/sumeragi*` (v2 specs and goals) | 14 588 |
| `crates/iroha_sumeragi_core` (Verus proofs of the v2 reducer) | 13 019 |
| `crates/iroha_data_model/src/block/consensus_v2*` | 11 201 |

Outside the subsystem, 262 `iroha_core` files and a few files in `iroha_torii`, `irohad`,
`iroha_kagami`, `iroha_test_network` and `iroha_cli` reference `crate::sumeragi::*`.

## Open questions for the owner

These are the open questions of spec §15. The implementation uses the stated default until
the owner decides otherwise.

1. **Committee lag.** Is a lag of 2 heights acceptable for NPoS epochs? Default: a committee
   or parameter change decided in block `h` binds at `h + 2`, which keeps apply and storage
   off the critical path.
2. **Idle cadence.** Should dataspace and lane instances default to an idle interval of 30 s or
   more? Default: `idle_block_interval` is 5 s for every instance (about 17 280 empty blocks
   per idle day); the spec only recommends 30 s or more for dataspaces and lanes.
3. **Crashed leader under load.** Should view 0 after a non-empty parent wait
   `block_time + build_timeout` instead of the idle interval? That cuts the cost of a crashed
   view-0 leader under load from about 7.2 s to about 3.2 s and adds one empty block after each
   busy period. Default: not adopted; view 0 always waits up to
   `idle_block_interval + build_timeout` for a proposal.
4. **Predictable schedule.** Are leaders and proxy tails that are known ahead of time
   acceptable? Default: round-robin over a per-committee permutation. It prevents seed
   grinding and gives slot fairness, but lets an attacker aim a DoS at upcoming leaders;
   view change and demotion bound the cost. The alternative seeds each round from the
   previous leader's unique BLS signature.
5. **Demotion.** Default: only leaders of failed views are demoted, for `W = 128` heights,
   at most `f` at a time, with slot substitution. Silent set-A members and proxy tails are not
   demoted. Confirm.
6. **Block timestamps.** Which time-monotonicity rule should the application enforce? Default:
   timestamps are application payload and the core never checks clocks; no rule is defined
   yet, and it must not compare against local clocks.
7. **Execution divergence.** Is a halt acceptable for launch? Default: `ApplyDiverged` halts
   the instance; recovery needs a state-snapshot path that no goal covers yet.
8. **Payload size.** Is leader bandwidth enough for launch blocks? Default: the leader sends
   the full payload (at most `max_block_bytes`, 4 MiB) to `n − 1` peers, set A first; at
   n = 22 its uplink caps block size. Relay through set A and erasure coding are deferred
   (spec §14).
9. **AMX epoch length.** What epoch length should AMX-participating instances use (handoff
   cadence for light clients, spec §11)? Default: `epoch_length` is 3 600 heights, one hour at
   1 s blocks.
