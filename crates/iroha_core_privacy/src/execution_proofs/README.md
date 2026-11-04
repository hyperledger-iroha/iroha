# Native execution proofs: RaceV1

This module is a native Iroha execution relation and local transparent prover. It does not accept
RISC Zero, SP1, Groth16, a binding-only AIR, a signed server result, or successful host replay as a
substitute for the cryptographic proof. The compiled profile remains **unqualified** and native stake
admission must stay closed until the release gates below pass.

`race.rs` is the node's reference simulation and the prover's witness generator. `proof.rs` binds the
canonical public replay, the initial, checkpoint and final states and the statement envelope to a proof
from `iroha_execution_stark` ([spec](../../../../specs/execution_stark_v1.md)). Its RaceV1 circuit
(`iroha_execution_stark::games::race_v1`) proves one tick per row of a 2^13-row trace with a
Goldilocks/Fp4 DEEP-ALI STARK, LogUp range checks against a 16-bit table, BLAKE2b-384 commitments
and transcript (48-byte digests), and no masking. `race_air.rs`, `environment_air.rs` and `integer_air.rs` remain only for the
browser kernel export. The verifier builds the public columns (controls, weather and removals) itself;
it does not execute vehicle physics to decide whether a result is valid.

## Statement and history binding

The generic envelope binds network identity, session identity, immutable manifest and participant
roster hashes, full cumulative transcript root, retained dispute-history root, generic outcome hash,
and exact profile ID. Race-specific track, physics rules and result are inside the adapter payload.
The profile ID hashes the profile descriptor, the rules, the node simulation and adapter, the payload
models and every `iroha_execution_stark` source. Only compiled profiles can be registered. That ID is a
255-bit ledger `Hash`, so every proof statement also binds a full-strength BLAKE2b-384 commitment to
the same descriptor, rules and (path, source) pairs, and the statement's canonical bytes enter the
transcript unhashed.

`registry.rs` contains the closed compiled catalog and generic dispatch. The first release has one canonical stock relation, proved with `iroha_execution_stark`; obsolete draft profile and proof formats, including the earlier staged Poseidon relation, have no dispatch path. `race_result_v1` ranks every eligible racer before all forfeits, including forfeited earlier finishers. Exact eligible finish or distance ties split awards, and all-forfeit outcomes have no winners. Registry and export tooling are outside the profile source commitment; the actual rules, arithmetic, reference outcome and cryptographic engine are committed. Changes to those sources produce a new exact identity and require fresh qualification.

The RaceV1 proof request and payload require the immutable `GameAdmissionBodyV1` immediately
after the manifest. The verifier checks the canonical original wallet/input-key/data roster,
explicit wager and resource projections, recomputes the sole network/session-bound `roster`
commitment, and enforces the compiled adapter's admission geometry. Stock racing admits 2–8
participants with one cosmetic byte each and no equipment. Mutable forfeits, payout recipients
and custody heights are not part of this immutable projection. NFT ownership is established by
authenticated ledger admission and finalized settlement inclusion, rather than by a public worker.
The profile source commitment now includes the native game/resource/execution payload models
that define this validation and encoding. Their changes require fresh proof qualification.

The RaceV1 proof payload carries the generic manifest/admission/outcome, adapter-specific relation statement,
entire canonical replay, terminal state, optional intermediate
checkpoint state, and the actual STARK wire. Controls are public fixed columns, so transcript hashing
does not need to be proved inside the arithmetic circuit. The full public input is bound into the
Fiat–Shamir transcript before commitments/challenges are used. A replaceable prover learns no wallet
secret and cannot choose a different result or replay without generating another valid proof.

Settlement must use `verify_game_proof_for_history_v1` with the retained checkpoint, anchors, forced
batches, participant roster and epoch grouped in `GameProofHistoryV1`. It compares the actual retained checkpoint and
forced batches, verifies the certified prefix root and state root, requires exact slot-ordered forced
controls, and derives pre-tick DNF events from chain state. The engine statement binds that
intermediate state as a boundary at its tick. Removal ticks are public columns: the final state must
record every removal, including those at the final tick, and a checkpoint exactly the removals before
its tick, matching its certified transcript prefix. Consensus removals after a cryptographically proved terminal boundary have no
retroactive effect on winners. `verify_execution_proof_v1` verifies semantics and returns the adapter-authenticated generic outcome; it intentionally does
not authenticate arbitrary caller-supplied participant rosters or transcripts for payouts.

## Reusable application boundary

`GameManifestV1` carries application identity, compiled profile identity, opaque application parameters,
participant/input limits, access and payout policy. `GameTranscriptV1` carries opaque slot-ordered
input batches and consensus removal events. Transcript roots hash this generic type with the
`iroha:game:session:v1\0` context and `input-transcript` domain. State roots hash the canonical
`Vec<u8>` encoding of the application's state bytes under `simulation-state`. The RaceV1 adapter
converts its own replay/state types into those generic formats.

The generic `GameOutcomeV1` carries terminal tick, winner slots and opaque result bytes. RaceV1 derives
all three from the proof-bound final state; it never accepts caller-selected payout allocations. Native
escrow interprets the retained immutable payout policy only after that adapter validation succeeds.
Manifest validation and participant/input validation dispatch through the compiled profile catalog.
For RaceV1, parameters encode exactly `RaceTrackV1`, participant data is one skin byte, and each active
input batch is six little-endian `u16` masks. DNFs use empty generic input payloads.

New applications reuse the same generic native lifecycle, profile registration, proof verification and
settlement ISIs. An application using the existing race relation only supplies its manifest and website.
A different execution relation requires a new reviewed, compiled adapter and explicitly versioned profile
catalog update. Registering a profile cannot install arbitrary code, replace cryptographic primitives or
weaken parameters. Arbitrary uploaded IVM programs are **not** currently claimed to be provable.

## Arithmetic soundness obligations

The RaceV1 circuit is written with the engine's bounded-integer builder. Every register and
intermediate carries a proven integer interval below the field's signed range; comparisons decompose a
shifted difference with one 16-bit LogUp lookup and a Boolean; divisions constrain range-checked
quotients and remainders; track cells and objects use one-hot selectors. Contacts are evaluated in the
fixed ascending `(i,j)` order with slowing deferred to the end of the tick, which the node simulation
defines identically. With the fixed initial boundary and the next-row identities, induction makes each
row the unique integer transition. The gadget contracts, the trusted `assume_interval` uses and the
adapter's obligations are in the [spec](../../../../specs/execution_stark_v1.md) (§3, §4 and §8);
`crates/iroha_execution_stark/tests/race_oracle.rs` checks the circuit against this module's
`race.rs` on every tick of full races.

Finished and DNF cars are frozen and have no collision body. DNF records input-key inactivity even
when a car already finished; the earlier finish remains history, but the DNF car is ineligible for a
prize. Finishes retain their actual tick while the last six-tick batch is consumed to its boundary.
At a six-tick boundary, fewer than two active keys also ends the game: the earliest eligible finishers
share priority, otherwise the sole eligible survivor wins. An all-forfeit outcome refunds regardless
of historical finishes. At the 5,400-tick limit, if no eligible racer finished, the eligible racers with
greatest progress share the prize. An unfinished active race therefore cannot become a
refund merely by refusing to cross the line. Ranking is derived from the complete proof-constrained
terminal state and bound into the outcome statement; timeout adds no separate arithmetic witness.
The circuit proves that some car is still racing at every stepped six-tick boundary; the adapter
checks the public half (with two or more cars, at least two stay unremoved). Removals take effect
before their tick, and removals at the final tick change no register. No unproved forfeit payout exists.

The default tests prove and verify genuine races (`proof_tests.rs`): statement, roster, checkpoint,
replay and final-state forgeries are rejected, and forged node states cannot be proved. The engine's
own tests cover parameter validation, canonical decoding, multiplicity canonicality and the size
bound. Neither establishes STARK soundness or transport feasibility.

## Cryptographic geometry and limits of the security claim

The engine uses the Goldilocks field with the quartic extension `X^4 - 7`, BLAKE2b-384 (RFC 7693:
a native 48-byte digest with a 512-bit chaining state) for Merkle trees with batch openings and the
Fiat–Shamir transcript, one Fp4 DEEP point, blowup 8, 87 queries, arity-8 FRI down to degree 16, and
20-bit grinding that is not counted as security. The proof wire is `IEB1`; the earlier BLAKE3 `IES1`
wire is rejected. There is no trusted setup and no masking: replays are public. Query rounds are
sized for a 129-bit per-round target in the Johnson regime (ethSTARK v1.2, Corollary 3). Before
every proof and every verification the adapter checks the exact circuit's hash-limited budget
against this profile's 128-bit target (`Params::check_security`) and fails closed below it; the
RaceV1 circuits reach about 129 bits. Meeting the target is necessary but is not release
qualification: the profile stays unqualified. The [spec](../../../../specs/execution_stark_v1.md)
(§7) gives the accounting and what an independent cryptographic review must still cover: AIR
completeness, DEEP/ALI composition, transcript order, domain separation and decoding.

## Interactive environment

Every track has twelve fixed object cells per lap, repeated over three laps. Immutable tree, sign and oil catalogs, rain phases and wind tables are committed in the rules and compiled profile. Tree/sign contacts use swept forward progress and strict lateral contact radii; stationary cars cannot repeatedly hit the same object. Oil changes damping and steering inside a bounded inclusive rectangle; rain reduces steering and lateral speed. Wind contributes `trunc(gust * speed / 2400)`, so parked cars remain stationary.

The whole-tick graph exports the exact `oil_contact`, `solid_hit` and object-kind predicates used in the state computation for browser effects. These outputs are gated by the car's active state. They add no mutable ledger state. The exported fixed prefix includes native-derived rain and wind values, and the native verifier reconstructs those values from the public track and tick. Browser presentation cannot choose physics parameters.

## Resource evidence and qualification

Eight-car circuits have 1,321 main columns (56 registers), 49 public columns, 1,201 constraints and
576 lookups over a 2^13-row trace for every track. Their computed worst-case wire is 2,044,445 bytes,
below the explicit 3,250,000-byte STARK admission cap; actual eight-car proofs are about 1.66 MB.
The codec test bounds the maximum-duration envelope and typed settlement below the shared 4 MiB cap.
These are codec bounds, not validator-throughput measurements; network transaction, block, Torii,
Connect and gossip limits remain separate admission requirements.

The engine [README](../../../iroha_execution_stark/README.md) records proving and verification
times and their measured scopes; retained BLAKE2b-384 runs on a shared Apple M4 Max proved full
eight-car, 5,400-tick races in 2.160–3.547 s and verified them in memory in 28.2–39.8 ms.
These standalone measurements exclude Core settlement and network costs.
The `full_duration_eight_car_proof_is_thread_deterministic_and_round_trips`
release gate proves that race at 1, 2 and 16 threads, requires identical bytes, and retains the request
and proof with `IROHA_EXECUTION_PARITY_OUTPUT`. The previous staged
Poseidon engine's evidence under `sora-cars/output/qualification/native-proof/` (profile
`c6b9b7536ca0a1d7c3f25ad67208927d36e79b5385af516cb93177a82eabc925`: 525.9 s and 25.7 GB peak to
prove one eight-car race) describes the removed construction and qualifies nothing here.

Browser generation checks the exact repository-relative source inventory shared with the native profile commitment, so unchanged physics cannot conceal stale admission or verifier code. All 17 regular CLI finality tests and the genuine proof inclusion test in a signed two-height fixture ledger passed with BLAKE2b-384 on 2026-11-03. The same Core gate attempt failed codec target selection, backend inventory, four-validator fee admission and source stability; it does not qualify a release. Before funding activation, complete supported-target and accelerated-path qualification, independent soundness review, actual four-validator adversarial settlement and finality verification, deployed wallet interoperability, and deployment qualification. The engine uses scalar arithmetic plus Rayon; unrelated FASTPQ GPU kernels do not establish execution-proof acceleration parity. No profile is marked cryptographically qualified.
