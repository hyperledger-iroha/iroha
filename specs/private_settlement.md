# Atomic Private Cross-Dataspace Settlement V1

## Status

`AtomicPrivateSettlementV1` is a first-release, governed, fail-closed protocol
path. It is separate from transparent Native AMX DvP/PvP. Configuration keeps
it disabled by default. Enabling the flag is necessary but not sufficient:
admission also requires a governed activation height, the active compiled
`IrohaIvmPrivateNoteStarkV1` capability, adequate fixed-slot limits, V1 policy
permission, and the configured governance notice period.

The implementation is not production-qualified until every release gate in
this document and `specs/private_settlement_threat_model.md` is satisfied.
Independent cryptographic review and DOI publication are external gates and
cannot be satisfied by repository tests alone.

## Guarantees and intentional disclosure

One bundle contains 2 through 255 ordered legs, with at most one leg for each
dataspace. Every route is `(dataspace_id, lane_id, lane_incarnation)` and legs
are canonically sorted by that tuple with ordinals `0..N-1`. Every dataspace
authority has exactly four validators and every availability, Prepare, and
Commit certificate contains exactly three distinct valid signatures from that
authority. Observers cannot pad a quorum.

The authority is not a caller-selected set of four keys. At the manifest's
`authority_context_height`, every validator resolves the exact canonical
ordered roster and active lane incarnation from consensus state, requires the
resolved authority height to equal that context height, requires the V1
`f = 1` four-validator geometry, and verifies every BLS proof of possession.
Private-settlement authorities require a live `Committee` key for every member;
the generic participant-lane resolver's live `Validator` compatibility fallback
is retained only for the separately supported transparent path. A
Committee-only peer is registered in WSV and trusted P2P state but is never
added to lane `0`'s signed topology, NPoS candidate set, or global quorum.
Restricted upload, Prepare voting, and global receipt admission all use this
same state-anchored authority boundary.

Ordinary validators receive the proof and opaque fixed-shape delta, not audit
plaintext. Authorized local auditors decrypt and approve the exact plaintext;
the governed default is one approval from N authorized auditors. The global
carrier applies every leg in one state transaction or applies none.

The public record deliberately reveals:

- network and bundle identifiers;
- ordered participant routes and count;
- authority, expiry, lifecycle, and finality heights;
- opaque pool identifiers, epochs, roots, nullifiers, commitments, and fixed
  ciphertext slots;
- availability, statement, proof, capsule, policy, authority, and QC digests;
- committee authorities and exact 3-of-4 QCs through a deduplicated catalog;
- sponsor, public fee intent, reimbursement commitment, and terminal status.

It does not carry literal account identifiers, literal asset identifiers,
amounts, memos, or business results outside encrypted audit material. V1 does
not hide timing, participant count, dataspace identity, stable-pool activity,
or possible asset inference when one dataspace hosts only one CBDC.

## Canonical Norito objects

The implementation lives in
`crates/iroha_data_model/src/nexus/private_settlement.rs`. All objects advertise
V1 explicitly and are encoded with canonical Norito.

- `AtomicPrivateSettlementV1` binds the network, derived bundle id, authority
  context and expiry, sponsor, exact public fee intent and digest, private
  reimbursement commitment, reimbursement leg ordinal, and canonical leg
  commitments.
- `PrivateSettlementLegPayloadV1` contains the restricted proof statement,
  proof bytes, opaque public delta, encrypted audit capsule, and certified
  restricted-availability metadata.
- `PrivateSettlementDeltaV1` contains one route and opaque pool, old/new
  roots and epochs, exactly two nullifiers, exactly three output commitments
  with aligned ciphertexts, and statement/proof/capsule/policy bindings.
- `PrivateSettlementAuditPolicyV1` carries distinct auditor signing and hybrid
  encryption keys, key epoch and height validity, and `min_approvals`.
- `PrivateSettlementAuditApprovalV1` is a purpose-separated signature over the
  exact bundle, leg, route, roots, statement/proof/capsule/policy digests, key
  epoch, authority context, and expiry.
- `PrivateSettlementReceiptV1` carries the exact manifest plus a compact,
  deduplicated authority catalog and per-leg delta, Prepare-QC, and Commit-QC
  references. `PrivateSettlementAbortReceiptV1` carries only opaque identifiers
  and a public reason class.
- `RegisterAtomicPrivateSettlementPrepareV1` is the sponsor-authorized direct
  instruction carrying the exact complete all-Prepare barrier into the
  replicated control-lock map. It is distinct from the later
  `FinalizeAtomicPrivateSettlementV1` financial carrier.

The Prepare barrier, commit bundle, and receipt share the same two-level
`PrivateSettlementAuthorityCatalogV1`. Its `rosters` contain route-free
validator identities and aligned BLS proofs of possession, deduplicated in
canonical first-use order. `leg_roster_indices[i]` selects the roster for
manifest leg `i`. A phase certificate's `authority_catalog_index` remains the
logical manifest-leg ordinal, not the roster index. Before authority-digest or
QC verification, validators combine that manifest leg's exact route and active
lane incarnation with the selected roster to reconstruct the route-bound
`PrivateSettlementCommitteeAuthorityV1`.

Reserved all-zero values, duplicate routes or state keys, noncanonical order,
wrong ordinals, wrong participant bounds, expiry before/at authority context,
oversized carriers, and mismatched derived digests are rejected before state
mutation. A receipt is self-contained enough for every global validator to
verify it without fetching confidential material.

## Private-note proof profile

The audited profile is implemented under
`crates/iroha_core/src/privacy_engines/atomic_private_settlement/` and reuses the
pinned IVM private-note STARK machinery. Its relation has exactly two input and
three output slots. Private selectors activate real slots; inactive slots are
domain-separated, zero-value, and non-spendable. The output roles are recipient,
optional payer change, and sponsor reimbursement.

The relation enforces a balanced confidential transition and rejects the
directional public-balance bridge. It binds the network, manifest proof-binding
digest, bundle and leg ordinal, exact route, salted opaque asset/pool binding,
old/new root and epoch, fixed nullifiers and output ciphertexts, capsule and
policy digests, key epoch, sponsor, public fee intent, and reimbursement terms.
When honestly generated, one-time recipient/view keys reduce on-ledger account
linkability. A proof is accepted only after native verification of the exact
public statement.

Every fixed encrypted output's public `recipient` identifier is derived from
its authorized one-time output view key. The statement and delta require all
three identifiers in one leg to be distinct. The complete Prepare barrier and
receipt reject an identifier reused by any other leg. Before voting Prepare,
each committee validator also rejects an identifier already present in
finalized WSV. Global finalization checks a deterministic recipient index over
all finalized bundle history. That derived index is excluded from snapshot
payloads and rebuilt from canonical encrypted outputs during restore, so a
duplicate in persisted canonical output state fails closed. These checks
enforce one-time identifier use at protocol admission; they do not prevent a
malicious sender or network observer from correlating traffic before
publication.

The Rust wallet owns witness material in an owner-only APWB V1 envelope, exposes
only public inspection, and consumes the envelope on every terminal proof
attempt. Its secret input type is deliberately not cloneable, debuggable, or
serializable. After proof self-verification, the native completion boundary
derives the exact fixed-shape delta from the proved statement and an
authenticated successor root. A second native boundary binds each completed leg
to its governed audit policy, exact four-validator authority, and retention
height, then derives every payload/delta content address and one identical
all-leg provisional manifest. Callers cannot supply those post-proof digests as
trusted values. The Python native worker retains the envelope in a native vault
addressed by an opaque one-shot handle; Python supplies only the public
successor root and receives the statement, proof, derived delta, and encrypted
capsule. No witness enters Python.

## Audit capsule and approval

`crates/iroha_core/src/private_settlement/audit.rs` pads the canonical plaintext
to one configured class and encrypts it under a random 256-bit DEK with
XChaCha20-Poly1305. The same DEK is independently wrapped to every auditor in
the exact policy order using the existing X25519/ML-KEM-768 hybrid KEM and an
independent authenticated nonce. Capsule and wrap AAD bind the network,
dataspace route and incarnation, bundle, leg, policy, key epoch, and canonical
plaintext commitment. They also bind the digest of the exact state-anchored
four-validator authority and its `authority_context_height`. The capsule AEAD
authenticates the canonical complete AAD; each DEK-wrap AEAD authenticates that
same AAD plus its exact auditor identity, recipient hybrid key, and KEM
ciphertext. A capsule or wrapped DEK therefore cannot be transplanted to a
different historical roster or authority height.

The capsule includes the exact parties, asset, amount, memo, policy references,
view data, note openings, and output-encryption openings required for audit. It
does not contain spending authorities. Decryption returns a zeroizing buffer.
The auditor recomputes every public binding, validates policy and height, then
signs through `crates/iroha_core/src/private_settlement/auditor.rs`. A
deployment-owned credential-provider boundary keeps encryption and signing keys
outside Iroha; Iroha independently checks the provider's governed public keys and
returned approval signature.
Capsule access is a read-only authenticated `POST`, not a bearer read keyed only
by the payload digest. Its strict `PrivateSettlementAuditorCapsuleRequestV1`
body contains the complete access policy, and the canonical Norito JSON body is
covered by the identity-bound request signature. The server treats that policy
as evidence rather than authority: one committed `StateView` supplies the
network, authoritative height, and exact route/pool governance projection. An
O(log n) content-addressed lookup reads one durable sidecar, then core binds the
sidecar policy to the governance revision effective at the manifest's
`authority_context_height` and the requested access policy to the revision
effective at the current height. Missing, expired, or unauthorized reads all
return the same unavailable result.

A same-revision read requires the exact historical policy. A successor-policy
read requires a later pool-governance revision in the same `policy_id` lineage,
strictly higher policy revision and key epoch, and an authenticated current
signing key that maps to an auditor identity present in both the historical
policy and the capsule's wrapped-DEK set. Consensus-key reuse is rejected.
This stable-identity rule lets an authorized auditor inspect a retained capsule
after rotation without letting a newly added auditor inherit old ciphertext.
The response attestation binds both the historical `audit_policy` and the
current `access_audit_policy`. Approval submission additionally requires those
policies to be identical, so a successor-policy auditor may read retained
evidence but cannot add an approval to an old-policy in-flight leg.

The Rust production transport accepts a separately governed
`PrivateSettlementCommitteeAuthorityV1` and contacts exactly four distinct
endpoints in that authority's canonical validator order. Every auditor response
carries a purpose-separated BLS attestation by the validator at its endpoint's
roster index. The signed body binds the network, payload digest, authority
digest (including route/incarnation and roster), complete restricted-view
digest, exact lifecycle, node-authoritative height, and responder peer ID.
The transport excludes an authority mismatch, an endpoint serving as another
roster member, or the same responder identity appearing through multiple URLs
from quorum counting. One Byzantine or unavailable endpoint therefore cannot
veto three aligned responders. It requires three canonically identical views
after normalizing only the retry-equivalent Collecting/Audited lifecycle and
height, then returns the actually signed middle-height response; it never
rewrites an authenticated height after verification. The authority-bootstrap
compatibility method is not a production trust anchor. Approval delivery is
then attempted at all four authority-ordered endpoints. Each durable
acknowledgement carries a separate purpose-separated BLS responder attestation
binding the network, exact signed approval digest, payload, authority/roster,
complete acknowledgement view, lifecycle, node-authoritative height, and
responder peer ID. The production client accepts only three unique
roster-aligned responders. It normalizes `newly_recorded` and height solely for
retry-equivalent grouping, keeps `collected`, `required`, and lifecycle exact,
and returns an actually signed middle-height response without rewriting any
authenticated field. Misaligned or duplicate responses are excluded rather
than giving one Byzantine endpoint a veto. The authority-bootstrap approval
method remains compatibility-only.
Missing, stale, unauthorized, split-view, or insufficient approvals prevent
Prepare.

The shipping online-auditor CLI uses only the authority-pinned transport. It
requires the exact current governed policy through an explicit
`--audit-policy` owner-only file, the ordered committee authority as a separate
absolute, owner-only, non-symlink governance trust-anchor file without
unapproved xattrs or extended ACL entries, and an owner-only strict
business-policy file under the same descriptor-bound custody rules in addition
to an explicit local `approve` decision. Restricted files
are opened nonblocking so a checked-path replacement with a FIFO cannot stall
admission before the post-open inode and file-type checks. That
policy binds one exact network, route,
pool, audit-policy lineage and key epoch, canonical payer/recipient/sponsor and
asset allowlists, amount and reimbursement limits, memo bounds, policy
references, and a maximum remaining-height window. Empty identity or asset
allowlists are not wildcards, unknown fields fail closed, and an operator's
`approve` decision cannot bypass a policy mismatch. The business-policy,
decryption-key, and restricted pool-governance inputs have no production
environment-variable fallback.

Restricted files are xattr-free on Linux. On macOS only the exact
`com.apple.provenance` metadata attribute is permitted; the platform ACL
authority attribute `com.apple.system.Security`, every other xattr, and every
extended ACL entry fail closed. Descriptor-bound xattr and ACL checks run both
before and after the bounded read.
Operators must place these files on a qualified local filesystem whose
descriptor-bound xattr and ACL APIs expose every effective access grant. The
release qualification matrix includes native macOS APFS, Linux filesystems
with and without SELinux labels, and rejects unqualified NFS/SMB custody rather
than assuming mode `0600` fully represents remote ACL semantics.

Retired decryption keys must be retained, or retained capsules must be rewrapped,
for the applicable regulatory retention period. The software online-auditor
path accepts repeatable `--auditor-retired-decryption-key-file` inputs and
constructs a nonempty, duplicate-free current-plus-retired runtime keyring. It
selects the exact historical hybrid key named by the governed sidecar policy;
it never tries keys until authentication succeeds. Key files remain runtime
only. Retention or rewrapping is still an operator/governance obligation and a
release recovery test, not an implicit property of AEAD.

## Governed pool activation and policy rotation

`ActivatePrivateSettlementPoolV1` creates the public projection of one
restricted route/pool/asset binding and its canonical initial commitments.
`RotatePrivateSettlementPoolPolicyV1` is the privacy-governance-authorized
replacement boundary for its auditor policy and key epoch. A rotation must name
the exact current `governance_digest`, preserve the route, pool, asset-binding
commitment, frontier, roots, nullifiers, outputs, and receipts, advance the
governance revision by exactly one, use a strictly newer key epoch and different
policy/governance digests, and activate at the block that contains the rotation.
The predecessor must still be active at the preceding height. A rotation is
rejected if a receipt touching the same exact route/pool is finalized at that
activation height; policy activation and pool finalization cannot share that
height.

The globally replicated projection retains a gap-free public lineage of every
superseded policy digest, key epoch, lifecycle, and governance digest. Snapshot
and restart validation resolve the revision effective at both a finalized
receipt's `authority_context_height` and finalization height, so a receipt
finalized before rotation remains valid as historical evidence while its exact
replay is rejected without mutation. This history does not grandfather pending work: a bundle prepared
under the old policy that crosses the activation boundary fails closed before
any global mutation. Operational recovery still requires retaining old
decryption keys for retained capsules or governing and testing capsule
rewrapping before those keys are destroyed.

## Restricted confidential availability

`PrivateSettlementFileSidecarStoreV1` persists provisional and certified
encrypted records addressed by the committed payload digest. The store enforces
owner-only directories and files, non-followed single-link regular files,
same-effective-user ownership, a single-writer process lease, create-new temp
files, canonical decode/re-encode, fsync before rename, and directory fsync.
Startup scanning rejects unknown, noncanonical, substituted, or capacity-
violating records and reconstructs staged pool/nullifier/output reservations.
Those locks use exact-route keys: pool heads are reserved by
`(route, pool_id, old_epoch, old_root)`, nullifiers by
`(route, pool_id, nullifier)`, and outputs by
`(route, pool_id, commitment)`. Equal opaque values on different routes do not
alias, while a conflict on the same full route cannot be bypassed by reusing a
pool identifier.

Access views are least privilege:

- the exact four-validator authority may fetch proof, approvals, and opaque
  delta material;
- a currently governed auditor may fetch the padded encrypted capsule through
  the exact state-bound policy and stable-identity authorization described
  above; decryption additionally requires the exact historical key retained by
  that identity;
- public and authenticated client status views expose only lifecycle metadata;
- missing, unauthorized, and retention-expired restricted reads share one
  unavailable result class.

Nonterminal `Collecting`, `Audited`, `Prepared`, and `CommitCertified` records
are never pruned. Terminal records may be pruned only after their signed
retention height. A finalized global receipt or authoritative abort/expiry is
required before staged reservations are released. Torii's supervised finality
worker runs retention pruning against its synchronously snapshotted
authoritative height on every reconciliation page, including an empty page;
any reconciliation or pruning error stops the worker and fails the service
closed.

## Protocol state machine

The durable lifecycle is:

```text
Collecting -> Audited -> Prepared -> CommitCertified -> Finalized
     |           |          |               |
     +-----------+----------+---------------+-> Aborted / Expired
```

1. The client creates the proof and encrypted capsule locally, uploads exact
   provisional material to all four validators, and obtains a canonical exact
   3-of-4 availability certificate. A certified sidecar is fsynced before the
   provisional file is removed.
2. An authorized auditor fetches and decrypts its capsule, recomputes all
   bindings, applies local policy, and submits a signed approval.
3. Every committee validator independently verifies the proof, approvals,
   current governed pool head, nullifier/output uniqueness, expiry, fixed shape,
   and durable availability. It fsyncs the verified delta and reservations
   before returning its Prepare vote.
4. The coordinator selects the canonical lowest exact three valid votes and
   persists the resulting Prepare QC on every successfully staged responder.
   Once every leg has a Prepare QC, the complete Prepare barrier is immutable.
5. The sponsor submits that complete barrier in one exact
   `RegisterAtomicPrivateSettlementPrepareV1` control carrier through the
   ordinary bounded public-fee path. Global consensus atomically installs one
   bundle row plus opaque pool-head, nullifier, output, and recipient resource
   rows in WSV. Certified-body-equivalent barrier retries are
   WSV-write-idempotent even when valid signer subsets differ (each accepted
   transaction remains subject to ordinary fee admission); substituted,
   conflicting, terminal, or expired barriers fail closed. Registration must
   finalize strictly before expiry so at least one successor height remains for
   the financial carrier. The Rust
   `register_private_settlement_prepare_and_wait_v1` coordinator API waits for
   the exact signed transaction to reach global, state-resolved `Applied`
   finality; block-height advancement or cache-only status is not sufficient.
   Registration and finalization are two separately admitted ordinary
   transactions, so an enabled fee schedule may charge both. The designated
   reimbursement leg's private amount and terms commitment binds the exact
   public fee intent and the V1 constant of two successful fee-bearing
   carriers, and local auditor policy must accept the agreed aggregate
   sponsorship cost in that leg's CBDC. V1 does not introduce a fee-free
   control transaction or silently infer a second reimbursement. Failures
   before registration incur no settlement-carrier fee. If an already
   registered bundle expires, the sponsor bears its one registration fee; if
   the sponsor submits an abort carrier, it bears both registration and abort
   fees. Neither failure path creates the private reimbursement output.
6. Every validator verifies that the barrier's certified-body identity and its
   complete replicated WSV lock set are live before voting Commit. The canonical
   Commit QC binds the exact complete-bundle digest and is persisted on every
   successful responder. Commit certification does not mutate WSV.
7. The sponsor signs and submits one carrier containing exactly one
   `FinalizeAtomicPrivateSettlementV1` instruction. The carrier binds the
   sponsor and exact public fee intent and carries the complete certified
   bundle. Coordinator and WSV preflight measure the complete boxed
   finalization instruction, including its registered instruction framing.
   Torii admission and the core transaction-scoped carrier binding additionally
   measure the exact canonical sponsor-signed transaction--including authority,
   metadata, fee intent, and signature--for `max_carrier_bytes` before any WSV
   mutation. Consensus-assigned `finalized_height` is not signed carrier
   material. Torii rejects already stale ingress and requires room for at least
   the next block within the manifest expiry window. Before finalization, the
   same sponsor may instead submit one
   `AbortAtomicPrivateSettlementV1` carrier that binds the complete public
   manifest and a stable public reason class; it never carries a delta or
   confidential sidecar material.
8. Global receipt planning verifies every authority/QC/delta, the registered
   certified-body identity and exact resource rows, and every current
   WSV invariant. Only after all fallible checks succeed are all pool heads,
   root provenance entries, nullifiers, encrypted outputs, replay receipt, and
   public receipt inserted and the complete replicated lock set removed in one
   overlay. Dropping or rejecting the transaction leaves the parent state
   byte-identical.
9. Local stores reconcile the immutable public receipt or abort marker after
   finality and on restart. Only then do they mark the sidecar terminal and
   release its staged reservations.

The complete prepared-bundle digest commits to every certified Prepare body and
authority-catalog index, but normalizes away the signer bitmap and aggregate
signature. Every exact three-of-four subset over the same body is therefore
quorum-equivalent: a coordinator restart cannot fork Commit digests merely
because a different honest validator is unavailable. Certificate cryptography
is still verified independently before the normalized digest is accepted.
Participant nodes expose a sponsor-authenticated, leg-scoped recovery read for
their locally durable Prepare and Commit QCs. Restart-safe clients query at
least three committee members, canonically select any recovered equivalent QC,
re-fan it out, and verify that the certified body remains durably recoverable.
The finalized carrier and receipt still retain the exact certificate bytes the
sponsor presented; an authenticated coordinator transcript is the evidence for
historical signer-subset collection when a deployment requires that provenance.

Height-based block-start expiry removes expired replicated WSV lock sets;
expiry reconciliation or an authoritative abort also releases local staged
locks without applying any leg. Exact terminal replay, conflicting replay, finalized-after-abort,
abort-after-finalized, or expired finalization is rejected deterministically
without changing any root, nullifier, output, receipt, or terminal marker.

## Torii interfaces

The canonical route catalog is
`crates/iroha_torii_shared/src/route_catalog/private_settlement.rs`.

| Route | Principal | Result |
|---|---|---|
| `POST /v1/nexus/private-settlements/legs` | canonical account signature | encrypted leg upload |
| `POST .../legs/availability-shares` | canonical account signature | one validator availability share |
| `POST .../phases/prepare-votes` | canonical account signature | one independently verified Prepare vote |
| `POST .../phases/commit-votes` | canonical account signature | one complete-barrier Commit vote |
| `POST .../phases/certificates` | canonical account signature | persist exact Prepare/Commit QC |
| `GET .../legs/{payload_digest}/phase-certificates` | canonical sponsor signature | locally durable Prepare/Commit QCs for recovery |
| `GET .../legs/{payload_digest}/status` | canonical account signature | redacted leg lifecycle |
| `GET .../legs/{payload_digest}/committee-proof` | identity-bound exact validator | proof, approvals, opaque delta |
| `POST .../legs/{payload_digest}/audit-capsule` | identity-bound governed auditor plus exact governed-policy body | padded encrypted capsule plus historical/access-policy bindings |
| `POST .../legs/{payload_digest}/audit-approvals` | identity-bound governed auditor | record approval |
| `POST /v1/nexus/private-settlements/bundles` | canonical sponsor signature | submit exact Prepare-lock registration, finalization, or abort carrier |
| `GET .../bundles/{bundle_id}` | public | redacted bundle status |
| `GET .../bundles/{bundle_id}/receipt` | public | final receipt or abort marker |

All restricted and authenticated settlement responses advertise private
`no-store` behavior. The ordinary leg-status response exposes lifecycle and
public route/timing fields only; audit approval counts and the governed
threshold remain restricted committee/auditor material. Handlers return
bounded stable error classes rather than echoing identifiers, plaintext,
parser detail, or policy internals. SDKs retain a server reject-code header only
when it matches the bounded public grammar `[A-Za-z0-9_.:-]{1,128}`. Response
decode and validation failures discard parser causes as well as body text, so
cause-aware logging cannot recover attacker-controlled field names or response
documents.
Carrier admission returns only the bundle identifier, observed admission
height, and signed carrier hash. It does not claim that a queued abort is
already durable; the public status and receipt routes are the authoritative
terminal projection. Every SDK requires both identifiers to be canonical,
checksummed Norito `Hash` JSON literals and the height to be an unsigned
64-bit integer; missing, additional, mistyped, non-canonical, or overflowed
fields fail closed before the response reaches application code.
Success status is part of the exact V1 contract: carrier admission requires
HTTP `202`, while every other private-settlement operation requires HTTP `200`.
SDKs reject an alternate successful `2xx` code before accepting its body and
do not echo that unexpected response body through client errors.

Restricted response acceptance is a native cryptographic boundary, not a
schema-only SDK check. Rust performs the authoritative validation directly;
Python, JavaScript, Kotlin/JVM, mirrored Java Android, and Swift call the same
Rust rules through their shipping native bridge. Committee-proof verification
binds the exact response bytes to the configured network and requested payload
digest and validates the complete authority roster, proofs of possession,
signatures, proof statement, opaque delta, approvals, availability, and
lifecycle. Auditor-capsule verification additionally binds the exact governed
auditor signing key, while approval acknowledgement verification also binds the
exact request bytes sent by the client and verifies the local approval
signature. Missing bridge symbols, an ABI mismatch, or any native rejection
fails closed with a fixed redacted error; restricted HTTP dispatch does not
begin until bridge availability is established. Public redacted status and
receipt queries do not depend on access to restricted verification material.

## Configuration

Production behavior is sourced from `[nexus.atomic_private_settlement]`; no
environment variable enables the feature. The actual configuration includes:

- `enabled` and `activation_height`;
- `minimum_activation_notice_blocks` and `proof_profile_version`;
- `max_participants` (hard V1 maximum 255) and `max_expiry_blocks`;
- audit, Prepare, and Commit height timeouts;
- strictly increasing capsule padding classes;
- proof, whole-canonical-capsule, carrier, per-record, and retention bounds;
- `sidecar_max_records` and `sidecar_max_total_bytes`, which are consensus-
  committed governed capacities passed to every Torii sidecar store at open;
- `default_min_auditor_approvals`, which is the governed minimum threshold
  accepted for every newly admitted policy, and permitted audit-policy versions.

`max_capsule_bytes` limits the canonical Norito encoding of the complete
`PrivateSettlementAuditCapsuleV1`, including AAD, nonce, ciphertext, vector
framing, auditor identifiers, and every wrapped-DEK row; it is not a
ciphertext-only allowance. Configuration parsing proves that each enabled
padding class can fit the conservative complete-capsule envelope for at least
`default_min_auditor_approvals` auditors. Admission then rejects any policy
whose `min_approvals` is below that governed floor and any actual capsule whose
whole canonical encoding exceeds `max_capsule_bytes`.

`max_carrier_bytes` limits the complete canonical sponsor-signed direct
transaction, including registered instruction framing, authority, metadata,
fee intent, and signature. Coordinator/WSV receipt preflight reconstructs and
measures the boxed finalization instruction, while Torii and the transaction-
scoped core binding enforce the exact signed-transaction size. Ordinary network
transaction bounds remain independent.

Invalid zero values, unsupported V1 versions, duplicate/unsorted padding or
policy lists, enabled-without-activation configurations, and values above hard
protocol limits are configuration errors.

## Determinism and recovery invariants

- All hashes and signatures cover canonical Norito objects with explicit domain
  separation; no JSON or platform-native layout enters consensus.
- Committee choice is canonical and independent of arrival timing once at least
  three valid votes exist.
- Expiry is height based, not wall-clock based.
- A QC is never returned before the corresponding sidecar/delta/certificate is
  durably persisted.
- Commit votes require the complete all-Prepare certified-body identity to be
  present in the globally replicated WSV lock map. Independently valid 3-of-4
  aggregate encodings over the same bodies are accepted without rewriting the
  original lock; transaction admission, height advancement, and local sidecars
  alone do not satisfy this gate.
- Commit QCs are evidence only and never mutate state. The separately finalized
  registration carrier changes only opaque global control-lock state; every
  confidential financial effect occurs together in the single finalization
  `StateTransaction`.
- Every global leg is validated before the first overlay write.
- Replay markers and terminal receipts survive snapshots, Kura replay, and
  restart; ambiguous local state fails closed and reconciles from immutable WSV.
- Snapshot restore accepts exactly the current 188-field `World` schema or the
  frozen 180-field schema emitted by revision
  `1bdec3b88c348a84776241839fb0e8ad71738b3e`. That upgrade boundary adds 13
  serialized fields: the eight private-settlement maps,
  `sccp_ton_breaker_observations`, `sccp_replay_forests`,
  `privacy_exact12_qualification`, `consensus_evidence`, and
  `tle_key_session_lifecycles`. It retires five historical stores:
  `sccp_outbound_proofs`, `sccp_inbound_messages`,
  `direct_lane_block_application_markers`, `council`, and
  `parliament_bodies`. Restore decodes every retired store with its exact
  historical key/value types, accepts only canonical empty `revert` and
  `blocks` maps, and synthesizes only canonical empty/default values for all 13
  successors. The same exact predecessor `State` boundary carries the retired
  `SumeragiParameters.key_require_hsm = false` and ordered
  `key_allowed_hsm_providers = ["pkcs11", "softkey", "yubihsm"]` fields.
  Restore accepts only those canonical predecessor defaults before removing the
  fields from the current typed state. Canonical hashing makes one root-level
  compatibility decision and reinstates both HSM defaults together with the
  complete 13/5 `World` bridge; neither half may normalize independently.
  Retained governed parameters, including any canonical valid predecessor
  `sumeragi_npos_parameters.slashing_delay_blocks`, are preserved byte-for-byte
  and are not mistaken for migration markers. Literal field order, checked-in
  full-State and `World` bytes,
  artifact SHA-256 values, and schema-order SHA-256 are frozen to that exact
  revision. Partial omission, reordering, renaming, extra fields, hybrid
  predecessor/current schemas, or any retired state is rejected. Canonical WSV
  hashing normalizes only this complete 13-empty/5-absent boundary, preserving
  the complete predecessor State commitment and its exact Kura
  block/checkpoint/manifest binding; no selected-field projection or APS-only
  hybrid is accepted.
- Governed pool projections retain exact policy-revision lineage so historical
  finalized receipts remain restart-valid after a rotation while exact replay
  is rejected byte-silently and old-policy in-flight bundles remain inadmissible.
- Mandatory signed RS16 DA/RBC remains enabled in every deployment and fault
  test; there is no private-settlement bypass.

## Verification and publication gates

### Correctness and cryptography

- Roundtrip and negative Norito fixtures cover 2 and 255 legs, ordering,
  duplicates, authority catalogs, exact quorum, and all digest substitutions.
- Proof tests cover real/dummy selector combinations, balanced two-input/
  three-output transitions, salted asset binding, audit binding, sponsor
  reimbursement, fee substitution, root/nullifier/output replay, and witness
  terminal zeroization.
- State tests assert byte-identical roots, nullifiers, commitments, outputs,
  receipts, and balances after any invalid leg; success advances every leg
  exactly once.
- The count-symmetry TLC model covers atomicity, idempotency, expiry, crash
  recovery, and bounded liveness for 3 and 255 legs, with negative controls
  that must violate safety when the atomic guard is removed. The complementary
  committee-indexed refinement covers independent exact four-validator
  committees, static `f=1` Byzantine/unavailable identities, local auditors,
  authenticated channel faults, global quorum, and every named durability
  boundary. The indexed model uses a canonical representative for exchangeable
  validator identities. Concrete Hold, Drop, and Delay controller modes refine
  to one common delivery-blocking formal state; kind-specific delay delivery,
  retry timing, and hold release remain real-process obligations. This quotient
  is part of the specification rather than TLC `SYMMETRY` or `VIEW`, preserving
  sound liveness checking with the pinned tool while the real-process matrix
  retains every concrete controller mode. Durability is a temporal action
  property over every transition, with abort/expiry as the sole staged-lock
  release. A dedicated indexed mutation must violate that property by losing a
  staged record on crash. Exact signer sets are retained until certification;
  a second temporal action property checks every new sidecar, Prepare QC, and
  Commit QC against its authenticated pre-state quorum before that no-longer-
  observable vote history is discarded. Durable QCs are opaque markers rather
  than retained signer bitmaps, and recovered-state initialization would need
  an explicit provenance witness. Faults are injected at the first clock-free
  phase where they can affect voting or delivery, and future-inert local state
  is canonicalized after Commit. Aggregate weak fairness is justified by
  finite fault budgets, state-changing recovery, and monotonic progress; any
  future unbounded retry cycle requires per-instance fairness. Fault budgets
  are independent per committee rather than shared across the bundle. The
  indexed expiry action abstracts crossing the configured height and does not
  claim to model block-height passage or timing. The release runner requires the N=2
  validator-focused and full bounded-fault configurations, paper-primary N=3
  fault configuration, N=4 clean path, and N=3 expiry/replay configuration.
  Current mutable-checkout runs pass all five indexed positive rows, including
  58,085 distinct states for full N=2 and 8,898,534 for paper-primary N=3, and
  the staged-loss mutation produces the required action-property violation at
  status 13. These are frozen-input development results, not clean-candidate
  release evidence.
- Complete formal-release evidence is fail-closed over one exact ordered
  `(configuration, expected outcome, model)` matrix. Its source-sealed digest
  binds both TLA+ models, every configuration, and a separate evidence-code
  digest for the report producer, runner, result-contract helper, and Java
  resolver. The archived transcript uses canonical metadata and exact ordered
  model-qualified sections; the DOI verifier independently replays SANY and
  TLC parsing, rejects injected/reordered headers, noncanonical run controls,
  unexpected diagnostics, duplicate JSON keys, and report/transcript row
  drift. The report also records the resolved Java executable byte digest and
  bounded version output. Independent release review must match that runtime
  identity to the declared builder environment; recording it is not itself a
  trusted-runtime attestation.
- Independent review must cover the revised AIR, dummy selectors, asset/capsule
  bindings, reimbursement relation, hybrid encryption, approval/QC domains,
  and state machine.

### Real-network fault matrix

Run exact four-validator processes for every dataspace at N=2,3,4,8,16, using
N=3 as the paper's primary configuration. The canonical participant profile is
one non-universal public dataspace followed by N-1 restricted dataspaces; the
global coordination dataspace remains public and is not a settlement leg. The
primary N=3 capture therefore contains eight public P2P validator ports (four
global plus four public-participant validators) and eight restricted P2P
validator ports. Both the frozen configuration and each harness request bind
this ordered visibility vector, and any omission, reordering, substitution, or
all-restricted profile fails closed. Stop/restart one validator in every
committee plus coordinator/global nodes. Use only the authenticated consensus
message controller, acknowledge each hold/drop, and exercise 5%, 10%, and 20%
loss, phase-cut partitions, delayed delivery, healing, and crashes after
sidecar, staged delta, Prepare QC, Commit QC, receipt publication, and both the
Kura-append and WSV-application boundaries of the all-Prepare registration and
finalization carriers. Continuously assert that no strict subset becomes
visible or spendable and that every node converges after healing. Keep signed
RS16 DA/RBC enabled.

The feature-isolated adversarial daemon exposes one-shot process cuts for the
distinct durability boundaries above. Each command binds the exact public bundle
id, is stored in the peer's owner-only control directory, and is acknowledged
with an fsynced canonical record before the process aborts. The sidecar,
staged-delta, and committee-certificate cuts occur only after the corresponding
record and directory entry are durable; the Kura cut follows the block and V2
finality append; the WSV cut follows atomic state publication; and the receipt
cut follows durable committee reconciliation. Shipping binaries do not compile
these abort hooks. Presence of a hook is not evidence: the release harness must
retain its exact command/acknowledgement and demonstrate restart convergence.
For every trial whose nonfinalized checkpoint is after Prepare registration
(all route-loss/phase-cut trials, Commit-QC cuts, both registration cuts, and
all finalization cuts), the retained snapshot must show the complete `1 + 9N`
replicated reservation rows with one identical commitment on every validator.
Global validators must have an empty committee-local lock plane; every one of
the four validators in each participant committee must expose exactly one pool
head, two nullifiers, and three output reservations with an identical
committee-local commitment. Sidecar, staged-delta, and Prepare-QC cuts occur
before global registration and therefore retain the empty replicated plane.
Every terminal snapshot must restore both lock planes to their exact empty
commitments.

Each real-process run emits one strict JSONL record for
`scripts/private_settlement_fault_report.py`. The reporter requires the exact
N=2,3,4,8,16 matrix across at least ten seeds per N, one validator restart in
every committee, coordinator/global restarts, acknowledged 5/10/20-percent
loss for restricted DA, Prepare, and Commit, all phase cuts and persistence
boundaries, convergence, byte-identical invalid-leg state, exactly-once success,
replay rejection, and zero partial visibility or spendability observations.
Every run binds the same full source commit and archived hardware-description
SHA-256, plus the archived N-specific configuration SHA-256. A canonical
configuration manifest covers N=2,3,4,8,16 in order and binds every exact
configuration file while asserting four validators, 3-of-4 quorum, and
mandatory signed RS16 DA/RBC. It also binds the exact bounded coordinator/
prover Rayon width and validator scheduler, Rayon, and pipeline worker width.
The real-process launcher must override any ambient `RAYON_NUM_THREADS` value
with the recorded coordinator/prover width; the localnet builder must write the
recorded validator width into every peer configuration; and the Rust harness
must reject either mismatch before starting a validator. The execution record
also fixes Cargo build jobs, release codegen units, and incremental compilation.
The launcher removes caller-provided Rust flags, compiler wrappers, build
targets, target-specific flags, and profile overrides before applying those
canonical build settings. The summary binds
every raw JSONL file by length and SHA-256. Every individual loss, phase-cut,
and crash row also identifies
one globally non-reusable record reference inside a SHA-256-bound archived
authenticated-controller transcript and one inside an archived atomicity
observation capture. The final DOI verifier resolves both digests to unique
manifest artifacts and requires each referenced JSONL row to match its raw
participant count, seed/run, collection, trial index and fault parameters. It
also checks the exact controller acknowledgement/recovery fields and the
capture's positive continuous-check count plus zero partial visibility and
spendability observations before regenerating the summary from the archived raw
runs rather than trusting a detached matrix claim.
Synthetic or simulated records are test fixtures only and must never be
published as real-process evidence.

N=17 through 255 are deterministic state-machine, codec, carrier-size, and TLC
tests. They must not be labeled real-network latency measurements.

### Leakage and performance

Capture Torii, restricted/public P2P, block wire, Kura/merge artifacts,
snapshots, queries, events, logs, and telemetry. Plant account, asset, amount,
memo, and capsule canaries in multiple encodings. Differential runs in which
only secrets change must preserve public shapes and traffic counts; only
cryptographic values may differ. Publish the residual metadata listed above.
`scripts/private_settlement_leakage_audit.py` enforces this with byte-level
canary scans, exact public file/size/JSON-shape comparison, and mandatory paired
V1 traffic-count manifests covering Torii requests/responses,
public/restricted P2P packets, blocks, queries, events, logs, and telemetry.
Its report binds the canary manifest, every scanned artifact, and both
traffic-count manifests by byte length and SHA-256.
The restricted source archive also carries a separately raw-bound registration
checkpoint for every validator, captured only after the exact registration
transaction reaches state-resolved finality. Independent replay requires the
checkpoint height to lie within that validator's retained observation interval,
the financial ledger to remain byte-identical to baseline, the same complete
`1 + 9N` replicated lock on all validators, no local locks on global validators,
and one exact six-row local leg lock on every member of each participant
committee. State responses must be canonical compact JSON and every Iroha hash
literal must have a valid checksum; syntactically shaped but noncanonical or
checksum-corrupted evidence is rejected.
Canonical account and asset canaries are expanded into their protocol-native
I105-controller and asset-UUID byte encodings as well as ordinary text, integer,
hex, Base64, URL, and JSON representations. The real loopback capture is split
by the exact per-run Torii, public-lane P2P, and restricted-lane P2P port
manifest with `scripts/private_settlement_capture_split.py`; packet payloads and
timestamps are copied unchanged into canonical pcapng surfaces, and an empty
declared channel fails closed. The unfiltered classic pcap is retained as an
owner-only `restricted_packet_source`; final capture provenance binds its exact
bytes, the canonical port manifest, and the raw `tcpdump` stderr. Validation
reparses the capture statistics, requires a nonzero capture with no kernel
drops, rejects truncated/fragmented packets, scans the complete raw capture for
canaries before filtering, and reconstructs all four published pcapng surfaces
byte-for-byte from that retained source.

Non-packet evidence is retained once more in an owner-only
`restricted_audit_source` archive. Its canonical rows preserve the exact source
bytes for block wire, queries, events, operator/coordinator logs, telemetry,
Kura/merge/snapshot inputs, confidential DA, and all 16 peer atomicity
observations. Replay verifies each public digest projection against those raw
bytes. Atomicity replay additionally requires a common byte-identical baseline
and terminal state across every peer, bounded nondecreasing heights, no baseline
or terminal staged locks, and exactly the expected N=3 pool/root/nullifier/
output/receipt transition with no partial visible or spendable state.
The DOI-bundle verifier independently requires those bindings to cover every
archived public and restricted capture, Kura/merge and snapshot artifact,
query/event/log/telemetry record, both retained raw-source archives, and both
traffic-count manifests; a clean report from another capture set cannot satisfy
the gate. It reloads each archived capture-provenance response and reruns the
release runner's complete raw-pcap, restricted-source, and atomicity validator
against the archived left/right directories. It also reloads the canary
manifest and independently rescans every archived privacy surface, so a
digest-rebound report with suppressed findings still fails. A separate
canonical differential-pair manifest binds the left and right artifact paths,
kinds, lengths, and SHA-256 digests for every required privacy surface. The
verifier requires the declared left/right roots to contain exactly that paired
archive inventory and loads every pair itself. Public/fixed-shape surfaces must
have equal byte lengths and JSON shapes. The entropy-bearing raw pcap and
restricted-source archive are explicit whole-file size exceptions; instead the
verifier requires equal packet link types and per-packet length sequences, equal
restricted row identities, and equal lengths for the restricted fixed-shape
groups. Changing a same-size public field name, a packet boundary, a
capture-provenance claim, or an unpaired differential file cannot be hidden by
rewriting summary reports and their digests.

For each real N, run at least five warmups and thirty measured bundles across
multiple seeds on pinned hardware. Report p50/p95/p99 with confidence intervals
for proof, upload/availability, auditor response, committee verification,
Prepare QC aggregation, all-Prepare registration finality, Commit QC
aggregation, financial finalization, and end-to-end latency, plus throughput,
CPU, RSS, network bytes, proof/receipt size, and storage growth. Transparent AMX
is the control. The first release publishes its measured envelope; later releases fail
when p95 regresses by more than `max(10%, 3 MAD)` or p99 by more than 20% against
the signed baseline.

`scripts/private_settlement_benchmark_report.py` validates the raw JSONL matrix
and emits deterministic bootstrap intervals. The final release verifier parses
the retained raw samples again, regenerates every p50/p95/p99, MAD, and
deterministic confidence interval, and requires the published measured-run and
seed identities, stage/resource shapes, counts, and passing regression result
to match exactly. Every raw sample and the generated report bind the same full
release commit, archived hardware-description digest, stable hardware-profile
digest, and exact N-specific configuration digest, so results from another
build, host profile, network configuration, or altered summary cannot be
relabeled. The stable profile is the canonical hash of the validated host,
operating-system, kernel, architecture, CPU/core, memory, storage, network,
clock, power, and virtualization fields; only the release-specific commit,
collection timestamp, and validation marker are excluded. This lets a later
candidate carry its own commit-bound hardware artifact while still proving that
it ran on the same pinned environment. Later-release regression comparisons
reject a baseline captured with a different stable hardware profile,
configuration matrix, or benchmark requirements before applying the p95/p99
thresholds.

### Release artifact

Focused suites, workspace tests, strict clippy, format verification, ten strict
randomized seeds, two-hour soak, serial privacy-release checks, release
inventory, reproducible build, SBOM, and every real-network matrix must pass.
Archive raw CSV/JSON, exact configurations, sanitized captures, plots, logs,
manifest hashes, commit id, hardware description, threat model, protocol
argument, limitations, and independent audit reports in a DOI-backed artifact
for the BCK26 paper.

Validate the final, already-published artifact with
`scripts/private_settlement_release_evidence.py <bundle>/release-manifest-v1.json`.
The V1 manifest binds every file by byte length and SHA-256, rejects unlisted
files and symlinks, and requires the exact real-network participant/loss/crash
matrix, four-validator 3-of-4 committees, ten randomized seeds, a two-hour
soak, benchmark sample minima, reproducible-build/SBOM evidence, a complete
independent-audit scope, an auditor key-custody/rotation report, and a canonical
DOI. Focused/workspace tests, strict Clippy, format verification, SDK,
inventory, ten-seed randomization, two-hour atomic soak, and serial
privacy-release gates are structured V1 reports bound to the exact source
commit and to distinct separately archived operator transcripts; a one-line
placeholder cannot satisfy them. Reproducible-build evidence must contain at
least two distinct builder/environment records that produce byte-identical
archived release binaries, and the CycloneDX 1.5/1.6 SBOM must bind the same
commit and SHA-256 hashes. The hardware description is a commit-bound structured
record and derives the stable hardware-profile binding used for cross-release
benchmark comparison. The configuration manifest must bind every archived
N=2,3,4,8,16 configuration used by both the fault and benchmark matrices. This
verifier deliberately does not manufacture or waive any of those external
results.

The release inventory is derived, never ratcheted to a literal count. The
candidate producer must first prove a clean `HEAD`, index, and worktree, then
archive the raw Git commit object, canonical recursive
`(path, mode, object_type, object_id)` tree rows, the exact binary source path
list, the deterministic `iroha-workspace-source-seal-v1` archive, and the
candidate `Cargo.lock`. The verifier hashes the raw commit object to the
release commit, reads its tree header, reconstructs that root tree from the
rows, derives the file count, compares the path list byte-for-byte, streams and
rehashes every regular file or symlink in the source seal to its Git object,
checks gitlink shape, recomputes the full-permission workspace manifest, and
requires the sealed `Cargo.lock` bytes to equal the separately archived
lockfile. Missing, unexpected, or changed paths are reported by name; two
matching self-reported counts cannot satisfy the gate.

Both the producer and the independent verifier resolve the complete archived
symlink graph before acceptance. A target that is lexically in-root but escapes
after another archived link is followed, a cycle, `.git` traversal, or a
Windows-drive/backslash target fails closed before any link is materialized.
Valid deeply nested inventory paths are reconstructed iteratively rather than
through the host language's recursion stack. The source manifest and every
structured command-gate report are reread through a stable bounded file handle
and authenticated against the exact artifact digest and length at the point of
JSON parsing. Exactly one archive, raw commit, lockfile, path list, source
manifest, and release-inventory report may appear in a release bundle.

Generate those source artifacts from the final clean checkout before assembling
the DOI bundle:

```sh
python3 scripts/private_settlement_source_evidence.py \
  --repository-root . \
  --bundle-root /absolute/path/to/release-bundle
```

The command refuses an output directory inside the checkout, any staged,
unstaged, untracked, or unmerged source, an existing destination, non-canonical
tree rows, or source identity drift during capture. It publishes
`evidence/source` by one directory rename only after repeating the source and
inventory checks. Add every artifact declaration printed in its JSON result to
the final release manifest; the final verifier remains the authority for the
complete DOI bundle.
