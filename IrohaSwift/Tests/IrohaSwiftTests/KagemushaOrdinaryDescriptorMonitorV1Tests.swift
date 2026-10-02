import Foundation
import XCTest
@testable import IrohaSwift

/// Scripted monitor failure controls, never a qualified Native or financial positive.
final class KagemushaOrdinaryDescriptorMonitorV1Tests: XCTestCase {
  func testNewTransportFailureRevokesSameDescriptorBeforeTeardownEvenIfCloseFails() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/private/fixture", endpoint: endpoint)
    var calls = 0
    XCTAssertThrowsError(try bridge.withOrdinaryDescriptor { handle -> Void in
      XCTAssertEqual(handle, 41); calls += 1; throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-312)
    }) { XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .nativeFailure(-312)) }
    XCTAssertEqual(endpoint.closed, [41])
    XCTAssertThrowsError(try bridge.requireOrdinaryDescriptorOpen())
    XCTAssertThrowsError(try bridge.withOrdinaryDescriptor { _ in calls += 1 })
    XCTAssertEqual(calls, 1)
    try bridge.close(); XCTAssertEqual(endpoint.closed, [41])
  }
  func testExistingCloseAlsoRevokesNewInternalTransportWithoutDispatch() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/private/fixture", endpoint: endpoint)
    try? bridge.close()
    var called = false
    XCTAssertThrowsError(try bridge.withOrdinaryDescriptor { _ in called = true })
    XCTAssertFalse(called); XCTAssertEqual(endpoint.closed, [41])
  }
  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var closed = [UInt64]()
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) throws -> UInt64 { 41 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeMintFunding(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func close(handle: UInt64) throws { closed.append(handle); throw KagemushaCoreCoordinatorErrorV1.unavailable }
  }
}
