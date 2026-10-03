import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Codec specimens only. No parsed bytes construct the opaque Native incoming holder.
final class KagemushaOrdinaryIncomingSigningProjectionV1Tests: XCTestCase {
  func testBothIncomingPurposesRetainExactPublicOriginalsAndIndependentCounter() throws {
    for operation in ["mint_fold", "receive_fold"] { for terminal in [false, true] {
      let fields = try Self.specimen(operation: operation, terminal: terminal)
      let projection = try KagemushaOrdinaryIncomingSigningProjectionV1(fields)
      XCTAssertEqual(projection.fields, fields)
      XCTAssertEqual(projection.terminal, terminal)
      XCTAssertEqual(projection.counterFloor, 7)
      XCTAssertEqual(projection.approval.subjectSigningDigest, Data(SHA256.hash(data: fields[2])))
      XCTAssertEqual(projection.approval.clientDataHash, Data(SHA256.hash(data: fields[1])))
      XCTAssertEqual(projection.appID, fields[9])
      XCTAssertEqual(projection.keyID, fields[7])
      let frame = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.originalPlatformSigning,
        handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.originalPlatformSigning,
        handle: 19, response: frame), fields)
      XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.originalPlatformCounter,
        handle: 19, response: frame))
    } }
  }

  func testCorruptedKeyOperationSubjectCredentialAndCounterRefuse() throws {
    let original = try Self.specimen()
    for slot in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
      var changed = original; changed[slot] = Data()
      XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(changed), "empty field \(slot)")
    }
    for slot in [0, 6, 7, 8] {
      var changed = original; changed[slot][0] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(changed), "substituted field \(slot)")
    }
    var changed = original; changed[1][245] ^= 1
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(changed))
    changed = original; changed[5] = Data("different-key".utf8)
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(changed))
    changed = original; changed[10] = Data(repeating: 0, count: 16)
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(changed))
    changed = original; changed[9] = Data(repeating: 0, count: 32)
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(changed))
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(Array(original.dropLast())))
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(original + [Data()]))
  }

  func testDataProjectionCannotAssertAppIDAuthorityAndRetainsOwnedCopies() throws {
    var fields = try Self.specimen()
    // The parser accepts different well-shaped App ID DATA. Actual Native credential custody
    // and the opaque holder, which has no byte initializer, must authenticate that value.
    fields[9] = Data(repeating: 0xa5, count: 32)
    let original = fields
    let projection = try KagemushaOrdinaryIncomingSigningProjectionV1(fields.map { (Data([0xff]) + $0).dropFirst() })
    for index in fields.indices { fields[index].resetBytes(in: 0..<fields[index].count) }
    var returned = projection.fields; returned[9][0] ^= 1
    XCTAssertEqual(projection.fields, original)
    XCTAssertEqual(projection.appID, original[9])
    var android = original; android[4] = Data([5]); android[5] = Data("ordinary-key".utf8); android[10] = Data()
    XCTAssertNil(try KagemushaOrdinaryIncomingSigningProjectionV1(android).counterFloor)
    android[10] = Data(repeating: 0, count: 4)
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingSigningProjectionV1(android))
  }

  func testActualP256EquationUsesSingleWHashExactAppIDAndUnexhaustedCounter() throws {
    let key = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 3, count: 32))
    let release = try KagemushaAppAttestExpectedReleaseV1(validationCategory: 3, bundleVersion: "1",
      authenticatedAppReleaseDigest: KagemushaAppAttestExpectedReleaseV1.canonicalReleaseDigest(
        validationCategory: 3, bundleVersion: "1"))
    for terminal in [false, true] {
      let p = try KagemushaOrdinaryIncomingSigningProjectionV1(Self.specimen(terminal: terminal))
      func assertion(hash: Data, app: Data, count: UInt32 = 8) throws -> Data {
        var counter = count.bigEndian
        let auth = app + Data([0x40]) + withUnsafeBytes(of: &counter) { Data($0) }
        let nonce = Data(SHA256.hash(data: auth + hash))
        func bytes(_ value: Data) -> Data { Data([0x58, UInt8(value.count)]) + value }
        func text(_ value: String) -> Data { Data([0x60 | UInt8(value.utf8.count)]) + Data(value.utf8) }
        return Data([0xa2]) + text("signature") + bytes(try key.signature(for: nonce).derRepresentation)
          + text("authenticatorData") + bytes(auth)
      }
      let raw = try assertion(hash: p.approval.clientDataHash, app: p.appID)
      let evidence = try KagemushaIncomingAppAttestOriginalV1(raw: raw, projection: p, expectedRelease: release)
      XCTAssertEqual(evidence.rawAssertion, raw); XCTAssertEqual(evidence.observedCounter, 8)
      XCTAssertEqual(evidence.clientDataHash, Data(SHA256.hash(data: p.fields[1])))
      for wrongHash in [Data(SHA256.hash(data: p.approval.clientDataHash)), Data(SHA256.hash(data: p.fields[2]))] {
        XCTAssertThrowsError(try KagemushaIncomingAppAttestOriginalV1(
          raw: assertion(hash: wrongHash, app: p.appID), projection: p, expectedRelease: release))
      }
      for count: UInt32 in [0, 7] {
        XCTAssertThrowsError(try KagemushaIncomingAppAttestOriginalV1(
          raw: assertion(hash: p.approval.clientDataHash, app: p.appID, count: count), projection: p, expectedRelease: release))
      }
      XCTAssertThrowsError(try KagemushaIncomingAppAttestOriginalV1(
        raw: assertion(hash: p.approval.clientDataHash, app: Data(repeating: 8, count: 32)), projection: p, expectedRelease: release))
      var exhausted = p.fields; exhausted[10] = Data(repeating: 0xff, count: 4)
      XCTAssertThrowsError(try KagemushaIncomingAppAttestOriginalV1(raw: raw,
        projection: KagemushaOrdinaryIncomingSigningProjectionV1(exhausted), expectedRelease: release))
      XCTAssertThrowsError(try KagemushaIncomingAppAttestOriginalV1(
        raw: key.signature(for: p.fields[1]).derRepresentation, projection: p, expectedRelease: release))
    }
  }

  static func specimen(operation: String = "mint_fold", terminal: Bool = false) throws -> [Data] {
    let vectors = try fixtures()
    var w = try XCTUnwrap(vectors["w_\(operation)_9"]), s = try XCTUnwrap(vectors["s_\(operation)_9"])
    // Explicitly inert grammar specimen derived from the maintained Rust codec vectors.
    // No issuer, finality, State, Guard or physical-key evidence is asserted.
    let key = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 3, count: 32))
    let point = key.publicKey.x963Representation, id = Data(SHA256.hash(data: point))
    w.replaceSubrange(181..<213, with: id)
    s.replaceSubrange(155..<187, with: w[213..<245])
    w[52] = terminal ? 1 : 2
    s.replaceSubrange(364..<396, with: Data(repeating: terminal ? 12 : 0, count: 32))
    s.replaceSubrange(396..<428, with: Data(repeating: terminal ? 13 : 0, count: 32))
    w.replaceSubrange(245..<277, with: Data(SHA256.hash(data: s)))
    var issued = UInt64(1).littleEndian, expires = UInt64(10_001).littleEndian
    w.replaceSubrange(309..<317, with: withUnsafeBytes(of: &issued) { Data($0) })
    w.replaceSubrange(317..<325, with: withUnsafeBytes(of: &expires) { Data($0) })
    return [Data(w[53..<85]), w, s, Data([9]), Data([4]), Data(id.base64EncodedString().utf8),
      point, id, Data(w[213..<245]), Data(repeating: 4, count: 32), Data([7, 0, 0, 0])]
  }
  private static func fixtures() throws -> [String: Data] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_app_platform_messages_v1.tsv")
      if FileManager.default.fileExists(atPath: path.path) {
        var result: [String: Data] = [:]
        for line in try String(contentsOf: path, encoding: .utf8).split(separator: "\n") where !line.hasPrefix("#") {
          let columns = line.split(separator: "\t", omittingEmptySubsequences: false)
          guard columns.count == 2, columns[1].count % 2 == 0 else { throw Failure.fixture }
          let chars = Array(columns[1].utf8); var bytes = Data()
          for offset in stride(from: 0, to: chars.count, by: 2) {
            guard let byte = UInt8(String(decoding: chars[offset..<(offset + 2)], as: UTF8.self), radix: 16) else { throw Failure.fixture }
            bytes.append(byte)
          }
          guard result.updateValue(bytes, forKey: String(columns[0])) == nil else { throw Failure.fixture }
        }
        return result
      }
      directory.deleteLastPathComponent()
    }
    throw Failure.fixture
  }
  private enum Failure: Error { case fixture }
}
