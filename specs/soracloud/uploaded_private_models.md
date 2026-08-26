# Soracloud Uploaded Models

Production V1 supports uploaded-model registration and runtime-owned private
execution. Model, input, and output plaintext never enter Torii or world state.
SoraFS stores the encrypted artifacts; Soracloud records their canonical
content identities, finalized model metadata, and committed execution receipts.

The chain never stores uploaded model bytes, encrypted chunk payloads, or
process-local upload sessions. Torii exposes the signed registration mutation:

- `POST /v1/soracloud/model/upload/register`

Torii also exposes upload readiness and private-execution routes:

- `GET /v1/soracloud/model/upload/encryption-recipient` (public, one response
  object capped at 64 KiB of encoded JSON)
- `GET /v1/soracloud/model/upload/status` (exact-network canonical account
  authentication)
- `POST /v1/soracloud/model/upload/private/execute` (authenticated,
  runtime-owned deterministic execution and durable two-phase submission)
- `GET /v1/soracloud/model/upload/private/receipts` (exact-network canonical
  account authentication over committed receipts)

## Register Flow

1. The client normalizes the model repository into the accepted package format.
2. The client encrypts the package locally for the advertised Soracloud upload
   recipient.
3. The client pins the encrypted package through SoraFS.
4. The client waits until the SoraFS pin record is active and approved.
5. The client submits a signed `upload/register` request containing:
   - `SoraUploadedModelBundleV1`;
   - model artifact metadata;
   - model weight-version metadata;
   - independent provenance signatures for the bundle and final registry update.
6. Core validates the active approved SoraFS pin before committing Soracloud
   bundle, registry, artifact, and weight-version records.

Registration fails when the referenced SoraFS manifest digest is missing,
pending, retired, or does not match the committed bundle metadata.

## Operator Pin-And-Register Checklist

Before enabling an uploaded-model registration corridor, operators should
prepare the storage, runtime, and signing surfaces as one controlled change.

1. Build the encrypted model package locally and record the plaintext package
   root, encrypted bundle root, chunk-manifest root, ciphertext byte count, and
   chunk count. These values must match `SoraUploadedModelBundleV1`; model
   bytes and plaintext inputs stay off-chain.
2. Pin the encrypted package through SoraFS and wait for the pin registry to
   report an active approved manifest. Treat the approved
   `sorafs_manifest_digest`, content length, chunker profile, policy, and
   approval sequence as release evidence for the model bundle.
3. Configure the Soracloud runtime fee payer before accepting production
   traffic. Select either the validator authority or one exact sponsor-program
   revision. Runtime mutations are quoted against current ledger state, then
   only the returned `FeePaymentIntent` limits are inserted before signing.
4. Submit the signed `POST /v1/soracloud/model/upload/register` request with
   the uploaded bundle, model artifact metadata, weight-version metadata, and
   provenance signatures. The bundle provenance and final registry provenance
   must cover the exact committed roots, artifact id, weight version, dataset
   reference, policy id, and SoraFS digest.
5. Qualify the local deterministic runtime, embedded SoraFS storage, validator
   identity, runtime-owned journal, and phase-transaction sink together. The
   execute route accepts only an authenticated exact service/model release,
   committed decryption request, rooted encrypted input reference, and typed
   output recipient. Treat `202 Accepted` as progress evidence for its typed
   submission phase; use the committed receipt query or an exact `200 OK`
   replay as commit evidence.

## Chain State

`SoraUploadedModelBundleV1` is the canonical storage reference. It contains:

- service name;
- uploaded model id;
- weight version;
- model family and modalities;
- plaintext and encrypted bundle roots;
- approved SoraFS manifest digest;
- chunk count and byte counts;
- chunk-manifest root;
- upload recipient metadata;
- wrapped bundle-key metadata;
- storage pricing snapshot;
- decryption policy reference metadata.

`SoraModelArtifactRecordV1` and `SoraModelWeightVersionRecordV1` link the
uploaded model into the normal Soracloud model registry. For uploaded models,
their `source_provenance` uses `UserUpload` and points back to the uploaded
model id. V1 model ids and weight versions are `1..=128` ASCII bytes drawn
from letters, digits, and `[-_.:#]`. Service versions are exact, well-formed
Unicode-scalar, unpadded, control-free strings of at most 256 UTF-8 bytes.
Recipient key ids are also exact, unpadded, control-free Unicode-scalar
strings; no compatibility normalization is applied.
Finalization verifies provenance over the exact submitted spelling: model names
must already be NFC, artifact ids use the same canonical identifier alphabet,
and dataset references are unpadded, control-free strings of at most 512 UTF-8
bytes. A `(service_name, model_id, weight_version)` uploaded release is
finalized exactly once across the service, regardless of model-name or artifact
aliases; any pre-existing `UserUpload` projection rejects another finalization.

## Runtime Posture

`PrivateUploadedModelExecuteRequest` is a closed authenticated request. It
contains exact `service_name`, `service_version`, and `weight_version` values;
the canonical immutable `model_id`; its exact committed `bundle_root`; the
committed `decryption_request_id`; a rooted `input`-role
`SoraPrivateModelArtifactRefV1`; and the typed
`SoraUploadedModelEncryptionRecipientV1` for the output. The input reference
binds its SoraFS manifest digest, root CID, ciphertext hash, and ciphertext
length, with encrypted artifacts restricted to `1..=72 MiB` in V1. The output
recipient carries one canonical non-low-order X25519 public key and its exact
Iroha BLAKE2b-256 prehash fingerprint. Plaintext, model weights, policy
overrides, execution-host claims, output claims, receipts, and mutable
`model_name` discovery aliases are not request fields.

Torii verifies the canonical account-signature headers and requires one
verified signer to equal the signer recorded on the decryption request. The
selector must resolve one finalized
`DeterministicQuantizedCpuV1` bundle, and the exact retained service revision
must permit model inference. Finalization means exactly one `UserUpload`
weight projection and one linked artifact projection whose service/model keys,
weight-version links, source identity, provenance hashes, and chunk root agree
exactly. The artifact registration sequence must be the checked consecutive
successor of the weight registration sequence. `service_version` remains
lifecycle metadata: promotion may update the weight projection without
rewriting the immutable artifact, so request revision admission is enforced
separately. The model and input SoraFS pins must be active and match their
authoritative content identities. The decryption record and audit event must
match the service version, bundle policy, and input ciphertext commitment. A
fresh execution requires the half-open authorization window to remain active at
the next possible atomic prepare height. Prepare freezes that exact height and
consensus epoch in an immutable claim; provider completion and receipt commit
subsequently rely on the claim instead of extending or recreating the original
authorization. Fresh private output admission also requires permissionless
automatic SoraFS pin approval. Torii rejects a council-gated policy before
inference, and consensus independently rejects a fresh prepare under that
policy; an already exact-matched claim remains an idempotent no-op if governance
changes later.

The qualified runtime independently revalidates those bindings, reads the
encrypted model and input under admitted SoraFS leases, and opens only canonical
envelopes addressed to its persisted custody recipient. Envelope contexts bind
the service, service version, model id, weight version, policy, and decryption
request. Execution uses the bounded deterministic quantized-CPU V1 integer
rules. The runtime derives custody-blinded input and output commitments,
zeroizes decrypted working material, and encrypts the canonical result to the
request's typed recipient.

Torii derives a canonical single-file SoraFS manifest for the encrypted output,
inherits the input pin's governed storage policy, and durably ingests the
ciphertext before publishing recovery evidence. An existing output is reused
only when its content-addressed bytes match exactly. Before execution, after
inference, and after local output ingestion, both source pins must retain the
complete three-attempt Prepare horizon, four recovery intervals, and a fixed
60-second safety allowance. The output lease is paid separately from the later
of local wall time and finalized consensus time: it covers that full Prepare
horizon, the strict 24-hour automatic-replication SLA, and then the larger of
the governed settlement window or the complete three-attempt Receipt horizon
plus the 600-second consensus recovery floor.

`SoracloudPrivateUploadedModelExecutionJournalV1` is the runtime-owned durable
outbox keyed by service and decryption request. It stores only the complete
request fingerprint, canonical output manifest, receipt submission, and latest
signed transaction; decrypted model, input, and output payloads are absent.
The runtime durably enters the `Prepare` phase after output ingestion, then
journals the exact validator-signed transaction for `Prepare` and `Receipt`
before each enqueue. It first fsyncs the exact fee-quoted unsigned payload and
increments the attempt, then invokes the qualified signer. An unsigned
reservation recovered after a crash is conservatively consumed and replaced,
never signed again, so randomized signers cannot escape the hard three-attempt
cap through a sign-to-journal crash. Every produced signature therefore
consumes one of exactly three attempts for its phase, including one that is
immediately found too old to enqueue. A committed Prepare moves to the explicit
internal `AwaitingDurability` phase while retaining the signed Prepare evidence;
its HTTP projection deliberately has no transaction hash. Recovery verifies the
output is still readable, reuses a pending transaction, replaces an expired
transaction within the bound, removes committed entries, removes an
authorization-expired entry only before its claim is prepared, and retains
terminal evidence for a permanently rejected transaction without re-executing.
Malformed journal files are quarantined per entry so they cannot prevent later
valid entries from reconciling. Exhausted attempts, another validator's winning
claim, and a retry-stable incompatible output-pin collision are also retained as
terminal evidence without degrading unrelated entries; exact replay returns
HTTP `409 Conflict` unless the canonical receipt has since committed, in which
case that authoritative `200 OK` result wins.

The first transaction contains one
`PrepareSoracloudPrivateUploadedModelExecution` instruction carrying the exact
output `ManifestV1` and complete receipt submission. Consensus canonical-decodes
the manifest, registers its paid pin when absent or accepts only the exact live
existing pin, and persists the immutable authorization claim atomically while
the decryption release and validator attester are active. An exact retry is a
no-op; different evidence cannot reuse the service/decryption-request key. The
permissionless registration approves a new pin and creates its deterministic
automatic replication order in that same transaction; an existing pin must
already be approved at the claim epoch. The runtime then waits for the exact
`output_replication_order_id` to reach the configured provider quorum.
Automatic replication order record
epochs and provider payload timestamps are Unix seconds, and the governed
24-hour ingestion deadline must fit within the pin retention horizon. Its ID is
`BLAKE3("sorafs:auto-replication-order:v1" || manifest_digest)` with the high bit
of byte zero set to reserve the automatic namespace; generic and Musubi-purpose
orders must leave that bit clear. The automatic payload has the exact pin root
and chunker, one immutable assignment per required replica, the fixed V1 SLA,
and empty metadata. Missing eligible capacity fails pin registration or
approval atomically, and an existing derived-ID record is never overwritten.
An eligible provider declaration is exact-bound to its canonical payload and
the consensus second when it was registered, remains active through the whole
order deadline, matches the pin's explicit storage class and chunker profile,
and commits enough profile capacity for the manifest's checked GiB ceiling.
Completion authority cannot be revoked while an assigned pending order still
needs that provider. A pending order observed after its deadline is terminally
invalid rather than continuing to report an awaiting-quorum phase. Only then
does the runtime submit the second transaction, which contains one
`RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction. Both
transactions use the receipt's exact public-lane validator attester as
authority; active authorization and placement are checked when prepare freezes
the claim, not recreated at receipt time.

Prepare admission revalidates the finalized model, service revision,
decryption authorization and audit event, authorization height, active
validator placement, input pin, exact output manifest, and claim-time output
retention. Receipt admission requires that exact prepared claim, the approved
output pin, its canonical deterministic replication order, every assigned
provider completion needed for the governed replica count, and at least 600
seconds of post-receipt retention. Consensus
rejects explicit early retirement of a committed private-output pin until its
advertised retention epoch, so that admission-time promise remains enforceable.
It copies `authorization_claim_block_height` and
`authorization_claim_epoch` from the claim and assigns `emitted_sequence`,
`emitted_block_height`, and `emitted_epoch`; exact transaction replay is a
no-op, while different evidence cannot reuse a receipt id or consume the same
service/decryption request. All five ledger coordinates use the complete
unsigned 64-bit range; Java and Kotlin expose them as `BigInteger` so no valid
chain height, sequence, or consensus epoch is narrowed to a signed `long`.

Snapshot restore requires capacity declarations, private-execution claims, pin
manifests, and replication orders together. The signed snapshot authenticates
the ledger-assigned coordinates and complete world state before decoding;
historical epochs are not reconstructed from block bodies that may be absent in
an audited hash-only prefix. Restore still revalidates every claim against its
retained decryption-release height and timestamp and surrounding world state,
and every committed receipt must match its claim byte-for-byte after clearing
the five ledger-owned coordinates. Claim and receipt block heights must also
fall within the snapshot's committed hash prefix; internally consistent
future-height evidence is rejected. A retired model or input pin remains valid
evidence only when its immutable approval epoch and retention horizon prove
that it was already live and paid through the claim epoch. Every claim also
requires its exact canonical output order, issued no later than the claim and
not terminal before it, plus output retention through the order deadline and
the 600-second receipt-recovery floor. Pin and order lifecycle epochs have
one-second resolution, so a claim followed by retirement or cancellation later
in the same epoch remains restorable. Restore rejects an approved pin without
its one derived automatic order, an automatic order without its exact pin, or
any claim, declaration, payload, assignment, timestamp, status, or completion
projection that does not replay the same first-release invariants as live
admission.

A fresh or durably recovered request returns HTTP `202 Accepted` with one
closed `submission_phase` value, the receipt submission, and the encrypted
output reference. `awaiting_output_durability` has no transaction hash;
`prepare_submitted` and `receipt_submitted` carry the exact current-phase
transaction hash. These are progress evidence, not receipt-commit evidence.
Once the exact receipt is visible in world state, replaying the request returns
HTTP `200 OK` with `submission_phase = "committed"`, no transaction hash, and
the ledger-owned receipt. Concurrent and pre-commit exact retries are
coalesced without executing the released ciphertext twice. Pending response
receipts keep all five ledger-owned coordinates at zero; the committed variant
requires all five to be non-zero.

The authenticated receipts route queries committed world state by optional
`receipt_id`, `service_name`, `model_id`, and `weight_version`, with bounded or
exact count mode. Its `limit` is optional, defaults to `50`, and must be in
`1..=500`; out-of-range values are rejected instead of normalized. Results are
ordered by emitted sequence and receipt id. When another page exists, the
response carries one canonical opaque `continue_cursor`; pass it unchanged as
the next request's `cursor`. The token binds the exact filters and the first
page's ledger sequence snapshot, so later append-only receipts do not change an
in-progress traversal. `has_more` and cursor presence agree exactly. Bounded
mode leaves both `total` and `remaining_items` null and retains only a bounded
page candidate set; exact mode returns the snapshot-wide `total` and the exact
number of rows remaining after the current page. Every receipt carries the
SoraFS output reference needed to retrieve the encrypted result.

## Production Gates

Soracloud production deployments must enable `soracloud_runtime.production_mode`.
`soracloud_runtime.inrou.enabled` remains false by default. Explicit enablement
accepts exactly one Linux KVM PortableVM V1 backend and requires an equal
`portable_vm_uid`/`portable_vm_gid` pair selecting one of four canonical slots,
`70000` through `70003`, plus exact CPU, memory, and storage budgets. Backend, accelerator, concurrency,
supplementary-group, and control-path selectors are retired; `backends` and
`max_concurrent_vms` are unknown Inrou configuration keys. A qualified host
must use exactly `passwd: files` and `group: files` in a
root-custodied `/etc/nsswitch.conf`. Slot `i` requires the exact locked local
account/group name `iroha-inrou-i`; a same-host four-validator qualification
provisions all four slots and a single-validator public host uses
`iroha-inrou-0` at uid/gid `70000`: password fields `x`, home
`/nonexistent`, trusted nologin/false shell, locked shadow/gshadow entries, and
no extra membership, administrator entry, duplicate, decimal-name collision,
or primary-gid reuse. `subid` NSS, when declared, must use only `files`; no
subordinate range may belong to that identity/numeric spelling or cover a
configured id. Reused disks require Linux write-lease support.

Capability advertisement and local hosting activate only after the same
startup invocation authenticates the fixed QEMU runtime closure and passes the
KVM, identity, filesystem, anonymous-QMP, root-owned `iptables`, private
mount/network/IPC/UTS/PID/cgroup namespace, loopback-only connector, and exact
cgroup-v2 CPU/memory/swap/pids/I/O preflight. The minimal root exposes no host
`/run`, broad `/sys`, Unix socket, or unrelated descriptor. PortableVM V1
accepts only `Isolated` networking; `Open` and `Allowlist` are rejected.
Production mode also rejects broad runtime egress, missing fail-closed egress
budgets, and Hugging Face inference-bridge fallback. Production profiles should
state the runtime fee payer explicitly; `authority` is the deterministic
development default.

Generated public Hugging Face imports are authenticated inert storage in V1;
they are not a host-local execution backend. `local_execution_enabled = true`
is a configuration error in every mode, direct actual/runtime-manager
construction is checked again, and the resident Python worker boundary is
closed. Import reconciliation must not probe the host model stack, emit warmth
heartbeats, or project a fully hydrated source as locally `Ready`. Re-enabling
execution requires routing the worker through the same authenticated Inrou
isolation and resource corridor used for hostile service workloads.

HF source admission uses one exact case-sensitive `namespace/repository`
identity and one full lowercase commit OID. Model-info must return that same
`modelId` and commit byte-for-byte and must be requested with `blobs=true`.
Torii and the importer select GGUF, then SafeTensors, then PyTorch, require
canonical LFS SHA-256 and positive size metadata for every selected shard, and
apply identical file-count, per-file, and aggregate byte limits. The consensus
resource profile records the selected shard count, authenticated byte total,
format/backend family, and domain-separated commitment to the sorted exact
path/size/digest set.

Production behavior is sourced from configuration, not environment variables.
An unqualified or disabled node does not advertise or materialize Inrou
capacity and actively withdraws a pre-existing local advert. Code-path
admission is not a claim of Linux/AArch64/KVM deployment qualification.

## Test Expectations

Focused V1 coverage should include:

- registration accepts an exact approved SoraFS pin and rejects missing,
  pending, retired, or mismatched pin metadata;
- execute decoding requires the canonical model id, exact bundle root, service
  version, committed release id, rooted input artifact, and typed output recipient and
  rejects caller-controlled plaintext, policy, host, output, and receipt
  claims;
- authentication, exact service/model selection, release signer, audit
  binding, half-open authorization window, service inference capability, pin
  identity, retention, and local admitted bytes fail closed independently;
- deterministic quantized execution validates both encrypted envelope
  contexts and their commitments and binds repeatable output metadata to the
  exact request and recipient;
- encrypted output is durable before `Prepared(0)`, signed evidence is durable
  before enqueue, and restart recovery covers pending, unknown, expired,
  rejected, committed, and authorization-expired journal states;
- the validator-signed prepare phase atomically registers or exact-matches the
  output manifest and freezes authorization, waits for its deterministic order
  and exact replication quorum, and only then permits the receipt phase to copy
  the claim coordinates and assign sequence, block height, and commit epoch;
  exact replay is idempotent and duplicate decryption-request or conflicting
  receipt evidence is rejected;
- HTTP behavior reports the exact typed pending phase under `202` and
  `committed` under `200`, coalesces concurrent and pre-commit retries, and
  exposes committed receipts through authenticated filtered queries;
- world state and the durable journal contain no model, input, or output
  plaintext;
- production and non-production parsing accept only the exact opt-in
  PortableVM V1 shape; retired selectors and programmatic shape drift are
  configuration or startup errors;
- capability publication remains absent until the exact production preflight
  passes; runtime tests cover namespace, cgroup, KVM, identity, filesystem,
  QMP, connector, and firewall checks;
- unavailable PortableVm capability rejects startup, and capability loss withdraws
  the host advert and reconciles affected placements; and
- SDK helpers encode the exact request and receipt-query shapes without
  accepting raw private keys.
