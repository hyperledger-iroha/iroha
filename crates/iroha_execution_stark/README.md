# iroha_execution_stark

A bounded-integer execution STARK: transparent proofs that a deterministic integer step function,
applied to public inputs that every verifier rebuilds (for example a game's replay of player
controls), takes a claimed initial state to a claimed final state. A caller describes one step with
a bounded-integer circuit builder; the crate proves and verifies whole runs with a DEEP-ALI STARK
over Goldilocks, BLAKE2b-384 commitments and FRI. It is not a general zkVM and gives no witness privacy.
SORA Nexus applications whose transitions fit this bounded-integer circuit model can use it;
the SORA CARS Stock race (`RaceV1`) is the first circuit, in
[`src/games/race_v1.rs`](src/games/race_v1.rs).

The protocol, security accounting and design record (including the literature review behind it)
are in [`specs/execution_stark_v1.md`](../../specs/execution_stark_v1.md).

## Full RaceV1 race (8 cars, 5,400 ticks), Apple M4 Max

The two provers were measured on different replays with the same geometry (8 cars, 5,400 ticks),
so this is not a controlled same-input comparison. This crate's trace dimensions are fixed by the
circuit shape (2^13 rows); instruction counts varied by under 1% across the measured replays.
These retained measurements describe the measured binaries, not a fresh benchmark of every later
review fix. The prove-only range includes other runs and must not be combined with the single
whole-process measurement to derive a speedup.
The comparison table below describes the earlier BLAKE3 prototype. The retained BLAKE2b-384
`benchmarks-v2` run on 2026-11-03 measured eight-car, 5,400-tick proofs on all three tracks at
2.160–3.547 s for `prove()` and 28.2–39.8 ms for in-memory verification, using 16 Rayon threads.
Raw proofs were 1,659,020–1,664,865 bytes. Proving includes witness/trace construction but excludes
native replay simulation, reference-state preparation, circuit setup, serialization and verification;
verification excludes decoding. Evidence is retained in
`sora-cars/output/reviews/execution-stark-20261103/security-upgrade/benchmarks-v2/`.
These shared-machine measurements establish neither end-to-end settlement latency nor qualification.

| Scope | Previous native prover (streaming lab build, 16 threads) | Earlier BLAKE3 prototype of this crate |
|---|---:|---:|
| Replay | lane-keeping, always-throttle lab replay | `race_bench NeonTokyo 8 55 1` (random 55% throttle, one removal at tick 2,623) |
| Whole process, wall | 126.03 s | 0.52–0.53 s (replay generation, native simulation, circuit build, prove, verify) |
| Whole process, work | 881.5 CPU-s; 10.2 × 10^12 instructions | ≈4.7 CPU-s; 62.5 × 10^9 instructions |
| Prove only, wall | 121.58 s (including self-verification) | 0.49–1.04 s across runs and tracks (no self-verification) |
| Verify only | 2.2 s (Core native build) | 11–20 ms |
| Peak memory | 5.1 GB footprint (Core native: 25.7 GB RSS) | 1.4 GB footprint |
| Raw STARK bytes | 2,494,704 | 1,616,725 |
| Committed trace | 2^19 rows × 379 base + 101 fixed columns | 2^13 rows × 1,321 base + 145 extension columns |

The previous figures are the retained runs in `sora-cars/output/qualification`
(`codex-proof-takeover-20261025/memory/lab-validation/full8-repaired-t16.log` and the
`execution_proofs` README). The Core native build's 3.09 MB figure is the whole execution envelope
(STARK plus replay and statement), not the raw STARK. Instruction counts come from
`/usr/bin/time -l`; they measure executed work rather than elapsed time. Wall times are sensitive
to machine load, and this machine was shared. The builds, replays, verification scopes and memory
metrics differ, so the table does not establish a same-workload speedup or release qualification.

Where the speed comes from, in order of effect:

1. **Arithmetization.** One trace row per tick holds the whole step (8 cars and 28 ordered
   contacts), instead of 85 multiplexed microcycle rows; contact slowing is applied once per car
   (`max(speed - 120 k, 0)`); weather, controls and removals are verifier-built public columns.
2. **Commitments.** Byte-oriented row hashing instead of six-lane Poseidon sponges, and no trace
   masking (replays are public). The current suite uses native BLAKE2b-384; the retained timings
   above used BLAKE3.
3. **Lookups.** LogUp range checks on 16-bit limbs with a verifier-summed table, four lookups per
   helper column, comparisons as one lookup of `(a - b) + 2^K * bit`.
4. **Engine.** Quotient split into a degree-3 group on a 2n domain and a degree-5 helper group on a
   4n domain; coefficient-space DEEP batching; cache-tiled constraint evaluation; shared coset
   powers and per-stage twiddles in the NTT; parallel everything.

## Writing a circuit

```rust
use iroha_execution_stark::{Builder, Expr, Params, Statement, prove, verify};

let mut b = Builder::new("my-application v1", 13); // 2^13 rows: up to 8,191 steps
let push = b.public("push", -3, 3); // a public input, rebuilt by every verifier
let position = b.register("position", -1_000_000, 1_000_000);
let velocity = b.register("velocity", -400, 400);

let drag = b.div_trunc(&velocity.scale(7), 8, "drag"); // exact truncating division
let raw = drag.add(&push.scale(10));
let velocity_next = b.clamp(&raw, &Expr::constant(-400), &Expr::constant(400), "clamp");
let position_next = b.clamp(
    &position.add(&velocity_next),
    &Expr::constant(-1_000_000),
    &Expr::constant(1_000_000),
    "bounds",
);
b.set_next(&position, &position_next);
b.set_next(&velocity, &velocity_next);
let circuit = b.finish();
// prove(&circuit, &Params::standard(), &statement, &public_columns, &states)?;
// verify(&circuit, &Params::standard(), &statement, &public_columns, &proof)?;
```

Every value is an integer with a proven interval far below `p/2`, so field arithmetic matches
integer arithmetic. Gadgets: `mul`, `select`, `and`/`or`/`not`, `lt`/`le`/`gt`/`ge`, `min`/`max`/
`clamp`/`abs`, `div_floor`/`div_trunc`/`div_rem_floor`, `one_hot`/`one_hot_quotient` with
constant tables, `range_check`, `narrow` (an enforced bound), `assert_zero_when`,
`assert_nonzero_when`. Each committed column comes from a gadget that fully constrains it.

Rules that keep a circuit sound and provable:

- Keep a native reference simulation and pass its states to `prove`; the prover evaluates the
  circuit on every row and refuses to prove if any next state differs from the reference.
- Compare the circuit with the reference on many replays with `check_trace` (see the RaceV1 tests)
  and compare the reference with the node's own rules (see `tests/race_oracle.rs`).
- `assume_interval` / `assume_boolean` are trusted compiler assertions, not constraints. Use them
  only for facts implied by constraints already added; use `narrow` otherwise.
- Rows after the last step freeze the state (public `active` selector), but every gadget is still
  evaluated there; gate anything that must not happen in padding with `b.active()`.
- Checks that only depend on public data (replay structure, removal schedules) belong in the
  application's adapter, as `race_v1::validate_replay` does.

## Parameters and security

`Params::standard()`: blowup 8, 87 FRI queries, arity-8 folding, final degree 16, 20 grinding bits.
Queries are sized with the proven Johnson-regime bound (BCIKS20, ethSTARK v1.2), not the refuted
"up to capacity" conjecture; all challenges, LogUp and out-of-domain points are in
`Fp4 = Fp[X]/(X^4 - 7)`. The standard query budget targets 129 bits: at the smallest trace it gives
129.522 bits before the one-bit per-round margin. For a circuit whose query term is the weakest,
`Params::proven_security_bits` therefore reports 128.522 bits at that height; the full result also
depends on the circuit's identities, degree, lookup table and DEEP batch width. The native
BLAKE2b-384 suite uses 48-byte digests and a 512-bit chaining state; RFC 7693 assigns it a
192-bit collision-security target. This is a primitive change, not extended BLAKE3 output.
The ethSTARK Corollary 3 hash condition therefore imposes a 190.5-bit ceiling; the full-proof
budget is the minimum of that ceiling and the circuit-specific terms. Standard RaceV1 shapes
target at least 128 classical bits under this accounting, not 190.5 full-proof bits. See
[RFC 7693](https://www.rfc-editor.org/rfc/rfc7693.html#section-4) and
[ethSTARK v1.2](https://eprint.iacr.org/2021/582).

`Params::check` rejects parameter/circuit combinations the prover, verifier or codec cannot use.
`Params::check_security(&circuit, TARGET_SECURITY_BITS)` additionally enforces the actual
circuit's hash-limited soundness budget and rejects weaker profiles; it does not grant release
qualification. The RaceV1 qualification gate remains separate. Grinding is never counted.

## Benchmarks and tests

```sh
cargo run -p iroha_execution_stark --release --example race_bench -- NeonTokyo 8 55 1
IROHA_EXECUTION_STARK_TIMINGS=1 cargo run -p iroha_execution_stark --release --example race_bench -- Harbor 8 55 2
cargo run -p iroha_execution_stark --release --example race_census
cargo test -p iroha_execution_stark --release
```

`race_bench` takes `<NeonTokyo|Harbor|Sakura> <players 1-8> <throttle-percent 0-100> <seed>` and an
optional lookups-per-helper (1–4); anything else exits with status 2. It prints the binary hash,
arguments, Rayon threads, circuit identity, replay digest and proof hash before the timings. The
tests cover field, NTT, Merkle, transcript and FRI units, a toy game with tampering and divergence rejection,
every RaceV1 tick against the native port (full races in release builds), padding and adapter
regressions, and `tests/race_oracle.rs`, which compiles `iroha_core_privacy`'s own `race.rs` and
`environment.rs` and compares states tick by tick.

## Status

The engine, builder and RaceV1 circuit are implemented and have undergone bounded code reviews
(see the spec). Those reviews do not replace an independent cryptographic audit or release
qualification; the RaceV1 profile remains unqualified. Work still required:

- Done (2026-11-03): the node's RaceV1 execution proof in `iroha_core_privacy` proves and verifies
  with this engine behind the unchanged envelope, registry profile and settlement path; the old
  staged relation was removed. SDKs carry the unchanged payload shape (the proof bytes are opaque),
  so no SDK fixture changed.
- TODO: ClassedRaceV1 (unregistered), the IVM step chips and Touring still use the older
  proof-managed-note engine. The expensive CLI end-to-end settlement test remains explicitly
  ignored by default; its 2026-11-03 BLAKE2b-384 run and all 17 regular finality tests passed.
  That Core gate attempt still failed other checks, including the four-validator gate and source stability.
- TODO: Metal/CUDA kernels for the LDE and Merkle phases (about 70% of proving time).
- TODO: schedule one step over several rows to shrink wide proofs, and Street/Touring circuits.
