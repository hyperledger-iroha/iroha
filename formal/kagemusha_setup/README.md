# Source-bound setup and fixed-Poseidon algebra controls

This standard-library Python package exercises an exact-source Pasta setup
simulation in an **ideal raw-hash model**, followed by fourteen small complete PIPA
proofs checked with the unchanged reference verifier and fixed RP57 transcript.
It is verification material, not a production prover or parameter loader.

From the repository root, choose a new output directory:

```sh
python3.12 -B -S -m formal.kagemusha_setup.check --output /absolute/new/output
```

The owner checks exact source pins before importing the subjects, runs the
maintained prime/group certificates, then seventy-seven controls. It refuses optimized
Python and existing output directories. It retains setup bytes, raw-model
tables, synthetic private logs, proofs and failures. Nothing in this package
imports or reads a development output directory. All dependencies are relative
to this repository; no Cargo build, large parameter artifact or native prover
execution is involved.

The controls retain the five exact finite-map/law/cap cases, five single-map/KAT cases, eight raw-query cases, six
fresh-target/parameter cases and ten complete-proof cases, plus two source
custody cases, thirteen bounded-producer parity/resource/custody cases and
eleven sequential request/entropy/replay cases and seventeen generic public-table/rebinding cases. One complete-proof method checks exact retained diagnostic
proof/parameter hashes after the arithmetic-helper relocation. The only retired test checks the replaced fixed-target sampler;
its failure, zero-target and cap guarantees are covered by the fresh-target
controls. There is one maintained pair sampler, which draws a fresh private
target on every ordinary inverse miss. No compatibility sampler is retained.

## Exact setup boundary

`preimage.py` uses the native SWU/isogeny constants, canonical parity, exceptional
zero branch and complete ordered inverse of size at most nine. It reuses
`formal.kagemusha_pasta.auxiliary` for constant parsing and finite auxiliary
group arithmetic. The maintained prime/group certificates bind the bases and
isogenies; the geometric regularity argument is separate and is not needed for
this fresh-target sampler.

The sole raw oracle reserves native XMD's three entries atomically on the first
designated raw m0 query, including early queries. It never overwrites an input.
Caps, occupied entries and native identity points are retained failures, never
repaired or resampled. The sampler has J4096/h256 budgets; ordinary test cases
may deliberately select smaller caps to exercise failures.

Each k6 family uses 64 g messages plus shared W/U. Across both curves there are
132 contexts and one unrelated prior raw query. Prefixes, W/U and repeated
queries reuse exact originals. Lagrange logs use one n-inverse normalization;
the native encoding is 64n+68 bytes. Private logs use the source base
isogeny(SWU(1)), not an independently chosen unrelated base.

For every ordered pair of field inputs, one target scalar and inverse slot
return it, so successful pairs are exactly uniform without a regularity loss.
In the stated ideal-coin/raw-oracle experiment, the reviewed finite-pair, lift
and reduction terms are less than 12292·2^-256 per distinct setup context.
Both complete k≤16 families have 131076 contexts, giving local aggregate less
than 2^-224, plus the raw-query collision bound
`2*Q_s*(Q+3*Q_s)/2^512`. This is a sampler bound, not a protocol security-level
claim. The executable controls only construct k≤6 families; larger producer requests require an explicit resource opt-in.

## Verifier authority and proof scope

The family is constructed **before** writing its private authority, descriptor
and key. For every maintained small control, all seven Python reference verifier modules
are copied verbatim into a fresh namespace. Only that namespace's params_ipa authority data differs;
RP57 constants and all verifier checks, including the actual folded-generator
decision, remain unchanged. Repository KATS and parameter authorities are never
modified. Current-authority refusal is an explicit control.

Two families are reused across ordinary and contradictory relation examples.
The simulator samples points in native order, squeezes fixed RP57, handles the
exact coefficient-rank collapse branch and solves the final IPA blind from
private known logs. No transcript answer is programmed. Tests check every
derived commitment log, complete proof verification, f/c/G/instance mutations
and the declared contradictory relation. Acceptance of those contradictory
toy cases is confined to the simulator's newly constructed test authority.

The four relation-comparison proofs explicitly supply the same diagnostic
PRNG seed within each curve for the two relation examples. This is a common-coin, counterfactual algebra diagnostic. The other four proofs exercise sequential requests with an explicit
shared stream and an input chosen from the preceding public proof. Neither
experiment establishes an adaptive privacy theorem. Setup
coins use a separate deterministic seed. Python `random.Random` is test
reproducibility machinery; neither it nor the retained synthetic private logs
are part of a production wallet interface. The unchanged proof sampler has its
own 128-draw cap; the setup's h256 bound does not silently cover other draws.

These tests do not recover logs of current release parameters, instantiate an
ideal oracle with concrete BLAKE2b, prove the full adaptive outer-proof
distribution, satisfy all private fold/wallet relations, or close C12. See the
implementation-coupled [soundness memo](../../specs/kagemusha_recursion_soundness_v1.md)
for the separate remaining obligations. Test reports retain
`current_setup_qualified=false` and `C12_closed=false` even when all cases pass.


## Bounded parameter producer

`bounded.py` reuses the same sampler, raw oracle, and source-map/log check. One
owner shares contexts across curves and domain sizes, preserves early raw
queries and exact replay, and permanently stops after any cap, identity or
consistency failure. All g calls precede their first-identity check; each k has
its own inverse-omega scalar FFT with one normalization, followed by W/U and
encoding-order identity checks. It never truncates a larger Lagrange vector to
construct a smaller domain. The small quadratic helper remains an independent
bounded k≤6 control oracle; the scalable owner uses a radix-two transform.

The standalone producer is a diagnostic command with small defaults:

```sh
python3.12 -B -S -m formal.kagemusha_setup.produce_parameters --output /absolute/new/output
```

Defaults select both k6 curves,132 contexts,1024 raw calls,412 retained raw
entries,8 requests,1MiB parameter wire output and32MiB allocation allowance.
Every count has a finite absolute bound. k>6 additionally requires
`--allow-large`; the maximum is k16 and131076 distinct contexts across both
curves. Scheduling of other qualification or build work is not inferred
from this flag. Larger execution must be deliberately scheduled by its owner.

Memory admission distinguishes a conservative container reservation from
checkpointed `tracemalloc` peak allocations. The latter runs after each raw
call, point, FFT stage and final retention/hash pass. A bounded operation may
overshoot before its next checkpoint; this is not an instantaneous OS/RSS cap
or a performance qualification. Full two-curve k16 would require explicit
context/query/entry/output budgets and a larger memory option; no full-size
family is generated by the maintained checks or CI.

A fresh mode0700 directory retains exact parameter bytes, private logs, the
raw-oracle table, sampler attempts and failures. JSONL is streamed. Final
filename/size/SHA custody must match each generated wire record and the exact
selected artifact namespace before success. No crash-durability or resume
claim is made. Parameter identity records also derive the all-ones folded
G=sum(g_i), its private log and finite flag for that k. This is a candidate
mathematical ACC_TRIV value for a future private setup experiment, not a new
production literal or current verifier authority. An identity is retained,
never repaired; downstream relations still require finite admitted points.

The thirteen added controls compare k0/k2/k6 byte-for-byte with the unchanged
small family; verify shared context replay and direct scalar transforms;
refuse unsafe budgets, sampler exhaustion and native identity order; and
exercise fresh/existing output, CLI module prelaunch/source/optimized refusals,
allocation failure and mutated final artifact joins. CLI refusal tests use
`-B -S -m` and an isolated temporary repository copy for a missing-source case.
Only small parameters are constructed; they do not add a full-size proof or a
C12 claim.

## Sequential simulator requests

`simulate` requires an explicit bit source and never creates or resets a PRNG.
`requests.Owner` shares one entropy source across a finite sequential request
history. Its default uses `secrets.randbits`; tests explicitly supply a seeded
stream once per owner. Each invocation counts before calling the source, even
on error. Invalid bits, provider errors, field rejection exhaustion and either
draw cap retain a failed canonical request without retry or entropy fallback.
The toy still uses at most36 scalar samples and4608 source calls. The generic
engine derives `sample_budget(case)` from the descriptor: fresh point samples,
random advice/product/lookup evaluations, only the masked opening groups and
at most one final c. Every sample corresponds to a serialized32-byte message;
for an admitted proof≤10,000bytes there are at most312 samples, each allowing
128 rejection draws, hence at most39,936 source calls. Defaults of40,960 per
request and327,680 total cover eight requests at that conservative bound.
The exact historical Omega inventory has58 point samples,37 evaluation samples
and one possible c:96 samples or12,288 source calls. This is simulator scalar
sampling, not the native prover's131,218 polynomial/scalar draws. Smaller
explicit caps deliberately expose terminal failures; they do not change the
native distribution or authorize a retry. The descriptor bound is retained
in successful simulation metadata and every admitted request observation.

Exact descriptor/key/parameter/public-preprocessing bytes and canonical instance tuples bind each
request identity. A retry returns the same frozen outcome and consumes no new
entropy, including at capacity. New canonical requests use the next stream
portion. Syntax, changed-binding, capacity and reentrancy refusals publicly raise
`ValueError`; canonical attempts return only immutable proof bytes or failure.
Failure reasons, scalar logs and draw counts remain private diagnostic data.
All fallible proof validation and hash metadata precede success publication;
a caught failure or interrupt restores the failed outcome. Interrupts propagate.

Unused-tail independence holds for ideal independent bits at bounded stopping
points. It does not identify an OS source or seeded PRNG with that ideal, hide
observable failures/timing, or justify a low-quota/native draw-law substitution.
Shared mutable decoded case objects are trusted inputs, not Python capabilities.
This owner is neither thread-safe nor crash-durable wallet storage. Its synthetic
setup does not authorize foreign production parameters or resolve C12.

The eleven controls cover four complete proofs on both curves, golden first
proofs, adaptive second inputs, exact retry, altered bindings, invalid/provider/
exhausted entropy, interruptions, immutable outcomes, capacity, reentrancy,
storage failure and late-metadata failure. Two injected byte fixtures exercise
publication failures only; they are never counted as proofs.


## One generic outer algorithm and historical public originals

`simulator.py` is the sole fixed-RP57 proof algorithm. Existing ordinary and
contradictory cases and the sequential request owner use its explicit entropy
argument. `public_setup.py` decodes the exact PIPAPK01 public fixed/sigma
evaluation tables, descriptor/VK bindings, canonical fields and copy digest.
Its domain-aware barycentric evaluation handles on-domain points exactly.
Public-only opening groups are evaluated from those tables; only groups with
witness masks draw random opening values. Multiple products/lookups retain the
native ordering. No transcript challenge or permutation is programmed.

`rebind.make_case` consumes caller-supplied exact descriptor, VK and public
original bytes plus already produced chosen parameters/logs. It retains the
historical bytes, changes only the descriptor parameter digest, rebuilds fixed
and sigma commitments using the native default blind1, and preserves the
public tables/copy digest/selectors. Its fresh private authority is established
before decoding the new descriptor and key. The sole accepted historical k16
triple is fixed in `rebind.OMEGA`; it is custody metadata, not a signed catalog
or current source qualification. This path never discovers or opens catalog
originals by itself.

k>6 rebinding and proof execution require explicit `allow_large=True`; defaults
refuse before private-reference creation/large hashing. Direct private-reference
entry also validates a supplied authority tag, integer k, raw byte extent and
explicit large opt-in before creating any directory or hashing supplied bytes.
An exact descriptor override must be a canonical64-character lowercase hexadecimal
string before it can be inserted into private copied source. A direct caller owns
any reads performed before supplying bytes. Small checks/CI never opt in. The
k16 private reference changes exactly two resource checks: it permits only the
specified Pallas V2 descriptor digest and Pallas k16 parameter extent. Parsing,
RP57, key binding, all verifier equations, full generator decision and parameter
hash authority remain unchanged. These private copies do not alter repository
or production acceptance and do not admit an arbitrary issuer CRS.

Historical inner curve points, trivial-accumulator constants and embedded
recursive keys remain the old relation's constants after this outer rebinding.
It is not a coherent recursive catalog re-key and supplies no native wallet
proof or release authority. Any such experiment needs separately re-synthesized
relations and authenticated artifacts under one coherent chosen setup.

The additional small controls preserve the original generic arithmetic's six
complete cases, ordinary proof/descriptor/key parity, exact table and original
custody, public-only values, two-product/two-lookup shapes, all point logs,
mutations, authority refusal and bounded replay. Five further methods check
interpolation/canonical-table rejection, query bounds, descriptor-derived
entropy accounting and pre-I/O large-mode refusal. They generate six additional
k6 proofs, for fourteen total across77 methods. All prior60 assertions remain;
the request binding control additionally mutates public preprocessing bytes.
No k16 proof, large parameter family, timing qualification or C12 claim follows.
