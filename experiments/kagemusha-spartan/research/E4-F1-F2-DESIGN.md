<!-- Written by Codex astra (gpt-6-astra, ultra reasoning), read-only, on 2026-11-18 at iroha 60309bba27, for TODO E4/F1/F2; prompt and run log in ../../../output/kagemusha-100x-20261108/astra-e4-f1f2-20261118/ (local, git-ignored). Saved unedited. Proposed construction; nothing here is implemented yet. -->

# KAGEMUSHA Path 2: E4, F1 and F2 construction

> **Decision update, 2026-11-18.** Revision `1e2afdc338` selects four pairs and three Control modes. Bootstrap remains in the recurring polynomial. The Unload and Refresh relation specifications below still apply with the revised kind placement and hard anchor check. [E4 reweigh and integration decision](E4-REWEIGH.md)

**Target:** `experiments/kagemusha-spartan/research/E4-F1-F2-DESIGN.md`  
**Status:** Proposed construction and implementation plan. Not implemented or qualified.  
**Review basis:** Working tree based on `60309bba27e8205c6196bb0c6b040b954f6f584e`, reviewed read-only on 2026-11-18. No files changed or tests executed.

## 1. Decisions and evidence

This design selects:

1. **E4:** Keep Bootstrap as a separately authenticated base program. Discharge its fresh matrix claims **inside the first successor’s P/S circuits**, then exclude Bootstrap from the recurring matrix polynomial. Continue accumulating its generator obligations.
2. **F1:** Add a recursive Unload program and connect its completed proof to Core’s existing proof-before-payout path. Keep suffix-Unload disabled.
3. **F2:** Implement RefreshTimeAnchor inside a **single fixed Control program shared with Load**, using a constrained branch selector.

The resulting catalog has **ten programs: two Bootstrap programs and eight recurring programs**. The recurring polynomial still has four kinds and two selector coordinates.

This is a trust-neutral construction relative to the existing Path 2 assumptions. It does not introduce an issuer or Core signature asserting that a Bootstrap proof was accepted.

**Performance remains UNVERIFIED.** Removing Bootstrap’s matrix program from native `q` does not remove the cost of proving the new discharge gadget. The shared Control program also includes TimeAnchor authentication. Both changes can enlarge the common layout.

### 1.1 Source authority

The normative operation semantics come from:

- [Single design proposal](../../../specs/kagemusha_single_design_proposal.md), especially §§3, 6.1 and 7.
- [Wallet wire specification](../../../specs/kagemusha_wallet_wire_v1.md), especially §§3.1–3.3 and the ledger-finality/Unload claim definitions.
- [Enrollment policy](../../../specs/kagemusha_wallet_enrollment_policy_v1.md).

The implementation baseline comes from the current source, particularly:

- [`full.rs`](../src/full.rs), [`deferral.rs`](../src/deferral.rs), [`points.rs`](../src/points.rs).
- [`bootstrap.rs`](../src/bootstrap.rs), [`load.rs`](../src/load.rs), [`send.rs`](../src/send.rs), [`receive.rs`](../src/receive.rs), [`receive_lineage.rs`](../src/receive_lineage.rs), [`receive_originals.rs`](../src/receive_originals.rs).
- [`statement.rs`](../src/statement.rs), [`operation_auth.rs`](../src/operation_auth.rs), [`receipt.rs`](../src/receipt.rs).
- [`engine.rs`](../src/engine.rs), [`native.rs`](../src/native.rs), [`program_artifact.rs`](../src/program_artifact.rs), [`uniform.rs`](../src/uniform.rs).

[DESIGN.md](../DESIGN.md), [A2-UNIFORM.md](../A2-UNIFORM.md) and [CONSTRUCTION.md](CONSTRUCTION.md) contain historical checkpoints. In particular, `CONSTRUCTION.md` still describes three kinds, an unused selector corner and synthetic monetary relations. Those passages do not describe current source behavior.

### 1.2 Implemented versus proposed

| Area | Observed implementation | This proposal |
|---|---|---|
| Recursive kinds | Receive, Send, Bootstrap, Load | Separate Bootstrap namespace; recurring Receive, Send, Unload, Control |
| Matrix deferral | Every fold includes all installed kinds | Bootstrap fresh claims discharged in-circuit |
| Bootstrap accumulator | Constant empty claim; `rho=0` makes it valid even at the occupied `(1,1)` corner | Preserve that invariant |
| Unload | Statement constraints exist; no experimental recursive operation | Complete relation, native custody and Core verifier integration |
| RefreshTimeAnchor | Statement tag exists; no experimental recursive operation | Exact authority/time relation within Control |
| Core ordinary Unload | Verifies claim and proof before replay lookup and payout | Preserve ordering; add explicit Path 2 verification |
| Core suffix-Unload | Explicitly refused | Remains refused |
| Recursive security | G1/G2/G3 remain open | Local discharge argument supplied; those gates remain open |

“Proposed” below means required engineering. “UNVERIFIED” identifies a property not established by the source review or a measured candidate.

## 2. Shared protocol and artifact profile

### 2.1 Separate program class from semantic operation

Replace the overloaded numeric kind with distinct types:

```text
ProgramClass =
    ColdBootstrap
  | Hot(Receive | Send | Unload | Control)

SemanticOperation =
    Bootstrap
  | Load
  | Send
  | Receive
  | Unload
  | RefreshPolicy(TimeAnchor)
```

“Cold” means excluded from the recurring matrix polynomial. It does not mean unauthenticated.

Use this public-input assignment on **both** sides:

| Program | io60 | io61 | io62: `base` | io63: `control_mode` |
|---|---:|---:|---:|---:|
| Cold Bootstrap | 0 | 0 | 1 | 0 |
| Receive | 0 | 0 | 0 | 0 |
| Send | 1 | 0 | 0 | 0 |
| Unload | 0 | 1 | 0 | 0 |
| Control: Load | 1 | 1 | 0 | 0 |
| Control: RefreshTimeAnchor | 1 | 1 | 0 | 1 |

These are canonical encodings. Other combinations fail.

The semantic statement tags remain unchanged: Bootstrap `1`, Load `2`, Send `3`, Receive `4`, Unload `6`, RefreshPolicy `7`; TimeAnchor’s update-kind tag remains `4`.

`base` and `control_mode` are **not additional MatPoly coordinates**. They select constrained behavior within the authenticated programs.

Current S ordinary IO has length 62. This profile changes it to **64**, matching P. Update native geometry checks, projection, transcript hashing, exact proof-byte binding and carrier reconstruction together. Existing code that assumes 126 combined ordinary inputs must change to 128.

### 2.2 Fixed common geometry

Select and freeze, per side:

- Constraint, row and column dimensions.
- First, link, second and fixed-row geometry.
- Ordinary, late, link and generator IO locations.
- Sumcheck schedule.
- Generator namespace, capacity and derivation.
- Matrix-accumulator dimensions.

All ten programs use the same complete verifier geometry on a given side. This permits one predecessor verifier slot whose key is selected in-circuit.

Compile Bootstrap first at that geometry. Its matrices must depend only on its own relation, selected geometry, generators and constant empty claims—not recurring-program keys.

Then build the recurring fixed point at that geometry. If any program exceeds it, **restart the whole candidate**, including Bootstrap compilation and evaluator derivation. Do not silently enlarge a recurring program beneath an already frozen Bootstrap artifact.

This avoids two problems:

- A witness-selected `Shape` would produce different circuits rather than one verifier.
- A Bootstrap evaluator containing recurring keys would introduce a construction dependency back into the fixed point.

### 2.3 Artifact binding

Replace the four-pair bundle with explicitly named:

```text
cold_bootstrap: ProgramPair
hot: [Receive, Send, Unload, Control]
```

The authenticated manifest must bind:

- All ten canonical program digests.
- Common geometry and IO ABI.
- Generator identities, capacities and derivation.
- Cold evaluator source identity and normalization version.
- Hot selector ordering and semantic-operation mapping.
- Scheme, relation identity and independently selected ledger-finality scope.

Use a distinct experimental artifact/profile version for this schema. Do not accept the current four-pair schema under the new interpretation or infer a generation from available bytes.

The evaluator tape is derived from the exact authenticated Bootstrap program. Its fixed constants and address columns are authenticated through the recurring program keys.

Do not embed a manifest’s own digest into its program constants. Compute program identities first and sign the resulting manifest. Relation identity remains a trusted public context input, as required to avoid an identity cycle.

Preserve the generator rules in `CONSTRUCTION.md`. In particular, changing `Gens::new(N)` changes `h` and `u`; a shorter independently constructed set is not equivalent to a prefix retaining its parent’s `h` and `u`.

## 3. E4: authenticated Bootstrap discharge

### 3.1 Construction comparison

| Construction | Soundness requirement | Per-fold native `q` | Proof bytes | Circuit and program consequences |
|---|---|---|---|---|
| **Selected: in-circuit discharge** | First successor checks the exact Bootstrap fresh matrix claims under pinned Bootstrap keys | Bootstrap absent from recurring `restrict`; discharge-gadget matrices contribute instead | No second accumulator or extra transmitted proof pair; geometry can still grow | Ten total programs, four recurring kinds; discharge gadget in every recurring predecessor path |
| Separate Bootstrap-only accumulator | Every terminal verifier decides it; Receive merges both parents’ Bootstrap obligations | Unary steps can copy it; Receive still evaluates Bootstrap for its merge | Approximately **+2,880 B** at current accumulator dimensions | Additional late IO and merge constraints; ten programs |
| Native decision certified into state | An authorized signer attests exact proof, state, program and claim acceptance; introduction verifies that signature in-circuit | Can remove Bootstrap from recurring `q` | Certificate introduction and verification costs; not necessarily a growing history | Adds an acceptance-attestation trust role absent from the current specification |
| Fold Bootstrap once, then switch to a reduced polynomial | Requires a proof connecting the old obligation to the new polynomial | Apparent saving if that proof is omitted | Small only because an obligation was dropped | **Unsound without discharge** |
| Two-level polynomial retaining a Bootstrap term | Must still authenticate its value when the selector becomes generic | Generic restriction still evaluates Bootstrap unless another discharge mechanism is added | Depends on representation | Reorganizing the polynomial alone does not remove the work |

For the separate-accumulator construction, a Bootstrap-only claim has coordinates:

```text
(rho, alpha, beta, x[XB], y[YB]), value
```

Its additional raw size across both fields is:

```text
32 · Σside (XB + YB + 4)
```

Using current `FP_DIMS = ((20,22),(19,21))`, this is 2,880 bytes. Applied to the historical 11,597-byte Payment, it gives at least 14,477 bytes before further framing. It fails the experimental budget.

That construction is nevertheless sound if implemented completely: the first successor inserts Bootstrap’s fresh claim; unary operations carry the accumulated claim; Receive merges its own and its payer’s claims, for example along a line. The line degree is bounded by `XB + YB + 3 + dα + dβ`. A one-time native decision followed by omission is not this construction.

The signed-certificate alternative is rejected here because existing credential/table certification does not authorize a signer to attest arbitrary recursive-proof acceptance.

### 3.2 Recurring polynomial

For each side, define only:

```text
H = {Receive, Send, Unload, Control}
```

and

```text
P_hot(c0,rho,alpha,beta,x,y,c1)
  = Σk∈H eq((c0,c1),corner(k))
      · pad_k(x,y)
      · (rho A_k + rho² B_k + rho³ C_k).
```

The selector coordinates of accumulated claims remain general field elements. Booleanity applies to proof-kind inputs, not to folded accumulator coordinates.

Keep the existing cubic four-claim fold. For a one-predecessor operation, repeat its inherited/fresh pair as today. Receive also folds its payer’s inherited/fresh pair under the existing total-verification rules.

Keep the canonical empty claim:

```text
c0 = c1 = 1
rho = alpha = beta = all x = all y = value = 0
```

Its validity follows from `rho=0`. The `(1,1)` corner now selects Control; it is not an unused corner.

### 3.3 Predecessor verification and key selection

Let `b` be predecessor `io62`, read through its constrained verifier IO.

Require:

1. `b`, `c0` and `c1` are Boolean.
2. P and S bind the same discriminator, kind and control mode through the pair link.
3. The selected fixed-row key is exactly:

   ```text
   Kselected = b·KBootstrap + (1−b)·Khot(c0,c1).
   ```

   Implement the coordinate selection with materialized products and complete point checks; this notation does not authorize unchecked point interpolation.

4. Bootstrap’s programs pin `base=1`; every recurring program pins `base=0`.
5. `b=1` requires Bootstrap’s canonical kind bits and mode.
6. The ordinary scalar, points, transcript and pair-link checks run for either selected key.
7. Wallet-table substitution remains independently bound to the verified wallet’s io56–59.

Only the wallet’s own predecessor slot may select Bootstrap. Receive’s payer slot requires:

```text
base = 0
kind = Send
control_mode = 0
```

A host enum, cached native result or unverified profile array cannot supply these facts.

### 3.4 Exact cold matrix check

For each side, obtain from the predecessor verifier:

- Its actual matrix-combination challenge `r`.
- Its second-phase challenges `αB, βB`.
- Its exact `rx, ry`.
- The scalar verifier’s matrix-combination value `v`.

These are the predecessor’s transcript values, not the current successor’s LogUp challenges.

For every input word of the authenticated Bootstrap evaluation tape, constrain:

```text
u_i = b · transcript_input_i.
```

Evaluate Bootstrap’s exact matrices at `u`, obtaining `AB, BB, CB`, and require:

```text
b · (v − r·AB − r²·BB − r³·CB) = 0.
```

For `b=1`, this decides the fresh Bootstrap matrix obligation. For `b=0`, the evaluator runs on its fixed zero input and imposes no Bootstrap equality on the recurring predecessor.

The evaluator remains present in both cases. Host-side omission when `b=0` is forbidden.

Perform this check for **both P and S**. Neither half’s decision substitutes for the other.

### 3.5 Uniform evaluator

Do not expand Bootstrap’s tape into irregular per-instruction matrix coefficients. Use the existing uniform compiler and copy argument.

Normalize the immutable `uniform::Tape` into five opcode families:

| Opcode | Constraint |
|---|---|
| Add | `out − left − right = 0` |
| Sub | `out − left + right = 0` |
| Mul | `left · right = out` |
| Scale | `left · constant = out` |
| Const | `out = constant` |

Implementation requirements:

- Export every input and instruction result.
- Import operands using the tape’s immutable source mapping through `Wiring` copy imports.
- Place the constant pool in `fixed=true` arrays.
- Use the authenticated output indices for A, B and C.
- Derive instruction counts, multiplicities, widths, source addresses and padding from the program, never from witness values.
- Reject forward references, invalid constant indices and invalid output indices.
- Preserve the fixed address columns generated by the copy argument as authenticated fixed rows.

`uniform::copy_layout` already turns arbitrary import addresses into fixed committed columns and uniform LogUp checks. That is the mechanism intended here.

This is a fixed arithmetic evaluator gadget for authenticated matrix programs, not a new wallet VM or a prover-supplied instruction language.

### 3.6 Claim routing

Let `E` be the canonical empty claim, `Aprev` the predecessor’s actual late claim, and `Fhot` its fresh recurring claim.

Constrain every coordinate, including the value:

```text
inherited_to_fold = b·E + (1−b)·Aprev
fresh_to_fold     = b·E + (1−b)·Fhot
```

Additionally require:

```text
b · (Aprev − E) = 0
```

coordinatewise.

The fresh-claim substitution is permitted only with the matrix check in §3.4 and Bootstrap key selection in §3.3. A recurring proof cannot set a flag to erase its accumulator.

Do not fabricate `Fhot` by sending a Bootstrap enum through the hot-kind lookup. Construct the two routing cases explicitly with constrained muxes.

### 3.7 Generator obligations remain

Matrix discharge does **not** decide an IPA generator claim.

Bootstrap must pin its inherited generator accumulator to the canonical empty value. Its own IPA generator claims must enter the successor’s existing generator accumulation, just as other verified proofs’ own claims do.

Every accepting native boundary continues deciding:

- Both linked proofs.
- Both recurring matrix accumulators.
- Own and inherited generator obligations.
- Exact installed program identity.

Direct native acceptance of Bootstrap verifies its cold programs and requires its inherited claims to be canonical empty. No native `bootstrap_checked` Boolean is exported as recursive authority.

### 3.8 Soundness argument and limits

Condition on sound extraction/composition for the accepted Path 2 proof history.

For each wallet-predecessor edge:

- A recurring predecessor contributes its actual inherited and fresh matrix claims.
- A Bootstrap predecessor is checked under the exact Bootstrap key, its fresh matrix equality is enforced arithmetically, and its inherited claim is proven empty.

Thus a false Bootstrap matrix claim cannot be replaced by zero by a satisfying successor witness. Later proofs inherit that constraint through recursion. Receive preserves it across joined histories because its payer is a fully constrained Send and its associated obligations are retained.

A later counterparty needs the installed programs and the current Payment, not the original Bootstrap bytes.

This establishes the local discharge invariant. It does **not** close G1–G3. Still **UNVERIFIED**:

- Recursive Fiat–Shamir composition and extraction through joined histories.
- The complete finite-lifetime/query-budget argument.
- Aggregate soundness accounting.
- Independent cryptographic review.

For each hot fold, use the actual bound:

```text
D = 3·(2 + X + Y) + 9 + 3·dα + 3·dβ
```

over the raw 126-bit folding challenge distribution. A conditional algebraic term is `D/2^126`, before Fiat–Shamir, composition and grinding losses. Add the evaluator’s copy/lookup soundness terms; do not claim a free or exact-zero error reduction.

### 3.9 Cost consequences

The historical 53,964 figure counts products, not all evaluator work. The cited checkpoint also contained:

- 27,962 constant scales.
- 168,952 total P-Bootstrap instructions.
- 412 total S-Bootstrap instructions.

The discharge gadget includes those instruction families plus copy imports, exports, lookup helpers, fixed columns and range/multiplicity work. It is not a 53,964-row addition.

Its benefit is structural: the original Bootstrap tape is absent from every recurring `restrict` evaluation. Its cost is that every recurring circuit proves a uniform evaluator, including on non-Bootstrap steps.

**UNVERIFIED:** whether the resulting native `q`, complete proving time, memory and Payment bytes improve. The historical 72% product share is not a predicted speedup.

## 4. F1: recursive Unload

### 4.1 Inputs and common authentication

Unload has one hard wallet predecessor and no incoming payer slot.

Let:

```text
C, R       = predecessor Core30 and Rest6
C′, R′     = successor Core30 and Rest6
L          = predecessor’s authenticated lineage outputs
a          = face amount
c          = online charge
j          = C[14], the redemption ordinal
q          = charge-quote digest, or zero
```

Require:

- Canonical before/after state openings.
- `commit(C,R) = verified_predecessor.head`.
- Current credential authentication by exact predecessor continuity.
- Equality of scheme, relation, asset, wallet, credential, payment key and certified table commitment.
- Predecessor lifecycle, policy epoch and enabled-controls joins.
- A valid own Advance receipt under the continuing payment key.

Reuse `operation_auth::append_current`; opening credential bytes alone does not authenticate them.

### 4.2 Exact state transition

Constrain `a,c,j` and all affected counters as checked `u128`.

Require:

```text
a > 0
0 ≤ c ≤ a
q = 0 iff c = 0
```

Introduce `r`, constrained to `u128`, and enforce:

```text
C[9] = a + L.burned_total + r
C′[9] + a = C[9]
C′[10] = L.burned_total
C′[18] = L.pending_outgoing_root
C′[11] = C[11] + 1
C′[14] = j + 1
```

The bounded integer equations reject underflow and overflow; they must not be modular arithmetic shortcuts.

The use of `L.burned_total` and `L.pending_outgoing_root` is mandatory. Private Core values can be stale after lineage adjustments.

Compute:

```text
N = P(kgwnull1, [scheme_lo, scheme_hi, wallet_lo, wallet_hi, j])
```

and require `N` nonzero.

Insert into the shared Load/redeem recovery map:

```text
key   = 2·2^128 + j
value = P(kgwrdm_1, [j, N, a, c])
```

The depth-32 indexed-map relation must prove:

- Absence under `C[19]`.
- Correct low-leaf relinking.
- An empty destination slot.
- Exact resulting root `C′[19]`.

Set `C′[29]` to a canonical nonzero successor nonce supplied by native entropy. The relation proves its encoding, not randomness.

The complete changed-index set is:

```text
{9, 10, 11, 14, 18, 19, 29}
```

Every other Core field is equal. Every Rest field is equal.

Active and Retiring wallets may Unload. Preserve lifecycle. Do not add Send quotas, lease expiry or current attestation-revocation checks as new Unload restrictions.

Output lineage carries unchanged verified burned, pending and credit-digest roots, with lifecycle/policy/control facts consistent with the successor.

### 4.3 Statement and receipt

Use the existing 26-field statement layout.

| Fields | Required value |
|---|---|
| 0 | Version `1` |
| 1–2 | Authenticated relation |
| 3–4 | Scheme |
| 5–6 | Asset |
| 7 | Current credential digest |
| 8 | Successor lifecycle |
| 9 | Successor sequence |
| 10 | Successor `next_load` |
| 11 | Predecessor enabled-controls mask |
| 12 | `L.burned_total` |
| 13 | `L.pending_outgoing_root` |
| 14–15 | Before and after commitments |
| 16 | Semantic Unload tag `6` |
| 17–21 | `N, j, a, c, q` |
| 22–25 | Zero |

Compute the existing operation identifier:

```text
operation_id = P(kgwopid1, [wallet_lo, wallet_hi, 6, N])
```

The own receipt binds this operation, exact statement digest, sequence, predecessor, successor and frozen custody/preparation commitments. Its incoming Payment digest is zero.

Preserve the existing receipt/preparation ordering. Do not make a receipt sign the completed recursive proof containing that receipt.

### 4.4 Quote and payout binding

Keep the normative division: the relation binds the charge and quote digest; Native and Core authenticate the exact signed quote.

Before Advance, Native verifies and retains any quote. Core independently verifies:

- RegulatoryPolicy-role certificate under the installed scheme root.
- Quote signature and canonical digest.
- Scheme, asset, wallet, Unload kind, ordinal, face amount and charge.
- Beneficiary account digest.

Core also requires:

```text
H("account", claim.account) = authenticated credential.account_digest
```

The payout is exactly:

```text
a − c  → credential-bound account
c      → quote-bound beneficiary
```

For zero charge, the quote is absent and the account receives `a`.

Any account may relay the transaction. Its authority cannot change either payout recipient.

### 4.5 Core acceptance

Introduce a future-generation Unload claim carrying:

- Exact current credential and required certificates.
- Exact statement and own receipt.
- Completed linked Unload proof pair and reconstructable public profile.
- Credential-bound `AccountId`.
- Optional exact quote and beneficiary `AccountId`.

The completed pair recursively proves the predecessor obligation; this is not an unproven suffix or a request to pay before proving.

Core must:

1. Select the authenticated program set from its installed World state.
2. Decode canonical bounded bytes.
3. Require the Unload program class and semantic tag.
4. Verify P and S, their link, all matrix accumulators and all generator obligations.
5. Bind carried statement/receipt/credential to the proof’s public outputs.
6. Check the account and quote bindings.
7. Only then inspect the payout replay record or mutate balances.

Return an opaque verified claim internally. No caller-supplied proof or finality Boolean authorizes payout.

Current `ledger.rs::pay_unload` already verifies before replay lookup. Preserve that ordering, including exact retries.

### 4.6 Exactly-once storage

Use the existing replay key:

```text
(scheme_id, Unload(N))
```

For the new profile, store at least:

```text
nullifier
canonical claim digest
face amount
original successful transaction hash
```

The claim digest must commit the entire canonical claim, including payout account and quote originals. Define the new domain:

```text
claim_digest = P_bytes(kgwuclm1, canonical_unload_claim_bytes)
```

The claim contains no self-referential digest field. Canonical decoding precedes hashing.

Retain the accepted exact claim where required for bounded recovery. Apply reserve debit, both transfers and the replay record in one atomic ledger batch.

- Same nullifier and identical claim: return the original result after proof verification.
- Same nullifier with another claim: reject.
- Failure of either transfer: roll back all transfers and the replay insertion.
- Concurrent duplicate transactions: at most one payout.

Keep the current normative Unload claim bound of 16,384 bytes unless separately amended. Fit of the new complete claim is **UNVERIFIED**.

### 4.7 Finality, retries and suffix hold

Core verifies the wallet’s proof itself. It accepts no externally asserted finality verdict. Ledger-origin evidence supplied to Core must be authenticated by its native finality owner; executing its own payout remains ordinary consensus execution.

The wallet marks payout confirmed only after authenticating finalized execution of the exact retained signed envelope:

- Selected network and epoch authority.
- CommitQC.
- Entrypoint inclusion.
- Joined Network output inclusion.
- Matching entry/output/input indices.
- Successful execution of the instruction carrying the identical claim.

An authenticated rejection concerns that envelope only. It never restores offline value or permits replacement or retargeting. Another envelope must carry the identical claim.

Keep all `SuffixUnloadBegin`, `SuffixUnloadAppend` and `SuffixUnloadFinalize` refusals in `execute_with_verifier`. This work does not lift the proof-before-payout hold.

**Finality limitation:** the existing Load relation binds the receipt and Advance but does not prove BLS finality. Its normative native pre-Advance finality carve-out remains. The recursive proof does not newly let Core independently establish every ancestral Load’s finality from proof bytes alone. That stronger construction is **UNVERIFIED and outside this proposal**.

Unload spends current proven spendable balance. It cannot reclaim an earlier Send. Credited evidence, timeout, delivery loss or ledger rejection cannot refund or retarget a committed payment.

## 5. F2: RefreshTimeAnchor

### 5.1 Meaning

RefreshTimeAnchor authenticates a signed TimeAnchor, changes the held anchor digest, and updates the accepted-time floor exactly.

It does not:

- Renew a credential or lease.
- Change the payment key or certified table.
- Change prior-credential membership.
- Reset quotas or replay maps.
- Prove genuine OS time, boot identity or response latency.

Use semantic `RefreshPolicy`, tag `7`, with update kind `TimeAnchor=4`.

### 5.2 Signed authority object

Parse the exact 138-byte body:

| Bytes | Field |
|---|---|
| 0–1 | LE16 version `1` |
| 2–33 | Scheme |
| 34–65 | Wallet |
| 66–97 | Nonzero 256-bit nonce |
| 98–105 | LE64 issuer time `T` |
| 106–137 | Canonical nonzero signer-certificate digest |

Verify the exact 108-byte certificate and its signature under the installed scheme root. Require:

- Matching scheme.
- **TimeAnchor role `4`**.
- Canonical valid P-256 public key.
- Exact certificate-object digest equal to the body’s signer-certificate field.

Compute and verify:

```text
m = P_bytes(kgwtanc1, body)
ECDSA-P256-SHA256(signature, canonical_32_bytes(m))
d = P(kgwotim1, [m, r_lo, r_hi, s_lo, s_hi])
```

Enforce canonical signature scalars and low-S.

The TimeAnchor key is distinct from the wallet payment key. The current certificate has no certified table commitment. If comb verification is used, derive and constrain its table from the authenticated certificate key. Do not reuse the wallet table or trust a supplied signer table.

### 5.3 State and continuity

Open `C,R,C′,R′` and authenticate the predecessor as for other operations.

Use current-credential continuity and require:

```text
anchor.scheme = C.scheme
anchor.wallet = C.wallet
d ≠ 0
d ≠ R[5]
```

Constrain:

```text
C′[11] = C[11] + 1
C′[28] = max(C[28], T)
R′[5]  = d
```

with checked sequence arithmetic.

Every other Core field except nonce `[29]` is equal. Every other Rest field is equal. The successor nonce is canonical nonzero and chosen by native custody.

In particular, preserve:

- Credential digest `C[7]`.
- Prior-credential root `C[8]`.
- Balance and private Core burned value.
- All ordinals, chains and map roots.
- Lease, quota and policy fields.
- Exact payment key and io56–59 table commitment.
- All predecessor lineage outputs, including adjusted burn and roots.

Do not materialize a lineage adjustment into Core during this non-consuming operation.

Both Active and Retiring lifecycle values remain valid.

The existing model does not require `T>0` or `T>old_floor`. A different authenticated older anchor is a valid transition that preserves the floor.

### 5.4 Exact maximum constraint

Let `f=C[28]`, with `f,T,F,d0,d1` constrained to `u64`, and `h` Boolean:

```text
(1−h)·(f − T − d0) = 0
h·(T − f − 1 − d1) = 0
F = f + h·(T − f)
C′[28] = F
```

Optionally pin inactive differences to zero:

```text
h·d0 = 0
(1−h)·d1 = 0
```

This forces `h=1` exactly when `T>f`. Merely requiring `F≥f` and `F≥T` is insufficient because it permits an artificial floor jump.

### 5.5 Statement and receipt

Use the common statement bindings from Unload, except:

```text
statement[12] = statement[13] = 0
statement[16] = 7
statement[17] = 4
statement[18] = d
statement[19] = C′[28]
statement[20..26] = 0
```

Compute:

```text
operation_id = P(kgwopid1, [wallet_lo, wallet_hi, 7, d])
```

Verify the own Advance receipt under the continuing wallet key/table. Its incoming Payment digest is zero.

### 5.6 One fixed Control program

Let `t=io63`, with Load `0` and RefreshTimeAnchor `1`.

The same P program must contain both branches. Its common portion owns:

- Predecessor verification and E4 discharge.
- State openings.
- Current credential and table continuity.
- Statement digest.
- Own receipt.
- Public outputs and lineage.
- Matrix and generator accumulation.

Constrain:

```text
semantic_tag = 2 + 5t
operation_input =
    (1−t)·load_receipt_digest + t·anchor_digest
operation_id =
    P(kgwopid1,
      [wallet_lo, wallet_hi, semantic_tag, operation_input])
```

The Load branch retains the complete existing Load relation and native finality check. The Refresh branch enforces §§5.2–5.5.

S binds the same `t` through the pair link. Non-Control programs pin it to zero.

A Rust `match` that compiles different matrices is not permitted. Both branches, valid and invalid inputs, differing signer keys and differing floor comparisons must produce identical canonical public programs.

### 5.7 Branch construction rules

Duplicate branch-specific computation, not the common credential/receipt machinery.

For a branch with enable `e`, bind each branch-local external input using:

```text
x_branch = e·x_actual + (1−e)·x_dummy
```

Use fixed canonical dummy data. No inactive copy import may accidentally require real-state values that conflict with that dummy.

For a branch-specific row `A·B=C`, a safe R1CS enable transformation is:

```text
z = A·B
e·(z−C) = 0
```

Apply enabling to every required branch-specific join and validity assertion. Keep all lookup/copy structure and multiplicity widths fixed across both modes.

Inactive authority branches cannot require a genuine root-signed dummy certificate. Use total, constrained signature predicates:

```text
t·(1−certificate_valid) = 0
t·(1−anchor_signature_valid) = 0
```

Original encoding validity, role, scheme and key bindings must remain part of those predicates or active constraints.

The existing hard-valid certificate builder and Request-specific total signature interface do not already provide this general interface. Their refactor is required engineering. Completeness and cost are **UNVERIFIED**.

### 5.8 Native direct-time custody

Reuse the existing direct-exchange owners:

- `session_custody.rs::DirectTimeExchangeV1::complete`.
- `Coordinator::{begin_direct_time_exchange,finish_direct_time_exchange}`.
- `preparation_custody.rs::anchored_time`.
- `preparation_v1/refresh.rs::time_anchor_update`.

A usable local anchor requires a native-owned direct exchange:

1. Fresh nonce and request monotonic reading in the current boot.
2. Exact signed response matching scheme, wallet and nonce.
3. Same boot and `m_rcv ≥ m_req`.
4. Response width within the committed maximum response time.
5. Durable protected observations bound to the exact anchor digest.

A cached or peer-carried signed anchor may update the state floor. It must not create those protected observations or restart elapsed time.

At a later reading `m`, use checked arithmetic:

```text
lower = T + (m − m_rcv)
upper = T + (m − m_req)
```

Time-controlled Send uses the protected observation for the exact committed anchor. Reboot, monotonic rollback, missing observations or overflow refuse the applicable control. Controls-off payments acquire no new clock requirement.

RefreshTimeAnchor is not RefreshCredential. When credential renewal is implemented, it must authenticate the replacement credential, preserve the payment key/table, and extend the prior-credential array. This operation must neither emulate nor bypass that work.

## 6. Fixed point, bytes and timing

### 6.1 Program count and fixed-point effects

| Change | Program-count effect | Fixed-point effect |
|---|---|---|
| E4 | Separate cold pair; Bootstrap removed from recurring polynomial | Five-way key selection and evaluator arrays change all recurring programs |
| F1 | Adds Unload P/S at the freed recurring corner | Unload matrices participate in every recurring `q` |
| F2 | Replaces Load P/S with Control P/S; no additional pair | Both branches affect Control size and potentially every shared verifier |
| Combined | **10 total, 8 recurring; 4 recurring kinds** | Full rebuild and new authenticated program set |

All affected program digests must be regenerated. Equality of 32 sampled matrix evaluations is not a fixed-point certificate.

Require canonical full-program equality for consecutive stable rebuilds, covering instructions, constants, fixed values, geometry, challenge degrees and IO maps. Test witness independence separately.

### 6.2 Payment bytes

No Bootstrap original, TimeAnchor original, Unload claim or ledger-finality evidence is appended to later offline Payments.

At unchanged geometry and accumulator dimensions:

- E4 adds no second accumulator.
- F1 adds no Payment object.
- F2 adds no Payment object.
- Kind/base/mode inputs are reconstructable from the authenticated profile and semantic operation.

However, S ordinary IO grows from 62 to 64. A carrier that serializes all ordinary inputs grows by 64 bytes; the compact Payment profile must reconstruct the added values. The corresponding transcript/circuit changes still require measurement.

Geometry growth may add row commitments, sumcheck commitments, IPA rounds or late coordinates. Fixed rows being omitted from proof bytes does not make their layout cost free.

Historical observations are only reference points:

| Historical complete Payment | Margin against 12,000 B |
|---:|---:|
| 11,597 B | 403 B |
| 11,698 B | 302 B |
| 12,050 B | Fails by 50 B |

**Final Payment size: UNVERIFIED.**

Measure the entire uncompressed carrier, including framing, reconstructed-context profile, proof pair, link, originals and companion. `Proof::to_bytes()` alone is not the budget.

The experimental 12,000-byte target does not amend the production 10,034-byte limit.

### 6.3 Native `q` and circuit work

Measure native restriction work using the complete installed hot set:

```text
Wq ≈ Σk∈hot evaluation_cost(k)·grid_points(k)
     + extension/interpolation work.
```

Grid sizes depend on the actual matrix-variable and challenge degrees. Recompute those degrees after adding evaluator lookups and Control branches.

The historical four-thread Mac medians were:

| Side/operation | Native `q` |
|---|---:|
| P Receive | 196.6 ms |
| S Receive | 32.2 ms |
| P Send | 179.9 ms |
| S Send | 48.9 ms |

These describe a historical three-kind checkpoint, not this construction or Pixel performance.

Expected structural effects:

- **E4:** removes repeated raw Bootstrap evaluation; adds evaluator proving work to every recurring operation.
- **F1:** one predecessor, one own receipt, checked arithmetic and one recovery insertion; no payer verification or in-circuit BLS.
- **F2:** shared Control costs approximately `common + Load-only + Time-only + selection`, not their maximum. Time-only includes certificate and anchor authentication and signer-table derivation.

Sharing minimizes kind count. It does not prove minimum runtime.

### 6.4 Pixel acceptance criterion

Measure:

```text
retained exact input
→ preparation checks
→ authorized Advance/signing
→ proof construction, including q
→ complete native verification
→ durable output publication
→ releasable output
```

Record non-overlapping phase timings, peak RSS and complete Payment bytes from the same candidate and run.

**Approximately 3 seconds: UNVERIFIED.** The historical 2.08-second uniform pair and kernel models do not measure these complete relations.

Preserve the 0.68 GiB proving-admission target for 4 GB phones. Do not trade a smaller native `q` for an unadmitted witness layout.

Physical Pixel work requires the existing owner authorization in the operator’s chat. This design review authorizes no device run.

## 7. Implementation plan

### 7.1 E4 owner: recursion and compiler

Add `src/bootstrap_discharge.rs` with:

- Canonical cold-tape normalization.
- Five-family evaluator construction.
- Transcript-input binding.
- Discharge equality.
- Accumulator routing.

Modify:

- `full.rs`: `TracedPair`, `UniformPair`, `PairGens`, kind types, `slot_kinds`, `kind_values`, `kind_pin`, `p_circuit_impl`, `s_circuit_impl`, `defer_into_mode`, `p_boot_from_inputs`, `s_boot`, and fixed-point constructors.
- `points.rs::KeySelect` and keyed verification: explicit cold/hot selection.
- `deferral.rs`: hot-only polynomial construction and canonical empty-claim handling.
- `generator_integration.rs`: preserve cold own claims and enforce cold inherited emptiness.
- `uniform.rs`: expose a checked immutable tape view; reuse copy imports and fixed arrays.
- `bootstrap_layout.rs`: frozen geometry and overflow rejection.
- `program_artifact.rs`: authenticated cold dependency/evaluator metadata and roundtrip checks.

Do not weaken invalid incoming Payment/Burn handling while changing claim routing.

### 7.2 F1 owner: monetary relation and Core

Add `src/unload.rs`:

```text
Witness
UnloadOperation<R>
CompleteUnload
PreparedUnload
append
append_preparation
append_relation
```

Reuse `monetary_state`, `operation_auth`, `statement`, `receipt` and `indexed_map`.

Extend:

- `full.rs::{OperationWitness,CompleteOperation,public_sources,p_operation}`.
- `native.rs::ExactOperationWitness`.
- `native_witness.rs` canonical encoding and secret wiping.
- `engine.rs::{kind,terms,receipt_mut,check_preparation,prepare_retained,finalize,verify_completion}`.
- Native retained claim/submission/finality recovery ownership.

Core/model integration targets:

- `crates/iroha_core/src/kagemusha_wallet_v1/{artifacts.rs,ledger.rs,wsv.rs,tests.rs}`.
- `crates/iroha_core/src/smartcontracts/isi/kagemusha_wallet.rs`.
- The future-generation counterpart of `KagemushaWalletUnloadClaimV1::{verify,payout}`.
- Existing native Unload and ledger-producer custody owners.

Keep balance and replay authority in the canonical owners. The Core adapter must obtain authenticated verifier material from World, not the submitted claim.

### 7.3 F2 owner: authority relation and native time

Add:

- `src/time_anchor.rs`: exact body, authority, digests and refresh constraints.
- `src/control.rs`: fixed Load/Refresh union and shared operation ports.

Refactor:

- `load.rs`: expose branch-specific relation construction without changing Load semantics.
- `certificate.rs::prepare`: composable required-role authentication.
- `signature.rs::append_request_signature_total`: reusable total key/message/signature interface.
- `statement.rs::append_statement`: fixed-layout selected Control constraints.
- `receipt.rs::{append_body_fields,append_own,append_preparation}`: constrained semantic tag and operation-input selection.
- Native witness, preparation, completion and resume paths.

Reuse direct-time custody rather than adding an app-maintained anchor journal.

### 7.4 Shared integration owner

Update:

- `ParameterManifest`, `ProgramBundle`, `ParameterSet` and installation.
- `carrier.rs`, `payment_wire.rs`, `proof_bytes.rs`, incoming decoding and public retention.
- `chain_fixture.rs`, `backend_chain_tests.rs`, native fixtures and memory admission.
- SDK/profile projections when the future codec is implemented.

Regenerate the complete artifact set only after E4, F1 and F2 are integrated. Production credential/Core codecs and rollout remain F4/F5 work; this document does not claim they are complete.

### 7.5 Execution order

1. Implement and test the evaluator independently against exact Bootstrap tapes.
2. Implement the fixed Control relation and Unload relation in parallel.
3. Integrate the discriminator, key selection, claim routing and catalog.
4. Census actual layouts and select one frozen geometry.
5. Compile Bootstrap, derive its evaluator, and converge the recurring fixed point.
6. Integrate Core proof-first payout and native recovery.
7. Run full chain, byte, memory and security refusals.
8. Measure the authorized device candidate and obtain independent review.

If the selected construction fails the byte/time/memory gate, retain the sound implementation as research evidence and revisit the construction. Do not obtain a passing measurement by omitting discharge, generator checks, authority verification or payout proof verification.

## 8. Required validation

### 8.1 E4 refusals

- Evaluator/native tape parity in Fp and Fq, including every opcode, zero inputs, repeated operands and boundary indices.
- Mutated operand, result, constant, output address or copy helper breaks constraints.
- Modified tape/address/fixed data fails installed identity checks.
- Forward references and malformed artifacts reject.
- Cold/hot key substitution, non-Boolean flags, P/S flag mismatch and noncanonical cold encoding reject.
- Receive payer presented as Bootstrap rejects.
- False cold P or S matrix claims cannot be erased.
- Nonempty cold inherited matrix or generator claims reject.
- Corrupt cold **own** generator claims remain detectable downstream.
- False hot accumulators cannot be erased with the base branch.
- Changed table commitment or generator identity rejects.
- Active and inactive evaluator witnesses have identical public programs.
- Several successors and two-wallet Receive joins reject corrupted Bootstrap histories when the final verifier has no Bootstrap original.
- Construction/template artifacts with false accumulators reject at every accepting native boundary.

### 8.2 Unload correctness and refusals

- Partial and full spendable Unload; zero/nonzero charge; Active/Retiring.
- Exact `balance−adjusted_burn` boundary passes; one above fails.
- Stale Core burn/pending values cannot override lineage.
- Zero amount, excessive charge, quote-presence mismatch and counter overflow fail.
- Wrong nullifier scope/ordinal, duplicate recovery key, occupied slot, altered path/root fail.
- Mutate every preserved state field and re-sign: transition still fails.
- Wrong predecessor, credential, key, table, statement or program fails.
- Every corrupted proof/matrix/generator case refuses payout.
- An invalid proof still refuses after a previous successful payout.
- Account, beneficiary and quote substitutions fail.
- Concurrent exact retries pay once.
- Failed transfer atomically rolls back payout records and all transfers.
- Restart and uncertain-submit recovery preserve exact claim bytes.
- Credited evidence, rejected envelopes and timeouts never restore or retarget value.
- Preserve `unload_pays_once_requires_proof_even_on_retry_and_keeps_full_value` and suffix refusal coverage.
- Foreign network, wrong epoch, substituted envelope and mismatched inclusion/output indices never confirm payout.

### 8.3 Refresh and Control refusals

- Native/circuit parity for body, signing message, object digest, statement and receipt.
- Wrong root, role, scheme, wallet, certificate or signer table fails.
- Bad/high-S signatures, zero nonce and noncanonical fields fail.
- Older/equal/newer issuer times produce the exact maximum.
- Artificial floor jumps and sequence overflow fail.
- Reinstalling the currently held anchor fails; completed-operation retry returns retained bytes.
- Every preserved state, lineage, credential, prior-root, payment-key and table field is mutation-tested.
- Flip mode, semantic tag, update kind or operation-ID input: reject.
- Both Control branches produce identical program bytes.
- Inactive invalid signatures do not block Load; the same signatures cannot authorize Refresh.
- Active authority computation cannot omit an input-validity predicate.
- Cached/peer anchors cannot create direct-exchange observations.
- Wrong/reused nonce, reboot, timeout, reversed readings and wrong retained digest fail native anchoring.
- Controls-off Send acquires no clock dependency.

### 8.4 Combined fixed-point and lifecycle tests

Run fresh chains including:

```text
Bootstrap → Load → Send
Bootstrap → Receive
Bootstrap → RefreshTimeAnchor → Load
Receive → Unload → Send
Unload → Load → Unload
RefreshTimeAnchor → Receive → Send
```

Use two distinct wallets for payments. Include both Control modes after every valid predecessor class, repeated Refresh, post-Burn spendable-value checks and joined histories.

For every candidate:

- Require exact converged program bytes/digests.
- Rebuild under differing wallet keys and witnesses.
- Reject mixed geometry, old profiles and substituted programs.
- Verify every produced pair and all deferred values.
- Serialize actual complete Payment and Unload carriers.
- Record phase time and peak memory.

Run focused experiment tests first, then affected Core/model suites and integration tests. Reuse warm Cargo targets and repository build wrappers where applicable. No build or device test was run during this read-only review.

## 9. Completion criteria and remaining assumptions

E4 is complete only when Bootstrap is absent from recurring `MatPoly`, its claims cannot be discarded without the in-circuit discharge, downstream refusals pass, and actual costs are recorded.

F1 is complete only when a completed Unload proof causes an exactly-once Core payout, retries remain immutable, native finalized-outcome recovery works, and suffix proof skipping remains refused.

F2 is complete only when both operations share one exact Control program, TimeAnchor authority is proven, credential/table continuity holds, and native direct-time custody remains separate from signed-time floor updates.

The following remain **UNVERIFIED** until separately demonstrated:

- Final circuit sizes and exact fixed point.
- Complete Payment ≤12,000 bytes.
- Complete Unload claim within its bound.
- Approximately three-second Pixel completion.
- Proving-memory admission.
- G1/G2 recursive security and concrete bounds.
- Independent cryptographic review.
- Production codec, SDK and device-custody qualification.

Proofs establish local conservation and authenticated history under their assumptions. They do not prevent two valid spends from a forked funded state. Durable selection, hardware-backed signing and trusted wallet execution remain the double-spend boundary.

No implementation of this document may replace those assumptions with a host Boolean, interpret credited evidence as refund authority, or pay an Unload before Core verifies its proof.