import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaWalletEnrollmentV1Tests: XCTestCase {
  func testEnrollmentOutputBoundsAndShapesKeepPaymentLimit() throws {
    for (status,count) in [(18,32),(19,161),(20,0),(21,0),(22,0),(23,32),(24,131072),(25,262144),(26,0),(27,16384),(28,1024)] {
      let reply = try KagemushaWalletCallV1(status:Int32(status),sequenceLow:7,sequenceHigh:0,detail:0,bytes:Data(repeating:5,count:count))
      XCTAssertEqual(reply.bytes.count,count)
      XCTAssertThrowsError(try reply.completion())
      XCTAssertThrowsError(try KagemushaWalletCallV1(status:Int32(status),sequenceLow:0,sequenceHigh:0,detail:0,bytes:reply.bytes))
    }
    XCTAssertThrowsError(try KagemushaWalletCallV1(status:1,sequenceLow:0,sequenceHigh:0,detail:0,bytes:Data(count:10001)))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status:25,sequenceLow:7,sequenceHigh:0,detail:0,bytes:Data(count:262145)))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status:19,sequenceLow:7,sequenceHigh:0,detail:0,bytes:Data(count:160)))
  }
  func testEnrollmentInputsAreBoundedAndKeepOnlyOriginals() throws {
    for selector:UInt32 in [0,1,2,4,5,6,7,8,9,10,11,12] {
      XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(selector,Data(count:262145)))
    }
    XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(3))
    XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(4,Data(count:32),Data(count:65536),Data(count:4097)))
    let input = try KagemushaWalletEnrollmentInputV1(4,Data(repeating:1,count:32),Data([2]),Data([3]))
    input.withRequest { request in
      XCTAssertEqual(request.pointee.selector,4)
      XCTAssertEqual(request.pointee.first_length,32)
      XCTAssertNil(request.pointee.certificates)
      XCTAssertEqual(request.pointee.certificate_count,0)
    }
    let target = try KagemushaWalletEnrollmentTargetV1(Data((0..<161).map(UInt8.init)))
    XCTAssertEqual(target.slot.count,32);XCTAssertEqual(target.paymentKey.count,65)
    XCTAssertEqual(target.challengeDigest.first,97);XCTAssertEqual(target.keyBindingDigest.first,129)
  }
  func testPersistedResultIntakeRetainsOnlyExactResultAndAccountFrames() throws {
    let result = Data(repeating: 9, count: KagemushaWalletEnrollmentV1.RESULT_MAX_BYTES)
    let account = Data(repeating: 7, count: 4096)
    let input = try KagemushaWalletEnrollmentInputV1(12, result, account)
    input.withRequest { request in
      XCTAssertEqual(request.pointee.selector, 12)
      XCTAssertEqual(request.pointee.first_length, result.count)
      XCTAssertEqual(request.pointee.second_length, account.count)
      XCTAssertEqual(request.pointee.third_length, 0)
      XCTAssertEqual(request.pointee.certificate_count, 0)
    }
    XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(12, result, Data(count: 4097)))
    XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(12, result, account, Data([1])))
  }
  func testSessionTransportAllowsEmptyProofWithoutWeakeningTokenOrRootBounds() throws {
    let token = Data(repeating: 1, count: 16_384), root = Data(repeating: 2, count: 16_384)
    let bpng = try KagemushaWalletEnrollmentSessionOriginalsV1(accessToken: token, dpopProof: Data(), attestationRootDER: root)
    XCTAssertEqual(bpng.originals, [token, Data(), root])
    let proof = Data(repeating: 3, count: 4096)
    XCTAssertEqual(try KagemushaWalletEnrollmentSessionOriginalsV1(accessToken: token, dpopProof: proof, attestationRootDER: root).originals[1], proof)
    for values in [[Data(),Data(),root],[token,Data(),Data()],[token + Data([1]),Data(),root],[token,proof + Data([1]),root],[token,Data(),root + Data([1])]] {
      XCTAssertThrowsError(try KagemushaWalletEnrollmentSessionOriginalsV1(accessToken: values[0], dpopProof: values[1], attestationRootDER: values[2]))
    }
  }
  func testRetainedResultSelectorHasNoCallerOriginalsAndPreservesAppleSelectors() throws {
    for selector: UInt32 in [13,16,17,18] {
      let input = try KagemushaWalletEnrollmentInputV1(selector)
      XCTAssertEqual(input.originals, [Data(),Data(),Data()])
      XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(selector,Data([1])))
      XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(selector,Data(),Data([1])))
      XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(selector,Data(),Data(),Data([1])))
    }
    _ = try KagemushaWalletEnrollmentInputV1(14,Data([1]))
    _ = try KagemushaWalletEnrollmentInputV1(15,Data([1]),Data([2]))
    XCTAssertThrowsError(try KagemushaWalletEnrollmentInputV1(19))
  }
  func testAppleExplicitlyRefusesAndroidTeeProfile() {
    XCTAssertNil(KagemushaWalletAppleKeyProfileV1(rawValue:3))
  }
}
