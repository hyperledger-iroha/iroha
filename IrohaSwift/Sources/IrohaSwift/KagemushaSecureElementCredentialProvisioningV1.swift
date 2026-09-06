// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import CryptoKit
import Foundation

#if canImport(Security)
  import Security
#endif

#if OFFLINE_SECURE_ELEMENT_CREDENTIAL && canImport(SecureElementCredential) && canImport(Security)
  import SecureElementCredential
#endif

/// Apple product configuration and exact KAGEMUSHA applet identity used for provisioning.
public struct KagemushaSecureElementCredentialConfigurationV1: Equatable, Sendable {
  /// The sole KAGEMUSHA V1 Secure Element instance application identifier.
  public static let instanceAIDV1 = Data([0xf0, 0x4f, 0x44, 0x4a, 0x52, 0x4e, 0x00, 0x01])

  /// Opaque product configuration UUID issued through Apple's business-registration process.
  public let productConfigurationIdentifier: UUID
  /// Exact applet instance AID. V1 accepts only ``instanceAIDV1``.
  public let instanceAID: Data
  /// Nonempty name shown for the provisioned credential.
  public let displayName: String
  /// Exact KAGEMUSHA release identifier that scopes durable provisioning state.
  public let releaseID: Data

  /// Construct a validated, release-scoped product configuration.
  public init(
    productConfigurationIdentifier: UUID,
    instanceAID: Data,
    displayName: String,
    releaseID: Data
  ) throws {
    guard !Self.uuidBytes(productConfigurationIdentifier).allSatisfy({ $0 == 0 }) else {
      throw Self.invalid("productConfigurationIdentifier must be non-zero")
    }
    guard instanceAID == Self.instanceAIDV1 else {
      throw Self.invalid("instanceAID is not the exact KAGEMUSHA V1 applet AID")
    }
    let displayNameBytes = Data(displayName.utf8)
    guard !displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
      !displayName.unicodeScalars.contains(where: { $0.value == 0 }),
      displayNameBytes.count <= 128
    else {
      throw Self.invalid("displayName must contain 1...128 non-NUL UTF-8 bytes")
    }
    guard releaseID.count == 32, releaseID.contains(where: { $0 != 0 }) else {
      throw Self.invalid("releaseID must contain exactly 32 non-zero bytes")
    }
    self.productConfigurationIdentifier = productConfigurationIdentifier
    self.instanceAID = instanceAID
    self.displayName = displayName
    self.releaseID = releaseID
  }

  /// Construct the sole KAGEMUSHA V1 applet configuration.
  public static func foundation(
    productConfigurationIdentifier: UUID,
    displayName: String,
    releaseID: Data
  ) throws -> KagemushaSecureElementCredentialConfigurationV1 {
    try KagemushaSecureElementCredentialConfigurationV1(
      productConfigurationIdentifier: productConfigurationIdentifier,
      instanceAID: instanceAIDV1,
      displayName: displayName,
      releaseID: releaseID
    )
  }

  fileprivate static func uuidBytes(_ value: UUID) -> Data {
    withUnsafeBytes(of: value.uuid) { Data($0) }
  }

  private static func invalid(_ reason: String) -> KagemushaDeviceLifecycleBridgeErrorV1 {
    .invalidContract(reason)
  }
}

enum KagemushaSecureElementCredentialSnapshotStateV1: Equatable, Sendable {
  case installationPending
  case installed(instanceAIDs: [Data])
  case installationFailed
}

struct KagemushaSecureElementCredentialSnapshotV1: Equatable, Sendable {
  let identifier: UUID
  let displayName: String
  let state: KagemushaSecureElementCredentialSnapshotStateV1
}

protocol KagemushaSecureElementCredentialSessionBackendV1: Sendable {
  func listCredentials() async throws -> [KagemushaSecureElementCredentialSnapshotV1]
  func provisionCredential(
    productConfigurationIdentifier: UUID,
    displayName: String
  ) async throws -> KagemushaSecureElementCredentialSnapshotV1
  func waitForInstallation(
    credentialIdentifier: UUID,
    timeoutNanoseconds: UInt64
  ) async throws
  func enterWiredMode(credentialIdentifier: UUID) async throws
  func readCapabilityFrame() async throws -> Data
  func transceive(_ command: Data) async throws -> Data
  func endWiredMode() async throws
  func invalidate() async throws
}

protocol KagemushaSecureElementCredentialBackendV1: Sendable {
  func isEligible() async throws -> Bool
  func startSession() async throws -> any KagemushaSecureElementCredentialSessionBackendV1
}

enum KagemushaSecureElementCredentialPersistentStateV1: Equatable, Sendable {
  case provisioning
  case pending(UUID)
  case admitted(UUID)
}

struct KagemushaSecureElementCredentialPersistenceScopeV1: Equatable, Sendable {
  let digest: Data

  var accountIdentifier: String {
    digest.map { String(format: "%02x", $0) }.joined()
  }

  init(
    configuration: KagemushaSecureElementCredentialConfigurationV1,
    applicationIdentifier: String
  ) throws {
    let application = Data(applicationIdentifier.utf8)
    guard !applicationIdentifier.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
      application.count <= 512
    else {
      throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
        "application identifier is unavailable or invalid"
      )
    }
    var material = Data("iroha.swift.kagemusha.se-credential.scope.v1\0".utf8)
    Self.appendLengthDelimited(application, to: &material)
    Self.appendLengthDelimited(configuration.releaseID, to: &material)
    Self.appendLengthDelimited(
      KagemushaSecureElementCredentialConfigurationV1.uuidBytes(
        configuration.productConfigurationIdentifier
      ),
      to: &material
    )
    digest = Data(SHA256.hash(data: material))
  }

  private static func appendLengthDelimited(_ value: Data, to output: inout Data) {
    var length = UInt64(value.count).bigEndian
    withUnsafeBytes(of: &length) { output.append(contentsOf: $0) }
    output.append(value)
  }
}

protocol KagemushaSecureElementCredentialPersistenceV1: Sendable {
  func load(
    scope: KagemushaSecureElementCredentialPersistenceScopeV1
  ) throws -> KagemushaSecureElementCredentialPersistentStateV1?
  func save(
    _ state: KagemushaSecureElementCredentialPersistentStateV1,
    scope: KagemushaSecureElementCredentialPersistenceScopeV1
  ) throws
}

final class KagemushaSecureElementCredentialAdmissionV1: @unchecked Sendable {
  let session: any KagemushaSecureElementCredentialSessionBackendV1
  let credentialIdentifier: UUID
  let acceptedCapabilities: KagemushaDeviceLifecycleCapabilitiesV1
  let gate = KagemushaSecureElementCredentialAsyncGateV1()

  init(
    session: any KagemushaSecureElementCredentialSessionBackendV1,
    credentialIdentifier: UUID,
    acceptedCapabilities: KagemushaDeviceLifecycleCapabilitiesV1
  ) {
    self.session = session
    self.credentialIdentifier = credentialIdentifier
    self.acceptedCapabilities = acceptedCapabilities
  }
}

actor KagemushaSecureElementCredentialAsyncGateV1 {
  private var occupied = false
  private var waiters: [CheckedContinuation<Void, Never>] = []

  func withLock<T>(_ operation: () async throws -> T) async rethrows -> T {
    await acquire()
    defer { release() }
    return try await operation()
  }

  func withLockWithoutThrowing(_ operation: () async -> Void) async {
    await acquire()
    defer { release() }
    await operation()
  }

  private func acquire() async {
    if !occupied {
      occupied = true
      return
    }
    await withCheckedContinuation { continuation in
      waiters.append(continuation)
    }
  }

  private func release() {
    if waiters.isEmpty {
      occupied = false
    } else {
      waiters.removeFirst().resume()
    }
  }
}

/// Single-flight provisioning and reconciliation for one process.
actor KagemushaSecureElementCredentialProvisionerV1 {
  private struct InFlight {
    let token: UInt64
    let scopeDigest: Data
    let task: Task<KagemushaSecureElementCredentialAdmissionV1?, Never>
  }

  private enum Resolution {
    case provision
    case pending(UUID)
    case installed(UUID)
  }

  private let backend: any KagemushaSecureElementCredentialBackendV1
  private let persistence: any KagemushaSecureElementCredentialPersistenceV1
  private let installationTimeoutNanoseconds: UInt64
  private var nextToken: UInt64 = 0
  private var inFlight: InFlight?

  init(
    backend: any KagemushaSecureElementCredentialBackendV1,
    persistence: any KagemushaSecureElementCredentialPersistenceV1,
    installationTimeoutNanoseconds: UInt64 = 120_000_000_000
  ) {
    self.backend = backend
    self.persistence = persistence
    self.installationTimeoutNanoseconds = installationTimeoutNanoseconds
  }

  func open(
    configuration: KagemushaSecureElementCredentialConfigurationV1,
    applicationIdentifier: String
  ) async -> KagemushaSecureElementCredentialAdmissionV1? {
    let scope: KagemushaSecureElementCredentialPersistenceScopeV1
    do {
      scope = try KagemushaSecureElementCredentialPersistenceScopeV1(
        configuration: configuration,
        applicationIdentifier: applicationIdentifier
      )
    } catch {
      return nil
    }
    if let inFlight {
      guard inFlight.scopeDigest == scope.digest else { return nil }
      return await inFlight.task.value
    }
    nextToken &+= 1
    let token = nextToken
    let backend = backend
    let persistence = persistence
    let timeout = installationTimeoutNanoseconds
    let task = Task<KagemushaSecureElementCredentialAdmissionV1?, Never> {
      do {
        return try await Self.performOpen(
          configuration: configuration,
          scope: scope,
          backend: backend,
          persistence: persistence,
          installationTimeoutNanoseconds: timeout
        )
      } catch {
        return nil
      }
    }
    inFlight = InFlight(token: token, scopeDigest: scope.digest, task: task)
    let result = await task.value
    if inFlight?.token == token { inFlight = nil }
    return result
  }

  private static func performOpen(
    configuration: KagemushaSecureElementCredentialConfigurationV1,
    scope: KagemushaSecureElementCredentialPersistenceScopeV1,
    backend: any KagemushaSecureElementCredentialBackendV1,
    persistence: any KagemushaSecureElementCredentialPersistenceV1,
    installationTimeoutNanoseconds: UInt64
  ) async throws -> KagemushaSecureElementCredentialAdmissionV1 {
    guard try await backend.isEligible() else {
      throw invalid("device is not eligible for Secure Element credentials")
    }
    let session = try await backend.startSession()
    do {
      let persisted = try persistence.load(scope: scope)
      let listed = try await session.listCredentials()
      var resolution = try resolve(
        persisted: persisted,
        credentials: listed,
        configuration: configuration
      )
      var pendingStateWasPersisted = false
      if case .provision = resolution {
        // This intent is deliberately durable before Apple's potentially ambiguous await.
        try persistence.save(.provisioning, scope: scope)
        let provisioned = try await session.provisionCredential(
          productConfigurationIdentifier: configuration.productConfigurationIdentifier,
          displayName: configuration.displayName
        )
        guard !isZero(provisioned.identifier),
          provisioned.displayName == configuration.displayName,
          provisioned.state == .installationPending
        else {
          throw invalid("provisionCredential returned a non-canonical pending credential")
        }
        try persistence.save(.pending(provisioned.identifier), scope: scope)
        pendingStateWasPersisted = true
        resolution = .pending(provisioned.identifier)
      }

      let credentialIdentifier: UUID
      switch resolution {
      case .provision:
        throw invalid("internal provisioning resolution was not consumed")
      case .pending(let identifier):
        if !pendingStateWasPersisted && (persisted == nil || persisted == .provisioning) {
          try persistence.save(.pending(identifier), scope: scope)
        }
        try await session.waitForInstallation(
          credentialIdentifier: identifier,
          timeoutNanoseconds: installationTimeoutNanoseconds
        )
        // Apple credentials and installation events are snapshots. Authority comes from re-listing.
        let refreshed = try await session.listCredentials()
        try requireExactlyInstalled(
          identifier: identifier,
          credentials: refreshed,
          configuration: configuration
        )
        credentialIdentifier = identifier
      case .installed(let identifier):
        try requireExactlyInstalled(
          identifier: identifier,
          credentials: listed,
          configuration: configuration
        )
        credentialIdentifier = identifier
      }

      try await session.enterWiredMode(credentialIdentifier: credentialIdentifier)
      let capabilityFrame = try await session.readCapabilityFrame()
      let capabilities = try KagemushaDeviceLifecycleBridgeV1.Codec.decodeCapabilities(
        capabilityFrame,
        expectedPlatform: KagemushaDeviceLifecycleBridgeV1.Codec.iosPlatformCode
      )
      try persistence.save(.admitted(credentialIdentifier), scope: scope)
      return KagemushaSecureElementCredentialAdmissionV1(
        session: session,
        credentialIdentifier: credentialIdentifier,
        acceptedCapabilities: capabilities
      )
    } catch {
      try? await session.endWiredMode()
      try? await session.invalidate()
      throw error
    }
  }

  private static func resolve(
    persisted: KagemushaSecureElementCredentialPersistentStateV1?,
    credentials: [KagemushaSecureElementCredentialSnapshotV1],
    configuration: KagemushaSecureElementCredentialConfigurationV1
  ) throws -> Resolution {
    switch persisted {
    case .admitted(let identifier):
      try requireExactlyInstalled(
        identifier: identifier,
        credentials: credentials,
        configuration: configuration
      )
      return .installed(identifier)
    case .pending(let identifier):
      guard credentials.filter({ $0.identifier == identifier }).count == 1,
        let credential = credentials.first(where: { $0.identifier == identifier })
      else {
        throw invalid("persisted pending credential is absent or ambiguous")
      }
      try requireNoCompetingCredential(
        selectedIdentifier: identifier,
        credentials: credentials,
        configuration: configuration
      )
      switch credential.state {
      case .installationPending:
        return .pending(identifier)
      case .installed:
        try requireExactlyInstalled(
          identifier: identifier,
          credentials: credentials,
          configuration: configuration
        )
        return .installed(identifier)
      case .installationFailed:
        throw invalid("persisted credential installation failed")
      }
    case .provisioning, nil:
      let exactInstalled = credentials.filter {
        isExactInstalled($0, instanceAID: configuration.instanceAID)
      }
      let exactName = credentials.filter { $0.displayName == configuration.displayName }
      let relevantIdentifiers = Set((exactInstalled + exactName).map(\.identifier))
      guard relevantIdentifiers.count <= 1 else {
        throw invalid("installed or pending KAGEMUSHA credential is ambiguous")
      }
      if let identifier = relevantIdentifiers.first,
        let credential = credentials.first(where: { $0.identifier == identifier })
      {
        if isExactInstalled(credential, instanceAID: configuration.instanceAID) {
          try requireExactlyInstalled(
            identifier: identifier,
            credentials: credentials,
            configuration: configuration
          )
          return .installed(identifier)
        }
        if credential.displayName == configuration.displayName,
          credential.state == .installationPending
        {
          return .pending(identifier)
        }
        throw invalid("candidate credential has the wrong AID or terminal installation state")
      }
      if persisted == .provisioning {
        throw invalid("provisioning outcome is uncertain; automatic reprovisioning is forbidden")
      }
      return .provision
    }
  }

  private static func requireExactlyInstalled(
    identifier: UUID,
    credentials: [KagemushaSecureElementCredentialSnapshotV1],
    configuration: KagemushaSecureElementCredentialConfigurationV1
  ) throws {
    guard !isZero(identifier),
      credentials.filter({ $0.identifier == identifier }).count == 1,
      let credential = credentials.first(where: { $0.identifier == identifier }),
      isExactInstalled(credential, instanceAID: configuration.instanceAID)
    else {
      throw invalid("credential is absent, ambiguous, or has the wrong instance AID")
    }
    try requireNoCompetingCredential(
      selectedIdentifier: identifier,
      credentials: credentials,
      configuration: configuration
    )
  }

  private static func requireNoCompetingCredential(
    selectedIdentifier: UUID,
    credentials: [KagemushaSecureElementCredentialSnapshotV1],
    configuration: KagemushaSecureElementCredentialConfigurationV1
  ) throws {
    let competing = credentials.contains { credential in
      credential.identifier != selectedIdentifier
        && (credential.displayName == configuration.displayName
          || isExactInstalled(credential, instanceAID: configuration.instanceAID))
    }
    guard !competing else {
      throw invalid("a competing KAGEMUSHA credential makes reconciliation ambiguous")
    }
  }

  private static func isExactInstalled(
    _ credential: KagemushaSecureElementCredentialSnapshotV1,
    instanceAID: Data
  ) -> Bool {
    guard case .installed(let instanceAIDs) = credential.state else { return false }
    return instanceAIDs.count == 1 && instanceAIDs[0] == instanceAID
  }

  private static func isZero(_ identifier: UUID) -> Bool {
    KagemushaSecureElementCredentialConfigurationV1.uuidBytes(identifier).allSatisfy { $0 == 0 }
  }

  private static func invalid(_ reason: String) -> KagemushaDeviceLifecycleBridgeErrorV1 {
    .invalidContract(reason)
  }
}

#if canImport(Security)
  final class KagemushaSecureElementCredentialKeychainPersistenceV1:
    KagemushaSecureElementCredentialPersistenceV1, @unchecked Sendable
  {
    private static let service = "org.hyperledger.iroha.kagemusha.sec-credential.v1"

    func load(
      scope: KagemushaSecureElementCredentialPersistenceScopeV1
    ) throws -> KagemushaSecureElementCredentialPersistentStateV1? {
      var query = baseQuery(scope: scope)
      query[kSecReturnData as String] = true
      query[kSecMatchLimit as String] = kSecMatchLimitOne
      var item: CFTypeRef?
      let status = SecItemCopyMatching(query as CFDictionary, &item)
      switch status {
      case errSecSuccess:
        guard let encoded = item as? Data else { throw keychainError(errSecDecode) }
        return try Envelope.decode(encoded, expectedScopeDigest: scope.digest)
      case errSecItemNotFound:
        return nil
      default:
        throw keychainError(status)
      }
    }

    func save(
      _ state: KagemushaSecureElementCredentialPersistentStateV1,
      scope: KagemushaSecureElementCredentialPersistenceScopeV1
    ) throws {
      let encoded = Envelope.encode(state, scopeDigest: scope.digest)
      var attributes = baseQuery(scope: scope)
      attributes[kSecValueData as String] = encoded
      attributes[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
      let status = SecItemAdd(attributes as CFDictionary, nil)
      if status == errSecDuplicateItem {
        let updateStatus = SecItemUpdate(
          baseQuery(scope: scope) as CFDictionary,
          [
            kSecValueData as String: encoded,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
          ] as CFDictionary
        )
        guard updateStatus == errSecSuccess else { throw keychainError(updateStatus) }
      } else if status != errSecSuccess {
        throw keychainError(status)
      }
    }

    private func baseQuery(
      scope: KagemushaSecureElementCredentialPersistenceScopeV1
    ) -> [String: Any] {
      [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: Self.service,
        kSecAttrAccount as String: scope.accountIdentifier,
        kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
      ]
    }

    private func keychainError(_ status: OSStatus) -> KagemushaDeviceLifecycleBridgeErrorV1 {
      .invalidContract("KAGEMUSHA provisioning Keychain failure: \(status)")
    }

    private enum Envelope {
      static let magic = Data("IKGMSEK1".utf8)
      static let checksumDomain = Data("iroha.swift.kagemusha.se-credential.record.v1\0".utf8)
      static let encodedBytes = 8 + 1 + 1 + 6 + 32 + 16 + 32

      static func encode(
        _ state: KagemushaSecureElementCredentialPersistentStateV1,
        scopeDigest: Data
      ) -> Data {
        var output = Data(capacity: encodedBytes)
        output.append(magic)
        output.append(1)
        let identifier: UUID?
        switch state {
        case .provisioning:
          output.append(1)
          identifier = nil
        case .pending(let value):
          output.append(2)
          identifier = value
        case .admitted(let value):
          output.append(3)
          identifier = value
        }
        output.append(Data(repeating: 0, count: 6))
        output.append(scopeDigest)
        output.append(
          identifier.map(KagemushaSecureElementCredentialConfigurationV1.uuidBytes)
            ?? Data(repeating: 0, count: 16)
        )
        output.append(Data(SHA256.hash(data: checksumDomain + output)))
        return output
      }

      static func decode(
        _ encoded: Data,
        expectedScopeDigest: Data
      ) throws -> KagemushaSecureElementCredentialPersistentStateV1 {
        guard encoded.count == encodedBytes,
          Data(encoded[0..<8]) == magic,
          encoded[8] == 1,
          Data(encoded[10..<16]) == Data(repeating: 0, count: 6),
          Data(encoded[16..<48]) == expectedScopeDigest,
          Data(encoded[64..<96])
            == Data(SHA256.hash(data: checksumDomain + Data(encoded[0..<64])))
        else {
          throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
            "corrupt KAGEMUSHA provisioning Keychain record"
          )
        }
        let identifierBytes = Data(encoded[48..<64])
        switch encoded[9] {
        case 1 where identifierBytes.allSatisfy({ $0 == 0 }):
          return .provisioning
        case 2:
          return .pending(try identifier(identifierBytes))
        case 3:
          return .admitted(try identifier(identifierBytes))
        default:
          throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
            "invalid KAGEMUSHA provisioning Keychain state"
          )
        }
      }

      private static func identifier(_ bytes: Data) throws -> UUID {
        guard bytes.count == 16, bytes.contains(where: { $0 != 0 }) else {
          throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
            "invalid KAGEMUSHA credential identifier"
          )
        }
        return UUID(
          uuid: (
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
          )
        )
      }
    }
  }
#endif

#if OFFLINE_SECURE_ELEMENT_CREDENTIAL && canImport(SecureElementCredential) && canImport(Security)
  @available(iOS 18.1, *)
  struct KagemushaAppleSecureElementCredentialBackendV1:
    KagemushaSecureElementCredentialBackendV1
  {
    func isEligible() async throws -> Bool { try await CredentialSession.isEligible }

    func startSession() async throws -> any KagemushaSecureElementCredentialSessionBackendV1 {
      let session = try await CredentialSession.startSession()
      // Acquire the stream before listing or provisioning so a fast installation cannot race
      // ahead of event observation. AsyncStream buffers the matching completion until awaited.
      let installationEvents = await session.eventStream
      return KagemushaAppleSecureElementCredentialSessionBackendV1(
        session: session,
        installationEvents: installationEvents
      )
    }
  }

  @available(iOS 18.1, *)
  actor KagemushaAppleSecureElementCredentialSessionBackendV1:
    KagemushaSecureElementCredentialSessionBackendV1
  {
    private let session: CredentialSession
    private let installationEvents: AsyncStream<CredentialSession.Event>
    private var credentials: [UUID: CredentialSession.Credential] = [:]

    init(
      session: CredentialSession,
      installationEvents: AsyncStream<CredentialSession.Event>
    ) {
      self.session = session
      self.installationEvents = installationEvents
    }

    func listCredentials() async throws -> [KagemushaSecureElementCredentialSnapshotV1] {
      let listed = try await session.listCredentials()
      var indexed: [UUID: CredentialSession.Credential] = [:]
      for credential in listed {
        guard indexed.updateValue(credential, forKey: credential.identifier) == nil else {
          throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
            "Secure Element credential list contains a duplicate identifier"
          )
        }
      }
      credentials = indexed
      return listed.map(Self.snapshot)
    }

    func provisionCredential(
      productConfigurationIdentifier: UUID,
      displayName: String
    ) async throws -> KagemushaSecureElementCredentialSnapshotV1 {
      let credential = try await session.provisionCredential(
        configurationUUID: productConfigurationIdentifier,
        name: displayName
      )
      credentials[credential.identifier] = credential
      return Self.snapshot(credential)
    }

    func waitForInstallation(
      credentialIdentifier: UUID,
      timeoutNanoseconds: UInt64
    ) async throws {
      let installationEvents = installationEvents
      try await withThrowingTaskGroup(of: Void.self) { group in
        group.addTask {
          for await event in installationEvents {
            switch event {
            case .credentialFinishedInstalling(let credential)
              where credential.identifier == credentialIdentifier:
              return
            case .sessionInvalidated(let reason):
              throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
                "Secure Element credential session invalidated: \(reason)"
              )
            default:
              continue
            }
          }
          throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
            "Secure Element credential event stream ended before installation"
          )
        }
        group.addTask {
          try await Task.sleep(nanoseconds: timeoutNanoseconds)
          throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
            "Secure Element credential installation timed out"
          )
        }
        _ = try await group.next()
        group.cancelAll()
      }
    }

    func enterWiredMode(credentialIdentifier: UUID) async throws {
      guard let credential = credentials[credentialIdentifier] else {
        throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
          "fresh credential snapshot is unavailable"
        )
      }
      try await session.enterWiredMode(using: credential)
    }

    func readCapabilityFrame() async throws -> Data {
      var raw = try await session.transceive(Data([0x80, 0x11, 0, 0, 96]))
      defer { raw.resetBytes(in: raw.startIndex..<raw.endIndex) }
      guard raw.count == 98,
        raw[raw.count - 2] == 0x90,
        raw[raw.count - 1] == 0
      else {
        throw KagemushaDeviceLifecycleBridgeErrorV1.invalidContract(
          "secure-element capability APDU failed or returned the wrong length"
        )
      }
      return Data(raw.prefix(96))
    }

    func transceive(_ command: Data) async throws -> Data {
      try await session.transceive(command)
    }

    func endWiredMode() async throws { try await session.endWiredMode() }

    func invalidate() async throws { try await session.invalidate() }

    private static func snapshot(
      _ credential: CredentialSession.Credential
    ) -> KagemushaSecureElementCredentialSnapshotV1 {
      let state: KagemushaSecureElementCredentialSnapshotStateV1
      switch credential.state {
      case .installationPending:
        state = .installationPending
      case .installed(let instances):
        state = .installed(instanceAIDs: instances.map(\.instanceAID))
      case .installationFailed:
        state = .installationFailed
      @unknown default:
        state = .installationFailed
      }
      return KagemushaSecureElementCredentialSnapshotV1(
        identifier: credential.identifier,
        displayName: credential.name,
        state: state
      )
    }
  }
#endif
