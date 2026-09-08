# Conditional round-by-round state for the ideal compact AIR IOP

Reviewed conditional component, 2026-09-06. Separate internal review checked
the state predicates, every-prefix argument and exact arithmetic. This concerns the **ideal typed
interactive protocol** below. It does not map FASTPQ's concrete hash chain,
vector expansion, canonical codec, Merkle layout, or bounded sampler to a
BCS compiler. It does not establish knowledge extraction, zero knowledge,
strong round-by-round soundness, or concrete qROM security.

**Finding.** The proposed verifier-event state can satisfy CMS Definitions
8.3–8.4, including arbitrary partial prover words, with the conditional bound

```
epsilon_RBR(q_initial) <= max {
  4019707974324 / p^4,
  binom(360447,q_initial) / binom(524288,q_initial)
},
p=2^64-2^32+1.
```

At 237 initial positions this is `<2^-128`. The commit-round term is
`<2^-214`; this reduction's largest commit-round majorant begins dominating
its query term at 396 positions. These are ideal interactive state bounds,
not instantiated qROM security bits. In particular, a larger query count
does not reduce that commit-round term.

## CMS requirements actually reviewed

Chiesa, Manohar and Spooner,
[*Succinct Arguments in the Quantum Random Oracle Model*](https://eprint.iacr.org/2019/834.pdf),
January 14, 2020 full version: **§8.1, Definitions 8.3–8.4, and Theorem 8.6,
pp. 38–40**. The relevant state is deterministic, possibly inefficient, and
must have these properties:

1. It is zero on the empty transcript.
2. Extending a zero-state transcript by **any prover message** keeps it zero.
3. A full transcript with state zero is rejected by the verifier.
4. For a false instance and **every** zero-state prefix immediately before a
   verifier move, the next random verifier message changes the state to one
   with probability at most one uniform bound epsilon.

The transcript definition expressly allows prover strings to be partial
functions. It does **not** allow an earlier verifier message to be missing
and later filled in under Definitions 8.3–8.4; that stronger requirement is
introduced in **§8.6**. The lemma below does not cover it. Merely checking
ordinary soundness or a final sum of bad-event probabilities would not supply
the universal prover-move and every-prefix clauses.

## Ideal protocol and message boundaries

Fix the public formal AIR instance, geometry and schema before interaction.
Use the exact degree/field/satisfaction assumptions in
[the formal AIR reduction](fastpq_compact_air_bound.md), including formal AIR falsity. The degree audit
in [the degree ledger](fastpq_compact_air_degree_ledger.md) establishes the stated degree bound for the
reviewed source. The [semantic source audit](fastpq_compact_air_semantics.md)
records the transfer/AXT computation argument and external caller obligations.
Use `a=11/16`, `N=65536`, `L=8N`, and `K=F_(p^4)`.

An oracle message fixes a whole function or partial function on its prescribed
finite domain. It is not a later promise to choose queried values. Merkle
roots and authentication paths are absent from this ideal IOP. Its schedule,
with a singleton initial verifier message to match the CMS convention, is:

| Verifier message | Distribution | Subsequent prover oracle |
| --- | --- | --- |
| `m1` | Singleton empty message | `A:D->F_p^342` |
| `m2` | Uniform `lambda in K^342` | `T:D->K` |
| `m3` | Uniform `alpha in K^923` | `Q:D->K` |
| `m4` | Uniform `(rho,sigma) in K^2` | `f0:D->K` |
| `m_(5+i)`, `0<=i<=16` | Uniform `beta_i in K` | `f_(i+1):D_(i+1)->K` |
| `m22` | Uniform subset `I subset D`, size `q_initial` | None; verifier decides |

There are 21 prover oracle messages and 22 verifier messages, including the
dummy first and final query messages. The actual source samples vector entries
by many transcript hashes; grouping each vector here is an explicit ideal
message boundary, **not** a proof that the concrete expansion matches one
CMS hash output. CMS's `q` denotes total oracle queries, not `q_initial`.
This lemma assigns neither the compiler's total query count nor its hash-work
parameter by conflating those quantities.

The query verifier performs the source's current-row mixture, current/next
quotient and joint-value checks; all binary FRI consistency checks; and the
entire four-entry constant-terminal check. Extra canonical/resource rejection
may be added only when it cannot turn rejection into acceptance.

## Partial prover words

For each partial oracle, deterministically fill every undefined symbol by
zero when constructing the mathematical complete word used in the state.
Keep the actual definedness mask. The IOP verifier rejects if any symbol it
needs is undefined, including **any** of the four terminal values. The state
need not read efficiently: enumerating the complete finite domain and every
candidate polynomial is permitted by Definition 8.3.

All proximity predicates and measures below use this fixed zero completion.
The final acceptance predicate uses the actual partial words and their masks.
Any accepting query execution on the partial words has all required symbols
defined and hence also accepts on their completions. This establishes the
needed containment of passing positions without treating a hole as a freely
chosen answer after the query.

Once appended, a partial oracle in an ordinary transcript remains fixed;
subsequent prover moves append the next oracle and do not fill earlier holes.
The CMS compressed-database extractor can produce different partial words as
its database changes. This is not an additional prover move in Definition
8.3: changes of extracted words are separately controlled in CMS Lemma 8.8.
No corresponding extraction lemma for FASTPQ's actual commitment/hash format
is claimed here.

Prover symbols belong to the prescribed canonical finite alphabets. One may
extend the ideal verifier to malformed symbols by rejecting when such a
symbol is queried and replacing it by zero for the state analysis, just as
for an undefined symbol. Do not send a transcript to an automatic global
reject sink merely because an **unqueried** cell is malformed unless the
defined verifier itself necessarily rejects it. Public schema/geometry errors
and genuinely malformed message schedules may be assigned a permanent reject
sink because the verifier rejects every completion of those prefixes.

## Definition of the state

On a well-formed transcript, replay the verifier messages that have appeared
and maintain a sticky bit `bad`, initially zero. Prover messages only append
fixed partial oracles. They do not themselves change the bit. At each
verifier message below, set `bad=1` if its indicated predicate is true; once
one, leave it one. The state is the resulting bit.

**After lambda.** Let `P` be the unique degree-`<N` polynomial row vector
having common agreement at least `aL` with completed `A`, if such a vector
exists. It is defined solely from A and therefore before lambda and alpha.
Let `R_lambda=sum lambda_j*A_j`. Trigger when either:

- P does not exist and R_lambda agrees with a degree-`<N` polynomial on at
  least `aL` points; or
- P exists and some x has `A(x)!=P(x)` but
  `R_lambda(x)=sum lambda_j*P_j(x)`.

**After alpha.** If P exists and violates the formal AIR, choose the first
violating subgroup point in the fixed subgroup order. Trigger when its
nonzero numerator vector has dot product zero with alpha. If no such P or
point exists, this predicate is false.

**After the joint pair.** Using already fixed A, lambda, T, alpha, Q, define

```
mu(x) = 1/L times the indicator that
  T(x)=R_lambda(x), and
  (x^N-1)*Q(x)=sum alpha_k*C_k(x,A(x),A(omega*x)).
J(x)=Q(x)+rho*T(x)+sigma*x^N*T(x).
```

Trigger iff some polynomial of degree `<2N` agrees with J on mu-weight at
least a. This test occurs **before f0 is supplied**. It is a direct weighted
proximity predicate, rather than a flag set after observing whether a later
f0 makes the initial FRI potential large.

**After beta_i.** For the completed f0 and later words already present,
initialize and update prefix-survival measures by

```
mu_0(x) = mu(x)*1[f0(x)=J(x)],
nu_i(y) = mu_i(x)+mu_i(-x),                      y=x^2,
mu_(i+1)(y)=nu_i(y)*1[f_(i+1)(y)=e_i(y)+beta_i*o_i(y)].
```

Here e_i,o_i are the even/odd words of the already committed f_i. Trigger
iff some next-code polynomial, degree `<d_(i+1)`, agrees with
`e_i+beta_i*o_i` on nu_i-weight at least a. Again this is tested **before
f_(i+1) is supplied**, so a choice of that oracle cannot create a new flag.
At the last fold the next code is the constant code on four points; the same
predicate is used, with the elementary six-exception proof.

**After the final query subset.** Trigger iff the actual partial-oracle
verifier accepts, including all definedness and full-terminal checks. There
is no subsequent prover message in this schedule.

For syntactically invalid prefixes or public inputs on which the ideal
verifier always rejects, define state zero permanently. Such a sink cannot
be repaired by appending a message. This convention is confined to provably
rejecting prefixes and is not used to disguise an algebraic invariant failure.

## Every zero-state prefix has the required invariant

The assertions here hold for **all** well-formed false-instance prefixes
whose recomputed state is zero, irrespective of whether their previous
random messages looked probable or their prover followed the honest algorithm.

- After A, no invariant beyond a fixed completed row oracle is needed.
- After a good lambda, a close row mixture forces the fixed candidate P to
  exist, and mixture equality with its polynomial mixture implies equality
  of the entire row at every point. A subsequent arbitrary T changes neither
  assertion about A and lambda.
- After a good alpha, if P exists, every degree-`<2N` q has a nonzero
  residual of degree at most `3N-1`. A subsequent arbitrary Q cannot cancel
  that residual at the already chosen violating subgroup point.
- Before the joint pair, the AIR reduction proves that Q,T,X^N*T have no
  common degree-`<2N` polynomial triple on mu-weight at least a. If one
  existed, it would force t degree `<N`, identify P on the common set S,
  and make the nonzero residual vanish on
  `S intersection omega^(-1)S`, at least `3N` points. This is a deterministic
  contradiction on the previous good prefix.
- After a good joint pair, every choice of f0 has initial weighted code
  agreement below a, because agreement under mu_0 also gives agreement
  with J under mu.
- Before beta_i, f_i has weighted code agreement below a under mu_i, whose
  atoms are at most `1/|D_i|`. If the beta predicate is false, every choice
  of f_(i+1) keeps the next weighted potential below a: its agreement on
  surviving edges is agreement of the genuine folded word under nu_i.
- After f17, either its completion is nonconstant, causing full-terminal
  rejection, or its weighted potential equals total surviving initial mass,
  which is below a. Actual undefined values can only reject further.

In particular, an allegedly good false-instance prefix in which appending f0
or a later oracle produces potential at least a is impossible by the preceding
implications. The definition must **not** be patched to set state one after
such a prover move; that would violate CMS's universal prover-move clause.

## Per-verifier-message bounds

All bounds are conditional on an arbitrary zero-state prefix immediately
before the corresponding verifier message. Let `B_row=35260596266/3`,
`B_joint=4409294859`, and `C_F=134561/16`, using the conservative theorem
constants from the AIR reduction.

| Message | Upper bound for a zero-to-one state change |
| --- | --- |
| Initial singleton | 0 |
| Lambda | `max(342*B_row, L)/|K| = 4019707974324/|K|` |
| Alpha | `1/|K|` |
| Joint pair | `2*B_joint/|K|` |
| Beta_i, i=0..15 | `(C_F*|D_(i+1)|+11)/|K|` |
| Beta_16 | `6/|K|` |
| Final query subset | `binom(360447,q_initial)/binom(L,q_initial)` |

For lambda the two branches are exclusive **before** lambda is drawn:
candidate existence is fixed by A. If absent, the independent-vector
correlated-agreement lemma applies; if present, the all-position cancellation
bound applies. Hence a maximum suffices at this move. Alpha uses the fixed
violating-point argument. The joint pair uses independent-vector correlated
agreement on the already fixed mu and absence of a common triple. Beta bounds
use the weighted even/odd theorem and the maintained potential; the terminal
uses at most six distinct slope-intersection challenges.

For the final message, the completed-oracle passing set has size at most
`ceil(aL)-1=360447`; acceptance on actual partial words is contained in this
event. Uniform without-replacement queries give the displayed exact ratio.
No independence of descendant groups is assumed.

Taking the maximum over this table proves the announced epsilon_RBR. An
ordinary whole-interaction union bound could instead add stage errors, as
in the previous AIR draft; CMS Definition 8.4 requires a maximum conditional
one-step bound, so the sum is unnecessary for this state parameter.

## Checking Definitions 8.3 and 8.4

The empty transcript has bad=0. On any prover move, all already evaluated
predicates use only previous messages, so appending an arbitrary next oracle
preserves a zero bit. The next verifier event—not the prover move—can set
it. At a full transcript, the final event explicitly sets the bit if the
actual verifier accepts, so state zero implies rejection. These clauses hold
for true instances as well; for example, a true instance may set the joint
predicate with high probability, which Definition 8.4 does not restrict.

For false instances, the every-prefix invariants establish the conditional
table above. Reject sinks have zero future flip probability. Thus this
particular ideal IOP has a state meeting Definitions 8.3–8.4, conditional on
the imported proximity theorem and the reviewed AIR reduction. The state is
not claimed efficient, and the argument supplies no polynomial-time witness
extractor required by Definition 8.5.

## Remaining compiler boundary

CMS Theorem 8.6 applies to its specified BCS construction and random-oracle
model. The ideal state lemma does not show that FASTPQ's append/challenge
hashes implement that construction. Still needed are: mapping each grouped
field/vector/subset message to the actual multi-call transcript expansion;
the four-lane challenge projection versus six-lane transcript state; bounded
rejection/dedup and abort behavior; domains, typed roots, pair leaves and
shared multiproofs; canonical encodings; oracle-query/work accounting;
concurrent/adaptive statements and attempts; and concrete hash assumptions.

In particular, setting `lambda=384` merely from digest byte width or setting
the compiler's `t` to the initial query count would be unjustified. Supporting
the original BCS hash chain could require §8.6's stronger hole-filling state
property; this state has not been shown to satisfy it. No compiler theorem
or production-security target is instantiated by this document.

### Concrete multi-call expansion obstruction

The grouped ideal lambda and alpha messages have approximately 87,552 and
236,288 bits of entropy, respectively. A uniform 136-subset of this domain
has entropy strictly between 1,811 and 1,812 bits; a 237-subset has entropy
strictly between 2,969 and 2,970 bits. One `F_p^6` digest has less than 384
bits. Thus these grouped messages cannot simply be identified with one of
the existing hash outputs. The actual implementation derives coefficients
and query candidates through many sequential hash calls, updating the state
each time. Fresh ideal oracle calls can supply further randomness, but the
compiler must account for that expansion and adversarial access to its
intermediate inputs; it is not the one-hash-output mapping of §8.2 by assertion.

Splitting the query sampler's internal digests into many CMS verifier rounds
also invalidates this state's tiny final-query bound. At an allowed prefix
where all earlier selected positions lie in a passing set, conditioning on
that prefix leaves only the final few new positions random. The chance of
acceptance can then be of the order of `a` (one remaining point) or `a^6`
(six remaining points), rather than the whole `a^q_initial` probability.
Our state postpones its acceptance flag until the **one grouped fresh subset
message**; it does not establish a comparable every-prefix bound for those
finer round boundaries.

A compiler-compatible challenge expansion or a new reduction for the exact
multi-call transcript is therefore a concrete remaining prerequisite. This
obstruction does not prescribe increasing queries or changing a frozen profile.
The currently recorded release accounting uses 54 targets and `t=2^32`;
neither figure has been folded into this ideal state bound, and this document
does not substitute a different adversarial work parameter.
