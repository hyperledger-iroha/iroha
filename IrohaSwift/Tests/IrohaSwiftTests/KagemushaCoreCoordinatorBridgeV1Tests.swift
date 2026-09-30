import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCoreCoordinatorBridgeV1Tests: XCTestCase {
  func testInstallsOnlyExactStoragePathBeforeOpening() throws {
    let endpoint = Endpoint()
    let path = "/durable/🔒"
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: path, endpoint: endpoint)
    XCTAssertEqual(endpoint.events, ["install", "open"])
    XCTAssertEqual(endpoint.installedPaths, [Data(path.utf8)])
    XCTAssertEqual(endpoint.openedPaths, endpoint.installedPaths)
    try bridge.close()
  }

  func testFailedNativeInstallationCannotOpenOrGrantHandle() {
    for error in [KagemushaCoreCoordinatorErrorV1.unavailable, .nativeFailure(-310)] {
      let endpoint = Endpoint()
      endpoint.installFailure = error
      XCTAssertThrowsError(try KagemushaCoreCoordinatorBridgeV1.openEndpoint(
        storagePath: "/durable/store", endpoint: endpoint)) { actual in
        XCTAssertEqual(actual as? KagemushaCoreCoordinatorErrorV1, error)
      }
      XCTAssertEqual(endpoint.events, ["install"])
      XCTAssertEqual(endpoint.openCalls, 0)
      XCTAssertEqual(endpoint.closeCalls, 0)
    }
  }

  func testRetiredNativeABIIsRejectedBeforeInstallation() {
    let endpoint = Endpoint()
    endpoint.contractWords[1] = 23
    XCTAssertThrowsError(try KagemushaCoreCoordinatorBridgeV1.openEndpoint(
      storagePath: "/durable/store", endpoint: endpoint))
    XCTAssertEqual(endpoint.events, [])
  }

  func testTransportCorrelatesCallerIdentity() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    let id = Data(repeating: 7, count: 32)
    let fields = [KagemushaCoreCoordinatorFrameV1.u32(22), id, Data([1])]
    XCTAssertEqual(try bridge.invoke(.reserveOperationID, fields: fields), [id])
    endpoint.substituteResponse = true
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: fields))
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: fields))
    XCTAssertEqual(endpoint.invokeCalls, 2)
    try bridge.close()
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testMismatchedContractAndMissingBackendStayUnavailable() throws {
    let mismatch = Endpoint()
    mismatch.contractWords[0] = 1
    XCTAssertThrowsError(try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: mismatch))
    XCTAssertEqual(mismatch.openCalls, 0)
    XCTAssertEqual(mismatch.installedPaths, [])
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
    XCTAssertEqual(endpoint.installedPaths, [])
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/🔒", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: []))
    XCTAssertEqual(endpoint.invokeCalls, 0)
    XCTAssertEqual(endpoint.closeCalls, 0)
  }

  func testUncertainDispatchRevokesBeforeFailedTeardownAndPreservesOriginalError() throws {
    let endpoint = Endpoint()
    endpoint.failInvoke = true
    endpoint.failClose = true
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    let fields = [KagemushaCoreCoordinatorFrameV1.u32(22), Data(repeating: 7, count: 32), Data([1])]
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: fields)) { error in
      guard let failure = error as? KagemushaCoreCoordinatorErrorV1,
        case .invalidFrame(let reason) = failure else {
        return XCTFail("teardown must preserve the original uncertain dispatch error")
      }
      XCTAssertEqual(reason, "scripted dispatch failure")
    }
    XCTAssertEqual(endpoint.invokeCalls, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID, fields: fields))
    try bridge.close()
    XCTAssertEqual(endpoint.invokeCalls, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testCloseRevokesLocallyAndCallsNativeOnce() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    try bridge.close()
    try bridge.close()
    XCTAssertEqual(endpoint.closeCalls, 1)
    let id = Data(repeating: 7, count: 32)
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(22), id, Data([1])]))
    XCTAssertEqual(endpoint.invokeCalls, 0)
  }

  func testFailedNativeTeardownStillRevokesLocalHandle() throws {
    let endpoint = Endpoint()
    endpoint.failClose = true
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.close())
    XCTAssertThrowsError(try bridge.invoke(.reserveOperationID,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(22), Data(repeating: 7, count: 32), Data([1])]))
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertEqual(endpoint.invokeCalls, 0)
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var contractWords: [UInt32] = [2, 25, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 14]
    var returnedHandle = UInt64.max
    var openCalls = 0
    var invokeCalls = 0
    var closeCalls = 0
    var failClose = false
    var failInvoke = false
    var substituteResponse = false
    var installFailure: KagemushaCoreCoordinatorErrorV1?
    var installedPaths: [Data] = []
    var openedPaths: [Data] = []
    var events: [String] = []
    func contract() throws -> [UInt32] { contractWords }
    func install(storagePath: Data) throws {
      events.append("install")
      installedPaths.append(storagePath)
      if let installFailure { throw installFailure }
    }
    func open(storagePath: Data) throws -> UInt64 {
      events.append("open"); openedPaths.append(storagePath)
      openCalls += 1; return returnedHandle
    }
    func close(handle: UInt64) throws {
      closeCalls += 1; XCTAssertEqual(handle, returnedHandle)
      if failClose { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      invokeCalls += 1
      XCTAssertEqual(handle, returnedHandle)
      XCTAssertEqual(method, 1)
      if failInvoke { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("scripted dispatch failure") }
      let fields = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.reserveOperationID, frame: request)
      var response = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.reserveOperationID, requestFrame: request, fields: [fields[1]])
      if substituteResponse { response[20] = 8 }
      return response
    }
  }
}
