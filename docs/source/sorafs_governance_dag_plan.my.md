---
lang: my
direction: ltr
source: docs/source/sorafs_governance_dag_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 281b9435d91b25484425a2b5dbfcb8413aefb974228dd7bd3ca97f3e14d9d246
source_last_modified: "2026-06-25T17:58:13+00:00"
translation_last_reviewed: 2026-06-25
title: Governance DAG Publishing Pipeline
summary: SF-12 implementation status for governance log schemas, local filesystem publishers, validation tooling, and remaining IPFS/IPNS DAG rollout.
---

# Governance DAG Publishing Pipeline

## Status
SF-12 is partially implemented. The workspace ships governance-log payloads,
signature validation, reference validation tooling, and local filesystem
publishers for several SoraFS evidence streams. It also ships a local
`sorafs_cli governance dag` operator surface for pre-IPFS archive inspection,
validation, export, signed block/head snapshot verification, and local CARv2
segment emission. Local mirror index build/query tooling is also available for
signed block/head snapshots, local signed-head rebuild can recover a head
manifest from an existing block snapshot, and local checkpoint metadata can bind
verified heads to optional CAR and mirror-index artifacts for handoff.
Checkpoint verification and recovery can replay those local bindings and rebuild
a local mirror index before public recovery tooling exists. Torii can expose a
read-only local dashboard/query API over a configured mirror index. Filesystem
governance publishers also maintain a local `publish-index.json` as runtime
artifacts are materialized, and Torii exposes read-only publish-index queries,
giving operators a deterministic publication feed before a RocksDB/IPLD mirror
exists. The filesystem publisher also assembles a runtime-local `car-queue.json`
with per-publication deterministic CARv2 segments for the canonical `.to`
payload, JSON mirror, and BLAKE3 sidecars, and Torii exposes read-only queue
and segment lookup endpoints for that local queue. When configured with a
runtime-only publisher peer ID and Ed25519 signing-key path, the filesystem
publisher also appends supported published payloads to a local signed
`runtime-dag/` chain with `GovernanceDagBlockV1` blocks,
`GovernanceDagHeadV1` head bytes, and a
`sorafs.governance_dag.runtime_signed_index.v1` lookup index. Torii exposes
read-only runtime index/head/block/node/digest/kind queries over that local
index. Publish-index, CAR queue, and runtime digest/kind lookup responses keep
full match counts visible while bounding returned `entries`, `segments`, or
`blocks` arrays through `limit` (default 50, max 500). It does not yet ship the
full IPFS/IPNS governance DAG pipeline described by the roadmap.

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
- `FilesystemGovernancePublisher` support in `crates/sorafs_node` for local
  deal settlement, repair, GC, reconciliation, reputation, moderation ballot
  lifecycle, appeal finance report, appeal finance weekly rollup, appeal
  finance settlement receipt, and orderbook settlement receipt evidence.
  Each successful filesystem publish now updates a
  local `sorafs.governance_dag.local_publish_index.v1` `publish-index.json` with
  artifact paths, BLAKE3 digests, payload-kind counts, digest lookup maps, and
  compact labels for query surfaces, then ensures a local
  `sorafs.governance_dag.local_car_queue.v1` `car-queue.json` entry and
  assembled CARv2 segment under `car-segments/` for that publication. With
  `sorafs.storage.governance_dag_publisher_peer_id` and
  `sorafs.storage.governance_dag_signing_key_path` configured, deal
  settlements, reputation snapshots, moderation ballot lifecycle events, appeal
  finance reports, weekly rollups, appeal finance settlement receipts, and
  orderbook settlement receipts also append to the local signed runtime DAG;
  duplicate publishes are idempotent and malformed runtime DAG index state
  fails closed.
  The local filesystem sink also updates the Governance DAG backlog gauge from
  CAR queue pending segment counts and refreshes the local signed runtime-head
  age gauge when runtime DAG state is written or de-duplicated.
- `scripts/check_sorafs_governance_dag_rollout_evidence.py` now provides the
  fail-closed SF-12 rollout evidence gate for deployed Governance DAG
  promotion packets, and
  `scripts/run_sorafs_governance_dag_rollout_evidence.py` provides the matching
  reviewed evidence collection planner/runner. Mirror datastore, checkpoint
  recovery, dashboard, observability, IPFS/IPNS end-to-end, and governance
  approval artifacts must carry the same `public_head_cid_hex` as a valid
  publisher-service artifact in the same bundle, so rollout evidence cannot mix
  mirror, recovery, dashboard, public IPFS/IPNS, or approval records from
  different signed DAG heads. Public-head binding failures are recorded on the
  offending artifact before required-kind validity is computed, so the JSON
  summary matches the fail-closed rollout decision.
- Typed appeal finance transparency rollups: `sorafs_manifest` validates
  `SoraFsAppealFinanceWeeklyRollupV1` payloads and can aggregate validated
  finance reports into deterministic weekly dashboard rows for the filesystem
  publisher and signed runtime DAG.
- Torii PoR filesystem publishing for challenge/report artifacts rooted at the configured `sorafs_por.governance_dag_dir`.
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
- Ingest, DAG-builder, and publisher services that persist and publish the
  shipped public block/head schemas.
- IPFS Cluster pinning and IPNS head publication for the signed runtime DAG.
- Runtime RocksDB/IPLD mirror datastore and query service.
- IPFS/IPNS-backed `sorafs governance dag` operations for live heads, public
  checkpoint publication, and public checkpoint recovery.
- Runtime/IPFS-backed dashboard REST/GraphQL API for live DAG queries.
- Live IPFS/IPNS publisher metrics, public-head dashboards, and alert routing
  evidence beyond the local filesystem publication telemetry.
- End-to-end tests with local IPFS/IPNS infrastructure and live rollout evidence.

## Goals & Scope
- Capture governance artifacts such as adverts, replication orders, PoR events, repairs, settlements, reputation snapshots, verdicts, and reports in append-only evidence.
- Preserve deterministic validation through Norito payloads and publisher signatures.
- Provide a future verifiable DAG head so operators, SDKs, and auditors can retrieve current governance state.
- Keep local filesystem evidence compatible with a later IPFS/IPNS publisher rather than treating local files as the final public archive.

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

These publishers are local materialization hooks. They do not pin content to
IPFS, publish IPNS heads, emit a public DAG head event, or provide historical
DAG queries. The optional runtime DAG signer writes local signed block/head
bytes only for payload variants already represented by `GovernanceLogPayloadV1`.

## Target Architecture
| Component | Responsibility | Current workspace status |
|-----------|----------------|--------------------------|
| Ingest service | Subscribe to Torii/governance evidence, load full payloads, and verify signatures. | Not shipped. |
| DAG builder | Wrap validated payloads into DAG blocks, compute parent linkage, and assemble CAR segments. | Local filesystem builder, build-output verifier, and optional CARv2 segment emission are shipped for validated node archives and signed block/head snapshots. Runtime-local CAR queueing, per-publication CARv2 segment assembly, and config-backed local signed runtime block/head assembly are shipped for supported filesystem-published governance artifacts; the always-on ingest/publisher service boundary is not shipped. |
| Publisher | Pin CAR/block data to IPFS Cluster and publish a signed IPNS/head manifest. | Not shipped. |
| Mirror datastore | Maintain queryable block and payload indexes. | Local JSON mirror index build/query commands are shipped for signed snapshot roots, filesystem publishers maintain runtime-local `publish-index.json`, `car-queue.json`, and `runtime-dag-index.json` feeds for materialized governance artifacts, and Torii exposes read-only publish-index, CAR queue, and runtime DAG lookup endpoints; the runtime RocksDB/IPLD mirror datastore and query service are not shipped. |
| Dashboard/API backend | Serve governance history, block lookup, snapshots, and proof queries. | Torii read-only local mirror, publish-index, CAR queue, and runtime DAG endpoints are shipped for configured filesystem state; the runtime/IPFS-backed dashboard backend is not shipped. |
| Operator CLI | Inspect heads, list/fetch blocks, export snapshots, verify chains, and rebuild heads. | Local archive list/show/verify/export/build/verify-build/rebuild-head/checkpoint/checkpoint-verify/checkpoint-recover/mirror-build/mirror-query is shipped for `.to` governance nodes and block/head snapshots, and `sorafs-validate governance` validates local block/head Norito payloads; live IPFS/IPNS head, fetch, public checkpoint publication/recovery, and runtime mirror-service commands are not shipped. |

## Target Publishing Workflow
1. Ingest validates a Norito payload and deduplicates it by digest.
2. The builder links the payload to the current head, computes a deterministic block CID, and writes a CAR segment.
3. A validator re-derives the CID, checks parent availability, verifies the payload and publisher signature, and quarantines invalid blocks.
4. The publisher pins the CAR/block data to IPFS Cluster and updates a signed head manifest/IPNS record.
5. Torii or the dashboard backend announces the new head for subscribers.
6. Clients resolve the head, verify signatures and digests, and replay blocks back to a trusted checkpoint.

The local filesystem publisher now performs step 2 for supported payloads when a
runtime DAG signer is configured. The end-to-end public workflow remains target
design until IPFS/IPNS publishing services and tests are present.

## Security & Verification Requirements
- All payloads must remain Norito-first; JSON should be a presentation format only.
- Unknown payload or schema versions must fail closed.
- Head and block signatures must be deterministic and replayable.
- Publisher keys need HSM or sealed-secret handling before production use.
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
- `sorafs_governance_dag_head_age_seconds{sink}` reports the local signed
  runtime DAG head age for the filesystem sink when runtime DAG state is
  written or refreshed. Public IPFS/IPNS head-age emission remains future work.
- `dashboards/grafana/sorafs_governance_dag.json` visualizes local publication
  outcomes, published bytes, backlog, head age, and publish age.
- `dashboards/alerts/sorafs_governance_dag_rules.yml` alerts on local
  publication failures, backlog, stale heads, and missing recent publications.

Still-required live signals include block count by payload kind, publish
duration, IPNS/head update results, validation failures, CAR queue depth, pin
lag, mirror/index drift, and last successful public IPNS head.

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
- Integration tests with a local IPFS/IPNS-compatible environment.
- End-to-end replay of fixtures from `fixtures/sorafs_manifest/governance/`, PoR, repair, settlement, and reputation evidence.
- Snapshot/export/import tests that preserve block hashes.
- Failure tests for pinning outage, publisher key failure, invalid parent, duplicate payload, and mirror recovery.

Implemented local unit tests now cover DAG block creation, CID derivation,
signature-payload stability, parent linkage, missing-parent failure, signed head
manifest validation, and head block-count mismatch rejection. The remaining
tests above require the runtime builder/publisher and IPFS/IPNS stack.

The rollout evidence scripts have focused Python coverage in:

- `scripts/tests/check_sorafs_governance_dag_rollout_evidence_test.py`
- `scripts/tests/run_sorafs_governance_dag_rollout_evidence_test.py`

## Documentation & Tooling
- Keep `sorafs-validate governance` as the reference local verifier for `GovernanceLogNodeV1`.
- Use `sorafs-validate governance --block <block.to>` and `sorafs-validate
  governance --head <head.to> --block <block.to>...` for local
  `GovernanceDagBlockV1` and signed-head chain verification.
- Use `sorafs_cli governance dag list|show|verify|export` for pre-IPFS local
  archive inspection and evidence handoff. Treat exported snapshots as local
  verification bundles, not public IPNS heads.
- Use `sorafs_cli governance dag build` when a local archive needs a signed
  `GovernanceDagBlockV1`/`GovernanceDagHeadV1` snapshot before the runtime
  builder and IPFS/IPNS publisher exist. Keep `--key-hex` and `--key` inputs
  runtime-only. Add `--car-out <path>` when downstream tests or handoff jobs
  need a deterministic CARv2 segment for the generated snapshot payloads.
- Use `sorafs_cli governance dag verify-build` before handing a local
  block/head snapshot to downstream tests or operators; it verifies decoded
  block/head linkage and catches sidecar or tampering drift in the builder
  output directory.
- Use `sorafs_cli governance dag rebuild-head` when a local block snapshot needs
  a fresh signed head for recovery, handoff, or checkpoint testing. Keep
  `--key-hex` and `--key` inputs runtime-only.
- Use `sorafs_cli governance dag checkpoint` when a verified local block/head
  snapshot needs a JSON handoff manifest that binds the head to optional CARv2
  and mirror-index artifacts. Treat this as local checkpoint metadata until the
  IPFS/IPNS publisher exists.
- Use `sorafs_cli governance dag checkpoint-verify` before trusting or handing
  off local checkpoint metadata; it verifies the signed snapshot and recorded
  artifact digests, and can point at recovered artifact paths with `--root`,
  `--car`, and `--mirror-index`.
- Use `sorafs_cli governance dag checkpoint-recover` to rebuild a local mirror
  index from a verified checkpoint and recovered block/head snapshot. The
  command refuses to write the recovered mirror index if snapshot or CAR
  bindings fail.
- Use `sorafs_cli governance dag mirror-build` and `mirror-query` for local
  signed-snapshot lookup by head, block CID, or governance-node CID before the
  runtime mirror service exists.
- Use Torii `GET /v1/sorafs/governance/dag/dashboard`, `/head`,
  `/blocks/{block_cid_hex}`, and `/nodes/{node_cid_hex}` when an enabled SoraFS
  node has `sorafs.storage.governance_dag_dir` pointing at a local
  `mirror-index.json` and operators need a read-only dashboard/query surface
  before the runtime mirror datastore and IPFS/IPNS publisher exist.
- Use `publish-index.json` in the configured governance directory as the
  runtime-local feed of filesystem-published governance artifacts. It is an
  operator handoff and dashboard source, not a public IPNS head or a replacement
  for the planned RocksDB/IPLD mirror.
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
  segment queue state before public IPFS/IPNS publication exists. Digest/kind
  lookup segment arrays are bounded by `limit` while total match counts remain
  visible.
- Use Torii `GET /v1/sorafs/governance/dag/runtime`, `/runtime/head`,
  `/runtime/blocks/{block_cid_hex}`, `/runtime/nodes/{node_cid_hex}`,
  `/runtime/digests/{encoded_blake3_hex}`, and `/runtime/kinds/{payload_kind}`
  to inspect the local signed runtime DAG index before a RocksDB/IPLD mirror or
  public IPFS/IPNS head exists. Digest/kind lookup block arrays are bounded by
  `limit` while total match counts remain visible.
- Keep `configs/taikai_cache/` and `cargo xtask sorafs-taikai-cache-bundle` documented as Taikai cache governance bundle tooling, not as the full DAG publisher.
- Add live-head, public checkpoint recovery, and dashboard runbooks only when
  the IPFS/IPNS pipeline and metrics actually exist.

## Rollout Evidence Gate

Use the rollout gate after the deployed ingest service, IPFS/IPNS publisher,
RocksDB/IPLD mirror datastore, public checkpoint recovery workflow,
runtime/IPFS-backed dashboard API, live observability, IPFS/IPNS end-to-end
tests, and governance packet have produced reviewed, payload-free JSON evidence:

```sh
python3 scripts/check_sorafs_governance_dag_rollout_evidence.py \
  @scripts/examples/sorafs_governance_dag_rollout_evidence.args.example
```

For staged collections with reviewed evidence paths, prefer the planner so the
verifier command and summary path are reproducible:

```sh
python3 scripts/run_sorafs_governance_dag_rollout_evidence.py \
  @scripts/examples/sorafs_governance_dag_rollout_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.governance_dag.*` SF-12 rollout schemas for
ingest service, publisher service, mirror datastore, operator recovery,
dashboard API, observability, IPFS/IPNS end-to-end tests, and governance
approval evidence. It reports `ready` only when every required kind is present,
every recognized artifact is valid, raw DAG blocks, raw heads, CAR payloads,
node payloads, response bodies, private keys, bearer tokens, signed
transactions, and ledgers are absent, route latency, IPFS pin lag, and public
head age stay under configured thresholds, enough public blocks and payload
kinds are covered, and governance is bound to `iroha_config`.

## Rollout Status
- Done: governance log schema, public DAG block/head schemas, deterministic node-CID/block-CID derivation, block/head signature helpers, parent-chain and signed-head validation, payload validation including appeal finance reports, weekly rollups, and settlement receipts, Ed25519/ML-DSA signature verification, reference validation hooks for nodes/blocks/heads, governance log-node FFI hooks, fixtures, local filesystem publishing hooks with local `publish-index.json` including appeal finance reports, weekly rollups, and settlement receipts, appeal-finance rollup summaries embedded in local SoraFS reconciliation reports, runtime-local `car-queue.json` and CARv2 segment assembly for filesystem-published artifacts, config-backed local signed runtime block/head assembly for supported filesystem-published payloads, Torii publish-index, CAR queue, and runtime signed-DAG query APIs with `limit`-bounded top-level/lookup arrays and full total counts, PoR report/challenge filesystem publication, Taikai cache bundle generation, local Governance DAG operator inventory/verify/export/build/verify-build/rebuild-head/checkpoint/checkpoint-verify/checkpoint-recover/mirror-build/mirror-query commands, local CARv2 segment emission for signed snapshots, Torii local mirror dashboard/query API, local filesystem backlog/head-age metric emission, local Governance DAG publication metrics/dashboard/alerts, fail-closed rollout evidence gate, collection planner, operator argfile templates, and focused tests.
- Remaining: implement the always-on ingest/publisher services, IPFS/IPNS publication, runtime RocksDB/IPLD mirror datastore and query service, live-head/public-checkpoint publication and recovery operator commands, runtime/IPFS-backed dashboard API, live public IPFS/IPNS head and pin/mirror metric emission, IPFS-backed tests, and staged/live publication evidence that passes the SF-12 gate.
