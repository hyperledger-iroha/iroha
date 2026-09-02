import Foundation

/// Failures from the hardware-authoritative Kagemusha V1 orchestration layer.
public enum KagemushaWalletErrorV1: Error, Equatable, Sendable {
  case onlineOnly
  case invalidHardwareContract(String)
  case invalidHardwareResult(String)
  case nativeVerificationRequired
  case conflictingRecoveredEnvelope
}

/// The governed profile and compact credential used by one qualified device service.
public struct KagemushaHardwareQualificationV1: Equatable, Sendable {
  public let releaseID: Data
  public let hardwarePolicyDigest: Data
  public let profile: KagemushaHardwareProfileV1
  public let credential: KagemushaHardwareCredentialV1

  public init(
    releaseID: Data,
    hardwarePolicyDigest: Data,
    profile: KagemushaHardwareProfileV1,
    credential: KagemushaHardwareCredentialV1
  ) throws {
    guard profile.hardwareProfileID == credential.hardwareProfileID,
      profile.policyEpoch == credential.policyEpoch,
      profile.firmwarePolicyDigest == credential.firmwarePolicyDigest,
      profile.validFromMS <= credential.issuedAtMS,
      credential.issuedAtMS < profile.expiresAtMS
    else { throw KagemushaWalletErrorV1.invalidHardwareContract("profile/credential mismatch") }
    self.releaseID = try kagemushaDigest(releaseID, "releaseID")
    self.hardwarePolicyDigest = try kagemushaDigest(
      hardwarePolicyDigest, "hardwarePolicyDigest")
    self.profile = profile
    self.credential = credential
  }
}

/// Result of atomically staging a payment in the authenticated hardware inbox.
public enum KagemushaHardwareStageDispositionV1: Equatable, Sendable {
  case staged
  case exactDuplicate
}

/// Staged payment result. The acknowledgement is emitted only after durable persistence.
public struct KagemushaStagedPaymentV1: Equatable, Sendable {
  public let disposition: KagemushaHardwareStageDispositionV1
  public let acknowledgement: KagemushaAcknowledgementV1

  public init(
    disposition: KagemushaHardwareStageDispositionV1,
    acknowledgement: KagemushaAcknowledgementV1
  ) {
    self.disposition = disposition
    self.acknowledgement = acknowledgement
  }
}

/// Result of folding one staged credit into the aggregate balance.
public struct KagemushaReceiveFoldResultV1: Equatable, Sendable {
  public let aggregateState: KagemushaAggregateStateCommitmentV1

  public init(aggregateState: KagemushaAggregateStateCommitmentV1) {
    self.aggregateState = aggregateState
  }
}

/// Mandatory qualified-device boundary used by `KagemushaWalletV1`.
///
/// Implementations must delegate proof generation/verification, AEAD, signature operations,
/// capacity reservations, replay state, prepare/prove/commit, and recovery to the single audited
/// native core plus the governed non-forking hardware service. A software implementation of this
/// protocol is not an Kagemusha V1 provider.
public protocol KagemushaHardwareProviderV1: AnyObject {
  func qualification() throws -> KagemushaHardwareQualificationV1
  func recoverAggregateState() throws -> Data

  func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    validityWindowMS: UInt64
  ) throws -> Data

  func prepareProveCommitPayment(
    canonicalRequest: Data
  ) throws -> Data

  func recoverPayment(creditID: Data) throws -> Data?

  func verifyAndStageInboundPayment(
    canonicalRequest: Data,
    canonicalPayment: Data
  ) throws -> (KagemushaHardwareStageDispositionV1, Data)

  func releasePaymentOutbox(
    canonicalRequest: Data,
    canonicalPayment: Data,
    canonicalAcknowledgement: Data
  ) throws

  func prepareMintAuthorization(
    operationID: Data,
    amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  ) throws -> Data

  func verifyAuthorizationAndStageMintCredit(
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws -> KagemushaHardwareStageDispositionV1

  func pendingCreditWatermark() throws -> KagemushaUInt128V1

  func foldNextReceive(
    upToInclusive watermark: KagemushaUInt128V1
  ) throws -> Data?

  func prepareProveCommitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> Data

  func recoverRedemption(redemptionID: Data) throws -> Data?

  func rotateHardwareEpoch() throws -> Data
}

/// Hardware-authoritative orchestration around canonical V1 bytes.
///
/// This class never constructs a proof, signs a message, encrypts/decrypts a credit, derives an
/// identifier, or advances monetary state in Swift. It checks canonical public shape and ordering
/// around provider calls, while native release authentication remains mandatory.
public final class KagemushaWalletV1: @unchecked Sendable {
  private let provider: KagemushaHardwareProviderV1
  private let lock = NSLock()
  private var qualificationValue: KagemushaHardwareQualificationV1
  private var aggregateStateValue: KagemushaAggregateStateCommitmentV1

  private init(
    provider: KagemushaHardwareProviderV1,
    qualification: KagemushaHardwareQualificationV1,
    aggregateState: KagemushaAggregateStateCommitmentV1
  ) {
    self.provider = provider
    qualificationValue = qualification
    aggregateStateValue = aggregateState
  }

  /// Open only after the provider returns a coherent governed profile, credential, and state.
  public static func open(provider: KagemushaHardwareProviderV1) throws -> KagemushaWalletV1 {
    let qualification = try provider.qualification()
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(
      provider.recoverAggregateState())
    guard state.networkID == qualification.credential.networkID,
      state.releaseID == qualification.releaseID,
      state.hardwarePolicyID == qualification.hardwarePolicyDigest,
      state.hardwareEpochID == qualification.credential.hardwareEpochID,
      state.keyReference == qualification.credential.deviceKeyReference
    else { throw KagemushaWalletErrorV1.invalidHardwareResult("recovered state binding") }
    return KagemushaWalletV1(
      provider: provider, qualification: qualification, aggregateState: state)
  }

  public func qualification() -> KagemushaHardwareQualificationV1 {
    lock.withLock { qualificationValue }
  }

  public func aggregateState() -> KagemushaAggregateStateCommitmentV1 {
    lock.withLock { aggregateStateValue }
  }

  /// Ask qualified receiver hardware to create and sign an exact-amount request.
  public func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    validityWindowMS: UInt64
  ) throws -> KagemushaPaymentRequestV1 {
    guard !amount.isZero,
      validityWindowMS > 0, validityWindowMS <= KagemushaWireV1.requestMaximumTTLMS
    else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("invalid request amount or validity window")
    }
    return try lock.withLock {
      let value = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
        provider.createPaymentRequest(
          recipient: recipient, amount: amount, validityWindowMS: validityWindowMS))
      guard value.recipient == recipient, value.amount == amount,
        value.networkID == aggregateStateValue.networkID,
        value.asset == aggregateStateValue.asset,
        value.assetIncarnation == aggregateStateValue.assetIncarnation,
        value.releaseID == aggregateStateValue.releaseID,
        value.hardwareCredential == qualificationValue.credential
      else { throw KagemushaWalletErrorV1.invalidHardwareResult("request binding") }
      return value
    }
  }

  /// Execute recoverable prepare/prove/commit and expose only the final canonical payment.
  public func commitPayment(request: KagemushaPaymentRequestV1) throws -> KagemushaPaymentV1 {
    return try lock.withLock {
      _ = try drainStagedCreditsLocked()
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
        provider.prepareProveCommitPayment(
          canonicalRequest: KagemushaNoritoV1.encodePaymentRequestShape(request)),
        against: request)
      return payment
    }
  }

  /// Recover the byte-identical final payment after a crash at any post-commit boundary.
  public func recoverPayment(
    request: KagemushaPaymentRequestV1,
    creditID: Data
  ) throws -> KagemushaPaymentV1? {
    guard kagemushaIsDigest(creditID) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("credit id")
    }
    guard let bytes = try provider.recoverPayment(creditID: creditID) else {
      return nil
    }
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(bytes, against: request)
    guard payment.statement.lifecycle.creditID == creditID,
      try KagemushaNoritoV1.encodePaymentShape(payment, against: request) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    return payment
  }

  /// Stage a valid receiver-bound payment despite later traffic and return the durable ACK.
  public func stageInboundPayment(
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1
  ) throws -> KagemushaStagedPaymentV1 {
    let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
    let canonicalPayment = try KagemushaNoritoV1.encodePaymentShape(payment, against: request)
    let (disposition, acknowledgementBytes) = try provider.verifyAndStageInboundPayment(
      canonicalRequest: canonicalRequest, canonicalPayment: canonicalPayment)
    let acknowledgement = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgementBytes, against: request, payment: payment)
    _ = try KagemushaNoritoV1.validateTerminalDeliveryShape(
      request: request, payment: payment, acknowledgement: acknowledgement)
    return KagemushaStagedPaymentV1(
      disposition: disposition, acknowledgement: acknowledgement)
  }

  /// Release the sender retry-outbox entry only after the exact ACK is validated in native code.
  public func releasePaymentOutbox(
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1
  ) throws {
    try provider.releasePaymentOutbox(
      canonicalRequest: KagemushaNoritoV1.encodePaymentRequestShape(request),
      canonicalPayment: KagemushaNoritoV1.encodePaymentShape(payment, against: request),
      canonicalAcknowledgement: KagemushaNoritoV1.encodeAcknowledgementShape(
        acknowledgement, against: request, payment: payment))
  }

  /// Ask recipient hardware for the exact proof-bearing authorization used before reserve debit.
  public func prepareMintAuthorization(
    operationID: Data,
    amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  ) throws -> KagemushaMintAuthorizationV1 {
    guard kagemushaIsDigest(operationID), !amount.isZero else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("mint authorization input")
    }
    return try KagemushaNoritoV1.decodeMintAuthorizationShapeExact(
      provider.prepareMintAuthorization(
        operationID: operationID, amount: amount, payer: payer, recipient: recipient))
  }

  /// Stage a mint credit only after native code verifies its exact pre-debit authorization.
  public func stageMintCredit(
    authorization: KagemushaMintAuthorizationV1,
    credit: KagemushaMintCreditV1
  ) throws -> KagemushaHardwareStageDispositionV1 {
    let context = authorization.statement.context
    let lifecycle = credit.statement.lifecycle
    guard credit.statement.amount == context.amount,
      credit.statement.recipient == context.recipient,
      credit.statement.recipientCredentialCommitment == context.recipientCredentialCommitment,
      credit.statement.issuanceCommitment == authorization.statement.issuanceCommitment,
      credit.statement.creditCommitment == context.creditCommitment,
      lifecycle.creditID == authorization.statement.creditID,
      lifecycle.ciphertextDigest == authorization.statement.ciphertextDigest,
      lifecycle.releaseID == context.releaseID,
      lifecycle.suiteID == context.suiteID,
      lifecycle.vkDigest == context.vkDigest,
      lifecycle.networkID == context.networkID,
      lifecycle.asset == context.asset,
      lifecycle.assetIncarnation == context.assetIncarnation,
      lifecycle.scale == context.scale,
      lifecycle.liabilityPoolID == context.liabilityPoolID,
      lifecycle.hardwareProfileID == context.hardwareProfileID,
      lifecycle.policyEpoch == context.policyEpoch,
      credit.artifactManifestDigest == context.artifactManifestDigest
    else { throw KagemushaWalletErrorV1.invalidHardwareResult("mint public binding") }
    return try provider.verifyAuthorizationAndStageMintCredit(
      canonicalAuthorization: KagemushaNoritoV1.encodeMintAuthorizationShape(authorization),
      canonicalMintCredit: KagemushaNoritoV1.encodeMintCreditShape(credit))
  }

  /// Fold the next staged credit into the aggregate balance.
  public func foldNextReceive() throws -> KagemushaReceiveFoldResultV1?
  {
    return try lock.withLock {
      let watermark = try provider.pendingCreditWatermark()
      return try foldNextReceiveLocked(upToInclusive: watermark)
    }
  }

  /// Drain all staged credits one fixed-shape transition at a time before a send or redemption.
  public func drainStagedCredits() throws -> KagemushaUInt128V1 {
    try lock.withLock { try drainStagedCreditsLocked() }
  }

  private func foldNextReceiveLocked(
    upToInclusive watermark: KagemushaUInt128V1
  ) throws -> KagemushaReceiveFoldResultV1?
  {
    guard let bytes = try provider.foldNextReceive(upToInclusive: watermark) else { return nil }
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
    guard sameBalanceIdentity(aggregateStateValue, state),
      state.hardwareEpochID == aggregateStateValue.hardwareEpochID,
      state.keyReference == aggregateStateValue.keyReference,
      state.hardwarePolicyID == aggregateStateValue.hardwarePolicyID,
      state.sequence == (try aggregateStateValue.sequence.adding(1)),
      state.stateCommitment != aggregateStateValue.stateCommitment
    else {
      throw KagemushaWalletErrorV1.invalidHardwareResult(
        "fold did not install the exact next aggregate state")
    }
    let result = KagemushaReceiveFoldResultV1(aggregateState: state)
    aggregateStateValue = state
    return result
  }

  private func drainStagedCreditsLocked() throws -> KagemushaUInt128V1 {
    let watermark = try provider.pendingCreditWatermark()
    var count = KagemushaUInt128V1.zero
    while try foldNextReceiveLocked(upToInclusive: watermark) != nil {
      count = try count.adding(1)
    }
    return count
  }

  /// Execute recoverable prepare/prove/commit for an unlinkable terminal redemption.
  public func commitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> KagemushaRedemptionVoucherV1 {
    guard !amount.isZero else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("redemption amount")
    }
    return try lock.withLock {
      _ = try drainStagedCreditsLocked()
      return try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(
        provider.prepareProveCommitRedemption(amount: amount, beneficiary: beneficiary))
    }
  }

  /// Recover a byte-identical redemption envelope after hardware commit.
  public func recoverRedemption(redemptionID: Data) throws -> KagemushaRedemptionVoucherV1? {
    guard kagemushaIsDigest(redemptionID) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("redemption id")
    }
    guard let bytes = try provider.recoverRedemption(redemptionID: redemptionID) else { return nil }
    let voucher = try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(bytes)
    guard voucher.statement.redemptionID == redemptionID,
      try KagemushaNoritoV1.encodeRedemptionVoucherShape(voucher) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    return voucher
  }

  /// Drain the stable inbox snapshot, then rotate the complete private balance in hardware.
  public func rotateHardwareEpoch() throws -> KagemushaAggregateStateCommitmentV1 {
    try lock.withLock {
      _ = try drainStagedCreditsLocked()
      let previousState = aggregateStateValue
      let previousQualification = qualificationValue
      let rotatedState = try KagemushaNoritoV1.decodeAggregateStateShapeExact(
        provider.rotateHardwareEpoch())
      let rotatedQualification = try provider.qualification()
      guard previousQualification.credential.hardwareEpochGeneration < UInt64.max,
        rotatedQualification.releaseID == previousQualification.releaseID,
        rotatedQualification.hardwarePolicyDigest == previousQualification.hardwarePolicyDigest,
        rotatedQualification.credential.networkID
          == previousQualification.credential.networkID,
        rotatedQualification.credential.laneCommitment
          == previousQualification.credential.laneCommitment,
        rotatedQualification.credential.hardwareEpochGeneration
          == previousQualification.credential.hardwareEpochGeneration + 1,
        rotatedQualification.credential.hardwareEpochID
          != previousQualification.credential.hardwareEpochID,
        sameBalanceIdentity(previousState, rotatedState),
        rotatedState.hardwareEpochID == rotatedQualification.credential.hardwareEpochID,
        rotatedState.keyReference == rotatedQualification.credential.deviceKeyReference,
        rotatedState.hardwarePolicyID == rotatedQualification.hardwarePolicyDigest,
        rotatedState.sequence.isZero
      else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "rotation did not install the exact next hardware epoch")
      }
      qualificationValue = rotatedQualification
      aggregateStateValue = rotatedState
      return rotatedState
    }
  }

  private func sameBalanceIdentity(
    _ lhs: KagemushaAggregateStateCommitmentV1,
    _ rhs: KagemushaAggregateStateCommitmentV1
  ) -> Bool {
    lhs.releaseID == rhs.releaseID
      && lhs.networkID == rhs.networkID
      && lhs.asset == rhs.asset
      && lhs.assetIncarnation == rhs.assetIncarnation
      && lhs.scale == rhs.scale
      && lhs.liabilityPoolID == rhs.liabilityPoolID
      && lhs.laneID == rhs.laneID
  }
}

extension NSLock {
  fileprivate func withLock<T>(_ body: () throws -> T) rethrows -> T {
    lock()
    defer { unlock() }
    return try body()
  }
}
