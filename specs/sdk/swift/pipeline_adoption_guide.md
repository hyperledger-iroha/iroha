---
title: Swift `/v1/pipeline` Adoption Guide
summary: Checklist and runbook for enabling the Torii pipeline endpoints inside Swift clients as required by IOS2-WB2.
---

# 1. Scope & Status

- **Prerequisites:** `IrohaSwift` 0.9+ (supports `PipelineEndpointMode`, offline queues, and telemetry hooks); Torii nodes exposing `/v1/pipeline/transactions`, `/v1/pipeline/transactions/status`, and `/v1/pipeline/recovery/{height}`; Norito encoders wired through `SwiftTransactionEncoder`.
- **References:** `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, `IrohaSwift/Sources/IrohaSwift/TxBuilder.swift`, `specs/sdk/swift/index.md` (landing page).
- **Server validation:** Torii owners record `/v1/pipeline` staging evidence with the runbook in [`specs/torii/pipeline_staging_validation.md`](../../torii/pipeline_staging_validation.md) so SDK proofs reference the same artefacts.

# 2. Endpoint Selection & Rollback Guardrails

`IrohaSDK.pipelineEndpointMode` defaults to `.pipeline` and controls the target endpoints for submits and status polls:

| Mode | Submit path | Status path | When to use |
|------|-------------|-------------|-------------|
| `.pipeline` (default) | `/v1/pipeline/transactions` | `/v1/pipeline/transactions/status` | All production/staging Torii clusters. Required for IOS2 readiness. |


✅ Acceptance criteria: every shipping build must leave the mode at `.pipeline`, log any temporary downgrades, and restore the default immediately after Torii recovers.

# 3. One-shot Submission Semantics

`PipelineSubmitOptions` governs only the idempotency key on the single submission attempt:

```swift
let submitOptions = PipelineSubmitOptions(
    idempotencyKeyFactory: { envelope in
        // The default already uses the transaction hash hex.
        "swift-demo-\(envelope.hashHex)"
    }
)

let sdk = IrohaSDK(
    baseURL: torii.baseURL,
    pipelineSubmitOptions: submitOptions
)
```

- `TxBuilder.submit`, `IrohaSDK.submit`, and `IrohaSDK.submitAndWait` send the signed Norito envelope exactly once. Redirects, transport failures, 429 responses, and 5xx responses are returned to the caller without replay.
- `PipelineSubmitOptions.defaultIdempotencyKeyFactory` emits the envelope hash, but this server-side deduplication hint does not authorize a client retry.
- After an ambiguous transport result, call `getTransactionStatus(hashHex:)`. Do not resubmit until the application has reconciled the original envelope hash.
- Custom `ToriiTransactionSubmitting` implementations must reject redirects and provide the same one-shot contract.

# 4. Polling, Classification, and Errors

`submitAndWait` and `pollPipelineStatus` use `PipelineStatusPollOptions` to classify terminal states. Defaults treat `Approved`, `Committed`, and `Applied` as success and `Rejected`/`Expired` as failure. Customize the window when tighter SLAs or additional statuses are required:

```swift
var pollOptions = PipelineStatusPollOptions(
    pollInterval: 0.5,
    timeout: 45,
    maxAttempts: 120,
    successStates: [.approved, .committed, .applied],
    failureStates: [.rejected, .expired, PipelineTransactionState(kind: "FAILED_VALIDATION")]
)

let status = try await sdk.submitAndWait(
    transfer: transfer,
    keypair: keypair,
    pollOptions: pollOptions
)
```

- When a transaction never reaches a terminal state within the configured attempts, the SDK throws `PipelineStatusError.timeout(hash:attempts:)` so callers can surface the stalled hash and capture `/v1/pipeline/recovery` evidence.
- Failures (e.g., `Rejected`) yield `PipelineStatusError.failure` with the final `ToriiPipelineTransactionStatus` payload for logging and telemetry.
- Use `ToriiClient.getTransactionStatus(hashHex:mode:)` when monitoring a hash submitted by other SDKs or CLI automation.
- A `404` from `/v1/pipeline/transactions/status` indicates Torii has no cached status yet (for example after a restart), so the Swift SDK treats it as "pending" and continues polling.

# 5. Caller-managed Archives & Recovery Evidence

Swift clients may archive an envelope explicitly before submission:

```swift
let archiveURL = FileManager.default
    .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("pending.pipeline.queue")

let archive = try FilePendingTransactionQueue(fileURL: archiveURL)
try archive.enqueue(envelope)
```

- `IrohaSDK` never drains or submits this archive. The application owns reconciliation,
  removal, and any explicit later action.
- Before paging Torii, configure the client once with an immutable
  `ToriiOperatorSigningContext` built from the exact genesis `NetworkId` and
  operator signing key. Capture `/v1/pipeline/recovery/{height}` via
  `ToriiClient.getPipelineRecovery(height:)` and export the JSON artifact with
  the incident report. The helper signs the exact `GET`, substituted path,
  query, and empty body, then dispatches once without redirects or retries.
- Pair the recovery evidence with operator-authenticated
  `ToriiClient.getTimeStatus()`/`getSumeragiStatus()` samples and the
  transaction-hash status lookup. Bearer/API tokens are not an operator-read
  fallback.

# 6. Observability & Reporting

To satisfy IOS2 reporting gates:

1. Log `pipelineEndpointMode` and `pipelinePollOptions` at startup. Include the configuration in weekly digests and attach them to the `swift_parity_*` telemetry bundle exported by `scripts/swift_status_export.py`.
2. When downgrades occur, annotate the Buildkite `ci/xcode-swift-parity` run and update `status.md` with the affected build numbers and hashes.
3. Track ambiguous one-shot outcomes and their hash-reconciliation results (see `specs/sdk/swift/telemetry_redaction.md`) without logging signed bodies.
4. Store `submitAndWait` traces together with the Torii `/v1/pipeline/transactions/status` responses—auditors must be able to associate every operator-facing alert with the hash, final status, and recovery evidence mentioned above.

Once the above artefacts are captured, update the roadmap entry for IOS2-WB2 and the Swift section of `status.md` so reviewers can trace the adoption across docs, code, and telemetry.
