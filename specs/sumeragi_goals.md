# Sumeragi goals

Set: 2026-09-25. Overall goal: **Active**. Complete and qualify the single
first-release Sumeragi core, node integration and application consumers.
Implementation completion and network/release qualification are separate gates.
Required working directory: `/Users/takemiyamakoto/dev/iroha`. Required branch:
`optimizations`.

Protocol contract: [specs/sumeragi.md](sumeragi.md). Implementation:
[`crates/iroha_sumeragi`](../crates/iroha_sumeragi) (sans-IO core and deterministic
simulator). There is one protocol and it is called Sumeragi; no v1/v2 naming survives
the cutover. Why and how it was built: 2026-09-25 record.

## Decisions (2026-09-25)

| Decision | Choice |
| --- | --- |
| Voting | Execute before vote. Prepare and Commit votes sign `(instance, height, view, block, result)`; a CommitQC finalizes order and result. |
| Topology | B-Chain overlay per round: leader, set A (first `n − f`), proxy tail (last of set A), set B (standby `f`). Only set A works in the normal case. |
| Collector | Proxy tail aggregates; graded fallback (retransmit, set B via the proxy tail, then broadcast voting) when it or a set-A member fails. |
| Finality | One core instance per chain. Dataspaces and lanes keep DS-local finality; cross-dataspace AMX uses two-phase commit through the global chain (spec §11). |
| Provenance | In-house protocol and code; no external consensus library. |
| Compatibility | None. Fresh wire, storage and genesis; Taira is reset. |
| Scale | The global production committee is exactly `3f + 1` with `1 <= f <= 10` (4 through 31), and every certificate contains exactly `2f + 1` equal validator votes. Candidate and observer pools do not contribute voting seats. The generic simulator exercises additional sizes; those runs do not qualify global production committees. |

## Goals

| Id | Goal | Status | Acceptance |
| --- | --- | --- | --- |
| S1 | Sans-IO core and simulator | Implemented; qualification in progress. Every §13.4 mutation is killed by its named test and every fault scenario passes at 500 seeds (evidence). CI: the PR job runs the simulator and the driver/node/lane tests; `.github/workflows/nightly_sumeragi.yml` runs every scenario at 10 000 seeds and the mutation gate (`scripts/sumeragi_mutation_gate.py --strict`). | Spec conformance; every safety and liveness mutation of spec §13.4 killed by its named deterministic test (`scripts/sumeragi_mutation_gate.py`); all fault scenarios pass nightly at 10 000 seeds. |
| S2 | Node driver | Implemented: `iroha_core::sumeragi::driver` (kernel, persistence, executor scheduling, serving, ingress) with the simulator conformance run; E49, E55. | Driver in `iroha_core` generic over `Net`, `RecordStore`, `BodyStore`, `BlockStore`, `Clock` and `Executor`; the same driver code runs in the simulator (spec §13.5); O2/O3/O5/O8/O9 conformance oracles pass at every I/O completion. |
| S3 | Execution and storage binding | Implemented with the E51 result commitment (complete World roots, ordered events and witnessed writes): State executor on one live overlay, Kura frames with commit certificates, file record/body stores, genesis result-only certificate, replay checked against each certified result; 4 in-process validators commit, restart and reject a forged result. | Define `R` (post-state root, transaction-outcome root, event root, scheduled committee); speculative execution chained on certified parents; apply reuses cached post-states; Kura stores blocks with CommitQCs; per-key safety-record files with the installation log and store id (spec §7.4). |
| S4 | Cutover and deletion | Done (2026-09-29). irohad starts only Sumeragi (`node::prepare` then `start_on_network`); four `iroha3d` peers over P2P commit transactions, restart one and all, and replace a crashed leader (`integration_tests/tests/sumeragi.rs`). Application readers of committed blocks read Kura's certified frames through the certified-chain reader (spec §12.7, Appendix E, E57). `kagami localnet` passes `--sumeragi-assert-fresh-key` on a peer's first boot; the SoraFS provider-ingest and reputation finalized archives are captured by the native executor (`sumeragi::executor::FinalizedArchives`); bridge finality proofs, bundles and challenge-bound attestations are built from commit certificates (`iroha_core::sumeragi::finality`). The v2 runtime and everything that existed only for it are deleted (inventory below). Accelerated snapshot restoration moved to S9. | Swap at `SumeragiStartArgs::start` (`crates/iroha_core/src/sumeragi/mod.rs`, started from `crates/irohad/src/main.rs`); P2P control/bulk traffic classes; config surface reduced to spec §12.4; status endpoint from `Core::status`. Then delete the v2 runtime and everything that exists only for it (inventory below). |
| S5 | Dataspace and lane instances | Lanes implemented ([specs/sumeragi_lanes.md](sumeragi_lanes.md)): every lane incarnation is a core instance with a pinned committee, the global chain merges certified lane blocks by reference, and autoscale opens, closes and retires elastic lanes; tested in-process with 4 validators and over P2P (`integration_tests/tests/sumeragi_lanes.rs`). Open: hosting dataspace instances with their own state (needed by S6). | Several cores per node, instance-id derivation, per-instance committees and records, O9 isolation (one stalled instance never delays another). |
| S6 | AMX two-phase commit | In progress. `G` side in the node: the `sumeragi_amx` World cell, `RegisterAmxDataspaceV1`, `BeginAmxV1`, `RelayAmxPreparedV1`, `RelayAmxHandoffV1` and the deadline step (`crates/iroha_core/src/sumeragi/amx/`); records are World writes proved against the witnessed-write root of `R`; foreign-committee trackers advance one epoch per handoff proof; the dataspace participant (prepare, escrow, settle) is a component over an escrow interface; the simulator runs `G` with two or three dataspaces under the O-AMX oracle (F31, mutations MX1–MX12 killed). Open: hosting dataspace instances with their own state (TODO(S6), Appendix E, E58). | Begin, prepare/escrow, relay, decision by deadline, settle; foreign-committee tracking and handoff proofs (spec §11); O-AMX oracle in the simulator. |
| S7 | Taira reset and qualification | In progress. Runbook [specs/runbooks/sumeragi_taira_reset.md](runbooks/sumeragi_taira_reset.md); current startup and restart defects are being fixed. `scripts/sumeragi_soak.py`, `integration_tests/tests/sumeragi_lanes_soak.rs` and `.github/workflows/nightly_sumeragi_soak.yml` provide optional engineering diagnostics with O-AGR, O-SIGN, O-LIVE and O-PERF computed from node logs. | Fresh genesis and operator runbook (spec §14.5); authenticated native control authority, genesis and committee, safety-record custody, and live four-validator readiness/write/restart evidence. Deployment policy belongs to on-chain governance for Taira and production. No fixed fault-test duration or soak verdict authorizes or blocks deployment. |
| S8 | Evidence and committee scheduling | Open; complete historical key/PoP records and prefix verifier implemented, Core qualification pending | Evidence → penalties; authenticated E+2 elections, a complete E+1 preparation interval and atomic activation/retention under the validator staking requirements below. Qualify every historical `CommitQC` against the genesis-anchored authority prefix (spec §12.7), including rotated-away and revoked keys, without a local-trust fallback. |
| S9 | Full state root in `R` | Implemented; current-candidate qualification and authenticated accelerated snapshot restoration remain open. E51 binds parent/post-execution World roots from an incremental LtHash16 accumulator and ordered event roots; publication incorporates deterministic tail writes and startup checks certified replay against a cold World capture. | Qualify exhaustive canonical World coverage, incremental/cold equivalence, rollback, event ordering and restart against certified results. Accelerated restoration additionally authenticates the complete restored State and retained native tip/history; a matching World root alone is insufficient. |

S2 and S3 come before S4. S5 and S6 build on S4. S7 gates the release.

**Snapshot contract.** The node publishes signed local snapshot exports only after
original genesis execution and certified journal replay finish and the native driver
starts. A native halt or worker failure revokes the writer gate; orderly shutdown
retains successful recovery for the final write. Export signatures authenticate bytes,
not execution of a decoded World. Strict startup rejects nonempty snapshot caches and
replays the original journal. Accelerated restoration remains an S9 outcome: it needs
authenticated complete State provenance plus the exact native tip, CommitQC and
retained headers. The implemented World accumulator and a local export signature
do not authenticate State-level fields outside World.

### Validator staking integration requirements

The [validator staking completion plan](staking_validator_completion.md) remains
active across the consensus replacement. Replacing runtime owners does not waive
the original liveness acceptance criteria: silent authors, saturation,
final-transaction progress, authenticated loss, lane retirement and restart must
be qualified on the production path. The rewrite's simulator passes alone do not
close those outcomes.

- Global membership comes from authenticated prepared elections, with E+2 frozen
  at the end of E and E+1 reserved for keys and beacon DKG. The lag-2 storage window
  is not a substitute for this full-epoch preparation policy. Registration adds
  candidates only; exact voting geometry alone does not authenticate a transition.
- Activation publishes the complete ordered committee, Pasta authority and beacon
  session together after every target seat proves custody and the current exact
  quorum certifies the boundary. Failed preparation requires certified retention
  of the current generation and cancellation of that attempt. It cannot shrink or
  reroll the frozen roster, and does not require fresh incumbent keys.
- Authority generations and scheduling epochs are separate authenticated records.
  Every signature binds its epoch and complete context. Leader scheduling retains
  fresh authenticated epoch-boundary randomness.
- Signed RS16 availability remains mandatory. Raw full-body dissemination is not
  the qualified replacement. Original resource-funded execution must survive
  validation, publication, application and restart under the sole production owner.
- Bonds, rewards, fees and withdrawals use the immutable network-authenticated real
  XOR asset with exact scoped custody; no placeholder token or implicit funding.
  Retained exit requests keep voting and slashing obligations until an
  authenticated replacement takes effect.

These requirements are not yet implemented end to end in the new native driver.
One unchanged candidate must pass actual disposable 4→7→4 transitions with
noncommittee-sized candidate pools, all-seat restart and the full monetary,
formal, DA, workspace and SDK gates. Qualification does not authorize live
network deployment or value-moving transactions.

## Guardrails

- A liveness fix lands only with a simulator seed or deterministic test that fails before it
  and passes after it.
- Every new rule gets a mutation in spec §13.4 and a named test that kills it; the mutation
  gate runs in CI.
- Changes preserve the source-bound protocol owners, deterministic behavior and
  independently checked safety, liveness and custody invariants (spec §12.6).
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

Removed: `crates/iroha_sumeragi_core` and the v2 CI and release-gate scripts (2026-09-27);
the v2 runtime modules and the old Nexus lane machinery (QueuePlan, lane relay, merge ledger,
native AMX, v2 finality artifacts and snapshot bootstrap), `formal/sumeragi_v2/`, the v2
specs, scripts, CI jobs and configuration surface (2026-09-29). The shared consensus
vocabulary of `block::consensus_v2` now lives in `iroha_data_model::block::consensus`.

## Open questions for the owner

These are the open questions of spec §15. The implementation uses the stated default until
the owner decides otherwise.

1. **Committee preparation.** Decided for validator staking: freeze E+2 at the end of E
   and prepare throughout E+1. The native lag-2 height window must carry this authenticated
   epoch policy; it cannot activate every registered key at `h + 2`. Integration is open.
2. **No empty blocks.** Decided: idle instances never create blocks. Empty or missed builds
   wait for queued transactions, with a bounded `payload_retry_interval` (default 5 s).
   Decoded application payloads must contain at least one network entrypoint.
3. **Crashed leader under load.** View 0 currently allows
   `payload_retry_interval + build_timeout` for a proposal. A shorter wait after a busy
   parent is a future latency optimization; neither timeout nor retry creates a block.
4. **Schedule randomness.** Validator staking requires fresh authenticated epoch-boundary
   randomness for leader scheduling. The topology consumes the authenticated epoch seed;
   preparation must not preselect or reroll boundary randomness. Network qualification remains open.
5. **Demotion.** Default: only leaders of failed views are demoted, for `W = 128` heights,
   at most `f` at a time, with slot substitution. Silent set-A members and proxy tails are not
   demoted. Confirm.
6. **Block timestamps.** Which time-monotonicity rule should the application enforce? Default:
   timestamps are application payload and the core never checks clocks; no rule is defined
   yet, and it must not compare against local clocks.
7. **Execution divergence.** Is a halt acceptable for launch? Default: `ApplyDiverged` halts
   the instance; recovery needs a state-snapshot path that no goal covers yet.
8. **Payload availability.** Signed RS16 `PayloadManifest`/`PayloadChunk` availability
   is mandatory for the first release. The core integrates signed manifests, authenticated RS16 row acquisition and
   source-bound payload custody. Whole-node loss/withholding, committee geometry
   and resource qualification remain required; component tests do not close them.
9. **AMX epoch length.** What epoch length should AMX-participating instances use (handoff
   cadence for light clients, spec §11)? Default: `epoch_length` is 3 600 heights, one hour at
   1 s blocks.
