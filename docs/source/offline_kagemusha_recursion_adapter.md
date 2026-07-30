# Kagemusha recursive-verifier adapter audit

This note records the proof-system boundary for the sole ABI-21/V4 Kagemusha
recursive-spend lifecycle.
It is an implementation audit, not a readiness claim. The production backend
remains unavailable until the fixed-key paired deferred verifier and its
release artifacts pass every soundness, device, and chain gate.

## Rejected BN254/KZG experiment

A standalone release-mode experiment pinned `halo2-base`/`halo2-ecc` v0.5.3
(`c54cbac60da598e8e484b8aea858e0bf3c51a857`) and `snark-verifier`
`bbfcc721d714bea0d44a27c8fc6c4736e73ca853`. It generated and natively
verified two 448-byte application proofs with the GWC KZG opening scheme and
the circuit-native Poseidon transcript. The outer circuit:

- loaded the complete compiled protocol as circuit constants;
- parsed both proof transcripts inside Halo2;
- assigned and re-exposed both exact `(domain, statement_root)` pairs;
- ran both PLONK succinct verifiers;
- accumulated their KZG opening claims;
- exposed the resulting accumulator as twelve 88-bit limbs; and
- generated a real outer proof whose verifier performed the final KZG pairing
  decision.

At outer degree 16 the calculated shape was 59 phase-zero advice columns, 9
phase-zero lookup-advice columns, 1 fixed column, 1 instance column, and 16
public field elements (12 accumulator limbs plus 4 statement elements). The
accumulation proof itself was empty for GWC; the outer proof was 21,312 bytes.
On the Apple-arm64 development host, after release compilation and excluding
Cargo/Iroha build memory, the measured run was:

| Phase | Time |
|---|---:|
| inner setup | 0.006 s |
| two inner proofs | 0.056 s |
| virtual outer shape | 0.759 s |
| outer SRS + VK/PK | 17.707 s |
| outer proof | 17.090 s |
| outer verification + accumulator decision | 0.060 s |
| adversarial accumulator checks | 0.934 s |
| total | 36.862 s |

Peak incremental RSS was 3,359,866,880 bytes. A separate degree-16
`MockProver` diagnostic reached 3.73 GiB RSS after 34 seconds and was stopped.
Shape-only runs already consumed 464--504 MiB incremental RSS:

| Outer degree | Advice | Lookup advice | Incremental RSS |
|---:|---:|---:|---:|
| 20 | 4 | 1 | 463,765,504 B |
| 19 | 7 | 1 | 467,714,048 B |
| 18 | 15 | 3 | 472,563,712 B |
| 17 | 29 | 5 | 497,385,472 B |
| 16 | 59 | 9 | 503,988,224 B |

Proof-byte, protocol/VK, public-statement, transcript-profile, and exposed
accumulator substitutions all failed at outer verification. At the inner
boundary, malformed proof encoding failed parsing; parseable VK/protocol,
instance, and transcript substitutions emitted accumulators that failed the
canonical KZG decision.

This path fails independently on proof payload (21,312 > 12,288 bytes), warm
and cold RSS (greater than 3 GiB versus 96/128 MiB), and host proof time
(17.090 s versus the 5 s phone target). Lowering the degree trades SRS rows for
more columns and does not cure the virtual-circuit memory floor. KZG is
therefore not a candidate for the mobile Kagemusha lifecycle. Its pinned
dependencies and executable compatibility checks are test-only and must not be
linked into production Iroha binaries or artifacts.

## Exact Axiom 0.5 IPA wire

The existing Iroha Halo2 backend uses `ParamsIPA<EqAffine>`, circuit scalar
field `Fp`, `VerifierIPA`, and `Blake2bRead/Write<Challenge255>`. A compatible
adapter must reproduce the verifier transcript exactly; a host verification
receipt or a hash of proof bytes is not a substitute.

For the current transcript, Blake2b uses the `Halo2-Transcript`
personalization. Squeezing appends tag `0`; absorbing a point appends tag `1`
and canonical affine `x || y`; absorbing a scalar appends tag `2` and its
canonical representation. A squeeze clones/finalizes 64 bytes and
`Challenge255` maps them with `from_uniform_bytes`, retaining the resulting
canonical 32-byte scalar representation. The VK contributes its
`transcript_repr` common scalar, itself derived from the pinned VK under the
`Halo2-Verify-Key` personalization. Because `VerifierIPA::QUERY_INSTANCE` is
true, each zero-padded instance polynomial is committed in the Lagrange basis
and that commitment is absorbed; the raw instance scalars are not substituted
for this step.

The PLONK transcript then processes, in order:

1. advice commitments and per-phase challenges;
2. `theta`, lookup-permuted commitments, `beta`, and `gamma`;
3. permutation-product, lookup-product, and pre-`y` vanishing commitments;
4. `y`, remaining quotient commitments, then `x`;
5. instance, advice, fixed, vanishing, permutation, and lookup evaluations in
   the verifier's canonical query order; and
6. the IPA multi-opening argument.

The Axiom BGH19 multi-opening wire squeezes `x_1`, squeezes `x_2`, reads the
multi-point quotient commitment, squeezes `x_3`, reads one `Q_i(x_3)` scalar
per distinct point set, and squeezes `x_4`. Its final IPA opening wire is:

```text
S || challenge xi || challenge z ||
  (L_0 || R_0 || challenge u_0) || ... ||
  (L_{k-1} || R_{k-1} || challenge u_{k-1}) ||
  scalar c || scalar f
```

There is no folded-generator point after `(c, f)` in an ordinary Axiom proof.
The pinned `snark-verifier` BGH19 reader consumes the same Axiom sequence and
then expects one additional compressed folded-generator point. Using that
reader against an unversioned ordinary proof is categorically invalid; using it
against the explicitly augmented wire below is compatible.

## Implemented test-only Poseidon wire

`kagemusha_recursion_adapter::tests::pasta_ipa_poseidon_wire` now exercises the
exact pinned Axiom/snark-verifier boundary without granting production
authority:

1. generate an ordinary `ParamsIPA<EqAffine>` Halo2 proof using the
   circuit-native Poseidon transcript;
2. run complete native Halo2 verification with `snark-verifier`'s
   `system::halo2::strategy::ipa::SingleStrategy`, which both checks the proof
   and returns the folded canonical generator;
3. append exactly one canonical compressed `EqAffine` point to the ordinary
   proof bytes;
4. parse the augmented bytes as `IpaAs<EqAffine, Bgh19>` and constrain the full
   PLONK plus IPA residual; and
5. decide the returned accumulator with `IpaDecidingKey`, which recomputes the
   generator fold from every round challenge and the canonical `ParamsIPA`
   generator vector.

A substituted but canonically encoded appended point fails the opening
residual. A substituted accumulator point fails the independent terminal
decision. This closes the proof-wire ambiguity; it does not close the
production recursion gate. The runtime application prover still emits
Blake2b/Challenge255 proofs, the artifact ABI does not yet select this Poseidon
wire, and no opposite-field Pasta-cycle loader exists in the pinned stack.

## Required Axiom IPA abstract-PCS adapter

The adapter must reconstruct every PLONK query and the BGH19 combined opening
claim before evaluating the Axiom opening equation. After transcript parsing,
the residual equation is:

```text
P - v*G_0 + xi*S
  + sum(u_j^-1 * L_j) + sum(u_j * R_j)
  - c*G_folded - c*b*z*U - f*W = O

b = product_i(1 + u_{k-1-i} * x_3^(2^i))
```

`P` is the combined multi-open commitment MSM and `v` its combined evaluation.
The circuit can constrain every term except the claim that `G_folded` is the
correct fold of the canonical generator vector without doing a linear-size
fixed-base MSM.

The versioned recursive proof wire therefore appends one canonical
`G_folded` point after `f`. No later Fiat--Shamir challenge is required: the
point is algebraically bound because the circuit includes `-c*G_folded` in the
opening residual and constrains that residual to the identity. It emits the IPA
accumulator:

```text
(parameter_generation, ipa_k, G_folded, [u_0, ..., u_{k-1}])
```

along with the transcript profile, exact VK/protocol commitment, exact public
statement commitment, and curve/parity identifier. Omitting any round
challenge, accepting a caller-provided accumulator, or exposing only a hash of
this data is unsound. At terminal verification/redemption, the witnessless
decider must recompute `s = compute_s(u, 1)` and require
`G_folded == sum(s_i * G_i)` against the content-addressed canonical
`ParamsIPA` generator set. The folded point is appended immediately after the
ordinary proof transcript in the versioned wire and must be decided at every
terminal path; merely carrying it to the next hop is not acceptance.

To keep hop payload constant, multiple outstanding IPA accumulator claims must
be accumulated into one claim with a separately specified, circuit-native
accumulation protocol. Appending one accumulator per hop is not recursive
compression. The augmented BGH19 wire now has independent native parity and
terminal-decider tests; in-circuit parity and recursive accumulation remain
release gates.

The Pasta commitment cycle needs two current-proof artifact roles. Every
logical transition carries an `EqAffine`/Vesta proof and an `EpAffine`/Pallas
proof for the same exact transition. The next pair closes both parent halves;
it never replaces one current parity with a temporal predecessor proof.
Artifacts bind both VKs, both parameter generations, and the complete 138-limb
compact V5 field-neutral state nested inside ABI-21/V4. Each parity exposes an
exact 64-element public column at authenticated degree 16. The supported
same-scalar-field tuples compile: an `EqAffine` proof can be loaded in an `Fp`
circuit and the reciprocal `EpAffine` proof in an `Fq` circuit. Earlier direct
verifier prototypes measured multi-gigabyte RSS and are retained only as
historical evidence; they are not a runtime or generation fallback.

The production route is the reviewed compact fixed-key split described by
`paired_deferred_verifier`, not the generic `Halo2Loader` fallback. Its
native-scalar half must derive every transcript challenge and residual
coefficient, its reciprocal native-point half must constrain the identical
proof/VK point stream and complete MSM, and both halves must bind the same exact
field-neutral SHA-256 identity. The terminal path must additionally decide
`U == MSM(h_coeffs(xi), params.generators)`; checking only the 38-term PLONK
opening residual is not an IPA decision. Native/in-circuit transcript parity,
substitution tests, both outer proofs, recursive accumulation, and both terminal
decisions are mandatory before `CircuitVerifierUnavailable` can be removed.

## ABI-21 and artifact V4 contract

The current contract is bridge ABI `21`, manifest schema
`kagemusha.offline.recursive_spend.artifact_manifest.v4`, proof backend
`halo2/ipa-pasta-cycle-compact-v5`, and transcript profile
`kagemusha-pasta-cycle-poseidon-compact-v5`. These values carry no mode field. The two
fixed recursive circuit roles are:

- registry role `kagemusha_recursive_step_eq_v4_verifier_record` with circuit
  `kagemusha-recursive-spend-step-eq-compact-layout-v5`, the EqAffine/Vesta
  half; and
- registry role `kagemusha_recursive_step_ep_v4_verifier_record` with circuit
  `kagemusha-recursive-spend-step-ep-compact-lineage-v5`, the EpAffine/Pallas
  half.

Both registry records use backend `halo2/ipa`. They must be selected atomically,
remain independently keyed, and agree with one authenticated release's
activation window and proof-pair limit.

`KagemushaRecursiveSpendStateBoundaryV2` still crosses the field boundary as a
compact V5 layout version followed by all 138 explicit little-endian `u32`
result-state limbs, including the statement's append-only
`next_zero_leaf_index`. ABI 21 and manifest V4 remain the only lifecycle, but
keys, bootstrap witnesses, proofs, manifests, and schema hashes from the former
large layout are incompatible and must not be reused. The nested compact
profile fixes the single public-instance column at 64 field elements and degree
16. Each authenticated `KagemushaStepCircuitParamsV4` still binds the
parameter-layout version, IPA degree, advice and lookup-advice columns by
phase, fixed and instance columns, lookup width, exact public-input length,
minimum unusable rows, and parent-proof byte bound. Its default value is an
invalid sentinel; neither local configuration nor FFI input may replace the
value authenticated by the manifest.

The compact `proof_step_count` is runtime witness data. StepEq copy-constrains
that public cell to the exact step field inside the native operation relation
and range-checks it as a `u32`; it must never be assigned through a dynamic
`assert_is_const`. Key generation uses step one, so placing that value in a
fixed column would make step two and every later recursive transition
incompatible with the authenticated proving and verifying key. Release
validation therefore includes a real step-one-to-step-two proof chain under
the same final key pair, followed by terminal verification.

Bootstrap payload version 5 also authenticates the exact cumulative
virtual-region breakpoints captured from the final key-generation circuit for
each phase. Decoding rejects a phase-count mismatch, non-increasing or
out-of-domain segments, and more boundaries than the authenticated advice
columns permit. Live proof construction installs those breakpoints into a
witness-generation-only builder and verifies that its advice and lookup
population fits them. It does not retain or reconstruct the key-generation
constraint graph beside a processed proving key.

Full-size key generation uses consuming verifier- and proving-key entrypoints.
Their post-synthesis extractor copies or validates the populated breakpoints,
then releases the owned key-generation circuit before fixed and permutation
key assembly. Proving-key assembly reuses the supplied verifier key's domain;
permutation construction moves out only the completed mapping so union-find
scratch is released, keeps omega and delta factors separate instead of
materializing an n-by-m grid, and converts one coefficient column at a time.
Empty-bootstrap assembly keeps an identity permutation implicit and
materializes union-find state only on the first nontrivial copy. Verifier-key
construction builds, commits, and drops one permutation polynomial at a time,
avoiding retention of the complete reviewed 534-column permutation inventory.
Those streamed commitments complete before assigned fixed columns and
bit-packed selectors expand into degree-sized field polynomials. The ordinary
borrowed Halo2 entrypoints remain available, and the consuming paths produce
byte-identical compressed and uncompressed processed keys.

The proof engine also takes ownership of that witness-only circuit and its
single processed proving key. It releases the circuit immediately after
witness synthesis, then releases the domain-sized fixed-value and permutation
Lagrange preprocessing as soon as their commitments are complete. The
remaining proof stages consume the key and return its embedded verifying key;
the caller finalizes the transcript, immediately verifies the new proof with
that returned key, and then drops it. This avoids borrowing a live circuit and
the complete processed key through proof finalization or reparsing a duplicate
verifier domain alongside them.

The consuming quotient evaluator preserves the ordinary prover's constraint
and Horner order but transforms only one degree-sized copy-permutation sigma
chunk at a time. For the reviewed 534-column permutation this keeps at most two
sigma cosets live instead of retaining the full permutation inventory.
Instance conversion is deferred until its Lagrange allocation can be
consumed, and configure metadata, selector polynomials, and the evaluator graph
are released at their final use. Borrowed and consuming proofs are regression-
checked byte for byte. Parameter construction also evicts each one-shot
projective FFT cache after the corresponding Eq/Ep transform, removing roughly
12 MiB of worker-lifetime retention on ARM at k16. Evaluation domains eagerly
initialize only the base FFT table and leave unused 2n/4n tables lazy, avoiding
roughly 24 MiB at k16. Quotient parts are written directly into their final
interleaved polynomial instead of through a transpose allocation, avoiding
roughly 8 MiB, and cached recursive FFT scratch is evicted before h-piece MSMs,
removing another roughly 8 MiB from that overlap. The outer lifecycle remains
in a disposable one-worker pool. Large MSMs alone acquire process-wide
admission before scalar/base preprocessing and run in a fixed two-worker
window pool, bounding concurrent preprocessing, window buckets, and allocator
caches without changing accumulator order. The checked static admission
estimate is 9,747,562,496 bytes (9.078125 GiB), not a physical RSS prediction;
it remains below the reviewed 12 GiB exact-profile preflight ceiling, and the
16 GiB supervisor remains authoritative.

Each live V4 step carries the exact operation vector, ordered parent state and
lineage slots, post-proof and branch folds, deferred-equation audit words, and
the live selector derived from that authenticated layout. Missing parent slots
use the manifest-bound final-key selector-zero bootstrap witness; they are not
host-created zero placeholders. `KagemushaPastaCycleProofEnvelopeV4` binds the
state boundary, both ordered circuit identifiers, artifact generation,
authenticated manifest SHA-256, both parameter generations, both inline
circuit-parameter identities, both processed verifier-key payload SHA-256
values, and the canonical opaque Eq/Ep proof pair. There is no parity selector:
the pair and both terminal IPA decisions remain inseparable.

A V4 manifest contains exactly the Eq profile followed by the Ep profile. Each
profile contains exactly four external files, in order: `ParamsIPA`, processed
proving key, processed verifying key, and the final-key selector-zero bootstrap
witness. The external cryptographic inventory is exactly eight files. Circuit
parameters are authenticated inline in the two profiles and digest-bound into
each `KRV4KEY` header; they are not additional streamed artifacts. Every
descriptor records both framed-file and unframed-payload lengths and SHA-256
digests, so a role header cannot disguise duplicated or substituted material.
Each file is content-addressed and bounded, while the release additionally
binds its source revision and tree, chain, asset and scale, issuance window,
measured proof bounds, physical-device evidence, independent review, signed
release attestation, and canonical top-up-finality roster. A generation label
is not a trust anchor.

The supported two-stage packager is:

```text
cd <clean-checkout>
SOURCE_COMMIT=$(git rev-parse --verify 'HEAD^{commit}')
git verify-commit "$SOURCE_COMMIT"
python3 -I scripts/build_kagemusha_v4_candidate_bundle.py --root "$PWD"
# Require source_commit in the build report to equal $SOURCE_COMMIT.

python3 scripts/run_kagemusha_v4_generation.py \
  --resource-report <new-resource-report-directory> -- \
  <binary_path-from-sealed-build-report> \
  generate-candidate \
  --out-dir <new-directory> \
  --chain-id <chain> --asset-definition-id <asset> --asset-scale <u32> \
  --generation <id> --parameter-generation <id> \
  --source-commit <40-lower-hex> --source-tree-sha256 <64-lower-hex> \
  --activation-height <u64> --withdrawal-height <u64> \
  --step-eq-circuit-params <canonical-norito-file> \
  --step-ep-circuit-params <canonical-norito-file> \
  --topup-finality-roster <canonical-norito-file>

<binary_path-from-sealed-build-report> \
  finalize-release \
  --candidate-dir <generated-candidate> \
  --out-dir <new-final-directory> \
  --release-policy <canonical-norito-file> \
  --release-attestation <canonical-norito-file> \
  --benchmark-evidence <exact-file> \
  --cryptographic-review <canonical-signed-norito-file>
```

Direct unsupervised `generate-candidate` execution is rejected. The launcher
holds the per-user heavy-job lock shared with the strict TLAPS runner and applies
a bounded polling ceiling at the lower of 16 GiB or half of physical RAM, with a
target 50 ms cadence and 2-second per-inspection timeout. Inspection failure is
terminal and a supervisor lifeline cleans the exact process group; sampling is
scheduled from completed probes so a delayed macOS inspection cannot create a
catch-up storm. This portable userspace polling is not an operating-system hard
allocation limit. Generation also requires at least 16 GiB free on the pinned
disk-backed output filesystem before it creates the two raw proving-key spools
and each framed artifact copy.
Every sample reuses one process snapshot for memory accounting and detection of
same-user TLAPM, Isabelle, Poly/ML, or Kagemusha work outside the owned group.
A conflict terminates only the owned generation group with status 74, and a
final exclusion check runs before successful candidate finalization so a late
direct job cannot produce valid evidence.
The launcher writes owner-private JSONL and summary evidence. A lower
`--max-memory-gib` is accepted; the ceiling cannot be raised. Its one-shot
inherited launch capability prevents stale environment markers from bypassing
the wrapper, but is not a security boundary against a malicious same-UID
process. The launcher injects an unguessable per-run staging id; after every
normal, failed, signalled, or memory-limited return it securely removes only
owner-private residue carrying that exact id. Build the binary first: the
launcher accepts only the prebuilt bundle
executable followed by `generate-candidate`, so Cargo and rustc are never
included in the 16 GiB process group. Finalization and candidate validation do
not require this generation guard. Commit-signature verification is the
separate Git step above, and the returned `source_commit` must equal that
verified commit. The sealed build helper requires the same clean exact source
identity before, during, and after the locked release build, sanitizes ambient
compiler controls, and returns the exact source and binary digests. Its 24 GiB
installed-memory floor is compiler-build admission, not an OS-hard allocation
limit.

For diagnostic RSS calibration on a dirty or unsigned development tree, build
and run the separate non-shipping benchmark. It executes the complete compact
k16 key-generation, bootstrap, live-proof, and verification lifecycle, streams
both proving keys to anonymous files, and emits only validated byte counts. It
cannot frame, publish, or promote a candidate, and its resource report is not
release evidence:

```text
cargo build --release -p iroha_core \
  --features kagemusha-generation-memory-lab \
  --bin kagemusha_recursive_spend_v4_memory_benchmark --jobs 1

python3 scripts/run_kagemusha_v4_generation_benchmark.py \
  --resource-report <new-diagnostic-report-directory> -- \
  target/release/kagemusha_recursive_spend_v4_memory_benchmark \
  measure-compact-k16
```

Use the optimized binary for calibration. An `opt-level=0` debug build derives
the same deterministic public parameters and byte geometry, but its sequential
hash-to-curve setup is intentionally slow and is not representative runtime
evidence. Proof payload bytes use fresh prover randomness and need not match
between runs.

Generation also computes every ParamsIPA, processed verifier-key, and processed
proving-key length from the authenticated circuit shape before allocating IPA
parameters. The reviewed compact profile is degree 16 with `[443]` advice,
`[47, 0, 0]` lookup-advice, one fixed, and one instance column. The two trailing
zero lookup phases are canonical `BaseCircuitBuilder` output, not allocated
advice phases. Its exact per-parity encodings are 4,194,372 bytes for ParamsIPA,
35,018 bytes for the processed VK, and 4,594,903,830 bytes for the processed PK.
Proving keys serialize directly into bounded owner-private staging files and are framed by streaming reads; Eq and Ep
processed keys are never retained together or copied through a release-sized
`Vec`. The final verifier- and proving-key circuits are consumed after
post-synthesis breakpoint extraction or validation and before key assembly.
Generation then stages the exact processed key before handing its owned value
and the witness-only calibration circuit to the consuming prover. The 5 GiB
PK role cap, 12 GiB exact-profile preflight ceiling, and 16 GiB aggregate
generation guard are fixed. The complete
outer generation lifecycle runs inside a disposable one-worker Rayon pool, so
FFT and quotient scratch cannot multiply the resident key and worker-local FFT
caches are released at the end of the attempt. Large MSMs use the process-wide
admitted two-worker window pool described above; their fixed accumulator order
is unchanged. These are safety ceilings, not operator-tunable capacity targets.
The dedicated physical degree-16 run and its observed peak-RSS evidence are
still pending; the checked estimate and userspace guard are not substitutes for
that measurement.

Candidate generation records the clean source and exact inline Eq/Ep circuit
parameters and emits the eight role-separated artifacts. The candidate is not
an approved release. The review file keeps the historical
`cryptographic-review.evidence` name, but its content is not opaque text. It is
the canonical Norito
`KagemushaRecursiveSpendCryptographicReviewEvidenceV4` envelope. Every reviewer
signs the exact domain-separated payload containing the immutable candidate
digest and release identity, an approved decision, the nonzero retained-report
digest, the exact eight artifact roles, and the fixed ordered six-check matrix
with distinct nonzero evidence digests. All checks must pass. Reviewer keys are
strictly ordered, must satisfy the configured cryptographic-review policy, and
must exactly equal the cryptographic-review signer set in the release
attestation. Plain strings, non-canonical encodings, rejected or incomplete
reviews, substituted candidates, duplicate identities, and signature or signer
set mismatches fail closed.

Finalization authenticates that review, the policy, and the attestation, binds
the exact evidence files, and rechecks the staged bytes before publishing a new
immutable directory. Runtime and proof-envelope validation consume the
canonical Norito bytes; JSON remains an operator view and is never re-encoded
into a trust anchor. A partial candidate or finalization failure cannot expose
an active generation.

The bridge's bounded V4 ingestion authenticates headers, descriptors, framed
and payload hashes, inline circuit-parameter identities, and the exact
eight-role order before atomically installing a generation. Successful
installation can permit construction of an authenticated backend, but it never
authorizes proof admission by itself. Torii exposes that distinction through a
required nullable `artifact_set` and `proof_backend_available`. The artifact
set binds generation, manifest, release-policy and release-attestation digests,
issuance window, proof-pair bound, and asset scale to both exact recursive
verifier records. A null set requires both records and backend construction to
be unavailable with exactly one `recursive_v4_registry_unavailable` or
`recursive_v4_registry_malformed` blocker; a non-null set forbids both.

Lineage admission is selected by the exact authenticated release rather than a
process-wide flag. `recursive_lineage_supported` is true only when the non-null
artifact set, distinct active Eq/Ep records, and production backend are all
present; `recursive_lineage_unavailable` is its exact inverse. `ready` is true
only when every typed blocker is absent. Symbol presence or partial artifact
ingestion remains insufficient: transaction admission also authenticates the
release-qualified consensus records, immutable startup catalog, and consensus
release record.

## Branch-bound recipient and change proofs

A split transition produces distinct recipient and change proofs from one
shared split intent. Both branch statements bind exact conservation and the
same split binding digest, for example:

```text
split_root = H(
  split_domain,
  transition_binding,
  recipient_output_commitment,
  change_present,
  change_output_commitment
)
```

The recipient bundle binds branch index 0, its output note, and its disjoint
branch claims. The optional change bundle binds branch index 1, its different
output note, and its own disjoint claims. Their public statements and recursive
proofs are therefore distinct even though both authenticate the same split
intent and `split_binding_digest`. Tampering with either branch selector,
output, or claim history must invalidate that branch's proof. Ledger
transition-choice markers and branch-specific nullifiers remain necessary to
prevent mixing branches from alternative splits.

Peer hops are capped at eight independently of the 64-level branch-path
capacity. A peer split increments both the branch depth and peer-hop count;
redemption-change extends the branch without incrementing peer hops. Canonical
ingress, Torii readiness, maintained clients, and the Eq/Vesta transition
relation all enforce the exact eight-hop ceiling.

Implementations may share internal witness computation, transcript preparation,
or parent-proof verification while constructing the two proofs, but the wire
contract never reuses one proof/public statement for both independently
spendable branches.
