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

  func testTypedRecoveredAdapterSendsExactDualProofsAndSameAttemptCancellation() throws {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    var fixture: [String: Any]?
    while directory.path != "/" {
      let path = directory.appendingPathComponent("fixtures/offline/kagemusha_enrolled_open_challenge_v1.json")
      if FileManager.default.fileExists(atPath: path.path) {
        fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: path)) as? [String: Any]; break
      }
      directory.deleteLastPathComponent()
    }
    let root = try XCTUnwrap(fixture)
    let canonical = try XCTUnwrap(Data(hexString: XCTUnwrap(root["recovery_challenge_canonical_hex"] as? String)))
    let challenge = try KagemushaEnrolledOpenAccountChallengeV1.decodeCanonicalExact(canonical)
    let signature = try XCTUnwrap(Data(hexString: XCTUnwrap(root["recovery_account_signature_hex"] as? String)))
    let original = Data("IKGMJRS1-scripted-original-response".utf8)
    let ticket = Data([7, 0, 0, 0, 0, 0, 0, 0])
    let endpoint = Endpoint()
    var requests: [[Data]] = []
    var completed = false
    endpoint.scriptedInvoke = { method, frame in
      XCTAssertEqual(method, 12)
      let fields = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.initialEnrollment, frame: frame)
      requests.append(fields)
      let response: [Data]
      switch fields[0] {
      case KagemushaCoreCoordinatorFrameV1.u32(9):
        response = [ticket, canonical, challenge.accountSigningMessage(),
          try KagemushaDeviceOperationCodecV1.encodeControlCommand(.readActiveHardwareCredential), challenge.nonce]
      case KagemushaCoreCoordinatorFrameV1.u32(10):
        XCTAssertEqual(fields, [KagemushaCoreCoordinatorFrameV1.u32(10), ticket, signature, original])
        completed = true; response = [ticket]
      case KagemushaCoreCoordinatorFrameV1.u32(11):
        XCTAssertFalse(completed, "Only an outstanding native attempt can use phase11")
        XCTAssertEqual(fields, [KagemushaCoreCoordinatorFrameV1.u32(11), ticket]); response = []
      default: throw KagemushaCoreCoordinatorErrorV1.unavailable
      }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment, requestFrame: frame, fields: response)
    }
    let adapter = KagemushaNativeCoreCoordinatorAdapterV1(bridge:
      try .openEndpoint(storagePath: "/durable/store", endpoint: endpoint))
    let attempt = try adapter.beginEnrolledRecovery()
    try adapter.completeEnrolledRecovery(attempt, accountSignature: signature, originalDeviceResponse: original)
    XCTAssertEqual(requests.count, 2); XCTAssertEqual(endpoint.closeCalls, 0)
    try adapter.close(); XCTAssertEqual(endpoint.closeCalls, 1)

    // Completed leases retire their original handle. Cancellation phase11 belongs
    // only to a fresh outstanding challenge, matching the canonical native owner.
    completed = false
    let pendingEndpoint = Endpoint()
    pendingEndpoint.scriptedInvoke = endpoint.scriptedInvoke
    let pending = KagemushaNativeCoreCoordinatorAdapterV1(bridge:
      try .openEndpoint(storagePath: "/durable/store", endpoint: pendingEndpoint))
    let outstanding = try pending.beginEnrolledRecovery()
    try pending.cancelEnrolledRecovery(outstanding)
    XCTAssertEqual(requests.count, 4); XCTAssertEqual(pendingEndpoint.closeCalls, 0)
    try pending.close(); XCTAssertEqual(pendingEndpoint.closeCalls, 1)
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var contractWords: [UInt32] = [2, 25, 3, 6, 50, 8, 6, 22, 16, 0xffff, 1, 18]
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
    var scriptedInvoke: ((UInt8, Data) throws -> Data)?
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
      if let scriptedInvoke { return try scriptedInvoke(method, request) }
      XCTAssertEqual(method, 1)
      if failInvoke { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("scripted dispatch failure") }
      let fields = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.reserveOperationID, frame: request)
      var response = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.reserveOperationID, requestFrame: request, fields: [fields[1]])
      if substituteResponse { response[20] = 8 }
      return response
    }
  }
}
