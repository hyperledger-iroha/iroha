import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaAppEnrollmentPreparationVerifierV1Tests: XCTestCase {
  private let policyID = Data(repeating: 0x57, count: 32)

  private func binding() throws -> KagemushaAppEnrollmentPreparationBindingV1 {
    try KagemushaAppEnrollmentPreparationBindingV1(
      platformClass: .appleAppAttest,
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
    binding: KagemushaAppEnrollmentPreparationBindingV1,
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

    let changedKey = try KagemushaAppEnrollmentPreparationBindingV1(
      platformClass: .appleAppAttest,
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

  func testExpiredRetainedFrameReauthenticatesWithoutGrantingFreshUse() throws {
    let key = try Curve25519.Signing.PrivateKey(
      rawRepresentation: Data(repeating: 0x42, count: 32))
    let selected = try binding()
    let canonicalAccount = try account(seed: 0x31)
    let frame = try preparation(key: key, account: canonicalAccount, binding: selected)
    let verifier = try KagemushaAppEnrollmentPreparationVerifierV1(
      issuerPublicKey: key.publicKey.rawRepresentation, issuerPolicyID: policyID)

    XCTAssertNoThrow(try verifier.verifySignedFields(frame,
      canonicalAccountID: canonicalAccount, binding: selected))
    XCTAssertThrowsError(try verifier.verify(frame,
      canonicalAccountID: canonicalAccount, binding: selected, nowMS: 1_120_000))

    var tampered = frame
    tampered[209] ^= 1
    XCTAssertThrowsError(try verifier.verifySignedFields(tampered,
      canonicalAccountID: canonicalAccount, binding: selected))
    XCTAssertThrowsError(try verifier.verifySignedFields(frame,
      canonicalAccountID: try account(seed: 0x32), binding: selected))
  }

  func testIndependentlyIssuedAndroidPreparationCrossSDKFixture() throws {
    let fixture = try loadAndroidPreparationFixture()
    XCTAssertEqual(try XCTUnwrap(fixture["schema"] as? String),
      "iroha.kagemusha.app-preparation.android.v1")
    let policy = try XCTUnwrap(Data(base64Encoded:
      XCTUnwrap(fixture["canonicalPolicyBase64"] as? String)))
    let policyDigest = SHA256.hash(data: policy).map { String(format: "%02x", $0) }.joined()
    XCTAssertEqual(policyDigest, try XCTUnwrap(fixture["policySha256Hex"] as? String))

    let issuer = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 81, count: 32))
    let verifier = try KagemushaAppEnrollmentPreparationVerifierV1(
      issuerPublicKey: issuer.publicKey.rawRepresentation,
      issuerPolicyID: Data(repeating: 71, count: 32))
    let binding = try KagemushaAppEnrollmentPreparationBindingV1(
      platformClass: .androidKeyMint,
      clientNonce: try fixtureHex(fixture, "clientNonceHex"),
      serverNonce: try fixtureHex(fixture, "serverNonceHex"),
      releaseID: try fixtureHex(fixture, "releaseIdHex"),
      profileID: try fixtureHex(fixture, "profileIdHex"),
      attestedKeyID: Data(repeating: 0, count: 32),
      laneID: try fixtureHex(fixture, "laneIdHex"))
    let frame = try XCTUnwrap(Data(base64Encoded:
      XCTUnwrap(fixture["signedPreparationBase64"] as? String)))
    let accountID = try XCTUnwrap(fixture["accountI105"] as? String)
    let nowMS = try XCTUnwrap(fixture["trustedNowMs"] as? NSNumber).uint64Value
    XCTAssertNoThrow(try verifier.verify(frame, canonicalAccountID: accountID,
      binding: binding, nowMS: nowMS))

    var corrupt = frame
    corrupt[209] ^= 1
    XCTAssertThrowsError(try verifier.verify(corrupt, canonicalAccountID: accountID,
      binding: binding, nowMS: nowMS))
    XCTAssertThrowsError(try verifier.verify(frame, canonicalAccountID: accountID,
      binding: binding, nowMS: nowMS + 120_000))
    XCTAssertThrowsError(try KagemushaAppEnrollmentPreparationVerifierV1(
      issuerPublicKey: issuer.publicKey.rawRepresentation,
      issuerPolicyID: Data(repeating: 0, count: 32)))
    XCTAssertThrowsError(try KagemushaAppEnrollmentPreparationBindingV1(
      platformClass: .appleAppAttest,
      clientNonce: binding.clientNonce, serverNonce: binding.serverNonce,
      releaseID: binding.releaseID, profileID: binding.profileID,
      attestedKeyID: binding.attestedKeyID, laneID: binding.laneID))
    XCTAssertThrowsError(try KagemushaAppEnrollmentPreparationBindingV1(
      platformClass: .androidKeyMint,
      clientNonce: binding.clientNonce, serverNonce: binding.serverNonce,
      releaseID: binding.releaseID, profileID: binding.profileID,
      attestedKeyID: Data(repeating: 1, count: 32), laneID: binding.laneID))
  }

  private func loadAndroidPreparationFixture() throws -> [String: Any] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent(
        "fixtures/offline/kagemusha_signed_app_preparation_android_v1.json")
      if FileManager.default.fileExists(atPath: path.path) {
        return try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any])
      }
      directory.deleteLastPathComponent()
    }
    throw NSError(domain: "KagemushaAppEnrollmentPreparationVerifierV1Tests", code: -1)
  }

  private func fixtureHex(_ fixture: [String: Any], _ key: String) throws -> Data {
    let value = try XCTUnwrap(fixture[key] as? String)
    guard value.count.isMultiple(of: 2) else {
      throw NSError(domain: "KagemushaAppEnrollmentPreparationVerifierV1Tests", code: -2)
    }
    var bytes = Data()
    for offset in stride(from: 0, to: value.count, by: 2) {
      let first = value.index(value.startIndex, offsetBy: offset)
      let next = value.index(first, offsetBy: 2)
      bytes.append(try XCTUnwrap(UInt8(value[first..<next], radix: 16)))
    }
    return bytes
  }
}
