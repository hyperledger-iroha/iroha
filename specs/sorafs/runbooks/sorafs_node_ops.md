# SoraFS Node Operations Runbook

This runbook walks operators through validating an embedded `sorafs-node`
deployment inside Torii. Each section maps directly to the SF-3 deliverables:
pin/fetch round trips, restart recovery, quota rejection, and PoR sampling.

> Public and in-depth documentation is maintained in the sibling
> `iroha-docs` repository and published at <https://docs.iroha.tech/>.

## 1. Prerequisites

- Enable the storage worker in `sorafs.storage`:

  ```toml
  [sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Configure distinct proof-outcome, repair, reserve, and orderbook entries under
  `sorafs.storage.native_transaction_signers`, and inject all four matching live
  providers. Storage startup requires this durable-drain bundle even when the
  repair/reserve/orderbook new-work generation flags are disabled. A reserve or
  orderbook role with both storage and its generation flag disabled is paused
  before task creation and makes zero external progress. Opening the local
  `NodeHandle` may still durably normalize an interrupted signer-only `Signing`
  claim back to `Ready` without refunding its attempt.
- Ensure the Torii process has read/write access to `data_dir`.
- Confirm the node advertises the expected capacity via
  `GET /v1/sorafs/capacity/state` once a declaration is recorded.
- When smoothing is enabled, dashboards expose both the raw and smoothed
  GiB·hour/PoR counters to highlight jitter-free trends alongside spot values.

### CLI Dry Run (Optional)

Before exposing HTTP endpoints you can sanity-check the storage backend with
the bundled CLI.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --max-capacity-bytes=10737418240 \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

The commands print Norito JSON summaries and refuse chunk-profile or digest
mismatches, making them useful for CI smoke checks ahead of Torii wiring.【crates/sorafs_node/tests/cli.rs:1】

### PoR Proof Rehearsal

Operators can now replay governance-issued PoR artefacts before uploading them
to Torii. The same CLI wires into `NodeHandle::record_por_{challenge,proof,
verdict}`, so local rehearsals surface the exact validation errors Torii would
return.【crates/sorafs_node/src/bin/sorafs-node.rs:197】【crates/sorafs_node/tests/cli.rs:62】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

The command emits a JSON summary containing the manifest digest, provider id,
proof digest, sample count, and (optionally) verdict outcome. Pass
`--manifest-id=<hex>` when you want the CLI to double-check that the stored
manifest matches the challenge digest, and `--json-out=<path>` to archive the
resulting summary alongside the original artefacts for audit evidence. Keep the
`--verdict` flag handy when rehearsing verdict validation—missing verdicts still
exercise challenge + proof ingestion, while a successful verdict proves that
the verdict logic and telemetry counters line up before you call the HTTP API.
The helper intentionally rejects a failed verdict before finalization because
it has no native repair-transaction handoff. Exercise failed-verdict repair only
through Torii's authenticated production lifecycle.

Once Torii is live you can retrieve the same artefacts via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Add `?limit=N` to metadata-only manifest probes when a bounded `files` preview
is enough; omit it when another gateway needs the complete directory layout.
Storage-plan probes default to bounded `files`, `chunk_digests_blake3`, and
`chunks` arrays (`limit` default 50, max 500) while keeping full count and
truncation fields.

Both endpoints are served by the embedded storage worker, so CLI smoke tests and
gateway probes stay in sync.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Capacity telemetry + penalty expectations

- Cooldowns are measured in **settlement windows** (`cooldown_windows *
  settlement_window_secs`). With a strike threshold of `1`, back-to-back
  under-delivery windows only slash once until the cooldown interval elapses;
  a second slash is applied on the first window after cooldown.
- The 30-day soak regression locks the current accrual math. Running
  `cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::capacity_fee_ledger_30_day_soak_deterministic -- --nocapture`
  should print `capacity_soak_digest=71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`.
  If pricing or accrual logic changes, refresh the digest and rerun the test.

### Evidence-viewer checkpoint authority

Before enabling `[sorafs.storage.evidence_viewer]`, bind
`checkpoint_store_handle`, non-zero `checkpoint_store_revision`, and canonical
non-zero `checkpoint_store_policy_digest_hex` to the deployment's exact
linearizable sealed-CAS provider, alongside the required WebAuthn, grant,
receipt-signer, erasure, immutable compaction-archive, and transparency-publisher
bindings. The publisher binding is public only and must contain the exact
`transparency_publisher_handle`, non-zero `transparency_publisher_revision`,
canonical non-zero `transparency_publisher_policy_digest_hex`, and canonical
Ed25519 `transparency_publisher_public_key_hex`. The archive also requires a
non-zero namespace id and exact Ed25519 public key. Keep
`compaction_interval_ms` within `1000..=86400000` and
`compaction_max_records` within `1..=1024`; the checked-in
[`evidence_viewer_runtime_binding.toml`](../snippets/evidence_viewer_runtime_binding.toml)
shows the complete non-secret shape. These are public identity and policy pins
only; CAS/archive/publisher credentials, private keys, and vendor diagnostics
remain inside the runtime providers. The deployment registry must return all
seven dependencies when the viewer is enabled and none when it is disabled.
Missing, unrequested, substituted, stale, unavailable, or test-marked providers
stop startup; so does failure to reconcile the publisher's current signed
authoritative head. The stock daemon broker protocol implements slots 22–26,
47, and 53 with exact public qualification metadata, canonical bounded requests,
replay-safe mutation identities, typed ambiguity, and authenticated
checkpoint/archive/transparency-head readback. It requalifies the configured
publisher around every bounded head load and compare-and-publish call. A
standard-binary deployment still fails closed until the broker service is
packaged and supplied with genuine deployment-owned backends; no local provider
is substituted.

Torii owns the supervised transparency worker only while the evidence viewer and
its qualified publisher are enabled. Startup constructs the producer and must
reconcile the publisher's current signed authoritative head before the service
can start. At the configured compaction cadence, the worker first
fresh-reconciles that external head, reads a fresh service audit checkpoint, and
walks at most 16 signed payload-free projection pages of 256 receipts each. Each
advancing head uses exact predecessor compare-and-publish; rejected or ambiguous
completion is resolved only by authoritative signed readback, without replaying
the mutation. Transport uncertainty retires the affected broker connection and
uses a fresh same-UID, exact-catalog session only for requalification and
readback. Consecutive failures skip 1, 3, then at most 7 cadence ticks and emit
payload-free logs. Normal shutdown signals, stops, and joins the worker. The
repository does not provide the external durable publisher implementation.

Treat the external signed head as authoritative. `checkpoint_path` names only a
private revalidated cache: it may be absent, match the authoritative record, or
be exactly one predecessor behind it, but it must never seed or replace the
external head. Operate at least two Torii replicas against the same authority
and rehearse concurrent CAS rejection, ambiguous-result readback, provider
rotation, rollback, restart, and cache-loss recovery before promotion. The
shutdown-aware worker archives a bounded set, verifies exact canonical readback
and the archive's Ed25519 receipt, and only then prunes and commits the next CAS
head. Restart and every explicit live refresh traverse the signed predecessor
operation ids to archive genesis and verify every historical artifact and both
signatures before adopting the cache or in-memory state. Rehearse a missing or
corrupt historical generation, archive-success/checkpoint-failure replay, and
erasure-before-compaction: a terminal erasure must not regain a retention floor
when an older session is archived later. An unavailable, ambiguous, zero-digest,
or partially successful erasure-intent reconciliation must leave the replica
unavailable for reads until authoritative restart recovery. A poisoned
quarantine-object index must also surface as unavailable state, never as
`NotFound`. The source tree supplies the injection contract, not a production
CAS/external-software-signer/object-lock implementation or deployment evidence.

### Fused privacy publication and signed Governance DAG

Enabling `[sorafs.storage.privacy_aggregates]` requires an explicit
`[sorafs.storage].governance_dag_dir` and the complete signed publisher binding:
`governance_dag_publisher_peer_id`, `governance_dag_signer_handle`, non-zero
`governance_dag_signer_revision`, non-zero
`governance_dag_signer_policy_digest_hex`, and canonical
`governance_dag_publisher_public_key_hex`. Any configured
`governance_dag_dir`, independently of whether the public Governance DAG
service is enabled, also requires the complete
`[sorafs.storage.governance_dag_service]` checkpoint-store triple:
`checkpoint_store_handle`, non-zero `checkpoint_store_revision`, and canonical
non-zero `checkpoint_store_policy_digest_hex`. Supplying only part of either
binding, supplying either without a consumer, or omitting either injected
runtime provider stops startup. The directory is public publication state, not
permission to place a private key or credential in configuration.

The embedded producer uses service-independent sealed slots for its root
checkpoint and write-ahead publication intent. Before accepting the first
append it seals an empty-root binding covering the canonical directory, signer,
peer, public key, and checkpoint-store identity. Each successor intent binds
the exact block, head, index, predecessor revision, and resulting digests.
Startup recovers a retained intent before auditing the root, then authenticates
canonical JSON and Norito bytes, sidecars, source payloads, signatures, CIDs,
lineage, reverse maps, head/index agreement, entry counts, and bounded total
bytes. A local root without its sealed checkpoint, an implicit signer or store
substitution, an unindexed artifact, or an unsafe filesystem object fails
closed.

The single `fenced_privacy_publisher_handle`, non-zero
`fenced_privacy_publisher_revision`, and non-zero
`fenced_privacy_publisher_policy_digest_hex` binding pins an exact pair:
`FencedTransparencyPublisherV1` is the writer and
`FencedTransparencyAuthoritativeHeadReaderV1` is its authenticated reader.
The deployment registry must supply both roles with exactly that same public
identity and qualification, or neither when privacy aggregates are disabled.
Missing, partial, unrequested, substituted, stale, unavailable, or test-marked
roles fail closed. Torii qualifies both roles before global configuration or
background workers start. When a prebuilt node is supplied instead, Torii
revalidates the retained pair against configuration and each live provider;
the node repeats live revalidation as defense in depth.

Publication retry identity is independent of a leader lease. It binds the query
and cycle identities, release sequence, release-record digest, and payload
digest; the lease, fencing token, and predecessor head remain authorization
metadata.
An exact retry after lease rotation, failover, or local-cache loss must return
`AlreadyIncluded` without appending again. The authenticated reader must then
prove that exact publication identity and payload at its inclusion head and
prove that head's ancestry to the current authoritative head. A newer head,
local cache match, or proof for another payload is insufficient; reuse of the
same publication scope with different evidence is a publication conflict.

Before qualification, rehearse the exact retry under a new lease on at least
two replicas, verify that no second block is appended, validate exact inclusion
and ancestry after failover, and preserve the conflict negative. The source
tree carries the injection, qualification, and replay contracts. On Linux and
macOS, the stock daemon registry resolves the implemented Governance DAG
signer/store/request-authenticator, moderation-quarantine, provider-ingest, and
evidence-viewer roles through a platform-fixed, service-UID-owned local broker.
The broker client and injected server-library boundary are bounded, canonical,
session/request-bound, and peer-credential checked; unsupported roles and
platforms fail closed. Governance outbound operations use only canonical
descriptor/envelope request authentication, with the exact Ed25519 verifier,
provider qualification, body bound, lifetime, skew, and replay identity
revalidated around use; credential headers and compatibility representations
are rejected. The deployment-owned broker executable and genuine external software signer,
signer/store, Kubo/head, and writer/reader backends are not packaged in-tree.
The local producer retains a canonical outgoing/incoming dual-signed
key-transition journal and bounded signed qualification archives; archive
readback and sealed-CAS readback precede prefix pruning, while implicit rotation
still fails closed. Each transition advances a monotonic authority segment and
binds both publisher identities and Ed25519 keys to the exact predecessor
checkpoint and current head. Recovery rebuilds that bounded lineage, verifies
every retained block under its active segment, and binds the signed head to the
tip authority. Authenticated DAG block-prefix pruning still needs a bounded
retention protocol. The producer durably stages block, head, and full-index
bytes and seals only their exact lengths and BLAKE3 digests under a 64 KiB
producer-intent ceiling. Recovery authenticates staged readback before changing
the live root.

On the filesystem-flag-qualified Linux, Android, macOS, iOS, FreeBSD, OpenBSD,
NetBSD, and DragonFly targets, configure every producer, service source,
service state, and mirror root as its exact physical canonical path with no
symlink component. In particular, use `/private/var/...` rather than `/var/...`
on macOS. Startup retains `O_DIRECTORY|O_NOFOLLOW` handles for the root and
every ancestor, applies role-specific owner/mode and trusted-sticky-parent
policy, and revalidates device, inode, owner, mode, and effective UID around
filesystem operations. Other Unix targets, and Android architectures outside
arm, aarch64, x86, x86_64, and riscv64, fail compilation until their native
flags and target tests are qualified. Cross-UID replacement or owner/mode drift
observed at a revalidation boundary fails before the then-resolved target is
touched. Linux/macOS/Windows descendant operations are component-rooted through
retained no-follow handles with exact object-identity rechecks. Linux and macOS
require two identical bounded descriptor ACL snapshots and reject protected ACL
namespaces or untrusted mutation grants. Windows pins the root owner SID,
strictly parses two identical bounded security descriptors, rejects untrusted
mutation grants, and retains file IDs through crash recovery and atomic-temp
cleanup. Treat the still-full index as an allocation/retention blocker rather
than clearing crash recovery manually. These local contracts do not make L0,
L1, or L2 green; focused validation, external deployment qualification, and
promotion evidence remain open.

## 2. Finalized Registration → Provider Ingest → Fetch

Before enabling the provider worker, bind
`[sorafs.storage.provider_ingest_runtime]` to the deployment's exact
`authenticated_source_fetch_handle`, non-zero
`authenticated_source_fetch_revision`, non-zero
`authenticated_source_fetch_policy_digest_hex`,
`completion_signer_resolver_handle`, non-zero
`completion_signer_resolver_revision`, non-zero
`completion_signer_resolver_policy_digest_hex`, `completion_signer_handle`,
non-zero `completion_signer_adapter_revision`, complete
`completion_signer_policy_id_hex` / `completion_signer_policy_revision` /
`completion_signer_policy_predecessor_digest_hex` /
`completion_signer_policy_digest_hex` lineage, admitted
`completion_signer_algorithm`, canonical `completion_signer_public_key_hex`,
`checkpoint_store_handle`, non-zero `checkpoint_store_revision`, and non-zero
`checkpoint_store_policy_digest_hex`, and inject the matching sealed monotonic
checkpoint provider together with the authenticated source and governed signer
resolver. These fields are public identity/policy pins only; credentials and
private keys remain runtime-only. Missing, substituted, stale, or test-marked
providers must stop startup. Do not replace a failed provider with the local
checkpoint file: the external sealed head is authoritative and
`provider_ingest_outbox_v1.to` is only a revalidated cache.

Set
`[sorafs.storage.provider_ingest_runtime.outbox].checkpoint_operation_timeout_ms`
to the deployment-reviewed `1..=86400000` millisecond deadline (the default is
30 seconds). One
dedicated worker with a one-request bounded queue owns the external checkpoint
provider boundary. Qualification, head loads, CAS, and mandatory readback all
use that deadline; a timeout latches the worker closed, never spawns another
operation thread, and requires dropping the outbox and reopening it from the
authoritative sealed head before work can resume. Dropping a worker does not
wait on a hung provider call. The worker retains the checkpoint writer lease
until that call exits, so repeated reopen attempts fail busy instead of
accumulating detached workers.

The outbox checkpoint defaults to 192 MiB and the canonical signed-completion
limit must remain within 64 KiB..=64 MiB. Capacity admission counts both copies
that an active row may retain—the unsigned `expected_payload` and the complete
`signed_transaction`—plus conservative canonical active/terminal structural
budgets and checkpoint framing. The checked aggregate for
`max_active_entries` and `max_terminal_entries` must fit
`checkpoint_max_bytes`; a 128 MiB signed-transaction limit therefore cannot be
made valid by the 192 MiB checkpoint ceiling and fails configuration loading.

Set
`[sorafs.storage.provider_ingest_runtime.finalized_archive]` explicitly for
each deployment. `relative_root` is a normalized relative namespace below the
daemon's resolved Kura root; absolute paths and dot components are rejected.
The record, entry, aggregate-byte, provider, per-provider order, aggregate
provider/order, page, and Kura-tip-lag ceilings are validated together without
clamping. Enabled startup opens one single-writer archive, reconciles the
recovered State and Kura tips with a zero-gap barrier, and only then applies the
configured live-lag ceiling. A new archive establishes an explicit activation
floor at that authenticated tip and never claims earlier historical coverage.

Retention remains manual by default (`retention_enabled = false`), and manual
startup rejects any durable checkpoint candidate rather than installing or
cleaning it up. To admit explicit retention, set `retention_enabled = true`
together with `retention_authority_handle`,
`retention_authority_revision`, and
`retention_authority_policy_digest_hex`, then inject the separately namespaced
deployment-owned authority matching that exact public binding. Handles are
credential-free and test-marked handles are rejected. The authority must
provide linearizable per-chain CAS storage for the canonical V1 approval
record. A compaction proposal is read-only; checkpoint publication and prefix
unlink occur only after the exact successor is returned by authoritative
readback. An ambiguous CAS without that readback, qualification drift,
rollback, or a competing head leaves local storage unchanged. Startup can
finish a CAS-before-publication or publication-before-cleanup crash only when
the same authority still names the exact fence and canonical checkpoint.

1. Produce a manifest + payload bundle (for example with
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Submit the canonical caller-signed registration transaction through
   `POST /v1/sorafs/pin/register`, then record the returned finalized height and
   block hash. V1 has no public payload-upload route.
3. Confirm the supervised provider worker admitted the exact finalized
   manifest and provider replication assignment into its durable outbox. The
   outbox identity binds the finalized cursor, manifest digest, provider id,
   and replication-order id; duplicate delivery is idempotent and exhausted
   retries are visible in the provider dead-letter inventory.
   The sealed checkpoint record must advance monotonically and bind the exact
   predecessor revision/checkpoint digest, bounded canonical checkpoint bytes
   and digest, and deterministic CAS revision.
4. Fetch the provider-ingested data with the deployment's exact-network
   operator request-signing helper (for example a `ToriiClient` constructed
   with a runtime-only `OperatorSigningContext`). The helper must generate a
   fresh timestamp and nonce and sign the exact
   `POST /v1/sorafs/storage/fetch` JSON body; unsigned curl, bearer/API tokens,
   precomputed headers, and redirects are not accepted. Base64-decode the
   `data_b64` field and verify it matches the original bytes. For normal content
   delivery, use request-bound stream tokens with the CAR/chunk GET routes;
   this POST remains a local operator diagnostic only.

## 3. Restart Recovery Drill

1. Complete one provider-outbox ingest as above.
2. Record the sealed provider's public head sequence, revision, checkpoint
   digest, and predecessor binding through the deployment-owned provider
   operations plane. The local cache may be absent, match that head, or be
   exactly one predecessor behind it; any other relationship must fail startup.
3. Restart the Torii process (or the entire node). Startup must requalify the
   configured provider before and after loading its head and revalidate or
   rewrite the local cache from that authoritative record.
4. Re-submit the freshly operator-signed fetch request. The payload must still
   be retrievable and the returned digest must match the pre-restart value.
5. Inspect a freshly operator-signed `GET /v1/sorafs/storage/state` to confirm
   `bytes_used` reflects the persisted manifests after the reboot.

For CAS timeout/unknown-result drills, the provider adapter must report an
ambiguous outcome and permit immediate authoritative readback. The exact
successor proves the write committed. The exact unchanged predecessor yields
`CheckpointCasUnchanged` and is safe to retry. Any different head, failed
readback, or provider identity/policy drift is an ambiguous durability failure;
stop the worker and investigate rather than editing the cache or advancing the
cursor manually.

## 4. Capacity Rejection Test

1. Temporarily lower `sorafs.storage.max_capacity_bytes` to a small value
   (for example the size of a single manifest).
2. Let one finalized replication assignment complete through the provider
   outbox.
3. Reconcile a second assignment of similar size. The provider worker must
   reject it before storage mutation with `storage capacity exceeded`; no HTTP
   request can reserve capacity or change this result.
4. Restore the normal capacity limit when finished.

## 5. Retention / GC Inspection (Read-only)

1. Run a local retention scan against the storage directory:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Inspect only expired manifests (dry-run only, no deletions):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. Use `--now` or `--grace-secs` to pin the evaluation window when comparing
   reports across hosts or incidents.

The GC CLI is intentionally read-only. Use it to capture retention deadlines
and expired-manifest inventory for audit trails; do not remove data manually in
production.

## 6. Authenticated PoR Sampling Probe

1. Pin and approve the canonical manifest, then obtain the provider identifier
   and runtime-only proof-stream credential for the deployment.
2. Request bounded PoR witnesses through the authenticated proof stream:

   ```bash
   sorafs_cli proof stream \
     --manifest=/path/to/manifest.to \
     --torii-url="https://$TORII/" \
     --provider-id-hex="$PROVIDER_ID_HEX" \
     --proof-kind=por \
     --samples=4 \
     --sample-seed=12345 \
     --stream-token="$SORAFS_STREAM_TOKEN" \
     --emit-events=false \
     --summary-out=por_probe_summary.json
   ```

   The unauthenticated local `/v1/sorafs/storage/por-sample` route is retired
   and is not mounted. Proof-stream requests must set `sample_count` between 1
   and 500. The CLI must resolve the exact native pin record first and carry its
   non-zero finalized height/hash pair in the PoR request; Torii rejects stale
   cursors and verifies every generated witness against that record's committed
   manifest root.

3. Archive the payload-free summary only after the CLI has authenticated every
   returned witness against the exact canonical manifest.

## 7. PoR Proof Replay Helper

Operators can now validate challenge/proof bundles locally before submitting
them to Torii. Use the new CLI helper from the same host that stores the
manifest:

```
sorafs-node ingest por \
  --data-dir=/var/lib/sorafs \
  --manifest-id=<manifest_cid_hex> \
  --challenge=fixtures/por/challenge_v1.to \
  --proof=fixtures/por/proof_v1.to \
  --json-out=por_replay.json
```

The command decodes the Norito payloads, checks that the challenge/proof
content matches the stored manifest, and feeds both objects through the same
`NodeHandle` tracker Torii uses for live ingestion. The JSON summary contains the
manifest digest, provider identifier, proof digest, and (optionally) the verdict
metadata when `--verdict=<path>` is provided. Include the summary artefact in
incident tickets so SRE/governance reviewers can confirm which proofs were
replayed prior to submission.

## 8. Automation Hooks

- CI / smoke tests can reuse the targeted checks added in
  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```
  which covers `pin_fetch_roundtrip`, `pin_survives_restart`,
  `pin_quota_rejection`, and `por_sampling_returns_verified_proofs`.
- Dashboards should track:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `sorafs_provider_ingest_inflight` and `torii_sorafs_storage_fetch_inflight`
  - PoR success/failure counters surfaced via `/v1/sorafs/capacity/state`

Following these drills ensures the embedded storage worker can ingest data,
survive restarts, respect configured quotas, and generate deterministic PoR
proofs before the node advertises capacity to the wider network.
