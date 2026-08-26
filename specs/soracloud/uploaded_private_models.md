# Soracloud Uploaded Models

Production V1 supports uploaded-model registration as a storage and registry
workflow only. Model bytes are encrypted and pinned through SoraFS, then
Soracloud records the approved SoraFS manifest digest plus deterministic roots,
byte counts, artifact metadata, and weight-version metadata.

The chain never stores uploaded model bytes, encrypted chunk payloads, or
process-local upload sessions. Torii exposes one signed mutation for the
uploaded-model path:

- `POST /v1/soracloud/model/upload/register`

Torii also exposes the authenticated registry read:

- `GET /v1/soracloud/model/upload/status`

There is no uploaded-model execution endpoint in V1. In particular, Torii does
not accept caller-supplied model weights, biases, plaintext inference input, or
encrypted-artifact receipt claims. The node does not publish an upload
recipient or retain a corresponding decryption secret. The registry schema also
contains no recipient, wrapped-key, KEM/AEAD, or decryption-policy metadata.

## Register Flow

1. The client normalizes the model repository into the accepted package format.
2. The client prepares the stored package locally. If an operator encrypts it,
   recipient selection and key custody remain entirely outside the V1 request,
   registry, and validator process.
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
5. Query `GET /v1/soracloud/model/upload/status` with exact-network canonical
   account authentication and verify the committed bundle, artifact, and
   weight-version bindings. Registration is complete at this point; do not
   route the encrypted bundle to a validator for execution.

## Chain State

`SoraUploadedModelBundleV1` is the canonical storage reference. It contains:

- service name;
- uploaded model id;
- weight version;
- model family and modalities;
- plaintext root, package format, and canonical bundle root;
- approved SoraFS manifest digest;
- chunk count and byte counts;
- chunk-manifest root;
- storage pricing snapshot.

`SoraModelArtifactRecordV1` and `SoraModelWeightVersionRecordV1` link the
uploaded model into the normal Soracloud model registry. For uploaded models,
their `source_provenance` uses `UserUpload` and points back to the uploaded
model id.

## Runtime Posture

Uploaded-model V1 is registry-only. Validators validate approved SoraFS pins
and commit metadata; they never decrypt a registered bundle or run inference
over caller plaintext. No execution receipt type or receipt-recording
instruction is part of the V1 data model, and the JavaScript, Kotlin, and Java
SDKs expose no execution or receipt helper.

This separation is deliberate: accepting caller-provided weights beside a
registered encrypted bundle would not prove that the registered bundle was
used, and keeping a general decryption key in the validator process would not
provide confidential execution. A future execution design must first define an
attested confidential runtime, bind decrypted bytes to the registered roots,
and ensure plaintext never crosses Torii or ledger state.

## Production Gates

Soracloud production deployments must enable `soracloud_runtime.production_mode`.
`soracloud_runtime.inrou.enabled` remains false by default. Explicit enablement
accepts exactly one Linux KVM PortableVM V1 backend and requires an equal
`portable_vm_uid`/`portable_vm_gid` pair selecting one of four canonical slots,
`70000` through `70003`, one exact operator-approved guest artifact, and exact
CPU, memory, and writable-storage budgets. Immutable guest-image materialization
has a separate non-zero byte bound: 4 GiB by default and at most 16 GiB. Backend,
accelerator, concurrency, supplementary-group, and control-path selectors are
retired; `backends` and `max_concurrent_vms` are unknown Inrou configuration
keys. A qualified host
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
budgets, and any host-local Hugging Face execution path outside Inrou.
Production profiles should state the runtime fee payer explicitly; `authority`
is the deterministic development default.

Generated public Hugging Face sources are authenticated metadata-only records
in V1. No service or apartment runtime is generated for them, and they expose
no inference handler or tool. The authoritative source is `Admitted`;
the runtime does not materialize it, advertise it as execution-ready, probe a host model
stack, start a resident worker, or publish an inference bridge. Registered model
metadata therefore has no execution surface in this release.

HF source admission uses one exact case-sensitive `namespace/repository`
identity and one full lowercase commit OID. Model-info must return that same
`modelId` and commit byte-for-byte and must be requested with `blobs=true`.
Torii admission selects GGUF, then SafeTensors, then PyTorch, requires
canonical LFS SHA-256 and positive size metadata for every selected shard, and
applies identical file-count, per-file, and aggregate byte limits. The
authoritative source profile records the selected shard count, authenticated
byte total, format, and domain-separated commitment to the sorted exact
path/size/digest set.

Production behavior is sourced from configuration, not environment variables.
An unqualified or disabled node does not advertise or materialize Inrou
capacity and actively withdraws a pre-existing local advert. Code-path
admission is not a claim of Linux/AArch64/KVM deployment qualification.

## Test Expectations

Focused V1 coverage should include:

- approved active SoraFS pin succeeds;
- missing, pending, and retired SoraFS pins fail;
- world state contains no uploaded-model bytes;
- uploaded-model execution, receipt, and recipient-discovery routes are absent;
- request bodies containing raw weights, biases, or plaintext input receive a
  route-level `404` before deserialization;
- no validator startup path creates or reads an uploaded-model decryption key;
- production and non-production parsing accept only the exact opt-in
  PortableVM V1 shape; retired selectors and programmatic shape drift are
  configuration or startup errors;
- capability publication remains absent until the exact production preflight
  passes; runtime tests cover namespace, cgroup, KVM, identity, filesystem,
  QMP, connector, and firewall checks;
- unavailable PortableVm capability rejects startup, and capability loss withdraws
  the host advert and reconciles affected placements;
- JavaScript, Kotlin, and Java SDK inventories contain no uploaded-model
  execution or receipt helper.
