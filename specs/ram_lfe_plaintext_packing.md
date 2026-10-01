# RAM-LFE scalar packing over F257

Date: 2026-09-29. Status: public algebra candidate, no encryption profile selected
or enabled. This refines the packing question in the
[replacement plan](ram_lfe_encryption_replacement.md). It changes no production
code, security claim, admission limit or proof relation.

## A fixed scalar subalgebra

For the illustrative ring `R = F257[x]/(x^4096+1)`,
`257^k = 1+256k mod8192`, so the multiplicative order of 257 modulo 8192 is 32.
The finite-field cyclotomic factorization theorem therefore gives 128 distinct
irreducible factors of degree 32. The theorem and factor count are stated in
[Mathlib's cyclotomic factorization source](https://leanprover-community.github.io/mathlib4_docs/Mathlib/RingTheory/Polynomial/Cyclotomic/Factorization.html).
This agrees with the general extension-field slot model documented by
[HElib](https://ibm.github.io/fhe-toolkit-linux/html/helib/classhelib_1_1_ptxt.html);
it is not a claim that SEAL's BFV `BatchEncoder` supports these parameters.

The following explicit encoding is a derivation for this ring. Fix `zeta=3` in
F257, whose order is 256, and `beta_j = zeta^(2j+1)` for `0 <= j < 128`. Then

```text
x^4096 + 1 = product_j (x^32 - beta_j)
R = product_j F257[x]/(x^32-beta_j) = product_j F_(257^32)
```

Each root of `x^32-beta_j` has order 8192, hence minimal polynomial degree 32;
the degree-32 factor is irreducible. The factorization follows by substituting
`y=x^32` in the split polynomial `y^128+1`. Embed a scalar `v_j` as the constant
element of each factor field. All 128 scalars can then be encoded as

```text
encode(v) = g(x^32),       degree(g) < 128
g(beta_j) = v_j
g_k = (1/128) * sum_j v_j * beta_j^(-k) mod257
decode(g)_j = sum_k g_k * beta_j^k mod257
```

Only 128 coefficient positions, the multiples of 32, can be nonzero. This is an
injective ring embedding of `F257[y]/(y^128+1)` into `R`, so componentwise addition,
subtraction and multiplication preserve scalar F257 semantics. There is no need
to increase the plaintext modulus merely to obtain scalar packing.

The scalar subalgebra is exactly the Frobenius-fixed part: `m^257=m`. Every
multiple-of-32 monomial is fixed; each other monomial's Frobenius orbit contains
its negative, forcing its coefficient to zero in odd characteristic. This also
gives a direct encoding/well-formedness condition, independently of a library
slot numbering convention.

## Selection, movement and output disclosure

For admitted scalar slots, `1-c^256` is one exactly when `c=0` and zero otherwise.
The existing eight squarings, final multiply by one and branch multiplication
therefore implement `SelectEqZero` componentwise without changing its F257
semantics. This statement does **not** hold for arbitrary extension-field slots.
For example, the nonzero element `c=x` in `F257[x]/(x^32-beta_0)` has
`c^256=beta_0^8 != 1`; its purported zero indicator equals 122. A decoder accepting
arbitrary ciphertexts cannot assume that their unknown plaintext is scalar.

Admission must establish the encoded plaintext's subalgebra membership, byte and
length ranges, and zero unused slots. A client can possess plaintext/encryption
coins for an input well-formedness proof; the evaluator generally cannot supply
that witness. Select and review this separate input-admission mechanism and its
binding to the execution statement, rather than silently requiring the evaluator
to know the client's plaintext. Similarly, trusted policy must authenticate
appropriate key material; shape checks alone do not establish its security.

The current tape accesses independent scalar inputs/registers, not a uniform SIMD
program. One possible adapter keeps scalar intermediate values broadcast in all
128 slots. Loading input slot `j` first multiplies by the public algebra's fixed
idempotent `e_j=encode(one_hot_j)`, then sums its orbit. On the scalar subalgebra,
automorphisms `x -> x^a` act as the permutation
`j -> ((a*(2j+1)-1)/2) mod128`. The exponents
`5,25,625,5601,4033,3969,8191` support seven doubling additions that broadcast a
single selected scalar. Actual encrypted rotations need seven authenticated
Galois/key-switch keys and incur their full noise, work and proof costs. The
hidden input index and choice of mask must be constrained inside the private
execution relation; this algebra does not provide an uncosted random-access gate.

Outputs can be assigned to fixed slots by idempotent masks and summed into one
packed plaintext. Force every undeclared output slot to zero and prove the masks
and order. Masking plaintext slots does not sanitize ciphertext noise or establish
hidden-function privacy. The opening authority must authenticate only the exact
declared output sequence and context, with no alternate decode of extension-field
coordinates. Function identity and rekey stability still follow the semantic
commitment design.

## Noise, representation and proof costs

Centered coefficients of a generic encoded scalar vector are bounded by 128 and
have at most 128 nonzero positions, giving an `l1` bound of 16,384. A single-slot
idempotent has all 128 centered absolute coefficient magnitudes `1..128`, so its
exact `l1` norm is 8,256. Under the elementary convolution inequality,
`||e_j * noise||_infinity <= 8,256 * ||noise||_infinity` before any additional
modulus/rounding contribution. Plaintext masking is therefore not a unit-cost
scalar multiply with negligible noise. Slot broadcast adds seven key switches
and additions. The planner must derive bounds for the complete selected BFV-RNS
algorithm; these public polynomial norms are only inputs to that derivation.

Ciphertext components and errors still live in the full degree-4096 ring. Do not
compress ciphertext coefficient arrays to the 128 sparse plaintext positions or
substitute 128 for the ring dimension in ciphertext multiplication/noise terms.
The plaintext embedding also does not authorize sampling keys or encryption
randomness from that smaller subalgebra; their reviewed distributions stay
separate from message encoding.
For illustration only, two components with two 64-bit RNS limbs consume 131,072
bytes per ciphertext before framing, versus 8,388,608 bytes for 64 such separate
ciphertexts. Two limbs are not a recommended or qualified modulus. Evaluation
keys, masked/broadcast intermediates, input admission, output packing and the
complete proof may still exceed current budgets.

Pin root choice, slot order, plaintext coefficient representatives, sparse
embedding, ciphertext residue order and exact bounded Norito framing as one
profile contract if this design is adopted. Each residue still requires canonical
range checks; no alternate sparse ciphertext decoder or extra public sidecar is
introduced. Input admission and execution must share the same encoding identity.
The full relation must constrain the transform, slot masks, automorphisms, key
switches, output-zero policy and all ordinary BFV arithmetic. Recompute complete
rows/cells/lookup, proof/envelope and memory costs; the prior degree-64 estimate
does not qualify this representation.

## Evidence and next decision

`dist/zk-remediation/2026-09-29/ram-lfe-encryption-replacement/packing-algebra.json`
records public integer/F257 checks: order/factor facts, all 128 basis roundtrips,
four mixed product/selection vectors, all 257 scalar zero indicators, all 128
fixed broadcasts, an extension-field counterexample and output masking. Its
companion script generates no key or ciphertext and performs no encryption,
decryption or recovery experiment. The test matrix fingerprint is not a wire
profile commitment or an independently reviewed cryptographic artifact.

The unused Rust [plaintext candidate](../crates/iroha_crypto/src/fhe_bfv/plaintext_packing_candidate.rs)
now implements clearing fixed-size scalar/coefficient owners, transforms, masks,
plaintext ring arithmetic and automorphisms under `cfg(test)`. Its exact-source
isolated Cargo harness passes seven native controls and strict Clippy, including
the independent vector, canonical bounds and observed live-cell erasure on
success, error and unwind. Evidence is retained in
`dist/zk-remediation/2026-09-29/ram-lfe-plaintext-candidate/20260929T110759Z/`.
The first attempt's native passes and one formatting-idiom Clippy failure remain
separate. Normal whole-crypto integration is pending; this module defines no
encryption, wire frame or supported profile.

Prefer evaluating this fixed F257 scalar packing before changing plaintext
semantics. It resolves an algebraic packing obstacle, not the replacement's
parameter, noise-depth, circuit-privacy or proof-capacity gates. Next compare the
complete masked/broadcast tape plan against a reviewed BFV-RNS modulus and key
schedule, including maximum `SelectEqZero` programs and every output. Production
remains unavailable until all replacement-plan milestones pass.

## Complete scalar-tape lowering contract candidate

This is a private structural planning contract, not a selected encryption
profile. It preserves the source interpreter's eleven instructions and admits
no alternate evaluator. The concrete plaintext implementation covers N=4096;
RNS dimension, primes, distributions, levels and sanitization are still
unselected. Symbolic polynomial counts below must be instantiated and checked
against a reviewed profile before any encryption or production admission.

### Input, output and private owners

The input is one packed scalar vector: slot 0 is length `0..63`, slots
`1..=length` are bytes `0..255`, and all other slots through 127 are zero. The
first 64 logical slots preserve `encode_identifier_slots` exactly; a value of
256 is invalid in a byte slot. Message normalization and identifier-specific
canonicality remain separate checks. The client owns the message and encryption
coins. Its well-formedness proof must establish those bounds, the exact scalar
embedding and encryption relation under the authenticated policy key, binding
the same input commitment/context as the evaluator's execution proof. The
evaluator must not be required to know the client's plaintext or coins.

Both proof relations must be verified and bound to the identical typed input
commitment; an unchecked client assertion or token is insufficient. A proposed
compound envelope must count both proofs and their public instances in its
limits. This does not prescribe recursive verification or authorize a new
trusted input issuer. The input-admission relation is not implemented.

Four registers start at scalar zero. Reads before the first load are valid:
the source initializes these registers, rather than treating them as undefined.
Thirty-two memory lanes are rederived for each evaluation from the private key,
function identity and associated data; `StoreState` persists only within that
tape invocation. No cross-request state transition is introduced.

Preserve 1..256 tape instructions, four registers, 32 lanes and 1..64 ordered
outputs. Scalars and immediates range over `0..256`. An output is a scalar, not
a byte: never truncate 256. `Output` snapshots the source at that instruction;
later writes cannot alter earlier outputs. Pack the ordered outputs in slots
`0..output_count`, with all remaining slots zero, including slots 64..127.
The opening authority must check this exact output encoding, declared count and
context before hashing the opened scalar sequence. Its authenticated plaintext
opening remains distinct from the execution proof and ciphertext commitment.

The existing outer request cap is 1 MiB; associated data is `0..512` bytes. The
hidden tape owner reserves 12,288 instruction bytes and its explicit frame cap
remains `RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES`. Existing generic secrets remain
`1..4096` bytes for their separate backends. The proposed programmed `ProgramKey`
is exactly one canonical nonzero 32-byte Fp element; arbitrary old secret bytes
are not silently converted into that key. Planning consumes no secret key,
plaintext input or randomness. It borrows the validated clearing tape, keeps
its program-dependent counts/indices private, and clears owned plan scratch.
No raw program, private count report or planning trace is added as a public
sidecar. Detailed errors belong to the policy owner's local builder; public
execution refusal must not reveal hidden opcodes or instruction positions.

### Test-only typed shape and public-statement boundary

The existing plaintext module now has `ClientInput` and `ScalarOutput` owners.
They reuse `ScalarSlots`/`ClearingWords`, move an existing allocation when
validating decoded slots, and reject byte 256, nonzero undeclared slots, input
length 64 and output counts outside 1..64. An output owns its ordered snapshot;
its count and full 128-word allocation clear on drop. These local checks do not
prove an encryption relation or establish oblivious validation. Borrowed and
compiler-created copies remain outside the owners' erasure claim. No private
plaintext wire format or hash is introduced.

`InputAdmissionStatementCandidateV1` is a test-only public Norito grammar with
this exact order: version, policy hash, parameter digest, encryption-public-key
digest, evaluation-key digest, encryption-profile digest, semantic-context hash,
packing-contract hash, associated-data hash, input-ciphertext hash and input
byte length. This is only the already-public ciphertext frame length, never
slot 0 or the private plaintext byte count. A selected fixed-profile ciphertext
codec must enforce its fixed shape; the local input owner does not publish its
length. The canonical frame is 348 bytes. All nine hashes use the existing
public Iroha `Hash` representation, never reduction into Fp. The policy hash is
explicit; the current production receipt, which lacks that field, is unchanged.

The semantic context must bind chain, program, owner and backend under the
selected future policy grammar. The profile and key digests must cover their
complete canonical authenticated contents. A future purpose verifier must derive
the expected statement from authoritative policy/context and the actual submitted
input, then bind both proof relations to that same statement. This code only
hashes bounded opaque input bytes and compares public fields. It neither checks
ciphertext canonicality nor authenticates the caller-supplied identities. There
is no trusted input issuer, admitted-ciphertext type or execution capability.
The future semantic Fp input commitment, if adopted, must additionally be
constrained to this exact ciphertext binding; this public hash is not an alias
for that field commitment.

The packing-contract hash covers its candidate V1 schema, N=4096, t=257,
128 scalar slots, the 63-byte/64-output limits, all seven ordered Galois roles,
and required relinearization. It selects neither a refresh algorithm nor a
bootstrap prohibition; any refresh/sanitizer and its extra authenticated keys
belong to the unresolved encryption profile. This is an encoding/key-role
contract, not a selected BFV-RNS security profile. The planner imports the same
Galois-role constant. Exact level/basis variants, primes, key distributions,
key ownership and admission proofs remain unresolved.

The checked proof-envelope inventory includes the 348-byte statement, both
nonempty proof byte counts and declared remaining envelope bytes. The separate
encrypted-input cap remains 1 MiB; an input hash does not place the ciphertext
in the proof envelope. The inventory rejects arithmetic overflow, combined proofs
above 192 KiB and proof-envelope totals above 1 MiB. Remaining bytes must include
all additional framing/instances and any input/output/key bytes actually carried;
separately owned input/output/key material is not silently duplicated. This is
a declared-size calculation, not an implemented complete envelope codec:
completeness, policy-key loading, algorithm/prover scratch and actual proof
verification remain required.
All nine existing `UnqualifiedPlan` blockers remain; no production caller uses
these candidate owners or statements.

### One explicit lowering for all eleven operations

Use a uniform broadcast-scalar representation for each register and memory
lane. `Embed(v)` denotes the selected BFV scheme's exact trivial plaintext
embedding, not fresh encryption or sanitization. Those intermediate values
remain private evaluator/prover material. `CMul` includes its full tensor
product, scale/round and relinearization; `Galois(a)` includes a full key switch.
Neither is a host callback exempt from the final proof.

| Tape instruction | Required semantic lowering | Logical multiplication rank |
| --- | --- | --- |
| `LoadInput(dst,j)` | Mask packed input by `e_j`; for each of the seven fixed exponents, apply Galois then add to the running value; replace dst with the broadcast scalar. | 0 |
| `LoadState(dst,lane)` | Copy the lane's immutable value into dst, without changing the lane. | lane rank |
| `StoreState(lane,src)` | Snapshot src into the lane, without changing src. | src rank |
| `LoadConst(dst,v)` | `Embed(v)` broadcast to every scalar slot. | 0 |
| `Add(dst,a,b)` | Align immutable operand copies, add, replace dst. | `max(a,b)` |
| `AddPlain(dst,a,v)` | Exact canonical scalar plaintext addition. | a rank |
| `SubPlain(dst,a,v)` | Exact canonical scalar plaintext subtraction. | a rank |
| `MulPlain(dst,a,v)` | Exact canonical scalar plaintext multiplication. | a rank |
| `Mul(dst,a,b)` | Align copies, `CMul(a,b)`, replace dst. | `max(a,b)+1` |
| `SelectEqZero(dst,c,z,n)` | Full expansion below; source values are captured before replacing dst. | `max(c+10,z+1,n+1)` |
| `Output(src)` | Multiply captured src by next output mask; align and add to the packed output accumulator, preserving order. | max emitted rank |

The table's ranks are dependency depths, **not RNS modulus levels or noise
budgets**. A physical level scheduler may map ranks to a reviewed chain only
when its exact switching/rounding semantics and correctness bounds are supplied.
Addition or plaintext multiplication can exhaust noise without changing rank.
The existing logical rank ceiling 16 is retained as a planning bound, not a
promise that any encryption profile supports that depth.

For `SelectEqZero`, perform exactly eight repeated ciphertext squarings of c,
then one ciphertext multiplication by `Embed(1)` as in the source exponentiation
schedule. Form `indicator = Embed(1) - powered`, `delta = z - n`, then
`result = n + CMul(indicator, delta)`. This is ten ciphertext multiplications,
two ciphertext subtractions, one ciphertext addition and two explicit one
embeddings; constant folding is not part of this initial lowering. Squarings
require relinearization too. Aliasing dst with any source is safe only because
all sources remain immutable until the result is complete. This expansion is
correct only for scalar F257 inputs; the client admission and subsequent
scalar-preserving transitions are essential.

Each input load costs one polynomial plaintext mask, seven automorphisms with
key switches and seven ciphertext additions. Each output costs one polynomial
plaintext mask, and outputs after the first cost an accumulator addition.
A mask has centered l1 norm 8,256. A broadcast scalar has one nonzero polynomial
coefficient bounded by 128; a general scalar-packed message has centered l1 at
most 16,384. Intermediate partially broadcast/accumulated messages must retain
the appropriate message-shape bound. These facts are inputs to a noise proof,
not replacements for encoding/rounding and key-switch error terms.

The seven Galois-key roles and relinearization role must be authenticated from
the typed policy. The old diagnostic bundle's prohibition of rotation keys
cannot be reused for this replacement. Required actual level/basis variants,
decomposition digits, special primes and key sizes remain explicit unresolved
profile requirements. No bootstrap/refresh algorithm is implemented or
authorized by this plaintext contract. A future reviewed encryption profile
must specify its complete refresh/sanitization algorithm, extra authenticated
keys, public schedule, exact arithmetic and resource/noise rules. A refresh mask
is not a bootstrap, and no implicit modulus reset is permitted.

### Resource accounting and unresolved qualification

The planner must separate three outputs: exact structural counts, symbolic
profile-dependent requirements, and measured proof/backend limits. It may
produce an `UnqualifiedPlan`; it must not produce an executable/qualified token
while any noise, input-admission, key, physical-level, sanitization, codec or
proof requirement is absent. Unknown costs never become zero.

Track each register/lane's current value and rank, instruction/output counts,
all primitive counts, required key roles and checked live storage. Process all
256 possible tape positions; the universal relation constrains inactive positions
to preserve state and emit nothing. Private instruction count, opcode and index
selectors must not select a smaller public circuit, proof or key. Active-work
counts alone therefore underestimate the fixed hidden-program relation. Native
timing/allocation leakage needs a separately stated and qualified policy; this
planner does not establish oblivious execution.

The structurally valid worst cases are material: 255 independent selects followed
by an output require 2,550 ciphertext multiplications even though each select
has rank 10. Conversely, 255 input loads followed by an output require 1,785
Galois key switches. These are separate extrema, not one jointly attainable
program. Sixty-four outputs require 64 masks and 63 accumulator additions.
Initialized zero registers do not justify removing private instructions from
the proof relation. A complete feasibility estimate must cost the universal
selection/routing machinery and inactive positions as well as real arithmetic.

Let `C(level)=2*N*L(level)*8` bytes for a two-component residue owner and
`T(level)=3*N*L(level)*8` for an unrelinearized tensor, using canonical u64
residues. These are checked storage formulas, not selected N or limb counts.
Keep the input owner, 32 lane owners, four register owners, optional output
accumulator and all operands live until each replacement commits. Count
alignment copies, old destination, running broadcast plus rotated/add result,
select intermediates, three-component multiplication buffers, decomposition,
basis-extension/NTT scratch and clearing transfers separately. Immutable sharing
may reduce allocations only when a liveness proof counts the actual owners;
logical aliases do not authorize destructive mutation or early erasure.

The selected RNS implementation must provide a checked workspace inventory and
physical-level/noise transfer rule for every primitive. Plaintext masking needs
its coefficient norm **and** the scheme's plaintext-embedding/rounding terms;
`E'=8256*E` alone is not a complete BFV bound. Key switches, scale-and-round and
modulus switches need their actual error terms and temporary bases. A missing
rule yields a typed unresolved requirement, not a conservative-looking guessed
number. No planner may invoke the old scalar-modulus reconstruction owner as a
genuine Q-product profile.

After output packing, require an explicit reviewed circuit-privacy/sanitization
stage even for constant-only programs. Its fresh randomness, distribution,
noise cost, buffers, key material, proof constraints and lifetime/query limits
belong in the plan. Adding an encryption of zero is not accepted as a generic
sanitizer. A proof can constrain bounded coins and the transformation; it does
not prove that an honest random sampler supplied entropy. That assumption and
the clearing RNG owner require independent review and implementation evidence.

Input and output frames, both proof relations, verification keys, authenticated
evaluation-key material and any public-instance encodings must have exact
bounded canonical sizes. The combined proof budget remains 192 KiB, proof envelope
1 MiB and k<=16. The encrypted-input cap is separately 1 MiB;
inputs/evaluation keys already owned by the consumer must not be
duplicated as sidecars. Policy-key storage/loading is also bounded separately,
not hidden outside the accounting. Profile-specific maxima and prover-memory
ceilings remain to be supplied rather than invented here. The maximum single
Poseidon prototype already measured 233–239 ms complete IPA verification,
exceeding the existing 20 ms soft budget; no complete execution/admission budget
has passed.

The next test-only planner can implement tape/rank/count/alias bookkeeping and
symbolic primitive requirements without keys or encryption. Required controls
cover all eleven operations, all register/lane alias cases, zero-initialized
reads, exact 256/257 and 64/65 boundaries, rank-16/17 cases, all ten select
multiplications, the two extrema above, output snapshots, checked byte arithmetic
and actual scratch erasure on success/error/unwind. It remains structurally
unqualified until the replacement plan supplies and independently reviews every
profile-dependent rule and the complete proof budget.
