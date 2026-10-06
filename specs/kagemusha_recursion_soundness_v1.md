# KAGEMUSHA recursive verifier and accumulation argument

This is the implementation-coupled M4 engineering argument for
[`iroha_plonk_recursion`](../crates/iroha_plonk_recursion/src/lib.rs). It covers
the explicit PIPA-R profile, native PIPA-AS, the incoming-claim branch rule and
the obligations that the operation relations must bind. It is not an external
audit, a security reduction for arbitrary lineage depth, or acceptance of an
unimplemented operation circuit. The current evidence remains in the
[gate checklist](kagemusha_evidence_gate.md#8-recorded-results).

## Assumptions and theorem boundary

The construction assumes discrete-log hardness in the two pinned prime-order
Pasta groups; collision resistance of the pinned Poseidon hashes; and the
random-oracle model for the domain-separated RP57 transcripts. A concrete
Poseidon implementation is a heuristic instantiation of that model.

[BCMS20](https://eprint.iacr.org/2020/499.pdf) gives accumulation definitions,
a PCD construction and a discrete-log polynomial-commitment accumulation
construction (§§4–7 and Appendix A). [BCLMS21](https://eprint.iacr.org/2020/1618)
studies split accumulation and PCD under discrete-log assumptions in the ROM.
[BGH19](https://eprint.iacr.org/2019/1021) is the underlying Halo construction.
These results motivate the architecture; citing them does not prove that the
particular layouts, encodings, mixed-curve wiring or containment rule here
satisfy all their hypotheses. The arguments below are local derivations for
this implementation. The canonical construction's caveat about unbounded
lineage depth remains in force.

## Injective framing and challenge distributions

PIPA-R has an explicit V2 descriptor and decoder. Its key commits to the
descriptor, the pinned parameter digest, the transcript profile, Direct
instances and the FoldedGenerator suffix. No decoder retries a retired
profile. The transcript starts with the `pipa-rb1` domain and absorbs the key
representation, `pipainst`, column count, every column length, every type code
and the column-major instance values. This makes the statement's shape and
type part of the challenge input. Identity points and noncanonical integers
are rejected, rather than reduced or given an alternate encoding.

Proof scalars use a single canonical base-field element when the scalar
modulus is smaller, and the S6 `(lo128, hi127)` encoding otherwise. Range and
modulus checks make S6 injective. Points use finite canonical `(x,y)` pairs;
the compressed byte decoder binds both x and the sign bit to those pairs.
The circuit also checks the curve equation. An outer byte-link relation must
bind the actual supplied length and every byte; a valid dummy used by a soft
decoder is never a replacement for that binding.

Let p be the Pallas base modulus, q the Vesta base modulus and δ = q−p.
The checked constants give
`δ = 86663725065984043395317760 = 0x47afc1f319ba3400000000`.
For a uniform Fp transcript word, identity embedding into Fq has statistical
distance δ/q from uniform Fq. For a uniform Fq word, reduction by one p gives
Fp probabilities 2/q on `[0,δ)` and 1/q elsewhere; its statistical distance
from uniform Fp is `δ(p−δ)/(pq)`, at most δ/q. Thus each mapped challenge
adds at most `2^-167.836` to a uniform-challenge hybrid bound. This is a
distribution calculation conditional on the ROM assumption, not a bound on
the security of Poseidon itself. Zero and other forbidden challenges still
produce rejection bits; total soft arithmetic selects a safe denominator
before inversion and retains the rejection bit.

## What an accumulation fold proves

For an input `(G_j,k_j,u_j)`, normalized challenges have exactly `16−k_j`
leading zeros and k_j nonzero canonical suffix elements. They determine a
length-65536 coefficient vector s_j whose nonzero prefix agrees with the
source generator prefix. The deferred claim is `G_j = Commit(s_j)`.

PIPA-AS absorbs its `pipa-as1` domain, canonical external salt, input count,
and, in fixed slot order, each G, selected source k and all 16 normalized
challenges. It then squeezes α, z and ζ. Define
`h = sum_j α^j s_j`, `C = sum_j α^j G_j`, and
`v = h(z)`. The prover opens the shifted coefficient vector `h−v e_0` at z
with claimed evaluation zero. Its commitment is `C−v g_0`. The non-hiding
IPA has 16 L/R pairs, one final scalar and the unabsorbed folded-generator
suffix. Its body is 1,088 bytes; salt makes the local witness 1,120 bytes.
There is no independent hiding-generator term that could absorb a false
commitment claim. Final acceptance includes the deferred generator decide.

For fixed input errors `E_j = G_j−Commit(s_j)`, a nonzero polynomial
`sum_j α^j E_j` has at most r−1 roots over the prime-order scalar field.
This is the cancellation term. The evaluation test supplies a separate
degree-at-most-65535 term. Under the commitment/IPA extraction assumptions,
the ROM reduction must also account for adaptive hash queries and the
challenge-map distance; a suggestive `(r+65536)/|F|` term is not a complete
standalone security bound or a literal equivalence at every challenge.
The salt is fixed before α; each grinding attempt is another hash query.

An undecided suffix can satisfy the succinct equation even for a false
claim. The API therefore returns an explicitly undecided accumulator.
Neither successful parsing nor `verify_fold` alone authorizes a payment.
Native acceptance must decide all three claims with pinned parameters: the
Pallas batch contains Omega's own opening and `acc_P`, and the Vesta batch
contains `acc_V`. Checking only the two exposed accumulators is insufficient.

Short-only non-hiding folds have a structural liveness problem: the upper
coefficient half is zero and the first-round L commitment is identity for
every salt. The caller therefore supplies an explicit pinned k16 trivial
slot when all actual sources are short. Its position is fixed, counted and
absorbed. It never replaces another obligation or appears implicitly.

## Branch uniqueness and containment

`ModeCells` constrains exactly one of Accept, Trivial and Corrected in each
incoming slot. `constrain_incoming_modes` constrains the sum of Corrected
bits to a boolean, computes `valid = AND(soft_bits) AND no_correction`, and
equates every Accept bit to valid. These equations imply the exact §2.7
branch rule: acceptance uses every original incoming claim; rejection uses
only trivial replacements except for at most one correction; if all soft
checks pass, rejection requires that correction.

Selection must preserve the original challenges and source k in Accept and
Corrected modes. Corrected also requires finite `G* != G`. Trivial selects
the pinned `ACC_TRIV` with source k16 and sixteen ones, including when the
original sigma source was k12 or k14. The selected k is constrained and
absorbed; relabeling a short padded input as k16 is invalid. A valid dummy
from soft decoding is not an authorized Trivial replacement.

Conditioned on accumulation soundness, a selected correction that eventually
decides demonstrates that the original claim failed. If all succinct and
consumer checks and all original deferred claims are true, neither a false
soft bit nor a distinct deciding correction exists. The receiver therefore
cannot choose burn for valid evidence. Conversely, an invalid deferred claim
has a deterministically computable algebraic correction. A finite correction
can be transported and folded without poisoning otherwise valid wallet
lineage. If that correction is the identity, the finite-point encoding
rejects it; the API reports an encoding failure, never an accepted burn.
Thus this is not unconditional liveness for every possible challenge vector.
The exceptional identity event needs the same ROM/computational scope as the
IPA's other exceptional messages; complete operation qualification must not
claim that an encoding failure completes a payment. Hard own/predecessor/Q/A obligations
never enter this selection API. Receive burn and ArchiveSent no-op effects
still require the complete operation, map and integer constraints; the
mode rule alone does not establish monetary conservation.

## Composition, keys and disclosure

The fixed ledger identifies each opening and each transported accumulator
separately, even if their bytes happen to be equal. Current construction
metadata covers all 14 unsplit variants. The single-sigma forwarding path
preserves its original source k; the two-sigma path includes its explicit
full-length trivial slot. Every consumer must constrain all forwarding
equalities and the mode instances across Q, A and Omega. TODO: bind the
conditional A-split context schedule when a measured variant needs it.

The `kgwvkey1` digest binds curve, k, descriptor digest, representation and
every fixed/permutation commitment in canonical order. One-hot allowlists
select sigma and A keys. The final native verifier pins `vkOmega_digest`;
each A proves that its witnessed predecessor key hashes to that carried
field and copies it forward. Bootstrap has no predecessor, and its free
digest is ultimately pinned by that same final native check. This induction
requires the actual key-hash and equality constraints in every operation,
not just a native digest helper.

Omega is hiding. Q/A and local fold proofs remain private. An accumulator
reveals G and its transcript challenge vector; for a deciding accumulator,
G is a deterministic function of that vector and the pinned generators.
This observation alone is not a zero-knowledge proof: a simulator must also
match the joint distribution and correlations of both exposed accumulators,
public step proofs, D_A and all adversarial oracle queries. TODO: complete
and independently review that simulator argument for the final composed
relation and artifact set. No additional privacy guarantee is inferred from
the fact that the local fold protocol is non-hiding.

## Executable checks and remaining review

The native PIPA-R tests pin both-curve transcript KATs, type boundaries,
profile rejection and proof-byte invariance across worker counts. Native
fold tests check genuine proof claims, nondeciding claims, omitted/reordered
inputs, source padding, trivial constants and mutation cases. Codec tests
exercise malformed points/scalars, modulus aliases, challenge-map endpoints
and every assigned cell. Obligation tests enumerate all four-slot mode
combinations on both fields and reject dropped, duplicated, gated or
reshaped fixed obligations.

The full operation relations must add the foreign-key, dropped-accumulator,
bad history-path, invalid-burn and middle-proof mutations, and the depth-16
cycle test. The carry argument is separately recorded in
[the reviewed M3b memo](kagemusha_ff_carry_v1.md). The final soundness review
must bind the actual descriptor/VK hashes and generated proof behavior.

## Receive raw ownership and sigma projection

The Receive context commits the complete Q public inputs, all eleven original
object triples, all five predicate claims, all incoming modes and the original
Omega verifier opening. `ReceiveStagePlan` requires every named operation task
exactly once. Each continuation verifies its exact preceding W key and the same
context; a component result is not an admission certificate for a stage key.

The separately scheduled hard ProofDigest owner derives the combined consuming
hash from both original active tapes and their LE32 lengths. It binds that result
to slot 4 and both original raw lengths/tape commitments to slots 4 and 5. This
owner has no soft result bit and is mandatory exactly once (task code 23). Objects
uses only the same context's claimed combined digest; it cannot admit a different
preimage because the hard producer and every W continuation authenticate it.

The Objects owner assigns the original active incoming sigma tape, including
zero-length, short and over-descriptor inputs within its fixed capacity. Its
safe Q view is `LE32(original length)` followed by the descriptor-sized original
prefix, padded with zero only when the original is short. The remaining active
tail remains in the exact step and combined consuming digests. The Objects
owner hard-binds the incoming statement digest, selected key and every Q-view
chunk to Q0. It also hard-binds the original tape's length, content address and
raw commitment to context slot 5. These equalities are unconditional: a false
Objects predicate does not release the sigma source or permit different Q
inputs.

The Proofs owner therefore consumes the same incoming sigma Q projection without
allocating a second maximum-capacity raw sigma tape. `from_active_omega` requires
original active provenance for Omega and an incoming sigma projection;
`bind_context` equates the projection's statement, selector and every chunk to
the identical committed Q0. Q0 is hard-verified at its unique scheduled owner.
The sigma proof validity bit and mode still come from `bind_sigma`; neither is
provided by the caller. This removes duplicate raw hashing without changing
which original sigma bytes enter the soft verdict or consuming digest. A fixed
schedule missing Objects, Q0 verification, a W/context equality or terminal
mode accounting does not satisfy this argument and must not be admitted.

Omega keeps its original active tape in the Proofs owner. That owner authenticates
its exact raw length and byte commitment, constrains its total public/point
projections, checks the carried Omega key, derives the recursive verifier verdict
and binds every exported opening cell. ProofDigest independently derives the
combined Omega-plus-sigma digest from both original active tapes. A safe decoder dummy is
not permission to omit an original deferred obligation; terminal mode selection
still follows the branch uniqueness argument above.

The transport verifier takes the trusted successor Omega-key digest explicitly.
Its verifier-key witness is hard-bound to that digest, while a foreign incoming
identity or proof contributes a false soft verdict. The digest is absent from
`public320`; the production active decoder already supplies the trusted successor
digest as lineage field 17. Removing the redundant owner equality therefore
hardens the shared API boundary without identifying a current-wire admission bug.
Substituting the verifier key itself remains a hard failure.
An actual k16 PIPA-R component regression generates proofs under two distinct
keys with the same descriptor. The foreign proof satisfies the transport
constraints only with a false verdict, both when field 17 uses the trusted key
and when it proposes the foreign identity. Substituting the verifier-key witness
fails for either verdict. The source relation binds public columns only; this
tests the cryptographic boundary, not admission of a complete Omega program.

Component tests construct a sigma projection with neither an active carrier nor
a step digest and require the complete fixed owners to reject all proposed raw
object triples, Q digest/index/chunk substitutions and original-length changes.
They additionally reject raw sigma and Q substitutions when Objects is false.
The recursive fixture separately distinguishes its authenticated receiver head
from the incoming payer head; an accepted-credit fixture uses actual payer Load
and receiver Bootstrap proofs rebuilt under one common compact Omega key.
TODO: complete that maximum-capacity owner chain, its corrected-claim branches
and final catalog admission before treating these component checks as a Receive
qualification result.

The pre-split canonical-capacity Receive run produced real A1–A3 proofs, including
all retained-object/result/context mutation checks. Objects A4 then failed the
unchanged k16 capacity gate: its diagnostic layout used 73,112 sponge rows,
62,148 main rows and 44,118 range rows. The diagnostic k18 layout was rejected,
not accepted as qualification. The new fixed ProofDigest stage separates the
combined hash from semantic decoding; every stage still checks known/unknown
layout equality and must produce and decide its actual k16 proof. The first nine-stage attempt produced genuine A1–A5 proofs with maxima of
29,600, 60,976, 55,056, 64,195 and 64,787 rows respectively, including the
maximum-capacity digest and Objects owners. A6 then failed at 67,044 sponge rows
when hard current authorization and own-sigma hashing were combined. The current
schedule moves the mandatory OwnProof task into A1, leaving Q0 verification at
A2 and current authorization/Q1 at A6. The same context binds OwnProof's exact
sigma bytes, Q0 exports and own Receipt before later Q verification; no task or
hard predicate is removed. An unknown-witness preflight now checks every fixed
owner's capacity before producing proofs; it does not replace the actual
known/unknown equality and real-proof checks. Moving OwnProof reduced A6 to
64,972 rows, with A1 at 35,446 rows; that nine-stage preflight then rejected
Signatures plus Q2 at 69,523 rows. No proof was produced by that preflight.

The measured ten-stage schedule hard-verifies and folds Q2 alone at A7, then
executes Signatures at A8. A9 owns Nonmembership/Blacklist. Signatures extracts a typed projection of the exact Q2 public cells in
`D_ctx`; it cannot construct a verified-Q result or a pending opening. The
complete ContextPlan pins the Q key digest and requires each Q exactly once;
the Receive plan rejects a missing or future Q2 owner. The projection checks the
fixed signature schema, retains all original 128-bit key/signature limbs and
boolean verdicts, and binds every scalar half to the same context. The Q2 proof
and its opening are still hard-verified and accumulated by A7, so these are
constrained exports of that exact proof, not caller-selected signature verdicts.
Omitting the Q2 owner, its W continuation, context equality or terminal obligation
closure invalidates this composition argument. The ten-stage preflight fit
A1–A9 with maxima 35,483, 61,013, 55,093, 64,232, 64,824, 65,009, 62,530,
57,794 and 62,715 rows. It then rejected terminal A10 at 84,545 sponge rows;
that attempt produced no Receive A proofs.

The twelve-stage candidate separated the two depth32 map effects into
mandatory hard owners: consumed-credit at A10 (task36) and permanent credit record
at A11 (task37), followed by terminal Effects at A12. Each map owner binds the
same complete statement/state/lineage context, all five result claims, all four
modes and exact Payment digest. Each independently derives the identical complete
mode verdict; no new result bit is proposed. The consumed owner authenticates the
exact insertion/no-op and successor consumed root, including empty insertion-slot
checks. The credit owner authenticates and preserves the first Payment/burn record
and successor credit root. Terminal Effects applies the exact adjusted-burn rule,
preserves unrelated roots, and closes the ordinary terminal obligations. Catalog
admission requires both hard owners exactly once before terminal; a terminal-only
map projection is not sufficient. The composed `MapEffectsChip::receive` calls all
three effect parts for standalone consumers. The twelve-stage preflight fit
A1–A9, then rejected consumed-effect A10 at 69,375 sponge rows. The current fixed
ten-stage candidate moves both hard map owners into A1 alongside OwnProof and
the authenticated receiver predecessor. This avoids the later accumulated trace
cost; it does not derive or accept a result early. A1 binds the same complete
context, including all five proposed results, while each mandatory later owner
still derives its result before terminal admission. The terminal A10 performs
burn/preservation and complete obligation closure. The ordinary burn candidate
now passes all ten actual A proofs and nine W proofs at these fixed capacities.
Independent code review found no omitted map predicate or premature acceptance
in this ordering; that review assumes the complete source-key chain executes
every declared owner and does not substitute for actual proof/restoration tests.

The ordinary full-capacity preflight now fits all ten stages, with maxima
65,157; 61,050; 55,130; 64,269; 64,861; 65,046; 62,567; 57,831; 62,752; and
54,945 rows. The first native A relation matches the independent test circuit's
public context, fixed columns, permutation and advice layout and passes strict
constraints. The ordinary burn chain also passes all ten native A layouts and
strict constraints against the independent proof keys, restores all ten A and
nine W checkpoints, rejects wrong stages, truncation, foreign sessions and
substituted original inputs, and preserves exact terminal exports. Its complete
test takes 1,819.06 seconds on this busy host; this is a component test duration,
not a qualified latency measurement or final Omega admission. The renewed
variant also passes its complete ten-A/nine-W proof chain, every native stage's
layout/strict constraints and checkpoint mutations in 1,990.77 seconds. Its A1
maximum is 65,527 rows, only two below the strict 65,529 gate; that actual stage
passes unchanged. Its larger Q2 owner uses 63,677 rows and terminal uses 55,315
rows. No spare-row or runtime margin is inferred from these constraint counts.

The split map regression passes acceptance,
burn insertion and burn no-op with known/unknown equality and mutations of the
verdict, roots, authenticated paths, Payment and credit identity. The twelve-stage
Receive component suite passes eleven checks, including the fixed task/Q-owner
schema and original-byte context mutations. Acceptance and corrected-claim
whole-chain closure still require their own actual proof results.

The first distinct-wallet accepted-chain attempt reached actual A1–A4 proofs,
then correctly rejected the fixture at Objects A5: its Payment encoded the
receiver's key in the payer-key field. The fixture now copies that key from the
retained payer credential and has a direct regression check. That failure is not
an accepted credit result; the corrected chain must run again.

The native producer pins this full-envelope schedule, exact current internal
Tagged4 and terminal Tagged3 descriptors, installed A/W keys, and both k16
parameter sets. Incoming decoder fields, results and corrections remain witness
proposals whose fixed circuit owners authenticate the original sources. Native
preparation checks hard predecessor/Q proofs and decides selected incoming claims;
checkpoint restoration checks exact proof frames and complete accumulators. The
producer never generates runtime keys or chooses a witness-dependent profile.
The native key-ownership revision retains only installed descriptor/VK metadata.
Each proving call borrows its single exact stage PK, rejects a descriptor or VK
mismatch before proving, and releases the borrow on return. Restoration and
next-circuit preparation use no PK. Component tooling likewise releases each
generated PK and retains verifier metadata; later native parity reconstructs
one stage key at a time. This removes implicit simultaneous residency of nineteen
PKs without changing circuits or proof formats. The full borrowed-key regression
and deterministic proof-byte parity remain pending; memory gate qualification
still requires actual peak measurements under the agreed harness.
Its sigma classes are pinned to the admitted k12 or k14 source descriptors. The
installed descriptor-sized Omega transport (excluding public320) plus incoming
sigma must fit the actual 8,277-byte proof budget; the full raw capacities remain
unchanged for total malformed-input handling.
Its pure component checks cover every result/mode combination, distinct deciding
corrections on both curves, exact original lengths/tails and all signed limbs.
The shared circuit joint-length predicate additionally passes every one of 8,278
boundary splits and both one-byte-over variants under identical fixed/permutation
layouts; native admission tests reject the corresponding over-bound inputs and
individual-capacity/overflow cases. This arithmetic component does not replace
the original active-tape provenance and capacity checks.
TODO: complete the borrowed-key regression, accepted native stage parity and
checkpoint mutation tests, corrected-claim owner chains and final Omega admission.

The completed distinct payer-Load/receiver-Bootstrap shared-key source fixture is
component evidence under the superseded Load voucher trust construction. The
human-selected ordinary-transaction/finality Load replacement must rebuild the
source/catalog before release qualification or artifact freezing. No result here
establishes the final catalog, phone performance or the durable-completion gate.

The adversarial test-only compact source helper uses a deliberately forged
succinct PIPA-AS transcript, following the recursion mutation test. It requires
the resulting original Vesta claim to fail decide and derives a distinct deciding
commitment under exactly the same challenges. The standalone forged-fold component
passes. The complete common-key helper also passes: a genuine 3,712-byte Omega
under the immutable shared key verifies while its original Vesta claim fails
decide, and the distinct correction with identical challenges decides. Both
signed source chains and the distinct receiver are rebuilt under that same key;
the adversarial Omega keeps its source A unchanged. This test takes 1,984.25
seconds on the busy host and is superseded-Load-trust component evidence, never
a valid monetary head. Corrected Receive burn/no-op proof chains remain open. Production proving
and complete-decide checks are unchanged, and this is never a valid monetary head.
