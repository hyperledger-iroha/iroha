import Foundation

/// Validation failures for the first-release native Kaigi instruction surface.
public enum KaigiV1Error: Error, Equatable, LocalizedError, Sendable {
  case invalidValue(String)

  public var errorDescription: String? {
    switch self {
    case .invalidValue(let message):
      return message
    }
  }
}

/// A locally encoded first-release Kaigi instruction.
public protocol KaigiInstructionV1: Sendable {
  /// Stable dynamic instruction identifier carried by `InstructionBox`.
  var wireID: String { get }

  /// Compiler-emitted Rust type name whose hash identifies the concrete payload frame.
  var concreteSchemaName: String { get }

  /// Encode the concrete instruction without its Norito header.
  func barePayload() throws -> Data
}

extension KaigiInstructionV1 {
  /// Build the dynamic instruction frame embedded directly in a transaction.
  public func transactionInstructionFrame() throws -> TransactionInstructionFrame {
    let frame = noritoEncode(
      typeName: concreteSchemaName,
      payload: try barePayload(),
      flags: NoritoHeader.compactLen,
      payloadAlignment: 8
    )
    return try TransactionInstructionFrame(wireName: wireID, framedPayload: frame)
  }

  /// Encode the standalone `InstructionBox` frame returned by instruction builders.
  public func standaloneInstructionBoxFrame() throws -> Data {
    let instruction = try transactionInstructionFrame()
    return noritoEncode(
      typeName: "(alloc::string::String, alloc::vec::Vec<u8>)",
      payload: try instruction.compactInstructionBoxPayload(),
      flags: NoritoHeader.compactLen,
      payloadAlignment: 8
    )
  }
}

/// Exact marked 32-byte Iroha hash used by Kaigi privacy fields.
public struct KaigiHashV1: Equatable, Hashable, Sendable {
  public static let byteCount = 32

  private let storage: Data

  public init(bytes: Data) throws {
    guard bytes.count == Self.byteCount,
      bytes.last.map({ $0 & 1 == 1 }) == true
    else {
      throw KaigiV1Error.invalidValue(
        "Kaigi hashes must contain exactly 32 bytes with the Iroha marker bit set."
      )
    }
    storage = Data(bytes)
  }

  /// A defensive copy of the exact marked hash bytes.
  public var bytes: Data { Data(storage) }

  fileprivate var isZeroSentinel: Bool {
    storage.dropLast().allSatisfy { $0 == 0 } && storage.last == 1
  }
}

/// Domain-scoped identifier for one Kaigi session.
public struct KaigiIdV1: Equatable, Hashable, Sendable {
  public let domainID: String
  public let callName: String

  public init(domainID: String, callName: String) throws {
    self.domainID = try kaigiCanonicalDomainID(domainID)
    try kaigiRequireName(callName, field: "Kaigi call name")
    self.callName = callName
  }
}

/// Privacy mode in Rust/Norito declaration order.
public enum KaigiPrivacyModeV1: UInt32, CaseIterable, Sendable {
  case transparent = 0
  case zkRosterV1 = 1
}

/// Room access policy in Rust/Norito declaration order.
public enum KaigiRoomPolicyV1: UInt32, CaseIterable, Sendable {
  case `public` = 0
  case authenticated = 1
}

/// Relay health status in Rust/Norito declaration order.
public enum KaigiRelayHealthStatusV1: UInt32, CaseIterable, Sendable {
  case healthy = 0
  case degraded = 1
  case unavailable = 2
}

/// Resource limits shared by first-release Kaigi relay descriptors.
public enum KaigiRelayBoundsV1 {
  public static let maxManifestHops = 8
  public static let maxHPKEPublicKeyBytes = 4_096
}

/// Ledger-safe participant commitment.
///
/// Rust reserves `alias_tag` for diagnostics and production admission requires
/// it to be absent. The Swift V1 type therefore does not expose that field.
public struct KaigiParticipantCommitmentV1: Equatable, Hashable, Sendable {
  public let commitment: KaigiHashV1

  public init(commitment: KaigiHashV1) {
    self.commitment = commitment
  }
}

/// Ledger-safe participant nullifier.
///
/// Rust requires `issued_at_ms == 0` for on-chain privacy artifacts. The Swift
/// V1 type fixes that reserved field to zero and rejects the zero sentinel.
public struct KaigiParticipantNullifierV1: Equatable, Hashable, Sendable {
  public let digest: KaigiHashV1

  public init(digest: KaigiHashV1) throws {
    guard !digest.isZeroSentinel else {
      throw KaigiV1Error.invalidValue("Kaigi privacy nullifiers must be non-zero.")
    }
    self.digest = digest
  }
}

/// Complete roster-proof artifact set used by create, join, and host-end actions.
public struct KaigiPrivacyArtifactsV1: Equatable, Sendable {
  public let commitment: KaigiParticipantCommitmentV1
  public let nullifier: KaigiParticipantNullifierV1
  public let rosterRoot: KaigiHashV1
  public let proof: Data

  public init(
    commitment: KaigiParticipantCommitmentV1,
    nullifier: KaigiParticipantNullifierV1,
    rosterRoot: KaigiHashV1,
    proof: Data
  ) throws {
    guard !proof.isEmpty else {
      throw KaigiV1Error.invalidValue("Kaigi privacy proof bytes must not be empty.")
    }
    self.commitment = commitment
    self.nullifier = nullifier
    self.rosterRoot = rosterRoot
    self.proof = Data(proof)
  }
}

/// Complete privacy payload for one usage segment.
public struct KaigiUsagePrivacyV1: Equatable, Sendable {
  public let commitment: KaigiHashV1
  public let proof: Data

  public init(commitment: KaigiHashV1, proof: Data) throws {
    guard !proof.isEmpty else {
      throw KaigiV1Error.invalidValue("Kaigi usage proof bytes must not be empty.")
    }
    self.commitment = commitment
    self.proof = Data(proof)
  }
}

/// One relay hop in an onion-routing manifest.
public struct KaigiRelayHopV1: Equatable, Sendable {
  public let relayID: String
  public let hpkePublicKey: Data
  public let weight: UInt8

  public init(relayID: String, hpkePublicKey: Data, weight: UInt8) throws {
    guard !hpkePublicKey.isEmpty else {
      throw KaigiV1Error.invalidValue("Kaigi relay hops require a non-empty HPKE key.")
    }
    guard hpkePublicKey.count <= KaigiRelayBoundsV1.maxHPKEPublicKeyBytes else {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay hop HPKE keys must not exceed \(KaigiRelayBoundsV1.maxHPKEPublicKeyBytes) bytes."
      )
    }
    guard weight > 0 else {
      throw KaigiV1Error.invalidValue("Kaigi relay hop weights must be non-zero.")
    }
    self.relayID = try kaigiCanonicalAccountID(relayID, field: "relay_id")
    self.hpkePublicKey = Data(hpkePublicKey)
    self.weight = weight
  }
}

/// Relay manifest carried by a call or manifest-update instruction.
public struct KaigiRelayManifestV1: Equatable, Sendable {
  public let hops: [KaigiRelayHopV1]
  public let expiryMs: UInt64

  public init(hops: [KaigiRelayHopV1], expiryMs: UInt64) throws {
    guard hops.count >= 3 else {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay manifests must contain at least three hops."
      )
    }
    guard hops.count <= KaigiRelayBoundsV1.maxManifestHops else {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay manifests must not exceed \(KaigiRelayBoundsV1.maxManifestHops) hops."
      )
    }
    var identities = Set<Data>()
    for hop in hops {
      let identity = try CanonicalNorito.encodeCompactAccountId(hop.relayID)
      guard identities.insert(identity).inserted else {
        throw KaigiV1Error.invalidValue(
          "Kaigi relay manifests must not contain duplicate relay accounts."
        )
      }
    }
    self.hops = Array(hops)
    self.expiryMs = expiryMs
  }
}

/// Registration descriptor for one relay account.
public struct KaigiRelayRegistrationV1: Equatable, Sendable {
  public let relayID: String
  public let hpkePublicKey: Data
  public let bandwidthClass: UInt8

  public init(relayID: String, hpkePublicKey: Data, bandwidthClass: UInt8) throws {
    guard !hpkePublicKey.isEmpty else {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay registrations require a non-empty HPKE key."
      )
    }
    guard hpkePublicKey.count <= KaigiRelayBoundsV1.maxHPKEPublicKeyBytes else {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay registration HPKE keys must not exceed \(KaigiRelayBoundsV1.maxHPKEPublicKeyBytes) bytes."
      )
    }
    guard bandwidthClass > 0 else {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay registration bandwidth class must be non-zero."
      )
    }
    self.relayID = try kaigiCanonicalAccountID(relayID, field: "relay_id")
    self.hpkePublicKey = Data(hpkePublicKey)
    self.bandwidthClass = bandwidthClass
  }
}

/// Host-supplied configuration for a new Kaigi session.
public struct NewKaigiV1: Equatable, Sendable {
  public let id: KaigiIdV1
  public let host: String
  public let title: String?
  public let sessionDescription: String?
  public let maxParticipants: UInt32?
  public let gasRatePerMinute: UInt64
  public let metadata: [String: ToriiJSONValue]
  public let scheduledStartMs: UInt64?
  public let billingAccount: String?
  public let privacyMode: KaigiPrivacyModeV1
  public let roomPolicy: KaigiRoomPolicyV1
  public let relayManifest: KaigiRelayManifestV1?

  public init(
    id: KaigiIdV1,
    host: String,
    title: String? = nil,
    description: String? = nil,
    maxParticipants: UInt32? = nil,
    gasRatePerMinute: UInt64 = 0,
    metadata: [String: ToriiJSONValue] = [:],
    scheduledStartMs: UInt64? = nil,
    billingAccount: String? = nil,
    privacyMode: KaigiPrivacyModeV1 = .transparent,
    roomPolicy: KaigiRoomPolicyV1 = .authenticated,
    relayManifest: KaigiRelayManifestV1? = nil
  ) throws {
    if let maxParticipants, maxParticipants == 0 {
      throw KaigiV1Error.invalidValue(
        "Kaigi maxParticipants must be greater than zero when provided."
      )
    }
    let canonicalHost = try kaigiCanonicalAccountID(host, field: "host")
    let canonicalBilling = try billingAccount.map {
      try kaigiCanonicalAccountID($0, field: "billing_account")
    }
    if let canonicalBilling {
      let hostWire = try CanonicalNorito.encodeCompactAccountId(canonicalHost)
      let billingWire = try CanonicalNorito.encodeCompactAccountId(canonicalBilling)
      guard hostWire == billingWire else {
        throw KaigiV1Error.invalidValue(
          "Kaigi billingAccount must identify the host in V1."
        )
      }
    }
    try kaigiValidateMetadata(metadata)
    self.id = id
    self.host = canonicalHost
    self.title = title
    sessionDescription = description
    self.maxParticipants = maxParticipants
    self.gasRatePerMinute = gasRatePerMinute
    self.metadata = metadata
    self.scheduledStartMs = scheduledStartMs
    self.billingAccount = canonicalBilling
    self.privacyMode = privacyMode
    self.roomPolicy = roomPolicy
    self.relayManifest = relayManifest
  }
}

/// Create a new domain-scoped Kaigi session.
public struct CreateKaigiInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::CreateKaigi"
  public static let schemaName = "iroha_data_model::isi::kaigi::CreateKaigi"

  public let call: NewKaigiV1
  public let privacyArtifacts: KaigiPrivacyArtifactsV1?

  public init(call: NewKaigiV1, privacyArtifacts: KaigiPrivacyArtifactsV1? = nil) throws {
    guard call.privacyMode != .transparent || privacyArtifacts == nil else {
      throw KaigiV1Error.invalidValue(
        "Transparent Kaigi creation must not contain privacy artifacts."
      )
    }
    self.call = call
    self.privacyArtifacts = privacyArtifacts
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(try KaigiInstructionNoritoV1.newKaigi(call))
    KaigiInstructionNoritoV1.writePrivacyArtifacts(privacyArtifacts, into: &writer)
    return writer.data
  }
}

/// Add a participant to an active Kaigi.
///
/// The optional privacy fields preserve the canonical wire contract. Native
/// production admission currently rejects ZkRosterV1 joins until its proof
/// statement binds the signed participant authority; transparent joins use
/// `nil` here.
public struct JoinKaigiInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::JoinKaigi"
  public static let schemaName = "iroha_data_model::isi::kaigi::JoinKaigi"

  public let callID: KaigiIdV1
  public let participant: String
  public let privacyArtifacts: KaigiPrivacyArtifactsV1?

  public init(
    callID: KaigiIdV1,
    participant: String,
    privacyArtifacts: KaigiPrivacyArtifactsV1? = nil
  ) throws {
    self.callID = callID
    self.participant = try kaigiCanonicalAccountID(participant, field: "participant")
    self.privacyArtifacts = privacyArtifacts
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(KaigiInstructionNoritoV1.kaigiID(callID))
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(participant))
    KaigiInstructionNoritoV1.writePrivacyArtifacts(privacyArtifacts, into: &writer)
    return writer.data
  }
}

/// Remove a participant from an active transparent Kaigi.
///
/// Native V1 performs privacy-mode departure off-chain. The four reserved
/// privacy fields are therefore always encoded as `None`.
public struct LeaveKaigiInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::LeaveKaigi"
  public static let schemaName = "iroha_data_model::isi::kaigi::LeaveKaigi"

  public let callID: KaigiIdV1
  public let participant: String

  public init(callID: KaigiIdV1, participant: String) throws {
    self.callID = callID
    self.participant = try kaigiCanonicalAccountID(participant, field: "participant")
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(KaigiInstructionNoritoV1.kaigiID(callID))
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(participant))
    for _ in 0..<4 { writer.writeField(Data([0])) }
    return writer.data
  }
}

/// Conclude an active Kaigi.
public struct EndKaigiInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::EndKaigi"
  public static let schemaName = "iroha_data_model::isi::kaigi::EndKaigi"

  public let callID: KaigiIdV1
  public let endedAtMs: UInt64?
  public let privacyArtifacts: KaigiPrivacyArtifactsV1?

  public init(
    callID: KaigiIdV1,
    endedAtMs: UInt64? = nil,
    privacyArtifacts: KaigiPrivacyArtifactsV1? = nil
  ) {
    self.callID = callID
    self.endedAtMs = endedAtMs
    self.privacyArtifacts = privacyArtifacts
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(KaigiInstructionNoritoV1.kaigiID(callID))
    writer.writeField(
      KaigiInstructionNoritoV1.optional(endedAtMs, encode: CompactNorito.encodeUInt64)
    )
    KaigiInstructionNoritoV1.writePrivacyArtifacts(privacyArtifacts, into: &writer)
    return writer.data
  }
}

/// Record usage metrics for one Kaigi segment.
public struct RecordKaigiUsageInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::RecordKaigiUsage"
  public static let schemaName = "iroha_data_model::isi::kaigi::RecordKaigiUsage"

  public let callID: KaigiIdV1
  public let durationMs: UInt64
  public let billedGas: UInt64
  public let privacy: KaigiUsagePrivacyV1?

  public init(
    callID: KaigiIdV1,
    durationMs: UInt64,
    billedGas: UInt64,
    privacy: KaigiUsagePrivacyV1? = nil
  ) throws {
    guard durationMs > 0 else {
      throw KaigiV1Error.invalidValue("Kaigi usage duration must be positive.")
    }
    self.callID = callID
    self.durationMs = durationMs
    self.billedGas = billedGas
    self.privacy = privacy
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(KaigiInstructionNoritoV1.kaigiID(callID))
    writer.writeField(CompactNorito.encodeUInt64(durationMs))
    writer.writeField(CompactNorito.encodeUInt64(billedGas))
    writer.writeField(
      KaigiInstructionNoritoV1.optional(privacy?.commitment) { $0.bytes }
    )
    writer.writeField(
      KaigiInstructionNoritoV1.optional(privacy?.proof) {
        CompactNorito.encodeBytesVec($0)
      }
    )
    return writer.data
  }
}

/// Replace or clear the relay manifest advertised for a Kaigi session.
public struct SetKaigiRelayManifestInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::SetKaigiRelayManifest"
  public static let schemaName = "iroha_data_model::isi::kaigi::SetKaigiRelayManifest"

  public let callID: KaigiIdV1
  public let relayManifest: KaigiRelayManifestV1?

  public init(callID: KaigiIdV1, relayManifest: KaigiRelayManifestV1?) {
    self.callID = callID
    self.relayManifest = relayManifest
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(KaigiInstructionNoritoV1.kaigiID(callID))
    writer.writeField(
      try KaigiInstructionNoritoV1.optional(relayManifest) {
        try KaigiInstructionNoritoV1.relayManifest($0)
      }
    )
    return writer.data
  }
}

/// Register or update one Kaigi relay.
public struct RegisterKaigiRelayInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::RegisterKaigiRelay"
  public static let schemaName = "iroha_data_model::isi::kaigi::RegisterKaigiRelay"

  public let relay: KaigiRelayRegistrationV1

  public init(relay: KaigiRelayRegistrationV1) {
    self.relay = relay
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(try KaigiInstructionNoritoV1.relayRegistration(relay))
    return writer.data
  }
}

/// Retire one Kaigi relay descriptor and its retained health feedback.
public struct UnregisterKaigiRelayInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::UnregisterKaigiRelay"
  public static let schemaName = "iroha_data_model::isi::kaigi::UnregisterKaigiRelay"

  public let relayID: String

  public init(relayID: String) throws {
    self.relayID = try kaigiCanonicalAccountID(relayID, field: "relay_id")
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(relayID))
    return writer.data
  }
}

/// Report observed health for a relay in an active Kaigi manifest.
public struct ReportKaigiRelayHealthInstructionV1: KaigiInstructionV1, Equatable {
  public static let stableWireID = "iroha.instruction.v1::kaigi::ReportKaigiRelayHealth"
  public static let schemaName = "iroha_data_model::isi::kaigi::ReportKaigiRelayHealth"

  public let callID: KaigiIdV1
  public let relayID: String
  public let status: KaigiRelayHealthStatusV1
  public let reportedAtMs: UInt64
  public let notes: String?

  public init(
    callID: KaigiIdV1,
    relayID: String,
    status: KaigiRelayHealthStatusV1,
    reportedAtMs: UInt64,
    notes: String? = nil
  ) throws {
    if let notes, notes.unicodeScalars.count > 512 {
      throw KaigiV1Error.invalidValue(
        "Kaigi relay health notes must not exceed 512 Unicode scalar values."
      )
    }
    self.callID = callID
    self.relayID = try kaigiCanonicalAccountID(relayID, field: "relay_id")
    self.status = status
    self.reportedAtMs = reportedAtMs
    self.notes = notes
  }

  public var wireID: String { Self.stableWireID }
  public var concreteSchemaName: String { Self.schemaName }

  public func barePayload() throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(KaigiInstructionNoritoV1.kaigiID(callID))
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(relayID))
    writer.writeField(CompactNorito.encodeUInt32(status.rawValue))
    writer.writeField(CompactNorito.encodeUInt64(reportedAtMs))
    writer.writeField(
      KaigiInstructionNoritoV1.optional(notes, encode: CompactNorito.encodeString)
    )
    return writer.data
  }
}

private enum KaigiInstructionNoritoV1 {
  static func optional<T>(_ value: T?, encode: (T) throws -> Data) rethrows -> Data {
    var writer = CompactNoritoWriter()
    guard let value else {
      writer.writeUInt8(0)
      return writer.data
    }
    writer.writeUInt8(1)
    writer.writeField(try encode(value))
    return writer.data
  }

  static func kaigiID(_ value: KaigiIdV1) -> Data {
    let parts = value.domainID.split(separator: ".", omittingEmptySubsequences: false)
    var domain = CompactNoritoWriter()
    domain.writeField(CompactNorito.encodeString(String(parts[0])))
    domain.writeField(CompactNorito.encodeString(String(parts[1])))

    var writer = CompactNoritoWriter()
    writer.writeField(domain.data)
    writer.writeField(CompactNorito.encodeString(value.callName))
    return writer.data
  }

  static func relayHop(_ value: KaigiRelayHopV1) throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.relayID))
    writer.writeField(CompactNorito.encodeBytesVec(value.hpkePublicKey))
    writer.writeField(CompactNorito.encodeUInt8(value.weight))
    return writer.data
  }

  static func relayManifest(_ value: KaigiRelayManifestV1) throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(try CompactNorito.encodeVec(value.hops, encode: relayHop))
    writer.writeField(CompactNorito.encodeUInt64(value.expiryMs))
    return writer.data
  }

  static func relayRegistration(_ value: KaigiRelayRegistrationV1) throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.relayID))
    writer.writeField(CompactNorito.encodeBytesVec(value.hpkePublicKey))
    writer.writeField(CompactNorito.encodeUInt8(value.bandwidthClass))
    return writer.data
  }

  static func participantCommitment(_ value: KaigiParticipantCommitmentV1) -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(value.commitment.bytes)
    writer.writeField(Data([0]))
    return writer.data
  }

  static func participantNullifier(_ value: KaigiParticipantNullifierV1) -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(value.digest.bytes)
    writer.writeField(CompactNorito.encodeUInt64(0))
    return writer.data
  }

  static func writePrivacyArtifacts(
    _ value: KaigiPrivacyArtifactsV1?,
    into writer: inout CompactNoritoWriter
  ) {
    writer.writeField(optional(value?.commitment, encode: participantCommitment))
    writer.writeField(optional(value?.nullifier, encode: participantNullifier))
    writer.writeField(optional(value?.rosterRoot) { $0.bytes })
    writer.writeField(optional(value?.proof) { CompactNorito.encodeBytesVec($0) })
  }

  static func newKaigi(_ value: NewKaigiV1) throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(kaigiID(value.id))
    writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.host))
    writer.writeField(optional(value.title, encode: CompactNorito.encodeString))
    writer.writeField(optional(value.sessionDescription, encode: CompactNorito.encodeString))
    writer.writeField(optional(value.maxParticipants, encode: CompactNorito.encodeUInt32))
    writer.writeField(CompactNorito.encodeUInt64(value.gasRatePerMinute))
    writer.writeField(try metadata(value.metadata))
    writer.writeField(optional(value.scheduledStartMs, encode: CompactNorito.encodeUInt64))
    writer.writeField(
      try optional(value.billingAccount) {
        try CanonicalNorito.encodeCompactAccountId($0)
      }
    )
    writer.writeField(CompactNorito.encodeUInt32(value.privacyMode.rawValue))
    writer.writeField(CompactNorito.encodeUInt32(value.roomPolicy.rawValue))
    writer.writeField(
      try optional(value.relayManifest) { try relayManifest($0) }
    )
    return writer.data
  }

  static func metadata(_ metadata: [String: ToriiJSONValue]) throws -> Data {
    var writer = CompactNoritoWriter()
    let keys = metadata.keys.sorted(by: kaigiUTF8Less)
    writer.writeUInt64LE(UInt64(keys.count))
    for key in keys {
      guard let value = metadata[key] else { continue }
      var entry = CompactNoritoWriter()
      entry.writeField(CompactNorito.encodeString(key))
      var json = CompactNoritoWriter()
      json.writeField(
        CompactNorito.encodeString(try CanonicalNorito.jsonString(from: value))
      )
      entry.writeField(json.data)
      writer.writeField(entry.data)
    }
    return writer.data
  }
}

private func kaigiCanonicalAccountID(_ value: String, field: String) throws -> String {
  do {
    return try TransactionInputValidator.sanitizeAccountId(value, field: field)
  } catch {
    throw KaigiV1Error.invalidValue(
      "Kaigi \(field) must be an exact canonical I105 account identifier."
    )
  }
}

private func kaigiCanonicalDomainID(_ value: String) throws -> String {
  do {
    return try TransactionInputValidator.sanitizeDomainId(value, field: "call.domain_id")
  } catch {
    throw KaigiV1Error.invalidValue(
      "Kaigi domainID must be a canonical fully-qualified domain identifier."
    )
  }
}

private func kaigiRequireName(_ value: String, field: String) throws {
  guard !value.isEmpty,
    value.utf8.count <= 255,
    value.precomposedStringWithCanonicalMapping == value,
    value.unicodeScalars.allSatisfy({ scalar in
      scalar.properties.generalCategory != .control
        && !CharacterSet.whitespacesAndNewlines.contains(scalar)
        && !kaigiIsBidiControl(scalar)
        && scalar != "@"
        && scalar != "#"
        && scalar != "$"
    })
  else {
    throw KaigiV1Error.invalidValue("\(field) must be a canonical Iroha Name.")
  }
}

private func kaigiIsBidiControl(_ scalar: Unicode.Scalar) -> Bool {
  switch scalar.value {
  case 0x061C, 0x200E, 0x200F, 0x202A...0x202E, 0x2066...0x2069:
    return true
  default:
    return false
  }
}

private func kaigiValidateMetadata(_ metadata: [String: ToriiJSONValue]) throws {
  for (key, value) in metadata {
    try kaigiRequireName(key, field: "Kaigi metadata key")
    do {
      _ = try CanonicalNorito.jsonString(from: value)
    } catch {
      throw KaigiV1Error.invalidValue(
        "Kaigi metadata values must be finite canonical JSON values."
      )
    }
  }
}

private func kaigiUTF8Less(_ lhs: String, _ rhs: String) -> Bool {
  Data(lhs.utf8).lexicographicallyPrecedes(Data(rhs.utf8))
}
