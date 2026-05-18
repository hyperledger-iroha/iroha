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

- `GET /v1/soracloud/model/upload/encryption-recipient`
- `GET /v1/soracloud/model/upload/status`
- `POST /v1/soracloud/model/upload/private/execute`
- `GET /v1/soracloud/model/upload/private/receipts`

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
  client signing and transaction submission.

Plaintext input and output are runtime-local and must not be written to chain
state. Committed chain state stores receipt commitments and encrypted artifact
references only.

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
receipt instruction skeleton for external transaction signing.

The Kotlin core SDK and Java Android SDK mirror the client-visible response
parsers for private execute and committed receipt-list responses. Both expose a
helper that extracts the
`RecordSoracloudPrivateUploadedModelExecutionReceipt` instruction skeleton from
the Torii response so mobile clients can pass it to their normal external
transaction signing pipeline.

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
- deterministic quantized CPU private execution emits stable receipts and a
  receipt-recording transaction instruction;
- receipt recording rejects non-deterministic uploaded-model formats and
  mismatched manifest, bundle-root, or policy bindings;
- production config rejects missing Inrou enablement or gas asset;
- zero-backend Inrou hosts emit no host adverts;
- JavaScript Soracloud helpers expose unsigned drafts and do not accept raw
  private keys.
