import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

/// Scripted transport/canonical framing tests. No proof, hardware or release qualification.
final class KagemushaOrdinaryIncomingFrameV1Tests: XCTestCase {
  private let digest = Data(repeating: 7, count: 32)
  private let prepared = [Data(repeating: 7, count: 32), Data(repeating: 1, count: 325),
    Data(repeating: 2, count: 460), Data([3])]

  func testAllClosedPhaseResponseShapesAndExactCorrelation() throws {
    for phase in KagemushaOrdinaryIncomingPhaseV1.allCases {
      let fields: [Data]
      switch phase {
      case .prepareFinalizedMint, .prepareReceive, .prepareTerminal: fields = prepared
      case .fencePreparation, .recoverPreparation, .fenceTerminal, .recoverTerminal:
        fields = [Data([0]), Data(), Data()]
      case .retainPreparationAssertion, .proveCandidate, .retainGlobalResult,
        .retainTerminalAssertion, .proveCommit: fields = [digest]
      case .reserveTransport, .commitTransport:
        let body = Data([11, 12]); fields = [Data([0]), body, Data(repeating: 1, count: 64),
          Data([8]), Data(SHA256.hash(data: body))]
      case .originalPlatformCounter: fields = [Data([4]), Data([0, 0, 0, 0])]
      case .advanceState, .acknowledge, .refreshAccountClock: fields = []
      }
      let frame = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(phase, handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(phase, handle: 19,
        response: frame), fields)
      XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(phase,
        handle: 20, response: frame))
    }
  }

  func testFullOriginalTransportExceedsSmallCoordinatorCeiling() throws {
    let fullFinalized = Data(repeating: 9, count: 300_000)
    let frame = try KagemushaOrdinaryIncomingFrameV1.encodeRequest(.prepareFinalizedMint,
      handle: 19, originals: [fullFinalized, Data([8])])
    XCTAssertGreaterThan(frame.count, KagemushaCoreCoordinatorFrameV1.maximumRequestBytes)
    let receive = try KagemushaOrdinaryIncomingFrameV1.encodeRequest(.prepareReceive,
      handle: 19, originals: [digest, fullFinalized, fullFinalized])
    XCTAssertGreaterThan(receive.count, 600_000)
  }

  func testMalformedRequestDoesNotDispatchOrCloseRetainedOwner() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.prepareReceive,
      originals: [Data(repeating: 0, count: 32), Data([1]), Data([2])]))
    XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.refreshAccountClock,
      originals: [Data([1])]))
    XCTAssertEqual(endpoint.invocations, 0)
    XCTAssertEqual(endpoint.closes, 0)
    endpoint.response = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.refreshAccountClock,
      handle: 19, fields: [])
    XCTAssertEqual(try bridge.invokeOrdinaryIncoming(.refreshAccountClock), [])
    XCTAssertEqual(endpoint.invocations, 1)
  }

  func testSubstitutedPhaseFreezesOwnerBeforeTeardown() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    endpoint.response = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.acknowledge,
      handle: 19, fields: [])
    XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.refreshAccountClock))
    XCTAssertEqual(endpoint.closes, 1)
    XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.refreshAccountClock))
    XCTAssertEqual(endpoint.invocations, 1)
  }

  func testBadCanonicalEnvelopeAndTrailingPayloadRefuse() throws {
    let frame = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.refreshAccountClock,
      handle: 19, fields: [])
    var corrupted = frame; corrupted[31] ^= 1
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.refreshAccountClock,
      handle: 19, response: corrupted))
    var retired = frame; retired[39] = 0
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.refreshAccountClock,
      handle: 19, response: retired))
    var payload = try XCTUnwrap(noritoDecodeFrame(frame)).payload; payload.append(0)
    let trailing = noritoEncode(typeName: "connect_norito_bridge::KagemushaOrdinaryNativeIncomingResponseV1",
      payload: payload, flags: NoritoHeader.compactLen, payloadAlignment: 8)
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.refreshAccountClock,
      handle: 19, response: trailing))
  }

  func testSignedTransportRetainsWholeRequestDigestAndRejectsStatusSubstitution() throws {
    let body = Data([4, 5]); let fields = [Data([2]), body, Data(repeating: 1, count: 64),
      Data(repeating: 2, count: 300_000), Data(SHA256.hash(data: body))]
    let frame = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.commitTransport,
      handle: 19, fields: fields)
    XCTAssertEqual(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.commitTransport,
      handle: 19, response: frame), fields)
    var wrong = fields; wrong[4] = digest
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.commitTransport,
      handle: 19, fields: wrong))
    wrong = fields; wrong[0] = Data([1])
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.reserveTransport,
      handle: 19, fields: wrong))
  }

  func testNativeFailurePreservesErrorAndFreezesEvenIfCloseFails() throws {
    let endpoint = Endpoint(); endpoint.failure = .nativeFailure(-310)
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.fencePreparation)) {
      XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .nativeFailure(-310))
    }
    XCTAssertEqual(endpoint.closes, 1)
    XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.fencePreparation))
    XCTAssertEqual(endpoint.invocations, 1)
  }

  func testPlatformCounterIsAppleOnlyAndUsesItsFullUInt32Original() throws {
    for fields in [[Data([4]), Data([0, 0, 0, 0])], [Data([4]), Data([255, 255, 255, 255])], [Data([5]), Data()]] {
      let frame = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.originalPlatformCounter,
        handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryIncomingFrameV1.decodeResponse(.originalPlatformCounter,
        handle: 19, response: frame), fields)
    }
    for fields in [[Data([4]), Data()], [Data([5]), Data([0, 0, 0, 0])], [Data([4]), Data(repeating: 0, count: 16)]] {
      XCTAssertThrowsError(try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.originalPlatformCounter,
        handle: 19, fields: fields))
    }
  }

  func testCounterReadRejectsOfferedPurposeOrUnknownOperationBeforeDispatch() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    for originals in [[digest, Data([0])], [Data(repeating: 0, count: 32), Data([2])], [digest, Data(repeating: 1, count: 4)]] {
      XCTAssertThrowsError(try bridge.invokeOrdinaryIncoming(.originalPlatformCounter, originals: originals))
    }
    XCTAssertEqual(endpoint.invocations, 0)
    XCTAssertEqual(endpoint.closes, 0)
    endpoint.response = try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.originalPlatformCounter,
      handle: 19, fields: [Data([4]), Data([255, 255, 255, 255])])
    XCTAssertEqual(try bridge.invokeOrdinaryIncoming(.originalPlatformCounter,
      originals: [digest, Data([2])]), [Data([4]), Data([255, 255, 255, 255])])
    XCTAssertEqual(endpoint.invocations, 1)
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var response = Data()
    var failure: KagemushaCoreCoordinatorErrorV1?
    var invocations = 0, closes = 0
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) throws -> UInt64 { 19 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      throw KagemushaCoreCoordinatorErrorV1.unavailable
    }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data {
      invocations += 1
      if let failure { throw failure }
      return response
    }
    func close(handle: UInt64) throws {
      closes += 1
      if failure != nil { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-311) }
    }
  }
}
