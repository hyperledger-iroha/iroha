---
id: nexus-app-facade
title: Nexus App Facade
description: High-level SDK facade for SORA Nexus wallet-approved transfer flows.
---

The Nexus App Facade lets app developers run the common wallet-mediated transfer
flow without directly using Torii submission, Connect frames, transaction
encoding, or pipeline polling primitives.

V1 supports app-role Connect and Ed25519 numeric asset transfers:

1. `startConnect(options)` returns a wallet launch URI and session metadata.
2. `awaitApproval(session)` validates wallet approval and returns the approved
   account plus signing public key.
3. `buildTransferDraft(input)` creates canonical transaction payload bytes and
   `payloadHashHex`.
4. `requestSignature(session, signable)` asks the wallet to sign the payload.
5. `finalizeAndSubmit(signable, signature, options)` submits the signed
   transaction to Torii and waits for final pipeline status.
6. `transferWithWallet(session, input)` wraps draft, sign, finalize, submit, and
   wait.

Shared concepts are `NexusAppConfig`, `NexusConnectOptions`,
`NexusConnectSession`, `NexusTransferDraft`, `NexusSignableTransaction`,
`NexusTransferReceipt`, and `NexusAppError`.

SDK entry points:

- Rust: `iroha::nexus_app::NexusAppClient`
- JavaScript/TypeScript: `@iroha/iroha-js/nexus-app`
- Python: `iroha_python.nexus_app`
- Swift: `IrohaSwift.NexusAppClient`
- Kotlin/JVM: `org.hyperledger.iroha.sdk.nexus.NexusAppClient`
- Java Android: `org.hyperledger.iroha.android.nexus.NexusAppClient`

JS browser apps can use the built-in Connect browser path when a base Torii URL
is configured. Python uses the native transaction codec by default and can use
the default Connect WebSocket path with `NexusAppConfig(base_url=...)` plus the
`iroha-python[ws]` extra. Other SDKs keep wallet Connect transport injection
explicit in this first pass.

The shared parity fixture is
`fixtures/sdk/nexus_connect_transfer_v1.json`.
