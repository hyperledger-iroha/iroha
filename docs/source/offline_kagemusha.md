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
anchored by an earlier topup or audit output. Audit outputs are now one-to-one
with public output commitments, and public asset definitions and amount totals
must conserve before proof verification. Output certificates are signature
checked against the output account they claim before their lineage is recorded,
so a relayer can submit audit metadata without becoming a certificate issuer.
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
Focused core coverage executes a real online-to-offline topup followed by an
audit submitted by an unrelated relayer, and also rejects certificate-only or
missing-certificate lineage and invalid audit-output certificate signatures
before proof verification.

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
currently require the transparent `halo2/ipa` confidential-transfer-v2 circuit;
trusted-setup labels such as KZG/Groth16/BN254 are rejected before proof
verification, including standalone labels such as `kzg`, `bn254`, and
`bls12_381` and colon-delimited profiles such as `halo2/ipa:kzg`. The shared
classifiers match those setup and developer-only markers ASCII-case-insensitively,
so mixed-case labels such as `halo2/ipa:KZG` or `halo2/ipa:Mock-Proof` cannot
pass broad allowlists. The attachment backend, proof backend, and verifier-key
reference backend must match exactly. The attachment must also publish the asset-bound
verifier-key commitment and a non-empty verifier-key id name; missing or empty
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
dispatch.
The `zk-preverify` block sidecar path records verified trace digests only. The
background trace lane revalidates queued traces for diagnostics and future
transparent IVM trace proving, but it does not emit mock proof artifacts.
Torii-generated IVM proof attachments now carry the active verifier-key
commitment that was checked while producing the proof, preserving the same
trust-anchor metadata for downstream proof submission.
Offline recursive proving also uses transparent Halo2 IPA and caches the derived
proving key by verifier-key hash when no serialized proving key is supplied, so
production proving avoids repeated key derivation without adding a trusted setup.
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
label in SDK-side Offline Note flows. Wallet and redeem-planner draft bundles
use the explicit unsupported `offline-note/draft-placeholder` backend until a
real proof provider replaces them, so draft placeholders do not pass
proof-binding validation.

Compact multi-hop Kagemusha tokens now have a canonical folded public-input
transcript in the Rust data model. Wallet/prover code folds bounded private hops
into `KagemushaFoldedPublicInputs`: each hop is limited to one or two input
nullifiers and one or two output commitments, nullifiers and commitments are
canonicalized as sets, repeated nullifiers or commitments are rejected, adjacent
hop roots must be continuous, and the resulting public input hash binds chain
id, asset definition, initial root, final root, hop count, aggregate
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
`kagemusha_poseidon_aggregation_transcript_digest` helper, so SDKs and
recursive circuits derive the same canonicalized Norito/Poseidon2 layout as the
chain-visible folded public inputs. The digest helper validates direct
statements before hashing: checked aggregation mode, hop count, hop indices,
initial/final roots, root continuity, canonical sorted non-zero nullifier and
output sets, duplicate absence, and transparent verifier-key backends must all
match the builder output. Folded-hop proof public-input digests,
verifier-key commitments, and verifier-key Poseidon2 digests must also be
non-zero, so raw SDK transcript assembly cannot use all-zero wildcard binding
material.
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
over-64 folded hop counts.
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
verifier-key ids and bytes are rejected before transcript derivation. Missing
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
The high-level compact-token prover uses this checked path before emitting the
`kagemusha-folded-v1` proof, so production callers do not have to assemble
preverified public inputs by hand. A record-backed variant additionally checks
each hop against WSV-style verifier metadata: the hop `vk_ref` must resolve to
an active confidential-transfer-v2 record with the expected backend tag,
circuit id, public-input schema hash, verifier-key commitment, key length,
proof-size cap, and optional inline key bytes before the hop can enter the
folded transcript. The supplied verifier-record set must be exact: every
referenced verifier must be present once, and unrelated records are rejected
before per-hop verifier work starts. Generic active transparent circuits are
rejected before compact proof generation, with or without verifier records.
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
runtimes that hold private hop proof bundles. The input is a Norito-encoded
`KagemushaVerifiedFoldBundle`; the bridge verifies every bundled hop proof and
verifier-key commitment before deriving folded public inputs, then returns a
Norito-encoded `KagemushaCompactPaymentToken`. For production callers that can
carry verifier records, the preferred bridge entry point is
`connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`;
it accepts `KagemushaVerifiedFoldRecordBundle` and enforces the WSV-style
verifier metadata for every private hop before compact proof generation.
Malformed bundles, oversized bundle hop counts, malformed hop shapes, root
discontinuities, duplicate nullifiers or output commitments, tampered hop
proofs, oversized hop proof payloads, missing or inactive records, verifier
schema/commitment/proof-cap mismatches, duplicate or extraneous verifier
records, forged envelope-hash metadata, non-canonical hop auxiliary bytes, and
reserved aggregation modes are rejected before compact proof generation.
Swift exposes this record-backed path as
`KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords`,
and Kotlin/JVM plus Java Android expose mirrored
`KagemushaCompactPaymentTokenProver` native wrappers. These SDK APIs accept and
return raw Norito archives so wallet code can keep using the Rust data-model
layout while the native bridge performs the proof and verifier-record checks.
The Swift dynamic loader now requires bridge ABI 4 and the record-backed
Kagemusha symbol before reporting the compact-token prover as available.

The first folded-token proof path is `kagemusha-folded-v1`, a transparent
Halo2 IPA circuit over Pasta. It binds the 30-column folded public statement,
constrains aggregation mode to checked pre-fold v1, constrains `hop_count` to
the compact-token corridor, publishes an active verifier-key record, and exposes
Rust helpers to prove and verify compact payment tokens without a trusted setup.
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
Halo2 IPA proof in-circuit. For that reason aggregation mode `2` remains a
reserved wire value with a stable rejection reason, and public prover/verifier
entry points accept only checked pre-fold mode `1`.
