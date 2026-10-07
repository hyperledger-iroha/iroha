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
equalities and the mode instances across Q, A and Omega. Split operation
sources bind their fixed task/Q partition and complete context through each
preceding W key and continuation digest. Complete source execution and terminal
catalog admission must still be established separately for every operation;
the split metadata constructor alone is insufficient.

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

Own-state policy fixes the independently selected provider and root key, while
scheme identity comes from constrained state cells. Bootstrap binds those same
scheme cells across state, statement, public lineage, signed certificate and
credential. Other own-object owners use the authenticated predecessor's state
scope. Native admission must still bind the carried scheme/relation to the
installed inventory. Compiling the derived SchemeID into these source policies
would instead make A keys depend cyclically on their finished Omega catalog.

The corrected Bootstrap regression constructs two complete genuine
sigma/Q/A1/W/A2 chains with different schemes and requires identical six source
VK byte strings and known/unknown layouts. A third chain retains actual proofs
and valid original signatures but rejects its foreign-scheme certificate at A2.
It passes 1/1 in 289.24 s (`target/qualification/bootstrap-carried-scheme`).
This establishes the tested source independence and authorization rejection,
not a complete acyclic artifact inventory or full operation soundness review.

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
verdict, roots, authenticated paths, Payment and credit identity. The Receive
component suite passes eleven checks, including the fixed task/Q-owner
schema and original-byte context mutations. Corrected-claim no-insertion
closure now has its separate captured proof result, scoped below.

The first distinct-wallet accepted-chain attempt reached actual A1–A4 proofs,
then correctly rejected the fixture at Objects A5: its Payment encoded the
receiver's key in the payer-key field. The fixture now copies that key from the
retained payer credential and has a direct regression check. The repaired
distinct-wallet chain passes all ten A and nine W proofs, native descriptor/VK
and strict-constraint parity, checkpoint restores and terminal exports in
5,078.85 seconds on the busy host. This captured run uses the earlier resident-PK
test harness and superseded Load trust; it establishes neither final Omega
admission nor the newer canonical serialized-custody checks.

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
PKs without changing circuits or proof formats. Receive now uses the shared
`native::artifact::KeyArtifact` directly, with no local type or compatibility
alias. The focused native/shared suite passes all fifteen checks, including
same-descriptor foreign-key rejection, wrong descriptor/pair rejection and
both-curve metadata retained after the PK is dropped. Pair construction checks
identity only; authenticated installation and exact original-source validation
remain separate requirements. Receive's strict original-PK intake now reconstructs
one fixed unknown A or W source from the installed plan, including the complete
owner schedule, source capacities, preceding wrapper and internal Tagged4 versus
terminal Tagged3 profile. It bounds the original, validates its compiled source
tables, and checks the exact installed descriptor/VK before returning one PK.
The factory never returns a checked witness or checkpoint. Its source-shape and
intake-bound components pass for both Receive variants. The renewed variant
also passes all nineteen sequential original-key imports, exact verifier identity,
repeated unknown-layout equality and truncated/wrong-stage rejection in 719.11
seconds. That sweep uses constructor-only Q metadata and does not qualify genuine
proof workloads. The ordinary constructor-only sweep also passes in489.91 seconds.
Both sweeps precede the constant-size continuation rekey described below; their
original source keys do not qualify the new circuit.
A separate current-source regression constructs only a genuinely authenticated
zero-balance Bootstrap receiver. It alters the original incoming Send sigma before
Q preparation, signs the exact altered proof digest and Payment, and requires
Q's incoming verdict to be false with every incoming mode Trivial. The branch
preserves the consumed root and records the burned credit while keeping adjusted
spendable value zero. A locally generated Send witness supplies adversarial bytes
only; no funded Load lineage is admitted. This regression includes every fixed
owner, original-key import and canonical checkpoint. Its current constant-size
context source passes complete10A/9W execution in2,045.12 seconds, including all
nineteen strict original imports, borrowed-key exact proof parity and exact-byte
canonical replays after creating a fresh native session. All ten source layouts
fit k16, with maximum65,305 rows under65,529. Original/Q/context substitutions,
changed current P/W obligations, foreign sessions and malformed custody reject.
The captured executable, source and final log are retained in
`target/qualification/kagemusha-receive/o1-current/`. The prior2,580.44-second
history-hash capture remains historical evidence. A separate captured run
closes the Bootstrap-only burn through actual final Omega in 1,949.62 seconds.
It rebuilds the signed Bootstrap under the fresh immutable Bootstrap/Receive
catalog, reproduces the exact planned Receive terminal VK, executes all ten A and
nine W proofs with all nineteen original imports and exact native replay, and
proves the canonical four-input outer fold. Every selected input decides;
dropping the fourth is rejected. The genuine outer proof is 3,712 bytes and its
transport is 4,800 bytes, with burned-credit membership retained and adjusted
spendable value zero. The complete captured source, executable and final log are
in `target/qualification/kagemusha-receive/bootstrap-burn-omega-o1/`.
These captures predate the acyclic policy correction: signed scheme scope now
uses carried state/lineage cells rather than compiled SchemeID constants. The
changed source and all dependent A/W/catalog keys require fresh qualification.
This captured malformed-source burn result cannot qualify accepted payment,
corrected-claim closure, funded offline onward spending or the full catalog;
neither captured runtime is a performance gate.
The canonical Norito
checkpoint codec derives its exact stage/key/proof length from those installed
artifacts, bounds the original before decoding, and restores through the same
native proof and full-claim verifier. Its layout/codec component tests reject
changed role, stage, descriptor, VK, proof length and framing; the fresh integration
test's ordinary full-envelope burn now passes all nineteen exact-byte
encode/restore/re-encode checks in a fresh session, together with the complete
ten-A/nine-W proofs, native VK/strict-constraint parity and changed-original,
foreign-session, role/stage and framing rejection. The captured run completed
in 4,560.08 seconds on the busy host; it predates the shared helper extraction
and does not qualify the other branches or final Omega. The earlier captured full
borrowed-key regression passes all ten A and nine W proofs and native restores,
exact native VK/strict-constraint parity, pre-proving wrong-stage key rejection
and identical deterministic A2 proof bytes in 3,403.65 seconds on the busy host.
That duration is component evidence, not a latency gate; memory qualification
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
The separate renewed accepted-chain capture passes all ten A and nine W proofs,
exact native key/strict-constraint parity, borrowed-stage proof equality and all
nineteen canonical checkpoints with exact-byte replay in a fresh session. It also
rejects changed originals, foreign sessions, wrong stages and framing mutations.
The run took 8,181.79 seconds on the busy host and uses the superseded Load source;
it predates shared-helper extraction and strict original-PK intake. It does not
qualify current ordinary-finality Load, performance or final Omega.
TODO: requalify current original-key import and complete final Omega admission.

The separate Receive outer consumer now plans every exact A/W verifier key,
extends the common catalog, rebuilds the signed payer/receiver sources under the
resulting immutable outer key, and requires the actual terminal VK to match the
planned bytes. It calls the native `terminal_fold_inputs` selector and native
outer producer, retaining all four obligations and the exact resulting credit
membership path. The canonical selector is an encoding/selection API only;
source-proof verification, complete decisions and artifact authority remain
separate mandatory boundaries. The captured pinned-native test passes in7,069.43 seconds: common three-terminal
Bootstrap/Load/Receive key continuity, actual accepted Receive outer proof3,712 B /
transport4,800 B, all four selected obligations decided, droppedfourth rejection,
original import/canonical checkpoint replay and exact credit membership. Its final
log and binary provenance are retained in
`target/qualification/kagemusha-receive/pinned-omega-accept/`. This capture predates
the constant-size continuation rekey and uses superseded Load trust. It does not
qualify the current source, corrected-burn outer closure, ordinary-finality Load,
the full catalog or performance. Earlier witnessed-key results likewise do not
qualify another compiled source.

The captured distinct payer-Load/receiver-Bootstrap shared-key source fixture is
component evidence under the superseded Load voucher trust construction. Current
native and recursive Load producers consume ordinary transaction finality, and
the integration helpers require explicit genuine finality evidence and original
proving artifacts. The retired issuer fixture is removed. Installing the current
fixture and rebuilding the source/catalog remain prerequisites for release
qualification or artifact freezing. No result here establishes the final catalog,
phone performance or the durable-completion gate.

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
a valid monetary head. The corrected Receive burn with consumed-credit insertion
also passes the complete ten-A/nine-W chain, all native fixed/permutation and
strict-constraint parity, checkpoint restoration/mutations and exact terminal
exports in 4,883.52 seconds. Both actual sigma proofs remain valid and all five
result groups are true; the original non-deciding Vesta claim is the reason this
cannot take the acceptance branch. The captured run uses the earlier resident-PK
test harness, not the new serialized custody path. The separate corrected
no-insertion run passes all ten A and nine W proofs, native key/strict-constraint
parity, borrowed-stage proof equality, every canonical checkpoint with exact-byte
fresh-session replay and the custody mutations in 8,299.91 seconds on the busy
host. It retains the unchanged consumed root and exports the corrected incoming
Vesta obligation. This captured source uses superseded Load ancestry and predates
the shared helper extraction and strict original-PK importer; it is component
evidence, not latency or memory qualification. Final Receive Omega admission
remains open. Production proving and complete-decide checks are unchanged, and
the adversarial source never qualifies as a valid monetary head.

## Archive original sources and conditional terminal closure

`ArchiveStagePlan` requires exactly one owner for each of OwnProof, retained
Proofs, retained Payment, incoming Proofs, Evidence, Signatures, current
Authorization, CorePending, LineagePending and Effects.
Effects must be terminal, while Proofs must precede it. Q1 and Q2 execute at the
Authorization and Signatures owners respectively. In the fixed ten-stage
source, Q0 executes in its own stage: original-proof binding consumes the same
committed Q public words, and later hard Q verification closes that exact
opening. Every Q index occurs exactly once; index order need not match stage
order. No earlier checkpoint grants a terminal operation verdict. Every stage commits the same
original-object schema: seventeen slots for Receive evidence or nineteen for
Status evidence. The result object commits the three predicate bits together
with their fixed owner indices, plus the original Status verifier opening when
present. The native/context proposal helpers grant no proof authority.

Conditioned on collision resistance and authenticated execution of every fixed
stage, a result proposal cannot change between its producer and Effects: each
continuation binds the same context, and the unique producer equates its derived
bit to that proposal. Omitting a producer or admitting its key without executing
the complete prescribed source invalidates this argument. Full A/W and final
Omega verification must also close each Q opening and each intermediate A/W
opening; equal bytes do not permit merging distinct obligation slots.

Both retained owners are unconditional. The Payment owner binds the held Request, payer
Credential, original Send Receipt, quoted receiver Credential, exact Send
statement and pending descriptor. The separate Proofs owner derives both active
proof commitments and includes their original lengths in the combined consuming
digest. The Payment owner reads those same context slots9/10, hard-equals their
digests, and uses that digest in the signed receipt/package checks. Both owners
are mandatory; a proposed commitment does not replace proof-owner execution. Separate capacity bounds do
not imply the Payment bound: their exact UInt32 lengths are summed as UInt33
and constrained to at most 8,597 bytes, comprising the 8,277-byte combined proof
budget plus the 320-byte public transcript carried in the raw Omega source.
Neither field wraparound nor a false incoming verdict releases this constraint.
Historical proof bytes remain opaque in this owner; it does not verify an old
Payment's proof or receipt signature. The hard predecessor and pending opening
authenticate the seven-word Send descriptor, not the historical proof bytes or
their lengths. The native durable archive must supply the exact retained Payment;
the circuit then binds those supplied originals to the incoming evidence. Pending
membership alone cannot justify limiting a historical sigma to 3,456 bytes.

Receive evidence binds its original sigma to the incoming Q0 slot and uses the
Request-recorded blacklist selector. Status instead binds the original active
Omega carrier, decoded public fields and both transported claims; its verified
key must hash to the current lineage's carried Omega-key identity. Its exact
soft-verifier opening is committed even when verification returns false.
Canonical content addresses bind their supplied preimages unconditionally;
a different canonical component cannot be used merely to force a false verdict.
Malformed originals retain their original digest and derive the corresponding
false predicate. Q2 independently binds the incoming receipt signature to the
receiver key quoted by the held Request.

At Effects, the three predicate groups and all incoming modes use the same
branch-uniqueness equations as Receive. For Status, terminal selection uses the
committed original opening, original P/V claims and at most one distinct
same-challenge correction. Both pending-map removal paths are authenticated. The separate CorePending
owner unconditionally removes the core pending entry and clears its physical
leaf. The separate LineagePending owner authenticates the adjusted path even on no-op; the adjusted
lineage pending root removes the entry only on acceptance and retains it on
no-op. It uses the same complete result/mode equations as terminal Effects,
including the no-Corrected condition. Three true soft predicates alone cannot
authorize adjusted removal. All unrelated monetary, credential and policy fields remain constrained.
Consequently a discretionary no-op for entirely valid evidence would require
breaking a producer binding, a mode equation or a deferred-claim decision.

The shared owner/schema, source and map tests support these local implications.
The joint-budget regression first reproduced acceptance of coherent oversized
originals, then rejected cap+1 in either tape after re-signing/re-hashing, while
accepting the inclusive bound. Complete seven-A/six-W ArchiveReceive runs remain
in progress; their captured binaries predate that constraint. ArchiveStatus full
composition, native installed production and complete common-key Omega admission
remain open. Independent internal review of the source-bound argument found no
binding gap, conditional on executing all fixed owners and deciding every
obligation; it does not qualify those pending implementations. No theorem or
release claim here assumes those checks have passed.

The complete current source schema therefore reserves 8,597 raw Omega bytes and
8,277 sigma bytes for the retained Payment, with the unconditional joint bound
above. Incoming Receive evidence reserves 9,321 sigma bytes (10,000 minus the
measured 679-byte Credited carrier); Status reserves 8,132 raw Omega bytes (the
320-byte public transcript plus 10,000 minus 2,188 fixed carrier bytes). Installed
descriptor equality remains a total proof-verdict check over those original
lengths; it cannot erase tails or replace mismatched bytes. Artifact admission
pins the allowed sigma classes and verifier identities. The Archive total-input
path still accepts every original length within its complete envelope capacity;
it cannot use descriptor length to exclude malformed tails from the no-op
relation. A narrow component circuit does not establish that relation. The captured seven-stage jobs
use the earlier 3,456-byte sigma capacities; the complete domains require fresh
sequential source-key preflight, actual proofs and native parity. Additional
independent capacity review established this retained/soft-input distinction and
corrected the history wording above. The isolated full-capacity retained owner
passes strict constraints and known/unknown layout equality at the inclusive
joint bound. The shared full-domain constructor also passes both-variant owner,
Q-placement and source-schema tests. Neither test verifies a recursive chain.

Independent source review of the full-tail fixture confirms that Q receives its
fixed descriptor-sized prefix and the original length9,321, producing a false
incoming verdict and Trivial selection. A reconstructs that same safe view while
hashing the entire original tail into its sigma and context digests. The incoming
receipt and Credited transcript are recomputed over that full original. This
review does not substitute for execution of the new complete no-op proof case.

The shared native Archive producer preserves these same ten operation owners
and separate Q0 stage. Its input
preparation verifies the hard predecessor and three Q proofs, retains every
original and selected claim, and grants no soft predicate by itself. Every A/W
checkpoint is bound to its original context, fixed role/stage and installed key;
a fresh session reconstructs current claims by verifying each previous checkpoint.
Independent source review found no omitted Pallas or Vesta obligation: terminal
Status folds both selected Pallas slots, while final Omega retains the original
Vesta claim with its committed mode and correction. Five constructor tests and
the checkpoint codec/import-bound tests pass, as does the Status safe-view sweep
across all8,133 admitted lengths. These are intake and source components; genuine
full-domain A/W replay and final Omega remain required before artifact admission.

The original full-domain seven-stage source failed k16 before original-key import:
constructor-only Q metadata overflowed Receive stage6 and Status stage1. The
failure is retained, and no source or proof gate was marked passed. The nine-stage
split isolates retained proof hashing and core pending removal without narrowing
any raw capacity or dropping a constraint. Isolated depth32 Core/Lineage owners
pass strict and known/unknown layout checks at16,983 rows each. Full native source
key/import sweeps and genuine proof workloads must re-establish the complete gate.

The subsequent nine-stage preflight uses actual Archive/Receive sigma and Q
source keys, retaining only a constructor predecessor. Both variants overflowed
k16 at the combined Proofs owner under tagged3. The candidate now fixes tagged4
for internal A sources and tagged3 for terminal A, as Receive already does.
Each W pins its exact preceding A key and descriptor; the common terminal catalog
admits only the tagged3 terminal source. This is a layout change, not a relaxation
of domain size, original-byte bounds, proof obligations or key authentication.
The failed immutable source runs remain recorded; the changed sources require
fresh capacity/import/proof evidence. A separate strict projection regression
copies all six retained proof commitment words and rejects mismatched consuming
digests, missing context slots and substituted slot schemas; it does not confer
raw-byte authentication before the mandatory retained-proofs owner executes.
The tagged4 attempt also exceeded k16 at the combined Proofs owner. Subsequent
eleven-stage attempts overflowed the signatures or later core-removal stage.
The current ten-stage source combines unconditional core removal with the first
own-proof stage and keeps hard Q0 verification, raw incoming proof binding and
adjusted removal separate. Each map owner receives its exact removal path.
Its captured actual-Q preflight fits stages0 through5 but exceeds k16 at the
hard Q0 owner:68,450 Status and73,001 Receive sponge rows. These source checks
still use a constructor predecessor. The layout remains unqualified; neither
a larger diagnostic synthesis domain nor an unexecuted original importer
counts as passing the fixed k16 gate.

## Fixed-stage continuation context

The current shared split implementation replaces historical hash replay with
`C = P(kgwctx_1, immutable context)` and
`D_i = P(kgwlink1, [1, variant, i, C, encode(P_i)])`. C includes every compiled
owner/Q/source schema, exact state/statement/object commitment, original proof
projection, proposed result, mode and correction previously committed. Only the
stage's carried P is separated into D_i. Every A computes C once; an internal
continuation reuses those cells for its next digest.

A_i hard-verifies its exact predecessor W key against recomputed D_(i-1) and
V_(i-1). Hash binding gives equal C and P_(i-1) to the authenticated preceding
A frame. That frame, through its exact source key, enforces the prior operation
owners and hard fold. The current A fold still includes prior P plus the W
opening, and every mandatory current Q/predecessor/incoming/source claim.
W retains the A opening and original V part through the unchanged four-slot
fold. Induction therefore retains every obligation without a history witness.
Only the terminal A exports the final lineage frame; no internal digest is
accepted as `kgwomg_1`. The compiled stage index is bounded and domain-separated.

Independent local review found no omitted context or P/V obligation in this
change. The old trace, its helper and private persisted copies are removed.
This is a deliberate source re-key across all staged producers; captured earlier
proofs establish their recorded sources only. Fresh layout, known/unknown parity,
original-key import, context/current-P/W-V mutations, actual A/W proofs and final
catalog qualification remain required for the new candidate.


### Exact message halves in the immutable context

Explicit Omega message projections now append low128 and
`high127 + 2^127 * top1` to C. The private `LeElement` representation already
constrains low128, high127 and Boolean top1. The combined high half is at most
2^128−1, strictly below Fp; equality of the two native words therefore implies
equality of every one of the256 original bits. No canonical-point or canonical-
scalar assumption is needed, so malformed inputs retain total soft semantics.
The fixed message count and original length remain bound, as do the independently
owned active-tape commitment, decoded claims, result and opening. The native
Archive Status preimage uses the same two little-endian16-byte halves. PIPA-R
message decoding and terminal obligation selection still use their original
constraints. This changes the Archive Status context source key; old three-word
context proofs are not admitted as current evidence.

Independent local review found no ambiguity in this encoding. The strict
regression passes for zero, all-ones, mixed and limb-boundary bytes, every changed
original byte and either changed public half, including bit255, with identical
known/unknown fixed, permutation and advice layouts. Its captured executable and
72.24s result are retained in `target/qualification/archive-two-half-context`;
this is binding evidence, not a performance gate or complete Archive proof.
