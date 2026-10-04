import Foundation

// Enrollment step E5 of the iPhone wallet adapter: App Attest evidence for a Secure Enclave
// payment key (spec §2.2; G2 design rev 2, iOS "Enrollment evidence"). The App Attest calls go
// through the retained `KagemushaAppAttestServiceV1` protocol; this adapter never calls
// DeviceCheck itself.

/// App Attest enrollment evidence of one payment key.
public struct KagemushaWalletAppleEnrollmentEvidenceV1: Equatable, Sendable {
  /// Identifier of the fresh App Attest key; the enrollment owner keeps it for renewal
  /// assertions (an App Attest key is attested only once).
  public let appAttestKeyID: String
  /// `attestKey(clientDataHash: challenge_digest)`.
  public let attestation: Data
  /// `generateAssertion(clientDataHash: key_binding_digest)` by the same App Attest key.
  public let keyBindingAssertion: Data
}

/// Why App Attest enrollment evidence was not produced. Nothing is deleted or regenerated.
public enum KagemushaWalletAppleEnrollmentAttestationErrorV1: Error, Equatable, Sendable {
  /// A digest is not 32 bytes.
  case invalidDigest
  /// App Attest is unsupported on this device; enrollment is refused.
  case appAttestUnsupported
  /// The keychain gave no definitive answer about the payment key; retry.
  case paymentKeyUnavailable(KagemushaWalletAppleUnavailableV1)
  /// The keychain definitively reports no payment key for the slot.
  case paymentKeyAbsent
  /// The slot's payment key is not `paymentPublicKey`, or was generated for another challenge.
  case bindingMismatch
  /// An App Attest call failed at `stage` (`generateKey`, `attestKey`, `generateAssertion`).
  /// A retry uses a fresh App Attest key.
  case appAttestFailed(stage: String)
}

extension KagemushaWalletApplePlatformV1 {
  /// Produce App Attest evidence for the slot's payment key (design E5, after the payment key
  /// and the generation-0 marker are durable).
  ///
  /// The payment key must be `paymentPublicKey` and must have been generated for
  /// `challengeDigest` (recorded with the key at generation). A fresh App Attest key is
  /// generated and attested with `clientDataHash = challengeDigest`; the same key then asserts
  /// `keyBindingDigest`, the Rust `enrollment-key-binding` digest that binds the payment
  /// public key and the challenge. Each call uses a fresh App Attest key, so a crash after
  /// attestation but before the evidence is retained never reuses an attested key.
  // TODO(G2-bridge): the Rust enrollment owner supplies `keyBindingDigest` and retains the
  // evidence (and the App Attest key identifier for renewal) in its E5 credential request.
  public func attestEnrollment(
    slot: KagemushaWalletAppleSlotV1, paymentPublicKey: Data, challengeDigest: Data,
    keyBindingDigest: Data
  ) async throws -> KagemushaWalletAppleEnrollmentEvidenceV1 {
    guard challengeDigest.count == 32, keyBindingDigest.count == 32 else {
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
    let keyID: String
    do {
      keyID = try await appAttest.generateKey()
    } catch {
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestFailed(stage: "generateKey")
    }
    let attestation: Data
    do {
      attestation = try await appAttest.attestKey(keyID, clientDataHash: challengeDigest)
    } catch {
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestFailed(stage: "attestKey")
    }
    let assertion: Data
    do {
      assertion = try await appAttest.generateAssertion(keyID, clientDataHash: keyBindingDigest)
    } catch {
      throw KagemushaWalletAppleEnrollmentAttestationErrorV1.appAttestFailed(
        stage: "generateAssertion")
    }
    return KagemushaWalletAppleEnrollmentEvidenceV1(
      appAttestKeyID: keyID, attestation: attestation, keyBindingAssertion: assertion)
  }
}
