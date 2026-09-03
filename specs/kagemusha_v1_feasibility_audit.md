# KAGEMUSHA V1 static feasibility audit

Source audit, 2026-09-03. This is **static evidence**, not measured circuit,
proof, artifact, latency, memory, energy, thermal, or hardware qualification.
No circuit or release limit was changed for this audit. The original plan's
6,528-byte paired proof and 9,211/12,288-byte raw/text exchange goals remain
hard requirements; its padded receive width remains 1--16.

## Fixed SHA geometry

[`PastaSha256JobsV1::capacity_profile`](../crates/iroha_core/src/zk/pasta_sha256.rs)
uses five lanes and the following conservative admission estimate:

```text
blocks(message) = ceil((message_bytes + 9) / 64)
required_rows = max(ceil(total_blocks / lanes) * 2304 + jobs * 64, 65527)
k = 16; usable_rows = 65536 - 9 = 65527
```

The message lengths and padded block counts below are exact for the audited
fixed transcript shapes. The 2,304 rows/block and 64 rows/job are conservative
accounting constants, not a measured placement lower bound. Inactive slots
still enqueue hashes: gating an equality does not remove the hash job.

### State private carrier

The inventory follows `build_scalar_half`,
`constrain_incoming_common_binding_v1`, and
`constrain_receive_batch_binding_v1` in
[`composite.rs`](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs),
plus `constrain_guard_bundle_semantics_v1` in
[`guard_bundle.rs`](../crates/iroha_core/src/zk/kagemusha_v1_recursion/guard_bundle.rs).

| Job | Repetitions | Message bytes each | Blocks each |
| --- | ---: | ---: | ---: |
| Platform credential | 2 | 631 | 10 |
| Device authority opening | 2 | 74 | 2 |
| Normalized Guard statement | 1 | 1,344 | 22 |
| Suite-upgrade bridge | 1 | 328 | 6 |
| Outer state head | 2 | 106 | 2 |
| Sixteen-slot receive batch | 1 | 3,367 | 53 |
| Incoming plaintext-opening commitment | 16 | 228 | 4 |
| Incoming terminal output binding | 16 | 242 | 4 |

Total: **41 jobs / 237 blocks / 113,216 estimated rows per lane**.
The incoming slots account for 32 jobs / 128 blocks. The current
`validate_capacity(65527)` still rejects the
fixed State circuit before key generation, independently of active occupancy.
Keeping every current hash needs at least nine lanes under this estimator:
nine gives a 64,832-row work subtotal, floored to 65,527; eight gives 71,744.
This is a row-admission result, not a feasible release profile.

There is also a genuine physical lower bound, independent of that estimator.
The current Table8
[`compression_util.rs`](../crates/iroha_core/src/zk/pasta_sha256_table8/table8/compression/compression_util.rs)
retains the retired Table16 implementation's 64 compression rounds of 24 rows
each. Each same-lane compression region occupies those advice columns; the
scheduler round-robins individual blocks across lanes. Among 237 total blocks,
one of five lanes receives at least 48 blocks. Compression alone thus needs at
least `48 * 1536 = 73728` rows on that lane, before message scheduling, packing,
initialization, or digest handling. Five-lane State overflow is not merely
conservative-accounting pessimism.

### Post-commit inner TerminalAuthorization

[`terminal_authorization.rs`](../crates/iroha_core/src/zk/kagemusha_v1_recursion/terminal_authorization.rs)
queues the same five Guard jobs / 46 blocks, then these jobs through
`constrain_terminal_commit_semantics_v1`,
`constrain_terminal_send_opening_v1`, and the candidate projection binding:

| Job | Message bytes | Blocks |
| --- | ---: | ---: |
| Prepared one-use authorization | 293 | 5 |
| Predecessor conflict nullifier | 84 | 2 |
| Signed request transcript | 422 | 7 |
| Receiver credential ID opening | 426 | 7 |
| Compact intent transcript | 159 | 3 |
| Signed ticket transcript | 259 | 5 |
| Payment output transcript | 246 | 4 |
| Stable credit ID | 93 | 2 |
| Prepared-transfer transcript | 255 | 5 |
| Incoming claims binding | 283 | 5 |
| Terminal output binding, including receiver lane and exact claims | 242 | 4 |
| Sender one-time commitment | 195 | 4 |
| Payment body | 104 | 2 |
| Outbox reservation | 102 | 2 |
| Commit-evidence opening | 166 | 3 |
| Certificate ID | 287 | 5 |
| Certificate digest | 316 | 6 |
| Precommit binding | 247 | 4 |
| Terminal commit binding | 727 | 12 |
| Normalized 85-cell candidate projection | 1,412 | 23 |

Total: **25 jobs / 156 blocks / 75,328 estimated rows** with five lanes.
The current admission check rejects this inner circuit too. Six lanes reduce
the work subtotal to 61,504, with an admitted estimate of 65,527 after the
table floor. No physical overflow claim for this circuit is inferred from
the conservative estimate alone.
The fixed inactive SendSplit branch is still present for redemption.

The outer `CommitWrapper` is Base-only: it recursively verifies the genuine
inner proof and history, copies its first 39 public cells, and constrains both
authenticated inner protocol identities. It does not directly instantiate
the SHA or dense-MSM columns. This separation is necessary, but does not prove
the outer circuit's size or proving feasibility.

### Current inner MintAuthority

The fixed queue in `mint_helper.rs` and `mint_authority.rs` contains **36 jobs /
176 compression blocks**, including all 31 challenge slots in every branch:

| Job ordinal | Job | Message bytes | Blocks | Global block stages |
| --- | --- | ---: | ---: | --- |
| 0 | Root bridge | 102 | 2 | 0--1 |
| 1 | Seal signing digest | 289 | 5 | 2--6 |
| 2--32 | Validator challenges | 144 each | 93 total | 7--99 |
| 33 | Padded roster | 3,099 | 49 | 100--148 |
| 34 | Certificate binding | 204 | 4 | 149--152 |
| 35 | Inner authority-pair binding | 1,447 | 23 | 153--175 |

This gives 85,248 estimated rows under the current five-lane capacity formula,
above 65,527. These are source-derived counts, not measured placement. Preserve
the existing u16 chain version, inactive-slot masking, little-endian transcript
integers, bridge hash marking (`digest[31] |= 1`), and 128-bit challenge
extraction. The pair job commits the existing inner audits and histories.

A one-block internal hash leaf is a possible measurement unit, not a qualified
replacement. It would need an externally copy-bound eight-word compression
state, which the current whole-hash queue does not expose. A one-lane instance
of the retired Table16 configuration had an uncompressed Processed-PK lower
bound of about 146 MiB before Base. The replacement Table8 source estimates a
10,094,062-byte one-lane Processed key at K12, below the 64 MiB helper-key cap,
but that estimate is not a serialized artifact or a peak-RSS measurement.
Any continuation must enforce exact ordered block/job indices, standard IV at
each job boundary, canonical padding, and completion of the full fixed plan.
The plan itself must be constrained from typed inputs: membership in a
prover-chosen plan root is not semantic authority. Statement and peer-ID hashes
currently assigned on the host are outside this 36-job inventory and remain
separate canonical-binding work.

### Measured, retired Base-only compression candidate

The removed `pasta_sha256_boolean.rs` experiment supplied a fixed-topology,
Base-only SHA-256 compression relation over eight assigned chaining words and
sixteen assigned block words. It constrained 32-bit decomposition, the FIPS
180-4 schedule and all 64 rounds without lookup tables. Both-field KAT,
continuation, substitution, non-u32, cancellation, missing-cell and shape checks
passed **8/8** in 8.58 seconds. This was one compression function only: it did
not constrain standard IV selection, padding, the ordered 176-block mint plan,
or monetary authority.

The exact configured K16 profile is identical in both Pasta parities: four
advice columns, one instance column, one configured fixed column, four
selectors, five processed fixed columns and six permutation columns. Its
current Processed-key prediction is a 362-byte VK and a 52,429,278-byte PK, so
this isolated helper fits the 64 KiB VK and 64 MiB helper-PK ceilings. The
profile check passes **1/1** in 119.43 seconds.

A standalone native key/proof run subsequently passes **1/1** across both
parities in 218.17 seconds. Each parity produces exactly the predicted key
sizes and a 3,200-byte augmented proof. The process reaches **401,735,680
bytes maximum RSS** and a 323,765,256-byte peak memory footprint. The first
Eq-only attempt also exposed a 32-byte accounting defect: the predictor counted
the raw transcript's final scalar but not KAGEMUSHA's appended folded SRS
generator. `pasta_ipa_augmented_proof_bytes_v1` now counts both, and its focused
regression passes **1/1**; the corrected 3,200-byte prediction matches both real
proofs.

This candidate is therefore not qualified. Its real prover RSS is about 383
MiB, above the unchanged 128 MiB selected-process cap. Its bare proof also
cannot occupy the unchanged 2,495-byte transported parity slot; treating it as
internal evidence instead still requires an unmeasured recursive parent and
cannot reduce the measured prover memory. Decorating the relation with exact
plan, padding and authority constraints can only add work. The logs are
`boolean-sha-constraint-tests.log`, `boolean-sha-resource-profile.log`,
`pasta-ipa-proof-size-accounting.log`, and
`boolean-sha-real-k16-diagnostic-rerun.log`; their six-file source snapshot is
`boolean-sha-source-sha256.txt` under
`target/kagemusha-mint-integration.hwK7pG/`.

## Processed proving-key obstruction

The retired five-lane Table16 configuration had 55 advice columns and **41
permutation columns**, including its shared constant column. Its source was
removed by the first-release Table8 hard cut after this obstruction was measured.
The three dense-MSM lanes add 111 advice and **nine permutation columns**;
see `configuration_stays_at_degree_five` in
[`pasta_dense_msm.rs`](../crates/iroha_core/src/zk/pasta_dense_msm.rs).
Both private carriers therefore have at least 50 permutation columns before
any Base columns.

The actual vendored processed-key encoding stores both full `n = 65536`
Lagrange and coefficient permutation polynomials. This follows from
[`permutation/keygen.rs::build_pk`](../vendor/halo2-axiom/src/plonk/permutation/keygen.rs),
[`permutation.rs::ProvingKey::write`](../vendor/halo2-axiom/src/plonk/permutation.rs),
and [`poly.rs::Polynomial::write_streaming`](../vendor/halo2-axiom/src/poly.rs).
Each Pasta field value occupies 32 bytes; this encoding does not compress
zero, identity, or repeated polynomial values.

```text
permutation data alone >= 50 * 2 * 65536 * 32
                       = 209715200 bytes = 200 MiB
```

Headers, Base permutations, fixed polynomials, selectors, other proving-key
polynomials, and the verifier key only increase this bound. The full lower
bound including just the permutation vector/polynomial headers is
209,715,608 bytes. It is not a measurement of process RSS, but retaining these
two polynomial bases already exceeds the current 128 MiB process profile.

The newly integrated inner MintAuthority has the same unconditional format
conflict. Excluding every Base column, its five SHA lanes configure 41 equality
columns (40 advice plus the shared constant), three dense-MSM lanes configure
nine more, and key generation currently expands 110 selectors into fixed
columns. At K16, the 40 SHA advice permutation columns alone serialize to
167,772,480 bytes, already above the 64 MiB helper-PK cap. Under the current
uncompressed-selector format, including the 117 auxiliary fixed columns, 50
auxiliary permutation columns, masks and vector headers—but still excluding
Base—gives an exact source-derived lower bound of 706,746,942 bytes per inner
PK. This is not an observed artifact/RSS result. It proves that the current
layout and Processed representation cannot meet the selected cap; seed-key
optimizations do not remove this inner-key conflict. The generation path now
computes this prediction from the actual configured constraint system before
key generation and fails with a separate resource-profile diagnostic. The same
guard now covers the final four VK-stability rebuilds as well as ordinary
key generation; exact final VK-byte comparisons remain required. Five focused
tests cover checked arithmetic and caps, both real inner geometries,
byte-for-byte prediction agreement with generated K6 PK/VK encodings, and an
over-limit configuration that demonstrably rejects before synthesis in both
parities. The complete current generation selector passes 27/27; this early
rejection is evidence of the obstruction, not successful K16 proof generation.

[`kagemusha_release_v1.rs`](../crates/iroha_data_model/src/kagemusha/kagemusha_release_v1.rs)
currently caps each State PK at 48,234,934 bytes, each helper PK at 64 MiB,
each VK at 64 KiB, the artifact package at 512 MiB, and process RSS at
128 MiB. [`generation.rs`](../crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs)
applies `build_generated` to both inner State keys and
`build_generated_helper_parity` to the inner terminal keys. Consequently,
the current processed-key paths contradict those profiles even at five SHA
lanes. The two State and two terminal inner keys alone exceed 800 MiB of
permutation data, beyond the entire package ceiling.

Naive widening makes this worse. State at nine SHA lanes plus dense MSM has
at least 82 permutation columns, or 328 MiB of permutation data per parity;
terminal at six lanes has at least 58, or 232 MiB. Widening also increases
inner commitments, queries, deferred equations, and the work required in
the compact outer verifier. No existing cap is raised by this audit.

For the outer ordinary IPA proof,
[`ordinary_ipa_proof_profile_v1`](../crates/iroha_core/src/zk/kagemusha_v1_recursion/deferred_parent.rs)
computes `32 * (W + T + E + Q + 37)` bytes at k=16. The 2,495-byte parity cap
therefore permits at most 2,464 encoded bytes and requires
`W + T + E + Q <= 40`. An 81-cell public shape or Base-only configuration
does not establish that inequality. Actual compiled profiles and generated
proofs remain required. Dense-MSM capacity is another independent gate:
three lanes allow at most 1,170 sources in one job, with cumulative lane
occupancy checked separately.

## Sound next steps

The mint transport obstruction below is independent of the State/terminal row
and resource obstructions. Fixing only one does not establish a payment corridor.

1. Add source-coupled inventory regressions around the actual fixed builders
   before expensive key generation. Record their exact Base parameters,
   SHA jobs/blocks, dense source counts, permutation/fixed columns, and the
   ordinary-proof transcript inventory. Retain the failing admission gates.
2. Validate the implemented acyclic incoming-claims reduction below against
   the actual sender/receiver proof chain. Preserve receiver amount/context,
   lane equality, plaintext-opening knowledge, replay, checked sums, padding,
   and exact batch/Guard binding. Isolated hash tests cannot replace this gate.
3. Resolve the processed-key/profile inconsistency before presenting a
   wider circuit as feasible. Any alternate exact key representation or
   circuit layout needs explicit design review, deterministic loading and
   identity binding, negative tests, and measured memory/artifact evidence.
   Changes to local resource presets are not established by the original
   plan and must not be made silently. Neither fewer jobs nor more lanes
   demonstrates the hard proof/exchange targets.
4. Only after these gates pass, generate and verify the real inner/outer
   proof chain for every receive occupancy 1--16 and all required branches,
   measure complete canonical artifacts/exchanges, and run the plan's
   device latency, memory, energy, thermal, recovery, and privacy gates.

### Candidate decomposition for measurement

Compact disk encoding alone cannot resolve the selected process profile. The
current prover materializes at least 166 dense K16 advice vectors for the inner
MintAuthority (55 SHA plus 111 dense-MSM columns), or 332 MiB before Base
columns, keys, lookups, products and quotient evaluation. Its coefficient and
extended advice forms coexist across proving phases, so those same auxiliary
advice columns imply at least 664 MiB there. Existing
consuming/streaming writers remove copies but do not change that live geometry.

The smallest sound experiment is a staged proof DAG, not a wider monolith:

1. A reusable `MintHashShard` proves bounded canonical SHA job/block segments
   and commits their ordered typed certificate inputs and chaining states. It
   has no recursive monetary predecessor.
2. A Base-only `MintScalarPlan` proves the scalar-verifier/transcript calculations
   for the prior authority checkpoint and normalizes certificate inputs,
   committing ordered deferred group-equation slices. It grants no authority
   until the corresponding equation shards are verified.
   Reusable `MintEquationShard` instances prove bounded slices of those
   reciprocal/signature equations against that exact plan commitment. Reusing
   today's 37-advice-column dense lane is not RSS-safe: its advice and extended
   advice parts alone use 148 MiB. The ordinary-backend experiment therefore
   needs a genuinely narrower serialized machine (initial target at most 10--12
   advice columns, trading more rows/slices), or a separately qualified
   out-of-core prover; renaming the current lane is not decomposition.
3. An alternating-Pasta `MintClaimFold` consumes one previous fixed-stage claim
   plus one hash/scalar/equation leaf at a time, enforces the stage index and
   claim accumulator, and emits an authority claim only at the terminal stage.
   Bootstrap disables only the prior-authority edge. The final claim-fold proof
   replaces the present monolithic inner proof consumed by the compact outer.

This DAG is acyclic by `(authority height, fixed transition-stage index)`; no
same-step proof consumes itself. Sharding bounds only the finite work of one
certificate transition. Existing aggregate monetary checkpoint histories stay
constant-sized and unbounded in transition count, so this does not introduce a
hop, ancestry, origin, receipt, fan-in or proof-depth limit.

Before implementing this candidate, configure and synthesize each actual shard
to record advice/fixed/selector/permutation columns, rows and the exact compiled
ordinary-proof inventory. Every generated/loaded leaf must bind a release role,
protocol digest, stage index, plan commitment and final-claim position; reordered,
missing, duplicate and cross-release shards need negative tests. Selector
compression and sparse recipes are only candidates: each actual key must remain
under its selected role cap and each proving phase must be measured below the
selected RSS budget. Finally the compiled compact outer and canonical envelope
must still demonstrate the unchanged 6,528/9,211/12,288-byte limits.

This audit supplies no release qualification and does not weaken the
receiver opening, metadata, hardware, crash-recovery, or transport contract.

## Mint transport obstruction and required integration

`KagemushaMintAuthorizationV1.proof` and `KagemushaMintCreditV1.proof` are
bounded `KagemushaPairedProofV1` values, not internal-only proof blobs. Runtime
verifies the former before changing the pooled reserve; finalization places
the generated MintAuthority proof directly in the latter. `MintFold` then
recursively verifies that same transported proof and its complete history.
See the model's `kagemusha_v1.rs`, runtime `isi/kagemusha.rs`, and
`composite.rs::build_scalar_half` / `constrain_mint_authority_binding_v1`.

Both mint circuit configurations currently include all five SHA and three dense
MSM lanes, even when a branch's jobs are inactive. Their 166 advice columns alone
write at least `166 * 32 = 5312` commitment bytes per parity, before any Base
columns, evaluations, quotient or opening proof. This already exceeds the
2,495-byte per-parity transport cap. Removing that cap would not meet the plan.

The next integration is a genuine compact Base-only outer decider for each
mint family, following the existing State transport verification/fold pattern:

- Retain existing `MintAuthorization*` and `MintCredit*` artifact roles for the
  outer proofs; authenticate eight additional explicit inner PK/VK bindings
  and four inner circuit profiles. Never let a host-supplied inner key authorize
  value without the outer release's pinned constants.
- Authorization keeps 84 public cells: semantic cells 0--45, outer audits
  46--49, and the folded history at 50--83. Authority keeps 56: semantic
  0--11, outer protocol identities 12--15, outer audits 16--19, the proven
  inner pair commitment 20--21, and folded history 22--55.
- Each outer half must verify the actual inner proof, bind its prior history,
  fold the current opening into that history, constrain the reciprocal curve
  equations and inner metadata, and expose the exact common semantic claim.
- Authority's existing SHA pair digest includes the **inner** audits and
  histories. Preserve it as a proven inner pair commitment, rather than
  pretending it equals a new hash of outer metadata. Update its model/Rustdoc
  meaning and native/checkpoint consumers together. Keep exact common-input,
  release, roster, outer-history and mixed-parity rejection checks.
- Inner authority consumes compact outer predecessor checkpoints through the
  existing dynamically authenticated protocol path. Its public protocol
  identities must name that outer authority, independently of its own inner
  proving-key identity. The outer key pins the actual inner VK; the inner key
  pins the outer value-free structure. Final preprocessing must verify that
  the provisional outer structure did not change. No additional unbounded
  checkpoint history or private predecessor archive is required.

Both compact mint decider circuits are now implemented in source. The release
inventory authenticates 42 roles, including eight distinct private mint keys,
without changing the existing role ordinals or resource/transport ceilings.
The four inner layouts are required profile inputs rather than implicit copies
of the outer layouts. These are implementation changes, not release evidence.

MintAuthorization generation and loading now distinguish four inner keys from
four compact outer keys. Generation requires a genuine inner proof and a
current/history fold before building the outer key; device proving uses the
sealed recovery seed with distinct inner and outer phase labels. Both inner
and exported current/history claims are terminally decided. Only the compact
proof is exposed as the existing 84-cell statement. Inner artifacts still
have to pass their unchanged resource limits. Focused compilation, parser,
binding, and integration checks pass as detailed in the receiver-admission
validation record; this source path has not yet
produced a qualified full-size mint proof.

MintAuthority now has the corresponding inner/outer generation and proving
workflow in source. Fresh generation rejects non-Bootstrap input, searches
both outer protocol structure fixed points, rebuilds the inner Bootstrap with
the final outer identities, and compares the actual rebuilt inner **and outer**
VK bytes. Only the selector-zero inner predecessor uses bootstrap parser
scaffolding; the outer requires a genuine, decided inner proof and history
fold. Generation additionally proves and decides the final compact Bootstrap
before exporting its ten parameter/key bindings. Checkpoint recurrence then
consumes compact outer predecessors. Loading authenticates the complete profile
(including separate inner layouts and the genesis roster) before artifact I/O,
and direct proving rejects mixed release/profile/manifest/roster identities.
The rebuilt Core harness passes 53 focused regressions, including the bounded
and fixed canonical SHA checks, prefix CRC, artifact/loading contracts and
isolated compact projections. This is not successful full-size key generation
or proof/resource qualification. The earlier real Bootstrap/FinalizedMint
diagnostic now records a worker panic at the SHA capacity gate: its 176 blocks
require 85,248 rows per the then-active, now-retired Table16 lane, above 65,527.
The old process has exited; no ordinary test-summary/exit-code evidence was
recovered from that durable log. This is failed real mint-key generation, not a
completed real proof or splice test. The complete corridor remains a release
gate. See
[`kagemusha_receiver_admission_v1.md`](kagemusha_receiver_admission_v1.md#bounded-canonical-hashes-and-compact-authority-2026-09-04)
for exact test scopes, timings, source fingerprints and logs.

The fixed-capacity SHA queue now has a variable-prefix API in source: length,
zero tail, exact padding/trailer, every intermediate SHA snapshot, and final
digest selection are constrained without witness-dependent topology. The CRC
helper likewise constrains exact active-prefix CRC64-XZ by undoing fixed zero
padding with constant inverse GF(2) matrices. These primitives do not yet close
MintFold: canonical variable-length assembly, lifecycle/recipient binding and
exact replay-key/envelope binding still need integration. Model-side capacity
arithmetic takes release-pinned **actual** proof widths; it does not restrict
AccountId controller forms or presume every proof consumes its maximum slot.

The next context assembler must preserve the exact model layout: its payload
is a 323-byte fixed-position prefix, `ULEB(P) || payer_payload`,
`ULEB(R) || recipient_payload`, and a 174-byte fixed-position suffix. Thus
`c = 497 + P + ULEB_width(P) + R + ULEB_width(R)`. The asset UUID field contains
sixteen per-byte prefixes (33 bytes including its field prefix), and incarnation
has both its outer and inner prefixes; replacing either with raw identity bytes
would change the canonical hash. Header/padding occupy 48 bytes, CRC covers
only the active payload, and the domain-separated SHA message has length
`102 + c`. These are source-derived assembly requirements, not a completed
circuit. Derive payload capacity from authenticated **outer** MintAuthorization
ordinary-proof profiles and the model's exact-width helper. Fixed binary
barrel shifts can place variable account fields and the suffix without
witness-selected offsets. The same account byte/length cells must participate
in their authenticated identity hashes; host serialization alone is not that
binding. The combined existing envelope budget, not a new account/member cap,
must govern the accepted lengths.

### MintFold ownership and replay closure still required

The compact transport work does not close two independently confirmed monetary
constraints. `composite.rs::constrain_mint_authority_binding_v1` currently binds
the helper's operation, opaque semantic digest, amount, release, protocol IDs,
and pair commitment. The separate active-Guard/state binding does not join that
mint's recipient credential, one-time key, or private credit opening to the
receiving lane. `state_relation.rs` requires nonzero mint replay identifiers and
an empty-to-present sparse-tree update, but its assigned mint replay credit and
envelope fields have no consumer tying them to the finalized mint's exact ID.
Correct host-derived fields or hardware-certified inbox staging alone are not
an in-circuit proof of either relationship.

The next circuit change must privately open the exact finalized mint statement,
hash it into the already verified helper semantic digest, constrain its complete
asset/lifecycle scope and recipient ownership, and bind the exact derived mint
credit ID and envelope to the replay insertion. Ownership must remain valid for
legitimate delayed credits across the plan's ordinary suite/epoch rotation;
simply requiring equality to the latest credential would reject committed
money. Add recipient, amount/asset/scope, opening, replay-key substitution,
duplicate-fold, and retained-credential regression proofs without exposing a
stable recipient/sender state identifier or relaxing the fixed public shape.
These gaps are release blockers until actual constraints and tests close them.

The state public `lifecycle_binding_digest` is the native local-transition
framing, not `KagemushaMintCreditStatementV1.lifecycle.canonical_digest()`.
Bind lifecycle fields individually and preserve the native framing rather than
equating those unlike hashes. The replay value is likewise the native SHA-256
of `BE64(domain_len) || domain || BE64(canonical_credit_len) ||
canonical(KagemushaMintCreditV1)`, where the domain is the mint-credit domain
including its trailing zero byte. It covers the full finalized credit, not
merely its ciphertext or statement digest.

The retained mint reservation already holds the original authorization,
credential and credit opening, but `CreditFoldPreviewV1` and the recursive
state witness do not yet forward that material into a mint-only opening
relation. Reuse the original credential-ID opening to its stable lane and the
randomized recipient commitment; do not require the old device secret or
replace a retained credential with the latest one. Native same-release rotation
allows an older credential generation, requiring epoch-ID equality only when
its generation equals the current one. Cross-release/suite carry still needs
its separately authenticated verifier bridge.

Full-credit hashing also needs an assigned proof-byte seam: the ordinary
recursive verifier currently consumes native proof bytes without returning
the constrained scalar/point encodings that were read. Passing the same Rust
slice to a separate SHA witness does not bind those circuit bytes. Reconstruct
or expose the verifier's exact canonical encodings and tie them to the hashed
credit, alongside both parity histories, outer audits, certificate metadata
and the existing inner pair commitment. The release manifest must be pinned
by the authenticated artifact path, not accepted as an arbitrary witness.

The prerequisite hash audit found a model/circuit mismatch in the private
MintAuthorization relation: raw field concatenations differed from the model's
canonical Norito frames. The source now uses authoritative model layouts for
credential IDs (376 bytes), profile IDs (378), recipient commitments (139), mint
opening commitments (305), and exact plaintext openings (200). A shared assigned
assembler binds every semantic byte and pins headers, alignment and prefixes.
It computes CRC64-XZ inside the circuit using a fixed affine Boolean map and
constrained parity sums; a fresh commitment cannot supply an arbitrary checksum.
Native credential/profile checks use the same canonical bytes. Six model and
ten focused Core regressions pass, including the formerly failing identity
parity case, actual both-parity SHA synthesis and substituted-field rejection.
These are not a successful full recursive mint proof or a resource qualification.
Re-measure the actual circuit
and keys after these additional constraints; no resource ceiling has changed.

The first-release operation surface has no suite-upgrade transition. `Rotate`
is the sole offline hardware-epoch transition and must recursively carry the
complete balance and replay root while authorizing any device-authority change.
Verifier and artifact changes are ordinary release-policy changes rather than a
wallet-state operation or compatibility bridge.

Do not treat canonical account identity as a fixed raw public key: `AccountId`
also supports variable-size controller policies. The finalized statement and
mint credit ID include that typed account. The current SHA scheduler keys each
job to its exact length, so hashing extra zero padding to a maximum is not an
equivalent hash. Fixed-shape handling must preserve the accepted identity scope
and exact existing digest semantics; source-derived block estimates are not
real proof or resource measurements.

The fixed maximum-block SHA schedule is now implemented with constrained actual
length and padding, authenticated Table8 state snapshots, and selection of the
real final chaining state. Active-prefix CRC64-XZ is also constrained without
witness-selected topology; focused positive and mutation checks pass in both
Pasta fields. Variable Norito composition must still constrain compact lengths,
field offsets, account-identity openings and assembly of those primitives into
the exact canonical frame. Derive capacities from the existing complete-envelope
bound and actual pinned proof widths, never a new global AccountId/member-count
limit. The implemented primitives are not a full monetary-circuit or resource
measurement.

K16 remains unchanged. A uniform K17 change would also change deterministic
parameter sizes, history lengths/limbs, fold transcripts, public shapes and
ordinary proof sizes; dense scheduling separately contains K16 limits.
Role-specific SHA widths or a different measured resource profile are still
unqualified experiments. None may silently relax the user proof/exchange caps.

## Implemented acyclic incoming-claims reduction

The current inventories above include this source revision; it is not measured
proof, artifact, memory, or latency qualification. The before/after counts below
record the removed redundant work without treating the old layout as current.
The signed request, compact intent, ticket, output, ciphertext, candidate,
certificate, durable staged records, and 208-byte batch slots are unchanged.

TerminalAuthorization now derives the existing 32-byte prepared-transfer
digest from its exact 210-byte transcript and authenticates the unchanged
incoming-claims digest once:

```text
I = SHA256(existing incoming-binding domain || 0 ||
           request_digest || intent_digest || ticket_digest || output_digest ||
           actual_ciphertext_digest || candidate_digest || certificate_digest)
B = SHA256(existing NUL-terminated terminal-output domain || u16LE(1) ||
           credit_id || ticket_key || receiver_lane || prepared_transfer_digest ||
           output_digest || I)
```

`B` is exactly 242 bytes before SHA padding (four blocks); it includes the
prepared-transfer **digest**, not the 210-byte preimage. The receiver
recomputes `B` against the actual verified CommitWrapper output and retains
the independent four-block plaintext-opening knowledge constraint. Direct
proof equalities for amount, request, ticket, nullifier, opening commitment,
network/asset/release context and both pinned protocols remain. So do local
lane equality, replay insertion, checked sums, canonical inactive padding,
full proof/history verification, and the exact batch/Guard/receipt bindings.
Redundant private intent-ID, sender-commitment, and ciphertext-digest witness
fields are removed; their exact durable metadata is authenticated through
`I`, not discarded from storage or accepted on a host Boolean.

There is no fixed point: State transport semantics remain the payment body
`H(output_digest, ciphertext_digest)`. The normalized 85-cell candidate hash
and hardware certificate are fixed before constructing `I` and `B`.
Neither `I` nor `B` enters the candidate, lifecycle, certificate, precommit,
or hardware terminal transcript. The dependency order is body → candidate
→ certificate → `I` → `B` → TerminalAuthorization → CommitWrapper → complete
envelope digest. The receiving lane remains private inside `B`; no public
lane identifier or wire field is added.

| Carrier | Before | After |
| --- | ---: | ---: |
| One receive slot | 7 jobs / 26 blocks | 2 jobs / 8 blocks |
| All 16 receive slots, including padding | 112 jobs / 416 blocks | 32 jobs / 128 blocks |
| State including unchanged other jobs | 121 jobs / 525 blocks | 41 jobs / 237 blocks |
| Inner TerminalAuthorization | 23 jobs / 147 blocks | 25 jobs / 156 blocks |

At five lanes the new State admission estimate is still 113,216 rows, above
65,527 usable rows. Even compression alone requires at least
`ceil(237 / 5) * 1536 = 73728` rows. Nine lanes satisfy only the conservative
SHA row estimate (64,832 before the table floor); this is **not** a circuit,
processed-key, public-proof, or device feasibility result. No lane/resource
profile, public 85/119/81 layout, or 6,528/9,211/12,288-byte cap changes are
part of this reduction. Authenticated circuit/protocol artifacts must be
regenerated for the revised internal binding transcript.

Focused regressions use the production hash gadgets for every `B` and `I`
component, prepared-transfer field/amount, and recipient-opening secret.
They compare both Pasta parities with native/model transcripts and retain
fixed SHA inventory for zero, one, and sixteen active slots. These isolated
constraint and structural tests do not qualify complete sender/receiver
recursive proofs; full actual-proof mutation and release-resource gates
remain outstanding. Tests were added but not executed by the editing agent;
the shared warm Core build lane owns compilation and execution.
