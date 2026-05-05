---
lang: hy
direction: ltr
source: docs/source/sdk/swift/ios4_crypto_api_rfc.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79cb4de62bb5286ddb4f84152f1a107fbf26014a93ce3ff50960c9e3f59272f4
source_last_modified: "2026-01-05T18:22:23.407342+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# IOS4 Swift Crypto API RFC

This RFC fulfils the IOS4 roadmap action “Draft crypto API proposal” by
describing how the Swift SDK will grow from its current Ed25519-only helpers
(`IrohaSwift/Sources/IrohaSwift/Crypto.swift`) into a multi-algorithm, multisig
surface with deterministic fallbacks. It aligns the Swift plan with the
`iroha_crypto` crates that already expose Ed25519, secp256k1, SM2, and ML-DSA,
and provides the API contracts that the hardware requirements review will vet.

> Update (2027-05): secp256k1 is wired end-to-end — NoritoBridge exports
> public-key/sign/verify FFI with deterministic vectors and Swift wraps it via
> `Secp256k1Keypair`/`SigningKey.secp256k1` plus dedicated tests, keeping the
> envelope path aligned with Ed/SM2/ML-DSA coverage and the existing multisig/
> transaction encoder flows.【crates/connect_norito_bridge/src/lib.rs:1】【crates/connect_norito_bridge/include/connect_norito_bridge.h:1】【IrohaSwift/Sources/IrohaSwift/Crypto.swift:1】【IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1】【IrohaSwift/Tests/IrohaSwiftTests/Secp256k1SigningTests.swift:1】【IrohaSwift/Tests/IrohaSwiftTests/MultisigPolicyBuilderTests.swift:1】【IrohaSwift/Tests/IrohaSwiftTests/SwiftTransactionEncoderSigningKeyTests.swift:1】

## 1. Goals and Non-Goals

**Goals**

- Support Ed25519, secp256k1, SM2, and ML-DSA signing in Swift with consistent
  APIs and Norito metadata.
- Provide multisig policy builders that match the encoding/validation logic
  already embedded in `AccountAddress.multisigPolicyInfo()`
  (`IrohaSwift/Sources/IrohaSwift/AccountAddress.swift:295`) so clients can
  assemble threshold policies without hand-rolled CBOR.
- Define storage abstractions that let apps plug in Secure Enclave/Keychain,
  deterministic seed derivation, or transient in-memory keys without coupling
  business logic to any one backend.
- Capture telemetry, testing, and rollout requirements needed for IOS4 and the
  follow-on IOS5/IOS6 milestones.

**Non-Goals**

- Implement new algorithms inside this RFC; concrete types will arrive in
  follow-up PRs.
- Redesign the Norito wire formats or change the ABI policy that `iroha_crypto`
  enforces (ABI v1 remains the only supported version).

## 2. Current State Summary

| Capability | Source | Notes |
|------------|--------|-------|
| Ed25519 signing/keygen | `Keypair` in `Crypto.swift` | Pure CryptoKit, no metadata about algorithm or storage tier. |
| SM2 deterministic keypairs | `Sm2Keypair` + `NoritoNativeBridge` (`Crypto.swift:41`) | Already honours `sm2_distid_default` but not yet wired into higher-level signing surfaces. |
| secp256k1 signing + envelopes | `Secp256k1Keypair` + `SigningKey.secp256k1` (`Crypto.swift`) | Deterministic signing via NoritoBridge with compressed public keys; covered by `Secp256k1SigningTests` to keep parity evidence. |
| Multisig metadata | `AccountAddress.multisigPolicyInfo()` | Can decode an address and render CTAP2/Blake2b digests, but there is no builder to emit new policies. |
| Transaction signing | `TxBuilder` + `TransactionEncoder` | Multi-alg signing is wired via `SigningKey` (Ed25519, secp256k1, SM2, ML‑DSA) with NoritoBridge; multisig/asset-specific signing controls still track the roadmap. |

## 3. Proposed API Surface

### 3.1 SigningAlgorithm

Add a `public enum SigningAlgorithm` with cases `.ed25519`, `.secp256k1`,
`.sm2`, `.mlDsa44`. Each case exposes:

- `signatureCodec`: maps to the Norito codec discriminant so `TransactionEncoder`
  can annotate the envelope.
- `defaultAccountIdPrefix`: e.g. `ed0120` for Ed25519, `secp256k1-0120` etc,
  matching `iroha_crypto::PublicKey`.
- `requiresDistId`: true for SM2 so higher layers insist on a `distid`.

### 3.2 SigningKey and KeyMaterial

Define:

```swift
public struct SigningKey: Sendable {
    public let algorithm: SigningAlgorithm
    public let material: SigningMaterial
    public let metadata: SigningMetadata
    public func sign(message: Data) throws -> Data
}
```

`SigningMaterial` is a sum type:

- `.inMemory(PrivateKeyBlob)` — wraps deterministic seeds or exported keys.
- `.secureEnclave(SecureEnclaveReference)` — stores Keychain tag + access group.
- `.bridge(NoritoNativeBridge.Handle)` — delegates to C FFI for SM2/ML-DSA once
  the bridge exposes those entrypoints.

`SigningMetadata` captures `distid`, account aliases, and optional device
attestation handles so telemetry and manifests can attest to the key origin.

The struct is responsible for:

- Enforcing deterministic signing by routing through `iroha_crypto` fixtures
  (via `NoritoNativeBridge`) for SM2/ML-DSA while keeping CryptoKit for Ed25519.
- Surfacing `verify(message:signature:)` for local smoke tests when possible.

### 3.3 Deterministic Signing Pipeline

- `SigningKey.sign` always returns canonical (r∥s) forms, relying on the bridge
  for SM2 and ML-DSA.
- A new `SignatureEnvelope` encodes `{algorithm, publicKey, signatureBytes,
  metadata}` so multisig builders can attach per-signer context.
- `TxBuilder.sign(envelope:)` accepts any algorithm and writes the matching
  Norito variant. For scalar builds we preserve the existing `sign(privateKey:)`
  convenience, implemented in terms of `SigningKey`.

### 3.4 Multisig Policy Builder

- `MultisigPolicyBuilder` exposes a fluent API:

```swift
let policy = MultisigPolicyBuilder()
    .version(.v1)
    .threshold(Weight(3))
    .addMember(.init(algorithm: .ed25519,
                     weight: Weight(1),
                     publicKey: keyA))
    .addMember(...)
    .build()
```

- `build()` emits the CBOR blob plus Blake2b digest identical to
  `encodeMultisigPolicyCTAP2` and surfaces them through a value type so
  `AccountAddress` and transaction builders can share code.
- Validation rules (weight overflow, zero threshold, duplicate members) mirror
  `AccountAddress` errors to keep UX consistent.

### 3.5 Multisig Transaction Builder Extensions

- `TxBuilder` gains `withMultisig(policy: MultisigPolicy, signatures: [SignatureEnvelope])`.
- `TxBuilder` continues to support single-signature flows; internally it builds
  a `MultisigPolicy` with one member when necessary so the encoding path is
  unified.
- Add a lightweight `MultisigSigningSession` helper that tracks pending
  signatures, threshold fulfilment, and per-member status for UI use.

### 3.6 `iroha_crypto` Discriminant Alignment

`SigningAlgorithm` must mirror the `Algorithm` enum encoded on-chain inside
`iroha_crypto` (`crates/iroha_crypto/src/algorithm.rs`). The table below captures
the mapping Swift will reuse when serialising keys/signatures:

| SigningAlgorithm case | `Algorithm` variant | `repr(u8)` ID | String ID | Parameter set / source |
|-----------------------|---------------------|---------------|-----------|------------------------|
| `.ed25519` | `Algorithm::Ed25519` | `0` (default) | `ed25519` | CryptoKit-backed today; matches the default Norito layout |
| `.secp256k1` | `Algorithm::Secp256k1` | `1` | `secp256k1` | Deterministic ECDSA signing via `signature::secp256k1` |
| `.mlDsa44` (aka `.mlDsa`) | `Algorithm::MlDsa` | `4` | `ml-dsa` | `PublicKeyFull::MlDsa` uses `pqcrypto_mldsa::mldsa65`, i.e. NIST ML‑DSA‑65 (`crates/iroha_crypto/src/lib.rs`, `ml-dsa` feature) |
| `.sm2` | `Algorithm::Sm2` | `10` | `sm2` | `Sm2PublicKey::from_sec1_bytes` enforces the canonical 65-byte SEC1 encoding; SM2 public key payloads prefix `distid_len (u16 BE) || distid bytes` ahead of the SEC1 bytes (`crates/iroha_crypto/src/lib.rs`). |

Swift will keep these IDs in a small lookup so `SignatureEnvelope` encodes the
same discriminants that `iroha_crypto` emits. ML‑DSA uses the Dilithium3 /
ML‑DSA‑65 parameter set everywhere in the workspace (see the `pqcrypto_mldsa`
usage inside `PublicKeyFull::MlDsa`), so the Swift SDK only needs to expose that
single security level for IOS4; additional suites (e.g., ML‑DSA‑44/87) can be
added later by extending both `Algorithm` and `SigningAlgorithm`.

NoritoBridge exposes companion FFI entrypoints (`connect_norito_encode_*_signed_transaction_alg`)
that accept the discriminant + raw private key so Swift no longer needs to rely on
Ed25519-only signing paths; `SigningKey` simply forwards the discriminant for each
transaction kind.

## 4. Storage and Hardware Abstractions

| Backend | Usage | Notes |
|---------|-------|-------|
| Secure Enclave (`SecureEnclaveReference`) | Production wallets on iOS/macOS | Wraps `SecKey`, enforces access control, stores attestation certificate chain for governance reviews. |
| Deterministic seed (`SeedDerivationConfig`) | CLI-style/offline tooling | HKDF(salt=`offline.allowance.seed_salt`) with Norito header to keep parity with Android’s AND2 plan. |
| Bridge-backed (`NoritoNativeBridge`) | SM2 / ML-DSA | Adds `sm2Sign`, `sm2Verify`, `mldsa44_sign`, etc. Already partially available for SM2. |

Secure Enclave providers must expose:

- `func attestationPayload() throws -> SecureEnclaveAttestation` so IOS4 can
  reuse Android AND2 governance artefacts.
- `func exportPublicKey() -> Data` to map onto `AccountAddress`.

## 5. Config & Manifest Wiring

- `SigningAlgorithm` maps onto the `iroha_config.crypto.allowed_signing`
  entries; the Swift SDK will refuse to sign with algorithms that the node
  rejected in its latest manifest.
- `Sm2Keypair.defaultDistid()` already threads config defaults via
  `NoritoNativeBridge`. The RFC requires the shared `SigningKey` wrapper to
  expose `.distid` so we can persist it into admission manifests and telemetry.
- Multisig policies reference account IDs via `AccountAddress`, so the builder
  will include helpers that derive Sora canonical IDs using the existing
  `AccountAddress.fromComponents` factory.

## 6. Telemetry & Observability

- Emit `swift.crypto.signing` spans with attributes:
  `algorithm`, `storage_backend`, `attestation_state`, `key_alias`.
- Record Secure Enclave failures and fallback activations using the existing
  telemetry exporters described in
  `docs/source/sdk/swift/telemetry_redaction.md`.
- Multisig helpers emit `swift.multisig.session` metrics (`members_total`,
  `signatures_collected`, `threshold_met`) so operators can prove readiness
  during IOS7 Connect rehearsals.

## 7. Testing Strategy

| Test | Location | Coverage |
|------|----------|----------|
| Unit tests for `SigningKey` backends | `IrohaSwift/Tests/IrohaSwiftTests/SigningKeyTests.swift` | Deterministic signatures, error mapping, attestation export. |
| Multisig policy golden tests | `IrohaSwift/Tests/IrohaSwiftTests/MultisigPolicyBuilderTests.swift` | CTAP2 equivalence vs `AccountAddress.encodeMultisigPolicyCTAP2`. |
| Fixture parity | Shared JSON fixtures under `fixtures/swift/crypto/` | Mirrors `crates/iroha_crypto/tests/sm2_fixture_vectors.rs`. |
| Integration smoke | `swift test --filter CryptoIntegrationTests` | Builds a signed transfer (single + multisig) and replays against Torii mock harness. |

CI will reuse the existing `ci/swift-test.sh` lane and add a `swift-crypto`
label so the roadmap guardrails can assert when crypto/multisig changes land.

## 8. Rollout Plan

1. **Phase A (May 2026)** — Land `SigningAlgorithm`/`SigningKey`, Ed25519 +
   SM2 support, unit tests, and telemetry hooks.
2. **Phase B (June 2026)** — Add secp256k1 + ML-DSA backends once
   `NoritoNativeBridge` exposes them; populate fixture parity harness.
3. **Phase C (July 2026)** — Deliver multisig policy builder + transaction
   extensions, wire into Connect/Torii clients, and update docs/runbooks.
4. **Phase D (Q3 2026)** — Secure Enclave + device inventory review (ties into
   the “hardware requirements review” listed in the roadmap) with attestation
   export evidence recorded in `docs/source/sdk/swift/hardware_matrix.md`.

## 9. Open Items

- **`iroha_crypto` alignment:** confirm the ML-DSA parameter set (44 or 65) the
  Swift SDK should target and export the matching discriminant ID.
- **Secure hardware matrix:** Docs/Support to schedule the dedicated hardware
  review (owners: Swift Observability TL + Android AND2 TL) no later than
  2026‑05‑15; feed outcomes back into `status.md`.
- **Telemetry schema diffs:** Add `swift.crypto.signing` and
  `swift.multisig.session` to the schema tracker before rolling out Phase A.

Publishing this RFC unlocks the next roadmap actions: finishing the
`iroha_crypto` alignment (parameter confirmation + discriminants) and booking
the hardware requirements review.
