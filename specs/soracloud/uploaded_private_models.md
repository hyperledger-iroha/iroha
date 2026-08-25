# Soracloud Uploaded Models

Production V1 supports uploaded-model registration as a storage and registry
workflow only. Model bytes are encrypted and pinned through SoraFS, then
Soracloud records the approved SoraFS manifest digest plus deterministic roots,
byte counts, artifact metadata, and weight-version metadata.

The chain never stores uploaded model bytes, encrypted chunk payloads, or
process-local upload sessions. Torii exposes one signed mutation for the
uploaded-model path:

- `POST /v1/soracloud/model/upload/register`

Torii also exposes upload readiness endpoints and reserves private-execution
routes:

- `GET /v1/soracloud/model/upload/encryption-recipient` (public, one response
  object capped at 64 KiB of encoded JSON)
- `GET /v1/soracloud/model/upload/status` (exact-network canonical account
  authentication)
- `POST /v1/soracloud/model/upload/private/execute` (fail-closed; unavailable in
  the first release)
- `GET /v1/soracloud/model/upload/private/receipts` (exact-network canonical
  account authentication; no receipts can exist in the first release)

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
5. Do not enable or advertise private execution in the first release. The
   execute route validates the exact registered bundle, committed decryption
   request, policy, and encrypted input reference, then returns a conflict
   response without loading model plaintext, publishing output, or emitting a
   receipt. Receipt instructions are rejected by consensus and persisted
   private receipts are rejected during state restore.

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

Private uploaded-model execution is not part of the first-release correctness
surface. A safe implementation still requires all of the following as one
atomic, consensus-verifiable path:

- loading and authenticating the exact finalized SoraFS bundle;
- opening it only under the exact committed decryption authorization;
- deterministic inference over ciphertext-referenced input;
- encrypted output publication and pinning before success is observable; and
- a self-contained validator attestation that binds the decryption request,
  input artifact, output artifact, runtime, and exact model release.

Until that path exists, Torii never accepts caller-supplied model weights,
plaintext, output claims, execution-host claims, or policy overrides. The
execute request contains only the exact service/model selectors, committed
`decryption_request_id`, and encrypted input artifact. A valid request ends in
an explicit unavailable/conflict response; it does not produce an instruction
skeleton or receipt. Consensus receipt persistence and snapshot restore both
fail closed.

SDK request/query helpers may encode the reserved wire shape, but applications
must treat the route as unavailable and must not implement a signing or receipt
submission workflow for this release.

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

- approved active SoraFS pin succeeds;
- missing, pending, and retired SoraFS pins fail;
- world state contains no uploaded-model bytes;
- private execution accepts only the ciphertext-reference request shape and
  validates the exact committed decryption authorization before returning an
  explicit unavailable/conflict response;
- private execution emits neither a receipt nor a transaction instruction;
- consensus receipt recording and snapshot restore reject every private
  uploaded-model receipt until the complete artifact execution path exists;
- production and non-production parsing accept only the exact opt-in
  PortableVM V1 shape; retired selectors and programmatic shape drift are
  configuration or startup errors;
- capability publication remains absent until the exact production preflight
  passes; runtime tests cover namespace, cgroup, KVM, identity, filesystem,
  QMP, connector, and firewall checks;
- unavailable PortableVm capability rejects startup, and capability loss withdraws
  the host advert and reconciles affected placements;
- SDK Soracloud helpers do not accept raw private keys and clearly expose the
  private execution route as unavailable for the first release.
