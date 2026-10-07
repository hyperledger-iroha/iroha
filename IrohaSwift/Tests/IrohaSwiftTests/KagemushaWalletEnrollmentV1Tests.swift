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
    for selector:UInt32 in [0,1,2,4,5,6,7,8,9,10] {
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
  func testAppleExplicitlyRefusesAndroidTeeProfile() {
    XCTAssertNil(KagemushaWalletAppleKeyProfileV1(rawValue:3))
  }
}
