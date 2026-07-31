---
title: Governance DAG Publishing Pipeline
summary: SF-12 implementation status for the always-on IPFS/IPNS publisher, authenticated recovery, bounded public mirror, and remaining deployment evidence.
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
a local mirror index independently of the network publisher. Torii can expose a
read-only local dashboard/query API over a configured mirror index. Filesystem
governance publishers also maintain a local `publish-index.json` as runtime
artifacts are materialized, and Torii exposes read-only publish-index queries,
giving operators a deterministic publication feed without requiring the
optional RocksDB/IPLD backend. The filesystem publisher also assembles a
runtime-local `car-queue.json`
with per-publication deterministic CARv2 segments for the canonical `.to`
payload, JSON mirror, and BLAKE3 sidecars, and Torii exposes read-only queue
and segment lookup endpoints for that local queue. When configured with a
publisher peer ID, opaque signer handle, canonical Ed25519 public key, and a
matching deployment-injected runtime signer, the filesystem publisher also
appends supported published payloads to a local signed
`runtime-dag/` chain with `GovernanceDagBlockV1` blocks,
`GovernanceDagHeadV1` head bytes, and a
`sorafs.governance_dag.runtime_signed_index.v1` lookup index. Torii exposes
read-only runtime index/head/block/node/digest/kind queries over that local
index. Publish-index, CAR queue, and runtime digest/kind lookup responses keep
full match counts visible while bounding returned `entries`, `segments`, or
`blocks` arrays through `limit` (default 50, max 500).

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
service. It loads its policy from `iroha_config` and accepts a local signed DAG
only when the producer's separate sealed checkpoint authenticates the exact
canonical root, signer/store qualifications, block count, head, and index
digests and no producer write-ahead intent is active. The service performs this
check on both sides of its bounded source read so a concurrent producer cannot
publish filesystem state before its checkpoint CAS commits. It then uploads
and recursively pins every new block plus the signed head through a pinned IPFS
API, verifies pin state, reads every object back, and publishes the head with
either authenticated HTTP compare-and-swap or IPNS resolve/publish/resolve
compare-and-swap. A service-specific authenticated checkpoint and write-ahead
publish intent live in the same deployment-injected sealed monotonic store
under distinct slots, making restart recovery fail closed across partial
block, head, and mirror publication. Every outbound control-plane request is
authenticated through a rotation-aware runtime provider selected by an opaque
configuration handle. Signer, authenticator, and checkpoint providers must
also return a non-zero public policy revision and digest; startup pins those
qualifications before opening publisher/checkpoint state or resolving
publication endpoints, and every operation rejects handle or qualification
drift. Canonical configuration and runtime validation reject
null/mock/test/dev/fake/placeholder handle components. The public
`run_governance_dag_service_with_runtime_registry` launcher resolves one exact
set of those stable handles through a deployment-owned registry and then uses
the same pre-state qualification path. Missing, stale, rejected, incomplete,
substituted, or test-marked registry results fail with typed redacted errors
before service state is opened. The same process serves a bounded public mirror,
head, block, node, checkpoint, health, and Prometheus surface. The stock
`irohad` launcher now resolves the embedded signer, sealed-CAS store, and
IPFS/head request authenticators through a platform-fixed, service-UID-owned
local broker on Linux and macOS. Its bounded canonical Norito protocol binds
the chain, exact provider catalog, fresh session, qualification metadata,
monotonic request identity, operation, and payload; unsupported roles and
platforms fail closed. Request authentication is a hard cut to a signed
canonical descriptor and envelope: configuration pins each strong Ed25519
verification key plus bounded freshness/skew policy, every outbound operation
requalifies the exact provider, and bearer/cookie/alias representations are
rejected. The source also exports a transport-agnostic inbound verifier for the
exact eight authentication headers. It binds the endpoint scope, method,
canonical absolute URL and query, selected public headers, body length and
BLAKE3 digest, freshness interval, nonce, and pinned Ed25519 signature before
backend dispatch. That receiver is not installed in deployment-owned Kubo or
head-service ingress, and its bounded caller-owned replay cache is process-local
rather than sealed or cross-replica. The source tree supplies both the
authenticated client and an injected broker-server library boundary. SF-12
still requires a supervised deployment-owned broker executable, genuine
HSM/sealed-store and authenticated Kubo/head backends, receiver installation
with durable shared replay state, deployment/package integration, optional
RocksDB/IPLD storage if the JSON mirror cannot meet deployment scale, and
captured multi-instance public rollout evidence—not an unimplemented IPFS/IPNS
publisher.

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
  Each successful filesystem publish now updates a
  local `sorafs.governance_dag.local_publish_index.v1` `publish-index.json` with
  artifact paths, BLAKE3 digests, payload-kind counts, digest lookup maps, and
  compact labels for query surfaces, then ensures a local
  `sorafs.governance_dag.local_car_queue.v1` `car-queue.json` entry and
  assembled CARv2 segment under `car-segments/` for that publication. With
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
  worker or publisher state. No signing-key file setting or compatibility
  loader remains. A filesystem publisher now holds an
  exclusive lock on its root
  for its full lifetime and serializes the artifact files, publish index, CAR
  queue, and signed block/head update as one in-process publication transaction,
  so two publishers cannot race the same mutable indexes. Atomic artifact
  replacements fsync both the file and its parent directory before publication
  is acknowledged. The exclusive lock and mutable publish, CAR
  queue, and runtime-DAG JSON indexes reject symlinks and hard links; indexes
  are bounded at 64 MiB and must remain the same file throughout each read.
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
- `scripts/check_sorafs_governance_dag_rollout_evidence.py` now provides the
  fail-closed SF-12 rollout evidence gate for deployed Governance DAG
  promotion packets, and
  `scripts/run_sorafs_governance_dag_rollout_evidence.py` provides the matching
  reviewed evidence collection planner/runner. The checker exports its required
  top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`, and the runner
  dry-run emits the checker-backed `evidence_contract` map for selected SF-12
  evidence kinds, and validates the schema-closed collection plan, required
  kinds, thresholds, external evidence map, evidence contract, and command steps
  before dry-run output or verifier execution. That validation now also rejects
  non-canonical nested required-kind, threshold, external-evidence,
  evidence-contract, and command-step shapes. Mirror
  datastore, checkpoint recovery, dashboard, observability, IPFS/IPNS
  end-to-end, and governance approval artifacts must carry the same
  `public_head_cid_hex` as a valid
  publisher-service artifact in the same bundle, so rollout evidence cannot mix
  mirror, recovery, dashboard, public IPFS/IPNS, or approval records from
  different signed DAG heads. Publisher-service artifacts must also carry
  `policy_digest_hex`, and governance approval artifacts must match that
  publisher policy digest before promotion. Public-head and policy binding
  failures are recorded on the offending artifact before required-kind validity
  is computed, so the JSON summary matches the fail-closed rollout decision.
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
  - `GET /v1/sorafs/governance/dag/dashboard` summarizes the configured
    `mirror-index.json` with signed-head metadata, block counts, payload-kind
    counts, sequence bounds, timestamp bounds, BLAKE3 digest, and ETag/cache
    headers.
  - `GET /v1/sorafs/governance/dag/head` returns the signed-head portion of the
    configured local mirror index.
  - `GET /v1/sorafs/governance/dag/blocks/{block_cid_hex}` and
    `/v1/sorafs/governance/dag/nodes/{node_cid_hex}` look up the indexed block
    by block CID or governance-node CID. The API reads only the node-configured
    governance directory and fails closed on missing, malformed, or unsupported
    mirror indexes.
  - `GET /v1/sorafs/governance/dag/publish-index?limit=N` returns the
    runtime-local filesystem publication feed from `publish-index.json`,
    including payload-kind counts, total entry counts, and a `limit`-bounded
    embedded entry list.
  - `GET /v1/sorafs/governance/dag/publish-index/digests/{encoded_blake3_hex}`
    and `/v1/sorafs/governance/dag/publish-index/kinds/{payload_kind}` query
    that local feed by encoded payload digest or payload kind. The handlers
    validate lookup keys, support ETag revalidation, report total and returned
    counts, bound the returned `entries` array with `limit` (default 50, max
    500), and fail closed on missing, malformed, or unsupported publish indexes.
  - `GET /v1/sorafs/governance/dag/car-queue` returns the runtime-local CAR
    segment queue from `car-queue.json`, including assembled/pending counts and
    the full local queue.
  - `GET /v1/sorafs/governance/dag/car-queue/digests/{encoded_blake3_hex}`,
    `/v1/sorafs/governance/dag/car-queue/kinds/{payload_kind}`, and
    `/v1/sorafs/governance/dag/car-queue/archives/{car_archive_blake3_hex}`
    query assembled local segments by encoded payload digest, payload kind, or
    CAR archive digest. Digest/kind handlers validate lookup keys, support ETag
    revalidation, report total and returned counts, bound the returned
    `segments` array with `limit` (default 50, max 500), and fail closed on
    missing, malformed, or unsupported CAR queues.
  - `GET /v1/sorafs/governance/dag/runtime` summarizes the local signed runtime
    DAG index from `runtime-dag-index.json`, including publisher identity,
    head metadata, block counts, payload-kind counts, and the full local index.
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
- Local publication telemetry: `sorafs_governance_dag_publish_total`,
  `sorafs_governance_dag_published_bytes_total`,
  `sorafs_governance_dag_last_publish_timestamp_seconds`,
  `sorafs_governance_dag_backlog`, and
  `sorafs_governance_dag_head_age_seconds`, plus the checked-in
  `dashboards/grafana/sorafs_governance_dag.json` dashboard and
  `dashboards/alerts/sorafs_governance_dag_rules.yml` alert pack.

Still outstanding:
- Package and supervise `sorafs_governance_dag` in the supported deployment
  bundles, implement and provision the deployment-owned HSM signer,
  rotation-aware IPFS/head authenticators, and sealed monotonic checkpoint-store
  adapters that derive their qualification revision/digest from the external
  control plane and reject revoked/stale policy internally, and capture
  multi-instance rollout/rollback evidence. Install the exact inbound verifier
  in the deployment-owned Kubo/head receivers and replace its process-local
  replay cache with sealed cross-replica state. The generic packaged binary
  now lives with `irohad`, requires a canonical public chain ID, projects only
  the IPFS authenticator, optional signed-head authenticator, and sealed
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
- Capture public IPFS/IPNS, dashboard, alert-routing, and disaster-recovery
  evidence that passes the SF-12 rollout gate.

## Goals & Scope
- Capture governance artifacts such as adverts, replication orders, PoR events, repairs, settlements, reputation snapshots, verdicts, and reports in append-only evidence.
- Preserve deterministic validation through Norito payloads and publisher signatures.
- Provide a verifiable public DAG head so operators, SDKs, and auditors can retrieve current governance state.
- Keep local filesystem evidence byte-compatible with the shipped IPFS/IPNS
  publisher while treating authenticated public checkpoints as the durable
  cross-operator recovery boundary.

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
missing publisher peer IDs, malformed signatures, and invalid nested payloads.
Signature verification covers canonical Norito bytes that exclude
`publisher_signature`, so signers and verifiers operate on stable payload bytes.

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
pins content to IPFS, publishes a signed HTTP or IPNS head, and exposes bounded
historical queries. The optional runtime DAG signer writes local signed
block/head bytes only for payload variants already represented by
`GovernanceLogPayloadV1`.

## Target Architecture
| Component | Responsibility | Current workspace status |
|-----------|----------------|--------------------------|
| Ingest service | Subscribe to Torii/governance evidence, load full payloads, and verify signatures. | Shipped: filesystem publishers materialize typed payloads and a signed runtime chain; `sorafs_governance_dag` revalidates the bounded source snapshot and rejects rollback, forks, unsupported payload kinds, or signature drift before publication. |
| DAG builder | Wrap validated payloads into DAG blocks, compute parent linkage, and assemble CAR segments. | Shipped: local builders, CARv2 segments, signed runtime blocks/heads, source-chain validation, and deterministic replay are implemented. |
| Publisher | Pin CAR/block data to IPFS and publish a signed public head. | Shipped: verified IPFS add/pin/list/cat plus signed-HTTP or IPNS compare-and-swap publication, with SSRF controls, bounded responses, and authenticated restart intent recovery. |
| Mirror datastore | Maintain queryable block and payload indexes. | Shipped as a bounded authenticated JSON mirror and runtime indexes. RocksDB/IPLD remains an optional scale backend rather than a prerequisite for protocol correctness. |
| Dashboard/API backend | Serve governance history, block lookup, snapshots, and proof queries. | Shipped: the always-on service exposes bounded mirror/head/block/node/checkpoint, health, and metrics routes; Torii also exposes local pre-publication indexes. |
| Operator CLI | Inspect heads, list/fetch blocks, export snapshots, verify chains, and rebuild heads. | Local archive list/show/verify/export/build/verify-build/rebuild-head/checkpoint/checkpoint-verify/checkpoint-recover/mirror-build/mirror-query is shipped for `.to` governance nodes and block/head snapshots. Direct convenience wrappers for the public service can be added without changing the protocol boundary. |

## Target Publishing Workflow
1. Ingest validates a Norito payload and deduplicates it by digest.
2. The builder links the payload to the current head, computes a deterministic block CID, and writes a CAR segment.
3. A validator re-derives the CID, checks parent availability, verifies the payload and publisher signature, and quarantines invalid blocks.
4. The publisher pins the CAR/block data to IPFS Cluster and updates a signed head manifest/IPNS record.
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
  payload digests remain enforced by the CAS layer. Removed key/token path
  fields are rejected as unknown V1 configuration.
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
  segment count for the filesystem sink when `car-queue.json` is built or
  refreshed.
- `sorafs_governance_dag_head_age_seconds{sink}` reports the signed
  runtime DAG head age for the filesystem sink when runtime DAG state is
  written or refreshed.
- The always-on service exports IPFS publish success/failure and byte counters,
  backlog, head age, IPFS pin lag, IPNS update success/failure, last successful
  IPNS update time, and mirror drift.
- `dashboards/grafana/sorafs_governance_dag.json` visualizes local publication
  outcomes, published bytes, backlog, head age, and publish age.
- `dashboards/alerts/sorafs_governance_dag_rules.yml` alerts on local
  publication failures, backlog, stale heads, and missing recent publications.

Still-required rollout evidence must demonstrate block count by payload kind,
publish duration/SLOs, validation failures, and alert delivery alongside the
already exported IPNS/head, queue, pin-lag, mirror-drift, and public-head
signals.

Required live alerts include no new public block for the configured SLA,
validation failure, pin lag, IPNS/head update failure, and mirror/index drift.

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
verifies the local Governance DAG dashboard/head/block/node handlers, ETag
revalidation, malformed CID rejection, and missing CID rejection over a
configured local mirror index. Focused `sorafs_node` unit coverage verifies that
filesystem governance publishers update `publish-index.json`, populate
payload-kind and digest lookup maps, write a BLAKE3 sidecar for the index, and
avoid duplicate index entries when the same artifact is republished. The same
coverage now verifies runtime-local `car-queue.json` maintenance, deterministic
CAR segment/plan/manifest emission, CAR sidecars, segment queue de-duplication,
and fail-closed rejection of malformed CAR queue state. It also verifies
config-backed signed runtime DAG append for supported payloads, duplicate
publish idempotency, decoded head/block signature-chain validation with
`validate_governance_dag_head_against_chain_v1`, and fail-closed rejection of
malformed runtime DAG index state, including orderbook settlement receipt
publication. Focused Torii coverage verifies publish-index reads, digest
lookups, payload-kind lookups, `limit`-bounded returned entries, ETag
revalidation, malformed lookup rejection, and missing lookup rejection over a
configured local publish index. Torii CAR queue coverage verifies local queue
reads, digest lookups, payload-kind lookups, `limit`-bounded returned segments,
CAR archive digest lookups, ETag revalidation, malformed lookup rejection, and
missing lookup rejection over a configured local CAR queue. Torii runtime DAG
coverage verifies local runtime-index reads, runtime head reads, block/node/
digest/payload-kind lookups, `limit`-bounded returned blocks, ETag
revalidation, malformed lookup rejection, missing lookup rejection, and
unsupported runtime index schema rejection. Focused `sorafs_node` helper
coverage verifies local CAR queue backlog counting and signed runtime-head age
saturation.

Required before rollout:
- Run the opt-in real-Kubo IPNS restart/tamper lane in the governed release
  environment and archive its output.
- End-to-end replay of fixtures from `fixtures/sorafs_manifest/governance/`, PoR, repair, settlement, and reputation evidence.
- Snapshot/export/import tests that preserve block hashes.
- Failure tests for pinning outage, publisher key failure, invalid parent, duplicate payload, and mirror recovery.

Implemented local unit tests now cover DAG block creation, CID derivation,
signature-payload stability, parent linkage, missing-parent failure, signed head
manifest validation, and head block-count mismatch rejection. The always-on
service additionally has mock-IPFS/IPNS adversarial coverage for pin/readback,
CAS conflicts, response bounds, SSRF policy, checkpoint/intent corruption,
restart recovery, mirror tamper, rollback, replay/deletion, provider
missing/mismatch/outage, per-request credential rotation, legacy secret-path
rejection without following symlinks, and fork rejection. Embedded signer
coverage rejects malformed/weak keys, mismatched or drifting identities,
invalid signatures, and secret-bearing provider failures. Its opt-in Kubo lane
covers real IPNS restart and tamper behavior. Remaining work is governed
release-environment execution with real deployment providers and captured
deployment evidence.

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
  verification bundles, not public IPNS heads.
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
  node has `sorafs.storage.governance_dag_dir` pointing at a local
  `mirror-index.json` and operators need a read-only dashboard/query surface
  over the pre-publication local mirror.
- Use `publish-index.json` in the configured governance directory as the
  runtime-local feed of filesystem-published governance artifacts. It is an
  operator handoff and dashboard source, not a public IPNS head or a replacement
  for the shipped bounded authenticated public mirror.
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
  to inspect the local signed runtime DAG index independently of the public
  IPFS/IPNS head. Digest/kind lookup block arrays are bounded by
  `limit` while total match counts remain visible.
- Use `scripts/build_sorafs_governance_dag_canary.py` to turn reviewed SF-12
  deployment facts into payload-free canary JSON before running the rollout
  gate. The payload-free Governance DAG canary builder covers ingest service,
  publisher service, mirror datastore, operator recovery, dashboard API,
  observability, IPFS/IPNS end-to-end, and governance approval artifacts. It
  requires every positive proof claim explicitly, requires complete closed-set
  verified-claim, payload-kind, dashboard-route, or metric coverage where
  applicable, rejects duplicate or unknown closed-set values before writing,
  requires reviewed publisher/IPFS `governance-dag-block-*` block-reference
  inventories whose unique rows match `--block-count`, rejects non-production
  block-reference markers before writing, forces raw block/head/CAR/checkpoint/
  response inclusion flags to `false`, requires an explicit
  `--route-body-blake3-hex` digest for dashboard canaries, validates the
  generated artifact through the SF-12 checker, and writes atomically without
  following output symlinks.
- Keep `configs/taikai_cache/` and `cargo xtask sorafs-taikai-cache-bundle` documented as Taikai cache governance bundle tooling, not as the full DAG publisher.
- Package the shipped live-head, public checkpoint recovery, and dashboard
  runbooks with the supervised deployment.

## Rollout Evidence Gate

Use the rollout gate after the deployed ingest service, IPFS/IPNS publisher,
bounded authenticated mirror datastore, public checkpoint recovery workflow,
runtime/IPFS-backed dashboard API, live observability, IPFS/IPNS end-to-end
tests, and governance packet have produced reviewed, payload-free JSON evidence.
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
dashboard API, observability, IPFS/IPNS end-to-end tests, and governance
approval evidence. It reports `ready` only when every required kind is present,
every recognized artifact is valid, raw DAG blocks, raw heads, CAR payloads,
node payloads, response bodies, private keys, bearer tokens, signed
transactions, and ledgers are absent, route latency, IPFS pin lag, and public
head age stay under configured thresholds, those timing fields are
non-negative integer-unit evidence, enough public blocks and payload kinds are
covered, and governance is bound to `iroha_config`. Ingest-service
artifacts also bind `source_count` to the unique canonical `payload_kinds`
inventory and reject duplicate or unknown payload-kind entries before promotion
can report ready. Publisher-service and IPFS/IPNS end-to-end artifacts also bind
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
final promotion can report ready. Governance DAG rollout summaries must expose
exactly one active public head CID, one active publisher policy digest, and one
active checkpoint digest; mixed valid public-head, policy, or checkpoint
anchors fail closed before final promotion can report ready. Governance DAG
payload-safety artifacts must explicitly set `payload_bytes_included`,
`raw_head_included`, `raw_car_included`, `mirror_drift_detected`,
`raw_blocks_included`, `raw_checkpoint_included`, `response_bodies_included`,
and `critical_alerts_firing` to `false` before promotion can report ready. Valid
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
- Done: governance log schema, public DAG block/head schemas, deterministic node-CID/block-CID derivation, block/head signature helpers, parent-chain and signed-head validation, payload validation including appeal finance reports, weekly rollups, and settlement receipts, Ed25519/ML-DSA signature verification, reference validation hooks for nodes/blocks/heads, governance log-node FFI hooks, fixtures, local filesystem publishing hooks with local `publish-index.json` including appeal finance reports, weekly rollups, and settlement receipts, appeal-finance rollup summaries embedded in local SoraFS reconciliation reports, runtime-local `car-queue.json` and CARv2 segment assembly for filesystem-published artifacts, config-backed local signed runtime block/head assembly for supported filesystem-published payloads, Torii publish-index, CAR queue, and runtime signed-DAG query APIs with `limit`-bounded top-level/lookup arrays and full total counts, PoR report/challenge filesystem publication, Taikai cache bundle generation, local Governance DAG operator inventory/verify/export/build/verify-build/rebuild-head/checkpoint/checkpoint-verify/checkpoint-recover/mirror-build/mirror-query commands, local CARv2 segment emission for signed snapshots, Torii local mirror dashboard/query API, local filesystem backlog/head-age metric emission, local Governance DAG publication metrics/dashboard/alerts, fail-closed rollout evidence gate, collection planner with dry-run evidence-contract export and schema-closed plan validation, payload-free canary builder for all SF-12 evidence kinds, operator argfile templates, and focused tests.
- Done addendum: valid SF-12 operator-recovery evidence now surfaces
  `valid_checkpoint_digests`, and aggregate readiness validates those checkpoint
  digests as payload-free metadata tied to recognized artifact fingerprints.
- Done addendum: `sorafs_governance_dag` provides the always-on validated source
  ingest, verified IPFS add/pin/readback, signed-HTTP/IPNS CAS head publication,
  sealed checkpoint and publish-intent recovery, runtime-authenticated outbound
  requests, bounded public mirror, health/checkpoint APIs, and IPFS/IPNS/mirror
  metrics with mock-adversarial and opt-in real-Kubo coverage. The production
  boundary is a public library launcher with opaque signer/authenticator/store
  traits; all former service key/token paths and the embedded signer-key path
  are removed.
- Done addendum: the embedded producer has an explicit old/new dual-signed,
  predecessor/head/index-bound signer/store qualification journal, independent
  sealed transition/archive generations, and bounded signed archive compaction
  with durable archive/checkpoint readback before prune and restart-safe staged
  replay. This is provider-qualification retention; it does not claim a
  deployed HSM/sealed-store backend or DAG block-prefix compaction.
- Remaining: implement the deployment-owned provider adapters, package and
  supervise two service instances, decide whether production scale requires the
  optional RocksDB/IPLD mirror, add any operator convenience commands required
  by deployment practice, and capture staged/live publication, provider
  rotation/outage, recovery, dashboard, and alert evidence that passes the
  SF-12 gate.
