import Foundation

/// Non-authoritative caller journal. Saving must sync both data and directory before returning.
/// Records survive errors, sign-out and KYC changes. A saved reply is recovery evidence, never
/// permission to fabricate a current hardware response or Core preparation.
public protocol KagemushaOperationIntentStoringV1: AnyObject {
  var applicationScope: Data { get }
  var exclusiveLock: NSRecursiveLock { get }
  func load(operation: UInt8, operationID: Data) throws -> KagemushaOperationIntentV1?
  func pending(operation: UInt8, purpose: String, qualificationScope: Data?) throws -> [KagemushaOperationIntentV1]
  func save(_ intent: KagemushaOperationIntentV1) throws
}

/// Historical evidence retained by a durable mutation, never a current read challenge.
public struct KagemushaObservationEvidenceV1: Codable, Equatable, Sendable {
  public let operation: UInt8
  public let nonce: Data
  public let qualificationScope: Data
  public let canonicalCommand: Data
  public let canonicalReply: Data
  public let responseAuthenticator: Data

  init(operation: UInt8, nonce: Data, qualificationScope: Data, canonicalCommand: Data,
    canonicalReply: Data, responseAuthenticator: Data) throws {
    self.operation = operation; self.nonce = nonce; self.qualificationScope = qualificationScope
    self.canonicalCommand = canonicalCommand; self.canonicalReply = canonicalReply
    self.responseAuthenticator = responseAuthenticator
    try validate()
  }

  public func validate() throws {
    guard [UInt8(1), 13, 18, 21].contains(operation), nonce.count == 32,
      nonce.contains(where: { $0 != 0 }), !qualificationScope.isEmpty,
      qualificationScope.count <= 65_536, !canonicalReply.isEmpty,
      canonicalReply.count <= 65_536, canonicalCommand.count <= 65_536 else {
      throw KagemushaAuthenticatedHardwareProviderErrorV1.invalidContract("invalid historical observation evidence")
    }
    _ = try KagemushaDeviceSignatureV1(rawBytes: responseAuthenticator)
    _ = try KagemushaDeviceOperationCodecV1.decodeControlCommand(operation: operation,
      requestID: nonce, canonicalBytes: canonicalCommand)
    _ = try KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(operation: operation,
      canonicalBytes: canonicalReply)
  }
}

public struct KagemushaOperationIntentV1: Codable, Equatable, Sendable {
  public let version: UInt16
  public let applicationScope: Data
  public let qualificationScope: Data
  public let operation: UInt8
  public let operationID: Data
  public let purpose: String
  public let arguments: Data
  public let publicBinding: Data
  public var canonicalCommand: Data?
  public var canonicalReply: Data?
  public var responseAuthenticator: Data?
  public var canonicalResult: Data?
  public var authenticatedSnapshotEvidence: KagemushaObservationEvidenceV1?
  public var acknowledged: Bool

  public init(applicationScope: Data, qualificationScope: Data, operation: UInt8,
    operationID: Data, purpose: String, arguments: Data, publicBinding: Data,
    canonicalCommand: Data? = nil) throws {
    version = 1
    self.applicationScope = applicationScope
    self.qualificationScope = qualificationScope
    self.operation = operation
    self.operationID = operationID
    self.purpose = purpose
    self.arguments = arguments
    self.publicBinding = publicBinding
    self.canonicalCommand = canonicalCommand
    acknowledged = false
    try validate()
  }

  public func validate() throws {
    guard version == 1, !applicationScope.isEmpty, applicationScope.count <= 65_536,
      qualificationScope.count <= 65_536, (1...22).contains(operation),
      ![UInt8(1), 13, 18, 21].contains(operation),
      operationID.count == 32, operationID.contains(where: { $0 != 0 }),
      !purpose.isEmpty, purpose.utf8.count <= 256, !publicBinding.isEmpty,
      arguments.count <= 65_536, publicBinding.count <= 65_536,
      canonicalCommand?.isEmpty != true, canonicalReply?.isEmpty != true,
      (canonicalCommand?.count ?? 0) <= 65_536, (canonicalReply?.count ?? 0) <= 65_536,
      (canonicalReply == nil) == (responseAuthenticator == nil),
      canonicalReply == nil || canonicalCommand != nil,
      responseAuthenticator == nil || responseAuthenticator?.count == 64,
      (canonicalResult?.count ?? 0) <= 65_536, canonicalResult?.isEmpty != true,
      canonicalResult == nil || canonicalReply != nil,
      !acknowledged || canonicalReply != nil else {
      throw KagemushaAuthenticatedHardwareProviderErrorV1.invalidContract("invalid durable operation intent")
    }
    if let responseAuthenticator { _ = try KagemushaDeviceSignatureV1(rawBytes: responseAuthenticator) }
    if let authenticatedSnapshotEvidence {
      try authenticatedSnapshotEvidence.validate()
      guard authenticatedSnapshotEvidence.operation == 21, canonicalReply != nil else {
        throw KagemushaAuthenticatedHardwareProviderErrorV1.invalidContract("snapshot evidence has no dependent accepted mutation")
      }
    }
  }

  public func validateSuccessor(of prior: Self) throws {
    try validate()
    var identity = self
    identity.canonicalCommand = prior.canonicalCommand
    identity.canonicalReply = prior.canonicalReply
    identity.responseAuthenticator = prior.responseAuthenticator
    identity.canonicalResult = prior.canonicalResult
    identity.authenticatedSnapshotEvidence = prior.authenticatedSnapshotEvidence
    identity.acknowledged = prior.acknowledged
    guard identity == prior,
      prior.canonicalCommand == nil || canonicalCommand == prior.canonicalCommand,
      prior.canonicalReply == nil || canonicalReply == prior.canonicalReply,
      prior.responseAuthenticator == nil || responseAuthenticator == prior.responseAuthenticator,
      prior.canonicalResult == nil || canonicalResult == prior.canonicalResult,
      prior.authenticatedSnapshotEvidence == nil || authenticatedSnapshotEvidence == prior.authenticatedSnapshotEvidence,
      !prior.acknowledged || acknowledged else {
      throw KagemushaAuthenticatedHardwareProviderErrorV1.invalidContract("conflicting durable operation intent")
    }
  }
}

/// Synchronous intent owner injected into the authenticated client. There is no ephemeral fallback.
public final class KagemushaOperationIntentOwnerV1: @unchecked Sendable {
  private let store: any KagemushaOperationIntentStoringV1
  private let generateID: () throws -> Data

  public init(store: any KagemushaOperationIntentStoringV1,
    generateID: @escaping () throws -> Data = {
      var random = SystemRandomNumberGenerator()
      return Data((0..<32).map { _ in UInt8.random(in: .min ... .max, using: &random) })
    }) {
    self.store = store
    self.generateID = generateID
  }

  func begin(operation: UInt8, purpose: String, arguments: Data, qualificationScope: Data,
    command: (Data) throws -> Data) throws -> KagemushaOperationIntentV1 {
    try locked {
      guard ![UInt8(1), 13, 18, 21].contains(operation) else { throw invalid("observations do not enter durable intents") }
      do {
        let pending = try store.pending(operation: operation, purpose: purpose,
          qualificationScope: nil)
        guard pending.count <= 1 else { throw invalid("multiple unresolved internal intents") }
        if let prior = pending.first {
          try prior.validate()
          guard prior.applicationScope == store.applicationScope,
            prior.arguments == arguments, !prior.acknowledged,
            prior.canonicalCommand == (try command(prior.operationID)) else {
            throw invalid("unresolved internal intent has different scope or arguments")
          }
          return prior
        }
      }
      let value = try KagemushaOperationIntentV1(applicationScope: store.applicationScope,
        qualificationScope: qualificationScope, operation: operation,
        operationID: generateID(), purpose: purpose, arguments: arguments,
        publicBinding: Data([operation]))
      let canonical = try command(value.operationID)
      let complete = try KagemushaOperationIntentV1(applicationScope: value.applicationScope,
        qualificationScope: qualificationScope, operation: operation,
        operationID: value.operationID, purpose: purpose, arguments: arguments,
        publicBinding: canonical, canonicalCommand: canonical)
      guard try (UInt8(1)...UInt8(22)).allSatisfy({
        try store.load(operation: $0, operationID: complete.operationID) == nil
      }) else {
        throw invalid("operation ID collision")
      }
      try persist(complete)
      return complete
    }
  }

  func reserve(operation: UInt8, operationID: Data, binding: Data, qualificationScope: Data) throws {
    try locked {
      if let prior = try store.load(operation: operation, operationID: operationID) {
        try prior.validate()
        guard prior.applicationScope == store.applicationScope, prior.publicBinding == binding,
          prior.qualificationScope == qualificationScope else { throw invalid("operation reservation conflict") }
        return
      }
      guard try (UInt8(1)...UInt8(22)).allSatisfy({
        try store.load(operation: $0, operationID: operationID) == nil
      }) else { throw invalid("operation ID belongs to another action") }
      try persist(KagemushaOperationIntentV1(applicationScope: store.applicationScope,
        qualificationScope: qualificationScope, operation: operation, operationID: operationID,
        purpose: "caller-\(operation)", arguments: binding, publicBinding: binding))
    }
  }

  func willDispatch(operation: UInt8, operationID: Data, command: Data,
    qualificationScope: Data) throws {
    try locked {
      var value: KagemushaOperationIntentV1
      if let prior = try store.load(operation: operation, operationID: operationID) {
        value = prior
        guard value.qualificationScope == qualificationScope else { throw invalid("dispatch scope changed") }
        if let saved = value.canonicalCommand, saved != command { throw invalid("dispatch command changed") }
      } else {
        value = try KagemushaOperationIntentV1(applicationScope: store.applicationScope,
          qualificationScope: qualificationScope, operation: operation, operationID: operationID,
          purpose: "command-\(operation)", arguments: command, publicBinding: command)
      }
      value.canonicalCommand = command
      try persist(value)
    }
  }

  func accepted(operation: UInt8, operationID: Data, command: Data, reply: Data,
    authenticator: Data, qualificationScope: Data) throws {
    try locked {
      guard var value = try store.load(operation: operation, operationID: operationID),
        value.applicationScope == store.applicationScope,
        value.canonicalCommand == command,
        value.qualificationScope == qualificationScope else {
        throw invalid("accepted reply has no matching durable command and qualification")
      }
      try value.validate()
      if let original = value.canonicalReply {
        guard original == reply, value.responseAuthenticator == authenticator else {
          throw invalid("authenticated retry changed its result or authenticator")
        }
        return
      }
      value.canonicalReply = reply
      value.responseAuthenticator = authenticator
      try persist(value)
    }
  }

  func retainSnapshotEvidence(operation: UInt8, operationID: Data, evidence: KagemushaObservationEvidenceV1) throws {
    try locked {
      guard evidence.operation == 21, var value = try store.load(operation: operation, operationID: operationID),
        value.canonicalReply != nil else { throw invalid("snapshot evidence has no accepted mutation") }
      try evidence.validate()
      if value.authenticatedSnapshotEvidence != nil { return }
      value.authenticatedSnapshotEvidence = evidence
      try persist(value)
    }
  }

  func completedResult(operation: UInt8, operationID: Data, canonicalResult: Data) throws {
    try locked {
      guard var value = try store.load(operation: operation, operationID: operationID),
        value.canonicalReply != nil else { throw invalid("result has no Core-accepted reply") }
      value.canonicalResult = canonicalResult
      try persist(value)
    }
  }

  /// The provider binds the full returned result after Core has accepted the complete operation.
  /// The app calls this only after syncing the dependent transcript with that exact result.
  public func acknowledgeDurableResult(operationID: Data, canonicalResult: Data) throws {
    try locked {
      let values = try (UInt8(1)...UInt8(22)).compactMap {
        try store.load(operation: $0, operationID: operationID)
      }
      guard values.contains(where: { $0.canonicalResult == canonicalResult }) else {
        throw invalid("durable transcript result does not match the admitted operation")
      }
      for var value in values where value.canonicalReply != nil {
        value.acknowledged = true
        try persist(value)
      }
    }
  }

  func recordedQualificationScope(operation: UInt8, operationID: Data) throws -> Data? {
    try locked { try store.load(operation: operation, operationID: operationID)?.qualificationScope }
  }

  func pendingInternal(operation: UInt8) throws -> [KagemushaOperationIntentV1] {
    try locked {
      try store.pending(operation: operation, purpose: "internal-\(operation)", qualificationScope: nil)
    }
  }

  /// Call only after dependent presentation data is durable or a fresh authenticated recovery
  /// proves this result is installed. Acknowledgement retains the full evidence record.
  func acknowledge(operation: UInt8, operationID: Data, canonicalReply: Data) throws {
    try locked {
      guard var value = try store.load(operation: operation, operationID: operationID),
        value.canonicalReply == canonicalReply, value.authenticatedSnapshotEvidence != nil else {
        throw invalid("acknowledgement has no accepted reply and durable snapshot evidence")
      }
      value.acknowledged = true
      try persist(value)
    }
  }

  private func persist(_ value: KagemushaOperationIntentV1) throws {
    try value.validate()
    if let prior = try store.load(operation: value.operation, operationID: value.operationID) {
      try value.validateSuccessor(of: prior)
    }
    try store.save(value)
    guard try store.load(operation: value.operation, operationID: value.operationID) == value else {
      throw invalid("durable operation intent readback mismatch")
    }
  }
  var exclusiveLock: NSRecursiveLock { store.exclusiveLock }

  func withExclusive<T>(_ body: () throws -> T) rethrows -> T {
    store.exclusiveLock.lock(); defer { store.exclusiveLock.unlock() }; return try body()
  }
  private func locked<T>(_ body: () throws -> T) rethrows -> T { try withExclusive(body) }
  private func invalid(_ message: String) -> KagemushaAuthenticatedHardwareProviderErrorV1 {
    .invalidContract(message)
  }
}
