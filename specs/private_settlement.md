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
One-time recipient/view keys reduce account linkability. A proof is accepted
only after native verification of the exact public statement.

The Rust wallet owns witness material in an owner-only APWB V1 envelope, exposes
only public inspection, and consumes the envelope on every terminal proof
attempt. Its secret input type is deliberately not cloneable, debuggable, or
serializable. The Python native worker retains the envelope in a native vault
addressed by an opaque one-shot handle; Python receives only public bindings and
the final public proof result.

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
signs through `crates/iroha_core/src/private_settlement/auditor.rs`. Missing,
stale, unauthorized, or insufficient approvals prevent Prepare.

Retired decryption keys must be retained, or retained capsules must be rewrapped,
for the applicable regulatory retention period. That is an operator/governance
obligation and a release recovery test, not an implicit property of AEAD.

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
finalized before rotation remains valid and its exact replay remains
idempotent. This history does not grandfather pending work: a bundle prepared
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
- a currently governed auditor may fetch the padded encrypted capsule addressed
  to its policy identity/key;
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
5. Every validator verifies that exact barrier and votes Commit. The canonical
   Commit QC binds the exact complete-bundle digest and is persisted on every
   successful responder. Commit certification does not mutate WSV.
6. The sponsor signs and submits one carrier containing exactly one
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
7. Global receipt planning verifies every authority/QC/delta and every current
   WSV invariant. Only after all fallible checks succeed are all pool heads,
   root provenance entries, nullifiers, encrypted outputs, replay receipt, and
   public receipt inserted into one overlay. Dropping or rejecting the
   transaction leaves the parent state byte-identical.
8. Local stores reconcile the immutable public receipt or abort marker after
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

Expiry or an authoritative abort releases local staged locks without applying
any leg. Replaying a receipt is idempotent only when the exact stored receipt is
identical; conflicting replay, finalized-after-abort, abort-after-finalized,
or expired finalization is rejected deterministically.

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
| `GET .../legs/{payload_digest}/audit-capsule` | identity-bound governed auditor | padded encrypted capsule |
| `POST .../legs/{payload_digest}/audit-approvals` | identity-bound governed auditor | record approval |
| `POST /v1/nexus/private-settlements/bundles` | canonical sponsor signature | submit exact global carrier |
| `GET .../bundles/{bundle_id}` | public | redacted bundle status |
| `GET .../bundles/{bundle_id}/receipt` | public | final receipt or abort marker |

All restricted and authenticated settlement responses advertise private
`no-store` behavior. The ordinary leg-status response exposes lifecycle and
public route/timing fields only; audit approval counts and the governed
threshold remain restricted committee/auditor material. Handlers return
bounded stable error classes rather than echoing identifiers, plaintext,
parser detail, or policy internals.

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
- Commit QCs are evidence only; global state changes only through one finalized
  carrier transaction.
- Every global leg is validated before the first overlay write.
- Replay markers and terminal receipts survive snapshots, Kura replay, and
  restart; ambiguous local state fails closed and reconciles from immutable WSV.
- Governed pool projections retain exact policy-revision lineage so historical
  finalized receipts remain restart-valid and exact-replay idempotent after a
  rotation, while old-policy in-flight bundles remain inadmissible.
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
  boundary. Fault budgets are independent per committee rather than shared
  across the bundle. The release runner requires the N=2 validator-focused and
  full bounded-fault configurations, paper-primary N=3 fault configuration,
  N=4 clean path, and N=3 expiry/replay configuration. The corrected full N=2
  and paper-primary N=3 runs remain unclaimed in repository evidence.
- Independent review must cover the revised AIR, dummy selectors, asset/capsule
  bindings, reimbursement relation, hybrid encryption, approval/QC domains,
  and state machine.

### Real-network fault matrix

Run exact four-validator processes for every dataspace at N=2,3,4,8,16, using
N=3 as the paper's primary configuration. Stop/restart one validator in every
committee plus coordinator/global nodes. Use only the authenticated consensus
message controller, acknowledge each hold/drop, and exercise 5%, 10%, and 20%
loss, phase-cut partitions, delayed delivery, healing, and crashes after
sidecar, staged delta, Prepare QC, Commit QC, Kura append, WSV apply, and receipt
publication. Continuously assert that no strict subset becomes visible or
spendable and that every node converges after healing. Keep signed RS16 DA/RBC
enabled.

The feature-isolated adversarial daemon exposes one-shot process cuts for the
seven durability boundaries above. Each command binds the exact public bundle
id, is stored in the peer's owner-only control directory, and is acknowledged
with an fsynced canonical record before the process aborts. The sidecar,
staged-delta, and committee-certificate cuts occur only after the corresponding
record and directory entry are durable; the Kura cut follows the block and V2
finality append; the WSV cut follows atomic state publication; and the receipt
cut follows durable committee reconciliation. Shipping binaries do not compile
these abort hooks. Presence of a hook is not evidence: the release harness must
retain its exact command/acknowledgement and demonstrate restart convergence.

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
mandatory signed RS16 DA/RBC. The summary binds every raw JSONL file by length
and SHA-256. Every individual loss, phase-cut, and crash row also identifies
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
Prepare, Commit, finality, and end-to-end latency, plus throughput, CPU, RSS,
network bytes, proof/receipt size, and storage growth. Transparent AMX is the
control. The first release publishes its measured envelope; later releases fail
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
