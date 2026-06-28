# Offline Kagemusha

Kagemusha is the only active chain implementation for offline payments. Nodes
expose offline-offline payments through `settlement.offline.kagemusha_enabled`,
which defaults to `true`; there is no runtime legacy bearer-audit fallback.
Mobile artifact archives are served and gated by Core API, so Torii readiness
keeps the ABI-7 artifact booleans false while still advertising metadata.
Classic `IssueOfflineNote`, `AuditOfflineNote`, and `RedeemOfflineNote` payloads,
plus SDK/bridge defund compatibility composites, are retained only as historical
data-model compatibility fixtures and are not registered or dispatched by the
node's default instruction surface.

Classic Offline Note hardening notes from earlier drafts are archived with the
legacy data model. Torii issue/redeem endpoints reject classic payment
construction, the Norito bridge classic transaction builders fail closed, and
the core executor does not dispatch classic issue/audit/redeem instructions in
production. Swift, Kotlin/JVM, and Java Android classic payment submitters,
including defund submitters, also fail before signing or sending chain
transactions, and their default Torii issuer clients reject classic note issue
locally before posting to Torii.
Historical serialization and proof fixtures may still mention those types, but
production payment admission uses Kagemusha online-to-offline top-ups,
`KagemushaTransfer`, and `RedeemKagemushaRecursive`.

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
Gas metering follows the online boundary. Offline-offline Kagemusha transfers
do not burn chain gas, and `KagemushaTransfer` contributes zero confidential
gas when inspected by the chain meter. Online-to-offline top-ups and
offline-to-online redemptions remain chain transactions and are metered; the
recursive redemption path charges the final redeem proof, every top-up anchor
nullifier, recursive proof bytes/public inputs, and any chain-submitted lineage
witness material including hop proof attachments, previous recursive proofs,
and Pallas open-envelope archives.
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
remain prefix-specific and are replayed by core. One-hop verifier-slice
evidence binding now has adversarial coverage for proof-count,
verifier-witness profile, parameter fingerprint, fixed-window schedule,
shared-table manifest, opening-length, table-base, and hop-bound batch-digest
splices before receiver admission can trust that evidence. Semantic previous
proofs must
leave the lineage scalar-projection digest zero; Reserved-lineage previous
proofs must bind a non-zero scalar-projection digest and require the active
matching lineage verifier record before native hosts can serialize a redeem
instruction.
Core chain-side replay
preflights those previous proofs before
reconstructing hop evidence, so backend/profile substitutions, empty previous
proofs, stale public-input hashes, verifier-context splices,
scalar-projection splices, prefix-spliced folded public-input hashes, and
out-of-order hop counts fail before expensive Pallas replay. Chain execution also requires
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
collide on the anchor even when they end in different final notes. Recursive
redeem now uses the confidential unshield-v3 final proof shape exclusively:
whole-note redeem binds a zero private output, while partial redeem binds one
non-zero private change commitment in `change_output`. When change is present,
chain execution appends that commitment to the same deterministic shielded
accumulator/root-frontier path used by confidential unshield outputs and mints
only the requested public amount. Partial redeem without change, full redeem
with change, zero or already-existing change commitments, and
proof/change-output mismatches reject before state mutation. Append hops
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
carried top-up anchor nullifier. Append transition profiles carry the previous
accumulator's top-up anchor nullifiers explicitly, so detached append evidence
cannot omit, reorder, zero, or reuse those anchors in the next hop's outputs or
current spendable note material before append-boundary derivation.
Transition profile validation also re-derives the resulting accumulator digest
and append-boundary-free recursive public-input hash from the profile's own
transition fields, so a self-consistent profile cannot refresh its outer digest
around forged non-zero result hashes.
The accumulator's streaming nullifier, output, and fold-transcript digests use
recursive-spend domain tags, separate from the folded-token list/transcript
digest tags, so a one-hop recursive spend state cannot be replayed as a plain
checked folded transcript digest with the same field values. Accumulator
validation also requires the recursive aggregation transcript digest to equal
the lineage digest, so the proof public input cannot be detached from the
spend-lineage accumulator it is supposed to compress.
SDK surfaces must not expose recursive accumulator material as caller-provided
inputs. Package declarations and source guards treat proof-chain,
accumulator-state, accumulator-snapshot, recursive-snapshot, lineage-snapshot,
generic proof-state (`proofState`, `ProofState`, `proof_state`),
recursive/lineage proof-state, aggregation-transcript, fixed-window
table-schedule/shared-manifest/table-base, verifier-witness batch,
transition-profile binding, append-opening preflight, recursive
verifier scalar-projection, and previous/resulting accumulator aliases as
native-owned accumulator bytes.
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
Production init requests and Reserved-lineage append-output requests must carry
packaged lineage key artifacts. Release tooling can distribute the portable
Norito `KagemushaRecursiveSpendLineageKeyArtifactsV1` package, which binds a
profile-specific lineage circuit id, supported Pallas verifier opening length,
`lineage_verifier_key`, and `lineage_proving_key_archive`. Init builders accept
only the one-hop package and append builders accept only the append package;
unknown profiles, unsupported opening lengths, wrong verifier backends, empty
verifier keys, empty proving archives, and attempts to attach lineage artifacts
to semantic append output fail during request construction. The bridge validates
the proving-key archive against that verifier key before proving. Runtime
verifier-slice key generation is disabled by default and is available only
behind the explicit developer environment override used to generate artifacts,
so SDKs should treat missing artifacts as a deterministic request-construction
error rather than a fallback to runtime keygen. Release key-artifact generation
derives the one-hop and append verifier/proving keys through key-generation-only
verifier-slice shapes whose verifier-key commitments are regression-checked
against the full recursive verifier circuits, reducing keygen witness memory
without changing the published circuit identities. Swift, Kotlin/JVM, Java
Android, JavaScript/Node, Python, and C# expose typed lineage key artifact
helpers for the same package shape; those helpers defensively copy key bytes and
reject unknown profile ids, unsupported opening lengths, non-`halo2/ipa`
verifier backends,
empty verifier keys, and empty proving-key archives before wallet code can
construct a native request archive. The helpers also require the proving-key
archive payload to contain both the selected circuit id bytes and the stable
verifier-key commitment for the supplied verifier key, so a self-consistent
archive cannot be paired with a different verifier package. Kotlin/JVM also
converts Java-callable null
lineage artifact inputs into the same stable field errors instead of relying on
Kotlin intrinsic null checks. Swift, Kotlin/JVM, Java Android, JavaScript/Node,
Python, and C# now also parse the
canonical `KagemushaRecursiveSpendLineageKeyArtifactsV1` proving-key archive
fields and reject byte-smuggled circuit ids or verifier-key commitments, stale
schema hashes, unsupported archive flags, wrong archive versions, empty proving
keys, trailing payloads, non-canonical compact Norito length encodings,
canonical compact lengths above each SDK's addressable archive bounds, compact
length encodings whose terminal byte would overflow the u64 length space, and
invalid UTF-8 circuit family fields before native bridge loading, so overlong,
over-cap, or overflowing varints cannot smuggle otherwise valid lineage metadata
through the SDK guard.

Release tooling can produce the portable Norito packages with:

```bash
iroha app zk kagemusha lineage-key-artifacts \
  --profile init \
  --opening-len 128 \
  --out artifacts/kagemusha/lineage-init-len128.norito \
  --record-out artifacts/kagemusha/lineage-init-len128.record.norito \
  --vk-out artifacts/kagemusha/lineage-init-len128.vk \
  --pk-out artifacts/kagemusha/lineage-init-len128.pk

iroha app zk kagemusha lineage-key-artifacts \
  --profile append \
  --opening-len 128 \
  --out artifacts/kagemusha/lineage-append-len128.norito \
  --record-out artifacts/kagemusha/lineage-append-len128.record.norito \
  --vk-out artifacts/kagemusha/lineage-append-len128.vk \
  --pk-out artifacts/kagemusha/lineage-append-len128.pk

iroha app zk kagemusha recursive-compact-key-artifacts \
  --vk-out artifacts/kagemusha/recursive-compact-len4.vk \
  --pk-out artifacts/kagemusha/recursive-compact-len4.pk \
  --key-artifacts-out artifacts/kagemusha/recursive-compact-key-artifacts.norito \
  --verifier-keys-out artifacts/kagemusha/recursive-compact-verifier-keys.norito \
  --record-out artifacts/kagemusha/recursive-compact-len4.record.norito \
  --record-namespace offline_kagemusha \
  --record-version 1
```

For lineage commands, the primary `.norito` package is the SDK input.
`--record-out` writes the governance/WSV `VerifyingKeyRecord` bound to `offline_kagemusha`;
use `--record-namespace` and `--record-version` when an
operator needs a different namespace or governance version. The separate `.vk`
and `.pk` files are optional operator artifacts for inspection, checksums, or
escrow. The `recursive-compact-key-artifacts` command writes the ABI-7 one-hop
LEN=4 compact verifier key and packaged compact proving-key archive used by
`kagemusha-recursive-compact-v1`, plus the LEN=4 recursive compact
`KagemushaRecursiveCompactKeyArtifactsV1` and
`KagemushaRecursiveCompactVerifierKeysV1` Norito packages consumed by
multi-hop ABI-7 SDK/bridge calls; its optional record is the governance input.
Both package output paths are mandatory for the recursive compact release
command, so old `.vk`/`.pk`-only invocations and one-sided package-output
invocations fail before expensive key generation starts.
Generating these artifacts is intentionally expensive; do it during release
preparation, not on payment devices or inside request handling.
The production readiness rollup requires
`artifacts/kagemusha/lineage-proof-evidence.json`,
`artifacts/kagemusha/recursive-compact-key-evidence.json`, and
`artifacts/kagemusha/kagemusha-localnet-lifecycle-evidence.json` to sit beside these
Reserved-lineage `.norito`, `.record.norito`, `.vk`, and `.pk` files, the
ABI-7 recursive compact `.record.norito`, `.vk`, `.pk`, key-artifacts package,
and verifier-keys package files, plus the captured `record-archive-proof.log`
from the ignored production proof run and the audited 4-peer localnet lifecycle
run. The
canonical `lineage-proof-evidence.json` and
`recursive-compact-key-evidence.json` filenames and the canonical
`kagemusha-localnet-lifecycle-evidence.json` filename are part of the release
packet contract; renamed, copied, symlinked, or symlink-ancestor evidence JSON files
are rejected. The localnet lifecycle evidence JSON is read under its own
explicit parser size cap, separate from the Reserved-lineage proof evidence
cap; the helper applies that localnet cap family to the acceptance report
before it can publish final evidence, and validates generated evidence size
before creating temporary validation files. The lineage and compact evidence JSON declares SHA-256
digests and byte sizes for Reserved-lineage and ABI-7 compact key artifacts.
The localnet lifecycle evidence binds a production 4-peer run id, chain id,
peer ids, smoke/replay/restart/state-recovery hashes, and the eight
shield-to-redeem lifecycle hashes. The run id, chain id, and each peer id must
carry explicit `production`/`prod` and `localnet` markers and must not carry
non-production environment markers such as `dev`, `testnet`, `sample`, `demo`,
`sandbox`, `fixture`, `mock`, `qa`, `uat`, `preprod`, `preproduction`,
`preview`, or `zero`, including joined labels such as `localnetqa`,
`localnetpreprod`, `localnetpreview`, `localnetstage`, `localnettest`,
`localnetuat`, and `localnetzero`, and must not mix localnet identity with
`mainnet` labels such as `mainnet-localnet`, `mainnetlocalnet`, or
`localnetmainnet`. Marker-boundary matching allows legitimate production
identifiers such as `localnet-qatar`, but still rejects suffixed
non-production labels such as `localnet-testlane`, `localnettestlane`, or
`localnetpreprod1`; the four peer ids must also be sorted into a canonical
roster order. The evidence helpers compute
each artifact digest and size from the same opened regular file, and the rollup
recomputes each digest and artifact size from the same opened adjacent regular
file after path-identity revalidation, requires those
artifacts and the proof log to be regular non-symlink, non-hardlinked files
with readable leaf metadata, requires lineage and compact key artifacts to be
non-empty, rejects all-zero Reserved-lineage artifacts and obvious plain-text or
all-zero placeholder compact key artifacts before digest-only acceptance, and
the compact-key helper rejects secret-looking, control-character, missing,
symlinked, hardlinked, non-regular, or unreadable `--generator-log` paths
and rejects symlinked `--artifact-dir` values and symlinked generator-log
ancestor directories before resolving the generator-log parent or reading any
release artifacts, and
classifies artifact/log missing-vs-unreadable state from the lstat-backed
local-file validators rather than `Path.is_file()`,
and treats the checked-in ABI-6 manifest plus ABI-7 fail-closed and
Reserved-lineage release-tooling marker files as local trust roots that must
also be ordinary non-symlink, non-hardlinked files with symlink-free ancestors
before their contents can satisfy readiness, with ABI-6 manifest JSON and
marker text decoded from the same opened regular files after path-identity
revalidation and the ABI-6 manifest JSON capped at 1 MiB before parsing,
while ABI-7 and Reserved-lineage marker text reads are capped at 8 MiB so
large checked-in Rust bridge files remain bounded without becoming false
readiness blockers,
then hashes and parses the local proof log from the same opened regular file
and re-checks that it
contains the passing cargo result for the production Reserved-lineage test
as the exact single expected `test ... ok` line with exactly one one-test cargo
result, canonical LF line endings, strict UTF-8 bytes, and no trailing
whitespace or forged result-line suffix, with the log ending in a final LF
terminator, and rejects interrupted compile/start logs that only show the
production test beginning without the final `ok` line and cargo result,
rerunning the lineage local-file validator immediately before reading
proof-log text; local release JSON, checked-in source-marker, and
lineage artifact readers reject padded path components before ancestor
validation or metadata reads,
and that the recorded command is the production
`cargo test -p iroha_core ... --lib -- --ignored --test-threads=1 --nocapture`
run exactly as the canonical command string, with runtime lineage keygen unset
and no quoted-token aliases, newlines, or appended shell commands, before it can
report ready. The compact key evidence separately requires the exact
`iroha app zk kagemusha recursive-compact-key-artifacts --vk-out artifacts/kagemusha/recursive-compact-len4.vk --pk-out artifacts/kagemusha/recursive-compact-len4.pk --key-artifacts-out artifacts/kagemusha/recursive-compact-key-artifacts.norito --verifier-keys-out artifacts/kagemusha/recursive-compact-verifier-keys.norito --record-out artifacts/kagemusha/recursive-compact-len4.record.norito --record-namespace offline_kagemusha --record-version 1`
command string, LEN=4, IPA `k = 8`, `halo2/ipa`, circuit id
`kagemusha-recursive-compact-v1`, record namespace `offline_kagemusha`, record
version `1`, adjacent non-empty digest- and size-checked compact key files, and
the captured `recursive-compact-key-artifacts.log` stdout line from the
canonical key-generation command. The rollup hashes and parses that generator
log from the same opened regular file bound to the validation-time `lstat()`
identity, requires it to contain exactly the canonical CLI summary line with canonical
LF line endings, strict UTF-8 bytes, and a final LF terminator, and checks the
reported `.vk`, `.pk`, key-artifacts package, verifier-keys package, and
`.record.norito` byte sizes and SHA-256 digests against the adjacent artifact
bytes.
All-zero Reserved-lineage artifacts and plain-text or all-zero placeholder
compact key artifacts are rejected from the same prefix captured while hashing
the artifact, so non-production fixtures cannot be hidden by replacing the path
between digest and content checks even when their SHA-256 digest and byte size
match the evidence JSON.
The compact key evidence helper also hashes, sizes, decodes, and parses the
generator log from one opened regular file, so helper-generated evidence cannot
combine a digest from one log with parsed artifact claims from a later path
replacement.
Marker-stuffed proof logs with extra passing tests are rejected
even when their digest matches the evidence JSON. The evidence JSON and its nested `circuit_ids`,
`artifacts`, and `tests` objects are closed schemas, so extra release claims are
rejected instead of ignored, with control-character or secret-looking unexpected
field names redacted in blocker details; duplicate JSON object keys are also invalid, so
non-standard `NaN`/`Infinity` JSON constants are rejected before schema checks
and redacted in evidence/readiness blockers, and
auditors never have to interpret last-key-wins evidence packets. Proof-evidence
JSON is parsed from the same opened regular file after path-identity
revalidation and capped at 16 MiB before parsing, so post-preflight swaps or
oversized metadata cannot replace or exhaust the evidence parser before schema
checks. Unreadable or
non-UTF-8 ABI-6 manifest and proof-evidence JSON files fail closed as structured
read blockers instead of tracebacks. Reserved-lineage proof evidence, ABI-7
compact key evidence, and localnet lifecycle evidence with omitted, non-string,
noncanonical, control-character-bearing, stale, or future-dated
`generated_at_utc` values remain blocked even when every artifact digest is
otherwise valid. Their timestamps must use canonical UTC
`YYYY-MM-DDTHH:MM:SSZ` form, and recorded proof
commands with surrounding whitespace, control characters, or secret-looking
material such as `token=` are rejected without echoing unsafe command bytes.
The helper rejects noncanonical `--generated-at-utc`
values such as `+00:00` offsets or surrounding whitespace instead of
normalizing them into the evidence JSON. Direct helper calls also reject
omitted, boolean, numeric, list, or empty `generated_at_utc` inputs before
artifact, proof-log, generator-log, or localnet acceptance metadata is read.
Malformed direct `elapsed_seconds` values and non-integer future-skew limits
fail the same scalar preflight before local metadata is read. Direct
artifact-dir, proof-log, generator-log, acceptance-report, and output-preflight
helper calls reject
control-character, surrounding-whitespace, or secret-looking artifact,
proof-log, generator-log, acceptance-report, and output paths before resolving
corridors, creating output parents, creating temporary evidence files, or
writing evidence JSON. The shared local lineage file validator
also rejects control-character, secret-looking, parent-segment, or
backslash-bearing evidence, artifact, or proof-log paths and symlinked
local-file ancestors before JSON parsing, digest calculation, or proof-log
reads; both the readiness rollup's direct SHA-256 reader and the lineage
helper's direct SHA-256 reader repeat that file-shape validation before
returning artifact digests, and the readiness, lineage-helper, and compact-key
helper readers bind each digest/text read to the first validated `lstat()`
identity so post-preflight regular-file replacements fail closed. Ready summaries publish only
sanitized SHA-256 maps for the accepted Reserved-lineage artifacts and proof
log, not the local artifact directory path, so release reviewers can compare
evidence packets without capturing workstation paths. Android freshness checks consume the
scanner-validated signed-evidence timestamp already present in each accepted slot
report instead of re-opening slot metadata or signed-evidence JSON during the
rollup, but still revalidate that it is canonical UTC before comparing
freshness windows. Their stale/future blockers operate on sanitized Android
slot reports, so secret-looking or control-character slot identifiers are
redacted before freshness diagnostics are serialized. Direct signed-evidence
summary validation also rejects malformed path, digest, and identity fields
without echoing secret-looking or control-character values. Localnet lifecycle evidence also has direct generated-at
presence, shape, and freshness coverage, so omitted, non-string, noncanonical,
control-character-bearing, stale, or future-dated 4-peer lifecycle attestations
cannot satisfy readiness even when their artifact hashes are otherwise valid.
Unexpected top-level or nested localnet acceptance fields are rejected and
redacted through the same secret/control-character display policy as lineage
and compact evidence. Localnet lifecycle summary identity values, including
`localnet_run_id`, `chain_id`, `target`, and `peer_ids`, are also passed through
that display policy so rejected secret-looking or control-character values are
not emitted in readiness summaries.
The rollup rejects symlinked `--repo-root` directories, symlinked
repo-root ancestors, unreadable repo-root metadata, and direct control-character
or secret-looking repo-root validator inputs before resolving checked-in
ABI/source trust roots. Surrounding-whitespace, parent-segment, and
backslash-bearing `--repo-root` aliases, including path components with
surrounding whitespace, are rejected before repo-root metadata reads, resolver
normalization, or trust-root section reads.
Surrounding-whitespace, parent-segment, and backslash-bearing
`--device-lab-root`, `--lineage-proof-evidence`, `--compact-key-evidence`, and
`--localnet-lifecycle-evidence` values, including path components with
surrounding whitespace, are rejected before Android root
classification, readiness rollup construction, or evidence JSON reads. Trusted
signer public-key paths reject the same aliases before key loading, OpenSSL
lookup, Android slot metadata reads, or summary rendering. The summary writer
also rejects surrounding-whitespace, parent-segment, and backslash-bearing
`--summary-out` aliases before readiness rollup construction, trusted signer
key loading, or output creation.
The ABI-6 manifest, ABI-7 marker, and Reserved-lineage release-tooling section
checks repeat that repo-root preflight before reading their checked-in
trust-root files, and the lower-level release JSON/source marker file validators
reject control-character, secret-looking, parent-segment, or backslash-bearing
direct file paths plus unreadable ABI-6 release JSON/source-marker leaf metadata
before content parsing.
ABI-7 and Reserved-lineage
source marker text reads also rerun the source-marker file validator immediately
before loading marker text and bind the opened read to that preflight `lstat()`
identity, so symlink, hardlink, non-regular, secret-bearing, or post-preflight
regular-file source aliases cannot satisfy readiness markers after an earlier
check. Source-marker text reads are capped at 8 MiB using opened-file metadata
and streamed byte counts. Unreadable source-marker leaf metadata, unreadable
marker bytes, or non-UTF-8 ABI-7 and Reserved-lineage marker files return
structured blockers instead of raw decode errors. The
ABI-7 compact section also extracts the relevant Rust function bodies before
trusting the checked-in source: compact record preflight must reject malformed
multi-hop Pallas archives before proof composition, one-hop and append Pallas
archives must bind to the production LEN=4 verifier-slice keys and either
consume packaged proving-key material or fail at the proving-key gate, and the C
bridge must still map true compact key-material unavailability to
`KagemushaRecursiveCompactUnavailable` instead of a generic proof error. The
readiness summary writer also rejects symlinked `--summary-out` ancestors and
symlinked, hardlinked, non-regular, dangling-symlink, or unreadable-metadata
output leaves, plus control-character or secret-looking direct output paths,
before creating missing output parent directories, then rechecks the output parent and ancestors before
writing and binds post-write readback to the opened summary file identity,
keeping local rollup artifacts from being emitted through secret or aliased
paths. The
Android signing helper runs the slot/artifact symlink, hardlink, and
regular-file preflight before parsing `slot.json`, and classifies the slot
directory plus its parent with `lstat()` so unreadable slot or parent metadata
fails closed before metadata-derived output paths, signatures, or manifest
refreshes can start from an aliased slot bundle. The shared Android device-lab
signing path also validates the preserved `attestation/harness-result.json`
against the slot challenge, copied certificate-chain count, exact StrongBox
level labels, and canonical lowercase challenge hex before producing or binding
signed evidence. The slot assembler and scanner apply the same exact-string
harness policy, so whitespace-normalized harness aliases or level labels cannot
become signed production evidence. Signed slot metadata must also keep
`keymint_security_level` as an exact accepted StrongBox label instead of relying
on case normalization. The verifier `attestation/report.json` must also carry
`keymint_security_level`, `attestation_security_level`, and
`keymaster_security_level`, and each must be an exact StrongBox label before a
slot can be signed or accepted by the scanner. Scanner validation and
signed-slot assembly also require verifier report app-package, status, and
level fields to match `attestation/result.json` exactly, and scanner validation binds
`attestation/result.json` `keymint_security_level` back to `slot.json` exactly,
so app-package substitutions, non-`ok` status aliases,
or StrongBox spellings cannot mask a source-artifact splice. The signed-slot
assembler also rejects unexpected attestation result, report, verifier, D2D
transcript, or wallet-integrity transcript fields, report schema/verifier drift,
plus D2D and wallet transcript schema-id drift, before publishing source
artifacts. It also runs the scanner D2D and wallet transcript semantic
validators on staged copies before publish, so queue splices, wallet state
non-rotation, and other scanner-only transcript failures cannot be staged into
unsigned production slots. Physical Android slots can now be captured through
`python3 scripts/kagemusha_android_device_lab_capture.py`, which serial-scopes
the Gradle install and instrumentation export, first requires `adb -s <serial>
get-state` to report exact `device`, pulls the raw slot, derives the attestation
challenge SHA-256 from `attestation/challenge.hex`, renders the verifier report,
reports bounded redacted ADB stdout/stderr when that serial-scoped preflight
fails or reports a non-`device` state, runs a bounded redacted `adb devices -l`
diagnostic after serial-scoped ADB failures that reports only attached-device
row/state counts or the stable `no_visible_devices` token, redacts the
configured ADB serial from command and stdout/stderr diagnostics, caps long
non-`device` state strings, caps failed
preflight command displays, optionally checks `--expected-device-family` by
reading `ro.product.model` and `ro.product.device` through the same
non-disruptive ADB runner before build/install/instrumentation, accepting a
single ADB transport `\r\n` line ending while still rejecting embedded control
characters, can optionally wait with `--adb-visibility-wait-seconds` by polling
only the same serial-scoped `adb get-state` and bounded `adb devices -l`
diagnostic without restarting or reconnecting ADB, accepts `--serial auto` only
when `adb devices -l` reports exactly one safe `device` row before switching back
to the serial-scoped preflight, assembles signed evidence with
`nearby_offline`, `nfc_hce`, and `qr` D2D transcript bindings, and validates
the signed slot. The capture wrapper, raw puller, and slot assembler accept
`--*-timeout-seconds 0` as an explicit no-timeout mode for operator-controlled
captures where subprocess interruption is forbidden, while negative timeout
values, negative ADB visibility wait values, and non-positive visibility poll
intervals still fail closed before ADB or Gradle is invoked.
The wrapper requires an explicit `--physical-device-attestation` assertion and
does not manage or stop
other processes. It also preflights the local `--private-key` and `--public-key`
signing inputs as existing, non-empty, non-symlinked, non-hardlinked regular
files before ADB visibility checks, builds, instrumentation, or raw-slot pulls.
The wrapper now rejects broad process/device-management
commands such as `kill`/`pkill`/`killall`, `adb kill-server`, `adb reconnect`,
`adb disconnect`, and `adb shell am force-stop` before invoking any capture
runner; its ADB visibility preflight, raw-summary, attestation-result, and
challenge reads are bounded and opened-file identity-bound before report
rendering. The standalone raw puller applies the same non-disruptive command
gate to its latest-slot query and raw-slot tar pull before invoking the ADB
runner, and the signed-slot assembler applies it before any bounded standalone
`adb getprop` identity query. It independently rejects forged raw attestation results
before report rendering unless the result matches the raw slot id, reports
exact `status = ok`, preserves the selected run-as app package, asserts
physical StrongBox/KeyMint attestation, and binds `attestation_challenge_sha256` to the
pulled challenge bytes plus
`attestation_certificate_chain_sha256` to the certificate-chain text that was
already accepted by the guarded raw-slot reader, without reopening the path for
a second digest read. Raw text artifacts such as `latest-slot.txt`,
`attestation/challenge.hex`, the certificate chain, status NDJSON, and runtime
logs are read through an opened-file identity check so path swaps fail with
`changed while being read`.
Its optional capture summary writer creates
missing output parents one path component at a time through directory file
descriptors with no-follow flags, then creates the temporary JSON file, atomic
replacement, exact byte readback, rollback cleanup, and final parent-directory
sync through the captured parent descriptor. If the public summary path is
swapped before final sync, the writer fails closed and removes the file it
installed through that descriptor instead of populating the swapped-in target.
Rollback cleanup only unlinks a regular file whose current identity still
matches the installed summary, preserves swapped replacements, and reports
unlink or cleanup-sync failures with the sync/write error. Failed temporary
writes use the same descriptor-relative identity check, so a swapped temp
pathname is preserved and an unremovable temp file is reported with the write
failure.
The strict matrix scanner remains the authority for deciding when every
standard device family has production evidence.
Required telemetry, status NDJSON, queue,
attestation, and runtime-log artifact shape checks now also run on staged
assembler output before publish, so failed status records, missing runtime
completion markers, malformed telemetry, noncanonical telemetry identity
strings, unexpected telemetry, status-event, or pending queue fields, non-`ok`
status events, non-empty post-handoff pending transactions, or malformed
pending queue JSON cannot be installed as unsigned production slots. The raw
Android puller applies the same telemetry field allowlist, status-event field
and value allowlists, telemetry identity exactness, pending queue field
allowlist, telemetry app-package binding, and queue empty-after-handoff check
before raw artifacts can be promoted into a signed slot. When the slot assembler reads attached device identity
through ADB `getprop`, each response must be exactly one LF-terminated value
and the value itself must not require trimming before it can be bound into
signed slot metadata. Non-zero ADB `getprop` exits are normalized to property
and exit-code diagnostics instead of copying the raw subprocess command list,
and ADB launch failures report only that the property query could not be
executed, so configured ADB serials cannot leak through Python exception text. The
on-device lab exporter also fails closed if
`Build.MODEL` and `Build.DEVICE` do not both identify the same standard
Kagemusha matrix family through exact Pixel model/codename matches or the
standard Samsung S23/S24 model-prefix and codename pairs, preventing unsupported
near matches from producing Pixel-labeled or otherwise covered raw slots before
host-side assembly. The host assembler applies the same binding to explicit
`--device-family` values, so an operator-supplied family must match the family
inferred from model/codename evidence before a signed slot can be published;
one-sided or conflicting standard model/codename evidence also fails closed
instead of trusting whichever field matched first.
The assembler writes the exact model and codename into `slot.json` and signed
evidence, and the scanner recomputes the standard matrix family from those
signed fields, rejects one-sided or conflicting standard model/codename pairs,
and requires telemetry to repeat the same identity values.
The shared Android device-lab
JSON loader rejects duplicate keys and non-standard `NaN`/`Infinity` constants
before slot metadata, attestation, signed evidence, D2D handoff, or
wallet-integrity schema validation, redacts the literal non-finite token from
diagnostics, and caps those JSON inputs at 16 MiB from the opened file metadata
and streamed byte count. The raw-puller and scanner status NDJSON parsers plus
the Android capture wrapper apply the same non-finite redaction before telemetry
or raw helper output can feed signed-slot assembly.
The shared control-character
predicate also treats Unicode format controls such as bidirectional overrides as
unsafe control material, so deceptive path or evidence labels are rejected or
redacted through the same gates as ASCII escape bytes. Scanner and signing-helper direct calls also route top-level slot
artifact enumeration through the same guarded list helper, so a post-validation
slot directory listing failure becomes `slot directory could not be listed`
instead of a traceback or a partial manifest rewrite. Runtime private-key and signer public-key paths are validated for
secret-looking material before OpenSSL lookup, and private-key leaf metadata is
checked before classifying missing, symlinked, non-regular, hardlinked, or
unreadable key files. Trusted public-key leaf metadata follows the same
missing, symlink, non-regular, hardlink, and unreadable-metadata shape checks.
Missing local tooling cannot mask unsafe operator path inputs, and signature
verification preserves key-path validation failures
separately from private/public key mismatches. Temporary
OpenSSL staging writes use exclusive temporary files, flush and fsync staged
payload/signature bytes, read them back with opened-file identity binding before
invoking OpenSSL, reject staged files that gain hardlink aliases before
readback, and report staging write, readback mismatch, signature output
read failures, or non-64-byte
Ed25519 signature outputs as structured signer/verifier errors instead of
tracebacks; signature output bytes are also read through opened-file identity
binding and hardlink rejection, bounded to one byte beyond the 64-byte Ed25519
shape, before the shape check. Lower-level direct symlink, hardlink, and regular-file artifact
validators reject secret-looking slot paths before traversing, stat-ing, or
classifying slot artifacts. The symlink validator now reports unreadable
slot-metadata, artifact-directory, and nested-artifact metadata before alias
classification, and the regular-file validator classifies leaves with `lstat()`
before any `exists()` preflight can mask unreadable metadata. Hardlink and
regular-file validators also classify artifact directories with `lstat()`
before any `exists()` preflight, and the regular-file validator classifies
nested artifacts before any `is_symlink()` preflight. Required-artifact shape
checks, required status/runtime text reads, the D2D queue digest binding, and
the signed-evidence artifact binding also classify artifacts with `lstat()`
before any `is_file()` preflight. Signed-evidence `artifact_digests` and the
`slot.json`-referenced
metadata/text reads bind those artifacts to the opened regular file before
hashing or decoding bytes and cap those reads at 16 MiB. Direct signer metadata-loader and SHA-256 manifest rewrite helper calls
also reject secret-looking slot paths and unreadable slot or parent metadata
before metadata parsing, artifact traversal, hashing, or manifest writes. The lower-level signer artifact-digest
builder reruns that slot preflight before hashing required signed-evidence
artifacts, so direct calls cannot hash through secret-bearing or aliased slot
paths, and the per-artifact digest helper rechecks each relative artifact path
for secret-looking names, unreadable leaf metadata, symlinks, hardlinks, and
non-regular files immediately before digest reads used by signed evidence and
manifest rewrites, then binds each digest read to the opened regular-file
identity and caps each signer-side slot-artifact digest read at 16 MiB.
Low-level signer output writers also
reject secret-looking signed-evidence and manifest paths before creating output
parents or writing files, convert absolute signed-evidence output path resolver
failures into `signed evidence output path could not be resolved`, reject
symlinked absolute signed-evidence output ancestors and symlinked absolute
output leaves before path resolution can normalize them into the canonical slot
output, reject signer private/public key paths with surrounding whitespace
before slot metadata reads or OpenSSL lookup, reject explicit `--output` strings
with surrounding whitespace or whitespace-padded path components, backslashes,
or parent segments before slot metadata is read, reject
backslash-bearing output paths and absolute parent-segment aliases before
resolver normalization, reject unreadable output leaf metadata before write or digest reads, reject dangling
symlink output leaves as symlinks even when the target is missing,
rerun parent and ancestor checks after creating missing output parents, write
`signed-evidence.json` and `sha256sum.txt` through fsynced same-directory
temporary files, atomically replace the final outputs, identity-check temporary
cleanup after failed writes, read them back before success through opened-file
identity binding, sync the captured output-parent identity after replacement,
and preserve existing outputs if replacement fails.
The
signer JSON outputs for `signed-evidence.json` and `slot.json` also reject
serialized JSON above 16 MiB before creating temporary files and enforce the
same cap during opened-file readback.
signing helper revalidates the signed-evidence output as a regular non-symlink,
non-hardlinked file before hashing it back into `slot.json`, then binds that
digest read to the opened file identity and caps the readback at 16 MiB using
opened-file metadata and streamed byte counts. `sha256sum.txt` manifest rewrites
also reject text above the 1 MiB manifest cap before temporary-file creation and
during opened-file readback. Direct SHA-256 manifest parser and verifier helper
calls reject secret-looking slot paths, unreadable slot-root metadata,
symlinked slot roots, and symlinked slot ancestors before parsing
`sha256sum.txt` or traversing slot artifacts. Direct
parser and verifier calls also reject unreadable-metadata and hardlinked
`sha256sum.txt` manifests before reading manifest bytes or discovering slot
files, and the parser binds `sha256sum.txt` bytes to the opened file identity
so post-preflight regular-file swaps fail closed; manifest bytes are capped at
1 MiB using both opened-file metadata and streamed byte counts before UTF-8
decoding. Nonblank manifest lines must not rely on leading or trailing
whitespace normalization or leading `*` path normalization before digest/path
parsing. Direct slot-file discovery reports unreadable slot-root and
artifact-directory metadata through caller error lists, returns no artifacts for
secret-looking slot paths, symlinked slot ancestors, missing roots,
non-directory roots, or symlinked slot roots before traversal, and skips
symlinked artifact directories instead of discovering files through them. Direct
manifest verification rejects entries under symlinked artifact directories
before reading or hashing bytes, and the manifest artifact digest helper
revalidates each `sha256sum.txt` entry for secret-looking names, symlinks,
hardlinks, and non-regular files immediately before hashing, then binds the
digest read to the opened file identity and caps it at 16 MiB. Direct
attestation, D2D handoff, wallet-integrity,
required-artifact, signed-evidence, and production-metadata validator helper
calls reject the same slot paths before parsing artifacts, reading transcript
bindings, or hashing signed evidence. Signed-evidence artifact digest
verification also revalidates required artifact paths for secret-looking names,
symlinks, hardlinks, and non-regular files immediately before hashing the bytes
claimed by `artifact_digests`, including the `slot.json` release APK,
attestation certificate-chain, D2D handoff, and wallet-integrity transcript
paths, and binds each digest to the opened file identity. Telemetry,
transcript, report, log, and certificate-chain artifacts keep the 16 MiB cap;
the offline wallet APK path must be a canonical child under `evidence/` and is
capped separately at 64 MiB so arm64 JNI proof bridge builds fit without
relaxing the smaller evidence artifacts.
After rebuilding Android native bridge artifacts, run a clean lab-app package
build before capture; stale APK package outputs can retain discarded signing or
padding bytes and exceed the 64 MiB cap even when the current central-directory
entries are under the limit. Device evidence must be captured from the same
clean APK whose SHA-256 is copied into the signed slot.
Post-preflight regular-file swaps still fail closed. The Android device-lab root validator
also rejects secret-looking, control-character, parent-segment, and
backslash-bearing root paths before root metadata reads or slot discovery, and
scan_slot(...) rejects unreadable slot directory or parent metadata before slot
traversal. Scanner and rollup missing-root decisions also consume the same
`lstat()`-classified root presence instead of calling `Path.exists()`.
The direct device-lab summary writer rejects
secret-looking, control-character, parent-segment, and backslash-bearing output
paths before parent metadata reads, plus unreadable output parent or leaf
metadata, classifies summary output parents with `lstat()` before any
`Path.is_dir()` preflight, rechecks created output parents before writing JSON, writes
`--json-out` through a fsynced same-directory temporary file, atomically replaces
the final summary, reads it back through opened-file identity binding before
success, caps the serialized summary before temporary-file creation and the
opened-file readback at 16 MiB, preserves an existing summary if replacement
fails, and removes only the just-installed summary identity if final
parent-sync fails. The signed-slot assembler uses the same captured-parent
rollback rule for `slot.json` and copied evidence artifacts after post-install
parent-sync failures. The signed-evidence helper also
classifies signer-controlled output parents with `lstat()` before write or
read-back digest preflight, so unreadable parent metadata stays a structured
signer blocker instead of being hidden by `Path.is_dir()`.
Scanner summary construction also normalizes finite float values in direct
report inputs as unsupported summary values and redacts non-finite numbers, so
release-facing summary JSON cannot preserve injected floating-point scalars that
the scanner itself would not emit. When Kagemusha production evidence or the
standard matrix is requested, scanner summary coverage and
`duplicate_bindings` rollups only admit reports with safe slot ids, canonical
family/model/codename identity, ABI 7, canonical signed-evidence timestamps,
non-zero artifact digests, a signer digest present in the supplied trusted
signer pins, and safe release artifact path/digest pairs.
Scanner slot inventory also classifies expected top-level directories,
`sha256sum.txt`, and recursive file-count entries with `lstat()`, so summary
presence and file-count fields do not follow symlinks or hide unreadable
metadata behind `Path.is_dir()` or `Path.is_file()`.
Automatic slot discovery also classifies each device-lab root entry with
`lstat()`, preserves symlinked slot entries for fail-closed `scan_slot(...)`
rejection, and reports unreadable slot-entry metadata without falling back to
`Path.is_dir()`.
The readiness CLI converts
`--repo-root` resolver failures into `--repo-root could not be resolved` before
expanding relative lab or lineage evidence paths, and shared Android ancestor
validation reports cwd metadata failures as structured path blockers for
relative helper inputs. Shared Android ancestor validation also classifies each
ancestor with `lstat()`, so ancestor metadata errors and symlink rejections do
not rely on `Path.is_symlink()` or `Path.exists()` preflights. Manifest artifact digest validation
also classifies slot-relative ancestor directories with
`lstat()` before symlink checks, so nested artifact paths do not depend on
`Path.is_symlink()`. Slot-metadata digest checks also revalidate `slot.json`-referenced
attestation-chain, offline-wallet APK, and signed-evidence artifact paths before
reading bytes for SHA-256 comparison, then bind the bytes to the opened file
identity using the same 16 MiB evidence cap and 64 MiB APK cap so
post-preflight regular-file swaps fail closed. The attestation report helper
also requires `--slot-id` to be an exact canonical single directory name and
rejects noncanonical certificate-chain path spellings such as
`attestation/./...`, repeated separators, or trailing slash forms before
writing `attestation/report.json`; backslash-bearing chain paths are rejected
through the same pre-report gate. Local certificate-chain source paths also
reject parent-segment and backslash aliases before ancestor validation or
metadata reads, and the harness-result source path uses the shared guarded JSON
loader so parent-segment, backslash, control-character, and secret-looking
paths fail before metadata reads or parsing. D2D handoff and
wallet-integrity transcript bindings, including `queue/pending_queue.json`, use
the same digest-time revalidation before comparing SHA-256 values. Signed-slot
assembly, raw Android pulls, and explicit scanner slot selection also require
`--slot-id`/`--slot` values to be exact canonical single directory names before
any slot path is joined or created; backslash-bearing slot IDs fail the same
safe-name gate. Filesystem-discovered slot directory names are held to the same
policy before metadata is read. Direct manifest parsing, slot-file inventory,
digest validation, and signing-helper slot path calls also reject
surrounding-whitespace, parent-segment, and backslash-bearing slot path aliases
before slot metadata reads, manifest rewrites, artifact hashing, or
signed-evidence metadata loads.
Slot-relative manifest and metadata paths must
also use exact canonical spellings; dot segments, repeated separators, and
trailing slash aliases fail before digest binding, and path components with
surrounding whitespace fail before artifact hashing. Required
status NDJSON and runtime log marker checks also revalidate their slot-relative
files for symlinks, hardlinks, symlinked artifact directories, non-regular
files, and secret-looking names immediately before text decoding, with the same
opened-file identity binding. Status NDJSON must use LF line endings with a
trailing newline, only exact `ok` status values are accepted, each status line
must carry the matching slot id, and nonblank status lines must not rely on
surrounding whitespace being stripped before JSON parsing. The shared Android device-lab JSON loader
also rejects secret-looking, control-character, parent-segment, and
backslash-bearing direct file paths and symlinked ancestor directories before parsing JSON, then
decodes JSON bytes from one opened regular file after preflight path-identity
revalidation, so direct metadata, attestation, handoff, wallet-integrity, or
signed-evidence validation cannot read through secret-bearing directories,
aliased directories, or post-preflight leaf aliases; unreadable leaf metadata,
unreadable bytes, or non-UTF-8 JSON bytes fail closed as structured read errors
instead of tracebacks. Manifest inventory discovery reports unreadable artifact metadata as
structured slot-artifact blockers instead of omitting those files from
`sha256sum.txt` coverage, and direct hardlink artifact validation reports the
same unreadable metadata before hardlink checks. Digest-time artifact validators
for `sha256sum.txt`, slot metadata bindings, nested attestation/D2D/wallet
transcript bindings, signed-evidence payload fields, and signed-evidence
artifact digests reject all-zero SHA-256 placeholders and distinguish missing
files from unreadable leaf metadata before classifying symlink, non-regular,
hardlink, or read failures.
The raw Android slot puller and slot assembler apply the same non-zero
SHA-256 rule to attestation source digests and assembled metadata artifact
bindings before a raw device result can be installed or promoted to signed
production slot evidence.

Capture the production ignored proof log into the same artifact directory and
build the evidence JSON from those local bytes. Enable `pipefail` before the
capturing pipelines so `tee` cannot mask a terminated prover or key-generation
process:

```bash
set -o pipefail
SECONDS=0
cargo test -p iroha_core \
  kagemusha_recursive_spend_lineage_init_append_from_record_archives_proves_reserved_lineage_output \
  --lib -- --ignored --test-threads=1 --nocapture \
  2>&1 | tee artifacts/kagemusha/record-archive-proof.log
elapsed_seconds=$SECONDS

python3 scripts/kagemusha_lineage_proof_evidence.py \
  --artifact-dir artifacts/kagemusha \
  --proof-log artifacts/kagemusha/record-archive-proof.log \
  --elapsed-seconds "$elapsed_seconds" \
  --max-generated-at-future-skew-seconds 300 \
  --out artifacts/kagemusha/lineage-proof-evidence.json

# If the production proof ran in a detached staging directory, finalize only
# after its wrapper has written a zero exit marker. The wrapper also captures
# the lineage key artifacts. With no path flags, the runner and finalizer both
# use the symlink-free resolution of /tmp, for example /private/tmp on macOS.
# The runner preserves the canonical `iroha ...` command in reports but prepends
# `<repo-root>/target/release` and `<repo-root>/target/debug` to the child PATH,
# so a built workspace does not require an operator shell PATH change.
# Explicit staged path flags must already be canonical paths: parent-segment,
# backslash-bearing, and surrounding-whitespace component aliases fail before
# metadata reads. The runner rejects secret-looking, control-character,
# surrounding-whitespace, backslash, and parent-segment --exit-file and
# --elapsed-seconds-file values before staged-directory metadata is read.
python3 scripts/kagemusha_run_lineage_proof_staged.py \
  --repo-root . \
  --staged-artifact-dir <staged>/artifacts/kagemusha \
  --exit-file <staged-exit-file> \
  --elapsed-seconds-file <staged-elapsed-seconds-file>

python3 scripts/kagemusha_finalize_lineage_proof_staged_run.py \
  --staged-artifact-dir <staged>/artifacts/kagemusha \
  --exit-file <staged-exit-file> \
  --elapsed-seconds-file <staged-elapsed-seconds-file> \
  --max-generated-at-future-skew-seconds 300 \
  --artifact-dir artifacts/kagemusha \
  --out artifacts/kagemusha/lineage-proof-evidence.json

iroha app zk kagemusha recursive-compact-key-artifacts \
  --vk-out artifacts/kagemusha/recursive-compact-len4.vk \
  --pk-out artifacts/kagemusha/recursive-compact-len4.pk \
  --key-artifacts-out artifacts/kagemusha/recursive-compact-key-artifacts.norito \
  --verifier-keys-out artifacts/kagemusha/recursive-compact-verifier-keys.norito \
  --record-out artifacts/kagemusha/recursive-compact-len4.record.norito \
  --record-namespace offline_kagemusha \
  --record-version 1 \
  2>&1 | tee artifacts/kagemusha/recursive-compact-key-artifacts.log

python3 scripts/kagemusha_recursive_compact_key_evidence.py \
  --artifact-dir artifacts/kagemusha \
  --generator-log artifacts/kagemusha/recursive-compact-key-artifacts.log \
  --generated-at-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --max-generated-at-future-skew-seconds 300 \
  --out artifacts/kagemusha/recursive-compact-key-evidence.json
# The compact evidence helper checks the canonical --out corridor and filename
# and preflights symlinked or otherwise aliased --out paths before reading
# artifact-directory metadata.

# If the compact keygen must run detached, run it through the staged wrapper
# first. It captures the generator log and writes the real process exit marker.
# With no path flags, the runner and finalizer both use the symlink-free
# resolution of /tmp, for example /private/tmp on macOS. Explicit staged path
# flags must already be canonical paths: parent-segment, backslash-bearing, and
# surrounding-whitespace component aliases fail before metadata reads. The
# finalizers also reject secret-looking, control-character,
# surrounding-whitespace, backslash, and parent-segment --exit-file values
# before staged-directory metadata is read. The finalizers apply the same
# path-shape gate before staged or publish directory metadata; the lineage
# finalizer also applies it to --elapsed-seconds-file.
# Before checking the exit marker or publishing, finalizers also reject staged
# artifact directories that still contain runner-owned `.staged-runner.tmp`
# files, because those files mean the detached wrapper did not complete.
python3 scripts/kagemusha_run_recursive_compact_keygen_staged.py \
  --repo-root . \
  --staged-artifact-dir <staged>/artifacts/kagemusha \
  --exit-file <staged-exit-file>

# Finalize only after the staged wrapper has written a zero exit marker.
python3 scripts/kagemusha_finalize_recursive_compact_key_staged_run.py \
  --staged-artifact-dir <staged>/artifacts/kagemusha \
  --exit-file <staged-exit-file> \
  --max-generated-at-future-skew-seconds 300 \
  --artifact-dir artifacts/kagemusha \
  --out artifacts/kagemusha/recursive-compact-key-evidence.json

# Generate the twelve source artifacts from the real 4-peer lifecycle run. The
# recorder is opt-in: with IROHA_KAGEMUSHA_LOCALNET_LIFECYCLE_SOURCE_DIR unset,
# the integration test keeps its normal behavior and writes no release evidence.
# With it set, the test fails if the pre-restart or post-restart replay
# transaction is accepted instead of rejected, and each emitted JSON document
# binds the run id, chain id, sorted peer ids, source slot, kind, transaction or
# rejection material, and observed non-empty block target. Recorder mode uses
# IROHA_KAGEMUSHA_LOCALNET_LIFECYCLE_CHAIN_ID as the actual network chain id,
# defaulting to kagemusha-production-localnet-v1, and labels actual peer
# identities as peer-N@production-localnet:<peer-id> for the acceptance report.
# The recorder rejects secret-looking or aliased source directories, rejects
# symlinked source roots or ancestors, creates the source directory as 0700 on
# Unix, and publishes each source JSON through a create-new 0600 temporary file
# before an atomic rename to the canonical artifact filename.
KAGEMUSHA_LOCALNET_SOURCE_DIR="$PWD/artifacts/kagemusha/localnet-lifecycle-sources"
IROHA_KAGEMUSHA_LOCALNET_LIFECYCLE_SOURCE_DIR="$KAGEMUSHA_LOCALNET_SOURCE_DIR" \
IROHA_KAGEMUSHA_LOCALNET_LIFECYCLE_RUN_ID=<production-4-peer-localnet-run-id> \
IROHA_KAGEMUSHA_LOCALNET_LIFECYCLE_CHAIN_ID=kagemusha-production-localnet-v1 \
cargo test -p integration_tests --test consensus_and_da \
  confidential_dual_restart_stress_mid_flow_localnet -- --nocapture

# Publish the 4-peer production localnet lifecycle acceptance report. The
# acceptance report must be named kagemusha-localnet-lifecycle-acceptance.json,
# live directly under --artifact-dir, and contain the localnet_acceptance fields:
# run id, chain id, four peer ids, smoke/replay/restart/state-recovery results,
# and the full shield-to-redeem lifecycle artifact hashes. The acceptance helper
# validates every source JSON document against the CLI run id, chain id, sorted
# peer ids, expected artifact slot, expected kind, transaction/replay/event
# fields, generated timestamp, and non-empty block target before hashing source
# bytes. Run, chain, and peer identifiers must be explicitly
# production/prod and localnet labeled, free of dev/testnet/sample/demo/mock/zero
# markers including joined localnetqa/localnetpreprod/localnettest/localnetuat
# labels, and free of contradictory mainnet/localnet labels such as
# mainnetlocalnet or localnetmainnet; the four peer ids must be sorted. The
# lifecycle hashes may use `sha256:`, `urn:sha256:`, or `hash://sha256/`
# prefixes, are normalized to the lowercase 64-hex digest in readiness
# summaries, and reject uppercase, single-character placeholder, duplicate, or
# suffix-bearing variants.
# The acceptance helper validates the run id, chain id, and exact four distinct
# sorted peer ids before hashing any source artifact, so malformed identities
# cannot force local evidence reads or leak through diagnostics. It also raw-
# preflights every source artifact path string before source metadata reads, and
# validates missing, aliased, or reused source files before hashing any source
# bytes. Source JSON documents are closed-schema and their context,
# transaction, replay, and event strings are checked for control characters and
# secret-looking material before any source artifact digest is computed.
# Transaction and replay source hashes must also be non-zero lowercase
# SHA-256-shaped hex strings, matching the recorder's emitted transaction ids.
# The helper rejects secret-looking, control-character, backslash, and parent-segment
# --acceptance-report paths before artifact-directory metadata is read. The
# helper also verifies the canonical --out path is directly under
# --artifact-dir before creating a missing output parent, then preflights
# symlinked ancestors before reading localnet acceptance input metadata. It
# wraps those bytes with the release schema and timestamp, validates the result
# through the production readiness gate, fsyncs validation scratch cleanup, and
# publishes the canonical evidence JSON under the localnet lifecycle size cap.
python3 scripts/kagemusha_localnet_lifecycle_acceptance.py \
  --artifact-dir artifacts/kagemusha \
  --out artifacts/kagemusha/kagemusha-localnet-lifecycle-acceptance.json \
  --run-id <production-4-peer-localnet-run-id> \
  --chain-id kagemusha-production-localnet-v1 \
  --peer-id <peer-0@production-localnet:actual-peer-id> \
  --peer-id <peer-1@production-localnet:actual-peer-id> \
  --peer-id <peer-2@production-localnet:actual-peer-id> \
  --peer-id <peer-3@production-localnet:actual-peer-id> \
  --smoke-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/smoke-artifact.json" \
  --replay-rejection-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/replay-rejection-artifact.json" \
  --restart-replay-rejection-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/restart-replay-rejection-artifact.json" \
  --state-recovery-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/state-recovery-artifact.json" \
  --lifecycle-shield-tx-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-shield-tx-artifact.json" \
  --lifecycle-hop-proof-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-hop-proof-artifact.json" \
  --lifecycle-recursive-init-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-recursive-init-artifact.json" \
  --lifecycle-recursive-init-verify-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-recursive-init-verify-artifact.json" \
  --lifecycle-recursive-append-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-recursive-append-artifact.json" \
  --lifecycle-recursive-append-verify-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-recursive-append-verify-artifact.json" \
  --lifecycle-unshield-proof-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-unshield-proof-artifact.json" \
  --lifecycle-redeem-tx-artifact "$KAGEMUSHA_LOCALNET_SOURCE_DIR/lifecycle-redeem-tx-artifact.json"

python3 scripts/kagemusha_localnet_lifecycle_evidence.py \
  --artifact-dir artifacts/kagemusha \
  --acceptance-report artifacts/kagemusha/kagemusha-localnet-lifecycle-acceptance.json \
  --generated-at-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --max-generated-at-future-skew-seconds 300 \
  --out artifacts/kagemusha/kagemusha-localnet-lifecycle-evidence.json
```

The staged Reserved-lineage finalizer performs the same artifact, proof-log,
command, timestamp, and digest checks as the direct helper, but additionally
rejects noncanonical or future-dated `--generated-at-utc` during preflight
before creating the temporary publish stage. It requires the detached wrapper's
zero exit marker and refuses to overwrite any published lineage artifact or
`lineage-proof-evidence.json` unless `--replace` is explicit. After installing
each staged file into the published artifact
directory, it reopens the published file through the identity-bound artifact
reader and byte-compares it against the staged source so post-install drift
fails before the final evidence check, then revalidates the public
`--artifact-dir` path and fsyncs the captured published artifact-directory
descriptor so directory swaps before final fsync fail closed while successful
syncs cover the directory that received the files.
It creates or tightens the published artifact directory, finalizer temporary
parent, and inner stage directory to `0700`, and it chmods each copied staged
file to `0600` before fsyncing and promoting it. Its temporary staging cleanup
also revalidates the captured temp-parent identity before removing anything.
Rollback cleanup after copy, verification, or publish errors also unlinks only
paths whose current file identity still matches the identity captured
immediately after install, so a swapped published artifact is reported as a
cleanup failure instead of being removed.
The staged runner first runs the canonical init and append
`iroha app zk kagemusha lineage-key-artifacts` commands from the staged root so
the relative `artifacts/kagemusha/...` outputs match the release contract, then
preserves the real keygen or cargo exit code in `<staged-exit-file>` instead of
normalizing failures to success. It keeps the command string canonical for
evidence and run-report validation while absolutizing relative `--repo-root`
values before prepending the validated repo root's `target/release` and
`target/debug` directories to the child PATH, so detached staging can use
locally built binaries without weakening command exactness. Operators can also
pass `--iroha-bin <path/to/iroha>` to place a validated regular,
non-symlink executable named `iroha` under non-symlink ancestors first on the
child PATH, which prevents a stale `target/release/iroha` from silently
shadowing a current release or debug binary while keeping the recorded command
canonical. Secret-looking, control-character, surrounding-whitespace,
backslash, and parent-segment path material is rejected before metadata reads or
child PATH construction.
Each init, append, and proof phase also writes
a closed-schema execution report beside its log, recording the canonical
command, phase, exit code, elapsed seconds, log byte count, and
execution-report SHA-256 of the child log. The keygen and
proof children write combined stdout/stderr directly to their temporary staged
log files, and those logs are flushed and fsynced after each child exits so a
supervisor interruption does not make child output depend on a Python-owned
pipe. Long-running proof and compact-key children also emit periodic fsynced
heartbeat lines into those same staged logs while waiting, so an operator can
distinguish a healthy silent prover from a dead or abandoned temporary output.
The runner also creates or tightens its staged root, `artifacts`
directory, and `artifacts/kagemusha` directory to `0700`, and writes staged
child logs, execution reports, run reports, elapsed-time files, and exit
markers as `0600` files before accepting them. Each temporary child log is
installed only after syncing the captured
output-parent identity, so parent-directory swaps before log fsync fail closed.
Staged execution-report and run-report command fields are exact non-empty
strings; values with surrounding whitespace, control characters, or
secret-looking material are rejected without echoing unsafe bytes.
The runner writes `record-archive-proof.log` only after the canonical
production command returns, writes
`lineage-proof-staged-run.json` with the canonical command, exit code, elapsed
seconds, proof-log filename, proof-log byte count, and init/append
lineage-key-artifact log byte counts, and refuses to overwrite previous staged
key artifacts, staged keygen logs, proof logs, execution reports, run reports,
elapsed-time files, or exit markers unless `--replace` is explicit. The
`--replace` path also removes stale runner-owned `.staged-runner.tmp` files for
child logs, execution reports, the run report, elapsed-time sidecar, and exit
marker through the same identity-checked cleanup path before launching the next
child process. The
finalizer refuses any staged artifact directory that still contains a
runner-owned `.staged-runner.tmp` file before reading the exit marker,
validating run reports, or publishing evidence, so an interrupted wrapper
cannot be mistaken for a completed production run. The
explicit `--resume-key-artifacts` mode is narrower than `--replace`: it reuses
an init or append key-artifact phase only when all four profile artifacts, the
phase log, and a zero-exit execution report validate with the canonical command
and matching log byte count. Missing, failed, or otherwise invalid regular
phase outputs, including signal-style `exit_code = -9` attempts, are replaced
and rerun, while symlinked, hardlinked, or special staged outputs still fail
closed before anything is removed. Resume also
replaces stale proof logs, proof execution reports, run reports, elapsed files,
and exit markers so a previous nonzero proof or keygen marker cannot block a
validated phase-boundary retry. `--resume-key-artifacts` and `--replace` are
mutually exclusive, so operators must choose either selective validated resume
or full staged-output replacement before any cleanup can occur. Resume,
`--replace`, and temporary child-log cleanup now remove stale staged files only
through a parent directory file descriptor when the file identity captured after
validation or creation still matches at unlink time, so a swapped path is
reported as cleanup drift instead of being removed, and parent-directory sync
failures after identity-matched cleanup are reported as cleanup-sync failures.
The
runner reopens each installed metadata file after the atomic rename, checks the
opened file identity, syncs the captured output-parent identity, and compares
the exact bytes so marker, elapsed, and JSON report drift fails before
finalization. If final parent sync fails after installing a staged child log,
marker, report, or elapsed-time file, the runner rolls back only the
just-installed file identity through the captured parent descriptor and reports
rollback unlink or cleanup-sync failures with the sync failure. The
finalizer applies that
runner-report binding whenever the exit marker claims success, requires the
success marker to be exactly `0\n`, requires the staged elapsed-seconds file
to be the runner's exact positive decimal line with six fractional digits and
one trailing newline, rejects exit-code, elapsed-second, proof-log-size, or
lineage-key-artifact log-size drift between the marker, elapsed file, report,
and staged logs, requires each init, append, and proof execution report to
carry a zero exit code plus a non-zero SHA-256 digest that matches the staged
child log bytes, and still
refuses to publish any artifacts from nonzero staged exits. Marker failures are
reported with control-character and secret-looking marker values redacted
before success-only elapsed, command, timestamp, or run-report checks
so a partial stage cannot obscure the root failure. Staged run-report duplicate
or unexpected JSON field diagnostics also redact control-character and
secret-looking keys, including nested `lineage_key_artifact_logs` profile and
entry-field names. When the staged subprocess
terminates with a signal-style negative status, the runner returns a
conventional nonzero wrapper status while preserving the exact subprocess code
in the staged marker and report. Spawn-failure coverage exercises the real
`subprocess.Popen(...)` boundary for the Reserved-lineage key-artifact runner,
Reserved-lineage proof runner, and ABI-7 compact-key runner, so partially
written temporary child logs are removed and the caller receives the same
conventional staged-runner error even when process launch fails before a child
can report an exit status. Those launch failures report only the fixed
`process launch failed` reason instead of echoing raw process-spawn exception
text.
The ABI-7 compact-key staged runner applies the same detached-run contract for
the key-generation command: it runs the canonical
`iroha app zk kagemusha recursive-compact-key-artifacts` command from the
staged root, absolutizes relative `--repo-root` values before building the
child PATH, finds repo-local `target/release` or `target/debug` binaries, and
accepts the same validated `--iroha-bin <path/to/iroha>` override when a
specific current binary must take precedence over a stale repo-local release
build; wrong-name, missing, non-regular, non-executable, symlinked, and
symlinked-ancestor overrides plus secret-looking, control-character,
surrounding-whitespace, backslash, and parent-segment path material are rejected
before launch. It gives
the child process direct ownership of the temporary
`recursive-compact-key-artifacts.log` stdout/stderr target, flushes and fsyncs
that log after child exit, installs it only after syncing the captured
output-parent identity, rolls back the just-installed log identity if final
parent sync fails, reports rollback unlink or cleanup-sync failures with the
sync failure, creates or tightens its staged root, `artifacts`
directory, and `artifacts/kagemusha` directory to `0700`, writes staged
generator logs, execution reports, run reports, and exit markers as `0600`
files before accepting them, preserves the real keygen exit code in
`<staged-exit-file>`, writes a closed-schema
`recursive-compact-key-execution.json` with the canonical command, phase, exit
code, elapsed seconds, generator-log byte count, and execution-report SHA-256
of the generator log, writes
`recursive-compact-key-staged-run.json` with the canonical command, exit code,
elapsed seconds, generator-log filename, and generator-log byte count, and
refuses to overwrite staged key artifacts, generator logs, execution reports,
run reports, or exit markers unless `--replace` is explicit. The compact-key
runner's explicit `--resume-keygen` mode reuses only a complete staged keygen
whose artifacts, generator log, zero-exit execution report, zero-exit run
report, and exact `0\n` exit marker all validate against exact non-empty
command strings with no surrounding whitespace, control characters, or
secret-looking material, the canonical command value, and current generator-log
byte count plus SHA-256 digest. If regular staged outputs are missing,
nonzero, padded, or malformed, including signal-style `exit_code = -9` attempts,
resume replaces the whole compact keygen stage and reruns it; symlinked,
hardlinked, special, or secret-looking staged outputs
still fail closed before cleanup. `--resume-keygen` and `--replace` are
mutually exclusive, so a caller cannot accidentally request both a validated
resume and destructive staged-output replacement. The compact-key
`--replace` path also identity-cleans stale runner-owned `.staged-runner.tmp`
files for the generator log, execution report, run report, and exit marker
before launching a replacement child process. The compact-key
finalizer likewise rejects any staged artifact directory that still contains a
runner-owned `.staged-runner.tmp` file before marker, report, or publish
checks, ensuring an interrupted keygen wrapper remains visibly incomplete. The
compact-key
runner also reopens marker and JSON report outputs after the atomic rename,
checks the opened file identity, syncs the captured output-parent identity, and
compares the exact bytes before returning.
The compact-key finalizer requires that
run report whenever the exit marker claims success, requires the success marker
to be exactly `0\n`, rejects exit-code or
generator-log-size drift between the marker, report, and staged log, requires
the staged execution report to carry a zero exit code plus a non-zero SHA-256
digest that matches the staged generator log bytes, rejects elapsed-time drift
between the run report and execution report, and still
refuses to publish any artifacts from nonzero staged exits. It also reopens each
published key artifact, generator log, and evidence JSON after the final
install and compares the identity-bound readback against the staged bytes before
success is reported, then syncs the captured published artifact-directory
identity so directory swaps before final fsync fail closed. Its temporary
staging directories are forced to `0700`, and each staged key artifact,
generator log, and evidence JSON copy is forced to `0600` before promotion.
Cleanup also revalidates the captured temp-parent identity before removing
anything. Marker failures are
reported with control-character and secret-looking marker values redacted
before success-only command, timestamp, or run-report checks. Staged run-report
duplicate or unexpected JSON field diagnostics also redact control-character
and secret-looking keys. The
compact runner uses the same wrapper-exit convention: its process status is
conventional, but the exact keygen status remains bound in the marker and run
report.

The production-readiness CLI rejects secret-looking, control-character, or
surrounding-whitespace
`--lineage-proof-evidence`, `--compact-key-evidence`, and
`--localnet-lifecycle-evidence` paths before running the rollup, so unsafe
evidence path strings and padded path components are not echoed in summaries or
diagnostics.
Operators may repeat `--device-lab-root` when Android matrix evidence is
captured into separate per-device or per-family roots. The rollup validates each
root with the same symlink/path-shape preflight, scans every valid root, keeps
the summary root redacted as `<local-device-lab-root>`, and still rejects
duplicate slot ids or copied physical-device/D2D bindings before readiness can
pass. `scripts/kagemusha_release_bundle.py` accepts the same repeated
`--device-lab-root` form when packaging the ready summary, and binds each
Android evidence artifact to the supplied root that contains its slot.
The ABI-7 compact-key evidence validator applies the same redaction to
noncanonical `generator_log_path` claims before serializing blocker details, so
forged secret-looking or control-character log-path values are never echoed
back in readiness output.
Reserved-lineage per-test `log_path` mismatches are also rejected before local
proof-log validation, and forged secret-looking or control-character claimed log
paths are not echoed in blocker details.

After `scripts/kagemusha_production_readiness.py` writes a ready
`dist/kagemusha-production-readiness.json`, package release evidence with the
strict bundle verifier:

```bash
python3 scripts/kagemusha_release_bundle.py \
  --repo-root . \
  --bundle-root . \
  --readiness-summary dist/kagemusha-production-readiness.json \
  --lineage-proof-evidence artifacts/kagemusha/lineage-proof-evidence.json \
  --compact-key-evidence artifacts/kagemusha/recursive-compact-key-evidence.json \
  --localnet-lifecycle-evidence artifacts/kagemusha/kagemusha-localnet-lifecycle-evidence.json \
  --device-lab-root artifacts/android/device_lab \
  --trusted-signer-public-key <lab-public-key.pem> \
  --out dist/kagemusha-production-release-bundle.json
```

To verify an already-published manifest without rewriting `--out`, run the same
command with `--verify-existing dist/kagemusha-production-release-bundle.json`.
Verification recomputes the manifest from local release evidence and performs a
stable manifest comparison that ignores only `generated_at_utc`. The existing
manifest path is checked under `--bundle-root` before readiness, proof-evidence,
compact-key, or device-lab scanners run. The existing manifest's nested
evidence inventory is closed before comparison: required evidence groups and
`path`/`sha256`/`size_bytes` entry fields must be present, unexpected groups or
entry fields are rejected, lineage/compact artifact and proof-log groups must
match their required key sets, Android evidence slot groups must match
`android_device_lab.signed_evidence`, and Android slot artifact maps must name
exactly the release-critical artifact kinds. Every `path` entry must be a
canonical safe relative string with a non-zero lowercase SHA-256 digest that is
not the empty-file digest and a positive integer `size_bytes`, so traversal,
absolute, non-string, whitespace-normalized, all-zero digest, empty-file
digest, missing-size, boolean-size, zero-size, or non-integer-size evidence
entries fail as manifest-shape blockers.
Nested release evidence entries for Reserved-lineage artifacts,
Reserved-lineage proof logs, and ABI-7 compact artifacts are also rebound to
freshly recomputed bundle-relative paths, SHA-256 digests, and byte sizes before
generic manifest drift is considered, so a saved manifest cannot forge a
positive proof-log size while keeping the digest and path unchanged.
The readiness summary JSON and any `--verify-existing` release manifest are
capped at 16 MiB from the opened file metadata and streamed byte count before
JSON decoding, so oversized local metadata fails as file-shape evidence instead
of reaching parser or comparison code.

The checked-in ABI-6 manifest is parsed only after its local-file preflight
returns a regular, non-hardlinked file identity; the opened JSON bytes must
match that preflight identity and remain path-bound after the read, so a
post-preflight manifest replacement fails closed before schema checks.
The checked-in ABI-7 recursive-spend fixture manifest and archive fixture use
the same local-file preflight, strict duplicate-key and non-finite JSON
rejection, and 1 MiB JSON cap before readiness. Readiness requires both files to
be JSON objects, and then requires the manifest schema,
bridge ABI, generator provenance object, lineage accumulator domain object,
operation inventory array with named operation objects, archive fixture
reference object path/schema, and every archive entry's unique name,
named-object shape, decoded byte length, and SHA-256 digest to match the
checked-in bundle. The staged Reserved-lineage and ABI-7 compact runners and
finalizers apply the same non-finite constant redaction to their execution and
run-report JSON loaders before resume or publish validation can report a
corrupted staged report.
committed fixture pair. Both files are closed-schema JSON: unexpected manifest,
manifest-generator, manifest-domain, manifest-operation, archive-reference,
archive-fixture, or per-archive fields block readiness before the release bundle
can copy the ABI-7 fixture digests.
The non-C# SDK fixture tests mirror that contract by asserting the exact
manifest, archive-reference, generator, domain, operation, archive-fixture, and
per-archive key sets, checking the exact archive row count, then recomputing
each decoded archive's byte length and SHA-256 digest before decoding the
shared ABI-7 request archives.

The release bundle manifest uses
`iroha.kagemusha.production_release_bundle.v1`, recomputes the checked-in ABI-6,
ABI-7, and lineage release-tooling trust roots plus the readiness summary,
Reserved-lineage proof evidence, ABI-7 compact key evidence, and Android
signed-evidence inventory, and parses readiness summaries and existing release
manifests from opened regular JSON files whose identities match their preflight
`lstat()` checks. The emitted manifest records bundle-relative
per-slot Android signed-evidence artifact paths and SHA-256 digests for every
validated device-lab slot, keeps the Reserved-lineage and ABI-7 compact
artifact digest and size maps plus ABI-7 fixture manifest/archive SHA-256
bindings from the recomputed readiness summary, records every packaged lineage artifact,
compact key artifact, compact key generator log, production proof log, release
APK, D2D handoff transcript, any extra per-transport D2D transcript declared in
the signed slot `d2d_payment_transcripts` map, wallet-integrity transcript, and
attestation certificate-chain file with
bundle-relative path, SHA-256 digest, and byte size computed from bytes whose
opened file identity matches the preflight `lstat()` identity and remains
path-bound after the read, and revalidates each slot
name, signed-evidence summary field set, signed-evidence timestamp, signed
device family/model/codename binding, summary digest, and slot-relative artifact
path before
constructing manifest paths. The verifier rejects summary drift,
release-manifest drift, duplicate JSON keys,
unexpected top-level, section-level, evidence-inventory, evidence-entry,
Android manifest, or per-slot Android signed-evidence summary fields, missing
release section/evidence fields, malformed release section states, timestamps,
ABI constants, projected ABI-6 limits/modes, ABI-7 fixture manifest/archive
paths, schemas, ABI versions, operation counts, circuit ids, digest maps, size
maps, section map inventories, checked-file inventories, Android family
coverage, readiness-summary Android matrix lists, Android trusted-signer pins,
readiness-summary trusted-signer digest lists, any non-empty readiness-summary
or saved release-manifest Android `duplicate_bindings` inventory,
duplicate-binding slot lists, duplicate-binding value inventories, and malformed summary
digests, malformed, noncanonical, or future-dated summary/manifest timestamps,
including nested readiness-summary and release-manifest lineage/compact
evidence-section timestamps plus Android readiness timestamp bounds, and drift
between readiness-summary Android minimum signed-evidence timestamp bounds and
freshly scanned device-lab evidence, plus maximum signed-evidence bounds that
exclude any signed Android evidence timestamp, non-standard
`NaN`/`Infinity` JSON constants in summaries or manifests, malformed or
inventory-mismatched Android readiness slot lists, malformed Android slot
Kagemusha detail fields, unexpected nested slot/Kagemusha detail fields,
slot detail/signature-summary binding drift, slot device-family inventory drift,
accepted Android slot errors or malformed present/file-count summaries,
accepted Android slot metadata drift from freshly scanned device-lab evidence,
signed Android evidence whose signer digest is absent from the trusted signer
digest list,
missing readiness-summary top-level fields, non-object readiness-summary
sections, missing readiness-summary section fields, per-section blockers in a
ready summary, missing release-manifest top-level fields, `ready=false` release
manifests, non-empty release-manifest blocker lists,
unsafe Android signed-evidence or slot-artifact map slot ids in saved release
manifests,
unexpected fixed-inventory release evidence item keys,
non-string, unsafe, noncanonical, or reused nested evidence inventory paths,
malformed, empty-file, or reused nested evidence digests, or
missing/boolean/non-integer/non-positive nested evidence sizes in existing
release manifests, empty-file or duplicate readiness-summary and
release-manifest section-level digest-map values,
control-character paths, secret-looking paths, evidence outside
`--bundle-root`, secret-looking strings anywhere inside the readiness summary
or release manifest, control-character strings anywhere inside either JSON root,
all-zero lineage artifacts in the lineage inventory, plain-text or all-zero
placeholder compact key artifacts in the compact-key artifact inventory,
missing or digest-drifted Android release APK, D2D handoff,
wallet-integrity, and attestation-chain artifacts, symlinked bundle roots or
bundle-root ancestors, noncanonical parent-segment bundle-root aliases,
backslash-bearing bundle-root aliases, surrounding-whitespace component
bundle-root aliases, and
symlinked or hardlinked manifest outputs, and
records only bundle-relative evidence paths. If any release input path escapes
`--bundle-root`, or `--bundle-root` itself is a symlink or has a symlink
ancestor, or `--bundle-root` contains a parent-segment, backslash, or
surrounding-whitespace component alias, the
verifier stops before loading any readiness JSON, existing release manifest,
proof evidence, compact-key evidence, Android device-lab tree, artifact
inventory, or bundle-root metadata.
The release-bundle CLI also applies those alias-shape checks to non-signer
paths before loading trusted signer public keys, so a malformed local path
cannot force signer material to be read first.
Secret-looking trusted signer key paths and trusted signer key paths containing
control characters, surrounding whitespace, parent-segment aliases, or
backslashes are rejected before key loading. Device-lab slot validation returns
immediately when no trusted
signer map is supplied, so a missing trust configuration is not misreported as
an untrusted signed-evidence signer digest. The production-readiness Android
rollup also stops before slot discovery when the trusted signer map is missing,
while still reporting the missing standard-family and D2D transport matrices.
Direct trusted-signer public-key loading treats a single string or `Path` as one
public-key input, not an iterable of characters or path components, and rejects
non-path entries with sanitized errors before OpenSSL lookup.
Control-character `--out` values are rejected before output parent creation.
Newly-created release-bundle output parents are revalidated before writing so a
symlinked parent cannot be introduced during output creation. The manifest is
rejected above 16 MiB before any temporary output is created, written through a
fsynced temporary file in the target directory, promoted with descriptor-relative
replacement through the captured output-parent descriptor, revalidates the
public parent identity after install, syncs that captured descriptor, and is
read back through
opened-file identity binding with the same 16 MiB cap before success is
reported, and `--out` cannot overwrite any readiness summary, evidence JSON,
proof log, key artifact, or Android signed-evidence file already hash-bound into the manifest.

The helper rejects a symlinked or unreadable-metadata `--artifact-dir` and
refuses to write `lineage-proof-evidence.json` through symlinked, hardlinked,
non-regular, dangling-symlink, unreadable-metadata, or symlink-ancestor output aliases
and rejects all-zero Reserved-lineage artifacts before emitting evidence JSON.
It also rejects surrounding-whitespace components, parent-segment aliases, and
backslash aliases in `--artifact-dir`, `--proof-log`, `--generator-log`, and
`--out` before resolving paths or reading filesystem metadata.
The direct lineage and compact-key evidence helpers reject `generated_at_utc`
values more than 300 seconds ahead of the helper clock by default, matching the
production readiness rollup's future-skew allowance; use
`--max-generated-at-future-skew-seconds` only to make that local helper bound
stricter for a controlled release run.
Its evidence writer publishes through descriptor-relative replacement, revalidates
the public output-parent identity, and syncs the captured output-parent
descriptor before readback, so parent directory swaps after atomic replacement
fail closed; the compact key evidence helper applies the same output checks for
`recursive-compact-key-evidence.json` before reading compact key artifacts. It rejects
obvious plain-text or all-zero placeholder compact key artifacts before emitting
evidence JSON and requires `recursive-compact-key-artifacts.log` beside the
key artifacts and verifies that the canonical generator summary sizes match the
local `.vk`, `.pk`, key-artifacts package, verifier-keys package, and
`.record.norito` files. The staged compact-key finalizer adds a zero-exit-marker
gate plus a runner-report binding for successful staged exits, refuses
destination overwrites unless `--replace` is explicit, reopens each published
file after install to compare it with the staged source bytes, and runs the same
generator-log and evidence checks before reporting staged artifacts as
published.
The staged Reserved-lineage and compact-key runners also identity-bind child-log
parent syncs before accepting installed proof, key-artifact, or generator logs.
The readiness summary writer, Android device-lab summary writer, Android
signed-evidence helper, both evidence helpers, and the release-bundle writer
serialize with strict JSON; non-finite values such as `NaN` and
`Infinity` fail before any temporary release output is created.
The lineage and compact-key evidence helpers apply the same strict JSON
serialization before creating validation scratch files under `--artifact-dir`
and report validation scratch-file cleanup failures even when the scratch write
itself fails. Validation scratch cleanup is identity-checked through the
scratch file's parent directory, so a swapped validation file is reported as
cleanup drift instead of being removed.
The lineage evidence helper validates unsafe `--proof-log` strings before
artifact-directory metadata checks, so hostile proof-log input cannot trigger
artifact-dir traversal or metadata reads.
The readiness summary writer enforces a 16 MiB `--summary-out` cap before
temporary-file creation, during final opened-file readback, and reports
identity-checked temporary-file cleanup failures after write or post-stage
output-validation errors. It also forces the fsynced summary temporary file to
`0600` and rejects final opened-file readback unless the promoted
`--summary-out` file still has private `0600` permissions. The release-bundle writer applies the same
identity-bound cleanup before accepting or reporting `--out` artifacts. The lineage and compact-key
evidence helpers also enforce the readiness evidence JSON byte caps
before creating `--out` temporary files and again while reading back the opened
output file after atomic replacement, so oversized same-inode output growth
cannot be accepted as a verified write, and they report identity-checked temporary-file cleanup
failures after output write or post-stage output-validation errors.
The Android raw puller's host `latest-slot.txt` and raw-pull summary writers
now create their temporary files, promote replacements, and verify readback
through the captured output-parent descriptor. They also report
identity-checked temporary-file cleanup failures after failed writes, refuse to
unlink a temp output whose file identity changed before cleanup, and remove the
installed metadata file from the original parent if the public parent path is
swapped before final directory sync. Published-output rollback is also
identity-bound, so a swapped replacement is preserved and unlink failures are
reported explicitly.
The Android device-lab scanner `--json-out` writer applies the same durability
rule to validation summaries: it promotes the summary through the captured
output-parent descriptor, syncs that descriptor, and removes only the
just-installed summary identity when final parent sync fails.
The release-bundle writer applies the same descriptor-relative publish and
captured-parent sync pattern to its manifest output with a 16 MiB cap before
temporary-file creation and during final opened-file readback, and reports
temporary-file cleanup failures after write or post-stage
output-validation errors as structured blockers. It also forces the fsynced
manifest temporary file to `0600` and rejects final opened-file readback unless
the promoted `--out` file still has private `0600` permissions. Its bundle-relative path
calculator also rejects secret-looking or control-character evidence and
bundle-root strings before resolving paths for the release inventory, rejects
parent-segment, backslash-bearing, and surrounding-whitespace component evidence
paths before they can normalize into manifest entries, rejects parent-segment,
backslash-bearing, and surrounding-whitespace component `--bundle-root` aliases
before bundle-root metadata reads or shared bundle-relative path resolution,
rejects the same aliases on `--out` before manifest writes, rejects them on
`--verify-existing` before manifest loading,
and release evidence entries run the bundle-root containment check before
hashing evidence bytes.
The release-bundle `--out` writer and production-readiness `--summary-out`
writer keep their output-parent descriptors open across atomic replacement and
roll back the installed JSON on final parent-sync failure only when the current
file identity still matches the just-written output.
The compact-key evidence helper also rejects secret-looking or
control-character `--artifact-dir` strings inside the generator-log validator
before any resolve, and resolves only the generator log's parent before the
local file-shape check so a symlinked log is not followed during corridor
validation. Direct compact-key evidence builder calls reject explicitly unsafe
`generator_log_path` strings before artifact-directory metadata checks.
The Android device-lab summary writer, Android signed-evidence helper, and
signed-slot assembler JSON metadata writer also report identity-checked
temporary-file cleanup failures after write or post-stage output-validation
errors instead of swallowing failed cleanup.
The signed-slot assembler, attestation-report writer, and signed-evidence
helper also force the published device-lab root, report output parent, staging
directories, slot subdirectories, copied artifacts, `attestation/report.json`,
`slot.json`, `sha256sum.txt`, and `evidence/signed-evidence.json` to private
host permissions (`0700` for directories and `0600` for files), then verify
those modes after write/publish so signed production evidence does not depend
on the operator shell's umask. The attestation-report writer also publishes
`attestation/report.json` through descriptor-relative replacement, revalidates
the public parent identity, syncs the captured output-parent descriptor, and
rolls back the report on final parent-sync failure only when the current file
identity still matches the report just written. The signed-evidence
helper applies the same descriptor-relative replacement, public-parent
revalidation, captured-fd sync, and descriptor-bound rollback rule to installed
JSON and manifest outputs after final parent-sync failures. The signed-slot assembler
now applies that captured-parent rollback rule to `slot.json` and copied
evidence artifacts as well, using descriptor-relative replacement/creation and
preserving swapped replacements whose identity no longer matches the
just-installed file.
Direct Android device-lab scanner path preflights reject control-character
roots, slot paths, JSON artifact paths, trusted signer public keys, and
`--json-out` destinations; reject surrounding-whitespace roots, slot paths,
trusted signer public keys, and `--json-out` destinations; and reject
parent-segment or backslash-bearing root, slot, trusted signer public-key, and
`--json-out` aliases
before metadata reads, key loading, JSON parsing, slot discovery, or output
parent creation. Explicit scanner slot ids are
validated and deduplicated before root classification, and the direct discovery
helper repeats that validation before joining ids to the root. Direct
trusted-signer maps are also screened for unsafe public-key path strings before
slot metadata reads. The signed-evidence helper applies the same fail-closed
alias checks to runtime private-key and signer public-key paths before slot
metadata reads, key metadata reads, or OpenSSL lookup.
Direct scanner summary-output validation also rejects padded output path
components before parent metadata reads or parent creation, matching the
`--json-out` CLI preflight.
The Android capture wrapper applies the same surrounding-whitespace component
preflight to `--repo-root`, `--kotlin-dir`, `--raw-root`, `--slot-root`,
summary-output paths, and the offline-wallet APK path before ADB preflight,
Gradle, instrumentation, raw pulls, signer loading, or summary writes.
The raw Android slot puller rejects padded components in `--out-root` and
`--summary-out` before ADB queries, tar pulls, output-root creation, or summary
writes.
The Android attestation report writer rejects padded components in the
certificate-chain source path and report `--out` path before ancestor checks,
source metadata reads, output parent creation, or report writes.
Shared Android device-lab JSON source loading and the signed-slot assembler
also reject padded path components before JSON/source metadata reads, ancestor
validation, root classification, directory creation, or source-copy staging.
The signed-evidence helper also treats
`evidence` and `attestation` as directory labels rather than child artifacts,
so root-only signed-evidence outputs, offline-wallet APK paths, or
attestation-chain paths, including trailing-slash root aliases, fail before
hashing or writing signed evidence. The production readiness
rollup applies the same explicit slot-id validation before root classification.
Root-discovered scanner slots and top-level slot artifact entries are sorted by
directory name before scanning so JSON summaries, release inputs, and slot
diagnostics remain deterministic across filesystems.
Android device-lab and readiness-rollup summary construction copy direct report
dictionaries through a secret/control-string/non-finite-number sanitizer before
release-facing JSON rendering, preserve the first value and emit explicit
diagnostics for redacted report-key collisions, normalize malformed direct report statuses
to failed rows, normalize non-string direct report keys before JSON rendering,
redact non-finite direct report numbers, normalize unsupported direct report values,
normalize malformed direct report error lists to explicit safe placeholders,
normalize malformed direct Kagemusha report sections, render duplicate-binding
slot lists through safe slot labels, redact unsafe direct binding slot labels in duplicate and
malformed-digest blockers, reject malformed direct binding digests before
duplicate checks, reject all-zero binding digest placeholders, require canonical device-family strings before matrix
coverage, and only reflect duplicate-binding values and trusted-signer
summary keys that are non-zero canonical lowercase SHA-256 hex digests. Raw
scanner duplicate-binding diagnostics also ignore all-zero binding placeholders.
The direct
device-lab scanner summary and the production-readiness summary both require
complete signed evidence before a status-ok direct report can count toward
standard matrix coverage, offline D2D transport coverage, or release-facing
`duplicate_bindings` metadata. When scanner standard-matrix or production
evidence mode is active, the scanner also prunes release-facing per-slot
Kagemusha fields from `slots[*].kagemusha` unless that row passes the complete
trusted signed-evidence gate; non-release diagnostics are retained. The direct
scanner's standard-matrix mode and
the readiness rollup both require accepted Android evidence to cover every
declared offline D2D payment transport (`nearby_offline`, `nfc_hce`, and `qr`)
before a production release bundle can be marked ready. The readiness rollup
also records `covered_d2d_payment_transports_by_family` and
`missing_d2d_payment_transport_pairs`, and blocks the matrix unless every
standard Android device family has signed evidence for every required transport.
Release-bundle validation recomputes `missing_d2d_payment_transport_pairs` from
`covered_d2d_payment_transports_by_family` and rejects any forged or stale
missing-pair complement before a summary or existing bundle can be accepted.
The readiness rollup
only credits a multi-transport slot when `d2d_payment_transports` is a sorted
unique list that exactly matches a `d2d_payment_transcripts` object, every
entry has a canonical non-zero path/digest binding, paths and digests are
distinct across transports, and the primary transport
entry matches `d2d_payment_transcript_path` plus
`d2d_payment_transcript_sha256`; the direct scanner summary applies the same
exact-map rule before reporting transport coverage, and the scanner summary
plus release-bundle summary, manifest-shape
checks, and copied slot-artifact verifier mirror the same sorted/unique list
and artifact-root requirements before cutting Android D2D artifacts into a
production bundle: release-bundle slot artifact paths must stay under
`artifacts/android/device_lab/<slot>/`, D2D transcripts stay under `handoff/`,
wallet-integrity transcripts under `wallet/`, attestation chains under
`attestation/`, and offline wallet APKs under `evidence/`. Cross-slot reuse of
the primary `d2d_payment_transcript_sha256` or any
`d2d_payment_transcripts[*].sha256` map entry is reported through
release-facing `duplicate_bindings` and blocks readiness, so copied D2D
handoff transcripts cannot satisfy the transport matrix.
Android signed evidence also requires `signed_evidence_artifact_path` to bind
the canonical `evidence/signed-evidence.json`, and digest inventory only treats
children under artifact roots as digestable slot artifacts. The
release-bundle verifier also mirrors the localnet lifecycle identity contract:
run id, chain id, and peer ids must remain production/prod and localnet
labeled, free of non-production environment markers including joined labels
such as `localnetqa`, `localnetpreprod`, `localnetpreview`, `localnetstage`,
`localnettest`, `localnetuat`, and `localnetzero`, free of contradictory
mainnet/localnet labels such as `mainnetlocalnet` or `localnetmainnet`, and the
peer roster must stay sorted. It also checks exact localnet target, peer-count, and
artifact-count values. Localnet lifecycle artifact hashes in readiness summaries
and existing bundle manifests must also stay non-placeholder, so all-zero and
single-nibble repeated SHA-256 digests are rejected; readiness-summary
localnet hashes and existing-manifest localnet hashes must also remain distinct.
Slots may use the legacy primary `d2d_payment_transcript_path` and
`d2d_payment_transcript_sha256` fields for one transport, or add a signed
`d2d_payment_transcripts` object keyed by those transport names. Each map entry
must contain a `handoff/` path and matching non-zero SHA-256 digest, must include
the primary transcript binding, and must not reuse one transcript path for
multiple transports. Non-primary entries are packaged as distinct dynamic
release bundle artifacts so missing or digest-drifted NFC/QR/Nearby transcript
files fail `--verify-existing` instead of being hidden by the primary
transcript.
The physical Android lab exporter writes the required raw transport transcripts
as `handoff/d2d-payment.json`, `handoff/d2d-payment-nfc_hce.json`, and
`handoff/d2d-payment-qr.json`; the raw puller requires all three, and the slot
assembler binds the latter two with repeatable
`--d2d-payment-transcript-extra transport=path` arguments before the signer
copies the resulting `d2d_payment_transcripts` map into `signed-evidence.json`.
Duplicate-binding blockers still catch copied evidence from incomplete direct
reports, but release-facing duplicate summaries only reflect slots admitted
through the complete signed-evidence gates. Direct
signed-evidence summary fields are also revalidated before reflection:
timestamps must be canonical UTC, digest fields must be non-zero lowercase
SHA-256, artifact paths must be canonical safe relative paths under their
release roots (`evidence/` for the APK, `handoff/` for D2D payment
transcripts, `wallet/` for wallet-integrity transcripts, and `attestation/` for
attestation certificate chains), and multiple validated reports must not
collapse to the same redacted signed-evidence summary slot label. If a direct
signed-evidence report carries a partial or
non-standard device-family/model/codename tuple, readiness keeps the blocker but
omits all three identity fields from the per-slot signed-evidence summary
instead of publishing misbound identity metadata. Signed-evidence summary slot
keys must be safe real slot ids; unsafe, redacted, traversal-shaped, or
control-character slot names keep blockers but are omitted instead of being
published under placeholder keys. Direct reports with any
missing or malformed release-facing signed-evidence field keep their blockers
but are omitted from the per-slot signed-evidence summary and do not count
toward standard device-family coverage. Their
`android_device_lab.slots[*].kagemusha` release-facing fields are omitted until
the same slot is admitted into the complete signed-evidence summary. Duplicate
or colliding slot reports cannot borrow another report's admitted
signed-evidence entry; each slot report must exactly match the admitted
per-slot signed-evidence fields before its Kagemusha details or device family
are reflected. The rollup also requires the admitted
`signer_public_key_sha256` to be present in the validated trusted-signer digest
set, so a forged or regressed `status = ok` report with an untrusted signer
cannot populate `signed_evidence`, satisfy device-family or D2D transport
coverage, or expose release-facing slot Kagemusha fields. The signed-at
timestamp, signed-evidence artifact digest, and signer public-key digest are
reflected only as a complete provenance group. Release-artifact path/digest
bindings for the APK, D2D transcript, wallet integrity transcript, and
attestation certificate chain use the same all-or-nothing reflection rule.
The readiness rollup validates caller-provided trusted-signer maps before
Android root classification and only reflects non-zero canonical signer-key
SHA-256 ids in summaries; direct signer maps reject non-mapping containers, mixed malformed
digest keys without invoking caller-controlled key representations, and
non-`Path` values before slot metadata reads. They also recompute the OpenSSL
DER public-key SHA-256 for direct map entries and reject digest/path misbinding
before any slot metadata can be accepted. Trusted signer loading rejects PEM
private-key blocks before OpenSSL can derive a public key from signing-key
material, so release validation must be rooted in explicit public keys. Direct
release-bundle builders apply the same trusted-signer map
preflight before bundle-root metadata checks, the verify-existing path applies
it before manifest loading, and blocked manifests emit only non-zero canonical
signer digests from the same mapping-safe sanitizer. They also reuse the
repo-root alias validator so parent-segment or
backslash-bearing `--repo-root` aliases stop before bundle-root metadata reads
or release-bundle JSON loads.
Direct release-bundle build and verify calls also validate `repo_root` before
bundle-root metadata checks or readiness/release manifest loading, so unsafe
repository roots fail without touching bundled evidence.
Release-bundle verification also requires each `android_signed_evidence`
release entry to use the canonical
`artifacts/android/device_lab/<slot>/evidence/signed-evidence.json` path, and
requires the `android_signed_evidence` and `android_slot_artifacts`
inventories to match freshly computed release evidence. It then binds each
signed-evidence release entry plus every Android slot artifact release entry
back to the Android signed-evidence summary path/digest and freshly computed
release-evidence size before the generic manifest-drift comparison.
The release bundle Android summary fields, including `duplicate_bindings`,
`android_device_lab.slots`, the per-slot `signed_evidence` map, device-family
lists, and trusted signer digest list, are also compared with freshly computed
device-lab evidence during both readiness-summary comparison and
existing-manifest verification before generic summary or manifest drift.
Each per-slot signed-evidence summary entry carries `device_family`,
`device_model`, and `device_codename`; both readiness-summary validation and
release-bundle verification require non-empty exact strings and recompute the
standard matrix family from the model and codename before accepting the entry.
Build-time release-bundle comparison also reports exact signed-evidence and
slot identity drift with Android-specific blockers before the broader summary
metadata drift checks, so same-family model/codename substitutions do not hide
behind generic drift diagnostics.
Existing release-bundle verification applies the same exact identity binding to
manifest `android_device_lab.signed_evidence` entries and the corresponding
`android_device_lab.slots[*].kagemusha` identity fields before generic manifest
drift.
Android covered-family summary drift now fails with an Android-specific blocker
before generic summary drift.
Build-time readiness-summary comparison also rejects Android signed-evidence
slot inventory and per-slot field drift with Android-specific blockers before
generic summary drift.
Lineage and compact evidence digest/size maps in the readiness summary also
fail with section-evidence drift blockers before generic summary drift.
ABI-6, ABI-7, lineage release-tooling, lineage metadata including the required
test inventory, and compact metadata including record namespace/version fields
in the readiness summary fail with section-value drift blockers before generic
summary drift.
Existing release-bundle manifests also bind ABI-6, ABI-7, lineage tooling,
lineage proof evidence, and compact-key evidence section values back to freshly
computed release evidence before generic manifest drift, so canonical-looking
section timestamp or map edits fail with a section value binding blocker.
Lineage and compact release artifact entries, proof-log entries, and the
compact generator-log entry are likewise checked against their expected
bundle-relative paths plus release-section digest and size fields before
generic manifest drift.
The compact generator-log artifact digest and size maps are also bound to
freshly computed compact evidence before generic manifest drift.
Top-level readiness-summary, lineage evidence, compact evidence JSON, and
compact generator-log entries are also pinned to the canonical release-packet
filenames, digest, and size fields from freshly computed release evidence
before generic manifest drift.
Release-bundle build validation also compares per-slot Android signed-evidence
summary fields against freshly validated device-lab evidence before generic
summary drift, so safe but forged slot artifact paths fail with a
signed-evidence drift blocker.
Slot-relative artifact path normalizers also reject
control-character or surrounding-whitespace relative paths before stripping,
manifest, metadata, signed-evidence, or signer digest reads.
The signed-evidence helper also rejects control-character slot, private-key,
public-key, and signed-evidence output paths before metadata reads, OpenSSL
lookup, JSON parsing, or output parent creation. Its lower-level JSON output
write/read validators also reject parent-segment and backslash-bearing output
aliases before output parent metadata reads, so direct helper calls cannot
normalize those paths after review.
The Android attestation-report writer rejects control-character local
certificate-chain source paths before ancestor validation or metadata reads,
matching the slot-relative certificate-chain path preflight.
The signed-slot assembler source-copy preflight rejects control-character,
surrounding-whitespace component, parent-segment, and backslash-bearing
artifact source paths before ancestor validation, metadata reads, or
destination directory creation, and its device-lab root path preflight rejects
control-character, surrounding-whitespace component, parent-segment, and
backslash-bearing roots before root classification or directory creation.
Slot assembler source metadata strings also reject control characters before
they can be copied into signed slot metadata. The signed-slot assembler can
now derive missing device identity from validated source artifacts before
trying ADB: attestation result/report provide fingerprint and OS build hints,
and
telemetry provides model/codename hints. Those source identity hints use the
same no-surrounding-whitespace, no-control-character, and no-secret-material
validation as explicit overrides, require duplicate source hints to agree, and
require explicit overrides to match captured hints when both are present.
Present-but-empty source identity fields are rejected instead of being treated
as missing, and explicit empty identity overrides are rejected instead of
falling back to source artifacts or ADB. Malformed or conflicting hints fail before any slot is installed.
The signed-slot assembler source digest preflights reject blank or noncanonical
attestation challenge, app-signing, and offline-policy SHA-256 fields before
unsigned staging output or signed evidence can be published.
The Android raw puller and signed-slot assembler now also report temporary
staging directory removal and cleanup-sync failures, and block success when the
original staging directory cannot be removed or the containing directory cannot
be synced afterward; identity-swapped staging directories are still preserved
instead of removed.
The signed-slot assembler tightens pre-existing device-lab roots and all staged
or published slot directories to `0700`, installs copied source artifacts and
generated JSON/manifest/signature evidence as `0600`, and verifies those modes
after write/publish before returning success. Copied source artifacts and
normalized JSON metadata are installed through captured parent descriptors, and
post-install parent-sync failures roll back only the just-installed file
identity before the assembler reports failure.
Raw partial-install cleanup also reports removal failures, so a failed install
cannot hide an unremoved partially-created slot directory.
The raw puller also redacts control-character or secret-looking unexpected
top-level install-source names before reporting raw slot install failures.
Standalone raw-puller ADB stderr, launch-error, and timeout details are also
bounded and redacted before reporting latest-slot or tar-pull failures,
including non-UTF-8 latest-slot and tar stderr, and the configured ADB serial is
redacted from ordinary attached-device failure text before it reaches stderr.
It now rejects control-character output-root, summary-output, raw-slot, and
raw artifact path strings, plus parent-segment and backslash-bearing
output-root, summary-output, and raw-slot aliases, before ADB access, metadata
reads, directory creation, or error reporting can expose the raw bytes; raw
`attestation/result.json` identity strings and raw tar-member paths also reject
control characters before evidence assembly or tar path normalization, and
noncanonical tar member spellings such as `./` or repeated separators fail
instead of being normalized into accepted evidence paths. It also accepts only
the uncompressed `tar -cf -` stream emitted by the Android exporter, so
compressed archive streams fail before extraction, and caps the total raw tar
entry count so empty-directory flooding cannot exhaust host-side staging before
slot validation. The local raw output root is tightened to `0700`, and extracted
raw artifact directories are materialized one component at a time through
no-follow directory descriptors, forced to `0700`, and files are created with
exclusive descriptor-relative `0600` opens before installation. Raw
`latest-slot.txt` and raw-pull summary outputs are likewise
forced to `0600`, written through descriptor-relative temporary files and
replacement calls, and verified during final opened-file readback, so evidence
confidentiality does not depend on the host process umask or a mutable public
pathname. Their failed-write and failed-parent-sync cleanup paths recheck file
identity through the captured parent descriptor before unlinking, then fsync
that descriptor after successful cleanup.
All of these release-output writers also fail closed if the parent-directory
sync after atomic replacement fails, so a release/readiness artifact is not
accepted as durable when the directory entry cannot be fsynced. The readiness
summary and release-bundle writers also reject parent-directory identity swaps
before that fsync, so a replaced output cannot be accepted after its target
directory has been exchanged. The staged lineage and compact-key finalizers use
the same identity discipline for rollback cleanup: failed publish paths are
removed only while their publish-time identity is still present, then the
captured artifact-directory descriptor is fsynced before cleanup is reported
successful.
They also force published artifact directories and finalizer temporary staging
directories to `0700`, and force every copied/published lineage, compact-key,
log, and evidence JSON file to `0600` before final readback.
Both staged finalizers also reject noncanonical or future-dated
`--generated-at-utc` values during preflight, before temporary publish staging
or artifact copying starts.
If a staged finalizer cannot remove a publish-time file during rollback, that
cleanup failure is returned with the original publish failure instead of being
silently swallowed; rollback cleanup sync failures are reported the same way.
Finalizer temporary staging directory cleanup also reports removal failures and
cleanup sync failures, and can block success; cleanup still preserves a
directory whose identity changed before removal.
Staged lineage and compact-key runner/finalizer path validators reject
control-character staging, exit-marker, elapsed-seconds, artifact, and output
paths before ancestor validation, metadata reads, or staged output cleanup can
start. Both staged finalizers also check missing `--artifact-dir` publish
directories for symlinked ancestors before creating them, so a caller cannot
publish lineage or compact-key evidence through an alias parent that does not
yet contain the final directory name. Finalizer-created artifact directories
are opened and created one component at a time through directory file
descriptors with no-follow flags, so a parent-directory swap during creation
cannot redirect evidence into a symlink target. Publish-stage temporary files,
renames, byte verification, and rollback cleanup are likewise anchored to the
captured artifact-directory file descriptor; if the public artifact path is
swapped before final sync, the finalizer fails closed and removes the files it
installed through that descriptor instead of populating the swapped-in target.
Android signed-evidence canonical signature payloads also serialize with strict
JSON before hashing, signing, or verification, so non-standard constants cannot
become signed bytes.
Readiness enforces the production proof-log and compact generator-log byte caps
from the opened file metadata used for hashing and decoding, so a separate
path-size lookup cannot satisfy the size gate for replacement log bytes.
The checked-in ABI-6 manifest and both Kagemusha evidence JSON files use the
same fail-closed size policy before JSON decoding.
Both helpers create missing output parents only after these preflights,
classify `--out` parents with `lstat()` before any `Path.is_dir()` preflight,
recheck created output parents before direct helper preflight returns, and
force the helper-controlled output parent to `0700`. Validation scratch files
under `--artifact-dir` and final `lineage-proof-evidence.json` /
`recursive-compact-key-evidence.json` writes are forced to `0600` and verified.
The localnet lifecycle helper also validates through a private `0700`
artifact directory with a `0600` scratch file, then writes
`kagemusha-localnet-lifecycle-evidence.json` through the same private output
path, so direct evidence generation does not depend on the operator shell's
umask. Localnet raw CLI path strings are screened for secret-looking and
control-character material before `Path(...)` conversion; output-path errors
stop before input validation, evidence building, document validation, or final
publication; input-path errors stop before evidence building, validation, or
final publication; build errors stop before validation or final publication;
validator blockers stop before the final writer is called; and if that final
localnet evidence write fails, the helper reports the structured writer error
and does not print a successful publication message.
The lineage proof, recursive compact key, and localnet lifecycle validators
also fsync the artifact directory after identity-checked validation scratch
cleanup, so cleanup durability failures are reported.
The localnet lifecycle helper passes the same localnet evidence size cap into
the final private writer that it uses during validation, so final publication
does not inherit the Reserved-lineage writer default.
The private final writers also fsync output-directory cleanup after removing an
identity-checked temporary file, so failed publication attempts report cleanup
durability failures instead of treating unlink as enough. Their published
rollback helpers also fsync after identity-checked cleanup on parent-sync
failure.
Android slot metadata JSON, attestation report, and signed-evidence writers use
the same output-directory fsync after identity-checked temporary cleanup on
failed-write and post-stage output-validation paths, and their published
rollback helpers fsync the parent descriptor after cleanup on parent-sync
failure. The shared Android device-lab `--json-out` summary writer, Android
capture-summary writer, raw-puller summary/latest output helpers,
production-readiness `--summary-out` writer, and release-bundle `--out` writer
follow the same rule, including descriptor-relative parent fsync after capture
rollback cleanup.
The lineage proof, recursive compact key, and localnet lifecycle helpers also
reject malformed timestamps, commands, and elapsed-time scalars at both the
direct builder and CLI boundaries before artifact, log, acceptance-report, or
output-corridor metadata is read.
They also reject noncanonical proof-log, compact generator-log, and localnet
acceptance-report filenames before artifact-directory metadata is read.
The readiness rollup applies the same fail-closed ordering to the three
release evidence JSON filenames: a noncanonical lineage proof, compact key, or
localnet lifecycle evidence path returns a filename blocker before ancestor
validation, file metadata, or JSON loading starts.
After atomic replacement, both helper outputs revalidate final path shape and
the public output-parent identity, capture the output identity through the held
parent descriptor, roll back the installed evidence JSON on parent drift or
final parent-sync failure only when that identity still matches the just-written
file, read back through the opened regular file, and reject post-replace symlink
or regular-file swaps as `--out changed while being read`.
Input and output corridor resolution failures return structured `--proof-log
parent`, `--acceptance-report parent`, `--out parent`, or `--artifact-dir`
blockers instead of raw resolver errors, and the localnet lifecycle helper pins
the same input and output resolver failures before localnet acceptance metadata
is read.
The shared evidence builder also rejects secret-looking, control-character, or
surrounding-whitespace `--artifact-dir`/`--proof-log`
paths and detached proof logs that are not the canonical
`record-archive-proof.log` directly under `--artifact-dir` before hashing
artifacts or reading the proof log. Direct validation and output-writer helpers
also reject control-character or secret-looking artifact or output paths before
creating temporary directories or writing evidence JSON, and final
evidence-output write failures return `--out could not be written`. The readiness rollup and evidence helper
both convert read-time failures while hashing lineage artifacts or proof logs
into structured blockers after rerunning the local-file preflight.
Do not set `IROHA_KAGEMUSHA_ALLOW_RUNTIME_LINEAGE_KEYGEN` for that proof run;
release evidence with runtime keygen enabled is rejected.
If a release process already has a verifier-key file and only needs the WSV
record, build it without regenerating or deriving keys:

```bash
iroha app zk kagemusha lineage-record \
  --profile init \
  --opening-len 128 \
  --vk artifacts/kagemusha/lineage-init-len128.vk \
  --out artifacts/kagemusha/lineage-init-len128.record.norito

iroha app zk kagemusha lineage-record \
  --profile append \
  --opening-len 128 \
  --vk artifacts/kagemusha/lineage-append-len128.vk \
  --out artifacts/kagemusha/lineage-append-len128.record.norito
```
Receivers can verify, store, and re-spend without contacting a node while the
D2D payload keeps only this constant-size proof-chain commitment instead of
prior proof bundles. The final holder submits the recursive bundle, public
redeem amount, and final unshield proof for online redemption. Reserved-lineage
bundles whose hop count is inside
`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64` can redeem
witnesslessly when they carry the active lineage verifier record and pass
chain admission. Semantic v1 recursive bundles still carry a record-backed
lineage witness for online redemption.
Chain admission first honors the `kagemusha_enabled = true` config gate, validates the recursive proof envelope,
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
Redeem requests retain the legacy single `lineage_verifier_record` field and
add the trailing defaulted `lineage_verifier_records` vector for additional
Reserved-lineage verifier profiles. Multi-profile record-backed lineage
witnesses must provide records for every Reserved-lineage previous proof; newer
SDK typed builders may place all Reserved-lineage records in the plural field,
while older single-record callers remain valid for one-profile cases.
Torii offline-v2 redeem ingress routes `/v1/offline/v2/notes/redeem` requests
that carry `redeem_request_norito_base64`,
`compact_payment_token_norito_base64`, or
`projection_verifier_record_norito_base64` through the Kagemusha recursive
redeem path instead of the legacy Offline Note V2 redemption parser. Production
ingress requires a canonical standard-base64
`KagemushaRecursiveSpendRedeemRequestV1` archive with no surrounding
whitespace, validates its public binding, and rejects authenticated-account,
chain-id, asset, requested-amount, and source note commitment mismatches before
emitting a `RedeemKagemushaRecursive` transaction or settlement response. If
the optional `amount` or `source_note_commitment` echo fields are present, they
must be canonical non-empty strings and must match the archive; amount echoes
must use the exact canonical `Numeric` text, so plus signs, leading zeroes, and
redundant decimal points are rejected.
The compact-token and projection-verifier fields are dispatch markers only:
once `redeem_request_norito_base64` is present, those auxiliary fields must be
omitted so stale or mismatched client-side token material is not silently
ignored by Torii.
Legacy Offline Note V2 redemption fields do not act as a fallback once any
Kagemusha redeem, compact-token, or projection-verifier field is present; mixed
legacy/Kagemusha bodies are rejected before archive decoding.
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
Lineage witnesses therefore allow carried previous recursive proofs to expose a
different fixed-window table-base public input from the current bundle proof,
while still rejecting splices in stable verifier context such as opening length,
parameters, schedule, shared manifest, and scalar projection.

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
The native privacy bridge keeps those builders fail-closed by default and
dispatches to the real Halo2 IPA prover/verifier only when
`privacy-production-enabled` is compiled in. Android production builds must pass
that feature to the cargo-ndk bridge build only after the production gate
evidence is complete. Kotlin/JVM and Java Android derive their public privacy
readiness booleans from the native Norito capability archive when the bridge is
loaded; malformed, duplicate, incomplete, or missing native evidence keeps the
SDK capability surface fail-closed. Unshield v3 also rejects overflowing input
amount sums before proving, so malformed witness archives return the proving
failure status instead of wrapping the private total.
Public JavaScript production-evidence rows now mirror the Python privacy catalog
by requiring exact `sdk_exports` and `review_scope` sections before a row can
promote readiness: every SDK surface repeats the admitted entrypoint list, and
the review scope binds algorithm id, chain id, verifier metadata, required
state, fuzz/performance artifact hashes, and the localnet run id.
The JavaScript package declarations expose the same derived SDK export, review
scope, SDK parity artifact, localnet acceptance, fuzz/performance result, and
gate-evidence shapes as readonly TypeScript surfaces for descriptor consumers.
Package-root `getPrivacyCapabilities(...)` tests also exercise complete
production evidence so the distributable import path observes the same derived
fields and immutable evidence objects. Those declarations must not expose
recursive-spend accumulator digest knobs under exact or prefixed names:
prefixed aliases such as `terminalAccumulatorDigest` and
`walletRecursiveProofChainDigest`, and suffixed aliases such as
`terminalAccumulatorDigestV1` and `walletRecursiveProofChainDigestBytes`,
remain native-owned and are rejected by the package declaration sweep.
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
hop's verifier-key binding, and the aggregation mode. Checked transparent
pre-fold v1 remains the legacy folded-token verification mode. ABI 7 recursive
compact mode `2` is live only through the `kagemusha-recursive-compact-v1`
prover/verifier path, where the compact token is preverified against the
canonical recursive compact verifier record; the legacy checked verifier still
rejects mode `2`. Folded inputs also expose a Poseidon2 aggregation transcript
digest over the canonical hop sequence. The ordinary Iroha `fold_digest`
remains available for host-side lineage checks, while the Poseidon2 digest gives
the recursive compact verifier a hash-friendly public accumulator without adding
a trusted setup.
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
be folded even through lower helper entry points. Halo2 IPA confidential-transfer-v2
hop envelopes must expose exactly nine single-row public instance columns; extra
public columns or rectangular multi-row ZK1 instance payloads reject before the
folded transcript or Pallas opening metadata can be derived. Missing
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
Native bridge ABI 7 keeps the recursive compact-token entry point
`connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes`.
The ABI-7 recursive compact-token symbols now route one-hop
`kagemusha-recursive-compact-v1` compact proving when the native proof bundle
carries packaged compact one-hop and append proving-key archives,
LEN=4 compact-token proof path inputs, and matching verifier-slice open-envelope
evidence. The routine readiness route still validates the ABI-6 reserved-lineage
recursive spend verifier and redemption surface, while SDK preferred-mode selection can choose ABI-7 compact when compact prover/verifier support is
advertised. Package-backed ABI-7 compact callers must pass the
Norito key-artifact or verifier-key package explicitly; malformed or missing
packages fail closed before proving or verification. Generic compact-token
reservation and compact-first SDK selection do not open receiver admission
without the packaged-key and evidence gates. Python compact-projection wrapper
regressions now also pin permissive malformed-probe rejection, immutable native
dispatch copies for mutable archives, unsafe native projection output rejection,
and non-boolean verifier rejection on both optional-height entry points. Swift
projection wrapper regressions pin the same ABI-7 boundary for empty or
malformed bundle and compact-token inputs before bridge dispatch, nil or empty
native projection output, empty-payload projected tokens, and projection
verifier native rejection mapping. The production-readiness guard now
mutates the Rust selector and the JavaScript, Python, Swift, Kotlin, Java
Android, and C# SDK selectors as negative controls, so cross-SDK compact-default
drift is rejected before release evidence is trusted. It also self-audits that
every production-readiness negative-control handler is routed through the PR
workflow and workflow requirement inventory exactly once. Release key-artifact commands, the
Reserved-lineage/recursive-compact evidence JSON helpers, Android signer
`signed-evidence.json` and `sha256sum.txt` outputs, and readiness
`--summary-out` writes use same-directory temporary files, fsync the bytes,
atomically rename the finished file into place, read back JSON outputs, and sync
the parent directory when the platform permits it, so interrupted evidence runs
cannot leave a partial artifact under a trusted release path. The ABI-7 compact verifier
key remains CID-distinct from the ABI-6 recursive aggregation verifier key while
reusing the semantic aggregation circuit shape for projection tests; those
projection helpers are not a receiver-admission path.
Bridge ABI 6 introduced, and ABI 6-or-later bridges expose, the production
recursive spendable-cash entry points:
`connect_norito_kagemusha_recursive_spend_init`,
`connect_norito_kagemusha_recursive_spend_append`,
`connect_norito_kagemusha_recursive_spend_transition_profile_init`,
`connect_norito_kagemusha_recursive_spend_transition_profile_append`,
`connect_norito_kagemusha_recursive_spend_lineage_append_boundary`,
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
witness before going online. SDKs also keep shortened
`lineage_witness_required` / `lineageWitnessRequired` aliases for compatibility.
The
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
before proving or returning output bytes. Those Norito `Option<[u8; 32]>`
metadata bodies are the raw 32-byte value inside the option payload, not a
per-byte fixed-array child sequence; non-C# SDK request preflight and
JavaScript package-dist coverage reject stale fixed-array metadata bodies before
native dispatch. Native append
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
JavaScript/Node, Python, Swift, Kotlin/JVM, Java Android, and C# bundle-summary
decoders apply the same supported-previous-proof-circuit check, and require the
decoded verifier-key and proof backends to remain `halo2/ipa` with non-empty
recursive proof bytes, before returning accumulator or note summary metadata.
They also require the recursive proof to carry non-empty public inputs and a
non-zero 32-byte public-input hash that matches `Hash::new(...)` over the
compact `KagemushaRecursiveAggregationProofPublicInputs` Norito archive before
summary metadata is returned.
Python, Swift, Kotlin/JVM, and Java Android direct bundle-summary construction
applies the same accumulator summary invariants before wallet code can trust
manually assembled metadata.
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
previous top-up anchor nullifiers for append profiles,
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
example, the second offline spend hashes as hop `1`. They also validate the
previous top-up anchor vector before hashing and reject append outputs or
current-note fields that reuse any carried previous anchor. Before returning a
profile digest, validators reconstruct the resulting accumulator with the
profile's non-circular binding digest and compare both
`resulting_accumulator_digest` and `resulting_public_inputs_hash`; detached
profiles with refreshed but forged result hashes fail before append-boundary
derivation. SDK fixtures should compare
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
shared-table manifest digest. The public previous-proof opening domain-tag and
metadata helpers validate the exact previous accumulator/proof public-input
binding before hashing or returning metadata, so callers cannot derive opening
metadata from a mismatched previous bundle. Core checks that this contract is
attached to a Reserved-lineage previous proof, that its previous
accumulator/proof artifact digests match the
actual previous bundle, and that the resulting accumulator public inputs carry
the same append opening preflight digest, compact append-boundary digest, and
verifier corridor. The Reserved-lineage append proof branch binds those digests
into the transition profile before emitting output. This material is circuit
input and remains bounded by the 64-hop witnessless cap. Recursive proof
public inputs also fail closed if a one-hop proof carries a non-zero append
opening preflight digest; append-opening and append-boundary fields only become
admissible after the first append hop, and append-boundary values additionally
require the compact boundary contract. Semantic recursive spend proofs also
reject append-opening preflight state, even when it is consistently carried by
the accumulator and proof public inputs; digest-only append-opening state is
transition evidence, not a semantic final proof channel. Generic recursive
aggregation proofs must keep recursive spend state zero: proof-chain,
transition-profile binding, append-opening, append-boundary, and recursive
verifier scalar-projection digests are accepted only by recursive spend or
Reserved-lineage proof binding. The standalone recursive spend proof artifact
digest helper runs the same circuit gates before hashing: plain recursive
aggregation proofs cannot be promoted into spend artifacts, semantic spend
proofs cannot carry Reserved-lineage-only append-opening, append-boundary, or
scalar-projection state, and Reserved-lineage proofs that carry an append
opening preflight digest must also carry the compact append-boundary digest.
The native C
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
verifier wrappers reject malformed Reserved-lineage verify request archives
before returning a diagnostic result, including requests missing the lineage
record or carrying a forged lineage-record commitment, so receivers do not
store or re-spend unverifiable witnessless cash.
The typed non-C# request constructors and encoders also decode the final bundle
summary before native dispatch: Reserved-lineage final bundles fail without
`lineage_verifier_record`, while semantic final bundles fail if that field is
present. This keeps SDK-created archives from reaching native/core with a
lineage-record shape that the verifier would reject later. Their typed init
and append constructors also decode the supplied record-bundle archive before
serialization, derive the folded hop count, require the
`Vec<iroha_zkp_halo2::OpenVerifyEnvelope>` Pallas archive to contain exactly
one structurally valid envelope per hop, and require lineage-append
`previous_proof_open_envelopes` to contain exactly one bounded Pallas envelope.
Malformed Pallas schema hashes, unsupported curve ids, missing transcript
labels, missing non-zero metadata options, wrong generator/opening counts, and
trailing bytes fail before native dispatch.
Redeem request record selection is witness-aware rather than the verify
request's final-bundle-only rule. Reserved-lineage final bundles require
`lineage_verifier_record`; semantic final bundles may carry it only when a
record-backed `lineage_witness` is present and proves a prior Reserved-lineage
recursive proof. The non-C# typed redeem constructors and encoders now decode
the lineage-witness previous-proof summary before native dispatch: semantic
final bundles require `lineage_verifier_record` when the witness contains a
prior Reserved-lineage recursive proof, and they reject the record when no such
prior proof is present, including the no-witness case. Native/core remains
authoritative for full lineage-witness replay, verifier-record metadata, and
proof binding.
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
lineage verifier record matching the final proof circuit id, and semantic final
bundles must leave it empty unless a record-backed witness path explicitly needs
the record to verify prior Reserved-lineage proofs with the same circuit id.
The C bridge, Node NAPI host, and Python PyO3 host pin that boundary with
wrong-`circuit_id` regressions so raw-archive redeem builders reject a
valid-looking lineage verifier record from another Reserved-lineage circuit
family before serializing an instruction.
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
builder. Native append streams the previous recursive proof bytes and per-hop
accumulator material into native-owned accumulator digests
(`recursive_proof_chain_digest`, lineage/aggregation transcript, fixed-window
schedule/shared-manifest/table-base, verifier-witness batch, transition-profile,
append-opening-preflight, append-boundary, scalar-projection, and
previous/resulting accumulator digests); SDKs must not derive, supply, or patch
accumulator state themselves. Append validation also rejects a stale cached
`previous_recursive_proof.public_inputs_hash` before accepting the new append
state. Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C# expose
current-hop and previous-proof Pallas open-envelope archive builders
(`buildPallasOpenEnvelopesArchive`,
`buildPreviousProofOpenEnvelopesArchive`,
`kagemushaBuildPallasOpenEnvelopesArchive`,
`kagemusha_build_pallas_open_envelopes_archive`, and
`BuildPallasOpenEnvelopesArchive` /
`BuildPreviousProofOpenEnvelopesArchive`) so wallet code asks native
code to derive those archives from the record bundle or previous recursive
bundle. SDKs should treat the generated archives as native-owned opaque Norito
bytes. The JavaScript published package mirrors the source-side record-backed
compact-token, recursive aggregation, and Pallas builder wrappers with
regressions for owned archive dispatch, copied `Buffer` outputs, invalid local
archive rejection before native dispatch, malformed native output rejection,
and empty-payload native output rejection. The CI
benchmark
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
fixtures use the actual split Reserved-lineage proof ids
(`kagemusha-recursive-spend-lineage-onehop-v1` at hop 1 and
`kagemusha-recursive-spend-lineage-append-v1` afterward), metadata-bound
previous recursive proof opening archives, full append opening-preflight
contracts, derived compact append boundaries, and accumulator-carried
`append_boundary_digest` values. The CI reducer writes
`reserved_lineage_payload_bytes.tsv` and
`reserved_lineage_transition_profile_bytes.tsv` and checks those series for the
same hop-count independence. The current fixed-proof Reserved-lineage D2D bundle
is 3,847 bytes, and the append Reserved-lineage transition profile is 2,817
bytes, with separate size ceilings because the Reserved-lineage fixtures carry
proof-opening metadata that the compatibility semantic payload does not.
The dedicated `Kagemusha Payload Benchmark` workflow runs
`ci/check_kagemusha_recursive_spend_payload_bench.sh` on relevant Kagemusha
payload, accumulator, and proof-surface changes and uploads the reduced-sample
Criterion summary.
Native bridge ABI 6 also retains
`connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes`.
That proof-carrying entry point accepts the same record-backed bundle plus a
Norito archive of proof-derived Pallas opening envelopes, enforces active
verifier-record and hop-proof checks, binds the Pallas envelope metadata to each
hop proof, and returns a Norito-encoded
`KagemushaRecursiveAggregationProofBundle`. It is still admission-neutral:
the Python native extension mirrors the same path for local proof-bundle
generation.
ABI 7 reserves the same record-backed private-hop evidence for
`kagemusha-recursive-compact-v1`. The public compact-token prover/verifier now
admits the production LEN=4 one-hop verifier-slice profile and rejects semantic
compact-CID envelopes that omit the in-circuit verifier-slice side column.
The package-aware compact helper now owns the append verifier-slice loop for
multi-hop proof construction when compact key artifacts are supplied, but
compact-first SDK selection still depends on advertised compact prover/verifier
support and the compact artifact/evidence gates. Multi-hop Pallas
verifier-batch archives with missing openings, forged metadata, duplicated
openings, or reordered openings fail as record-backed preflight drift before
compact proof generation. The height-aware core compact
prover also rejects detached Pallas archives, extra one-hop Pallas openings,
and the same missing-opening, forged-metadata, duplicated-opening, and
reordered-opening multi-hop preflight drift before default selection can treat
ABI-7 compact as production-ready. Wallets that need production offline-offline
spend-again behavior should use the recursive spendable-cash path, while
recursive aggregation proof bundles remain admission-neutral.
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
verifier-record checks. Their recursive compact-token and record-backed
recursive aggregation entry points reject empty or malformed local Norito
archives before owned-copy or native dispatch, so caller-input failures stay
distinct from reserved recursive compact unavailable diagnostics.
Swift, Kotlin/JVM, Java Android, JavaScript/Node,
Python, and C# also expose `KagemushaRecursiveSpend*` helpers around the ABI 6
recursive spend init/append/lineage-witness/verify/redeem surface, with
empty-input and malformed-archive rejection before native calls where the host
language can preflight. Swift exposes the witness helpers as
`lineageWitnessFromInitResult` and `lineageWitnessAppendResult`; Kotlin/JVM and
Java Android expose the same names, JavaScript/Node and Python use the native
snake/camel-case bridge names, and C# exposes
`LineageWitnessFromInitResult`/`LineageWitnessAppendResult` DTO wrappers.
C# also exposes `ValidateRedeemLineagePreflight(...)`,
`ValidateVerifyLineagePreflight(...)`, `ValidateRedeemChangeOutputPreflight(...)`,
`ValidateRedeemChangeOutputBytes(...)`, and metadata-bound `Redeem(...)`
overloads so callers that already decoded the bundle circuit id, hop count,
public amount, current note amount, change-output presence, and change-output
bytes reject missing semantic lineage witnesses, missing Reserved-lineage
verifier records, partial-without-change, over-amount, full-with-change,
short change commitments, and all-zero change commitments before P/Invoke
dispatch. Metadata-bound C# `Verify(...)` overloads expose the same final
bundle/lineage-record selection rule as the typed SDK request constructors:
Reserved-lineage final bundles require a lineage verifier record, while
semantic final bundles reject a dangling record before P/Invoke dispatch. The
transaction builder exposes matching metadata-bound
`KagemushaRecursiveRedeem(...)` overloads before builder mutation. C# amount
metadata uses canonical positive unsigned decimal u128 text, so padding, sign
characters, decimal points, zero, and overflow values fail before native request
parsing.
Swift, Kotlin/JVM, Java Android, JavaScript/Node, and Python also expose typed
ABI-6 recursive-spend request codecs for init, append, verify, and redeem, plus
verify-result and bundle summary decoders. Those codecs validate nested Norito
archives, canonical nonzero note amounts, nonnegative block heights, previous
lineage-record gaps, and append-output transition compatibility before native
dispatch. Typed verify request encoders and C# metadata-bound verify preflight
additionally enforce final bundle/lineage-record selection before native calls:
missing records are invalid for Reserved-lineage bundles, and extra records are
invalid for semantic bundles. Non-C# typed redeem request encoders use a
different semantic-bundle rule: they decode the lineage-witness previous-proof
summary, require lineage verifier records when semantic witnesses contain prior
Reserved-lineage proofs, and reject dangling records for init-only or absent
lineage witnesses. JavaScript source and package-dist compute raw plural
`lineageVerifierRecords` supplied state before normalizing record refs, so
semantic bundles with dangling plural record fields fail on field selection
instead of record archive parsing. The same non-C# plural record paths pin
request-state ownership: Swift stores value-typed `Data` and arrays, JavaScript
normalizes to frozen record refs with copied record bytes, Python freezes tuple
inputs, and Kotlin/JVM plus Java Android copy caller-owned lists into unmodifiable
request state before encoding. JVM/Android record refs also copy verifier-record archive
bytes on construction and accessor reads. Full record-backed lineage-witness
validation remains enforced by native/core. Their request-layout regressions decode the emitted
archives and pin
raw embedded record/bundle/proof payloads, Norito `Option` child-length framing,
and Rust `[u8; N]` fixed-array encoding as per-element compact
length-prefixed bytes without an extra sequence-length header. C# exposes a
managed `DecodeBundleSummary(...)` for the same bundle-summary preflight before
wallet code trusts decoded metadata. Swift exposes
the same surface through value-typed request/result structs,
`KagemushaRecursiveSpendRequestCodecs.encode*Request(...)`,
`decodeVerifyResult(...)`, `decodeBundle(...)`, and typed
`KagemushaRecursiveSpendProver` overloads. JavaScript/Node exposes the same
surface through object request inputs, TypeScript declarations,
`encodeKagemushaRecursiveSpend*Request(...)`,
`decodeKagemushaRecursiveSpendVerifyResult(...)`,
`decodeKagemushaRecursiveSpendBundle(...)`, and typed native convenience
wrappers that delegate only after request encoding succeeds. Python exposes the
same surface through frozen request/result dataclasses,
`encode_kagemusha_recursive_spend_*_request(...)`,
`decode_kagemusha_recursive_spend_verify_result(...)`,
`decode_kagemusha_recursive_spend_bundle(...)`, and typed native convenience
wrappers that delegate only after request encoding succeeds.
JavaScript/Node, Python, Swift, Kotlin/JVM, Java Android, and C# recursive-spend
bundle decoders also fail closed on accumulator summaries with `hop_count == 0`
or `hop_count > 64` before trusting the decoded chain, asset, root, or note
summary. The package-dist JavaScript coverage mutates the same shared fixture so
published package layouts cannot drift from source decoding behavior.
The recursive-spend bundle decoders also reject missing recursive-proof public
inputs, all-zero public-input hashes, and public-input hash mismatches before
native dispatch, with package-dist JavaScript, C# xUnit coverage, and SDK
parity negative controls pinning those fixture mutations.
JavaScript/Node, Python, Swift, Kotlin/JVM, Java Android, and C# also pin
adversarial current-note fixture mutations that reject all-zero note
commitments, all-zero spend nullifiers, note/nullifier aliasing, and zero
amounts before native dispatch. The SDKs and JavaScript package-dist coverage
also reject short/long fixed-array encodings for the nested note commitment and
spend nullifier fields before summary metadata is trusted. The same bundle
summary decoders reject raw accumulator `chain_id` string payloads,
proof-box-only backend drift, and extra fields appended to the top-level
bundle, accumulator summary, nested current note, current-note amount,
recursive proof, verifier-key id, and proof box before summary metadata is
trusted. Editable non-C# decoders and JavaScript package-dist coverage also
pin a combined proof-box vector where `ProofBox.backend = halo2/kzg` and
`ProofBox.bytes` is empty, requiring the `bundle.proof_backend` diagnostic
before the empty-proof-bytes diagnostic. They also pin the same precedence for
lineage-witness previous recursive proofs, requiring
`lineageWitness.previousRecursiveProofs.proof_backend` before the empty
previous-proof diagnostic. The C# mirrors are part of the managed
decoder parity guard and still need Windows host certification alongside the
rest of the C# lane.
The same managed decoder guard now also covers ABI-7 verify-result archives and
lineage-witness summaries, rejecting surplus fields on verify results, top-level
lineage witnesses, previous-recursive-proof sequences, individual previous
proofs, and nested previous-proof verifier-key ids before wallet preflight can
trust those summaries.
Python, Swift, JavaScript/Node, Kotlin/JVM, Java Android, and C# also fail
closed when proof-producing native calls return no archive or a zero-length
archive, so missing native proof material cannot be coerced into a successful SDK
result. C# Torii identifier policy summaries and flat resolve receipts also
reject padded proof metadata, duplicate JSON fields, and negative receipt
timestamps before wallet or verifier code can trust the returned account
binding. Kotlin/JVM and Java Android `ClaimIdentifier` wire encoders require
the caller account id to be exact before comparing it with the receipt account,
so padded account strings cannot be normalized into a signed identifier claim.
Kotlin/JVM, Java Android, Swift, and JavaScript/Node identifier claim-record
parsers also require exact returned `policy_id`, `opaque_id`, `receipt_hash`,
`uaid`, and `account_id` strings, so Torii receipt-hash claim lookups cannot
trim persisted claim state before wallet code consumes it.
The same non-C# SDK surfaces now fail closed on padded RAM-LFE execute and
receipt-verify response program IDs, hash fields, backend tags, verification
modes, and exposed output ciphertext before proof material reaches wallet code.
Their RAM-LFE program-policy list parsers also require exact returned program
IDs, owners, resolver keys, backend and verification-mode tags, input
encryption metadata, and proof-verifier metadata, so policy material cannot be
trusted after whitespace trimming or case normalization. JavaScript/Node also
preserves the returned `proof_verifier` object on program-policy summaries,
matching the Java, Kotlin/JVM, and Swift policy metadata surfaces.
Identifier-policy summaries use the same `proof_verifier` exactness, and
JavaScript/Node now preserves that object instead of dropping it during
normalization. The same JavaScript/Node, Swift, Kotlin/JVM, and Java Android
identifier-policy parsers also reject padded or case-normalized returned owner,
normalization, backend, input-encryption, input-parameter, nested
`norito_length_encoding`, and note metadata before encrypted identifier policy
material reaches wallet code. Account-alias resolution parsers on those same
non-C# SDKs also reject padded returned `alias`, `account_id`, alternate
`account_ids`, and `source` fields before wallet code trusts alias bindings.
Swift, JavaScript/Node, and Python Torii contract/explorer query helpers also
reject padded selector filters before dispatch, including account, authority,
asset-definition/id, owned-by, participant, contract-address, contract-alias,
and asset-id values; JavaScript SNS domain route selectors follow the same
no-trim rule before route construction. Python, Kotlin/JVM, and Android Java
UAID portfolio query
helpers reject padded asset-id, asset, and scope selectors before dispatch, and
Swift account asset scope filters use the same exact selector rule instead of
silently trimming caller input. JavaScript package-dist, Python focused-runner,
Swift focused-runner, and JVM/Android guard coverage mirror the same
contract/UAID selector exactness.
JavaScript/Node, Swift, Python, Kotlin/JVM, and Android Java UAID route
literals also reject whitespace normalization before dispatch: exact raw hex
and uppercase `UAID:` literals are canonicalized, but padded literals or
`uaid:` values with padded hex portions fail before a Torii request is
constructed.
Kotlin/JVM and Android Java offline transfer-list query parameters use the
same fail-closed selector rule for optional `asset_id` filters: exact non-empty
values are preserved, while padded or blank values fail before query
serialization instead of being trimmed into a different selector.
JavaScript/Node, Swift, Kotlin/JVM, Java Android, and Python multisig response
parsers also reject padded, alias-shaped, or otherwise non-canonical returned
`resolved_multisig_account_id` values before proposal or spec state is trusted
by wallet code.
The JavaScript published package also mirrors source recursive-spend native
availability checks: if any ABI-6 helper is missing, including transition
profile, append-boundary, lineage-witness assembly, verify, or redeem helpers,
the package reports recursive spend unavailable, falls back to
`checked_prefold_v1`, and refuses helper dispatch instead of selecting an
incomplete native binding. Package-dist availability tests also fail closed
when the ABI-version probe throws or when any required native helper accepts the
malformed probe archive instead of rejecting it with a Kagemusha
archive/Norito/probe diagnostic. The same package-dist tests reject empty, missing,
text, oversized, malformed Norito, or empty-payload native outputs from every
ABI-6 recursive-spend helper before callers receive archive bytes, and reject
empty, oversized, malformed Norito, or empty-payload request archives for every
ABI-6 helper argument before native dispatch. Valid package-dist helper calls
also return copied `Buffer` outputs and pass owned archive copies to native, so
callers cannot mutate request or response views after dispatch. Package-dist
coverage also checks that native semantic rejections keep their diagnostic
details for over-cap Reserved-lineage hop counts, forged lineage verifier-record
commitments, and forged transition-profile openings, so wallet code can
surface the same fail-closed reason that source wrappers surface.
Swift's native bridge caps Kagemusha native output lengths before
copying native pointers into `Data`, so over-cap native outputs are rejected
and freed at the bridge boundary. The Swift dynamic loader now requires bridge
ABI 6 or later and the
record-backed plus complete recursive-spend Kagemusha symbols, including both
lineage-witness assembly helpers, before probing those symbols with malformed
Norito archives. Swift reports native compact-token, recursive-aggregation, and
recursive-spend Kagemusha provers as available only when the loaded bridge
returns the expected Kagemusha rejection without output bytes, and the Swift
recursive-spend wrapper refuses to select `recursive_spend_v1` unless the full
ABI-6-or-later surface passes that probe.
Native bridge ABI 7 exposes fail-closed reserved symbols for
`connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes`
plus
`connect_norito_kagemusha_verify_recursive_compact_payment_token` for the
`kagemusha-recursive-compact-v1` compact-token circuit. The prover decodes and
validates both input archive shapes, rejects extra one-hop or missing multi-hop
Pallas openings as malformed record-backed preflight input, then returns the
recursive-compact-unavailable error instead of emitting a token for shape-valid
one-hop input. The verifier clears stale output, rejects malformed archives and
malformed token/public-input bindings through the same core preverification
gate, and returns `valid = 0` for shape-valid compact tokens until compact-token
proofs compose the private-hop verifier-slice relation in-circuit. The C
bridge, JNI shim, Node host, and PyO3 host all apply this decode/preverify
boundary before exposing a boolean receiver result. Swift, Kotlin/JVM, Java
Android, JavaScript/Node, Python, and C# expose typed recursive-spend compact
projection verifier facades that accept raw Norito compact-token and
verifier-record archives, reject malformed local inputs before native dispatch,
gate availability on the ABI-7 compact projection verifier symbols, and return
the native boolean receiver result. The JavaScript published package mirrors
that ABI-7 boundary with regressions for owned archive copies, copied projected
token outputs, optional-height dispatch, invalid-height preflight, invalid
local archive rejection before native dispatch, malformed native projection
output rejection, and non-boolean verifier output rejection. Kotlin/JVM and
Java Android expose
`isRecursiveCompactUnavailable(...)` classifiers for the same ABI-7 reservation
strings and map native compact prover reservation failures to
`IllegalStateException`, while empty or malformed local archives remain
`IllegalArgumentException` caller-input failures. Their compact-projection
wrappers also reject malformed native Norito bytes and empty-payload native
projection outputs through `KagemushaCompactPaymentTokenProver.requireNativeOutput(...)`.
Their projection availability is also probed independently from the full
recursive-compact prover/verifier gate, so
`recursiveSpendCompactPaymentTokenFromBundle(...)` can be available whenever
the ABI-7 projection symbol is present.
Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C# compact-token, recursive aggregation,
recursive compact, and recursive spend wrappers also reject oversized caller
archives with explicit `must not exceed` diagnostics before owned byte copies,
Norito parsing, native availability checks, or native dispatch; the Node NAPI
and Python PyO3 hosts enforce the same 64 MiB cap for direct native-host
entrypoints that bypass the high-level SDK wrappers. ABI-7 recursive-spend host
regressions now cover oversized init, append, transition-profile,
lineage-boundary, verify, redeem, and lineage witness archives before Norito
decode, including each multi-archive lineage witness input slot. The same host
boundary, plus the shared C bridge, now preflights nested current-hop Pallas
open-envelope archives inside init/append request archives, so empty Pallas
material is rejected before core prover or nested decode paths run.
The C bridge clears stale output pointers before rejecting null inputs, rejects
null output slots before writing, and does not slice adversarial input lengths
on either ABI-7 entry point. The same bounded archive reader is shared by the
ABI-6 recursive spend, compact-token, recursive aggregation, and recursive
compact C entry points, so empty or oversized Kagemusha archives reject before
raw slices are formed. The recursive compact prover returns no stale archive
output on malformed record/envelope archives or multi-hop archives that still
require append-batch composition, and the verifier returns `valid = 0` for
shape-valid compact tokens whose inner proof body fails the ABI-7 compact proof
payload floor or backend verification.
The data model and core pre-admission helpers now also validate the recursive
mode-2 folded public-input projection against a recursive aggregation proof
bundle. That check binds the folded compact-token chain id, asset, roots, hop
count, nullifier/output digests, fold digest, and aggregation transcript digest
to the canonical recursive evidence while leaving the legacy
`validate_supported_context` path restricted to checked pre-fold mode `1`. Core
also exposes raw-key, verifier-record, and height-aware record
preverification/verification helpers for that projection, so a valid recursive
aggregation proof can be checked against the exact compact-token public
projection before compact-token admission.
Data-model construction of a recursive compact payment token also fails closed
before core admission when the folded projection carries an all-zero recursive
aggregation transcript digest, when recursive proof/verifier backends are
unsupported or mismatched, when supported non-Halo2 recursive backends attempt
to enter the ABI-7 path, when proof bytes are empty, when the recursive circuit
id is not one of the semantic, compact, or Reserved-lineage ids, or when stale
public-input hashes, transcript splices, or hop-count splices are replayed.
The PR policy guard also scans the active non-C# Kagemusha source, docs, SDK,
CLI, localnet integration, release-evidence, staged-runner, Android-lab, and
guard-script surfaces for
unfinished-work markers. The only remaining release handoff markers allowed by
that guard live in the active roadmap's C# Windows host certification notes, so
non-C# Kagemusha follow-up placeholders cannot be introduced without a routed
negative control failing first. Generic runtime and ingress files that carry
the Kagemusha path, including core transaction admission, offline ISI execution,
Torii Offline V2/OpenAPI/ZK-prover ingress, and the JavaScript native host, are
also explicitly scanned and injected by the routed control. The guard inventories
source-like Kagemusha paths under the active workflow, CI, Rust, SDK, script,
integration, and docs roots and fails if any such path is left outside the marker
scan, except for the policy script's own injected-marker controls and the
deferred C# Windows runner. A second content-bearing inventory covers generic
non-C# runtime and SDK files whose filenames do not include Kagemusha but whose
contents do, including Swift, Android/Java, JVM/Kotlin, JavaScript, Python, core,
and Torii surfaces. Those generic files use a Kagemusha-line-scoped marker check
so unrelated ISO currency fixtures or non-Kagemusha implementation notes do not
mask a Kagemusha release handoff.
Recursive aggregation proof public inputs also expose the folded public-input
hash as four public limbs. Internal expected-circuit helpers compare those limbs
to the compact token's chain-visible folded public inputs and enforce the
mode-2 folded context, token public-input hash, canonical verifier-key
CID/hash binding, recursive proof schema, folded-public-input hash limbs,
aggregation-transcript limbs, witness count, hop count, and verifier-record
height windows for projection tests. The exported core recursive compact prover/preverify/verify
helpers remain fail-closed until a composed private-hop verifier-slice compact
proof exists, semantic recursive aggregation proofs are rejected by the public
compact verifier, and `verify_kagemusha_compact_payment_token` continues to
reject mode `2`. Bridge FFI plus native JavaScript and Python host verification
treat non-canonical compact envelope verifier-key hashes as malformed input,
not as a soft invalid unavailable proof result; Kotlin/JVM and Java Android
wrapper tests pin the same classifier boundary for verifier-key hash mismatch
diagnostics.
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
Swift, Kotlin/JVM, Java Android, JavaScript/Node, Python, and C# also expose a
typed archived-instruction transaction surface for on-chain Kagemusha
submission. These helpers accept valid Norito archives for `KagemushaTransfer`
and `RedeemKagemushaRecursive`, preserve their canonical bytes rather than
re-framing them, and reject empty, malformed, tampered, or wrong-type
instruction archives before transaction payload construction. Swift uses
`KagemushaInstructionTransactionRequest` and
`IrohaSDK.buildKagemushaRecursiveRedeem(...)`; Kotlin/JVM and Java Android use
`KagemushaInstructionArchives` helpers that build a single archived instruction
transaction payload; JavaScript/Node exposes
`buildKagemushaInstructionArchiveInstruction(...)`,
`buildKagemushaInstructionTransaction(...)`, and
`buildKagemushaRecursiveRedeemTransaction(...)`; Python exposes
`kagemusha_instruction_archive_instruction(...)`,
`build_kagemusha_instruction_transaction(...)`,
`build_kagemusha_recursive_redeem_transaction(...)`,
`TransactionDraft.kagemusha_instruction_archive(...)`, and
`TransactionDraft.kagemusha_recursive_redeem(...)`; and C# exposes
`TransactionInstruction.KagemushaInstructionArchive(...)`,
`KagemushaInstructionArchiveInstruction`,
`TransactionBuilder.KagemushaInstructionArchive(...)`, and
`TransactionBuilder.KagemushaRecursiveRedeem(...)`. Recursive redeem derivation
inside the transaction helper consumes the native recursive redeem request so
wallet code signs exactly one `RedeemKagemushaRecursive` instruction. The C#
transaction-builder overloads that receive decoded lineage and
amount/change-output metadata run the managed preflight before native request
parsing and leave the builder unmodified when those relationships are invalid.
Python direct helper calls and the optional C# P/Invoke wrapper also require
that complete ABI-6 surface before producing any recursive spend output; C# also
requires the expected Kagemusha empty-archive bridge error during symbol probes.
A partial or permissive native bridge cannot emit an init or append bundle
without the witness helpers needed for later redemption.
The Swift wrapper also exports the same ABI-6 requirement for wallet-side
capability checks.
JavaScript/Node and Python now require an ABI-6-or-later native version probe
before reporting recursive spend as available or selecting `recursive_spend_v1`;
the Node NAPI host exports `connectNoritoBridgeAbiVersion`, while the Python
PyO3 extension exports `kagemusha_recursive_spend_native_bridge_abi_version`.
Kotlin/JVM and Java Android also call the native bridge ABI-version JNI probe and
probe the verify plus both lineage-witness JNI symbols before reporting
recursive spend as available or defaulting to `recursive_spend_v1`. C#
publishes the same ABI-6-or-later requirement and probes verify plus both
lineage-witness P/Invoke symbols before its optional wrapper calls the bridge.
All SDKs expose the same default spend-mode choice:
`recursive_compact_v1` is selected when the ABI-7 compact prover/verifier
surface is available, `recursive_spend_v1` is selected when only the recursive
spend ABI-6-or-later surface is available, and `checked_prefold_v1` remains the
compatibility fallback for older runtimes that only provide the record-backed
compact-token path. The C# selector remains on the ABI-6 fallback policy until
the matching compact-first change is certified on a Windows host.
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
transcript. Recursive compact mode `2` is handled by the ABI-7
`kagemusha-recursive-compact-v1` token prover/verifier for one-hop LEN=4
compact receipts; callers that need spend-again-offline cash should still use
`KagemushaRecursiveSpendBundleV1` and ABI 6 because spend bundles carry lineage
state for additional offline hops. The final aggregation proof must continue to
use no trusted setup and preserve the same public nullifier/commitment/root
semantics.
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
That batch summary is the host-side evidence surface feeding the private-hop
recursive compact proof path. The data model now also has a recursive
aggregation evidence statement that Norito/Poseidon-binds that batch digest and parameter
fingerprint to the same ordered hop transcript and to the canonical
`pallas-ipa-transparent-v1/vesta-recursive-fixed-window-64x4` verifier-witness
profile. It validates mode `2` evidence shape, hop continuity, witness count,
profile, verifier opening length, fixed-window table schedule/base digests, and
non-zero batch fields before reserved compact-token projection checks. The Poseidon2
aggregation transcript digest deliberately accepts both checked pre-fold mode
`1` and recursive compact mode `2`, while rejecting unknown modes, so recursive
evidence and compact verifier witnesses bind the same transcript shape. Focused
data-model coverage also roundtrips the evidence through Norito, validates the
decoded profile-bound digest, rejects decoded unsupported-profile evidence,
rejects unsupported or non-power-of-two opening lengths, rejects zero
schedule/base commitments, and rejects truncated evidence archives.
The recursive evidence validator also has explicit adversarial coverage for
empty transcripts, over-cap hop lists, duplicate input nullifiers, and duplicate
output commitments.
Core exposes record-backed evidence builders for that recursive path. They first
enforce the same active WSV-style confidential-transfer-v2 hop verifier records
used by record-backed compact-token proving, verify every private hop proof, and
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
uses the 64-by-4 fixed-window Vesta verifier witness profile that covers the
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
public inputs and bundles whose 59 public instance columns bind a transparent
no-trusted-setup proof payload to the recursive evidence digest, folded public
input hash, aggregation transcript digest, verifier-parameter fingerprint,
fixed-window schedule digest, shared-table manifest digest, table-base digest,
native witness-batch digest, recursive spend proof-chain digest,
non-circular transition-profile binding digest, Reserved-lineage append opening
preflight digest, compact Reserved-lineage append-boundary digest, reserved
recursive verifier scalar-projection digest, verifier opening length, witness
count, and hop count while rejecting backend,
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
until a composed verifier-slice proof is used. Direct proof-artifact hashing
uses these same public-input gates, so a stale semantic proof, a plain
recursive aggregation proof, or a digest-only Reserved-lineage append-opening
proof cannot be hashed into the previous-proof chain.
This evidence binding is consumed by the ABI-7 recursive compact prover rather
than rederived by SDKs from each compact-hop Halo2 proof envelope.
Core preverification for that proof-carrying bundle now checks the transparent
Halo2 IPA `OpenVerifyEnvelope`, canonical circuit id, verifier-key hash,
public-input schema, empty auxiliary metadata, exactly 59 one-row Pasta public
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
keeps the ZK1 public-instance parser bounded while allowing the 59-column
recursive aggregation envelope through the native bridge and backend verifier.
It also ships the transparent Halo2 IPA semantic proof/prover/verifier path for
the recursive aggregation evidence layout. The semantic circuit constrains the
opening-length corridor, binds the fixed-window schedule and shared-table
manifest digest limbs to the selected opening width, constrains the hop-count
corridor and witness-count equality, and rejects nine non-zero digest groups
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
This proof path remains admission-neutral for compact-token receivers. The
recursive compact-token helper verifies reserved projection shape for internal
tests, while the public ABI-7 compact prover/verifier opens packaged one-hop and
append proof paths only when compact proving-key and verifier-slice evidence are
present; missing packaged keys fail closed and default-selection cases remain
reserved.
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
cheap while the full recursive witness layout is stress-tested separately;
constructing every per-term fixed-window table for `n = 128` is not
production-viable for normal unit tests, so those exhaustive synthesis checks
remain opt-in heavyweight coverage rather than a prerequisite for ABI-7 compact
token verification.
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
heavyweight and ignored by default. The routine offline-offline production path
uses the ABI-6 reserved-lineage recursive spend verifier and redemption surface,
while ABI-7 recursive compact-token symbols have package-aware one-hop and
append proof wiring when packaged compact proving-key archives and
verifier-slice open-envelope evidence are present; malformed or absent packaged
keys fail closed and compact-first Rust/SDK selector surfaces remain pinned by
the readiness guard without bypassing packaged-key evidence.
The ignored MockProver
cases remain deep synthesis stress coverage for future verifier-layout changes.
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
preflight binding, recursive aggregation evidence/proof-public input binding,
folded public-input projection preverification, folded-public-input hash limbs
in the recursive proof schema, height-aware record checks, full backend
verification wrappers, and a transparent Halo2 IPA semantic proof for that
evidence are present. ABI 7 keeps recursive compact entry points and mode `2`
source-stable with package-aware one-hop and append proof wiring; production
compact-token capability advertisement stays fail-closed until the compact key
package, evidence JSON, generator log, and signed release/device evidence are
all present. The legacy checked-folded entry points remain mode `1` only.
