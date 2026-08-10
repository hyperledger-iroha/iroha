---
title: Governance DAG Publishing Pipeline
summary: SF-12 implementation status for deterministic Kubo publication, signed-HTTP head CAS, sealed recovery, a bounded authenticated mirror, and remaining deployment evidence.
---

# Governance DAG Publishing Pipeline

## Status
SF-12 is implemented locally through the public publishing boundary. The
workspace ships governance-log payloads,
signature validation, reference validation tooling, and local filesystem
publishers for several SoraFS evidence streams. It also ships a local
`sorafs_cli governance dag` operator surface for offline archive inspection,
validation, export, signed block/head snapshot verification, and local CARv2
segment emission. Local mirror index build/query tooling is also available for
signed block/head snapshots, local signed-head rebuild can recover a head
manifest from an existing block snapshot, and local checkpoint metadata can bind
verified heads to optional CAR and mirror-index artifacts for handoff.
Checkpoint verification and recovery can replay those local bindings and rebuild
a local mirror index independently of the network publisher. In an embedded
node, Torii receives the supervised service's authenticated mirror-read
capability before the first `NodeHandle` clone is shared; it does not open a
mirror filename as a second authority. Filesystem governance publishers commit
one canonical typed two-slot publication envelope as runtime artifacts are
materialized. Its one-to-one `publish_index` and `car_queue` sections expose the
deterministic publication feed and fully assembled per-publication CARv2
segments for the canonical `.to` payload, JSON mirror, and BLAKE3 sidecars.
Both sections become visible in one typed-store generation. Torii obtains a
path-free snapshot from `NodeHandle` and exposes read-only publish-index, queue,
and segment lookup
endpoints over that same generation without requiring the optional RocksDB/IPLD
backend. When configured with a
publisher peer ID, opaque signer handle, canonical Ed25519 public key, and a
matching deployment-injected runtime signer, the filesystem publisher also
appends supported published payloads to a local signed
`runtime-dag/` chain with `GovernanceDagBlockV1` blocks,
`GovernanceDagHeadV1` head bytes, and a
`sorafs.governance_dag.runtime_signed_index.v1` lookup index. The canonical head
and index are committed together in the authenticated runtime two-slot store;
Torii obtains both bytes plus their sealed producer-checkpoint identity through
`NodeHandle`, then exposes read-only runtime index/head/block/node/digest/kind
queries. Publish-index, CAR queue, and runtime digest/kind lookup responses keep
full match counts visible while bounding returned `entries`, `segments`, or
`blocks` arrays through `limit` (default 50, max 500).

Every mutable-authority response identifies its opaque typed source with
`source`, `source_generation`, and `source_record_blake3`; mirror and runtime
responses additionally carry `source_checkpoint_generation` and
`source_checkpoint_revision`. Host authority paths and `head_path` are not part
of the public contract. Conditional ETags bind the complete typed-store record
and, where present, the sealed checkpoint identity, so changed authentication
metadata cannot be hidden by a stale `304 Not Modified` response.

Filesystem publication objects are immutable and use only composite identities.
The encoded and JSON sources live at
`publication-sources/<payload_kind>/<source_pair_id>/payload.{to,json}`, where
`payload_kind` is canonical lowercase ASCII and `source_pair_id` binds that
kind plus both exact byte lengths and BLAKE3 digests. CAR, plan, and manifest objects live at
`car-segments/<position>_<source_pair_id>.{car,plan.json,json}`. Validators
derive every path from the committed identity, reject cross-entry path or
archive-digest aliasing, and include each digest sidecar in the same ownership
map. The retained segment manifest has one shared 128 KiB producer/readback
ceiling. Compact scalar labels are limited to 64 entries, canonical 128-byte
keys, 4 KiB string values, and 64 KiB total JSON.

The producer constructs and serializes the complete bounded successor envelope
and CAR segment in memory before creating any immutable object. It then writes
the exact source/CAR objects create-only and replaces the single authority
envelope last. A byte-identical duplicate verifies its exact source and CAR
objects but does not rewrite the envelope, advance its generation, or refresh a
timestamp; divergent immutable bytes fail closed. If an object write or the
final envelope swap fails, descriptor-rooted reconciliation rereads whichever
valid envelope is actually visible and removes only the single bounded batch of
canonical, unreferenced objects. Startup performs the same bounded recovery,
removes a bounded exact-name atomic temporary for each canonical target plus
empty identity directories, and rejects unknown names, excess orphan batches,
missing committed objects, links, and reparse points. Initialization first
publishes an explicit generation-zero authority and then a durable marker. Once
that marker exists, a missing authority is never interpreted as an empty root
and recovery must not delete retained history. Startup deterministically
rebuilds each segment from its authority-bound sources and exact-compares every
source, digest sidecar, CAR, plan, and manifest before admitting the publisher.
This detects in-place corruption even when a substituted sidecar matches the
substituted file.

The canonical binary payload exists only as `payload.to`; JSON sidecars do not
carry a redundant base64 copy. Reputation snapshots can reach the full 64 MiB
encoded V1 ceiling, so their JSON sidecar is a bounded metadata projection
rather than a second structured copy of the snapshot. The source-pair identity
still binds the exact metadata JSON bytes alongside the canonical payload.

Caller-supplied runtime payloads also carry server-derived
`GovernanceDagSubmissionProvenanceV1`. The canonical universal account ID and
exact Torii ingress origin participate in the log-node CID and publisher
signature preimages; runtime-index copies are checked against the signed node.
Appeal-finance reports and weekly rollups require matching provenance.
Proof-token issuances and transparency-ledger publications preserve matching
provenance when admitted through an authenticated route, while their trusted
in-process producer APIs are represented by its absence and remain attested by
the node signer. Other internally derived records, including settlement
receipts, reject caller provenance. Only the first-release provenance-bearing
runtime DAG schema is admitted.

The node derives `publisher_account_id` from the typed canonical `AccountId`;
it is never accepted from the request body. The manifest layer independently
bounds that UTF-8 display and rejects whitespace/control characters, but does
not pretend that this lower-level crate re-parses I105 account semantics.

V1 now separates binary schema ceilings from mutable JSON state: canonical
producer source payloads are capped at 64 MiB, node/block/head signing and CID
payloads at 128 MiB, and a complete canonical block at 128 MiB plus a checked
64 KiB signature/envelope allowance. The filesystem producer exercises the
exact canonical source serialization through an allocation-free counting sink
before writing source, latest, or JSON artifacts, rejects any supplied-length
substitution, and proves the parent-bearing node and block upper bounds with
checked fixed-envelope arithmetic. It no longer clones the source payload or
allocates dummy node/block/CID frames during preflight. Block and source readers
use their respective binary limits;
runtime indexes, JSON mirrors, queues, and signed heads retain the independent
64 MiB mutable-state limit. The public service's `max_request_bytes` default
and minimum are `134283264`, so a block admitted by the canonical schema cannot
be rejected only by the service transport setting. Per-variant semantic
collection/string ceilings are enforced before nested validation.

The `sorafs_governance_dag` service implementation, exposed through the public
`run_governance_dag_service` library launcher, is the always-on production
service. It loads
policy from `iroha_config` and accepts a local signed DAG only when the
producer's separate sealed checkpoint authenticates the exact canonical root,
signer/store qualifications, block count, head, and index digests and no
producer write-ahead intent is active. The service performs this check on both
sides of its bounded source read so a concurrent producer cannot expose source
state before its checkpoint CAS commits.

Objects are published through one deterministic Kubo UnixFS profile. The
service uploads and recursively pins every new block plus the signed head, then
reads each object back against its locally derived CID. Files up to 1 MiB use a
raw CIDv1 SHA-256 block; larger files use fixed 1 MiB raw leaves
under one canonical DAG-PB UnixFS file root, with at most 1,024 links. The Kubo
request fixes CID version, hash, chunker, raw leaves, link limit, directory
wrapping, and trickle behavior. The service derives the expected root locally,
rejects any different Kubo result, pins the object, and reads the exact bytes
back. Head publication uses authenticated HTTP compare-and-swap or IPNS
resolve/publish/readback, selected by the closed V1 service configuration; both
paths authenticate the exact qualified endpoint and verify the published head.

A service-specific authenticated checkpoint and write-ahead publish intent occupy
distinct slots in the deployment-injected monotonic store. The typed mirror
candidate binds the sealed intent digest before publication; restart either
continues that exact intent or reconstructs byte-identical mirror JSON from the
authenticated source and checkpoint. V1 mirror retention is protocol-fixed at
65,536 source blocks and 512 MiB of canonical source-block bytes, whichever
limit is reached first; node-local retention knobs are rejected. Before the
public-head CAS, the service verifies or repairs every retained block and head
object and requires repaired uploads to produce the same locally derived CID.
The first steady-state pass performs a full head and retained-object audit.
Later polls revalidate the head and rotate through retained blocks under the
64-entry/16 MiB audit budget, always checking at least the selected first block.
If a pin or object disappears after CAS, these audits restore it from the
authenticated bytes and require the same deterministic CID before readiness.

Every outbound control-plane request is authenticated through a rotation-aware
runtime provider selected by an opaque configuration handle. For each Kubo and
signed-head endpoint, configuration derives an exact
`GovernanceDagRequestIngressBindingV1` over scope, normalized endpoint digest,
Ed25519 key, body ceiling, and timing policy. The provider's live
`GovernanceDagRequestAuthenticator::ingress_qualification` must return a
matching `GovernanceDagRequestIngressQualificationV1`, including provider,
receiver-policy, replay-namespace, and replica-set identities. The only V1
postures are
`GovernanceDagRequestIngressEnforcementV1::ExclusiveAuthenticatedReceiver` and
`GovernanceDagRequestReplayPostureV1::SharedSealedAtomicConsumeUntilExpiry`.
`GovernanceDagHttpRequestReceiverV1` consumes one finalized typed HTTP request,
requires exactly one endpoint-matching `Host` for HTTP/1.x, validates any URI
authority against the same qualified origin, and rejects ambiguous framing or
unsigned semantic headers. It then verifies the exact eight authentication
headers, canonical request, freshness, signature, and one atomic nonce consume
through `GovernanceDagRequestAuthenticationReplayStoreV1`. The resulting
backend-dispatch capability has the authentication headers removed and its URI
normalized to origin form, so a downstream proxy cannot reinterpret an
absolute request target. The in-memory replay cache is an isolated-test utility
and is not production qualification evidence.
The standard outbound path already consumes verified nonces through separate
sealed IPFS and signed-head slots; these slots preserve the authenticated
request namespace across process restart.
Immediately before signing and again after final request construction, the
client requires the signed-head URL to equal its qualified endpoint exactly;
Kubo requests must retain the same scheme, host, effective port, and normalized
base-path prefix. A future caller therefore cannot reuse a qualified signer for
a cross-origin or sibling-path request.

Signer, authenticator, and checkpoint providers return non-zero public policy
revisions and digests. Startup pins those qualifications before opening state
or resolving endpoints, and every operation rejects identity drift. The public
`run_governance_dag_service_with_runtime_registry` launcher resolves one exact
provider set through a deployment-owned registry; missing, stale, incomplete,
substituted, or test-marked results fail with typed redacted errors before state
access. The stock `irohad` launcher resolves the embedded signer, sealed CAS
store, and both request authenticators through its local broker boundary before
Sumeragi startup.

Preparation also returns a service-owned `GovernanceDagMirrorReadHandleV1`.
Each read reopens the typed mirror and sealed checkpoint, verifies both retained
roots and all provider bindings, and checks one shared readiness epoch before
and after the read. Reconciliation failure and runner drop withdraw that epoch,
so retained clones cannot serve a cached generation after service liveness is
lost. The process also serves a bounded public mirror, head, block, node,
checkpoint, health, and Prometheus surface, with readiness withdrawn whenever
the supervised service loses its authenticated authority.

SF-12 deployment qualification still requires the supervised broker and
genuine external software signer, sealed store, exclusive authenticated Kubo/head receivers with one
shared sealed atomic replay namespace, package integration, any scale-motivated
mirror backend, and captured multi-instance rollout evidence.

Implemented foundations include:
- `GovernanceLogNodeV1`, `GovernanceLogPayloadV1`,
  `GovernanceLogSignatureV1`, and validation errors in
  `crates/sorafs_manifest/src/governance.rs`.
- Public `GovernanceDagBlockV1` and `GovernanceDagHeadV1` schemas with
  deterministic node-CID/block-CID derivation, block/head signing payloads,
  parent-chain validation, and signed-head-to-chain binding helpers.
- Ed25519 and ML-DSA/Dilithium3 publisher-signature verification over canonical Norito signing bytes.
- `sorafs-validate governance --node <path>`, `--block <path>`, and `--head
  <path> --block <path>...`, plus `sorafs-validate sign --kind governance`.
- Reference SDK validation for Norito-encoded governance log nodes, governance
  DAG blocks, and signed governance DAG heads. The C FFI surface currently
  exposes governance log-node validation; downstream block/head FFI wrappers
  remain SDK distribution work.
- Governance fixtures under `fixtures/sorafs_manifest/governance/` and PoR fixture generation that emits governance nodes.
- Embedded `NodeHandle` Governance DAG publication in `crates/sorafs_node` for
  finalized native settlement, repair, GC, reconciliation, reputation, moderation
  ballot lifecycle, appeal finance, orderbook settlement, and canonical PoR
  challenge/report evidence. The filesystem implementation and its unsigned
  assembly constructor are crate-private; production startup requires the
  configured publisher identity plus matching runtime-injected signer and
  config-pinned sealed-CAS checkpoint store. Before the first append the
  producer seals an empty-root identity. Each later append seals an exact
  producer-specific intent containing the direct successor checkpoint,
  predecessor revision, and exact block/head/index length-and-digest
  descriptors before changing the filesystem. The block, head, and full index
  are first written and read back through one bounded local staging root; the
  sealed intent never embeds those artifact bytes and is independently capped
  at 64 KiB. Restart authenticates every staged artifact against the sealed
  descriptors before reconciling the intent and auditing the live root. The audit bounds
  entry and aggregate bytes and verifies canonical JSON/Norito, sidecars,
  source payloads, signatures, CIDs, lineage, reverse maps, and head/index
  agreement.
  Each successful filesystem publish now atomically updates the nested
  `sorafs.governance_dag.local_publish_index.v1` and
  `sorafs.governance_dag.local_car_queue.v1` sections of one typed two-slot
  publication authority. The index contains root-relative artifact paths,
  BLAKE3 digests, payload-kind counts, digest lookup maps, and compact labels;
  its corresponding queue entry references a fully qualified CARv2 segment
  under its position/source-pair composite path in `car-segments/`. Sources use
  their payload-kind/source-pair composite path under `publication-sources/`.
  The complete successor state is size-checked before object creation, and the
  two sections are validated as an exact one-to-one projection before the
  authority generation is committed. Bounded descriptor-rooted recovery removes only
  canonical unreferenced objects and exact canonical-target atomic temporaries
  from an interrupted publication. With
  `sorafs.storage.governance_dag_publisher_peer_id`,
  `sorafs.storage.governance_dag_signer_handle`,
  `sorafs.storage.governance_dag_signer_revision`,
  `sorafs.storage.governance_dag_signer_policy_digest_hex`, and
  `sorafs.storage.governance_dag_publisher_public_key_hex` configured as one
  all-or-nothing public binding, plus the checkpoint-store handle, revision,
  and policy digest under
  `[sorafs.storage.governance_dag_service]`, and matching
  `GovernanceDagRuntimeSigner` and `GovernanceDagSealedCheckpointStore`
  implementations injected through `NodeRuntimeDeps`,
  deal settlements, reputation snapshots, moderation ballot lifecycle events,
  appeal finance reports, weekly rollups, appeal finance settlement receipts,
  and orderbook settlement receipts also append to the local signed runtime DAG;
  duplicate publishes are idempotent and malformed runtime DAG index state
  fails closed. A dormant binding without `governance_dag_dir` or one attached
  to disabled storage is rejected. Producer checkpoint-store state remains
  required when the public service is disabled; service checkpoint/intent
  state is not reused as producer authority. Provider construction rejects missing,
  stale, unqualified, or
  test-marked adapters; mismatched handles, peer identities, public keys,
  malformed/weak Ed25519 points; a revision or policy digest that differs from
  the exact configured qualification; provider identity/policy drift between
  two startup reads or around signing; invalid signatures; and signer outages
  without exposing provider diagnostics. Embedded-node startup completes both
  qualification reads and every exact comparison before opening any durable
  worker or publisher state. Signing authority is supplied only through the
  qualified runtime provider. A filesystem publisher holds an exclusive lock on its root
  for its full lifetime and serializes immutable-object creation plus typed
  publication and runtime-state updates as one in-process publication
  transaction. Publication state, including its `publish_index` and `car_queue`
  payloads, and runtime staging/committed head-index state use descriptor-rooted
  fixed-region two-slot stores with exact root/slot identity checks and
  trailer-last compare-and-swap commits. These names describe bounded payload
  sections, not mutable authority filenames. Atomic immutable-artifact creation
  synchronizes both the file and parent directory before acknowledgement; links,
  substituted roots or slots, ambiguous lineage, and corrupt newest committed
  records fail closed.
  Implicit signer or store rotation remains deliberately rejected. The explicit
  rotation path now appends a canonical Norito key-transition envelope
  independently signed by the outgoing and incoming runtime signers. Each
  envelope carries monotonic outgoing/incoming segment revisions and the digest
  of a transition body that binds the canonical producer root, exact
  predecessor sealed-checkpoint revision, current block/head digests,
  predecessor and successor index digests, both publisher identities and
  Ed25519 keys, both signer/store handles, revisions and policy digests, and the
  current archive head. The transition block count is the incoming segment's
  activation boundary. Recovery reconstructs the bounded archived-plus-live
  lineage, verifies every retained block under the authority active at its
  sequence, and binds the head to the actual tip segment. A rotation with no
  intervening block therefore retains the outgoing-signed head until the
  incoming authority appends its first block.
  The producer checkpoint generation advances independently for blocks,
  qualification transitions, and qualification archives, so same-store
  rotation cannot reuse a sealed generation.

  The active qualification journal is capped at 64 transitions. Compaction
  writes a canonical, current-signer-authenticated immutable archive of at most
  64 transitions, reads it and its digest sidecar back, then advances the exact
  archive digest through sealed monotonic CAS and reads that checkpoint back
  before pruning the live prefix. At most 64 linked archives are accepted.
  Restart finishes either a staged pre-CAS archive or a post-CAS/pre-prune
  archive idempotently. Canonical validation rejects tamper, fork, rollback,
  duplicate, truncation, trailing bytes, and provider-qualification
  substitution. This compacts provider-qualification history only: the current
  full DAG block index is not yet a release-complete retention design. The
  producer intent now binds that full index by length and BLAKE3 digest while
  the bytes remain in durable local staging, so sealed-CAS decoding is
  independent of index history length. A bounded authenticated block-prefix
  retention protocol remains required for long-running deployments. On the filesystem-flag-qualified
  Linux, Android, macOS, iOS, FreeBSD, OpenBSD, NetBSD, and DragonFly targets,
  producer, service source, service state, and mirror roots now require their
  role-specific owner/mode policy, an exact canonical lexical path without
  symlink components, and a retained `O_DIRECTORY|O_NOFOLLOW` handle for the
  root and every ancestor. Other Unix targets, and Android architectures outside
  arm, aarch64, x86, x86_64, and riscv64, fail compilation until their native
  flags and target tests are qualified.
  Device, inode, owner, mode, and effective-UID identity are revalidated around
  publication, recovery, source reads, state changes, and mirror operations;
  sticky writable ancestors are accepted only for a trusted owner. This catches
  cross-UID pathname replacement under the ordinary Unix mode model. Producer
  and service descendants are now opened, enumerated, promoted, recovered, and
  locked relative to retained directory handles on Linux, macOS, and Windows,
  with no-follow semantics and exact object-identity rechecks. Linux requires
  two identical bounded descriptor-xattr snapshots and rejects protected
  extended ACL namespaces; macOS requires two identical bounded descriptor ACL
  snapshots and rejects untrusted mutation grants. Windows pins the root owner
  SID, validates two identical self-relative security descriptors with bounded
  SID/ACE traversal, rejects untrusted mutation grants, and retains file IDs
  through crash recovery and atomic-temp cleanup. macOS configuration must use
  the physical canonical path (for example `/private/var/...`, not `/var/...`).
  The local filesystem sink also updates the Governance DAG backlog gauge from
  CAR queue pending segment counts and refreshes the local signed runtime-head
  age gauge when runtime DAG state is written or de-duplicated.
- `scripts/check_sorafs_governance_dag_rollout_evidence.py` provides the
  existing SF-12 rollout evidence checker framework for deployed Governance DAG
  promotion packets, and
  `scripts/run_sorafs_governance_dag_rollout_evidence.py` provides the matching
  reviewed evidence collection planner/runner. The checker exports its required
  top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the runner
  dry-run emits the checker-backed `evidence_contract` map for selected SF-12
  evidence kinds, and validates the schema-closed collection plan, required
  kinds, thresholds, external evidence map, evidence contract, and command steps
  before dry-run output or verifier execution. That validation now also rejects
  non-canonical nested required-kind, threshold, external-evidence,
  evidence-contract, and command-step shapes. Evidence payloads and nested
  dashboard route rows are schema-closed. Mirror datastore, checkpoint
  recovery, dashboard, observability, `publication_e2e`, and
  governance approval artifacts must carry the same
  `public_head_cid_hex` as a valid
  publisher-service artifact in the same bundle, so rollout evidence cannot mix
  mirror, recovery, dashboard, publication, or approval records from
  different signed DAG heads. Publisher-service artifacts must also carry
  `policy_digest_hex`, and governance approval artifacts must match that
  publisher policy digest plus its receiver-policy, replay-namespace, and
  replica-set qualification digests and the complete per-Kubo and per-signed-
  head ingress-binding digests before promotion. Zero authority digests are
  invalid. Every recognized bound artifact requires its publisher anchor even
  when that artifact is optional for a custom subset gate. Publisher evidence fixes
  the Kubo UnixFS and signed-HTTP CAS profiles; mirror/governance evidence fixes
  the V1 retention limits; recovery, observability, and dashboard evidence bind
  same-CID repair, rotating-audit, and fresh liveness-bound read guarantees.
  Public-head, policy, and ingress-qualification binding failures are recorded
  on the offending artifact before required-kind validity is computed. The
  retired IPNS evidence kind and collection flag are rejected rather than
  treated as compatibility aliases.
- Typed appeal finance transparency rollups: `sorafs_manifest` validates
  `SoraFsAppealFinanceWeeklyRollupV1` payloads and can aggregate validated
  finance reports into deterministic weekly dashboard rows for the filesystem
  publisher and signed runtime DAG.
- Torii PoR publishes validated `PorChallengePublicationV1` envelopes and
  validated weekly reports through the embedded node's durable signed outbox.
  Private coordinator, drand, and provider-VRF state is derived under the
  single `sorafs.por.state_dir`; PoR has no governance-output directory
  or local unsigned publisher.
- Taikai cache governance bundle generation via `cargo xtask sorafs-taikai-cache-bundle`.
- Local operator commands:
  - `sorafs_cli governance dag list --root <dir>` inventories local `.to`
    artifacts, reports BLAKE3 sidecar state, identifies
    `GovernanceLogNodeV1` payloads, and emits table or JSON output.
  - `sorafs_cli governance dag show --node <path>` inspects one local node and
    includes the SF-11 `ValidationOutcomeV1` reference verdict in JSON mode.
  - `sorafs_cli governance dag verify --root <dir>` validates all discovered
    governance nodes, publisher signatures, sidecars, optional expected heads,
    and optional parent linkage with `--require-chain`.
  - `sorafs_cli governance dag export --root <dir> --out <dir>` writes a
    normalized local snapshot containing validated governance nodes, regenerated
    `.to.blake3` sidecars, and a `manifest.json` verification record.
  - `sorafs_cli governance dag build --root <dir> --out <dir>
    --publisher-peer-id <id> (--key-hex <hex> | --key <path>)` builds a local
    public block/head snapshot from validated governance-node archives by
    ordering nodes deterministically, writing signed `GovernanceDagBlockV1`
    blocks, writing a signed `GovernanceDagHeadV1`, and regenerating BLAKE3
    sidecars plus `manifest.json`. Signing seed material is runtime-only and is
    not persisted in the output manifest. With `--car-out <path>`, the command
    also emits a deterministic CARv2 segment containing the generated
    `head.to`, `blocks/*.to`, and sidecar payloads, with optional
    `--car-plan-out <path>` chunk-plan metadata.
  - `sorafs_cli governance dag verify-build --root <dir>` validates a local
    builder output root by decoding `head.to` and `blocks/*.to`, checking
    sidecars, enforcing an optional expected head CID, and replaying
    `validate_governance_dag_head_against_chain_v1` over the decoded blocks.
  - `sorafs_cli governance dag rebuild-head --root <dir> --head-out <path>
    --publisher-peer-id <id> (--key-hex <hex> | --key <path>)` regenerates a
    signed `GovernanceDagHeadV1` from existing validated `blocks/*.to` payloads
    for local recovery and checkpoint handoff workflows.
  - `sorafs_cli governance dag checkpoint --root <dir> --out <path>` writes a
    local `sorafs.governance_dag.checkpoint.v1` JSON handoff manifest after
    verifying the signed snapshot. It records the signed head digest and
    advertised head block, optionally hashes a supplied CARv2 segment, and
    checks that a supplied local mirror index advertises the same head and block
    count.
  - `sorafs_cli governance dag checkpoint-verify --checkpoint <path>` verifies
    a local checkpoint handoff manifest by replaying snapshot verification,
    checking the recorded head digest, and checking optional CARv2 and
    mirror-index artifact digests. Operators can override recorded local paths
    with `--root`, `--car`, and `--mirror-index` when testing recovered
    artifacts.
  - `sorafs_cli governance dag checkpoint-recover --checkpoint <path> --root
    <dir> --out <path>` verifies a checkpoint against a recovered signed
    snapshot and optional CARv2 artifact, then rebuilds a local mirror index
    only if the recovery inputs pass.
  - `sorafs_cli governance dag mirror-build --root <dir> --out <path>` writes a
    deterministic local mirror index for a verified signed snapshot, keyed by
    block CID and governance-node CID.
  - `sorafs_cli governance dag mirror-query --index <path>` queries the local
    mirror index by head, block CID, or node CID in table or JSON form.
- Torii local Governance DAG mirror API:
  - `GET /v1/sorafs/governance/dag/dashboard` summarizes the service-owned
    authenticated mirror snapshot with signed-head metadata, block counts,
    payload-kind counts, sequence bounds, timestamp bounds, typed-store and
    checkpoint identities, a BLAKE3 digest, and ETag/cache headers.
  - `GET /v1/sorafs/governance/dag/head` returns the signed-head portion of the
    same authenticated typed mirror snapshot.
  - `GET /v1/sorafs/governance/dag/blocks/{block_cid_hex}` and
    `/v1/sorafs/governance/dag/nodes/{node_cid_hex}` look up the indexed block
    by block CID or governance-node CID. Each block exposes nullable submission
    account/origin fields copied from the signed node; the publisher rejects a
    runtime-index copy that disagrees with those signed bytes. The API reads
    only the installed mirror-read capability and descriptor-rooted immutable
    block artifacts. A configured root without that capability returns no
    mirror; malformed, substituted, or checkpoint-incoherent snapshots fail
    closed. The installed typed capability is the sole mirror authority.
  - `GET /v1/sorafs/governance/dag/publish-index?limit=N` returns the
    runtime-local publication feed from the `publish_index` payload section of
    one typed publication-authority snapshot,
    including payload-kind counts, total entry counts, and a `limit`-bounded
    embedded entry list.
  - `GET /v1/sorafs/governance/dag/publish-index/digests/{encoded_blake3_hex}`
    and `/v1/sorafs/governance/dag/publish-index/kinds/{payload_kind}` query
    that local feed by encoded payload digest or payload kind. The handlers
    validate lookup keys, use route/key/limit-specific ETags, report total and returned
    counts, bound the returned `entries` array with `limit` (default 50, max
    500), and fail closed on missing, malformed, or unsupported publish indexes.
  - `GET /v1/sorafs/governance/dag/car-queue` returns the runtime-local CAR
    segment queue from the same typed generation's `car_queue` section, including
    assembled/pending counts and the full local queue.
  - `GET /v1/sorafs/governance/dag/car-queue/digests/{encoded_blake3_hex}`,
    `/v1/sorafs/governance/dag/car-queue/kinds/{payload_kind}`, and
    `/v1/sorafs/governance/dag/car-queue/archives/{car_archive_blake3_hex}`
    query assembled local segments by encoded payload digest, payload kind, or
    CAR archive digest. Digest/kind handlers validate lookup keys, use
    route/key/limit-specific ETags
    revalidation, report total and returned counts, bound the returned
    `segments` array with `limit` (default 50, max 500), and fail closed on
    missing, malformed, or unsupported CAR queues. Archive lookup verifies the
    retained source, sidecars, plan, manifest, and canonical CAR before it may
    answer either `200` or `304`; a matching stale ETag cannot mask a missing or
    substituted archive.
  - `GET /v1/sorafs/governance/dag/runtime` summarizes the local signed runtime
    DAG index from one NodeHandle-authenticated head/index generation, including
    publisher identity, head metadata, block counts, payload-kind counts, and
    the full local index.
  - `GET /v1/sorafs/governance/dag/runtime/head` returns the latest runtime head
    metadata and latest indexed block.
  - `GET /v1/sorafs/governance/dag/runtime/blocks/{block_cid_hex}` and
    `/v1/sorafs/governance/dag/runtime/nodes/{node_cid_hex}` look up runtime
    block entries by block CID or governance-node CID.
  - `GET /v1/sorafs/governance/dag/runtime/digests/{encoded_blake3_hex}` and
    `/v1/sorafs/governance/dag/runtime/kinds/{payload_kind}` query runtime
    block entries by encoded payload digest or payload kind. The handlers
    validate lookup keys, support ETag revalidation, report total and returned
    counts, bound the returned `blocks` array with `limit` (default 50, max
    500), and fail closed on missing, malformed, or unsupported runtime indexes.
  - Every conditional route finishes lookup, route-specific metadata
    validation, bounded projection, and fallible serialization before matching
    `If-None-Match`. Entity-tag opaque values compare case-sensitively; weak
    tags use the exact `W/` syntax and malformed quoting never aliases a valid
    tag. The base tag commits the typed record identity and the authenticated
    mirror/runtime checkpoint identity when applicable. A matching
    representation ETag therefore cannot turn an unconditional
    `404`, `409`, or `500` into `304`.
- Local publication telemetry: `sorafs_governance_dag_publish_total`,
  `sorafs_governance_dag_published_bytes_total`,
  `sorafs_governance_dag_last_publish_timestamp_seconds`,
  `sorafs_governance_dag_backlog`, and
  `sorafs_governance_dag_head_age_seconds`, plus the checked-in
  `dashboards/grafana/sorafs_governance_dag.json` dashboard and
  `dashboards/alerts/sorafs_governance_dag_rules.yml` alert pack.

Still outstanding:
- Package and supervise `sorafs_governance_dag` in the supported deployment
  bundles, implement and provision the independently administered external software signer,
  rotation-aware Kubo/head authenticators, and sealed monotonic checkpoint-store
  adapters that derive their qualification revision/digest from the external
  control plane and reject revoked/stale policy internally. Their live
  `ingress_qualification` probes must bind the exact configured endpoints to an
  exclusive `GovernanceDagHttpRequestReceiverV1` ingress backed by one sealed
  atomic replay namespace shared by the complete replica set. The deployment
  must provide the receiver-side sealed cross-replica replay adapter. The generic
  packaged binary now lives with `irohad`, requires a canonical public chain ID,
  projects only the Kubo authenticator, signed-head authenticator, and sealed
  checkpoint-store roles, and resolves them through the stock fixed local
  runtime-provider broker. Missing, substituted, stale, revoked, test-marked,
  or incomplete broker providers fail before state access. Deployment packages
  must still supply and supervise the audited broker server and concrete
  adapters; no credential, private-key, environment, or provider-file fallback
  exists. The standalone launcher accepts one bounded, self-contained service
  TOML; unresolved `extends` is rejected rather than silently reading an
  incomplete overlay.
- Define the bounded protocol needed to prune the authenticated DAG block
  prefix.
- Decide from measured production scale whether the bounded authenticated JSON
  mirror is sufficient; add a RocksDB/IPLD backend only if the governed
  deployment profile requires it.
- Publish operator-facing live-head/checkpoint convenience commands over the
  already shipped service API where they materially improve operations.
- Capture Kubo, signed-head CAS, dashboard, alert-routing, and disaster-recovery
  evidence that passes the reconciled SF-12 rollout gate. The checked-in
  checker, planner, canary builder, examples, and tests already enforce the
  signed-HTTP-only head and exact ingress-qualification contract.

## Goals & Scope
- Capture governance artifacts such as adverts, replication orders, PoR events, repairs, settlements, reputation snapshots, verdicts, and reports in append-only evidence.
- Preserve deterministic validation through Norito payloads and publisher signatures.
- Provide a verifiable public DAG head so operators, SDKs, and auditors can retrieve current governance state.
- Keep local filesystem evidence byte-identical to objects admitted by the
  fixed Kubo UnixFS profile while treating authenticated signed-HTTP heads and
  sealed checkpoints as the durable cross-operator recovery boundary.

## Current Data Model
The shipped canonical governance log node is `GovernanceLogNodeV1`. Its fields
are:

```norito
struct GovernanceLogNodeV1 {
    version: u8,
    node_cid: Vec<u8>,
    prev_cid: Option<Vec<u8>>,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    submission_provenance: Option<GovernanceDagSubmissionProvenanceV1>,
    payload: GovernanceLogPayloadV1,
    publisher_signature: GovernanceLogSignatureV1,
}
```

`GovernanceLogPayloadV1` currently carries:
- `ProviderAdvert`
- `ReplicationOrder`
- `PorChallenge`
- `PorProof`
- `AuditVerdict`
- `DealSettlement`
- `ReputationSnapshot`
- `ModerationBallotEvent`
- `AppealFinanceReport`
- `AppealFinanceWeeklyRollup`
- `AppealFinanceSettlementReceipt`
- `OrderbookSettlementReceipt`

`GovernanceLogSignatureV1` stores the algorithm, public key, and raw signature.
Validation rejects unsupported versions, empty node CIDs, empty previous CIDs,
missing publisher peer IDs, malformed signatures, invalid nested payloads,
missing finance provenance, mismatched provenance on authenticated-ingress
payloads, and provenance attached to payloads with no external producer.
Signature verification covers canonical Norito bytes, including submission
provenance, that exclude only `publisher_signature`, so signers and verifiers
operate on stable payload bytes.

The shipped public DAG block/head surface wraps those log nodes without changing
their payload semantics:

```norito
struct GovernanceDagBlockV1 {
    version: u8,
    block_cid: Vec<u8>,
    prev_block_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
    publisher_peer_id: Vec<u8>,
    node: GovernanceLogNodeV1,
    block_signature: GovernanceLogSignatureV1,
}

struct GovernanceDagHeadV1 {
    version: u8,
    head_block_cid: Vec<u8>,
    block_count: u64,
    generated_at: u64,
    publisher_peer_id: Vec<u8>,
    checkpoint_cid: Option<Vec<u8>>,
    head_signature: GovernanceLogSignatureV1,
}
```

`governance_dag_block_cid_v1` derives BLAKE3-256 block CID bytes from the
canonical Norito block payload under the
`sorafs.governance_dag.block.cid.v1` domain. Block signatures cover the block
CID and canonical payload while excluding `block_signature`; head signatures
cover the advertised head block, block count, timestamp, publisher, and
checkpoint while excluding `head_signature`. `validate_governance_dag_chain_v1`
rejects empty chains, malformed blocks, duplicate block CIDs, missing parents,
sequence gaps, timestamp regressions, multiple heads, and expected-head drift.
`validate_governance_dag_head_against_chain_v1` also verifies the signed head
manifest and block-count binding.

## Current Publishing Paths
Local filesystem publishing is available for evidence that downstream jobs can
archive or ingest:

- `crates/sorafs_node/src/governance.rs` writes deal settlement, repair audit,
  repair slash, GC audit, reconciliation, and reputation snapshot artifacts with
  digest sidecars.
- `crates/iroha_torii/src/sorafs/por.rs` writes PoR challenge and weekly report
  JSON artifacts for the Torii PoR runtime.
- Configuration exposes governance output directories through the SoraFS storage
  and PoR configuration trees.

These filesystem publishers are local materialization hooks. The separate
`sorafs_governance_dag` service consumes their verified signed runtime chain,
publishes content through the fixed Kubo UnixFS profile, updates the signed-HTTP
head through strong-ETag CAS, and exposes bounded
historical queries. The optional runtime DAG signer writes local signed
block/head bytes only for payload variants already represented by
`GovernanceLogPayloadV1`.

## Target Architecture
| Component | Responsibility | Current workspace status |
|-----------|----------------|--------------------------|
| Ingest service | Subscribe to Torii/governance evidence, load full payloads, and verify signatures. | Shipped: filesystem publishers materialize typed payloads and a signed runtime chain; `sorafs_governance_dag` revalidates the bounded source snapshot and rejects rollback, forks, unsupported payload kinds, or signature drift before publication. |
| DAG builder | Wrap validated payloads into DAG blocks, compute parent linkage, and assemble CAR segments. | Shipped: local builders, CARv2 segments, signed runtime blocks/heads, source-chain validation, and deterministic replay are implemented. |
| Publisher | Publish canonical blocks and a signed public head. | Shipped: locally derived fixed-profile Kubo CIDs, verified add/pin/list/cat, pre-CAS object repair, and signed-HTTP strong-ETag compare-and-swap with SSRF controls, bounded responses, and sealed intent/checkpoint recovery. |
| Mirror datastore | Maintain queryable block and payload indexes. | Shipped as a protocol-retained authenticated typed JSON mirror derived from the sealed checkpoint and source. RocksDB/IPLD remains an optional scale backend rather than a prerequisite for protocol correctness. |
| Dashboard/API backend | Serve governance history, block lookup, snapshots, and proof queries. | Shipped: the always-on service exposes bounded mirror/head/block/node/checkpoint, health, and metrics routes; Torii also exposes local pre-publication indexes. |
| Operator CLI | Inspect heads, list/fetch blocks, export snapshots, verify chains, and rebuild heads. | Local archive list/show/verify/export/build/verify-build/rebuild-head/checkpoint/checkpoint-verify/checkpoint-recover/mirror-build/mirror-query is shipped for `.to` governance nodes and block/head snapshots. Direct convenience wrappers for the public service can be added without changing the protocol boundary. |

## Target Publishing Workflow
1. Ingest validates a Norito payload and deduplicates it by digest.
2. The builder links the payload to the current head, computes a deterministic block CID, and writes a CAR segment.
3. A validator re-derives the CID, checks parent availability, verifies the payload and publisher signature, and quarantines invalid blocks.
4. The publisher verifies or repairs every fixed-profile Kubo object, then
   updates the signed-HTTP head through strong-ETag compare-and-swap.
5. Torii or the dashboard backend announces the new head for subscribers.
6. Clients resolve the head, verify signatures and digests, and replay blocks back to a trusted checkpoint.

The local filesystem publisher performs steps 1-2 for supported payloads when a
matching runtime DAG signer is injected. `sorafs_governance_dag` performs steps 3-5,
and the public mirror/checkpoint API supplies step 6. Production rollout still
has to prove this workflow against the governed public deployment.

## Security & Verification Requirements
- All payloads must remain Norito-first; JSON should be a presentation format only.
- Unknown payload or schema versions must fail closed.
- Head and block signatures must be deterministic and replayable.
- The embedded publisher accepts only an injected signer whose opaque handle,
  peer identity, and canonical non-weak Ed25519 public key exactly match
  `iroha_config`; it pins the provider's non-zero public policy
  revision/digest before durable startup and rechecks them for every signature.
  Private signing material has no configuration or file path.
- The always-on service accepts only injected rotation-aware authenticators and
  a sealed monotonic CAS checkpoint store whose opaque handles exactly match
  `iroha_config`. Their non-zero qualification revisions/digests are pinned
  before mutable state or endpoint access and rechecked for every authenticated
  request and sealed-state operation; exact record revisions and canonical
  payload digests remain enforced by the CAS layer. Configuration carries only
  opaque handles and public policy identities, never key or token paths.
- Both public control-plane endpoints must return a live
  `GovernanceDagRequestIngressQualificationV1` matching the configured
  `GovernanceDagRequestIngressBindingV1`. Qualification requires the exact
  endpoint, Ed25519 key, body and timing limits, an exclusive authenticated
  receiver, and one shared sealed atomic replay namespace covering every
  ingress replica through envelope expiry.
- Mirror retention is the fixed V1 suffix of at most 65,536 blocks and 512 MiB
  of canonical source bytes. Each checkpoint generation must pass a full first
  audit before readiness; steady polls recheck the signed head and rotate
  through the retained object set.
- Mirror reads use the installed liveness-bound capability and authenticate the
  typed mirror plus sealed checkpoint on every read. A failed reconciliation,
  changed readiness epoch, or stopped runner must return unavailable rather
  than serve the last cached snapshot.
- Consumers must verify payload validation status, publisher signature, parent linkage, and head signature before trusting a block.
- Public rollout must not persist runtime secrets such as publisher private keys or bearer tokens in repo files.

## Observability Requirements
Local publication metrics now cover filesystem-backed governance evidence
materialization:

- `sorafs_governance_dag_publish_total{payload_kind,result,sink}` counts
  publication attempts for settlement, repair, GC, reconciliation, and
  reputation evidence.
- `sorafs_governance_dag_published_bytes_total{payload_kind,sink}` records
  successfully written Norito payload bytes.
- `sorafs_governance_dag_last_publish_timestamp_seconds{payload_kind,sink}`
  records the last successful local publication timestamp.
- `sorafs_governance_dag_backlog{sink}` reports the local CAR queue pending
  segment count for the filesystem sink when the authoritative publication
  envelope is built or refreshed.
- `sorafs_governance_dag_head_age_seconds{sink}` reports the signed
  runtime DAG head age for the filesystem sink when runtime DAG state is
  written or refreshed.
- The always-on service exports Kubo publish success/failure and byte counters,
  backlog, signed-head age, Kubo pin lag, last successful public-head update
  time, validation failures, and mirror coherence. Any failed reconciliation
  latches the drift gauge and withdraws public readiness; only a complete
  checkpoint-coherent reconciliation clears it.
- `dashboards/grafana/sorafs_governance_dag.json` visualizes local publication
  outcomes, published bytes, backlog, head age, and publish age.
- `dashboards/alerts/sorafs_governance_dag_rules.yml` alerts on publication and
  validation failures, Kubo pin lag, mirror drift, backlog, stale heads, and
  missing recent publications. Its publication-failure annotation is valid for
  both service-level and payload-kind-labelled series.

Still-required rollout evidence must demonstrate block count by payload kind,
publish duration/SLOs, validation failures, and alert delivery alongside the
already exported signed-head, queue, pin-lag, mirror-drift, and public-head
signals.

Required live alerts include no new public block for the configured SLA,
validation failure, pin lag, signed-head update failure, and mirror/index drift.

## Testing Strategy
Implemented coverage exists for governance log validation and signature
verification in `sorafs_manifest`, reference validation of governance DAG blocks
and signed heads, CLI fixture validation paths, and focused `sorafs_cli
governance dag` tests for fixture inventory/show, expected-head verification,
mismatch rejection, normalized local export sidecars, and signed local
block/head snapshot building plus CAR segment emission and valid/tampered
build-output verification, and local mirror index build/query coverage.
Signed-head rebuild coverage verifies deterministic regeneration from existing
blocks and refusal to write a head for tampered block snapshots. Checkpoint
coverage verifies local handoff manifest generation with CAR and mirror-index
digests, rejects unsupported mirror-index schemas, accepts verified checkpoint
manifests with explicit recovered-artifact paths, and rejects tampered CAR
artifact drift. Recovery coverage verifies mirror-index rebuild from a
checkpoint after the original mirror index is removed and refuses to write a
recovered index when the CAR binding is tampered. Focused Torii unit coverage
verifies mirror parsing and response projection, typed source/checkpoint
metadata, ETag revalidation, malformed and missing CID rejection through real
handlers, and fail-closed behavior when no mirror capability is installed. A
source regression also runs the publisher, installs its checkpoint-coherent
capability, exercises real Torii head/block `200` responses, removes committed
Kubo objects and observes deterministic repair, then verifies `503` after the
runner exits; executing that regression remains part of the outstanding focused
validation work. Focused `sorafs_node` coverage
verifies successful real-reader installation, visibility from clones created
before installation in both bootstrap and checkpoint-coherent states,
duplicate rejection, and wrong-root rejection without slot consumption. It
also verifies that an existing empty typed mirror is
install-ready at genesis and that a removed typed store fails readiness. The
same coverage verifies that filesystem governance
publishers atomically update the `publish_index` and `car_queue` sections of one
typed publication authority, populate
canonical lookup maps, derive source and CAR paths from the full composite
identity, and leave the authority bytes and generation unchanged when the exact
same artifact is republished. The same coverage verifies deterministic CAR
segment/plan/manifest emission, CAR sidecars, segment queue de-duplication,
byte-exact reuse, fail-closed immutable substitution, exact predecessor
preservation after a failed successor swap, bounded startup orphan reclamation,
and fail-closed rejection of malformed, missing, or cross-substituted
publication state. It also verifies
config-backed signed runtime DAG append for supported payloads, duplicate
publish idempotency, decoded head/block signature-chain validation with
`validate_governance_dag_head_against_chain_v1`, and fail-closed rejection of
malformed runtime DAG index state, including orderbook settlement receipt
publication. Focused Torii coverage verifies publish-index reads, digest
lookups, payload-kind lookups, `limit`-bounded returned entries, ETag
revalidation without cross-route/key cache collisions, malformed lookup
rejection, and missing lookup rejection over a
typed publication snapshot. Torii CAR queue coverage verifies local queue
reads, digest lookups, payload-kind lookups, `limit`-bounded returned segments,
CAR archive digest lookups, ETag revalidation only after actual archive
verification, malformed lookup rejection, and missing lookup rejection over a
typed publication snapshot. Torii runtime DAG
coverage verifies authenticated typed runtime-index reads, runtime head reads, block/node/
digest/payload-kind lookups, `limit`-bounded returned blocks, ETag
revalidation bound to checkpoint identity, malformed lookup rejection, missing
lookup rejection, and unsupported runtime index schema rejection. Focused `sorafs_node` helper
coverage verifies local CAR queue backlog counting and signed runtime-head age
saturation.

Required before rollout:
- Run the checked-in real-Kubo signed-head restart/tamper lane and archive its
  output. `.github/workflows/sorafs-governance-dag-kubo.yml` downloads the
  checksum-pinned Kubo `v0.42.0` release, rejects any runtime version mismatch,
  and compares Iroha's local CID derivation against Kubo at 1 MiB minus one,
  exactly 1 MiB, 1 MiB plus one, and the maximum admitted object size.
- End-to-end replay of fixtures from `fixtures/sorafs_manifest/governance/`, PoR, repair, settlement, and reputation evidence.
- Snapshot/export/import tests that preserve block hashes.
- Failure tests for pinning outage, publisher key failure, invalid parent, duplicate payload, and mirror recovery.

Implemented local unit tests now cover DAG block creation, CID derivation,
signature-payload stability, parent linkage, missing-parent failure, signed head
manifest validation, and head block-count mismatch rejection. The always-on
service source additionally has mock-Kubo/signed-HTTP adversarial coverage for pin/readback,
CAS conflicts, response bounds, SSRF policy, checkpoint/intent corruption,
restart recovery, mirror tamper, rollback, replay/deletion, provider
missing/mismatch/outage, per-request credential rotation, forbidden secret-path
rejection without following symlinks, and fork rejection. It also includes
focused regressions for the fixed UnixFS chunk-boundary vectors, protocol-fixed
mirror retention, deterministic mirror reconstruction, liveness-bound reader
withdrawal, exact ingress qualification, and repair of a crash-resumed intent's
objects before public-head CAS. Embedded signer
coverage rejects malformed/weak keys, mismatched or drifting identities,
invalid signatures, and secret-bearing provider failures. The opt-in Kubo lane
exercises fixed-profile boundary conformance, signed-head restart, and tamper
behavior against the exact checksum-pinned release used by its dedicated CI
workflow. Cargo execution of these source tests remains pending, followed by
governed release-environment execution with real deployment providers and
captured deployment evidence.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_governance_dag_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_governance_dag_rollout_evidence_test.py`

## Documentation & Tooling
- Keep `sorafs-validate governance` as the reference local verifier for `GovernanceLogNodeV1`.
- Use `sorafs-validate governance --block <block.to>` and `sorafs-validate
  governance --head <head.to> --block <block.to>...` for local
  `GovernanceDagBlockV1` and signed-head chain verification.
- Use `sorafs_cli governance dag list|show|verify|export` for offline local
  archive inspection and evidence handoff. Treat exported snapshots as local
  verification bundles, not the authenticated public head.
- Use `sorafs_cli governance dag build` when an offline archive needs a signed
  `GovernanceDagBlockV1`/`GovernanceDagHeadV1` snapshot independent of the
  always-on publisher. Keep `--key-hex` and `--key` inputs runtime-only. Add
  `--car-out <path>` when downstream tests or handoff jobs need a deterministic
  CARv2 segment for the generated snapshot payloads.
- Use `sorafs_cli governance dag verify-build` before handing a local
  block/head snapshot to downstream tests or operators; it verifies decoded
  block/head linkage and catches sidecar or tampering drift in the builder
  output directory.
- Use `sorafs_cli governance dag rebuild-head` when a local block snapshot needs
  a fresh signed head for recovery, handoff, or checkpoint testing. Keep
  `--key-hex` and `--key` inputs runtime-only.
- Use `sorafs_cli governance dag checkpoint` when a verified local block/head
  snapshot needs a JSON handoff manifest that binds the head to optional CARv2
  and mirror-index artifacts. This is the offline handoff form; the always-on
  service exposes its authenticated public checkpoint through the service API.
- Use `sorafs_cli governance dag checkpoint-verify` before trusting or handing
  off local checkpoint metadata; it verifies the signed snapshot and recorded
  artifact digests, and can point at recovered artifact paths with `--root`,
  `--car`, and `--mirror-index`.
- Use `sorafs_cli governance dag checkpoint-recover` to rebuild a local mirror
  index from a verified checkpoint and recovered block/head snapshot. The
  command refuses to write the recovered mirror index if snapshot or CAR
  bindings fail.
- Use `sorafs_cli governance dag mirror-build` and `mirror-query` for local
  signed-snapshot lookup by head, block CID, or governance-node CID without
  contacting the shipped runtime mirror service.
- Use Torii `GET /v1/sorafs/governance/dag/dashboard`, `/head`,
  `/blocks/{block_cid_hex}`, and `/nodes/{node_cid_hex}` when an enabled SoraFS
  node has a configured Governance DAG root and irohad has installed the
  supervised service's mirror-read capability. That capability is the route's
  sole mirror authority.
- Use the `publish_index` section of the NodeHandle-authenticated typed
  publication snapshot as the runtime-local feed of filesystem-published
  governance artifacts. The envelope is a dashboard source, not the public
  signed head or a replacement for the bounded authenticated public mirror.
- Use Torii `GET /v1/sorafs/governance/dag/publish-index?limit=N`,
  `/publish-index/digests/{encoded_blake3_hex}`, and
  `/publish-index/kinds/{payload_kind}` to query that runtime-local feed through
  the node API when filesystem governance publication is enabled. Top-level and
  digest/kind result arrays are bounded by `limit` while total match counts remain
  visible.
- Use Torii `GET /v1/sorafs/governance/dag/car-queue`,
  `/car-queue/digests/{encoded_blake3_hex}`,
  `/car-queue/kinds/{payload_kind}`, and
  `/car-queue/archives/{car_archive_blake3_hex}` to inspect assembled local CAR
  segment queue state before the always-on service publishes it. Digest/kind
  lookup segment arrays are bounded by `limit` while total match counts remain
  visible.
- Use Torii `GET /v1/sorafs/governance/dag/runtime`, `/runtime/head`,
  `/runtime/blocks/{block_cid_hex}`, `/runtime/nodes/{node_cid_hex}`,
  `/runtime/digests/{encoded_blake3_hex}`, and `/runtime/kinds/{payload_kind}`
  to inspect the authenticated typed runtime DAG head/index generation
  independently of the public signed-HTTP head. Digest/kind lookup block arrays
  are bounded by `limit` while total match counts remain visible.
- Use `scripts/build_sorafs_governance_dag_canary.py` to turn reviewed SF-12
  deployment facts into payload-free canary JSON before running the rollout
  gate. The canary builder covers ingest service, publisher service, mirror
  datastore, operator recovery, dashboard API, observability,
  `publication_e2e`, and governance approval artifacts. It
  requires every positive proof claim explicitly, requires complete closed-set
  verified-claim, payload-kind, dashboard-route, or metric coverage where
  applicable, rejects duplicate or unknown closed-set values before writing,
  requires reviewed publisher/publication `governance-dag-block-*` block-reference
  inventories whose unique rows match `--block-count`, rejects non-production
  block-reference markers before writing, forces the schema-supported raw
  block/head/checkpoint/response inclusion flags to `false`, rejects CAR or
  other payload-bearing fields through the closed schema, requires an explicit
  `--route-body-blake3-hex` digest for dashboard canaries, validates the
  generated artifact through the SF-12 checker, and writes atomically without
  following output symlinks.
- Keep `configs/taikai_cache/` and `cargo xtask sorafs-taikai-cache-bundle` documented as Taikai cache governance bundle tooling, not as the full DAG publisher.
- Package the shipped live-head, public checkpoint recovery, and dashboard
  runbooks with the supervised deployment.

## Rollout Evidence Gate

Use the rollout gate when the deployed ingest service, fixed-profile Kubo
publisher, signed-HTTP head service, bounded authenticated mirror, checkpoint
recovery workflow, liveness-bound Torii reads, live observability, end-to-end
tests, and governance packet have produced reviewed, payload-free JSON
evidence. The V1 evidence objects and dashboard route rows are schema-closed.
Publisher evidence must prove the deterministic 1 MiB/raw-leaf/balanced Kubo
UnixFS profile, locally derived CIDv1 SHA-256 roots, strong single-ETag
signed-HTTP CAS and readback, and the exact exclusive receiver/shared sealed
atomic replay qualification. It exports non-zero digests of the complete Kubo
and signed-head ingress bindings; governance approval must match both, as well
as the receiver-policy, replay-namespace, replica-set, and publisher-policy
identities. Optional recognized bound evidence is rejected when its publisher
anchor is absent. Mirror and governance evidence must bind the
protocol-fixed 65,536-block/512 MiB suffix. Recovery, observability, dashboard,
and `publication_e2e` evidence respectively prove same-CID post-loss repair,
the full-first then 64-entry/16 MiB rotating audit, fresh checkpoint-coherent
reads, and reader withdrawal after service liveness ends.
RocksDB/IPLD is required only if the governed SF-12 capacity measurement rejects
the shipped bounded JSON mirror:

```sh
python3 scripts/check_sorafs_governance_dag_rollout_evidence.py \
  @scripts/examples/sorafs_governance_dag_rollout_evidence.args.example
```

For staged collections with reviewed evidence paths, prefer the planner so the
verifier command, summary path, thresholds, and current required payload-free
field contract are reproducible:

```sh
python3 scripts/run_sorafs_governance_dag_rollout_evidence.py \
  @scripts/examples/sorafs_governance_dag_rollout_collection.args.example \
  --dry-run
```

Build reviewed payload-free publisher-service and dashboard canaries:

```sh
python3 scripts/build_sorafs_governance_dag_canary.py \
  @scripts/examples/sorafs_governance_dag_publisher_canary.args.example
python3 scripts/build_sorafs_governance_dag_canary.py \
  @scripts/examples/sorafs_governance_dag_dashboard_canary.args.example
```

The checker recognizes `sorafs.governance_dag.*` SF-12 rollout schemas for
ingest service, publisher service, mirror datastore, operator recovery,
dashboard API, observability, `publication_e2e`, and governance approval
evidence. The retired IPNS kind and flag are not aliases. It reports `ready`
only when every required kind is present,
every recognized artifact is valid, raw DAG blocks, raw heads, CAR payloads,
node payloads, response bodies, private keys, bearer tokens, signed
transactions, and ledgers are absent, route latency, IPFS pin lag, and public
head age stay under configured thresholds, those timing fields are
non-negative integer-unit evidence, enough public blocks and payload kinds are
covered, and governance is bound to `iroha_config`. Ingest-service
artifacts also bind `source_count` to the unique canonical `payload_kinds`
inventory and reject duplicate or unknown payload-kind entries before promotion
can report ready. Publisher-service and `publication_e2e` artifacts also bind
`block_count` to the unique canonical `block_refs` inventory, bind
`payload_kind_count` to the unique canonical `payload_kinds` inventory, require
reviewed `governance-dag-block-*` block-reference labels without non-production
markers, and reject duplicate block-reference entries and duplicate or unknown
payload-kind entries before promotion can report ready. Dashboard API artifacts also bind `route_count` to the unique canonical
`routes[].name` inventory and reject duplicate or unknown route entries before promotion
can report ready. Every dashboard route response must also include a
`body_blake3_hex` digest. Observability artifacts also bind `metric_count` to the
unique canonical `metrics` inventory and reject duplicate or unknown metric
entries before promotion can report ready. The summary exports the sorted
reviewed `metrics` inventory plus `metric_count_values`, and the aggregate
production-readiness gate requires those fields to match the observability
artifact fingerprint before final promotion can report ready. Governance DAG
aggregate promotion also rechecks the lane-proven relationships: public-head
bound artifact fingerprints must match `valid_public_head_cids`, and
policy-bound artifact fingerprints must match `valid_policy_digests` before
final promotion can report ready. Governance approval must additionally carry
the same receiver-policy, replay-namespace, and replica-set digests as its
publisher-service evidence, plus the same complete Kubo and signed-head ingress-
binding digests. The aggregate gate independently rechecks all five ingress
metadata anchors against the governance-approval fingerprint. Governance DAG
rollout summaries must expose exactly one active public head CID, publisher
policy digest, checkpoint digest, receiver-policy digest, replay-namespace
digest, replica-set digest, Kubo ingress-binding digest, and signed-head
ingress-binding digest; mixed valid anchors fail closed before final promotion
can report ready. Governance DAG
payload-safety artifacts must explicitly set `payload_bytes_included`,
`raw_head_included`, `mirror_drift_detected`,
`raw_blocks_included`, `raw_checkpoint_included`, `response_bodies_included`,
and `critical_alerts_firing` to `false` before promotion can report ready; raw
CAR and other undeclared payload fields are rejected by the closed schema. Valid
operator-recovery artifacts now publish their reviewed `checkpoint_digest_hex`
values as `valid_checkpoint_digests`, and the aggregate production-readiness
gate accepts those digests only as payload-free lowercase-hex metadata tethered
to recognized artifact fingerprints. The collection
planner dry-run JSON also includes the checker-backed `evidence_contract` map so operators
can inspect the exact required fields for each requested evidence kind, and the
runner validates the schema-closed collection plan, required kinds, thresholds,
external evidence map, evidence contract, and command steps before collecting
or submitting live publication artifacts. It also rejects non-canonical nested
required-kind, threshold, external-evidence, evidence-contract, and command-step
shapes. Use the
canary builder for reviewed SF-12 promotion evidence so public-head binding,
freshness thresholds,
payload-free inclusion flags, and checker prevalidation stay consistent with the
rollout gate. This evidence must be collected from the shipped always-on
publisher rather than synthesized from the pre-publication filesystem hooks.

## Rollout Status
- Done: governance log schema, public DAG block/head schemas, deterministic node-CID/block-CID derivation, block/head signature helpers, parent-chain and signed-head validation, payload validation including appeal finance reports, weekly rollups, and settlement receipts, Ed25519/ML-DSA signature verification, reference validation hooks for nodes/blocks/heads, governance log-node FFI hooks, fixtures, local filesystem publishing hooks with one typed atomic publication authority containing one-to-one publish-index and CAR-queue sections, appeal-finance rollup summaries embedded in local SoraFS reconciliation reports, deterministic CARv2 segment assembly for filesystem-published artifacts, config-backed local signed runtime block/head assembly for supported filesystem-published payloads, path-free typed NodeHandle snapshots for Torii publish-index, CAR queue, runtime signed-DAG, and supervised mirror query APIs with checkpoint-bound ETags and `limit`-bounded top-level/lookup arrays, PoR report/challenge filesystem publication, Taikai cache bundle generation, local Governance DAG operator inventory/verify/export/build/verify-build/rebuild-head/checkpoint/checkpoint-verify/checkpoint-recover/mirror-build/mirror-query commands, local CARv2 segment emission for signed snapshots, local filesystem backlog/head-age metric emission, local Governance DAG publication metrics/dashboard/alerts, a rollout evidence checker/planner framework, payload-free canary builder, operator argfile templates, and focused source tests.
- Done addendum: valid SF-12 operator-recovery evidence now surfaces
  `valid_checkpoint_digests`, and aggregate readiness validates those checkpoint
  digests as payload-free metadata tied to recognized artifact fingerprints.
- Done addendum: `sorafs_governance_dag` provides the always-on validated source
  ingest, locally derived fixed-profile Kubo CIDs with verified
  add/pin/readback, signed-HTTP CAS head publication, sealed intent/checkpoint
  recovery, pre-CAS object repair, protocol-fixed mirror retention, full-first
  and rotating steady audits, runtime-authenticated outbound requests, and a
  liveness-bound public mirror. Both control-plane providers must attest the
  exact exclusive receiver and shared sealed atomic replay namespace. The
  public library boundary accepts only opaque signer/authenticator/store traits
  and public policy identities.
- Done addendum: the embedded producer has explicit joint outgoing/incoming signatures on its
  predecessor/head/index-bound signer/store qualification journal, independent
  sealed transition/archive generations, and bounded signed archive compaction
  with durable archive/checkpoint readback before prune and restart-safe staged
  replay. This is provider-qualification retention; it does not claim a
  deployed software-signer/sealed-store backend or DAG block-prefix compaction.
- Remaining: implement the deployment-owned provider adapters, package and
  supervise two service instances, decide whether production scale requires the
  optional RocksDB/IPLD mirror, add any operator convenience commands required
  by deployment practice, run the alert fixture with `promtool` on a host that
  ships it, and capture staged/live publication, provider rotation/outage,
  recovery, dashboard, and alert evidence that passes the updated SF-12 gate.
  The checked-in rollout builder, checker, runner, examples, and closed evidence
  schemas already enforce the current transport, ingress, retention, recovery,
  audit, and fresh-read contracts.
