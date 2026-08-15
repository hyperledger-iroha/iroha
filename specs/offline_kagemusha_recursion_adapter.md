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
exact 66-element public column at authenticated degree 17. The supported
same-scalar-field tuples compile: an `EqAffine` proof can be loaded in an `Fp`
circuit and the reciprocal `EpAffine` proof in an `Fq` circuit. Earlier direct
verifier prototypes measured multi-gigabyte RSS and are retained only as
historical evidence; they are not a runtime or generation fallback.

The production route is the reviewed compact fixed-key split described by
`paired_deferred_verifier`, not the generic `Halo2Loader` fallback. Its
native-scalar half must derive every transcript challenge and residual
coefficient, its reciprocal native-point half must constrain the identical
proof/VK point stream and complete MSM, and both halves must bind the same exact
V2 compiled-protocol identity. The value-free compiled-protocol structure hash
remains V1. Identity V2 absorbs its domain, parity, V1 structure digest, point
count, every canonical compressed verifier-key point, and transcript initial
state with Poseidon. Every 32-byte compressed point is split injectively into
two little-endian `u128` field elements, so neither Pasta direction loses the
compressed sign bit or aliases coordinates by modular reduction. A short
domain/version wrapper then SHA-256-hashes the canonical Poseidon field
encoding in one compression block. The V6 deferred-equation audit independently
uses the same injective compressed-point-to-Poseidon construction and its own
one-block SHA-256 domain. Native, scalar-circuit, and reciprocal-circuit
commitments must match exactly. The terminal path must additionally decide
`U == MSM(h_coeffs(xi), params.generators)`; checking only the 38-term PLONK
opening residual is not an IPA decision. Native/in-circuit transcript parity,
substitution tests, both outer proofs, recursive accumulation, and both terminal
decisions are mandatory before `CircuitVerifierUnavailable` can be removed.

## ABI-21 and artifact V4 contract

The current contract is bridge ABI `22`, manifest schema
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

`KagemushaRecursiveSpendStateBoundaryV5` crosses the field boundary as the
canonical V5 layout version followed by all 138 explicit little-endian `u32`
result-state limbs, including the statement's append-only
`next_zero_leaf_index`. ABI 21 and manifest V4 remain the only lifecycle, but
keys, bootstrap witnesses, proofs, manifests, and schema hashes from the former
large layout are incompatible and must not be reused. The nested compact
profile fixes the single public-instance column at 66 field elements and degree
17. The common semantic header occupies elements `[0, 19)`, the 38-element IPA
accumulator occupies `[19, 57)`, Eq and Ep deferred-audit words occupy
`[57, 61)` and `[61, 65)`, and the live selector is element 65. Each
authenticated `KagemushaStepCircuitParamsV4` still binds the
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
avoiding retention of the complete reviewed 297-column permutation inventory.
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
chunk at a time. For the reviewed 297-column permutation this keeps at most two
sigma cosets live instead of retaining the full permutation inventory.
Instance conversion is deferred until its Lagrange allocation can be
consumed, and configure metadata, selector polynomials, and the evaluator graph
are released at their final use. Borrowed and consuming proofs are regression-
checked byte for byte. Parameter construction also evicts each one-shot
projective FFT cache after the corresponding Eq/Ep transform, removing its
domain-sized worker-lifetime retention. Evaluation domains eagerly initialize
only the base FFT table and leave unused 2n/4n tables lazy. Quotient parts are
written directly into their final interleaved polynomial instead of through a
transpose allocation, and cached recursive FFT scratch is evicted before
h-piece MSMs. The outer lifecycle remains
in a disposable one-worker pool. Large MSMs alone acquire process-wide
admission before scalar/base preprocessing and run in a fixed two-worker
window pool, bounding concurrent preprocessing, window buckets, and allocator
caches without changing accumulator order. The checked phase-aware admission
estimate is 53,108,563,136 bytes (49.4612 GiB), not a physical-memory
prediction. It includes the virtual graph, synthesis-local map, physical advice
columns, processed key, and allocator reserve that overlap during the consuming
prover. It is 7,020,979,008 bytes below the reviewed 56 GiB exact-profile
preflight ceiling, and the 64 GiB / half-physical-RAM supervisor remains
authoritative. The
superseded precompact
diagnostic peaked at 4,998,922,240 bytes; a fresh final-source k17 probe must
replace that physical measurement.

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

The release candidate boundary requires at least four public validators in
every supplied top-up-finality roster window. The general roster data model
remains capable of representing smaller development fixtures, but such a
fixture is not admissible as ABI-21 release evidence.

Runtime qualification keeps its decoded working set beneath the non-raiseable
256 MiB catalog budget. It never materializes a processed proving key: each
multi-gigabyte PK is authenticated with fixed 64 KiB scratch, checked against
the exact processed-key geometry, and its bounded embedded-VK prefix is hashed
and matched to the separately parsed VK. Full `ProvingKey` parsing remains
confined to generation and proving paths that actually consume the polynomial
vectors. The receipt verifier then parses the other six bounded roles once and
terminally decides both stored Eq/Ep pairs with that verifier set.

The supported two-stage packager is:

```text
mkdir -m 700 <private-release-input-parent>
cargo run --locked --target-dir <external-cargo-target> \
  -p iroha_kagami --bin kagami -- \
  kagemusha prepare-release-circuit-params-v4 \
  --output-dir <private-release-input-parent>/circuit-params-v4

cd <clean-checkout>
SOURCE_COMMIT=$(git rev-parse --verify 'HEAD^{commit}')
# Configure the reviewer's global gpg.ssh.allowedSignersFile to one absolute,
# owner-controlled, single-key policy. Optionally configure the corresponding
# gpg.ssh.revocationFile. Repository-local signature settings are ignored.
python3 -I scripts/kagemusha_source_tree_seal.py descriptor \
  --root "$PWD" > <private-reviewed-source-closure.json>
python3 -I scripts/build_kagemusha_v4_candidate_bundle.py \
  --root "$PWD" \
  --cargo <absolute-reviewed-cargo-binary> \
  --cargo-sha256 <reviewed-cargo-binary-sha256> \
  --rustc <absolute-reviewed-rustc-binary> \
  --rustc-sha256 <reviewed-rustc-binary-sha256> \
  --cargo-home <absolute-private-cache-only-cargo-home> \
  --target-dir <new-external-cargo-target> \
  --reviewed-source-closure <private-reviewed-source-closure.json> \
  --reviewed-source-closure-sha256 <reviewed-descriptor-sha256> \
  --authenticated-source-seal-projection \
    <private-authenticated-source-seal-projection.json> \
  --authenticated-source-seal-projection-sha256 \
    <reviewed-authenticated-source-seal-projection-sha256> \
  > <sealed-build-report.json>
# Require source_commit in the build report to equal $SOURCE_COMMIT.

python3 scripts/run_kagemusha_v4_generation.py \
  --resource-report <new-resource-report-directory> -- \
  <binary_path-from-sealed-build-report> \
  generate-candidate \
  --out-dir <new-directory> \
  --network-id <canonical-network-id> --asset-definition-id <asset> --asset-scale <u32> \
  --generation <id> --parameter-generation <id> \
  --source-commit <40-lower-hex> --source-tree-sha256 <64-lower-hex> \
  --activation-height <u64> --withdrawal-height <u64> \
  --step-eq-circuit-params <private-release-input-parent>/circuit-params-v4/step-eq-circuit-params.norito \
  --step-ep-circuit-params <private-release-input-parent>/circuit-params-v4/step-ep-circuit-params.norito \
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

`prepare-release-circuit-params-v4` is the only supported constructor for the
reviewed first-release Eq/Ep parameter inputs. It validates the centralized
profile, canonical-Norito round-trips it, writes separate owner-private Eq and
Ep files into one private staging directory, syncs both files and that
directory, and makes the complete pair visible with one no-replace directory
rename. The command refuses an existing output directory and reports both the
raw file SHA-256 and domain-separated circuit-parameter SHA-256. A failed
parent-directory sync after visibility is reported as `commit-uncertain` with
exit status 75, never as a safely retryable failure.

The source seal is a first-release clean-only contract. Descriptor emission,
sealed build, generation, validation, and finalization all reject a nonempty
tracked diff, any untracked or ignored file, an absent or nonempty
tracked-gitlink directory, a root `Cargo.lock` that is not exactly one tracked
mode-`100644` index entry or differs from its separate V1 digest binding, or a
commit whose signature cannot be verified locally. The legacy
`ignored_cargo_lock_*` descriptor field names remain unchanged for V1 wire
compatibility but bind that tracked file. `source_repo_dirty` remains in the
closure only as an explicit invariant and must be `false`; the tracked-diff and
untracked manifest digests must both identify empty byte strings.

For the production V4 wire, the sealed build also authenticates the canonical
source-seal projection and the exact Cargo and rustc binaries. The builder sets
absolute `CARGO`/`RUSTC` paths and exports
`KAGEMUSHA_BUILD_REVIEWED_CARGO_BINARY_SHA256` and
`KAGEMUSHA_BUILD_REVIEWED_RUSTC_BINARY_SHA256`; `build.rs` hashes those actual
files before embedding their digests. The candidate manifest, qualification
receipt, cryptographic-review and release-attestation subjects, promotion
record and reports, and catalog qualification seal all bind the same non-zero
three-digest identity. Existing V4 wire artifacts predate this identity and are
deliberately invalid: regenerate candidates and qualification receipts, then
repeat review, attestation, promotion, and runtime sealing.

The source-sealed binary always starts its own fail-closed footprint monitor;
the launcher adds the host-global lifecycle, publication, and evidence boundary.
After pinning the executable (and creating the private execution copy on
Darwin), the launcher invokes its read-only `memory-capacity-v1` operation
through the same supervisor-death lifeline. That versioned record is the single
authority for the effective host/container capacity, absolute maximum,
enforcement profile, and half-cap policy. The launcher may only lower the
returned ceiling and passes the exact byte result back to generation. It also
holds the per-user heavy-job lock shared with the strict TLAPS runner and applies
a bounded polling ceiling at that Rust-derived limit. On
macOS, the 250 ms runtime loop enumerates only the owned group with
`proc_listpgrppids`, validates stable BSD identity and ownership around
`proc_pid_rusage`, and enforces the greater of RSS or physical-footprint high
water. Enumeration saturation, identity reuse, ownership drift, and kernel API
failure are terminal. A threshold crossing stops the group before one final
scoped measurement, then kills and reaps only that group. The direct child's
kernel `wait4` peak RSS remains an independent final gate. This portable
userspace polling is not an operating-system hard allocation limit.
The query, generation, and publisher execute under one exact child-environment
allowlist: fixed `HOME`, `LANG`, `LC_ALL`, and system-only `PATH`, plus `TMPDIR`
derived from the admitted output parent for generation/publication. Ambient
`LD_*`/`DYLD_*` loader hooks, tool-resolution paths, allocator/runtime knobs,
SDK discovery, and Python/Rust override variables are not forwarded into the
source-sealed executable.
Generation also requires at least 16 GiB free on the pinned
disk-backed output filesystem before it creates the two raw proving-key spools
and each framed artifact copy.
Admission and final-success gates use a full-host process snapshot to detect
same-user TLAPM, Isabelle, Poly/ML, or Kagemusha work outside the owned group.
The runtime loop never invokes global `ps`, so memory or APFS pressure cannot
block enforcement. A conflict terminates only the owned generation group with
status 74, and the final exclusion check prevents a late direct job from
producing valid evidence.
The launcher writes owner-private JSONL and summary evidence, including the
exact Rust memory-capacity record. A lower `--max-memory-gib` is accepted; the
ceiling cannot be raised. Executable bytes and path identity are revalidated
before and after the query and every later bundle operation. The launcher
injects an unguessable per-run staging id; after every
normal, failed, signalled, or memory-limited return it securely removes only
owner-private residue carrying that exact id. Build the binary first: the
launcher accepts only the prebuilt bundle
executable followed by `generate-candidate`, so Cargo and rustc are never
included in the guarded process group. Finalization and candidate validation
start the same mandatory in-process monitor directly. Commit-signature verification is the
separate Git step above, and the returned `source_commit` must equal that
verified commit. The sealed build helper requires the same clean exact source
identity before, during, and after the locked release build, sanitizes ambient
compiler controls, and returns the exact source and binary digests. Its 24 GiB
installed-memory floor is compiler-build admission, not an OS-hard allocation
limit.

For diagnostic memory calibration on a dirty or unsigned development tree, build
and run the separate non-shipping benchmark. It executes the complete compact
k17 key-generation, bootstrap, live-proof, and verification lifecycle, streams
both proving keys to anonymous files, and emits only validated byte counts. It
cannot frame, publish, or promote a candidate, and its resource report is not
release evidence:

```text
cargo build --release -p iroha_core \
  --features kagemusha-generation-memory-lab,dev-tools \
  --bin kagemusha_recursive_spend_v4_memory_benchmark --jobs 1

python3 scripts/run_kagemusha_v4_generation_benchmark.py \
  --resource-report <new-diagnostic-report-directory> \
  --scratch-parent <owner-private-disk-directory> -- \
  target/release/kagemusha_recursive_spend_v4_memory_benchmark \
  measure-compact-k17
```

Use `probe-compact-k17-shape` in place of `measure-compact-k17` to rerun the
populated four-role closure diagnostic. The guard admits only those two exact
operations and rejects extra arguments. On Darwin, both diagnostic operations
use the production runner's 250 ms process-group sampling and enforce the
greater of aggregate RSS or physical footprint. Other hosts retain the
process-group RSS policy because they do not expose the Darwin footprint
counter.

Use the optimized binary for calibration. An `opt-level=0` debug build derives
the same deterministic public parameters and byte geometry, but its sequential
hash-to-curve setup is intentionally slow and is not representative runtime
evidence. Proof payload bytes use fresh prover randomness and need not match
between runs.

Generation also computes every ParamsIPA, processed verifier-key, and processed
proving-key length from the authenticated circuit shape before allocating IPA
parameters. The reviewed compact profile is degree 17 with `[220]` advice,
`[25, 0, 0]` lookup-advice, one fixed, and one instance column. The two trailing
zero lookup phases are canonical `BaseCircuitBuilder` output, not allocated
advice phases. Its complete configured inventory is 411 advice columns, nine
base fixed columns, 330 selectors, 297 equality/permutation columns, 339 fixed
polynomials, and 636 commitments. Its exact per-parity encodings are 8,388,676
bytes for ParamsIPA, 20,362 bytes for the processed VK, and 5,347,763,078 bytes
for the processed PK.
Constant-copy constraints retain one exact field value per distinct constant
in an ordered map and every constrained `ContextCell`, including duplicate
edges, in that constant's bucket. A one-entry last-constant cache avoids the
ordered lookup for repeated runs. Keygen shape calculation reads the distinct
map length directly. Physical assignment sorts each complete cell bucket and
then traverses constants in field order, reproducing the former flat
`(constant, ContextCell)` lexicographic sequence exactly for fixed assignment
and permutation constraints. For `E` constant equalities, the cell payload is
`8E` bytes plus the bounded distinct-constant index instead of `40E` bytes on
the reviewed 64-bit Pasta target. The earlier complete populated graph recorded
912,209,172 edges, so the representation removes 29,190,693,504 bytes of flat
field-element payload before allocator capacity and compression effects.
Virtual advice-cell coordinates use a nonzero packed 64-bit word: a pinned
two-bit canonical region tag, a checked 29-bit context index, a checked 32-bit
row index, and one niche bit. The constructor and accessors remain
`usize`-based, reject unknown tags and out-of-range coordinates explicitly, and
preserve the former string/context/row equality, ordering, debug, and legacy
`usize` hash semantics. Both `ContextCell` and `Option<ContextCell>` are eight
bytes on 64-bit targets.
Context advice retains exact `Assigned::{Zero, Trivial, Rational}` semantics in
a dense numerator/value vector, a packed `Zero` bit mask, and sorted checked-u32
rational positions paired with a sparse denominator vector. Random access uses
binary search over rational positions, while assignment, iteration, and debug
formatting merge the sparse inventory sequentially. Rational witnesses,
including a zero denominator, are reconstructed without evaluation before
Halo2 assignment, so backend batch-inversion timing and circuit synthesis order
are unchanged. For `N` advice values and `R` rationals, the written backing
storage is `32N + ceil(N / 8) + 36R` bytes instead of 72 bytes per value. The
220-column profile contains at least 28,702,798 virtual advice values, and the
reviewed rational-producing primitives contribute at most one denominator per
eight values. Written storage therefore falls by at least 1,015,361,506 bytes
before the additional packed-cell equality and lookup savings. The guarded
probe reports mask bytes, active rational positions, denominators, every advice
backing-vector capacity, constant bucket count and cell capacity, and
last-constant cache hits versus ordered-index lookups to detect allocation or
construction-time cliffs. The two cache counters are schedule-local diagnostic
performance data, not reproducibility evidence or promotion predicates. These
are in-memory representation changes only; circuit shape and proof bytes are
unchanged.
The reciprocal Pasta scalar loader also recognizes exactly one assigned
product with coefficient one and constant zero, the shape emitted by
`Halo2Loader` for assigned-by-assigned multiplication. `FpChip::mul` already
returns a three-limb `ProperCrtUint` with the same proper-limb residue
invariant as the old trailing carry operations, so the loader returns that
product directly instead of multiplying it by one and adding zero. No
other sum/product shape is specialized. Unlike the storage changes above,
this algebraic identity deliberately changes advice, lookup, selector, and
copy-constraint placement. All prior Eq/Ep breakpoints, processed VK/PK
payloads, compiled-protocol identities, bootstrap witnesses, proofs, artifact
digests, and source-seal evidence are therefore invalid and must be regenerated
before release qualification.
Proving keys serialize directly into bounded owner-private staging files and
are framed by streaming reads; Eq and Ep
processed keys are never retained together or copied through a release-sized
`Vec`. The final verifier- and proving-key circuits are consumed after
post-synthesis breakpoint extraction or validation and before key assembly.
Generation then stages the exact processed key before handing its owned value
and the witness-only calibration circuit to the consuming prover. The 5 GiB
PK role cap, 56 GiB exact-profile preflight ceiling, and 64 GiB /
half-physical-RAM aggregate generation guard are fixed. The complete
outer generation lifecycle runs inside a disposable one-worker Rayon pool, so
FFT and quotient scratch cannot multiply the resident key and worker-local FFT
caches are released at the end of the attempt. Large MSMs use the process-wide
admitted two-worker window pool described above; their fixed accumulator order
is unchanged. These are safety ceilings, not operator-tunable capacity targets.
The earlier guarded populated-profile probe synthesized all four Eq/Ep
bootstrap/live roles twice and reported `[175]` advice with `[19, 0, 0]`
lookup advice, but it did not contain the authentic final verifying-key point
stream and is superseded for promotion. Authentic generation later reached a
20,154-byte raw protocol-identity SHA preimage: 316 compression blocks required
147,520 rows across the five Table16 lanes, exceeding the 131,063 usable k17
rows, and failed only after roughly 29 minutes of setup. V2 protocol identity
and V6 deferred audit now Poseidon-absorb their full injective point encodings
and feed only their 53-byte domain/version/digest wrappers to SHA-256. The
authentic reciprocal StepEq audit contains 1,867 dense sources; its former
single 313,659-row trace is now split in stable order across three disjoint
lanes whose global accumulator is equality-bound endpoint to start. The
623-source longest lane consumes 104,667 rows, and closing the final endpoint
to the first start enforces exactly the original unsplit identity relation.
The composite builder also checks exact SHA-job and dense-MSM row profiles
before key generation, so an impossible auxiliary layout fails before
expensive setup. A fresh guarded k17 probe must re-establish the final graph's
shape and structure consistency. The 93,120-byte per-role transcript, 186,852-byte
initialization pair, and 191,862-byte maximum pair remain expected values until
that probe and authentic source-sealed candidate generation confirm them;
release finalization, physical-device evidence, and live Taira rollout remain
pending.

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

Production generation, candidate validation, and finalization intentionally
have no fault-injection option. Such an option would add a test backdoor to the
exact release command surface and could contaminate source-sealed evidence.
Negative qualification remains the candidate-preserving `validate-candidate` gate over a
deliberately substituted copy, together with the existing role/header/key
substitution and pre-/post-rename atomic-publication regressions. Those tests
remain the failure-path authority; operators must not simulate them by adding
a production flag.

Kagami publishes both promotion records and prepared activation instruction
files through the same descriptor-relative, no-replace durable-file primitive.
It syncs the private file before rename and the pinned parent afterward. A
failure after the rename is reported as the exact
`iroha.kagami.kagemusha.durable_file_publication.v1` `commit-uncertain` record
with exit status 75; it is never collapsed into ordinary success or a safely
retryable pre-commit error.

The bridge's bounded V4 ingestion authenticates headers, descriptors, framed
and payload hashes, inline circuit-parameter identities, and the exact
eight-role order before atomically installing a generation. Successful
installation can permit construction of an authenticated backend, but it never
enables offline support or changes node readiness. Lineage admission for a
specific online top-up or redemption command is selected by that command's
exact authenticated release rather than a process-wide flag. Transaction
admission authenticates the release-qualified consensus records, any configured
local release cache, and the consensus release record. Missing command material
cannot block startup, `/health`, `/readyz`, or wallet/device peer handoff.

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
