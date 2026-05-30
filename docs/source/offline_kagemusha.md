# Offline Kagemusha

Kagemusha is the default direction for offline-offline payments. Nodes expose it
through `settlement.offline.kagemusha_enabled`, which defaults to `true`; the
legacy bearer-audit path remains available only as an explicit migration fallback
through `settlement.offline.kagemusha_force_legacy`, which defaults to `false`.
The real chain execution fixture asserts those defaults before running a
Kagemusha transfer, so default-disabled regressions are caught by focused core
tests.

The current hardening keeps the production `AuditOfflineNote` lineage anchored to
the original online-to-offline topup. Audit submitters no longer need to be the
note account, but every input claim must match the sender key certificate that was
anchored by an earlier topup or audit output, and the exact input-claim hash
must have been issued by that lineage. A claim mutation under a legitimately
issued certificate is rejected before recursive proof verification. Audit outputs
are now one-to-one with public output commitments, and public asset definitions
and amount totals must conserve before proof verification. Output certificates are signature
checked against the output account they claim before their lineage is recorded,
so a relayer can submit audit metadata without becoming a certificate issuer.
Audit output key certificates must also be fresh one-use certificates: any
output certificate whose replay key was already anchored by an online-to-offline
topup or prior audit output is rejected before recursive proof verification.
Note commitments are one-use across both topup issue and audit-output replay
domains, so a commitment created by an online-to-offline topup cannot be
reintroduced as a P2P bearer output, and a prior P2P output commitment cannot be
loaded again through a later topup.
Offline recursive audit/redeem proof envelopes must bind the active verifier-key
commitment exactly and carry empty auxiliary bytes, matching the canonical
transparent Halo2 IPA prover output. Their verifier records must also carry
matching inline key bytes, key length, namespace, active circuit/version index,
schema hash, commitment, canonical `offline-note-recursive` circuit family, and
proof-size cap before proof verification starts. A verifier record and envelope
that agree on a non-canonical circuit id are rejected before backend
verification.
Inline verifier-key bytes and verifier-key id names must be non-empty, so a
verifier record cannot silently anchor to absent key material.
Focused core coverage executes a real online-to-offline topup, an audit
submitted by an unrelated relayer, and a second audit whose input is the first
audit's output claim. It also rejects certificate-only or missing-certificate
lineage, exact-claim mutations under an issued topup certificate, reused topup
certificates as audit outputs, and cross-domain note-commitment reuse before
proof verification.

`KagemushaTransfer` is the chain-side shielded offline-offline instruction. It
reuses the existing ZK asset accumulator in WSV instead of introducing a second
commitment tree: input nullifiers are consumed from `zk_assets`, output
commitments are appended to the same deterministic tree/frontier path, recent
root hints are enforced by the existing root-history window, and verification
requires the asset's bound confidential-transfer-v2 Halo2 IPA verifier record.
The record must be active, must be the active `(circuit_id, version)` mapping in
WSV, and must publish inline verifier-key bytes with matching length,
commitment, schema, and proof-size cap before the proof envelope is decoded.
This keeps Kagemusha ledger state, duplicate-nullifier protection, routing, gas,
and confidential-policy admission aligned with production shielded transfers.

Kagemusha proof attachments are transparent-only. Chain-side Kagemusha transfers
currently require the literal confidential-transfer-v2 circuit id
`halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified`. Normalized
aliases such as `anon-transfer-2x2-merkle16-poseidon-diversified` are rejected
before proof verification, even if the verifier record and proof envelope agree
on the alias. Trusted-setup labels such as KZG/Groth16/BN254 are rejected before
proof verification, including standalone labels such as `kzg`, `bn254`, and
`bls12_381` and colon-delimited profiles such as `halo2/ipa:kzg`. The shared
classifiers match those setup and developer-only markers ASCII-case-insensitively,
and setup markers are recognized across `/`, `:`, or ASCII-whitespace
delimiters. Mixed or padded labels such as `halo2/ipa: KZG` or
`halo2/ipa:Mock-Proof` therefore cannot pass broad allowlists. The attachment
backend, proof backend, and verifier-key reference backend must match exactly.
The attachment must also publish the asset-bound verifier-key commitment and a
non-empty verifier-key id name; missing or empty
trust-anchor metadata is rejected before envelope decoding. Kagemusha verifier
keys must carry non-empty inline bytes before chain-side transfer admission,
checked fold construction, or compact-token record verification can proceed.
The same production envelope policy is now shared by generic `VerifyProof`,
governance voting proofs, STARK shielded transfer/unshield wrappers,
IVM-proved overlays, IVM host registered-key verify syscalls, and Kaigi privacy
proofs. RAM-LFE proof receipts used by generic program policies and identifier
claims now follow the same canonical envelope rule. A submitted
`OpenVerifyEnvelope` may not use a zero verifier-key hash as a wildcard, its
`vk_hash` must exactly match the active registered verifier-key commitment, and
auxiliary bytes must be empty before backend verification starts.
Torii's non-consensus proof/prover worker also rejects trusted-setup backend
labels before applying broad backend allowlists, so `halo2/` prefixes cannot
admit KZG, BN254/BLS12, Groth16, standalone setup labels, or colon-profile
variants such as `halo2/ipa:kzg` as work items. Backend labels containing
developer-only `debug` or `mock` markers, in any ASCII case, are rejected at
the same boundary before registry lookup, and proof/attachment backend mismatches stop at that
same fatal pre-registry boundary.
The shared core preverify cache and guardrail dispatch wrappers enforce the
same developer-only backend rejection before dedup insertion or verifier
dispatch. Pre-validation metadata helpers, including gas public-input metering
and generic proof envelope metadata decoding, use the same non-production label
gate before attempting to decode Halo2 envelopes.
The `zk-preverify` block sidecar path records verified trace digests only. The
background trace lane revalidates queued traces for diagnostics and future
transparent IVM trace proving, but it does not emit mock proof artifacts.
Torii-generated IVM proof attachments now carry the active verifier-key
commitment that was checked while producing the proof, preserving the same
trust-anchor metadata for downstream proof submission.
Offline recursive proving also uses transparent Halo2 IPA and caches the derived
proving key by verifier-key hash when no serialized proving key is supplied, so
production proving avoids repeated key derivation without adding a trusted setup.
The Offline recursive prover and chain verifier require the literal
`offline-note-recursive` circuit id; alias spellings such as
`halo2/ipa:offline-note-recursive` are rejected before proof generation or
backend verification.
Serialized Halo2 IPA proving keys are stored as Norito archives that bind the
canonical circuit family and verifier-key commitment before the raw Halo2 key is
decoded, so Offline Note, IVM execution, and Kagemusha folded provers reject raw
or cross-circuit proving-key material before proof creation.
Chain-side Offline recursive verifier resolution uses the shared trusted-setup
and developer-only backend classifier before verifier-registry lookup, so
Groth16/KZG/BN254/BLS12 labels and labels containing `debug` or `mock` fail at
the proof metadata boundary.
Swift, Kotlin/JVM, and Java Android wallet validation require the canonical
`halo2/ipa:offline-note-recursive` verifier-key id and `halo2/ipa` proof backend
before `validateProofBinding` accepts a recursive proof. A valid embedded
envelope therefore cannot be replayed under a trusted-setup or wrong verifier
label in SDK-side Offline Note flows. Kotlin/JVM and Java Android also trim
Offline recursive verifier/proof backend metadata at construction and reject
colon separators in verifier-key backend/name fields, matching the Swift
validator surface. Wallet and redeem-planner draft bundles use the explicit
unsupported `offline-note/draft-placeholder` backend until a real proof provider
replaces them, so draft placeholders do not pass proof-binding validation.

Compact multi-hop Kagemusha tokens now have a canonical folded public-input
transcript in the Rust data model. Wallet/prover code folds bounded private hops
into `KagemushaFoldedPublicInputs`: each hop is limited to one or two input
nullifiers and one or two output commitments, nullifiers and commitments are
canonicalized as sets, repeated nullifiers or commitments are rejected, each hop
must change the Merkle root, adjacent hop roots must be continuous, and the
resulting public input hash binds chain id, asset definition, initial root,
final root, hop count, aggregate
nullifier/commitment digests, the ordered folded-hop transcript digest, each
hop's proof payload hash, each hop's proof public-input statement digest, each
hop's verifier-key binding, and the aggregation mode. The only supported
aggregation mode in this release is
checked transparent pre-fold v1: future recursive aggregation modes are reserved
but rejected by token binding and by the folded circuit until their verifier
logic exists in-tree. Folded inputs also expose a Poseidon2 aggregation
transcript digest over the canonical hop sequence. The ordinary Iroha
`fold_digest` remains available for host-side lineage checks, while the
Poseidon2 digest gives future recursive verifier circuits a hash-friendly public
accumulator without adding a trusted setup.
The aggregation statement is exposed as
`KagemushaPoseidonAggregationTranscriptStatement` with the canonical
`kagemusha_poseidon_aggregation_transcript_statement` builder and
`kagemusha_poseidon_aggregation_transcript_digest` helper. The data model also
exposes `kagemusha_folded_public_inputs_from_aggregation_statement` and
`kagemusha_validate_folded_public_inputs_against_aggregation_statement`, so SDKs
and recursive circuit tests can recompute every folded public-input digest
column from the full aggregation statement instead of checking only the final
Poseidon2 transcript digest. The digest and projection helpers validate direct
statements before hashing: checked aggregation mode, hop count, hop indices,
initial/final non-zero roots, root continuity, canonical sorted non-zero
nullifier and output sets, duplicate absence, and transparent verifier-key
backends must all match the builder output. Folded-hop proof public-input digests,
verifier-key commitments, and verifier-key Poseidon2 digests must also be
non-zero, so raw SDK transcript assembly cannot use all-zero wildcard binding
material. Folded public-input context validation also rejects all-zero initial
or final roots, identical initial/final roots, and an all-zero Poseidon2
aggregation transcript digest before compact proof generation or verification.
Each folded hop also carries a domain-separated Poseidon2 digest of the
verifier-key backend and bytes used to verify that hop. This preserves the
ordinary verifier-key commitment for host-side registry checks while giving
future recursive verifier circuits a hash-friendly verifier-key binding. The
data-model proof-statement and verifier-key digest helpers reject unsupported or
trusted-setup backend labels, non-empty proof-statement auxiliary bytes, and
zero verifier-key hashes before deriving transcript material. They also require
non-empty circuit ids, public-input schema bytes, instance columns, and
verifier-key ids and bytes, so empty metadata cannot act as wildcard
transcript material. STARK/FRI backend labels are accepted as `stark/fri` or as
`stark/fri/<profile>` with a non-empty profile, not as a bare trailing-slash
prefix; the shared core backend classifier and Torii proof/prover paths enforce
the same delimiter rule before Offline/Kagemusha verifier admission reaches
proof decoding. STARK/FRI profiles that name trusted-setup curves or openings
such as KZG, BN254, or BLS12, or developer-only `debug`/mock markers, are
rejected by the same ASCII-case-insensitive classifier.
`KagemushaCompactPaymentToken` wraps those public inputs with a transparent
folded proof and rejects tokens whose proof-declared public-input hash does not
match the canonical transcript before backend verification runs. The final
folded Halo2 IPA envelope must also be in canonical form with empty auxiliary
bytes, so unbound application metadata cannot be smuggled through a proof that
the backend verifier would otherwise accept.
The data model exposes `KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES` and
`norito_encoded_len()` helpers so wallets can keep the chain-visible folded
transcript within a fixed budget before adding backend proof bytes. The same
budget is enforced by folded-context validation, which also rejects zero or
over-64 folded hop counts, all-zero roots, unchanged public roots, and all-zero
aggregation transcript digests.
Core prover helpers also provide a checked fold-construction path: each private
hop proof attachment must verify against its transparent verifier key, backend
labels and verifier-key commitments must match, and the actual encoded
`ProofBox` hash, the Poseidon2 digest of the proof's verified public-input
statement, and the verifier-key id/commitment/Poseidon2 digest are what enter
the folded hop digest. The public-input statement digest is extracted from the
transparent `OpenVerifyEnvelope`: Halo2 IPA hops bind their embedded instance columns, and
STARK/FRI hops bind the wrapper's public input columns. The digest preimage is
publicly modeled as `KagemushaProofPublicInputsStatement` and hashed with the
domain-separated `kagemusha_proof_public_inputs_statement_digest` helper, so
wallets, SDKs, and future recursive circuits target the same Norito/Poseidon2
layout. Missing or empty circuit ids, schema descriptors, instance columns, and
verifier-key ids and bytes are rejected before transcript derivation. Halo2 IPA
proof-statement extraction also requires the literal confidential-transfer-v2
circuit id and public-input schema, so alias or fixture circuit metadata cannot
be folded even through lower helper entry points. Missing
verifier-key commitments and envelope verifier-key hash
mismatches are rejected before a hop can be folded, so a folded transcript
cannot be replayed under a different registered verifier; the envelope
verifier-key hash and backend-tag checks run before backend proof verification
to avoid spending verifier work on an impossible key binding or transparent
backend mismatch. If a hop carries optional envelope-hash metadata for audit
correlation, the checked fold path also
requires that hash to match the submitted envelope bytes before the hop is
accepted. For
confidential-transfer-v2 proof envelopes, fold construction also checks the
public root, nullifier, output, asset, and chain tags against the hop metadata.
Raw checked fold construction enforces the same confidential-transfer-v2
`max_proof_bytes` cap before parsing hop envelopes, so callers cannot bypass
the record-backed proof-size guard by using the non-record bundle path.
Confidential-transfer-v2 hop envelopes must also use canonical empty auxiliary
bytes, matching the proof emitted by the production prover and preventing
unverified application metadata from entering private-hop bundles.
Checked bundle paths also reject empty or over-64-hop bundles before decoding
hop proof envelopes, keeping the compact-token corridor cheap to reject under
adversarial input. Malformed per-hop input/output shapes and explicit all-zero
nullifier or commitment entries are rejected before proof metadata is parsed or
verifier records are looked up, and the same early metadata pass rejects root
discontinuities plus duplicate nullifiers or output commitments before
cryptographic verification starts.
The production high-level compact-token prover uses the record-backed checked
path before emitting the `kagemusha-folded-v1` proof, so production callers do
not have to assemble preverified public inputs by hand and cannot choose
unanchored inline verifier keys. Each hop is checked against WSV-style verifier
metadata: the hop `vk_ref` must resolve to an active confidential-transfer-v2
record with the expected backend tag, literal circuit id
`halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified`,
public-input schema hash, verifier-key commitment, key length, proof-size cap,
and optional inline key bytes before the hop can enter the folded transcript.
Self-consistent alias records and alias proof envelopes are rejected before hop
verification or compact proof generation. The supplied
verifier-record set must be exact: every referenced verifier must be present
once, and unrelated records are rejected before per-hop verifier work starts.
The unanchored Rust compact-token prover entry points are retained only to
return a stable verifier-record-required error. Generic active transparent
circuits are rejected before compact proof generation, with or without verifier
records.
Chain-side `KagemushaTransfer` uses the same production binding: the submitted
proof must decode as a Halo2 IPA `OpenVerifyEnvelope` whose backend tag,
confidential-transfer-v2 circuit id, public-input schema, and verifier-key hash
match the asset-bound active verifier record before the shared shielded
accumulator executor runs. The active record must also publish inline Halo2 IPA
verifier-key bytes, a matching key length, a matching verifier-key commitment,
and a non-zero proof-size cap before the proof envelope is decoded; non-empty
auxiliary bytes are rejected there as a non-canonical envelope. Duplicate input
nullifiers or output commitments are rejected before proof envelope decoding,
matching the folded-token set invariant and preventing duplicate commitments
from entering the shielded tree. Explicit all-zero nullifiers or output
commitments are rejected at the same boundary; zero remains padding inside fixed
confidential-v2 public input columns, not a valid submitted set member.
The shared executor then checks the proof's public root, nullifier, output
commitment, asset tag, and chain tag against the transaction fields before
proof verification can mutate shielded state. When callers provide optional
envelope-hash metadata for audit correlation, that hash must also match the
submitted envelope bytes.
`connect_norito_bridge` exposes
`connect_norito_kagemusha_prove_verified_compact_payment_token` for mobile
runtimes that still link against the legacy ABI. That unanchored symbol rejects
even valid Norito-encoded `KagemushaVerifiedFoldBundle` archives without
returning output bytes, because production compact-token proving must carry
verifier-record trust anchors. The production bridge entry point is
`connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`;
it accepts `KagemushaVerifiedFoldRecordBundle` and enforces the WSV-style
verifier metadata for every private hop before compact proof generation.
Malformed bundles, oversized bundle hop counts, malformed hop shapes, root
discontinuities, duplicate nullifiers or output commitments, tampered hop
proofs, oversized hop proof payloads, missing or inactive records, verifier
schema/commitment/proof-cap mismatches, duplicate or extraneous verifier
records, self-consistent confidential-transfer-v2 circuit-id aliases, forged
envelope-hash metadata, non-canonical hop auxiliary bytes, and reserved
aggregation modes are rejected before compact proof generation.
Swift exposes this record-backed path as
`KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords`,
and Kotlin/JVM plus Java Android expose mirrored
`KagemushaCompactPaymentTokenProver` native wrappers. These SDK APIs accept and
return raw Norito archives so wallet code can keep using the Rust data-model
layout while the native bridge performs the proof and verifier-record checks.
The Swift dynamic loader now requires bridge ABI 4 and the record-backed
Kagemusha symbol before reporting the compact-token prover as available.
Verifier records for chain-side transfers, record-backed compact-token proving,
and final folded-token record verification must live in the canonical
`offline_kagemusha` namespace; generic active confidential-transfer verifier
records are rejected before proof decoding.

The first folded-token proof path is `kagemusha-folded-v1`, a transparent
Halo2 IPA circuit over Pasta. It binds the 30-column folded public statement,
constrains aggregation mode to checked pre-fold v1, constrains `hop_count` to
the compact-token corridor, constrains the public-input hash, initial/final
roots, aggregate nullifier/output digests, fold digest, and Poseidon2
aggregation transcript digest to be non-zero, publishes an active verifier-key
record, proves that the final root differs from the initial root, and exposes
Rust helpers to prove and verify compact payment tokens without a trusted setup. The
non-zero root/digest checks use inverse witnesses inside the circuit, so
all-zero wildcard root or digest columns fail during Halo2 constraint
verification rather than only at host-side admission. The root-change check uses
a one-hot selected-limb inverse witness, so an unchanged root transition cannot
be hidden behind a valid proof envelope.
Final folded-token proving, direct verification, and record-backed verification
require that literal circuit id; alias spellings such as
`halo2/ipa:kagemusha-folded-v1` are rejected at the compact-token boundary.
Direct and record-backed compact-token verification reject trusted-setup and
developer-only final folded proof backend labels, preserving the canonical
`halo2/ipa` folded proof boundary.
Precomputed folded proving keys use the same Norito archive binding as Offline
Note and IVM proving keys, so a key generated for another circuit family or
verifier-key commitment is rejected before final proof creation.
Compact-token verification explicitly binds the token verifier-key id,
`OpenVerifyEnvelope` circuit id, Halo2 IPA backend tag, public-input schema,
public instance columns, and verifier-key hash before running backend
verification. WSV-style verifier-record entry points add the governance checks
expected for production admission for both the final folded proof and the
private hop proofs: active status, canonical schema hash, verifier-key
commitment, key length, proof-size cap, optional inline-key consistency, and
circuit id.

Remaining Kagemusha work is recursive aggregation of the private per-hop proofs
inside the compact folded-token proof. Until that lands, the checked fold
constructor verifies each hop proof and records the exact public-input statement
that was verified before emitting the folded public transcript. The final
aggregation proof must continue to use no trusted setup and preserve the same
public nullifier/commitment/root semantics.
The current tree has native Halo2 IPA proof verification and IPA polynomial
opening code, but it does not ship a Halo2 circuit gadget that verifies another
Halo2 IPA proof in-circuit. The first reusable recursive-verifier foundations
are now present in the Pasta circuit module: a non-native `u64` limb
range/decomposition gadget that proves a public limb has exactly 64 boolean
little-endian bits and no high residue above bit 63, a native Pasta/Fp scalar
decomposition gadget that can run in public or private exposure mode, binds a
scalar to four private `u64` limbs, proves those limbs are the canonical 255-bit
representation below the Fp modulus, and rejects `value + modulus` aliases,
plus a canonical Vesta/Fq range gadget that proves four such limbs are strictly
below the Vesta base-field modulus using a private slack and borrow chain.
Modular Vesta/Fq addition now
checks an unreduced-sum witness and independent carry chains for
`lhs + rhs = sum` and `out + reduction*q = sum`; modular multiplication checks
schoolbook product limbs, private `u128` product/reduction carries, a private
canonical quotient, and `out + quotient*q = lhs*rhs`. The same foundation now
includes an affine Vesta on-curve check for public `x/y` coordinates, linking
private `x*x`, `y*y`, `x^2*x`, and `x^3 + 5` witnesses back to the public
coordinates. It also includes a distinct affine point-addition gadget that
checks `P`, `Q`, and `R` are on curve, enforces `x(P) != x(Q)` through a private
denominator inverse, and links the non-native slope, x-coordinate, and
y-coordinate equations for `P + Q = R`. A matching affine point-doubling gadget
checks `P` and `R` are on curve, proves `2*y(P)` is invertible, links
`lambda * (2*y(P)) = 3*x(P)^2`, and enforces the doubled-point x/y output
equations. The circuit module also has a point-or-identity validity gadget with
canonical identity encoding `(0, 0, 1)`, boolean identity enforcement, and
conditional on-curve checks for non-identity points. Complete non-native Vesta
addition now composes those pieces under one-hot private branch selectors for
left-identity passthrough, right-identity passthrough, inverse-pair output
identity, doubling, and distinct affine addition, with adversarial checks for
selector substitution and branch-equation tampering. A conditional-add layer now
proves `R = Acc + (bit ? Addend : Identity)`, keeps the selected addend private,
enforces the selector bit and identity encoding, and links into the complete-add
selector so missing sub-gadget activation is caught. The first bounded
scalar-multiplication wrapper binds a public `u64` scalar to the shared bit
decomposition, proves the addend doubling ladder from the public base, keeps
intermediate accumulators private, and exposes only the base and output point
encodings. A native-scalar variant now consumes the canonical Pasta/Fp scalar
decomposition directly, links each conditional-add selector to the decomposed
scalar bits, enforces high-bit zeroing for bounded widths, and proves the same
private addend-doubling ladder from the public base. A fixed-window Pasta/Fp
scalar decomposition gadget now proves deterministic little-endian window
digits for the production windowed-MSM path: every digit bit is boolean and
linked to the canonical private scalar bit at the same global offset, and all
scalar bits above the configured window width are constrained to zero. A
non-native Vesta fixed-window point selector now proves that a selected private
point-or-identity comes from a private `2^WINDOW_BITS` table under those
canonical window bits. The selector uses a quadratic binary selection network
rather than a high-degree product selector. A companion table-derivation gadget
now proves the private table is exactly `[0, B, 2B, ...]` for a public base
point by linking entry zero to identity, entry one to the public base, and later
entries to a complete-add chain. A fixed-window native-scalar multiplication
wrapper now composes scalar windows, shifted-base tables, selectors, per-window
base doublings, and selected-point accumulation into a public
`output = scalar * base` statement. A fixed-window native-scalar MSM wrapper now
composes multiple windowed scalar-multiplication terms into one public
multi-scalar accumulator, keeping per-term outputs private while linking public
bases and the final public output. A bounded native-scalar
Vesta MSM wrapper now composes those scalar-multiplication witnesses with
private canonical Pasta/Fp scalars, public base encodings, a public final output
encoding, and a private running point-or-identity sum. It links each per-term
conditional-add ladder to the decomposed scalar bits, proves every doubled
addend step, starts the MSM accumulator at identity, and chains every term
output into the final public sum. Those layers are needed before Vesta/Fq
coordinates from Halo2 IPA commitments can be represented and accumulated
soundly inside the folded proof circuit. The next IPA-specific wrapper now
proves the final verifier comparison `Q = a*G + b*H + (a*b)*U`: it reuses the
three-term bounded MSM while adding a native-field product link for the third
scalar, so a self-consistent MSM cannot forge the `a*b` term. An optimized
fixed-window version of the same final comparison now routes the three-term MSM
through private canonical scalar windows, shifted-base tables, and table
selections while keeping the same `a*b` product-link invariant. A per-round IPA
accumulator wrapper now proves `Q' = x^2*L + Q + x^{-2}*R` through the
fixed-window MSM path, keeps `x` and `x^{-1}` as private canonical Pasta/Fp
scalars, constrains them as inverses, and links the three MSM scalars to
`x^2`, `1`, and `x^{-2}`. A generator-fold wrapper now proves
`G' = x^{-1}*G_L + x*G_R` and `H' = x*H_L + x^{-1}*H_R` with two
shared-challenge fixed-window MSMs, so folded generator witnesses cannot drift
from the transcript challenge. The native transparent IPA
verifier also exposes canonical per-round transcript projections: after the
caller has absorbed the polynomial-opening statement, the helper records the
state before and after each `L || R` absorb, the derived `ipa.x` challenge, its
inverse, and the post-challenge state used by the ordinary verifier. This gives
the future recursive circuit a stable witness layout for label, statement, and
round-order binding without changing the transparent no-trusted-setup backend.
The native-field side of IPA reduction now also has scalar and segment-vector
fold gadgets for `b' = b_L*x^{-1} + b_R*x`; they keep `x` and `x^{-1}` private
and canonical, expose the folded public scalar outputs, share one challenge
pair across vector segments, and reject mismatched inverses, substituted public
inputs or outputs, and noncanonical scalar aliases. A companion multi-round
`b`-vector reduction gadget folds an entire power-of-two public vector to the
final public scalar while keeping intermediate vector layers private and
canonical. Its round challenges and inverses are public circuit inputs linked
to the private canonical decompositions, giving the future recursive verifier a
single checked path from the statement's `b` vector and externally projected
Fiat-Shamir challenges to the proof's final `b` scalar. The native transparent
IPA verifier now exposes the same deterministic `b`-reduction projection and
rejects proofs whose `proof.b_final` is not the transcript-challenge fold of
the public statement vector.
On the native proving/verifying side, IPA vector commitments now dispatch
through backend-level deterministic MSM hooks. Pallas and BN254 use
`halo2curves::msm_best`, with the previous one-scalar-mul-per-base fold kept as
the generic fallback for simple backends; this does not add a trusted setup and
does not change the transparent generator derivation.
The native verifier also exposes its scalar-multiplication accumulation
projection: initial `Q = P * H(b) * U^t`, each round's `Q` update, challenge
squares, folded `g/h` vectors, final folded generators, and final expected term.
That projection now rejects externally supplied challenge witnesses unless
`x*x^{-1}=1` before using them in the round accumulator.
The native zkp crate also derives and validates a combined IPA verifier witness
that bundles a public transcript projection, the full `b`-vector reduction, the
scalar-multiplication accumulation projection, and the proof's final scalars.
The transcript projection records the `ipa.n` state boundary, each round's
`L/R` bytes, domain-separated round-byte digest, transcript states, challenge,
inverse, and final transcript state. Validation rejects transcript-state,
round-byte, round-digest, round-order, challenge, reduction, accumulator, or
final-scalar substitution before those values can be consumed by the recursive
circuit witness builder.
The same native projection now has a field-friendly transcript binding for
recursive circuits: the host maps the SHA3-validated header, each complete round
projection, each challenge/inverse pair, and the final transcript state into
Pasta/Fp scalars, then folds them with a transparent Pow5 accumulator. A native
Pasta/Fp circuit enforces that accumulator over public projection and
challenge/inverse scalars, checks `x*x^{-1}=1`, and links every compressor row
to the final public digest. This binding does not replace the native SHA3
transcript check; it gives the recursive verifier a compact challenge-binding
public input without adding a trusted setup.
The in-circuit side now has a one-round verifier composition slice that shares
one private canonical `x/x^{-1}` pair across the `b`-vector fold, `Q`
accumulator update, generator fold, and final MSM comparison. The slice links
the folded `b`, `Q'`, `G'`, and `H'` advice values directly across subgadgets,
so independently valid subproof witnesses cannot be spliced together with
different intermediate values. A generic power-of-two multi-round composition
wrapper now extends that linking across all IPA rounds: every round shares its
challenge pair across `b`, `Q`, and every generator-fold pair, each round's
`Q`, `G`, and `H` outputs feed the next round, and the last folded values feed
the final fixed-window IPA comparison. The same wrapper now composes the
transcript-binding accumulator and links its public challenge/inverse rows back
to the decomposed `b`-reduction challenge columns, so a self-consistent binding
digest cannot be paired with different verifier challenges. The host-side
bridge can now translate native Pallas IPA verifier witnesses into this Vesta
recursive witness shape by validating the transcript, `b`-reduction, and
accumulator projections, round ordering, and canonical compressed point
encodings in a cheap preflight path. That preflight also recomputes the native
Pallas `b`, `Q`, `G`, `H`, and final-term fold relations before converting
scalars and compressed points only through canonical byte encodings. The group
relation recomputation uses the deterministic optimized Pallas MSM backend, so
the production preflight follows the same no-trusted-setup IPA backend path used
by native verification. The same bridge can now validate an ordered batch of
native Pallas verifier witnesses and emit a compact domain-separated aggregate
digest using the streaming Poseidon2 byte sponge. That digest binds the
transparent parameter fingerprint, witness order, transcript projections, `b`
reductions, accumulator folds, final terms, and proof-final scalars after every
witness passes the single-witness preflight.
That batch summary is the host-side evidence surface intended to feed the
future private-hop recursive aggregation circuit while mode `2` remains
reserved. The data model now also has a reserved-mode recursive aggregation
evidence statement that Norito/Poseidon-binds that batch digest and parameter
fingerprint to the same ordered hop transcript and to the canonical
`pallas-ipa-transparent-v1/vesta-recursive-fixed-window-85x3` verifier-witness
profile. It validates mode `2` evidence shape, hop continuity, witness count,
profile, and non-zero batch fields without making mode `2` accepted for
compact-token admission. Focused data-model coverage also roundtrips the
evidence through Norito, validates the decoded profile-bound digest, rejects
decoded unsupported-profile evidence, and rejects truncated evidence archives.
The reserved evidence validator also has explicit adversarial coverage for
empty transcripts, over-cap hop lists, duplicate input nullifiers, and duplicate
output commitments.
Core exposes record-backed evidence builders for that reserved path. They first
enforce the same active WSV-style confidential-transfer-v2 hop verifier records
used by production compact-token proving, verify every private hop proof, and
then bind the supplied native verifier-witness batch digest and parameter
fingerprint to the canonical hop transcript. Witness-count mismatches and
all-zero batch metadata are rejected before hop proof decoding, and the
serializable `KagemushaVerifiedFoldRecordBundle` wrapper follows the same
checked path.
Core also exposes a public Pallas IPA batch preflight helper and record-backed
evidence builders that take the native verifier witnesses directly instead of a
detached digest tuple. The helper accepts only power-of-two opening widths
`2..=128` and at most 64 witnesses, matching the current `k = 7` transparent
Halo2 IPA corridor and compact-token hop cap, and uses the 85-by-3 fixed-window
Vesta verifier witness profile that covers the 255-bit Pasta scalar width
without a trusted setup. Its aggregate digest is Poseidon2-backed rather than a
generic hash transcript, so the host-side reserved evidence path stays aligned
with the field-friendly no-trusted-setup Kagemusha transcript surface. The
combined builders re-derive that digest with the ordered checked-hop proof
hashes before storing it in reserved evidence, so a valid detached native batch
cannot be replayed against a different folded-hop proof transcript without
changing the evidence digest. This is still a reserved evidence binding, not
the final derivation of IPA verifier witnesses from each hop proof envelope.
The combined builders reject native witness-count
mismatches before native preflight or hop proof decoding, the public preflight
rejects empty witness batches directly, and native preflight then rejects
wrong-width witness/parameter pairings, transcript, reduction, accumulator, or
final-term splices before constructing reserved-mode evidence whose digest is
profile-bound and hop-proof-hash-bound.
Native Pasta/Fp scalar decomposition, fixed-window scalar decomposition,
fixed-window Vesta point selection, table derivation, and scalar-multiplication
composition, fixed-window multi-term MSM, native IPA scalar/vector-fold, full
`b`-vector reduction, bounded MSM, fixed-window final IPA MSM, IPA
generator-fold, round-accumulator, and final IPA comparison composition plus
one-round and generic multi-round verifier composition with transcript binding,
native Pallas witness translation, batch preflight binding, and reserved-mode
recursive aggregation evidence binding are present. The
remaining recursive-circuit work is producing production-width composed circuit
evidence and private-hop recursive aggregation, so aggregation mode `2` remains a
reserved wire value with a stable rejection reason, and public prover/verifier
entry points accept only checked pre-fold mode `1`.
