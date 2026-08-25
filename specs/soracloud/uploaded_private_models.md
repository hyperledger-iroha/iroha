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
  runtime-owned deterministic execution and atomic submission)
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
   identity, runtime-owned journal, and atomic mutation sink together. The
   execute route accepts only an authenticated exact service/model release,
   committed decryption request, rooted encrypted input reference, and typed
   output recipient. Treat `202 Accepted` as transaction submission; use the
   committed receipt query or an exact `200 OK` replay as commit evidence.

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
exactly one of `model_id` or `model_name`; an optional exact `bundle_root`
constraint; the committed `decryption_request_id`; a rooted `input`-role
`SoraPrivateModelArtifactRefV1`; and the typed
`SoraUploadedModelEncryptionRecipientV1` for the output. The input reference
binds its SoraFS manifest digest, root CID, ciphertext hash, and ciphertext
length, with encrypted artifacts restricted to `1..=72 MiB` in V1. The output
recipient carries one canonical non-low-order X25519 public key and its exact
Iroha BLAKE2b-256 prehash fingerprint. Plaintext, model weights, policy
overrides, execution-host claims, output claims, and receipts are not request
fields.

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
match the service version, bundle policy, and input ciphertext commitment and
remain active at the candidate block height.

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
only when its content-addressed bytes match exactly.

`SoracloudPrivateUploadedModelExecutionJournalV1` is the runtime-owned durable
outbox keyed by service and decryption request. It stores only the complete
request fingerprint, canonical output manifest, receipt submission, and latest
signed transaction; decrypted model, input, and output payloads are absent.
The runtime durably writes `Prepared(0)` after output ingestion, then journals
the exact validator-signed transaction before enqueue. Recovery verifies the
output is still readable, reuses a pending transaction, replaces an expired
transaction within the three-attempt bound, removes committed or
authorization-expired entries, and retains terminal evidence for a permanently
rejected transaction without re-executing.

The signed transaction contains one
`RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction. Its authority
must be the receipt's exact active public-lane validator attester. Consensus
revalidates the finalized model, service revision, decryption authorization and
audit event, authorization height, and input pin. It verifies that the
canonical output manifest matches the receipt, then atomically registers or
reuses the output pin and persists the validator receipt. Consensus assigns
`emitted_sequence` and `emitted_block_height`; exact transaction replay is a
no-op, while different evidence cannot reuse a receipt id or consume the same
service/decryption request. Both ledger coordinates use the complete unsigned
64-bit range; Java and Kotlin expose them as `BigInteger` so no valid chain
height or sequence is narrowed to a signed `long`.

A fresh or durably recovered submission returns HTTP `202 Accepted` with
`submission_status = "submitted"`, the signed transaction hash, receipt
submission, and encrypted output reference. This is admission evidence, not
commit evidence. Once the exact receipt is visible in world state, replaying
the request returns HTTP `200 OK` with `submission_status = "committed"`, no
transaction hash, and the ledger-owned receipt. Concurrent and pre-commit
exact retries are coalesced without executing the released ciphertext twice.

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
- execute decoding requires explicit selector keys, the service version,
  committed release id, rooted input artifact, and typed output recipient and
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
- the validator-signed instruction atomically registers the matching output
  manifest and receipt, assigns sequence and block height, treats exact replay
  idempotently, and rejects duplicate decryption-request or conflicting
  receipt evidence;
- HTTP behavior distinguishes `202 submitted` from `200 committed` replay,
  coalesces concurrent and pre-commit retries, and exposes committed receipts
  through authenticated filtered queries;
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
