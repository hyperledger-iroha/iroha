import Foundation

/// Failures from the hardware-authoritative Offline Cash V1 orchestration layer.
public enum OfflineCashWalletErrorV1: Error, Equatable, Sendable {
  case onlineOnly
  case invalidHardwareContract(String)
  case invalidHardwareResult(String)
  case nativeVerificationRequired
  case conflictingRecoveredEnvelope
}

/// The governed profile and compact credential used by one qualified device service.
public struct OfflineCashHardwareQualificationV1: Equatable, Sendable {
  public let releaseID: Data
  public let hardwarePolicyDigest: Data
  public let profile: OfflineCashHardwareProfileV1
  public let credential: OfflineCashHardwareCredentialV1

  public init(
    releaseID: Data,
    hardwarePolicyDigest: Data,
    profile: OfflineCashHardwareProfileV1,
    credential: OfflineCashHardwareCredentialV1
  ) throws {
    guard profile.hardwareProfileID == credential.hardwareProfileID,
      profile.policyEpoch == credential.policyEpoch,
      profile.firmwarePolicyDigest == credential.firmwarePolicyDigest,
      profile.validFromMS <= credential.issuedAtMS,
      credential.issuedAtMS < profile.expiresAtMS
    else { throw OfflineCashWalletErrorV1.invalidHardwareContract("profile/credential mismatch") }
    self.releaseID = try offlineCashDigest(releaseID, "releaseID")
    self.hardwarePolicyDigest = try offlineCashDigest(
      hardwarePolicyDigest, "hardwarePolicyDigest")
    self.profile = profile
    self.credential = credential
  }
}

/// Result of atomically staging a payment in the authenticated hardware inbox.
public enum OfflineCashHardwareStageDispositionV1: Equatable, Sendable {
  case staged
  case exactDuplicate
}

/// Staged payment result. The acknowledgement is emitted only after durable persistence.
public struct OfflineCashStagedPaymentV1: Equatable, Sendable {
  public let disposition: OfflineCashHardwareStageDispositionV1
  public let acknowledgement: OfflineCashAcknowledgementV1

  public init(
    disposition: OfflineCashHardwareStageDispositionV1,
    acknowledgement: OfflineCashAcknowledgementV1
  ) {
    self.disposition = disposition
    self.acknowledgement = acknowledgement
  }
}

/// Result of folding one staged credit into the aggregate balance.
public struct OfflineCashReceiveFoldResultV1: Equatable, Sendable {
  public let aggregateState: OfflineCashAggregateStateCommitmentV1

  public init(aggregateState: OfflineCashAggregateStateCommitmentV1) {
    self.aggregateState = aggregateState
  }
}

/// Mandatory qualified-device boundary used by `OfflineCashWalletV1`.
///
/// Implementations must delegate proof generation/verification, AEAD, signature operations,
/// capacity reservations, replay state, prepare/prove/commit, and recovery to the single audited
/// native core plus the governed non-forking hardware service. A software implementation of this
/// protocol is not an Offline Cash V1 provider.
public protocol OfflineCashHardwareProviderV1: AnyObject {
  func qualification() throws -> OfflineCashHardwareQualificationV1
  func recoverAggregateState() throws -> Data

  func createPaymentRequest(
    recipient: OfflineCashAccountIDV1,
    amount: OfflineCashUInt128V1,
    validityWindowMS: UInt64
  ) throws -> Data

  func prepareAcceptanceIntentAuthorization(
    canonicalRequest: Data,
    exactAmount: OfflineCashUInt128V1
  ) throws -> Data

  func verifyAuthorizationReserveInboxAndIssueTicket(
    canonicalRequest: Data,
    canonicalAuthorization: Data
  ) throws -> Data

  func prepareProveCommitPayment(
    canonicalRequest: Data,
    canonicalAuthorization: Data,
    canonicalTicket: Data
  ) throws -> Data

  func recoverPayment(acceptanceTicketID: Data) throws -> Data?

  func verifyAndStageInboundPayment(
    canonicalRequest: Data,
    canonicalAuthorization: Data,
    canonicalTicket: Data,
    canonicalPayment: Data
  ) throws -> (OfflineCashHardwareStageDispositionV1, Data)

  func releasePaymentOutbox(
    canonicalRequest: Data,
    canonicalPayment: Data,
    canonicalAcknowledgement: Data
  ) throws

  func prepareMintAuthorization(
    operationID: Data,
    amount: OfflineCashUInt128V1,
    payer: OfflineCashAccountIDV1,
    recipient: OfflineCashAccountIDV1
  ) throws -> Data

  func verifyAuthorizationAndStageMintCredit(
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws -> OfflineCashHardwareStageDispositionV1

  func pendingCreditWatermark() throws -> OfflineCashUInt128V1

  func foldNextReceive(
    upToInclusive watermark: OfflineCashUInt128V1
  ) throws -> Data?

  func prepareProveCommitRedemption(
    amount: OfflineCashUInt128V1,
    beneficiary: OfflineCashAccountIDV1
  ) throws -> Data

  func recoverRedemption(redemptionID: Data) throws -> Data?

  func rotateHardwareEpoch() throws -> Data
}

/// Hardware-authoritative orchestration around canonical V1 bytes.
///
/// This class never constructs a proof, signs a message, encrypts/decrypts a credit, derives an
/// identifier, or advances monetary state in Swift. It checks canonical public shape and ordering
/// around provider calls, while native release authentication remains mandatory.
public final class OfflineCashWalletV1: @unchecked Sendable {
  private let provider: OfflineCashHardwareProviderV1
  private let lock = NSLock()
  private var qualificationValue: OfflineCashHardwareQualificationV1
  private var aggregateStateValue: OfflineCashAggregateStateCommitmentV1

  private init(
    provider: OfflineCashHardwareProviderV1,
    qualification: OfflineCashHardwareQualificationV1,
    aggregateState: OfflineCashAggregateStateCommitmentV1
  ) {
    self.provider = provider
    qualificationValue = qualification
    aggregateStateValue = aggregateState
  }

  /// Open only after the provider returns a coherent governed profile, credential, and state.
  public static func open(provider: OfflineCashHardwareProviderV1) throws -> OfflineCashWalletV1 {
    let qualification = try provider.qualification()
    let state = try OfflineCashNoritoV1.decodeAggregateStateShapeExact(
      provider.recoverAggregateState())
    guard state.networkID == qualification.credential.networkID,
      state.releaseID == qualification.releaseID,
      state.hardwarePolicyID == qualification.hardwarePolicyDigest,
      state.hardwareEpochID == qualification.credential.hardwareEpochID,
      state.keyReference == qualification.credential.deviceKeyReference
    else { throw OfflineCashWalletErrorV1.invalidHardwareResult("recovered state binding") }
    return OfflineCashWalletV1(
      provider: provider, qualification: qualification, aggregateState: state)
  }

  public func qualification() -> OfflineCashHardwareQualificationV1 {
    lock.withLock { qualificationValue }
  }

  public func aggregateState() -> OfflineCashAggregateStateCommitmentV1 {
    lock.withLock { aggregateStateValue }
  }

  /// Ask qualified receiver hardware to create and sign an exact-amount request.
  public func createPaymentRequest(
    recipient: OfflineCashAccountIDV1,
    amount: OfflineCashUInt128V1,
    validityWindowMS: UInt64
  ) throws -> OfflineCashPaymentRequestV1 {
    guard !amount.isZero,
      validityWindowMS > 0, validityWindowMS <= OfflineCashWireV1.requestMaximumTTLMS
    else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult("invalid request amount or validity window")
    }
    return try lock.withLock {
      let value = try OfflineCashNoritoV1.decodePaymentRequestShapeExact(
        provider.createPaymentRequest(
          recipient: recipient, amount: amount, validityWindowMS: validityWindowMS))
      guard value.recipient == recipient, value.amount == amount,
        value.networkID == aggregateStateValue.networkID,
        value.asset == aggregateStateValue.asset,
        value.assetIncarnation == aggregateStateValue.assetIncarnation,
        value.releaseID == aggregateStateValue.releaseID,
        value.hardwareCredential == qualificationValue.credential
      else { throw OfflineCashWalletErrorV1.invalidHardwareResult("request binding") }
      return value
    }
  }

  /// Prepare a proof-bearing one-use sender authorization before receiver capacity is consumed.
  public func prepareAcceptanceIntentAuthorization(
    request: OfflineCashPaymentRequestV1
  ) throws -> OfflineCashAcceptanceIntentAuthorizationV1 {
    let canonicalRequest = try OfflineCashNoritoV1.encodePaymentRequestShape(request)
    let authorization = try OfflineCashNoritoV1.decodeAcceptanceIntentAuthorizationShapeExact(
      provider.prepareAcceptanceIntentAuthorization(
        canonicalRequest: canonicalRequest, exactAmount: request.amount))
    guard authorization.statement.intent.exactAmount == request.amount,
      authorization.statement.releaseID == request.releaseID,
      authorization.statement.suiteID == qualification().credential.suiteID
    else { throw OfflineCashWalletErrorV1.invalidHardwareResult("authorization binding") }
    return authorization
  }

  /// Verify sender authorization in native code, atomically reserve inbox bytes, then issue a ticket.
  public func issueAcceptanceTicket(
    request: OfflineCashPaymentRequestV1,
    authorization: OfflineCashAcceptanceIntentAuthorizationV1
  ) throws -> OfflineCashAcceptanceTicketV1 {
    let canonicalRequest = try OfflineCashNoritoV1.encodePaymentRequestShape(request)
    let canonicalAuthorization =
      try OfflineCashNoritoV1
      .encodeAcceptanceIntentAuthorizationShape(authorization)
    let ticket = try OfflineCashNoritoV1.decodeAcceptanceTicketShapeExact(
      provider.verifyAuthorizationReserveInboxAndIssueTicket(
        canonicalRequest: canonicalRequest, canonicalAuthorization: canonicalAuthorization))
    _ = try OfflineCashNoritoV1.validatePreTicketExchangeShape(
      request: request, authorization: authorization, ticket: ticket)
    return ticket
  }

  /// Execute recoverable prepare/prove/commit and expose only the final canonical payment.
  public func commitPayment(
    request: OfflineCashPaymentRequestV1,
    authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ticket: OfflineCashAcceptanceTicketV1
  ) throws -> OfflineCashPaymentV1 {
    _ = try OfflineCashNoritoV1.validatePreTicketExchangeShape(
      request: request, authorization: authorization, ticket: ticket)
    return try lock.withLock {
      _ = try drainStagedCreditsLocked()
      let payment = try OfflineCashNoritoV1.decodePaymentShapeExact(
        provider.prepareProveCommitPayment(
          canonicalRequest: OfflineCashNoritoV1.encodePaymentRequestShape(request),
          canonicalAuthorization:
            OfflineCashNoritoV1
            .encodeAcceptanceIntentAuthorizationShape(authorization),
          canonicalTicket: OfflineCashNoritoV1.encodeAcceptanceTicketShape(ticket)),
        against: request)
      guard payment.acceptanceIntent == authorization.statement.intent,
        payment.acceptanceTicket == ticket
      else { throw OfflineCashWalletErrorV1.invalidHardwareResult("payment pre-ticket binding") }
      return payment
    }
  }

  /// Recover the byte-identical final payment after a crash at any post-commit boundary.
  public func recoverPayment(
    request: OfflineCashPaymentRequestV1,
    acceptanceTicketID: Data
  ) throws -> OfflineCashPaymentV1? {
    guard offlineCashIsDigest(acceptanceTicketID) else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult("acceptance ticket id")
    }
    guard let bytes = try provider.recoverPayment(acceptanceTicketID: acceptanceTicketID) else {
      return nil
    }
    let payment = try OfflineCashNoritoV1.decodePaymentShapeExact(bytes, against: request)
    guard payment.acceptanceTicket.acceptanceTicketID == acceptanceTicketID,
      try OfflineCashNoritoV1.encodePaymentShape(payment, against: request) == bytes
    else { throw OfflineCashWalletErrorV1.conflictingRecoveredEnvelope }
    return payment
  }

  /// Stage a valid ticket-bound payment despite later traffic and return the durable ACK.
  public func stageInboundPayment(
    request: OfflineCashPaymentRequestV1,
    authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ticket: OfflineCashAcceptanceTicketV1,
    payment: OfflineCashPaymentV1
  ) throws -> OfflineCashStagedPaymentV1 {
    _ = try OfflineCashNoritoV1.validatePreTicketExchangeShape(
      request: request, authorization: authorization, ticket: ticket)
    let canonicalRequest = try OfflineCashNoritoV1.encodePaymentRequestShape(request)
    let canonicalAuthorization =
      try OfflineCashNoritoV1
      .encodeAcceptanceIntentAuthorizationShape(authorization)
    let canonicalTicket = try OfflineCashNoritoV1.encodeAcceptanceTicketShape(ticket)
    let canonicalPayment = try OfflineCashNoritoV1.encodePaymentShape(payment, against: request)
    let (disposition, acknowledgementBytes) = try provider.verifyAndStageInboundPayment(
      canonicalRequest: canonicalRequest, canonicalAuthorization: canonicalAuthorization,
      canonicalTicket: canonicalTicket, canonicalPayment: canonicalPayment)
    let acknowledgement = try OfflineCashNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgementBytes, against: request, payment: payment)
    _ = try OfflineCashNoritoV1.validateCompleteExchangeShape(
      request: request, authorization: authorization, ticket: ticket,
      payment: payment, acknowledgement: acknowledgement)
    return OfflineCashStagedPaymentV1(
      disposition: disposition, acknowledgement: acknowledgement)
  }

  /// Release the sender retry-outbox entry only after the exact ACK is validated in native code.
  public func releasePaymentOutbox(
    request: OfflineCashPaymentRequestV1,
    payment: OfflineCashPaymentV1,
    acknowledgement: OfflineCashAcknowledgementV1
  ) throws {
    try provider.releasePaymentOutbox(
      canonicalRequest: OfflineCashNoritoV1.encodePaymentRequestShape(request),
      canonicalPayment: OfflineCashNoritoV1.encodePaymentShape(payment, against: request),
      canonicalAcknowledgement: OfflineCashNoritoV1.encodeAcknowledgementShape(
        acknowledgement, against: request, payment: payment))
  }

  /// Ask recipient hardware for the exact proof-bearing authorization used before reserve debit.
  public func prepareMintAuthorization(
    operationID: Data,
    amount: OfflineCashUInt128V1,
    payer: OfflineCashAccountIDV1,
    recipient: OfflineCashAccountIDV1
  ) throws -> OfflineCashMintAuthorizationV1 {
    guard offlineCashIsDigest(operationID), !amount.isZero else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult("mint authorization input")
    }
    return try OfflineCashNoritoV1.decodeMintAuthorizationShapeExact(
      provider.prepareMintAuthorization(
        operationID: operationID, amount: amount, payer: payer, recipient: recipient))
  }

  /// Stage a mint credit only after native code verifies its exact pre-debit authorization.
  public func stageMintCredit(
    authorization: OfflineCashMintAuthorizationV1,
    credit: OfflineCashMintCreditV1
  ) throws -> OfflineCashHardwareStageDispositionV1 {
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
    else { throw OfflineCashWalletErrorV1.invalidHardwareResult("mint public binding") }
    return try provider.verifyAuthorizationAndStageMintCredit(
      canonicalAuthorization: OfflineCashNoritoV1.encodeMintAuthorizationShape(authorization),
      canonicalMintCredit: OfflineCashNoritoV1.encodeMintCreditShape(credit))
  }

  /// Fold the next staged credit into the aggregate balance.
  public func foldNextReceive() throws -> OfflineCashReceiveFoldResultV1?
  {
    return try lock.withLock {
      let watermark = try provider.pendingCreditWatermark()
      return try foldNextReceiveLocked(upToInclusive: watermark)
    }
  }

  /// Drain all staged credits one fixed-shape transition at a time before a send or redemption.
  public func drainStagedCredits() throws -> OfflineCashUInt128V1 {
    try lock.withLock { try drainStagedCreditsLocked() }
  }

  private func foldNextReceiveLocked(
    upToInclusive watermark: OfflineCashUInt128V1
  ) throws -> OfflineCashReceiveFoldResultV1?
  {
    guard let bytes = try provider.foldNextReceive(upToInclusive: watermark) else { return nil }
    let state = try OfflineCashNoritoV1.decodeAggregateStateShapeExact(bytes)
    guard sameBalanceIdentity(aggregateStateValue, state),
      state.hardwareEpochID == aggregateStateValue.hardwareEpochID,
      state.keyReference == aggregateStateValue.keyReference,
      state.hardwarePolicyID == aggregateStateValue.hardwarePolicyID,
      state.sequence == (try aggregateStateValue.sequence.adding(1)),
      state.stateCommitment != aggregateStateValue.stateCommitment
    else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult(
        "fold did not install the exact next aggregate state")
    }
    let result = OfflineCashReceiveFoldResultV1(aggregateState: state)
    aggregateStateValue = state
    return result
  }

  private func drainStagedCreditsLocked() throws -> OfflineCashUInt128V1 {
    let watermark = try provider.pendingCreditWatermark()
    var count = OfflineCashUInt128V1.zero
    while try foldNextReceiveLocked(upToInclusive: watermark) != nil {
      count = try count.adding(1)
    }
    return count
  }

  /// Execute recoverable prepare/prove/commit for an unlinkable terminal redemption.
  public func commitRedemption(
    amount: OfflineCashUInt128V1,
    beneficiary: OfflineCashAccountIDV1
  ) throws -> OfflineCashRedemptionVoucherV1 {
    guard !amount.isZero else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult("redemption amount")
    }
    return try lock.withLock {
      _ = try drainStagedCreditsLocked()
      return try OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(
        provider.prepareProveCommitRedemption(amount: amount, beneficiary: beneficiary))
    }
  }

  /// Recover a byte-identical redemption envelope after hardware commit.
  public func recoverRedemption(redemptionID: Data) throws -> OfflineCashRedemptionVoucherV1? {
    guard offlineCashIsDigest(redemptionID) else {
      throw OfflineCashWalletErrorV1.invalidHardwareResult("redemption id")
    }
    guard let bytes = try provider.recoverRedemption(redemptionID: redemptionID) else { return nil }
    let voucher = try OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact(bytes)
    guard voucher.statement.redemptionID == redemptionID,
      try OfflineCashNoritoV1.encodeRedemptionVoucherShape(voucher) == bytes
    else { throw OfflineCashWalletErrorV1.conflictingRecoveredEnvelope }
    return voucher
  }

  /// Drain the stable inbox snapshot, then rotate the complete private balance in hardware.
  public func rotateHardwareEpoch() throws -> OfflineCashAggregateStateCommitmentV1 {
    try lock.withLock {
      _ = try drainStagedCreditsLocked()
      let previousState = aggregateStateValue
      let previousQualification = qualificationValue
      let rotatedState = try OfflineCashNoritoV1.decodeAggregateStateShapeExact(
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
        throw OfflineCashWalletErrorV1.invalidHardwareResult(
          "rotation did not install the exact next hardware epoch")
      }
      qualificationValue = rotatedQualification
      aggregateStateValue = rotatedState
      return rotatedState
    }
  }

  private func sameBalanceIdentity(
    _ lhs: OfflineCashAggregateStateCommitmentV1,
    _ rhs: OfflineCashAggregateStateCommitmentV1
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
