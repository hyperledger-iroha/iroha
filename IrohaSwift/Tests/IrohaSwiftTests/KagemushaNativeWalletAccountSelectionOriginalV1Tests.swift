// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import XCTest
@testable import IrohaSwift

/// Scripted transport controls exercise the real typed SDK consumer and exact correlation.
/// Fixture DATA is not a Native owner, canonical multisig admission or monetary provider.
final class KagemushaNativeWalletAccountSelectionOriginalV1Tests: XCTestCase {
  func testPublicBridgeHolderRechecksExactOriginalOwnerAndNeverReservesOrSigns() throws {
    let endpoint = Endpoint()
    let bridge = try open(endpoint)
    let selection = try bridge.currentWalletAccountSelection()
    XCTAssertEqual(endpoint.invokeCalls, 2)
    XCTAssertEqual(try selection.walletAccountId(), "fixture-wallet")
    XCTAssertEqual(try selection.signatoryAccountId(), "fixture-member")
    try selection.requireCurrent()
    try selection.requireForCoordinator(bridge)
    XCTAssertEqual(endpoint.invokeCalls, 8)
    XCTAssertEqual(endpoint.methods, Array(repeating: UInt8(21), count: 8))
    XCTAssertEqual(endpoint.requests, Array(repeating: [n(15)], count: 8))
    XCTAssertEqual(endpoint.closeCalls, 0)
    try bridge.close()
  }

  func testPublicAdapterForwardsToSameOpaqueNativeHolder() throws {
    let endpoint = Endpoint()
    let bridge = try open(endpoint)
    let adapter = KagemushaNativeCoreCoordinatorAdapterV1(bridge: bridge)
    let selection = try adapter.currentWalletAccountSelection()
    try selection.requireForCoordinator(bridge)
    XCTAssertEqual(try selection.walletAccountId(), "fixture-wallet")
    XCTAssertEqual(endpoint.methods, Array(repeating: UInt8(21), count: 5))
    try adapter.close()
    XCTAssertThrowsError(try selection.requireCurrent())
    XCTAssertEqual(endpoint.invokeCalls, 5)
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testEachSessionWalletAndSignatorySubstitutionFreezesOriginalAndClosesOnce() throws {
    for index in 0...2 {
      let endpoint = Endpoint()
      let bridge = try open(endpoint)
      let selection = try bridge.currentWalletAccountSelection()
      endpoint.fields[index] = index == 0
        ? Data([2] + [UInt8](repeating: 0, count: 7))
        : Data("substituted-account-\(index)".utf8)
      XCTAssertThrowsError(try selection.requireCurrent())
      XCTAssertEqual(endpoint.closeCalls, 1)
      let invocations = endpoint.invokeCalls
      endpoint.fields = Endpoint.originalFields
      XCTAssertThrowsError(try selection.walletAccountId())
      XCTAssertThrowsError(try selection.signatoryAccountId())
      XCTAssertThrowsError(try selection.requireCurrent())
      XCTAssertThrowsError(try bridge.currentWalletAccountSelection())
      XCTAssertEqual(endpoint.invokeCalls, invocations)
      try bridge.close()
      XCTAssertEqual(endpoint.closeCalls, 1)
    }
  }

  func testSecondNativeReadMustMatchBeforeHolderCanEscape() throws {
    let endpoint = Endpoint()
    endpoint.secondReadSubstitution = true
    let bridge = try open(endpoint)
    XCTAssertThrowsError(try bridge.currentWalletAccountSelection())
    XCTAssertEqual(endpoint.invokeCalls, 2)
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertThrowsError(try bridge.currentWalletAccountSelection())
    XCTAssertEqual(endpoint.invokeCalls, 2)
  }

  func testDifferentBridgeCannotRebindOriginalSelection() throws {
    let endpoint = Endpoint(), otherEndpoint = Endpoint()
    let bridge = try open(endpoint), other = try open(otherEndpoint)
    let selection = try bridge.currentWalletAccountSelection()
    XCTAssertThrowsError(try selection.requireForCoordinator(other))
    XCTAssertEqual(endpoint.invokeCalls, 2)
    XCTAssertEqual(otherEndpoint.invokeCalls, 0)
    try selection.requireForCoordinator(bridge)
    XCTAssertEqual(endpoint.invokeCalls, 3)
    try bridge.close(); try other.close()
  }

  func testUnavailableNativeReadFreezesEvenWhenTeardownFails() throws {
    let endpoint = Endpoint()
    let bridge = try open(endpoint)
    let selection = try bridge.currentWalletAccountSelection()
    endpoint.failInvoke = true
    endpoint.failClose = true
    XCTAssertThrowsError(try selection.requireCurrent()) { error in
      XCTAssertEqual(error as? KagemushaCoreCoordinatorErrorV1, .unavailable)
    }
    XCTAssertEqual(endpoint.closeCalls, 1)
    endpoint.failInvoke = false
    XCTAssertThrowsError(try selection.requireCurrent())
    XCTAssertThrowsError(try bridge.currentWalletAccountSelection())
    XCTAssertEqual(endpoint.invokeCalls, 3)
    try bridge.close()
    XCTAssertEqual(endpoint.closeCalls, 1)
  }

  func testMalformedNativeAccountResponseRevokesBeforeAnyTypedHolder() throws {
    let endpoint = Endpoint()
    endpoint.fields[0] = Data(repeating: 0, count: 8)
    let bridge = try open(endpoint)
    XCTAssertThrowsError(try bridge.currentWalletAccountSelection())
    XCTAssertEqual(endpoint.invokeCalls, 1)
    XCTAssertEqual(endpoint.closeCalls, 1)
    XCTAssertThrowsError(try bridge.currentWalletAccountSelection())
    XCTAssertEqual(endpoint.invokeCalls, 1)
  }

  private func open(_ endpoint: Endpoint) throws -> KagemushaCoreCoordinatorBridgeV1 {
    try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
  }
  private func n(_ value: UInt32) -> Data { KagemushaCoreCoordinatorFrameV1.u32(value) }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    static var originalFields: [Data] {
      [Data([1] + [UInt8](repeating: 0, count: 7)), Data("fixture-wallet".utf8), Data("fixture-member".utf8)]
    }
    var fields = Endpoint.originalFields
    var invokeCalls = 0, closeCalls = 0
    var methods: [UInt8] = [], requests: [[Data]] = []
    var secondReadSubstitution = false, failInvoke = false, failClose = false
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) throws -> UInt64 { 7 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      invokeCalls += 1
      guard handle == 7, method == KagemushaCoreCoordinatorMethodV1.preparedOrdinaryAppIdentity.rawValue else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("fixture expected only current account method21")
      }
      let decoded = try KagemushaCoreCoordinatorFrameV1.decodeRequest(.preparedOrdinaryAppIdentity, frame: request)
      methods.append(method); requests.append(decoded)
      guard decoded == [KagemushaCoreCoordinatorFrameV1.u32(15)] else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("fixture expected only input-free account phase15")
      }
      if failInvoke { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      var response = fields
      if secondReadSubstitution && invokeCalls == 2 { response[2] = Data("substituted-member".utf8) }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.preparedOrdinaryAppIdentity,
        requestFrame: request, fields: response)
    }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeMintFunding(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIncoming(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func close(handle: UInt64) throws {
      closeCalls += 1
      if failClose { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-310) }
    }
  }
}
