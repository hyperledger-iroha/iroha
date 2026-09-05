import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCoreCoordinatorBridgeV1Tests: XCTestCase {
  func testTransportCorrelatesCallerIdentity() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    let id = Data(repeating: 7, count: 32)
    let fields = [KagemushaCoreCoordinatorFrameV1.u32(22), id, Data([1])]
    XCTAssertEqual(try bridge.invoke(.reserveOperationID, fields: fields), [id])
    endpoint.substituteResponse = true
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: fields))
  }

  func testMismatchedContractAndMissingBackendStayUnavailable() throws {
    let mismatch = Endpoint()
    mismatch.contractWords[0] = 1
    XCTAssertThrowsError(try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: mismatch))
    XCTAssertEqual(mismatch.openCalls, 0)
    let missing = Endpoint()
    missing.returnedHandle = 0
    XCTAssertThrowsError(try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: missing))
  }

  func testInvalidPathsAndRequestsDoNotReachNative() throws {
    let endpoint = Endpoint()
    for path in ["", " ", "nul\0path", String(repeating: "x", count: 4097)] {
      XCTAssertThrowsError(try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: path, endpoint: endpoint))
    }
    XCTAssertEqual(endpoint.openCalls, 0)
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/🔒", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: []))
    XCTAssertEqual(endpoint.invokeCalls, 0)
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var contractWords: [UInt32] = [2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff]
    var returnedHandle = UInt64.max
    var openCalls = 0
    var invokeCalls = 0
    var substituteResponse = false
    func contract() throws -> [UInt32] { contractWords }
    func open(storagePath: Data) throws -> UInt64 { openCalls += 1; return returnedHandle }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      invokeCalls += 1
      XCTAssertEqual(handle, returnedHandle)
      XCTAssertEqual(method, 1)
      let fields = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.reserveOperationID, frame: request)
      var response = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.reserveOperationID, requestFrame: request, fields: [fields[1]])
      if substituteResponse { response[20] = 8 }
      return response
    }
  }
}
