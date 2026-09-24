import CryptoKit
import Foundation

/// Verifies the exact issuer preparation before App Attest receives its challenge.
/// The public key and policy ID must come from authenticated release policy.
public struct KagemushaAppEnrollmentPreparationVerifierV1: Sendable {
  private static let signingDomain = Data("iroha:kagemusha:v1:app-enrollment-preparation\0".utf8)
  private let issuerPublicKey: Curve25519.Signing.PublicKey
  private let issuerPolicyID: Data

  public init(issuerPublicKey: Data, issuerPolicyID: Data) throws {
    guard issuerPublicKey.count == 32,
      issuerPolicyID.count == 32, issuerPolicyID.contains(where: { $0 != 0 }),
      let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: issuerPublicKey)
    else { throw KagemushaAppEnrollmentPreparationErrorV1.invalidPolicy }
    self.issuerPublicKey = publicKey
    self.issuerPolicyID = issuerPolicyID
  }

  /// Verifies version, exact six-field selection, lease, account and Ed25519 signature.
  /// This is issuer authentication; hardware qualification and wallet admission follow it.
  public func verify(_ preparation: Data, canonicalAccountID: String,
    binding: KagemushaAppAttestEnrollmentBindingV1, nowMS: UInt64) throws {
    guard preparation.count == 273, preparation[0] == 1 else {
      throw KagemushaAppEnrollmentPreparationErrorV1.invalidFrame
    }
    // Native address admission must agree with these exact canonical I105 bytes.
    guard !canonicalAccountID.isEmpty,
      !canonicalAccountID.utf8.contains(0),
      (try? KagemushaAccountIDV1(canonicalAccountID)) != nil
    else { throw KagemushaAppEnrollmentPreparationErrorV1.invalidAccount }
    let expected = [binding.clientNonce, binding.serverNonce, binding.releaseID,
      binding.profileID, binding.attestedKeyID, binding.laneID]
    for index in 0..<expected.count {
      let lower = 17 + 32 * index
      guard Data(preparation[lower..<(lower + 32)]) == expected[index] else {
        throw KagemushaAppEnrollmentPreparationErrorV1.selectionMismatch
      }
    }
    let issued = Self.readUInt64LE(preparation, from: 1)
    let expires = Self.readUInt64LE(preparation, from: 9)
    guard issued > 0, issued <= UInt64.max - 120_000,
      expires == issued + 120_000, nowMS >= issued, nowMS < expires
    else { throw KagemushaAppEnrollmentPreparationErrorV1.invalidLease }
    var message = Self.signingDomain
    message.append(preparation[1..<209])
    message.append(issuerPolicyID)
    message.append(contentsOf: SHA256.hash(data: Data(canonicalAccountID.utf8)))
    guard issuerPublicKey.isValidSignature(preparation[209..<273], for: message) else {
      throw KagemushaAppEnrollmentPreparationErrorV1.invalidSignature
    }
  }

  private static func readUInt64LE(_ bytes: Data, from start: Int) -> UInt64 {
    var result: UInt64 = 0
    for offset in 0..<8 { result |= UInt64(bytes[start + offset]) << (8 * offset) }
    return result
  }
}

public enum KagemushaAppEnrollmentPreparationErrorV1: Error, Equatable, Sendable {
  case invalidPolicy
  case invalidFrame
  case invalidAccount
  case selectionMismatch
  case invalidLease
  case invalidSignature
}
