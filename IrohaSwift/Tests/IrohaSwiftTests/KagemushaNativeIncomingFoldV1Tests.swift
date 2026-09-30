import Foundation
import XCTest
@testable import IrohaSwift

/// Synthetic public framing only; these tests grant no proof or hardware authority.
final class KagemushaNativeIncomingFoldV1Tests: XCTestCase {
  private let credit = Data(repeating: 0x41, count: 32)
  private let history = Data(repeating: 0x42, count: 32)
  private func fields() -> [Data] {
    [history, credit, Data([3]), Data(repeating: 4, count: 32), Data(repeating: 5, count: 32),
      Data([6]), Data(repeating: 7, count: 32), KagemushaUInt128V1(8).littleEndianBytes,
      Data(repeating: 9, count: 32), Data([10])]
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
    var original = fields()
    let work = try KagemushaNativeIncomingFoldWorkV1(selector: selector, nativeFields: original)
    original[9][0] = 99
    var copied = work.nativePairedProof; copied[0] = 98
    XCTAssertEqual(work.kind, .receive); XCTAssertEqual(work.creditID, credit)
    XCTAssertEqual(work.historyID, history); XCTAssertEqual(work.nativePairedProof, Data([10]))
    XCTAssertEqual(work.hardwareEpochGeneration, .init(8))
  }

  func testWorkRejectsEveryMissingFieldAndCreditSubstitution() throws {
    let selector = try KagemushaPendingCreditSelectorV1(kind: .mint, creditID: credit)
    for i in fields().indices {
      var missing = fields(); missing.remove(at: i)
      XCTAssertThrowsError(try KagemushaNativeIncomingFoldWorkV1(selector: selector, nativeFields: missing))
    }
    var changed = fields(); changed[1] = Data(repeating: 0x43, count: 32)
    XCTAssertThrowsError(try KagemushaNativeIncomingFoldWorkV1(selector: selector, nativeFields: changed))
  }

  func testAllPublicDigestAndEpochFieldsAreExactNonzeroWidths() throws {
    let request = try request()
    for index in [0, 1, 3, 4, 6, 8] {
      for bad in [Data(), Data(repeating: 0, count: 32), Data(repeating: 1, count: 31), Data(repeating: 1, count: 33)] {
        var changed = fields(); changed[index] = bad
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
          requestFrame: request, fields: changed))
      }
    }
    for bad in [Data(repeating: 0, count: 16), Data(repeating: 1, count: 15), Data(repeating: 1, count: 17)] {
      var changed = fields(); changed[7] = bad
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.prepareIncomingFold,
        requestFrame: request, fields: changed))
    }
  }

  func testNativePublicArchiveBoundsAdmitExactLimitAndRejectNextByte() throws {
    let request = try request()
    for (index, maximum) in [(2, 8192), (5, 32768), (9, 8192)] {
      var changed = fields(); changed[index] = Data(repeating: 1, count: maximum)
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
      fields: [history, Data([1]), Data([2]), signature])
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.completeIncomingFold,
      requestFrame: request, fields: [history]))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.completeIncomingFold,
      requestFrame: request, fields: [credit]))
    for (index, bad) in [(1, Data()), (1, Data(repeating: 1, count: 8193)),
      (2, Data()), (2, Data(repeating: 1, count: 96 * 1024 + 1)), (3, Data(repeating: 0, count: 64))] {
      var changed = [history, Data([1]), Data([2]), signature]; changed[index] = bad
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.completeIncomingFold, fields: changed))
    }
  }
}
