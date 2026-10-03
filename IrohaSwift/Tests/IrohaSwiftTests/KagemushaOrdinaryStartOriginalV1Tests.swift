// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Pure bounded DATA only. These bytes never construct E or attest a device.
final class KagemushaOrdinaryStartOriginalV1Tests: XCTestCase {
  func testCompleteOriginalChunksPreserveBytesAndNativeDomainDigest() throws {
    let c = try fixtureC(), credential = Data([3, 4])
    let scope = Data(repeating: 5, count: 32), domainDigest = Data(repeating: 6, count: 32)
    XCTAssertNotEqual(domainDigest, Data(SHA256.hash(data: credential)))
    let body = try OrdinaryEnrollmentWire.json(fields(c, credential, platformSize: 100000))
    let original = chunks(body, scope, domainDigest)
    XCTAssertEqual(original.count, 3)
    XCTAssertEqual(try decode(original, c, credential, scope, domainDigest), body)
    XCTAssertThrowsError(try decode(Array(original.dropLast()), c, credential, scope, domainDigest))
    for field in 0..<6 {
      var changed = original
      changed[1][field][0] ^= 1
      XCTAssertThrowsError(try decode(changed, c, credential, scope, domainDigest), "field \(field)")
    }
    XCTAssertThrowsError(try decode(original, c, credential, scope, Data(SHA256.hash(data: credential))))
    XCTAssertThrowsError(try decode([], c, credential, scope, domainDigest))
  }

  func testEveryFieldRequiredInitialIntegrityNullAndOldCarrierRefused() throws {
    let c = try fixtureC(), credential = Data([3, 4])
    let complete = fields(c, credential)
    XCTAssertEqual(complete.count, 7)
    let body = try OrdinaryEnrollmentWire.json(complete)
    XCTAssertEqual(try OrdinaryEnrollmentWire.requireRetailStartOriginal(body,
      signedPreparation: c, credential: credential), body)
    for key in complete.keys {
      var incomplete = complete; incomplete.removeValue(forKey: key)
      XCTAssertThrowsError(try require(incomplete, c, credential), key)
    }
    var extra = complete; extra["ready"] = true
    XCTAssertThrowsError(try require(extra, c, credential))
    var selected = complete
    selected["selected_integrity"] = ["challenge": "AA==", "lease": "AA=="]
    XCTAssertThrowsError(try require(selected, c, credential))
    selected["selected_integrity"] = false
    XCTAssertThrowsError(try require(selected, c, credential))
    XCTAssertThrowsError(try require(["signed_preparation_base64": c.base64EncodedString(),
      "app_certificate_base64": credential.base64EncodedString()], c, credential))
    XCTAssertThrowsError(try require(complete, c, Data([3, 5])))
  }

  func testPerOriginalBoundsCanonicalBase64AndExactPreparation() throws {
    let c = try fixtureC(), credential = Data([3, 4])
    let complete = fields(c, credential, platformSize: 131072)
    _ = try require(complete, c, credential)
    for (key, size) in [("raw_admission_original_base64", 315),
      ("platform_original_base64", 131073), ("core_possession_original_base64", 5121),
      ("app_certificate_base64", 16385)] {
      var oversized = complete; oversized[key] = Data(repeating: 1, count: size).base64EncodedString()
      XCTAssertThrowsError(try require(oversized, c, credential), key)
    }
    var shortRaw = complete
    shortRaw["raw_admission_original_base64"] = Data(repeating: 2, count: 313).base64EncodedString()
    XCTAssertThrowsError(try require(shortRaw, c, credential))
    for key in ["platform_original_base64", "core_possession_original_base64"] {
      var empty = complete; empty[key] = ""
      XCTAssertThrowsError(try require(empty, c, credential))
    }
    var noncanonical = complete
    noncanonical["signed_preparation_base64"] = c.base64EncodedString() + "\n"
    XCTAssertThrowsError(try require(noncanonical, c, credential))
    var foreignC = c; foreignC[foreignC.count - 1] ^= 1
    noncanonical["signed_preparation_base64"] = foreignC.base64EncodedString()
    XCTAssertThrowsError(try require(noncanonical, c, credential))
    var wallet = complete; wallet["wallet"] = "inert setup DATA"
    XCTAssertThrowsError(try require(wallet, c, credential))
    wallet["wallet"] = String(repeating: "a", count: 4097)
    XCTAssertThrowsError(try require(wallet, c, credential))
  }

  func testDuplicateKeyAndInvalidUTF8RefuseWithoutCreatingOriginals() throws {
    let c = try fixtureC(), credential = Data([3, 4])
    let body = try OrdinaryEnrollmentWire.json(fields(c, credential))
    let duplicate = Data("{\"wallet\":\"substituted\",".utf8) + body.dropFirst()
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.requireRetailStartOriginal(duplicate,
      signedPreparation: c, credential: credential))
    var invalidUTF8 = body; invalidUTF8.append(0xff)
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.requireRetailStartOriginal(invalidUTF8,
      signedPreparation: c, credential: credential))
  }

  func testFullBodyLimitFourChunksAndOneExtraByteRefuse() throws {
    let c = try fixtureC(), credential = Data([3, 4])
    let scope = Data(repeating: 5, count: 32), digest = Data(repeating: 6, count: 32)
    var exact = try OrdinaryEnrollmentWire.json(fields(c, credential))
    exact.append(Data(repeating: 32, count: 262144 - exact.count))
    let full = chunks(exact, scope, digest)
    XCTAssertEqual(full.count, 4)
    XCTAssertEqual(try decode(full, c, credential, scope, digest), exact)
    var extra = exact; extra.append(32)
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.requireRetailStartOriginal(extra,
      signedPreparation: c, credential: credential))
    XCTAssertThrowsError(try decode(chunks(extra, scope, digest), c, credential, scope, digest))
    var wrongLength = full; wrongLength[3][1].removeLast()
    XCTAssertThrowsError(try decode(wrongLength, c, credential, scope, digest))
  }

  func testEnrollmentChunkGrammarDoesNotChangeCashPreparationOrPermitForeignTicket() throws {
    let phase = KagemushaCoreCoordinatorFrameV1.u32(15), ticket = Data(repeating: 1, count: 8)
    try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
      [phase, ticket, KagemushaCoreCoordinatorFrameV1.u32(3)])
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
      [phase, ticket, KagemushaCoreCoordinatorFrameV1.u32(4)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
      [phase, Data(repeating: 0, count: 8), KagemushaCoreCoordinatorFrameV1.u32(0)]))
    try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [phase, KagemushaCoreCoordinatorFrameV1.u32(2), Data([1])])
    var amount = Data(repeating: 0, count: 16); amount[0] = 1
    try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [phase, KagemushaCoreCoordinatorFrameV1.u32(4), amount])
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [phase, ticket, KagemushaCoreCoordinatorFrameV1.u32(0)]))
  }

  private func fields(_ c: Data, _ credential: Data, platformSize: Int = 8) -> [String: Any] {
    ["wallet": "inert-setup-data-only", "signed_preparation_base64": c.base64EncodedString(),
      "raw_admission_original_base64": Data(repeating: 2, count: 314).base64EncodedString(),
      "platform_original_base64": Data(repeating: 7, count: platformSize).base64EncodedString(),
      "core_possession_original_base64": Data([8]).base64EncodedString(),
      "app_certificate_base64": credential.base64EncodedString(), "selected_integrity": NSNull()]
  }
  private func require(_ fields: [String: Any], _ c: Data, _ credential: Data) throws -> Data {
    try OrdinaryEnrollmentWire.requireRetailStartOriginal(OrdinaryEnrollmentWire.json(fields),
      signedPreparation: c, credential: credential)
  }
  private func chunks(_ body: Data, _ scope: Data, _ digest: Data) -> [[Data]] {
    stride(from: 0, to: body.count, by: 65536).enumerated().map { index, offset in
      [KagemushaCoreCoordinatorFrameV1.u32(UInt32(index)), Data(body[offset..<min(offset + 65536, body.count)]),
        Data(SHA256.hash(data: body)), KagemushaCoreCoordinatorFrameV1.u32(UInt32(body.count)), scope, digest]
    }
  }
  private func decode(_ chunks: [[Data]], _ c: Data, _ credential: Data, _ scope: Data, _ digest: Data) throws -> Data {
    try OrdinaryEnrollmentWire.retailStartOriginalChunks(chunks, signedPreparation: c, credential: credential,
      pendingScope: scope, credentialDigest: digest, nativeTicket: Data(repeating: 1, count: 8))
  }
  private func fixtureC() throws -> Data {
    var root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    let suffix = "fixtures/offline/kagemusha_ordinary_app_enrollment_v1.json"
    while !FileManager.default.fileExists(atPath: root.appendingPathComponent(suffix).path) {
      guard root.path != "/" else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("missing retained DATA fixture") }
      root.deleteLastPathComponent()
    }
    let json = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent(suffix)))
    let object = try XCTUnwrap(json as? [String: Any])
    let vectors = try XCTUnwrap(object["vectors"] as? [[String: Any]])
    let encoded = try XCTUnwrap(vectors.first?["signed_preparation_base64"] as? String)
    return try XCTUnwrap(Data(base64Encoded: encoded))
  }
}
