import Foundation
import XCTest
@testable import IrohaSwift

/// Synthetic public framing only; these tests grant no proof or hardware authority.
func testIncomingCanonicalPairedProof(proofBytes: Int = 1) throws -> Data {
  func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }
  return try KagemushaNoritoV1.encodePairedProofShape(KagemushaPairedProofV1(
    eqProtocolDigest: digest(1), epProtocolDigest: digest(2), semanticDigest: digest(3),
    guardEqCredentialAudit: digest(4), guardEpCredentialAudit: digest(5),
    eqDeferredAudit: digest(6), epDeferredAudit: digest(7),
    eqProof: Data(repeating: 1, count: proofBytes), epProof: Data(repeating: 2, count: proofBytes),
    eqHistory: Data(repeating: 3, count: KagemushaWireV1.historyAccumulatorBytes),
    epHistory: Data(repeating: 4, count: KagemushaWireV1.historyAccumulatorBytes)))
}

final class KagemushaNativeIncomingFoldV1Tests: XCTestCase {
  private let credit = Data(repeating: 0x41, count: 32)
  private let history = Data(repeating: 0x42, count: 32)
  private func fields() throws -> [Data] {
    [history, credit, Data([3]), Data(repeating: 4, count: 32), Data(repeating: 5, count: 32),
      Data([6]), Data(repeating: 7, count: 32), KagemushaUInt128V1(8).littleEndianBytes,
      Data(repeating: 9, count: 32), try testIncomingCanonicalPairedProof()]
  }
  private var signature: Data {
    var value = Data(repeating: 0, count: 64); value[31] = 1; value[63] = 1; return value
  }
  private func request() throws -> Data {
    try KagemushaCoreCoordinatorFrameV1.encodeRequest(.prepareIncomingFold,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(1), credit])
  }

  func testExactWorkPreservesNativePairAndPublicSelectionWithDefensiveOwnership() throws {
    let selector = try KagemushaPendingCreditSelectorV1(kind: .receive, creditID: credit)
    var original = try fields()
    let work = try KagemushaNativeIncomingFoldWorkV1(selector: selector, nativeFields: original)
    original[9][0] = 99
    var copied = work.nativePairedProof; copied[0] = 98
    XCTAssertEqual(work.kind, .receive); XCTAssertEqual(work.creditID, credit)
    XCTAssertEqual(work.historyID, history); XCTAssertEqual(work.nativePairedProof, try testIncomingCanonicalPairedProof())
    XCTAssertEqual(work.hardwareEpochGeneration, .init(8))
  }

  func testWorkRejectsEveryMissingFieldAndCreditSubstitution() throws {
    let selector = try KagemushaPendingCreditSelectorV1(kind: .mint, creditID: credit)
    for i in try fields().indices {
      var missing = try fields(); missing.remove(at: i)
      XCTAssertThrowsError(try KagemushaNativeIncomingFoldWorkV1(selector: selector, nativeFields: missing))
    }
    var changed = try fields(); changed[1] = Data(repeating: 0x43, count: 32)
    XCTAssertThrowsError(try KagemushaNativeIncomingFoldWorkV1(selector: selector, nativeFields: changed))
  }

  func testAllPublicDigestAndEpochFieldsAreExactNonzeroWidths() throws {
    let request = try request()
    for index in [0, 1, 3, 4, 6, 8] {
      for bad in [Data(), Data(repeating: 0, count: 32), Data(repeating: 1, count: 31), Data(repeating: 1, count: 33)] {
        var changed = try fields(); changed[index] = bad
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
          requestFrame: request, fields: changed))
      }
    }
    for bad in [Data(repeating: 0, count: 16), Data(repeating: 1, count: 15), Data(repeating: 1, count: 17)] {
      var changed = try fields(); changed[7] = bad
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: request, fields: changed))
    }
  }

  func testNativePublicArchiveBoundsAdmitExactLimitAndRejectNextByte() throws {
    let request = try request()
    for (index, maximum) in [(2, 8192), (5, 32768)] {
      var changed = try fields(); changed[index] = Data(repeating: 1, count: maximum)
      XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: request, fields: changed))
      changed[index].append(1)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: request, fields: changed))
      changed[index] = Data()
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: request, fields: changed))
    }
  }

  func testIncomingMethodsHaveClosedSelectorsAndExactReturnedIdentity() throws {
    XCTAssertEqual(KagemushaCoreCoordinatorMethodV1.prepareIncomingFold.rawValue, 15)
    XCTAssertEqual(KagemushaCoreCoordinatorMethodV1.completeIncomingFold.rawValue, 16)
    XCTAssertEqual(KagemushaCoreCoordinatorMethodV1.stageIncomingOriginal.rawValue, 17)
    for kind: UInt32 in 0...2 {
      let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.stageIncomingOriginal,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(kind), credit])
      XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.stageIncomingOriginal,
        requestFrame: request, fields: [credit]))
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.stageIncomingOriginal,
        requestFrame: request, fields: [history]))
    }
    for method in [KagemushaCoreCoordinatorMethodV1.prepareIncomingFold, .stageIncomingOriginal] {
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
        fields: [KagemushaCoreCoordinatorFrameV1.u32(3), credit]))
    }
  }

  func testOriginalCertificateAndDeviceRootSignatureAreBoundedUntrustedBytes() throws {
    var certificate = Data([1])
    let evidence = try KagemushaOriginalIncomingFoldEvidenceV1(canonicalHardwareCertificate: certificate,
      deviceRootSelectionSignature: signature)
    certificate[0] = 2
    XCTAssertEqual(evidence.canonicalHardwareCertificate, Data([1]))
    for bad in [Data(), Data(repeating: 1, count: 96 * 1024 + 1)] {
      XCTAssertThrowsError(try KagemushaOriginalIncomingFoldEvidenceV1(canonicalHardwareCertificate: bad,
        deviceRootSelectionSignature: signature))
    }
    for bad in [Data(), Data(repeating: 0, count: 64), Data(repeating: 1, count: 63)] {
      XCTAssertThrowsError(try KagemushaOriginalIncomingFoldEvidenceV1(canonicalHardwareCertificate: Data([1]),
        deviceRootSelectionSignature: bad))
    }
  }

  func testCompletionRequiresRetainedPairCertificateAndExactHistoryResponse() throws {
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.completeIncomingFold,
      fields: [history, try testIncomingCanonicalPairedProof(), Data([2]), signature])
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.completeIncomingFold,
      requestFrame: request, fields: [history]))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.completeIncomingFold,
      requestFrame: request, fields: [credit]))
    for (index, bad) in [(1, Data()), (1, Data(repeating: 1, count: KagemushaWireV1.maximumPairedProofBytes + 1)),
      (2, Data()), (2, Data(repeating: 1, count: 96 * 1024 + 1)), (3, Data(repeating: 0, count: 64))] {
      var changed = [history, try testIncomingCanonicalPairedProof(), Data([2]), signature]; changed[index] = bad
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.completeIncomingFold, fields: changed))
    }
  }

  func testIncomingPairRejectsOpaqueTextNoncanonicalTailAndOversizedArchive() throws {
    let canonical = try testIncomingCanonicalPairedProof()
    var wrongSchema = canonical; wrongSchema[12] ^= 1
    let malformed = [Data(), Data(repeating: 0x78, count: 27), canonical + Data([0]),
      wrongSchema, Data(repeating: 1, count: KagemushaWireV1.maximumPairedProofBytes + 1)]
    let originalRequest = try request()
    for bytes in malformed {
      var changed = try fields(); changed[9] = bytes
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: originalRequest, fields: changed))
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.completeIncomingFold,
        fields: [history, bytes, Data([2]), signature]))
    }
  }

  func testMaximumModelProofSizesRemainCanonicalWithinIncomingBound() throws {
    let pair = try testIncomingCanonicalPairedProof(proofBytes: KagemushaWireV1.maximumParityProofBytes)
    XCTAssertLessThanOrEqual(pair.count, KagemushaWireV1.maximumPairedProofBytes)
    let decoded = try KagemushaNoritoV1.decodePairedProofShapeExact(pair)
    XCTAssertEqual(decoded.eqProof.count, KagemushaWireV1.maximumParityProofBytes)
    XCTAssertEqual(try KagemushaNoritoV1.encodePairedProofShape(decoded), pair)
    var original = try fields(); original[9] = pair
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
      requestFrame: request(), fields: original))
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.completeIncomingFold,
      fields: [history, pair, Data([2]), signature]))
    // Shape acceptance never verifies these deliberately public synthetic proofs.
  }

  func testZeroHistorySidesRejectConstructionAndChecksumValidCanonicalDecode() throws {
    let canonical = try testIncomingCanonicalPairedProof()
    let proof = try KagemushaNoritoV1.decodePairedProofShapeExact(canonical)
    let frame = try XCTUnwrap(noritoDecodeFrame(canonical))
    var reader = NoritoFieldFrames.Reader(frame.payload)
    var original: [Data] = []
    for _ in 0..<12 { original.append(try reader.field()) }
    try reader.finish()
    let zero = Data(repeating: 0, count: KagemushaWireV1.historyAccumulatorBytes)
    let originalRequest = try request()
    for index in [10, 11] {
      XCTAssertThrowsError(try KagemushaPairedProofV1(
        version: proof.version, eqProtocolDigest: proof.eqProtocolDigest,
        epProtocolDigest: proof.epProtocolDigest, semanticDigest: proof.semanticDigest,
        guardEqCredentialAudit: proof.guardEqCredentialAudit,
        guardEpCredentialAudit: proof.guardEpCredentialAudit,
        eqDeferredAudit: proof.eqDeferredAudit, epDeferredAudit: proof.epDeferredAudit,
        eqProof: proof.eqProof, epProof: proof.epProof,
        eqHistory: index == 10 ? zero : proof.eqHistory,
        epHistory: index == 11 ? zero : proof.epHistory))
      // Keep the canonical vector length, layout and checksum valid so the decoder
      // reaches the model's zero-history refusal instead of a framing failure.
      var changed = original
      XCTAssertEqual(changed[index].count, 8 + zero.count)
      changed[index].replaceSubrange(8..<changed[index].count, with: zero)
      let invalid = noritoEncode(typeName: "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPairedProofV1",
        payload: NoritoFieldFrames.record(changed), flags: NoritoHeader.compactLen,
        payloadAlignment: 8)
      XCTAssertNotNil(noritoDecodeFrame(invalid))
      XCTAssertThrowsError(try KagemushaNoritoV1.decodePairedProofShapeExact(invalid))
      var response = try fields(); response[9] = invalid
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: originalRequest, fields: response))
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.completeIncomingFold,
        fields: [history, invalid, Data([2]), signature]))
    }
  }
}
