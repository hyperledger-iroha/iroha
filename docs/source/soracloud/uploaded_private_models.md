# Soracloud Uploaded Models

Production V1 supports uploaded-model registration as a storage and registry
workflow only. Model bytes are encrypted and pinned through SoraFS, then
Soracloud records the approved SoraFS manifest digest plus deterministic roots,
byte counts, artifact metadata, and weight-version metadata.

The chain never stores uploaded model bytes, encrypted chunk payloads, or
process-local upload sessions. Torii exposes one signed mutation for the
uploaded-model path:

- `POST /v1/soracloud/model/upload/register`

Torii also exposes read-only upload readiness endpoints:

- `GET /v1/soracloud/model/upload/encryption-recipient`
- `GET /v1/soracloud/model/upload/status`

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

Production V1 does not expose private uploaded-model execution. Uploaded-model
status is limited to storage and registry readiness. Any future private runtime
must land as a real deterministic runtime with commitments, receipts,
decryption-policy release, and cross-peer determinism tests before public
execution APIs are added.

## Production Gates

Soracloud production deployments must enable `soracloud_runtime.production_mode`
and build `irohad` with `embedded-soracloud-runtime`. Production mode rejects
configs that leave Inrou disabled, use proxy-only Inrou host posture, omit the
runtime submission gas asset, leave broad runtime egress open, omit fail-closed
egress budgets, or enable Hugging Face inference-bridge fallback.

Production behavior is sourced from configuration, not environment variables.
Zero-backend or disabled Inrou hosts must not advertise runtime host placement.

## Test Expectations

Focused V1 coverage should include:

- approved active SoraFS pin succeeds;
- missing, pending, and retired SoraFS pins fail;
- world state contains no uploaded-model bytes;
- removed private execution endpoints return not found;
- production config rejects missing Inrou enablement or gas asset;
- zero-backend Inrou hosts emit no host adverts;
- JavaScript Soracloud helpers expose unsigned drafts and do not accept raw
  private keys.
