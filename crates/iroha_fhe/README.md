# iroha_fhe

Deterministic exact arithmetic shared by Iroha's homomorphic-encryption
protocols. This crate is the single owner of the reusable ring arithmetic below
the protocol layers; `specs/fhe_ownership_inventory.json` records which
function owns each primitive and `scripts/check_fhe_ownership_map.py` checks
that record against the source.

For the consumers below it holds arithmetic only. Their schemes, parameter
sets, key and ciphertext formats, wire codecs, noise accounting and proofs stay
with their protocols:

| Consumer | Location | What it keeps |
| --- | --- | --- |
| BFV and RAM-LFE | `crates/iroha_crypto/src/fhe_bfv.rs` | wire types, parameter policy, guard order, diagnostics, key switching flows |
| ZK-AMS multi-key BGV | `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe*` | profile, wire, collective protocols, test-only references |
| Jindo | `crates/iroha_core_privacy/src/privacy_engines/jindo` | pinned primes and roots, RNS polynomial type |
| Bootle-Lantern | `crates/iroha_core_privacy/src/privacy_engines/bootle_lantern` | pinned Montgomery constants and CRT profile, ring types |

Consumers call the kernels by their own names: a renamed import of a function
or type hides call sites from the ownership inventory, and its tests reject one.

The crate also owns `bgv`, the exact modular RNS-BGV scheme that RAM-LFE and
SoraCloud are to share (see [RNS-BGV](#rns-bgv)). The unqualified RAM-LFE
affine and complete generic bounded evaluators use it; production application
admission remains unavailable.

Production dependencies are `zeroize`, `thiserror`, `blake3` (coin expansion and
profile identities) and `getrandom` (operating-system entropy); a test fails
when that list changes. No policy, node or SDK crate can be reached from here,
so the dependency graph stays acyclic.

## Modules

| Module | Content |
| --- | --- |
| `modular` | add, subtract, multiply, power and inverse modulo a word modulus; Miller-Rabin; primitive-root search |
| `constant_time` | branch-free selects, canonical add/subtract, fixed-modulus Montgomery arithmetic, centered three-prime CRT |
| `ntt` | radix-2 cyclic transform and allocation-free cyclic product, negacyclic twist and product, exact CRT convolution |
| `polynomial` | coefficient-wise arithmetic and the schoolbook negacyclic products, one residue kernel generic over word and fixed-modulus arithmetic |
| `rns` | modulus-chain validation, residue decomposition, CRT reconstruction, basis extension |
| `rounding` | centered lift, nearest division, scale-and-round, modulus-switch rounding |
| `key_switch` | digit decomposition and the digit/key inner product |
| `automorphism` | the signed coefficient permutation of `X -> X^k` |
| `accel` | accelerated slice kernels, their dispatch and scalar references |
| `natural` | arbitrary-precision non-negative integers for public quantities |
| `rns_chain` | exact mixed-radix basis conversion and congruence-preserving scale-down over chains of any length |
| `ntt_plan` | precomputed constant-time negacyclic transform of one prime below `2^62` |
| `bgv` | the RNS-BGV scheme: profiles, keys, encryption, leveled operations, canonical encodings, noise bounds and shape planning, slot encodings, cost model |

## RNS-BGV

`bgv` is the scheme of Brakerski, Gentry and Vaikuntanathan with the plaintext
in the low bits: `c0 + c1*s = m + t*e (mod Q_l)`, `gcd(Q_l, t) = 1`. It is not
BFV scaling and not the exact-lift construction of `iroha_crypto`. It is a
starting candidate: no profile has a reviewed security estimate, and circuit
privacy, client well-formedness proofs, verifiable distributed opening and
production bootstrap qualification remain unfinished. Trusted local threshold opening is described below.

- **Plaintext precision.** One master profile binds an odd base prime `p` and
  exponent `E`, with `p^E < 2^31`. Each plaintext, ciphertext, shape, encoding
  and prepared map binds an explicit effective `1 <= k <= E`. All keys,
  sampled errors and modulus-switching corrections retain master scale
  `p^E`; ordinary arithmetic preserves `k` and rejects mixed precisions.
  Fresh noise is `(p^k-1)/2 + p^E*B*(2N+1)`. One CRT target per level converts
  to the master modulus, followed by constant-time reduction to `p^k`.
  The profile retains exactly `E * size_of::<PlaintextPrecision>()` bytes of
  context payload, plus its vector metadata, rather than a table per level
  and precision. No public exact-division/noise-reset operation exists.
- **Chain.** Any number of main primes and special primes below `2^62`, all
  `1 mod 2*N*t`. Basis extension is exact (mixed radix, centered): no `u128`
  reconstruction, no approximate conversion, no floating point.
- **Rounding.** A modulus switch and the division by the special modulus use
  `c' = (c - t*w) / D` with `w` the centered representative of `c * t^-1`
  modulo the odd `D`. No tie exists; the plaintext is unchanged because
  `D = 1 mod t`.
- **Key switching.** Hybrid RNS: digits of consecutive main primes, exact
  centered extension, congruence-preserving division by the special modulus.
- **Coins.** 64 bytes of operating-system entropy per operation, expanded by
  BLAKE3 `derive_key` with the profile identity and a purpose tag. Uniform
  ternary secrets and masks, centered binomial errors with `eta = 21`
  (support `[-21, 21]`, variance 10.5), uniform residues by rejection. The
  seed, the hash state that absorbs it and the buffered stream live in one
  heap block that is overwritten whole before it is released.
- **Noise.** Every ciphertext carries an exact public upper bound of its
  noise, computed from profile constants and the operation history only. An
  operation whose bound would exceed `(Q_l - 1) / 2` is refused. The bound is
  worst case; with the bounded error sampler an admitted sequence over honest
  coins never fails to decrypt.
- **Planning.** `bgv::plan` applies the same rules to the public shape of a
  ciphertext (master identity, effective precision, level, components,
  bound). Every operation takes the shape of its result from there, so a sequence can be planned without a key or a
  ciphertext and is admitted on ciphertexts exactly when its plan is.
- **Canonical encodings.** Parameter sets, ciphertexts, public keys, switching
  keys and evaluation keys encode to canonical words and fixed byte frames
  and decode from them. The parameter frame explicitly includes `p` and `E`;
  ciphertext byte 35 explicitly contains `k`, and zero is refused. Ciphertext
  residue bytes use each public prime's bit width, least-significant bits first,
  with component/limb order unchanged and zero high padding at each limb end.
  The fixed header and master-sized noise bound precede the packed residues.
  There is one canonical byte layout; word APIs, key frames and resident residue
  budgets retain their existing representations. Decoding
  rejects another profile, any other length and a
  non-canonical residue or nonzero limb padding. Exact length and claimed-noise
  admission precede residue allocation; decoded residues enter their final
  clearing owners directly. The secret key has a custody frame of two bits per
  coefficient in a clearing buffer. A decoded object is well formed as an
  encoding only; authentication, proofs of well-formedness, Norito wire types
  and bounded admission belong to the consumers.
- **Encodings.** `SlotEncoding` gives the scalar slots a plaintext modulus
  admits in the ring: the slot count comes from the base prime `p-1`,
  giving 128 sparse slots for `p = 257` at any `N >= 128`, including all
  admitted effective powers of 257. Roots form a compatible prime-power
  tower; inverse roots and slot counts are units modulo `p^k`.
- **Packed linear maps.** `SlotLinearPlan` and `SlotLinearMap` evaluate every
  dense matrix diagonal, including zeros, using baby-step/giant-step routing
  and recursive aggregation over the actual slot automorphism group. The plan
  selects a fixed schedule from public noise and coefficient/output/scratch
  byte budgets. Private matrices clear on drop; input/key checks precede their
  use. The scratch limit bounds owned ciphertext residue buffers, excluding
  shared-kernel scratch and metadata; it is not a complete application budget
  or a circuit-privacy construction.
- **Candidates.** `BgvCandidate` pins three parameter sets over `t = 257`.
  `specs/fhe_bgv_cost_summary.json` holds their sizes, work units, the plan of
  every maximum-shape tape family, host measurements, projections of
  sanitization headroom and proof work, and a core-SVP sanity calculation.
  `make check-fhe-bgv-cost` checks it with `scripts/fhe_bgv_cost_model.py`,
  and `tests/bgv_cost_model_vectors.rs` recomputes its sizes, work units,
  instruction costs, tape plans and contract inputs with this crate. No
  candidate holds the maximum shape of its class when a tape is lowered
  instruction by instruction; the summary says what each holds.
- **Security estimate (unreviewed).** `scripts/fhe_bgv_lattice_estimator.sage`
  ran the Lattice Estimator at a pinned commit on every candidate; the raw
  output is `specs/fhe_bgv_lattice_estimator_output.txt` and the summary
  restates it (`security_estimate.lattice_estimator`). For plain LWE of
  dimension `N` on the largest modulus `Q * P`, the cheapest attack that
  returned a cost is, in bits under BDGL16 classical / ADPS16 classical /
  ADPS16 quantum: affine 136.9 / 102.5 / 93.0, bounded 197.9 / 161.5 / 146.8,
  refresh 135.3 / 99.0 / 89.8. The design target of 128 classical bits under
  BDGL16 is met by all three on the attacks that returned a cost, with 8.9 and
  7.3 bits of margin for the affine and refresh candidates; some attacks
  returned none (the summary lists them). Under the core-SVP count of ADPS16
  only the bounded candidate meets it. The estimate does not cover the ring structure,
  the circular-security assumption of the published switching keys, keys
  that were not sampled honestly or side channels, and it leaves the class
  fit as it is. No candidate is qualified; the parameters await review.

## Full-ring relative-trace component

`bgv::relative_trace` decomposes `a(X)` into the unique coordinate banks
`a(X) = sum_r X^r g_r(X^d)`, where `d=N/S` and `S` is the existing scalar-slot
count. A fixed parity tree computes all banks with `d` plaintext products and
`d-1` Galois applications. Recomposition sums all ordered banks after public
monomial shifts. Output banks encode evaluations of `g_r`; this is not yet
a complete coefficient-to-slot transform.

Public plans check shapes, required keys, noise and scoped residue/plaintext
payloads before arithmetic. Clearing owners release partial results on error
and unwind. The payload scope excludes borrowed keys/inputs, backend scratch,
metadata and allocator overhead. Native tests include both N16384/q6 full-ring
roundtrips and independent physical-noise checks. Levels remain unchanged and
noise grows. The complete local bootstrap below adds the encrypted-secret
construction and nonlinear stages; proofs and production qualification remain open.

## Complete coefficient-to-slot transforms

`CoefficientSlotDecomposition` consumes a full-ring ciphertext and combines
trace normalization with one shared inverse Vandermonde map. A private unit-scaled
entry into the sole parity executor yields `d*g_r`, where `d=N/S`; matrix weights
`N^-1*zeta^(-(2j+1)k)` recover each original coefficient because `d*S/N=1`.
The public normalized relative-trace operation retains its own meaning. Both
paths derive the actual multiplier and its bound together; no phase bound is
reset or divided. The raw bank owner has no public constructor and is consumed
only by the complete coefficient map. Every coefficient is retained: bank `r`,
slot `j` contains `a[r+(N/S)*j]`. The inverse map and
existing recomposition restore the original polynomial. Both directions use
canonical packed routing and compatible roots modulo the effective `p^k`.

The actual transform initializes `CoefficientSlotBanks`; the concrete upper-digit
executor can return its consumed successor, preserving coordinate order while
changing values and effective precision. This owner is non-Clone and has no
byte-frame, ciphertext-vector or claimed-shape constructor. Its origin is
conditional on ciphertext/key correctness; it authenticates no imported input
and proves no imported noise claim.
Plans check exact shapes and the complete key union before request arithmetic.

The public complete-bank view collects the same consuming parity cursor used
by bootstrap, then moves owners into natural residue-class order. Its residue
ceiling includes the consumed input and every retained bank; matrix, layout,
preparation row and monomial payloads have separate limits.
Backend scratch, public shape metadata, keys and allocator overhead remain
outside those scopes. The forward/inverse transforms preserve levels and
increase noise; they provide no generic precision cast or division operation.
Moving normalization into the matrix avoids a separate modular-unit amplification.
The canonical planner still chooses the split and computes all transfer bounds;
only fixture-specific improvements are asserted. The initial unit product is
retained, with the same conservative operation and lifetime accounting.

## Encrypted upper digits

`EncryptedUpperDigitsPlan` lowers the immutable public `UpperDigitsProgram`
to canonical BGV operations on genuine `CoefficientSlotBanks`. A fixed public
schedule uses cached powers and balanced polynomial blocks; only public zero
and unit coefficients simplify the schedule. A public level per multiplicative
depth applies the same modulus drops to all still-live values.

Public constant products use `BgvProfile::mul_public_scalar` and the matching
shape planner. They derive the centered coefficient and its norm together,
then apply the shared accelerated modular scalar kernel to every residue,
including for zero and one. The result and noise equal ordinary multiplication
by that constant polynomial; no plaintext buffer or NTT is required. Only
public scalars use this API. Its declared cost is
`components * N * (level + 1)`; scalar reduction, validation, copying and
clearing retain the existing exclusions. Upper-digit constant products use
this sole path; the generic polynomial product remains for other plaintexts.

Only a matching compiled tableau subtraction can reach private exact division.
Scalar provenance and the matched lifting-tableau identity establish
coefficientwise divisibility of the unwrapped integer phase by `p`. Multiplication of every RNS residue by `p^-1` therefore
produces the exact centered quotient, with bound `floor(V/p)` and precision
`k-1`; it is not a generic caller-authorized bound or precision change.

Allocation-free counting checks compiler, metadata and work ceilings before
DAG allocation. Exact shape replay checks every operation, retained bank,
tensor/result overlap and modulus drop. Residue, kernel scratch, plaintext and
metadata budgets are separate; incoming metadata, allocator overhead, fixed
stack frames, profile/key storage and whole-process RSS are excluded. Failure
and unwind clear consumed inputs, live intermediates and completed outputs.

Native p5 E2/E3 tests cover every coefficient residue, reconstruct each actual
DAG phase by independent CRT/convolution, check each integer quotient, and
verify the complete inverse transform. These are unselected arithmetic
fixtures. The complete local bootstrap below supplies the matching encrypted
secret and full transform pipeline; circuit privacy, proofs and production
qualification remain open.

## Complete local bootstrap

`LocalBootstrapKey` generates matching public/evaluation material and an actual
encryption of its secret, returning secret custody separately. Runtime refresh
receives no secret key. A non-Clone ciphertext owner borrows this immutable key;
Canonical encryption, trivial construction, sums, plaintext products, modulus
switching, multiplication, squaring, packed routing and refresh preserve its
local origin. Trivial construction hides no plaintext. This origin establishes
local construction and bounded decryption semantics, not scalar-subalgebra
membership or external authentication. Exporting the ordinary ciphertext loses
that origin; independently generated keys are rejected even with the same profile.
`preflight_evaluation` checks the exact local source shape and complete requested
key union before a consumer reads request memory or private matrices.
`mul_plain` returns the unchanged checked input and its actual product by
moving both owners. A consumer explicitly retains or drops the input; no raw
conversion, ciphertext clone or second plaintext-product API is involved.
The input, result and existing arithmetic scratch coexist during that operation.

For master `T=p^E`, effective `t=p^r`, scale `a=p^(E-r)` and centered source
phase bound `V`, `BootstrapPlan` requires `2*a*V+(N+1)*Q<a*Q`. Centered RNS
conversion forms `w_i=[a*c_i]_Q`; canonical multiplication by the encrypted
secret and plaintext addition encrypt `w0+w1*s`. The strict dense-ternary carry
bound lets balanced upper digits recover the message after complete coefficient
decomposition; the inverse restores every original coefficient. This follows
the scaled homomorphic-decryption construction in Geelen and Vercauteren,
[*Bootstrapping for BGV and BFV Revisited*, §§3.2–3.3](https://doi.org/10.1007/s00145-023-09454-6),
with the explicit conservative carry condition above.

`BootstrapPlan::new` prepares one immutable transform/upper-digit core and an
exact source descriptor. `admit_source` reuses that same preparation for another
exact shape, rechecking profile, precision, components, strict carry and all
selected-stage, source, stream and complete-work ceilings before sharing it.
It cannot replace the program, level schedule or routing split, and it creates
no ciphertext origin. Each refresh still checks its exact source and same-key
owner and executes the complete pipeline; preparation reuse removes no work.

`LocalBootstrapKey::route` consumes a fixed array of same-owner inputs and
invokes the sole canonical `SlotLinearMap` evaluator. It returns the original
checked inputs and computed outputs by moving their owners, preserving input
allocations and exact shapes. A consumer can retain a carried base bank without
cloning it or adding another arithmetic output. Its residue ceiling includes consumed
inputs, planned outputs and routing scratch; backend scratch, plaintexts,
keys/profile and public metadata remain separate. Native two-input/three-output
controls cover heterogeneous bounds, zero/dense inputs, origin/budget/key/shape
refusals and late error/unwind clearing. An independent public API test checks
all output coefficients before refresh and through two square/refresh cycles.

Setup admits retained key residues, returned secret storage and secret-plaintext
payload separately. Runtime consumes the homomorphic input through one fixed
parity traversal. Each leaf carries its natural residue ordinal through the
inverse-N map, complete upper-digit DAG, forward map and monomial accumulator.
Only pending siblings and the current stages remain live; no complete bank
array is retained by refresh. Public complete-bank APIs collect the same core.
All `d=N/S` banks and their full arithmetic remain charged, with unchanged
phase bounds and output precision. There is no alternative refresh evaluator.

`stream_owned_residue_bytes` admits the combined post-input lifetime peak,
including pending high-level siblings, route scratch, DAG tensor/switch overlap
and the prior output accumulator. `stream_metadata_bytes` separately admits
new bounded traversal/frame/bank/accumulator headers and conservative requested
bound capacities. Existing prepared matrix/layout/program budgets and input
plaintext ownership remain distinct. Upper-digit kernel scratch keeps its own
ceiling; keys/profile, other backend/plaintext scopes, allocator overhead and
RSS are not inferred from this retained-residue ceiling. These are not the
complete application resource limits. Refusal or unwind drops the consumed
cursor, current bank and partial accumulator; incomplete output is not returned.

`CiphertextShape`, `SlotLinearPlan` and `BootstrapPlan` expose allocation-free
`metadata_requirement` bounds and `retained_metadata` observations for their
complementary typed storage. The requirement separates the enclosing inline
owner, retained public vectors and noise-bound allocations, and additional
sequential preparation scratch. The scratch includes simultaneous routing
candidates/caches, rejected transfers and bootstrap staging. An aggregate
caller admits these bounds before preparation, counts its own containers and
all distinct retained plans, then refuses excess observed capacities before
private preparation. `BootstrapPlan::split_metadata_requirement` and
`retained_metadata_split` separate the shared preparation graph from each source
descriptor; their standalone totals charge one of each. Count the core once only
when `shares_preparation` confirms actual shared allocation identity, and count
every descriptor and each call's full work. Sequential preparation takes the
maximum scratch. `shared_preparation_bytes` reports the conservative supplied
matrix/layout/upper scope, which remains separate from typed metadata and does
not shrink when a later descriptor supplies smaller ceilings. Current bounded
planning still constructs independent cores and must charge each one.
Matrix coefficients, shared layout graphs, upper-program/DAG payloads, profiles
and keys keep their separate mandatory scopes. These architecture-local
requested-storage bounds exclude allocator headers, excess reservations, compiler
stack frames and RSS; they establish no
consensus memory policy. Actual retained metadata is checked on complete
N65536 q23/q24 public plans as well as small encrypted-plan fixtures.

The plan also checks aggregate `canonical_arithmetic_units` and
`scalar_word_operations` across input conversion, both complete transforms and
all-bank upper digits. These are separate existing work categories, with checked
sums and products; streaming changes no bank count. Refusal precedes private
input execution. Existing public preparation ceilings remain enforced before
that aggregate can be computed; this is not a claim of refusal before all
public preparation. Slot encoding, copying, clearing and proof work are outside
these canonical counters and still need complete application accounting.

Native unselected p5/E3/N16 arithmetic tests preserve zero, basis and dense
messages through repeated square/refresh cycles; independent full-ring CRT
checks validate the actual phase and public bound. The five-prime fixture
returns effective precision 1 at level 1 with bound `4710378958336`; three
successive square/refresh cycles each strictly reduce the squared input's bound.
Refreshing a fresh ciphertext need not reduce its bound. The four-prime public plan
completes at level 0 and refuses a subsequent ciphertext square. Origin, strict
carry, budget, entropy, allocation and error/unwind clearing controls also pass.

Native unselected F257/E3/N256 tests also exercise the complete pipeline with
22 50-bit main primes, two 60-bit special primes and both coefficient banks.
Zero, `X-X^255` and dense full-ring messages pass nine complete bootstraps and
six squares, with 39 independent BigInt CRT/convolution checks of every
coefficient and actual phase bound. The fixed 18-wave schedule returns at
level 3; lowering to level 1, squaring and lowering to level 0 gives bound
`2183540320`, which admits the next refresh. Its scoped streaming residue peak
is `1622016` bytes, and each bootstrap declares `102490880` canonical arithmetic
units plus `7559680` explicit scalar-word operations. This small-ring fixture
does not establish full-size execution, production parameters or security.

An explicitly run public planner test admits the complete current pipeline at
unselected F257/E3/N65536 with 24 60-bit main primes and two 60-bit special
primes. It checks the exact 18-wave schedule, all modulus drops, output squaring
and the strict level-zero carry boundary against independent integer vectors.
The same public planner admits a 654,311,424-byte (624 MiB) combined streaming
residue ceiling for this fixed fixture, replacing the need to retain all 512
initial banks (12 GiB of residues). It still charges every bank's complete work;
the per-bank upper DAG peak is 368,050,176 bytes. Keys, other metadata/backend
scopes and actual process memory remain separate. This test generates no
ciphertexts or keys and proves no full-size encrypted execution, security
estimate or complete application resource bound.

At that public fixture's level zero, the packed two-component frame is 983,260
bytes, including its 36-byte header and 184-byte bound. This leaves 65,316 bytes
below the separate unchanged 1 MiB encrypted-input cap. A byte-codec fixture
alone authenticates no input and grants no local-bootstrap origin. Current cost
projections use packed frames; historical measured frame sizes remain labeled
with their recorded layout and are not presented as new measurements.

A separate unselected F257/E3/N65536 public-planner fixture uses 23 60-bit
main primes and two 60-bit special primes with the current generic matrix
bounds. Its 18-wave schedule returns at level 4. Ordinary switching to level 2,
squaring, then switching to level 0 yields bound `5588487865142`, which satisfies
the strict carry condition for the next refresh. The same schedule can repeat
from that actual source shape. Its bootstrap counters are `11423263096832`
canonical arithmetic units and `539348762624` explicit scalar-word operations.
Including the output drops, square and source drops gives `11423296061440`
canonical units, retaining the declared exclusions above. This public diagnostic generates
no keys or ciphertexts and establishes no minimal prime count, security level
or full-size encrypted execution.

TODO(C.8, R.9, R.11): qualify F257 auxiliary parameters and a complete application
schedule/resource ledger, prove external input/key correctness and circuit
privacy, and establish security before activating a production profile. Local
construction provenance authenticates no imported ciphertext or distributed key.
The same-key encrypted-secret material adds an `Enc_pk(s)` relation alongside
switching-key relations. Ordinary LWE estimator costs do not establish security
for this combined key-dependent-message view. The local owner has no bootstrap
key export/import API; private fields do not protect material from an evaluator
that controls the process. External deployment still requires an explicit
construction-specific circular/KDM assumption or applicable reduction.

## Offline threshold custody and local opening candidate

`bgv::threshold` is a bounded C.4 component for an explicitly trusted offline
dealer. It consumes an existing BGV secret key and issues opaque, non-Clone
Shamir shares over every main and special RNS prime. The context binds a
caller-declared threshold and roster, application key context, and epoch;
fresh per-invocation instance IDs prevent mixing separate sharings. These are
independent of consensus quorum rules. `t=1` means no threshold secrecy.

The existing coin and polynomial owners clear seeds, random coefficients,
the consumed key, and share buffers on drop/error/unwind. Final issuance
clears the dealer's retained secrets. A caller-supplied ceiling bounds the
dealer's secret buffers plus one issuance, excluding already returned shares,
public metadata, allocator overhead, and stack/register copies. It is not a
process-memory bound. One-time issuance is in-memory only.

`validate_selection_metadata` checks distinct participation and exact context
and instance binding. It is not an algebraic verifier. A canonical
`ThresholdSelection` weights each consumed local share in place into an opaque
subset-additive owner. `ThresholdSelection::open_local` then consumes that exact
ordered subset and opens a two-component ciphertext by summing ciphertext-share
products and applying the existing centered RNS plaintext conversion. It never
reconstructs the complete key or exports per-participant decryptions. Tensors
must first be relinearized. The operation accepts all admitted subsets and
supersets at the ciphertext's current level.

Opening preflight checks profile, level, component and share geometry, exact
selection/context/instance binding, and a caller-supplied buffer ceiling. The
ceiling counts all retained full shares plus three current-level polynomials;
conversion's output fits below that peak. Public metadata, allocator overhead,
borrowed ciphertext/profile storage, and stack/register copies are excluded.
A refusal returns the original vector and unmodified share buffers. Success
consumes the shares; all arithmetic scratch clears on error and unwind.

This is a local operation under one trusted, co-located owner. Profile and
caller-declared context identities do not authenticate a registered key,
application purpose, authoritative epoch, ciphertext provenance or request.
There is no production share codec/import, exported contribution, network route
or registered policy. Only test fixtures expose secret words for independent
oracles. This does not qualify C.3 profiles or complete C.4. TODO(C.4):
distributed key generation, authenticated durable custody, reviewed masked
ciphertext-bound contributions, malicious-party proofs, authorization and
replay/retry handling for verifiable distributed threshold opening. The
separate BFV and MKHE protocols retain their own semantics.

## Rounding rules

No floating point is used. Every rule is stated in `rounding` and tested at
exact halves and range boundaries:

- centered lift of `x` modulo `m`: `x` when `x <= floor(m / 2)`, else `x - m`
  (the even midpoint is positive);
- nearest division and scale-and-round: nearest integer, ties away from zero;
- modulus switching: centered lift, scale by `to / from` with the rule above,
  then the least non-negative residue modulo the target.

## Determinism and acceleration

Every function returns an exact integer result, so equal inputs give equal
words on every target. The scalar implementation is the semantic reference.

The `simd` feature adds NEON (AArch64) and AVX2 (x86-64, detected at run time)
kernels for modular addition, subtraction, multiplication and the transform
butterflies. They require canonical operands; sums require a modulus below
`2^62` and products an odd modulus below `2^32`. Any other input takes the
scalar path, and each dispatching function is tested word-for-word against its
`_scalar` reference. No environment variable or configuration value selects a
backend.

`simd` is a default feature of this crate, so its own tests cover the kernels.
Workspace consumers depend on it with `default-features = false`;
`iroha_crypto/bfv-accel` is the one feature that turns the kernels on in a
node build.

The NEON kernel runs natively on every AArch64 host. The AVX2 kernel runs with
the CPU's instructions on x86-64 hosts that have AVX2. Test builds on other
hosts compile the same kernel source over `accel/avx2_model.rs`, a lane model
of the twelve intrinsics it uses, and compare it with the scalar reference; on
x86-64 a test compares each modelled intrinsic with the CPU. The model checks
the kernel's schedule. It is not a run of the instructions.

Metal and CUDA kernels for word-modulus arithmetic do not exist yet. The GPU
transforms in `crates/fastpq_prover` are specialised to the Goldilocks proof
field and sit above this crate. The dispatch site in
`ntt::cyclic_ntt_in_place` carries the `TODO`.

## Secrets

Kernels hold copies of operands and partially built results in clearing
buffers, which are cleared on return, on a rejected input and on unwind.
`tests/clearing.rs` installs an allocator that inspects freed blocks and
asserts this for each of those exits. A value a kernel returns belongs to the
caller. `constant_time` is branch-free in operand values; `modular` is not.

## Tests

```sh
cargo test -p iroha_fhe                         # accelerated kernels and scalar references
cargo test -p iroha_fhe --no-default-features   # scalar-only build
make check-fhe-bgv-cost                         # cost summary, its view and the known answers are current
```

The BGV tests use two references that share no code with the crate:
`tests/vectors/bgv_reference.py` (Python integers, its own BLAKE3) generates
`tests/vectors/bgv_kat_v1.txt` from master seeds, and `tests/support/oracle.rs`
recomputes every pipeline at run time with `num-bigint`. `tests/bgv_vectors.rs`
and `tests/rns_chain_vectors.rs` require the crate to reproduce every pinned
word; `tests/bgv_noise.rs` compares the public noise bound with the exact
noise the oracle computes; `tests/bgv_clearing.rs` observes released memory,
the block that holds the seed and the stream state included;
`tests/bgv_surface.rs` checks that seeded entry points stay behind the
`bgv-test-fixtures` feature.

`tests/support/machine.rs` is the costing harness: one lowering of the eleven
V1 tape instructions that runs over public shapes and over ciphertexts.
`tests/bgv_tape_plan.rs` checks that the two runs agree instruction by
instruction and that the encrypted run computes the tape;
`tests/bgv_cost_model_vectors.rs` holds the tracked cost summary to the Rust
model; `tests/bgv_cost_measurement.rs` holds the measured workloads.

`tests/canonical_vectors.rs` pins known-answer vectors for every primitive.
The answers were derived with Python integers, and the test re-derives each one
with an independent `num-bigint` oracle built from the textbook definitions
(quadratic DFT, schoolbook negacyclic product, constructive CRT, exact rational
rounding), so neither the kernels nor the vectors can drift unnoticed.

## R.11 offline encryption relation

Ordinary encryption owns its sampled mask and errors in a non-cloneable,
clearing witness and still uses the original tagged streams and BGV arithmetic.
The `bgv-test-fixtures` module alone can retain that witness and check bounded
ternary/error coefficients, the exact borrowed key and plaintext, and every
ciphertext component and fresh noise-metadata word. It offers no witness codec
or raw coefficient accessor. The checked buffer ceiling covers owned sample and
RNS scratch payloads, not input objects, allocator overhead or process memory.

This variable-time offline oracle is a prerequisite, not a client proof or
admission route. It establishes neither honest key/randomness generation nor
circuit privacy. RAM-LFE tests compose the existing scalar encoding with its
64-slot length/byte policy and zero upper slots at the crypto layer.
TODO(R.11): authenticated committed input/key proofs, honest-generation
provenance, a reviewed sanitizer and fixed maximum-shape physical schedule,
query/replay enforcement, and full leakage/security/resource qualification.
