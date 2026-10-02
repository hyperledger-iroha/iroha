// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import XCTest
@testable import IrohaSwift

/// Draft scripted transport controls only. Fixture W/S fields are DATA, not actual Native
/// account custody, signed release admission, enrollment or physical conformance evidence.
final class KagemushaOrdinaryRuntimeStartupV1Tests: XCTestCase {
  func testOpenFailurePreservesOriginalErrorAndDoesNotReturnOrRetryOwner() {
    var attempts = 0
    XCTAssertThrowsError(try KagemushaOrdinaryRuntimeStartupV1.openInitialAccount(
      storagePath: "/durable/store", openCoordinator: { _ in
        attempts += 1
        throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-312)
      })) { error in
        XCTAssertEqual(error as? KagemushaCoreCoordinatorErrorV1, .nativeFailure(-312))
      }
    XCTAssertEqual(attempts, 1)
  }

  func testInstallRefusalPrecedesOpenAndOriginalRead() {
    let endpoint = Endpoint()
    endpoint.failInstall = true
    XCTAssertThrowsError(try open(endpoint))
    XCTAssertEqual(endpoint.installCalls, 1)
    XCTAssertEqual(endpoint.openCalls, 0)
    XCTAssertEqual(endpoint.invokeCalls, 0)
    XCTAssertEqual(endpoint.closeCalls, 0)
  }

  func testOriginalReadFailureClosesOnceEvenWhenTeardownFails() {
    let endpoint = Endpoint()
    endpoint.failInvoke = true; endpoint.failClose = true
    XCTAssertThrowsError(try open(endpoint)) { error in
      XCTAssertEqual(error as? KagemushaCoreCoordinatorErrorV1, .unavailable)
    }
    XCTAssertEqual(endpoint.openCalls, 1)
    XCTAssertEqual(endpoint.invokeCalls, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testChangedSecondReadCannotEscapeAsSession() {
    let endpoint = Endpoint()
    endpoint.changeSecondRead = true
    XCTAssertThrowsError(try open(endpoint))
    XCTAssertEqual(endpoint.invokeCalls, 2)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testSessionRetainsSameObjectsAndReadsOnlyInputFreePhase15() throws {
    let endpoint = Endpoint()
    let session = try open(endpoint)
    let coordinator = try session.originalCoordinator()
    let selection = try session.originalAccountSelection()
    XCTAssertTrue(try session.originalCoordinator() === coordinator)
    XCTAssertTrue(try session.originalAccountSelection() === selection)
    try session.requireCurrent()
    XCTAssertEqual(endpoint.installCalls, 1)
    XCTAssertEqual(endpoint.openCalls, 1)
    XCTAssertTrue(endpoint.requests.allSatisfy { $0 == [n(15)] })
    XCTAssertTrue(endpoint.methods.allSatisfy { $0 == UInt8(21) })
    try session.close()
    let reads = endpoint.invokeCalls
    XCTAssertThrowsError(try session.originalCoordinator())
    XCTAssertThrowsError(try session.originalAccountSelection())
    XCTAssertThrowsError(try session.requireCurrent())
    XCTAssertThrowsError(try selection.requireCurrent())
    XCTAssertEqual(endpoint.invokeCalls, reads)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testSessionWalletOrMemberReplacementFreezesWithoutReplacementRead() throws {
    for index in 0...2 {
      let endpoint = Endpoint()
      let session = try open(endpoint)
      endpoint.fields[index] = index == 0
        ? Data([2] + [UInt8](repeating: 0, count: 7))
        : Data("changed-account-\(index)".utf8)
      XCTAssertThrowsError(try session.requireCurrent())
      let reads = endpoint.invokeCalls
      endpoint.fields = Endpoint.originalFields
      XCTAssertThrowsError(try session.originalCoordinator())
      XCTAssertThrowsError(try session.originalAccountSelection())
      XCTAssertEqual(endpoint.invokeCalls, reads)
      try session.close()
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testCloseFailureStillFreezesAndCannotRetryTeardownOrOpen() throws {
    let endpoint = Endpoint()
    let session = try open(endpoint)
    endpoint.failClose = true
    XCTAssertThrowsError(try session.close())
    let reads = endpoint.invokeCalls
    XCTAssertThrowsError(try session.requireCurrent())
    XCTAssertThrowsError(try session.originalCoordinator())
    XCTAssertThrowsError(try session.originalAccountSelection())
    try session.close()
    XCTAssertEqual(endpoint.invokeCalls, reads)
    XCTAssertEqual(endpoint.openCalls, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testExternalOriginalCloseFreezesSessionOnNextUse() throws {
    let endpoint = Endpoint()
    let session = try open(endpoint)
    let coordinator = try session.originalCoordinator()
    try coordinator.close()
    let reads = endpoint.invokeCalls
    XCTAssertThrowsError(try session.requireCurrent())
    XCTAssertThrowsError(try session.originalAccountSelection())
    XCTAssertEqual(endpoint.invokeCalls, reads)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testSessionLifetimeOwnsTeardownEvenWithRetainedCoordinatorReference() throws {
    let endpoint = Endpoint()
    var session: KagemushaOrdinaryNativeAccountSessionV1? = try open(endpoint)
    let coordinator = try XCTUnwrap(session).originalCoordinator()
    session = nil
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertThrowsError(try coordinator.currentWalletAccountSelection())
    XCTAssertEqual(endpoint.openCalls, 1)
  }

  func testClockRenewalRunsBeforeExpiredSelectionAndKeepsOriginalIdentity() throws {
    let endpoint = Endpoint()
    let session = try open(endpoint)
    endpoint.failInvoke = true
    try session.refreshOriginalAccountClock()
    XCTAssertEqual(endpoint.incomingCalls, 1)
    try session.requireCurrent()
    XCTAssertEqual(endpoint.fields, Endpoint.originalFields)
    XCTAssertEqual(endpoint.closeCalls, 0)
  }

  func testRenewalCannotSubstituteOriginalSelectionOrReviveClosedSession() throws {
    for index in 0...2 {
      let endpoint = Endpoint()
      let session = try open(endpoint)
      endpoint.fields[index] = index == 0
        ? Data([9] + [UInt8](repeating: 0, count: 7)) : Data("changed-after-renewal".utf8)
      XCTAssertThrowsError(try session.refreshOriginalAccountClock())
      XCTAssertEqual(endpoint.incomingCalls, 1)
      XCTAssertEqual(endpoint.closeCalls, 1)
      endpoint.fields = Endpoint.originalFields
      XCTAssertThrowsError(try session.refreshOriginalAccountClock())
      XCTAssertEqual(endpoint.incomingCalls, 1)
    }
  }

  private func open(_ endpoint: Endpoint) throws -> KagemushaOrdinaryNativeAccountSessionV1 {
    try KagemushaOrdinaryRuntimeStartupV1.openInitialAccount(storagePath: "/durable/store",
      openCoordinator: { path in
        KagemushaNativeCoreCoordinatorAdapterV1(bridge: try KagemushaCoreCoordinatorBridgeV1
          .openEndpoint(storagePath: path, endpoint: endpoint))
      })
  }
  private func n(_ value: UInt32) -> Data { KagemushaCoreCoordinatorFrameV1.u32(value) }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    static var originalFields: [Data] {
      [Data([1] + [UInt8](repeating: 0, count: 7)), Data("fixture-wallet".utf8), Data("fixture-member".utf8)]
    }
    var fields = Endpoint.originalFields
    var installCalls = 0, openCalls = 0, invokeCalls = 0, closeCalls = 0
    var failInstall = false, failInvoke = false, failClose = false, changeSecondRead = false
    var incomingCalls = 0
    var requests: [[Data]] = [], methods: [UInt8] = []
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {
      installCalls += 1
      if failInstall { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    }
    func open(storagePath: Data) throws -> UInt64 { openCalls += 1; return 7 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      invokeCalls += 1
      guard handle == 7, method == KagemushaCoreCoordinatorMethodV1.preparedOrdinaryAppIdentity.rawValue else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("fixture expects current account method21")
      }
      let decoded = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.preparedOrdinaryAppIdentity, frame: request)
      requests.append(decoded); methods.append(method)
      guard decoded == [KagemushaCoreCoordinatorFrameV1.u32(15)] else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("fixture expects only input-free phase15")
      }
      if failInvoke { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      var response = fields
      if changeSecondRead && invokeCalls == 2 { response[2] = Data("changed-member".utf8) }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.preparedOrdinaryAppIdentity,
        requestFrame: request, fields: response)
    }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data {
      incomingCalls += 1
      XCTAssertEqual(request, try KagemushaOrdinaryIncomingFrameV1.encodeRequest(
        .refreshAccountClock, handle: 7, originals: []))
      failInvoke = false
      return try KagemushaOrdinaryIncomingFrameV1.encodeResponse(.refreshAccountClock,
        handle: 7, fields: [])
    }
    func close(handle: UInt64) throws {
      closeCalls += 1
      if failClose { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-310) }
    }
  }
}
