import CryptoKit
import Foundation
import XCTest

@testable import IrohaSwift

final class KagemushaAppAttestEnrollmentVerifierV1Tests: XCTestCase {
  private let sampleKeyID = "zgSY9YSD+7TaDXssY6WlOPVS1K3Lmk+pFhlcSWE+ZV0="

  private func fixture(_ name: String, extension ext: String) throws -> Data {
    let url = try XCTUnwrap(Bundle.module.url(forResource: name, withExtension: ext))
    return try Data(contentsOf: url)
  }

  private func sampleAttestation() throws -> Data {
    let text = String(decoding: try fixture("KagemushaAppleGuideAppAttestV1", extension: "b64"),
      as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
    return try XCTUnwrap(Data(base64Encoded: text))
  }

  private func officialRootDER() throws -> Data {
    let pem = String(decoding: try fixture("Apple_App_Attestation_Root_CA", extension: "pem"),
      as: UTF8.self)
    let body = pem.split(separator: "\n")
      .filter { !$0.hasPrefix("-----") }.joined()
    return try XCTUnwrap(Data(base64Encoded: body))
  }

  private func sampleVerifier(root: Data? = nil, appIDHash: Data? = nil,
    category: UInt32 = 1, version: String = "1",
    environment: KagemushaAppAttestEnvironmentV1 = .production,
    date: String = "2026-04-21T12:00:00Z") throws
    -> KagemushaAppAttestEnrollmentVerifierV1 {
    let releaseDigest = try KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
      validationCategory: category, bundleVersion: version)
    let release = try KagemushaAppAttestExpectedReleaseV1(
      validationCategory: category, bundleVersion: version,
      authenticatedAppReleaseDigest: releaseDigest)
    let expectedHash = Data(SHA256.hash(data: Data("1234567890.com.example.myapp".utf8)))
    let verificationDate = try XCTUnwrap(ISO8601DateFormatter().date(from: date))
    return try KagemushaAppAttestEnrollmentVerifierV1(
      rootCertificateDER: try root ?? officialRootDER(),
      expectedAppIDHash: appIDHash ?? expectedHash,
      expectedRelease: release, environment: environment,
      verificationDate: verificationDate)
  }

  private func sampleClientDataHash() -> Data {
    // Apple's published object used these raw bytes at the clientDataHash API boundary.
    // The production KAGEMUSHA binding above always supplies a SHA-256 digest instead.
    Data("example_server_challenge".utf8)
  }

  func testOfficialAppleGuideAttestationVerifiesEnrollment() throws {
    let raw = try sampleAttestation()
    let verifier = try sampleVerifier()
    let checked = try verifier.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash())
    XCTAssertEqual(checked.rawAttestation, raw)
    XCTAssertEqual(checked.authenticatorData[32], 0x40)
    XCTAssertEqual(checked.releaseMeasurement,
      .signed(validationCategory: 1, bundleVersion: "1"))
    XCTAssertEqual(checked.keyID, sampleKeyID)
    XCTAssertEqual(checked.publicKeyX963.count, 65)
    XCTAssertEqual(Data(SHA256.hash(data: checked.publicKeyX963)),
      Data(base64Encoded: sampleKeyID))
    XCTAssertFalse(checked.receipt.isEmpty)
  }

  func testWrongTrustRootNonceKeyAppReleaseAndEnvironmentAreRejected() throws {
    let raw = try sampleAttestation()
    var wrongRoot = try officialRootDER()
    wrongRoot[0] ^= 1
    XCTAssertThrowsError(try sampleVerifier(root: wrongRoot))
    let verifier = try sampleVerifier()
    XCTAssertThrowsError(try verifier.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: Data(repeating: 1, count: 32)))
    XCTAssertThrowsError(try verifier.verify(rawAttestation: raw,
      keyID: Data(repeating: 2, count: 32).base64EncodedString(),
      clientDataHash: sampleClientDataHash()))
    let otherApp = try sampleVerifier(appIDHash: Data(repeating: 1, count: 32))
    XCTAssertThrowsError(try otherApp.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash()))
    let otherCategory = try sampleVerifier(category: 2)
    XCTAssertThrowsError(try otherCategory.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash()))
    let otherVersion = try sampleVerifier(version: "2")
    XCTAssertThrowsError(try otherVersion.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash()))
    let development = try sampleVerifier(environment: .development)
    XCTAssertThrowsError(try development.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash()))
  }

  func testExpiredOrTamperedSampleIsRejected() throws {
    let raw = try sampleAttestation()
    let expired = try sampleVerifier(date: "2026-04-25T12:00:00Z")
    XCTAssertThrowsError(try expired.verify(rawAttestation: raw,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash()))
    var tampered = raw
    tampered[tampered.count - 1] ^= 1
    let verifier = try sampleVerifier()
    XCTAssertThrowsError(try verifier.verify(rawAttestation: tampered,
      keyID: sampleKeyID, clientDataHash: sampleClientDataHash()))
  }

  func testGuideChallengeCannotReplaceKagemushaEnrollmentBinding() throws {
    let verifier = try sampleVerifier()
    let binding = try KagemushaAppAttestEnrollmentBindingV1(
      clientNonce: Data(repeating: 1, count: 32),
      serverNonce: Data(repeating: 2, count: 32),
      releaseID: Data(repeating: 3, count: 32),
      profileID: Data(repeating: 4, count: 32),
      attestedKeyID: Data(repeating: 5, count: 32),
      laneID: Data(repeating: 6, count: 32))
    XCTAssertThrowsError(try verifier.verify(rawAttestation: sampleAttestation(),
      keyID: sampleKeyID, binding: binding))
  }
}
