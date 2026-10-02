import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Actual Rust message vectors plus explicitly inert shape/model specimens.
/// These tests create no credential, State, Guard, issuer, Native owner or money authority.
final class KagemushaOrdinaryCashApprovalProjectionV1Tests: XCTestCase {
  private let operations = ["mint_fold", "send_split", "receive_fold", "redeem_split", "rotate"]
  private let indices = ["9", "340282366920938463463374607431768211454"]
  private struct Specimen { var w: Data; var s: Data }

  func testActualRustTerminalVectorsPreserveFullHashesAndRemainUnissued() throws {
    let vectors = try fixtures()
    for operation in operations { for before in indices {
      let w = try XCTUnwrap(vectors["w_\(operation)_\(before)"])
      let s = try XCTUnwrap(vectors["s_\(operation)_\(before)"])
      let value = try KagemushaOrdinaryCashApprovalProjectionV1.requireModelMessageShape(
        .monetaryTransition, nativeSigningBytes: w, nativeFinancialSubject: s)
      XCTAssertEqual(value.subjectSigningDigest, vectors["s_\(operation)_\(before)_sha256"])
      XCTAssertEqual(value.clientDataHash, vectors["w_\(operation)_\(before)_sha256"])
      XCTAssertEqual(value.canonicalSigningBytes, w); XCTAssertEqual(value.canonicalFinancialSubject, s)
      if before == "9" {
        XCTAssertEqual(value.logicalIndexBeforeLE, Data([9]) + Data(repeating: 0, count: 15))
        XCTAssertEqual(value.logicalIndexAfterLE, Data([10]) + Data(repeating: 0, count: 15))
      } else {
        XCTAssertEqual(value.logicalIndexBeforeLE, Data([0xfe]) + Data(repeating: 0xff, count: 15))
        XCTAssertEqual(value.logicalIndexAfterLE, Data(repeating: 0xff, count: 16))
      }
      // Actual generic message fixtures deliberately use independent credential markers.
      XCTAssertThrowsError(try project(Specimen(w: w, s: s), preparation: false))
    } }
  }

  func testPreparationAndTerminalRemainDistinctForEveryCashOperationAndCarryBoundary() throws {
    for operation in operations { for before in indices {
      let terminal = try specimen(operation, before, false), preparation = try specimen(operation, before, true)
      let terminalValue = try project(terminal, preparation: false), preparedValue = try project(preparation, preparation: true)
      XCTAssertEqual(terminalValue.purpose, .monetaryTransition); XCTAssertEqual(preparedValue.purpose, .prepareTransition)
      XCTAssertEqual(terminalValue.operationID, preparedValue.operationID)
      XCTAssertEqual(terminalValue.operationTag, preparedValue.operationTag)
      XCTAssertEqual(terminalValue.transitionStatementDigest, preparedValue.transitionStatementDigest)
      XCTAssertNotEqual(terminalValue.clientDataHash, preparedValue.clientDataHash)
      XCTAssertThrowsError(try project(terminal, preparation: true))
      XCTAssertThrowsError(try project(preparation, preparation: false))
      for offset in [364, 396] {
        var changed = preparation.s; changed[offset] = 1
        XCTAssertThrowsError(try project(coherent(preparation, changed), preparation: true))
        changed = terminal.s
        if ["send_split", "redeem_split"].contains(operation) { clear(&changed, offset, 32) }
        else { changed[offset] = 1 }
        XCTAssertThrowsError(try project(coherent(terminal, changed), preparation: false))
      }
    } }
  }

  func testCashFramesIdentitiesEpochsAndUInt128IndicesRejectCoherentMutations() throws {
    for preparation in [false, true] {
      let original = try specimen("send_split", "9", preparation)
      for offset in [0, 42, 50, 51, 52] {
        var changed = original; changed.w[offset] ^= 0x40
        XCTAssertThrowsError(try project(changed, preparation: preparation))
      }
      for slot in 0..<8 {
        var changed = original; clear(&changed.w, 53 + slot * 32, 32)
        XCTAssertThrowsError(try project(changed, preparation: preparation))
      }
      for offset in [0, 49, 57, 58, 331] {
        var s = original.s; s[offset] ^= 0x40
        XCTAssertThrowsError(try project(coherent(original, s), preparation: preparation))
      }
      for offset in [59, 91, 123, 155, 187, 219, 251, 291, 332, 283, 323] {
        var s = original.s; clear(&s, offset, [283, 323].contains(offset) ? 8 : 32)
        XCTAssertThrowsError(try project(coherent(original, s), preparation: preparation))
      }
      for offset in [428, 444] {
        var s = original.s; s[offset] ^= 1
        XCTAssertThrowsError(try project(coherent(original, s), preparation: preparation))
      }
      var s = original.s; s.replaceSubrange(428..<444, with: Data(repeating: 0xff, count: 16)); clear(&s, 444, 16)
      XCTAssertThrowsError(try project(coherent(original, s), preparation: preparation))
      s = original.s; s[331] = 0; clear(&s, 364, 96)
      XCTAssertThrowsError(try project(coherent(original, s), preparation: preparation))
      for w in [Data(original.w.dropLast()), original.w + Data([0])] {
        XCTAssertThrowsError(try project(Specimen(w: w, s: original.s), preparation: preparation))
      }
      for s in [Data(original.s.dropLast()), original.s + Data([0])] {
        XCTAssertThrowsError(try KagemushaOrdinaryCashApprovalProjectionV1.requireModelMessageShape(
          preparation ? .prepareTransition : .monetaryTransition, nativeSigningBytes: original.w, nativeFinancialSubject: s))
      }
    }
  }

  func testUnsignedOriginalIntervalAndRawSubjectHashNeedNoPlatformClock() throws {
    let original = try specimen("rotate", "9", true)
    for (issued, expires) in [(UInt64.max - 120_000, UInt64.max), (UInt64(1), UInt64(120_001))] {
      var value = original; put64(&value.w, 309, issued); put64(&value.w, 317, expires)
      _ = try project(value, preparation: true)
    }
    for (issued, expires) in [(UInt64(0), UInt64(1)), (1, 1), (2, 1), (1, 120_002)] {
      var value = original; put64(&value.w, 309, issued); put64(&value.w, 317, expires)
      XCTAssertThrowsError(try project(value, preparation: true))
    }
    let projection = try project(original, preparation: true)
    XCTAssertEqual(projection.subjectSigningDigest, sha(original.s))
    XCTAssertEqual(projection.clientDataHash, sha(original.w))
    XCTAssertNotEqual(projection.clientDataHash, sha(projection.clientDataHash))
    var doublePrefix = original
    doublePrefix.w.replaceSubrange(245..<277, with: sha(Data("iroha:kagemusha:v1:hardware-transition-selection\0".utf8) + original.s))
    XCTAssertThrowsError(try project(doublePrefix, preparation: true))
  }

  func testRetainedPublicFieldsFullSelectionAndCredentialCannotBeSubstituted() throws {
    let original = try specimen("mint_fold", "9", true), retained = try binding(original)
    for offset in [53, 117, 149, 181, 213, 277] {
      var changed = original.w; changed[offset] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryCashApprovalProjectionV1.requirePreparation(
        nativeSigningBytes: changed, nativeFinancialSubject: original.s, binding: retained))
    }
    var selection = original.s; selection[59] ^= 1
    let changed = coherent(original, selection)
    XCTAssertThrowsError(try KagemushaOrdinaryCashApprovalProjectionV1.requirePreparation(
      nativeSigningBytes: changed.w, nativeFinancialSubject: changed.s, binding: retained))
    selection = original.s; selection[155] ^= 1
    XCTAssertThrowsError(try project(coherent(original, selection), preparation: true))
    for bad in [Data(repeating: 1, count: 31), Data(repeating: 0, count: 32), Data(repeating: 1, count: 33)] {
      XCTAssertThrowsError(try KagemushaOrdinaryCashApprovalOriginalBindingV1(operationID: bad,
        accountBinding: Data(repeating: 1, count: 32), authorityPolicyDigest: Data(repeating: 2, count: 32),
        attestedKeyID: Data(repeating: 3, count: 32), enrollmentDigest: Data(repeating: 4, count: 32),
        normalizedGuardDigest: Data(repeating: 5, count: 32), originalSelection: original.s))
    }
    XCTAssertThrowsError(try binding(Specimen(w: original.w, s: Data(original.s.dropLast()))))
  }

  func testCashProjectionOwnsCopiedDataIncludingSliceInputs() throws {
    var original = try specimen("redeem_split", "9", false)
    let expected = original, retained = try binding(original)
    let wSlice = (Data([0xff]) + original.w).dropFirst(), sSlice = (Data([0xff]) + original.s).dropFirst()
    let projected = try KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(
      nativeSigningBytes: wSlice, nativeFinancialSubject: sSlice, binding: retained)
    clear(&original.w, 0, original.w.count); clear(&original.s, 0, original.s.count)
    var returnedW = projected.canonicalSigningBytes, returnedS = projected.canonicalFinancialSubject
    clear(&returnedW, 0, returnedW.count); clear(&returnedS, 0, returnedS.count)
    XCTAssertEqual(projected.canonicalSigningBytes, expected.w); XCTAssertEqual(projected.canonicalFinancialSubject, expected.s)
    XCTAssertEqual(projected.operationID, Data(expected.w[53..<85]))
    XCTAssertEqual(projected.subjectSigningDigest, sha(expected.s)); XCTAssertEqual(projected.clientDataHash, sha(expected.w))
  }

  func testCompleteModelOriginalCorrelatesBothCashPurposesWithoutAnotherCodecPrefix() throws {
    let original = try modelSpecimen()
    for preparation in [false, true] {
      let cash = try approval(original, preparation), projection = try statement(original, cash)
      XCTAssertEqual(projection.operationTag, 2); XCTAssertEqual(projection.canonicalPreimage, original)
      XCTAssertEqual(projection.digest, sha(original)); XCTAssertEqual(projection.digest, cash.transitionStatementDigest)
      XCTAssertEqual(cash.subjectSigningDigest, sha(cash.canonicalFinancialSubject))
      // State journal 19/20 is independent of S logical indices 9/10.
      XCTAssertNotEqual(Data(original[(56 + 1025)..<(56 + 1041)]), cash.logicalIndexBeforeLE)
    }
  }

  func testAll1145OriginalBytesAndEveryTruncationAreBound() throws {
    let original = try modelSpecimen(), cash = try approval(original, true)
    for offset in original.indices {
      var changed = original; changed[offset] ^= 1
      XCTAssertThrowsError(try statement(changed, cash), "changed byte \(offset)")
    }
    for size in 0..<1145 { XCTAssertThrowsError(try statement(Data(original.prefix(size)), cash), "truncated \(size)") }
    for wrong in [original + Data([0]), Data(original.dropFirst(56)), Data(repeating: 0, count: 93 * 32)] {
      XCTAssertThrowsError(try statement(wrong, cash))
    }
  }

  func testCoherentModelDigestCannotRelabelVersionOperationDomainOrScope() throws {
    let original = try modelSpecimen()
    for offset in [0, 8, 47, 48, 55, 56, 57, 58, 59, 188,
      56 + 373, 56 + 405, 56 + 501, 56 + 533, 56 + 541, 56 + 573] {
      var changed = original; changed[offset] ^= 1
      XCTAssertThrowsError(try statement(changed, approval(changed, true)), "coherent changed byte \(offset)")
    }
    var wrongEndian = original; put64(&wrongEndian, 0, 40); put64(&wrongEndian, 48, 1089)
    XCTAssertThrowsError(try statement(wrongEndian, approval(wrongEndian, true)))
    var bootstrap = original; bootstrap[188] = 0
    XCTAssertThrowsError(try statement(bootstrap, approval(bootstrap, true)))
  }

  func testCompleteModelOriginalRemainsImmutable() throws {
    var original = try modelSpecimen(); let expected = original
    let value = try statement(original, approval(original, true))
    clear(&original, 0, original.count); var returned = value.canonicalPreimage; clear(&returned, 0, returned.count)
    XCTAssertEqual(value.canonicalPreimage, expected); XCTAssertEqual(value.digest, sha(expected))
  }

  private func specimen(_ operation: String, _ before: String, _ preparation: Bool) throws -> Specimen {
    let values = try fixtures()
    var value = Specimen(w: try XCTUnwrap(values["w_\(operation)_\(before)"]), s: try XCTUnwrap(values["s_\(operation)_\(before)"]))
    // Explicitly inert ordinary shape: align fixture markers, never mint issuer evidence.
    value.s.replaceSubrange(155..<187, with: value.w[213..<245])
    if preparation { value.w[52] = 2; clear(&value.s, 364, 64) }
    return coherent(value, value.s)
  }
  private func coherent(_ original: Specimen, _ selection: Data) -> Specimen {
    var w = original.w; w.replaceSubrange(245..<277, with: sha(selection)); return Specimen(w: w, s: selection)
  }
  private func binding(_ value: Specimen) throws -> KagemushaOrdinaryCashApprovalOriginalBindingV1 {
    try .init(operationID: Data(value.w[53..<85]), accountBinding: Data(value.w[117..<149]),
      authorityPolicyDigest: Data(value.w[149..<181]), attestedKeyID: Data(value.w[181..<213]),
      enrollmentDigest: Data(value.w[213..<245]), normalizedGuardDigest: Data(value.w[277..<309]), originalSelection: value.s)
  }
  private func project(_ value: Specimen, preparation: Bool) throws -> KagemushaOrdinaryCashApprovalProjectionV1 {
    let retained = try binding(value)
    return preparation ? try .requirePreparation(nativeSigningBytes: value.w, nativeFinancialSubject: value.s, binding: retained) :
      try .requireTerminal(nativeSigningBytes: value.w, nativeFinancialSubject: value.s, binding: retained)
  }
  private func approval(_ preimage: Data, _ preparation: Bool) throws -> KagemushaOrdinaryCashApprovalProjectionV1 {
    var value = try specimen("send_split", "9", preparation)
    value.s.replaceSubrange(332..<364, with: sha(preimage)); return try project(coherent(value, value.s), preparation: preparation)
  }
  private func statement(_ preimage: Data, _ cash: KagemushaOrdinaryCashApprovalProjectionV1) throws -> KagemushaOrdinaryTransitionStatementProjectionV1 {
    try .requireOriginal(canonicalModelPreimage: preimage, cashApproval: cash)
  }
  private func sha(_ data: Data) -> Data { Data(SHA256.hash(data: data)) }
  private func clear(_ data: inout Data, _ offset: Int, _ count: Int) { data.replaceSubrange(offset..<(offset + count), with: Data(repeating: 0, count: count)) }
  private func put64(_ data: inout Data, _ offset: Int, _ value: UInt64) { var x = value.littleEndian; data.replaceSubrange(offset..<(offset + 8), with: withUnsafeBytes(of: &x) { Data($0) }) }

  /// Exact field order from commitments.rs and its 1089-byte field-by-field Rust test.
  /// A synthetic model transcript is not an admitted State/Guard or financial relation.
  private func modelSpecimen() throws -> Data {
    let selection = try XCTUnwrap(try fixtures()["s_send_split_9"]); var body = Data()
    func integer(_ value: UInt64, _ width: Int) { body.append(contentsOf: (0..<width).map { UInt8(truncatingIfNeeded: value >> ($0 * 8)) }) }
    func wide(_ value: UInt64) { integer(value, 8); integer(0, 8) }
    func digest(_ value: UInt8) { body.append(Data(repeating: value, count: 32)) }
    func scoped(_ offset: Int) { body.append(selection[offset..<(offset + 32)]) }
    integer(1, 2); integer(1, 2); digest(0x22); digest(0x23); digest(0x22); digest(0x23)
    body.append(2); wide(7); digest(0); digest(0); digest(0x40); digest(0x41); digest(0); digest(0x42); digest(0)
    scoped(59); scoped(59); digest(0x43); digest(0x44); scoped(251)
    body.append(selection[283..<291]); scoped(187); scoped(219); digest(0x45); integer(2, 4)
    digest(0x30); digest(0x31); wide(9); wide(10); wide(1); digest(0x46); wide(1); digest(0x46)
    digest(0x20); digest(0x21); digest(0x20); digest(0x21)
    digest(0x32); digest(0x33); wide(19); wide(20); digest(0x34)
    XCTAssertEqual(body.count, 1089)
    let domain = Data("iroha:kagemusha:v1:transition-statement\0".utf8)
    var prefix = UInt64(domain.count).bigEndian, count = UInt64(body.count).bigEndian
    return withUnsafeBytes(of: &prefix) { Data($0) } + domain + withUnsafeBytes(of: &count) { Data($0) } + body
  }

  private func fixtures() throws -> [String: Data] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_app_platform_messages_v1.tsv")
      if FileManager.default.fileExists(atPath: path.path) {
        var result: [String: Data] = [:]
        for line in try String(contentsOf: path, encoding: .utf8).split(separator: "\n") where !line.hasPrefix("#") {
          let columns = line.split(separator: "\t", omittingEmptySubsequences: false)
          guard columns.count == 2, columns[1].count % 2 == 0 else { throw FixtureError.invalid }
          let chars = Array(columns[1].utf8); var value = Data()
          for offset in stride(from: 0, to: chars.count, by: 2) {
            guard let byte = UInt8(String(decoding: chars[offset..<(offset + 2)], as: UTF8.self), radix: 16) else { throw FixtureError.invalid }
            value.append(byte)
          }
          guard result.updateValue(value, forKey: String(columns[0])) == nil else { throw FixtureError.invalid }
        }
        return result
      }
      directory.deleteLastPathComponent()
    }
    throw FixtureError.invalid
  }
  private enum FixtureError: Error { case invalid }
}
