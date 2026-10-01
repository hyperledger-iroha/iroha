import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Pure E codec/equation cases; these projections create no native capability.
final class KagemushaAppAttestEnrollmentPossessionOriginalV1Tests: XCTestCase {
  private let appID = Data(SHA256.hash(data: Data("TESTTEAM01.example.fixture".utf8)))
  private func release() throws -> KagemushaAppAttestExpectedReleaseV1 {
    try KagemushaAppAttestExpectedReleaseV1(validationCategory: 3, bundleVersion: "1",
      authenticatedAppReleaseDigest: KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
        validationCategory: 3, bundleVersion: "1"))
  }
  func testAppleEHashesExactPossessionOnceAndRejectsDoubleHashOrCOnly() throws {
    let key = try P256.Signing.PrivateKey(rawRepresentation: digest(7))
    let p = try projection(key)
    let hash = Data(SHA256.hash(data: p.signingBytes))
    let raw = try assertion(key, hash: hash)
    let original = try KagemushaAppAttestEnrollmentPossessionOriginalV1(rawAssertion: raw,
      nativeProjection: p, expectedRelease: release())
    XCTAssertEqual(original.clientDataHash, hash); XCTAssertEqual(original.observedCounter, 1)
    for wrong in [p.generationChallenge, Data(SHA256.hash(data: hash))] {
      XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(
        rawAssertion: assertion(key, hash: wrong), nativeProjection: p, expectedRelease: release()))
    }
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(
      rawAssertion: key.signature(for: p.signingBytes).derRepresentation,
      nativeProjection: p, expectedRelease: release()))
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(
      rawAssertion: assertion(key, hash: hash, counter: 0), nativeProjection: p, expectedRelease: release()))
  }
  func testERenewedIntervalAndFinalCredentialCannotBecomePendingPossession() throws {
    let key = try P256.Signing.PrivateKey(rawRepresentation: digest(7))
    var f = fields(key)
    let domain = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8)
    f[1].replaceSubrange((domain.count + 8 + 355)..<(domain.count + 8 + 363), with: u64(1001))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: nil, enrollmentID: digest(1)))
    f = fields(key); f[8] = digest(0x88)
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: nil, enrollmentID: digest(1)))
  }
  private func projection(_ key: P256.Signing.PrivateKey) throws -> KagemushaAppPlatformPreparedProjectionV1 {
    try KagemushaAppPlatformPreparedProjectionV1(nativeFields: fields(key), approvalID: nil, enrollmentID: digest(1))
  }
  private func fields(_ key: P256.Signing.PrivateKey) -> [Data] {
    let point = key.publicKey.x963Representation, keyID = Data(SHA256.hash(data: point))
    var c = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
      + u64(451) + Data([1, 0, 2])
    for i in 1...13 { c.append(digest(UInt8(i))) }
    for i in [UInt64(1), 2, 1000, 121000] { c.append(u64(i)) }
    var e = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8)
      + u64(371) + Data([1, 0, 1])
    for d in [digest(1), digest(2), digest(3), digest(4), digest(5), digest(11),
      digest(7), digest(8), digest(6), keyID, digest(0x77)] { e.append(d) }
    e.append(u64(1000)); e.append(u64(121000))
    return [u64(9), e, Data([4]), Data(keyID.base64EncodedString().utf8), Data(SHA256.hash(data: c)),
      point, keyID, c, Data(), digest(0x99), Data(repeating: 0, count: 4), Data([0]), appID, Data()]
  }
  private func assertion(_ key: P256.Signing.PrivateKey, hash: Data, counter: UInt8 = 1) throws -> Data {
    let auth = appID + Data([0x40, 0, 0, 0, counter])
    let nonce = Data(SHA256.hash(data: auth + hash))
    return Data([0xa2]) + text("signature") + bytes(try key.signature(for: nonce).derRepresentation)
      + text("authenticatorData") + bytes(auth)
  }
  private func text(_ value: String) -> Data { Data([0x60 | UInt8(value.utf8.count)]) + Data(value.utf8) }
  private func bytes(_ data: Data) -> Data {
    (data.count < 24 ? Data([0x40 | UInt8(data.count)]) : Data([0x58, UInt8(data.count)])) + data
  }
  private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }
  private func u64(_ value: UInt64) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
}
