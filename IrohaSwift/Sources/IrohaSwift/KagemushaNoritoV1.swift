import CryptoKit
import Foundation

/// Exact canonical Norito shape codec for Kagemusha V1.
///
/// This layer performs framing, canonical re-encoding, bounds, and explicit public-field
/// consistency checks. It deliberately does not implement proof verification, signature
/// verification, AEAD, KDFs, or monetary authorization; those belong to the authenticated native
/// core and qualified hardware service.
public enum KagemushaNoritoV1 {
  private static let model = "iroha_data_model::kagemusha::kagemusha_v1::"
  private static let paymentRequestDigestDomain = Data(
    "iroha:kagemusha:v1:payment-request".utf8)
  private static let paymentDigestDomain = Data(
    "iroha:kagemusha:v1:payment".utf8)
  private static let lifecycleBindingDigestDomain = Data(
    "iroha:kagemusha:v1:lifecycle-binding".utf8)
  private static let sendSplitStatementDigestDomain = Data(
    "iroha:kagemusha:v1:send-split-statement".utf8)
  private static let pastaStateCommitmentDomain = Data(
    "iroha:kagemusha:v1:pasta-state-commitment".utf8)
  private static let peerCreditContextDigestDomain = Data(
    "iroha:kagemusha:v1:peer-credit-context".utf8)
  private static let peerCreditLifecycleContextDigestDomain = Data(
    "iroha:kagemusha:v1:peer-credit-lifecycle-context".utf8)
  private static let peerCreditIDDomain = Data("iroha:kagemusha:v1:credit-id".utf8)
  private static let ciphertextDigestDomain = Data("iroha:kagemusha:v1:ciphertext".utf8)
  private static let redemptionIDDomain = Data(
    "iroha:kagemusha:v1:redemption-id".utf8)
  private static let redemptionStatementDigestDomain = Data(
    "iroha:kagemusha:v1:redemption-statement".utf8)

  public static func encodeAggregateStateShape(
    _ value: KagemushaAggregateStateCommitmentV1
  ) throws -> Data {
    try bounded(
      frame("KagemushaAggregateStateCommitmentV1", aggregate(value), 16),
      KagemushaWireV1.maximumAggregateStateBytes)
  }

  public static func decodeAggregateStateShapeExact(
    _ bytes: Data
  ) throws -> KagemushaAggregateStateCommitmentV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumAggregateStateBytes,
      "KagemushaAggregateStateCommitmentV1", 16, decodeAggregate, encodeAggregateStateShape)
  }

  public static func encodePaymentRequestShape(_ value: KagemushaPaymentRequestV1) throws -> Data
  {
    try bounded(
      frame("KagemushaPaymentRequestV1", request(value), 16),
      KagemushaWireV1.maximumPaymentRequestBytes)
  }

  public static func decodePaymentRequestShapeExact(_ bytes: Data) throws
    -> KagemushaPaymentRequestV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumPaymentRequestBytes,
      "KagemushaPaymentRequestV1", 16, decodeRequest, encodePaymentRequestShape)
  }

  /// Return the canonical request digest used by Kagemusha V1 public bindings.
  ///
  /// This is a codec digest, not request-signature verification.
  public static func paymentRequestDigest(_ value: KagemushaPaymentRequestV1) -> Data {
    digestEncoded(
      paymentRequestDigestDomain,
      frame("KagemushaPaymentRequestV1", request(value), 16))
  }

  public static func paymentRequestDigestShape(
    _ value: KagemushaPaymentRequestV1
  ) throws -> Data {
    paymentRequestDigest(value)
  }

  public static func pastaStateCommitment(
    _ value: KagemushaPastaStateCommitmentV1
  ) -> Data {
    var preimage = pastaStateCommitmentDomain
    preimage.append(UInt8(0))
    preimage.append(value.eq)
    preimage.append(value.ep)
    return Data(SHA256.hash(data: preimage))
  }

  public static func encodePeerCreditContextShape(
    _ value: KagemushaPeerCreditContextV1
  ) -> Data {
    frame("KagemushaPeerCreditContextV1", peerCreditContext(value), 1)
  }

  public static func decodePeerCreditContextShapeExact(
    _ bytes: Data
  ) throws -> KagemushaPeerCreditContextV1 {
    try decodeExact(
      bytes, 512, "KagemushaPeerCreditContextV1", 1,
      decodePeerCreditContext, encodePeerCreditContextShape)
  }

  public static func peerCreditContextShape(
    statement: KagemushaTransferStatementV1,
    request requestValue: KagemushaPaymentRequestV1
  ) throws -> KagemushaPeerCreditContextV1 {
    guard statement.requestDigest == paymentRequestDigest(requestValue),
      statement.amount == requestValue.amount,
      statement.recipientLaneID == requestValue.recipientLaneID,
      statement.recipientEncryptionKey == requestValue.recipientEncryptionKey
    else { throw kagemushaInvalid("peerCreditContext.publicBinding") }
    return try KagemushaPeerCreditContextV1(
      requestDigest: statement.requestDigest,
      senderBeforeCommitment: statement.senderBeforeCommitment,
      senderAfterCommitment: statement.senderAfterCommitment,
      recipientLaneID: statement.recipientLaneID,
      recipientEncryptionKey: statement.recipientEncryptionKey,
      committedAtMS: statement.committedAtMS,
      hardwareTransitionCommitment: statement.hardwareTransitionCommitment,
      lifecycleContextDigest: peerLifecycleContextDigest(statement.lifecycle))
  }

  public static func peerCreditContextDigestShape(
    _ value: KagemushaPeerCreditContextV1
  ) -> Data {
    digestEncoded(peerCreditContextDigestDomain, encodePeerCreditContextShape(value))
  }

  public static func encryptedCreditAADForPeerShape(
    statement: KagemushaTransferStatementV1,
    request: KagemushaPaymentRequestV1
  ) throws -> KagemushaEncryptedCreditAADV1 {
    let context = try peerCreditContextShape(statement: statement, request: request)
    return try KagemushaEncryptedCreditAADV1(
      purpose: .peer, contextDigest: peerCreditContextDigestShape(context),
      issuanceOrTransitionCommitment: statement.hardwareTransitionCommitment,
      creditID: statement.lifecycle.creditID, amount: statement.amount)
  }

  public static func ciphertextDigestShape(_ bytes: Data) -> Data {
    digestEncoded(ciphertextDigestDomain, bytes)
  }

  public static func expectedPeerCreditIDShape(
    _ value: KagemushaTransferStatementV1
  ) -> Data {
    let preimage = fields([
      value.transitionNullifier, value.requestDigest,
      pastaState(value.senderBeforeCommitment), pastaState(value.senderAfterCommitment),
      value.recipientLaneID, value.recipientEncryptionKey.rawBytes,
      u64(value.committedAtMS), value.amount.littleEndianBytes,
      value.ciphertextCommitment, value.hardwareTransitionCommitment,
    ])
    return digestEncoded(
      peerCreditIDDomain,
      frameExact("iroha.kagemusha.v1.credit-id-preimage", preimage, 16))
  }

  /// Return the canonical lifecycle digest bound by the recursive proof.
  public static func lifecycleBindingDigestShape(_ value: KagemushaLifecycleBindingV1) -> Data {
    digestEncoded(
      lifecycleBindingDigestDomain,
      frame("KagemushaLifecycleBindingV1", lifecycle(value), 8))
  }

  /// Return the canonical send-split statement digest constrained by the paired proof.
  public static func transferStatementDigestShape(_ value: KagemushaTransferStatementV1) -> Data {
    digestEncoded(
      sendSplitStatementDigestDomain,
      frame("KagemushaTransferStatementV1", transferStatement(value), 16))
  }

  /// Return the canonical redemption identity derived from its public statement.
  public static func redemptionIDShape(_ value: KagemushaRedemptionStatementV1) -> Data {
    let preimage = fields([
      lifecycleBindingDigestShape(value.lifecycle), value.terminalNullifier,
      pastaState(value.senderBeforeCommitment), pastaState(value.senderAfterCommitment),
      u64(value.committedAtMS),
      value.amount.littleEndianBytes, value.beneficiary.canonicalPayload,
      value.redemptionCommitment, value.hardwareTransitionCommitment,
    ])
    return digestEncoded(
      redemptionIDDomain,
      frameExact("iroha.kagemusha.v1.redemption-id-preimage", preimage, 16))
  }

  /// Return the canonical redemption-statement digest constrained by the paired proof.
  public static func redemptionStatementDigestShape(
    _ value: KagemushaRedemptionStatementV1
  ) -> Data {
    digestEncoded(
      redemptionStatementDigestDomain,
      frame("KagemushaRedemptionStatementV1", redemptionStatement(value), 16))
  }

  /// Return the canonical payment digest after checking its request binding.
  public static func paymentDigestShape(
    _ value: KagemushaPaymentV1,
    against requestValue: KagemushaPaymentRequestV1
  ) throws -> Data {
    try digestEncoded(paymentDigestDomain, encodePaymentShape(value, against: requestValue))
  }

  public static func encodePaymentShape(
    _ value: KagemushaPaymentV1, against requestValue: KagemushaPaymentRequestV1
  ) throws -> Data {
    try validatePaymentPublicBindings(value, requestValue)
    let encryptedCredit = try encodeEncryptedCreditEnvelopeShape(value.encryptedCredit)
    return try bounded(
      frame("KagemushaPaymentV1", payment(value, encryptedCredit), 16),
      KagemushaWireV1.maximumPaymentBytes)
  }

  public static func decodePaymentShapeExact(
    _ bytes: Data, against requestValue: KagemushaPaymentRequestV1
  ) throws -> KagemushaPaymentV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumPaymentBytes, "KagemushaPaymentV1", 16,
      decodePayment
    ) { try encodePaymentShape($0, against: requestValue) }
  }

  public static func encodeAcknowledgementShape(
    _ value: KagemushaAcknowledgementV1, against requestValue: KagemushaPaymentRequestV1,
    payment paymentValue: KagemushaPaymentV1
  ) throws -> Data {
    try validateAcknowledgementPublicBindings(value, requestValue, paymentValue)
    return try bounded(
      frame("KagemushaAcknowledgementV1", acknowledgement(value), 2),
      KagemushaWireV1.maximumAcknowledgementBytes)
  }

  public static func decodeAcknowledgementShapeExact(
    _ bytes: Data, against requestValue: KagemushaPaymentRequestV1,
    payment paymentValue: KagemushaPaymentV1
  ) throws -> KagemushaAcknowledgementV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumAcknowledgementBytes,
      "KagemushaAcknowledgementV1", 2, decodeAcknowledgement
    ) {
      try encodeAcknowledgementShape($0, against: requestValue, payment: paymentValue)
    }
  }

  public static func encodeMintAuthorizationShape(
    _ value: KagemushaMintAuthorizationV1
  ) throws -> Data {
    try bounded(
      frame("KagemushaMintAuthorizationV1", mintAuthorization(value), 16),
      KagemushaWireV1.maximumMintAuthorizationBytes)
  }

  public static func decodeMintAuthorizationShapeExact(_ bytes: Data) throws
    -> KagemushaMintAuthorizationV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumMintAuthorizationBytes,
      "KagemushaMintAuthorizationV1", 16, decodeMintAuthorization,
      encodeMintAuthorizationShape)
  }

  public static func encodeMintCreditShape(_ value: KagemushaMintCreditV1) throws -> Data {
    let encryptedCredit = try encodeEncryptedCreditEnvelopeShape(value.encryptedCredit)
    return try bounded(
      frame("KagemushaMintCreditV1", mintCredit(value, encryptedCredit), 16),
      KagemushaWireV1.maximumMintCreditBytes)
  }

  public static func decodeMintCreditShapeExact(_ bytes: Data) throws -> KagemushaMintCreditV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumMintCreditBytes,
      "KagemushaMintCreditV1", 16, decodeMintCredit, encodeMintCreditShape)
  }

  public static func encodeRedemptionVoucherShape(
    _ value: KagemushaRedemptionVoucherV1
  ) throws -> Data {
    try validateRedemptionVoucherPublicBindings(value)
    return try bounded(
      frame("KagemushaRedemptionVoucherV1", redemptionVoucher(value), 16),
      KagemushaWireV1.maximumRedemptionVoucherBytes)
  }

  public static func decodeRedemptionVoucherShapeExact(_ bytes: Data) throws
    -> KagemushaRedemptionVoucherV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumRedemptionVoucherBytes,
      "KagemushaRedemptionVoucherV1", 16, decodeRedemptionVoucher,
      encodeRedemptionVoucherShape)
  }

  public static func encodeCreditOpeningShape(_ value: KagemushaCreditOpeningV1) throws -> Data {
    try bounded(
      frame("KagemushaCreditOpeningV1", creditOpening(value), 16),
      KagemushaWireV1.maximumCreditOpeningBytes)
  }

  public static func decodeCreditOpeningShapeExact(_ bytes: Data) throws
    -> KagemushaCreditOpeningV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumCreditOpeningBytes,
      "KagemushaCreditOpeningV1", 16, decodeCreditOpening, encodeCreditOpeningShape)
  }

  public static func encodeEncryptedCreditAADShape(
    _ value: KagemushaEncryptedCreditAADV1
  ) throws -> Data {
    frame("KagemushaEncryptedCreditAadV1", encryptedCreditAAD(value), 16)
  }

  public static func decodeEncryptedCreditAADShapeExact(_ bytes: Data) throws
    -> KagemushaEncryptedCreditAADV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumCreditOpeningBytes,
      "KagemushaEncryptedCreditAadV1", 16, decodeEncryptedCreditAAD,
      encodeEncryptedCreditAADShape)
  }

  public static func encodeEncryptedCreditEnvelopeShape(
    _ value: KagemushaEncryptedCreditEnvelopeV1
  ) throws -> Data {
    try bounded(
      frame("KagemushaEncryptedCreditEnvelopeV1", encryptedCreditEnvelope(value), 8),
      KagemushaWireV1.maximumEncryptedCreditBytes)
  }

  public static func decodeEncryptedCreditEnvelopeShapeExact(_ bytes: Data) throws
    -> KagemushaEncryptedCreditEnvelopeV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumEncryptedCreditBytes,
      "KagemushaEncryptedCreditEnvelopeV1", 8, decodeEncryptedCreditEnvelope,
      encodeEncryptedCreditEnvelopeShape)
  }

  public static func encodeText<T>(
    _ value: T, kind: KagemushaWirePayloadKindV1, encoder: (T) throws -> Data
  ) throws -> String {
    try KagemushaWireV1.encodeText(encoder(value), kind: kind)
  }

  /// Validate the authoritative request/payment/acknowledgement delivery trio.
  public static func validateTerminalDeliveryShape(
    request: KagemushaPaymentRequestV1, payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1
  ) throws -> Int {
    let sizes = try [
      encodePaymentRequestShape(request), encodePaymentShape(payment, against: request),
      encodeAcknowledgementShape(acknowledgement, against: request, payment: payment),
    ]
    let total = sizes.reduce(0) { $0 + $1.count }
    guard total <= KagemushaWireV1.maximumSessionRawBytes else {
      throw KagemushaWireEnvelopeErrorV1.sizeExceeded(
        actual: total, maximum: KagemushaWireV1.maximumSessionRawBytes)
    }
    return total
  }

  // MARK: - Public binding checks

  private static func validatePaymentPublicBindings(
    _ value: KagemushaPaymentV1, _ requestValue: KagemushaPaymentRequestV1
  ) throws {
    let statement = value.statement
    let lifecycle = statement.lifecycle
    let requestDigest = paymentRequestDigest(requestValue)
    let encryptedCredit = try encodeEncryptedCreditEnvelopeShape(value.encryptedCredit)
    guard lifecycle.operationKind == .sendSplit,
      lifecycle.networkID == requestValue.networkID,
      lifecycle.releaseID == requestValue.releaseID,
      lifecycle.asset == requestValue.asset,
      lifecycle.assetIncarnation == requestValue.assetIncarnation,
      lifecycle.scale == requestValue.scale,
      lifecycle.liabilityPoolID == requestValue.liabilityPoolID,
      lifecycle.requestID == requestValue.requestID,
      statement.requestDigest == requestDigest,
      statement.amount == requestValue.amount,
      statement.recipientLaneID == requestValue.recipientLaneID,
      statement.recipientEncryptionKey == requestValue.recipientEncryptionKey,
      statement.committedAtMS >= requestValue.issuedAtMS,
      statement.committedAtMS < requestValue.expiresAtMS,
      lifecycle.creditID == expectedPeerCreditIDShape(statement),
      lifecycle.ciphertextDigest == ciphertextDigestShape(encryptedCredit),
      value.proof.semanticDigest == transferStatementDigestShape(statement)
    else { throw kagemushaInvalid("payment.publicBinding") }
  }

  private static func validateRedemptionVoucherPublicBindings(
    _ value: KagemushaRedemptionVoucherV1
  ) throws {
    let statement = value.statement
    guard statement.lifecycle.operationKind == .redeemSplit,
      statement.redemptionID == redemptionIDShape(statement),
      value.proof.semanticDigest == redemptionStatementDigestShape(statement)
    else { throw kagemushaInvalid("redemptionVoucher.publicBinding") }
  }

  private static func validateAcknowledgementPublicBindings(
    _ value: KagemushaAcknowledgementV1, _ requestValue: KagemushaPaymentRequestV1,
    _ paymentValue: KagemushaPaymentV1
  ) throws {
    try validatePaymentPublicBindings(paymentValue, requestValue)
    guard value.requestDigest == paymentValue.statement.requestDigest,
      value.paymentDigest == (try paymentDigestShape(paymentValue, against: requestValue)),
      value.inboxReceipt.creditID == paymentValue.statement.lifecycle.creditID
    else { throw kagemushaInvalid("acknowledgement.publicBinding") }
  }

  private static func peerLifecycleContextDigest(
    _ value: KagemushaLifecycleBindingV1
  ) -> Data {
    let preimage = fields([
      u16(value.version), value.networkID, u16(value.protocolVersion), value.suiteID,
      value.vkDigest, value.releaseID, value.asset.canonicalPayload,
      assetIncarnation(value.assetIncarnation), u32(value.scale), value.liabilityPoolID,
      value.hardwareProfileID, u64(value.policyEpoch), enumUnit(value.operationKind.rawValue),
      value.requestID,
    ])
    return digestEncoded(
      peerCreditLifecycleContextDigestDomain,
      frameExact("iroha.kagemusha.v1.peer-credit-lifecycle-context-preimage", preimage, 1))
  }

  // MARK: - Encoders

  private static func aggregate(_ v: KagemushaAggregateStateCommitmentV1) -> Data {
    fields([
      u16(v.version), v.releaseID, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.liabilityPoolID, v.laneID,
      v.hardwareEpochID, v.keyReference, v.hardwarePolicyID,
      v.sequence.littleEndianBytes, v.stateCommitment,
    ])
  }

  private static func pairedProof(_ v: KagemushaPairedProofV1) -> Data {
    fields([
      u16(v.version), v.eqProtocolDigest, v.epProtocolDigest, v.semanticDigest,
      v.guardEqCredentialAudit, v.guardEpCredentialAudit, v.eqDeferredAudit,
      v.epDeferredAudit, vector(v.eqProof), vector(v.epProof), vector(v.eqHistory),
      vector(v.epHistory),
    ])
  }

  private static func pastaState(_ v: KagemushaPastaStateCommitmentV1) -> Data {
    fields([v.eq, v.ep])
  }

  private static func peerCreditContext(_ v: KagemushaPeerCreditContextV1) -> Data {
    fields([
      u16(v.version), v.requestDigest, pastaState(v.senderBeforeCommitment),
      pastaState(v.senderAfterCommitment), v.recipientLaneID,
      v.recipientEncryptionKey.rawBytes, u64(v.committedAtMS),
      v.hardwareTransitionCommitment, v.lifecycleContextDigest,
    ])
  }

  private static func hardwareCredential(_ v: KagemushaHardwareCredentialV1) -> Data {
    fields([
      u16(v.version), v.credentialID, v.networkID, v.hardwareProfileID, v.suiteID,
      v.firmwarePolicyDigest, u64(v.policyEpoch), v.laneCommitment, v.hardwareEpochID,
      u64(v.hardwareEpochGeneration), v.devicePublicKey.sec1Bytes, v.deviceKeyReference,
      u64(v.issuedAtMS), u64(v.expiresAtMS), v.governanceSignature.rawBytes,
    ])
  }

  private static func creditOpening(_ v: KagemushaCreditOpeningV1) -> Data {
    fields([
      u16(v.version), v.creditID, v.amount.littleEndianBytes,
      v.creditCommitmentOpening, v.recipientBindingOpening, v.recoveryNonce,
    ])
  }

  private static func encryptedCreditAAD(_ v: KagemushaEncryptedCreditAADV1) -> Data {
    fields([
      u16(v.version), enumUnit(v.purpose.rawValue), v.contextDigest,
      v.issuanceOrTransitionCommitment, v.creditID, v.amount.littleEndianBytes,
    ])
  }

  private static func encryptedCreditEnvelope(_ v: KagemushaEncryptedCreditEnvelopeV1) -> Data {
    fields([
      u16(v.version), v.ephemeralX25519PublicKey.rawBytes, v.nonce,
      vector(v.ciphertextAndTag),
    ])
  }

  private static func lifecycle(_ v: KagemushaLifecycleBindingV1) -> Data {
    fields([
      u16(v.version), v.networkID, u16(v.protocolVersion), v.suiteID, v.vkDigest,
      v.releaseID, v.asset.canonicalPayload, assetIncarnation(v.assetIncarnation), u32(v.scale),
      v.liabilityPoolID, v.hardwareProfileID, u64(v.policyEpoch),
      enumUnit(v.operationKind.rawValue), v.requestID, v.creditID, v.ciphertextDigest,
    ])
  }

  private static func request(_ v: KagemushaPaymentRequestV1) -> Data {
    fields([
      u16(v.version), v.releaseID, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.liabilityPoolID,
      v.recipient.canonicalPayload, v.recipientLaneID, v.recipientEncryptionKey.rawBytes,
      v.amount.littleEndianBytes,
      hardwareCredential(v.hardwareCredential), v.requestID, u64(v.issuedAtMS),
      u64(v.expiresAtMS), v.signature.rawBytes,
    ])
  }

  private static func transferStatement(_ v: KagemushaTransferStatementV1) -> Data {
    fields([
      u16(v.version), lifecycle(v.lifecycle), v.amount.littleEndianBytes,
      v.transitionNullifier, v.requestDigest, pastaState(v.senderBeforeCommitment),
      pastaState(v.senderAfterCommitment), v.recipientLaneID,
      v.recipientEncryptionKey.rawBytes, u64(v.committedAtMS),
      v.ciphertextCommitment, v.hardwareTransitionCommitment,
    ])
  }

  private static func payment(_ v: KagemushaPaymentV1, _ encryptedCredit: Data) -> Data {
    fields([
      u16(v.version), transferStatement(v.statement), pairedProof(v.proof),
      vector(encryptedCredit),
    ])
  }

  private static func inboxReceipt(_ v: KagemushaInboxReceiptV1) -> Data {
    fields([u16(v.version), v.creditID, v.receiptCommitment])
  }

  private static func acknowledgement(_ v: KagemushaAcknowledgementV1) -> Data {
    fields([
      u16(v.version), v.requestDigest, v.paymentDigest,
      inboxReceipt(v.inboxReceipt), v.signature.rawBytes,
    ])
  }

  private static func mintAuthorizationContext(
    _ v: KagemushaMintAuthorizationContextV1
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
    _ v: KagemushaMintAuthorizationStatementV1
  ) -> Data {
    fields([
      u16(v.version), mintAuthorizationContext(v.context),
      v.issuanceCommitment, v.creditID, v.ciphertextDigest,
    ])
  }

  private static func mintAuthorization(_ v: KagemushaMintAuthorizationV1) -> Data {
    fields([u16(v.version), mintAuthorizationStatement(v.statement), pairedProof(v.proof)])
  }

  private static func mintCreditStatement(_ v: KagemushaMintCreditStatementV1) -> Data {
    fields([
      u16(v.version), lifecycle(v.lifecycle), v.recipientCredentialCommitment,
      v.authorizationContextDigest, v.mintAuthorizationDigest,
      v.amount.littleEndianBytes, v.issuanceCommitment, v.recipient.canonicalPayload,
      v.creditCommitment, u64(v.mintedAtMS),
    ])
  }

  private static func mintCredit(_ v: KagemushaMintCreditV1, _ encryptedCredit: Data) -> Data {
    fields([
      u16(v.version), mintCreditStatement(v.statement), pairedProof(v.proof),
      v.finalityCertificateBinding, v.finalityAuthorityHead, v.finalityGenesisRosterID,
      v.finalityProofBindingDigest, vector(encryptedCredit), v.artifactManifestDigest,
    ])
  }

  private static func redemptionStatement(_ v: KagemushaRedemptionStatementV1) -> Data {
    fields([
      u16(v.version), lifecycle(v.lifecycle), v.amount.littleEndianBytes,
      v.beneficiary.canonicalPayload, v.terminalNullifier,
      pastaState(v.senderBeforeCommitment), pastaState(v.senderAfterCommitment),
      u64(v.committedAtMS), v.redemptionCommitment, v.redemptionID,
      v.hardwareTransitionCommitment,
    ])
  }

  private static func redemptionVoucher(_ v: KagemushaRedemptionVoucherV1) -> Data {
    fields([
      u16(v.version), redemptionStatement(v.statement), pairedProof(v.proof),
    ])
  }

  // MARK: - Decoders

  private static func decodeAggregate(_ payload: Data) throws
    -> KagemushaAggregateStateCommitmentV1
  {
    var r = OCReader(payload)
    let value = try KagemushaAggregateStateCommitmentV1(
      version: r.u16Field(), releaseID: r.digestField(), networkID: r.exactField(32),
      asset: KagemushaAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), liabilityPoolID: r.digestField(), laneID: r.digestField(),
      hardwareEpochID: r.digestField(), keyReference: r.digestField(),
      hardwarePolicyID: r.digestField(), sequence: r.u128Field(),
      stateCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodePairedProof(_ payload: Data) throws -> KagemushaPairedProofV1 {
    var r = OCReader(payload)
    let value = try KagemushaPairedProofV1(
      version: r.u16Field(), eqProtocolDigest: r.digestField(),
      epProtocolDigest: r.digestField(), semanticDigest: r.digestField(),
      guardEqCredentialAudit: r.digestField(), guardEpCredentialAudit: r.digestField(),
      eqDeferredAudit: r.digestField(), epDeferredAudit: r.digestField(),
      eqProof: r.vectorField(), epProof: r.vectorField(), eqHistory: r.vectorField(),
      epHistory: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodePastaState(
    _ payload: Data
  ) throws -> KagemushaPastaStateCommitmentV1 {
    var r = OCReader(payload)
    let value = try KagemushaPastaStateCommitmentV1(
      eq: r.exactField(32), ep: r.exactField(32))
    try r.finish()
    return value
  }

  private static func decodePeerCreditContext(
    _ payload: Data
  ) throws -> KagemushaPeerCreditContextV1 {
    var r = OCReader(payload)
    let value = try KagemushaPeerCreditContextV1(
      version: r.u16Field(), requestDigest: r.digestField(),
      senderBeforeCommitment: decodePastaState(r.field()),
      senderAfterCommitment: decodePastaState(r.field()),
      recipientLaneID: r.digestField(),
      recipientEncryptionKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)),
      committedAtMS: r.u64Field(), hardwareTransitionCommitment: r.digestField(),
      lifecycleContextDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeHardwareCredential(_ payload: Data) throws
    -> KagemushaHardwareCredentialV1
  {
    var r = OCReader(payload)
    let value = try KagemushaHardwareCredentialV1(
      version: r.u16Field(), credentialID: r.digestField(), networkID: r.exactField(32),
      hardwareProfileID: r.digestField(), suiteID: r.digestField(),
      firmwarePolicyDigest: r.digestField(), policyEpoch: r.u64Field(),
      laneCommitment: r.digestField(), hardwareEpochID: r.digestField(),
      hardwareEpochGeneration: r.u64Field(),
      devicePublicKey: KagemushaDevicePublicKeyV1(sec1Bytes: r.exactField(65)),
      deviceKeyReference: r.digestField(), issuedAtMS: r.u64Field(),
      expiresAtMS: r.u64Field(),
      governanceSignature: KagemushaDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeCreditOpening(_ payload: Data) throws -> KagemushaCreditOpeningV1 {
    var r = OCReader(payload)
    let value = try KagemushaCreditOpeningV1(
      version: r.u16Field(), creditID: r.digestField(), amount: r.u128Field(),
      creditCommitmentOpening: r.digestField(), recipientBindingOpening: r.digestField(),
      recoveryNonce: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeEncryptedCreditAAD(_ payload: Data) throws
    -> KagemushaEncryptedCreditAADV1
  {
    var r = OCReader(payload)
    let version = try r.u16Field()
    let purposeRaw = try decodeUnitEnum(r.field())
    guard let purpose = KagemushaEncryptedCreditPurposeV1(rawValue: purposeRaw) else {
      throw kagemushaInvalid("encryptedCreditPurpose")
    }
    let value = try KagemushaEncryptedCreditAADV1(
      version: version, purpose: purpose, contextDigest: r.digestField(),
      issuanceOrTransitionCommitment: r.digestField(), creditID: r.digestField(),
      amount: r.u128Field())
    try r.finish()
    return value
  }

  private static func decodeEncryptedCreditEnvelope(_ payload: Data) throws
    -> KagemushaEncryptedCreditEnvelopeV1
  {
    var r = OCReader(payload)
    let value = try KagemushaEncryptedCreditEnvelopeV1(
      version: r.u16Field(),
      ephemeralX25519PublicKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)),
      nonce: r.exactField(24), ciphertextAndTag: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodeLifecycle(_ payload: Data) throws -> KagemushaLifecycleBindingV1 {
    var r = OCReader(payload)
    let version = try r.u16Field()
    let networkID = try r.exactField(32)
    let protocolVersion = try r.u16Field()
    let suiteID = try r.digestField()
    let vkDigest = try r.digestField()
    let releaseID = try r.digestField()
    let asset = try KagemushaAssetDefinitionIDV1(canonicalPayload: r.field())
    let assetIncarnation = try decodeAssetIncarnation(r.field())
    let scale = try r.u32Field()
    let liabilityPoolID = try r.digestField()
    let hardwareProfileID = try r.digestField()
    let policyEpoch = try r.u64Field()
    let operationRaw = try decodeUnitEnum(r.field())
    guard let operation = KagemushaOperationKindV1(rawValue: operationRaw) else {
      throw kagemushaInvalid("operationKind")
    }
    let value = try KagemushaLifecycleBindingV1(
      version: version, networkID: networkID, protocolVersion: protocolVersion,
      suiteID: suiteID, vkDigest: vkDigest, releaseID: releaseID, asset: asset,
      assetIncarnation: assetIncarnation, scale: scale, liabilityPoolID: liabilityPoolID,
      hardwareProfileID: hardwareProfileID, policyEpoch: policyEpoch,
      operationKind: operation, requestID: r.exactField(32),
      creditID: r.exactField(32), ciphertextDigest: r.exactField(32))
    try r.finish()
    return value
  }

  private static func decodeRequest(_ payload: Data) throws -> KagemushaPaymentRequestV1 {
    var r = OCReader(payload)
    let value = try KagemushaPaymentRequestV1(
      version: r.u16Field(), releaseID: r.digestField(), networkID: r.exactField(32),
      asset: KagemushaAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), liabilityPoolID: r.digestField(),
      recipient: KagemushaAccountIDV1(canonicalPayload: r.field()),
      recipientLaneID: r.digestField(),
      recipientEncryptionKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)),
      amount: r.u128Field(),
      hardwareCredential: decodeHardwareCredential(r.field()), requestID: r.digestField(),
      issuedAtMS: r.u64Field(), expiresAtMS: r.u64Field(),
      signature: KagemushaDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeTransferStatement(_ payload: Data) throws
    -> KagemushaTransferStatementV1
  {
    var r = OCReader(payload)
    let value = try KagemushaTransferStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()), amount: r.u128Field(),
      transitionNullifier: r.digestField(), requestDigest: r.digestField(),
      senderBeforeCommitment: decodePastaState(r.field()),
      senderAfterCommitment: decodePastaState(r.field()), recipientLaneID: r.digestField(),
      recipientEncryptionKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)),
      committedAtMS: r.u64Field(), ciphertextCommitment: r.digestField(),
      hardwareTransitionCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodePayment(_ payload: Data) throws -> KagemushaPaymentV1 {
    var r = OCReader(payload)
    let value = try KagemushaPaymentV1(
      version: r.u16Field(), statement: decodeTransferStatement(r.field()),
      proof: decodePairedProof(r.field()),
      encryptedCredit: decodeEncryptedCreditEnvelopeShapeExact(r.vectorField()))
    try r.finish()
    return value
  }

  private static func decodeInboxReceipt(_ payload: Data) throws -> KagemushaInboxReceiptV1 {
    var r = OCReader(payload)
    let value = try KagemushaInboxReceiptV1(
      version: r.u16Field(), creditID: r.digestField(), receiptCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeAcknowledgement(_ payload: Data) throws
    -> KagemushaAcknowledgementV1
  {
    var r = OCReader(payload)
    let value = try KagemushaAcknowledgementV1(
      version: r.u16Field(), requestDigest: r.digestField(), paymentDigest: r.digestField(),
      inboxReceipt: decodeInboxReceipt(r.field()),
      signature: KagemushaDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodeMintAuthorizationContext(_ payload: Data) throws
    -> KagemushaMintAuthorizationContextV1
  {
    var r = OCReader(payload)
    let value = try KagemushaMintAuthorizationContextV1(
      version: r.u16Field(), operationID: r.digestField(), releaseID: r.digestField(),
      suiteID: r.digestField(), vkDigest: r.digestField(),
      artifactManifestDigest: r.digestField(), networkID: r.exactField(32),
      asset: KagemushaAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()),
      scale: r.u32Field(), liabilityPoolID: r.digestField(), amount: r.u128Field(),
      payer: KagemushaAccountIDV1(canonicalPayload: r.field()),
      recipient: KagemushaAccountIDV1(canonicalPayload: r.field()),
      hardwareCredentialID: r.digestField(), hardwareProfileID: r.digestField(),
      policyEpoch: r.u64Field(), recipientCredentialCommitment: r.digestField(),
      creditCommitment: r.digestField(),
      recipientOneTimeKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)))
    try r.finish()
    return value
  }

  private static func decodeMintAuthorizationStatement(_ payload: Data) throws
    -> KagemushaMintAuthorizationStatementV1
  {
    var r = OCReader(payload)
    let value = try KagemushaMintAuthorizationStatementV1(
      version: r.u16Field(), context: decodeMintAuthorizationContext(r.field()),
      issuanceCommitment: r.digestField(), creditID: r.digestField(),
      ciphertextDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeMintAuthorization(_ payload: Data) throws
    -> KagemushaMintAuthorizationV1
  {
    var r = OCReader(payload)
    let value = try KagemushaMintAuthorizationV1(
      version: r.u16Field(), statement: decodeMintAuthorizationStatement(r.field()),
      proof: decodePairedProof(r.field()))
    try r.finish()
    return value
  }

  private static func decodeMintCreditStatement(_ payload: Data) throws
    -> KagemushaMintCreditStatementV1
  {
    var r = OCReader(payload)
    let value = try KagemushaMintCreditStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()),
      recipientCredentialCommitment: r.digestField(),
      authorizationContextDigest: r.digestField(), mintAuthorizationDigest: r.digestField(),
      amount: r.u128Field(), issuanceCommitment: r.digestField(),
      recipient: KagemushaAccountIDV1(canonicalPayload: r.field()),
      creditCommitment: r.digestField(), mintedAtMS: r.u64Field())
    try r.finish()
    return value
  }

  private static func decodeMintCredit(_ payload: Data) throws -> KagemushaMintCreditV1 {
    var r = OCReader(payload)
    let value = try KagemushaMintCreditV1(
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
    -> KagemushaRedemptionStatementV1
  {
    var r = OCReader(payload)
    let value = try KagemushaRedemptionStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()), amount: r.u128Field(),
      beneficiary: KagemushaAccountIDV1(canonicalPayload: r.field()),
      terminalNullifier: r.digestField(), senderBeforeCommitment: decodePastaState(r.field()),
      senderAfterCommitment: decodePastaState(r.field()), committedAtMS: r.u64Field(),
      redemptionCommitment: r.digestField(), redemptionID: r.digestField(),
      hardwareTransitionCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeRedemptionVoucher(_ payload: Data) throws
    -> KagemushaRedemptionVoucherV1
  {
    var r = OCReader(payload)
    let value = try KagemushaRedemptionVoucherV1(
      version: r.u16Field(), statement: decodeRedemptionStatement(r.field()),
      proof: decodePairedProof(r.field()))
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
    else { throw KagemushaWireEnvelopeErrorV1.invalidText }
    let value = try decoder(decoded.payload)
    guard try encoder(value) == bytes else {
      throw KagemushaWireEnvelopeErrorV1.nonCanonicalBase64URL
    }
    return value
  }

  private static func bounded(_ data: Data, _ maximum: Int) throws -> Data {
    guard data.count <= maximum else {
      throw KagemushaWireEnvelopeErrorV1.sizeExceeded(actual: data.count, maximum: maximum)
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

private func digestEncoded(_ domain: Data, _ canonical: Data) -> Data {
  var preimage = domain
  preimage.append(UInt8(0))
  preimage.append(u64(UInt64(canonical.count)))
  preimage.append(canonical)
  return Data(SHA256.hash(data: preimage))
}

private func assetIncarnation(_ value: KagemushaAssetIncarnationV1) -> Data {
  fields([value.bytes])
}

private func decodeAssetIncarnation(_ payload: Data) throws
  -> KagemushaAssetIncarnationV1
{
  var reader = OCReader(payload)
  let value = try KagemushaAssetIncarnationV1(bytes: reader.exactField(32))
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
        throw KagemushaWireEnvelopeErrorV1.invalidText
      }
      result |= chunk << shift
      if byte & 0x80 == 0 {
        guard
          offset - start == 1
            || result >= UInt64(1) << UInt64(7 * (offset - start - 1)),
          result <= UInt64(data.count - offset)
        else { throw KagemushaWireEnvelopeErrorV1.invalidText }
        return Int(result)
      }
      shift += 7
    }
    throw KagemushaWireEnvelopeErrorV1.invalidText
  }

  mutating func raw(_ count: Int) throws -> Data {
    guard count >= 0, offset + count <= data.count else {
      throw KagemushaWireEnvelopeErrorV1.invalidText
    }
    defer { offset += count }
    return Data(data[(data.startIndex + offset)..<(data.startIndex + offset + count)])
  }

  mutating func field() throws -> Data { try raw(length()) }

  mutating func exactField(_ count: Int) throws -> Data {
    let value = try field()
    guard value.count == count else { throw KagemushaWireEnvelopeErrorV1.invalidText }
    return value
  }

  mutating func digestField() throws -> Data {
    let value = try exactField(32)
    guard kagemushaIsDigest(value) else { throw KagemushaWireEnvelopeErrorV1.invalidText }
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

  mutating func u128Field() throws -> KagemushaUInt128V1 {
    try KagemushaUInt128V1(littleEndianBytes: exactField(16))
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
      throw KagemushaWireEnvelopeErrorV1.invalidText
    }
    let value = try nested.raw(Int(count))
    try nested.finish()
    return value
  }

  func finish() throws {
    guard offset == data.count else { throw KagemushaWireEnvelopeErrorV1.invalidText }
  }
}
