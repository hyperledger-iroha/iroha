import Foundation

// Enrollment step E5 of the iPhone wallet adapter: App Attest evidence for a Secure Enclave
// payment key (spec §2.2; G2 design rev 2, iOS "Enrollment evidence"). The App Attest calls go
// through the retained `KagemushaAppAttestServiceV1` protocol; this adapter never calls
// DeviceCheck itself.

/// App Attest enrollment evidence of one payment key.
public struct KagemushaWalletAppleEnrollmentEvidenceV1: Equatable, Sendable {
  /// App Attest counter consumed by ``keyBindingAssertion``: the first assertion of a fresh App
  /// Attest key (its attestation carries counter 0). Any later assertion owner of
  /// ``appAttestKeyID`` (renewal) continues after this counter; it never bootstraps the key as
  /// unused (`KagemushaAppAttestFileIntentStoreV1.bootstrapNew` starts at 0).
  public static let keyBindingAssertionCounter: UInt32 = 1

  /// Identifier of the fresh App Attest key (base64 of 32 bytes); the enrollment owner keeps
  /// it for renewal assertions (an App Attest key is attested only once).
  public let appAttestKeyID: String
  /// `attestKey(clientDataHash: challenge_digest)`.
  public let attestation: Data
  /// `H("enrollment-key-binding", challenge_digest || payment_key)`, the Rust
  /// `kagemusha_wallet_enrollment_key_binding_v1`: the `clientDataHash` of
  /// ``keyBindingAssertion``.
  public let keyBindingDigest: Data
  /// `generateAssertion(clientDataHash: keyBindingDigest)` by the same App Attest key, at
  /// counter ``keyBindingAssertionCounter``.
  public let keyBindingAssertion: Data
}

/// App Attest call of enrollment step E5.
public enum KagemushaWalletAppleAppAttestStageV1: String, Equatable, Hashable, Sendable {
  case generateKey
  case attestKey
  case generateAssertion
}

/// Why App Attest enrollment evidence was not produced. Nothing is deleted or regenerated.
public enum KagemushaWalletAppleEnrollmentAttestationErrorV1: Error, Equatable, Sendable {
  /// The challenge digest is not 32 bytes.
  case invalidDigest
  /// App Attest is unsupported on this device; enrollment is refused.
  case appAttestUnsupported
  /// The keychain gave no definitive answer about the payment key; retry.
  case paymentKeyUnavailable(KagemushaWalletAppleUnavailableV1)
  /// The keychain definitively reports no payment key for the slot.
  case paymentKeyAbsent
  /// The slot's payment key is not `paymentPublicKey`, or was not generated for
  /// `challengeDigest` (its recorded binding is missing, malformed or different).
  case bindingMismatch
  /// App Attest returned a key identifier that is not base64 of 32 bytes; nothing was attested.
  case malformedAppAttestKeyID
  /// An App Attest call failed at `stage` with the underlying error's `domain` and `code`
  /// (`DCError.Code` in `DCErrorDomain`: for example `serverUnavailable` is worth retrying,
  /// `featureUnsupported` is not). A retry uses a fresh App Attest key.
  case appAttestFailed(stage: KagemushaWalletAppleAppAttestStageV1, domain: String, code: Int)
}

extension KagemushaWalletApplePlatformV1 {
  /// `H("enrollment-key-binding", challengeDigest || paymentPublicKey)` (Rust
  /// `kagemusha_wallet_enrollment_key_binding_v1`).
  static func enrollmentKeyBindingDigest(challengeDigest: Data, paymentPublicKey: Data) -> Data {
    KagemushaWalletWireV1.digest(role: .enrollmentKeyBinding, body: challengeDigest + paymentPublicKey)
  }

  /// Produce App Attest evidence for the slot's payment key (design E5, after the payment key
  /// and the generation-0 marker are durable).
  ///
  /// The payment key must be `paymentPublicKey` and must have been generated for
  /// `challengeDigest` (recorded with the key at generation). A fresh App Attest key is
  /// generated and attested with `clientDataHash = challengeDigest`; the same key then asserts
  /// the enrollment key-binding digest, computed here from `challengeDigest` and the bound
  /// payment key, so the assertion binds exactly that key. Each call uses a fresh App Attest
  /// key, so a crash after attestation but before the evidence is retained never reuses an
  /// attested key.
  // TODO(G2-bridge): the Rust enrollment owner retains the evidence, the App Attest key
  // identifier and its consumed assertion counter in its E5 credential request; it can check
  // `keyBindingDigest` against `kagemusha_wallet_enrollment_key_binding_v1`.
  public func attestEnrollment(
    slot: KagemushaWalletAppleSlotV1, paymentPublicKey: Data, challengeDigest: Data
  ) async throws -> KagemushaWalletAppleEnrollmentEvidenceV1 {
    guard challengeDigest.count == 32 else {
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.invalidDigest
    }
    guard appAttest.isSupported else {
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestUnsupported
    }
    switch bracketed({ lookupPaymentKey(slot) }) {
    case .present(let item):
      guard item.publicKey == paymentPublicKey,
        let binding = item.label.flatMap(KagemushaWalletAppleKeyGenerationRequestV1.init(label:)),
        binding.challengeDigest == challengeDigest
      else { throw KagemushaWalletAppleEnrollmentAttestationErrorV1.bindingMismatch }
    case .absent:
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.paymentKeyAbsent
    case .unavailable(let reason):
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.paymentKeyUnavailable(reason)
    }
    let keyBindingDigest = Self.enrollmentKeyBindingDigest(
      challengeDigest: challengeDigest, paymentPublicKey: paymentPublicKey)
    let keyID = try await appAttestCall(.generateKey) { try await appAttest.generateKey() }
    guard Data(base64Encoded: keyID)?.count == 32 else {
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.malformedAppAttestKeyID
    }
    let attestation = try await appAttestCall(.attestKey) {
      try await appAttest.attestKey(keyID, clientDataHash: challengeDigest)
    }
    let assertion = try await appAttestCall(.generateAssertion) {
      try await appAttest.generateAssertion(keyID, clientDataHash: keyBindingDigest)
    }
    return KagemushaWalletAppleEnrollmentEvidenceV1(
      appAttestKeyID: keyID, attestation: attestation, keyBindingDigest: keyBindingDigest,
      keyBindingAssertion: assertion)
  }

  /// Run one App Attest call, keeping the underlying error's domain and code on failure.
  private func appAttestCall<Value>(
    _ stage: KagemushaWalletAppleAppAttestStageV1, _ call: () async throws -> Value
  ) async throws -> Value {
    do {
      return try await call()
    } catch {
      let underlying = error as NSError
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestFailed(
        stage: stage, domain: underlying.domain, code: underlying.code)
    }
  }
}
