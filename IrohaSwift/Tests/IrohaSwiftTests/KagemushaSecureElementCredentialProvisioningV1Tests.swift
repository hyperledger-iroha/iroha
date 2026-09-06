// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation
import XCTest

@testable import IrohaSwift

final class KagemushaSecureElementCredentialProvisioningV1Tests: XCTestCase {
  func testConfigurationRejectsInvalidValuesWithoutTrapping() throws {
    let product = UUID(uuidString: "00112233-4455-6677-8899-AABBCCDDEEFF")!
    let release = Data(repeating: 0x44, count: 32)
    XCTAssertThrowsError(
      try KagemushaSecureElementCredentialConfigurationV1(
        productConfigurationIdentifier: UUID(uuid: (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)),
        instanceAID: KagemushaSecureElementCredentialConfigurationV1.instanceAIDV1,
        displayName: "KAGEMUSHA",
        releaseID: release
      ))
    XCTAssertThrowsError(
      try KagemushaSecureElementCredentialConfigurationV1(
        productConfigurationIdentifier: product,
        instanceAID: Data(repeating: 0x99, count: 8),
        displayName: "KAGEMUSHA",
        releaseID: release
      ))
    XCTAssertThrowsError(
      try KagemushaSecureElementCredentialConfigurationV1(
        productConfigurationIdentifier: product,
        instanceAID: KagemushaSecureElementCredentialConfigurationV1.instanceAIDV1,
        displayName: " \n ",
        releaseID: release
      ))
    XCTAssertThrowsError(
      try KagemushaSecureElementCredentialConfigurationV1(
        productConfigurationIdentifier: product,
        instanceAID: KagemushaSecureElementCredentialConfigurationV1.instanceAIDV1,
        displayName: "KAGEMUSHA",
        releaseID: Data(repeating: 0, count: 32)
      ))
  }

  func testPersistenceScopeSeparatesAppReleaseAndProductConfiguration() throws {
    let base = try config()
    let otherRelease = try config(releaseByte: 0x45)
    let otherProduct = try KagemushaSecureElementCredentialConfigurationV1.foundation(
      productConfigurationIdentifier: UUID(
        uuidString: "10112233-4455-6677-8899-AABBCCDDEEFF"
      )!,
      displayName: base.displayName,
      releaseID: base.releaseID
    )
    let scopes = try [
      KagemushaSecureElementCredentialPersistenceScopeV1(
        configuration: base,
        applicationIdentifier: "org.example.wallet"
      ),
      KagemushaSecureElementCredentialPersistenceScopeV1(
        configuration: base,
        applicationIdentifier: "org.example.other-wallet"
      ),
      KagemushaSecureElementCredentialPersistenceScopeV1(
        configuration: otherRelease,
        applicationIdentifier: "org.example.wallet"
      ),
      KagemushaSecureElementCredentialPersistenceScopeV1(
        configuration: otherProduct,
        applicationIdentifier: "org.example.wallet"
      ),
    ]
    XCTAssertEqual(Set(scopes.map(\.accountIdentifier)).count, scopes.count)
    XCTAssertTrue(scopes.allSatisfy { $0.digest.count == 32 })
  }

  func testFreshDevicePersistsIntentPendingAndAdmissionAroundFreshRelist() async throws {
    let configuration = try config()
    let identifier = credentialID(1)
    let pending = snapshot(identifier, .installationPending)
    let installed = snapshot(
      identifier,
      .installed(instanceAIDs: [configuration.instanceAID])
    )
    let log = ProvisioningEventLog()
    let session = FakeCredentialSession(
      lists: [[], [installed]],
      provisioned: pending,
      capabilityFrame: try capabilityFrame(),
      log: log
    )
    let backend = FakeCredentialBackend(session: session, log: log)
    let persistence = FakeCredentialPersistence(log: log)
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: backend,
      persistence: persistence,
      installationTimeoutNanoseconds: 1_000_000
    )

    let admission = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )

    XCTAssertEqual(admission?.credentialIdentifier, identifier)
    XCTAssertEqual(
      persistence.current,
      .admitted(identifier)
    )
    XCTAssertEqual(
      log.events,
      [
        "eligible", "start", "list", "save:provisioning", "provision",
        "save:pending", "wait:\(identifier.uuidString)", "list", "wired", "capabilities",
        "save:admitted",
      ]
    )
    let counts = await session.counts()
    XCTAssertEqual(counts.list, 2)
    XCTAssertEqual(counts.provision, 1)
    XCTAssertEqual(counts.endWired, 0)
    XCTAssertEqual(counts.invalidate, 0)
  }

  func testUniqueInstalledExactAIDIsAdoptedOnlyAfterCapabilityProbe() async throws {
    let configuration = try config()
    let identifier = credentialID(2)
    let installed = snapshot(
      identifier,
      .installed(instanceAIDs: [configuration.instanceAID]),
      displayName: "previous display name"
    )
    let log = ProvisioningEventLog()
    let session = FakeCredentialSession(
      lists: [[installed]],
      capabilityFrame: try capabilityFrame(),
      log: log
    )
    let persistence = FakeCredentialPersistence(log: log)
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: FakeCredentialBackend(session: session, log: log),
      persistence: persistence
    )

    let admission = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )

    XCTAssertEqual(admission?.credentialIdentifier, identifier)
    XCTAssertEqual(persistence.current, .admitted(identifier))
    XCTAssertLessThan(
      try XCTUnwrap(log.events.firstIndex(of: "capabilities")),
      try XCTUnwrap(log.events.firstIndex(of: "save:admitted"))
    )
    let counts = await session.counts()
    XCTAssertEqual(counts.provision, 0)
  }

  func testAmbiguousInstalledCandidatesFailWithoutProvisioning() async throws {
    let configuration = try config()
    let candidates = [credentialID(3), credentialID(4)].map {
      snapshot($0, .installed(instanceAIDs: [configuration.instanceAID]))
    }
    let session = FakeCredentialSession(
      lists: [candidates],
      capabilityFrame: try capabilityFrame()
    )
    let persistence = FakeCredentialPersistence()
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: FakeCredentialBackend(session: session),
      persistence: persistence
    )

    let admission = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )

    XCTAssertNil(admission)
    XCTAssertNil(persistence.current)
    let counts = await session.counts()
    XCTAssertEqual(counts.provision, 0)
    XCTAssertEqual(counts.invalidate, 1)
  }

  func testPersistedCredentialWithWrongAIDFailsAndStateIsPreserved() async throws {
    let configuration = try config()
    let identifier = credentialID(5)
    let session = FakeCredentialSession(
      lists: [[snapshot(identifier, .installed(instanceAIDs: [Data(repeating: 0x99, count: 8)]))]],
      capabilityFrame: try capabilityFrame()
    )
    let persistence = FakeCredentialPersistence(initial: .pending(identifier))
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: FakeCredentialBackend(session: session),
      persistence: persistence
    )

    let admission = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )

    XCTAssertNil(admission)
    XCTAssertEqual(persistence.current, .pending(identifier))
    XCTAssertTrue(persistence.saved.isEmpty)
    let counts = await session.counts()
    XCTAssertEqual(counts.provision, 0)
    XCTAssertEqual(counts.invalidate, 1)
  }

  func testCapabilityMismatchEndsWiredModeInvalidatesAndDoesNotAdmit() async throws {
    let configuration = try config()
    let identifier = credentialID(6)
    let session = FakeCredentialSession(
      lists: [[snapshot(identifier, .installed(instanceAIDs: [configuration.instanceAID]))]],
      capabilityFrame: Data(repeating: 0x77, count: 96)
    )
    let persistence = FakeCredentialPersistence()
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: FakeCredentialBackend(session: session),
      persistence: persistence
    )

    let admission = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )

    XCTAssertNil(admission)
    XCTAssertNil(persistence.current)
    let counts = await session.counts()
    XCTAssertEqual(counts.endWired, 1)
    XCTAssertEqual(counts.invalidate, 1)
  }

  func testPreReturnTransientFailuresLeaveUncertainIntentAndNeverReprovision() async throws {
    for failure in FakeCredentialFailure.transientCases {
      let configuration = try config(releaseByte: failure.releaseByte)
      let session = FakeCredentialSession(
        lists: [[], []],
        provisionError: failure,
        capabilityFrame: try capabilityFrame()
      )
      let persistence = FakeCredentialPersistence()
      let provisioner = KagemushaSecureElementCredentialProvisionerV1(
        backend: FakeCredentialBackend(session: session),
        persistence: persistence
      )

      let first = await provisioner.open(
        configuration: configuration,
        applicationIdentifier: "org.example.wallet"
      )
      XCTAssertNil(first)
      XCTAssertEqual(persistence.current, .provisioning, "failure: \(failure)")
      let second = await provisioner.open(
        configuration: configuration,
        applicationIdentifier: "org.example.wallet"
      )
      XCTAssertNil(second)
      XCTAssertEqual(persistence.current, .provisioning, "failure: \(failure)")
      let counts = await session.counts()
      XCTAssertEqual(counts.provision, 1, "failure: \(failure)")
      XCTAssertEqual(counts.invalidate, 2, "failure: \(failure)")
    }
  }

  func testPendingTimeoutSurvivesRestartAndNeverReprovisions() async throws {
    let configuration = try config()
    let identifier = credentialID(7)
    let pending = snapshot(identifier, .installationPending)
    let session = FakeCredentialSession(
      lists: [[], [pending]],
      provisioned: pending,
      waitError: FakeCredentialFailure.timeout,
      capabilityFrame: try capabilityFrame()
    )
    let persistence = FakeCredentialPersistence()
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: FakeCredentialBackend(session: session),
      persistence: persistence,
      installationTimeoutNanoseconds: 1
    )

    let first = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )
    XCTAssertNil(first)
    XCTAssertEqual(persistence.current, .pending(identifier))
    let second = await provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )
    XCTAssertNil(second)
    XCTAssertEqual(persistence.current, .pending(identifier))
    let counts = await session.counts()
    XCTAssertEqual(counts.provision, 1)
    XCTAssertEqual(counts.wait, 2)
  }

  func testConcurrentOpenIsSingleFlight() async throws {
    let configuration = try config()
    let identifier = credentialID(8)
    let pending = snapshot(identifier, .installationPending)
    let installed = snapshot(identifier, .installed(instanceAIDs: [configuration.instanceAID]))
    let session = FakeCredentialSession(
      lists: [[], [installed]],
      provisioned: pending,
      capabilityFrame: try capabilityFrame(),
      delayNanoseconds: 30_000_000
    )
    let backend = FakeCredentialBackend(session: session)
    let provisioner = KagemushaSecureElementCredentialProvisionerV1(
      backend: backend,
      persistence: FakeCredentialPersistence()
    )

    async let first = provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )
    async let second = provisioner.open(
      configuration: configuration,
      applicationIdentifier: "org.example.wallet"
    )
    let (left, right) = await (first, second)

    XCTAssertNotNil(left)
    XCTAssertTrue(left === right)
    let startCount = await backend.startCount()
    XCTAssertEqual(startCount, 1)
    let counts = await session.counts()
    XCTAssertEqual(counts.provision, 1)
  }

  private func config(releaseByte: UInt8 = 0x44) throws
    -> KagemushaSecureElementCredentialConfigurationV1
  {
    try .foundation(
      productConfigurationIdentifier: UUID(
        uuidString: "00112233-4455-6677-8899-AABBCCDDEEFF"
      )!,
      displayName: "KAGEMUSHA",
      releaseID: Data(repeating: releaseByte, count: 32)
    )
  }

  private func credentialID(_ suffix: UInt8) -> UUID {
    UUID(uuid: (0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, suffix))
  }

  private func snapshot(
    _ identifier: UUID,
    _ state: KagemushaSecureElementCredentialSnapshotStateV1,
    displayName: String = "KAGEMUSHA"
  ) -> KagemushaSecureElementCredentialSnapshotV1 {
    KagemushaSecureElementCredentialSnapshotV1(
      identifier: identifier,
      displayName: displayName,
      state: state
    )
  }

  private func capabilityFrame() throws -> Data {
    try KagemushaDeviceLifecycleBridgeV1.Codec.encodeCapabilitiesForTests(
      platform: KagemushaDeviceLifecycleBridgeV1.Codec.iosPlatformCode,
      policy: Data(repeating: 0x22, count: 32),
      attestation: Data(repeating: 0x33, count: 32)
    )
  }
}

private enum FakeCredentialFailure: Error, CaseIterable {
  case network
  case resourceUnavailable
  case cancelled
  case backgroundInvalidation
  case timeout

  static let transientCases: [FakeCredentialFailure] = [
    .network, .resourceUnavailable, .cancelled, .backgroundInvalidation,
  ]

  var releaseByte: UInt8 {
    switch self {
    case .network: 0x51
    case .resourceUnavailable: 0x52
    case .cancelled: 0x53
    case .backgroundInvalidation: 0x54
    case .timeout: 0x55
    }
  }
}

private final class ProvisioningEventLog: @unchecked Sendable {
  private let lock = NSLock()
  private var storage: [String] = []

  var events: [String] { lock.withLock { storage } }

  func append(_ value: String) { lock.withLock { storage.append(value) } }
}

private final class FakeCredentialPersistence:
  KagemushaSecureElementCredentialPersistenceV1, @unchecked Sendable
{
  private let lock = NSLock()
  private var state: KagemushaSecureElementCredentialPersistentStateV1?
  private var history: [KagemushaSecureElementCredentialPersistentStateV1] = []
  private let log: ProvisioningEventLog?

  init(
    initial: KagemushaSecureElementCredentialPersistentStateV1? = nil,
    log: ProvisioningEventLog? = nil
  ) {
    state = initial
    self.log = log
  }

  var current: KagemushaSecureElementCredentialPersistentStateV1? {
    lock.withLock { state }
  }

  var saved: [KagemushaSecureElementCredentialPersistentStateV1] {
    lock.withLock { history }
  }

  func load(
    scope _: KagemushaSecureElementCredentialPersistenceScopeV1
  ) throws -> KagemushaSecureElementCredentialPersistentStateV1? {
    current
  }

  func save(
    _ state: KagemushaSecureElementCredentialPersistentStateV1,
    scope _: KagemushaSecureElementCredentialPersistenceScopeV1
  ) throws {
    lock.withLock {
      self.state = state
      history.append(state)
    }
    switch state {
    case .provisioning: log?.append("save:provisioning")
    case .pending: log?.append("save:pending")
    case .admitted: log?.append("save:admitted")
    }
  }
}

private actor FakeCredentialBackend: KagemushaSecureElementCredentialBackendV1 {
  private let session: FakeCredentialSession
  private let eligible: Bool
  private let log: ProvisioningEventLog?
  private var starts = 0

  init(
    session: FakeCredentialSession,
    eligible: Bool = true,
    log: ProvisioningEventLog? = nil
  ) {
    self.session = session
    self.eligible = eligible
    self.log = log
  }

  func isEligible() async throws -> Bool {
    log?.append("eligible")
    return eligible
  }

  func startSession() async throws -> any KagemushaSecureElementCredentialSessionBackendV1 {
    starts += 1
    log?.append("start")
    return session
  }

  func startCount() -> Int { starts }
}

private actor FakeCredentialSession: KagemushaSecureElementCredentialSessionBackendV1 {
  struct Counts {
    let list: Int
    let provision: Int
    let wait: Int
    let endWired: Int
    let invalidate: Int
  }

  private var lists: [[KagemushaSecureElementCredentialSnapshotV1]]
  private let provisioned: KagemushaSecureElementCredentialSnapshotV1?
  private let provisionError: FakeCredentialFailure?
  private let waitError: FakeCredentialFailure?
  private let capabilityFrame: Data
  private let log: ProvisioningEventLog?
  private let delayNanoseconds: UInt64
  private var listCalls = 0
  private var provisionCalls = 0
  private var waitCalls = 0
  private var endWiredCalls = 0
  private var invalidateCalls = 0

  init(
    lists: [[KagemushaSecureElementCredentialSnapshotV1]],
    provisioned: KagemushaSecureElementCredentialSnapshotV1? = nil,
    provisionError: FakeCredentialFailure? = nil,
    waitError: FakeCredentialFailure? = nil,
    capabilityFrame: Data,
    log: ProvisioningEventLog? = nil,
    delayNanoseconds: UInt64 = 0
  ) {
    self.lists = lists
    self.provisioned = provisioned
    self.provisionError = provisionError
    self.waitError = waitError
    self.capabilityFrame = capabilityFrame
    self.log = log
    self.delayNanoseconds = delayNanoseconds
  }

  func listCredentials() async throws -> [KagemushaSecureElementCredentialSnapshotV1] {
    listCalls += 1
    log?.append("list")
    guard !lists.isEmpty else { return [] }
    return lists.removeFirst()
  }

  func provisionCredential(
    productConfigurationIdentifier _: UUID,
    displayName _: String
  ) async throws -> KagemushaSecureElementCredentialSnapshotV1 {
    provisionCalls += 1
    log?.append("provision")
    if delayNanoseconds > 0 { try await Task.sleep(nanoseconds: delayNanoseconds) }
    if let provisionError { throw provisionError }
    return try XCTUnwrap(provisioned)
  }

  func waitForInstallation(
    credentialIdentifier: UUID,
    timeoutNanoseconds _: UInt64
  ) async throws {
    waitCalls += 1
    log?.append("wait:\(credentialIdentifier.uuidString)")
    if let waitError { throw waitError }
  }

  func enterWiredMode(credentialIdentifier _: UUID) async throws { log?.append("wired") }

  func readCapabilityFrame() async throws -> Data {
    log?.append("capabilities")
    return capabilityFrame
  }

  func transceive(_: Data) async throws -> Data { throw FakeCredentialFailure.resourceUnavailable }

  func endWiredMode() async throws {
    endWiredCalls += 1
    log?.append("end")
  }

  func invalidate() async throws {
    invalidateCalls += 1
    log?.append("invalidate")
  }

  func counts() -> Counts {
    Counts(
      list: listCalls,
      provision: provisionCalls,
      wait: waitCalls,
      endWired: endWiredCalls,
      invalidate: invalidateCalls
    )
  }
}

private extension NSLock {
  func withLock<T>(_ body: () throws -> T) rethrows -> T {
    lock()
    defer { unlock() }
    return try body()
  }
}
