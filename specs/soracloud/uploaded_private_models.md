# Soracloud Uploaded Models

Production V1 supports uploaded-model registration as a storage and registry
workflow only. Model bytes are encrypted and pinned through SoraFS, then
Soracloud records the approved SoraFS manifest digest plus deterministic roots,
byte counts, artifact metadata, and weight-version metadata.

The chain never stores uploaded model bytes, encrypted chunk payloads, or
process-local upload sessions. Torii exposes one signed mutation for the
uploaded-model path:

- `POST /v1/soracloud/model/upload/register`

Torii also exposes upload readiness and deterministic private execution
endpoints:

- `GET /v1/soracloud/model/upload/encryption-recipient` (public, one response
  object capped at 64 KiB of encoded JSON)
- `GET /v1/soracloud/model/upload/status` (exact-network canonical account
  authentication)
- `POST /v1/soracloud/model/upload/private/execute`
- `GET /v1/soracloud/model/upload/private/receipts` (exact-network canonical
  account authentication; private, non-storable responses)

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
5. For private execution, require clients to call
   `POST /v1/soracloud/model/upload/private/execute` and inspect the returned
   receipt. Route the returned
   `RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction to a
   controlled `CanManageSoracloud` authority for signing and submission;
   validators and ordinary client authorities cannot commit this privileged
   ledger projection. The JavaScript helper returns an unsigned instruction
   skeleton only; raw private keys must not be embedded in helper inputs,
   browser payloads, logs, or repository files.
6. After submission, query
   `GET /v1/soracloud/model/upload/private/receipts` with the expected service,
   model id, weight version, and `count_mode=exact` when an operator needs an
   audited total. Sign the exact GET path and sorted query with the configured
   NetworkId and local account key; an API token is not a substitute for this
   proof. Compare the committed receipt commitments and encrypted artifact
   references with the runtime response before marking the release complete.

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
model id.

## Runtime Posture

Production V1 exposes only the deterministic quantized CPU private runtime for
uploaded models admitted with `DeterministicQuantizedCpuV1`. The CPU runtime is
the authoritative semantics: fixed signed-integer linear operations,
nearest-away-from-zero rounding, saturating output bounds, and stable receipt
commitments. Hardware acceleration is not part of the V1 correctness surface.

The private execution route validates that the finalized uploaded-model bundle
is backed by an active approved SoraFS pin, validates encrypted input and output
artifact references against active approved SoraFS pins, executes the CPU
reference runtime, and returns:

- `SoraPrivateUploadedModelExecutionReceiptV1`, containing only commitments,
  runtime version, policy id, bundle identifiers, and encrypted artifact
  references;
- `tx_instructions`, containing a canonical
  `RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction payload for
  signing and transaction submission by a `CanManageSoracloud` authority.

Plaintext input and output are runtime-local and must not be written to chain
state. Committed chain state stores receipt commitments and encrypted artifact
references only. Receipt identifiers are append-only: a later instruction
cannot replace an already committed receipt.

Committed private uploaded-model receipts can be queried with optional
`receipt_id`, `service_name`, `model_id`, `weight_version`, `limit`, and
`count_mode` filters. The list response defaults to `count_mode=bounded` and
accepts explicit `count_mode=exact` when clients need a `total`. It includes
pagination metadata (`returned_items`, `remaining_items`, `has_more`,
`count_mode`) alongside the receipt records.

The JavaScript SDK exposes unsigned helpers for this V1 flow:
`buildSoracloudPrivateUploadedModelExecuteRequest`,
`buildSoracloudPrivateUploadedModelReceiptQuery`, and
`privateUploadedModelReceiptInstruction`. These helpers normalize the Torii
request/query shapes, reject embedded signing secrets, and extract the returned
receipt instruction skeleton for external transaction signing by a controlled
`CanManageSoracloud` authority.

The Kotlin core SDK and Java Android SDK mirror the client-visible response
parsers for private execute and committed receipt-list responses. Both expose a
helper that extracts the
`RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction skeleton from
the Torii response so mobile clients can pass it to a manager-controlled
external transaction signing pipeline.

## Production Gates

Soracloud production deployments must enable `soracloud_runtime.production_mode`.
The first release fails closed whenever Inrou hosting is enabled, in both
production and non-production configuration. The current QEMU boundary does not yet provide mandatory mount,
network, IPC, and MAC isolation; QEMU's deny-list seccomp sandbox and uid-based
IP firewall cannot contain all host capabilities such as AF_UNIX. Production
nodes, including Taira, therefore advertise no Inrou hosting capability.

The built-in PortableVM implementation is retained only behind private unit
test fixtures while a mandatory privileged confinement launcher is developed.
It intrinsically means one Linux KVM VM; the former backend, accelerator,
concurrency, and supplementary-group selectors are retired. Its defense-in-depth
checks require dedicated `portable_vm_uid`/`portable_vm_gid` values in
`65536..524288` and exact resource budgets, with the sole supplementary gid
derived from the validated `/dev/kvm` device. A future qualified host must use
exactly `passwd: files` and `group: files` in a
root-custodied `/etc/nsswitch.conf`. The exact locked local account/group name
is `iroha-inrou` (uid/gid `70000` is the recommended qualification profile): password fields `x`, home
`/nonexistent`, trusted nologin/false shell, locked shadow/gshadow entries, and
no extra membership, administrator entry, duplicate, decimal-name collision,
or primary-gid reuse. `subid` NSS, when declared, must use only `files`; no
subordinate range may belong to that identity/numeric spelling or cover a
configured id. Reused disks require Linux write-lease support. Root-custodied
`iptables` and `ip6tables` are required. PortableVM V1 accepts only `Isolated`
networking until kernel-owned traffic counters provide complete egress
accounting; `Open` and `Allowlist` are rejected.
Production mode also rejects broad runtime egress, missing fail-closed egress
budgets, and Hugging Face inference-bridge fallback. Production profiles should
state the runtime fee payer explicitly; `authority` is the deterministic
development default.

Production behavior is sourced from configuration, not environment variables.
Shipping configuration rejects enabled Inrou before startup or reconciliation.
A disabled node does not advertise hosting capacity and actively withdraws a
pre-existing local advert.

## Test Expectations

Focused V1 coverage should include:

- approved active SoraFS pin succeeds;
- missing, pending, and retired SoraFS pins fail;
- world state contains no uploaded-model bytes;
- deterministic quantized CPU private execution emits stable receipts and a
  receipt-recording transaction instruction;
- receipt recording rejects validators and other authorities that do not hold
  `CanManageSoracloud`;
- receipt recording rejects non-deterministic uploaded-model formats and
  mismatched manifest, bundle-root, or policy bindings;
- production config and programmatic manager construction reject enabled Inrou
  hosting; retired backend, accelerator, concurrency, and supplementary-group
  selectors are configuration errors;
- non-production config and direct manager construction also reject enabled
  Inrou; private runtime tests cover the KVM, identity, and custody checks;
- unavailable PortableVm capability rejects startup, and capability loss withdraws
  the host advert and reconciles affected placements;
- JavaScript Soracloud helpers expose unsigned drafts and do not accept raw
  private keys.
