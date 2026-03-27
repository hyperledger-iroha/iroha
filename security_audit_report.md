# Security Audit Report

Date: 2026-03-26

## Executive Summary

This audit focused on the highest-risk surfaces in the current tree: Torii HTTP/API/auth flows, P2P transport, secret-handling APIs, SDK transport guards, and the attachment sanitizer path.

I found 6 actionable issues:

- 2 High severity findings
- 4 Medium severity findings

The most important problems are:

1. Torii currently logs inbound request headers for every HTTP request, which can expose bearer tokens, API tokens, operator session/bootstrap tokens, and forwarded mTLS markers to logs.
2. Multiple public Torii routes and SDKs still support sending raw `private_key` values to the server so Torii can sign on the caller's behalf.
3. Several "secret" paths are treated as ordinary request bodies, including confidential seed derivation and canonical request auth in some SDKs.

## Method

- Static review of Torii, P2P, crypto/VM, and SDK secret-handling paths
- Targeted validation commands:
  - `cargo check -p iroha_torii --lib --message-format short` -> pass
  - `cargo check -p iroha_p2p --message-format short` -> pass
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> pass
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> pass, duplicate-version warnings only
- Not completed in this pass:
  - full workspace build/test/clippy
  - Swift/Gradle test suites
  - CUDA/Metal runtime validation

## Findings

### SA-001 High: Torii logs sensitive request headers globally

Impact: Any deployment that ships request tracing can leak bearer/API/operator tokens and related auth material into application logs.

Evidence:

- `crates/iroha_torii/src/lib.rs:20752` enables `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` enables `DefaultMakeSpan::default().include_headers(true)`
- Sensitive header names are actively used elsewhere in the same service:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Why this matters:

- `include_headers(true)` records full inbound header values into tracing spans.
- Torii accepts authentication material in headers such as `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token`, and `x-forwarded-client-cert`.
- A log sink compromise, debug log collection, or support bundle can therefore become a credential disclosure event.

Recommended remediation:

- Stop including full request headers in production spans.
- Add explicit redaction for security-sensitive headers if header logging is still needed for debugging.
- Treat request/response logging as secret-bearing by default unless the data is positively allow-listed.

### SA-002 High: Public Torii APIs still accept raw private keys for server-side signing

Impact: Clients are encouraged to transmit raw private keys over the network so the server can sign on their behalf, creating an unnecessary secret-exposure channel at the API, SDK, proxy, and server-memory layers.

Evidence:

- Governance route documentation explicitly advertises server-side signing:
  - `crates/iroha_torii/src/gov.rs:495`
- The route implementation parses the supplied private key and signs server-side:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDKs actively serialize `private_key` into JSON bodies:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Notes:

- This pattern is not isolated to one route family. The current tree contains the same convenience model across governance, offline cash, subscriptions, and other app-facing DTOs.
- HTTPS-only transport checks reduce accidental plaintext transport, but they do not solve server-side secret handling or logging/memory exposure risk.

Recommended remediation:

- Deprecate all request DTOs that carry raw `private_key` data.
- Require clients to sign locally and submit signatures or fully signed transactions/envelopes.
- Remove `private_key` examples from OpenAPI/SDKs after a compatibility window.

### SA-003 Medium: Confidential key derivation sends secret seed material to Torii and echoes it back

Impact: The confidential key-derivation API turns seed material into normal request/response payload data, increasing the chance of seed disclosure through proxies, middleware, logs, traces, crash reports, or client misuse.

Evidence:

- The request accepts seed material directly:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- The response schema echoes the seed back in both hex and base64:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- The handler explicitly re-encodes and returns the seed:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- The Swift SDK exposes this as a regular network method and persists the echoed seed in the response model:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Recommended remediation:

- Prefer local key derivation in CLI/SDK code and remove the remote derivation route entirely.
- If the route must remain, never return the seed in the response and mark seed-bearing bodies as sensitive in all transport guards and telemetry/logging paths.

### SA-004 Medium: SDK transport sensitivity detection has blind spots for non-`private_key` secret material

Impact: Some SDKs will enforce HTTPS for raw `private_key` requests, but still allow other security-sensitive request material to travel over insecure HTTP or to mismatched hosts.

Evidence:

- Swift treats canonical request auth headers as sensitive:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- But Swift still only body-matches on `"private_key"`:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin only recognizes `authorization` and `x-api-token` headers, then falls back to the same `"private_key"` body heuristic:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android has the same limitation:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java canonical request signers generate additional auth headers that are not classified as sensitive by their own transport guards:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Recommended remediation:

- Replace heuristic body scanning with explicit request classification.
- Treat canonical auth headers, seed/passphrase fields, signed mutation headers, and any future secret-bearing fields as sensitive by contract, not by substring match.
- Keep the sensitivity rules aligned across Swift, Kotlin, and Java.

### SA-005 Medium: Attachment "sandbox" is only a subprocess plus `setrlimit`

Impact: The attachment sanitizer is described and reported as "sandboxed", but the implementation is only a fork/exec of the current binary with resource limits. A parser or archive exploit would still execute with the same user, filesystem view, and ambient network/process privileges as Torii.

Evidence:

- The outer path marks the result as sandboxed after spawning a child:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- The child defaults to the current executable:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- The subprocess explicitly switches back into `AttachmentSanitizerMode::InProcess`:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- The only hardening applied is CPU/address-space `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Recommended remediation:

- Either implement a real OS sandbox (for example namespaces/seccomp/landlock/jail-style isolation, privilege drop, no-network, constrained filesystem) or stop labeling the result as `sandboxed`.
- Treat the current design as "subprocess isolation" rather than "sandboxing" in APIs, telemetry, and docs until true isolation exists.

### SA-006 Medium: Optional P2P TLS/QUIC transports disable certificate verification

Impact: When `quic` or `p2p_tls` is enabled, the channel provides encryption but does not authenticate the remote endpoint. An active on-path attacker can still relay or terminate the channel, defeating the normal security expectations operators associate with TLS/QUIC.

Evidence:

- QUIC explicitly documents permissive certificate verification:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- The QUIC verifier unconditionally accepts the server certificate:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- The TLS-over-TCP transport does the same:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Recommended remediation:

- Either verify peer certificates or add explicit channel binding between the higher-layer signed handshake and the transport session.
- If the current behavior is intentional, rename/document the feature as unauthenticated encrypted transport so operators do not mistake it for full TLS peer authentication.

## Recommended Remediation Order

1. Fix SA-001 immediately by redacting or disabling header logging.
2. Design and ship a migration plan for SA-002 so raw private keys stop crossing the API boundary.
3. Remove or narrow the remote confidential key derivation route and classify seed-bearing bodies as sensitive.
4. Align SDK transport sensitivity rules across Swift/Kotlin/Java.
5. Decide whether attachment sanitation needs a real sandbox or an honest rename/re-scoping.
6. Clarify and harden the P2P TLS/QUIC threat model before operators enable those transports expecting authenticated TLS.

## Validation Notes

- `cargo check -p iroha_torii --lib --message-format short` passed.
- `cargo check -p iroha_p2p --message-format short` passed.
- `cargo deny check advisories bans sources --hide-inclusion-graph` passed after running outside the sandbox; it emitted duplicate-version warnings but reported `advisories ok, bans ok, sources ok`.
- A focused Torii test for the confidential derive-keyset route was started during this audit but did not complete before the report was written; the finding is supported by direct source inspection regardless.
