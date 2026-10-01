import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Real CryptoKit E signature/equation tests with untrusted public projections.
/// Fixture keys and expected release values grant no hardware or native admission.
final class KagemushaAppAttestMeasuredEnrollmentPossessionV1Tests: XCTestCase {
  private let appID = Data(SHA256.hash(data: Data("TESTTEAM01.example.measured-e".utf8)))

  func testMeasuredEAuthenticatesWholeOriginalInBothFlagsAndKnownKeyOrders() throws {
    let key = try P256.Signing.PrivateKey(rawRepresentation: digest(7)), p = try projection(key)
    let version = String(repeating: "1", count: 128), expected = try release(version)
    for flag: UInt8 in [0x40, 0xc0] {
      for reversed in [false, true] {
        let auth = authenticator(flag: flag, extensions: extensions(version: version, reversed: reversed))
        XCTAssertEqual(auth.count, 206)
        let raw = try signed(key, auth: auth, hash: Data(SHA256.hash(data: p.signingBytes)), reversed: reversed)
        XCTAssertLessThanOrEqual(raw.count, 311)
        let result = try KagemushaAppAttestEnrollmentPossessionOriginalV1(rawAssertion: raw,
          nativeProjection: p, expectedRelease: expected)
        XCTAssertEqual(result.rawAssertion, raw)
        XCTAssertEqual(result.clientDataHash, Data(SHA256.hash(data: p.signingBytes)))
        XCTAssertEqual(result.observedCounter, 1)
        XCTAssertEqual(result.releaseMeasurement, .signed(validationCategory: 3, bundleVersion: version))
      }
    }
  }

  func testMeasuredERejectsReleaseKeyScopeSignatureAndCanonicalMutations() throws {
    let key = try P256.Signing.PrivateKey(rawRepresentation: digest(7)), p = try projection(key)
    let hash = Data(SHA256.hash(data: p.signingBytes)), expected = try release("1")
    let auth = authenticator(flag: 0xc0, extensions: extensions(version: "1"))
    let raw = try signed(key, auth: auth, hash: hash)
    var tampered = raw; tampered[tampered.count - 1] ^= 1
    let wrongRelease = authenticator(flag: 0xc0, extensions: extensions(version: "2"))
    let wrongCategory = authenticator(flag: 0xc0, extensions: extensions(version: "1", category: 4))
    let wrongRP = digest(0x23) + Data(auth.dropFirst(32))
    let duplicate = Data([0xa2]) + text("bundleVersion") + text("1") + text("bundleVersion") + text("1")
    let unknown = Data([0xa2]) + text("validationCategory") + bytes(u32(3)) + text("other") + text("1")
    let oversize = authenticator(flag: 0xc0, extensions: extensions(version: String(repeating: "1", count: 129)))
    let nonminimalAuth = try signed(key, auth: auth, hash: hash, nonminimalAuthLength: true)
    let candidates = [tampered, raw + Data([0]),
      try signed(key, auth: wrongRelease, hash: hash),
      try signed(key, auth: wrongCategory, hash: hash),
      try signed(key, auth: wrongRP, hash: hash),
      try signed(key, auth: auth, hash: Data(SHA256.hash(data: hash))),
      try signed(key, auth: auth, hash: p.generationChallenge),
      try signed(key, auth: authenticator(flag: 0x80, extensions: extensions(version: "1")), hash: hash),
      try signed(key, auth: authenticator(flag: 0xc0, counter: 0, extensions: extensions(version: "1")), hash: hash),
      try signed(key, auth: authenticator(flag: 0xc0, extensions: duplicate), hash: hash),
      try signed(key, auth: authenticator(flag: 0xc0, extensions: unknown), hash: hash),
      try signed(key, auth: oversize, hash: hash), nonminimalAuth]
    for invalid in candidates {
      XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(rawAssertion: invalid,
        nativeProjection: p, expectedRelease: expected))
    }
    var fields = preparedFields(key); fields[10] = u32(1)
    let advancedFloor = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: fields,
      approvalID: nil, enrollmentChallengeHash: fields[4])
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(rawAssertion: raw,
      nativeProjection: advancedFloor, expectedRelease: expected))
    let foreignKey = try P256.Signing.PrivateKey(rawRepresentation: digest(8))
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(
      rawAssertion: signed(foreignKey, auth: auth, hash: hash), nativeProjection: p, expectedRelease: expected))
  }

  func testLegacyEExplicitlyLacksReleaseMeasurementAndRejectsEmptyMeasuredSuffix() throws {
    let key = try P256.Signing.PrivateKey(rawRepresentation: digest(7)), p = try projection(key)
    let hash = Data(SHA256.hash(data: p.signingBytes)), auth = authenticator(flag: 0x40)
    XCTAssertEqual(auth.count, 37)
    let raw = try signed(key, auth: auth, hash: hash)
    let checked = try KagemushaAppAttestEnrollmentPossessionOriginalV1(rawAssertion: raw,
      nativeProjection: p, expectedRelease: release("1"))
    XCTAssertEqual(checked.rawAssertion, raw); XCTAssertEqual(checked.releaseMeasurement, .unavailable)
    for unsupported in [authenticator(flag: 0xc0), authenticator(flag: 0xc0, extensions: Data([0xa0]))] {
      XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(
        rawAssertion: signed(key, auth: unsupported, hash: hash), nativeProjection: p, expectedRelease: release("1")))
    }
    XCTAssertThrowsError(try KagemushaAppAttestEnrollmentPossessionOriginalV1(
      rawAssertion: Data(repeating: 1, count: 312), nativeProjection: p, expectedRelease: release("1")))
  }

  private func projection(_ key: P256.Signing.PrivateKey) throws -> KagemushaAppPlatformPreparedProjectionV1 {
    let f = preparedFields(key)
    return try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: nil, enrollmentChallengeHash: f[4])
  }
  private func preparedFields(_ key: P256.Signing.PrivateKey) -> [Data] {
    let point = key.publicKey.x963Representation, keyID = Data(SHA256.hash(data: point))
    var c = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
      + u64(451) + Data([1, 0, 2])
    for i in 1...13 { c.append(digest(UInt8(i))) }
    for i in [UInt64(1), 2, 1000, 121000] { c.append(u64(i)) }
    let challengeHash = Data(SHA256.hash(data: c))
    var e = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8)
      + u64(371) + Data([1, 0, 1])
    for d in [challengeHash, digest(2), digest(3), digest(4), digest(5), digest(11),
      digest(7), digest(8), digest(6), keyID, digest(0x77)] { e.append(d) }
    e.append(u64(1000)); e.append(u64(121000))
    return [u64(9), e, Data([4]), Data(keyID.base64EncodedString().utf8), challengeHash,
      point, keyID, c, Data(), digest(0x99), u32(0), Data([0]), appID, Data()]
  }
  private func release(_ version: String) throws -> KagemushaAppAttestExpectedReleaseV1 {
    try KagemushaAppAttestExpectedReleaseV1(validationCategory: 3, bundleVersion: version,
      authenticatedAppReleaseDigest: KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
        validationCategory: 3, bundleVersion: version))
  }
  private func extensions(version: String, category: UInt32 = 3, reversed: Bool = false) -> Data {
    let categoryField = text("validationCategory") + bytes(u32(category))
    let versionField = text("bundleVersion") + text(version)
    return Data([0xa2]) + (reversed ? versionField + categoryField : categoryField + versionField)
  }
  private func authenticator(flag: UInt8, counter: UInt8 = 1, extensions: Data = Data()) -> Data {
    appID + Data([flag, 0, 0, 0, counter]) + extensions
  }
  private func signed(_ key: P256.Signing.PrivateKey, auth: Data, hash: Data,
    reversed: Bool = false, nonminimalAuthLength: Bool = false) throws -> Data {
    let nonce = Data(SHA256.hash(data: auth + hash))
    let signature = try key.signature(for: nonce).derRepresentation
    let authBytes = nonminimalAuthLength ? Data([0x59, 0, UInt8(auth.count)]) + auth : bytes(auth)
    let authField = text("authenticatorData") + authBytes
    let signatureField = text("signature") + bytes(signature)
    return Data([0xa2]) + (reversed ? authField + signatureField : signatureField + authField)
  }
  private func text(_ value: String) -> Data {
    let raw = Data(value.utf8)
    return (raw.count < 24 ? Data([0x60 | UInt8(raw.count)]) : Data([0x78, UInt8(raw.count)])) + raw
  }
  private func bytes(_ raw: Data) -> Data {
    (raw.count < 24 ? Data([0x40 | UInt8(raw.count)]) : Data([0x58, UInt8(raw.count)])) + raw
  }
  private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }
  private func u32(_ value: UInt32) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
  private func u64(_ value: UInt64) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
}
