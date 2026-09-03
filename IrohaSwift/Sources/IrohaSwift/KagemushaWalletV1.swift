import Foundation

/// Failures from the hardware-authoritative KAGEMUSHA V1 orchestration layer.
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

/// Result of atomically staging a credit in the authenticated hardware inbox.
public enum KagemushaHardwareStageDispositionV1: Equatable, Sendable {
  case staged
  case exactDuplicate
}

/// Durable result of staging one finalized mint credit in the authenticated hardware inbox.
public struct KagemushaHardwareMintStageV1: Equatable, Sendable {
  public let disposition: KagemushaHardwareStageDispositionV1
  public let creditID: Data

  public init(disposition: KagemushaHardwareStageDispositionV1, creditID: Data) throws {
    self.disposition = disposition
    self.creditID = try kagemushaDigest(creditID, "creditID")
  }
}

/// Native recovery after every interrupted prepare/commit transition has been resolved.
public struct KagemushaHardwareRecoveryV1: Equatable, Sendable {
  public let aggregateState: Data?
  public let journalRevision: KagemushaUInt128V1
  public let pendingCreditCount: KagemushaUInt128V1
  public let retryOutboxCount: KagemushaUInt128V1

  public init(
    aggregateState: Data?,
    journalRevision: KagemushaUInt128V1,
    pendingCreditCount: KagemushaUInt128V1,
    retryOutboxCount: KagemushaUInt128V1
  ) throws {
    guard aggregateState?.isEmpty != true else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("empty recovered aggregate state")
    }
    self.aggregateState = aggregateState.map { Data($0) }
    self.journalRevision = journalRevision
    self.pendingCreditCount = pendingCreditCount
    self.retryOutboxCount = retryOutboxCount
  }
}

/// Canonical terminal envelope plus the native-authoritative aggregate successor.
public struct KagemushaHardwareTerminalResultV1: Equatable, Sendable {
  public let canonicalEnvelope: Data
  public let aggregateState: Data

  public init(canonicalEnvelope: Data, aggregateState: Data) throws {
    guard !canonicalEnvelope.isEmpty, !aggregateState.isEmpty else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("empty terminal result")
    }
    self.canonicalEnvelope = Data(canonicalEnvelope)
    self.aggregateState = Data(aggregateState)
  }
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

/// Result of atomically folding one padded 1...16-credit batch into the aggregate balance.
public struct KagemushaReceiveFoldBatchResultV1: Equatable, Sendable {
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
/// protocol is not a KAGEMUSHA V1 provider.
public protocol KagemushaHardwareProviderV1: AnyObject {
  func qualification() throws -> KagemushaHardwareQualificationV1

  /// Resolve interrupted work and return the authoritative durable snapshot.
  func recover() throws -> KagemushaHardwareRecoveryV1

  /// Establish the hardware-bound zero state when recovery has no prior aggregate.
  func bootstrapState() throws -> Data

  /// Return the rollback-resistant native journal revision.
  func journalRevision() throws -> KagemushaUInt128V1

  func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    requestMode: KagemushaPaymentRequestModeV1,
    validityWindowMS: UInt64
  ) throws -> Data

  func prepareAcceptanceIntent(
    canonicalRequest: Data,
    exactAmount: KagemushaUInt128V1
  ) throws -> Data

  func recoverAcceptanceIntent(intentID: Data) throws -> Data?

  func validateIntentReserveInboxAndIssueAcceptanceTicket(
    canonicalRequest: Data,
    canonicalIntent: Data
  ) throws -> Data

  func recoverAcceptanceTicket(acceptanceTicketID: Data) throws -> Data?

  /// Fold only staged batches needed for the ticketed amount, then prepare, prove, commit,
  /// install the proof-bearing terminal envelope and certificate, and persist one payment.
  func prepareProveCommitPayment(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data
  ) throws -> KagemushaHardwareTerminalResultV1

  func recoverPayment(creditID: Data) throws -> Data?

  func verifyAndStageInboundPayment(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data,
    canonicalPayment: Data
  ) throws -> (KagemushaHardwareStageDispositionV1, Data)

  func releasePaymentOutbox(
    canonicalRequest: Data,
    canonicalIntent: Data,
    canonicalTicket: Data,
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
  ) throws -> KagemushaHardwareMintStageV1

  func pendingCreditWatermark() throws -> KagemushaUInt128V1

  /// Install one fixed-shape batch containing one through sixteen staged credits.
  func foldReceiveBatch(
    upToInclusive watermark: KagemushaUInt128V1
  ) throws -> Data?

  /// Fold only staged credits needed for `amount`, then prepare, prove, commit, and persist one
  /// redemption. Unrelated inbox backlog remains available to background folding.
  func prepareProveCommitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> KagemushaHardwareTerminalResultV1

  func recoverRedemption(redemptionID: Data) throws -> Data?

  /// Prove and install a distinct SuiteUpgrade transition; rotation is not an upgrade.
  func prepareProveCommitSuiteUpgrade(
    authorizationDigest: Data
  ) throws -> Data

  func rotateHardwareEpoch() throws -> Data
}

/// Hardware-authoritative orchestration around canonical V1 bytes.
///
/// This class never constructs a proof, signs a message, encrypts/decrypts a credit, derives an
/// identifier, or advances monetary state in Swift. It checks canonical public shape and ordering
/// around provider calls, while native release authentication remains mandatory.
public final class KagemushaWalletV1: @unchecked Sendable {
  private let provider: KagemushaHardwareProviderV1
  private let lock = KagemushaForegroundGateV1()
  private var qualificationValue: KagemushaHardwareQualificationV1
  private var aggregateStateValue: KagemushaAggregateStateCommitmentV1
  private var journalRevisionValue: KagemushaUInt128V1

  private init(
    provider: KagemushaHardwareProviderV1,
    qualification: KagemushaHardwareQualificationV1,
    aggregateState: KagemushaAggregateStateCommitmentV1,
    journalRevision: KagemushaUInt128V1
  ) {
    self.provider = provider
    qualificationValue = qualification
    aggregateStateValue = aggregateState
    journalRevisionValue = journalRevision
  }

  /// Open only after the provider returns a coherent governed profile, credential, and state.
  public static func open(provider: KagemushaHardwareProviderV1) throws -> KagemushaWalletV1 {
    let (qualification, recovery, state) = try authoritativeRecoverySnapshot(
      provider: provider, allowBootstrap: true)
    return KagemushaWalletV1(
      provider: provider, qualification: qualification, aggregateState: state,
      journalRevision: recovery.journalRevision)
  }

  /// Bootstrap bytes are usable only after native recovery corroborates their durable installation.
  /// A wallet with previously observed state must never recreate a missing native journal.
  private static func authoritativeRecoverySnapshot(
    provider: KagemushaHardwareProviderV1,
    allowBootstrap: Bool
  ) throws -> (
    KagemushaHardwareQualificationV1, KagemushaHardwareRecoveryV1,
    KagemushaAggregateStateCommitmentV1
  ) {
    _ = try provider.qualification()
    var recovery = try provider.recover()
    var qualification = try provider.qualification()
    if recovery.aggregateState == nil {
      guard allowBootstrap else {
        throw KagemushaWalletErrorV1.invalidHardwareResult("recovery lost the durable aggregate state")
      }
      let bootstrapped = try provider.bootstrapState()
      recovery = try provider.recover()
      qualification = try provider.qualification()
      guard recovery.aggregateState == bootstrapped else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "bootstrap state was not durably recovered")
      }
    }
    guard let stateBytes = recovery.aggregateState else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("recovery omitted the durable aggregate state")
    }
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(stateBytes)
    try Self.requireStateQualification(state, qualification)
    guard try provider.journalRevision() == recovery.journalRevision else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("recovery returned an inconsistent journal")
    }
    return (qualification, recovery, state)
  }

  public func qualification() -> KagemushaHardwareQualificationV1 {
    lock.withLock { qualificationValue }
  }

  public func aggregateState() -> KagemushaAggregateStateCommitmentV1 {
    lock.withLock { aggregateStateValue }
  }

  /// Return the latest rollback-resistant journal revision observed from native core.
  public func journalRevision() -> KagemushaUInt128V1 {
    lock.withLock { journalRevisionValue }
  }

  /// Resolve interrupted native work and refresh the authoritative wallet snapshot.
  @discardableResult
  public func recover() throws -> KagemushaHardwareRecoveryV1 {
    try lock.withLock {
      let (qualification, recovery, state) = try Self.authoritativeRecoverySnapshot(
        provider: provider, allowBootstrap: false)
      let revision = recovery.journalRevision
      guard sameBalanceIdentity(aggregateStateValue, state, includingRelease: false),
        qualification.credential.laneCommitment == qualificationValue.credential.laneCommitment
      else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "recovery changed the wallet identity or returned an inconsistent journal")
      }
      let previousGeneration = qualificationValue.credential.hardwareEpochGeneration
      let recoveredGeneration = qualification.credential.hardwareEpochGeneration
      if state.hardwareEpochID == aggregateStateValue.hardwareEpochID {
        guard recoveredGeneration == previousGeneration,
          state.keyReference == aggregateStateValue.keyReference,
          journalRevisionValue.isLessThanOrEqual(to: revision),
          revision != journalRevisionValue || state == aggregateStateValue
        else {
          throw KagemushaWalletErrorV1.invalidHardwareResult(
            "recovery rolled back or equivocated durable state")
        }
      } else {
        // Journals and aggregate sequence counters are epoch-scoped. Native authenticates
        // rotation; the host only rejects stale/reused generations and cross-wallet recovery.
        guard recoveredGeneration > previousGeneration else {
          throw KagemushaWalletErrorV1.invalidHardwareResult(
            "recovery did not advance the authenticated hardware epoch")
        }
      }
      qualificationValue = qualification
      aggregateStateValue = state
      journalRevisionValue = revision
      return try KagemushaHardwareRecoveryV1(
        aggregateState: KagemushaNoritoV1.encodeAggregateStateShape(state),
        journalRevision: revision,
        pendingCreditCount: recovery.pendingCreditCount,
        retryOutboxCount: recovery.retryOutboxCount)
    }
  }

  /// Ask qualified receiver hardware to create and sign one closed request policy.
  public func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    requestMode: KagemushaPaymentRequestModeV1,
    validityWindowMS: UInt64
  ) throws -> KagemushaPaymentRequestV1 {
    guard validityWindowMS > 0, validityWindowMS <= KagemushaWireV1.requestMaximumTTLMS
    else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("invalid request validity window")
    }
    return try lock.withLock {
      let value = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
        provider.createPaymentRequest(
          recipient: recipient, requestMode: requestMode,
          validityWindowMS: validityWindowMS))
      guard value.recipient == recipient, value.requestMode == requestMode,
        value.networkID == aggregateStateValue.networkID,
        value.asset == aggregateStateValue.asset,
        value.assetIncarnation == aggregateStateValue.assetIncarnation,
        value.releaseID == aggregateStateValue.releaseID,
        value.hardwareCredential == qualificationValue.credential
      else { throw KagemushaWalletErrorV1.invalidHardwareResult("request binding") }
      return value
    }
  }

  /// Produce IPM1 message 2 through qualified sender hardware.
  public func prepareAcceptanceIntent(
    request: KagemushaPaymentRequestV1,
    exactAmount: KagemushaUInt128V1
  ) throws -> KagemushaAcceptanceIntentV1 {
    guard request.requestMode.acceptsPaymentAmount(exactAmount) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("amount rejected by request mode")
    }
    let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
    let value = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      provider.prepareAcceptanceIntent(
        canonicalRequest: canonicalRequest, exactAmount: exactAmount),
      against: request)
    guard value.exactAmount == exactAmount else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("intent amount binding")
    }
    return value
  }

  /// Recover IPM1 message 2 byte-identically after a sender crash.
  public func recoverAcceptanceIntent(
    request: KagemushaPaymentRequestV1,
    intentID: Data
  ) throws -> KagemushaAcceptanceIntentV1? {
    guard kagemushaIsDigest(intentID) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("intent id")
    }
    guard let bytes = try provider.recoverAcceptanceIntent(intentID: intentID)
    else { return nil }
    let value = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      bytes, against: request)
    guard value.intentID == intentID,
      try KagemushaNoritoV1.encodeAcceptanceIntentShape(
        value, against: request) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    return value
  }

  /// Verify message 2 and reserve receiver inbox capacity before issuing message 3.
  public func issueAcceptanceTicket(
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1
  ) throws -> KagemushaAcceptanceTicketV1 {
    let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
    let canonicalIntent =
      try KagemushaNoritoV1
      .encodeAcceptanceIntentShape(intent, against: request)
    let ticket = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      provider.validateIntentReserveInboxAndIssueAcceptanceTicket(
        canonicalRequest: canonicalRequest,
        canonicalIntent: canonicalIntent),
      against: request, intent: intent)
    _ = try KagemushaNoritoV1.validatePreTicketExchangeShape(
      request: request, intent: intent, ticket: ticket)
    return ticket
  }

  /// Recover the exact receiver reservation without issuing a second ticket.
  public func recoverAcceptanceTicket(
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1,
    acceptanceTicketID: Data
  ) throws -> KagemushaAcceptanceTicketV1? {
    guard kagemushaIsDigest(acceptanceTicketID) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("acceptance ticket id")
    }
    guard
      let bytes = try provider.recoverAcceptanceTicket(
        acceptanceTicketID: acceptanceTicketID)
    else { return nil }
    let value = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      bytes, against: request, intent: intent)
    guard value.acceptanceTicketID == acceptanceTicketID,
      try KagemushaNoritoV1.encodeAcceptanceTicketShape(
        value, against: request, intent: intent) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    return value
  }

  /// Execute recoverable prepare/prove/commit and expose only the final canonical payment.
  /// Qualified hardware folds only staged credits required to cover the requested amount.
  public func commitPayment(
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1,
    ticket: KagemushaAcceptanceTicketV1
  ) throws -> KagemushaPaymentV1 {
    return try lock.withLock {
      let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
      let canonicalIntent =
        try KagemushaNoritoV1
        .encodeAcceptanceIntentShape(intent, against: request)
      let canonicalTicket = try KagemushaNoritoV1.encodeAcceptanceTicketShape(
        ticket, against: request, intent: intent)
      let previousState = aggregateStateValue
      let previousRevision = journalRevisionValue
      let result = try provider.prepareProveCommitPayment(
        canonicalRequest: canonicalRequest,
        canonicalIntent: canonicalIntent,
        canonicalTicket: canonicalTicket)
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
        result.canonicalEnvelope,
        against: request, intent: intent, ticket: ticket)
      _ = try KagemushaNoritoV1.validateCommittedPaymentShape(
        request: request, intent: intent, ticket: ticket, payment: payment)
      let installed = try validatedSuccessor(
        result.aggregateState, after: previousState, journalRevision: previousRevision,
        operation: "payment")
      aggregateStateValue = installed.state
      journalRevisionValue = installed.revision
      return payment
    }
  }

  /// Recover the byte-identical final payment after a crash at any post-commit boundary.
  public func recoverPayment(
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1,
    ticket: KagemushaAcceptanceTicketV1,
    creditID: Data
  ) throws -> KagemushaPaymentV1? {
    guard kagemushaIsDigest(creditID) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("credit id")
    }
    guard let bytes = try provider.recoverPayment(creditID: creditID) else {
      return nil
    }
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      bytes, against: request, intent: intent, ticket: ticket)
    guard payment.output.creditID == creditID,
      try KagemushaNoritoV1.encodePaymentShape(
        payment, against: request, intent: intent, ticket: ticket) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    _ = try KagemushaNoritoV1.validateCommittedPaymentShape(
      request: request, intent: intent, ticket: ticket, payment: payment)
    return payment
  }

  /// Stage a valid receiver-bound payment despite later traffic and return the durable ACK.
  public func stageInboundPayment(
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1,
    ticket: KagemushaAcceptanceTicketV1,
    payment: KagemushaPaymentV1
  ) throws -> KagemushaStagedPaymentV1 {
    try lock.withLock {
      let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
      let canonicalIntent =
        try KagemushaNoritoV1
        .encodeAcceptanceIntentShape(intent, against: request)
      let canonicalTicket = try KagemushaNoritoV1.encodeAcceptanceTicketShape(
        ticket, against: request, intent: intent)
      _ = try KagemushaNoritoV1.validateCommittedPaymentShape(
        request: request, intent: intent, ticket: ticket, payment: payment)
      let canonicalPayment = try KagemushaNoritoV1.encodePaymentShape(
        payment, against: request, intent: intent, ticket: ticket)
      let before = try provider.journalRevision()
      let (disposition, acknowledgementBytes) = try provider.verifyAndStageInboundPayment(
        canonicalRequest: canonicalRequest,
        canonicalIntent: canonicalIntent,
        canonicalTicket: canonicalTicket,
        canonicalPayment: canonicalPayment)
      let acknowledgement = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
        acknowledgementBytes, against: request, intent: intent,
        ticket: ticket, payment: payment)
      _ = try KagemushaNoritoV1.validateCompleteExchangeShape(
        request: request, intent: intent, ticket: ticket,
        payment: payment, acknowledgement: acknowledgement)
      let after = try provider.journalRevision()
      // Staging advances the native inbox revision, not the monetary-state journal.
      // Read immediately before the call so an exact retry can recover a lost durable ACK.
      guard after == before else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "payment staging changed the monetary-state journal")
      }
      journalRevisionValue = after
      return KagemushaStagedPaymentV1(
        disposition: disposition, acknowledgement: acknowledgement)
    }
  }

  /// Release the sender retry-outbox entry only after the exact ACK is validated in native code.
  public func releasePaymentOutbox(
    request: KagemushaPaymentRequestV1,
    intent: KagemushaAcceptanceIntentV1,
    ticket: KagemushaAcceptanceTicketV1,
    payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1
  ) throws {
    _ = try KagemushaNoritoV1.validateCompleteExchangeShape(
      request: request, intent: intent, ticket: ticket,
      payment: payment, acknowledgement: acknowledgement)
    try provider.releasePaymentOutbox(
      canonicalRequest: KagemushaNoritoV1.encodePaymentRequestShape(request),
      canonicalIntent: KagemushaNoritoV1.encodeAcceptanceIntentShape(
        intent, against: request),
      canonicalTicket: KagemushaNoritoV1.encodeAcceptanceTicketShape(
        ticket, against: request, intent: intent),
      canonicalPayment: KagemushaNoritoV1.encodePaymentShape(
        payment, against: request, intent: intent, ticket: ticket),
      canonicalAcknowledgement: KagemushaNoritoV1.encodeAcknowledgementShape(
        acknowledgement, against: request, intent: intent,
        ticket: ticket, payment: payment))
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
    return try lock.withLock {
      let canonicalAuthorization = try KagemushaNoritoV1.encodeMintAuthorizationShape(authorization)
      let canonicalCredit = try KagemushaNoritoV1.encodeMintCreditShape(
        credit, against: authorization)
      let before = try provider.journalRevision()
      let staged = try provider.verifyAuthorizationAndStageMintCredit(
        canonicalAuthorization: canonicalAuthorization,
        canonicalMintCredit: canonicalCredit)
      guard staged.creditID == credit.statement.lifecycle.creditID else {
        throw KagemushaWalletErrorV1.invalidHardwareResult("mint staging credit ID mismatch")
      }
      let after = try provider.journalRevision()
      // Only the subsequent authenticated MintFold consumes monetary state.
      guard after == before else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "mint staging changed the monetary-state journal")
      }
      journalRevisionValue = after
      return staged.disposition
    }
  }

  /// Fold the next padded 1...16-credit batch into the aggregate balance.
  public func foldNextReceiveBatch() throws -> KagemushaReceiveFoldBatchResultV1? {
    return try lock.withLock {
      let watermark = try provider.pendingCreditWatermark()
      return try foldNextReceiveBatchLocked(upToInclusive: watermark)
    }
  }

  /// Drain one epoch-bound snapshot, yielding to queued foreground work after every batch.
  /// A concurrent epoch rotation interrupts this pass; retry to capture its new watermark.
  public func drainStagedCredits() throws -> KagemushaUInt128V1 {
    let snapshot = try lock.withBackgroundLock {
      (
        watermark: try provider.pendingCreditWatermark(),
        epochID: aggregateStateValue.hardwareEpochID,
        generation: qualificationValue.credential.hardwareEpochGeneration
      )
    }
    var count = KagemushaUInt128V1.zero
    while true {
      let batch = try lock.withBackgroundLock {
        guard aggregateStateValue.hardwareEpochID == snapshot.epochID,
          qualificationValue.credential.hardwareEpochGeneration == snapshot.generation
        else {
          throw KagemushaWalletErrorV1.invalidHardwareResult(
            "hardware epoch changed during inbox drain; start a new drain pass")
        }
        return try foldNextReceiveBatchLocked(upToInclusive: snapshot.watermark)
      }
      guard batch != nil else { return count }
      count = try count.adding(1)
    }
  }

  private func foldNextReceiveBatchLocked(
    upToInclusive watermark: KagemushaUInt128V1
  ) throws -> KagemushaReceiveFoldBatchResultV1? {
    let beforeRevision = journalRevisionValue
    guard let bytes = try provider.foldReceiveBatch(upToInclusive: watermark) else {
      guard try provider.journalRevision() == beforeRevision else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "empty receive fold changed the journal revision")
      }
      return nil
    }
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
    try Self.requireStateQualification(state, qualificationValue)
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
    let afterRevision = try provider.journalRevision()
    guard afterRevision == (try beforeRevision.adding(1)) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult(
        "receive fold did not consume exactly one journal revision")
    }
    let result = KagemushaReceiveFoldBatchResultV1(aggregateState: state)
    aggregateStateValue = state
    journalRevisionValue = afterRevision
    return result
  }

  /// Execute recoverable prepare/prove/commit for an unlinkable terminal redemption.
  /// Qualified hardware folds only staged credits required to cover `amount`.
  public func commitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> KagemushaRedemptionVoucherV1 {
    guard !amount.isZero else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("redemption amount")
    }
    return try lock.withLock {
      let previousState = aggregateStateValue
      let previousRevision = journalRevisionValue
      let result = try provider.prepareProveCommitRedemption(
        amount: amount, beneficiary: beneficiary)
      let voucher = try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(
        result.canonicalEnvelope)
      guard voucher.statement.amount == amount, voucher.statement.beneficiary == beneficiary else {
        throw KagemushaWalletErrorV1.invalidHardwareResult("redemption output binding")
      }
      let installed = try validatedSuccessor(
        result.aggregateState, after: previousState, journalRevision: previousRevision,
        operation: "redemption")
      aggregateStateValue = installed.state
      journalRevisionValue = installed.revision
      return voucher
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

  /// Install a distinct SuiteUpgrade relation without masquerading as hardware rotation.
  public func upgradeSuite(
    authorizationDigest: Data
  ) throws -> KagemushaAggregateStateCommitmentV1 {
    guard kagemushaIsDigest(authorizationDigest) else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("suite upgrade authorization")
    }
    return try lock.withLock {
      let previous = aggregateStateValue
      let previousRevision = journalRevisionValue
      let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(
        provider.prepareProveCommitSuiteUpgrade(
          authorizationDigest: authorizationDigest))
      let qualification = try provider.qualification()
      try Self.requireStateQualification(state, qualification)
      guard sameBalanceIdentity(previous, state, includingRelease: false),
        state.releaseID != previous.releaseID,
        state.hardwareEpochID == previous.hardwareEpochID,
        state.keyReference == previous.keyReference,
        state.hardwarePolicyID == previous.hardwarePolicyID,
        qualification.credential.hardwareEpochGeneration
          == qualificationValue.credential.hardwareEpochGeneration,
        qualification.credential.laneCommitment == qualificationValue.credential.laneCommitment,
        qualification.credential.devicePublicKey == qualificationValue.credential.devicePublicKey,
        qualification.profile.hardwareProfileID == qualificationValue.profile.hardwareProfileID,
        qualification.profile.policyEpoch == qualificationValue.profile.policyEpoch,
        state.sequence == (try previous.sequence.adding(1)),
        state.stateCommitment != previous.stateCommitment
      else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "suite upgrade did not install the exact next aggregate state")
      }
      let revision = try provider.journalRevision()
      guard revision == (try previousRevision.adding(1)) else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "suite upgrade did not consume exactly one journal revision")
      }
      qualificationValue = qualification
      aggregateStateValue = state
      journalRevisionValue = revision
      return state
    }
  }

  /// Rotate the complete private balance, replay state, and pending inbox in hardware.
  /// No receive fold precedes rotation: the old epoch's counters may already be exhausted.
  public func rotateHardwareEpoch() throws -> KagemushaAggregateStateCommitmentV1 {
    try lock.withLock {
      let previousState = aggregateStateValue
      let previousQualification = qualificationValue
      guard previousQualification.credential.hardwareEpochGeneration < UInt64.max else {
        throw KagemushaWalletErrorV1.invalidHardwareResult("hardware epoch generation exhausted")
      }
      let rotatedState = try KagemushaNoritoV1.decodeAggregateStateShapeExact(
        provider.rotateHardwareEpoch())
      let rotatedQualification = try provider.qualification()
      try Self.requireStateQualification(rotatedState, rotatedQualification)
      guard rotatedQualification.releaseID == previousQualification.releaseID,
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
        rotatedState.stateCommitment != previousState.stateCommitment,
        rotatedState.sequence.isZero
      else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "rotation did not install the exact next hardware epoch")
      }
      let revision = try provider.journalRevision()
      guard revision.isZero else {
        throw KagemushaWalletErrorV1.invalidHardwareResult(
          "rotation did not reset the new epoch journal revision")
      }
      qualificationValue = rotatedQualification
      aggregateStateValue = rotatedState
      journalRevisionValue = revision
      return rotatedState
    }
  }

  private func sameBalanceIdentity(
    _ lhs: KagemushaAggregateStateCommitmentV1,
    _ rhs: KagemushaAggregateStateCommitmentV1,
    includingRelease: Bool = true
  ) -> Bool {
    (!includingRelease || lhs.releaseID == rhs.releaseID)
      && lhs.networkID == rhs.networkID
      && lhs.asset == rhs.asset
      && lhs.assetIncarnation == rhs.assetIncarnation
      && lhs.scale == rhs.scale
      && lhs.liabilityPoolID == rhs.liabilityPoolID
      && lhs.laneID == rhs.laneID
  }

  private func validatedSuccessor(
    _ bytes: Data,
    after previous: KagemushaAggregateStateCommitmentV1,
    journalRevision previousRevision: KagemushaUInt128V1,
    operation: String
  ) throws -> (state: KagemushaAggregateStateCommitmentV1, revision: KagemushaUInt128V1) {
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
    guard sameBalanceIdentity(previous, state),
      state.hardwareEpochID == previous.hardwareEpochID,
      state.keyReference == previous.keyReference,
      state.hardwarePolicyID == previous.hardwarePolicyID,
      previous.sequence.isLessThanOrEqual(to: state.sequence),
      state.sequence != previous.sequence,
      state.stateCommitment != previous.stateCommitment
    else {
      throw KagemushaWalletErrorV1.invalidHardwareResult(
        "\(operation) did not advance the aggregate state")
    }
    let revision = try provider.journalRevision()
    // Native may fold multiple required batches before terminalizing. The authenticated
    // provider owns journal semantics; the host must not infer its delta from the sequence.
    guard previousRevision.isLessThanOrEqual(to: revision), revision != previousRevision else {
      throw KagemushaWalletErrorV1.invalidHardwareResult(
        "\(operation) did not advance the journal revision")
    }
    return (state, revision)
  }

  private static func requireStateQualification(
    _ state: KagemushaAggregateStateCommitmentV1,
    _ qualification: KagemushaHardwareQualificationV1
  ) throws {
    guard state.networkID == qualification.credential.networkID,
      state.releaseID == qualification.releaseID,
      state.hardwarePolicyID == qualification.hardwarePolicyDigest,
      state.hardwareEpochID == qualification.credential.hardwareEpochID,
      state.keyReference == qualification.credential.deviceKeyReference
    else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("recovered state binding")
    }
  }
}

/// Private host scheduling only: one lease at a time, with foreground priority between batches.
private final class KagemushaForegroundGateV1 {
  private let condition = NSCondition()
  private var occupied = false
  private var foregroundWaiters = 0

  func withLock<T>(_ body: () throws -> T) rethrows -> T {
    try withLease(background: false, body)
  }

  func withBackgroundLock<T>(_ body: () throws -> T) rethrows -> T {
    try withLease(background: true, body)
  }

  private func withLease<T>(background: Bool, _ body: () throws -> T) rethrows -> T {
    condition.lock()
    if !background { foregroundWaiters += 1 }
    while occupied || (background && foregroundWaiters > 0) {
      condition.wait()
    }
    if !background { foregroundWaiters -= 1 }
    occupied = true
    condition.unlock()
    defer {
      condition.lock()
      occupied = false
      condition.broadcast()
      condition.unlock()
    }
    return try body()
  }
}
