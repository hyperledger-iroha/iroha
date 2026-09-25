# Sumeragi rewrite — 2026-09-25

Scope: `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`, 2026-09-25 to 2026-09-26.
The work is an uncommitted working tree on top of `78b8f8033f` ("update sumeragi"), built
with rustc 1.93.1. New paths: `crates/iroha_sumeragi/` and
`scripts/sumeragi_mutation_gate.py`; `Cargo.lock` gains the new package (7 lines). No node
uses the new crate yet; every node still runs Sumeragi v2. An earlier cloud session built a
first version whose code was lost. Its lessons were carried over and the code was rebuilt
here.

Current goals: [Sumeragi goals](../../../specs/sumeragi_goals.md). Protocol:
[specs/sumeragi.md](../../../specs/sumeragi.md). Superseded:
[v2 liveness redesign goals](../../../specs/sumeragi_liveness_redesign_goals.md) and the
[last v2 liveness status](sumeragi-v2-liveness-status.md).

[Current status](../../../status.md) · [History index](../README.md)

## Why

Sumeragi v2 had constant liveness failures for months, and they came from its design, so
fixing them one at a time did not converge. Votes depended on each node's local conditions
(busy, recovery, queue release, a mandatory beacon pulse), so one bug made every honest node
refuse at once, a common-mode fault that view changes cannot mask. Five quorum instances
(global, lane, drain, QueuePlan, beacon) waited on each other and formed documented cycles.
The pacemaker shared a polling loop and an I/O FIFO with execution and storage, so
backpressure stalled the timeouts meant to escape it. Each block moved through about 17
hand-offs of per-message custody across four threads and four durable stores, instead of
nodes re-sending their latest state. Fail-stop was the default error path, so an unexpected
state exited the daemon, and restart was the most bug-prone path. Proofs and tests covered
the reducer, not the runtime where the failures happened. The subsystem grew without a
budget, from 3 376 lines in December 2025 to about 620 000 lines in
`crates/iroha_core/src/sumeragi`, and all six v2 liveness redesign goals (L1–L6, set
2026-09-16) were still open.

## Decisions

The owner's requirements and answers, 2026-09-25:

- Rewrite Sumeragi rather than repair v2, then freeze consensus. There is one protocol and
  it is called Sumeragi, with no v1 or v2. First release: no backward compatibility; remove
  unused code.
- Execute before vote is a hard requirement.
- Keep the B-Chain overlay (after Sisi Duan): each round orders the committee into a leader,
  set A, a proxy tail (last of set A) and set B.
- Lanes and dataspaces keep independent finality: a dataspace block is final at its own
  CommitQC; cross-dataspace AMX uses two-phase commit through the global chain. The core is
  instance-agnostic.
- Collector: the proxy tail aggregates; a fallback makes progress without it.
- In-house protocol and code; no external consensus library.
- Taira is a testnet with no value and will be reset.
- Scale: about 4 validators now, about 20 at launch.
- Work on `optimizations` and do not commit.

## What was built

**Specification.** Written 2026-09-25 and revised four times. Revision 2 addressed an
adversarial review with four lenses (safety, liveness, topology, integration; 60 findings).
Revision 3 applied five decisions from a skeptical review, including a simplicity pass.
Revision 4 addressed a second adversarial review with three lenses (41 findings: 6 high,
21 medium, 14 low; each high finding was independently confirmed or refuted). Revision 4.1
added eight refinements from its verification. The review logs are in the
[specification review record](sumeragi-spec-review.md); Appendix E of the specification
reconciles the text with the code. The reviews found, among others: bodies accepted without checking `payload_hash`; an
unsigned `justify` or payload that let a relay force early timeouts; bodies that did not
survive a whole-cluster restart; the TC that moved a node into a view was not persisted; a
grindable topology seed; a rolled-back or re-created safety record that forked with one
Byzantine member; and a proxy tail that never broadcast the CommitQC it formed.

**Core crate.** `crates/iroha_sumeragi` is a sans-IO state machine: `Core::new`,
`handle(now, Event) -> Vec<Action>`, `next_wakeup` and `status`. The driver owns timers,
network, storage, signing and execution. Its only dependency is `norito` (`base-codec`). It
uses no threads, clocks, randomness, floats, `HashMap` iteration or `unsafe`, and no
`unwrap`/`expect` on peer input. It was built in three stages (foundations, state machine,
simulator) and then checked independently.

**Simulator.** `src/sim`, compiled only with `cfg(any(test, feature = "sim"))`: a seeded
discrete-event network with loss, delay, partitions, clock drift, crash-restart, whole-cluster
restart and Byzantine strategies (equivocating and late leaders, withholding proxy tails,
split certificates, forged records and sync entries). It runs 36 fault scenarios (F1–F36)
with safety, liveness, resource and performance oracles after every event.

**Mutation gate.** `scripts/sumeragi_mutation_gate.py` builds each mutation of spec §13.4 as
a `cfg(sumeragi_mutation = "<ID>")` switch. `build.rs` sets it only with the crate feature
`mutation-testing`, so a production build cannot be mutated. Each mutation must be killed by
its named deterministic test. The first run killed 95 of 100. Five weak tests were then
strengthened.

**Bugs found.** The simulator found four core bugs, each fixed with its spec rule amended:

1. A rate-limited `Status` dropped the CommitQC a lagging node needed (F3, seed 12).
2. A restart trusted a safety record that contradicted the block store (F24 with a forged
   record, seed 11). The core now halts with `SafetyRecordInconsistent`.
3. Catch-up stalled forever on a gap left by dropped forged sync entries (F17, seed 106).
4. An honest leader was demoted when `PayloadReady` raced an empty build (F2, seed 115).

Reconciling the spec with the code found four more incomplete rules (Appendix E, E5–E8).
Four code-review lenses raised 14 findings; three were confirmed and fixed: a Byzantine
source could fill the sync buffer with forged entries and block catch-up for good (high; a
spec bug), extra `Init` configurations let commits run more than two heights ahead of apply
and then halted the core, and the crate docs named the old v2 spec.

A 500-seed sweep then failed F5 seed 279 (n = 22, crash churn). Byzantine leaders raised
honest start levels, one height took 26.4 s, and a replica committed only 4 heights after
heal. The start level now rises only on slow local execution (including pending executions,
maximum per height) or on a slow committing view, measured from when this node held the
proposal and its body. A proven leader equivocation causes an early timeout. An adversarial
review of that change found two more issues: slow executors could keep every height empty at
view 2, and late but valid leaders could push every honest node to the start cap. Both were
fixed with tests, mutations ML25–ML30 and scenario F36 (late leaders). Seed 279 now commits
81 heights after heal.

## Evidence

Final results, 2026-09-26, on the working tree above:

| Check | Command | Result |
| --- | --- | --- |
| Unit and scenario tests (debug) | `cargo test -p iroha_sumeragi` | 294 passed, 2 ignored (heavy run and report); spec tests 3 passed; about 24 s |
| Simulator sweep (release) | `SUMERAGI_SIM_SEEDS=500 cargo test -p iroha_sumeragi --release --features sim` | 294 passed; all 36 fault scenarios F1–F36 pass 500 seeds each; about 433 s |
| Mutation gate | `python3 scripts/sumeragi_mutation_gate.py --jobs 4` | Baseline passes; 105 of 105 mutations killed by their named deterministic tests; 0 survived; report `target/sumeragi-mutants/report.json` |
| Lints | `cargo clippy -p iroha_sumeragi --all-targets --all-features --no-deps -- -D warnings` | Clean. `--no-deps` is needed because the `simd_support` feature of `vendor/concread` trips `clippy::redundant_feature_names` in every crate. |
| Format and docs | `cargo fmt -p iroha_sumeragi -- --check`; `RUSTDOCFLAGS=-Dwarnings cargo doc -p iroha_sumeragi --no-deps` | Clean |

Core size in non-blank, non-comment lines
(`cargo test -p iroha_sumeragi --test spec core_size_budget -- --nocapture`):

| Module | Lines |
| --- | --- |
| types | 265 |
| topology | 274 |
| message, preimage and crypto | 976 |
| safety | 721 |
| pacemaker | 330 |
| machine | 3 022 |
| sync | 410 |
| api | 316 |
| Total (budget 8 000, spec §12.6) | 6 314 |

Tests add about 6 500 lines in `machine/tests` plus inline tests; the simulator is about
10 000 lines.

Simulated performance at a 1 s target block time
(`cargo test -p iroha_sumeragi --release --features sim report -- --ignored --nocapture`).
This is a simulated network, not a real-network measurement.

| n | Case | Average gap | p99 gap | Commit latency | Messages per height |
| --- | --- | --- | --- | --- | --- |
| 4 | normal | 1035 ms | 1078 ms | 155 ms | 16 |
| 4 | silent proxy tail | 977 ms | 1330 ms | 266 ms | 25 |
| 4 | crashed set-A member | 1057 ms | 1099 ms | 157 ms | 16 |
| 4 | 10 % loss | 1021 ms | 1622 ms | 269 ms | 23 |
| 22 | normal | 1034 ms | 1073 ms | 214 ms | 197 |
| 22 | silent proxy tail | 1029 ms | 1399 ms | 184 ms | 338 |
| 22 | crashed set-A member | 1052 ms | 1062 ms | 171 ms | 295 |
| 22 | 10 % loss | 986 ms | 1184 ms | 210 ms | 287 |

## Not done

Tracked as goals S1–S8 in the [Sumeragi goals](../../../specs/sumeragi_goals.md):

- No CI job runs the mutation gate or the spec test yet, and the 10 000-seed nightly
  simulator run does not exist (S1).
- Node driver and integration: P2P traffic classes, Kura, the definition of the execution
  result `R`, per-key safety-record files (S2, S3).
- Cutover at `SumeragiStartArgs::start` and deletion of the v2 runtime, about 1.5 million
  lines including formal models, scripts and Python tests (S4).
- Dataspace and lane instances and AMX two-phase commit (S5, S6).
- Taira reset and a 24 h real-network soak (S7).
- Evidence penalties and NPoS committee scheduling (S8).
