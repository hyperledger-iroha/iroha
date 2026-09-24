import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaAppEnrollmentPreparationVerifierV1Tests: XCTestCase {
  private let policyID = Data(repeating: 0x57, count: 32)

  private func binding() throws -> KagemushaAppAttestEnrollmentBindingV1 {
    try KagemushaAppAttestEnrollmentBindingV1(
      clientNonce: Data(repeating: 0x11, count: 32),
      serverNonce: Data(repeating: 0x22, count: 32),
      releaseID: Data(repeating: 0x33, count: 32),
      profileID: Data(repeating: 0x44, count: 32),
      attestedKeyID: Data(repeating: 0x55, count: 32),
      laneID: Data(repeating: 0x66, count: 32))
  }

  private func account(seed: UInt8) throws -> String {
    let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: seed, count: 32))
    return try AccountAddress.fromAccount(publicKey: key.publicKey.rawRepresentation)
      .toI105(networkPrefix: 753)
  }

  private func preparation(key: Curve25519.Signing.PrivateKey, account: String,
    binding: KagemushaAppAttestEnrollmentBindingV1,
    issued: UInt64 = 1_000_000) throws -> Data {
    var frame = Data([1])
    for value in [issued, issued + 120_000] {
      for offset in 0..<8 { frame.append(UInt8(truncatingIfNeeded: value >> (8 * offset))) }
    }
    for value in [binding.clientNonce, binding.serverNonce, binding.releaseID,
      binding.profileID, binding.attestedKeyID, binding.laneID] { frame.append(value) }
    XCTAssertEqual(frame.count, 209)
    var message = Data("iroha:kagemusha:v1:app-enrollment-preparation\0".utf8)
    message.append(frame[1..<209])
    message.append(policyID)
    message.append(contentsOf: SHA256.hash(data: Data(account.utf8)))
    frame.append(try key.signature(for: message))
    return frame
  }

  func testVerifiesOnlyExactPinnedSixFieldTranscript() throws {
    let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x42, count: 32))
    let selected = try binding()
    let canonicalAccount = try account(seed: 0x31)
    let frame = try preparation(key: key, account: canonicalAccount, binding: selected)
    let verifier = try KagemushaAppEnrollmentPreparationVerifierV1(
      issuerPublicKey: key.publicKey.rawRepresentation, issuerPolicyID: policyID)
    XCTAssertNoThrow(try verifier.verify(frame, canonicalAccountID: canonicalAccount,
      binding: selected, nowMS: 1_000_001))

    let changedKey = try KagemushaAppAttestEnrollmentBindingV1(
      clientNonce: selected.clientNonce, serverNonce: selected.serverNonce,
      releaseID: selected.releaseID, profileID: selected.profileID,
      attestedKeyID: Data(repeating: 0x77, count: 32), laneID: selected.laneID)
    XCTAssertThrowsError(try verifier.verify(frame, canonicalAccountID: canonicalAccount,
      binding: changedKey, nowMS: 1_000_001))
    XCTAssertThrowsError(try verifier.verify(frame, canonicalAccountID: try account(seed: 0x32),
      binding: selected, nowMS: 1_000_001))
    XCTAssertThrowsError(try KagemushaAppEnrollmentPreparationVerifierV1(
      issuerPublicKey: key.publicKey.rawRepresentation,
      issuerPolicyID: Data(repeating: 0x58, count: 32))
      .verify(frame, canonicalAccountID: canonicalAccount, binding: selected,
        nowMS: 1_000_001))
    XCTAssertThrowsError(try verifier.verify(frame, canonicalAccountID: canonicalAccount,
      binding: selected, nowMS: 1_120_000))

    var changedSignature = frame
    changedSignature[272] ^= 1
    XCTAssertThrowsError(try verifier.verify(changedSignature,
      canonicalAccountID: canonicalAccount, binding: selected, nowMS: 1_000_001))
    XCTAssertThrowsError(try verifier.verify(Data(frame.dropLast()),
      canonicalAccountID: canonicalAccount, binding: selected, nowMS: 1_000_001))
  }
}
