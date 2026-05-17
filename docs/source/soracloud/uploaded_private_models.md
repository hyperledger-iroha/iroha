# Soracloud User-Uploaded Models and Private Runtime

This note defines how So Ra's user-uploaded model flow should land on the
existing Soracloud model plane without inventing a parallel runtime.

## Design Goal

Production V1 adds a Soracloud-only uploaded-model registration path that lets
clients:

- upload their own model repositories;
- register a pinned, approved SoraFS package as a model artifact and weight
  version; and
- query authoritative storage/registry readiness.

Binding uploaded models to apartments, private inference, output release,
compile jobs, and runtime receipts are intentionally out of scope for production
V1 until a real deterministic private runtime exists.

This is not a `ram_lfe` feature. `ram_lfe` stays the generic hidden-function
subsystem documented in `../universal_accounts_guide.md`. Uploaded-model
private inference should instead extend Soracloud's existing model registry,
artifact, apartment-capability, FHE, and decryption-policy surfaces.

## Existing Soracloud Surfaces To Reuse

The current Soracloud stack already has the right base objects:

- `SoraModelRegistryV1`
  - authoritative per-service model name and promoted version state.
- `SoraModelWeightVersionRecordV1`
  - version lineage, promotion, rollback, provenance, and reproducibility
    hashes.
- `SoraModelArtifactRecordV1`
  - deterministic artifact metadata already tied to the model/weight pipeline.
- `SoraCapabilityPolicyV1.allow_model_inference`
  - apartment/service capability flag that should become mandatory for
    apartments bound to uploaded models.
- `FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1`, and `DecryptionRequestV1`
  - future policy/governance surfaces for encrypted execution and controlled
    output release.
- current Torii model routes:
  - `/v1/soracloud/model/weight/{register,promote,rollback,status}`
  - `/v1/soracloud/model/artifact/{register,status}`
- current Torii HF shared-lease routes:
  - `/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

The uploaded-model path should extend those surfaces. It should not overload
HF shared leases, and it should not reuse `ram_lfe` as a model-serving runtime.

## Canonical Upload Contract

Soracloud's private uploaded-model contract should admit only canonical
Hugging Face-style model repositories:

- required base files:
  - `config.json`
  - tokenizer files
  - processor/preprocessor files when the family requires them
  - `*.safetensors`
- admitted family groups in this milestone:
  - decoder-only causal LMs with RoPE/RMSNorm/SwiGLU/GQA semantics
  - LLaVA-style text+image models
  - Qwen2-VL-style text+image models
- rejected in this milestone:
  - GGUF as the uploaded private-runtime contract
  - ONNX
  - missing tokenizer/processor assets
  - unsupported architectures/shapes
  - audio/video multimodal packages

Why this contract:

- it matches the already dominant server-side model layout used by the existing
  ecosystem around safetensors and Hugging Face repos;
- it lets the model plane share one deterministic normalization path across
  So Ra, Torii, and runtime compilation; and
- it avoids conflating local-runtime import formats such as GGUF with the
  Soracloud private-runtime contract.

HF shared leases remain useful for shared/public source import workflows. The
uploaded-model path stores model bytes only in SoraFS; chain state stores the
approved manifest digest, roots, byte counts, and registry/artifact metadata.

## Production Posture

Soracloud production deployments must enable `soracloud_runtime.production_mode`
in node configuration and build `irohad` with `embedded-soracloud-runtime`.
Production mode rejects a config that leaves Inrou disabled, enables proxy-only
Inrou host posture, omits the explicit runtime submission gas asset, leaves
broad runtime egress open, omits egress rate/byte budgets, or enables Hugging
Face inference-bridge fallback. This prevents a node from advertising a
production runtime while refusing local placement work or relying on ambient
environment state for production submissions.
The fallback is disabled by default so a production node cannot silently route
private-model execution through the hosted bridge path.

Signed Soracloud mutation requests are body-limited before signature
verification. Use `torii.soracloud_mutation_max_body_bytes` for ordinary
mutation routes and `torii.soracloud_upload_max_body_bytes` for the
uploaded-model register route. Oversized bodies fail with `413 Payload Too
Large` before any canonical-request verification work is performed. After
signature verification,
signed Soracloud POST routes are also limited by
`torii.soracloud_mutation_rate_per_account_origin_per_sec`,
`torii.soracloud_mutation_burst_per_account_origin`, and
`torii.soracloud_mutation_max_inflight`; the rate key combines the verified
account, the request `Origin`, and the Soracloud route group (`mutation`,
`upload`, `model`, or `hf`). Public runtime routes keep separate remote-IP rate
and inflight knobs.

## Layered Model-Plane Design

### 1. Provenance and registry layer

Extend the current model registry design instead of replacing it:

- add `SoraModelProvenanceKindV1::UserUpload`
  - current kinds (`TrainingJob`, `HfImport`) are not enough to distinguish a
    model uploaded and normalized directly by a client such as So Ra.
- keep `SoraModelRegistryV1` as the promoted-version index.
- keep `SoraModelWeightVersionRecordV1` as the lineage/promote/rollback record.
- extend `SoraModelArtifactRecordV1` with optional uploaded-private-runtime
  references:
  - `private_bundle_root`
  - `chunk_manifest_root`
  - `compile_profile_hash`
  - `privacy_mode`

The artifact record remains the deterministic anchor that ties provenance,
reproducibility metadata, and bundle identity together. The weight record
remains the promoted-version lineage object.

### 2. Bundle storage-reference layer

Add a first-class Soracloud record for uploaded-model storage metadata:

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `sorafs_manifest_digest`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `chunk_manifest_root`
  - `pricing_policy`
  - `decryption_policy_ref`

Deterministic rules:

- local packaging may shard bytes before SoraFS pinning, but only roots, byte
  counts, and the approved SoraFS `ManifestDigest` are committed to chain state;
- chunk/root digests are stable metadata only; and
- core rejects registration unless the referenced SoraFS pin is active and
  approved.

Production V1 does not store literal encrypted model bytes in chain state and
does not expose a chain-resident upload-chunk append flow.

Because the chain is public, upload confidentiality has to come from a real
Soracloud-held recipient key, not from deterministic keys derived from public
metadata. The desktop should fetch an advertised upload-encryption recipient,
encrypt chunks under a random per-upload bundle key, and publish only the
recipient metadata plus wrapped bundle-key envelope alongside the ciphertext.

### 3. Future compile/runtime layer

A later release may add a dedicated private transformer compiler/runtime layer
under Soracloud:

- standardize on BFV-backed deterministic low-precision compiled inference for
  now, because CKKS exists in schema discussion but not in the implemented
  local runtime;
- compile admitted models into a deterministic Soracloud private IR that covers
  embeddings, linear/projector layers, attention matmuls, RoPE, RMSNorm /
  LayerNorm approximations, MLP blocks, vision patch projection, and
  image-to-decoder projector paths;
- use deterministic fixed-point inference with:
  - int8 weights
  - int16 activations
  - int32 accumulation
  - approved polynomial approximations for non-linearities

This compiler/runtime remains separate from `ram_lfe`. It may reuse BFV
primitives and Soracloud FHE governance objects, but it is not the same
execution engine or route family.

### 4. Future inference/session layer

A later release may add session and checkpoint records for private runs:

- `SoraPrivateCompileProfileV1`
  - `family`
  - `quantization`
  - `opset_version`
  - `max_context`
  - `max_images`
  - `vision_patch_policy`
  - `fhe_param_set`
  - `execution_policy`
- `SoraPrivateInferenceSessionV1`
  - `session_id`
  - `apartment`
  - `model_id`
  - `weight_version`
  - `bundle_root`
  - `input_commitments`
  - `token_budget`
  - `image_budget`
  - `status`
  - `receipt_root`
  - `xor_cost_nanos`
- `SoraPrivateInferenceCheckpointV1`
  - `session_id`
  - `step`
  - `ciphertext_state_root`
  - `receipt_hash`
  - `decrypt_request_id`
  - `released_token`
  - `compute_units`
  - `updated_at_ms`

When private execution is reintroduced, it must mean:

- encrypted prompt/image inputs;
- encrypted model weights and activations;
- explicit decryption-policy release for output;
- public runtime receipts and cost accounting.

It must not mean hidden execution without commitments or auditability. Production
V1 exposes none of these runtime/session routes.

## Client Responsibilities

So Ra or another client should perform deterministic local preprocessing before
the upload reaches Soracloud:

- tokenizer application;
- image preprocessing into patch tensors for admitted text+image families;
- deterministic bundle normalization;
- client-side encryption of token ids and image patch tensors.

Torii should receive encrypted inputs plus public commitments, not raw prompt
text or raw images, for the private path.

## API and ISI Plan

Keep the existing model registry routes as the canonical registry layer. In
production V1, uploaded model bytes live only in SoraFS and Torii exposes only
storage/registry readiness:

- `POST /v1/soracloud/model/upload/register`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `GET /v1/soracloud/model/upload/status`

The signed register request carries a `SoraUploadedModelBundleV1` that points to
an approved active SoraFS `ManifestDigest`, plus model-artifact and
weight-version metadata. Core validates the SoraFS pin before recording the
Soracloud bundle, registry, and artifact records. It never stores encrypted
model bytes or upload chunks in world state.

The flow is:

1. the client encrypts and packages model bytes locally;
2. the client pins the encrypted package through SoraFS;
3. the client waits until the pin record is active and approved;
4. `upload/register` records only registry, artifact, roots, byte counts, and
   digest metadata;
5. `upload/status` reports registry/storage readiness.

Production V1 does not expose compile, allow-model, run-private, run-status, or
decrypt-output routes. Those stay out of scope until a real deterministic
private runtime exists.

## Pricing and Control-Plane Policy

Extend the current Soracloud charging/control-plane behavior:

- `allow_model_inference` is required for apartments running uploaded models;
- price storage, compilation, runtime steps, and decryption release in XOR;
- keep narrative propagation disabled for uploaded-model runs in this milestone;
- keep uploaded models inside So Ra's closed arena and export-gated flows.

## Authorization and Binding Semantics

Upload registration is a storage and registry action only.

- uploading a model bundle must not implicitly authorize an apartment to run it;
- no compile profile, private session, or output-release record is created in
  production V1;
- mutation routes should continue to require the same Soracloud signed-request
  discipline used by the existing model/artifact/training routes and should be
  guarded by `CanManageSoracloud` or an equally explicit delegated authority
  model.

This prevents "I uploaded it, therefore an apartment can run it" drift and keeps
execution policy out of production V1 until deterministic runtime support
exists.

## Status and Audit Model

The new records need authoritative read and audit surfaces, not just mutation
routes.

Recommended additions:

- upload status
  - query by `service_name + model_name + weight_version` or by
    `model_id + bundle_root`;
- storage readiness
  - expose the approved SoraFS manifest digest and recorded roots/byte counts.

Audit should stay on the existing Soracloud global sequence rather than
creating a second per-feature counter. Add first-class audit events for:

- upload register / registry finalization
- the SoraFS pin digest and lifecycle metadata used for the registered bundle

That keeps uploaded-model activity visible in the same authoritative replay and
operations story as the current service, training, model-weight, model-artifact,
HF shared-lease, and apartment audit streams.

## Admission Quotas and State-Growth Limits

SoraFS-backed uploaded-model registration is still state growth and availability
sensitive, so admission remains bounded.

The implementation should define deterministic limits for at least:

- max plaintext bytes per uploaded bundle;
- max encrypted bytes per bundle;
- max chunk count metadata per bundle;
- max concurrent register requests per authority/service; and
- maximum accepted register-request body size.

Torii and core should reject uploads that exceed those declared limits before
state amplification occurs. The limits should be configuration-driven where
appropriate, but validation outcomes must remain deterministic across peers.

## Replay and Compiler Determinism

The future private compiler/runtime path has a higher determinism burden than a
normal service deployment.

Required invariants:

- family detection and normalization must produce a stable canonical bundle
  before any compile hash is emitted;
- compile profile hashes must bind:
  - normalized bundle root,
  - family,
  - quantization recipe,
  - opset version,
  - FHE parameter set,
  - execution policy;
- the runtime must avoid nondeterministic kernels, floating-point drift, and
  hardware-specific reductions that could change outputs or receipts across
  peers.

Before exposing private compile or runtime routes, land tiny deterministic
fixtures for each family class and lock compile outputs plus runtime receipts
with golden tests.

## Remaining Design Gaps Before Code

The largest unresolved implementation questions are now narrowed to future
runtime decisions:

- whether model/artifact status should become version-oriented rather than
  training-job-oriented when `UserUpload` is present;
- the precise revocation behavior when a pinned SoraFS manifest is retired; and
- the deterministic private runtime design needed before compile/run/decrypt
  routes can return.

Those are the next design-to-code bridge items. The architectural placement of
the feature should now be stable.

## Test Matrix

- upload validation:
  - accept canonical HF safetensors repos
  - reject GGUF, ONNX, missing tokenizer/processor assets, unsupported
    architectures, and audio/video multimodal packages
- SoraFS storage:
  - active approved pin required
  - missing or retired pins rejected
  - no uploaded-model bytes in world state
- registry consistency:
  - bundle/artifact/weight promotion correctness under replay
- future compiler:
  - one small fixture each for decoder-only, LLaVA-style, and Qwen2-VL-style
  - rejection for unsupported ops and shapes
- future private runtime:
  - encrypted tiny-fixture end-to-end smoke test with stable receipts and
    threshold output release
- pricing:
  - XOR charges for upload registration and storage
- So Ra integration:
  - encrypt/package, pin through SoraFS, register, inspect readiness
- safety:
  - no export-gate bypass
  - no narrative auto-propagation
- private runtime routes remain unavailable in production V1

## Implementation Slices

1. Keep SoraFS pin lifecycle and uploaded-model registry status aligned.
2. Add operator runbooks for packaging, pin approval, and `upload/register`.
3. Add a real deterministic private transformer runtime before restoring any
   compile, allow-model, private-run, run-status, or decrypt-output route.
4. Land So Ra integration around the SoraFS pin/register readiness flow first.
