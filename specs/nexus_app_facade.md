# Nexus App Facade

The Nexus App Facade is an additive SDK layer for app developers who want the
standard wallet-mediated transfer flow without using low-level Torii, Connect,
transaction encoding, or pipeline polling APIs directly.

V1 covers app-role Connect plus numeric asset transfers:

1. Create a Connect app session and wallet launch URI.
2. Wait for wallet approval and capture the approved account/signing key.
3. Build canonical signable transfer payload bytes and a payload hash.
4. Request a wallet signature.
5. Finalize the signed transaction, submit it to Torii, and wait for a terminal
   pipeline status.

The shared concepts are `NexusAppConfig`, `NexusConnectOptions`,
`NexusConnectSession`, `NexusTransferDraft`, `NexusSignableTransaction`,
`NexusTransferReceipt`, and `NexusAppError`. SDKs may expose a small
SDK-native approval helper in addition to these concepts.

## API surface

Every SDK exposes the same facade methods with native naming conventions:

- `startConnect(options)` registers an app Connect session and returns wallet
  launch metadata.
- `awaitApproval(session)` opens or uses the app-role Connect channel and
  returns the approved account.
- `buildTransferDraft(input)` creates canonical transaction payload bytes and
  `payloadHashHex`.
- `requestSignature(session, signable)` sends a Connect transaction-signature
  request to the wallet.
- `finalizeAndSubmit(signable, signature, options)` builds the signed
  transaction, submits it to Torii, and optionally waits for final status.
- `transferWithWallet(session, input)` runs draft, signature request,
  finalization, submission, and status wait as one call.

V1 accepts Ed25519 signatures only. SDKs must fail closed with
`unsupported_signature_algorithm` for other algorithms unless that SDK already
has tested parity. SDKs also fail with `missing_signing_public_key` when the
approved account cannot provide or derive the signing public key and no
explicit override was supplied.

Default transports/codecs are intentionally additive. JS browser apps can use
the `connect.browser` transport through `@iroha/iroha-js/nexus-app`. Python
uses the native transaction builder by default and can open a Connect
WebSocket when constructed with `NexusAppConfig(base_url=...)`; install the
`ws` extra for that path. Rust, Swift, Kotlin/JVM, and Java Android keep
explicit transport injection for wallet/session orchestration while using their
SDK-native transaction codecs.

## Parity fixture

The shared fixture is `fixtures/sdk/nexus_connect_transfer_v1.json`. It contains
deterministic Connect session data, wallet approval, transfer input, expected
payload bytes/hash, a wallet signature, a signed transaction hash, final status
sequence, and typed error cases.

Use this fixture for cross-SDK tests before adding live smoke tests. Live tests
should stay behind opt-in environment variables such as `NEXUS_CONNECT_LIVE=1`,
`TORII_URL`, and wallet credentials.

## SDK entry points

- Rust: `iroha::nexus_app::NexusAppClient`
- JavaScript/TypeScript: `@iroha/iroha-js/nexus-app`
- Python: `iroha_python.nexus_app`
- Swift: `IrohaSwift.NexusAppClient`
- Kotlin/JVM: `org.hyperledger.iroha.sdk.nexus.NexusAppClient`
- Java Android: `org.hyperledger.iroha.android.nexus.NexusAppClient`

## Minimal flow

```text
session = client.startConnect(options)
approval = client.awaitApproval(session)
draft = client.buildTransferDraft(input.with(authority = approval.account))
signature = client.requestSignature(approval.session, draft.signable)
receipt = client.finalizeAndSubmit(draft.signable, signature)
```

For app code that already has an approved session, use `transferWithWallet` to
run the last four steps as a single call.
