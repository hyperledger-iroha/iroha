# Kagemusha recursive-verifier adapter audit

This note records the proof-system boundary for Kagemusha recursive spend V2.
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
Artifacts bind both VKs, both parameter generations, and the complete 889-limb
field-neutral state. The supported same-scalar-field tuples compile: an
`EqAffine` proof can be loaded in an `Fp` circuit and the reciprocal `EpAffine`
proof in an `Fq` circuit. This is not a trait blocker. It is still structurally
outside the release budget. The fixed direct `Eq/Fp` verifier measured
4,659,490 advice cells at degree 12; a degree-18 outer proof was 7,296 bytes
ordinary and 7,328 bytes after appending the folded generator, with roughly
4 GiB live RSS. The release slot is 1,600 bytes per parity.

The production route must therefore be the reviewed fixed-key split described
by `paired_deferred_verifier`, not the generic `Halo2Loader` fallback. Its
native-scalar half must derive every transcript challenge and residual
coefficient, its reciprocal native-point half must constrain the identical
proof/VK point stream and complete MSM, and both halves must bind the same exact
field-neutral SHA-256 identity. The terminal path must additionally decide
`U == MSM(h_coeffs(xi), params.generators)`; checking only the 38-term PLONK
opening residual is not an IPA decision. Native/in-circuit transcript parity,
substitution tests, both outer proofs, recursive accumulation, and both terminal
decisions are mandatory before `CircuitVerifierUnavailable` can be removed.

## ABI-19 and artifact V3 contract

The fail-closed production contract is explicit even though the complete loader
is not yet available. Native capabilities report bridge ABI `19`, manifest schema
`kagemusha.offline.recursive_spend.artifact_manifest.v3`, proof backend
`halo2/ipa-pasta-cycle-v1`, and transcript profile
`kagemusha-pasta-cycle-poseidon-v1`. They carry no mode field. The two fixed
circuit roles are:

- `kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2`, an EqAffine/Vesta transition
  proof; and
- `kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2`, an EpAffine/Pallas
  transition proof.

`KagemushaRecursiveSpendStateBoundaryV1` crosses the field boundary as a
layout version followed by all 889 explicit little-endian `u32` result-state
limbs. Each proof's public column additionally carries `parent_count` and two
ordered 889-limb parent-state slots; absent slots must be all zero and present
slots must be in canonical order. Per-slot Eq and Ep deferred-equation SHA-256
joins bind the reciprocal scalar and point verifier halves. The lengths and
state-layout markers are validated exactly; no hash stands in for any carried
state. `KagemushaPastaCycleProofEnvelopeV3` binds that boundary, both
ordered Eq/Ep circuit identifiers, artifact generation, the SHA-256 of the
exact authenticated manifest, both `ParamsIPA` generations, both raw
verifier-key payload SHA-256 values, and the ordered Eq/Ep proof pair. There is
no step-parity selector: every transition carries and terminally verifies both
current proofs. A V3 manifest has exactly the Eq profile followed by the Ep profile;
each profile binds exactly one parameters, proving-key, and verifying-key file.
Each descriptor records both the complete framed-file digest and the unframed
payload digest/length, so a role header cannot disguise duplicated key material.
Every file is content-addressed, is at most 256 MiB, and a release additionally
binds its source revision, the SHA-256 of the exact tracked and untracked source
tree, whether that tree was dirty, chain, asset/scale,
activation/withdrawal heights, physical-device benchmark evidence,
cryptographic review, signed release attestation, and the canonical top-up
finality-roster archive. Finality verification accepts the roster only when its
canonical bytes match the SHA-256 selected by the authenticated manifest; a
human-readable generation label is not a trust anchor.

The supported bundle packager is:

```text
cargo run -p iroha_core --bin kagemusha_recursive_spend_v3_bundle -- \
  --out-dir <new-directory> \
  --chain-id <chain> --asset-definition-id <asset> --asset-scale <u32> \
  --generation <id> --parameter-generation <id> --source-commit <40-lower-hex> \
  --source-tree-sha256 <64-lower-hex> --source-repo-dirty <true|false> \
  --activation-height <u64> --withdrawal-height <u64> \
  --benchmark-evidence-sha256 <64-lower-hex> \
  --cryptographic-review-sha256 <64-lower-hex> \
  --release-attestation-sha256 <64-lower-hex> \
  --transition-parameters <file> --transition-proving-key <file> \
  --transition-verifying-key <file> --state-parameters <file> \
  --state-proving-key <file> --state-verifying-key <file> \
  --topup-finality-roster <canonical-norito-file>
```

The command consumes six externally generated and reviewed raw artifacts; it
does not substitute deterministic or runtime key generation for the missing
production prover. It opens each input once with no-follow/nonblocking safety,
rejects non-regular files, empty or oversized inputs, duplicate paths,
hardlinks, duplicate raw payloads, source mutation, non-canonical release
fields, roster gaps, an untrusted output-parent chain, and an existing output
entry before publication. It writes owner-only files into a private random
staging directory, reads every staged file back through held no-follow
descriptors, hashes both raw and complete framed bytes into the 2×3 inventory,
and fsyncs the files and directory. Only then does it promote the complete
directory with one descriptor-relative no-replace rename and fsync the parent.
Runtime and proof-envelope validation consume the exact Norito bytes; JSON is
an operator view and is never re-encoded into a trust anchor. A failure removes
the unpublished staging directory and never exposes a partial output path. The
durable publication corridor currently supports Linux, Android, macOS, and iOS
and fails closed elsewhere. The packager is deliberately an unsigned staging step:
evidence digests and the Norito content address are recorded, but
release-signature authentication happens in the separate release process.
The small file header does not embed the manifest digest because a manifest
that contains framed-file digests would make that content address circular;
proof envelopes bind the final authenticated manifest digest instead.

The bridge exports a capability archive plus bounded, manifest-bound V3
streaming ingestion. Ingestion checks header/descriptor fields plus raw and
framed hashes, but never authorizes proving. Exactly one finalized handle for
each of the six roles must be installed as a single manifest-bound set. Native
installation revalidates the held anonymous files, stores them in canonical
manifest role order, consumes all six handles only after every check succeeds,
and leaves the prior generation unchanged on failure. Rotation is atomic;
in-flight calls retain the selected generation by reference, and digest-guarded
uninstall cannot remove a replacement generation. The capability record names
every missing gate and reports `proof_backend_available = false`; all proof-gated
entrypoints fail closed. Symbol presence and successful ingestion are not
readiness signals. `authenticated_release_envelope` remains an explicit
missing gate until a signer/policy-bound verifier produces the trusted manifest
digest consumed by native verification. `paired_deferred_verifier` covers the
sound fixed-key scalar/point verifier halves and their terminal IPA decision;
`proof_bound_output_membership_witnesses` covers in-proof binding of every
recipient/change output and membership edge. Recursive init must also consume
the verified finality result before the backend can be enabled. The availability
constant may change only in the audited release that supplies those two
cryptographic gates, release-envelope authentication, adversarial substitution
tests, independent review, and physical-device evidence.

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
