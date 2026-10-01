import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Codec and cryptographic equation tests only. These fixtures are not native
/// preparation capabilities, platform attestation, or monetary authority.
final class KagemushaAppApprovalSigningProjectionV1Tests: XCTestCase {
  private func vectors() throws -> [String: Data] {
    let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
      .appendingPathComponent("Fixtures/kagemusha_app_platform_messages_v1.tsv")
    let text = try String(contentsOf: path, encoding: .utf8)
    var output: [String: Data] = [:]
    for line in text.split(separator: "\n") where !line.hasPrefix("#") {
      let parts = line.split(separator: "\t", omittingEmptySubsequences: false)
      guard parts.count == 2, parts[1].count % 2 == 0 else { throw Failure.invalidFixture }
      let hex = Array(parts[1].utf8)
      var bytes = Data()
      for offset in stride(from: 0, to: hex.count, by: 2) {
        guard let value = UInt8(String(decoding: hex[offset..<(offset + 2)], as: UTF8.self), radix: 16)
        else { throw Failure.invalidFixture }
        bytes.append(value)
      }
      guard output.updateValue(bytes, forKey: String(parts[0])) == nil else { throw Failure.invalidFixture }
    }
    return output
  }

  private enum Failure: Error { case invalidFixture }
  private func projection(_ wrapper: Data? = nil) throws -> KagemushaAppApprovalSigningProjectionV1 {
    let values = try vectors()
    return try KagemushaAppApprovalSigningProjectionV1(
      nativeSigningBytes: try XCTUnwrap(wrapper ?? values["w_approval_native_first"]),
      nativeFinancialSubject: try XCTUnwrap(values["s_mint_native_first"]))
  }

  func testCanonicalWMatchesActualNativeSerializerBytesAndHashes() throws {
    let values = try vectors()
    let value = try projection()
    XCTAssertEqual(value.canonicalSigningBytes.count, 325)
    XCTAssertEqual(value.canonicalFinancialSubject.count, 460)
    XCTAssertEqual(value.operationID, Data(repeating: 0x21, count: 32))
    XCTAssertEqual(value.nonce, Data(repeating: 0x22, count: 32))
    XCTAssertEqual(value.accountBinding, try hex("d0e2e9e612aea39ce6debabd529fb1b7e35e1ad8a92999309a96f511cc04f820"))
    XCTAssertEqual(value.authorityPolicyDigest, Data(repeating: 0x24, count: 32))
    XCTAssertEqual(value.attestedKeyID, Data(repeating: 0x25, count: 32))
    XCTAssertEqual(value.enrollmentDigest, Data(repeating: 0x26, count: 32))
    XCTAssertEqual(value.normalizedGuardDigest, Data(repeating: 0x28, count: 32))
    XCTAssertEqual(value.subjectSigningDigest, values["s_mint_native_first_sha256"])
    XCTAssertEqual(value.clientDataHash, values["w_approval_native_first_sha256"])
    XCTAssertEqual(value.issuedAtMS, 1_000)
    XCTAssertEqual(value.expiresAtMS, 2_000)
    // The exact root-owned terminal Rust artifact is checked below; no capability is decoded.
  }

  func testAllTenActualModelVectorsAcrossOperationsAndUInt128CarryBoundary() throws {
    let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
      .appendingPathComponent("Fixtures/kagemusha_app_owned_hardware_native_vectors_v1.json")
    let raw = try Data(contentsOf: path)
    XCTAssertEqual(Data(SHA256.hash(data: raw)), try hex("94c94415d076675dee98c0de5931d9314ffab4b6434b8ad1f1bad0aeb30ade16"))
    let root = try XCTUnwrap(JSONSerialization.jsonObject(with: raw) as? [String: Any])
    XCTAssertEqual(root["codec_only"] as? Bool, true)
    for flag in ["hardware_qualified", "monetary_authority", "native_authority"] {
      XCTAssertEqual(root[flag] as? Bool, false)
    }
    let vectors = try XCTUnwrap(root["vectors"] as? [[String: Any]])
    XCTAssertEqual(vectors.count, 10)
    var operationCounts: [Int: Int] = [:]
    for row in vectors {
      let subject = try hex(try XCTUnwrap(row["subject_signing_hex"] as? String))
      let wrapper = try hex(try XCTUnwrap(row["approval_signing_hex"] as? String))
      let p = try KagemushaAppApprovalSigningProjectionV1(nativeSigningBytes: wrapper,
        nativeFinancialSubject: subject)
      let tag = try XCTUnwrap(row["operation_tag"] as? Int)
      operationCounts[tag, default: 0] += 1
      XCTAssertEqual(Int(subject[331]), tag)
      XCTAssertEqual(p.accountBinding, try hex(try XCTUnwrap(root["account_binding_hex"] as? String)))
      XCTAssertEqual(p.subjectSigningDigest, try hex(try XCTUnwrap(row["subject_sha256_hex"] as? String)))
      XCTAssertEqual(Data(SHA256.hash(data: subject)), p.subjectSigningDigest)
      XCTAssertEqual(p.clientDataHash, try hex(try XCTUnwrap(row["approval_sha256_hex"] as? String)))
      XCTAssertEqual(p.canonicalSigningBytes, wrapper)
      let before = try XCTUnwrap(row["secure_index_before"] as? String)
      let after = try XCTUnwrap(row["secure_index_after"] as? String)
      if before == "9" {
        XCTAssertEqual(Data(subject[428..<444]), Data([9]) + Data(repeating: 0, count: 15))
        XCTAssertEqual(after, "10")
      } else {
        XCTAssertEqual(before, "340282366920938463463374607431768211454")
        XCTAssertEqual(Data(subject[428..<444]), Data([0xfe]) + Data(repeating: 0xff, count: 15))
        XCTAssertEqual(after, "340282366920938463463374607431768211455")
        XCTAssertEqual(Data(subject[444..<460]), Data(repeating: 0xff, count: 16))
      }
      let archive = try hex(try XCTUnwrap(row["challenge_archive_hex"] as? String))
      XCTAssertThrowsError(try KagemushaAppApprovalSigningProjectionV1(nativeSigningBytes: archive,
        nativeFinancialSubject: subject))
    }
    XCTAssertEqual(operationCounts, [1: 2, 2: 2, 3: 2, 4: 2, 5: 2])
  }

  private func hex(_ text: String) throws -> Data {
    let chars = Array(text.utf8)
    guard chars.count % 2 == 0 else { throw Failure.invalidFixture }
    var result = Data()
    for offset in stride(from: 0, to: chars.count, by: 2) {
      guard let byte = UInt8(String(decoding: chars[offset..<(offset + 2)], as: UTF8.self), radix: 16)
      else { throw Failure.invalidFixture }
      result.append(byte)
    }
    return result
  }

  func testWRejectsSubstitutedPurposeSubjectAndEveryMalformedBound() throws {
    let base = try projection().canonicalSigningBytes
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for offset in [0, start - 8, start, start + 1, start + 2, start + 3 + 6 * 32] {
      var changed = base; changed[offset] ^= 2
      XCTAssertThrowsError(try projection(changed), "offset \(offset)")
    }
    XCTAssertThrowsError(try projection(Data(base.dropLast())))
    XCTAssertThrowsError(try projection(base + Data([0])))
    for index in 0..<8 {
      var changed = base
      changed.replaceSubrange((start + 3 + index * 32)..<(start + 3 + (index + 1) * 32),
        with: Data(repeating: 0, count: 32))
      XCTAssertThrowsError(try projection(changed), "missing field \(index)")
    }
    for (issued, expires) in [(UInt64(0), UInt64(120_000)), (1_000, 1_000),
      (1_000, 121_001), (UInt64.max, UInt64.max)] {
      var changed = base
      changed.replaceSubrange((start + 259)..<(start + 267), with: littleEndian(issued))
      changed.replaceSubrange((start + 267)..<(start + 275), with: littleEndian(expires))
      XCTAssertThrowsError(try projection(changed))
    }
    let values = try vectors()
    var subject = try XCTUnwrap(values["s_mint_native_first"])
    subject[59] ^= 1
    XCTAssertThrowsError(try KagemushaAppApprovalSigningProjectionV1(
      nativeSigningBytes: base, nativeFinancialSubject: subject))
    XCTAssertThrowsError(try projection(try XCTUnwrap(values["e_enrollment_marker11"])))
  }

  func testAppAttestHashesExactWOnceAndSeparatesNonce() throws {
    let first = try projection()
    XCTAssertNotEqual(first.clientDataHash, Data(SHA256.hash(data: first.canonicalFinancialSubject)))
    XCTAssertNotEqual(first.clientDataHash, Data(SHA256.hash(data: first.clientDataHash)))
    var changed = first.canonicalSigningBytes
    changed[KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8 + 3 + 32] ^= 1
    XCTAssertNotEqual(try projection(changed).clientDataHash, first.clientDataHash)
  }

  private let appID = Data(SHA256.hash(data: Data("TESTTEAM01.example.fixture".utf8)))
  private func release() throws -> KagemushaAppAttestExpectedReleaseV1 {
    try KagemushaAppAttestExpectedReleaseV1(validationCategory: 3, bundleVersion: "1",
      authenticatedAppReleaseDigest: KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
        validationCategory: 3, bundleVersion: "1"))
  }
  private func key(_ marker: UInt8 = 1) throws -> P256.Signing.PrivateKey {
    try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: marker, count: 32))
  }
  private func keyedProjection(_ key: P256.Signing.PrivateKey) throws -> KagemushaAppApprovalSigningProjectionV1 {
    var wrapper = try projection().canonicalSigningBytes
    let offset = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8 + 3 + 4 * 32
    wrapper.replaceSubrange(offset..<(offset + 32),
      with: Data(SHA256.hash(data: key.publicKey.x963Representation)))
    return try projection(wrapper)
  }
  private func original(_ raw: Data, projection: KagemushaAppApprovalSigningProjectionV1,
    publicKey: Data, floor: UInt32 = 1) throws -> KagemushaAppAttestApprovalOriginalV1 {
    try KagemushaAppAttestApprovalOriginalV1(rawAssertion: raw, nativeProjection: projection,
      enrolledKeyID: Data(SHA256.hash(data: publicKey)), enrolledPublicKeyX963: publicKey,
      expectedAppIDHash: appID, expectedRelease: release(), nativeCounterFloor: floor)
  }

  func testAppleWEquationAcceptsOriginalAndRejectsSOnlyDoubleHashAndWrongKey() throws {
    let signer = try key(), value = try keyedProjection(signer)
    let raw = try assertion(key: signer, clientHash: value.clientDataHash)
    let checked = try original(raw, projection: value, publicKey: signer.publicKey.x963Representation)
    XCTAssertEqual(checked.rawAssertion, raw)
    XCTAssertEqual(checked.observedCounter, 3) // Online identity counter gaps are allowed.
    XCTAssertEqual(checked.clientDataHash, value.clientDataHash)
    for hash in [Data(SHA256.hash(data: value.canonicalFinancialSubject)),
      Data(SHA256.hash(data: value.clientDataHash))] {
      XCTAssertThrowsError(try original(assertion(key: signer, clientHash: hash),
        projection: value, publicKey: signer.publicKey.x963Representation))
    }
    var substituted = value.canonicalSigningBytes
    substituted[KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8 + 3 + 32] ^= 1
    XCTAssertThrowsError(try original(raw, projection: projection(substituted),
      publicKey: signer.publicKey.x963Representation))
    let other = try key(2)
    XCTAssertThrowsError(try original(raw, projection: keyedProjection(other),
      publicKey: other.publicKey.x963Representation))
  }

  func testAppleWRejectsReplayedCounterDirectDERAndUnsupportedAuthenticator() throws {
    let signer = try key(), value = try keyedProjection(signer)
    let publicKey = signer.publicKey.x963Representation
    let raw = try assertion(key: signer, clientHash: value.clientDataHash)
    XCTAssertThrowsError(try original(raw, projection: value, publicKey: publicKey, floor: 3))
    XCTAssertThrowsError(try original(raw, projection: value, publicKey: publicKey, floor: UInt32.max))
    let directAndroidDER = try signer.signature(for: value.canonicalSigningBytes).derRepresentation
    XCTAssertThrowsError(try original(directAndroidDER, projection: value, publicKey: publicKey))
    XCTAssertThrowsError(try original(assertion(key: signer, clientHash: value.clientDataHash, flags: 0),
      projection: value, publicKey: publicKey))
    XCTAssertThrowsError(try original(assertion(key: signer, clientHash: value.clientDataHash,
      relyingPartyHash: Data(repeating: 0x99, count: 32)), projection: value, publicKey: publicKey))
    XCTAssertThrowsError(try original(Data(repeating: 0xa2, count: 4_097),
      projection: value, publicKey: publicKey))
  }

  private func littleEndian(_ value: UInt64) -> Data {
    var value = value.littleEndian
    return withUnsafeBytes(of: &value) { Data($0) }
  }
  private func cborLength(_ count: Int, major: UInt8) -> Data {
    if count < 24 { return Data([major << 5 | UInt8(count)]) }
    if count <= 255 { return Data([major << 5 | 24, UInt8(count)]) }
    return Data([major << 5 | 25, UInt8(count >> 8), UInt8(count & 0xff)])
  }
  private func cborBytes(_ value: Data) -> Data { cborLength(value.count, major: 2) + value }
  private func cborText(_ value: String) -> Data {
    let bytes = Data(value.utf8)
    return cborLength(bytes.count, major: 3) + bytes
  }
  private func assertion(key: P256.Signing.PrivateKey, clientHash: Data,
    flags: UInt8 = 0x40, relyingPartyHash: Data? = nil) throws -> Data {
    let auth = (relyingPartyHash ?? appID) + Data([flags, 0, 0, 0, 3])
    let nonce = Data(SHA256.hash(data: auth + clientHash))
    let signature = try key.signature(for: nonce).derRepresentation
    return Data([0xa2]) + cborText("authenticatorData") + cborBytes(auth)
      + cborText("signature") + cborBytes(signature)
  }
}
