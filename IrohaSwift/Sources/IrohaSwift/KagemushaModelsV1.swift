import Foundation

/// Exact unsigned 128-bit little-endian integer used by KAGEMUSHA V1.
public struct KagemushaUInt128V1: Equatable, Hashable, Sendable {
  public let littleEndianBytes: Data

  public init(littleEndianBytes: Data) throws {
    guard littleEndianBytes.count == 16 else { throw kagemushaInvalid("u128") }
    self.littleEndianBytes = Data(littleEndianBytes)
  }

  public init(_ value: UInt64) {
    var value = value.littleEndian
    var bytes = withUnsafeBytes(of: &value) { Data($0) }
    bytes.append(Data(repeating: 0, count: 8))
    littleEndianBytes = bytes
  }

  public var isZero: Bool { !littleEndianBytes.contains(where: { $0 != 0 }) }

  static let zero = KagemushaUInt128V1(0)

  func isLessThanOrEqual(to other: KagemushaUInt128V1) -> Bool {
    for index in littleEndianBytes.indices.reversed() {
      if littleEndianBytes[index] != other.littleEndianBytes[index] {
        return littleEndianBytes[index] < other.littleEndianBytes[index]
      }
    }
    return true
  }

  func adding(_ other: KagemushaUInt128V1) throws -> KagemushaUInt128V1 {
    var result = Data(repeating: 0, count: 16)
    var carry: UInt16 = 0
    for index in result.indices {
      let sum =
        UInt16(littleEndianBytes[index])
        + UInt16(other.littleEndianBytes[index]) + carry
      result[index] = UInt8(truncatingIfNeeded: sum)
      carry = sum >> 8
    }
    guard carry == 0 else { throw kagemushaInvalid("u128.overflow") }
    return try KagemushaUInt128V1(littleEndianBytes: result)
  }

  func adding(_ value: UInt8) throws -> KagemushaUInt128V1 {
    var result = littleEndianBytes
    var carry = UInt16(value)
    for index in result.indices where carry != 0 {
      let sum = UInt16(result[index]) + carry
      result[index] = UInt8(truncatingIfNeeded: sum)
      carry = sum >> 8
    }
    guard carry == 0 else { throw kagemushaInvalid("u128.overflow") }
    return try KagemushaUInt128V1(littleEndianBytes: result)
  }
}

/// Exact typed `AssetDefinitionId` payload used by KAGEMUSHA V1.
public struct KagemushaAssetDefinitionIDV1: Equatable, Hashable, Sendable {
  public let canonicalPayload: Data

  public init(_ literal: String) throws {
    canonicalPayload = try CanonicalNorito.encodeCompactAssetDefinitionId(literal)
  }

  public init(canonicalPayload: Data) throws {
    guard !canonicalPayload.isEmpty, canonicalPayload.count <= 512 else {
      throw kagemushaInvalid("asset")
    }
    self.canonicalPayload = Data(canonicalPayload)
  }
}

/// Exact typed universal `AccountId` payload used by KAGEMUSHA V1.
public struct KagemushaAccountIDV1: Equatable, Hashable, Sendable {
  public let canonicalPayload: Data

  public init(_ literal: String) throws {
    canonicalPayload = try CanonicalNorito.encodeCompactAccountId(literal)
  }

  public init(canonicalPayload: Data) throws {
    guard !canonicalPayload.isEmpty, canonicalPayload.count <= 512,
      AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(canonicalPayload)
    else { throw kagemushaInvalid("account") }
    self.canonicalPayload = Data(canonicalPayload)
  }
}

/// Exact non-zero, marked asset-incarnation hash.
public struct KagemushaAssetIncarnationV1: Equatable, Hashable, Sendable {
  public let bytes: Data

  public init(bytes: Data) throws {
    guard bytes.count == 32, bytes.last.map({ $0 & 1 == 1 }) == true,
      bytes.dropLast().contains(where: { $0 != 0 }) || bytes.last != 1
    else { throw kagemushaInvalid("assetIncarnation") }
    self.bytes = Data(bytes)
  }
}

/// Fixed-width uncompressed SEC1 P-256 authority-key shape.
///
/// The release-pinned native core remains responsible for curve-point validation.
public struct KagemushaDevicePublicKeyV1: Equatable, Hashable, Sendable {
  public let sec1Bytes: Data

  public init(sec1Bytes: Data) throws {
    guard sec1Bytes.count == 65, sec1Bytes.first == 4,
      sec1Bytes.dropFirst().contains(where: { $0 != 0 })
    else { throw kagemushaInvalid("devicePublicKey") }
    self.sec1Bytes = Data(sec1Bytes)
  }
}

/// Canonical fixed-width low-S P-256 KAGEMUSHA V1 signature.
public struct KagemushaDeviceSignatureV1: Equatable, Hashable, Sendable {
  public let rawBytes: Data

  public init(rawBytes: Data) throws {
    guard rawBytes.count == 64 else { throw kagemushaInvalid("deviceSignature") }
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
    else { throw kagemushaInvalid("deviceSignature") }
    self.rawBytes = Data(rawBytes)
  }
}

/// Exact 32-byte X25519 public-key encoding.
public struct KagemushaX25519PublicKeyV1: Equatable, Hashable, Sendable {
  public let rawBytes: Data

  public init(rawBytes: Data) throws {
    guard rawBytes.count == KagemushaWireV1.x25519PublicKeyBytes,
      rawBytes.contains(where: { $0 != 0 })
    else { throw kagemushaInvalid("x25519PublicKey") }
    self.rawBytes = Data(rawBytes)
  }
}

/// Constant-size public metadata for one privately valued aggregate balance.
public struct KagemushaAggregateStateCommitmentV1: Equatable, Sendable {
  public let version: UInt16
  public let releaseID: Data
  public let networkID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let laneID: Data
  public let hardwareEpochID: Data
  public let keyReference: Data
  public let hardwarePolicyID: Data
  public let sequence: KagemushaUInt128V1
  public let stateCommitment: Data

  public init(
    version: UInt16 = 1, releaseID: Data, networkID: Data,
    asset: KagemushaAssetDefinitionIDV1, assetIncarnation: KagemushaAssetIncarnationV1,
    scale: UInt32, liabilityPoolID: Data, laneID: Data, hardwareEpochID: Data,
    keyReference: Data, hardwarePolicyID: Data, sequence: KagemushaUInt128V1,
    stateCommitment: Data
  ) throws {
    try kagemushaHeader(version, networkID, scale)
    self.version = version
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try kagemushaDigest(liabilityPoolID, "liabilityPoolID")
    self.laneID = try kagemushaDigest(laneID, "laneID")
    self.hardwareEpochID = try kagemushaDigest(hardwareEpochID, "hardwareEpochID")
    self.keyReference = try kagemushaDigest(keyReference, "keyReference")
    self.hardwarePolicyID = try kagemushaDigest(hardwarePolicyID, "hardwarePolicyID")
    self.sequence = sequence
    self.stateCommitment = try kagemushaDigest(stateCommitment, "stateCommitment")
  }
}

/// Pair of public Pasta-side commitments for one recursively proved state.
public struct KagemushaPastaStateCommitmentV1: Equatable, Sendable {
  public let eq: Data
  public let ep: Data

  public init(eq: Data, ep: Data) throws {
    guard eq.count == 32, ep.count == 32 else {
      throw kagemushaInvalid("pastaStateCommitment")
    }
    self.eq = Data(eq)
    self.ep = Data(ep)
  }

  public var isZero: Bool {
    eq.allSatisfy { $0 == 0 } && ep.allSatisfy { $0 == 0 }
  }
}

/// Closed paired-Pasta proof. State links and stable credential pseudonyms are absent.
public struct KagemushaPairedProofV1: Equatable, Sendable {
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
      eqProof.count <= KagemushaWireV1.maximumParityProofBytes,
      epProof.count <= KagemushaWireV1.maximumParityProofBytes,
      eqProof.count + epProof.count <= KagemushaWireV1.maximumCurrentProofsBytes,
      eqHistory.count == KagemushaWireV1.historyAccumulatorBytes,
      epHistory.count == KagemushaWireV1.historyAccumulatorBytes
    else { throw kagemushaInvalid("pairedProof") }
    self.version = version
    self.eqProtocolDigest = try kagemushaDigest(eqProtocolDigest, "eqProtocolDigest")
    self.epProtocolDigest = try kagemushaDigest(epProtocolDigest, "epProtocolDigest")
    self.semanticDigest = try kagemushaDigest(semanticDigest, "semanticDigest")
    self.guardEqCredentialAudit = try kagemushaDigest(
      guardEqCredentialAudit, "guardEqCredentialAudit")
    self.guardEpCredentialAudit = try kagemushaDigest(
      guardEpCredentialAudit, "guardEpCredentialAudit")
    self.eqDeferredAudit = try kagemushaDigest(eqDeferredAudit, "eqDeferredAudit")
    self.epDeferredAudit = try kagemushaDigest(epDeferredAudit, "epDeferredAudit")
    guard self.eqProtocolDigest != self.epProtocolDigest,
      self.guardEqCredentialAudit != self.guardEpCredentialAudit,
      self.eqDeferredAudit != self.epDeferredAudit, eqHistory != epHistory
    else { throw kagemushaInvalid("pairedProof.role") }
    self.eqProof = Data(eqProof)
    self.epProof = Data(epProof)
    self.eqHistory = Data(eqHistory)
    self.epHistory = Data(epHistory)
  }
}

/// Qualified platform class represented by one governed hardware profile.
public enum KagemushaHardwarePlatformClassV1: UInt32, CaseIterable, Sendable {
  case androidOEMService = 0
  case appleOEMService = 1
  case dedicatedSecureElement = 2
  case otherQualified = 3
}

/// Governed non-forking hardware-service profile.
public struct KagemushaHardwareProfileV1: Equatable, Sendable {
  public let version: UInt16
  public let protocolVersion: UInt16
  public let hardwareProfileID: Data
  public let providerID: Data
  public let platformClass: KagemushaHardwarePlatformClassV1
  public let productClassDigest: Data
  public let firmwarePolicyDigest: Data
  public let enrollmentAttestationVerifierDigest: Data
  public let attestationTrustRootsDigest: Data
  public let allowedSuiteCommitment: Data
  public let policyEpoch: UInt64
  public let governanceCredentialPublicKey: KagemushaDevicePublicKeyV1
  public let capabilityMask: UInt16
  public let qualificationReportDigest: Data
  public let validFromMS: UInt64
  public let expiresAtMS: UInt64

  public init(
    version: UInt16 = 1, protocolVersion: UInt16 = 1, hardwareProfileID: Data,
    providerID: Data, platformClass: KagemushaHardwarePlatformClassV1,
    productClassDigest: Data, firmwarePolicyDigest: Data,
    enrollmentAttestationVerifierDigest: Data, attestationTrustRootsDigest: Data,
    allowedSuiteCommitment: Data, policyEpoch: UInt64,
    governanceCredentialPublicKey: KagemushaDevicePublicKeyV1, capabilityMask: UInt16,
    qualificationReportDigest: Data, validFromMS: UInt64, expiresAtMS: UInt64
  ) throws {
    guard version == 1, protocolVersion == 1,
      capabilityMask == KagemushaWireV1.requiredHardwareCapabilityMask,
      policyEpoch > 0, expiresAtMS > validFromMS
    else { throw kagemushaInvalid("hardwareProfile") }
    self.version = version
    self.protocolVersion = protocolVersion
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.providerID = try kagemushaDigest(providerID, "providerID")
    self.platformClass = platformClass
    self.productClassDigest = try kagemushaDigest(productClassDigest, "productClassDigest")
    self.firmwarePolicyDigest = try kagemushaDigest(
      firmwarePolicyDigest, "firmwarePolicyDigest")
    self.enrollmentAttestationVerifierDigest = try kagemushaDigest(
      enrollmentAttestationVerifierDigest, "enrollmentAttestationVerifierDigest")
    self.attestationTrustRootsDigest = try kagemushaDigest(
      attestationTrustRootsDigest, "attestationTrustRootsDigest")
    self.allowedSuiteCommitment = try kagemushaDigest(
      allowedSuiteCommitment, "allowedSuiteCommitment")
    self.policyEpoch = policyEpoch
    self.governanceCredentialPublicKey = governanceCredentialPublicKey
    self.capabilityMask = capabilityMask
    self.qualificationReportDigest = try kagemushaDigest(
      qualificationReportDigest, "qualificationReportDigest")
    self.validFromMS = validFromMS
    self.expiresAtMS = expiresAtMS
  }
}

/// Compact governance credential consumed by the recursive hardware guard.
public struct KagemushaHardwareCredentialV1: Equatable, Sendable {
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
  public let devicePublicKey: KagemushaDevicePublicKeyV1
  public let deviceKeyReference: Data
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64
  public let governanceSignature: KagemushaDeviceSignatureV1

  public init(
    version: UInt16 = 1, credentialID: Data, networkID: Data, hardwareProfileID: Data,
    suiteID: Data, firmwarePolicyDigest: Data, policyEpoch: UInt64, laneCommitment: Data,
    hardwareEpochID: Data, hardwareEpochGeneration: UInt64,
    devicePublicKey: KagemushaDevicePublicKeyV1, deviceKeyReference: Data,
    issuedAtMS: UInt64, expiresAtMS: UInt64, governanceSignature: KagemushaDeviceSignatureV1
  ) throws {
    guard version == 1, networkID.count == 32, networkID.contains(where: { $0 != 0 }),
      policyEpoch > 0, hardwareEpochGeneration > 0, expiresAtMS > issuedAtMS
    else { throw kagemushaInvalid("hardwareCredential") }
    self.version = version
    self.credentialID = try kagemushaDigest(credentialID, "credentialID")
    self.networkID = Data(networkID)
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.suiteID = try kagemushaDigest(suiteID, "suiteID")
    self.firmwarePolicyDigest = try kagemushaDigest(
      firmwarePolicyDigest, "firmwarePolicyDigest")
    self.policyEpoch = policyEpoch
    self.laneCommitment = try kagemushaDigest(laneCommitment, "laneCommitment")
    self.hardwareEpochID = try kagemushaDigest(hardwareEpochID, "hardwareEpochID")
    self.hardwareEpochGeneration = hardwareEpochGeneration
    self.devicePublicKey = devicePublicKey
    self.deviceKeyReference = try kagemushaDigest(deviceKeyReference, "deviceKeyReference")
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
    self.governanceSignature = governanceSignature
  }
}

/// Exact pre-ID peer-transfer context authenticated by encrypted-credit AAD.
public struct KagemushaPeerCreditContextV1: Equatable, Sendable {
  public let version: UInt16
  public let requestDigest: Data
  public let amount: KagemushaUInt128V1
  public let senderBeforeCommitment: Data
  public let senderAfterCommitment: Data
  public let preparedTransferDigest: Data
  public let recipientEncryptionKey: KagemushaX25519PublicKeyV1

  public init(
    version: UInt16 = 1, requestDigest: Data, amount: KagemushaUInt128V1,
    senderBeforeCommitment: Data, senderAfterCommitment: Data,
    preparedTransferDigest: Data, recipientEncryptionKey: KagemushaX25519PublicKeyV1
  ) throws {
    guard version == 1, !amount.isZero, senderBeforeCommitment != senderAfterCommitment
    else { throw kagemushaInvalid("peerCreditContext") }
    self.version = version
    self.requestDigest = try kagemushaDigest(requestDigest, "requestDigest")
    self.amount = amount
    self.senderBeforeCommitment = try kagemushaDigest(
      senderBeforeCommitment, "senderBeforeCommitment")
    self.senderAfterCommitment = try kagemushaDigest(
      senderAfterCommitment, "senderAfterCommitment")
    self.preparedTransferDigest = try kagemushaDigest(
      preparedTransferDigest, "preparedTransferDigest")
    self.recipientEncryptionKey = recipientEncryptionKey
  }
}

/// Exact recipient-only plaintext protected by an encrypted credit envelope.
public struct KagemushaCreditOpeningV1: Equatable, Sendable {
  public let version: UInt16
  public let creditID: Data
  public let amount: KagemushaUInt128V1
  public let creditCommitmentOpening: Data
  public let recipientBindingOpening: Data
  public let recoveryNonce: Data

  public init(
    version: UInt16 = 1, creditID: Data, amount: KagemushaUInt128V1,
    creditCommitmentOpening: Data, recipientBindingOpening: Data, recoveryNonce: Data
  ) throws {
    guard version == 1, !amount.isZero else { throw kagemushaInvalid("creditOpening") }
    self.version = version
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.amount = amount
    self.creditCommitmentOpening = try kagemushaDigest(
      creditCommitmentOpening, "creditCommitmentOpening")
    self.recipientBindingOpening = try kagemushaDigest(
      recipientBindingOpening, "recipientBindingOpening")
    self.recoveryNonce = try kagemushaDigest(recoveryNonce, "recoveryNonce")
  }
}

/// Domain selector carried by encrypted-credit associated data.
public enum KagemushaEncryptedCreditPurposeV1: UInt32, CaseIterable, Sendable {
  case mint = 0
  case peer = 1
}

/// Canonical associated data authenticated by an encrypted credit.
public struct KagemushaEncryptedCreditAADV1: Equatable, Sendable {
  public let version: UInt16
  public let purpose: KagemushaEncryptedCreditPurposeV1
  public let contextDigest: Data
  public let issuanceOrTransitionCommitment: Data
  public let creditID: Data
  public let amount: KagemushaUInt128V1

  public init(
    version: UInt16 = 1, purpose: KagemushaEncryptedCreditPurposeV1,
    contextDigest: Data, issuanceOrTransitionCommitment: Data, creditID: Data,
    amount: KagemushaUInt128V1
  ) throws {
    guard version == 1, !amount.isZero else { throw kagemushaInvalid("encryptedCreditAAD") }
    self.version = version
    self.purpose = purpose
    self.contextDigest = try kagemushaDigest(contextDigest, "contextDigest")
    self.issuanceOrTransitionCommitment = try kagemushaDigest(
      issuanceOrTransitionCommitment, "issuanceOrTransitionCommitment")
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.amount = amount
  }
}

/// X25519/HKDF-SHA256/XChaCha20-Poly1305 encrypted-credit envelope.
public struct KagemushaEncryptedCreditEnvelopeV1: Equatable, Sendable {
  public let version: UInt16
  public let ephemeralX25519PublicKey: KagemushaX25519PublicKeyV1
  public let nonce: Data
  public let ciphertextAndTag: Data

  public init(
    version: UInt16 = 1, ephemeralX25519PublicKey: KagemushaX25519PublicKeyV1,
    nonce: Data, ciphertextAndTag: Data
  ) throws {
    guard version == 1, nonce.count == KagemushaWireV1.xchachaNonceBytes,
      ciphertextAndTag.count == KagemushaWireV1.encryptedCreditCiphertextAndTagBytes
    else { throw kagemushaInvalid("encryptedCreditEnvelope") }
    self.version = version
    self.ephemeralX25519PublicKey = ephemeralX25519PublicKey
    self.nonce = Data(nonce)
    self.ciphertextAndTag = Data(ciphertextAndTag)
  }
}

/// Qualified trusted-time evidence. Stable Norito tag: `0`.
public struct KagemushaTrustedCommitTimeV1: Equatable, Sendable {
  public let timeEvidenceCommitment: Data

  public init(timeEvidenceCommitment: Data) throws {
    self.timeEvidenceCommitment = try kagemushaDigest(
      timeEvidenceCommitment, "timeEvidenceCommitment")
  }
}

/// Secure monotonic-lease evidence. Stable Norito tag: `1`.
public struct KagemushaMonotonicLeaseV1: Equatable, Sendable {
  public let leaseEvidenceCommitment: Data

  public init(leaseEvidenceCommitment: Data) throws {
    self.leaseEvidenceCommitment = try kagemushaDigest(
      leaseEvidenceCommitment, "leaseEvidenceCommitment")
  }
}

/// Public evidence that qualified hardware committed before the applicable deadline.
public enum KagemushaCommitEvidenceV1: Equatable, Sendable {
  case trustedTime(KagemushaTrustedCommitTimeV1)
  case monotonicLease(KagemushaMonotonicLeaseV1)

  public var wireTag: UInt32 {
    switch self {
    case .trustedTime: 0
    case .monotonicLease: 1
    }
  }

  public var evidenceCommitment: Data {
    switch self {
    case .trustedTime(let value): value.timeEvidenceCommitment
    case .monotonicLease(let value): value.leaseEvidenceCommitment
    }
  }
}

/// Sender outbox capacity reserved before hardware may consume its predecessor.
public struct KagemushaOutboxReservationV1: Equatable, Sendable {
  public let reservationID: Data
  public let operationKind: KagemushaOperationKindV1
  public let reservedOutboxBytes: UInt32
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64

  public init(
    reservationID: Data, operationKind: KagemushaOperationKindV1,
    reservedOutboxBytes: UInt32, issuedAtMS: UInt64, expiresAtMS: UInt64
  ) throws {
    guard issuedAtMS < expiresAtMS else { throw kagemushaInvalid("outboxReservation") }
    self.reservationID = try kagemushaDigest(reservationID, "reservationID")
    self.operationKind = operationKind
    self.reservedOutboxBytes = reservedOutboxBytes
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
  }
}

/// Self-free terminal body committed before a certificate identity exists.
public struct KagemushaHardwareTerminalBodyV1: Equatable, Sendable {
  public let version: UInt16
  public let candidateEnvelopeDigest: Data
  public let lifecycleBindingDigest: Data
  public let transitionNullifier: Data
  public let outboxReservationCommitment: Data
  public let commitEvidence: KagemushaCommitEvidenceV1
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let privateSuccessorCommitment: Data
  public let privateJournalCommitment: Data
  public let privateRecoveryCommitment: Data

  public init(
    version: UInt16 = 1, candidateEnvelopeDigest: Data, lifecycleBindingDigest: Data,
    transitionNullifier: Data, outboxReservationCommitment: Data,
    commitEvidence: KagemushaCommitEvidenceV1, hardwareProfileID: Data,
    policyEpoch: UInt64, privateSuccessorCommitment: Data,
    privateJournalCommitment: Data, privateRecoveryCommitment: Data
  ) throws {
    guard version == 1, policyEpoch > 0 else { throw kagemushaInvalid("hardwareTerminalBody") }
    self.version = version
    self.candidateEnvelopeDigest = try kagemushaDigest(
      candidateEnvelopeDigest, "candidateEnvelopeDigest")
    self.lifecycleBindingDigest = try kagemushaDigest(
      lifecycleBindingDigest, "lifecycleBindingDigest")
    self.transitionNullifier = try kagemushaDigest(
      transitionNullifier, "transitionNullifier")
    self.outboxReservationCommitment = try kagemushaDigest(
      outboxReservationCommitment, "outboxReservationCommitment")
    self.commitEvidence = commitEvidence
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.privateSuccessorCommitment = try kagemushaDigest(
      privateSuccessorCommitment, "privateSuccessorCommitment")
    self.privateJournalCommitment = try kagemushaDigest(
      privateJournalCommitment, "privateJournalCommitment")
    self.privateRecoveryCommitment = try kagemushaDigest(
      privateRecoveryCommitment, "privateRecoveryCommitment")
  }
}

/// Recoverable certificate returned by the atomic hardware commit.
public struct KagemushaCommitCertificateV1: Equatable, Sendable {
  public let version: UInt16
  public let certificateID: Data
  public let candidateEnvelopeDigest: Data
  public let lifecycleBindingDigest: Data
  public let transitionNullifier: Data
  public let outboxReservationCommitment: Data
  public let commitEvidence: KagemushaCommitEvidenceV1
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let hardwareTerminalCommitment: Data

  public init(
    version: UInt16 = 1, certificateID: Data, candidateEnvelopeDigest: Data,
    lifecycleBindingDigest: Data, transitionNullifier: Data,
    outboxReservationCommitment: Data, commitEvidence: KagemushaCommitEvidenceV1,
    hardwareProfileID: Data, policyEpoch: UInt64, hardwareTerminalCommitment: Data
  ) throws {
    guard version == 1, policyEpoch > 0 else { throw kagemushaInvalid("commitCertificate") }
    self.version = version
    self.certificateID = try kagemushaDigest(certificateID, "certificateID")
    self.candidateEnvelopeDigest = try kagemushaDigest(
      candidateEnvelopeDigest, "candidateEnvelopeDigest")
    self.lifecycleBindingDigest = try kagemushaDigest(
      lifecycleBindingDigest, "lifecycleBindingDigest")
    self.transitionNullifier = try kagemushaDigest(
      transitionNullifier, "transitionNullifier")
    self.outboxReservationCommitment = try kagemushaDigest(
      outboxReservationCommitment, "outboxReservationCommitment")
    self.commitEvidence = commitEvidence
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.hardwareTerminalCommitment = try kagemushaDigest(
      hardwareTerminalCommitment, "hardwareTerminalCommitment")
  }
}

/// Final paired proof authorizing one committed offline payment.
public struct KagemushaPaymentProofV1: Equatable, Sendable {
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
    guard version == 1, eqProtocolDigest != epProtocolDigest,
      eqDeferredAudit != epDeferredAudit, !eqProof.isEmpty, !epProof.isEmpty,
      eqProof.count <= KagemushaWireV1.maximumParityProofBytes,
      epProof.count <= KagemushaWireV1.maximumParityProofBytes,
      eqProof.count + epProof.count <= KagemushaWireV1.maximumCurrentProofsBytes,
      eqHistory.count == KagemushaWireV1.historyAccumulatorBytes,
      epHistory.count == KagemushaWireV1.historyAccumulatorBytes,
      eqHistory.contains(where: { $0 != 0 }), epHistory.contains(where: { $0 != 0 }),
      eqHistory != epHistory
    else { throw kagemushaInvalid("paymentProof") }
    self.version = version
    self.eqProtocolDigest = try kagemushaDigest(eqProtocolDigest, "eqProtocolDigest")
    self.epProtocolDigest = try kagemushaDigest(epProtocolDigest, "epProtocolDigest")
    self.semanticDigest = try kagemushaDigest(semanticDigest, "semanticDigest")
    self.candidateEnvelopeDigest = try kagemushaDigest(
      candidateEnvelopeDigest, "candidateEnvelopeDigest")
    self.commitCertificateDigest = try kagemushaDigest(
      commitCertificateDigest, "commitCertificateDigest")
    self.eqDeferredAudit = try kagemushaDigest(eqDeferredAudit, "eqDeferredAudit")
    self.epDeferredAudit = try kagemushaDigest(epDeferredAudit, "epDeferredAudit")
    self.eqProof = Data(eqProof)
    self.epProof = Data(epProof)
    self.eqHistory = Data(eqHistory)
    self.epHistory = Data(epHistory)
  }
}

/// Final paired proof authorizing one online redemption.
public struct KagemushaRedemptionProofV1: Equatable, Sendable {
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
    guard version == 1, eqProtocolDigest != epProtocolDigest,
      eqDeferredAudit != epDeferredAudit, !eqProof.isEmpty, !epProof.isEmpty,
      eqProof.count <= KagemushaWireV1.maximumParityProofBytes,
      epProof.count <= KagemushaWireV1.maximumParityProofBytes,
      eqProof.count + epProof.count <= KagemushaWireV1.maximumCurrentProofsBytes,
      eqHistory.count == KagemushaWireV1.historyAccumulatorBytes,
      epHistory.count == KagemushaWireV1.historyAccumulatorBytes,
      eqHistory.contains(where: { $0 != 0 }), epHistory.contains(where: { $0 != 0 }),
      eqHistory != epHistory
    else { throw kagemushaInvalid("redemptionProof") }
    self.version = version
    self.eqProtocolDigest = try kagemushaDigest(eqProtocolDigest, "eqProtocolDigest")
    self.epProtocolDigest = try kagemushaDigest(epProtocolDigest, "epProtocolDigest")
    self.semanticDigest = try kagemushaDigest(semanticDigest, "semanticDigest")
    self.candidateEnvelopeDigest = try kagemushaDigest(
      candidateEnvelopeDigest, "candidateEnvelopeDigest")
    self.commitCertificateDigest = try kagemushaDigest(
      commitCertificateDigest, "commitCertificateDigest")
    self.eqDeferredAudit = try kagemushaDigest(eqDeferredAudit, "eqDeferredAudit")
    self.epDeferredAudit = try kagemushaDigest(epDeferredAudit, "epDeferredAudit")
    self.eqProof = Data(eqProof)
    self.epProof = Data(epProof)
    self.eqHistory = Data(eqHistory)
    self.epHistory = Data(epHistory)
  }
}

/// Monetary operation bound by a released V1 transition.
public enum KagemushaOperationKindV1: UInt32, CaseIterable, Sendable {
  case bootstrap = 0
  case mintFold = 1
  case sendSplit = 2
  case receiveFold = 3
  case redeemSplit = 4
  case rotate = 5
}

/// Complete public lifecycle context, without history or private state links.
public struct KagemushaLifecycleBindingV1: Equatable, Sendable {
  public let version: UInt16
  public let networkID: Data
  public let protocolVersion: UInt16
  public let suiteID: Data
  public let vkDigest: Data
  public let releaseID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let operationKind: KagemushaOperationKindV1
  public let requestID: Data
  public let receiverLaneCommitment: Data
  public let creditID: Data
  public let ciphertextDigest: Data

  public init(
    version: UInt16 = 1, networkID: Data, protocolVersion: UInt16 = 1,
    suiteID: Data, vkDigest: Data, releaseID: Data, asset: KagemushaAssetDefinitionIDV1,
    assetIncarnation: KagemushaAssetIncarnationV1, scale: UInt32, liabilityPoolID: Data,
    hardwareProfileID: Data, policyEpoch: UInt64, operationKind: KagemushaOperationKindV1,
    requestID: Data, receiverLaneCommitment: Data, creditID: Data, ciphertextDigest: Data
  ) throws {
    try kagemushaHeader(version, networkID, scale)
    guard protocolVersion == 1, policyEpoch > 0 else { throw kagemushaInvalid("lifecycle") }
    let zero = Data(repeating: 0, count: 32)
    let validOperationFields: Bool
    switch operationKind {
    case .sendSplit:
      validOperationFields = [requestID, receiverLaneCommitment, creditID, ciphertextDigest]
        .allSatisfy(kagemushaIsDigest)
    case .mintFold:
      validOperationFields =
        requestID == zero && receiverLaneCommitment == zero
        && kagemushaIsDigest(creditID) && kagemushaIsDigest(ciphertextDigest)
    case .bootstrap, .receiveFold, .redeemSplit, .rotate:
      validOperationFields = [requestID, receiverLaneCommitment, creditID, ciphertextDigest]
        .allSatisfy { $0 == zero }
    }
    guard validOperationFields else { throw kagemushaInvalid("lifecycle.operationFields") }
    self.version = version
    self.networkID = Data(networkID)
    self.protocolVersion = protocolVersion
    self.suiteID = try kagemushaDigest(suiteID, "suiteID")
    self.vkDigest = try kagemushaDigest(vkDigest, "vkDigest")
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try kagemushaDigest(liabilityPoolID, "liabilityPoolID")
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.operationKind = operationKind
    self.requestID = Data(requestID)
    self.receiverLaneCommitment = Data(receiverLaneCommitment)
    self.creditID = Data(creditID)
    self.ciphertextDigest = Data(ciphertextDigest)
  }
}

/// Receiver authorization for any number of distinct exact-amount payments.
public struct KagemushaPaymentRequestV1: Equatable, Sendable {
  public let version: UInt16
  public let releaseID: Data
  public let networkID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let recipient: KagemushaAccountIDV1
  public let amount: KagemushaUInt128V1
  public let recipientEncryptionKey: KagemushaX25519PublicKeyV1
  public let hardwareCredential: KagemushaHardwareCredentialV1
  public let requestID: Data
  public let issuedAtMS: UInt64
  public let expiresAtMS: UInt64
  public let signature: KagemushaDeviceSignatureV1

  public init(
    version: UInt16 = 1, releaseID: Data, networkID: Data,
    asset: KagemushaAssetDefinitionIDV1, assetIncarnation: KagemushaAssetIncarnationV1,
    scale: UInt32, liabilityPoolID: Data, recipient: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    recipientEncryptionKey: KagemushaX25519PublicKeyV1,
    hardwareCredential: KagemushaHardwareCredentialV1,
    requestID: Data,
    issuedAtMS: UInt64, expiresAtMS: UInt64, signature: KagemushaDeviceSignatureV1
  ) throws {
    try kagemushaHeader(version, networkID, scale)
    guard !amount.isZero, hardwareCredential.networkID == networkID, expiresAtMS > issuedAtMS,
      expiresAtMS - issuedAtMS <= KagemushaWireV1.requestMaximumTTLMS,
      expiresAtMS <= hardwareCredential.expiresAtMS
    else { throw kagemushaInvalid("paymentRequest") }
    self.version = version
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try kagemushaDigest(liabilityPoolID, "liabilityPoolID")
    self.recipient = recipient
    self.amount = amount
    self.recipientEncryptionKey = recipientEncryptionKey
    self.hardwareCredential = hardwareCredential
    self.requestID = try kagemushaDigest(requestID, "requestID")
    self.issuedAtMS = issuedAtMS
    self.expiresAtMS = expiresAtMS
    self.signature = signature
  }
}

/// Public output bound by the final payment proof.
public struct KagemushaPaymentOutputV1: Equatable, Sendable {
  public let version: UInt16
  public let requestDigest: Data
  public let amount: KagemushaUInt128V1
  public let senderBeforeCommitment: Data
  public let senderAfterCommitment: Data
  public let transitionNullifier: Data
  public let creditID: Data
  public let ciphertextCommitment: Data
  public let commitEvidence: KagemushaCommitEvidenceV1
  public let committedAtMS: UInt64

  public init(
    version: UInt16 = 1, requestDigest: Data, amount: KagemushaUInt128V1,
    senderBeforeCommitment: Data, senderAfterCommitment: Data,
    transitionNullifier: Data, creditID: Data, ciphertextCommitment: Data,
    commitEvidence: KagemushaCommitEvidenceV1, committedAtMS: UInt64
  ) throws {
    guard version == 1, !amount.isZero, senderBeforeCommitment != senderAfterCommitment,
      committedAtMS > 0
    else { throw kagemushaInvalid("paymentOutput") }
    self.version = version
    self.requestDigest = try kagemushaDigest(requestDigest, "requestDigest")
    self.amount = amount
    self.senderBeforeCommitment = try kagemushaDigest(
      senderBeforeCommitment, "senderBeforeCommitment")
    self.senderAfterCommitment = try kagemushaDigest(
      senderAfterCommitment, "senderAfterCommitment")
    self.transitionNullifier = try kagemushaDigest(transitionNullifier, "transitionNullifier")
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.ciphertextCommitment = try kagemushaDigest(
      ciphertextCommitment, "ciphertextCommitment")
    self.commitEvidence = commitEvidence
    self.committedAtMS = committedAtMS
  }
}

/// Sender terminal response carrying the post-commit certificate and paired payment proof.
public struct KagemushaPaymentV1: Equatable, Sendable {
  public let version: UInt16
  public let output: KagemushaPaymentOutputV1
  public let encryptedCredit: Data
  public let commitCertificate: KagemushaCommitCertificateV1
  public let proof: KagemushaPaymentProofV1

  public init(
    version: UInt16 = 1, output: KagemushaPaymentOutputV1,
    encryptedCredit: Data, commitCertificate: KagemushaCommitCertificateV1,
    proof: KagemushaPaymentProofV1
  ) throws {
    guard version == 1, output.version == version, commitCertificate.version == version,
      proof.version == version, !encryptedCredit.isEmpty,
      encryptedCredit.count <= KagemushaWireV1.maximumEncryptedCreditBytes
    else { throw kagemushaInvalid("payment") }
    self.version = version
    self.output = output
    self.encryptedCredit = Data(encryptedCredit)
    self.commitCertificate = commitCertificate
    self.proof = proof
  }
}

/// Durable secure-inbox record named by a receiver acknowledgement.
public struct KagemushaInboxReceiptV1: Equatable, Sendable {
  public let version: UInt16
  public let creditID: Data
  public let receiptCommitment: Data

  public init(version: UInt16 = 1, creditID: Data, receiptCommitment: Data) throws {
    guard version == 1 else { throw kagemushaInvalid("inboxReceipt") }
    self.version = version
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.receiptCommitment = try kagemushaDigest(receiptCommitment, "receiptCommitment")
  }
}

/// Receiver acknowledgement emitted only after durable inbox persistence.
public struct KagemushaAcknowledgementV1: Equatable, Sendable {
  public let version: UInt16
  public let requestDigest: Data
  public let paymentDigest: Data
  public let inboxReceipt: KagemushaInboxReceiptV1
  public let signature: KagemushaDeviceSignatureV1

  public init(
    version: UInt16 = 1, requestDigest: Data, paymentDigest: Data,
    inboxReceipt: KagemushaInboxReceiptV1, signature: KagemushaDeviceSignatureV1
  ) throws {
    guard version == 1, inboxReceipt.version == version else {
      throw kagemushaInvalid("acknowledgement")
    }
    self.version = version
    self.requestDigest = try kagemushaDigest(requestDigest, "requestDigest")
    self.paymentDigest = try kagemushaDigest(paymentDigest, "paymentDigest")
    self.inboxReceipt = inboxReceipt
    self.signature = signature
  }
}

/// Pre-ID recipient context authorized before reserve debit.
public struct KagemushaMintAuthorizationContextV1: Equatable, Sendable {
  public let version: UInt16
  public let operationID: Data
  public let releaseID: Data
  public let suiteID: Data
  public let vkDigest: Data
  public let artifactManifestDigest: Data
  public let networkID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let scale: UInt32
  public let liabilityPoolID: Data
  public let amount: KagemushaUInt128V1
  public let payer: KagemushaAccountIDV1
  public let recipient: KagemushaAccountIDV1
  public let hardwareCredentialID: Data
  public let hardwareProfileID: Data
  public let policyEpoch: UInt64
  public let recipientCredentialCommitment: Data
  public let creditCommitment: Data
  public let recipientOneTimeKey: KagemushaX25519PublicKeyV1

  public init(
    version: UInt16 = 1, operationID: Data, releaseID: Data, suiteID: Data,
    vkDigest: Data, artifactManifestDigest: Data, networkID: Data,
    asset: KagemushaAssetDefinitionIDV1, assetIncarnation: KagemushaAssetIncarnationV1,
    scale: UInt32, liabilityPoolID: Data, amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1, recipient: KagemushaAccountIDV1,
    hardwareCredentialID: Data, hardwareProfileID: Data, policyEpoch: UInt64,
    recipientCredentialCommitment: Data, creditCommitment: Data,
    recipientOneTimeKey: KagemushaX25519PublicKeyV1
  ) throws {
    try kagemushaHeader(version, networkID, scale)
    guard !amount.isZero, policyEpoch > 0 else { throw kagemushaInvalid("mintContext") }
    self.version = version
    self.operationID = try kagemushaDigest(operationID, "operationID")
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.suiteID = try kagemushaDigest(suiteID, "suiteID")
    self.vkDigest = try kagemushaDigest(vkDigest, "vkDigest")
    self.artifactManifestDigest = try kagemushaDigest(
      artifactManifestDigest, "artifactManifestDigest")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.liabilityPoolID = try kagemushaDigest(liabilityPoolID, "liabilityPoolID")
    self.amount = amount
    self.payer = payer
    self.recipient = recipient
    self.hardwareCredentialID = try kagemushaDigest(
      hardwareCredentialID, "hardwareCredentialID")
    self.hardwareProfileID = try kagemushaDigest(hardwareProfileID, "hardwareProfileID")
    self.policyEpoch = policyEpoch
    self.recipientCredentialCommitment = try kagemushaDigest(
      recipientCredentialCommitment, "recipientCredentialCommitment")
    self.creditCommitment = try kagemushaDigest(creditCommitment, "creditCommitment")
    self.recipientOneTimeKey = recipientOneTimeKey
  }
}

/// Exact pre-debit mint authorization statement.
public struct KagemushaMintAuthorizationStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let context: KagemushaMintAuthorizationContextV1
  public let issuanceCommitment: Data
  public let creditID: Data
  public let ciphertextDigest: Data

  public init(
    version: UInt16 = 1, context: KagemushaMintAuthorizationContextV1,
    issuanceCommitment: Data, creditID: Data, ciphertextDigest: Data
  ) throws {
    guard version == 1, context.version == version else {
      throw kagemushaInvalid("mintAuthorizationStatement")
    }
    self.version = version
    self.context = context
    self.issuanceCommitment = try kagemushaDigest(issuanceCommitment, "issuanceCommitment")
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.ciphertextDigest = try kagemushaDigest(ciphertextDigest, "ciphertextDigest")
  }
}

/// Release-pinned recipient authorization verified before reserve mutation.
public struct KagemushaMintAuthorizationV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: KagemushaMintAuthorizationStatementV1
  public let proof: KagemushaPairedProofV1

  public init(
    version: UInt16 = 1, statement: KagemushaMintAuthorizationStatementV1,
    proof: KagemushaPairedProofV1
  ) throws {
    guard version == 1, statement.version == version, proof.version == version else {
      throw kagemushaInvalid("mintAuthorization")
    }
    self.version = version
    self.statement = statement
    self.proof = proof
  }
}

/// Canonical chain-facing request for one reserve-backed KAGEMUSHA mint.
public struct KagemushaTopUpRequestV1: Equatable, Sendable {
  public let version: UInt16
  public let operationID: Data
  public let issuanceCommitment: Data
  public let creditID: Data
  public let releaseID: Data
  public let suiteID: Data
  public let vkDigest: Data
  public let networkID: Data
  public let asset: KagemushaAssetDefinitionIDV1
  public let assetIncarnation: KagemushaAssetIncarnationV1
  public let scale: UInt32
  public let amount: KagemushaUInt128V1
  public let liabilityPoolID: Data
  public let payer: KagemushaAccountIDV1
  public let recipient: KagemushaAccountIDV1
  public let hardwareCredential: KagemushaHardwareCredentialV1
  public let recipientCredentialCommitment: Data
  public let creditCommitment: Data
  public let recipientOneTimeKey: KagemushaX25519PublicKeyV1
  public let encryptedCredit: Data
  public let artifactManifestDigest: Data
  public let mintAuthorization: KagemushaMintAuthorizationV1

  public init(
    version: UInt16 = 1, operationID: Data, issuanceCommitment: Data,
    creditID: Data, releaseID: Data, suiteID: Data, vkDigest: Data,
    networkID: Data, asset: KagemushaAssetDefinitionIDV1,
    assetIncarnation: KagemushaAssetIncarnationV1, scale: UInt32,
    amount: KagemushaUInt128V1, liabilityPoolID: Data,
    payer: KagemushaAccountIDV1, recipient: KagemushaAccountIDV1,
    hardwareCredential: KagemushaHardwareCredentialV1,
    recipientCredentialCommitment: Data, creditCommitment: Data,
    recipientOneTimeKey: KagemushaX25519PublicKeyV1, encryptedCredit: Data,
    artifactManifestDigest: Data, mintAuthorization: KagemushaMintAuthorizationV1
  ) throws {
    try kagemushaHeader(version, networkID, scale)
    guard !amount.isZero, hardwareCredential.version == version,
      mintAuthorization.version == version, !encryptedCredit.isEmpty,
      encryptedCredit.count <= KagemushaWireV1.maximumEncryptedCreditBytes
    else { throw kagemushaInvalid("topUpRequest") }
    self.version = version
    self.operationID = try kagemushaDigest(operationID, "operationID")
    self.issuanceCommitment = try kagemushaDigest(
      issuanceCommitment, "issuanceCommitment")
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.suiteID = try kagemushaDigest(suiteID, "suiteID")
    self.vkDigest = try kagemushaDigest(vkDigest, "vkDigest")
    self.networkID = Data(networkID)
    self.asset = asset
    self.assetIncarnation = assetIncarnation
    self.scale = scale
    self.amount = amount
    self.liabilityPoolID = try kagemushaDigest(liabilityPoolID, "liabilityPoolID")
    self.payer = payer
    self.recipient = recipient
    self.hardwareCredential = hardwareCredential
    self.recipientCredentialCommitment = try kagemushaDigest(
      recipientCredentialCommitment, "recipientCredentialCommitment")
    self.creditCommitment = try kagemushaDigest(creditCommitment, "creditCommitment")
    self.recipientOneTimeKey = recipientOneTimeKey
    self.encryptedCredit = Data(encryptedCredit)
    self.artifactManifestDigest = try kagemushaDigest(
      artifactManifestDigest, "artifactManifestDigest")
    self.mintAuthorization = mintAuthorization
  }
}

/// Public top-up statement creating one foldable aggregate credit.
public struct KagemushaMintCreditStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let lifecycle: KagemushaLifecycleBindingV1
  public let recipientCredentialCommitment: Data
  public let authorizationContextDigest: Data
  public let mintAuthorizationDigest: Data
  public let amount: KagemushaUInt128V1
  public let issuanceCommitment: Data
  public let recipient: KagemushaAccountIDV1
  public let creditCommitment: Data
  public let mintedAtMS: UInt64

  public init(
    version: UInt16 = 1, lifecycle: KagemushaLifecycleBindingV1,
    recipientCredentialCommitment: Data, authorizationContextDigest: Data,
    mintAuthorizationDigest: Data, amount: KagemushaUInt128V1,
    issuanceCommitment: Data, recipient: KagemushaAccountIDV1,
    creditCommitment: Data, mintedAtMS: UInt64
  ) throws {
    guard version == 1, lifecycle.version == version, lifecycle.operationKind == .mintFold,
      !amount.isZero, mintedAtMS > 0
    else { throw kagemushaInvalid("mintCreditStatement") }
    self.version = version
    self.lifecycle = lifecycle
    self.recipientCredentialCommitment = try kagemushaDigest(
      recipientCredentialCommitment, "recipientCredentialCommitment")
    self.authorizationContextDigest = try kagemushaDigest(
      authorizationContextDigest, "authorizationContextDigest")
    self.mintAuthorizationDigest = try kagemushaDigest(
      mintAuthorizationDigest, "mintAuthorizationDigest")
    self.amount = amount
    self.issuanceCommitment = try kagemushaDigest(issuanceCommitment, "issuanceCommitment")
    self.recipient = recipient
    self.creditCommitment = try kagemushaDigest(creditCommitment, "creditCommitment")
    self.mintedAtMS = mintedAtMS
  }
}

/// Constant-size authenticated top-up credit folded into aggregate state.
public struct KagemushaMintCreditV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: KagemushaMintCreditStatementV1
  public let proof: KagemushaPairedProofV1
  public let finalityCertificateBinding: Data
  public let finalityAuthorityHead: Data
  public let finalityGenesisRosterID: Data
  public let finalityProofBindingDigest: Data
  public let encryptedCredit: KagemushaEncryptedCreditEnvelopeV1
  public let artifactManifestDigest: Data

  public init(
    version: UInt16 = 1, statement: KagemushaMintCreditStatementV1,
    proof: KagemushaPairedProofV1, finalityCertificateBinding: Data,
    finalityAuthorityHead: Data, finalityGenesisRosterID: Data,
    finalityProofBindingDigest: Data,
    encryptedCredit: KagemushaEncryptedCreditEnvelopeV1,
    artifactManifestDigest: Data
  ) throws {
    guard version == 1, statement.version == version, proof.version == version,
      encryptedCredit.version == version
    else { throw kagemushaInvalid("mintCredit") }
    self.version = version
    self.statement = statement
    self.proof = proof
    self.finalityCertificateBinding = try kagemushaDigest(
      finalityCertificateBinding, "finalityCertificateBinding")
    self.finalityAuthorityHead = try kagemushaDigest(
      finalityAuthorityHead, "finalityAuthorityHead")
    self.finalityGenesisRosterID = try kagemushaDigest(
      finalityGenesisRosterID, "finalityGenesisRosterID")
    self.finalityProofBindingDigest = try kagemushaDigest(
      finalityProofBindingDigest, "finalityProofBindingDigest")
    self.encryptedCredit = encryptedCredit
    self.artifactManifestDigest = try kagemushaDigest(
      artifactManifestDigest, "artifactManifestDigest")
  }
}

/// Unlinkable terminal transition that converts an aggregate KAGEMUSHA balance to an online claim.
public struct KagemushaRedemptionStatementV1: Equatable, Sendable {
  public let version: UInt16
  public let lifecycle: KagemushaLifecycleBindingV1
  public let amount: KagemushaUInt128V1
  public let beneficiary: KagemushaAccountIDV1
  public let terminalNullifier: Data
  public let redemptionCommitment: Data
  public let redemptionID: Data
  public let commitEvidence: KagemushaCommitEvidenceV1

  public init(
    version: UInt16 = 1, lifecycle: KagemushaLifecycleBindingV1,
    amount: KagemushaUInt128V1, beneficiary: KagemushaAccountIDV1,
    terminalNullifier: Data, redemptionCommitment: Data, redemptionID: Data,
    commitEvidence: KagemushaCommitEvidenceV1
  ) throws {
    guard version == 1, lifecycle.version == version, lifecycle.operationKind == .redeemSplit,
      !amount.isZero
    else { throw kagemushaInvalid("redemptionStatement") }
    self.version = version
    self.lifecycle = lifecycle
    self.amount = amount
    self.beneficiary = beneficiary
    self.terminalNullifier = try kagemushaDigest(terminalNullifier, "terminalNullifier")
    self.redemptionCommitment = try kagemushaDigest(
      redemptionCommitment, "redemptionCommitment")
    self.redemptionID = try kagemushaDigest(redemptionID, "redemptionID")
    self.commitEvidence = commitEvidence
  }
}

/// Constant-size terminal voucher submitted for online redemption.
public struct KagemushaRedemptionVoucherV1: Equatable, Sendable {
  public let version: UInt16
  public let statement: KagemushaRedemptionStatementV1
  public let commitCertificate: KagemushaCommitCertificateV1
  public let proof: KagemushaRedemptionProofV1
  public let artifactManifestDigest: Data

  public init(
    version: UInt16 = 1, statement: KagemushaRedemptionStatementV1,
    commitCertificate: KagemushaCommitCertificateV1,
    proof: KagemushaRedemptionProofV1, artifactManifestDigest: Data
  ) throws {
    guard version == 1, statement.version == version,
      commitCertificate.version == version, proof.version == version
    else { throw kagemushaInvalid("redemptionVoucher") }
    self.version = version
    self.statement = statement
    self.commitCertificate = commitCertificate
    self.proof = proof
    self.artifactManifestDigest = try kagemushaDigest(
      artifactManifestDigest, "artifactManifestDigest")
  }
}

/// Canonical chain-facing request for one full or partial reserve redemption.
public struct KagemushaRedemptionRequestV1: Equatable, Sendable {
  public let version: UInt16
  public let operationID: Data
  public let voucher: KagemushaRedemptionVoucherV1

  public init(
    version: UInt16 = 1, operationID: Data, voucher: KagemushaRedemptionVoucherV1
  ) throws {
    guard version == 1, voucher.version == version else {
      throw kagemushaInvalid("redemptionRequest")
    }
    self.version = version
    self.operationID = try kagemushaDigest(operationID, "operationID")
    self.voucher = voucher
  }
}

func kagemushaInvalid(_ field: String) -> KagemushaWireEnvelopeErrorV1 {
  .invalidField(field)
}

func kagemushaIsDigest(_ value: Data) -> Bool {
  value.count == 32 && value.contains(where: { $0 != 0 })
}

func kagemushaDigest(_ value: Data, _ field: String) throws -> Data {
  guard kagemushaIsDigest(value) else { throw kagemushaInvalid(field) }
  return Data(value)
}

func kagemushaHeader(_ version: UInt16, _ networkID: Data, _ scale: UInt32) throws {
  guard version == 1, networkID.count == 32, networkID.contains(where: { $0 != 0 }),
    scale <= KagemushaWireV1.maximumAssetScale
  else { throw kagemushaInvalid("header") }
}
