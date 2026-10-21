import Foundation
import XCTest
@testable import IrohaSwift

/// DATA-only transport tests. These fixtures carry no Native proof or review capability grant.
final class KagemushaWalletReviewV1Tests: XCTestCase {
  private func data(send: Bool = true) -> Data {
    var b = [UInt8](repeating: 0, count: 491)
    b.replaceSubrange(0..<8, with: [75,87,79,82,86,49,0,0]); b[8] = send ? 1 : 8
    for offset in [9,25,41,57] { for i in 0..<16 { b[offset+i] = UInt8(i+1) } }
    if send { b[73] = 1; b[74] = 2; b[138] = 3 }
    for offset in [106,202,234,266,298,330,362,394] { b[offset] = 4 }
    b[426] = 4; b[427] = 5
    b.append(contentsOf: send ? [3,0,0,0,0xa1,0xb2,0xc3] : [0,0,0,0])
    return Data(b)
  }
  private func reply(_ bytes: Data? = nil, status: Int32 = 18, token: UInt64 = 8, high: UInt64 = 0, detail: UInt32 = 0) -> KagemushaWalletReviewReplyV1 {
    .init(status: status, reason: -1, platformCode: 0, sequenceLow: token, sequenceHigh: high, detail: detail, bytes: bytes ?? data())
  }
  func testPreservesAllUnsignedScalarBitsAndWholeProjection() throws {
    var bytes = data(); bytes.replaceSubrange(9..<25, with: Data(repeating: 255, count: 16))
    let p = try KagemushaWalletReviewProjectionV1(bytes)
    XCTAssertEqual(p.amount.low, UInt64.max); XCTAssertEqual(p.amount.high, UInt64.max); XCTAssertEqual(bytes,p.bytes)
  }
  func testRetainsValueCopiesAndExactSourceFields() throws {
    var bytes = data(); let original = bytes, p = try KagemushaWalletReviewProjectionV1(bytes)
    bytes.resetBytes(in: 0..<bytes.count)
    var account = try XCTUnwrap(p.destinationAccountOriginal); account[0] = 0
    XCTAssertEqual(p.destinationAccountOriginal, Data([0xa1,0xb2,0xc3]))
    XCTAssertEqual(p.bytes,original); XCTAssertEqual(p.walletID[0],4); XCTAssertEqual(p.paymentPublicKey[0],4)
  }
  func testRequiresExactLengthMagicSelectorAndHardwareKeyForm() {
    var variants = [Data(data().dropLast()), data()+Data([0])]
    for (offset,value) in [(0,UInt8(0)),(8,2),(426,3)] { var b=data(); b[offset]=value; variants.append(b) }
    for b in variants { XCTAssertThrowsError(try KagemushaWalletReviewProjectionV1(b)) }
  }
  func testSendAndUnloadHaveDifferentReceiverAndRequestShapes() throws {
    var b=data(); b[73]=0; XCTAssertThrowsError(try KagemushaWalletReviewProjectionV1(b))
    b=data(); b[170]=1; XCTAssertThrowsError(try KagemushaWalletReviewProjectionV1(b))
    let p=try KagemushaWalletReviewProjectionV1(data(send:false)); XCTAssertNil(p.receiverWalletID); XCTAssertNil(p.destinationAccountOriginal); XCTAssertEqual(p.requestDigest,Data(repeating:0,count:32))
  }
  func testAccountOriginalLengthIsMandatoryExactAndBounded() throws {
    var variants = [Data(data().prefix(491)), Data(data().prefix(495)), data()+Data([0]),
      Data(data().prefix(491))+Data([0,0,0,0])]
    let lengths: [[UInt8]] = [[2,0,0,0],[4,0,0,0],[1,16,0,0],[255,255,255,255]]
    for length in lengths {
      var b = data(); b.replaceSubrange(491..<495, with: length); variants.append(b)
    }
    var unload = data(send:false); unload.replaceSubrange(491..<495, with:[1,0,0,0]); unload.append(7); variants.append(unload)
    for b in variants { XCTAssertThrowsError(try KagemushaWalletReviewProjectionV1(b)) }
    var maximum = Data(data().prefix(491)); maximum.append(contentsOf:[0,16,0,0]); maximum.append(Data(repeating:7,count:4_096))
    XCTAssertEqual(try KagemushaWalletReviewProjectionV1(maximum).destinationAccountOriginal?.count,4_096)
  }
  func testRejectsMissingCurrentSourceAndZeroAmount() {
    for offset in [106,202,234,266,298,330,362,394] { var b=data(); b.resetBytes(in:offset..<(offset+32)); XCTAssertThrowsError(try KagemushaWalletReviewProjectionV1(b)) }
    var b=data(); b.resetBytes(in:9..<25); XCTAssertThrowsError(try KagemushaWalletReviewProjectionV1(b))
  }
  func testDedicatedReviewRejectsOpenActivationAndMalformedToken() {
    for status: Int32 in [0,1,12,13,14,15,16,17,19] { XCTAssertThrowsError(try reply(status:status).readyFixture(origin:NSObject(),expected:.send)) }
    for r in [reply(token:0),reply(token:UInt64.max),reply(high:1),reply(detail:1)] { XCTAssertThrowsError(try r.readyFixture(origin:NSObject(),expected:.send)) }
    XCTAssertThrowsError(try reply().readyFixture(origin:NSObject(),expected:.unload))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status:18,sequenceLow:8,sequenceHigh:0,detail:0,bytes:data()))
  }
  func testForeignOwnerCannotConsumeAndOriginalCanConsumeOnlyOnce() throws {
    let owner=NSObject(), r=try reply().readyFixture(origin:owner,expected:.send)
    XCTAssertThrowsError(try r.consume(origin:NSObject())); XCTAssertEqual(try r.consume(origin:owner),8)
    XCTAssertThrowsError(try r.consume(origin:owner))
  }
  func testProjectionCopyDoesNotCreateAnotherToken() throws {
    let owner=NSObject(), r=try reply().readyFixture(origin:owner,expected:.send), p=r.projection
    XCTAssertEqual(p.bytes,r.projection.bytes); _=try r.consume(origin:owner)
    XCTAssertThrowsError(try r.consume(origin:owner)); XCTAssertEqual(p.bytes,data())
  }
  private func bound(_ kind: UInt32, detail: UInt32 = 0, low: UInt64 = 0,
    high: UInt64 = 0, payload: Data = Data()) throws -> KagemushaWalletCallV1 {
    var bytes = Data()
    func word<T: FixedWidthInteger>(_ value: T) { var v = value.littleEndian; withUnsafeBytes(of: &v) { bytes.append(contentsOf: $0) } }
    word(kind); word(detail); word(low); word(high); word(UInt32(payload.count)); bytes.append(payload)
    return try .init(status: 66, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: bytes)
  }
  func testExistingNeverBecomesUnknownOrNewExecution() throws {
    if case .preparing = try KagemushaWalletBoundRequestV1(bound(11)) {} else { XCTFail("expected Preparing") }
    for kind: UInt32 in [1,2,3,4,5,10] {
      let value = try bound(kind, payload: [1,10].contains(kind) ? Data([7]) : Data())
      if case .existing(.outcome(let original)) = try KagemushaWalletReviewedExecutionResultV1(value) {
        XCTAssertEqual(original.status, Int32(kind))
      } else { XCTFail("existing result became a dispatch") }
      XCTAssertThrowsError(try value.completion())
    }
    for kind: UInt32 in [0,6,7,8,9,12,18,64,65,66,67,UInt32.max] {
      XCTAssertThrowsError(try KagemushaWalletBoundRequestV1(bound(kind)))
    }
    for malformed in [try bound(11, detail: 1), try bound(11, low: 1),
      try bound(11, high: 1), try bound(11, payload: Data([1])), try bound(3, detail: 1)] {
      XCTAssertThrowsError(try KagemushaWalletBoundRequestV1(malformed))
    }
    let zero = try KagemushaWalletCallV1(status: 0, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data())
    XCTAssertThrowsError(try KagemushaWalletReviewedExecutionResultV1(zero))
  }
  func testIssuanceWaitingHasNoCapabilityAndExecutionRefusesIt() throws {
    for reason: UInt32 in [1,2,6] {
      let wait = reply(Data(), status: 67, token: 0, detail: reason)
      if case .waiting(let value) = try wait.decision(origin: NSObject(), requestID: Data(repeating: 1,count: 32), expected: .send) {
        XCTAssertEqual(value.rawValue, reason)
      } else { XCTFail("waiting produced a capability") }
      let result = try KagemushaWalletCallV1(status: 67, sequenceLow: 0, sequenceHigh: 0, detail: reason, bytes: Data())
      XCTAssertThrowsError(try KagemushaWalletReviewedExecutionResultV1(result))
    }
    for reason: UInt32 in [0,3,4,5,7,.max] {
      XCTAssertThrowsError(try reply(Data(),status:67,token:0,detail:reason).decision(origin:NSObject(),requestID:Data(repeating:1,count:32),expected:.send))
    }
    XCTAssertThrowsError(try reply(Data(),status:67,token:7,detail:1).decision(origin:NSObject(),requestID:Data(repeating:1,count:32),expected:.send))
    XCTAssertThrowsError(try reply(Data(),status:65,token:7,detail:1).decision(origin:NSObject(),requestID:Data(repeating:1,count:32),expected:.send))
  }
  func testRequestIdentityAndFullOriginalExpectationAreRetainedAtIssuance() throws {
    var identity = Data(repeating: 9, count: 32), original = data()
    let input = try KagemushaWalletReviewInputV1(requestID: identity,
      expectation: .originalUnstarted(original), selector: 1, first: Data([1]), second: Data([2]))
    let review = try reply().readyFixture(origin: NSObject(), expected: .send, requestID: identity)
    identity[0] = 0; original[0] = 0
    XCTAssertEqual(review.requestID, Data(repeating: 9,count: 32))
    input.withRequest { request in
      XCTAssertEqual(Data(bytes: request.pointee.request_id32!, count:32), Data(repeating:9,count:32))
      XCTAssertEqual(request.pointee.expectation,1)
      XCTAssertEqual(Data(bytes:request.pointee.expected_projection!,count:request.pointee.expected_projection_length),data())
    }
    for id in [Data(),Data(count:32),Data(repeating:1,count:31),Data(repeating:1,count:33)] {
      XCTAssertThrowsError(try KagemushaWalletReviewInputV1(requestID:id,expectation:.fresh,selector:1,first:Data([1]),second:Data([2])))
    }
    for old in [Data(repeating:65,count:64),Data(data().prefix(491)),data()+Data([0]),data(send:false)] {
      XCTAssertThrowsError(try KagemushaWalletReviewInputV1(requestID:identity,expectation:.originalUnstarted(old),selector:1,first:Data([1]),second:Data([2])))
    }
  }

  func testBoundEnvelopeRefusesLengthTokenAndNestedMetadataConfusion() throws {
    let valid = try bound(1, payload: Data([7]))
    for body in [Data(valid.bytes.dropLast()), valid.bytes + Data([0]), Data(repeating: 1, count: 10_029)] {
      XCTAssertThrowsError(try KagemushaWalletBoundRequestV1(.init(status: 66,
        sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: body)))
    }
    for result in [reply(valid.bytes, status:66, token:1), reply(valid.bytes,status:66,token:0,high:1),
      reply(valid.bytes,status:66,token:0,detail:1)] {
      XCTAssertThrowsError(try result.decision(origin:NSObject(),requestID:Data(repeating:1,count:32),expected:.send))
    }
    for kind: UInt32 in [1,2,3,4,5,10] {
      let bytes = [1,10].contains(kind) ? Data([1]) : Data()
      XCTAssertThrowsError(try KagemushaWalletBoundRequestV1(bound(kind,low:1,payload:bytes)))
      XCTAssertThrowsError(try KagemushaWalletReviewedExecutionResultV1(.init(status:Int32(kind),
        sequenceLow:1,sequenceHigh:0,detail:0,bytes:bytes)))
      XCTAssertThrowsError(try KagemushaWalletReviewedExecutionResultV1(.init(status:Int32(kind),
        sequenceLow:0,sequenceHigh:0,detail:1,bytes:bytes)))
    }
  }

}

// A test fixture unwraps only a real decoder Ready branch; no production compatibility API.
extension KagemushaWalletReviewReplyV1 {
  func readyFixture(origin: AnyObject, expected: KagemushaWalletReviewProjectionV1.Kind,
    requestID: Data = Data(repeating: 1, count: 32)) throws -> KagemushaWalletReviewV1 {
    guard case .ready(let review) = try decision(origin: origin, requestID: requestID, expected: expected)
    else { throw KagemushaWalletErrorV1.invalidNativeOutput }
    return review
  }
}
