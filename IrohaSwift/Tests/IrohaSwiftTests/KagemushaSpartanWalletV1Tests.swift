import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaSpartanWalletV1Tests: XCTestCase {
  func testUnloadSettlementDistinguishesPayoutRetryAndMissingEpoch() throws {
    func parse(_ bytes: Data) throws -> KagemushaSpartanUnloadSettlementV1 {
      try .init(.init(status: 102, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: bytes))
    }
    var wire = Data(repeating: 0, count: 83)
    wire[0] = 17
    wire[32] = 8
    wire[74] = 1
    wire[75] = 2
    XCTAssertEqual(try parse(wire).requiredEpoch, 2)
    wire[74] = 0
    XCTAssertThrowsError(try parse(wire))
    wire[75] = 0
    wire[32] = 9
    wire[40] = 1
    wire[73] = 1
    let retry = try parse(wire)
    XCTAssertEqual(retry.height, 9)
    XCTAssertFalse(retry.accepted)
    XCTAssertTrue(retry.paid)
    wire[72] = 1
    XCTAssertTrue(try parse(wire).accepted)
    wire[73] = 0
    XCTAssertThrowsError(try parse(wire))
  }

  func testLocalBlacklistInstallRequiresItsExactNativeDigest() throws {
    let digest = Data(repeating: 7, count: 32)
    func reply(_ status: Int32, _ token: UInt64, _ bytes: Data) -> KagemushaSpartanReplyV1 {
      .init(status: status, sequenceLow: token, sequenceHigh: 0, detail: 0, bytes: bytes)
    }
    XCTAssertEqual(try reply(95, 0, digest).blacklistDigest(), digest)
    for invalid in [reply(94, 0, digest), reply(95, 1, digest),
                    reply(95, 0, Data()), reply(95, 0, Data(count: 32))] {
      XCTAssertThrowsError(try invalid.blacklistDigest())
    }
  }

  func testDebitTermsScaleExactUnsignedAtomsWithoutRounding() {
    XCTAssertEqual(spartanScaledAmount("0", 2), "0.00")
    XCTAssertEqual(spartanScaledAmount("1", 2), "0.01")
    XCTAssertEqual(spartanScaledAmount("100", 2), "1.00")
    let maximum = "340282366920938463463374607431768211455"
    XCTAssertEqual(spartanScaledAmount(maximum, 0), maximum)
    XCTAssertEqual(spartanScaledAmount(maximum, 28), "34028236692.0938463463374607431768211455")
  }
  func testWalletOwnedCommandsAndSelectedBalanceSnapshot() throws {
    let id = Data(repeating: 4, count: 32)
    let face = KagemushaWalletUInt128V1(low: 1, high: 2)
    let base = try KagemushaSpartanWalletV1.unloadCommand(requestId: id, amount: face, quote: Data(), certificates: Data(), beneficiary: Data())
    XCTAssertEqual(base.count, 61)
    XCTAssertEqual(Array(base.prefix(9)), [75,83,79,49,6,32,0,0,0])
    XCTAssertEqual(Array(base.suffix(16)), [1,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0])
    let quoted = try KagemushaSpartanWalletV1.unloadCommand(requestId: id, amount: face, quote: Data([7]), certificates: Data([8]), beneficiary: Data([9]))
    XCTAssertEqual(quoted, base + Data([1,0,0,0,7,1,0,0,0,8,1,0,0,0,9]))
    XCTAssertThrowsError(try KagemushaSpartanWalletV1.unloadCommand(requestId: id, amount: face, quote: Data([7]), certificates: Data(), beneficiary: Data()))
    XCTAssertEqual(try KagemushaSpartanWalletV1.ownedCommand(.bootstrap, []), Data([75,83,79,49,1]))
    XCTAssertEqual(try KagemushaSpartanWalletV1.ownedCommand(.load, [Data([7]),Data([8,9])]),
      Data([75,83,79,49,2,1,0,0,0,7,2,0,0,0,8,9]))
    XCTAssertThrowsError(try KagemushaSpartanWalletV1.ownedCommand(.load, [Data()]))
    func snapshot(_ bytes: Data) throws -> KagemushaSpartanSnapshotV1 {
      try .init(.init(status: 101, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: bytes))
    }
    var b = Data(repeating: 0, count: 196); b[0] = 1; b[2] = 3; b[3] = 28
    for i in [4,36,68,100,132] { b[i] = 1 }
    b[156] = 1; b[164] = 1; b.replaceSubrange(180..<188, with: repeatElement(255 as UInt8, count: 8))
    guard case let .released(_,_,sequence,balance,burned,spendable) = try snapshot(b).stage else { return XCTFail() }
    XCTAssertEqual(sequence.low,1); XCTAssertEqual(balance.high,1); XCTAssertEqual(burned.low,1)
    XCTAssertEqual(spendable.low,UInt64.max); XCTAssertEqual(spendable.high,0)
    for i in [0,2,4,36,68,100,156,164,180] { var bad=b; bad[i] ^= 1; XCTAssertThrowsError(try snapshot(bad)) }
    var pending=b.prefix(100); pending[2]=2
    if case .pending = try snapshot(pending).stage {} else { XCTFail() }
    var enrolled=b.prefix(68); enrolled[2]=1
    if case .enrolled = try snapshot(enrolled).stage {} else { XCTFail() }
    XCTAssertThrowsError(try snapshot(pending + Data([0])))
  }

  func testSignedTimeAnchorRefreshUsesWalletActionSevenWithBoundedOriginals() throws {
    XCTAssertEqual(try KagemushaSpartanWalletV1.refreshTimeAnchorCommand(anchor: Data([7]), certificate: Data([8, 9])),
                   Data([75, 83, 79, 49, 7, 1, 0, 0, 0, 7, 2, 0, 0, 0, 8, 9]))
    let longest = try KagemushaSpartanWalletV1.refreshTimeAnchorCommand(
      anchor: Data(repeating: 1, count: 512), certificate: Data(repeating: 2, count: 512))
    XCTAssertEqual(longest.count, 5 + 4 + 512 + 4 + 512)
    for (anchor, certificate) in [(Data(), Data([8])), (Data([7]), Data()),
                                  (Data(count: 513), Data([8])), (Data([7]), Data(count: 513))] {
      XCTAssertThrowsError(try KagemushaSpartanWalletV1.refreshTimeAnchorCommand(anchor: anchor, certificate: certificate))
    }
  }

  func testBootstrapActivationValidatesScopeLengthsAndCertifiedDecision() throws {
    func reply(_ kind: Int32, _ bytes: Data) -> KagemushaSpartanReplyV1 {
      .init(status: kind, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: bytes)
    }
    let pins = Data((1...4).flatMap { Array(repeating: UInt8($0), count: 32) })
    let claim = Data([0x4b, 0x47, 4])
    let exported = try KagemushaSpartanActivationV1(reply(107, pins + claim))
    XCTAssertEqual(exported.original, claim)
    XCTAssertEqual(exported.walletId, Data(repeating: 4, count: 32))
    XCTAssertEqual(try exported.instructionFrame().wireName, "iroha.kagemusha.spartan.activate.v1")
    XCTAssertEqual(try exported.instructionFrame().framedPayload.map { String(format: "%02x", $0) }.joined(), "4e52543000009a1a14c7f2632fd04b30d02624e1f63a0090000000000000004326b61176e2052b022001010101010101010101010101010101010101010101010101010101010101012002020202020202020202020202020202020202020202020202020202020202022003030303030303030303030303030303030303030303030303030303030303032004040404040404040404040404040404040404040404040404040404040404040b03000000000000004b4704")
    XCTAssertNoThrow(try KagemushaSpartanActivationV1(reply(107, pins + Data(repeating: 7, count: 32_768))))
    for bad in [pins, Data(repeating: 0, count: 32) + pins.dropFirst(32) + claim, pins + Data(repeating: 7, count: 32_769)] {
      XCTAssertThrowsError(try KagemushaSpartanActivationV1(reply(107, bad)))
    }
    var valid = Data(repeating: 0, count: 83)
    valid[0] = 1; valid[32] = 17; valid[40] = 7; valid[72] = 1; valid[73] = 1
    XCTAssertTrue(try KagemushaSpartanActivationSettlementV1(reply(108, valid)).activated)
    XCTAssertThrowsError(try KagemushaSpartanActivationSettlementV1(reply(102, valid)))
    valid[73] = 0
    XCTAssertThrowsError(try KagemushaSpartanActivationSettlementV1(reply(108, valid)))
  }

  func testExpectedLoadOrdinalPreservesAllBitsAndRequiresExactShape() throws {
    func decode(_ kind: Int32, _ bytes: Data) throws -> KagemushaWalletUInt128V1 {
      try KagemushaSpartanReplyV1(status: kind, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: bytes).nextLoadOrdinal()
    }
    var bytes = Data(repeating: 0, count: 16)
    XCTAssertEqual(try decode(106, bytes), .init(low: 0, high: 0))
    bytes[7] = 128; bytes[8] = 7
    XCTAssertEqual(try decode(106, bytes), .init(low: 1 << 63, high: 7))
    XCTAssertThrowsError(try decode(101, bytes))
    XCTAssertThrowsError(try decode(106, bytes + Data([0])))
  }

  func testSchemaThreeRequestCarrierAndExactIssuedReply() throws {
    func payload(fee: Bool) -> Data {
      let size = fee ? 1_677 : 1_250
      var bytes = Data("KWRQAC1\0".utf8) + Data([0, 0, UInt8(size >> 8), UInt8(truncatingIfNeeded: size), 0, 0, 0, 1])
      bytes += Data(repeating: 0, count: size); bytes.append(97)
      for (at, value): (Int, UInt8) in [(16,1),(258,7),(402,4),(434,9),(498,1),(628,4),(1094,1),(1128,1),(1129,4)] {
        bytes[at] = value
      }
      if fee { for (at, value): (Int, UInt8) in [(274,1),(1266,1),(1300,3),(1301,4),(1438,1)] { bytes[at] = value } }
      return bytes
    }
    func message(_ payload: Data) throws -> IrohaPeerWireMessageV1 {
      try .init(profile: .kagemushaSpartanPaymentV3, kind: .request, schemaVersion: 3, canonicalPayload: payload)
    }
    for fee in [false, true] {
      let bytes = payload(fee: fee), peer = try message(bytes)
      let originals = try IrohaPeerSpartanOpenRequestV3(canonicalPayload: bytes)
      var completed = originals.request
      for at in [66, 129, 194, 209] { completed[at] = 0xff }
      func agrees(_ request: Data) -> Bool {
        KagemushaSpartanVerifiedPaymentV1(digest: Data(repeating: 1, count: 32), request: request).matches(originals)
      }
      XCTAssertTrue(agrees(completed))
      for at in [0, 65, 130, 193, 210, 242, 418, 481] {
        var altered = completed
        altered[at] ^= 1
        XCTAssertFalse(agrees(altered))
      }
      XCTAssertEqual(originals.destinationAccountOriginal, Data([97]))
      XCTAssertEqual(originals.certificates.count, fee ? 344 : 172)
      XCTAssertEqual(originals.feeSchedule.count, fee ? 255 : 0)
      let reply = KagemushaSpartanReplyV1(status: 96, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: peer.encoded)
      let amount = Data([7]) + Data(repeating: 0, count: 15)
      XCTAssertEqual(try reply.issuedOpenRequest(amount: amount, feeSchedule: originals.feeSchedule).encoded, peer.encoded)
      XCTAssertThrowsError(try reply.issuedOpenRequest(amount: Data(repeating: 7, count: 16), feeSchedule: originals.feeSchedule))
      XCTAssertThrowsError(try IrohaPeerKagemushaWalletAdapterV1.decode(peer))
      for index in [0, 11, 15, 16+66, 16+194, 498+66, 1094+34, 16+258] {
        var bad = bytes; bad[index] ^= 1; XCTAssertThrowsError(try message(bad))
      }
      XCTAssertThrowsError(try message(bytes + Data([0])))
      if !fee { XCTAssertEqual(peer.wireHash.hexEncodedString(), "6ec3f326dc20c44ebe71f9cb52d18cae36b35b44946c706e26279fa895910afa") }
    }
  }

  func testCreditStatusTransportRequiresCanonicalFieldsAndACompleteOpaqueProofBody() throws {
    func wrap(_ bytes: Data) throws -> IrohaPeerWireMessageV1 {
      try .init(profile: .kagemushaSpartanPaymentV3, kind: .credited, schemaVersion: 3, canonicalPayload: bytes)
    }
    // Only the native wallet can decide the folded proof or the claimed outcome.
    var bytes = Data("KGSCST3\0".utf8) + Data(repeating: 0, count: 1 + 9 + 160 + 6 + 8) + Data([7])
    for flag: UInt8 in [0, 1] {
      bytes[8] = flag
      let message = try wrap(bytes)
      func reply(_ code: Int32, _ body: Data) -> KagemushaSpartanReplyV1 {
        .init(status: code, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: body)
      }
      XCTAssertEqual(try reply(104, message.encoded).issuedCreditStatus(), message)
      XCTAssertEqual(try reply(105, Data([1 - flag])).verifiedCreditOutcome(for: message), flag == 0 ? .credited : .burned)
      XCTAssertThrowsError(try reply(105, Data([flag])).verifiedCreditOutcome(for: message))
      XCTAssertThrowsError(try reply(104, Data([1 - flag])).verifiedCreditOutcome(for: message))
      XCTAssertThrowsError(try reply(105, Data([2])).verifiedCreditOutcome(for: message))
      XCTAssertThrowsError(try reply(104, bytes).issuedCreditStatus())
      XCTAssertEqual(try IrohaPeerWireMessageV1.decode(message.encoded).canonicalPayload, bytes)
      XCTAssertThrowsError(try IrohaPeerKagemushaWalletAdapterV1.decode(message))
    }
    for (at, value): (Int, UInt8) in [(0, 0), (8, 2), (9, 33), (9, 1), (188, 1)] {
      var malformed = bytes
      malformed[at] = value
      XCTAssertThrowsError(try wrap(malformed))
    }
    XCTAssertThrowsError(try wrap(bytes.dropLast()))
    XCTAssertThrowsError(try wrap(bytes + Data(repeating: 0, count: 10_001 - bytes.count)))
  }

  func testSpartanPaymentCarrierRoundTripsExactBytesOutsideInstalledWalletProfile() throws {
    var wrapper = Data("KWPYPC1\0".utf8)
    wrapper += Data([0, 0, 2, 88, 0, 0, 0, 3]) + Data(repeating: 0, count: 600) + Data("abc".utf8)
    let message = try IrohaPeerWireMessageV1(profile: .kagemushaSpartanPaymentV3, kind: .payment,
        schemaVersion: 3, canonicalPayload: wrapper, compressionPolicy: .peerOptimized)
    XCTAssertEqual(message.encoding, .none)
    XCTAssertEqual(message.wireHash.hexEncodedString(), "0c03088d2c5cb1d25144cbed3f175633c7d3f40d2d66018e09cd126c6b21647d")
    XCTAssertThrowsError(try IrohaPeerKagemushaWalletAdapterV1.decode(message))
    XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(message.encoded, expectedProfile: .kagemushaWalletV1))
    let scan = IrohaPeerQRScanSessionV1(expectedProfile: .kagemushaSpartanPaymentV3, expectedKind: .payment, expectedSchemaVersion: 3)
    var received: Data?
    for frame in try IrohaPeerQRCodecV1.animatedFrameTexts(for: message) {
      if case .completed(let decoded) = try scan.ingest(frame) { received = decoded.encoded; break }
    }
    XCTAssertEqual(received, message.encoded)
    for offset in [5, 8, 10, 16, 52, 84, 92, 96] {
      var bad = message.encoded; bad[offset] ^= 1
      XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(bad))
    }
    for offset in [0, 8, 12] {
      var bad = wrapper; bad[offset] ^= 1
      XCTAssertThrowsError(try IrohaPeerWireMessageV1(profile: .kagemushaSpartanPaymentV3, kind: .payment,
          schemaVersion: 3, canonicalPayload: bad))
    }
  }

  func testPaymentMatchedReceiptNamesOnlyTheExactCompleteSpartanPayment() throws {
    func payment(envelope: Int, companion: Int) throws -> IrohaPeerWireMessageV1 {
      func be32(_ value: Int) -> Data { Data((0..<4).map { UInt8(truncatingIfNeeded: value >> (24 - 8 * $0)) }) }
      let wrapper = Data("KWPYPC1\0".utf8) + be32(envelope) + be32(companion)
        + Data(repeating: 0x11, count: envelope) + Data(repeating: 0x22, count: companion)
      return try .init(profile: .kagemushaSpartanPaymentV3, kind: .payment, schemaVersion: 3, canonicalPayload: wrapper)
    }
    // The complete carrier cap is the typed NFC Payment cap: header, framing, envelope and payer companion.
    XCTAssertEqual(IrohaPeerNfcV1.maximumPaymentMessageBytes, 84 + 16 + 10_034 + 2_048)
    let largest = try payment(envelope: 10_034, companion: 2_048)
    XCTAssertEqual(largest.encoded.count, IrohaPeerNfcV1.maximumPaymentMessageBytes)
    XCTAssertEqual(try KagemushaSpartanWalletV1.receiptPayment(largest), largest.encoded)
    let current = try payment(envelope: 600, companion: 3)
    XCTAssertEqual(try KagemushaSpartanWalletV1.receiptPayment(current), current.encoded)
    // No larger Spartan Payment exists to offer.
    XCTAssertThrowsError(try payment(envelope: 10_035, companion: 2_048))
    // Another kind is never offered as the Payment a receipt names.
    let status = try IrohaPeerWireMessageV1(profile: .kagemushaSpartanPaymentV3, kind: .credited, schemaVersion: 3,
      canonicalPayload: Data("KGSCST3\0".utf8) + Data(repeating: 0, count: 1 + 9 + 160 + 6 + 8) + Data([7]))
    XCTAssertThrowsError(try KagemushaSpartanWalletV1.receiptPayment(status))
  }

  func testAdmissionProjectsRetainedOutputsOnlyForAuthenticatedSlot() throws {
    let parameters = Data(repeating: 1, count: 32)
    let slot = Data(repeating: 2, count: 32)
    func reply(_ status: Int32, token: UInt64 = 3, bytes: Data? = nil) -> KagemushaSpartanReplyV1 {
      .init(status: status, sequenceLow: token, sequenceHigh: 0, detail: 0, bytes: bytes ?? parameters + slot)
    }
    for status: Int32 in [85, 87] {
      let result = try reply(status).authorizedSlot(handle: 3, parameterId: parameters)
      XCTAssertEqual(result.slot, slot)
      XCTAssertEqual(result.retainedOutputsOnly, status == 87)
      for invalid in [reply(status, token: 4), reply(status, bytes: slot + parameters),
        reply(status, bytes: parameters + Data(repeating: 0, count: 32)), reply(status, bytes: slot)] {
        XCTAssertThrowsError(try invalid.authorizedSlot(handle: 3, parameterId: parameters))
      }
    }
    XCTAssertThrowsError(try reply(86).authorizedSlot(handle: 3, parameterId: parameters))
  }

  func testDirectTimeIntervalPreservesUnsignedBounds() throws {
    var bytes = Data(repeating: 0, count: 16)
    bytes[0] = 1
    bytes.replaceSubrange(8..<16, with: repeatElement(UInt8.max, count: 8))
    let interval = try KagemushaSpartanTimeIntervalV1(bytes: bytes)
    XCTAssertEqual(interval.lowerMs, 1)
    XCTAssertEqual(interval.upperMs, UInt64.max)
    XCTAssertThrowsError(try KagemushaSpartanTimeIntervalV1(bytes: bytes.prefix(15)))
    bytes.replaceSubrange(8..<16, with: repeatElement(0, count: 8))
    XCTAssertThrowsError(try KagemushaSpartanTimeIntervalV1(bytes: bytes))
  }

  func testCompletedNonSendHasNoPaymentButHeadAndCompletionRemainRequired() throws {
    let empty = KagemushaSpartanReplyV1(status: 1, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data())
    guard case .released(let bytes) = try KagemushaSpartanRetainedOutputV1(empty, part: .payment) else {
      return XCTFail("Completed operation must remain released when it has no outgoing Payment")
    }
    XCTAssertTrue(bytes.isEmpty)
    let claim = Data(repeating: 3, count: 16_384)
    func released(_ bytes: Data) -> KagemushaSpartanReplyV1 {
      .init(status: 1, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: bytes)
    }
    guard case .released(let retained) = try KagemushaSpartanRetainedOutputV1(released(claim), part: .unloadClaim) else {
      return XCTFail("Missing retained Unload claim")
    }
    XCTAssertEqual(retained, claim)
    XCTAssertThrowsError(try KagemushaSpartanRetainedOutputV1(released(claim + Data([1])), part: .unloadClaim))
    for part: KagemushaSpartanOutputV1 in [.head, .completion, .unloadClaim] {
      XCTAssertThrowsError(try KagemushaSpartanRetainedOutputV1(empty, part: part))
    }
    let pending = KagemushaSpartanReplyV1(status: 2, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data([1]))
    XCTAssertThrowsError(try KagemushaSpartanRetainedOutputV1(pending, part: .payment))
  }
  func testAdmissionRejectsAnotherCredentialProfileBeforeNative() throws {
    for count in [0, 532, 595, 597] {
      XCTAssertThrowsError(try KagemushaSpartanAccountOriginalsV1(credential: Data(repeating: 0, count: count),
        enrollmentCertificate: Data(repeating: 0, count: 172), account: Data([1]), assetScope: Data([2])))
    }
    for count in [0, 171, 173] {
      XCTAssertThrowsError(try KagemushaSpartanAccountOriginalsV1(credential: Data(repeating: 0, count: 596),
        enrollmentCertificate: Data(repeating: 0, count: count), account: Data([1]), assetScope: Data([2])))
    }
  }
  private func projection() -> Data {
    var bytes = [UInt8](repeating: 0, count: 291)
    bytes[0] = 1; bytes[2] = 3
    for (index, offset) in [3, 35, 67, 99, 163, 195, 227, 259].enumerated() {
      for at in offset..<offset + 32 { bytes[at] = UInt8(index + 1) }
    }
    for (offset, word) in [(131, UInt64.max), (139, UInt64(1) << 63), (147, UInt64(7)), (155, UInt64(9))] {
      for index in 0..<8 { bytes[offset + index] = UInt8(truncatingIfNeeded: word >> (index * 8)) }
    }
    return Data(bytes)
  }
  func testReviewPreservesUnsignedAmountsAndBoundIdentities() throws {
    var bytes = projection()
    let review = try KagemushaSpartanReviewProjectionV1(bytes)
    bytes.resetBytes(in: 0..<bytes.count)
    var exported = review.bytes
    exported.resetBytes(in: 0..<exported.count)
    XCTAssertEqual(review.bytes, projection())
    XCTAssertEqual(review.operation, .send)
    XCTAssertEqual(review.amount, .init(low: .max, high: 1 << 63))
    XCTAssertEqual(review.fee, .init(low: 7, high: 9))
    for (index, identity) in [review.schemeId, review.walletId, review.operationId, review.capsuleDigest,
      review.receiverWalletId, review.receiverAccountId, review.beforeHead, review.afterHead].enumerated() {
      XCTAssertEqual(identity, Data(repeating: UInt8(index + 1), count: 32))
    }
  }
  func testReviewRejectsUnsupportedVersionKindAndLength() {
    for length in [0, 290, 292] {
      XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(Data(repeating: 0, count: length)))
    }
    for (offset, byte) in [(0, UInt8(2)), (1, UInt8(1)), (2, UInt8(0)), (2, UInt8(5))] {
      var bytes = projection(); bytes[offset] = byte
      XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(bytes))
    }
  }
  func testSignedTimeAnchorReviewBindsItsFloorAndHeads() throws {
    var bytes = Data(repeating: 0, count: 243)
    bytes[0] = 2; bytes[2] = 7
    for (index, offset) in [3, 35, 67, 99, 131, 179, 211].enumerated() {
      bytes.replaceSubrange(offset..<offset + 32, with: repeatElement(UInt8(index + 1), count: 32))
    }
    bytes[170] = 0x80
    bytes[178] = 0x80
    bytes[171] = 1
    let anchor = try KagemushaSpartanTimeAnchorProjectionV2(bytes)
    XCTAssertEqual(anchor.issuerTime, UInt64(1) << 63)
    XCTAssertEqual(anchor.successorFloor, (UInt64(1) << 63) + 1)
    for (index, field) in [anchor.schemeId, anchor.walletId, anchor.operationId, anchor.capsuleDigest,
      anchor.anchorDigest, anchor.beforeHead, anchor.afterHead].enumerated() {
      XCTAssertEqual(field, Data(repeating: UInt8(index + 1), count: 32))
    }
    XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(bytes))
    XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(projection()))
    for malformed in [Data(bytes.dropLast()), bytes + Data([0])] {
      XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(malformed))
    }
    for (offset, value) in [(0, UInt8(1)), (1, 1), (2, 6), (178, 0)] {
      var malformed = bytes; malformed[offset] = value
      XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(malformed))
    }
    bytes[171] = 0
    XCTAssertEqual(try KagemushaSpartanTimeAnchorProjectionV2(bytes).successorFloor, anchor.issuerTime)
  }

  private func unloadReviewOriginal() throws -> Data {
    #if os(iOS)
    let resource = try XCTUnwrap(Bundle(for: Self.self).url(
      forResource: "spartan_unload_review_v3", withExtension: "bin"))
    return try Data(contentsOf: resource)
    #else
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0..<8 {
      let file = directory.appendingPathComponent("fixtures/kagemusha/spartan_unload_review_v3.bin")
      if FileManager.default.fileExists(atPath: file.path) { return try Data(contentsOf: file) }
      directory.deleteLastPathComponent()
    }
    throw CocoaError(.fileNoSuchFile)
    #endif
  }

  func testUnloadReviewBindsBothDestinationAccountsAndCharge() throws {
    let wire = try unloadReviewOriginal()
    var source = wire
    let review = try KagemushaSpartanUnloadProjectionV3(source)
    source.resetBytes(in: 0..<source.count)
    var exported = review.bytes
    exported.resetBytes(in: 0..<exported.count)
    XCTAssertEqual(review.bytes, wire)
    XCTAssertEqual(review.faceDebit, .init(low: 4, high: 0))
    XCTAssertEqual(review.onlineCharge, .init(low: 1, high: 0))
    XCTAssertEqual(review.netToAccount, .init(low: 3, high: 0))
    let beneficiary = try XCTUnwrap(review.beneficiaryAccountBytes)
    XCTAssertNotEqual(beneficiary, review.credentialAccountBytes)
    XCTAssertEqual(try AccountAddress.fromCanonicalBytes(beneficiary).canonicalBytes(), beneficiary)
    XCTAssertEqual(try AccountAddress.fromCanonicalBytes(review.credentialAccountBytes).canonicalBytes(),
      review.credentialAccountBytes)
    XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(wire))
    XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(wire))

    var free = Data(wire.prefix(421 + review.credentialAccountBytes.count))
    free.append(contentsOf: [0, 0])
    for range in [227..<243, 259..<291, 323..<355] { free.resetBytes(in: range) }
    free[243] = 4
    XCTAssertNil(try KagemushaSpartanUnloadProjectionV3(free).beneficiaryAccountBytes)
    var borrowed = wire
    borrowed.resetBytes(in: 211..<227); borrowed[219] = 1
    borrowed.replaceSubrange(243..<259, with: [UInt8](repeating: 255, count: 8) + [UInt8](repeating: 0, count: 8))
    XCTAssertEqual(try KagemushaSpartanUnloadProjectionV3(borrowed).netToAccount, .init(low: .max, high: 0))
    var allCharged = wire
    allCharged[227] = 4; allCharged[243] = 0
    XCTAssertEqual(try KagemushaSpartanUnloadProjectionV3(allCharged).netToAccount, .init(low: 0, high: 0))

    for change in [0, 1, 2, 211, 243, 420] {
      var invalid = wire; invalid[change] ^= 0x80
      XCTAssertThrowsError(try KagemushaSpartanUnloadProjectionV3(invalid))
    }
    for erase in [163..<195, 211..<227, 227..<243, 259..<291, 323..<355, 419..<421] {
      var invalid = wire; invalid.resetBytes(in: erase)
      XCTAssertThrowsError(try KagemushaSpartanUnloadProjectionV3(invalid))
    }
    for invalid in [Data(wire.dropLast()), wire + Data([0]), Data(repeating: 0, count: 16_385)] {
      XCTAssertThrowsError(try KagemushaSpartanUnloadProjectionV3(invalid))
    }
  }

  func testExecuteValidatesNativeSequenceAndStatusSpecificReason() throws {
    func execution(_ status: Int32, low: UInt64 = 0, high: UInt64 = 0, detail: UInt32 = 0) throws -> KagemushaSpartanExecutionV1 {
      try .init(.init(status: status, sequenceLow: low, sequenceHigh: high, detail: detail,
        bytes: status == 1 ? Data([1]) : Data()))
    }
    for status in Int32(1)...5 {
      XCTAssertThrowsError(try execution(status, low: 1))
      XCTAssertThrowsError(try execution(status, high: 1))
      if status != 3 { XCTAssertThrowsError(try execution(status, detail: 1)) }
    }
    for reason in UInt32(0)...2 { XCTAssertEqual(try execution(3, detail: reason).detail, reason) }
    XCTAssertThrowsError(try execution(3, detail: 3))
  }

  func testReplyDomainRejectsFalseCompleteAndInvalidToken() throws {
    func reply(_ status: Int32, _ token: UInt64 = 0, _ bytes: Data = Data()) -> KagemushaSpartanReplyV1 {
      .init(status: status, sequenceLow: token, sequenceHigh: 0, detail: 0, bytes: bytes)
    }
    XCTAssertThrowsError(try reply(82, 0, projection()).requireShape(status: 82, count: 291, token: true))
    XCTAssertThrowsError(try reply(83, 1, Data(repeating: 0, count: 514)).requireShape(status: 83, count: 514))
    XCTAssertThrowsError(try KagemushaSpartanExecutionV1(reply(1)))
    XCTAssertThrowsError(try KagemushaSpartanExecutionV1(reply(2, 0, Data([1]))))
    XCTAssertThrowsError(try KagemushaSpartanExecutionV1(reply(84)))
    XCTAssertEqual(try KagemushaSpartanExecutionV1(reply(2)).status, .pending)
    XCTAssertEqual(try KagemushaSpartanExecutionV1(reply(1, 0, Data([1, 2, 3]))).completion, Data([1, 2, 3]))
  }

  func testInstalledOpenAcceptsOnlyItsOwnHandleAndNonzeroParameterIdentity() throws {
    func identity(_ status: Int32, _ token: UInt64, _ bytes: Data) throws -> Data {
      try KagemushaSpartanRuntimeV1.installedParameterId(
        .init(status: status, sequenceLow: token, sequenceHigh: 0, detail: 0, bytes: bytes), handle: 9)
    }
    let parameters = Data(repeating: 7, count: 32)
    XCTAssertEqual(try identity(81, 9, parameters), parameters)
    XCTAssertThrowsError(try identity(81, 8, parameters))
    XCTAssertThrowsError(try identity(81, 0, parameters))
    XCTAssertThrowsError(try identity(81, 9, Data(count: 32)))
    XCTAssertThrowsError(try identity(81, 9, Data(repeating: 7, count: 31)))
    XCTAssertThrowsError(try identity(15, 9, parameters))
  }
}
