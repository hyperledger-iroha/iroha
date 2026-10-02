import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

/// Scripted DATA controls exercise the real SDK holder; they qualify no Native/device custody.
final class KagemushaNativeCompletedAppKeyOriginalV1Tests: XCTestCase {
  func testSameDescriptorReadsCompletedAppleOriginalsWithoutEnrollmentOrSigning() throws {
    let endpoint = try Endpoint()
    let bridge = try open(endpoint)
    let adapter = KagemushaNativeCoreCoordinatorAdapterV1(bridge: bridge)
    let original = try adapter.completedAppKeyOriginal()
    try original.requireForCoordinator(bridge)
    let metadata = try original.metadata()
    XCTAssertEqual(metadata.keyReference, endpoint.fields[4].base64EncodedString())
    XCTAssertEqual(metadata.publicKey, endpoint.fields[3])
    XCTAssertEqual(metadata.credentialOriginal, endpoint.fields[6])
    XCTAssertEqual(metadata.financialCertificateOriginal, endpoint.fields[7])
    XCTAssertEqual(metadata.credentialDigest, endpoint.fields[10])
    XCTAssertEqual(metadata.androidSecurityMask, 0)
    XCTAssertEqual(metadata.cloudProjectNumber, 0)
    XCTAssertTrue(metadata.integrityPolicyOriginal.isEmpty)
    XCTAssertEqual(endpoint.readCalls, 5)
    XCTAssertEqual(endpoint.unrelatedCalls, 0)
    try adapter.close()
    XCTAssertThrowsError(try original.metadata())
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testEveryOriginalSubstitutionPermanentlyFreezesTheSameHolder() throws {
    for index in 0..<11 {
      let endpoint = try Endpoint()
      let bridge = try open(endpoint)
      let original = try bridge.completedAppKeyOriginal()
      let retained = endpoint.fields
      endpoint.fields[index].append(1)
      XCTAssertThrowsError(try original.requireOriginal())
      XCTAssertEqual(endpoint.closeCalls, 1)
      endpoint.fields = retained
      let calls = endpoint.readCalls
      XCTAssertThrowsError(try original.metadata())
      XCTAssertThrowsError(try bridge.completedAppKeyOriginal())
      XCTAssertEqual(endpoint.readCalls, calls)
      try bridge.close()
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testSecondReadMustMatchAndForeignBridgeCannotRebindOriginal() throws {
    let endpoint = try Endpoint()
    endpoint.substituteSecondRead = true
    let bridge = try open(endpoint)
    XCTAssertThrowsError(try bridge.completedAppKeyOriginal())
    XCTAssertEqual(endpoint.closeCalls, 1)
    let first = try Endpoint(), second = try Endpoint()
    let owner = try open(first), other = try open(second)
    let original = try owner.completedAppKeyOriginal()
    XCTAssertThrowsError(try original.requireForCoordinator(other))
    try original.requireForCoordinator(owner)
    XCTAssertEqual(second.readCalls, 0)
    XCTAssertEqual(first.closeCalls, 0)
    try owner.close(); try other.close()
  }

  func testSoleCanonicalResponseRejectsCorrelationTrailingAndPolicyChanges() throws {
    let endpoint = try Endpoint()
    let frame = try KagemushaCompletedAppKeyFrameV1.encodeResponse(handle: 7, fields: endpoint.fields)
    XCTAssertEqual(try KagemushaCompletedAppKeyFrameV1.decodeResponse(handle: 7, response: frame), endpoint.fields)
    XCTAssertThrowsError(try KagemushaCompletedAppKeyFrameV1.decodeResponse(handle: 8, response: frame))
    XCTAssertThrowsError(try KagemushaCompletedAppKeyFrameV1.decodeResponse(handle: 7, response: frame + Data([0])))
    for fields in [Array(endpoint.fields.dropLast()), endpoint.fields + [Data()]] {
      XCTAssertThrowsError(try KagemushaCompletedAppKeyFrameV1.encodeResponse(handle: 7, fields: fields))
    }
    var fields = endpoint.fields
    fields[8] = Data([1])
    XCTAssertThrowsError(try KagemushaCompletedAppKeyFrameV1.encodeResponse(handle: 7, fields: fields))
    fields[5] = Data([3]); fields[9] = CompactNorito.encodeUInt64(1)
    let android = try KagemushaCompletedAppKeyFrameV1.encodeResponse(handle: 7, fields: fields)
    XCTAssertEqual(try KagemushaCompletedAppKeyFrameV1.decodeResponse(handle: 7, response: android), fields)
    fields[6] = Data(repeating: 1, count: 65_536)
    fields[7] = Data(repeating: 1, count: 65_536)
    XCTAssertThrowsError(try KagemushaCompletedAppKeyFrameV1.encodeResponse(handle: 7, fields: fields))
  }

  func testNativeFailureSurvivesFailedTeardownAndCannotBeRetried() throws {
    let endpoint = try Endpoint()
    let bridge = try open(endpoint)
    let original = try bridge.completedAppKeyOriginal()
    endpoint.failRead = true; endpoint.failClose = true
    XCTAssertThrowsError(try original.requireOriginal()) {
      XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .nativeFailure(-310))
    }
    endpoint.failRead = false
    XCTAssertThrowsError(try original.metadata())
    XCTAssertEqual(endpoint.readCalls, 3)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  private func open(_ endpoint: Endpoint) throws -> KagemushaCoreCoordinatorBridgeV1 {
    try .openEndpoint(storagePath: "/tmp/scripted-completed-key", endpoint: endpoint)
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var fields: [Data]
    var readCalls = 0, unrelatedCalls = 0, closeCalls = 0
    var substituteSecondRead = false, failRead = false, failClose = false
    init() throws {
      let key = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 1, count: 32))
      let id = Data(repeating: 3, count: 32)
      fields = [Data(repeating: 1, count: 32), Data(id.base64EncodedString().utf8),
        Data(repeating: 2, count: 32), key.publicKey.x963Representation, id, Data([0]),
        Data([6]), Data([7]), Data(), CompactNorito.encodeUInt64(0), Data(repeating: 4, count: 32)]
    }
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) throws -> UInt64 { 7 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      unrelatedCalls += 1
      throw KagemushaCoreCoordinatorErrorV1.unavailable
    }
    func invokeMintFunding(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data {
      unrelatedCalls += 1
      throw KagemushaCoreCoordinatorErrorV1.unavailable
    }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data {
      readCalls += 1
      guard phase == 10, handle == 7, original.isEmpty else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("script expected input-free completed-key10")
      }
      if failRead { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-310) }
      var response = fields
      if substituteSecondRead && readCalls == 2 { response[7] = Data([8]) }
      return try KagemushaCompletedAppKeyFrameV1.encodeResponse(handle: handle, fields: response)
    }
    func close(handle: UInt64) throws {
      closeCalls += 1
      if failClose { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-309) }
    }
  }
}
