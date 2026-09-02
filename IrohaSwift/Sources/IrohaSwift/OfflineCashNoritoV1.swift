import CryptoKit
import Foundation

/// Exact canonical Norito shape codec for Offline Cash V1.
///
/// This layer performs framing, canonical re-encoding, bounds, and explicit public-field
/// consistency checks. It deliberately does not implement proof verification, signature
/// verification, AEAD, KDFs, or monetary authorization; those belong to the authenticated native
/// core and qualified hardware service.
public enum OfflineCashNoritoV1 {
  private static let model = "iroha_data_model::offline::offline_cash_v1::"
  private static let paymentRequestDigestDomain = Data(
    "iroha:offline-cash:v1:payment-request".utf8)
  private static let acceptanceIntentDigestDomain = Data(
    "iroha:offline-cash:v1:acceptance-intent".utf8)
  private static let acceptanceIntentAuthorizationStatementDigestDomain = Data(
    "iroha:offline-cash:v1:acceptance-intent-authorization-statement".utf8)
  private static let acceptanceIntentAuthorizationDigestDomain = Data(
    "iroha:offline-cash:v1:acceptance-intent-authorization".utf8)
  private static let acceptanceTicketDigestDomain = Data(
    "iroha:offline-cash:v1:acceptance-ticket".utf8)
  private static let paymentDigestDomain = Data(
    "iroha:offline-cash:v1:payment".utf8)
  private static let noCommitClosureStatementDigestDomain = Data(
    "iroha:offline-cash:v1:no-commit-closure-statement".utf8)
  private static let noCommitClosureDigestDomain = Data(
    "iroha:offline-cash:v1:no-commit-closure".utf8)
  private static let outboxReservationCommitmentDomain = Data(
    "iroha:offline-cash:v1:outbox-reservation".utf8)
  private static let lifecycleBindingDigestDomain = Data(
    "iroha:offline-cash:v1:lifecycle-binding".utf8)
  private static let sendSplitStatementDigestDomain = Data(
    "iroha:offline-cash:v1:send-split-statement".utf8)
  private static let commitCertificateIDDomain = Data(
    "iroha:offline-cash:v1:commit-certificate-id".utf8)
  private static let commitCertificateDigestDomain = Data(
    "iroha:offline-cash:v1:commit-certificate".utf8)
  private static let redemptionIDDomain = Data(
    "iroha:offline-cash:v1:redemption-id".utf8)
  private static let redemptionStatementDigestDomain = Data(
    "iroha:offline-cash:v1:redemption-statement".utf8)

  public static func encodeAggregateStateShape(
    _ value: OfflineCashAggregateStateCommitmentV1
  ) throws -> Data {
    try bounded(
      frame("OfflineCashAggregateStateCommitmentV1", aggregate(value), 16),
      OfflineCashWireV1.maximumAggregateStateBytes)
  }

  public static func decodeAggregateStateShapeExact(
    _ bytes: Data
  ) throws -> OfflineCashAggregateStateCommitmentV1 {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumAggregateStateBytes,
      "OfflineCashAggregateStateCommitmentV1", 16, decodeAggregate, encodeAggregateStateShape)
  }

  public static func encodePaymentRequestShape(_ value: OfflineCashPaymentRequestV1) throws -> Data
  {
    try bounded(
      frame("OfflineCashPaymentRequestV1", request(value), 16),
      OfflineCashWireV1.maximumPaymentRequestBytes)
  }

  public static func decodePaymentRequestShapeExact(_ bytes: Data) throws
    -> OfflineCashPaymentRequestV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumPaymentRequestBytes,
      "OfflineCashPaymentRequestV1", 16, decodeRequest, encodePaymentRequestShape)
  }

  public static func encodeAcceptanceIntentShape(_ value: OfflineCashAcceptanceIntentV1) throws
    -> Data
  {
    try bounded(
      frame("OfflineCashAcceptanceIntentV1", acceptanceIntent(value), 16),
      OfflineCashWireV1.maximumAcceptanceIntentBytes)
  }

  public static func decodeAcceptanceIntentShapeExact(_ bytes: Data) throws
    -> OfflineCashAcceptanceIntentV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumAcceptanceIntentBytes,
      "OfflineCashAcceptanceIntentV1", 16, decodeAcceptanceIntent,
      encodeAcceptanceIntentShape)
  }

  public static func encodeAcceptanceIntentAuthorizationShape(
    _ value: OfflineCashAcceptanceIntentAuthorizationV1
  ) throws -> Data {
    try bounded(
      frame("OfflineCashAcceptanceIntentAuthorizationV1", authorization(value), 16),
      OfflineCashWireV1.maximumAcceptanceIntentAuthorizationBytes)
  }

  public static func decodeAcceptanceIntentAuthorizationShapeExact(
    _ bytes: Data
  ) throws -> OfflineCashAcceptanceIntentAuthorizationV1 {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumAcceptanceIntentAuthorizationBytes,
      "OfflineCashAcceptanceIntentAuthorizationV1", 16, decodeAuthorization,
      encodeAcceptanceIntentAuthorizationShape)
  }

  /// Encode one fully cross-bound no-commit recovery envelope.
  ///
  /// This validates canonical public fields only. Its paired proof must still be passed to the
  /// authenticated native verifier before the envelope has monetary authority.
  public static func encodeNoCommitClosureShape(
    _ value: OfflineCashNoCommitClosureV1
  ) throws -> Data {
    try validateNoCommitClosurePublicBindings(value)
    return try bounded(
      frame("OfflineCashNoCommitClosureV1", noCommitClosure(value), 16),
      OfflineCashWireV1.maximumNoCommitClosureBytes)
  }

  /// Decode one exact, bounded, fully cross-bound no-commit recovery envelope.
  public static func decodeNoCommitClosureShapeExact(
    _ bytes: Data
  ) throws -> OfflineCashNoCommitClosureV1 {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumNoCommitClosureBytes,
      "OfflineCashNoCommitClosureV1", 16, decodeNoCommitClosure,
      encodeNoCommitClosureShape)
  }

  /// Return the canonical request digest used by Offline Cash V1 public bindings.
  ///
  /// This is a codec digest, not request-signature verification.
  public static func paymentRequestDigest(_ value: OfflineCashPaymentRequestV1) -> Data {
    digestEncoded(
      paymentRequestDigestDomain,
      frame("OfflineCashPaymentRequestV1", request(value), 16))
  }

  /// Return the canonical compact-intent digest after checking its request binding.
  public static func acceptanceIntentDigest(
    _ value: OfflineCashAcceptanceIntentV1, against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try validateAcceptanceIntentPublicBindings(value, requestValue)
    return acceptanceIntentDigestUnchecked(value)
  }

  /// Return the release-bound authorization-statement digest without verifying its proof.
  public static func acceptanceIntentAuthorizationStatementDigest(
    _ value: OfflineCashAcceptanceIntentAuthorizationStatementV1,
    against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try validateAcceptanceIntentPublicBindings(value.intent, requestValue)
    guard value.releaseID == requestValue.releaseID,
      value.suiteID == requestValue.hardwareCredential.suiteID
    else { throw offlineCashInvalid("acceptanceAuthorizationStatement.publicBinding") }
    return acceptanceIntentAuthorizationStatementDigestUnchecked(value)
  }

  /// Return the canonical proof-bearing authorization digest without verifying its proof.
  public static func acceptanceIntentAuthorizationDigest(
    _ value: OfflineCashAcceptanceIntentAuthorizationV1,
    against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try validateAcceptanceIntentAuthorizationPublicBindings(value, requestValue)
    return acceptanceIntentAuthorizationDigestUnchecked(value)
  }

  /// Return the canonical acceptance-ticket digest after checking request and intent bindings.
  public static func acceptanceTicketDigest(
    _ value: OfflineCashAcceptanceTicketV1, against requestValue: OfflineCashPaymentRequestV1,
    intent intentValue: OfflineCashAcceptanceIntentV1
  ) throws -> Data {
    try validateAcceptanceTicketPublicBindings(value, requestValue, intentValue)
    return acceptanceTicketDigestUnchecked(value)
  }

  /// Return the canonical public closure-statement digest constrained by both proof parities.
  public static func noCommitClosureStatementDigest(
    _ value: OfflineCashNoCommitClosureStatementV1
  ) -> Data {
    noCommitClosureStatementDigestUnchecked(value)
  }

  /// Validate and return the canonical identity of a complete no-commit recovery envelope.
  public static func noCommitClosureDigest(
    _ value: OfflineCashNoCommitClosureV1
  ) throws -> Data {
    try validateNoCommitClosurePublicBindings(value)
    return digestEncoded(
      noCommitClosureDigestDomain,
      frame("OfflineCashNoCommitClosureV1", noCommitClosure(value), 16))
  }

  public static func encodeAcceptanceTicketShape(_ value: OfflineCashAcceptanceTicketV1) throws
    -> Data
  {
    try bounded(
      frame("OfflineCashAcceptanceTicketV1", ticket(value), 16),
      OfflineCashWireV1.maximumAcceptanceTicketBytes)
  }

  public static func decodeAcceptanceTicketShapeExact(_ bytes: Data) throws
    -> OfflineCashAcceptanceTicketV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumAcceptanceTicketBytes,
      "OfflineCashAcceptanceTicketV1", 16, decodeTicket, encodeAcceptanceTicketShape)
  }

  public static func paymentRequestDigestShape(
    _ value: OfflineCashPaymentRequestV1
  ) throws -> Data {
    paymentRequestDigest(value)
  }

  public static func acceptanceIntentDigestShape(
    _ value: OfflineCashAcceptanceIntentV1,
    against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try acceptanceIntentDigest(value, against: requestValue)
  }

  public static func acceptanceIntentAuthorizationStatementDigestShape(
    _ value: OfflineCashAcceptanceIntentAuthorizationStatementV1,
    against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try acceptanceIntentAuthorizationStatementDigest(value, against: requestValue)
  }

  public static func acceptanceIntentAuthorizationDigestShape(
    _ value: OfflineCashAcceptanceIntentAuthorizationV1,
    against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try acceptanceIntentAuthorizationDigest(value, against: requestValue)
  }

  public static func acceptanceTicketDigestShape(
    _ value: OfflineCashAcceptanceTicketV1,
    against requestValue: OfflineCashPaymentRequestV1,
    authorization authorizationValue: OfflineCashAcceptanceIntentAuthorizationV1
  ) throws -> Data {
    try validateAcceptanceIntentAuthorizationPublicBindings(authorizationValue, requestValue)
    return try acceptanceTicketDigest(
      value, against: requestValue, intent: authorizationValue.statement.intent)
  }

  public static func noCommitClosureStatementDigestShape(
    _ value: OfflineCashNoCommitClosureStatementV1
  ) -> Data {
    noCommitClosureStatementDigest(value)
  }

  public static func noCommitClosureDigestShape(
    _ value: OfflineCashNoCommitClosureV1
  ) throws -> Data {
    try noCommitClosureDigest(value)
  }

  /// Return the circuit-bound commitment to a validated durable outbox reservation.
  public static func outboxReservationCommitmentShape(
    _ value: OfflineCashOutboxReservationV1
  ) -> Data {
    digestEncoded(outboxReservationCommitmentDomain, outboxReservationCircuitTranscript(value))
  }

  /// Return the canonical lifecycle digest bound into a terminal certificate.
  public static func lifecycleBindingDigestShape(_ value: OfflineCashLifecycleBindingV1) -> Data {
    digestEncoded(
      lifecycleBindingDigestDomain,
      frame("OfflineCashLifecycleBindingV1", lifecycle(value), 8))
  }

  /// Return the canonical send-split statement digest constrained by the wrapper.
  public static func transferStatementDigestShape(_ value: OfflineCashTransferStatementV1) -> Data {
    digestEncoded(
      sendSplitStatementDigestDomain,
      frame("OfflineCashTransferStatementV1", transferStatement(value), 16))
  }

  /// Return the fixed-width terminal-certificate identity expected by the circuit.
  public static func commitCertificateIDShape(_ value: OfflineCashCommitCertificateV1) -> Data {
    digestEncoded(commitCertificateIDDomain, commitCertificateIDCircuitTranscript(value))
  }

  /// Return the fixed-width terminal-certificate digest expected by the circuit.
  public static func commitCertificateDigestShape(_ value: OfflineCashCommitCertificateV1) -> Data {
    digestEncoded(commitCertificateDigestDomain, commitCertificateCircuitTranscript(value))
  }

  /// Return the canonical redemption identity derived from its public statement.
  public static func redemptionIDShape(_ value: OfflineCashRedemptionStatementV1) -> Data {
    let preimage = fields([
      lifecycleBindingDigestShape(value.lifecycle), value.terminalNullifier,
      value.amount.littleEndianBytes, value.beneficiary.canonicalPayload,
      value.redemptionCommitment,
    ])
    return digestEncoded(
      redemptionIDDomain,
      frameExact("iroha.offline-cash.v1.redemption-id-preimage", preimage, 16))
  }

  /// Return the canonical redemption-statement digest constrained by the wrapper.
  public static func redemptionStatementDigestShape(
    _ value: OfflineCashRedemptionStatementV1
  ) -> Data {
    digestEncoded(
      redemptionStatementDigestDomain,
      frame("OfflineCashRedemptionStatementV1", redemptionStatement(value), 16))
  }

  /// Return the canonical payment digest after checking its request binding.
  public static func paymentDigestShape(
    _ value: OfflineCashPaymentV1,
    against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try digestEncoded(paymentDigestDomain, encodePaymentShape(value, against: requestValue))
  }

  public static func encodePaymentShape(
    _ value: OfflineCashPaymentV1, against requestValue: OfflineCashPaymentRequestV1
  ) throws -> Data {
    try validatePaymentPublicBindings(value, requestValue)
    let encryptedCredit = try encodeEncryptedCreditEnvelopeShape(value.encryptedCredit)
    return try bounded(
      frame("OfflineCashPaymentV1", payment(value, encryptedCredit), 16),
      OfflineCashWireV1.maximumPaymentBytes)
  }

  public static func decodePaymentShapeExact(
    _ bytes: Data, against requestValue: OfflineCashPaymentRequestV1
  ) throws -> OfflineCashPaymentV1 {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumPaymentBytes, "OfflineCashPaymentV1", 16,
      decodePayment
    ) { try encodePaymentShape($0, against: requestValue) }
  }

  public static func encodeAcknowledgementShape(
    _ value: OfflineCashAcknowledgementV1, against requestValue: OfflineCashPaymentRequestV1,
    payment paymentValue: OfflineCashPaymentV1
  ) throws -> Data {
    try validateAcknowledgementPublicBindings(value, requestValue, paymentValue)
    return try bounded(
      frame("OfflineCashAcknowledgementV1", acknowledgement(value), 2),
      OfflineCashWireV1.maximumAcknowledgementBytes)
  }

  public static func decodeAcknowledgementShapeExact(
    _ bytes: Data, against requestValue: OfflineCashPaymentRequestV1,
    payment paymentValue: OfflineCashPaymentV1
  ) throws -> OfflineCashAcknowledgementV1 {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumAcknowledgementBytes,
      "OfflineCashAcknowledgementV1", 2, decodeAcknowledgement
    ) {
      try encodeAcknowledgementShape($0, against: requestValue, payment: paymentValue)
    }
  }

  public static func encodeMintAuthorizationShape(
    _ value: OfflineCashMintAuthorizationV1
  ) throws -> Data {
    try bounded(
      frame("OfflineCashMintAuthorizationV1", mintAuthorization(value), 16),
      OfflineCashWireV1.maximumMintAuthorizationBytes)
  }

  public static func decodeMintAuthorizationShapeExact(_ bytes: Data) throws
    -> OfflineCashMintAuthorizationV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumMintAuthorizationBytes,
      "OfflineCashMintAuthorizationV1", 16, decodeMintAuthorization,
      encodeMintAuthorizationShape)
  }

  public static func encodeMintCreditShape(_ value: OfflineCashMintCreditV1) throws -> Data {
    let encryptedCredit = try encodeEncryptedCreditEnvelopeShape(value.encryptedCredit)
    return try bounded(
      frame("OfflineCashMintCreditV1", mintCredit(value, encryptedCredit), 16),
      OfflineCashWireV1.maximumMintCreditBytes)
  }

  public static func decodeMintCreditShapeExact(_ bytes: Data) throws -> OfflineCashMintCreditV1 {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumMintCreditBytes,
      "OfflineCashMintCreditV1", 16, decodeMintCredit, encodeMintCreditShape)
  }

  public static func encodeRedemptionVoucherShape(
    _ value: OfflineCashRedemptionVoucherV1
  ) throws -> Data {
    try validateRedemptionVoucherPublicBindings(value)
    return try bounded(
      frame("OfflineCashRedemptionVoucherV1", redemptionVoucher(value), 16),
      OfflineCashWireV1.maximumRedemptionVoucherBytes)
  }

  public static func decodeRedemptionVoucherShapeExact(_ bytes: Data) throws
    -> OfflineCashRedemptionVoucherV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumRedemptionVoucherBytes,
      "OfflineCashRedemptionVoucherV1", 16, decodeRedemptionVoucher,
      encodeRedemptionVoucherShape)
  }

  public static func encodeCreditOpeningShape(_ value: OfflineCashCreditOpeningV1) throws -> Data {
    try bounded(
      frame("OfflineCashCreditOpeningV1", creditOpening(value), 16),
      OfflineCashWireV1.maximumCreditOpeningBytes)
  }

  public static func decodeCreditOpeningShapeExact(_ bytes: Data) throws
    -> OfflineCashCreditOpeningV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumCreditOpeningBytes,
      "OfflineCashCreditOpeningV1", 16, decodeCreditOpening, encodeCreditOpeningShape)
  }

  public static func encodeEncryptedCreditAADShape(
    _ value: OfflineCashEncryptedCreditAADV1
  ) throws -> Data {
    frame("OfflineCashEncryptedCreditAadV1", encryptedCreditAAD(value), 16)
  }

  public static func decodeEncryptedCreditAADShapeExact(_ bytes: Data) throws
    -> OfflineCashEncryptedCreditAADV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumCreditOpeningBytes,
      "OfflineCashEncryptedCreditAadV1", 16, decodeEncryptedCreditAAD,
      encodeEncryptedCreditAADShape)
  }

  public static func encodeEncryptedCreditEnvelopeShape(
    _ value: OfflineCashEncryptedCreditEnvelopeV1
  ) throws -> Data {
    try bounded(
      frame("OfflineCashEncryptedCreditEnvelopeV1", encryptedCreditEnvelope(value), 8),
      OfflineCashWireV1.maximumEncryptedCreditBytes)
  }

  public static func decodeEncryptedCreditEnvelopeShapeExact(_ bytes: Data) throws
    -> OfflineCashEncryptedCreditEnvelopeV1
  {
    try decodeExact(
      bytes, OfflineCashWireV1.maximumEncryptedCreditBytes,
      "OfflineCashEncryptedCreditEnvelopeV1", 8, decodeEncryptedCreditEnvelope,
      encodeEncryptedCreditEnvelopeShape)
  }

  public static func encodeText<T>(
    _ value: T, kind: OfflineCashWirePayloadKindV1, encoder: (T) throws -> Data
  ) throws -> String {
    try OfflineCashWireV1.encodeText(encoder(value), kind: kind)
  }

  public static func validatePreTicketExchangeShape(
    request: OfflineCashPaymentRequestV1,
    authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ticket: OfflineCashAcceptanceTicketV1
  ) throws -> Int {
    let intent = authorization.statement.intent
    guard authorization.statement.releaseID == request.releaseID,
      authorization.statement.suiteID == request.hardwareCredential.suiteID,
      request.requestMode.accepts(intent.exactAmount), ticket.networkID == request.networkID,
      ticket.requestID == request.requestID, ticket.asset == request.asset,
      ticket.assetIncarnation == request.assetIncarnation, ticket.scale == request.scale,
      ticket.requestMode == request.requestMode, ticket.exactAmount == intent.exactAmount,
      ticket.hardwareProfileID == request.hardwareCredential.hardwareProfileID,
      ticket.policyEpoch == request.hardwareCredential.policyEpoch
    else { throw offlineCashInvalid("preTicketExchange.publicBinding") }
    let sizes = try [
      encodePaymentRequestShape(request),
      encodeAcceptanceIntentAuthorizationShape(authorization), encodeAcceptanceTicketShape(ticket),
    ]
    let total = sizes.reduce(0) { $0 + $1.count }
    guard total <= OfflineCashWireV1.maximumPreTicketExchangeBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: total, maximum: OfflineCashWireV1.maximumPreTicketExchangeBytes)
    }
    return total
  }

  public static func validateCompleteExchangeShape(
    request: OfflineCashPaymentRequestV1,
    authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ticket: OfflineCashAcceptanceTicketV1,
    payment: OfflineCashPaymentV1,
    acknowledgement: OfflineCashAcknowledgementV1
  ) throws -> Int {
    _ = try validatePreTicketExchangeShape(
      request: request, authorization: authorization, ticket: ticket)
    guard payment.acceptanceTicket == ticket,
      payment.acceptanceIntent == authorization.statement.intent
    else { throw offlineCashInvalid("completeExchange.publicBinding") }
    let sizes = try [
      encodePaymentRequestShape(request),
      encodeAcceptanceIntentAuthorizationShape(authorization), encodeAcceptanceTicketShape(ticket),
      encodePaymentShape(payment, against: request),
      encodeAcknowledgementShape(acknowledgement, against: request, payment: payment),
    ]
    let total = sizes.reduce(0) { $0 + $1.count }
    guard total <= OfflineCashWireV1.maximumCompleteExchangeBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: total, maximum: OfflineCashWireV1.maximumCompleteExchangeBytes)
    }
    return total
  }

  // MARK: - Public binding checks

  private static func validateAcceptanceIntentPublicBindings(
    _ value: OfflineCashAcceptanceIntentV1,
    _ requestValue: OfflineCashPaymentRequestV1
  ) throws {
    let requestDigest = try paymentRequestDigestShape(requestValue)
    guard value.requestDigest == requestDigest,
      requestValue.requestMode.accepts(value.exactAmount)
    else { throw offlineCashInvalid("acceptanceIntent.publicBinding") }
  }

  private static func validateAcceptanceIntentAuthorizationPublicBindings(
    _ value: OfflineCashAcceptanceIntentAuthorizationV1,
    _ requestValue: OfflineCashPaymentRequestV1
  ) throws {
    try validateAcceptanceIntentPublicBindings(value.statement.intent, requestValue)
    let semanticDigest = try acceptanceIntentAuthorizationStatementDigestShape(
      value.statement, against: requestValue)
    guard value.statement.releaseID == requestValue.releaseID,
      value.statement.suiteID == requestValue.hardwareCredential.suiteID,
      value.proof.semanticDigest == semanticDigest
    else { throw offlineCashInvalid("acceptanceIntentAuthorization.publicBinding") }
  }

  private static func validateAcceptanceTicketPublicBindings(
    _ value: OfflineCashAcceptanceTicketV1,
    _ requestValue: OfflineCashPaymentRequestV1,
    _ authorizationValue: OfflineCashAcceptanceIntentAuthorizationV1
  ) throws {
    try validateAcceptanceIntentAuthorizationPublicBindings(
      authorizationValue, requestValue)
    try validateAcceptanceTicketPublicBindings(
      value, requestValue, authorizationValue.statement.intent)
  }

  private static func validateAcceptanceTicketPublicBindings(
    _ value: OfflineCashAcceptanceTicketV1,
    _ requestValue: OfflineCashPaymentRequestV1,
    _ intent: OfflineCashAcceptanceIntentV1
  ) throws {
    try validateAcceptanceIntentPublicBindings(intent, requestValue)
    let requestDigest = try paymentRequestDigestShape(requestValue)
    let intentDigest = try acceptanceIntentDigestShape(intent, against: requestValue)
    guard value.networkID == requestValue.networkID,
      value.requestID == requestValue.requestID,
      value.requestDigest == requestDigest,
      value.asset == requestValue.asset,
      value.assetIncarnation == requestValue.assetIncarnation,
      value.scale == requestValue.scale,
      value.requestMode == requestValue.requestMode,
      value.intentDigest == intentDigest,
      value.exactAmount == intent.exactAmount,
      value.hardwareProfileID == requestValue.hardwareCredential.hardwareProfileID,
      value.policyEpoch == requestValue.hardwareCredential.policyEpoch,
      value.issuedAtMS >= requestValue.issuedAtMS,
      value.expiresAtMS <= requestValue.expiresAtMS
    else { throw offlineCashInvalid("acceptanceTicket.publicBinding") }
  }

  private static func validateNoCommitClosurePublicBindings(
    _ value: OfflineCashNoCommitClosureV1
  ) throws {
    let statement = value.statement
    let requestValue = value.request
    let authorizationValue = value.intentAuthorization
    let intent = authorizationValue.statement.intent
    let ticketValue = value.acceptanceTicket
    try validateAcceptanceIntentAuthorizationPublicBindings(
      authorizationValue, requestValue)
    try validateAcceptanceTicketPublicBindings(
      ticketValue, requestValue, authorizationValue)
    let requestDigest = try paymentRequestDigestShape(requestValue)
    let ticketDigest = try acceptanceTicketDigestShape(
      ticketValue, against: requestValue, authorization: authorizationValue)
    let authorizationDigest = try acceptanceIntentAuthorizationDigestShape(
      authorizationValue, against: requestValue)
    let intentDigest = try acceptanceIntentDigestShape(intent, against: requestValue)
    guard statement.requestID == requestValue.requestID,
      statement.requestDigest == requestDigest,
      statement.acceptanceTicketID == ticketValue.acceptanceTicketID,
      statement.ticketDigest == ticketDigest,
      statement.intentAuthorizationDigest == authorizationDigest,
      statement.intentDigest == intentDigest,
      statement.exactAmount == intent.exactAmount,
      statement.exactAmount == ticketValue.exactAmount,
      statement.senderOneTimeCommitment == intent.senderOneTimeCommitment,
      statement.releaseID == authorizationValue.statement.releaseID,
      statement.suiteID == authorizationValue.statement.suiteID,
      statement.vkDigest == authorizationValue.statement.vkDigest,
      statement.artifactManifestDigest == authorizationValue.statement.artifactManifestDigest,
      value.proof.semanticDigest == noCommitClosureStatementDigestShape(statement)
    else { throw offlineCashInvalid("noCommitClosure.publicBinding") }
    let encoded = frame("OfflineCashNoCommitClosureV1", noCommitClosure(value), 16)
    guard encoded.count <= OfflineCashWireV1.maximumNoCommitClosureBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: encoded.count, maximum: OfflineCashWireV1.maximumNoCommitClosureBytes)
    }
  }

  private static func acceptanceIntentDigestUnchecked(
    _ value: OfflineCashAcceptanceIntentV1
  ) -> Data {
    digestEncoded(
      acceptanceIntentDigestDomain,
      acceptanceIntentCircuitTranscript(value))
  }

  private static func acceptanceIntentAuthorizationStatementDigestUnchecked(
    _ value: OfflineCashAcceptanceIntentAuthorizationStatementV1
  ) -> Data {
    digestEncoded(
      acceptanceIntentAuthorizationStatementDigestDomain,
      acceptanceIntentAuthorizationStatementCircuitTranscript(value))
  }

  private static func acceptanceIntentAuthorizationDigestUnchecked(
    _ value: OfflineCashAcceptanceIntentAuthorizationV1
  ) -> Data {
    digestEncoded(
      acceptanceIntentAuthorizationDigestDomain,
      frame("OfflineCashAcceptanceIntentAuthorizationV1", authorization(value), 16))
  }

  private static func acceptanceTicketDigestUnchecked(
    _ value: OfflineCashAcceptanceTicketV1
  ) -> Data {
    digestEncoded(
      acceptanceTicketDigestDomain,
      frame("OfflineCashAcceptanceTicketV1", ticket(value), 16))
  }

  private static func noCommitClosureStatementDigestUnchecked(
    _ value: OfflineCashNoCommitClosureStatementV1
  ) -> Data {
    digestEncoded(
      noCommitClosureStatementDigestDomain,
      noCommitClosureStatementCircuitTranscript(value))
  }

  // Circuit-bound digests hash fixed semantic transcripts, never Norito field framing.
  private static func acceptanceIntentCircuitTranscript(
    _ value: OfflineCashAcceptanceIntentV1
  ) -> Data {
    var bytes = u16(value.version)
    bytes.append(value.requestDigest)
    bytes.append(value.intentID)
    bytes.append(value.exactAmount.littleEndianBytes)
    bytes.append(value.senderOneTimeCommitment)
    return bytes
  }

  private static func acceptanceIntentAuthorizationStatementCircuitTranscript(
    _ value: OfflineCashAcceptanceIntentAuthorizationStatementV1
  ) -> Data {
    var bytes = u16(value.version)
    bytes.append(acceptanceIntentCircuitTranscript(value.intent))
    bytes.append(value.releaseID)
    bytes.append(value.suiteID)
    bytes.append(value.vkDigest)
    bytes.append(value.artifactManifestDigest)
    return bytes
  }

  private static func noCommitClosureStatementCircuitTranscript(
    _ value: OfflineCashNoCommitClosureStatementV1
  ) -> Data {
    var bytes = u16(value.version)
    for digest in [
      value.releaseID, value.suiteID, value.vkDigest, value.artifactManifestDigest,
      value.senderHardwareBindingCommitment, value.requestID, value.requestDigest,
      value.acceptanceTicketID, value.ticketDigest, value.intentAuthorizationDigest,
      value.intentDigest,
    ] {
      bytes.append(digest)
    }
    bytes.append(value.exactAmount.littleEndianBytes)
    for digest in [
      value.senderOneTimeCommitment, value.recoveryID, value.cancellationNullifier,
      value.equivalentDeliverySlotCommitment,
    ] {
      bytes.append(digest)
    }
    return bytes
  }

  private static func outboxReservationCircuitTranscript(
    _ value: OfflineCashOutboxReservationV1
  ) -> Data {
    var bytes = value.reservationID
    bytes.append(u32(value.operationKind.rawValue))
    bytes.append(u32(value.reservedOutboxBytes))
    bytes.append(u64(value.issuedAtMS))
    bytes.append(u64(value.expiresAtMS))
    return bytes
  }

  private static func commitEvidenceCircuitTranscript(
    _ value: OfflineCashCommitEvidenceV1
  ) -> Data {
    var bytes = Data()
    switch value {
    case .trustedTime(let commitment):
      bytes.append(u32(0))
      bytes.append(commitment)
    case .monotonicLease(let commitment):
      bytes.append(u32(1))
      bytes.append(commitment)
    }
    return bytes
  }

  private static func commitCertificateIDCircuitTranscript(
    _ value: OfflineCashCommitCertificateV1
  ) -> Data {
    var bytes = u16(value.version)
    bytes.append(value.candidateEnvelopeDigest)
    bytes.append(value.lifecycleBindingDigest)
    bytes.append(value.transitionNullifier)
    bytes.append(value.outboxReservationCommitment)
    bytes.append(commitEvidenceCircuitTranscript(value.commitEvidence))
    bytes.append(value.hardwareProfileID)
    bytes.append(u64(value.policyEpoch))
    bytes.append(value.hardwareTerminalCommitment)
    return bytes
  }

  private static func commitCertificateCircuitTranscript(
    _ value: OfflineCashCommitCertificateV1
  ) -> Data {
    var bytes = u16(value.version)
    bytes.append(value.certificateID)
    bytes.append(value.candidateEnvelopeDigest)
    bytes.append(value.lifecycleBindingDigest)
    bytes.append(value.transitionNullifier)
    bytes.append(value.outboxReservationCommitment)
    bytes.append(commitEvidenceCircuitTranscript(value.commitEvidence))
    bytes.append(value.hardwareProfileID)
    bytes.append(u64(value.policyEpoch))
    bytes.append(value.hardwareTerminalCommitment)
    return bytes
  }

  private static func validatePaymentPublicBindings(
    _ value: OfflineCashPaymentV1, _ requestValue: OfflineCashPaymentRequestV1
  ) throws {
    let statement = value.statement
    let lifecycle = statement.lifecycle
    let ticket = value.acceptanceTicket
    let intent = value.acceptanceIntent
    try validateAcceptanceIntentPublicBindings(intent, requestValue)
    try validateAcceptanceTicketPublicBindings(ticket, requestValue, intent)
    let requestDigest = try paymentRequestDigestShape(requestValue)
    let ticketDigest = try acceptanceTicketDigest(
      ticket, against: requestValue, intent: intent)
    let lifecycleDigest = lifecycleBindingDigestShape(lifecycle)
    let semanticDigest = transferStatementDigestShape(statement)
    let certificateID = commitCertificateIDShape(value.commitCertificate)
    let certificateDigest = commitCertificateDigestShape(value.commitCertificate)
    guard intent.requestDigest == statement.requestDigest,
      statement.requestDigest == requestDigest,
      statement.acceptanceTicketDigest == ticketDigest,
      intent.exactAmount == statement.amount, requestValue.requestMode.accepts(statement.amount),
      ticket.requestID == requestValue.requestID, ticket.requestDigest == statement.requestDigest,
      ticket.asset == requestValue.asset, ticket.assetIncarnation == requestValue.assetIncarnation,
      ticket.scale == requestValue.scale, ticket.requestMode == requestValue.requestMode,
      ticket.exactAmount == statement.amount,
      ticket.recipientOneTimeKey == statement.recipientOneTimeKey,
      lifecycle.networkID == requestValue.networkID, lifecycle.releaseID == requestValue.releaseID,
      lifecycle.asset == requestValue.asset,
      lifecycle.assetIncarnation == requestValue.assetIncarnation,
      lifecycle.scale == requestValue.scale,
      lifecycle.liabilityPoolID == requestValue.liabilityPoolID,
      lifecycle.requestID == requestValue.requestID,
      lifecycle.acceptanceTicketID == ticket.acceptanceTicketID,
      lifecycle.transitionProfileMatches(ticket),
      value.commitCertificate.lifecycleBindingDigest == lifecycleDigest,
      value.commitCertificate.transitionNullifier == statement.transitionNullifier,
      value.commitCertificate.commitEvidence == statement.commitEvidence,
      value.commitCertificate.hardwareProfileID == lifecycle.hardwareProfileID,
      value.commitCertificate.policyEpoch == lifecycle.policyEpoch,
      value.commitCertificate.certificateID == certificateID,
      value.proof.candidateEnvelopeDigest == value.commitCertificate.candidateEnvelopeDigest,
      value.proof.semanticDigest == semanticDigest,
      value.proof.commitCertificateDigest == certificateDigest
    else { throw offlineCashInvalid("payment.publicBinding") }
  }

  private static func validateCommitCertificatePublicBindings(
    _ certificate: OfflineCashCommitCertificateV1,
    lifecycle: OfflineCashLifecycleBindingV1,
    transitionNullifier: Data,
    evidence: OfflineCashCommitEvidenceV1
  ) throws {
    guard certificate.lifecycleBindingDigest == lifecycleBindingDigestShape(lifecycle),
      certificate.transitionNullifier == transitionNullifier,
      certificate.commitEvidence == evidence,
      certificate.hardwareProfileID == lifecycle.hardwareProfileID,
      certificate.policyEpoch == lifecycle.policyEpoch,
      certificate.certificateID == commitCertificateIDShape(certificate)
    else { throw offlineCashInvalid("commitCertificate.publicBinding") }
  }

  private static func validateCommitWrapperPublicBindings(
    _ proof: OfflineCashCommitWrapperProofV1,
    semanticDigest: Data,
    certificate: OfflineCashCommitCertificateV1
  ) throws {
    guard proof.semanticDigest == semanticDigest,
      proof.candidateEnvelopeDigest == certificate.candidateEnvelopeDigest,
      proof.commitCertificateDigest == commitCertificateDigestShape(certificate)
    else { throw offlineCashInvalid("commitWrapper.publicBinding") }
  }

  private static func validateRedemptionVoucherPublicBindings(
    _ value: OfflineCashRedemptionVoucherV1
  ) throws {
    let statement = value.statement
    let lifecycle = statement.lifecycle
    guard lifecycle.operationKind == .redeemSplit,
      statement.redemptionID == redemptionIDShape(statement)
    else { throw offlineCashInvalid("redemptionVoucher.publicBinding") }
    try validateCommitCertificatePublicBindings(
      value.commitCertificate, lifecycle: lifecycle,
      transitionNullifier: statement.terminalNullifier,
      evidence: statement.commitEvidence)
    try validateCommitWrapperPublicBindings(
      value.proof, semanticDigest: redemptionStatementDigestShape(statement),
      certificate: value.commitCertificate)
  }

  private static func validateAcknowledgementPublicBindings(
    _ value: OfflineCashAcknowledgementV1, _ requestValue: OfflineCashPaymentRequestV1,
    _ paymentValue: OfflineCashPaymentV1
  ) throws {
    try validatePaymentPublicBindings(paymentValue, requestValue)
    guard value.requestDigest == paymentValue.statement.requestDigest,
      value.paymentDigest == (try paymentDigestShape(paymentValue, against: requestValue)),
      value.inboxReceipt.creditID == paymentValue.statement.lifecycle.creditID
    else { throw offlineCashInvalid("acknowledgement.publicBinding") }
  }

  // MARK: - Encoders

  private static func aggregate(_ v: OfflineCashAggregateStateCommitmentV1) -> Data {
    fields([
      u16(v.version), v.releaseID, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.liabilityPoolID, v.laneID,
      v.hardwareEpochID, v.keyReference, v.hardwarePolicyID,
      v.sequence.littleEndianBytes, v.stateCommitment,
    ])
  }

  private static func pairedProof(_ v: OfflineCashPairedProofV1) -> Data {
    fields([
      u16(v.version), v.eqProtocolDigest, v.epProtocolDigest, v.semanticDigest,
      v.guardEqCredentialAudit, v.guardEpCredentialAudit, v.eqDeferredAudit,
      v.epDeferredAudit, vector(v.eqProof), vector(v.epProof), vector(v.eqHistory),
      vector(v.epHistory),
    ])
  }

  private static func hardwareCredential(_ v: OfflineCashHardwareCredentialV1) -> Data {
    fields([
      u16(v.version), v.credentialID, v.networkID, v.hardwareProfileID, v.suiteID,
      v.firmwarePolicyDigest, u64(v.policyEpoch), v.laneCommitment, v.hardwareEpochID,
      u64(v.hardwareEpochGeneration), v.devicePublicKey.sec1Bytes, v.deviceKeyReference,
      u64(v.issuedAtMS), u64(v.expiresAtMS), v.governanceSignature.rawBytes,
    ])
  }

  private static func amountPolicy(_ v: OfflineCashAmountPolicyV1) -> Data {
    fields([v.minimumAmount.littleEndianBytes, v.maximumAmount.littleEndianBytes])
  }

  private static func requestMode(_ v: OfflineCashPaymentRequestModeV1) -> Data {
    var writer = OCWriter()
    switch v {
    case .singleExact(let amount):
      writer.raw(u32(0))
      writer.field(fields([amount.littleEndianBytes]))
    case .partialUntilTotal(let totalAmount):
      writer.raw(u32(1))
      writer.field(fields([totalAmount.littleEndianBytes]))
    case .boundedMultiPayment(let maxPayments, let perPayment):
      writer.raw(u32(2))
      writer.field(fields([u32(maxPayments), amountPolicy(perPayment)]))
    case .openReceive(let perPayment):
      writer.raw(u32(3))
      writer.field(fields([amountPolicy(perPayment)]))
    }
    return writer.data
  }

  private static func acceptanceIntent(_ v: OfflineCashAcceptanceIntentV1) -> Data {
    fields([
      u16(v.version), v.requestDigest, v.intentID, v.exactAmount.littleEndianBytes,
      v.senderOneTimeCommitment,
    ])
  }

  private static func authorizationStatement(
    _ v: OfflineCashAcceptanceIntentAuthorizationStatementV1
  ) -> Data {
    fields([
      u16(v.version), acceptanceIntent(v.intent), v.releaseID, v.suiteID,
      v.vkDigest, v.artifactManifestDigest,
    ])
  }

  private static func authorization(_ v: OfflineCashAcceptanceIntentAuthorizationV1) -> Data {
    fields([u16(v.version), authorizationStatement(v.statement), pairedProof(v.proof)])
  }

  private static func noCommitClosureStatement(
    _ v: OfflineCashNoCommitClosureStatementV1
  ) -> Data {
    fields([
      u16(v.version), v.releaseID, v.suiteID, v.vkDigest,
      v.artifactManifestDigest, v.senderHardwareBindingCommitment,
      v.requestID, v.requestDigest, v.acceptanceTicketID, v.ticketDigest,
      v.intentAuthorizationDigest, v.intentDigest, v.exactAmount.littleEndianBytes,
      v.senderOneTimeCommitment, v.recoveryID, v.cancellationNullifier,
      v.equivalentDeliverySlotCommitment,
    ])
  }

  private static func noCommitClosure(_ v: OfflineCashNoCommitClosureV1) -> Data {
    fields([
      u16(v.version), noCommitClosureStatement(v.statement), request(v.request),
      authorization(v.intentAuthorization), ticket(v.acceptanceTicket), pairedProof(v.proof),
    ])
  }

  private static func ticket(_ v: OfflineCashAcceptanceTicketV1) -> Data {
    fields([
      u16(v.version), v.networkID, v.requestID, v.requestDigest,
      v.acceptanceTicketID, v.asset.canonicalPayload, assetIncarnation(v.assetIncarnation),
      u32(v.scale),
      requestMode(v.requestMode), v.intentDigest, v.exactAmount.littleEndianBytes,
      u32(v.reservedInboxBytes), v.recipientOneTimeKey.rawBytes, v.hardwareProfileID,
      u64(v.policyEpoch), u64(v.issuedAtMS), u64(v.expiresAtMS), v.signature.rawBytes,
    ])
  }

  private static func creditOpening(_ v: OfflineCashCreditOpeningV1) -> Data {
    fields([
      u16(v.version), v.creditID, v.amount.littleEndianBytes,
      v.creditCommitmentOpening, v.recipientBindingOpening, v.recoveryNonce,
    ])
  }

  private static func encryptedCreditAAD(_ v: OfflineCashEncryptedCreditAADV1) -> Data {
    fields([
      u16(v.version), enumUnit(v.purpose.rawValue), v.contextDigest,
      v.issuanceOrTransitionCommitment, v.creditID, v.amount.littleEndianBytes,
    ])
  }

  private static func encryptedCreditEnvelope(_ v: OfflineCashEncryptedCreditEnvelopeV1) -> Data {
    fields([
      u16(v.version), v.ephemeralX25519PublicKey.rawBytes, v.nonce,
      vector(v.ciphertextAndTag),
    ])
  }

  private static func lifecycle(_ v: OfflineCashLifecycleBindingV1) -> Data {
    fields([
      u16(v.version), v.networkID, u16(v.protocolVersion), v.suiteID, v.vkDigest,
      v.releaseID, v.asset.canonicalPayload, assetIncarnation(v.assetIncarnation), u32(v.scale),
      v.liabilityPoolID, v.hardwareProfileID, u64(v.policyEpoch),
      enumUnit(v.operationKind.rawValue), v.requestID, v.acceptanceTicketID,
      v.creditID, v.ciphertextDigest,
    ])
  }

  private static func commitEvidence(_ v: OfflineCashCommitEvidenceV1) -> Data {
    var writer = OCWriter()
    switch v {
    case .trustedTime(let commitment):
      writer.raw(u32(0))
      writer.field(fields([commitment]))
    case .monotonicLease(let commitment):
      writer.raw(u32(1))
      writer.field(fields([commitment]))
    }
    return writer.data
  }

  private static func commitCertificate(_ v: OfflineCashCommitCertificateV1) -> Data {
    fields([
      u16(v.version), v.certificateID, v.candidateEnvelopeDigest,
      v.lifecycleBindingDigest, v.transitionNullifier, v.outboxReservationCommitment,
      commitEvidence(v.commitEvidence), v.hardwareProfileID, u64(v.policyEpoch),
      v.hardwareTerminalCommitment,
    ])
  }

  private static func wrapperProof(_ v: OfflineCashCommitWrapperProofV1) -> Data {
    fields([
      u16(v.version), v.eqProtocolDigest, v.epProtocolDigest, v.semanticDigest,
      v.candidateEnvelopeDigest, v.commitCertificateDigest, v.eqDeferredAudit,
      v.epDeferredAudit, vector(v.eqProof), vector(v.epProof), vector(v.eqHistory),
      vector(v.epHistory),
    ])
  }

  private static func request(_ v: OfflineCashPaymentRequestV1) -> Data {
    fields([
      u16(v.version), v.releaseID, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.liabilityPoolID,
      v.recipient.canonicalPayload, requestMode(v.requestMode),
      hardwareCredential(v.hardwareCredential), v.requestID, u64(v.issuedAtMS),
      u64(v.expiresAtMS), v.signature.rawBytes,
    ])
  }

  private static func transferStatement(_ v: OfflineCashTransferStatementV1) -> Data {
    fields([
      u16(v.version), lifecycle(v.lifecycle), v.amount.littleEndianBytes,
      v.transitionNullifier, v.requestDigest, v.acceptanceTicketDigest,
      v.recipientOneTimeKey.rawBytes, v.ciphertextCommitment,
      commitEvidence(v.commitEvidence),
    ])
  }

  private static func payment(_ v: OfflineCashPaymentV1, _ encryptedCredit: Data) -> Data {
    fields([
      u16(v.version), transferStatement(v.statement),
      acceptanceIntent(v.acceptanceIntent), ticket(v.acceptanceTicket),
      commitCertificate(v.commitCertificate), wrapperProof(v.proof),
      vector(encryptedCredit), v.artifactManifestDigest,
    ])
  }

  private static func inboxReceipt(_ v: OfflineCashInboxReceiptV1) -> Data {
    fields([u16(v.version), v.creditID, v.receiptCommitment])
  }

  private static func acknowledgement(_ v: OfflineCashAcknowledgementV1) -> Data {
    fields([
      u16(v.version), v.requestDigest, v.paymentDigest,
      inboxReceipt(v.inboxReceipt), v.signature.rawBytes,
    ])
  }

  private static func mintAuthorizationContext(
    _ v: OfflineCashMintAuthorizationContextV1
  ) -> Data {
    fields([
      u16(v.version), v.operationID, v.releaseID, v.suiteID, v.vkDigest,
      v.artifactManifestDigest, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.liabilityPoolID,
      v.amount.littleEndianBytes, v.payer.canonicalPayload, v.recipient.canonicalPayload,
      v.hardwareCredentialID, v.hardwareProfileID, u64(v.policyEpoch),
      v.recipientCredentialCommitment, v.creditCommitment,
      v.recipientOneTimeKey.rawBytes,
    ])
  }

  private static func mintAuthorizationStatement(
    _ v: OfflineCashMintAuthorizationStatementV1
  ) -> Data {
    fields([
      u16(v.version), mintAuthorizationContext(v.context),
      v.issuanceCommitment, v.creditID, v.ciphertextDigest,
    ])
  }

  private static func mintAuthorization(_ v: OfflineCashMintAuthorizationV1) -> Data {
    fields([u16(v.version), mintAuthorizationStatement(v.statement), pairedProof(v.proof)])
  }

  private static func mintCreditStatement(_ v: OfflineCashMintCreditStatementV1) -> Data {
    fields([
      u16(v.version), lifecycle(v.lifecycle), v.recipientCredentialCommitment,
      v.authorizationContextDigest, v.mintAuthorizationDigest,
      v.amount.littleEndianBytes, v.issuanceCommitment, v.recipient.canonicalPayload,
      v.creditCommitment, u64(v.mintedAtMS),
    ])
  }

  private static func mintCredit(_ v: OfflineCashMintCreditV1, _ encryptedCredit: Data) -> Data {
    fields([
      u16(v.version), mintCreditStatement(v.statement), pairedProof(v.proof),
      v.finalityCertificateBinding, v.finalityAuthorityHead, v.finalityGenesisRosterID,
      v.finalityProofBindingDigest, vector(encryptedCredit), v.artifactManifestDigest,
    ])
  }

  private static func redemptionStatement(_ v: OfflineCashRedemptionStatementV1) -> Data {
    fields([
      u16(v.version), lifecycle(v.lifecycle), v.amount.littleEndianBytes,
      v.beneficiary.canonicalPayload, v.terminalNullifier, v.redemptionCommitment,
      v.redemptionID, commitEvidence(v.commitEvidence),
    ])
  }

  private static func redemptionVoucher(_ v: OfflineCashRedemptionVoucherV1) -> Data {
    fields([
      u16(v.version), redemptionStatement(v.statement),
      commitCertificate(v.commitCertificate), wrapperProof(v.proof),
      v.artifactManifestDigest,
    ])
  }

  // MARK: - Decoders

  private static func decodeAggregate(_ payload: Data) throws
    -> OfflineCashAggregateStateCommitmentV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashAggregateStateCommitmentV1(
      version: r.u16Field(), releaseID: r.digestField(), networkID: r.exactField(32),
      asset: OfflineCashAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), liabilityPoolID: r.digestField(), laneID: r.digestField(),
      hardwareEpochID: r.digestField(), keyReference: r.digestField(),
      hardwarePolicyID: r.digestField(), sequence: r.u128Field(),
      stateCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodePairedProof(_ payload: Data) throws -> OfflineCashPairedProofV1 {
    var r = OCReader(payload)
    let value = try OfflineCashPairedProofV1(
      version: r.u16Field(), eqProtocolDigest: r.digestField(),
      epProtocolDigest: r.digestField(), semanticDigest: r.digestField(),
      guardEqCredentialAudit: r.digestField(), guardEpCredentialAudit: r.digestField(),
      eqDeferredAudit: r.digestField(), epDeferredAudit: r.digestField(),
      eqProof: r.vectorField(), epProof: r.vectorField(), eqHistory: r.vectorField(),
      epHistory: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodeHardwareCredential(_ payload: Data) throws
    -> OfflineCashHardwareCredentialV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashHardwareCredentialV1(
      version: r.u16Field(), credentialID: r.digestField(), networkID: r.exactField(32),
      hardwareProfileID: r.digestField(), suiteID: r.digestField(),
      firmwarePolicyDigest: r.digestField(), policyEpoch: r.u64Field(),
      laneCommitment: r.digestField(), hardwareEpochID: r.digestField(),
      hardwareEpochGeneration: r.u64Field(),
      devicePublicKey: OfflineCashDevicePublicKeyV1(sec1Bytes: r.exactField(65)),
      deviceKeyReference: r.digestField(), issuedAtMS: r.u64Field(),
      expiresAtMS: r.u64Field(),
      governanceSignature: OfflineCashDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeAmountPolicy(_ payload: Data) throws -> OfflineCashAmountPolicyV1 {
    var r = OCReader(payload)
    let value = try OfflineCashAmountPolicyV1(
      minimumAmount: r.u128Field(), maximumAmount: r.u128Field())
    try r.finish()
    return value
  }

  private static func decodeRequestMode(_ payload: Data) throws
    -> OfflineCashPaymentRequestModeV1
  {
    var r = OCReader(payload)
    let variant = try r.rawU32()
    let body = try r.field()
    try r.finish()
    var bodyReader = OCReader(body)
    let value: OfflineCashPaymentRequestModeV1
    switch variant {
    case 0:
      value = .singleExact(amount: try bodyReader.u128Field())
    case 1:
      value = .partialUntilTotal(totalAmount: try bodyReader.u128Field())
    case 2:
      let maxPayments = try bodyReader.u32Field()
      let policy = try decodeAmountPolicy(bodyReader.field())
      guard maxPayments > 0 else { throw offlineCashInvalid("requestMode.maxPayments") }
      value = .boundedMultiPayment(maxPayments: maxPayments, perPayment: policy)
    case 3:
      value = .openReceive(perPayment: try decodeAmountPolicy(bodyReader.field()))
    default:
      throw offlineCashInvalid("requestMode.variant")
    }
    try bodyReader.finish()
    return value
  }

  private static func decodeAcceptanceIntent(_ payload: Data) throws
    -> OfflineCashAcceptanceIntentV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashAcceptanceIntentV1(
      version: r.u16Field(), requestDigest: r.digestField(), intentID: r.digestField(),
      exactAmount: r.u128Field(), senderOneTimeCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeAuthorizationStatement(_ payload: Data) throws
    -> OfflineCashAcceptanceIntentAuthorizationStatementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashAcceptanceIntentAuthorizationStatementV1(
      version: r.u16Field(), intent: decodeAcceptanceIntent(r.field()),
      releaseID: r.digestField(), suiteID: r.digestField(), vkDigest: r.digestField(),
      artifactManifestDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeAuthorization(_ payload: Data) throws
    -> OfflineCashAcceptanceIntentAuthorizationV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashAcceptanceIntentAuthorizationV1(
      version: r.u16Field(), statement: decodeAuthorizationStatement(r.field()),
      proof: decodePairedProof(r.field()))
    try r.finish()
    return value
  }

  private static func decodeNoCommitClosureStatement(_ payload: Data) throws
    -> OfflineCashNoCommitClosureStatementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashNoCommitClosureStatementV1(
      version: r.u16Field(), releaseID: r.digestField(), suiteID: r.digestField(),
      vkDigest: r.digestField(), artifactManifestDigest: r.digestField(),
      senderHardwareBindingCommitment: r.digestField(), requestID: r.digestField(),
      requestDigest: r.digestField(), acceptanceTicketID: r.digestField(),
      ticketDigest: r.digestField(), intentAuthorizationDigest: r.digestField(),
      intentDigest: r.digestField(), exactAmount: r.u128Field(),
      senderOneTimeCommitment: r.digestField(), recoveryID: r.digestField(),
      cancellationNullifier: r.digestField(),
      equivalentDeliverySlotCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeNoCommitClosure(_ payload: Data) throws
    -> OfflineCashNoCommitClosureV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashNoCommitClosureV1(
      version: r.u16Field(), statement: decodeNoCommitClosureStatement(r.field()),
      request: decodeRequest(r.field()),
      intentAuthorization: decodeAuthorization(r.field()),
      acceptanceTicket: decodeTicket(r.field()), proof: decodePairedProof(r.field()))
    try r.finish()
    return value
  }

  private static func decodeTicket(_ payload: Data) throws -> OfflineCashAcceptanceTicketV1 {
    var r = OCReader(payload)
    let value = try OfflineCashAcceptanceTicketV1(
      version: r.u16Field(), networkID: r.exactField(32), requestID: r.digestField(),
      requestDigest: r.digestField(), acceptanceTicketID: r.digestField(),
      asset: OfflineCashAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), requestMode: decodeRequestMode(r.field()),
      intentDigest: r.digestField(), exactAmount: r.u128Field(),
      reservedInboxBytes: r.u32Field(),
      recipientOneTimeKey: OfflineCashX25519PublicKeyV1(rawBytes: r.exactField(32)),
      hardwareProfileID: r.digestField(), policyEpoch: r.u64Field(),
      issuedAtMS: r.u64Field(), expiresAtMS: r.u64Field(),
      signature: OfflineCashDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeCreditOpening(_ payload: Data) throws -> OfflineCashCreditOpeningV1 {
    var r = OCReader(payload)
    let value = try OfflineCashCreditOpeningV1(
      version: r.u16Field(), creditID: r.digestField(), amount: r.u128Field(),
      creditCommitmentOpening: r.digestField(), recipientBindingOpening: r.digestField(),
      recoveryNonce: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeEncryptedCreditAAD(_ payload: Data) throws
    -> OfflineCashEncryptedCreditAADV1
  {
    var r = OCReader(payload)
    let version = try r.u16Field()
    let purposeRaw = try decodeUnitEnum(r.field())
    guard let purpose = OfflineCashEncryptedCreditPurposeV1(rawValue: purposeRaw) else {
      throw offlineCashInvalid("encryptedCreditPurpose")
    }
    let value = try OfflineCashEncryptedCreditAADV1(
      version: version, purpose: purpose, contextDigest: r.digestField(),
      issuanceOrTransitionCommitment: r.digestField(), creditID: r.digestField(),
      amount: r.u128Field())
    try r.finish()
    return value
  }

  private static func decodeEncryptedCreditEnvelope(_ payload: Data) throws
    -> OfflineCashEncryptedCreditEnvelopeV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashEncryptedCreditEnvelopeV1(
      version: r.u16Field(),
      ephemeralX25519PublicKey: OfflineCashX25519PublicKeyV1(rawBytes: r.exactField(32)),
      nonce: r.exactField(24), ciphertextAndTag: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodeLifecycle(_ payload: Data) throws -> OfflineCashLifecycleBindingV1 {
    var r = OCReader(payload)
    let version = try r.u16Field()
    let networkID = try r.exactField(32)
    let protocolVersion = try r.u16Field()
    let suiteID = try r.digestField()
    let vkDigest = try r.digestField()
    let releaseID = try r.digestField()
    let asset = try OfflineCashAssetDefinitionIDV1(canonicalPayload: r.field())
    let assetIncarnation = try decodeAssetIncarnation(r.field())
    let scale = try r.u32Field()
    let liabilityPoolID = try r.digestField()
    let hardwareProfileID = try r.digestField()
    let policyEpoch = try r.u64Field()
    let operationRaw = try decodeUnitEnum(r.field())
    guard let operation = OfflineCashOperationKindV1(rawValue: operationRaw) else {
      throw offlineCashInvalid("operationKind")
    }
    let value = try OfflineCashLifecycleBindingV1(
      version: version, networkID: networkID, protocolVersion: protocolVersion,
      suiteID: suiteID, vkDigest: vkDigest, releaseID: releaseID, asset: asset,
      assetIncarnation: assetIncarnation, scale: scale, liabilityPoolID: liabilityPoolID,
      hardwareProfileID: hardwareProfileID, policyEpoch: policyEpoch,
      operationKind: operation, requestID: r.exactField(32),
      acceptanceTicketID: r.exactField(32), creditID: r.exactField(32),
      ciphertextDigest: r.exactField(32))
    try r.finish()
    return value
  }

  private static func decodeCommitEvidence(_ payload: Data) throws
    -> OfflineCashCommitEvidenceV1
  {
    var r = OCReader(payload)
    let variant = try r.rawU32()
    var body = OCReader(try r.field())
    let commitment = try body.digestField()
    try body.finish()
    try r.finish()
    switch variant {
    case 0: return .trustedTime(commitment: commitment)
    case 1: return .monotonicLease(commitment: commitment)
    default: throw offlineCashInvalid("commitEvidence.variant")
    }
  }

  private static func decodeCommitCertificate(_ payload: Data) throws
    -> OfflineCashCommitCertificateV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashCommitCertificateV1(
      version: r.u16Field(), certificateID: r.digestField(),
      candidateEnvelopeDigest: r.digestField(), lifecycleBindingDigest: r.digestField(),
      transitionNullifier: r.digestField(), outboxReservationCommitment: r.digestField(),
      commitEvidence: decodeCommitEvidence(r.field()), hardwareProfileID: r.digestField(),
      policyEpoch: r.u64Field(), hardwareTerminalCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeWrapperProof(_ payload: Data) throws
    -> OfflineCashCommitWrapperProofV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashCommitWrapperProofV1(
      version: r.u16Field(), eqProtocolDigest: r.digestField(),
      epProtocolDigest: r.digestField(), semanticDigest: r.digestField(),
      candidateEnvelopeDigest: r.digestField(), commitCertificateDigest: r.digestField(),
      eqDeferredAudit: r.digestField(), epDeferredAudit: r.digestField(),
      eqProof: r.vectorField(), epProof: r.vectorField(), eqHistory: r.vectorField(),
      epHistory: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodeRequest(_ payload: Data) throws -> OfflineCashPaymentRequestV1 {
    var r = OCReader(payload)
    let value = try OfflineCashPaymentRequestV1(
      version: r.u16Field(), releaseID: r.digestField(), networkID: r.exactField(32),
      asset: OfflineCashAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), liabilityPoolID: r.digestField(),
      recipient: OfflineCashAccountIDV1(canonicalPayload: r.field()),
      requestMode: decodeRequestMode(r.field()),
      hardwareCredential: decodeHardwareCredential(r.field()), requestID: r.digestField(),
      issuedAtMS: r.u64Field(), expiresAtMS: r.u64Field(),
      signature: OfflineCashDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeTransferStatement(_ payload: Data) throws
    -> OfflineCashTransferStatementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashTransferStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()), amount: r.u128Field(),
      transitionNullifier: r.digestField(), requestDigest: r.digestField(),
      acceptanceTicketDigest: r.digestField(),
      recipientOneTimeKey: OfflineCashX25519PublicKeyV1(rawBytes: r.exactField(32)),
      ciphertextCommitment: r.digestField(), commitEvidence: decodeCommitEvidence(r.field()))
    try r.finish()
    return value
  }

  private static func decodePayment(_ payload: Data) throws -> OfflineCashPaymentV1 {
    var r = OCReader(payload)
    let value = try OfflineCashPaymentV1(
      version: r.u16Field(), statement: decodeTransferStatement(r.field()),
      acceptanceIntent: decodeAcceptanceIntent(r.field()),
      acceptanceTicket: decodeTicket(r.field()),
      commitCertificate: decodeCommitCertificate(r.field()),
      proof: decodeWrapperProof(r.field()),
      encryptedCredit: decodeEncryptedCreditEnvelopeShapeExact(r.vectorField()),
      artifactManifestDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeInboxReceipt(_ payload: Data) throws -> OfflineCashInboxReceiptV1 {
    var r = OCReader(payload)
    let value = try OfflineCashInboxReceiptV1(
      version: r.u16Field(), creditID: r.digestField(), receiptCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeAcknowledgement(_ payload: Data) throws
    -> OfflineCashAcknowledgementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashAcknowledgementV1(
      version: r.u16Field(), requestDigest: r.digestField(), paymentDigest: r.digestField(),
      inboxReceipt: decodeInboxReceipt(r.field()),
      signature: OfflineCashDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeMintAuthorizationContext(_ payload: Data) throws
    -> OfflineCashMintAuthorizationContextV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashMintAuthorizationContextV1(
      version: r.u16Field(), operationID: r.digestField(), releaseID: r.digestField(),
      suiteID: r.digestField(), vkDigest: r.digestField(),
      artifactManifestDigest: r.digestField(), networkID: r.exactField(32),
      asset: OfflineCashAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), liabilityPoolID: r.digestField(), amount: r.u128Field(),
      payer: OfflineCashAccountIDV1(canonicalPayload: r.field()),
      recipient: OfflineCashAccountIDV1(canonicalPayload: r.field()),
      hardwareCredentialID: r.digestField(), hardwareProfileID: r.digestField(),
      policyEpoch: r.u64Field(), recipientCredentialCommitment: r.digestField(),
      creditCommitment: r.digestField(),
      recipientOneTimeKey: OfflineCashX25519PublicKeyV1(rawBytes: r.exactField(32)))
    try r.finish()
    return value
  }

  private static func decodeMintAuthorizationStatement(_ payload: Data) throws
    -> OfflineCashMintAuthorizationStatementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashMintAuthorizationStatementV1(
      version: r.u16Field(), context: decodeMintAuthorizationContext(r.field()),
      issuanceCommitment: r.digestField(), creditID: r.digestField(),
      ciphertextDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeMintAuthorization(_ payload: Data) throws
    -> OfflineCashMintAuthorizationV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashMintAuthorizationV1(
      version: r.u16Field(), statement: decodeMintAuthorizationStatement(r.field()),
      proof: decodePairedProof(r.field()))
    try r.finish()
    return value
  }

  private static func decodeMintCreditStatement(_ payload: Data) throws
    -> OfflineCashMintCreditStatementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashMintCreditStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()),
      recipientCredentialCommitment: r.digestField(),
      authorizationContextDigest: r.digestField(), mintAuthorizationDigest: r.digestField(),
      amount: r.u128Field(), issuanceCommitment: r.digestField(),
      recipient: OfflineCashAccountIDV1(canonicalPayload: r.field()),
      creditCommitment: r.digestField(), mintedAtMS: r.u64Field())
    try r.finish()
    return value
  }

  private static func decodeMintCredit(_ payload: Data) throws -> OfflineCashMintCreditV1 {
    var r = OCReader(payload)
    let value = try OfflineCashMintCreditV1(
      version: r.u16Field(), statement: decodeMintCreditStatement(r.field()),
      proof: decodePairedProof(r.field()), finalityCertificateBinding: r.digestField(),
      finalityAuthorityHead: r.digestField(), finalityGenesisRosterID: r.digestField(),
      finalityProofBindingDigest: r.digestField(),
      encryptedCredit: decodeEncryptedCreditEnvelopeShapeExact(r.vectorField()),
      artifactManifestDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeRedemptionStatement(_ payload: Data) throws
    -> OfflineCashRedemptionStatementV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashRedemptionStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()), amount: r.u128Field(),
      beneficiary: OfflineCashAccountIDV1(canonicalPayload: r.field()),
      terminalNullifier: r.digestField(), redemptionCommitment: r.digestField(),
      redemptionID: r.digestField(), commitEvidence: decodeCommitEvidence(r.field()))
    try r.finish()
    return value
  }

  private static func decodeRedemptionVoucher(_ payload: Data) throws
    -> OfflineCashRedemptionVoucherV1
  {
    var r = OCReader(payload)
    let value = try OfflineCashRedemptionVoucherV1(
      version: r.u16Field(), statement: decodeRedemptionStatement(r.field()),
      commitCertificate: decodeCommitCertificate(r.field()),
      proof: decodeWrapperProof(r.field()), artifactManifestDigest: r.digestField())
    try r.finish()
    return value
  }

  // MARK: - Canonical framing helpers

  private static func decodeExact<T>(
    _ bytes: Data, _ maximum: Int, _ type: String, _ alignment: Int,
    _ decoder: (Data) throws -> T, _ encoder: (T) throws -> Data
  ) throws -> T {
    guard !bytes.isEmpty, bytes.count <= maximum, let decoded = noritoDecodeFrame(bytes),
      decoded.header.flags == NoritoHeader.compactLen,
      decoded.header.schema == noritoSchemaHash(forTypeName: model + type),
      decoded.paddingLength == noritoHeaderPaddingLength(payloadAlignment: alignment)
    else { throw OfflineCashWireEnvelopeErrorV1.invalidText }
    let value = try decoder(decoded.payload)
    guard try encoder(value) == bytes else {
      throw OfflineCashWireEnvelopeErrorV1.nonCanonicalBase64URL
    }
    return value
  }

  private static func bounded(_ data: Data, _ maximum: Int) throws -> Data {
    guard data.count <= maximum else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(actual: data.count, maximum: maximum)
    }
    return data
  }

  private static func frame(_ type: String, _ payload: Data, _ alignment: Int) -> Data {
    noritoEncode(
      typeName: model + type, payload: payload, flags: NoritoHeader.compactLen,
      payloadAlignment: alignment)
  }

  private static func frameExact(_ type: String, _ payload: Data, _ alignment: Int) -> Data {
    noritoEncode(
      typeName: type, payload: payload, flags: NoritoHeader.compactLen,
      payloadAlignment: alignment)
  }
}

extension OfflineCashLifecycleBindingV1 {
  fileprivate func transitionProfileMatches(_ ticket: OfflineCashAcceptanceTicketV1) -> Bool {
    hardwareProfileID == ticket.hardwareProfileID && policyEpoch == ticket.policyEpoch
  }
}

private func digestEncoded(_ domain: Data, _ canonical: Data) -> Data {
  var preimage = domain
  preimage.append(UInt8(0))
  preimage.append(u64(UInt64(canonical.count)))
  preimage.append(canonical)
  return Data(SHA256.hash(data: preimage))
}

private func assetIncarnation(_ value: OfflineCashAssetIncarnationV1) -> Data {
  fields([value.bytes])
}

private func decodeAssetIncarnation(_ payload: Data) throws
  -> OfflineCashAssetIncarnationV1
{
  var reader = OCReader(payload)
  let value = try OfflineCashAssetIncarnationV1(bytes: reader.exactField(32))
  try reader.finish()
  return value
}

private func fields(_ values: [Data]) -> Data {
  var writer = OCWriter()
  for value in values {
    writer.field(value)
  }
  return writer.data
}

private func enumUnit(_ value: UInt32) -> Data { u32(value) }

private func decodeUnitEnum(_ payload: Data) throws -> UInt32 {
  var reader = OCReader(payload)
  let value = try reader.rawU32()
  try reader.finish()
  return value
}

private func vector(_ value: Data) -> Data {
  var writer = OCWriter()
  writer.raw(u64(UInt64(value.count)))
  writer.raw(value)
  return writer.data
}

private func u16(_ value: UInt16) -> Data {
  var value = value.littleEndian
  return withUnsafeBytes(of: &value) { Data($0) }
}

private func u32(_ value: UInt32) -> Data {
  var value = value.littleEndian
  return withUnsafeBytes(of: &value) { Data($0) }
}

private func u64(_ value: UInt64) -> Data {
  var value = value.littleEndian
  return withUnsafeBytes(of: &value) { Data($0) }
}

private struct OCWriter {
  var data = Data()

  mutating func raw(_ value: Data) { data.append(value) }

  mutating func length(_ value: Int) {
    var value = UInt64(value)
    while value >= 0x80 {
      data.append(UInt8(value & 0x7f) | 0x80)
      value >>= 7
    }
    data.append(UInt8(value))
  }

  mutating func field(_ value: Data) {
    length(value.count)
    raw(value)
  }
}

private struct OCReader {
  let data: Data
  var offset = 0

  init(_ data: Data) { self.data = data }

  mutating func length() throws -> Int {
    var result: UInt64 = 0
    var shift: UInt64 = 0
    let start = offset
    for _ in 0..<10 {
      let byte = try raw(1)[0]
      let chunk = UInt64(byte & 0x7f)
      guard shift < 64, !(shift == 63 && chunk > 1) else {
        throw OfflineCashWireEnvelopeErrorV1.invalidText
      }
      result |= chunk << shift
      if byte & 0x80 == 0 {
        guard
          offset - start == 1
            || result >= UInt64(1) << UInt64(7 * (offset - start - 1)),
          result <= UInt64(data.count - offset)
        else { throw OfflineCashWireEnvelopeErrorV1.invalidText }
        return Int(result)
      }
      shift += 7
    }
    throw OfflineCashWireEnvelopeErrorV1.invalidText
  }

  mutating func raw(_ count: Int) throws -> Data {
    guard count >= 0, offset + count <= data.count else {
      throw OfflineCashWireEnvelopeErrorV1.invalidText
    }
    defer { offset += count }
    return Data(data[(data.startIndex + offset)..<(data.startIndex + offset + count)])
  }

  mutating func field() throws -> Data { try raw(length()) }

  mutating func exactField(_ count: Int) throws -> Data {
    let value = try field()
    guard value.count == count else { throw OfflineCashWireEnvelopeErrorV1.invalidText }
    return value
  }

  mutating func digestField() throws -> Data {
    let value = try exactField(32)
    guard offlineCashIsDigest(value) else { throw OfflineCashWireEnvelopeErrorV1.invalidText }
    return value
  }

  mutating func u16Field() throws -> UInt16 {
    let value = try exactField(2)
    return value.withUnsafeBytes { UInt16(littleEndian: $0.loadUnaligned(as: UInt16.self)) }
  }

  mutating func u32Field() throws -> UInt32 {
    let value = try exactField(4)
    return value.withUnsafeBytes { UInt32(littleEndian: $0.loadUnaligned(as: UInt32.self)) }
  }

  mutating func u64Field() throws -> UInt64 {
    let value = try exactField(8)
    return value.withUnsafeBytes { UInt64(littleEndian: $0.loadUnaligned(as: UInt64.self)) }
  }

  mutating func u128Field() throws -> OfflineCashUInt128V1 {
    try OfflineCashUInt128V1(littleEndianBytes: exactField(16))
  }

  mutating func rawU32() throws -> UInt32 {
    let value = try raw(4)
    return value.withUnsafeBytes { UInt32(littleEndian: $0.loadUnaligned(as: UInt32.self)) }
  }

  mutating func vectorField() throws -> Data {
    var nested = OCReader(try field())
    let count = try nested.raw(8).withUnsafeBytes {
      UInt64(littleEndian: $0.loadUnaligned(as: UInt64.self))
    }
    guard count <= UInt64(nested.data.count - nested.offset) else {
      throw OfflineCashWireEnvelopeErrorV1.invalidText
    }
    let value = try nested.raw(Int(count))
    try nested.finish()
    return value
  }

  func finish() throws {
    guard offset == data.count else { throw OfflineCashWireEnvelopeErrorV1.invalidText }
  }
}
