---
lang: ja
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-01-30
---

# Connect Error Taxonomy (Swift baseline)

This note tracks IOS-CONNECT-010 and documents the shared error taxonomy for
Nexus Connect SDKs. Swift now exports the canonical `ConnectError` wrapper,
which maps all Connect-related failures to one of six categories so telemetry,
dashboards, and UX copy stay aligned across platforms.

> Last updated: 2026-01-15  
> Owner: Swift SDK Lead (taxonomy steward)  
> Status: Swift + Android + JavaScript implementations **landed**; queue integration pending.

## Categories

| Category | Purpose | Typical sources |
|----------|---------|-----------------|
| `transport` | WebSocket/HTTP transport failures that terminate a session. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Serialization/bridge failures while encoding/decoding frames. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | TLS/attestation/policy failures requiring user or operator remediation. | `URLError(.secureConnectionFailed)`, Torii 4xx responses |
| `timeout` | Idle/offline expirations and watchdogs (queue TTL, request timeout). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | FIFO back-pressure signals so apps can shed load gracefully. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Everything else: SDK misuse, missing Norito bridge, corrupted journals. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Every SDK publishes an error type that conforms to the taxonomy and exposes
structured telemetry attributes: `category`, `code`, `fatal`, and optional
metadata (`http_status`, `underlying`).

## Swift mapping

Swift exports `ConnectError`, `ConnectErrorCategory`, and helper protocols in
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. All public Connect error
types conform to `ConnectErrorConvertible`, so apps can call `error.asConnectError()`
and forward the result to telemetry/logging layers.

| Swift error | Category | Code | Notes |
|-------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Indicates double `start()`; developer mistake. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Raised when sending/receiving after a close. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket delivered textual payload while expecting binary. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Counterparty closed the stream unexpectedly. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | Application forgot to configure symmetric keys. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito payload missing required fields. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Future payload seen by old SDK. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito bridge missing or failed to encode/decode frame bytes. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Bridge unavailable or mismatched key lengths. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | Offline queue length exceeded the configured bound. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | Surfaced by `URLSessionWebSocketTask`. |
| `URLError` TLS cases | `authorization` | `network.tls_failure` | ATS/TLS negotiation failures. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | JSON decoding/encoding failed elsewhere in the SDK; message uses the Swift decoder context. |
| Any other `Error` | `internal` | `unknown_error` | Guaranteed catch-all; message mirrors `LocalizedError`. |

Unit tests (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) lock down
the mapping so future refactors cannot silently change categories or codes.

### Example usage

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## Telemetry & dashboards

The Swift SDK provides `ConnectError.telemetryAttributes(fatal:httpStatus:)`
which returns the canonical attribute map. Exporters should forward these
attributes into `connect.error` OTEL events with optional extras:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Dashboards correlate `connect.error` counters with queue depth (`connect.queue_depth`)
and reconnect histograms to detect regressions without spelunking logs.

## Android mapping

The Android SDK exports `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions`, and the helper utilities under
`org.hyperledger.iroha.android.connect.error`. Builder-style helpers convert any `Throwable`
into a taxonomy-compliant payload, infer categories from transport/TLS/codec exceptions,
and expose deterministic telemetry attributes so OpenTelemetry/sampling stacks can consume the
result without bespoke adapters.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` already implements `ConnectErrorConvertible`, emitting the queueOverflow/timeout
categories for overflow/expiry conditions so offline queue instrumentation can wire into the same flow.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
The Android SDK README now references the taxonomy and shows how to wrap transport exceptions
before emitting telemetry, keeping dApp guidance aligned with the Swift baseline.【java/iroha_android/README.md:167】

## JavaScript mapping

Node.js/browser clients import `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory`, and
`connectErrorFrom()` from `@iroha/iroha-js`. The shared helper inspects HTTP status codes,
Node error codes (socket, TLS, timeout), `DOMException` names, and codec failures to emit the
same categories/codes documented in this note, while TypeScript definitions model the telemetry
attribute overrides so tooling can emit OTEL events without manual casting.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
The SDK README documents the workflow and links back to this taxonomy so application teams can
copy the instrumentation snippets verbatim.【javascript/iroha_js/README.md:1387】

## Next steps (cross-SDK)

- **Queue integration:** once the offline queue ships, ensure dequeue/drop logic
  surfaces `ConnectQueueError` values so overflow Telemetry remains trustworthy.
