import Foundation

/// Exact unsigned 128-bit little-endian integer used by Offline Cash V1.
public struct OfflineCashUInt128V1: Equatable, Hashable, Sendable {
  public let littleEndianBytes: Data

  public init(littleEndianBytes: Data) throws {
    guard littleEndianBytes.count == 16 else { throw offlineCashInvalid("u128") }
    self.littleEndianBytes = Data(littleEndianBytes)
  }

  public init(_ value: UInt64) {
    var value = value.littleEndian
    var bytes = withUnsafeBytes(of: &value) { Data($0) }
    bytes.append(Data(repeating: 0, count: 8))
    littleEndianBytes = bytes
  }

  public var isZero: Bool { !littleEndianBytes.contains(where: { $0 != 0 }) }

  static let zero = OfflineCashUInt128V1(0)

  func adding(_ value: UInt8) throws -> OfflineCashUInt128V1 {
    var result = littleEndianBytes
    var carry = UInt16(value)
    for index in result.indices where carry != 0 {
      let sum = UInt16(result[index]) + carry
      result[index] = UInt8(truncatingIfNeeded: sum)
      carry = sum >> 8
    }
    guard carry == 0 else { throw offlineCashInvalid("u128.overflow") }
    return try OfflineCashUInt128V1(littleEndianBytes: result)
  }
}

/// Exact typed `AssetDefinitionId` payload used by Offline Cash V1.
public struct OfflineCashAssetDefinitionIDV1: Equatable, Hashable, Sendable {
  public let canonicalPayload: Data

  public init(_ literal: String) throws {
    canonicalPayload = try CanonicalNorito.encodeCompactAssetDefinitionId(literal)
  }

  public init(canonicalPayload: Data) throws {
    guard !canonicalPayload.isEmpty, canonicalPayload.count <= 512 else {
      throw offlineCashInvalid("asset")
    }
    self.canonicalPayload = Data(canonicalPayload)
  }
}

/// Exact typed universal `AccountId` payload used by Offline Cash V1.
public struct OfflineCashAccountIDV1: Equatable, Hashable, Sendable {
  public let canonicalPayload: Data

  public init(_ literal: String) throws {
    canonicalPayload = try CanonicalNorito.encodeCompactAccountId(literal)
  }

  public init(canonicalPayload: Data) throws {
    guard !canonicalPayload.isEmpty, canonicalPayload.count <= 512,
      AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(canonicalPayload)
    else { throw offlineCashInvalid("account") }
    self.canonicalPayload = Data(canonicalPayload)
  }
}

/// Exact non-zero, marked asset-incarnation hash.
public struct OfflineCashAssetIncarnationV1: Equatable, Hashable, Sendable {
  public let bytes: Data

  public init(bytes: Data) throws {
    guard bytes.count == 32, bytes.last.map({ $0 & 1 == 1 }) == true,
      bytes.dropLast().contains(where: { $0 != 0 }) || bytes.last != 1
    else { throw offlineCashInvalid("assetIncarnation") }
    self.bytes = Data(bytes)
  }
}

/// Fixed-width uncompressed SEC1 P-256 authority-key shape.
///
/// The release-pinned native core remains responsible for curve-point validation.
public struct OfflineCashDevicePublicKeyV1: Equatable, Hashable, Sendable {
  public let sec1Bytes: Data

  public init(sec1Bytes: Data) throws {
    guard sec1Bytes.count == 65, sec1Bytes.first == 4,
      sec1Bytes.dropFirst().contains(where: { $0 != 0 })
    else { throw offlineCashInvalid("devicePublicKey") }
    self.sec1Bytes = Data(sec1Bytes)
  }
}

/// Canonical fixed-width low-S P-256 Offline Cash V1 signature.
public struct OfflineCashDeviceSignatureV1: Equatable, Hashable, Sendable {
  public let rawBytes: Data

  public init(rawBytes: Data) throws {
    guard rawBytes.count == 64 else { throw offlineCashInvalid("deviceSignature") }
    let order = Data([
      0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
      0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
      0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
      0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
    ])
    let halfOrder = Data([
      0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
      0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
      0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
      0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
    ])
    let r = Data(rawBytes.prefix(32))
    let s = Data(rawBytes.suffix(32))
    guard r.contains(where: { $0 != 0 }), s.contains(where: { $0 != 0 }),
      r.lexicographicallyPrecedes(order), s.lexicographicallyPrecedes(order),
      !halfOrder.lexicographicallyPrecedes(s)
    else { throw offlineCashInvalid("deviceSignature") }
    self.rawBytes = Data(rawBytes)
  }
}

/// Exact 32-byte X25519 public-key encoding.
public struct OfflineCashX25519PublicKeyV1: Equatable, Hashable, Sendable {
  public let rawBytes: Data

  public init(rawBytes: Data) throws {
    guard rawBytes.count == OfflineCashWireV1.x25519PublicKeyBytes,
      rawBytes.contains(where: { $0 != 0 })
    else { throw offlineCashInvalid("x25519PublicKey") }
    self.rawBytes = Data(rawBytes)
  }
}

/// Constant-size public metadata for one privately valued aggregate balance.
public struct OfflineCashAggregateStateCommitmentV1: Equatable, Sendable {
  public let version: UInt16
  public let releaseID: Data
  public let networkID: Data
  public let asset: OfflineCashAssetDefinitionIDV1
  public let assetIncarnation: OfflineCashAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let laneID: Data
  public let hardwareEpochID: Data
  public let keyReference: Data
  public let hardwarePolicyID: Data
  public let sequence: OfflineCashUInt128V1
  public let stateCommitment: Data

  public init(
    version: UInt16 = 1, releaseID: Data, networkID: Data,
    asset: OfflineCashAssetDefinitionIDV1, assetIncarnation: OfflineCashAssetIncarnationV1,
    scale: UInt32, liabilityPoolID: Data, laneID: Data, hardwareEpochID: Data,
    keyReference: Data, hardwarePolicyID: Data, sequence: OfflineCashUInt128V1,
    stateCommitment: Data
  ) throws {
    try offlineCashHeader(version, networkID, scale)
    self.version = version
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try offlineCashDigest(liabilityPoolID, "liabilityPoolID")
    self.laneID = try offlineCashDigest(laneID, "laneID")
    self.hardwareEpochID = try offlineCashDigest(hardwareEpochID, "hardwareEpochID")
    self.keyReference = try offlineCashDigest(keyReference, "keyReference")
    self.hardwarePolicyID = try offlineCashDigest(hardwarePolicyID, "hardwarePolicyID")
    self.sequence = sequence
    self.stateCommitment = try offlineCashDigest(stateCommitment, "stateCommitment")
  }
}

/// Closed paired-Pasta proof. State links and stable credential pseudonyms are absent.
public struct OfflineCashPairedProofV1: Equatable, Sendable {
  public let version: UInt16
  public let eqProtocolDigest: Data
  public let epProtocolDigest: Data
  public let semanticDigest: Data
  public let guardEqCredentialAudit: Data
  public let guardEpCredentialAudit: Data
  public let eqDeferredAudit: Data
  public let epDeferredAudit: Data
  public let eqProof: Data
  public let epProof: Data
  public let eqHistory: Data
  public let epHistory: Data

  public init(
    version: UInt16 = 1, eqProtocolDigest: Data, epProtocolDigest: Data,
    semanticDigest: Data, guardEqCredentialAudit: Data, guardEpCredentialAudit: Data,
    eqDeferredAudit: Data, epDeferredAudit: Data, eqProof: Data, epProof: Data,
    eqHistory: Data, epHistory: Data
  ) throws {
    guard version == 1, !eqProof.isEmpty, !epProof.isEmpty,
      eqProof.count <= OfflineCashWireV1.maximumParityProofBytes,
      epProof.count <= OfflineCashWireV1.maximumParityProofBytes,
      eqProof.count + epProof.count <= OfflineCashWireV1.maximumCurrentProofsBytes,
      eqHistory.count == OfflineCashWireV1.historyAccumulatorBytes,
      epHistory.count == OfflineCashWireV1.historyAccumulatorBytes
    else { throw offlineCashInvalid("pairedProof") }
    self.version = version
    self.eqProtocolDigest = try offlineCashDigest(eqProtocolDigest, "eqProtocolDigest")
    self.epProtocolDigest = try offlineCashDigest(epProtocolDigest, "epProtocolDigest")
    self.semanticDigest = try offlineCashDigest(semanticDigest, "semanticDigest")
    self.guardEqCredentialAudit = try offlineCashDigest(
      guardEqCredentialAudit, "guardEqCredentialAudit")
    self.guardEpCredentialAudit = try offlineCashDigest(
      guardEpCredentialAudit, "guardEpCredentialAudit")
    self.eqDeferredAudit = try offlineCashDigest(eqDeferredAudit, "eqDeferredAudit")
    self.epDeferredAudit = try offlineCashDigest(epDeferredAudit, "epDeferredAudit")
    guard self.eqProtocolDigest != self.epProtocolDigest,
      self.guardEqCredentialAudit != self.guardEpCredentialAudit,
      self.eqDeferredAudit != self.epDeferredAudit, eqHistory != epHistory
    else { throw offlineCashInvalid("pairedProof.role") }
    self.eqProof = Data(eqProof)
    self.epProof = Data(epProof)
    self.eqHistory = Data(eqHistory)
    self.epHistory = Data(epHistory)
  }
}

/// Qualified platform class represented by one governed hardware profile.
public enum OfflineCashHardwarePlatformClassV1: UInt32, CaseIterable, Sendable {
  case androidOEMService = 0
  case appleOEMService = 1
  case dedicatedSecureElement = 2
  case otherQualified = 3
}

/// Governed non-forking hardware-service profile.
public struct OfflineCashHardwareProfileV1: Equatable, Sendable {
  public let version: UInt16
  public let protocolVersion: UInt16
  public let hardwareProfileID: Data
  public let providerID: Data
  public let platformClass: OfflineCashHardwarePlatformClassV1
  public let productClassDigest: Data
  public let firmwarePolicyDigest: Data
  public let enrollmentAttestationVerifierDigest: Data
  public let attestationTrustRootsDigest: Data
  public let allowedSuiteCommitment: Data
  public let policyEpoch: UInt64
  public let governanceCredentialPublicKey: OfflineCashDevicePublicKeyV1
  public let capabilityMask: UInt16
  public let qualificationReportDigest: Data
  public let validFromMS: UInt64
  public let expiresAtMS: UInt64

  public init(
    version: UInt16 = 1, protocolVersion: UInt16 = 1, hardwareProfileID: Data,
    providerID: Data, platformClass: OfflineCashHardwarePlatformClassV1,
    productClassDigest: Data, firmwarePolicyDigest: Data,
    enrollmentAttestationVerifierDigest: Data, attestationTrustRootsDigest: Data,
    allowedSuiteCommitment: Data, policyEpoch: UInt64,
    governanceCredentialPublicKey: OfflineCashDevicePublicKeyV1, capabilityMask: UInt16,
    qualificationReportDigest: Data, validFromMS: UInt64, expiresAtMS: UInt64
  ) throws {
    guard version == 1, protocolVersion == 1,
      capabilityMask == OfflineCashWireV1.requiredHardwareCapabilityMask,
      policyEpoch > 0, expiresAtMS > validFromMS
    else { throw offlineCashInvalid("hardwareProfile") }
    self.version = version
    self.protocolVersion = protocolVersion
    self.hardwareProfileID = try offlineCashDigest(hardwareProfileID, "hardwareProfileID")
    self.providerID = try offlineCashDigest(providerID, "providerID")
    self.platformClass = platformClass
    self.productClassDigest = try offlineCashDigest(productClassDigest, "productClassDigest")
    self.firmwarePolicyDigest = try offlineCashDigest(
      firmwarePolicyDigest, "firmwarePolicyDigest")
    self.enrollmentAttestationVerifierDigest = try offlineCashDigest(
      enrollmentAttestationVerifierDigest, "enrollmentAttestationVerifierDigest")
    self.attestationTrustRootsDigest = try offlineCashDigest(
      attestationTrustRootsDigest, "attestationTrustRootsDigest")
    self.allowedSuiteCommitment = try offlineCashDigest(
      allowedSuiteCommitment, "allowedSuiteCommitment")
    self.policyEpoch = policyEpoch
    self.governanceCredentialPublicKey = governanceCredentialPublicKey
    self.capabilityMask = capabilityMask
    self.qualificationReportDigest = try offlineCashDigest(
      qualificationReportDigest, "qualificationReportDigest")
    self.validFromMS = validFromMS
    self.expiresAtMS = expiresAtMS
  }
}

/// Compact governance credential consumed by the recursive hardware guard.
public struct OfflineCashHardwareCredentialV1: Equatable, Sendable {
  public let version: UInt16
  public let credentialID: Data
  public let networkID: Data
  public let hardwareProfileID: Data
  public let suiteID: Data
  public let firmwarePolicyDigest: Data
  public let policyEpoch: UInt64
  public let laneCommitment: Data
  public let hardwareEpochID: Data
  public let hardwareEpochGeneration: UInt64
  public let devicePublicKey: OfflineCashDevicePublicKeyV1
  public let deviceKeyReference: Data
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64
  public let governanceSignature: OfflineCashDeviceSignatureV1

  public init(
    version: UInt16 = 1, credentialID: Data, networkID: Data, hardwareProfileID: Data,
    suiteID: Data, firmwarePolicyDigest: Data, policyEpoch: UInt64, laneCommitment: Data,
    hardwareEpochID: Data, hardwareEpochGeneration: UInt64,
    devicePublicKey: OfflineCashDevicePublicKeyV1, deviceKeyReference: Data,
    issuedAtMS: UInt64, expiresAtMS: UInt64, governanceSignature: OfflineCashDeviceSignatureV1
  ) throws {
    guard version == 1, networkID.count == 32, networkID.contains(where: { $0 != 0 }),
      policyEpoch > 0, hardwareEpochGeneration > 0, expiresAtMS > issuedAtMS
    else { throw offlineCashInvalid("hardwareCredential") }
    self.version = version
    self.credentialID = try offlineCashDigest(credentialID, "credentialID")
    self.networkID = Data(networkID)
    self.hardwareProfileID = try offlineCashDigest(hardwareProfileID, "hardwareProfileID")
    self.suiteID = try offlineCashDigest(suiteID, "suiteID")
    self.firmwarePolicyDigest = try offlineCashDigest(
      firmwarePolicyDigest, "firmwarePolicyDigest")
    self.policyEpoch = policyEpoch
    self.laneCommitment = try offlineCashDigest(laneCommitment, "laneCommitment")
    self.hardwareEpochID = try offlineCashDigest(hardwareEpochID, "hardwareEpochID")
    self.hardwareEpochGeneration = hardwareEpochGeneration
    self.devicePublicKey = devicePublicKey
    self.deviceKeyReference = try offlineCashDigest(deviceKeyReference, "deviceKeyReference")
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
    self.governanceSignature = governanceSignature
  }
}

/// Inclusive per-payment amount interval.
public struct OfflineCashAmountPolicyV1: Equatable, Sendable {
  public let minimumAmount: OfflineCashUInt128V1
  public let maximumAmount: OfflineCashUInt128V1

  public init(minimumAmount: OfflineCashUInt128V1, maximumAmount: OfflineCashUInt128V1) throws {
    guard !minimumAmount.isZero,
      offlineCashCompare(minimumAmount, maximumAmount) != .orderedDescending
    else { throw offlineCashInvalid("amountPolicy") }
    self.minimumAmount = minimumAmount
    self.maximumAmount = maximumAmount
  }
}

/// Closed receiver request mode; every payment still needs a distinct ticket.
public enum OfflineCashPaymentRequestModeV1: Equatable, Sendable {
  case singleExact(amount: OfflineCashUInt128V1)
  case partialUntilTotal(totalAmount: OfflineCashUInt128V1)
  case boundedMultiPayment(maxPayments: UInt32, perPayment: OfflineCashAmountPolicyV1)
  case openReceive(perPayment: OfflineCashAmountPolicyV1)

  public func accepts(_ amount: OfflineCashUInt128V1) -> Bool {
    switch self {
    case .singleExact(let required): amount == required
    case .partialUntilTotal(let total):
      !amount.isZero && offlineCashCompare(amount, total) != .orderedDescending
    case .boundedMultiPayment(let maxPayments, let policy):
      maxPayments > 0 && offlineCashAmount(amount, isWithin: policy)
    case .openReceive(let policy): offlineCashAmount(amount, isWithin: policy)
    }
  }
}

/// Sender-selected, compact one-use intent.
public struct OfflineCashAcceptanceIntentV1: Equatable, Sendable {
  public let version: UInt16
  public let requestDigest: Data
  public let intentID: Data
  public let exactAmount: OfflineCashUInt128V1
  public let senderOneTimeCommitment: Data

  public init(
    version: UInt16 = 1, requestDigest: Data, intentID: Data,
    exactAmount: OfflineCashUInt128V1, senderOneTimeCommitment: Data
  ) throws {
    guard version == 1, !exactAmount.isZero else { throw offlineCashInvalid("acceptanceIntent") }
    self.version = version
    self.requestDigest = try offlineCashDigest(requestDigest, "requestDigest")
    self.intentID = try offlineCashDigest(intentID, "intentID")
    self.exactAmount = exactAmount
    self.senderOneTimeCommitment = try offlineCashDigest(
      senderOneTimeCommitment, "senderOneTimeCommitment")
  }
}

/// Release-bound statement for the pre-ticket sender authorization.
public struct OfflineCashAcceptanceIntentAuthorizationStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let intent: OfflineCashAcceptanceIntentV1
  public let releaseID: Data
  public let suiteID: Data
  public let vkDigest: Data
  public let artifactManifestDigest: Data

  public init(
    version: UInt16 = 1, intent: OfflineCashAcceptanceIntentV1, releaseID: Data,
    suiteID: Data, vkDigest: Data, artifactManifestDigest: Data
  ) throws {
    guard version == 1, intent.version == version else {
      throw offlineCashInvalid("acceptanceIntentAuthorizationStatement")
    }
    self.version = version
    self.intent = intent
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.suiteID = try offlineCashDigest(suiteID, "suiteID")
    self.vkDigest = try offlineCashDigest(vkDigest, "vkDigest")
    self.artifactManifestDigest = try offlineCashDigest(
      artifactManifestDigest, "artifactManifestDigest")
  }
}

/// Proof-bearing sender capability that must be verified before reservation.
public struct OfflineCashAcceptanceIntentAuthorizationV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: OfflineCashAcceptanceIntentAuthorizationStatementV1
  public let proof: OfflineCashPairedProofV1

  public init(
    version: UInt16 = 1, statement: OfflineCashAcceptanceIntentAuthorizationStatementV1,
    proof: OfflineCashPairedProofV1
  ) throws {
    guard version == 1, statement.version == version, proof.version == version else {
      throw offlineCashInvalid("acceptanceIntentAuthorization")
    }
    self.version = version
    self.statement = statement
    self.proof = proof
  }
}

/// Public release-pinned statement proving that one prepared sender authorization was cancelled.
///
/// The statement contains only unlinkable commitments and digests. It deliberately exposes no
/// sender lane, hardware counter, predecessor, or successor state.
public struct OfflineCashNoCommitClosureStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let releaseID: Data
  public let suiteID: Data
  public let vkDigest: Data
  public let artifactManifestDigest: Data
  public let senderHardwareBindingCommitment: Data
  public let requestID: Data
  public let requestDigest: Data
  public let acceptanceTicketID: Data
  public let ticketDigest: Data
  public let intentAuthorizationDigest: Data
  public let intentDigest: Data
  public let exactAmount: OfflineCashUInt128V1
  public let senderOneTimeCommitment: Data
  public let recoveryID: Data
  public let cancellationNullifier: Data
  public let equivalentDeliverySlotCommitment: Data

  public init(
    version: UInt16 = 1, releaseID: Data, suiteID: Data, vkDigest: Data,
    artifactManifestDigest: Data, senderHardwareBindingCommitment: Data,
    requestID: Data, requestDigest: Data, acceptanceTicketID: Data,
    ticketDigest: Data, intentAuthorizationDigest: Data, intentDigest: Data,
    exactAmount: OfflineCashUInt128V1, senderOneTimeCommitment: Data,
    recoveryID: Data, cancellationNullifier: Data,
    equivalentDeliverySlotCommitment: Data
  ) throws {
    guard version == 1, !exactAmount.isZero else {
      throw offlineCashInvalid("noCommitClosureStatement")
    }
    self.version = version
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.suiteID = try offlineCashDigest(suiteID, "suiteID")
    self.vkDigest = try offlineCashDigest(vkDigest, "vkDigest")
    self.artifactManifestDigest = try offlineCashDigest(
      artifactManifestDigest, "artifactManifestDigest")
    self.senderHardwareBindingCommitment = try offlineCashDigest(
      senderHardwareBindingCommitment, "senderHardwareBindingCommitment")
    self.requestID = try offlineCashDigest(requestID, "requestID")
    self.requestDigest = try offlineCashDigest(requestDigest, "requestDigest")
    self.acceptanceTicketID = try offlineCashDigest(
      acceptanceTicketID, "acceptanceTicketID")
    self.ticketDigest = try offlineCashDigest(ticketDigest, "ticketDigest")
    self.intentAuthorizationDigest = try offlineCashDigest(
      intentAuthorizationDigest, "intentAuthorizationDigest")
    self.intentDigest = try offlineCashDigest(intentDigest, "intentDigest")
    self.exactAmount = exactAmount
    self.senderOneTimeCommitment = try offlineCashDigest(
      senderOneTimeCommitment, "senderOneTimeCommitment")
    self.recoveryID = try offlineCashDigest(recoveryID, "recoveryID")
    self.cancellationNullifier = try offlineCashDigest(
      cancellationNullifier, "cancellationNullifier")
    self.equivalentDeliverySlotCommitment = try offlineCashDigest(
      equivalentDeliverySlotCommitment, "equivalentDeliverySlotCommitment")
  }
}

/// Canonical public recovery envelope for one irreversible no-commit closure.
///
/// This type and its codec perform structural checks only. The paired proof is always verified by
/// the authenticated native core and qualified hardware service.
public struct OfflineCashNoCommitClosureV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: OfflineCashNoCommitClosureStatementV1
  public let request: OfflineCashPaymentRequestV1
  public let intentAuthorization: OfflineCashAcceptanceIntentAuthorizationV1
  public let acceptanceTicket: OfflineCashAcceptanceTicketV1
  public let proof: OfflineCashPairedProofV1

  public init(
    version: UInt16 = 1, statement: OfflineCashNoCommitClosureStatementV1,
    request: OfflineCashPaymentRequestV1,
    intentAuthorization: OfflineCashAcceptanceIntentAuthorizationV1,
    acceptanceTicket: OfflineCashAcceptanceTicketV1,
    proof: OfflineCashPairedProofV1
  ) throws {
    guard version == 1, statement.version == version, request.version == version,
      intentAuthorization.version == version, acceptanceTicket.version == version,
      proof.version == version
    else { throw offlineCashInvalid("noCommitClosure") }
    self.version = version
    self.statement = statement
    self.request = request
    self.intentAuthorization = intentAuthorization
    self.acceptanceTicket = acceptanceTicket
    self.proof = proof
  }
}

/// One-use receiver-hardware capacity reservation.
public struct OfflineCashAcceptanceTicketV1: Equatable, Sendable {
  public let version: UInt16
  public let networkID: Data
  public let requestID: Data
  public let requestDigest: Data
  public let acceptanceTicketID: Data
  public let asset: OfflineCashAssetDefinitionIDV1
  public let assetIncarnation: OfflineCashAssetIncarnationV1
  public let scale: UInt32
  public let requestMode: OfflineCashPaymentRequestModeV1
  public let intentDigest: Data
  public let exactAmount: OfflineCashUInt128V1
  public let reservedInboxBytes: UInt32
  public let recipientOneTimeKey: OfflineCashX25519PublicKeyV1
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64
  public let signature: OfflineCashDeviceSignatureV1

  public init(
    version: UInt16 = 1, networkID: Data, requestID: Data, requestDigest: Data,
    acceptanceTicketID: Data, asset: OfflineCashAssetDefinitionIDV1,
    assetIncarnation: OfflineCashAssetIncarnationV1, scale: UInt32,
    requestMode: OfflineCashPaymentRequestModeV1, intentDigest: Data,
    exactAmount: OfflineCashUInt128V1, reservedInboxBytes: UInt32,
    recipientOneTimeKey: OfflineCashX25519PublicKeyV1, hardwareProfileID: Data,
    policyEpoch: UInt64, issuedAtMS: UInt64, expiresAtMS: UInt64,
    signature: OfflineCashDeviceSignatureV1
  ) throws {
    try offlineCashHeader(version, networkID, scale)
    guard !exactAmount.isZero, requestMode.accepts(exactAmount),
      reservedInboxBytes >= OfflineCashWireV1.minimumReservedInboxBytes,
      policyEpoch > 0, expiresAtMS > issuedAtMS
    else { throw offlineCashInvalid("acceptanceTicket") }
    self.version = version
    self.networkID = Data(networkID)
    self.requestID = try offlineCashDigest(requestID, "requestID")
    self.requestDigest = try offlineCashDigest(requestDigest, "requestDigest")
    self.acceptanceTicketID = try offlineCashDigest(
      acceptanceTicketID, "acceptanceTicketID")
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.requestMode = requestMode
    self.intentDigest = try offlineCashDigest(intentDigest, "intentDigest")
    self.exactAmount = exactAmount
    self.reservedInboxBytes = reservedInboxBytes
    self.recipientOneTimeKey = recipientOneTimeKey
    self.hardwareProfileID = try offlineCashDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
    self.signature = signature
  }
}

/// Exact recipient-only plaintext protected by an encrypted credit envelope.
public struct OfflineCashCreditOpeningV1: Equatable, Sendable {
  public let version: UInt16
  public let creditID: Data
  public let amount: OfflineCashUInt128V1
  public let creditCommitmentOpening: Data
  public let recipientBindingOpening: Data
  public let recoveryNonce: Data

  public init(
    version: UInt16 = 1, creditID: Data, amount: OfflineCashUInt128V1,
    creditCommitmentOpening: Data, recipientBindingOpening: Data, recoveryNonce: Data
  ) throws {
    guard version == 1, !amount.isZero else { throw offlineCashInvalid("creditOpening") }
    self.version = version
    self.creditID = try offlineCashDigest(creditID, "creditID")
    self.amount = amount
    self.creditCommitmentOpening = try offlineCashDigest(
      creditCommitmentOpening, "creditCommitmentOpening")
    self.recipientBindingOpening = try offlineCashDigest(
      recipientBindingOpening, "recipientBindingOpening")
    self.recoveryNonce = try offlineCashDigest(recoveryNonce, "recoveryNonce")
  }
}

/// Domain selector carried by encrypted-credit associated data.
public enum OfflineCashEncryptedCreditPurposeV1: UInt32, CaseIterable, Sendable {
  case mint = 0
  case peer = 1
}

/// Canonical associated data authenticated by an encrypted credit.
public struct OfflineCashEncryptedCreditAADV1: Equatable, Sendable {
  public let version: UInt16
  public let purpose: OfflineCashEncryptedCreditPurposeV1
  public let contextDigest: Data
  public let issuanceOrTransitionCommitment: Data
  public let creditID: Data
  public let amount: OfflineCashUInt128V1

  public init(
    version: UInt16 = 1, purpose: OfflineCashEncryptedCreditPurposeV1,
    contextDigest: Data, issuanceOrTransitionCommitment: Data, creditID: Data,
    amount: OfflineCashUInt128V1
  ) throws {
    guard version == 1, !amount.isZero else { throw offlineCashInvalid("encryptedCreditAAD") }
    self.version = version
    self.purpose = purpose
    self.contextDigest = try offlineCashDigest(contextDigest, "contextDigest")
    self.issuanceOrTransitionCommitment = try offlineCashDigest(
      issuanceOrTransitionCommitment, "issuanceOrTransitionCommitment")
    self.creditID = try offlineCashDigest(creditID, "creditID")
    self.amount = amount
  }
}

/// X25519/HKDF-SHA256/XChaCha20-Poly1305 encrypted-credit envelope.
public struct OfflineCashEncryptedCreditEnvelopeV1: Equatable, Sendable {
  public let version: UInt16
  public let ephemeralX25519PublicKey: OfflineCashX25519PublicKeyV1
  public let nonce: Data
  public let ciphertextAndTag: Data

  public init(
    version: UInt16 = 1, ephemeralX25519PublicKey: OfflineCashX25519PublicKeyV1,
    nonce: Data, ciphertextAndTag: Data
  ) throws {
    guard version == 1, nonce.count == OfflineCashWireV1.xchachaNonceBytes,
      ciphertextAndTag.count > OfflineCashWireV1.xchachaTagBytes
    else { throw offlineCashInvalid("encryptedCreditEnvelope") }
    self.version = version
    self.ephemeralX25519PublicKey = ephemeralX25519PublicKey
    self.nonce = Data(nonce)
    self.ciphertextAndTag = Data(ciphertextAndTag)
  }
}

/// Monetary operation bound by a released V1 transition.
public enum OfflineCashOperationKindV1: UInt32, CaseIterable, Sendable {
  case bootstrap = 0
  case mintFold = 1
  case sendSplit = 2
  case receiveFoldBatch = 3
  case redeemSplit = 4
  case suiteUpgrade = 5
  case rotate = 6
}

/// Complete public lifecycle context, without history or private state links.
public struct OfflineCashLifecycleBindingV1: Equatable, Sendable {
  public let version: UInt16
  public let networkID: Data
  public let protocolVersion: UInt16
  public let suiteID: Data
  public let vkDigest: Data
  public let releaseID: Data
  public let asset: OfflineCashAssetDefinitionIDV1
  public let assetIncarnation: OfflineCashAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let operationKind: OfflineCashOperationKindV1
  public let requestID: Data
  public let acceptanceTicketID: Data
  public let creditID: Data
  public let ciphertextDigest: Data

  public init(
    version: UInt16 = 1, networkID: Data, protocolVersion: UInt16 = 1,
    suiteID: Data, vkDigest: Data, releaseID: Data, asset: OfflineCashAssetDefinitionIDV1,
    assetIncarnation: OfflineCashAssetIncarnationV1, scale: UInt32, liabilityPoolID: Data,
    hardwareProfileID: Data, policyEpoch: UInt64, operationKind: OfflineCashOperationKindV1,
    requestID: Data, acceptanceTicketID: Data, creditID: Data, ciphertextDigest: Data
  ) throws {
    try offlineCashHeader(version, networkID, scale)
    guard protocolVersion == 1, policyEpoch > 0 else { throw offlineCashInvalid("lifecycle") }
    let zero = Data(repeating: 0, count: 32)
    let validOperationFields: Bool
    switch operationKind {
    case .sendSplit:
      validOperationFields = [requestID, acceptanceTicketID, creditID, ciphertextDigest]
        .allSatisfy(offlineCashIsDigest)
    case .mintFold:
      validOperationFields =
        requestID == zero && acceptanceTicketID == zero
        && offlineCashIsDigest(creditID) && offlineCashIsDigest(ciphertextDigest)
    case .bootstrap, .receiveFoldBatch, .redeemSplit, .suiteUpgrade, .rotate:
      validOperationFields = [requestID, acceptanceTicketID, creditID, ciphertextDigest]
        .allSatisfy { $0 == zero }
    }
    guard validOperationFields else { throw offlineCashInvalid("lifecycle.operationFields") }
    self.version = version
    self.networkID = Data(networkID)
    self.protocolVersion = protocolVersion
    self.suiteID = try offlineCashDigest(suiteID, "suiteID")
    self.vkDigest = try offlineCashDigest(vkDigest, "vkDigest")
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try offlineCashDigest(liabilityPoolID, "liabilityPoolID")
    self.hardwareProfileID = try offlineCashDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.operationKind = operationKind
    self.requestID = Data(requestID)
    self.acceptanceTicketID = Data(acceptanceTicketID)
    self.creditID = Data(creditID)
    self.ciphertextDigest = Data(ciphertextDigest)
  }
}

/// Public evidence that hardware committed before the applicable deadline.
public enum OfflineCashCommitEvidenceV1: Equatable, Sendable {
  case trustedTime(commitment: Data)
  case monotonicLease(commitment: Data)

  var commitment: Data {
    switch self {
    case .trustedTime(let commitment), .monotonicLease(let commitment): commitment
    }
  }
}

/// Durable sender-outbox capacity reserved before a predecessor is locked.
public struct OfflineCashOutboxReservationV1: Equatable, Sendable {
  public let reservationID: Data
  public let operationKind: OfflineCashOperationKindV1
  public let reservedOutboxBytes: UInt32
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64

  public init(
    reservationID: Data, operationKind: OfflineCashOperationKindV1,
    reservedOutboxBytes: UInt32, issuedAtMS: UInt64, expiresAtMS: UInt64
  ) throws {
    let minimum: UInt32
    switch operationKind {
    case .sendSplit:
      minimum = UInt32(OfflineCashWireV1.minimumPaymentOutboxBytes)
    case .redeemSplit:
      minimum = UInt32(OfflineCashWireV1.minimumRedemptionOutboxBytes)
    case .bootstrap, .mintFold, .receiveFoldBatch, .suiteUpgrade, .rotate:
      throw offlineCashInvalid("outboxReservation.operationKind")
    }
    guard reservedOutboxBytes >= minimum, issuedAtMS < expiresAtMS else {
      throw offlineCashInvalid("outboxReservation")
    }
    self.reservationID = try offlineCashDigest(reservationID, "reservationID")
    self.operationKind = operationKind
    self.reservedOutboxBytes = reservedOutboxBytes
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
  }
}

/// Recoverable hardware terminal certificate emitted by atomic commit.
public struct OfflineCashCommitCertificateV1: Equatable, Sendable {
  public let version: UInt16
  public let certificateID: Data
  public let candidateEnvelopeDigest: Data
  public let lifecycleBindingDigest: Data
  public let transitionNullifier: Data
  public let outboxReservationCommitment: Data
  public let commitEvidence: OfflineCashCommitEvidenceV1
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let hardwareTerminalCommitment: Data

  public init(
    version: UInt16 = 1, certificateID: Data, candidateEnvelopeDigest: Data,
    lifecycleBindingDigest: Data, transitionNullifier: Data,
    outboxReservationCommitment: Data, commitEvidence: OfflineCashCommitEvidenceV1,
    hardwareProfileID: Data, policyEpoch: UInt64, hardwareTerminalCommitment: Data
  ) throws {
    guard version == 1, policyEpoch > 0, offlineCashIsDigest(commitEvidence.commitment) else {
      throw offlineCashInvalid("commitCertificate")
    }
    self.version = version
    self.certificateID = try offlineCashDigest(certificateID, "certificateID")
    self.candidateEnvelopeDigest = try offlineCashDigest(
      candidateEnvelopeDigest, "candidateEnvelopeDigest")
    self.lifecycleBindingDigest = try offlineCashDigest(
      lifecycleBindingDigest, "lifecycleBindingDigest")
    self.transitionNullifier = try offlineCashDigest(transitionNullifier, "transitionNullifier")
    self.outboxReservationCommitment = try offlineCashDigest(
      outboxReservationCommitment, "outboxReservationCommitment")
    self.commitEvidence = commitEvidence
    self.hardwareProfileID = try offlineCashDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.hardwareTerminalCommitment = try offlineCashDigest(
      hardwareTerminalCommitment, "hardwareTerminalCommitment")
  }
}

/// Final paired proof that turns a prepared transition into committed money.
public struct OfflineCashCommitWrapperProofV1: Equatable, Sendable {
  public let version: UInt16
  public let eqProtocolDigest: Data
  public let epProtocolDigest: Data
  public let semanticDigest: Data
  public let candidateEnvelopeDigest: Data
  public let commitCertificateDigest: Data
  public let eqDeferredAudit: Data
  public let epDeferredAudit: Data
  public let eqProof: Data
  public let epProof: Data
  public let eqHistory: Data
  public let epHistory: Data

  public init(
    version: UInt16 = 1, eqProtocolDigest: Data, epProtocolDigest: Data,
    semanticDigest: Data, candidateEnvelopeDigest: Data, commitCertificateDigest: Data,
    eqDeferredAudit: Data, epDeferredAudit: Data, eqProof: Data, epProof: Data,
    eqHistory: Data, epHistory: Data
  ) throws {
    guard version == 1, !eqProof.isEmpty, !epProof.isEmpty,
      eqProof.count <= OfflineCashWireV1.maximumParityProofBytes,
      epProof.count <= OfflineCashWireV1.maximumParityProofBytes,
      eqHistory.count == OfflineCashWireV1.historyAccumulatorBytes,
      epHistory.count == OfflineCashWireV1.historyAccumulatorBytes, eqHistory != epHistory
    else { throw offlineCashInvalid("commitWrapperProof") }
    self.version = version
    self.eqProtocolDigest = try offlineCashDigest(eqProtocolDigest, "eqProtocolDigest")
    self.epProtocolDigest = try offlineCashDigest(epProtocolDigest, "epProtocolDigest")
    self.semanticDigest = try offlineCashDigest(semanticDigest, "semanticDigest")
    self.candidateEnvelopeDigest = try offlineCashDigest(
      candidateEnvelopeDigest, "candidateEnvelopeDigest")
    self.commitCertificateDigest = try offlineCashDigest(
      commitCertificateDigest, "commitCertificateDigest")
    self.eqDeferredAudit = try offlineCashDigest(eqDeferredAudit, "eqDeferredAudit")
    self.epDeferredAudit = try offlineCashDigest(epDeferredAudit, "epDeferredAudit")
    guard self.eqProtocolDigest != self.epProtocolDigest,
      self.eqDeferredAudit != self.epDeferredAudit
    else { throw offlineCashInvalid("commitWrapperProof.role") }
    self.eqProof = Data(eqProof)
    self.epProof = Data(epProof)
    self.eqHistory = Data(eqHistory)
    self.epHistory = Data(epHistory)
  }
}

/// Receiver-created reusable request; every actual payment uses a distinct ticket.
public struct OfflineCashPaymentRequestV1: Equatable, Sendable {
  public let version: UInt16
  public let releaseID: Data
  public let networkID: Data
  public let asset: OfflineCashAssetDefinitionIDV1
  public let assetIncarnation: OfflineCashAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let recipient: OfflineCashAccountIDV1
  public let requestMode: OfflineCashPaymentRequestModeV1
  public let hardwareCredential: OfflineCashHardwareCredentialV1
  public let requestID: Data
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64
  public let signature: OfflineCashDeviceSignatureV1

  public init(
    version: UInt16 = 1, releaseID: Data, networkID: Data,
    asset: OfflineCashAssetDefinitionIDV1, assetIncarnation: OfflineCashAssetIncarnationV1,
    scale: UInt32, liabilityPoolID: Data, recipient: OfflineCashAccountIDV1,
    requestMode: OfflineCashPaymentRequestModeV1,
    hardwareCredential: OfflineCashHardwareCredentialV1, requestID: Data,
    issuedAtMS: UInt64, expiresAtMS: UInt64, signature: OfflineCashDeviceSignatureV1
  ) throws {
    try offlineCashHeader(version, networkID, scale)
    guard hardwareCredential.networkID == networkID, expiresAtMS > issuedAtMS,
      expiresAtMS - issuedAtMS <= OfflineCashWireV1.requestMaximumTTLMS,
      expiresAtMS <= hardwareCredential.expiresAtMS
    else { throw offlineCashInvalid("paymentRequest") }
    self.version = version
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try offlineCashDigest(liabilityPoolID, "liabilityPoolID")
    self.recipient = recipient
    self.requestMode = requestMode
    self.hardwareCredential = hardwareCredential
    self.requestID = try offlineCashDigest(requestID, "requestID")
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
    self.signature = signature
  }
}

/// Unlinkable public send statement decided by both wrapper-proof parities.
public struct OfflineCashTransferStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let lifecycle: OfflineCashLifecycleBindingV1
  public let amount: OfflineCashUInt128V1
  public let transitionNullifier: Data
  public let requestDigest: Data
  public let acceptanceTicketDigest: Data
  public let recipientOneTimeKey: OfflineCashX25519PublicKeyV1
  public let ciphertextCommitment: Data
  public let commitEvidence: OfflineCashCommitEvidenceV1

  public init(
    version: UInt16 = 1, lifecycle: OfflineCashLifecycleBindingV1,
    amount: OfflineCashUInt128V1, transitionNullifier: Data, requestDigest: Data,
    acceptanceTicketDigest: Data, recipientOneTimeKey: OfflineCashX25519PublicKeyV1,
    ciphertextCommitment: Data, commitEvidence: OfflineCashCommitEvidenceV1
  ) throws {
    guard version == 1, lifecycle.version == version, lifecycle.operationKind == .sendSplit,
      !amount.isZero, offlineCashIsDigest(commitEvidence.commitment)
    else { throw offlineCashInvalid("transferStatement") }
    self.version = version
    self.lifecycle = lifecycle
    self.amount = amount
    self.transitionNullifier = try offlineCashDigest(transitionNullifier, "transitionNullifier")
    self.requestDigest = try offlineCashDigest(requestDigest, "requestDigest")
    self.acceptanceTicketDigest = try offlineCashDigest(
      acceptanceTicketDigest, "acceptanceTicketDigest")
    self.recipientOneTimeKey = recipientOneTimeKey
    self.ciphertextCommitment = try offlineCashDigest(
      ciphertextCommitment, "ciphertextCommitment")
    self.commitEvidence = commitEvidence
  }
}

/// Sender response with one receiver-bound encrypted credit and no public state links.
public struct OfflineCashPaymentV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: OfflineCashTransferStatementV1
  public let acceptanceIntent: OfflineCashAcceptanceIntentV1
  public let acceptanceTicket: OfflineCashAcceptanceTicketV1
  public let commitCertificate: OfflineCashCommitCertificateV1
  public let proof: OfflineCashCommitWrapperProofV1
  public let encryptedCredit: OfflineCashEncryptedCreditEnvelopeV1
  public let artifactManifestDigest: Data

  public init(
    version: UInt16 = 1, statement: OfflineCashTransferStatementV1,
    acceptanceIntent: OfflineCashAcceptanceIntentV1,
    acceptanceTicket: OfflineCashAcceptanceTicketV1,
    commitCertificate: OfflineCashCommitCertificateV1,
    proof: OfflineCashCommitWrapperProofV1,
    encryptedCredit: OfflineCashEncryptedCreditEnvelopeV1,
    artifactManifestDigest: Data
  ) throws {
    guard version == 1, statement.version == version, acceptanceIntent.version == version,
      acceptanceTicket.version == version, commitCertificate.version == version,
      proof.version == version, encryptedCredit.version == version
    else { throw offlineCashInvalid("payment") }
    self.version = version
    self.statement = statement
    self.acceptanceIntent = acceptanceIntent
    self.acceptanceTicket = acceptanceTicket
    self.commitCertificate = commitCertificate
    self.proof = proof
    self.encryptedCredit = encryptedCredit
    self.artifactManifestDigest = try offlineCashDigest(
      artifactManifestDigest, "artifactManifestDigest")
  }
}

/// Durable secure-inbox record named by a receiver acknowledgement.
public struct OfflineCashInboxReceiptV1: Equatable, Sendable {
  public let version: UInt16
  public let creditID: Data
  public let receiptCommitment: Data

  public init(version: UInt16 = 1, creditID: Data, receiptCommitment: Data) throws {
    guard version == 1 else { throw offlineCashInvalid("inboxReceipt") }
    self.version = version
    self.creditID = try offlineCashDigest(creditID, "creditID")
    self.receiptCommitment = try offlineCashDigest(receiptCommitment, "receiptCommitment")
  }
}

/// Receiver acknowledgement emitted only after durable inbox persistence.
public struct OfflineCashAcknowledgementV1: Equatable, Sendable {
  public let version: UInt16
  public let requestDigest: Data
  public let paymentDigest: Data
  public let inboxReceipt: OfflineCashInboxReceiptV1
  public let signature: OfflineCashDeviceSignatureV1

  public init(
    version: UInt16 = 1, requestDigest: Data, paymentDigest: Data,
    inboxReceipt: OfflineCashInboxReceiptV1, signature: OfflineCashDeviceSignatureV1
  ) throws {
    guard version == 1, inboxReceipt.version == version else {
      throw offlineCashInvalid("acknowledgement")
    }
    self.version = version
    self.requestDigest = try offlineCashDigest(requestDigest, "requestDigest")
    self.paymentDigest = try offlineCashDigest(paymentDigest, "paymentDigest")
    self.inboxReceipt = inboxReceipt
    self.signature = signature
  }
}

/// Pre-ID recipient context authorized before reserve debit.
public struct OfflineCashMintAuthorizationContextV1: Equatable, Sendable {
  public let version: UInt16
  public let operationID: Data
  public let releaseID: Data
  public let suiteID: Data
  public let vkDigest: Data
  public let artifactManifestDigest: Data
  public let networkID: Data
  public let asset: OfflineCashAssetDefinitionIDV1
  public let assetIncarnation: OfflineCashAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let amount: OfflineCashUInt128V1
  public let payer: OfflineCashAccountIDV1
  public let recipient: OfflineCashAccountIDV1
  public let hardwareCredentialID: Data
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let recipientCredentialCommitment: Data
  public let creditCommitment: Data
  public let recipientOneTimeKey: OfflineCashX25519PublicKeyV1

  public init(
    version: UInt16 = 1, operationID: Data, releaseID: Data, suiteID: Data,
    vkDigest: Data, artifactManifestDigest: Data, networkID: Data,
    asset: OfflineCashAssetDefinitionIDV1, assetIncarnation: OfflineCashAssetIncarnationV1,
    scale: UInt32, liabilityPoolID: Data, amount: OfflineCashUInt128V1,
    payer: OfflineCashAccountIDV1, recipient: OfflineCashAccountIDV1,
    hardwareCredentialID: Data, hardwareProfileID: Data, policyEpoch: UInt64,
    recipientCredentialCommitment: Data, creditCommitment: Data,
    recipientOneTimeKey: OfflineCashX25519PublicKeyV1
  ) throws {
    try offlineCashHeader(version, networkID, scale)
    guard !amount.isZero, policyEpoch > 0 else { throw offlineCashInvalid("mintContext") }
    self.version = version
    self.operationID = try offlineCashDigest(operationID, "operationID")
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.suiteID = try offlineCashDigest(suiteID, "suiteID")
    self.vkDigest = try offlineCashDigest(vkDigest, "vkDigest")
    self.artifactManifestDigest = try offlineCashDigest(
      artifactManifestDigest, "artifactManifestDigest")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try offlineCashDigest(liabilityPoolID, "liabilityPoolID")
    self.amount = amount
    self.payer = payer
    self.recipient = recipient
    self.hardwareCredentialID = try offlineCashDigest(
      hardwareCredentialID, "hardwareCredentialID")
    self.hardwareProfileID = try offlineCashDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.recipientCredentialCommitment = try offlineCashDigest(
      recipientCredentialCommitment, "recipientCredentialCommitment")
    self.creditCommitment = try offlineCashDigest(creditCommitment, "creditCommitment")
    self.recipientOneTimeKey = recipientOneTimeKey
  }
}

/// Exact pre-debit mint authorization statement.
public struct OfflineCashMintAuthorizationStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let context: OfflineCashMintAuthorizationContextV1
  public let issuanceCommitment: Data
  public let creditID: Data
  public let ciphertextDigest: Data

  public init(
    version: UInt16 = 1, context: OfflineCashMintAuthorizationContextV1,
    issuanceCommitment: Data, creditID: Data, ciphertextDigest: Data
  ) throws {
    guard version == 1, context.version == version else {
      throw offlineCashInvalid("mintAuthorizationStatement")
    }
    self.version = version
    self.context = context
    self.issuanceCommitment = try offlineCashDigest(issuanceCommitment, "issuanceCommitment")
    self.creditID = try offlineCashDigest(creditID, "creditID")
    self.ciphertextDigest = try offlineCashDigest(ciphertextDigest, "ciphertextDigest")
  }
}

/// Release-pinned recipient authorization verified before reserve mutation.
public struct OfflineCashMintAuthorizationV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: OfflineCashMintAuthorizationStatementV1
  public let proof: OfflineCashPairedProofV1

  public init(
    version: UInt16 = 1, statement: OfflineCashMintAuthorizationStatementV1,
    proof: OfflineCashPairedProofV1
  ) throws {
    guard version == 1, statement.version == version, proof.version == version else {
      throw offlineCashInvalid("mintAuthorization")
    }
    self.version = version
    self.statement = statement
    self.proof = proof
  }
}

/// Public top-up statement creating one foldable aggregate credit.
public struct OfflineCashMintCreditStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let lifecycle: OfflineCashLifecycleBindingV1
  public let recipientCredentialCommitment: Data
  public let authorizationContextDigest: Data
  public let mintAuthorizationDigest: Data
  public let amount: OfflineCashUInt128V1
  public let issuanceCommitment: Data
  public let recipient: OfflineCashAccountIDV1
  public let creditCommitment: Data
  public let mintedAtMS: UInt64

  public init(
    version: UInt16 = 1, lifecycle: OfflineCashLifecycleBindingV1,
    recipientCredentialCommitment: Data, authorizationContextDigest: Data,
    mintAuthorizationDigest: Data, amount: OfflineCashUInt128V1,
    issuanceCommitment: Data, recipient: OfflineCashAccountIDV1,
    creditCommitment: Data, mintedAtMS: UInt64
  ) throws {
    guard version == 1, lifecycle.version == version, lifecycle.operationKind == .mintFold,
      !amount.isZero, mintedAtMS > 0
    else { throw offlineCashInvalid("mintCreditStatement") }
    self.version = version
    self.lifecycle = lifecycle
    self.recipientCredentialCommitment = try offlineCashDigest(
      recipientCredentialCommitment, "recipientCredentialCommitment")
    self.authorizationContextDigest = try offlineCashDigest(
      authorizationContextDigest, "authorizationContextDigest")
    self.mintAuthorizationDigest = try offlineCashDigest(
      mintAuthorizationDigest, "mintAuthorizationDigest")
    self.amount = amount
    self.issuanceCommitment = try offlineCashDigest(issuanceCommitment, "issuanceCommitment")
    self.recipient = recipient
    self.creditCommitment = try offlineCashDigest(creditCommitment, "creditCommitment")
    self.mintedAtMS = mintedAtMS
  }
}

/// Constant-size authenticated top-up credit folded into aggregate state.
public struct OfflineCashMintCreditV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: OfflineCashMintCreditStatementV1
  public let proof: OfflineCashPairedProofV1
  public let finalityCertificateBinding: Data
  public let finalityAuthorityHead: Data
  public let finalityGenesisRosterID: Data
  public let finalityProofBindingDigest: Data
  public let encryptedCredit: OfflineCashEncryptedCreditEnvelopeV1
  public let artifactManifestDigest: Data

  public init(
    version: UInt16 = 1, statement: OfflineCashMintCreditStatementV1,
    proof: OfflineCashPairedProofV1, finalityCertificateBinding: Data,
    finalityAuthorityHead: Data, finalityGenesisRosterID: Data,
    finalityProofBindingDigest: Data,
    encryptedCredit: OfflineCashEncryptedCreditEnvelopeV1,
    artifactManifestDigest: Data
  ) throws {
    guard version == 1, statement.version == version, proof.version == version,
      encryptedCredit.version == version
    else { throw offlineCashInvalid("mintCredit") }
    self.version = version
    self.statement = statement
    self.proof = proof
    self.finalityCertificateBinding = try offlineCashDigest(
      finalityCertificateBinding, "finalityCertificateBinding")
    self.finalityAuthorityHead = try offlineCashDigest(
      finalityAuthorityHead, "finalityAuthorityHead")
    self.finalityGenesisRosterID = try offlineCashDigest(
      finalityGenesisRosterID, "finalityGenesisRosterID")
    self.finalityProofBindingDigest = try offlineCashDigest(
      finalityProofBindingDigest, "finalityProofBindingDigest")
    self.encryptedCredit = encryptedCredit
    self.artifactManifestDigest = try offlineCashDigest(
      artifactManifestDigest, "artifactManifestDigest")
  }
}

/// Unlinkable terminal transition that converts aggregate cash to an online claim.
public struct OfflineCashRedemptionStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let lifecycle: OfflineCashLifecycleBindingV1
  public let amount: OfflineCashUInt128V1
  public let beneficiary: OfflineCashAccountIDV1
  public let terminalNullifier: Data
  public let redemptionCommitment: Data
  public let redemptionID: Data
  public let commitEvidence: OfflineCashCommitEvidenceV1

  public init(
    version: UInt16 = 1, lifecycle: OfflineCashLifecycleBindingV1,
    amount: OfflineCashUInt128V1, beneficiary: OfflineCashAccountIDV1,
    terminalNullifier: Data, redemptionCommitment: Data, redemptionID: Data,
    commitEvidence: OfflineCashCommitEvidenceV1
  ) throws {
    guard version == 1, lifecycle.version == version, lifecycle.operationKind == .redeemSplit,
      !amount.isZero, offlineCashIsDigest(commitEvidence.commitment)
    else { throw offlineCashInvalid("redemptionStatement") }
    self.version = version
    self.lifecycle = lifecycle
    self.amount = amount
    self.beneficiary = beneficiary
    self.terminalNullifier = try offlineCashDigest(terminalNullifier, "terminalNullifier")
    self.redemptionCommitment = try offlineCashDigest(
      redemptionCommitment, "redemptionCommitment")
    self.redemptionID = try offlineCashDigest(redemptionID, "redemptionID")
    self.commitEvidence = commitEvidence
  }
}

/// Constant-size terminal voucher submitted for online redemption.
public struct OfflineCashRedemptionVoucherV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: OfflineCashRedemptionStatementV1
  public let commitCertificate: OfflineCashCommitCertificateV1
  public let proof: OfflineCashCommitWrapperProofV1
  public let artifactManifestDigest: Data

  public init(
    version: UInt16 = 1, statement: OfflineCashRedemptionStatementV1,
    commitCertificate: OfflineCashCommitCertificateV1,
    proof: OfflineCashCommitWrapperProofV1, artifactManifestDigest: Data
  ) throws {
    guard version == 1, statement.version == version, commitCertificate.version == version,
      proof.version == version
    else { throw offlineCashInvalid("redemptionVoucher") }
    self.version = version
    self.statement = statement
    self.commitCertificate = commitCertificate
    self.proof = proof
    self.artifactManifestDigest = try offlineCashDigest(
      artifactManifestDigest, "artifactManifestDigest")
  }
}

func offlineCashInvalid(_ field: String) -> OfflineCashWireEnvelopeErrorV1 {
  .invalidField(field)
}

func offlineCashIsDigest(_ value: Data) -> Bool {
  value.count == 32 && value.contains(where: { $0 != 0 })
}

func offlineCashDigest(_ value: Data, _ field: String) throws -> Data {
  guard offlineCashIsDigest(value) else { throw offlineCashInvalid(field) }
  return Data(value)
}

func offlineCashHeader(_ version: UInt16, _ networkID: Data, _ scale: UInt32) throws {
  guard version == 1, networkID.count == 32, networkID.contains(where: { $0 != 0 }),
    scale <= OfflineCashWireV1.maximumAssetScale
  else { throw offlineCashInvalid("header") }
}

private func offlineCashCompare(
  _ lhs: OfflineCashUInt128V1, _ rhs: OfflineCashUInt128V1
) -> ComparisonResult {
  for (left, right) in zip(lhs.littleEndianBytes.reversed(), rhs.littleEndianBytes.reversed()) {
    if left < right { return .orderedAscending }
    if left > right { return .orderedDescending }
  }
  return .orderedSame
}

private func offlineCashAmount(
  _ amount: OfflineCashUInt128V1, isWithin policy: OfflineCashAmountPolicyV1
) -> Bool {
  offlineCashCompare(amount, policy.minimumAmount) != .orderedAscending
    && offlineCashCompare(amount, policy.maximumAmount) != .orderedDescending
}
