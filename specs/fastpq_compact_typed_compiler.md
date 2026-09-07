# Typed whole-tape compiler: conditional extractor and quantum bound

2026-09-06. **Internally reviewed conditional derivation; no production qualification.**
This is an ideal-model proof for the precise candidate compiler below. It does
not apply CMS Theorem 8.6 to current FASTPQ source by assertion. The H and G
functions are independent uniform ideal functions; they are not identified with
the frozen six-lane Poseidon construction or concrete SHAKE. No source, profile,
build, or parameter was changed.

The proof architecture follows Chiesa, Manohar and Spooner,
[*Succinct Arguments in the Quantum Random Oracle Model*](https://eprint.iacr.org/2019/834),
January 14, 2020, §§3.6, 4.2, 5 and 8.5. The typed extractor, finite-group norm
calculation, and unequal-alphabet acceptance comparison below are independent
derivations. **Their constants are not quoted as a theorem of CMS.** Retained
primary PDF SHA-256:
`c3258e2faa339bdc441403d73aba9fee7d03687121c3369ef007ccce71cd2b41`.

## 1. Exact claim and scope

Fix one false formal IOP instance and immutable canonical context c. Assume a
state satisfying ordinary CMS round-by-round soundness for all partial prover
words, with per-verifier-move errors epsilon_j. The imported conditional AIR
state is in [the round-by-round specification](fastpq_compact_round_by_round.md); its SHA-256 when read was
`93fee9f7b9b018a029f670d6c081bf52ae84de83d7d8b21ff425d02364205bbd`.
The AIR/proximity hypotheses remain assumptions here.

There are k prover words and k+1 whole verifier messages. All word lengths,
symbol alphabets, Merkle heights, tape lengths R_j, input size bounds, round
tags, and decoders are fixed before the first query. Set

```
C = (Z/pZ)^6,       s_H = p^6
B_j = (Z/2Z)^R_j,   s_j = 2^R_j.
```

Give the adversary independent ideal functions H:X_H -> C and G_j:X_j -> B_j.
H has disjoint typed leaf, binary-parent and chain input namespaces, but is one
common function: H collisions across all roles count. Every G_j is a separate
round type, even when tape lengths match. Its valid input is (c,sigma_(j-1)).
Encodings are injective, canonical and length bounded. Invalid inputs within a
declared oracle type may still receive uniform responses, but contain no
parsed pointers and cannot be used by the verifier. Invalid tags outside all
declared types get a public fixed response, requiring no oracle query.

The candidate compiler is:

```
sigma_0 = fixed canonical anchor
tau_j = G_j(c, sigma_(j-1))
m_j = Decode_j(tau_j)                     // one whole message, or ABORT
rt_j = typed Merkle root of prover Pi_j
sigma_j = H(chain, c, j, tau_j, rt_j)     // j <= k
```

The final tape supplies the entire final query message, with no later root.
Every chain input binds the **full raw tape**, including rejected candidates
and unused suffix. Replacing that tape by its digest adds an extraction edge
and is a different construction.

The dummy initial IOP message also needs a defined representation. One option
is a positive-length uniform tau_1 whose decoder always emits the empty
message. **R_1=0 does not have a negligible collision denominator**: every
G_1 output would coincide. Removing this oracle step instead requires another
anchor/extractor definition. This document does not change the recorded
21-prover-word/22-verifier-message schedule.

Assume each successful decoder has exactly the desired whole-message
distribution, and abort is permanent rejection. For any fixed good prefix,
the fraction of raw tapes causing a bad-state transition is then at most
epsilon_j, unconditionally. Honest finite-tape abort affects completeness, not
soundness. All queries across retries count.

Let T bound group-oracle queries by the adversary **plus verifier expansion**,
and n=T-1. Define

```
delta(T) = min(1, max {
  3*n/p^6,
  max_j [epsilon_j + n/2^R_j]
}).
```

For a successful verifier expansion w, list its distinct required oracle
entries and define

```
weight(w) = (# H entries)/p^6 + sum_j (# G_j entries)/2^R_j
K_weight = max_w weight(w).
```

The candidate conditional ideal soundness bound proved below is

```
Pr[accept false instance]
 <= min(1, (sqrt(6*T^2*delta(T)) + sqrt(2*K_weight))^2).
```

For binary XOR access to canonical encodings, each query can be simulated with
at most two group-oracle queries. Thus T=2*(t_adversary+v_expansion) is a valid
conservative choice under the model in §7. The output tape width never replaces
the shorter p^6 denominator.

## 2. Typed collisions and pointer counts

A database D consists of partial functions D_H and D_j, with total support size
|D|. Let B mean either equal H outputs at distinct H inputs, or equal D_j
outputs at distinct inputs for some one j. Tapes of different G types are not
compared, nor are tapes compared with short roots. B is insertion-monotone.

S_H(D) contains canonical short digest fields in parsed inputs: the two
children of an H parent, the root of an H chain, and the predecessor state of
a G input. Leaf bytes are data. Long tape bytes are not scanned as short-root
substrings. S_j(D) contains complete R_j-bit tape fields in valid round-j
H chain inputs. With n_H=|D_H| and n_G=sum_j|D_j|,

```
|S_H(D)| <= 2*n_H+n_G <= 2*|D|
|im(D_H)| <= n_H
|S_j(D)| + |im(D_j)| <= #H_chain_j + |D_j| <= |D|.
```

Even outputs of invalid-but-queryable inputs count in the appropriate image
collision set. Typed pointer counts are independent of tape length.

## 3. Anchored extractor and stability

Outside B, every output has at most one inverse in its own oracle type.
Tree(D,c,j,rt) recursively inverts H using the declared height, node position,
role and context. Missing, wrongly typed or malformed nodes supply undefined
symbols beneath that node. Valid leaves supply canonical symbols. This defines
a partial word of the fixed length. It does not postpone choosing any defined
symbol. Use fixed zero completion for the mathematical state and retain the
mask: the actual IOP verifier rejects any required undefined read.

Define E(D,c,r,sigma), returning r complete IOP rounds or FAIL:

1. If r=0, return the empty transcript iff sigma is the fixed anchor; otherwise
   FAIL. Never invert H at the anchor in this base case.
2. If r>0, invert D_H at sigma and require a valid chain input
   (chain,c,r,tau_r,rt_r); otherwise FAIL.
3. Invert D_r at the **full** tau_r and require input (c,sigma_prev). Decode
   tau_r, rejecting abort or malformed data with FAIL.
4. Obtain E(D,c,r-1,sigma_prev); propagate FAIL.
5. Append decoded m_r and Tree(D,c,r,rt_r).

The round decreases strictly. Short-value cycles cannot loop. A hash output
equal to the anchor needs no special exception: the r=0 case stops at the
anchor, and r>0 still checks its typed round. FAIL is not a transcript with
holes in previous verifier messages. This proof does not assume strong
round-by-round soundness.

Consider insertion at an undefined input, with old and new D outside B.

* H insertion with output y: a change to E(D,c,r,sigma) implies
  y in S_H(D) union {sigma}.
* G_j insertion with output tau: a change to that extraction implies
  tau in S_j(D).

Proof: follow the old/new backward traversals to their first differing lookup.
An H inverse can change only at the new output. Its target is the external
endpoint, or an existing chain's root, an existing G input's predecessor
state, or an existing parent's child. A G_j inverse can change only at the
full tape already present in an H chain input. A G insertion cannot itself
add that H input. If chain traversal is unchanged but a partial Merkle word
changes, apply the same first-difference argument to that tree traversal.
Wrong-type/context checks and FAIL-to-anchored changes are covered: old
extraction success is not required.

## 4. Bad transitions and two-sided insertion instability

Let P be the collision-free databases with an endpoint (j,sigma) such that

* tr=E(D,c,j-1,sigma) is anchored and immediately before verifier move j;
* state(tr)=0;
* D_j(c,sigma)=tau exists, decodes successfully, and
  state(tr || Decode_j(tau))=1.

The endpoint is a **G input already present in D**, which will remove an extra
endpoint +1 from the short-output exceptional set. Put R=B union P. Empty D
is outside R.

An accepting verifier expansion present in D implies D in R. If D has a
collision this is immediate. Otherwise all chain edges and actually read
authenticated symbols extract consistently with the expansion. Other extracted
symbols cannot change that verifier execution. The full extracted transcript
accepts, hence has state one. Its initial state is zero, and no prover move
can change zero to one. Its first such verifier move supplies the endpoint
witness for P. This includes the final whole-subset message.

Fix D of size n and an undefined input x. Bound both directions of R membership
after a fresh uniform output.

**H query.** New collisions require y in im(D_H). Outside collisions, creating
a P witness or destroying an existing one requires changed extraction: H
insertion cannot create, remove or change a G entry. The stability lemma gives
S_H(D) or that witness endpoint. The endpoint is already in a G input of D,
hence itself belongs to S_H(D). Either flip probability is at most

```
(|S_H(D)|+|im(D_H)|)/p^6 <= 3*n/p^6.
```

If D already has a collision, R cannot disappear. No extra guessed endpoint
is introduced.

**G_j query.** New collisions require tau in im(D_j). Changing P witnesses
whose G entries were already in D requires a changed extraction, hence
tau in S_j(D). The remaining way to create P is the inserted query itself
being a new endpoint witness. Its preceding extraction E(D,c,j-1,sigma)
is fixed independently of the new tape: it uses only G rounds below j. If
that prefix fails or is already bad, no such event occurs. Otherwise, the
whole-message state gives at most epsilon_j*2^R_j bad raw tapes. Thus either
direction has probability at most

```
epsilon_j+(|S_j(D)|+|im(D_j)|)/2^R_j <= epsilon_j+n/2^R_j.
```

Malformed inputs or another context cannot create the new proper endpoint,
but can still hit counted pointers/images. Destroying an old witness needs
no epsilon term; the same upper bound remains valid. Collisions persist.

The maximum over types and n<=T-1 proves two-sided insertion instability
at most delta(T) for R directly. Separately adding a global collision maximum
would be safe but looser. A deletion is the reverse edge from the smaller
database and uses these same two directions. A quantum replacement is **not**
treated as a uniformly random erasure; its entire block is bounded in §5.

## 5. Finite-abelian compressed oracle and lifting

This section derives the operator bound for unequal finite output alphabets.
No common binary output alphabet is introduced. Finite input domains follow
from the fixed input caps.

For input x with output group Y of size s, add an orthogonal |bot> to its
table-cell space. Let |u>=sum_y|y>/sqrt(s). Define the involutive unitary J
by swapping |bot> and |u> and fixing their orthogonal complement:

```
J=I-|bot><bot|-|u><u|+|bot><u|+|u><bot|.
```

Purify the ideal random table as independent uniform superpositions of every
cell. Conjugation by J on all table cells turns that initial state into all
bot. In the Fourier basis of a group response register, an oracle query acts
by a character chi(y); extend its phase as 1 on bot. The compressed query at x
is U=J diag(1,chi(y)) J. This conjugation proves exact simulation after tracing
out the database. Input/type/character control registers give orthogonal
blocks, including queries in superposition across types.

For trivial chi, U=I. For nontrivial chi, sum_y chi(y)=0 and |chi(y)|=1.
Direct multiplication gives

```
U|bot> = sum_y chi(y)/sqrt(s) |y>
U|w> = chi(w)|w> + chi(w)/sqrt(s)|bot>
       + sum_y [1-chi(w)-chi(y)]/s |y>.
```

Characters may be complex. With beta(w,y)=1-chi(w)-chi(y),

```
sum_y |beta(w,y)|^2 = s*(3-2*Re chi(w)) <= 5*s
sum_w |beta(w,y)|^2 = s*(3-2*Re chi(y)) <= 5*s.
```

Fix x, character and outside database E. The query preserves E, giving a
one-cell block. Let b be R membership of E, and A the outputs y for which
E+[x->y] has membership different from b. Two-sided instability delta gives
|A|/s<=delta. Cross-property matrix entries have no diagonal chi(w) term.

If b=0, the block from outside R to inside R has target rows A and source
columns bot and Y\A. Its squared Frobenius norm is at most

```
|A|/s + sum_(y in A) sum_(w notin A) |beta(w,y)|^2/s^2
 <= |A|/s+5*|A|/s <= 6*delta.
```

If b=1, source columns are A and target rows bot and Y\A. The column-sum
identity gives the same bound. Operator norm is at most Frobenius norm.
Taking a direct sum over x, character and E gives cross-property norm at
most sqrt(6*delta), with no sum over the number of oracle types.

Use the full tensor-table construction rather than an artificial rule at a
full compressed database. One query changes at most one non-bot cell. After
i queries, support size is at most i. Before query i<=T, the outside database
E has at most T-1 entries, precisely the range used by delta(T). Restricting
to reachable sizes cannot increase the above matrix-block bound.

Adversary operations do not touch database registers and commute with the
R projector. Beginning outside R, insert that projector after each query and
apply the triangle inequality to the first transition into R. At most T
cross-property operator norms contribute. Therefore

```
Pr[measured compressed D in R] <= min(1,6*T^2*delta(T)).
```

This is a proof for the independent ideal group oracles, not a computational
hash instantiation.

## 6. Oracle acceptance versus a database witness

After the adversary, run the deterministic verifier expansion and enumerate
all ideal-oracle constraints on which acceptance depended. Every expansion
query counts in T. Reject malformed expansions. Deduplicate repeated identical
constraints; conflicting outputs at one typed input never win. The list
contains full G tapes, all chain edges and all verified Merkle nodes/leaves,
including terminal reads. A concrete shared multiproof needs a separate proof
that its parser reconstructs precisely these typed entries.

For a classical candidate expansion w requiring distinct F(x_i)=y_i, let
s_i be the corresponding output cardinalities. Q_w projects its selected
table cells onto all specified values, tensored with identity elsewhere.
Expansion validity is also checked in the classical output register. On these
selected cells let J_w=product_i J_(x_i). Other decompressions commute with
Q_w and disappear from norms.

On the selected factor Q_w has rank one. Since
<y_i|J|y_i>=1-1/s_i, put a=product_i(1-1/s_i). Unitarity gives

```
Q_w J_w Q_w = a Q_w
||Q_w J_w (I-Q_w)||^2 = 1-a^2
 <= sum_i [1-(1-1/s_i)^2] <= 2*sum_i 1/s_i.
```

The middle expression is the squared norm of the off-diagonal part of one
normalized row. The product bound holds for all factors in [0,1]. Hence

```
||Q_w J_w psi|| <= ||Q_w psi|| + sqrt(2*weight(w))*||psi||.
```

Different classical output w values give orthogonal blocks. Taking their
maximum weight and applying the triangle inequality to these blocks yields

```
sqrt(Pr[win ideal oracle game])
 <= sqrt(Pr[output expansion present in measured D])+sqrt(2*K_weight)
 <= sqrt(Pr[D in R])+sqrt(2*K_weight).
```

The acceptance-to-database inclusion is §4. This proves §1's bound.
The conservative factor two is retained deliberately; this argument does
not cite an unproved unequal-alphabet adaptation of the sharper binary lemma.

## 7. Binary encoding and exact query overhead

A group addition query maps |type,x,z> to |type,x,z+F_type(x)>, with z in its
typed group. Its Fourier transform is the character-phase query of §5.
The query model permits arbitrary intermediate unitaries; no efficient exact
Goldilocks Fourier-transform circuit is asserted.

Canonical binary XOR access instead maps

```
|type,x,b> -> |type,x,b XOR enc_type(F_type(x))>.
```

Use a zero group-register ancilla. One group query computes F into it;
reversible encoding XORs enc(F) into b; one inverse group query clears it.
An inverse costs one forward group query by conjugating the response with
group negation. The temporary register can be a direct sum of typed groups,
so this works coherently across oracle types and extends unitarily outside
valid binary encodings. Two group queries per binary query suffice. Binary
G types could use one query directly, but the uniform factor two safely
handles superpositions of H and G types.

This exactly simulates canonical finite outputs. It does not approximate them
by reducing a uniform 384-bit output modulo p. Thus C remains p^6. Encoding
gates and the cost of actual oracle absorption/squeezing/evaluation are
separate: one long XOF query is not one Keccak permutation or one fixed unit
of coherent implementation work.

## 8. Remaining qualification obligations

The stated ideal construction now has a candidate anchored extractor,
explicit two-sided exceptional sets, finite-abelian lifting and acceptance
comparison with constants. The earlier suggested 3*n/p^6 and
epsilon_j+n/2^R_j survive this rederivation, subject to the precise definitions
above. The derivation received separate internal review; external cryptographic qualification remains required. Small-alphabet numerical controls
can find mistakes but cannot replace the proof.

Before applying it to FASTPQ:

* Freeze exact transcript source, canonical input codec, full-tape binding,
  typed domains, R_j table and initial dummy treatment. Current multi-call
  challenge expansion is different.
* Prove actual row/FRI pair-leaf commitments and shared multiproofs correspond
  to the typed Tree and verifier expansion, with public-instance binding.
* Prove exact decoders' uniformity and honest abort bounds; account all retries.
* Independently review this derivation and imported AIR/semantic assumptions.
* Qualify concrete H and G jointly. No Poseidon/SHAKE indifferentiability,
  collision cost or concrete post-quantum security level follows here.
* Fix actual query and coherent-work accounting, verifier expansion counts,
  adaptive-attempt rules and the existing 54-target release accounting.
  t=2^32 is not the proof's initial query count, or automatically T here.
* Preserve authenticated caller/source-state/finality obligations. Formal
  IOP soundness alone does not authenticate proof-carried balances, policies
  or remote-spend freshness.

A conditional interactive error below 2^-128 at 237 initial positions does
**not** imply compiled error below 2^-128: the 6*T^2 factor still applies.
Neither 136 nor 237 is approved as a production profile by this document.


## Reproduction and reviewed extensions

Run `python3 scripts/fastpq/check_compact_typed_compiler.py` and
`python3 scripts/fastpq/check_compact_typed_extractor.py` from the repository root.
Each accepts `--output PATH`; output defaults to the local FASTPQ validation
directory under `target/`. No retained PDF or proof is required. If the retained
CMS PDF is present at the standard local evidence path, its documented SHA-256
is verified. The finite controls test identities and bounded examples; they do
not prove this theorem or qualify its concrete instantiation.

[Adaptive contexts](fastpq_compact_adaptive_context.md) require one global
endpoint property and their explicit family-wide premises.
[Projected raw-XOF outputs](fastpq_compact_projected_xof.md) additionally cover
bounded rejection, aborts, public raw suffixes and auxiliary raw-input domains.
[Profile arithmetic](fastpq_compact_typed_profile.md) retains the separate
54-target accounting and labels each oracle-query model explicitly.
