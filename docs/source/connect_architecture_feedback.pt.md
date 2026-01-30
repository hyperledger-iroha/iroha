---
lang: pt
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2026-01-03T18:07:58.674563+00:00"
translation_last_reviewed: 2026-01-30
---

#! Connect Architecture Feedback Checklist

This checklist captures the open questions from the Connect Session Architecture
strawman that require input from the Android and JavaScript leads before the
Feb 2026 cross-SDK workshop. Use it to collect comments asynchronously, track
ownership, and unblock the workshop agenda.

> Status / Notes column captured final responses from Android and JS leads as of
> the Feb 2026 pre-workshop sync; link new follow-up issues inline if decisions
> evolve.

## Session Lifecycle & Transport

| Topic | Android owner | JS owner | Status / Notes |
|-------|---------------|----------|----------------|
| WebSocket reconnect back-off strategy (exponential vs. capped linear) | Android Networking TL | JS Lead | ✅ Agreed on exponential back-off with jitter, capped at 60 s; JS mirrors same constants for browser/node parity. |
| Offline buffer capacity defaults (current strawman: 32 frames) | Android Networking TL | JS Lead | ✅ Confirmed 32-frame default with config override; Android persists via `ConnectQueueConfig`, JS respects `window.connectQueueMax`. |
| Push-style reconnect notifications (FCM/APNS vs. polling) | Android Networking TL | JS Lead | ✅ Android will expose optional FCM hook for wallet apps; JS remains polling-based with exponential back-off, noting browser push constraints. |
| Ping/pong cadence guardrails for mobile clients | Android Networking TL | JS Lead | ✅ Standardised 30 s ping with 3× miss tolerance; Android balances Doze impact, JS clamps to ≥15 s to avoid browser throttling. |

## Encryption & Key Management

| Topic | Android owner | JS owner | Status / Notes |
|-------|---------------|----------|----------------|
| X25519 key storage expectations (StrongBox, WebCrypto secure contexts) | Android Crypto TL | JS Lead | ✅ Android stores X25519 in StrongBox when available (falls back to TEE); JS mandates secure-context WebCrypto for dApps, falling back to the native `iroha_js_host` bridge in Node. |
| ChaCha20-Poly1305 nonce management sharing across SDKs | Android Crypto TL | JS Lead | ✅ Adopt shared `sequence` counter API with 64-bit wrap guard and shared tests; JS uses BigInt counters to match Rust behaviour. |
| Hardware-backed attestation payload schema | Android Crypto TL | JS Lead | ✅ Schema finalised: `attestation { platform, evidence_b64, statement_hash }`; JS optional (browser), Node uses HSM plug-in hook. |
| Recovery flow for lost wallets (key rotation handshake) | Android Crypto TL | JS Lead | ✅ Wallet rotation handshake accepted: dApp issues `rotate` control, wallet replies with new pubkey + signed acknowledgment; JS re-keys WebCrypto material immediately. |

## Permissions & Proof Bundles

| Topic | Android owner | JS owner | Status / Notes |
|-------|---------------|----------|----------------|
| Minimum permission schema (methods/events/resources) for GA | Android Data Model TL | JS Lead | ✅ GA baseline: `methods`, `events`, `resources`, `constraints`; JS aligns TypeScript types with Rust manifest. |
| Wallet rejection payload (`reason_code`, localized messages) | Android Networking TL | JS Lead | ✅ Codes finalised (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) plus optional `localized_message`. |
| Proof bundle optional fields (compliance/KYC attachments) | Android Data Model TL | JS Lead | ✅ All SDKs accept optional `attachments[]` (Norito `AttachmentRef`) and `compliance_manifest_id`; no behaviour change required. |
| Alignment on Norito JSON schema vs. bridge-generated structs | Android Data Model TL | JS Lead | ✅ Decision: prefer bridge-generated structs; JSON path remains for debugging only, JS keeps `Value` adapter. |

## SDK Facades & API Shape

| Topic | Android owner | JS owner | Status / Notes |
|-------|---------------|----------|----------------|
| High-level async interfaces (`Flow`, async iterators) parity | Android Networking TL | JS Lead | ✅ Android exposes `Flow<ConnectEvent>`; JS uses `AsyncIterable<ConnectEvent>`; both map to shared `ConnectEventKind`. |
| Error taxonomy mapping (`ConnectError`, typed subclasses) | Android Networking TL | JS Lead | ✅ Adopt shared enum {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} with platform-specific payload details. |
| Cancellation semantics for in-flight sign requests | Android Networking TL | JS Lead | ✅ Introduced `cancelRequest(hash)` control; both SDKs surface cancellable coroutines/promises respecting wallet acknowledgment. |
| Shared telemetry hooks (events, metrics naming) | Android Networking TL | JS Lead | ✅ Metric names aligned: `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total`; sample exporters documented. |

## Offline Persistence & Journaling

| Topic | Android owner | JS owner | Status / Notes |
|-------|---------------|----------|----------------|
| Storage format for queued frames (binary Norito vs. JSON) | Android Data Model TL | JS Lead | ✅ Store binary Norito (`.to`) everywhere; JS uses IndexedDB `ArrayBuffer`. |
| Journal retention policy & size caps | Android Networking TL | JS Lead | ✅ Default retention 24 h and 1 MiB per session; configurable via `ConnectQueueConfig`. |
| Conflict resolution when both sides replay frames | Android Networking TL | JS Lead | ✅ Use `sequence` + `payload_hash`; duplicates ignored, conflicts trigger `ConnectError.Internal` with telemetry event. |
| Telemetry for queue depth and replay success | Android Networking TL | JS Lead | ✅ Emit `connect.queue_depth` gauge and `connect.replay_success_total` counter; both SDKs hook into shared Norito telemetry schema. |

## Implementation Spikes & References

- **Rust bridge fixtures:** `crates/connect_norito_bridge/src/lib.rs` and associated tests cover the canonical encode/decode paths used by every SDK.
- **Swift demo harness:** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` exercises Connect session flows with mocked transports.
- **Swift CI gating:** run `make swift-ci` when updating Connect artifacts to validate fixture parity, dashboard feeds, and Buildkite `ci/xcframework-smoke:<lane>:device_tag` metadata before sharing with other SDKs.
- **JavaScript SDK integration tests:** `javascript/iroha_js/test/integrationTorii.test.js` validate Connect status/session helpers against Torii.
- **Android client resilience notes:** `java/iroha_android/README.md:150` documents the current connectivity experiments that inspired the queue/back-off defaults.

## Workshop Prep Items

- [x] Android: assign point person for each table row above.
- [x] JS: assign point person for each table row above.
- [x] Collect links to existing implementation spikes or experiments.
- [x] Schedule pre-work review before Feb 2026 council (Booked for 2026-01-29 15:00 UTC with Android TL, JS Lead, Swift Lead).
- [x] Update `docs/source/connect_architecture_strawman.md` with accepted answers.

## Pre-read Package

- ✅ Bundle recorded under `artifacts/connect/pre-read/20260129/` (generated via `make docs-html` after refreshing the strawman, SDK guides, and this checklist).
- 📄 Summary + distribution steps live in `docs/source/project_tracker/connect_architecture_pre_read.md`; include the link in the Feb 2026 workshop invite and in the `#sdk-council` reminder.
- 🔁 When refreshing the bundle, update the path and hash inside the pre-read note and archive the announcement in `status.md` under the IOS7/AND7 readiness logs.
