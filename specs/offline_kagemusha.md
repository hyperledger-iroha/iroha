# Kagemusha offline cash

Kagemusha is the single offline-cash protocol in the first release. It supports
exact decimal amounts, sender change, offline multihop
spending, and full or partial online redemption. There is no runtime product
mode or alternative offline API. ABI 21 and the V4 chain, recursive, and
artifact carriers are the sole lifecycle surface. V2 names remain only for the
unchanged amount, note-opening, authorization, membership, and finality leaf
types that V4 embeds directly. The manifest and native capability schemas have
no `mode` field; schema/version, backend, transcript, and circuit identities
pin the exact cryptographic contract.

## Universal capability contract

Offline cash is a wallet and device protocol, not a validator deployment mode.
Every Iroha deployment exposes the ABI-21/V4 and `cash_handoff_v1` surfaces
needed to build an offline user experience. Operators do not enable that
capability per node, asset, domain, dataspace, or routing container. In particular,
there is no `settlement.offline.enabled` switch, `offline.enabled` asset
metadata, configured escrow catalog, or offline-specific startup/readiness
gate.

Applications decide whether to expose offline load, pay, receive, receipt, and
redemption screens. Two applications may use different dataspaces and expose
different user interfaces while talking to validators with the same universal
capability. `/health` and `/readyz` report ordinary process and node service
readiness; wallet/device state and the absence of an asset-specific proof
release cannot make a validator unhealthy. A command that references missing,
inactive, malformed, or unauthorized proof material is rejected as that
command's validation result rather than changing node admission.

## Amounts and assets

Every request binds the exact genesis-derived network id, asset definition,
authoritative asset scale, and an unsigned `u128` atomic-unit amount. The scale
is read from the live asset definition. Decimal conversion is exact: excess
precision, negative values, zero payments, and overflow are rejected. Top-up
debit, note conservation, and redemption credit use the same scaled `Quantity`
value.

A spend consumes one or two canonically ordered parent notes and creates one
recipient output plus optional sender change. The transition proves, and every
verifier rechecks:

```text
sum(inputs) = recipient + change
```

The same fixed Eq/Ep circuit and key material accepts initialization, one
parent, and two parents. Two-parent append is the only merge form: it binds the
ordered parent states and proves conservation inside the recursive statement;
host-side hashing or equality checks cannot manufacture a merge.

Every non-zero output is an independently spendable branch. Commitments,
nullifiers, input branches, and output branches must be distinct. Replay,
ancestor/descendant reuse, overlapping siblings, duplicate nullifiers, and
duplicate commitments fail closed.

## Direct Torii API

The lifecycle uses exactly four Torii routes:

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/v1/offline/readiness` | Discover the universal ABI-21/V4 offline protocol capability |
| `POST` | `/v1/offline/top-up` | Submit `OfflineTopUpRequest` |
| `POST` | `/v1/offline/redeem` | Submit `OfflineRedeemRequest` |
| `GET` | `/v1/offline/operations/{operation_id}` | Observe durable operation state and finality |

Top-up and redemption accept only the canonical typed value with
`Content-Type: application/x-norito`. They do not accept JSON request bodies or
an encoded-byte wrapper. The lowercase 64-hex `Idempotency-Key` is the signed
operation id. An identical retry returns the same operation; reuse with any
different request conflicts. A client retains its local pending operation until
Torii reports final chain finality.

Canonical request and native-bridge decoding rejects compression and alternate
Norito layouts from the fixed header before reconstruction. Each route applies
its exact framed-body ceiling. Public request extractors start with a fourfold
frame-derived allocation base. Lineage selectors add 64 KiB; top-up adds six
maximum 192 KiB shield proofs plus 64 KiB; and redeem budgets its bounded
384 KiB recursive proof pairs plus three fixed copies of the 192 KiB unshield
proof. Before owned reconstruction, a schema-aware canonical wire preflight
walks the redemption field path without allocating and rejects an unshield
proof whose encoded `Vec<u8>` count exceeds that limit. Native fixed-depth
schemas use the same fourfold base with a depth-derived fixed allowance; a
redemption build result has its own larger profile, including six fixed
unshield-proof copies, because it contains both the change bundle and its
duplicated change result. Finality, lineage, operation-status, and release
wrappers contain nested collections whose cardinality is checked after
reconstruction, so their explicit schema profiles retain Norito's conservative
32-fold frame-scaled ceiling plus 64 KiB. Both public and native paths retain
schema-specific nesting limits. A compressed length field, forged collection
count, oversized unshield proof, or structurally decodable non-canonical
representation therefore fails within its body and allocation ceilings.

Readiness and operation responses support Torii's typed response negotiation.
The readiness response advertises the protocol-level ABI and handoff
capability on every deployment; it is not an asset enrollment list and does
not gate `/health`, `/readyz`, Torii startup, consensus, or block production.
Asset scale, verifier windows, release material, authorization, and balances
are validated against the exact top-up or redemption command when that command
is submitted. The recursive pair uses registry backend `halo2/ipa` and exact roles
`kagemusha_recursive_step_eq_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-eq-compact-layout-v5` and
`kagemusha_recursive_step_ep_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-ep-compact-lineage-v5`.

## Online to offline

The wallet first obtains the authoritative confidential-tree root, leaf index,
active top-up-shield verifier record, and committed block context. It builds the
zero-input shield proof, signs the complete top-up request with its registered
device authority, and submits it to Torii. Core atomically:

1. validates authorization, operation replay state, exact network, scale, and policy;
2. recomputes the authoritative root and leaf index;
3. rejects the new note commitment and spend nullifier if either overlaps an
   existing commitment or spent-nullifier namespace;
4. verifies the top-up-shield public inputs and proof;
5. debits the exact public amount into escrow;
6. appends the initial note commitment; and
7. persists the finalized top-up anchor, its zeroed cumulative drawdown
   balance, and the operation receipt.

The drawdown balance is an ordinary consensus-witnessed state leaf keyed by
the top-up operation id. An anchor and its drawdown leaf are created together;
an existing half-pair, a missing leaf, a malformed value, or a value above the
anchor amount fails closed.

After finality the wallet creates the initial recursive bundle
with the ABI-21 SDK's `initSpendV4`. The note is not available for offline use until both the chain
operation and local encrypted-state transition are durable.

## Offline transfer

The receiver creates a nonce-bound payment request containing its output
commitment, exact amount, asset, scale, verifier generation, and expiry. The
sender authenticates the request, creates recipient and optional change
outputs, proves and locally verifies the transition, then performs the
`cash_handoff_v1` boundary atomically: it irreversibly consumes the selected
parents and durably binds/signs the exact outgoing payment before exposing any
payment byte to a receiver-capable transport.

The peer payload contains the recipient's opaque proof bundle and the exact
proof-bound, secret-free membership witness required for its next spend. Replay
identity remains derived only from the recipient bundle's authenticated split
transition. The payload never carries a spend key, sender change, or local key
reference.

The receiver runs `verifySpendV4` and checks the signed request, exact network,
asset, scale, exact amount, recipient commitment, hop limit, verifier activation
window, finalized top-up origin, recursive proof validity, and branch
disjointness. It atomically persists the received note before signing a durable
acknowledgement receipt. The receipt is evidence only, not acceptance or a
sender commit gate. Missing, invalid, or lost acknowledgements never unspend,
roll back, replace, or claw back the exact outgoing payment. Duplicate delivery
and exact retransmission remain idempotent. If no receiver ever obtains the
committed bytes, the sender bears cash-loss risk exactly as with physical cash.

No network or artifact fetch is permitted during send, receive, proof creation,
or peer verification. QR and NFC carry the same canonical request, payment, and
acknowledgement archives.

## Offline to online

Redemption uses the current unshield-v3 evidence API. Full redemption binds a
zero private output. Partial redemption binds exactly one non-zero Kagemusha
change output and proves exact conservation between the redeemed public amount
and the offline change branch.

Core validates the finalized top-up provenance, current recursive proof,
active recursive StepEq, recursive StepEp, and unshield verifier records,
nullifier freshness, exact scale, unshield public inputs, and optional change
branch before mutating balances. It also allocates the exact public redemption
amount across the canonically ordered top-up anchors and atomically advances
each cumulative drawdown balance. No anchor can fund redemptions above its
finalized amount, even if a later circuit or public-binding regression were to
weaken the cryptographic conservation check. Partial redemption debits only the
public amount, leaving the proven private change backed by the remaining
drawdown capacity.

Core then consumes the branch nullifier, credits the exact public `Quantity`,
appends the change commitment when present, commits the drawdown updates, and
persists an idempotent receipt as one transaction. A wallet keeps the source
note and pending request until finality; retries reuse the same operation id and
bytes.

## Wallet state and artifacts

Wallet state V9 is encrypted and stores a set of notes rather than one aggregate
token. Each note records its opaque bundle, exact atomic amount and scale,
top-up provenance, verifier references, artifact generation, hop count,
operation stage, per-note opening material, and a reference to the wallet-level
hardware-backed spend key. The displayed balance is derived from available
notes. Pending, reserved, spent, quarantined, and redeeming notes are not
silently reclassified.

The authenticated V4 manifest binds source commit, exact genesis-derived
`NetworkId`, asset, scale,
activation and withdrawal heights, exact bridge ABI 22, proof size, transcript,
backend, and benchmark evidence. It contains exactly two Pasta-cycle profiles
in Eq-then-Ep order. Each profile carries exactly four external artifacts:
`ParamsIPA`, processed proving key, processed verifying key, and the final-key
selector-zero bootstrap witness. The external inventory is exactly eight
files. The bounded `KagemushaStepCircuitParamsV4` value is authenticated inline
in each profile and digest-bound into every artifact header; it is not a
separate streamed file. The content-addressed top-up-finality roster remains
release metadata outside that eight-role cryptographic inventory. Every file
has an exact framed and payload size and SHA-256. Installation streams to
private files, requires supplied manifest bytes to equal the canonical encoding
of the validated manifest object, verifies every binding plus the candidate-bound
`promotion-record-v4.norito`, and atomically activates the complete generation.
Bootstrap payload version 5 authenticates the final key-generation circuit's
canonical per-phase virtual-region breakpoints. Runtime rejects malformed or
shape-incompatible breakpoints and builds only a witness-generation circuit
from them, so it does not retain the key-generation constraint graph beside a
processed proving key.
Generation obtains the two inline-profile inputs from
`kagami kagemusha prepare-release-circuit-params-v4`. That command uses the
central reviewed first-release constructor and atomically publishes a closed,
owner-private directory containing canonical Eq and Ep Norito files; hand-built
release profiles are not an operator interface. Eq and Ep share the exact
parameter bytes while retaining distinct circuit identities, proving keys, and
verifying keys.
Candidate verifier- and proving-key generation extracts or validates those
breakpoints after synthesis and drops the populated circuit before key
assembly. The reciprocal point audit no longer allocates the generic
variable-base MSM in the Base graph. It canonicalizes and combines source
coefficients there, then copy-binds the points and normalized GLV segments into
three source-major dense lanes. The authentic 1,867-source StepEq batch is
split in stable order as 623/622/622 sources, so the longest lane needs 104,667
rows instead of the impossible 313,659-row serial trace. Each lane has 37
advice columns and one fixed enable column; its bus and accumulator coordinates
are equality enabled. One globally selected non-identity offset is carried
across lane boundaries, and endpoint-to-start copy constraints close the last
lane back to the first. The ring closes exactly when the original unsplit MSM
is the identity. Fixed start tags, canonical decompositions, and affine
exceptional-case checks remain enforced. The five authenticated Table16 SHA
lanes have a 65,527-row table floor. The complete k17 circuit has 131,063
usable rows after its nine mandatory unusable rows.

The release-authenticated compiled-protocol structure digest remains the exact
value-free V1 SHA-256 descriptor. The qualified compiled-protocol identity is
V2: its domain, parity, V1 structure digest, point count, every canonical
32-byte compressed verifier-key point, and transcript initial state are
absorbed by Poseidon. Each compressed point is represented injectively as its
two little-endian `u128` halves, preserving the complete compressed encoding
instead of reducing a coordinate modulo the opposite Pasta field. The
Poseidon field element is then encoded canonically inside the short V2
domain/version SHA-256 wrapper, whose 53-byte message uses one compression
block. The deferred-equation audit uses the same construction under its
independent V6 domains: every source point contributes the same injective
two-`u128` encoding to Poseidon before the one-block SHA-256 wrapper. Identity
points are rejected, and native, scalar-circuit, and reciprocal-circuit paths
must derive identical commitments.

The authentic final-VK attempt exposed why the earlier placeholder-profile
probe was insufficient: a 20,154-byte raw protocol-identity SHA preimage
required 316 compression blocks and drove the five-lane geometry to 147,520
rows, beyond the 131,063-row k17 capacity, only after roughly 29 minutes of
setup. Circuit construction now computes the exact queued SHA and dense-MSM
row profiles and rejects either over-capacity auxiliary machine before key
generation. A fresh guarded k17 probe of this final compact source, authentic
candidate generation, release finalization, and live Taira rollout are still
pending.

The authenticated complete-circuit envelope is degree 17 with `[220]` advice
columns, `[25, 0, 0]` lookup-advice columns, one parameter fixed column, and
one instance column. The two trailing zero lookup phases are the exact
`BaseCircuitBuilder` shape; they do not allocate speculative advice phases.
Across the complete configured graph this yields 411 advice columns, nine
base fixed columns, 330 selectors, and 297 equality/permutation columns.
Processed-key serialization disables selector compression, so each parity
authenticates 339 fixed polynomials and 297 permutation polynomials, for 636
commitments. The exact unframed lengths are 8,388,676 bytes for `ParamsIPA`,
20,362 bytes for the processed verifier key, and 5,347,763,078 bytes for the
processed proving key. The proving key remains 20,946,042 bytes below the
fixed 5 GiB artifact corridor.

The single public-instance column contains 66 field elements. Its common
semantic header occupies `[0, 19)`, the 38-element IPA accumulator occupies
`[19, 57)`, the Eq and Ep deferred-audit words occupy `[57, 61)` and
`[61, 65)`, and the live selector is element 65. This layout is shared by the
Eq/Vesta and Ep/Pallas roles and is authenticated by their distinct schema
digests.

Proving-key assembly reuses the supplied verifier-key domain and stages compact
permutation scratch instead of retaining a domain-by-column factor grid or
parallel coefficient-clone fan-out. Verifier-key construction builds, commits,
and drops one permutation polynomial at a time. Proof generation transfers
ownership of the live circuit and one parsed processed key into the prover. The
circuit is released after witness synthesis; domain-sized fixed-value and
permutation Lagrange preprocessing is released after its last commitment; and
the consumed key yields only its embedded VK for immediate proof verification
before that VK is dropped. The quotient evaluator transforms one
copy-permutation sigma chunk at a time. Evaluation domains initialize only the
base FFT table eagerly, quotient parts are written directly into the final
interleaved polynomial, and cached recursive FFT scratch is evicted before
h-piece commitments. The outer lifecycle remains in a disposable one-worker
Rayon pool; large MSMs alone use the admitted fixed two-worker window.
Accumulator order is unchanged.

The checked phase-aware admission estimate for this complete shape is
53,108,563,136 bytes (49.4612 GiB), 7,020,979,008 bytes below the 56 GiB
reviewed ceiling; it is not a physical-memory prediction. The model includes
the authenticated upper-width virtual graph, physical-cell map, V1 assignment,
processed proving key, and allocator reserve that overlap during proving. The
superseded precompact diagnostic reached an externally guarded peak of
4,998,922,240 bytes, but that measurement does not validate the final V2/V6
commitment graph. A fresh guarded final-source k17 probe must establish its
physical peak and shape. A 93,120-byte transcript
per role, a 186,852-byte initialization pair, and a 191,862-byte maximum pair
remain expected values until authentic generation confirms them. Candidate
promotion must bind the exact generated bounds rather than the larger
defensive wire ceilings.

The newer non-shipping serialized-advice V7 lab is a separate candidate and
does not reinterpret those V4 projections. Its source-pinned k17 graph uses a
70-cell public column and currently expects 8,388,676-byte parameters,
20,394-byte verifier keys, 5,356,151,726-byte proving keys, and 93,184-byte
proofs per role (186,368 raw proof bytes per pair). Canonical carriers use a
compact Norito schema: live carriers retain both public columns, both proofs,
and both post-proof folds, while the authenticated reader derives and decides
the output lineages instead of serializing them twice. The all-zero NullParent
derives its fixed instance columns and succinct-verifier outputs but retains
four separately generated, non-equal post-proof/branch-fold transcripts. Cheap
layout tests pin 188,844-byte base, 191,532-byte recursive, and 191,828-byte
NullParent carriers, each within the unchanged 191,862-byte release maximum.
The complete four-node topology accounts for 30 terminal IPA decisions,
including the compact NullParent carrier's four separately generated, non-equal
post-proof and branch-merge fold transcripts. The V7 reviewed admission
estimate is 53,126,388,928 bytes. These are still
source-derived expectations, not production evidence: the guarded genuine
four-node proof remains ignored and the V7 release-review gate remains false
until the measured artifacts and independent review complete.

The reciprocal scalar loader removes only the identity tail of one assigned
product with coefficient one and constant zero: its direct `FpChip::mul`
result has the same three-limb `ProperCrtUint` residue invariant as the former
product-by-one plus zero path. All other sum/product shapes retain their old
operation sequence. This optimization intentionally changes circuit placement,
so prior Eq/Ep breakpoints, VK/PK payloads, compiled-protocol identities,
bootstrap witnesses, proofs, digests, and source-seal evidence are invalid and
must be regenerated before the final guarded qualification.

The userspace supervisor enforces the lower of
64 GiB or half of installed physical memory and cannot be raised by an
operator option. On macOS, candidate generation and its diagnostic benchmark
enumerate only the owned process group through `libproc`, pin its leader's
start identity and parent, and enforce the greater of summed RSS or physical-
footprint high water every 250 ms. A threshold crossing first stops allocation,
takes one final scoped sample, and then kills and reaps only the owned group.
The direct child's kernel `wait4` peak-RSS value remains an independent final
gate. The guarded output parent
must also retain at least 16 GiB of free disk before generation so both raw
proving-key spools and the framed copy cannot exhaust the filesystem.
Production candidate and physical-device peak-memory evidence remain required
before promotion.
Semantic construction loads one authenticated role at a time and drops each raw
carrier after parsing; it never assembles the six-role verifier or eight-role
prover payload inventory in memory. Runtime verifiers are transient rather
than cached per generation. Parent verification is dropped before a prover is
opened; after proving, terminal verification shares that prover's
Params/circuit context and reparses only the two small raw VKs after parsed
proving keys have been dropped. Memory-intensive install, top-up shielding,
recursive proof, and
verification entrypoints share one nonblocking process-wide permit, and a
contending ABI caller receives
`CONNECT_NORITO_ERR_KAGEMUSHA_BUSY` (`-318`) before its large input is copied.
Swift exposes that status as the retryable `proofWorkerBusy` error. If an
otherwise complete install encounters the busy permit, the coordinator retains
the authenticated spools and retries the identical candidate without streaming
the large artifact stream again; cancellation or a different candidate closes
that pending install. JVM artifact ingestion also limits every native write
chunk to 1 MiB; the
Kotlin and mirrored Java SDKs enforce that ceiling before cloning the caller's
array.
The authenticated `ParamsIPA` payload is not accepted as a signer-selected
generator set. Runtime first checks the fixed degree and exact encoded length,
derives `ParamsIPA::new(k)` from Halo2's transparent public derivation, and
requires its canonical serialized SHA-256 to match the authenticated payload.
Before Halo2 parses a verifier or proving key, an allocation-free structural
pass checks its exact degree, commitment counts, polynomial-vector counts,
polynomial lengths, and total encoding length against the authenticated circuit
shape. Serialized inner counts are never trusted as allocation sizes, and
vendored processed-key reader panics are converted into ordinary rejection.
Release verification and finalization likewise authenticate one framed role at
a time and drop its payload before opening the next. They do not reconstruct an
eight-role raw prover container merely to check carrier bindings.
Cache qualification performs the same exact shape-derived role-size preflight
before Halo2 parsing. Its decoded-memory budget includes retained IPA vectors,
verifier-key FFT domains, transient release files, and allocator headroom.
Runtime validates the shipped selector-zero proof with the final Step VK and
does not regenerate a bootstrap VK. A supplied release that cannot fit the
configured budget is rejected before verifier parsing; this does not prevent
the node from starting without that cache.
Swift, Kotlin, and Java release-authentication inputs therefore require that
promotion record alongside the trusted policy, attestation, benchmark evidence,
and review. A partial, unpromoted, or role-substituted generation never becomes
active.

## Validator provisioning and activation

No validator provisioning step enables offline support. Offline routes and
ABI-21/V4 command types are present even when no Kagemusha release cache is
installed and no asset has yet been used by an offline wallet. Escrow account
identity is derived deterministically when an online top-up or redemption
command executes; it is not selected through node configuration and does not
constitute an asset catalog.

An operator may install a local authenticated Kagemusha release cache to make
specific proof material available without fetching it during transaction
execution. The optional paired `kagemusha_release_policy_path` and
`kagemusha_artifact_dir`, optional qualification seal, and bounded decoded
working-set limit are cache qualification controls only. If supplied, malformed
or corrupt material is rejected and cannot be used for a command. If omitted,
Iroha still starts, participates in consensus, serves every offline route, and
reports ordinary node readiness. A transaction requiring unavailable release
material fails transaction validation with a precise error.

Promotion-scoped validator sealing is a separate, explicit corridor. Its
`kagemusha_promotion_controller_public_key`,
`kagemusha_catalog_revalidation_authority_key_id`,
`kagemusha_catalog_revalidation_authority_public_key`,
`kagemusha_promotion_reservation_path`, and
`kagemusha_validator_qualification_seal_path` settings must be configured
together and require `kagemusha_catalog_qualification_seal_path` to be
configured. That catalog-seal file must exist before the validator-seal command
runs. The controller key must be Ed25519. The reservation path names the
controller-signed canonical promotion reservation under root custody. The
catalog-revalidation key must be a distinct Ed25519 authority, and its key id
must match `[A-Za-z0-9][A-Za-z0-9._-]{0,127}`. The validator-seal path names a
distinct, absent root-owned output. None of these settings enables offline
support or authorizes ordinary daemon startup to publish a seal.

Qualification authenticates every candidate subdirectory, validates framed
and payload sizes and SHA-256 values, parses the six validator-side artifacts
(ParamsIPA, verifying key, and bootstrap witness for Eq and Ep), and builds an
immutable cache keyed by manifest digest. Loading fails before large artifact
allocation when the decoded estimate exceeds the configured budget. A supplied
qualification seal must be immutable, well formed, and bound to both the source
directories and running executable. For each Eq/Ep profile the seal binds the
manifest's value-free V1 compiled-protocol structure digest separately from
the qualified V2 full protocol identity derived from the final verifying key;
the two values must be non-zero and distinct. The per-release seal also repeats
the authenticated source-seal projection SHA-256 and the reviewed Cargo and
rustc binary SHA-256 values from the manifest. Any zero or one-bit-substituted
value invalidates the seal against the authenticated release.

Create the catalog seal as the first no-bind validation command. In the
production validator corridor, use the exact same integrity-bound flattened
configuration for both stages:

```text
sudo iroha3d --check-config \
  --config /absolute/path/config.toml \
  --config-blake3 '<64-hex-digest-of-exact-flattened-config>' \
  --genesis-manifest-json /absolute/path/genesis.json \
  --write-kagemusha-catalog-qualification-seal /absolute/root-owned/seals/catalog.norito
```

The command requires a locally available valid genesis, performs complete
catalog authentication, reuses that authenticated catalog for disposable
genesis execution, and publishes only after both succeed. The CLI path must
exactly match `kagemusha_catalog_qualification_seal_path`. Its pre-existing
parent chain must be root-owned and not group- or world-writable; the final
destination must be absent. On macOS, every trusted parent and source path must
also have no extended ACL: POSIX owner/mode bits alone do not exclude an ACL
write grant. Publication strips any inherited ACL from the pinned staging inode,
verifies the staged and final names remain bound to that ACL-free inode, writes
a root-owned mode `0444` file by an exclusive same-directory rename, and syncs
both the file and directory.
Seals are never replaced: use a new release-specific filename and update the
configuration for every qualified release.

Only after that configured catalog seal exists, create one validator seal with
a second process invocation:

```text
sudo iroha3d --check-config \
  --config /absolute/path/config.toml \
  --config-blake3 '<64-hex-digest-of-exact-flattened-config>' \
  --genesis-manifest-json /absolute/path/genesis.json \
  --write-kagemusha-validator-qualification-seal /absolute/root-owned/validator-seals/validator-1.norito
```

The second command reads the configured signed promotion reservation and
verifies it against the pinned controller key. It also reads the already
published catalog seal as an exact, bounded, root-owned mode-`0444` input,
performs a fresh same-load catalog qualification, and requires the canonical
seal produced by that load to match the existing catalog-seal bytes exactly.
The promotion-scoped catalog-revalidation receipt must name the configured
authority key id, bind the SHA-256 of that authority's canonical Ed25519 SPKI
DER, and carry a valid Ed25519 signature over the Python-compatible compact,
sorted JSON object after removing exactly `signature` and
`signature_payload_sha256`. Root custody of the controller reservation does not
replace this independent signature check. The same captured catalog is then
checked against the reservation and used to build the validator seal. Signing
binds the exact flattened TOML bytes retained by the integrity-bound config
reader and the exact signed-genesis bytes retained by the ordinary genesis
decoder; digests or reconstructed substitutes are not accepted. Publication
occurs only after complete configuration, catalog, reservation,
catalog-revalidation authority, signed-genesis, and disposable-genesis
validation succeeds.

The two write flags are mutually exclusive. A validator-seal invocation never
creates or replaces the catalog seal, so there is no simultaneous two-file
publication boundary. Its requested output must exactly equal
`kagemusha_validator_qualification_seal_path`, be absent, and use a directory
separate from the release policy, artifact tree, catalog seal, promotion
reservation, and executable. The reader and publisher reject non-canonical or
symlinked paths, untrusted or writable ancestry, ACLs or extended attributes,
and changing inodes. The validator seal is published as a root-owned
mode-`0444`, single-link file by a no-replace same-directory rename and is never
automatically retried after a commit-uncertain result.

Keep the seal directory separate from the release-policy parent, artifact tree,
and executable path. Publishing a directory entry changes that directory's
identity, so placing the seal in a qualified source path would invalidate the
snapshot it is meant to attest. For Taira, deployment configuration owns the
exact `kagemusha_catalog_qualification_seal_path`; there is no reset-packaging
injector or checked-in deployment-specific destination.
Qualification also requires the configured policy, complete artifact tree, and
running executable path chains to be root-owned and not group- or
world-writable. They may be read by the non-root validator after qualification,
but the validator identity must not be able to replace the sealed source
inodes.

After parsing, raw ParamsIPA and bootstrap payloads are released; the catalog
retains the parsed verifier and only the serialized verifying keys needed to
build governed activation records. Consensus never reads the filesystem.
The validator execution-policy identity commits to the configured policy and
the sorted identities of every authenticated catalog release. Peers with
different release inventories therefore cannot advertise the same execution
policy and then disagree when an activation or spend resolves local material.
Wallets and provers install all eight artifacts, including both proving keys.
Generated `dist/kagemusha/v4/*`, raw parameters, keys, device logs, and signing
inputs remain untracked runtime material.

Taira provisioning has a strict two-boundary sequence. First,
`kagami kagemusha prepare-taira-testnet-base-genesis-v4` may add only
network-independent accounts, permissions, base verifier records, asset
registration, aliasing, and fee liquidity to the unsigned genesis. Sign that
base genesis and publish its `genesis.expected_hash`; this hash is the sole
`NetworkId`. Only then may operators build the finality roster and recursive
release for that exact `NetworkId`, qualify it, and use
`prepare-activation-v4` to prepare the governed height-two activation. The
command takes the domain-separated digest of the complete unanimous
four-validator runtime projection; this is not a caller-selected release tag.
base-genesis check replays every mint, burn, and transfer of the backing asset
in instruction order and requires a nonzero final balance. An unsupported
instruction that could conceal a balance change fails closed; historical mint
activity alone is not backing liquidity. The
preparation command requires an explicit nonzero policy-evaluation Unix time,
uses Core's complete consensus validator for trusted X.509 CA structure,
critical extensions, certificate validity and production app policy, and binds
that evaluation time into its report. Certificate inputs are bounded to 16 KiB
each and four Apple or eight Android chain entries. Before semantic X.509
parsing, Core validates the original bytes as one complete canonical DER
certificate: an explicit v3 `TBSCertificate`, positive canonical serial of at
most 20 bytes, raw-identical inner and outer signature
`AlgorithmIdentifier`, and a signature bit string with zero unused bits. RSA
PKCS#1 accepts only SHA-256/384/512 with absent or NULL parameters;
ECDSA-SHA-256/384 and Ed25519 require absent parameters. RSA-PSS requires
explicit `[0]` hash, `[1]` MGF1, and `[2]` salt fields, the same
SHA-256/384/512 choice for the signature and MGF1, a digest-sized salt, and no
trailer override. Non-canonical lengths, nested remainder fields, retagged
OIDs/NULLs, parameter drift, and MD2/MD5/SHA-1 fail before signature
verification. Duplicate extensions, unprocessed name/policy constraints,
unsupported critical extensions, and leaf CA capabilities also fail closed.
Validity fields use only RFC 5280 whole-second Zulu encodings: `UTCTime`
through 2049 and `GeneralizedTime` from 2050, with no offsets or fractions.
Certificate validity is compared against the consensus millisecond clock
without flooring it to ASN.1 whole seconds: an exact `notAfter` instant is
valid, and the following millisecond is expired.

Static policy revocation uses `revoked_certificate_tbs_sha256`: SHA-256 of the
exact raw DER slice presented to the signature verifier as `TBSCertificate`.
It deliberately does not hash the complete certificate, because the outer
signature encoding is not the certificate identity and can change without
changing the signed subject, issuer, key, extensions, or validity. Submitted
chain entries and external anchors are checked by that TBS identity.
Trust-root rollover tries exact DER pins before same-subject issuer candidates.

Android scans rootward for the KeyMint attestation extension and requires the
root-nearest occurrence to be the assertion leaf, so a valid attested CA cannot
bless an attacker-added leaf. Release activation requires exactly the embedded
current Apple App Attest root and both embedded current Google Android
attestation roots, and all three must be governance-active at the release
evaluation time. The root-nearest non-anchor must classify
unambiguously as a factory chain or the exact Google RKP identity; unknown and
mixed identities fail closed. The on-chain KeyMint profile deliberately accepts
hardware TEE(1) or StrongBox(2); the StrongBox-only physical release-evidence
slot is a separate qualification requirement. Either level must carry exactly
one hardware-enforced `rootOfTrust` with a nonempty verified-boot key,
`deviceLocked=true`, `verifiedBootState=Verified`, and a 32-byte verified-boot
hash. Authorization-list tags are unique even when Core does not otherwise use
them. The attestation application id must contain exactly the registered
package and exactly its registered signing digest; extra shared-UID packages or
signers fail closed. Every non-target certificate must have reached
its `notBefore` by admission. Factory-chain non-target expiry may be ignored
only when the chain uses the legacy Google hardware-attestation root; other
factory chains must be unexpired at admission. Every RKP non-target certificate
must be valid both at admission and through the final admitted millisecond
before the exclusive registration expiry. Governed root activation bounds
independently cover the whole registration lifetime.

An Android-enabled policy also requires
`OfflineAndroidAttestationStatusSnapshotV1`. It binds the SHA-256 of the exact
Google response bytes, HTTP `Date`, optional `Last-Modified`, bounded
`Cache-Control: max-age`, and a sorted unique list of lowercase, unpadded
certificate serials whose status is `REVOKED` or `SUSPENDED`. The exact
response is limited to 256 KiB, 4,096 serials of at most 40 hexadecimal
characters, and a one-second to 24-hour cache lifetime. Every submitted
certificate and any external anchor is checked against that list. Freshness is
the half-open interval
`[response_date_ms, response_date_ms + max_age)` and must cover the complete
device-registration and validator-qualification validity windows, not only the
activation block. Device-registration expiry is already exclusive. Validator
qualification expiry is inclusive at the signing clock, so its coverage check
uses the following millisecond as a checked exclusive endpoint; equality with
the first stale millisecond is rejected. Once installed, the snapshot cannot be
removed.
Replacing it permits an identical replay, but otherwise requires a strictly
later `Date`; `Last-Modified` cannot disappear or decrease, and an equal
`Last-Modified` cannot accompany changed payload or serial state. This makes
the signed policy transition fail closed on stale or rolled-back upstream
state while permitting a newer authenticated status list to remove recovered
serials.

Capture the fixed upstream response before assembling the governed policy:

```sh
python3 -I scripts/capture_android_attestation_status.py \
  --output-directory /absolute/owner-private/new-android-status-capture
```

The helper connects only to
`https://android.googleapis.com/attestation/status`, forbids redirects and
content transformation, bounds the body and serial inventory, and checks
`Date`, `Age`, `Cache-Control`, `Expires`, and optional `Last-Modified` as one
fresh cache contract. It publishes create-new mode-`0600` `status.json`,
`snapshot.json`, and `capture-receipt.json` files beneath a mode-`0700`
owner-controlled directory. The Android evidence validator re-derives the
snapshot and receipt from the exact pinned status bytes and headers and rejects
it once stale. TLS retrieval and the unsigned receipt do not grant consensus
authority: the promotion controller/governance signature binds the reviewed
snapshot into the policy. Consensus validates only that deterministic governed
state and never fetches the network.

Validators repeat policy validation at the activation block's actual
timestamp. The exact-network escrow is materialized by Core from the live
`NetworkId` and asset definition. A recursive release, release-derived escrow
account, or device policy must never be embedded back into genesis: doing so
would change the genesis hash that those artifacts authenticate.

A validator that will validate a transaction against a particular locally
cached candidate must have that candidate before the transaction is submitted.
This is command material availability, not offline capability or node
readiness. `ActivateKagemushaRecursiveReleaseV4` carries the complete
`KagemushaV4PromotionBindingV1`, not only a promotion id. Consensus therefore
commits the independently pinned controller, exact reservation identity,
nonzero promotion run, `NetworkId`, reviewed closure, manifest and release
record, release policy, device-policy bytes, signed genesis, catalog policy,
execution-policy hash, and the domain-separated digest of the complete
unanimous validator runtime projection together with the activation. Candidate
validation compares that digest to the immutable local startup projection
before voting, repeats the comparison before durable application and every
ordinary or recovered consensus signature, and checks reconstructed state
before startup ingress opens. Authenticated-snapshot startup obtains the stable
genesis context from the exact verified root-owned local validator seal and the
current roster/PoPs only from the authenticated snapshot lineage; it never
substitutes the snapshot's later Nexus context. While a release is staged or
enabled, consensus runtime parameters covered by this lock cannot be changed.
Historical replay remains configuration-independent and applies the comparison
only after reconstruction. It also authenticates
the signed release and evidence, exact-eight inventory, asset/scale and future
issuance window, distinct inline Eq/Ep verifier records, matching local cached
material, and the embedded production iOS and Android device-attestation
policy. Each verifier record must carry the exact
release-derived owner identifier and public-input schema hash plus the
domain-separated commitment of its inline key; merely non-empty or non-zero
substitutes are invalid. Release-policy role thresholds must also fit together
within the 64-approval attestation ceiling. The instruction requires both release-
activation and device-policy governance permissions, then publishes the exact
device policy, release, and Eq/Ep records in one consensus transaction overlay.
There is no independently reorderable or standalone release-activation path. A
validator missing material required by a submitted operation rejects that
operation deterministically; it does not become unhealthy or leave the voting
set. Withdrawal ends new issuance and offline-change
creation, but retained material continues to verify and fully redeem previously
issued branches indefinitely. Later governed device-policy rotation remains a
separate operation and invalidates prior registrations, forcing re-registration.
Active native registrations are capped at 512 globally and 64 per account. The
active-world-state V4 record is capped at 4 KiB and stores a validated projection
plus the exact original registration and projection hashes instead of duplicating
the submitted report and evidence bytes from signed transaction history. Up to
1,024 compact records (128 per account) may be retained so one complete cohort
can cool while its replacement admits. Expired or superseded-policy records and
their replay markers are pruned only after the 128-block challenge window is
permanently past; the retained cap blocks further churn until then. These bounds
keep active snapshots within their 512-leaf, depth-9 protocol ceiling and the
registration-state value payloads within 4 MiB, alongside at most 1,024 state
keys and 4,096 registration replay markers.

This is a hard-cut V4 world-state format. Resetting an empty first-release Taira
deployment is compatible with it, but a nonempty chain containing V3
`kagemusha_online_registration_` keys must not upgrade without a separately
reviewed deterministic migration.

## Production boundary

Admission is selected by each transaction's exact authenticated ABI-21/V4
release binding. Consensus must contain the release-qualified Eq/Ep records
required by that transaction, and the production verifier must construct from
authenticated material. There is no process-wide boolean admission shortcut,
per-asset enablement bit, or node-readiness consequence.

`KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE` remains `false` in the
candidate change set. It may be changed only by the final signed promotion
commit after the authenticated review, benchmark, physical-device, and
role-threshold evidence has been added. Even after promotion it is not by
itself authorization for a transaction: the exact authenticated ABI-21/V4
material referenced by that transaction must validate and construct
successfully. None of these transaction checks gates node startup or health.
Top-up and redemption change additionally require the governed selected
release's issuance window to be active. Full redemption authenticates the
parent release for its longer redemption lifetime, so a legitimately issued
note can still be redeemed after issuance closes.

## Release verification

Candidate mode requires the complete source, guard, SDK, and test corridor but
expects production availability to remain false; it does not invent external
evidence. Promotion mode additionally requires a signed candidate commit, an
independently pinned reviewed source closure, the authenticated release bundle,
independent cryptographic review, complete signed physical-device evidence for
every platform slot required by the selected policy, signed role-threshold
approval, and the production corridor. Any proof-code change after the
reviewed closure invalidates that evidence and requires regeneration. A
first-release candidate is admissible only from that exact commit with one SSH
signature trusted by the reviewer's owner-controlled user-level
`gpg.ssh.allowedSignersFile`. Repository-local signature configuration is
ignored and every verifier/policy setting is overridden. The index must equal
`HEAD`; every mandatory worktree path must match the index blob and Git mode
through descriptor-rooted, no-symlink traversal; there must be zero untracked
or ignored files; and the root `Cargo.lock` must be exactly one tracked
mode-`100644` index entry whose bytes also match the separate V1 lock digest.
The legacy `ignored_cargo_lock_*` descriptor field names retain their V1 wire
spelling but bind this tracked file. The complete clean closure is hash-bound
throughout the candidate, native build, device transcript, and signed evidence.
Dirty closures have no compatibility admission path.

The V4 candidate manifest additionally binds
`authenticated_source_seal_projection_sha256`,
`reviewed_cargo_binary_sha256`, `reviewed_rustc_binary_sha256`,
`generator_binary_sha256`, and `sealed_candidate_build_report_sha256`. The
last digest authenticates a canonical report containing both independent build
identities, output byte identities, and the selected generator. The launcher
opens and holds that report, authenticates a private executable snapshot against
its selected-generator identity, and launches the snapshot rather than reopening
the supplied pathname. A production report is only the native controller's
`iroha.kagemusha.native_sealed_candidate_double_build_report.v2` envelope; a
direct Python V1 report is never promotion-admissible. The controller authenticates
the complete symlink-free Mach-O Python runtime and pinned macOS build before
exec, executes the reviewed builder through its held `/dev/fd/12` descriptor,
launches with an empty, reconstructed environment, and publishes the envelope
without replacement below its fixed root-owned report parent. Promotion binds
that envelope back to the exact controller, Python interpreter/runtime tree,
macOS TCB, and signed builder entrypoint used by the live readiness gate. The
readiness controller likewise accepts one exact named promotion environment,
rejects unknown `KAGEMUSHA_*` variables, clears the child environment, and
injects only its descriptor and native-TCB receipts. The sealed builder hashes
the actual absolute `CARGO` and `RUSTC` executables before the
build script accepts the external pins. It then materializes the authenticated
commit, dependency cache, nightly-Cargo root, stable-rustc/sysroot, and native
helper allowlist into inode-independent root-owned snapshots, drops to a
distinct runtime UID, and compiles twice under the signed macOS Seatbelt
boundary. The unit-graph preflight and the two builds use three distinct
Seatbelt profiles; each profile can read only its own authenticated source and
can read/write only its own target and temp roots. Hostile A-to-B write and
B-to-A read/copy probes must fail. A root-held runtime-UID lease excludes
concurrent jobs under the dedicated nologin UID/GID. Between runs, the shared
Cargo lock leaf must remain an empty, private, single-link file and root
atomically replaces it with a fresh inode, closing the last mutable cross-run
channel. The two builds also use distinct authenticated commit snapshots, so
accidental absolute-path capture changes the candidate bytes and fails the
equality check. Both fresh target roots must
produce byte-identical binaries; every
snapshot and host SDK input is revalidated between builds and while sealing the
candidate. The qualification receipt,
cryptographic-review subject, release-attestation subject, promotion record,
sealed-build and candidate-validation reports, Kagami verification report,
promotion gate, and runtime qualification seal all repeat or authenticate this
exact trio. This intentionally invalidates earlier V4 manifest, candidate,
receipt, review, attestation, promotion, activation, qualification-seal, and
sealed-build report bytes; regenerate, re-review, re-sign, and reseal them.

The Taira artifact exporter also runs a candidate-bound release-key regression
before publishing a catalog. One parsed installed prover creates an
initialization pair at proof step 1 and a single-parent recipient append at
proof step 2 with the same final PK/VK material; after the proving keys are
released, one installed verifier terminally verifies both bundles and their
exact `(proof_step_count, peer_hop_count)` values `(1, 0)` and `(2, 1)`.
Catalog export fails closed if that dynamic step transition cannot be proved.

Physical-device evidence is collected before finalization with the separate,
off-by-default `kagemusha-candidate-evidence-lab` build. That build accepts only
the exact reviewed candidate plus its exact ordered eight KRV4 artifacts and
calls the same ABI-21/V4 prover, verifier, and recursion protocol through the
ABI-22 Connect native bridge. Its symbols, marker-bearing native library, and
test host are distinct from production and are rejected by production
packaging. The normal artifact install and proof entrypoints remain unavailable,
and device evidence must record that production capability stayed false.

The current Taira-testnet evidence policy has one physical-iOS slot and makes
no Android-parity claim. Its
[physical-iPhone procedure](sdk/swift/readiness/kagemusha_candidate_ios_lab.md)
requires two fresh XCTest processes, a durable checkpoint and exact reopen,
real offline path measurements, a zero URL-request count, the complete
28-operation lifecycle, full redemption, a fixed resource ceiling, and a
closed Ed25519-signed raw-artifact inventory. Simulator or status-summary
output cannot satisfy that slot. Candidate-bound Android evidence remains a
separate policy slot: its marker-bearing candidate-lab APK is never relabelled
as the separately attested wallet APK used for StrongBox, rotation, rollback,
and device-to-device transfer evidence.

The candidate-native builder and both device launches must use the same full
two-line Xcode 26.6 build identity and the same `iphoneos` SDK identity. Both
values are authenticated by the native-build manifest and signed evidence;
Xcode 26.2 or any build/SDK substitution fails closed.

Run the repository corridor without external evidence while preparing a
candidate:

```bash
ci/check_kagemusha_production_readiness.sh candidate
```

The gate always loads its dedicated static source-contract provider from
bounded bytes. Candidate mode may read that checked-out provider locally;
promotion first root-custodies and descriptor-reads it, authenticates those
exact bytes against the reviewed source closure, and executes the held bytes
without reopening the path. The optional `--self-test` mode then loads the
dedicated internal negative-control harness only after that contract passes.
Promotion applies the same exact-byte custody and authentication rule to the
self-test helper before executing it.

### Authenticated source-seal projection controller

Cargo 1.93 keeps `--unit-graph` behind `-Z unstable-options`; `cargo metadata`
cannot represent command-selected feature or intra-package unit edges and must
not be used to manufacture this evidence. A separate reviewed controller must
capture the genuine V1 unit graph from a pinned Cargo that supports that
interface, assert that the supplied raw bytes are that capture, use the exact
semantic command carried by the projection, and then apply
`cargo-unit-graph-v1-package-root-relative-src-path-source-cache-placeholders-sorted-compact-lf-v1`.
That normalization is exact:

- preserve `version`, unit order, dependency indices, and every Cargo 1.93
  field; emit one root and require every unit to be reachable;
- each unit has exactly `dependencies`, `features`, `mode`, `pkg_id`,
  `platform`, `profile`, and `target`;
- each target has exactly `crate_types`, `doc`, `doctest`, `edition`, `kind`,
  `name`, `src_path`, and `test`, plus optional `required-features`; no target
  field is discarded;
- each profile has exactly `codegen_backend`, `codegen_units`,
  `debug_assertions`, `debuginfo`, `incremental`, `lto`, `name`, `opt_level`,
  `overflow_checks`, `panic`, `rpath`, `split_debuginfo`, and `strip`; no
  profile field is discarded;
- each dependency has exactly `extern_crate_name`, `index`, `noprelude`, and
  `public`;
- replace each absolute target source with its forward-slash,
  package-root-relative path, replace an absolute path-package URI with the
  literal `<PACKAGE_ROOT>` token, and replace any controller source-cache root
  with `<SOURCE_CACHE>`; raw absolute paths are forbidden;
- sort and deduplicate set-like string arrays, roots, and dependencies (by
  `(index, extern_crate_name)`), but do not reorder units; finally emit
  duplicate-free ASCII JSON with sorted object keys, compact separators, and
  one trailing LF.

The validator preserves the Cargo 1.93 field inventory rather than projecting
away fields it does not currently consume. The selected root must retain
`doc:true`, `test:true`, required features `dev-tools`,
`kagemusha-candidate-evidence-lab`, and `zk-halo2-ipa`, and the release profile
must retain `overflow_checks:false`, null `codegen_backend` and
`split_debuginfo`, and `strip:{"resolved":{"Named":"debuginfo"}}`. The
three-unit test graph is explicitly synthetic protocol coverage. This
repository currently contains no authenticated raw Cargo unit graph, complete
normalized release graph, or controller capture receipt; no test fixture or
field census is production Cargo evidence.

The canonical execution policy is
`iroha.kagemusha.source_seal_projection_execution_policy.v1`, is bounded at
64 KiB, and has exactly `schema`, `build_inputs`, `cargo`, `rustc`, and
`unit_graph`.
`cargo` and `rustc` each have exactly `binary_sha256`, `binary_size_bytes`,
`capabilities`, `version_argv`, and `version_stdout_lines`. Their direct
executable digests are nonzero, sizes are bounded at 512 MiB, and version output
is bounded canonical ASCII. Cargo is exactly the unit-graph-capable
`cargo 1.93.0-nightly (6c1b61003 2025-10-28)`; rustc is exactly
`rustc 1.93.1 (01f6ddf75 2026-02-11)`. Stock stable Cargo 1.93.1 is rejected
because it rejects this `-Z` command. The only version commands are
`["<DIRECT_CARGO>","-Vv"]` and
`["<DIRECT_RUSTC>","-Vv"]` respectively. The controller must authenticate
those direct binaries independently; the producer does not open a tool binary.

`unit_graph` has exactly `capture_argv`, `capture_environment`,
`capture_receipt`, `normalization`, `raw_sha256`, `raw_size_bytes`,
`normalized_sha256`, and `normalized_size_bytes`. The receipt binds the
successful exit status, exact Cargo/rustc/build-input/source identities, raw
stdout digest and length, and bounded stderr identity. The exact capture argv is:

```text
<DIRECT_CARGO> -Z unstable-options build --unit-graph
  --release --locked --offline
  --target aarch64-apple-darwin
  --target-dir <FRESH_EXTERNAL_TARGET_DIR>
  -p iroha_core
  --features iroha_core/dev-tools,iroha_core/kagemusha-candidate-source-seal,iroha_core/kagemusha-candidate-evidence-lab
  --bin kagemusha_recursive_spend_v4_bundle --jobs 1
```

The controller must run from an otherwise empty environment. In the recorded
policy it replaces the actual authenticated Cargo command path and fresh target
directory with the two argv placeholders above, and replaces only the actual
absolute Cargo-home and rustc paths with the environment placeholders below.
Every other name and value is exact:

```json
{"CARGO_ENCODED_RUSTFLAGS":"","CARGO_HOME":"<OWNER_CONTROLLED_CACHE_ONLY_CARGO_HOME>","CARGO_NET_OFFLINE":"true","DEVELOPER_DIR":"<ROOT_CUSTODIED_DEVELOPER_DIR>","HOME":"/var/empty","LANG":"C","LC_ALL":"C","PATH":"<ROOT_CUSTODIED_HOST_TOOL_BIN>","RUSTC":"<DIRECT_RUSTC>","RUSTC_WORKSPACE_WRAPPER":"","RUSTC_WRAPPER":"","RUSTFLAGS":"","SDKROOT":"<ROOT_CUSTODIED_SDKROOT>","TMPDIR":"<FRESH_WRITABLE_BUILD_TMP>","TZ":"UTC"}
```

`build_inputs` uses `iroha.kagemusha.build_input_closure.v1`. It binds the
bounded Cargo `git`/`registry` trees, separate complete nightly-Cargo and
rustc/sysroot trees, canonical developer and SDK trees, an exact native-helper
allowlist, fixed symlink-free Python interpreter/stdlib tree and preimport module
origins, dedicated runtime UID/GID, and the qualified macOS/Xcode/Seatbelt
identity. Full ancestry, ACL, xattr, hard-link, and tree identities are checked
before use and rechecked across the build. Compilation uses root-owned read-only
snapshots, a PATH containing only private helper copies, and three disjoint
Seatbelt profiles that deny network, ambient reads/writes, sibling build roots,
and unlisted executables. Each run has exactly one fresh Cargo target and one
private temp directory. The exact unstable unit-graph command is rerun through
the bounded Cargo runner (300 seconds and 16 MiB per output stream); each full
build has a 30-minute wall bound, live 128 MiB stdout/8 MiB stderr bounds,
`/dev/null` stdin, and unconditional private-process-group kill/reap. Both
complete builds' candidate bytes must match.

The Python tree must be provisioned without framework-layout symlinks. Every
non-OS Mach-O/dyld dependency of that interpreter must resolve inside the sealed
runtime tree; `/usr/lib`, `/System/Library`, and the dyld shared cache remain an
explicit signed macOS-build TCB and require external host qualification.

The workstation Xcode layout is not automatically accepted: `/Applications`
is group-writable on the current host and its selected SDK name is a symlink.
The authenticated source-seal build corridor therefore requires Xcode 26.2
build `17C52` under a canonical root-owned, non-group-writable hierarchy, with
a non-symlink SDK path and signed tree identities. This is separate from the
Xcode 26.6 candidate-device lane and the production App Attest capture path.
Ambient `xcode-select`, `DEVELOPER_DIR`, and `SDKROOT` changes cannot select
another SDK.

The installed macOS 26.2 Seatbelt has not yet completed that qualification.
With broad Mach and sysctl grants removed, even a minimal `/bin/echo` launch
under the fixed profile exits with status 134; the unified sandbox log reports
exact denials for `sysctl-read security.mac.lockdown_mode_state`, `sysctl-read
kern.bootargs`, and `file-read-data /`. Those reads are build-visible inputs,
so the builder deliberately does not grant them merely to make the probe pass.
Production remains blocked until a reviewed profile binds the exact required
values/read surface (and any exact Mach services discovered by the complete
build) or a stronger build sandbox replaces Seatbelt. Qualification requires
the real unit-graph preflight and both complete byte-identical builds; the
hostile probe alone is never a qualification receipt.

The policy binds both raw and normalized graph sizes and SHA-256 values plus
the exact normalization identifier. Request, production, and independent
verification each read both graph artifacts through separate external SHA-256
pins, then require both byte lengths and digests to equal the policy. This
authenticates the transferred bytes. The signed successful capture receipt and
the candidate's same-tool sandboxed preflight establish that the pinned nightly
Cargo really supports the required command; review of the controller's
normalization remains an explicit external responsibility. The resulting
projection carries the raw graph
digest/size and the exact Cargo/rustc digest/size pairs derived from that policy.
Candidate-build `--cargo-sha256` and `--rustc-sha256` remain independent pins,
but the consumer and `iroha_core` build script now also require the admitted
executables and their sizes to equal the signed policy identities. A separately
valid build-tool pin can no longer substitute a different toolchain after graph
capture.

The reviewed controller runs the producer through
`scripts/run_kagemusha_source_projection_snapshot.py` and an explicitly
digest-pinned Python interpreter. Promotion authenticates every producer import
against the signed commit, snapshots the complete package under root custody,
and passes that package root explicitly under `python -I -S`; no worktree import or
ambient Python path or global site initialization is used. Its three phases are
`request`, detached SSH signing in
namespace `iroha-kagemusha-source-seal-projection-v1`, and `produce`. A second
controller must run `verify` over the published projection. Request,
authorization, signature, allowed-signers policy, explicit revocation policy,
execution policy, raw graph, normalized graph, reviewed closure, and projection
are all passed with exact SHA-256 pins. The producer derives source authority
from the verified signed commit, recomputes all graph counts (including
`iroha_core` units by equality with the selected root's opaque package ID),
reconstructs the projection, and feeds it through the candidate consumer before
publishing.
Publication first completes and fsyncs a private same-directory temporary,
creates the final name with an atomic no-replace hard link, fsyncs the parent,
authenticates the final inode and bytes, removes the temporary, and fsyncs the
parent again. A post-link error is commit-uncertain and requires authenticating
the final path rather than blindly retrying. Each raw or normalized graph is
bounded at 16 MiB; the producer, candidate consumer, and promotion gate share
one inclusive 16 KiB projection bound.

The controller's normalization of Cargo's unstable unit graph remains an
explicit reviewed responsibility. The SSH signature and successful capture
receipt make that authority substitution-detectable; a hand-authored graph
without the exact nightly execution receipt is rejected. A root-owned
projection and digest pin alone are insufficient.
The protected promotion job must provide separately pinned authorization,
controller signature and trust policies, execution policy, raw graph, and
normalized graph. The gate snapshots every input under root custody and runs the
producer's deterministic `verify` mode itself after re-establishing the reviewed
source identity. It then cross-binds the reconstructed Cargo/rustc identities to
every release manifest before any authenticated artifact verification.

The protected environment therefore owns one canonical absolute path plus one
nonzero lowercase SHA-256 variable for each reconstruction input:

```text
KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION[_SHA256]
KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE[_SHA256]
KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS[_SHA256]
KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION[_SHA256]
KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY[_SHA256]
KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH[_SHA256]
KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH[_SHA256]
```

The revocation file may be empty, but its empty-file SHA-256 remains mandatory.
All seven files use distinct production-owned paths; none is checked in or
inferred from the projection.

For promotion, a separately authenticated launcher must install the reviewed
checkout at a root-owned path whose complete path, `ci/` directory, and gate
file are not group/world writable. It must verify the gate digest before
execution and pass that same digest below; an in-process self-check cannot make
a substituted script trustworthy. Pre-create the fixed owner-private staging
parent (`/private/var/db/iroha-kagemusha-readiness-v1` on macOS or
`/var/lib/iroha/kagemusha-readiness-v1` on Linux). Provision the same canonical
policy file used by validators and the root containing lowercase
manifest-digest directories. The authenticated source-seal projection binds
the exact reviewed closure to the allowed-signers and revocation-policy
digests. An explicitly pinned empty revocation file means no revoked keys; the
revocation input is never implicit. The corridor invokes the digest-pinned
Kagami typed verifier for every release, but never executes Kagami directly.
A distinct digest-pinned native controller must enforce
`iroha.authenticated-tool-os-isolation.v1`: an attested runtime identity,
closed inherited descriptors, no-new-privileges, no filesystem reads outside
the exact policy/release inventory and immutable OS runtime allowlist, no
filesystem writes, network, or tool child creation, bounded output and wall
time, and no residual job processes, with exact forwarding of the verifier
status and byte streams.
This OS boundary is mandatory because per-file limits, directory scans, and
process-group signals cannot contain open-unlinked files, ambient-path writes,
or `setsid` escapes. Its cumulative and maximum-live-root quotas must charge
unlinked writes, not just the final visible directory size. The controlled
verifier authenticates the policy, manifest,
signed attestation, evidence, exact-eight artifacts, bootstrap witnesses, and
promotion record rather than trusting filenames or JSON alone:

The repository implementation and hostile qualification procedure are defined
in `specs/authenticated_tool_os_isolation_v1.md`. The macOS Seatbelt backend is
implemented; Linux requests deliberately fail closed until the documented
Landlock/seccomp/cgroup backend has independent host qualification. A Linux
promotion host therefore remains blocked and must not substitute Bubblewrap or
an unqualified wrapper.

The protected GitHub job checks out `github.workflow_sha`, requires the
canonical `hyperledger-iroha/iroha` workflow path on a protected ref, requires
`github.sha == github.workflow_sha`, and requires both that checkout and the
root-custodied gate checkout to have the same `HEAD`. The gate then authenticates
that root checkout against the signed reviewed-source closure. None of those
identities comes from a repository or organization variable. The only admitted
controller install leaf is
`/Library/SORA/Kagemusha/bin/iroha_authenticated_tool_controller`; provision its
parent and exact-argument sudo grants before dispatch. A mutable variable cannot
select another root destination.

Each dispatched run also derives a promotion-scoped identifier as lowercase
SHA-256 over NUL-terminated values in this exact order:
`iroha.kagemusha.github-promotion-run.v1`, `github.repository`,
`github.workflow_ref`, `github.workflow_sha`, `github.run_id`, and
`github.run_attempt`. The run id and attempt are immutable GitHub contexts, not
repository or organization variables. After dispatch and before approving the
protected environment, the independent online authority must issue the exact
catalog revalidation receipt for that identifier and the complete ordered
release set. Provision it without replacement at
`/Library/SORA/Kagemusha/catalog-revalidation/<promotion-id>.json`. The workflow
constructs that path itself and the gate requires the file to be canonical,
bounded, root-custodied, and outside the immutable evidence root. A rerun has a
different identifier and therefore cannot replay the prior attempt's receipt.
The authority durably marks a promotion identifier expired the first time it
observes that its receipt lifetime has elapsed; a later wall-clock rollback or
authority restart cannot revive that identifier.
The workflow neither receives the authority private key/DeviceCheck JWT nor
invokes `revalidate-catalog`; those runtime-only credentials remain at the
external authority boundary.

```bash
KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256='<reviewed-gate-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON=/absolute/root-custodied/python3 \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256='<reviewed-python-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT=/absolute/root-custodied/python-runtime \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256='<reviewed-python-runtime-tree-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD='<reviewed-macos-build>' \
KAGEMUSHA_V4_KAGAMI_BIN=/absolute/root-custodied/kagami \
KAGEMUSHA_V4_KAGAMI_SHA256='<reviewed-kagami-64-lowercase-hex>' \
KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN=/Library/SORA/Kagemusha/bin/iroha_authenticated_tool_controller \
KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256='<reviewed-controller-64-lowercase-hex>' \
KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE=/absolute/root-custodied/reviewed-source-closure.json \
KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256='<reviewed-closure-64-lowercase-hex>' \
KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION=/absolute/root-custodied/authenticated-source-seal-projection.json \
KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256='<reviewed-projection-64-lowercase-hex>' \
KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_PATH=/absolute/root-custodied/sealed-candidate-build-report.json \
KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_SHA256='<reviewed-sealed-report-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH=/absolute/root-custodied/allowed-signers \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256='<reviewed-allowed-signers-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH=/absolute/root-custodied/revocation \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256='<reviewed-revocation-64-lowercase-hex>' \
KAGEMUSHA_V4_RELEASE_POLICY_PATH=/run/iroha/kagemusha/release-policy.norito \
KAGEMUSHA_V4_ARTIFACT_ROOT=/run/iroha/kagemusha/v4 \
  /absolute/root-custodied/reviewed-iroha/ci/check_kagemusha_production_readiness.sh promotion
```

When the selected policy uses the Taira physical-iOS slot, provide the trusted
Ed25519 identity and an owner-private evidence root whose child names match the
release manifest-digest directories. Each child contains the corresponding
runner's exact `raw/` tree:

```bash
KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256='<reviewed-gate-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON=/absolute/root-custodied/python3 \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256='<reviewed-python-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT=/absolute/root-custodied/python-runtime \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256='<reviewed-python-runtime-tree-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD='<reviewed-macos-build>' \
KAGEMUSHA_V4_KAGAMI_BIN=/absolute/root-custodied/kagami \
KAGEMUSHA_V4_KAGAMI_SHA256='<reviewed-kagami-64-lowercase-hex>' \
KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN=/Library/SORA/Kagemusha/bin/iroha_authenticated_tool_controller \
KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256='<reviewed-controller-64-lowercase-hex>' \
KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE=/absolute/root-custodied/reviewed-source-closure.json \
KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256='<reviewed-closure-64-lowercase-hex>' \
KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION=/absolute/root-custodied/authenticated-source-seal-projection.json \
KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256='<reviewed-projection-64-lowercase-hex>' \
KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_PATH=/absolute/root-custodied/sealed-candidate-build-report.json \
KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_SHA256='<reviewed-sealed-report-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH=/absolute/root-custodied/allowed-signers \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256='<reviewed-allowed-signers-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH=/absolute/root-custodied/revocation \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256='<reviewed-revocation-64-lowercase-hex>' \
KAGEMUSHA_V4_RELEASE_POLICY_PATH=/run/iroha/kagemusha/release-policy.norito \
KAGEMUSHA_V4_ARTIFACT_ROOT=/run/iroha/kagemusha/v4 \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT=/run/iroha/kagemusha/ios-device-evidence \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID="$TRUSTED_KEY_ID" \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY=/run/secrets/kagemusha-ios-evidence-ed25519.pub.pem \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY=/run/secrets/kagemusha-production-ios-policy-v1.json \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID="$FRESHNESS_AUTHORITY_KEY_ID" \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY=/run/secrets/kagemusha-ios-freshness-authority-ed25519.pub.pem \
KAGEMUSHA_V4_PROMOTION_ID="$PROMOTION_ID" \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT="/Library/SORA/Kagemusha/catalog-revalidation/$PROMOTION_ID.json" \
  /absolute/root-custodied/reviewed-iroha/ci/check_kagemusha_production_readiness.sh promotion
```

The checked-in workflow is deliberately named as readiness verification, not
promotion. Its gate authenticates a catalog that already contains
`promotion-record-v4.norito`; it does not publish that record, write a validator
qualification seal, prepare or submit an activation, or archive a
workflow-identity-bound durable receipt. Verification success is not promotion,
and the workflow does not invoke the authenticated controller's
`promote-kagemusha-release-v4` subcommand.

The local Kagami publication boundary is non-circular. `promote-release-v4`
first authenticates the exact seventeen-file pre-promotion candidate, including
the internal-validation receipt plus the release, cryptographic-review, and
physical-device approval roles. It
requires `--promotion-record` to be the exact canonical absent
`<bundle-dir>/promotion-record-v4.norito` leaf, publishes the derived record with
the descriptor-relative no-replace durable writer, and then runs the full exact
eighteen-file promoted-release verifier. The command reports success only from
that post-publication verification pass. `verify-release-v4` remains a distinct
verification-only command and always requires the complete eighteen-file
inventory.

The dedicated authenticated publisher now exists as
`iroha_authenticated_tool_controller promote-kagemusha-release-v4`.
The generic writable-file mode pre-creates outputs, so it cannot preserve the
required absent-leaf, no-replace publication boundary and is not admissible.
The publisher accepts exactly five option/value pairs, in this order, with no
additions or reordering:

```text
--expected-macos-build <build>
--kagami <path>
--kagami-sha256 <sha256>
--bundle-dir <path>
--release-policy <path>
```

The controller is macOS-only and root-only. It pins the expected macOS build,
requires canonical root-custodied controller, Kagami, bundle, and policy paths,
authenticates the exact Kagami binary and exact seventeen-file candidate, and
confines the authenticated Kagami snapshot to the fixed read roots and fixed
same-directory staging names plus the single absent
`promotion-record-v4.norito` leaf. Publication remains no-replace. Success
requires the exact eighteen-file result and Kagami's exact canonical one-line,
candidate-bound JSON report. Once publication may have started, any cleanup,
observation, or reporting ambiguity exits with status 75 and requires explicit
reconciliation rather than automatic retry.

This local controller does not change the checked-in GitHub workflow's status:
that workflow remains verification-only and does not invoke
`promote-kagemusha-release-v4`. Protected hosts must still install the signed
promotion reservation and independently publish the catalog and validator
qualification seals from separate `iroha3d --check-config` invocations; a
repository variable is not a substitute for those root-custodied host-local
inputs.

The data-model boundary used by the local rollout corridor is explicit.
`KagemushaV4ValidatorQualificationSealV1` is signed by the validator's exact
`PeerId` and binds a nonzero promotion id, network, reviewed closure, canonical
manifest and release record, exact release-policy source, canonical governed
device-policy bytes, ordinary genesis-rooted signed-genesis bytes, logical
catalog policy, and the protocol `execution_policy_hash`. It also binds that
host's exact `iroha3d`, flattened TOML source, and canonical catalog
qualification-seal bytes. Those three identities are intentionally per-host:
the catalog seal includes local path/stat identity and a heterogeneous
four-validator fleet may run separately reviewed target binaries. Only the
release, policy, genesis, logical catalog, execution policy, and canonical
runtime-effective projection are required to agree across all four seals.

That projection is derived only after disposable signed-genesis execution and
height-one freezing. It retains the effective chain and I105 discriminant, a
validator-role assertion, configured genesis key and hash, the exact ordered
four public peer addresses, the frozen genesis PoP bytes, the complete
post-overlay Sumeragi-config fingerprint, the signed-and-staged DA/Nexus/AMX/
execution context, and the Kagemusha decoded-memory ceiling. The configured PoP
map must contain exactly the frozen validators and PoPs byte-for-byte, with no
extra entry. The local identity and signing key must agree; its sealed endpoint
is `network.public_address`, never the internal bind address. Both signed
context commitments must use the canonical hash marker, and an observer-role
node cannot produce a seal. "Flattened TOML source" remains useful
source-custody evidence,
but it is not allowed to stand in for this projection when environment,
command-line, or profile overlays affect the actual configuration.

Validator qualification signs only when the exact signed-genesis metadata
selects permissioned consensus with exactly four voters. Permissioned genesis
construction fixes every voter to unit power; an NPoS-mode roster is not a
valid qualification source even when it contains exactly four validators.

`KagemushaV4ActivationFinalityReceiptV1` retains those four strictly ordered,
distinct seals, an independently pinned issuer whose key is disjoint from every
validator and governance member, and the exact canonical
governance `MultisigPolicy`. Governance must have at least two members and a
threshold of at least two, and the admitted transaction must carry at least two
distinct valid member signatures. The receipt also retains the payload-only
`SignedTransaction::hash()` intent, a digest of the complete
authorization-bearing `encode_wire_v1()` bytes, ordinary committed-transaction
inclusion and successful result, the exact result-bearing `SignedBlockWire`, a
separately checked exact-length/SHA-256 identity for those bytes, and its
Sumeragi-v2 finality successor chain. External expectations retain the exact
already-finalized anchor captured before transaction submission. The receipt
chain must contain one to 4096 immediate successors after that anchor and end at
the activation carrier. Its dedicated bounded Norito/JSON wrapper checks the
declared proof count before reserving or decoding the underlying vector;
snapshot-bootstrap contexts are rejected, and every
context must retain the exact sealed four-validator unit-power roster and
`execution_policy_hash`. After finality locates the authenticated block index,
verification compares the actual external transaction's complete signed wire
byte-for-byte with the approved wire; equal intent hashes cannot hide a different
multisignature bundle. Verification accepts only one direct
`ActivateKagemushaRecursiveReleaseV4` instruction; batch, IVM, contract,
sealed, time-trigger, merge-sidecar, failed-result, stale-anchor, substituted
block, roster, catalog, policy, transaction, or authorization-wire forms fail
closed. The instruction's complete promotion binding must equal the shared
sealed binding byte-for-byte. Consensus stores a canonical hash marker for that
full binding alongside the promotion-id marker, and the id is consumed once in the persistent replay
ledger before any later activation can reuse it. The
signed transaction must also carry an exclusive `expires_at_height`: it must be
above the finalized carrier height and no greater than anchor height plus 4097,
so every accepted receipt fits the proof-chain bound. Core enforces any present
height expiry even when configuration does not require one. A fresh
anchor-pinned finality verifier verifies the trusted anchor and every successor
for each receipt. The canonical
receipt is capped at 64 MiB and its block at 32 MiB; Norito uses explicit
cumulative decode limits and exact block re-encoding, while JSON ingress has a
raw-body cap and rejects an oversized base64 block token before decoded
allocation. Caller-built receipts use a real canonical counting-encoder pass to
enforce the 64 MiB ceiling before any complete receipt re-encoding or signature
hash allocation. The 64 MiB value is an encoded-input ceiling, not a peak-memory
promise.

The phase-separated local operator surface is
`iroha offline kagemusha rollout-v4`, with `create-expectations`, `submit`,
`finalize-receipt`, `create-canary-authorization`,
`submit-canary-authorization`, `submit-canary`, `finalize-canary-evidence`, and
`finalize-validator-liveness` subcommands. Its promotion-keyed state root is
canonical and root-owned at
`/var/lib/iroha/kagemusha-rollout-v1/<promotion-id-hex>/` on Linux or
`/private/var/db/iroha-kagemusha-rollout-v1/<promotion-id-hex>/` on macOS.
The immediate promotion directory is mode `0700`; generated expectations,
journals, receipts, authorizations, challenges, and evidence are root-owned,
singly linked, xattr/extended-ACL-free mode-`0400` regular files. Publication
uses a mode-`0600` staging inode, syncs and changes it to `0400`, runs the
verification callback immediately before a no-replace rename, then rereads and
reverifies the same committed inode. Public external qualification inputs use
the separately checked mode-`0444` policy; runtime signing keys remain bounded
mode-`0600` inputs and are never copied into rollout state.
`create-expectations` verifies the signed reservation, the four ordered
validator seals, the exact already-authorized transaction wire, and the trusted
anchor before publishing canonical
`activation-expectations-v1.norito` no-replace.

`submit` requires explicit `--write-authorized` consent. Before any POST or
retry it durably publishes and rereads
`activation-submission-journal-v1.norito`, keyed by the promotion and containing
the exact authenticated expectations bytes. An existing journal must
byte-match and reverify; any pre-existing Torii status without a journal is a
retrospective state and fails closed. POST, transport, and wait ambiguity is an
explicit submission-uncertain result and is reconciled through status and
evidence using the configured transaction timeout. An `Applied` status is not
sufficient by itself: the client fetches the bounded canonical block and full
finality successor chain, decodes canonical `SignedBlockWire`, constructs a
`TrustedBlockProofAnchor` from the finality artifact, locates the unique actual
entrypoint, and compares its complete authorization-bearing transaction wire
byte-for-byte with the approved wire.

`finalize-receipt` first requires the exact authenticated submission journal to
exist and reverify, then reuses that proof-anchored evidence, signs with the independent
receipt issuer, and publishes canonical
`activation-finality-receipt-v1.norito` no-replace.
The activation itself is not the promotion canary. After authenticating its
receipt, `create-canary-authorization` first signs a narrow
`KagemushaV4TairaCanaryPermitV1`, then builds exactly one ordinary
`RecordKagemushaTairaCanaryV4` transaction with a nonce, fee quote, finite
time-to-live, and exclusive height expiry. The permit binds the complete
activation promotion binding, exact expectations and receipt identities,
actual canary authority, canonical configured HTTPS Torii origin, wall-clock
interval, and height corridor. The instruction embeds that permit
byte-for-byte. The controller then signs a separate minimal reservation over
the permit, payload intent, complete-wire digest, and external entrypoint hash,
and finally signs the complete off-chain package containing the reservation and
exact `SignedTransaction`. Canonical
`canary-authorization-v1.norito` is therefore the private complete package; the
on-chain reservation does not reveal or carry the signed transaction or its
wire preimage.

`submit-canary-authorization` requires explicit `--write-authorized` consent
and builds one ordinary transaction carrying only
`AuthorizeKagemushaTairaCanaryV4(reservation)`. It durably publishes and
reverifies both `canary-authorization-submission-journal-v1.norito` and the
root-private complete `canary-submission-journal-v1.norito` before the first
authorization POST. Consensus verifies the reservation controller signature,
network, actual transaction authorizer, exclusive time/height limits, and exact
activation binding, then reserves the external canary entrypoint hash, complete
signed-wire identity, and byte-identical signed reservation under the same
promotion slot. Reapplying that exact reservation is
idempotent; another call, signed wire, or reservation for an occupied slot fails.
Because the public instruction contains only signed digests, a third party
cannot learn and submit the exact canary early to poison its pipeline hash.

`submit-canary` requires separate explicit `--write-authorized` consent, the
same exact Torii origin, network, and authority, the private byte-identical
canary journal, and a proof-finalized matching on-chain reservation. Immediately
before each local commit or network POST, the client advances an
activation-anchored finality verifier through every observed successor, checks
that the next-block inclusion margin remains before the exclusive height
expiry, reads a fresh protected-host time, and reverifies the complete
authorization. At each canonical transaction boundary Core clears prior context
and derives a complete-wire identity only when the enclosing entrypoint is
itself `External` and contains exactly one direct
`RecordKagemushaTairaCanaryV4`; top-level Batch, contract, IVM, dynamically
emitted, and `SealedReveal`-wrapped forms remain unbound. In particular, opening
a sealed commitment does not reinterpret its inner signed transaction as the
external evidence carrier reserved by the controller. Core accepts the record
only for `Ordinary` admission intent and only when both the current external
entrypoint hash and this
authorization-proof-bearing signed-wire identity equal their reserved markers
and the full activation binding and permit still match. The direct record
affinely takes and clears that wire capability before any child data-trigger
execution, and the exact canary is consumed once. Autonomous merge producers
and live or historical followers separately admit only `QueuePlanSynced`
entrypoints after authenticating their payload or batch commitments. The
controller-authorized canary is signed with `Ordinary`; changing that intent
changes its signed entrypoint and complete wire, so merge cannot reclassify it
as the reserved direct canary. A
pre-existing status without the corresponding journal, a mismatching journal,
or an ambiguous submission cannot be retrospectively converted into production
evidence.

`finalize-canary-evidence` requires that exact journal and independently proves
the authorized canary transaction was successfully committed in a block
strictly later than the activation receipt. It collects the byte-identical
committed transaction and canonical `SignedBlockWire`, a nonempty contiguous
four-validator finality extension rooted at the receipt's terminal proof, and
a global/state `Applied` observation at the canary block height. The canary
block creation time must fall inside the authorization interval. The
independent receipt issuer signs the full committed transaction, block, proof
extension, exact authorization identity, and diagnostic response identities;
the command publishes only the absent promotion-keyed
`canary-evidence-v1.norito` leaf. A fresh query of the activation receipt, host
timestamps alone, or generic `iroha taira write-canary` JSON is not promotion
evidence.

A four-member roster and a valid `2f + 1` commit certificate establish finality,
not proof that every validator was live. `finalize-validator-liveness` closes
that separate boundary. Before any request it creates or exact-rereads the
no-replace `post-canary-validator-liveness-challenge-v1.norito`: an
issuer-signed, canary-bound challenge containing a fresh nonzero OS-random
nonce, a maximum five-minute interval, and exactly four ordered qualified
`PeerId`/distinct canonical HTTPS-origin targets. The collector then queries all
four origins concurrently. Its status-hint and attestation transports share one
direct, proxy-free, redirect-free, transport-retry-free client, request identity
encoding, and reject every `Content-Encoding`. The status hint is the bounded,
canonical JSON `/status/blocks` scalar. The attestation response is bounded to
8 MiB and requires canonical Norito plus `Cache-Control: no-store`. The collector
never forwards the primary Torii
client's Basic, bearer, cookie, or custom headers to any separately supplied
validator origin. The attestation request carries only `Accept`,
`Accept-Encoding`, and the signed-challenge header; a validator that requires
separate authentication therefore fails closed instead of receiving a reused
credential. A status read is only an unsigned tip hint; a `404`
caused by a tip race is retried at the application layer with the same signed
challenge and a fresh hint while the interval remains open.

Each accepted response is a `BridgeFinalityAttestationV1` signed by the exact
qualified BLS node key over the common challenge, network, node identity,
genesis proof, restart-free reducer status, configuration/build fingerprints,
and durable-tip proof. The artifact also carries one cryptographically verified
immediate-successor chain from the authenticated canary proof to the highest
reported tip, and each embedded node tip must equal the corresponding proof in
that chain byte-for-byte. The receipt issuer signs the complete four-observation
artifact and publishes
`post-canary-validator-liveness-evidence-v1.norito` no-replace. This proves that
four distinct qualified signing keys answered the same fresh challenge with a
durable tip at or after the exact canary. The verified canary capability retains
the exact controller-authenticated activation-expectations artifact, and the
liveness verifier rejects substituting another expectations capability even
when its promotion id and activation intent match. It does not prove continuous uptime,
physical host independence, uncompromised validator keys, or that all four keys
voted in one quorum certificate. HTTPS origins and request times are trusted
collector observations; a reverse proxy can relay a request but cannot forge a
validator signature.

These commands are local primitives, not evidence of a live rollout or
promotion. Still required are
a clean immutable source commit with exactly one accepted SSH signature, a
fresh same-source ABI-22 Connect native artifact, real physical App Attest and
candidate artifacts, protected collection of all four host seals and their
identical runtime-effective projections, runtime governance and signing
inputs, and live four-validator and Taira submission/finality evidence. An
unsigned mixed commit, dirty checkout, or ignored ABI-19 bridge manifest cannot
satisfy those inputs. The checked-in verification workflow supplies none of
those live steps, and no promotion may be inferred from its success.

The signed JSON itself remains the release's
`physical-device-benchmark.evidence`. The corridor verifies its exact external
raw tree, trusted Ed25519 signature, physical-iOS invariants, the production
policy, and the independently signed online freshness/consumption receipt at
`<evidence-root>/<manifest-sha256>/online-freshness-consumption-receipt-v1.json`,
plus the fresh promotion-scoped catalog revalidation receipt at the fixed path
above, then compares the signed candidate-record digest with the immutable
candidate reconstructed by Kagami from the finalized release. All six iOS
trust variables plus the promotion id and catalog receipt are an all-or-none
input; a simulator, XCTest summary, missing or stale receipt, receipt for a
different run attempt or release set, or raw tree for a different manifest
digest fails closed.

The policy path is always an explicit runtime input. No build-time environment
variable or embedded policy selects a Kagemusha trust root.

The release driver funds four wallets with `10.75`, spends `6.25`, then `2.10`,
then `0.05` after a receiver restart, and redeems every remaining branch. It
asserts that the exact total remains `10.75`. The same driver covers the minimum
atomic unit, maximum supported precision, excess-precision rejection, full and
partial redemption, fees on and off, and hop depths 1, 2, 4, and 8.

Adversarial coverage includes request and proof tampering, replay, duplicate
delivery, lost acknowledgement, restart at every commit boundary, sibling and
ancestor double spend, artifact interruption and corruption, verifier rotation
and expiry, and network interdiction during every peer hop. Device release gates
measure readiness, proof creation, receive verification, QR/NFC end-to-end
latency, redemption finality, payload size, memory, thermal state, and repeated
lifecycle stability on the oldest supported device.
