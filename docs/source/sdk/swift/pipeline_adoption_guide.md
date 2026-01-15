---
title: Swift `/v1/pipeline` Adoption Guide
summary: Checklist and runbook for enabling the Torii pipeline endpoints inside Swift clients as required by IOS2-WB2.
---

# 1. Scope & Status

- **Prerequisites:** `IrohaSwift` 0.9+ (supports `PipelineEndpointMode`, offline queues, and telemetry hooks); Torii nodes exposing `/v1/pipeline/transactions`, `/v1/pipeline/transactions/status`, and `/v1/pipeline/recovery/{height}`; Norito encoders wired through `SwiftTransactionEncoder`.
- **References:** `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, `IrohaSwift/Sources/IrohaSwift/TxBuilder.swift`, `docs/source/sdk/swift/index.md` (landing page).
- **Server validation:** Torii owners record `/v1/pipeline` staging evidence with the runbook in [`docs/source/torii/pipeline_staging_validation.md`](../../torii/pipeline_staging_validation.md) so SDK proofs reference the same artefacts.

# 2. Endpoint Selection & Rollback Guardrails

`IrohaSDK.pipelineEndpointMode` defaults to `.pipeline` and controls the target endpoints for submits and status polls:

| Mode | Submit path | Status path | When to use |
|------|-------------|-------------|-------------|
| `.pipeline` (default) | `/v1/pipeline/transactions` | `/v1/pipeline/transactions/status` | All production/staging Torii clusters. Required for IOS2 readiness. |


✅ Acceptance criteria: every shipping build must leave the mode at `.pipeline`, log any temporary downgrades, and restore the default immediately after Torii recovers.

# 3. Submission & Retry Semantics

`PipelineSubmitOptions` governs retries and idempotency:

```swift
var submitOptions = PipelineSubmitOptions(
    maxRetries: 5,
    initialBackoffSeconds: 0.75,
    backoffMultiplier: 1.7,
    retryableStatusCodes: [429, 500, 502, 503, 504],
    idempotencyKeyFactory: { envelope in
        // Default uses the transaction hash hex; override if a custom key is required.
        "swift-demo-\(envelope.hashHex)"
    }
)

let sdk = IrohaSDK(
    baseURL: torii.baseURL,
    pipelineSubmitOptions: submitOptions
)
```

- Retries apply to transport failures and HTTP status codes listed in `retryableStatusCodes`. Each attempt sends the Norito payload with the optional `Idempotency-Key` header so Torii deduplicates replays (`PipelineSubmitOptions.defaultIdempotencyKeyFactory` already emits the envelope hash).
- `PipelineSubmitOptions` are shared by the transaction builders (`TxBuilder.submit`, `IrohaSDK.submit`, and `IrohaSDK.submitAndWait`). Per-call overrides are available through `submit(envelope:pipelineSubmitOptions:...)`.
- Exhausted retries surface `IrohaSDKError.pipelineSubmissionFailed` and, when `pendingTransactionQueue` is configured (see §5), the SDK persists the envelope for later replay.

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

# 5. Offline Queueing & Recovery Evidence

Swift clients should persist envelopes that exhaust their retry budget by configuring `PendingTransactionQueue`:

```swift
let queueURL = FileManager.default
    .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    .appendingPathComponent("pending.pipeline.queue")

sdk.pendingTransactionQueue = try FilePendingTransactionQueue(fileURL: queueURL)
```

- Envelopes replayed from the queue reuse the same `PipelineSubmitOptions`, idempotency key, and polling classification, guaranteeing that retries match the live submission semantics.
- Before paging Torii, capture the context from `/v1/pipeline/recovery/{height}` via `ToriiClient.getPipelineRecovery(height:)` and export the JSON artifact with the incident report. IOS2 requires these snapshots so governance can prove that Swift clients observe the new recovery diagnostics.
- Pair the recovery evidence with `ToriiClient.getTimeStatus()`/`getSumeragiStatus()` samples to show that queue drains align with the cluster state.

# 6. Observability & Reporting

To satisfy IOS2 reporting gates:

1. Log `pipelineEndpointMode`, `pipelineSubmitOptions`, and `pipelinePollOptions` at startup. Include the configuration in weekly digests and attach them to the `swift_parity_*` telemetry bundle exported by `scripts/swift_status_export.py`.
2. When downgrades occur, annotate the Buildkite `ci/xcode-swift-parity` run and update `status.md` with the affected build numbers and hashes.
3. Track `swift.torii.http.retry`/`swift.pipeline.queue.persisted`/`swift.pipeline.queue.replayed` events (see `docs/source/sdk/swift/telemetry_redaction.md`) to prove retry parity across shadow traffic.
4. Store `submitAndWait` traces together with the Torii `/v1/pipeline/transactions/status` responses—auditors must be able to associate every operator-facing alert with the hash, final status, and recovery evidence mentioned above.

Once the above artefacts are captured, update the roadmap entry for IOS2-WB2 and the Swift section of `status.md` so reviewers can trace the adoption across docs, code, and telemetry.
