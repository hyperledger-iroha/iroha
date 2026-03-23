# Soracloud User-Uploaded Models and Private Runtime

This note defines how So Ra's user-uploaded model flow should land on the
existing Soracloud model plane without inventing a parallel runtime.

## Design Goal

Add a Soracloud-only uploaded-model system that lets clients:

- upload their own model repositories;
- bind a pinned model version to an agent apartment or closed-arena team;
- run private inference with encrypted inputs and encrypted model/state; and
- receive public commitments, receipts, pricing, and audit trails.

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
- `SecretEnvelopeV1` and `CiphertextStateRecordV1`
  - deterministic encrypted-byte and ciphertext-state carriers.
- `FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1`, and `DecryptionRequestV1`
  - the policy/governance layer for encrypted execution and controlled output
    release.
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

HF shared leases remain useful for shared/public source import workflows, but
the private uploaded-model path stores encrypted compiled bytes on-chain rather
than leasing model bytes from a shared source pool.

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

### 2. Bundle/chunk storage layer

Add first-class Soracloud records for encrypted uploaded-model material:

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `pricing_policy`
  - `decryption_policy_ref`
- `SoraUploadedModelChunkV1`
  - `model_id`
  - `bundle_root`
  - `ordinal`
  - `offset_bytes`
  - `plaintext_len`
  - `ciphertext_len`
  - `ciphertext_hash`
  - encrypted payload (`SecretEnvelopeV1`)

Deterministic rules:

- plaintext bytes are sharded into fixed 4 MiB chunks before encryption;
- chunk ordering is strict and ordinal-driven;
- chunk/root digests are stable across replay; and
- each encrypted shard must stay below the current `SecretEnvelopeV1`
  ciphertext ceiling of `33,554,432` bytes.

This milestone stores literal encrypted bytes in chain state through chunk
records. It does not offload private uploaded-model bytes to SoraFS.

Because the chain is public, upload confidentiality has to come from a real
Soracloud-held recipient key, not from deterministic keys derived from public
metadata. The desktop should fetch an advertised upload-encryption recipient,
encrypt chunks under a random per-upload bundle key, and publish only the
recipient metadata plus wrapped bundle-key envelope alongside the ciphertext.

### 3. Compile/runtime layer

Add a dedicated private transformer compiler/runtime layer under Soracloud:

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

This compiler/runtime is separate from `ram_lfe`. It may reuse BFV primitives
and Soracloud FHE governance objects, but it is not the same execution engine
or route family.

### 4. Inference/session layer

Add session and checkpoint records for private runs:

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

Private execution means:

- encrypted prompt/image inputs;
- encrypted model weights and activations;
- explicit decryption-policy release for output;
- public runtime receipts and cost accounting.

It does not mean hidden execution without commitments or auditability.

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

Keep the existing model registry routes as the canonical registry layer and add
new upload/runtime routes on top:

- `POST /v1/soracloud/model/upload/init`
- `POST /v1/soracloud/model/upload/chunk`
- `POST /v1/soracloud/model/upload/finalize`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `POST /v1/soracloud/model/compile`
- `POST /v1/soracloud/model/allow`
- `POST /v1/soracloud/model/run-private`
- `GET /v1/soracloud/model/run-status`
- `POST /v1/soracloud/model/decrypt-output`

Back them with matching Soracloud ISIs:

- bundle registration
- chunk append/finalize
- compile admission
- private run start
- checkpoint record
- output release

The flow should be:

1. upload/init establishes the deterministic bundle session and expected root;
2. upload/chunk appends encrypted shards in ordinal order;
3. upload/finalize seals the bundle root and manifest;
4. compile produces a deterministic private compile profile bound to the
   admitted bundle;
5. model/artifact + model/weight registry records reference the uploaded bundle
   rather than a training job alone;
6. allow-model binds the uploaded model to an apartment that already admits
   `allow_model_inference`;
7. run-private records a session and emits checkpoints/receipts;
8. decrypt-output releases governed output material.

## Pricing and Control-Plane Policy

Extend the current Soracloud charging/control-plane behavior:

- `allow_model_inference` is required for apartments running uploaded models;
- price storage, compilation, runtime steps, and decryption release in XOR;
- keep narrative propagation disabled for uploaded-model runs in this milestone;
- keep uploaded models inside So Ra's closed arena and export-gated flows.

## Authorization and Binding Semantics

Upload, compile, and run are separate capabilities and should stay separate in
the model plane.

- uploading a model bundle must not implicitly authorize an apartment to run it;
- compile success must not implicitly promote a model version to current;
- apartment binding should be explicit through an `allow-model` style mutation
  that records:
  - apartment,
  - model id,
  - weight version,
  - bundle root,
  - privacy mode,
  - signer / audit sequence;
- apartments bound to uploaded models must already admit
  `allow_model_inference`;
- mutation routes should continue to require the same Soracloud signed-request
  discipline used by the existing model/artifact/training routes and should be
  guarded by `CanManageSoracloud` or an equally explicit delegated authority
  model.

This prevents "I uploaded it, therefore every private apartment can run it"
drift and keeps apartment execution policy explicit.

## Status and Audit Model

The new records need authoritative read and audit surfaces, not just mutation
routes.

Recommended additions:

- upload status
  - query by `service_name + model_name + weight_version` or by
    `model_id + bundle_root`;
- compile status
  - query by `model_id + bundle_root + compile_profile_hash`;
- private run status
  - query by `session_id`, with apartment/model/version context included in the
    response;
- decrypt-output status
  - query by `decrypt_request_id`.

Audit should stay on the existing Soracloud global sequence rather than
creating a second per-feature counter. Add first-class audit events for:

- upload init / finalize
- chunk append / seal
- compile admitted / compile rejected
- apartment model allow / revoke
- private run start / checkpoint / completion / failure
- output release / deny

That keeps uploaded-model activity visible in the same authoritative replay and
operations story as the current service, training, model-weight, model-artifact,
HF shared-lease, and apartment audit streams.

## Admission Quotas and State-Growth Limits

Literal encrypted model bytes on-chain are viable only if admission is bounded
aggressively.

The implementation should define deterministic limits for at least:

- max plaintext bytes per uploaded bundle;
- max encrypted bytes per bundle;
- max chunk count per bundle;
- max concurrent in-flight upload sessions per authority/service;
- max compile jobs per service/apartment window;
- max retained checkpoint count per private session;
- max output-release requests per session.

Torii and core should reject uploads that exceed those declared limits before
state amplification occurs. The limits should be configuration-driven where
appropriate, but validation outcomes must remain deterministic across peers.

## Replay and Compiler Determinism

The private compiler/runtime path has a higher determinism burden than a normal
service deployment.

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

Before scaling the admitted family set, land a tiny deterministic fixture for
each family class and lock compile outputs plus runtime receipts with golden
tests.

## Remaining Design Gaps Before Code

The largest unresolved implementation questions are now narrowed to concrete
backend decisions:

- exact upload/chunk/request DTO shapes and Norito schemas;
- world-state indexing keys for bundle/chunk/session/checkpoint lookups;
- quota/default configuration placement in `iroha_config`;
- whether model/artifact status should become version-oriented rather than
  training-job-oriented when `UserUpload` is present;
- the precise revocation behavior when an apartment loses
  `allow_model_inference` or a pinned model version is rolled back.

Those are the next design-to-code bridge items. The architectural placement of
the feature should now be stable.

## Test Matrix

- upload validation:
  - accept canonical HF safetensors repos
  - reject GGUF, ONNX, missing tokenizer/processor assets, unsupported
    architectures, and audio/video multimodal packages
- chunking:
  - deterministic bundle roots
  - stable chunk ordering
  - exact reconstruction
  - envelope ceiling enforcement
- registry consistency:
  - bundle/chunk/artifact/weight promotion correctness under replay
- compiler:
  - one small fixture each for decoder-only, LLaVA-style, and Qwen2-VL-style
  - rejection for unsupported ops and shapes
- private runtime:
  - encrypted tiny-fixture end-to-end smoke test with stable receipts and
    threshold output release
- pricing:
  - XOR charges for upload, compile, runtime steps, and decryption
- So Ra integration:
  - upload, compile, publish, bind to team, run closed arena, inspect receipts,
    save project, reopen, rerun deterministically
- safety:
  - no export-gate bypass
  - no narrative auto-propagation
  - apartment binding fails without `allow_model_inference`

## Implementation Slices

1. Add the missing data-model fields and new record types.
2. Add the new Torii request/response types and route handlers.
3. Add matching Soracloud ISIs and world-state storage.
4. Add deterministic bundle/chunk validation and on-chain encrypted-byte
   storage.
5. Add a tiny BFV-backed private transformer fixture/runtime path.
6. Extend CLI model commands to cover upload/compile/private-run flows.
7. Land So Ra integration once the backend path is authoritative.
