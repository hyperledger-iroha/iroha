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
must have been issued by that lineage. The input note commitment must also carry
an issued commitment replay key from the original topup or from a prior audited
output before the audit can reach proof verification. A claim mutation under a
legitimately issued certificate is rejected before recursive proof verification.
Final `RedeemOfflineNote` admission applies the same trust-anchor rule: the
redemption must have both the exact issued-claim replay key and the source note
commitment's issued replay key from the online-to-offline topup or a prior audit
output before it can reach proof verification. Claim-only redeem lineage is
rejected as an unissued source note.
Audit outputs are now one-to-one with public output commitments, and public
asset definitions and amount totals must conserve before proof verification.
Output certificates are signature checked against the output account they claim
before their lineage is recorded, so a relayer can submit audit metadata without
becoming a certificate issuer.
Audit output key certificates must also be fresh one-use certificates: any
output certificate whose replay key was already anchored by an online-to-offline
topup or prior audit output is rejected before recursive proof verification.
When a platform exposes a hardware usage-count limit in an offline note key
certificate, the value must be exactly `1`; multi-use or zero-use counters are
rejected even if the boolean one-use flag is set. The Torii offline issuer
enforces the same rule before minting the online-to-offline topup certificate,
and Swift, Kotlin/JVM, and Java Android SDK certificate constructors enforce it
before wallet code can serialize or submit certificate metadata. Their Torii
issuer-response parsers also reject malformed, overflowed, or non-numeric
certificate versions and counter values before narrowing them into SDK
certificate models.
Legacy audit metadata also rejects any output commitment that byte-equals a
consumed input nullifier before recursive proof verification, matching the
Kagemusha nullifier/commitment domain-separation guard.
Note commitments are one-use across both topup issue and audit-output replay
domains, so a commitment created by an online-to-offline topup cannot be
reintroduced as a P2P bearer output, and a prior P2P output commitment cannot be
loaded again through a later topup. Torii topup issuance now builds the wallet
JSON certificate and the `IssueOfflineNote` chain certificate from the same
signed certificate object, so the wallet's offline trust anchor is the exact
payload hash recorded by the online-to-offline transaction.
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
lineage, claim-only lineage without an issued input commitment key, exact-claim
mutations under an issued topup certificate, reused topup certificates as audit
outputs, forged redeem source commitments even when the forged claim key is
anchored, and cross-domain note-commitment reuse before proof verification.

`KagemushaTransfer` is the chain-side shielded offline-offline instruction. It
reuses the existing ZK asset accumulator in WSV instead of introducing a second
commitment tree: input nullifiers are consumed from `zk_assets`, output
commitments are appended to the same deterministic tree/frontier path, recent
root hints are enforced by the existing root-history window, and verification
requires the asset's bound confidential-transfer-v2 Halo2 IPA verifier record.
The record must be active, must be the active `(circuit_id, version)` mapping in
WSV, and must publish inline verifier-key bytes with matching length,
commitment, schema, and proof-size cap before the proof envelope is decoded.
The inline verifier key must also match the canonical confidential-transfer-v2
semantic circuit key, so a self-consistent record/binding cannot substitute
different Halo2 IPA key bytes under the production circuit id.
The canonical confidential-transfer-v2 and unshield verifier keys are generated
through process-local caches, avoiding repeated no-trusted-setup key generation
on hot verifier-record and Kagemusha guard paths. These verifier-key envelopes
also carry a `CID1` circuit-id TLV in addition to the Halo2 `H2VK` payload, so
their commitments are separated by semantic circuit id even when raw Halo2
verifier-key payloads deserialize across related circuits. The Halo2 IPA
backend verifier rejects a proof when a supplied verifier-key envelope carries a
`CID1` value that normalizes to a different circuit than the proof envelope.
This keeps Kagemusha ledger state, duplicate-nullifier protection, routing, gas,
and confidential-policy admission aligned with production shielded transfers.
Kagemusha transfer admission also rejects any byte-identical overlap between
consumed input nullifiers and newly created output commitments before proof
decoding, preserving the nullifier/commitment domain separation at the ledger
boundary. The reusable folded-public-input and Poseidon aggregation-transcript
validators enforce the same disjointness for same-hop and cross-hop statements
before compact-token or reserved recursive evidence can be built.

Recursive Kagemusha spend bundles now carry a chain-visible top-up anchor set:
the sorted first-hop input nullifiers from the online-to-offline top-up lineage.
The anchor set is included in the recursive spend accumulator digest and
therefore in the recursive proof public inputs. The final recursive redemption
path has two admission forms. Semantic v1 accumulator proofs require a
record-backed lineage witness next to the constant-size D2D bundle. That witness
contains the full checked hop record bundle, proof-derived Pallas open-envelope
archive, the spendable note descriptor created by each hop, and the intermediate
recursive proofs committed by `recursive_proof_chain_digest`. The witness record
bundle must name exactly the hop verifier records referenced by the lineage, and
the Pallas archive must decode to exactly one envelope per hop: no malformed,
missing, duplicate, unreferenced, or count-mismatched witness material is
accepted. Directly supplied redeem witnesses and helper-built init/append
witnesses share the same verifier-record checks: every referenced record must
be active, live in the canonical `offline_kagemusha` namespace, publish the
expected backend tag and curve label (`pallas` for Halo2 IPA, `goldilocks` for
Stark), a non-empty circuit id, a non-zero public-input schema hash, a non-zero
commitment, and a proof-size cap; it must match the proof attachment
commitment, match the inline verifier-key length, and carry inline key bytes
equal to the hop verifier key. For Halo2 IPA hops, request validation also
recomputes the stable verifier-key commitment from the inline key bytes, rejects
hop proof envelopes whose `vk_hash` does not match that commitment, and binds
each hop proof envelope's circuit id, verifier-key hash, and public-input
schema metadata to the active verifier record, and binds each lineage Pallas
opening envelope's verifier-key and public-input schema metadata to that checked
hop proof envelope before native evidence construction.
Witness assembly
first validates the recursive bundle's public-input binding, then requires
intermediate `previous_recursive_proofs` entries to be supported recursive spend
proofs in lineage order, with each proof's hop count equal to its one-based
position. Each previous proof must also share the final bundle's verifier
opening length, Pallas parameter fingerprint, fixed-window schedule digest, and
shared-table manifest digest; per-hop witness-batch and table-base digests
remain prefix-specific and are replayed by core. Semantic previous proofs must
leave the lineage scalar-projection digest zero; Reserved-lineage previous
proofs must bind a non-zero scalar-projection digest and require the active
lineage verifier record before native hosts can serialize a redeem instruction.
Core chain-side replay
preflights those previous proofs before
reconstructing hop evidence, so backend/profile substitutions, empty previous
proofs, stale public-input hashes, verifier-context splices,
scalar-projection splices, and out-of-order hop counts fail before expensive
Pallas replay. Chain execution also requires
every supplied record snapshot to equal the currently registered WSV verifier
record. Without that witness, semantic v1 accumulator proofs remain
admission-neutral. The reserved `kagemusha-recursive-spend-lineage-v1` profile
is the witnessless chain-admission path inside the 64-hop cap: its strict
envelope/profile shape, active verifier-record binding, inline lineage key
material, backend proof preverification, private hop chain, and accumulator
transition bindings must all verify before redemption proceeds. Once admitted,
redemption
consumes every top-up anchor nullifier plus the current spendable note nullifier
before minting the public amount, so two hidden branches from the same top-up
collide on the anchor even when they end in different final notes. Append hops
may consume only the previous spendable note nullifier and must preserve the
public amount carried by the previous spendable note; they cannot merge fresh
external inputs whose nullifiers would not be in the original top-up anchor set,
inflate the public note amount while offline, or create a new current note that
reuses the nullifier just consumed.
Accumulator validation also rejects forged cross-type collisions where a
top-up anchor equals the current spendable note commitment, or where a current
note spend nullifier equals any output commitment in the hop that created it.
Append validation rejects additional carried-state collisions: the next note
spend nullifier cannot reuse the previous spendable note commitment, and no
new output commitment, including a sibling output that is not selected as the
current spendable note, may reuse the previous spendable note commitment or any
carried top-up anchor nullifier.
The accumulator's streaming nullifier, output, and fold-transcript digests use
recursive-spend domain tags, separate from the folded-token list/transcript
digest tags, so a one-hop recursive spend state cannot be replayed as a plain
checked folded transcript digest with the same field values. Accumulator
validation also requires the recursive aggregation transcript digest to equal
the lineage digest, so the proof public input cannot be detached from the
spend-lineage accumulator it is supposed to compress.
The C bridge, Node NAPI host, and Python native redeem helpers also reject zero
or mismatched public amounts before emitting a `RedeemKagemushaRecursive`
instruction. Their shared request validation also rejects final redeem proof
attachments outside the current transparent `halo2/ipa` production corridor,
backend/proof/verifier-key backend mismatches, empty proof bytes, missing or
zero verifier-key commitments, and mismatched envelope hashes before instruction
serialization. The same request guard now also rejects recursive spend bundles
whose carried recursive proof leaves the transparent `halo2/ipa` corridor or
has empty proof bytes, so bridge/native redeem helpers fail before serializing
instructions that chain-side verifier-record admission would reject anyway.
This recursive spend bundle is the default spendable D2D payload for offline
cash. It carries the current public accumulator state, the current spendable note
descriptor, verifier references, final root/commitment binding, and one
recursive proof; it does not carry prior hop proof bundles. The append path
validates the previous recursive proof and the new one-hop
confidential-transfer-v2 evidence before emitting the next bundle. Each
append folds a domain-separated digest of the previous recursive proof artifact
into `recursive_proof_chain_digest`, and that digest is inside the accumulator
digest exposed to the next recursive proof. Recursive spend proofs also expose
that proof-chain digest as an explicit public-input limb group, while standalone
reserved recursive aggregation proofs set the same public-input field to zero.
The Reserved-lineage verifier-slice circuits also have a semantic public-input
limb group for `recursive_verifier_scalar_projection_digest`; standalone
recursive aggregation proofs and semantic v1 spend proofs set that field to
zero, while production Reserved-lineage spend proofs constrain the field to the
embedded IPA verifier's field-friendly scalar projection. The first offline hop
uses the one-hop verifier-slice profile. Later offline hops use the append
profile, which proves both the previous recursive proof opening and the next
confidential-transfer hop opening while emitting a single new recursive spend
bundle.
Receivers can verify, store, and re-spend without contacting a node while the
D2D payload keeps only this constant-size proof-chain commitment instead of
prior proof bundles. The final holder submits the recursive bundle, public
redeem amount, and final unshield proof for online redemption. Reserved-lineage
bundles whose hop count is inside
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` can redeem
witnesslessly when they carry the active lineage verifier record and pass
chain admission. Semantic v1 recursive bundles still carry a record-backed
lineage witness for online redemption.
Chain admission first honors the config gates (`kagemusha_enabled = true` and
`kagemusha_force_legacy = false`), validates the recursive proof envelope,
checks the bundle chain id, and checks the final root and note commitment
against the final unshield proof public inputs. If a semantic lineage witness is
present, admission verifies every private hop against the supplied verifier
records only after confirming those records exactly match WSV, checks the
Pallas open envelopes, replays the accumulator from the ordered per-hop note
descriptors, verifies each intermediate recursive proof committed by the
proof-chain digest, and requires the recomputed accumulator to match the redeem
bundle before nullifiers are consumed or public assets are minted. Semantic
`kagemusha-recursive-aggregation-v1` spend proofs are still rejected as
admission-neutral when that witness is missing, because the semantic proof alone
does not prove the private-hop lineage in-circuit. The
record-backed lineage gate runs before final semantic recursive proof backend
verification, while still preserving the cheaper bundle, verifier-record, and
final redeem public-input checks that produce more specific diagnostics.
The reserved `kagemusha-recursive-spend-lineage-v1` profile is the enabled
witnessless chain-admission path for constant-size lineage proofs inside the
64-hop cap. Its
preflight requires the proof and verifier key to stay in the transparent
`halo2/ipa`
corridor, rejects empty proof bytes, recomputes the recursive public-input
hash, checks the accumulator-derived public fields, and requires a non-zero
recursive verifier scalar-projection digest. It also decodes the inner
`OpenVerifyEnvelope` and binds it to the Halo2/Pasta backend tag, the lineage
circuit id, a non-zero verifier-key hash, the recursive public-input schema,
empty auxiliary metadata, and the accumulator-derived public instance columns.
The reserved lineage profile requires those instance columns to come from a
strict ZK1 no-trusted-setup inner proof envelope; legacy Halo2 proof-envelope
wrappers remain accepted only for semantic v1 preverification and are rejected
under the lineage circuit id. Record-backed preverification additionally
requires that the inline verifier-key envelope be a strict no-trusted-setup
Halo2 IPA ZK1 key container: exactly one reserved-lineage `CID1`, exactly one
bounded `IPAK` degree, exactly one non-empty `H2VK`, and no unrelated key TLVs.
Preverification also checks the cheap Halo2/Pasta verifier-key header so the
`H2VK` domain degree matches the bounded `IPAK`, and requires the payload to
contain the declared fixed-column commitments so truncated verifier keys are
rejected before the heavy verifier-slice circuit is materialized. The proof
envelope verifier-key hash must match the verifier-record commitment. Chain
admission validates the Reserved-lineage envelope/profile shape before backend
proof verification and accepts two strict public-instance layouts: one-hop init
(`witness_count = 1`, `hop_count = 1`, one verifier-slice scalar-projection
column) and append (`witness_count = hop_count`, `hop_count > 1`, non-zero
transition-profile, append-opening-preflight, append-boundary, and append
scalar-projection limb groups). Appends must bind the previous recursive proof
opening, current hop proof opening, chain and asset, final note, and accumulator
transition profile. Any lineage bundle outside
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` is rejected
before nullifiers or public assets are touched.
Malformed profile attempts that splice accumulator public inputs, refresh a
stale hash over forged fields, substitute a trusted-setup backend, omit scalar
projection, replay a semantic inner envelope or semantic verifier key, smuggle
duplicate or malformed verifier-key `CID1` TLV material, omit `IPAK` or `H2VK`,
use the wrong IPA degree, truncate declared fixed-column commitments, include
unexpected verifier-key TLVs, substitute envelope schema or public instance
columns, publish a zero verifier-record commitment, mismatch the envelope
verifier hash, or use an unrecognized lineage
circuit id are rejected before mint admission.
The spend accumulator and bridge redeem request validation are lineage-aware:
reserved lineage bundles compare the same accumulator-derived fields as
semantic v1 bundles while allowing the scalar-projection digest to be non-zero,
so lineage-proving D2D states can be represented by the accumulator and
their proof artifacts remain domain-separated from semantic v1 artifacts.
`kagemusha_recursive_spend_proof_artifact_digest` exposes that canonical
previous-proof digest to the core prover and SDK wrappers, so
Reserved-lineage append circuitry can bind the exact previous recursive proof
without each host inventing a separate hash. The ABI-6 bridge, Node native host,
and Python PyO3 host now serialize semantic
redeem instructions only when a record-backed lineage witness verifies the hop
records, Pallas open envelopes, intermediate recursive proofs, and final
accumulator binding. Witnessless semantic redeem requests return no instruction
bytes without a record-backed lineage witness. Witnessless Reserved-lineage
redeem requests serialize instruction bytes when
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64`,
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = true`, and the
bundle passes verifier-record, proof-envelope, chain/asset, final commitment,
root, and nullifier checks. Malformed Reserved-lineage requests continue to
fail earlier during profile validation.
Recursive spend append requests carry an optional
`previous_lineage_verifier_record`. Semantic v1 previous proofs use the
canonical recursive aggregation verifier and must leave this field empty.
Reserved-lineage previous proofs must provide the active lineage verifier record
so the append prover verifies the previous proof before folding the next hop;
missing records, semantic verifier records, malformed records, and tampered
previous proofs fail closed and the bridge returns no output bytes.
Swift, Kotlin/JVM, Java Android, JavaScript, Python, and C# expose stable
constants for both circuit ids so wallets can classify semantic v1 versus
reserved lineage bundles without duplicating string literals.
Redeem admission rejects both malformed redeem proof envelopes and
self-consistent envelope mutations whose refreshed hashes still expose public
instances for the wrong final spendable note. It also rejects recursive
proof-chain and scalar-projection public-input substitutions before spending
the anchor or final-note nullifiers.
Spendable note descriptors are deliberately narrow: note commitments and spend
nullifiers must be non-zero and distinct, the amount must be positive, scale
`0`, and fit the u128 public redeem amount corridor. Receivers reject malformed
notes, append-time amount drift, and tampered top-up anchor sets before storing
re-spendable cash.
Recursive spend append treats verifier opening length, verifier-parameter
fingerprint, fixed-window schedule digest, and shared-table manifest digest as
stable verifier context across hops. The fixed-window table-base digest is
proof-witness-specific, so the accumulator folds it through a separate
recursive-spend stream alongside the verifier-witness batch digest instead of
requiring it to remain identical across legitimate re-spends.

Kagemusha proof attachments are transparent-only. Chain-side Kagemusha transfers
currently require the literal confidential-transfer-v2 circuit id
`halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified`. Normalized
aliases such as `anon-transfer-2x2-merkle16-poseidon-diversified` are rejected
before proof verification, even if the verifier record and proof envelope agree
on the alias. Trusted-setup labels such as KZG/Groth16/BN254 are rejected before
proof verification, including standalone labels such as `kzg`, `bn254`, and
`bls12_381`, colon-delimited profiles such as `halo2/ipa:kzg`, and explicit
trusted-setup markers such as `srs`, `crs`, `ptau`, `ceremony`,
`trusted-setup`, `structured-reference-string`, and `powers-of-tau`. The
shared classifiers match those setup and developer-only markers
ASCII-case-insensitively, and setup markers are tokenized across every
non-alphanumeric delimiter, so punctuation-spliced profiles such as
`stark/fri/prod;kzg`, `stark/fri/prod+bn254`,
`stark/fri/prod-bls12-381`, `stark/fri/prod-s-r-s`, or
`stark/fri/prod-powers-of-tau` fail closed before broad STARK/FRI allowlists are
considered. Mixed or padded labels such as `halo2/ipa: KZG` or
`halo2/ipa:Mock-Proof` therefore cannot pass broad allowlists, and
delimiter-inserted setup spellings such as `stark/fri/prod-bn-254`,
`stark/fri/prod-groth-16`, `stark/fri/prod-k-z-g`, and
`stark/fri/structured-reference-string` are normalized before classification.
Developer-only spellings are normalized the same way, so
`stark/fri/d-e-b-u-g`, `stark/fri/m-o-c-k`, and
`halo2/ipa:m-o-c-k-proof` cannot be hidden inside otherwise valid profile
syntax.
and `stark/fri/` profiles with empty, padded, embedded-whitespace, non-ASCII, or
punctuation-bearing suffixes are rejected before STARK/FRI dispatch. Production
STARK/FRI profile suffixes are limited to ASCII alphanumeric characters plus
`-`, `_`, and `.`. The attachment
backend, proof backend, and verifier-key reference backend must match exactly.
The attachment must also publish the asset-bound verifier-key commitment and a
non-empty verifier-key id name; missing or empty
trust-anchor metadata is rejected before envelope decoding. Chain-side transfer
and recursive redeem admission also reject all-zero verifier-key commitments
and all-zero proof-envelope verifier-key hashes before registry comparison, so
zero cannot act as a wildcard for corrupted WSV bindings. Kagemusha verifier keys
must carry non-empty inline bytes before chain-side transfer admission, checked
fold construction, or compact-token record verification can proceed.
Record-backed compact-token verification, recursive aggregation
preverification, and checked fold-hop admission also reject all-zero verifier
record commitments before comparing inline verifier-key hashes.
The same production envelope policy is now shared by generic `VerifyProof`,
governance voting proofs, STARK shielded transfer/unshield wrappers,
IVM-proved overlays, IVM host registered-key verify syscalls, and Kaigi privacy
proofs. RAM-LFE proof receipts used by generic program policies and identifier
claims now follow the same canonical envelope rule. A submitted
`OpenVerifyEnvelope` may not use a zero verifier-key hash as a wildcard, its
`vk_hash` must exactly match the active registered verifier-key commitment, and
auxiliary bytes must be empty before backend verification starts. The shared
metadata validator rejects zero verifier-key hashes explicitly before generic
commitment mismatch handling, and ZK-ACE STARK authorization envelopes enforce
the same fail-closed rule. RAM-LFE execution receipts, identifier RAM-LFE
receipts, and Kaigi privacy proofs also reject zero verifier-key hashes before
their verifier-key commitment comparisons.
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
The canonical Offline recursive verifier-key envelope also carries a `CID1`
circuit-id TLV, and the backend verifier rejects a matching raw Halo2 key
payload when that TLV names a different semantic circuit.
The confidential-transfer-v2 and unshield prover builders use the same
process-local optimization for canonical circuits: they reuse cached derived
Halo2 IPA proving keys for canonical verifier-key envelopes and retain the
existing arbitrary-key derivation path for noncanonical test/custom keys.
The Offline recursive prover and chain verifier require the literal
`offline-note-recursive` circuit id; alias spellings such as
`halo2/ipa:offline-note-recursive` are rejected before proof generation or
backend verification.
The Offline recursive prover, proving-key derivation helper, and chain-side
verifier resolver also require the inline verifier key to match the canonical
`offline-note-recursive` semantic circuit key, so a self-consistent forged
record cannot substitute different verifier-key bytes even when its commitment
and proof envelope hash agree.
Serialized Halo2 IPA proving keys are stored as Norito archives that bind the
canonical circuit family and verifier-key commitment before the raw Halo2 key is
decoded, so Offline Note, IVM execution, and Kagemusha folded provers reject raw
or cross-circuit proving-key material before proof creation.
Chain-side Offline recursive verifier resolution uses the shared trusted-setup
and developer-only backend classifier before verifier-registry lookup, so
Groth16/KZG/BN254/BLS12 labels and labels containing `debug` or `mock` fail at
the proof metadata boundary. It also rejects all-zero proof-envelope
verifier-key hashes before verifier-record comparison, so zero cannot act as a
wildcard in legacy Offline recursive proof admission either.
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
`stark/fri/<profile>` with a non-empty ASCII profile containing only
alphanumeric characters plus `-`, `_`, and `.`, not as a bare trailing-slash
prefix, padded label, or punctuation wildcard; the shared core backend classifier
and Torii proof/prover paths enforce the same profile rule before
Offline/Kagemusha verifier admission reaches proof decoding. STARK/FRI profiles
that name trusted-setup curves or openings such as KZG, BN254, or BLS12, or
explicit setup markers such as SRS/CRS/PTAU/ceremony, or developer-only
`debug`/mock markers, are rejected by the same ASCII-case-insensitive
classifier.
`KagemushaCompactPaymentToken` wraps those public inputs with a transparent
folded proof and rejects tokens whose proof-declared public-input hash does not
match the canonical transcript before backend verification runs. The final
folded Halo2 IPA envelope must also be in canonical form with empty auxiliary
bytes, so unbound application metadata cannot be smuggled through a proof that
the backend verifier would otherwise accept. Folded-token proving, proving-key
derivation, and direct or record-backed token verification also require the
canonical `kagemusha-folded-v1` semantic verifier key, so a self-consistent
forged verifier record cannot swap in the recursive aggregation key or another
cross-circuit key before backend verification. The folded verifier-key envelope
carries the same `CID1` circuit-id TLV, so backend verification also rejects a
replay where the raw Halo2 key payload matches but the verifier-key envelope
names another circuit family.
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
optional inline key bytes, and the canonical confidential-transfer-v2 semantic
verifier key before the hop can enter the folded transcript.
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
verifier-key bytes, the canonical `pallas` curve label, a matching key length, a
matching verifier-key commitment, and a non-zero proof-size cap before the proof
envelope is decoded; non-empty auxiliary bytes are rejected there as a
non-canonical envelope. Duplicate input
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
The Python SDK exposes the same record-backed compact-token prover through the
native PyO3 extension, so Python wallets no longer need to drop to the C bridge
to exercise the production Kagemusha path.
Bridge ABI 6 exposes the production recursive spendable-cash entry points:
`connect_norito_kagemusha_recursive_spend_init`,
`connect_norito_kagemusha_recursive_spend_append`,
`connect_norito_kagemusha_recursive_spend_transition_profile_init`,
`connect_norito_kagemusha_recursive_spend_transition_profile_append`,
`connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result`,
`connect_norito_kagemusha_recursive_spend_lineage_witness_append_result`,
`connect_norito_kagemusha_recursive_spend_verify`, and
`connect_norito_kagemusha_recursive_spend_redeem`. All nine entry points accept
and return raw Norito archives so SDKs do not implement recursive proof internals,
accumulator derivation, or witness merging. The data model round-trips the raw
archive contracts for `init`, `append`, transition-profile preflight,
lineage-witness assembly, `verify`, `verify_result`, and `redeem` so SDK
wrappers share one Norito ABI shape. The
offline recipe is: load/top-up online, build the first
`KagemushaRecursiveSpendBundleV1` with `init`, verify and store the bundle on
receipt, append one verified hop plus the new spendable note descriptor for
every offline re-spend, and call `redeem` when the final holder comes back
online. Rust callers should use the validated `new` constructors on
`KagemushaRecursiveSpendInitRequestV1`,
`KagemushaRecursiveSpendAppendRequestV1`,
`KagemushaRecursiveSpendVerifyRequestV1`, and
`KagemushaRecursiveSpendRedeemRequestV1` before serializing request archives;
these constructors run the same public-binding guards as the native bridge
request boundary. Rust callers should use
`prove_kagemusha_recursive_spend_lineage_init_from_record_bundle_and_pallas_open_envelope_archive`
for the reserved-lineage first-hop proof surface. The generic recursive spend
append helper and ABI-6 append request now honor
`output_proof_circuit_id`: missing, empty, or semantic
`kagemusha-recursive-aggregation-v1` preserves the compatibility output path,
while explicit `kagemusha-recursive-spend-lineage-v1` selects the guarded
Reserved-lineage output path. ABI-6 `init` produces the current
`kagemusha-recursive-spend-lineage-v1` one-hop verifier-slice proof: the core
helper decodes the supplied Pallas open-envelope archive, derives the opening
length, selects the matching lineage verifier key, and returns a
reserved-lineage recursive spend bundle. ABI-6 `append` verifies semantic
previous proofs with the semantic recursive verifier and Reserved-lineage
previous proofs with the supplied active lineage verifier record, then selects
the output verifier key from the requested output circuit id. New wallets should
use the preferred append selector, which keeps Reserved-lineage output selected
for previous hop counts `1..63`; the semantic output remains available as the
legacy ABI-6 compatibility path. Callers that still use semantic output must
derive and retain the matching
`KagemushaRecursiveSpendLineageWitnessV1` with
`lineage_witness_from_init_result`, update it with
`lineage_witness_append_result` after each append, and attach that witness at
redeem. Because semantic recursive spend proofs verify only the public
accumulator columns, native bridge and SDK `verify` results are split between
offline spendability and chain admission: a semantically valid recursive proof
without record-backed lineage returns `valid = true` for receiver-side offline
acceptance, while `chain_admissible = false` carries the same
private-hop-lineage diagnostic that ledger redemption would emit.
Reserved-lineage proofs use the recursive
verifier-slice profile for witnessless chain admission inside the 64-hop cap.
Verify
results also expose `witnessless_redeem_supported` and
`lineage_witness_required_for_redeem`, so wallets that do not decode bundle
internals can still decide whether to keep or attach the record-backed lineage
witness before going online. The
native bridge and SDK `redeem` entry points are fail-closed on the
chain-admission gate:
semantic v1 bundles serialize redeem instructions only when the request carries
a verified record-backed lineage witness and the final recursive proof verifies
against the transparent recursive verifier key. Witnessless semantic v1 bundles,
tampered final recursive proofs, malformed reserved-lineage bundles,
reserved-lineage bundles missing their scalar projection, reserved-lineage
bundles missing verifier-slice columns, and over-cap Reserved-lineage bundles
return no instruction bytes. Metadata-valid Reserved-lineage bundles with
strict verifier-slice columns are profile-checked and can redeem witnesslessly
inside the 64-hop cap. Record-backed lineage validation also keeps the final
redeem note nullifier disjoint from every earlier lineage input and every
lineage output commitment, and core mirrors that check before Pallas replay so
nullifier/commitment splices fail with a deterministic preflight error. SDKs expose
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64`; wallets use
the helper predicates to decide when a record-backed semantic witness is still
required.
The SDK helpers `canRedeem...Witnessless` and
`requires...LineageWitnessForRedeem` expose the same 64-hop Reserved-lineage
decision so app code does not duplicate the rule. SDKs can still carry
record-backed lineage witnesses for semantic v1 production redemption and as an
over-cap compatibility fallback.
Append requests whose previous bundle already uses the reserved-lineage circuit
id must include the current lineage verifier record in
`previous_lineage_verifier_record`; semantic previous bundles must omit it. The
append request also carries the defaulted `output_proof_circuit_id` selector and
the defaulted `previous_recursive_proof_open_envelopes_archive` slot for the
production witnessless Reserved-lineage append circuit. Missing, empty, or
semantic output selectors preserve legacy ABI-6 behavior and may leave the
previous-proof archive empty. Append attempts that select
`kagemusha-recursive-spend-lineage-v1` as the output proof circuit must provide
it so the append verifier-slice circuit can consume the previous recursive proof's opening
material. The archive must be a Norito archive containing exactly one
`iroha_zkp_halo2::OpenVerifyEnvelope` at the data-model boundary and native
append preflight. The previous recursive proof envelope must carry the same
backend and circuit id as its verifier-key reference. The opening envelope's
verifier-key commitment, public-input schema hash, and recursive-proof domain
tag must match that checked previous recursive proof, so a caller cannot splice
a valid opening witness from a different recursive proof into a Reserved-lineage
append request. Malformed, mismatched, or missing required archives are rejected
before native proving starts. SDKs must
treat `previous_recursive_proof_open_envelopes_archive` as opaque native prover
material and must not construct, rewrite, or mutate it; the native bridge and
SDK append wrappers validate the metadata tuple (`vk_commitment`,
`public_inputs_schema_hash`, `domain_tag`) against the exact previous bundle
before proving or returning output bytes. Native append
preflight also caps the archive at
`KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES` (8 MiB) before
decoding and checks that each supplied previous-proof envelope is bounded Pallas
IPA opening material with non-zero verifier-key, public-input schema, and
recursive-proof domain metadata; matching parameter/public lengths; valid
generator and proof round counts; and an IPA opening proof that can derive a
verifier witness under lineage-sized resource limits. SDK recursive-spend
helpers expose the same required envelope count and 8 MiB cap so wallets can
preflight the archive before native calls. SDKs expose append-side branching
helpers alongside the redeem helpers:
`canAppend...WitnesslessLineage(previousHopCount)` returns true for previous
hop counts `1..63`, and
`requires...PreviousProofOpenEnvelopesForAppend(outputCircuitId,
previousHopCount)` returns true for Reserved-lineage append attempts after an
existing previous bundle so wallet code can populate the required archive slot.
SDKs also expose
`preferred...AppendOutput...CircuitId(previousHopCount)`, which is the
recommended selector for new append requests; in this release it returns the
Reserved-lineage circuit for previous hop counts `1..63`, while first-hop
`init` continues to produce the one-hop Reserved-lineage bundle.
`canProve...AppendOutput...CircuitId(outputCircuitId, previousHopCount)`;
it returns true for semantic recursive append outputs through hop 64 and for
Reserved-lineage append outputs when the previous hop is `1..63`, so wallets
can select witnessless Reserved-lineage append while staying inside the cap.
`canSelect...AppendOutput...CircuitId(previousProofCircuitId, outputCircuitId,
previousHopCount)` applies the same proving check plus the previous-proof
transition rule before a wallet serializes the append request.
`isSupported...PreviousProof...CircuitId(previousProofCircuitId)` and
`requires...PreviousLineageVerifierRecordForAppend(previousProofCircuitId)` let
wallets reject unknown previous recursive proof circuits and include the
previous lineage verifier record only for Reserved-lineage previous bundles.
`normalize...AppendOutput...CircuitId` and `isSupported...AppendOutput...CircuitId`
helpers let wallet code apply the same missing-or-empty-to-semantic selector
rule before constructing Norito archives. A Reserved-lineage append output also
requires the previous bundle to already be Reserved-lineage; semantic previous
proofs cannot be upgraded into witnessless Reserved-lineage by supplying only a
previous-proof opening archive, and must keep using semantic append plus a
record-backed lineage witness for redemption.
The Rust data model also exposes
`KagemushaRecursiveSpendTransitionProfileV1`, built by the initial and append
transition-profile helpers, as the canonical Reserved-lineage accumulator
transition contract. The profile is a Norito object and digest that binds the
previous accumulator digest, previous recursive proof artifact digest, previous
accumulator public-input hash, previous recursive proof public-input hash,
previous proof opening-archive digest when raw request bytes are supplied,
normalized hop index, current hop statement, current note, verifier-witness
batch digest, fixed-window table-base digest, resulting
lineage/proof-chain/table/nullifier/output/fold digests, resulting accumulator
digest, and resulting recursive public-input hash. Request-backed append
transition profiles additionally bind
`previous_recursive_proof_open_envelopes_archive_digest`; legacy evidence-only
helpers omit it for compatibility. Reserved-lineage append proof output also
binds `append_opening_preflight_digest` and, when
native hosts computed the two Pallas preflights, the full
`KagemushaRecursiveSpendLineageAppendOpeningPreflightV1` Norito contract. The
contract must hash back to the digest and match the previous accumulator digest,
previous recursive proof artifact digest, previous opening archive digest,
current-hop proof hash, and current-hop verifier preflight aggregate. The digest
is valid only when the previous opening archive digest is also present, so SDKs
should use raw-archive helpers for actual append requests. The archive-aware
Rust helper validates non-empty previous-proof opening archive metadata against
the exact previous bundle before hashing it. Append profiles require the
previous accumulator public-input hash to equal the previous recursive proof
public-input hash before the profile can be digested. Append profiles normalize
a one-hop transport fragment to the accumulated hop index before hashing; for
example, the second offline spend hashes as hop `1`. SDK fixtures should compare
against this profile with
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64`; matching the
profile is required circuit preflight, and chain admission also verifies the
lineage proof envelope and final redeem binding. Core init and append proof constructors build and
compare the profile with the accumulator before emitting an ABI-6 recursive
spend bundle, so bridge callers get the same Reserved-lineage transition guard
as Rust fixtures. The native bridge and SDK raw-archive append
transition-profile helpers now also compute this append opening preflight when
`previous_recursive_proof_open_envelopes_archive` is
present, bind its digest into `append_opening_preflight_digest`, and keep the
legacy no-digest profile only when the previous opening archive is absent. The
SDKs should treat the contract as opaque native verifier metadata and compare or
roundtrip the Norito bytes; they must not synthesize it from partial fields.
Core also exposes a Reserved-lineage append opening preflight
contract that validates and digests the two Pallas opening witnesses the
production append circuit must prove: the previous recursive proof opening,
bound to the exact previous bundle and previous opening archive, and the current
checked-hop opening, bound to the checked-hop proof hash. The two openings must
also share the same verifier context before the contract can digest: opening
length, Pallas parameter fingerprint, fixed-window schedule digest, and
shared-table manifest digest. Core checks that this contract is attached to a Reserved-lineage
previous proof, that its previous accumulator/proof artifact digests match the
actual previous bundle, and that the resulting accumulator public inputs carry
the same append opening preflight digest, compact append-boundary digest, and
verifier corridor. The Reserved-lineage append proof branch binds those digests
into the transition profile before emitting output. This material is circuit
input and remains bounded by the 64-hop witnessless cap. The native C
bridge, Swift, Android Java/Kotlin, JavaScript,
Python, and C# SDK surfaces expose raw-archive transition-profile init/append
helpers and the stable
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1`
domain string, and require those symbols in their ABI-6 availability probes.
They also expose the compact
`KagemushaRecursiveSpendLineageAppendBoundaryV1` archive through
`connect_norito_kagemusha_recursive_spend_lineage_append_boundary` and matching
SDK helpers such as `lineageAppendBoundary`/
`kagemushaRecursiveSpendLineageAppendBoundary`. This boundary is derived only
from a full append transition profile with an attached opening preflight
contract. It binds the profile digest, profile binding digest, explicit
chain/asset binding digest, explicit final-root/current-note binding digest,
previous accumulator and proof artifact digests, previous opening archive
digest, previous/current opening aggregate digests, current-hop proof hash,
resulting accumulator digest, the boundary-free resulting public-input hash,
hop count, verifier opening length, and verifier-context fingerprints under the
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1` domain. SDKs
should treat it as opaque Norito verifier material; detached, stale, zero, or
one-hop boundary fields reject before append proving.
Full-contract Reserved-lineage append accumulators now store this compact
boundary digest in `KagemushaRecursiveSpendAccumulatorV1.append_boundary_digest`.
The accumulator digest intentionally blanks that field before hashing, and the
compact boundary uses the resulting public-input hash with append-boundary limbs
blanked, so the boundary digest can be placed back into final recursive proof
public inputs without a fixed point. Digest-only compatibility append builders
leave `append_boundary_digest` zero; a Reserved-lineage proof that exposes a
nonzero append-boundary public input must match the accumulator field exactly.
`KagemushaRecursiveSpendLineageAppendBoundaryV1::validate_against_transition_profile`
re-derives the compact boundary from the full transition profile and compares
every field, so a detached boundary with a refreshed self-digest still rejects
if its previous accumulator, append-opening digest, opening aggregate,
resulting public-input hash, verifier context, or hop count was spliced from a
different transition. The native bridge plus Swift, Android Java/Kotlin,
JavaScript, Python, and C# SDK wrappers call this validator before returning
boundary archives or surfacing them to wallet code.
SDKs also expose a structural append proof-transition helper: semantic
previous proofs may only append semantic output, Reserved-lineage previous
proofs may append semantic output for compatibility or Reserved-lineage output
for witnessless spend-again-offline while the previous hop is `1..63`. Request
validators require the previous Reserved-lineage verifier record and
previous-proof opening archive before selecting Reserved-lineage output. The
native bridge and SDK append wrappers derive the append Reserved-lineage
verifier key from that opening archive so archive, transition-profile, and
opening-contract checks run before proving. In this context, "one-hop
Reserved-lineage" means the active lineage verifier slice proves exactly the
first private hop and constrains `hop_count == 1`; it is sufficient for
receiver-side verification of the first offline receipt, but it is not the
multi-hop append verifier.
Core now also carries the append verifier-slice circuit scaffold and host
contract: `KagemushaRecursiveAggregationAppendVerifierSlice` composes recursive
aggregation semantics with two shared-table IPA verifier slices, requires
append-shaped recursive public inputs (`hop_count >= 2`), nonzero transition,
append-opening, append-boundary, and scalar-projection public fields, exactly
one previous-recursive-proof opening preflight, exactly one current-hop opening
preflight, and an in-circuit combined scalar-projection digest over both
verifier slices without adding per-hop projection public columns. Heavyweight
ignored MockProver coverage exercises the honest two-opening profile plus
scalar-projection, append-boundary, and current-verifier transcript splices.
The Halo2 IPA backend profile classifier now distinguishes one-hop and append
Reserved-lineage instance layouts under the same reserved circuit-family id:
one-hop layouts continue to dispatch to the one-hop verifier slice, while
append-shaped layouts dispatch to the append verifier-slice circuit only when
the supplied verifier key matches that append circuit.
The append proof constructor now has a concrete append-circuit target: it revalidates the previous
recursive proof opening against its detached preflight, revalidates the current
hop opening against the hop-bound preflight, builds the two-verifier append
slice, and uses an append-specific proving-key cache keyed by the supplied
append verifier key. Core also exposes a separate append verifier-key builder
for this circuit; the compatibility lineage verifier-key builder remains
one-hop for first-hop init and legacy verifier records. Product witnessless
append output is reachable below the 64-hop cap.
Verify requests whose received bundle uses the reserved-lineage circuit id must
include the current lineage verifier record in `lineage_verifier_record`;
semantic v1 verify requests must leave that field empty. Native bridge and SDK
verifier wrappers return `valid = false` with a stable missing-record
diagnostic when a reserved-lineage D2D payload is presented without that
record, so receivers do not store or re-spend unverifiable witnessless cash.
Init and append request archives now have the same cheap public-binding
preflight before any recursive prover is invoked. Init preflight validates the
one-hop record-backed fragment, Pallas envelope archive count, exact verifier
records, and the first spendable note's output/nullifier binding. Append
preflight additionally validates previous-bundle proof metadata, semantic vs.
reserved-lineage previous-record selection, output proof circuit selection,
required previous-proof opening archives for Reserved-lineage output append,
chain/asset/root continuity, amount preservation, previous-note nullifier
consumption, and top-up-anchor/output non-overlap. The native bridge plus SDK
init/append entry points call these validators before constructing Halo2
proving keys or returning native output bytes, so malformed D2D fragments fail
consistently across SDKs.
Rust and SDK callers can assemble the separate redeem witness with
`kagemusha_recursive_spend_lineage_witness_from_init_result` after the first
`init` proof and `kagemusha_recursive_spend_lineage_witness_append_result`
after each `append` proof. These helpers first run the same init/append
public-binding preflight as the prover path, then validate one-hop
record-backed fragments, exact verifier-record sets, Pallas open-envelope
archive counts, Pallas transcript labels, non-zero opening metadata, Pallas
curve ids, version fields, opening-length bounds, generator counts, IPA round
counts, root continuity, duplicate or overlapping lineage nullifiers/commitments,
accumulator initial/final root binding, current-note/output collisions, and the
proof-attachment/backend/inline-key shape for every hop. Halo2 IPA fragments
also bind Pallas opening `vk_commitment` and `public_inputs_schema_hash`
metadata to the hop attachment and transparent hop proof envelope. They also reject
inactive, missing-key, over-proof-cap, commitment/key-length-mismatched,
namespace-mismatched, backend/curve-mismatched, empty-circuit, or zero-schema
verifier-record snapshots before the witness is attached to a redeem request.
SDKs expose the shared
`KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128`
bound for early UI/client checks, but native Norito validation remains
authoritative.
They merge the envelope archive, carry forward ordered semantic previous
recursive proofs, and reject record conflicts, chain/asset-spliced append
fragments, stale previous bundles, and stale appended bundle results before the
witness is attached to a redeem request.
Reserved-lineage previous proofs are accepted at this helper boundary only when
the append request carries an active matching lineage verifier record; semantic
previous proofs reject that record. Direct redeem-request validation mirrors the
same selection rule: a final Reserved-lineage bundle must carry the active
lineage verifier record, and semantic final bundles must leave it empty unless a
record-backed witness path explicitly needs the record to verify prior
Reserved-lineage proofs.
Ledger execution repeats that witness verification and additionally rejects
lineage witnesses whose hop verifier-record snapshots are stale, missing,
duplicated, unreferenced, or absent from the current WSV registry.
For recursive redeem instructions, ledger execution validates the final
unshield/redeem proof public binding before attempting reserved-lineage
chain-admission checks. Those checks admit witnessless Reserved-lineage bundles
inside the 64-hop cap only after verifying the active lineage verifier record,
the recursive proof envelope, chain/asset binding, final commitment binding,
root freshness, and nullifier set. A tampered final spendable-note binding
therefore fails at the final-proof gate instead of being masked by lineage-key
metadata.
Appenders must provide the previous recursive proof to the native append
builder; SDKs should not derive the accumulator state themselves. The CI benchmark
`kagemusha_recursive_spend_payload_bytes` records constant fixture archives for
1, 2, 3, 5, 8, 13, 21, 34, 55, and 64 hops when the proof payload is fixed at
256 bytes; production proof bytes can change the absolute number, but the
Norito D2D archive size is asserted not to grow with hop count. The data-model
CI guard also pins the current fixed-proof recursive spend bundle at 1,751 bytes
and fails if the same fixture crosses the 2,048-byte material-growth ceiling.
The same benchmark emits `kagemusha_recursive_spend_transition_profile_bytes`
and the CI reducer writes `transition_profile_bytes.tsv`; append transition
profiles are built through the archive-aware helper with metadata-bound
previous-proof opening archives and are asserted hop-count-independent
separately from the D2D bundle so the Reserved-lineage accumulator preflight
overhead is visible without loosening the receiver payload gate. The current
fixed-proof semantic append transition profile is 2,094 bytes.
The benchmark also emits
`kagemusha_recursive_spend_reserved_lineage_payload_bytes` and
`kagemusha_reserved_lineage_transition_profile_bytes`. Those
fixtures use Reserved-lineage proof ids, metadata-bound previous recursive proof
opening archives, full append opening-preflight contracts, derived compact
append boundaries, and accumulator-carried `append_boundary_digest` values. The
CI reducer writes `reserved_lineage_payload_bytes.tsv` and
`reserved_lineage_transition_profile_bytes.tsv` and checks those series for the
same hop-count independence. The current fixed-proof Reserved-lineage D2D bundle
is 3,833 bytes, and the append Reserved-lineage transition profile is 2,817
bytes, with separate size ceilings because the Reserved-lineage fixtures carry
proof-opening metadata that the compatibility semantic payload does not.
The dedicated `Kagemusha Payload Benchmark` workflow runs
`ci/check_kagemusha_recursive_spend_payload_bench.sh` on relevant Kagemusha
payload, accumulator, and proof-surface changes and uploads the reduced-sample
Criterion summary.
Bridge ABI 6 also retains
`connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes`.
That proof-carrying entry point accepts the same record-backed bundle plus a
Norito archive of proof-derived Pallas opening envelopes, enforces active
verifier-record and hop-proof checks, binds the Pallas envelope metadata to each
hop proof, and returns a Norito-encoded
`KagemushaRecursiveAggregationProofBundle`. It is still admission-neutral:
the Python native extension mirrors the same path for local proof-bundle
generation.
Compact-token aggregation mode `2` remains reserved until the recursive circuit
verifies private-hop opening evidence in-circuit.
Malformed bundles, oversized bundle hop counts, malformed hop shapes, root
discontinuities, duplicate nullifiers or output commitments, tampered hop
proofs, oversized hop proof payloads, missing or inactive records, verifier
schema/commitment/proof-cap mismatches, duplicate or extraneous verifier
records, self-consistent confidential-transfer-v2 circuit-id aliases, forged
envelope-hash metadata, non-canonical hop auxiliary bytes, and reserved
aggregation modes are rejected before compact proof generation.
Swift exposes this record-backed path as
`KagemushaCompactPaymentTokenProver.proveVerifiedCompactPaymentTokenWithRecords`,
and the recursive proof-bundle path as
`KagemushaRecursiveAggregationProofBundleProver.proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes`.
Kotlin/JVM plus Java Android expose mirrored `KagemushaCompactPaymentTokenProver`
and `KagemushaRecursiveAggregationProofBundleProver` native wrappers. These SDK
APIs accept and return raw Norito archives so wallet code can keep using the
Rust data-model layout while the native bridge performs the proof and
verifier-record checks. Swift, Kotlin/JVM, Java Android, JavaScript/Node,
Python, and C# also expose `KagemushaRecursiveSpend*` helpers around the ABI 6
recursive spend init/append/lineage-witness/verify/redeem surface, with
empty-input and malformed-archive rejection before native calls where the host
language can preflight. Swift exposes the witness helpers as
`lineageWitnessFromInitResult` and `lineageWitnessAppendResult`; Kotlin/JVM and
Java Android expose the same names, JavaScript/Node and Python use the native
snake/camel-case bridge names, and C# exposes
`LineageWitnessFromInitResult`/`LineageWitnessAppendResult` DTO wrappers.
Python, Swift, JavaScript/Node, Kotlin/JVM, and Java Android also fail closed
when proof-producing native calls return no archive or a zero-length archive,
so missing native proof material cannot be coerced into a successful SDK
result. The Swift dynamic loader now requires bridge ABI 6 and the
record-backed plus complete recursive-spend Kagemusha symbols, including both
lineage-witness assembly helpers, before probing those symbols with malformed
Norito archives. Swift reports native compact-token, recursive-aggregation, and
recursive-spend Kagemusha provers as available only when the loaded bridge
returns the expected Kagemusha rejection without output bytes, and the Swift
recursive-spend wrapper refuses to select `recursive_spend_v1` unless the full
ABI 6 surface passes that probe.
JavaScript/Node, Python, Kotlin/JVM, Java Android, and C# apply the same
fail-closed rule in their native availability probes: init, append, both
transition-profile helpers, the append-boundary helper, both lineage-witness
helpers, verify, and redeem must be callable from the loaded native bridge
before wallet code is told recursive redemption is supported.
JavaScript/Node, Python, Swift, Kotlin/JVM, Java Android, and C# also treat
malformed native loading or ABI-version probing as unavailable, and their
availability probes now reject a native symbol that accepts empty or malformed
archives. Only the expected Kagemusha rejection across the required symbol set
can prove that a loaded native surface exists. When a non-empty recursive spend
redeem request is rejected by the native bridge, SDK wrappers surface that
rejection instead of substituting fallback bytes or downgrading it to bridge
availability.
Python direct helper calls and the optional C# P/Invoke wrapper also require
that complete ABI-6 surface before producing any recursive spend output; C# also
requires the expected Kagemusha empty-archive bridge error during symbol probes.
A partial or permissive native bridge cannot emit an init or append bundle
without the witness helpers needed for later redemption.
The Swift wrapper also exports the same ABI-6 requirement for wallet-side
capability checks.
JavaScript/Node and Python now require an ABI-6 native version probe before
reporting recursive spend as available or selecting `recursive_spend_v1`; the
Node NAPI host exports `connectNoritoBridgeAbiVersion`, while the Python PyO3
extension exports `kagemusha_recursive_spend_bridge_abi_version`.
Kotlin/JVM and Java Android also call the bridge ABI-version JNI probe and
probe the verify plus both lineage-witness JNI symbols before reporting
recursive spend as available or defaulting to `recursive_spend_v1`. C#
publishes the same ABI-6 requirement and probes verify plus both
lineage-witness P/Invoke symbols before its optional wrapper calls the bridge.
All SDKs expose the same default spend-mode choice:
`recursive_spend_v1` is selected when the recursive spend ABI 6 surface is
available, and `checked_prefold_v1` remains the compatibility fallback for
older runtimes that only provide the record-backed compact-token path.
Verifier records for chain-side transfers, recursive final redeem/unshield,
record-backed compact-token proving, and final folded-token record verification
must live in the canonical `offline_kagemusha` namespace and publish the
expected backend/curve pair. Chain-side transfer and final redeem/unshield
records must also be the active record for their circuit/version index and
embed the canonical confidential transfer or unshield verifier key for that
circuit; generic active confidential-transfer verifier records, stale circuit
bindings, substituted inline keys, or Halo2 IPA records claiming a non-`pallas`
curve are rejected before proof decoding.

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

The recursive spendable-cash path above is separate from the older compact
folded-token path. Checked pre-fold mode `1` remains available for compatibility
and continues to verify each hop proof before emitting the folded public
transcript. Compact-token aggregation mode `2` remains reserved for a future
fully in-circuit private-hop verifier inside the folded-token circuit; callers
that need spend-again-offline cash should use `KagemushaRecursiveSpendBundleV1`
and ABI 6 instead of attempting to enable mode `2`. The final aggregation proof
must continue to use no trusted setup and preserve the same public
nullifier/commitment/root semantics.
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
selections while keeping the same `a*b` product-link invariant. The bounded,
fixed-window, and shared-table fixed-window final MSM wrappers also have
explicit identity-output coverage, so final verifier comparisons that evaluate
to the canonical point at infinity stay on the complete-add/point-or-identity
constraint path. A per-round IPA
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
recursive witness shape by validating the transcript projection, re-deriving
the field-friendly transcript binding, `b`-reduction and accumulator
projections, round ordering, and canonical compressed point encodings in a cheap
preflight path. That preflight also recomputes the native Pallas `b`, `Q`, `G`,
`H`, and final-term fold relations before converting scalars and compressed
points only through canonical byte encodings. The group relation recomputation
uses the deterministic optimized Pallas MSM backend, so the production preflight
follows the same no-trusted-setup IPA backend path used by native verification.
The same bridge can now validate an ordered batch of
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
profile, verifier opening length, fixed-window table schedule/base digests, and
non-zero batch fields without making mode `2` accepted for compact-token
admission. The Poseidon2 aggregation transcript digest now deliberately accepts
both checked pre-fold mode `1` and reserved recursive mode `2`, while rejecting
unknown modes, so recursive evidence and future in-circuit verifier witnesses
bind the same transcript shape without opening compact-token admission. Focused
data-model coverage also roundtrips the evidence through Norito, validates the
decoded profile-bound digest, rejects decoded unsupported-profile evidence,
rejects unsupported or non-power-of-two opening lengths, rejects zero
schedule/base commitments, and rejects truncated evidence archives.
The reserved evidence validator also has explicit adversarial coverage for
empty transcripts, over-cap hop lists, duplicate input nullifiers, and duplicate
output commitments.
Core exposes record-backed evidence builders for that reserved path. They first
enforce the same active WSV-style confidential-transfer-v2 hop verifier records
used by production compact-token proving, verify every private hop proof, and
then bind the supplied native verifier-witness batch digest and parameter
fingerprint to the canonical hop transcript. Witness-count mismatches and
all-zero batch metadata are rejected before hop proof decoding. The core
builders also validate the declared Pallas verifier opening length before any
hop proof is decoded and, when the Halo2 IPA path is compiled in, recompute the
deterministic fixed-window schedule digest from that opening length so a
reserved evidence record cannot pair one verifier width with another width's
table schedule. The serializable `KagemushaVerifiedFoldRecordBundle` wrapper
follows the same checked path.
Core also exposes a public Pallas IPA batch preflight helper and record-backed
evidence builders that take the native verifier witnesses directly instead of a
detached digest tuple. A proof-derived variant reconstructs those native
witnesses from transparent Pallas polynomial-opening envelopes before applying
the same Kagemusha preflight and hop-proof-hash binding. That path applies the
Kagemusha `2..=128`/power-of-two opening-width corridor and the `k = 7`
transparent-envelope resource limit before verifier-witness derivation, so
oversized or unsupported Pallas opening envelopes, oversized generator vectors,
and oversized proof-round vectors are rejected before parameter or proof
reconstruction. Kagemusha proof-derived preflight also rejects empty or over-128
byte transcript labels and requires non-zero verifier-key commitment,
public-input schema, and hop-domain metadata before verifier-witness derivation,
so detached generic polynomial-opening envelopes cannot enter reserved
recursive evidence. Hop-proof hash count mismatches are rejected before native
witness preflight or proof-derived envelope witness derivation, so malformed
detached batches cannot force expensive recursive-verifier preflight work.
Record-backed builders also require every supplied Pallas
opening envelope to carry transcript metadata derived from its exact checked
hop: verifier-key commitment, the confidential transfer v2 schema hash, and a
Poseidon2 hop-domain tag over chain, asset, hop index, roots, nullifiers, output
commitments, proof hash, public-input digest, and verifier-key binding. The
helper accepts at most 64 witnesses, matching the compact-token hop cap, and
uses the 85-by-3 fixed-window Vesta verifier witness profile that covers the
255-bit Pasta scalar width without a trusted setup. Its aggregate digest is
Poseidon2-backed rather than a generic hash transcript, so the host-side
reserved evidence path stays aligned with the field-friendly no-trusted-setup
Kagemusha transcript surface. Native preflight exposes and the native batch
digest binds the recursive fixed-window table profile, the Poseidon2 digest of
the deterministic shared-table schedule, the shared-table manifest digest, and a
Poseidon2 digest of the ordered fixed-window table bases used by each native
witness. A valid witness batch therefore cannot
silently switch table accounting, table-family order, or actual table-base
encodings while keeping the same verifier-witness profile string. The combined
builders re-derive that digest with the opening length, proof count, schedule,
shared-table manifest, public table-base digest, native batch digest, and
ordered checked-hop proof hashes before storing it in reserved evidence, so a
valid detached native batch or proof-derived opening-envelope batch cannot be
replayed against a different folded-hop proof transcript, verifier opening
width, or table-base commitment without changing the evidence digest. Reserved
recursive evidence now carries the verifier opening length plus schedule,
shared-table manifest, and base digests explicitly as well as binding them
through the aggregate batch digest, rejects unsupported or non-power-of-two
opening lengths, rejects
all-zero schedule/manifest/base commitments, and rejects schedule or manifest
commitments that do not match the declared opening length before hop proof
decoding. The data model also defines proof-carrying recursive aggregation
public inputs and bundles whose 55 public instance columns bind a transparent
no-trusted-setup proof payload to the recursive evidence digest, aggregation
transcript digest, verifier-parameter fingerprint, fixed-window schedule
digest, shared-table manifest digest, table-base digest, native witness-batch
digest, recursive spend proof-chain digest, non-circular transition-profile
binding digest, Reserved-lineage append opening preflight digest, compact
Reserved-lineage append-boundary digest, reserved recursive verifier
scalar-projection digest, verifier opening length, witness count, and hop count
while rejecting backend,
circuit-id, public-input-hash, and evidence-field substitution. That
proof-carrying bundle is pinned to the
canonical transparent Halo2 IPA/Pasta recursive aggregation circuit and rejects
empty proof payloads before backend verification; STARK/FRI remains available
only for supported hop transcript material, not for this in-tree recursive
proof circuit. Standalone recursive aggregation evidence carries a
zero proof-chain digest, zero transition-profile binding digest, zero append
opening preflight digest, zero append-boundary digest, and zero recursive
verifier scalar-projection digest; `KagemushaRecursiveSpendBundleV1`
requires the public-input proof-chain, transition-profile binding, and append
opening preflight digests to equal the accumulator's
`recursive_proof_chain_digest`, `transition_profile_binding_digest`, and
`append_opening_preflight_digest`. The first two are nonzero for spend bundles;
the append preflight digest remains zero for init and semantic append, and is
nonzero only for Reserved-lineage append outputs. The append-boundary digest is
nonzero only for those Reserved-lineage append proofs and must be paired with a
nonzero append-opening preflight digest. The scalar-projection digest stays zero
until a composed verifier-slice proof is used.
This is still a reserved evidence binding, not the final in-circuit derivation
from each compact-hop Halo2 proof envelope.
Core preverification for that proof-carrying bundle now checks the transparent
Halo2 IPA `OpenVerifyEnvelope`, canonical circuit id, verifier-key hash,
public-input schema, empty auxiliary metadata, exactly 55 one-row Pasta public
instance columns, the fixed-window schedule and shared-table manifest digests
for the declared opening length, proof-size cap, active Kagemusha
verifier-record namespace, inline verifier-key length, and verifier-key
commitment before backend verification runs. It also rejects shortened,
extended, or multi-row recursive instance vectors before comparing the concrete
semantic public inputs, rejects Halo2 proof envelopes with trailing unbound
suffix bytes, rejects ZK1 inner proof envelopes with unexpected or duplicate
`PROF`/`I10P` TLVs, and rejects cross-circuit verifier keys before backend
verification, even when a forged verifier record and proof envelope are
self-consistent about the folded-token verifier-key commitment and `vk_hash`.
Core also
keeps the ZK1 public-instance parser bounded while allowing the 55-column
recursive aggregation envelope through the native bridge and backend verifier.
It also ships the transparent Halo2 IPA semantic proof/prover/verifier path for
the recursive aggregation evidence layout. The semantic circuit constrains the
opening-length corridor, binds the fixed-window schedule and shared-table
manifest digest limbs to the selected opening width, constrains the hop-count
corridor and witness-count equality, and rejects eight non-zero digest groups
without trusted setup. Recursive verifier-key containers carry a `CID1`
circuit-id TLV so registry commitments stay circuit-family-separated even if
the underlying small Halo2 verifier-key bytes collide, and backend verification
rejects structurally matching raw verifier-key payloads whose `CID1` names a
different circuit family. The verifier-record helper rejects empty or
non-production key material and binds the record to the recursive aggregation
schema and proof-size cap.
Record-backed combined proof-bundle builders now derive recursive evidence from
active hop verifier records plus either native Pallas verifier witnesses or
proof-derived Pallas opening envelopes, then immediately prove that evidence with
the canonical transparent recursive aggregation circuit. This prevents callers
from proving detached recursive evidence while the private-hop verifier relation
is still being completed. The lower-level detached-evidence prover and raw
metadata evidence builder are crate-private implementation helpers; public
callers must use the record-backed Pallas preflight proof-bundle entry points.
This proof path remains admission-neutral for compact tokens: aggregation mode
`2` is still rejected until the private-hop verifier evidence is checked
recursively.
The combined builders reject native witness-count
mismatches before native preflight or hop proof decoding, the public preflight
rejects empty witness batches directly, and native preflight then rejects
wrong-width witness/parameter pairings, transcript-binding, transcript,
reduction, accumulator, malformed table profiles, invalid opening-length,
schedule-digest, shared-table-manifest, or table-base metadata, unsupported
opening-envelope widths and malformed opening-envelope shape or wire versions
before witness derivation, mismatched Pallas generator counts, tampered opening
envelopes, mixed opening-parameter sets, missing or substituted opening-envelope
metadata, or final-term splices before constructing reserved-mode evidence whose
digest is profile-bound, opening-width-bound, schedule-bound,
shared-table-manifest-bound, table-base-bound, and hop-proof-hash-bound.
Core also exposes a lightweight production layout guard for the recursive Vesta
IPA verifier profile. For `n = 128` it pins seven IPA rounds, generator-fold
layers `[64, 32, 16, 8, 4, 2, 1]`, 85 fixed windows of 3 bits each (255 Pasta
scalar bits), and 262 represented windowed MSM gadgets. A companion table plan
pins the full-width cost model: 532 scalar-mul terms, 45,220 naive window-table
witnesses plus 45,220 duplicated selection-table witnesses, 723,520 naive point
rows, and a transparent shared-table target of 532 table families with 361,760
point rows and no trusted setup. Core now exposes the deterministic shared-table
schedule for that target and a Poseidon2 schedule digest, so future compressed
recursive evidence can commit to exact table-family order and shifted-window
ownership instead of relying only on prose counts. Core also exposes a concrete
shared-table manifest that maps those 532 families to contiguous shared-row
ranges, binds the manifest with Poseidon2, and keeps `trusted_setup_required =
false`. The first circuit-level compression primitive is also present: a
fixed-window selector can now read the already-derived Vesta table columns
directly instead of assigning a duplicate private selection-table copy, and the
shared-table native-scalar multiplication wrapper now composes that selector
with scalar decomposition, shifted-base table derivation, window-base doubling,
and selected-point accumulation without assigning selection-table copies. A
shared-table multi-term MSM wrapper now chains those scalar multiplications into
one public MSM output while keeping term outputs private, and the shared-table
final IPA MSM wrapper additionally constrains the third scalar to `a * b`. This
now extends through the per-round IPA accumulator and generator-fold wrappers,
which bind the shared-table MSM scalars back to the transcript challenge and
inverse without duplicated selection-table copies. The one-round shared-table
verifier slice composes those pieces with `b`-vector reduction and the
shared-table final IPA comparison, preserving the final-output and folded
generator cross-links. The shared-table multi-round verifier builder now
generalizes the same shape across all IPA rounds, including transcript-binding,
`b`-reduction, accumulator chaining, folded-generator chaining, and final-MSM
linkage without selection-table copies. It can now be constructed directly from
native Pallas verifier witnesses after the existing proof-derived Pallas
preflight validates transcript, `b`-reduction, accumulator, and generator-fold
consistency, and its ordered batch-preflight entry point now returns the same
schedule, shared-table manifest, table-base, and aggregate digests used by
reserved recursive aggregation evidence. Those recursive preflight digests now
field-label every length absorbed into the streaming Poseidon2 transcript, so
equal numeric lengths in different transcript positions cannot be replayed under
another field label. The public Kagemusha Pallas batch preflight now dispatches
through that shared-table verifier entry point, and the
production-width shared-table profile is checked against the 128-point manifest
without materializing every witness object. This keeps production shape coverage
cheap while the full recursive witness layout is being compressed; constructing
every per-term fixed-window table for `n = 128` is not production-viable for
normal unit tests and must be replaced by shared-table or otherwise compressed
circuit evidence before mode `2` can be accepted.
The LEN=4 two-round shared-table verifier now also has an explicit heavyweight
MockProver harness covering an honest synthetic statement, a real Pallas opening
translation, and public-instance, `Q`, generator-fold, challenge, and final-MSM
splices. Those cases are ignored by default because the composed non-native
layout is too expensive for routine validation, while builder, native Pallas
preflight, batch-preflight, and host-link adversarial tests remain in the normal
suite.
The first recursive-aggregation composition boundary is now represented by a
one-hop verifier-slice circuit: it composes the recursive aggregation semantic
public-input constraints with a const-generic shared-table IPA verifier,
including the transcript-binding accumulator, and links the public opening
length, witness count, and hop count to the single-hop profile. Active coverage
now includes both the legacy `LEN = 2` one-round verifier slice and the
production-width `LEN = 4` two-round slice used by current confidential-transfer
v2 hop fixtures. Its active tests cover builder acceptance, profile and
metadata-witness mismatch rejection, stale semantic non-zero inverse rejection,
zero public digest-group rejection, native Pallas preflight metadata binding,
LEN=4 selector/preflight opening-length splices, preflight digest/fingerprint
substitution rejection, zeroed preflight digest rejection, rejection when a
valid Pallas preflight is paired with an invalid verifier witness, and rejection
when a production Pallas preflight is paired with a non-production fixed-window
profile. The shared-table host-link guard also enforces the expected IPA round
count and per-round `b`-reduction and generator-fold layer widths, so a composed
witness cannot omit an initial `b` layer or generator-fold layer and still rely
on the final MSM relation alone. The native and shared-table verifier
`synthesize` paths now repeat those witness/config shape checks before
assignment, so malformed direct circuit witnesses fail closed instead of
silently skipping omitted `b`-reduction or generator-fold regions. The composed
circuit now also exposes a
public verifier transcript-binding digest instance and links it to the embedded
verifier's transcript-binding accumulator. It also exposes a public
scalar-projection digest over that transcript-binding digest, the public
`b`-reduction input scalars, challenge, inverse, and final folded `b` scalar,
and constrains the projection with the same field-friendly Pow5 compression.
The one-hop constructors also recompute that projection from the host verifier
witness and reject semantic public-input limb splices before circuit assignment.
The shared-table verifier host-link guard now also mirrors the final IPA MSM
product relation by checking `a_final * b_final == a_b_final`, so a composed
one-hop recursive witness rejects final-product splices before circuit
construction as well as inside the final-MSM product-link gate.
The production one-hop host API can also re-derive the native Pallas
preflight from the supplied witness and require it to match the preflight digest
bound in recursive metadata before materializing the recursive Vesta verifier,
preventing valid metadata from one witness from being paired with a different
self-consistent verifier witness. It can also re-derive the reserved
hop-proof-hash-bound preflight from the supplied witness plus the expected hop
proof hash, so one-hop mode-2 evidence cannot accidentally validate against the
detached native batch digest. The hop-bound path now has its own constructor,
separate from the detached native-batch constructor, so reserved evidence
callers cannot accidentally materialize a one-hop verifier slice through the
wrong preflight shape. The hop-bound witness guard first derives the native
preflight through the verifier slice's declared opening length, so a LEN=4
witness cannot satisfy a LEN=2 one-hop slice before the hop proof hash is mixed
into the reserved digest. LEN=4 hop-bound tests also require the detached native
batch digest to differ and reject wrong hop-proof hashes. The same constructor
path now rejects malformed semantic non-zero inverse witnesses and all-zero
preflight fingerprint, table-schedule, shared-table-manifest, table-base, or
verifier-batch digests before accepting a composed recursive verifier witness.
The record-backed one-hop recursive evidence builders now also run the returned
evidence through the same verifier-slice metadata guard before handing it to
recursive spend init/append. The real Pallas batch and proof-derived
open-envelope paths therefore fail closed if the evidence/preflight pair is
spliced across verifier-witness digest, opening length, or fixed-window
table-base digest fields.
Recursive spend append now also requires the previous recursive proof when
deriving the accumulator and streams a proof-artifact digest into
`recursive_proof_chain_digest`. Negative coverage rejects previous-proof
public-input splices and shows byte-level proof mutations produce a different
constant-size accumulator digest instead of being silently detached from the
append transcript.
Digest-splice, scalar-projection side-instance splice, and scalar-projection
semantic public-input splice MockProver cases are present
but heavyweight and ignored by default. Direct native/shared-table verifier
synthesis-shape MockProver cases for omitted `b`-reduction and generator-fold
layers are also present but ignored by default because they require the
large-stack non-native verifier harness. Full production fixed-window Pallas
verifier materialization and composed MockProver acceptance/splice tests are
heavyweight and ignored by default. This moves verifier composition into the
recursive aggregation circuit surface without accepting compact-token
aggregation mode `2`; full mode-2 admission still requires composing the
private-hop verifier batch and proving the complete Poseidon2 witness-batch
digest relation in-circuit.
The routine Rust test suite also skips real Kagemusha folded-token, recursive
aggregation, recursive-spend, and bridge success proof generators by default;
those cases remain available as opt-in `--ignored --test-threads=1` runs so
resource-constrained WSL hosts do not run multiple Halo2 IPA proof generators at
the same time. Default coverage keeps the deterministic semantic circuits,
metadata preflight, record checks, shape rejection, and hop-proof validation
active.
Native Pasta/Fp scalar decomposition, fixed-window scalar decomposition,
fixed-window Vesta point selection, table derivation, and scalar-multiplication
composition, shared-table fixed-window selection, shared-table native-scalar
windowed multiplication, shared-table multi-term MSM, shared-table final IPA MSM,
shared-table round-accumulator and generator-fold composition,
shared-table one-round IPA verifier composition,
shared-table multi-round IPA verifier builder, host-link composition, and
heavyweight composed-circuit MockProver coverage, one-hop recursive verifier-slice
composition,
fixed-window multi-term MSM,
native IPA scalar/vector-fold, full `b`-vector reduction, bounded MSM,
fixed-window final IPA MSM, IPA generator-fold, round-accumulator, and final IPA
comparison composition plus one-round and generic multi-round verifier
composition with transcript binding, native Pallas witness translation, batch
preflight binding, reserved-mode recursive aggregation evidence/proof-public
input binding, and a transparent Halo2 IPA semantic proof for that evidence are
present. The remaining recursive-circuit work is proving the private-hop Pallas
IPA verifier witnesses inside the recursive circuit and wiring that proof into
compact-token admission, so aggregation mode `2` remains a reserved wire value
with a stable rejection reason, and public prover/verifier entry points accept
only checked pre-fold mode `1`.
