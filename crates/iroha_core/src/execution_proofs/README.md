# Native execution proofs: RaceV1

This module is a native Iroha execution relation and local transparent prover. It does not accept
RISC Zero, SP1, Groth16, a binding-only AIR, a signed server result, or successful host replay as a
substitute for the cryptographic proof. The compiled profile remains **unqualified** and native stake
admission must stay closed until the release gates below pass.

`integer_air.rs` is the reusable bounded-integer constraint compiler. `staged_race_air.rs` compiles the
complete narrow race relation. `race_air.rs` provides the equivalent complete-tick graph used to
generate the browser kernel. `race.rs` is a separate readable reference implementation and local replay
tool. `proof.rs` binds canonical public replay data, initial/checkpoint/final boundaries, and the
native Goldilocks/Fp4 DEEP-ALI/FRI driver. The verifier constructs public fixed columns; it does not
execute vehicle physics to decide whether a result is valid.

## Statement and history binding

The generic envelope binds network identity, session identity, immutable manifest and participant
roster hashes, full cumulative transcript root, retained dispute-history root, generic outcome hash,
and exact profile ID. Race-specific track, physics rules and result are inside the adapter payload.
The profile ID hashes the rules, geometry descriptor, complete compiled integer relation and verifier,
and the reused transparent proof-driver sources. Only compiled profiles can be registered.

`registry.rs` contains the closed compiled catalog and generic dispatch. The first release has one canonical stock relation using the wide Poseidon2 execution substrate in `stark/`; obsolete draft profile and proof formats have no dispatch path. `race_result_v1` ranks every eligible racer before all forfeits, including forfeited earlier finishers. Exact eligible finish or distance ties split awards, and all-forfeit outcomes have no winners. Registry and export tooling are outside the profile source commitment; the actual rules, arithmetic, reference outcome and cryptographic engine are committed. Changes to those sources produce a new exact identity and require fresh qualification.

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

Settlement must use `verify_game_proof_for_history_v1`. It compares the actual retained checkpoint and
forced batches, verifies the certified prefix root and state root, requires exact slot-ordered forced
controls, and derives pre-tick DNF events from chain state. The AIR binds that intermediate state at
the checkpoint row. Consensus removals after a cryptographically proved terminal boundary have no
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

Each arithmetic witness node has a polynomial equation. Addition, subtraction, multiplication and
selection have degree at most two. Comparison decomposes a bounded signed difference plus a power-of-
two offset, using Boolean top bits and radix-four digits. Every radix-four digit satisfies
`d(d-1)(d-2)(d-3)=0`; decomposition reconstructs the complete value. Unsigned division constrains
quotient/remainder ranges, `a=d*q+r`, and `r<d`. Signed division separately constrains sign, magnitude
and truncation toward zero. Track curvature uses a complete Boolean one-hot lookup with sum one and
an exact index reconstruction.

All integer intervals remain below `2^50`; the actual race arithmetic is much smaller. With the fixed
initial boundary and complete next-row equations, induction makes each row the unique integer
transition. Range checks prevent a prover from interpreting arbitrary field residues as signed
integers. The important review obligation is every manually narrowed interval in `race_air.rs`,
particularly maximum contact displacement and lookup result bounds. A host branch in the witness
generator is never itself a verifier constraint.

The narrow trace executes one boundary row, drive/curve/movement rows for each car, every ascending
`(i,j)` contact row, and a finish row per car. Eight cars use 61 rows per tick. Complete car state and
a curvature register carry between rows. Public fixed selectors enforce the complete ordered schedule.
The stage compiler multiplexes normal, Boolean and radix-four banks. Stage arithmetic equations are
gated (degree at most three), while all shared range-bank equations remain unconditional (degree four).
Multiplying a stage selector by a quartic range equation would be degree five and is explicitly avoided.

Finished and DNF cars are frozen and have no collision body. DNF records input-key inactivity even
when a car already finished; earlier finishers retain prize priority. Finishes retain their actual tick,
while the last six-tick batch is consumed to its boundary. At a six-tick boundary, fewer than two active
keys also ends the game: prior finishers win first, otherwise a sole active survivor wins, and all DNF
refunds when nobody previously finished. At the 5,400-tick limit, if nobody finished, the eligible
racers with greatest progress share the prize. An unfinished active race therefore cannot become a
refund merely by refusing to cross the line. Ranking is derived from the complete proof-constrained
terminal state and bound into the outcome statement; timeout adds no separate arithmetic witness.
Two inverse registers constrain nonterminal batch admission after the exact current DNF phase.
The final public state is constrained after a disabled microtick applies any terminal-boundary DNF;
checkpoint states are constrained before their boundary DNF phase. No unproved forfeit payout exists.

The finite-difference test checks the complete residue evaluator on arbitrary field lines and must
observe exactly degree four. Differential tests compare the arithmetic trace to the separate native
simulation. Neither test alone establishes STARK soundness or transport feasibility.

## Cryptographic geometry and limits of the security claim

The profile uses the existing native Goldilocks base field and irreducible quartic extension,
six-lane Poseidon commitments, framed Fiat–Shamir transcript, one extension-field DEEP point, four
composition chunks, and unique unbiased FRI queries. There is no trusted setup.

The native trace is the smallest power of two covering all microcycles plus the final boundary,
with minimum 8,192 and maximum 524,288 rows. The eight-times LDE spans 65,536 to 4,194,304 rows.
With maximum degree four, 136 FRI queries, one Fp4 DEEP query, and reduced AIR degree three, the
existing masking check requires `2*3*(4*1+136)+136 = 976` coefficients, hence mask degree 975.
At the minimum domain the masked trace has at most 9,168 coefficients, below `65,536/7`;
the ratio only improves for larger domains. Six through twelve binary folds terminate at 1,024
values with degree bound 143; `144/1,024 <= 1/7`.

The existing machine-checked affine-batched FRI certificate verifies the exact rational query term
`(7/36)^68 < 2^-160`. At the maximum LDE domain log 22, its two algebraic commitment terms use
the extension-field lower bound 252 bits and combine below `2^-187`; at the minimum domain the
bound is `2^-199`. The execution-only maximum domain and minimum bound are distinct from the
unchanged privacy maximum trace log 14 / commitment term 197. These are **FRI theorem terms**,
not independent evidence of complete-system security. The release target is 128 bits. A complete
review must cover AIR completeness, DEEP/ALI composition, transcript ordering, Poseidon assumptions,
domain separation, query/grinding behavior, exact decoding, and the union of all failure terms.
Twenty-bit grinding is nonadditive and is not counted as extra security.

The execution-only [qualification record](SECURITY_REVIEW.md) identifies the remaining proof obligations. The sole compiled suite uses the published wide Poseidon2 construction with a complete binary fold schedule. Privacy proof formats remain separate. Obsolete development proof and profile formats have no runtime dispatch entry.

## Interactive environment

Every track has twelve fixed object cells per lap, repeated over three laps. Immutable tree, sign and oil catalogs, rain phases and wind tables are committed in the rules and compiled profile. Tree/sign contacts use swept forward progress and strict lateral contact radii; stationary cars cannot repeatedly hit the same object. Oil changes damping and steering inside a bounded inclusive rectangle; rain reduces steering and lateral speed. Wind contributes `trunc(gust * speed / 2400)`, so parked cars remain stationary.

The whole-tick graph exports the exact `oil_contact`, `solid_hit` and object-kind predicates used in the state computation for browser effects. These outputs are gated by the car's active state. They add no mutable ledger state. The exported fixed prefix includes native-derived rain and wind values, and the native verifier reconstructs those values from the public track and tick. Browser presentation cannot choose physics parameters.

## Resource evidence and qualification

Current eight-car staged widths are 379 columns for NeonTokyo, 381 for Harbor and 377 for Sakura; the compiled ceiling is 384, plus eight shared inactive copy cells and 118 auxiliary columns. Eight cars use 85 microcycles per tick, and the full 5,400-tick run still pads to 524,288 rows. Shared one-hot lookup selectors and four scratch values avoid duplicating object-table proofs across movement stages.

The native codec enumerates all 21 track/roster layouts and permitted trace domains. The cryptographic wire bound is 3,166,240 bytes and its explicit admission cap is 3,250,000 bytes. The current native bound test passes with a conservative 3,659,186-byte envelope and 3,659,497-byte typed settlement, including a 271,970-byte admission body bound. It reserves eight independently bounded 32-KiB encoded account controllers, eight 1-KiB encoded NFT identifiers, exact Ed25519 key geometry, cosmetic bytes and all canonical field/sequence framing. The complete envelope cap remains 4 MiB. These are codec bounds, not generated-proof or validator-throughput measurements. Network transaction, block, Torii, Connect and gossip limits remain separate admission requirements.

The actual production-module qualification harness passes all 52 regular and explicit expensive execution tests, including all 1,572,864 full-duration microcycle rows across three tracks; every row matches the independent native reference and has zero constraint residues. Environmental boundary tests exercise 2,592 wet/dry object-edge cases and reject altered computed outputs. Degree-four and fixed-query LDE parity tests also pass. These facts do not constitute independent cryptographic review or a successful full-node integration run.

Execution verification rejects malformed wire, transcripts and Merkle openings before constructing public fixed traces. It evaluates the 136 authenticated query positions using exact subgroup Lagrange evaluation of the same public polynomials committed by the prover. The CPU prover uses actual Goldilocks/Fp4 arithmetic and wide Poseidon2 commitments; workers require public replay data only.

Current immutable-admission evidence is retained in `sora-cars/output/qualification/native-proof/admission-first-release/20260906T125522Z/qualification.json` for profile `c6b9b7536ca0a1d7c3f25ad67208927d36e79b5385af516cb93177a82eabc925`. Its genuine eight-car/5,400-tick envelope is 3,094,036 bytes. Proving plus internal verification took 525.902 seconds with 25,739,657,216 bytes peak RSS; separate verification took 2.233 seconds with 616,153,088 bytes peak RSS. The fixed-entropy CPU matrix also completed at widths 1, 2 and 16: byte-identical 3,099,124-byte envelopes and three independent CLI verifications. Proving/internal verification took 2,929.937, 1,751.132 and 480.046 seconds, respectively. These shared-host measurements are not isolated throughput or whole-validator costs. The one-worker run exceeds the unchanged 30-minute proof-worker limit.

Browser generation checks the exact repository-relative source inventory shared with the native profile commitment, so unchanged physics cannot conceal stale admission or verifier code. Actual complete CLI33 passes all 13 regular finality tests and genuine proof inclusion in a signed two-height fixture ledger through direct carriers, the first-release tagged file and hashed index. Before funding activation, complete supported-target and accelerated-path qualification, independent soundness review, actual four-validator adversarial settlement and finality verification, deployed wallet interoperability, and deployment qualification. The current execution backend uses scalar arithmetic plus Rayon; unrelated FASTPQ GPU kernels do not establish execution-proof acceleration parity. No profile is marked cryptographically qualified.
