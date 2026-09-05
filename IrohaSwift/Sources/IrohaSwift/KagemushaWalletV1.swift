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
  public let coreAuthorizationKeyReference: Data
  public let profile: KagemushaHardwareProfileV1
  public let credential: KagemushaHardwareCredentialV1

  public init(
    releaseID: Data, hardwarePolicyDigest: Data, coreAuthorizationKeyReference: Data,
    profile: KagemushaHardwareProfileV1, credential: KagemushaHardwareCredentialV1
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
    self.coreAuthorizationKeyReference = try kagemushaDigest(
      coreAuthorizationKeyReference, "coreAuthorizationKeyReference")
    self.profile = profile
    self.credential = credential
  }
}

/// Whether inbound data was newly staged or matched an exact durable duplicate.
public enum KagemushaHardwareStageDispositionV1: Equatable, Sendable {
  case staged
  case exactDuplicate
}

/// Durable result of staging one verified peer payment.
public struct KagemushaHardwarePaymentStageV1: Equatable, Sendable {
  public let disposition: KagemushaHardwareStageDispositionV1
  public let creditID: Data
  public let canonicalAcknowledgement: Data

  public init(
    disposition: KagemushaHardwareStageDispositionV1,
    creditID: Data, canonicalAcknowledgement: Data
  ) throws {
    guard !canonicalAcknowledgement.isEmpty,
      canonicalAcknowledgement.count <= KagemushaWireV1.maximumAcknowledgementBytes
    else { throw KagemushaWalletErrorV1.invalidHardwareResult("acknowledgement") }
    self.disposition = disposition
    self.creditID = try kagemushaDigest(creditID, "creditID")
    self.canonicalAcknowledgement = Data(canonicalAcknowledgement)
  }
}

/// Durable result of staging one finalized mint credit.
public struct KagemushaHardwareMintStageV1: Equatable, Sendable {
  public let disposition: KagemushaHardwareStageDispositionV1
  public let creditID: Data

  public init(disposition: KagemushaHardwareStageDispositionV1, creditID: Data) throws {
    self.disposition = disposition
    self.creditID = try kagemushaDigest(creditID, "creditID")
  }
}

/// Native recovery after every interrupted transition has been resolved.
public struct KagemushaHardwareRecoveryV1: Equatable, Sendable {
  public let aggregateState: Data?
  public let journalRevision: KagemushaUInt128V1
  public let pendingCreditCount: KagemushaUInt128V1
  public let retryOutboxCount: KagemushaUInt128V1

  public init(
    aggregateState: Data?, journalRevision: KagemushaUInt128V1,
    pendingCreditCount: KagemushaUInt128V1, retryOutboxCount: KagemushaUInt128V1
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

/// One exact `MintFold` or `ReceiveFold` transition and the staged credit it consumes.
public struct KagemushaHardwareReceiveFoldV1: Equatable, Sendable {
  public let aggregateState: Data
  public let selector: KagemushaPendingCreditSelectorV1

  public init(aggregateState: Data, selector: KagemushaPendingCreditSelectorV1) throws {
    guard !aggregateState.isEmpty else {
      throw KagemushaWalletErrorV1.invalidHardwareResult("empty pending-fold state")
    }
    self.aggregateState = Data(aggregateState)
    self.selector = selector
  }
}

/// Public result of installing one authenticated pending credit.
public struct KagemushaReceiveFoldResultV1: Equatable, Sendable {
  public let aggregateState: KagemushaAggregateStateCommitmentV1
  public let selector: KagemushaPendingCreditSelectorV1

  public init(
    aggregateState: KagemushaAggregateStateCommitmentV1,
    selector: KagemushaPendingCreditSelectorV1
  ) {
    self.aggregateState = aggregateState
    self.selector = selector
  }
}

/// Acknowledgement emitted only after irreversible secure staging.
public struct KagemushaStagedPaymentV1: Equatable, Sendable {
  public let disposition: KagemushaHardwareStageDispositionV1
  public let acknowledgement: KagemushaAcknowledgementV1
  public let canonicalAcknowledgement: Data

  public init(
    disposition: KagemushaHardwareStageDispositionV1,
    acknowledgement: KagemushaAcknowledgementV1,
    canonicalAcknowledgement: Data
  ) {
    self.disposition = disposition
    self.acknowledgement = acknowledgement
    self.canonicalAcknowledgement = Data(canonicalAcknowledgement)
  }
}

/// Mandatory non-forking secure-device boundary. There is no software fallback.
public protocol KagemushaHardwareProviderV1: AnyObject {
  func qualification() throws -> KagemushaHardwareQualificationV1
  func recover() throws -> KagemushaHardwareRecoveryV1
  func bootstrapState() throws -> Data
  func journalRevision() throws -> KagemushaUInt128V1

  func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    validityWindowMS: UInt64
  ) throws -> Data

  func stagePayment(
    canonicalRequest: Data,
    canonicalPayment: Data
  ) throws -> KagemushaHardwarePaymentStageV1

  func reservePaymentOperationID(canonicalRequest: Data) throws -> Data

  func prepareProveCommitPayment(
    operationID: Data,
    canonicalRequest: Data
  ) throws -> KagemushaHardwareTerminalResultV1

  func recoverPayment(creditID: Data) throws -> Data?
  func recoverPaymentByOperationID(operationID: Data, canonicalRequest: Data) throws -> Data?

  func recordAcknowledgement(
    creditID: Data, canonicalRequest: Data,
    canonicalPayment: Data, canonicalAcknowledgement: Data
  ) throws

  func reserveMintOperationID(
    amount: KagemushaUInt128V1, payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  ) throws -> Data

  func prepareMintConstructionBundle(
    operationID: Data, amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1, recipient: KagemushaAccountIDV1
  ) throws -> KagemushaMintConstructionBundleV1

  func recoverMintConstructionBundle(
    operationID: Data
  ) throws -> KagemushaMintConstructionBundleV1?

  func verifyAuthorizationAndStageMintCredit(
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws -> KagemushaHardwareMintStageV1

  func selectPendingCredit(
    watermark: KagemushaPendingCreditWatermarkV1?,
    target: KagemushaPendingCreditTargetV1
  ) throws -> KagemushaPendingCreditSelectionV1
  func foldPendingCredit(
    selector: KagemushaPendingCreditSelectorV1
  ) throws -> KagemushaHardwareReceiveFoldV1

  func prepareProveCommitRedemption(
    operationID: Data,
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> KagemushaHardwareTerminalResultV1

  func reserveRedemptionOperationID(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> Data
  func recoverRedemption(redemptionID: Data) throws -> Data?
  func recoverRedemptionByOperationID(operationID: Data) throws -> Data?
  func rotateHardwareEpoch() throws -> Data
}

/// Aggregate-balance KAGEMUSHA V1 orchestration over the authoritative hardware boundary.
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

  public static func open(provider: KagemushaHardwareProviderV1) throws -> KagemushaWalletV1 {
    let snapshot = try authoritativeRecoverySnapshot(provider: provider, allowBootstrap: true)
    return KagemushaWalletV1(
      provider: provider, qualification: snapshot.qualification,
      aggregateState: snapshot.state, journalRevision: snapshot.recovery.journalRevision)
  }

  public func qualification() -> KagemushaHardwareQualificationV1 {
    lock.withLock { qualificationValue }
  }

  public func aggregateState() -> KagemushaAggregateStateCommitmentV1 {
    lock.withLock { aggregateStateValue }
  }

  public func journalRevision() -> KagemushaUInt128V1 {
    lock.withLock { journalRevisionValue }
  }

  @discardableResult
  public func recover() throws -> KagemushaHardwareRecoveryV1 {
    try lock.withLock {
      let snapshot = try Self.authoritativeRecoverySnapshot(
        provider: provider, allowBootstrap: false)
      guard sameBalanceIdentity(aggregateStateValue, snapshot.state, includingRelease: false),
        snapshot.qualification.credential.laneCommitment
          == qualificationValue.credential.laneCommitment
      else { throw invalid("recovery changed wallet identity") }
      if snapshot.state.hardwareEpochID == aggregateStateValue.hardwareEpochID {
        guard snapshot.qualification.credential.hardwareEpochGeneration
            == qualificationValue.credential.hardwareEpochGeneration,
          journalRevisionValue.isLessThanOrEqual(to: snapshot.recovery.journalRevision),
          snapshot.recovery.journalRevision != journalRevisionValue
            || snapshot.state == aggregateStateValue
        else { throw invalid("recovery rolled back or equivocated") }
      } else {
        guard snapshot.qualification.credential.hardwareEpochGeneration
          > qualificationValue.credential.hardwareEpochGeneration
        else { throw invalid("recovery reused a hardware epoch") }
      }
      qualificationValue = snapshot.qualification
      aggregateStateValue = snapshot.state
      journalRevisionValue = snapshot.recovery.journalRevision
      return snapshot.recovery
    }
  }

  /// Create a signed positive exact-amount request. It never binds the receiver balance head.
  public func createPaymentRequest(
    recipient: KagemushaAccountIDV1,
    amount: KagemushaUInt128V1,
    validityWindowMS: UInt64
  ) throws -> KagemushaPaymentRequestV1 {
    guard !amount.isZero, validityWindowMS > 0,
      validityWindowMS <= KagemushaWireV1.requestMaximumTTLMS
    else { throw invalid("invalid request amount or validity window") }
    return try lock.withLock {
      let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
        provider.createPaymentRequest(
          recipient: recipient, amount: amount, validityWindowMS: validityWindowMS))
      guard request.recipient == recipient, request.amount == amount,
        request.expiresAtMS - request.issuedAtMS == validityWindowMS,
        request.releaseID == aggregateStateValue.releaseID,
        request.networkID == aggregateStateValue.networkID,
        request.asset == aggregateStateValue.asset,
        request.assetIncarnation == aggregateStateValue.assetIncarnation,
        request.scale == aggregateStateValue.scale,
        request.liabilityPoolID == aggregateStateValue.liabilityPoolID
      else { throw invalid("request binding") }
      return request
    }
  }

  /// Reserve and return the identity the caller must persist before beginning a payment.
  public func reservePaymentOperationID(
    request: KagemushaPaymentRequestV1
  ) throws -> Data {
    try provider.reservePaymentOperationID(
      canonicalRequest: KagemushaNoritoV1.encodePaymentRequestShape(request))
  }

  /// Commit a receiver-bound payment using the caller-persisted operation identity.
  public func commitPayment(
    request: KagemushaPaymentRequestV1,
    operationID: Data
  ) throws -> KagemushaPaymentV1 {
    let operationID = try kagemushaDigest(operationID, "operationID")
    return try lock.withLock {
      try foldRequiredCreditsLocked(requiredBalance: request.amount)
      let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
      let result = try provider.prepareProveCommitPayment(
        operationID: operationID,
        canonicalRequest: canonicalRequest
      )
      let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
        result.canonicalEnvelope, against: request)
      try installAuthoritativeState(result.aggregateState)
      return payment
    }
  }

  public func recoverPayment(
    request: KagemushaPaymentRequestV1, creditID: Data
  ) throws -> KagemushaPaymentV1? {
    let expected = try kagemushaDigest(creditID, "creditID")
    guard let bytes = try provider.recoverPayment(creditID: expected) else { return nil }
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(bytes, against: request)
    guard payment.output.creditID == expected,
      try KagemushaNoritoV1.encodePaymentShape(payment, against: request) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    return payment
  }

  /// Recover after a crash which occurred before the terminal credit ID reached the caller.
  public func recoverPaymentByOperationID(
    request: KagemushaPaymentRequestV1,
    operationID: Data
  ) throws -> KagemushaPaymentV1? {
    let operationID = try kagemushaDigest(operationID, "operationID")
    let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
    guard
      let bytes = try provider.recoverPaymentByOperationID(
        operationID: operationID,
        canonicalRequest: canonicalRequest
      )
    else { return nil }
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(bytes, against: request)
    guard try KagemushaNoritoV1.encodePaymentShape(payment, against: request) == bytes else {
      throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope
    }
    return payment
  }

  /// ACK only after irreversible hardware inbox staging.
  public func stageInboundPayment(
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1
  ) throws -> KagemushaStagedPaymentV1 {
    try lock.withLock {
      let canonicalRequest = try KagemushaNoritoV1.encodePaymentRequestShape(request)
      let canonicalPayment = try KagemushaNoritoV1.encodePaymentShape(payment, against: request)
      let before = try provider.journalRevision()
      let staged = try provider.stagePayment(
        canonicalRequest: canonicalRequest, canonicalPayment: canonicalPayment)
      guard staged.creditID == payment.output.creditID else {
        throw invalid("staged credit ID mismatch")
      }
      let acknowledgement = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
        staged.canonicalAcknowledgement, against: request, payment: payment)
      let after = try provider.journalRevision()
      guard after == before else { throw invalid("inbox staging changed monetary journal") }
      journalRevisionValue = after
      return KagemushaStagedPaymentV1(
        disposition: staged.disposition, acknowledgement: acknowledgement,
        canonicalAcknowledgement: staged.canonicalAcknowledgement)
    }
  }

  public func recordAcknowledgement(
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1
  ) throws {
    _ = try KagemushaNoritoV1.validateCompleteExchangeShape(
      request: request, payment: payment, acknowledgement: acknowledgement)
    try provider.recordAcknowledgement(
      creditID: payment.output.creditID,
      canonicalRequest: KagemushaNoritoV1.encodePaymentRequestShape(request),
      canonicalPayment: KagemushaNoritoV1.encodePaymentShape(payment, against: request),
      canonicalAcknowledgement: KagemushaNoritoV1.encodeAcknowledgementShape(
        acknowledgement, against: request, payment: payment))
  }

  /// Reserve and return the ID the caller must persist before mint preparation.
  public func reserveMintOperationID(
    amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1,
    recipient: KagemushaAccountIDV1
  ) throws -> Data {
    try provider.reserveMintOperationID(amount: amount, payer: payer, recipient: recipient)
  }

  public func prepareMintConstructionBundle(
    operationID: Data, amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1, recipient: KagemushaAccountIDV1
  ) throws -> KagemushaMintConstructionBundleV1 {
    guard kagemushaIsDigest(operationID), !amount.isZero else {
      throw invalid("mint authorization input")
    }
    return try provider.prepareMintConstructionBundle(
      operationID: operationID, amount: amount, payer: payer, recipient: recipient)
  }

  public func recoverMintConstructionBundle(
    operationID: Data
  ) throws -> KagemushaMintConstructionBundleV1? {
    guard kagemushaIsDigest(operationID) else { throw invalid("mint operation ID") }
    return try provider.recoverMintConstructionBundle(operationID: operationID)
  }

  /// Prepare the complete immutable reserve-facing request from hardware-owned bytes.
  public func prepareTopUpRequest(
    operationID: Data, amount: KagemushaUInt128V1,
    payer: KagemushaAccountIDV1, recipient: KagemushaAccountIDV1
  ) throws -> KagemushaTopUpRequestV1 {
    let bundle = try prepareMintConstructionBundle(
      operationID: operationID, amount: amount, payer: payer, recipient: recipient)
    return try bundle.topUpRequest(hardwareCredential: qualification().credential)
  }

  public func stageMintCredit(
    authorization: KagemushaMintAuthorizationV1,
    credit: KagemushaMintCreditV1
  ) throws -> KagemushaHardwareStageDispositionV1 {
    try lock.withLock {
      let before = try provider.journalRevision()
      let staged = try provider.verifyAuthorizationAndStageMintCredit(
        canonicalAuthorization: KagemushaNoritoV1.encodeMintAuthorizationShape(authorization),
        canonicalMintCredit: KagemushaNoritoV1.encodeMintCreditShape(
          credit, against: authorization))
      guard staged.creditID == credit.statement.lifecycle.creditID,
        try provider.journalRevision() == before
      else { throw invalid("mint staging binding or journal") }
      journalRevisionValue = before
      return staged.disposition
    }
  }

  /// Fold exactly one authenticated mint or peer selector.
  public func foldPendingCredit(
    selector: KagemushaPendingCreditSelectorV1
  ) throws -> KagemushaReceiveFoldResultV1 {
    try lock.withLock { try foldPendingCreditLocked(selector: selector) }
  }

  /// Drain all pending credits one at a time. There is no count-based rejection.
  public func drainStagedCredits() throws -> KagemushaUInt128V1 {
    let epoch = lock.withLock {
      (aggregateStateValue.hardwareEpochID,
       qualificationValue.credential.hardwareEpochGeneration)
    }
    var count = KagemushaUInt128V1.zero
    var watermark: KagemushaPendingCreditWatermarkV1?
    while true {
      let didFold = try lock.withBackgroundLock { () -> Bool in
        guard aggregateStateValue.hardwareEpochID == epoch.0,
          qualificationValue.credential.hardwareEpochGeneration == epoch.1
        else { throw invalid("hardware epoch changed during inbox drain") }
        let selection = try provider.selectPendingCredit(
          watermark: watermark, target: .drainAll)
        if let expected = watermark, selection.watermark != expected {
          throw invalid("pending-credit watermark changed during drain")
        }
        watermark = selection.watermark
        guard let selector = selection.nextPending else { return false }
        _ = try foldPendingCreditLocked(selector: selector)
        return true
      }
      guard didFold else { return count }
      count = try count.adding(1)
    }
  }

  public func reserveRedemptionOperationID(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1
  ) throws -> Data {
    guard !amount.isZero else { throw invalid("redemption amount") }
    return try provider.reserveRedemptionOperationID(amount: amount, beneficiary: beneficiary)
  }

  public func commitRedemption(
    amount: KagemushaUInt128V1,
    beneficiary: KagemushaAccountIDV1,
    operationID: Data
  ) throws -> KagemushaRedemptionVoucherV1 {
    guard !amount.isZero else { throw invalid("redemption amount") }
    let operationID = try kagemushaDigest(operationID, "operationID")
    return try lock.withLock {
      try foldRequiredCreditsLocked(requiredBalance: amount)
      let result = try provider.prepareProveCommitRedemption(
        operationID: operationID, amount: amount, beneficiary: beneficiary)
      let voucher = try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(
        result.canonicalEnvelope)
      guard voucher.statement.amount == amount, voucher.statement.beneficiary == beneficiary
      else { throw invalid("redemption output binding") }
      try installAuthoritativeState(result.aggregateState)
      return voucher
    }
  }

  public func recoverRedemption(redemptionID: Data) throws -> KagemushaRedemptionVoucherV1? {
    let expected = try kagemushaDigest(redemptionID, "redemptionID")
    guard let bytes = try provider.recoverRedemption(redemptionID: expected) else { return nil }
    let voucher = try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(bytes)
    guard voucher.statement.redemptionID == expected,
      try KagemushaNoritoV1.encodeRedemptionVoucherShape(voucher) == bytes
    else { throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope }
    return voucher
  }

  public func recoverRedemptionByOperationID(
    operationID: Data
  ) throws -> KagemushaRedemptionVoucherV1? {
    let operationID = try kagemushaDigest(operationID, "operationID")
    guard let bytes = try provider.recoverRedemptionByOperationID(operationID: operationID)
    else { return nil }
    let voucher = try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(bytes)
    guard try KagemushaNoritoV1.encodeRedemptionVoucherShape(voucher) == bytes else {
      throw KagemushaWalletErrorV1.conflictingRecoveredEnvelope
    }
    return voucher
  }

  public func rotateHardwareEpoch() throws -> KagemushaAggregateStateCommitmentV1 {
    try lock.withLock {
      let previousState = aggregateStateValue
      let previousQualification = qualificationValue
      guard previousQualification.credential.hardwareEpochGeneration < UInt64.max
      else { throw invalid("hardware epoch generation exhausted") }
      let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(
        provider.rotateHardwareEpoch())
      let qualification = try provider.qualification()
      try Self.requireStateQualification(state, qualification)
      guard sameBalanceIdentity(previousState, state),
        qualification.credential.laneCommitment
          == previousQualification.credential.laneCommitment,
        qualification.credential.hardwareEpochGeneration
          == previousQualification.credential.hardwareEpochGeneration + 1,
        qualification.credential.hardwareEpochID
          != previousQualification.credential.hardwareEpochID,
        state.sequence.isZero, state.stateCommitment != previousState.stateCommitment,
        try provider.journalRevision() == .zero
      else { throw invalid("invalid hardware epoch rotation") }
      qualificationValue = qualification
      aggregateStateValue = state
      journalRevisionValue = .zero
      return state
    }
  }

  private func foldPendingCreditLocked(
    selector: KagemushaPendingCreditSelectorV1
  ) throws -> KagemushaReceiveFoldResultV1 {
    let beforeState = aggregateStateValue
    let beforeRevision = journalRevisionValue
    let folded = try provider.foldPendingCredit(selector: selector)
    guard folded.selector == selector else { throw invalid("pending-fold selector") }
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(folded.aggregateState)
    try Self.requireStateQualification(state, qualificationValue)
    guard sameBalanceIdentity(beforeState, state),
      state.sequence == (try beforeState.sequence.adding(1)),
      state.stateCommitment != beforeState.stateCommitment
    else { throw invalid("receive fold did not install exact successor") }
    let revision = try provider.journalRevision()
    guard revision == (try beforeRevision.adding(1))
    else { throw invalid("receive fold did not advance journal exactly once") }
    aggregateStateValue = state
    journalRevisionValue = revision
    return KagemushaReceiveFoldResultV1(aggregateState: state, selector: selector)
  }

  /// Drain the complete provider-visible mixed mint/peer inbox while the monetary lane is held.
  ///
  /// This loop intentionally has no item ceiling. Physical processing time is the only bound;
  /// accepted value is never rejected because too many credits preceded it.
  private func drainPendingCreditsLocked() throws {
    var watermark: KagemushaPendingCreditWatermarkV1?
    while true {
      let selection = try provider.selectPendingCredit(
        watermark: watermark, target: .drainAll)
      if let expected = watermark, selection.watermark != expected {
        throw invalid("pending-credit watermark changed during drain")
      }
      watermark = selection.watermark
      guard let selector = selection.nextPending else { return }
      _ = try foldPendingCreditLocked(selector: selector)
    }
  }

  private func foldRequiredCreditsLocked(requiredBalance: KagemushaUInt128V1) throws {
    while true {
      let selection = try provider.selectPendingCredit(
        watermark: nil, target: .requiredBalance(requiredBalance))
      guard let selector = selection.nextPending else { return }
      _ = try foldPendingCreditLocked(selector: selector)
    }
  }

  private func installAuthoritativeState(_ bytes: Data) throws {
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
    try Self.requireStateQualification(state, qualificationValue)
    guard sameBalanceIdentity(aggregateStateValue, state),
      state.stateCommitment != aggregateStateValue.stateCommitment
    else { throw invalid("terminal operation did not advance aggregate state") }
    let revision = try provider.journalRevision()
    guard journalRevisionValue.isLessThanOrEqual(to: revision),
      revision != journalRevisionValue
    else { throw invalid("terminal operation did not advance journal") }
    aggregateStateValue = state
    journalRevisionValue = revision
  }

  private static func authoritativeRecoverySnapshot(
    provider: KagemushaHardwareProviderV1, allowBootstrap: Bool
  ) throws -> (
    qualification: KagemushaHardwareQualificationV1,
    recovery: KagemushaHardwareRecoveryV1,
    state: KagemushaAggregateStateCommitmentV1
  ) {
    _ = try provider.qualification()
    var recovery = try provider.recover()
    var qualification = try provider.qualification()
    if recovery.aggregateState == nil {
      guard allowBootstrap else { throw invalid("recovery lost durable state") }
      let bootstrapped = try provider.bootstrapState()
      recovery = try provider.recover()
      qualification = try provider.qualification()
      guard recovery.aggregateState == bootstrapped
      else { throw invalid("bootstrap was not durably recovered") }
    }
    guard let bytes = recovery.aggregateState else { throw invalid("missing aggregate state") }
    let state = try KagemushaNoritoV1.decodeAggregateStateShapeExact(bytes)
    try requireStateQualification(state, qualification)
    guard try provider.journalRevision() == recovery.journalRevision
    else { throw invalid("inconsistent recovered journal") }
    return (qualification, recovery, state)
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
    else { throw invalid("aggregate state qualification") }
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

  private static func invalid(_ message: String) -> KagemushaWalletErrorV1 {
    .invalidHardwareResult(message)
  }

  private func invalid(_ message: String) -> KagemushaWalletErrorV1 {
    Self.invalid(message)
  }
}

/// Private host scheduling only: one monetary transition at a time.
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
    while occupied || (background && foregroundWaiters > 0) { condition.wait() }
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
