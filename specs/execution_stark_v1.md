# Bounded-integer execution STARK v1 (`iroha_execution_stark`)

Status: implemented in `crates/iroha_execution_stark` (engine, circuit builder, SORA CARS RaceV1
circuit). Since 2026-11-03 the node's RaceV1 execution proof (`iroha_core_privacy::execution_proofs`)
proves and verifies with it; the earlier staged Poseidon RaceV1 relation was removed. The RaceV1
profile remains unqualified. Written 2026-11-03.

## 1. Statement

A circuit describes one step of a deterministic bounded-integer state transition. A statement fixes
the circuit, the number of steps `T`, the register vector before the first step and after the last,
an optional checkpoint (the registers after `c <= T` steps), an application-defined binding and the
public columns the verifier rebuilds from public data. The standalone RaceV1 adapter binds a
canonical replay digest; a node adapter must additionally bind its execution envelope. The proof
shows that applying the step `T` times from the initial registers under those public columns
passes through the checkpoint and ends
at the final registers, and that every gadget assertion held on each applied step. Inputs are
public; this is not a general zkVM, there is no witness privacy and no trace masking.

Outcome derivation, ledger inclusion, authenticated inputs and settlement stay outside the proof
and belong to the application adapter and the node (see section 8).

## 2. Primitives

- Base field Goldilocks `p = 2^64 - 2^32 + 1`; extension `Fp4 = Fp[X]/(X^4 - 7)` (7 generates
  `Fp*`, so it is a non-residue and the binomial is irreducible). All verifier challenges,
  LogUp values, quotients and out-of-domain values are in `Fp4`.
- Trace domain `H = <w>`, `|H| = n = 2^log_rows`, `8 <= log_rows <= 22`. Low-degree extension on
  `7 * <v>`, `|<v>| = N = 8n` (blowup 8), stored as 8 cosets of `H`.
- Native BLAKE2b-384, with 48-byte digests and a 512-bit chaining state. The shared `hash` module
  fixes the suite identifier and length-frames each hash's purpose and input items. Separate
  purposes cover Merkle leaves and nodes, transcript state, challenge counter blocks, proof of
  work, circuit identities, public columns and the standalone replay binding. The transcript keeps
  the full 48-byte state; field challenges retain rejection sampling over the resulting byte stream.
  This uses BLAKE2b's native 48-byte output parameter, not a longer BLAKE3 XOF output.
- Merkle leaves are whole rows (all columns of one matrix at one LDE index, little-endian canonical
  `u64` limbs). Batch openings send only siblings the verifier cannot recompute.

## 3. Circuit model

One row per step. Columns:

- **Main** (committed, base field): registers (the state before the step) and every gadget output.
- **Public** (never committed): engine columns `active` (row `< T`), `first`, `last` (row `T`),
  `row` (the step index), `checkpoint` (row `c`, or zero everywhere), then application columns
  built from public data. The verifier builds them and
  evaluates them only at the out-of-domain point by barycentric interpolation; every value must lie
  in its declared interval (checked by prover and verifier).
- **Auxiliary** (committed after the LogUp challenge, `Fp4`): one helper per four lookups and a
  running accumulator.

Every expression carries an integer interval below `2^60`. Gadgets (see the crate README)
add only columns that their constraints determine:

- products and selects: one degree-2 identity;
- `a < b`: a Boolean `s` and one range lookup of `u = (a - b) + 2^K s`, `K` a multiple of the limb
  width with `2^K > |a - b|`; a wrong `s` makes `u` negative or at least `2^K`;
- division by a constant `c`: quotient `q` with `q - lo >= 0` on a small limb budget (`c * 2^budget`
  stays below `2^60`) and the remainder checked on both sides (sign-adjusted for truncation); exact
  integer division follows because no term can wrap;
- one-hot selectors: Booleans summing to one, with the index fixed by an exact remainder check;
- `narrow`: an enforced interval; `assume_interval`/`assume_boolean`: trusted assertions that the
  prover re-checks on honest rows but the AIR does not enforce.

Range checks use limbs of `range_bits` (default 16) bits looked up in the table `[0, 2^range_bits)`.

Registers transition by `next - cur - active * (step - cur) = 0` on every row but the last domain
row; boundary identities are `first * (reg - initial) = 0`, `last * (reg - final) = 0` and
`checkpoint * (reg - checkpoint_value) = 0`.

## 4. LogUp

The prover sends the table multiplicities `m_v` (`u64`, varint-encoded) after the main root and
before the challenge `beta`. Before using them the verifier requires each `m_v <= rows * lookups`
and the exact integer sum `sum_v m_v = rows * lookups` (every lookup of every row, padding
included), with that total below `p`; field-equal aliases such as `m_v + p` are rejected. With `T_beta = sum_v m_v / (beta - v)` computed by the verifier:

- helper identity for lookups `a_1..a_k` (`k <= 4`): `h * prod(beta - a_i) - sum_i prod_{j != i}(beta - a_j) = 0`
  (the prover evaluates it with elementary symmetric sums of the base-field `a_i`);
- accumulator identity on every row, the last included: `S(wx) - S(x) - sum_j h_j(x) + T_beta / n = 0`.

Telescoping over `H` forces `sum_rows sum_lookups 1/(beta - a) = T_beta`; with lookups and
multiplicities fixed before `beta` and fewer than `p` lookups, every looked-up value is in the
table except with probability `(lookups_per_row * n + 2^range_bits) / |Fp4|` (Haböck,
ePrint 2022/1530). The count includes padding rows; `range_bits` is the circuit's actual table width.

## 5. Protocol and transcript order

Transcript: `Transcript::new("iroha_execution_stark blake2b-384 v1")`, then in order:

1. `circuit` (circuit identity), `params` (blowup, queries, arity, final degree, grinding),
   `steps`, `initial`, `final`, `checkpoint` (absent, or step and registers), `binding`, `public`
   (public-column digest);
2. `main-root`; `multiplicities` (if lookups); challenge `logup-beta`;
3. `aux-root`; challenge `composition-alpha` (one sequence: circuit and boundary identities, the
   accumulator, then helpers);
4. `quotient-root`; challenge `ood-z` (rejected if `z^n = 1` or `z`, `wz` lie in the LDE domain);
5. `ood-main`, `ood-main-next` (columns referenced at the next row), `ood-aux`, `ood-aux-next`
   (accumulator), `ood-quotient`; challenge `deep-gamma`;
6. per FRI layer `fri-root` then challenge `fri-beta`; `fri-final` (coefficients);
7. proof of work `pow-nonce` (20 bits); challenge `queries` (87 distinct indices of the LDE domain,
   exact masking).

Quotients form two groups with separate chunks: circuit, boundary and accumulator identities
(degree 3, two chunks, evaluated on a `2n` coset), and helper identities (degree 5, four chunks,
evaluated on a `4n` coset). The verifier checks both identities at `z`. The DEEP polynomial batches
every committed polynomial with powers of `gamma` at `z`, and next-referenced columns at `wz`; it is
committed as FRI layer 0 and checked against the opened rows at each query point (the construction
used by Winterfell and Plonky3). FRI folds by 8 until the degree bound is at most 16.

## 6. Proof encoding

`IEB1`, main root, multiplicities, optional aux root, quotient root, OOD vectors, FRI roots, final
coefficients, FRI layer openings, nonce, then main/aux/quotient rows and siblings at the sorted
query positions. Decoding is strict: canonical field elements and varints, bounded dimensions (at
most 1,024 rows, 2^16 row width, 32 layers, arity 16) and a total element budget charged before
allocation, no trailing bytes. Empty row matrices and empty FRI layers encode a zero width or
arity; alternative empty encodings are rejected. Proofs without lookups carry no multiplicities, auxiliary rows or
auxiliary siblings. Every root and sibling occupies 48 bytes; an `Fp4` element still occupies 32.
The retired prototype magic `IES1` is rejected, with no compatibility fallback. `max_proof_bytes`
bounds every valid encoding; it and both entry points first run
`Params::check`, which rejects unsupported values, malformed circuit dimensions (before any shift),
more queries than LDE points, a blowup below the quotient domain, FRI folds larger than their
codeword, rows wider than the codec and lookup totals at or above `p`. The maximum decoded element
count must also fit the unchanged codec budget for every query layout; configurations are rejected
even if a particularly compact individual proof could fit.

## 7. Security accounting

Per-round errors (each must stay below `2^-(lambda+1)`; ethSTARK v1.2 Corollary 3):

- query phase: `(sqrt(rho+) (1 + 1/(2m)))^s` with `rho+ = (n + 2)/N`, `m = 128`; blowup 8 and
  `s = 87` give at least 129.522 bits for every supported trace size (BCIKS20 Thm 8.3; ethSTARK eq. 10;
  Haböck ePrint 2022/1216 Thm 8);
- commit phase: `(L - 1/2)(m + 1/2)^7 N^2 / (2 rho^(3/2) |F|)` per batched round, `L` the DEEP batch
  width. At the standard blowup 8, this expression is below `2^-140` for `L = 10^4`, `N = 2^25`.
  The estimator additionally includes folding arity minus one and the number of folding rounds;
  it does not apply that numeric example to every supported parameter combination. BCHKS25
  (ePrint 2025/2055) provides a tighter bound linear in `N`;
- DEEP/ALI: `L+ (C + d (n + 2) + n) / |F|`, `L+ = (m + 1/2)/sqrt(rho+)`;
- LogUp: `(lookups_per_row * n + 2^range_bits) / |F|`.

The standard query budget targets 129 bits. At the smallest trace its raw query-phase bound is
129.522 bits; when that is the weakest term, `Params::proven_security_bits` reports 128.522 bits
after the one-bit per-round margin. `SecurityShape::of` supplies the circuit's actual degree,
identity count, lookups per row, table width and DEEP batch width to the other terms.

The shared suite's 48-byte output gives `kappa = 384` in the Corollary 3 condition
`kappa >= 2 lambda + 3`, hence a hash ceiling of 190.5 bits. `HASH_SECURITY_BITS` names this
hash-output width, not full-proof security. `Params::hash_limited_security_bits` takes the minimum
of that ceiling and the circuit-specific budget. Native BLAKE2b-384 has a 512-bit chaining state
and a 192-bit collision-security target, as specified in [RFC 7693 sections 2 and 4](https://www.rfc-editor.org/rfc/rfc7693.html).
In contrast, [BLAKE3's security notes](https://github.com/BLAKE3-team/BLAKE3/blob/master/c/README.md#security-notes)
explicitly give no additional security for output beyond 256 bits, so extending the former suite's
output alone would not justify this change.

`Params::check_security(circuit, target)` first enforces compatibility, rejects nonfinite or
nonpositive targets, and rejects a nonfinite or insufficient hash-limited budget. Production
RaceV1 callers use the unchanged 128-bit target. Regression tests cover all three tracks with
one through eight cars and require an 80-query profile to fail this security check even though it
remains feasible. This is a classical random-oracle cost/success bound, conditional on the stated
round-by-round protocol analysis; it is not a 128-bit quantum-security claim. Twenty grinding bits
are applied and not counted. Passing the budget check does not replace circuit review, the theorem
mapping or release qualification; the qualification gate remains separate.

## 8. RaceV1 adapter obligations

`race_v1::validate_replay` checks what the proof does not: 1..=8 players, `T <= 5400`, one control
mask per car per tick with no reserved bits, removal events with ascending ticks `<= T`, nonempty
ascending in-range slots, no slot removed twice, and the public half of the six-tick terminal rule
(two or more cars must stay unremoved at every stepped six-tick boundary). The circuit enforces the
other half ("some car is still racing"). `verify_replay` rebuilds the canonical grid, the public
columns and the replay binding itself, verifies, and returns the final state with removal ticks
(including removals at tick `T`). Settlement must additionally check terminality, derive the outcome
from that state, and bind the network/session/manifest/roster envelope (for example through
`Statement::binding`).

The node adapter (`crates/iroha_core_privacy/src/execution_proofs/proof.rs`) meets these obligations
as follows. Before any proof work it runs the unchanged payload, admission, roster, transcript,
history and settlement checks, then `validate_replay` on the converted replay. The statement's
initial registers come from the node's canonical grid, its final and checkpoint registers from the
payload states (progress, lateral position, speed, lateral velocity, energy, finished flag, finish
tick or 0), and the checkpoint step is the checkpoint tick. Removal ticks are not registers, so the
adapter requires the final state's removal ticks to equal the public schedule, including removals at
`T`, and a checkpoint's to equal the removals strictly before its tick, matching the certified
transcript prefix.

`Statement::binding` contains, in order, the engine-binding domain, the ledger profile ID, the full
48-byte profile commitment, the envelope's public statement, and the payload's manifest, admission,
outcome, relation inputs, replay, final state and checkpoint state. Each field has a little-endian
`u64` byte-length prefix; structured fields use their canonical encoding. These bytes are absorbed
directly into the engine transcript, without reducing the binding to a ledger `Hash` first. The
separate BLAKE2b-384 profile commitment frames the profile descriptor, rules, source count, and every
ordered `(repository-relative path, source bytes)` pair in `RACE_PROFILE_SOURCES_V1`. Thus the
255-bit ledger profile ID remains the registry key while the proof also binds the full-width source
commitment and canonical public inputs.

Both proving and verification obtain their parameters through `engine_params`, which calls
`Params::standard().check_security` for the actual circuit and the immutable
`RACE_TARGET_SECURITY_BITS_V1 = 128` target before invoking the engine. Failure rejects the
operation. This budget check does not grant release qualification or replace the protocol analysis.

The prover's witness is the node's own `race.rs` simulation; `tests/race_oracle.rs` compares its
states with the circuit's reference execution. Proof bytes must decode
canonically and stay within `RACE_MAX_STARK_BYTES_V1` (3,250,000). The computed size bound must
cover every registered track and roster circuit using the current 48-byte digest suite; retained
32-byte-digest size measurements do not establish that bound.

## 9. Reviews

- Codex (gpt-6-astra, ultra) design review: bottleneck analysis, deferred contact slowing,
  per-table-value LogUp inverses, symmetric-sum helpers, wider helpers, multi-row scheduling.
- Codex engine review 1 found no forgery in its scope; fixed a padding completeness bug (a car just
  short of the line at timeout "finished" in padding rows), corrected the `assume_interval` contract, added the DEEP
  batch width to the estimator, bounded decoder allocations; added adapter checks for the public
  terminal rule and replay structure.
- Codex engine review 2 examined the external table, split quotient, symmetric sums and linear
  comparisons; no forgery was identified in that scope. It widened multiplicities to `u64` and
  aligned parameter validation with the decoder.
- Codex release review (rename to `iroha_execution_stark`): circuit-aware parameter validation at
  both entry points, returning errors before any shift or allocation; canonical LogUp multiplicities
  (each at most the occurrence total, exact sum, total below `p`); a checked size bound; lookup-free
  proofs carry no auxiliary openings; strict benchmark arguments; separated measurement scopes. It
  found no further forgery in its scope, which is not a full cryptographic audit.
- Open: a written theorem mapping for the committed DEEP layer with one consistency point per query
  (Codex's second review judged it acceptable under the batched-FRI construction), an independent
  cryptographic audit, and formal verification of the gadgets.

## 10. Literature review (November 2026)

**Proving engines.** For 10^7 to 10^8 trace cells on one laptop, a monolithic univariate AIR-STARK
is the strongest-evidence design. Plonky3's own small-field measurements show about 1.5 × 10^8
cells/s on KoalaBear and about 5.7 × 10^7 cells/s on Goldilocks for a Keccak AIR on a laptop
(Small Fields in Plonky3, 2025). S-two (Circle STARKs over M31, LogUp-GKR) reports over 600k
Poseidon2 hashes/s on an M3 Pro (StarkWare, 2024); client-side S-two proofs ran on 1,420 phone
models in FibRace (arXiv 2510.14693). In a Plonky2-style prover (Goldilocks, Poseidon, blowup 8),
Merkle hashing is 57–69% of time and NTT 16–22% (UniZK, ASPLOS 2025), which motivated the original
BLAKE3 prototype. The current suite uses BLAKE2b-384 to meet the hash-security margin above.
Binary-field systems (Binius64, BinarySpartan, ePrint 2026/1656) win on bitwise
hashing; GKR systems (Expander) suit wide, shallow circuits rather than thousands of sequential
ticks. Folding (Nova, LatticeFold+ ePrint 2025/247, Neo ePrint 2026/242) and zkVM continuations
add per-step overhead without benefit at this size; Nexus replaced folding with an S-two AIR.
Goldilocks keeps products of game values below `2^50` in one element; 31-bit fields would need
limbs for most physics products.

**Soundness.** The "up to capacity" proximity-gap conjecture used by most deployed parameter sets
was refuted in 2025 (Diamond–Gruen, ePrint 2025/2010; Crites–Stewart, ePrint 2025/2046; see also
Krachun–Kazanin–Haböck, ePrint 2026/782). Proven Johnson-regime bounds (BCIKS20; ethSTARK v1.2;
improved by BCHKS25, ePrint 2025/2055) cost about twice the conjectured queries; the Ethereum
Foundation now requires provable security (100 bits by May 2026, 128 by end-2026). Over a quartic
extension only the query term matters. Fiat–Shamir must bind the whole statement and every prover
message before the challenge that depends on it (Dao et al., ePrint 2023/691); recent forgeries came
from an unabsorbed LogUp claimed sum (stwo-cairo, 2026) and challenger malleability (Plonky3,
CVE-2026-46654). "How to Prove False Statements" (ePrint 2025/118) shows that random-oracle
arguments do not cover circuits that evaluate their own transcript hash; this engine never hashes
inside the circuit.

**Lookups.** LogUp (Haböck, ePrint 2022/1530) costs about half an extension column per lookup with
degree-3 identities; batching four lookups per helper and evaluating with symmetric sums cuts the
auxiliary width by half. LogUp-GKR (ePrint 2023/1284) removes helper columns entirely and is the
next step if auxiliary width dominates again; Lasso and Shout assume multilinear commitments.

**Game proving.** No public system proves full real-time game replays in seconds: the RISC Zero
DOOM demo proved 3,186 ticks in 1m44s; zkNoid (Mina) recurses over 10-tick chunks; Dark Forest
proves individual moves; Dojo/Saya proves chains rather than matches; optimistic designs (Cartesi
RIVES/Dave, arXiv 2411.05463) settle disputes over challenge periods. A per-tick custom AIR is the
only route to second-scale proofs. Circuit bugs are overwhelmingly under-constrained gadgets (95 of
99 circuit bugs in the USENIX Security 2024 SoK), including a division that never checked the
remainder bound (Veridise on RISC Zero), which is why every gadget here range-checks both quotient
and remainder and why the prover cross-checks the circuit against native simulation on every row.
