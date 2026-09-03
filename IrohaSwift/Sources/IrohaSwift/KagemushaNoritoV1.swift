import CryptoKit
import Foundation

/// Exact canonical Norito shape codec for KAGEMUSHA V1.
///
/// This layer performs framing, canonical re-encoding, bounds, and explicit public-field
/// consistency checks. It deliberately does not implement proof verification, signature
/// verification, AEAD, KDFs, or monetary authorization; those belong to the authenticated native
/// core and qualified hardware service.
public enum KagemushaNoritoV1 {
  private static let model = "iroha_data_model::kagemusha::kagemusha_v1::"
  private static let deviceModel = "iroha_data_model::kagemusha::kagemusha_device_v1::"
  private static let deviceMintStageCommandSchema =
    deviceModel + "KagemushaDeviceMintStageCommandV1"
  private static let deviceMintStageResultSchema =
    deviceModel + "KagemushaDeviceMintStageResultV1"
  private static let paymentRequestDigestDomain = Data(
    "iroha:kagemusha:v1:payment-request".utf8)
  private static let paymentRequestSigningDomain = Data(
    "iroha:kagemusha:v1:payment-request-signing".utf8)
  private static let assetIdentityDigestDomain = Data(
    "iroha:kagemusha:v1:asset-identity".utf8)
  private static let accountIdentityDigestDomain = Data(
    "iroha:kagemusha:v1:account-identity".utf8)
  private static let preparedTransferDigestDomain = Data(
    "iroha:kagemusha:v1:prepared-transfer".utf8)
  private static let paymentDigestDomain = Data(
    "iroha:kagemusha:v1:payment".utf8)
  private static let paymentBodyDigestDomain = Data("iroha:kagemusha:v1:payment-body".utf8)
  private static let lifecycleBindingDigestDomain = Data(
    "iroha:kagemusha:v1:lifecycle-binding".utf8)
  private static let sendSplitStatementDigestDomain = Data(
    "iroha:kagemusha:v1:send-split-statement".utf8)
  private static let pastaStateCommitmentDomain = Data(
    "iroha:kagemusha:v1:pasta-state-commitment".utf8)
  private static let peerCreditContextDigestDomain = Data(
    "iroha:kagemusha:v1:peer-credit-context".utf8)
  private static let peerCreditIDDomain = Data("iroha:kagemusha:v1:credit-id".utf8)
  private static let peerCreditOpeningCommitmentDomain = Data(
    "iroha:kagemusha:v1:peer-credit-opening-commitment".utf8)
  private static let ciphertextDigestDomain = Data("iroha:kagemusha:v1:ciphertext".utf8)
  private static let commitCertificateIDDomain = Data(
    "iroha:kagemusha:v1:commit-certificate-id".utf8)
  private static let commitCertificateDigestDomain = Data(
    "iroha:kagemusha:v1:commit-certificate".utf8)
  private static let hardwareTerminalBodyCommitmentDomain = Data(
    "iroha:kagemusha:v1:hardware-terminal-body".utf8)
  private static let outboxReservationCommitmentDomain = Data(
    "iroha:kagemusha:v1:outbox-reservation".utf8)
  private static let receiveFoldBatchDomain = Data(
    "iroha:kagemusha:v1:receive-fold-batch\0".utf8)
  private static let liabilityPoolDomain = Data(
    "iroha:kagemusha:v1:liability-pool".utf8)
  private static let deviceKeyReferenceDomain = Data(
    "iroha:kagemusha:v1:device-key-reference".utf8)
  private static let mintAuthorizationContextDigestDomain = Data(
    "iroha:kagemusha:v1:mint-authorization-context".utf8)
  private static let mintAuthorizationStatementDigestDomain = Data(
    "iroha:kagemusha:v1:mint-authorization-statement".utf8)
  private static let mintAuthorizationDigestDomain = Data(
    "iroha:kagemusha:v1:mint-authorization".utf8)
  private static let mintLifecycleContextDigestDomain = Data(
    "iroha:kagemusha:v1:mint-lifecycle-context".utf8)
  private static let mintCreditIDDomain = Data("iroha:kagemusha:v1:mint-credit-id".utf8)
  private static let mintStatementDigestDomain = Data("iroha:kagemusha:v1:mint-statement".utf8)
  private static let redemptionIDDomain = Data(
    "iroha:kagemusha:v1:redemption-id".utf8)
  private static let redemptionStatementDigestDomain = Data(
    "iroha:kagemusha:v1:redemption-statement".utf8)
  private static let topUpRequestSchema = "iroha.torii.v1.kagemusha.top_up.request"
  private static let topUpInstructionSchema =
    "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1"
  private static let redemptionRequestSchema = "iroha.torii.v1.kagemusha.redeem.request"

  /// Maximum canonical top-up request archive accepted by KAGEMUSHA V1.
  public static let maximumTopUpRequestBytes = 16 * 1024

  /// Maximum canonical bytes in the public body of secure-device operation 21.
  public static let maximumDeviceMintStageCommandBytes = 64 * 1024

  /// Maximum canonical bytes in the fixed public result of secure-device operation 21.
  public static let maximumDeviceMintStageResultBytes = 128

  /// Sole dynamic instruction identifier for a KAGEMUSHA V1 top-up.
  public static let topUpInstructionWireName = "iroha.kagemusha.v1.top_up"

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

  /// Encode a bounded governed profile without granting it qualification authority.
  public static func encodeHardwareProfileShape(_ value: KagemushaHardwareProfileV1) throws -> Data
  {
    try bounded(frame("KagemushaHardwareProfileV1", hardwareProfile(value), 8), 512)
  }

  /// Decode exactly one canonical hardware-profile archive.
  public static func decodeHardwareProfileShapeExact(_ bytes: Data) throws
    -> KagemushaHardwareProfileV1
  {
    try decodeExact(
      bytes, 512, "KagemushaHardwareProfileV1", 8,
      decodeHardwareProfile, encodeHardwareProfileShape)
  }

  /// Encode a bounded credential without verifying its governance signature.
  public static func encodeHardwareCredentialShape(_ value: KagemushaHardwareCredentialV1) throws
    -> Data
  {
    try bounded(frame("KagemushaHardwareCredentialV1", hardwareCredential(value), 8), 768)
  }

  /// Decode exactly one canonical hardware-credential archive.
  public static func decodeHardwareCredentialShapeExact(_ bytes: Data) throws
    -> KagemushaHardwareCredentialV1
  {
    try decodeExact(
      bytes, 768, "KagemushaHardwareCredentialV1", 8,
      decodeHardwareCredential, encodeHardwareCredentialShape)
  }

  /// Public reference to an already validated device key, not a signing capability.
  public static func deviceKeyReferenceShape(_ publicKey: KagemushaDevicePublicKeyV1) -> Data {
    var preimage = deviceKeyReferenceDomain
    preimage.append(UInt8(0))
    preimage.append(publicKey.sec1Bytes)
    return Data(SHA256.hash(data: preimage))
  }

  public static func encodePaymentRequestShape(_ value: KagemushaPaymentRequestV1) throws -> Data {
    try validatePaymentRequestPublicBindings(value)
    return try bounded(
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

  /// Return the canonical request digest used by KAGEMUSHA V1 public bindings.
  ///
  /// This is a codec digest, not request-signature verification.
  public static func paymentRequestDigest(_ value: KagemushaPaymentRequestV1) -> Data {
    var transcript = paymentRequestUnsignedTranscript(value)
    transcript.append(value.signature.rawBytes)
    return digestEncoded(paymentRequestDigestDomain, transcript)
  }

  /// Normalized asset identity, retaining its exact canonical typed Norito frame.
  public static func assetIdentityDigestShape(_ value: KagemushaAssetDefinitionIDV1) -> Data {
    digestEncoded(
      assetIdentityDigestDomain,
      frameExact(
        "iroha_data_model::asset::id::model::AssetDefinitionId", value.canonicalPayload, 1))
  }

  /// Normalized universal account identity with no domain or alias substitution.
  public static func accountIdentityDigestShape(_ value: KagemushaAccountIDV1) -> Data {
    digestEncoded(
      accountIdentityDigestDomain,
      frameExact(
        "iroha_data_model::account::model::AccountId", value.canonicalPayload, 8))
  }

  /// Exact hardware signing preimage: domain, zero, then the unsigned 324-byte transcript.
  public static func paymentRequestSigningBytesShape(
    _ value: KagemushaPaymentRequestV1
  ) throws -> Data {
    try validatePaymentRequestPublicBindings(value)
    var bytes = paymentRequestSigningDomain
    bytes.append(UInt8(0))
    bytes.append(paymentRequestUnsignedTranscript(value))
    return bytes
  }

  public static func paymentRequestDigestShape(
    _ value: KagemushaPaymentRequestV1
  ) throws -> Data {
    try validatePaymentRequestPublicBindings(value)
    return paymentRequestDigest(value)
  }

  /// Exact fixed-width binding for a 1...16 active receive-fold batch.
  public static func receiveFoldBatchDigestShape(
    _ value: KagemushaReceiveFoldBatchV1
  ) -> Data {
    var bytes = receiveFoldBatchDomain
    bytes.append(value.canonicalBody)
    return Data(SHA256.hash(data: bytes))
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
    frame("KagemushaPeerCreditContextV1", peerCreditContext(value), 8)
  }

  public static func decodePeerCreditContextShapeExact(
    _ bytes: Data
  ) throws -> KagemushaPeerCreditContextV1 {
    try decodeExact(
      bytes, 512, "KagemushaPeerCreditContextV1", 8,
      decodePeerCreditContext, encodePeerCreditContextShape)
  }

  public static func peerCreditContextShape(
    output: KagemushaPaymentOutputV1,
    request requestValue: KagemushaPaymentRequestV1
  ) throws -> KagemushaPeerCreditContextV1 {
    let requestDigest = paymentRequestDigest(requestValue)
    guard output.requestDigest == requestDigest, output.amount == requestValue.amount,
      output.creditID == (try expectedPeerCreditIDShape(output, request: requestValue)),
      output.committedAtMS >= requestValue.issuedAtMS,
      output.committedAtMS < requestValue.expiresAtMS
    else { throw kagemushaInvalid("peerCreditContext.publicBinding") }
    return try KagemushaPeerCreditContextV1(
      requestDigest: requestDigest, amount: output.amount,
      senderBeforeCommitment: output.senderBeforeCommitment,
      senderAfterCommitment: output.senderAfterCommitment,
      preparedTransferDigest: preparedTransferDigestShape(
        request: requestValue,
        senderBeforeCommitment: output.senderBeforeCommitment,
        senderAfterCommitment: output.senderAfterCommitment,
        transitionNullifier: output.transitionNullifier,
        ciphertextCommitment: output.ciphertextCommitment),
      recipientEncryptionKey: requestValue.recipientEncryptionKey)
  }

  public static func peerCreditContextDigestShape(
    _ value: KagemushaPeerCreditContextV1
  ) -> Data {
    digestEncoded(peerCreditContextDigestDomain, encodePeerCreditContextShape(value))
  }

  public static func encryptedCreditAADForPeerShape(
    output: KagemushaPaymentOutputV1,
    request: KagemushaPaymentRequestV1
  ) throws -> KagemushaEncryptedCreditAADV1 {
    let context = try peerCreditContextShape(
      output: output, request: request)
    return try KagemushaEncryptedCreditAADV1(
      purpose: .peer, contextDigest: peerCreditContextDigestShape(context),
      issuanceOrTransitionCommitment: output.ciphertextCommitment,
      creditID: output.creditID, amount: output.amount)
  }

  public static func ciphertextDigestShape(_ bytes: Data) -> Data {
    digestEncoded(ciphertextDigestDomain, bytes)
  }

  /// Derive the sole pooled reserve identity for one asset incarnation.
  public static func liabilityPoolID(
    networkID: Data, asset: KagemushaAssetDefinitionIDV1,
    incarnation: KagemushaAssetIncarnationV1
  ) -> Data {
    let preimage = fields([
      networkID, asset.canonicalPayload, assetIncarnation(incarnation),
    ])
    return digestEncoded(
      liabilityPoolDomain,
      frameExact("iroha.kagemusha.v1.liability-pool-preimage", preimage, 1))
  }

  /// Digest the exact 210-byte direct request-bound transfer transcript before encryption.
  public static func preparedTransferDigestShape(
    request: KagemushaPaymentRequestV1,
    senderBeforeCommitment: Data,
    senderAfterCommitment: Data,
    transitionNullifier: Data,
    ciphertextCommitment: Data
  ) throws -> Data {
    try validatePaymentRequestPublicBindings(request)
    let before = try kagemushaDigest(senderBeforeCommitment, "preparedTransfer.senderBefore")
    let after = try kagemushaDigest(senderAfterCommitment, "preparedTransfer.senderAfter")
    guard before != after else { throw kagemushaInvalid("preparedTransfer.stateCommitments") }
    var transcript = u16(KagemushaWireV1.wireVersion)
    transcript.append(paymentRequestDigest(request))
    transcript.append(request.amount.littleEndianBytes)
    transcript.append(before)
    transcript.append(after)
    transcript.append(
      try kagemushaDigest(transitionNullifier, "preparedTransfer.transitionNullifier"))
    transcript.append(request.recipientEncryptionKey.rawBytes)
    transcript.append(
      try kagemushaDigest(ciphertextCommitment, "preparedTransfer.ciphertextCommitment"))
    return digestEncoded(preparedTransferDigestDomain, transcript)
  }

  public static func expectedPeerCreditIDShape(
    _ value: KagemushaPaymentOutputV1,
    request: KagemushaPaymentRequestV1
  ) throws -> Data {
    try validatePaymentRequestPublicBindings(request)
    var preimage = peerCreditIDDomain
    preimage.append(UInt8(0))
    preimage.append(value.transitionNullifier)
    preimage.append(paymentRequestDigest(request))
    return Data(SHA256.hash(data: preimage))
  }

  /// Commit one private peer-credit opening before deriving its credit ID.
  public static func peerCreditOpeningCommitmentShape(
    requestDigest: Data,
    recipientOneTimeKey: KagemushaX25519PublicKeyV1,
    amount: KagemushaUInt128V1,
    creditCommitmentOpening: Data,
    recipientBindingOpening: Data,
    recoveryNonce: Data
  ) throws -> Data {
    guard !amount.isZero else { throw kagemushaInvalid("peerCreditOpening.amount") }
    var preimage = peerCreditOpeningCommitmentDomain
    preimage.append(UInt8(0))
    preimage.append(u16(KagemushaWireV1.wireVersion))
    preimage.append(try kagemushaDigest(requestDigest, "peerCreditOpening.requestDigest"))
    preimage.append(recipientOneTimeKey.rawBytes)
    preimage.append(amount.littleEndianBytes)
    preimage.append(
      try kagemushaDigest(
        creditCommitmentOpening, "peerCreditOpening.creditCommitmentOpening"))
    preimage.append(
      try kagemushaDigest(
        recipientBindingOpening, "peerCreditOpening.recipientBindingOpening"))
    preimage.append(try kagemushaDigest(recoveryNonce, "peerCreditOpening.recoveryNonce"))
    return Data(SHA256.hash(data: preimage))
  }

  /// Return the canonical lifecycle digest bound by the recursive proof.
  public static func lifecycleBindingDigestShape(_ value: KagemushaLifecycleBindingV1) -> Data {
    digestEncoded(
      lifecycleBindingDigestDomain,
      frame("KagemushaLifecycleBindingV1", lifecycle(value), 8))
  }

  public static func outboxReservationCommitmentShape(
    _ value: KagemushaOutboxReservationV1
  ) throws -> Data {
    guard
      value.reservedOutboxBytes
        >= KagemushaWireV1.minimumOutboxBytes(
          for: value.operationKind),
      value.operationKind == .sendSplit || value.operationKind == .redeemSplit
    else { throw kagemushaInvalid("outboxReservation.capacity") }
    return digestEncoded(
      outboxReservationCommitmentDomain, outboxReservationTranscript(value))
  }

  public static func hardwareTerminalBodyCommitmentShape(
    _ value: KagemushaHardwareTerminalBodyV1
  ) -> Data {
    digestEncoded(
      hardwareTerminalBodyCommitmentDomain,
      frame("KagemushaHardwareTerminalBodyV1", hardwareTerminalBody(value), 8))
  }

  public static func commitCertificateIDShape(
    _ value: KagemushaCommitCertificateV1
  ) -> Data {
    digestEncoded(commitCertificateIDDomain, commitCertificateIDTranscript(value))
  }

  public static func commitCertificateDigestShape(
    _ value: KagemushaCommitCertificateV1
  ) -> Data {
    digestEncoded(commitCertificateDigestDomain, commitCertificateTranscript(value))
  }

  public static func encodeCommitCertificateShape(
    _ value: KagemushaCommitCertificateV1
  ) throws -> Data {
    guard value.certificateID == commitCertificateIDShape(value) else {
      throw kagemushaInvalid("commitCertificate.certificateID")
    }
    return try bounded(
      frame("KagemushaCommitCertificateV1", commitCertificate(value), 8),
      KagemushaWireV1.maximumCommitCertificateBytes)
  }

  public static func decodeCommitCertificateShapeExact(
    _ bytes: Data
  ) throws -> KagemushaCommitCertificateV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumCommitCertificateBytes,
      "KagemushaCommitCertificateV1", 8, decodeCommitCertificate,
      encodeCommitCertificateShape)
  }

  public static func encodeRedemptionProofShape(
    _ value: KagemushaRedemptionProofV1
  ) throws -> Data {
    try bounded(
      frame("KagemushaRedemptionProofV1", redemptionProof(value), 8),
      Int(KagemushaWireV1.maximumRedemptionProofBytes))
  }

  public static func decodeRedemptionProofShapeExact(
    _ bytes: Data
  ) throws -> KagemushaRedemptionProofV1 {
    try decodeExact(
      bytes, Int(KagemushaWireV1.maximumRedemptionProofBytes),
      "KagemushaRedemptionProofV1", 8, decodeRedemptionProof,
      encodeRedemptionProofShape)
  }

  public static func encodePaymentProofShape(
    _ value: KagemushaPaymentProofV1
  ) throws -> Data {
    try bounded(
      frame("KagemushaPaymentProofV1", paymentProof(value), 8),
      Int(KagemushaWireV1.maximumPaymentProofBytes))
  }

  public static func decodePaymentProofShapeExact(
    _ bytes: Data
  ) throws -> KagemushaPaymentProofV1 {
    try decodeExact(
      bytes, Int(KagemushaWireV1.maximumPaymentProofBytes),
      "KagemushaPaymentProofV1", 8, decodePaymentProof,
      encodePaymentProofShape)
  }

  /// Return the canonical compact payment-output digest.
  public static func paymentOutputDigestShape(_ value: KagemushaPaymentOutputV1) -> Data {
    digestEncoded(
      sendSplitStatementDigestDomain,
      paymentOutputTranscript(value))
  }

  /// Return the canonical redemption identity derived from its public statement.
  public static func redemptionIDShape(_ value: KagemushaRedemptionStatementV1) -> Data {
    let preimage = fields([
      lifecycleBindingDigestShape(value.lifecycle), value.terminalNullifier,
      value.amount.littleEndianBytes, value.beneficiary.canonicalPayload,
      value.redemptionCommitment,
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

  /// Proof-independent body commitment. The final certificate is bound separately by the proof.
  public static func paymentBodyDigestShape(
    output: KagemushaPaymentOutputV1, encryptedCredit: Data
  ) throws -> Data {
    _ = try decodeEncryptedCreditEnvelopeShapeExact(encryptedCredit)
    var transcript = paymentOutputDigestShape(output)
    transcript.append(ciphertextDigestShape(encryptedCredit))
    return digestEncoded(paymentBodyDigestDomain, transcript)
  }

  /// Return the canonical proof-bearing envelope digest after checking its request binding.
  public static func paymentDigestShape(
    _ value: KagemushaPaymentV1,
    against requestValue: KagemushaPaymentRequestV1
  ) throws -> Data {
    try digestEncoded(
      paymentDigestDomain,
      encodePaymentShape(value, against: requestValue))
  }

  public static func encodePaymentShape(
    _ value: KagemushaPaymentV1, against requestValue: KagemushaPaymentRequestV1
  ) throws -> Data {
    try validatePaymentPublicBindings(value, requestValue)
    return try bounded(
      frame("KagemushaPaymentV1", payment(value), 8),
      KagemushaWireV1.maximumPaymentBytes)
  }

  public static func decodePaymentShapeExact(
    _ bytes: Data, against requestValue: KagemushaPaymentRequestV1
  ) throws -> KagemushaPaymentV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumPaymentBytes, "KagemushaPaymentV1", 8,
      decodePayment
    ) {
      try encodePaymentShape($0, against: requestValue)
    }
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
      try encodeAcknowledgementShape(
        $0, against: requestValue, payment: paymentValue)
    }
  }

  public static func encodeMintAuthorizationShape(
    _ value: KagemushaMintAuthorizationV1
  ) throws -> Data {
    try validateMintAuthorizationPublicBindings(value)
    return try bounded(
      frame("KagemushaMintAuthorizationV1", mintAuthorization(value), 16),
      KagemushaWireV1.maximumMintAuthorizationBytes)
  }

  /// Return the semantic digest constrained by one mint-authorization proof.
  public static func mintAuthorizationStatementDigestShape(
    _ value: KagemushaMintAuthorizationStatementV1
  ) throws -> Data {
    try validateMintAuthorizationContextPublicBindings(value.context)
    return digestEncoded(
      mintAuthorizationStatementDigestDomain,
      frame("KagemushaMintAuthorizationStatementV1", mintAuthorizationStatement(value), 16))
  }

  public static func decodeMintAuthorizationShapeExact(_ bytes: Data) throws
    -> KagemushaMintAuthorizationV1
  {
    try decodeExact(
      bytes, KagemushaWireV1.maximumMintAuthorizationBytes,
      "KagemushaMintAuthorizationV1", 16, decodeMintAuthorization,
      encodeMintAuthorizationShape)
  }

  /// Canonical public mint context, checked against its liability-pool identity.
  public static func mintAuthorizationContextDigestShape(
    _ value: KagemushaMintAuthorizationContextV1
  ) throws -> Data {
    try validateMintAuthorizationContextPublicBindings(value)
    return digestEncoded(
      mintAuthorizationContextDigestDomain,
      frame("KagemushaMintAuthorizationContextV1", mintAuthorizationContext(value), 16))
  }

  /// Digest of the complete pre-debit authorization after public-field consistency checks.
  public static func mintAuthorizationDigestShape(_ value: KagemushaMintAuthorizationV1) throws
    -> Data
  {
    try digestEncoded(mintAuthorizationDigestDomain, encodeMintAuthorizationShape(value))
  }

  /// Unique mint-credit identity derived from the pre-credit lifecycle and issuance context.
  public static func expectedMintCreditIDShape(_ value: KagemushaMintCreditStatementV1) throws
    -> Data
  {
    let lifecycle = value.lifecycle
    guard lifecycle.operationKind == .mintFold,
      lifecycle.liabilityPoolID
        == liabilityPoolID(
          networkID: lifecycle.networkID, asset: lifecycle.asset,
          incarnation: lifecycle.assetIncarnation)
    else { throw kagemushaInvalid("mintCredit.lifecycle") }
    let lifecyclePreimage = fields([
      u16(lifecycle.version), lifecycle.networkID, u16(lifecycle.protocolVersion),
      lifecycle.suiteID, lifecycle.vkDigest, lifecycle.releaseID,
      lifecycle.asset.canonicalPayload, assetIncarnation(lifecycle.assetIncarnation),
      u32(lifecycle.scale), lifecycle.liabilityPoolID, lifecycle.hardwareProfileID,
      u64(lifecycle.policyEpoch), enumUnit(lifecycle.operationKind.rawValue),
    ])
    let lifecycleDigest = digestEncoded(
      mintLifecycleContextDigestDomain,
      frameExact("iroha.kagemusha.v1.mint-lifecycle-context-preimage", lifecyclePreimage, 8))
    let preimage = fields([
      lifecycleDigest, value.recipientCredentialCommitment, value.authorizationContextDigest,
      value.amount.littleEndianBytes, value.issuanceCommitment,
      value.recipient.canonicalPayload, value.creditCommitment,
    ])
    return digestEncoded(
      mintCreditIDDomain, frameExact("iroha.kagemusha.v1.mint-credit-id-preimage", preimage, 16))
  }

  /// Semantic statement digest required by the mint-credit proof; this does not verify it.
  public static func mintCreditStatementDigestShape(_ value: KagemushaMintCreditStatementV1) throws
    -> Data
  {
    guard value.lifecycle.creditID == (try expectedMintCreditIDShape(value)) else {
      throw kagemushaInvalid("mintCredit.creditID")
    }
    return digestEncoded(
      mintStatementDigestDomain,
      frame("KagemushaMintCreditStatementV1", mintCreditStatement(value), 16))
  }

  /// Public AEAD associated-data shape for the exact pre-debit mint authorization.
  public static func encryptedCreditAADForMintShape(
    _ value: KagemushaMintAuthorizationStatementV1
  ) throws -> KagemushaEncryptedCreditAADV1 {
    try KagemushaEncryptedCreditAADV1(
      purpose: .mint, contextDigest: mintAuthorizationContextDigestShape(value.context),
      issuanceOrTransitionCommitment: value.issuanceCommitment,
      creditID: value.creditID, amount: value.context.amount)
  }

  public static func encodeMintCreditShape(_ value: KagemushaMintCreditV1) throws -> Data {
    try validateMintCreditPublicBindings(value)
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

  /// Encode a mint credit only when every public field agrees with its exact authorization.
  public static func encodeMintCreditShape(
    _ value: KagemushaMintCreditV1, against authorization: KagemushaMintAuthorizationV1
  ) throws -> Data {
    try validateMintCreditPublicBindings(value, authorization)
    return try encodeMintCreditShape(value)
  }

  /// Decode an exact mint credit bound to one pre-debit authorization, without proof verification.
  public static func decodeMintCreditShapeExact(
    _ bytes: Data, against authorization: KagemushaMintAuthorizationV1
  ) throws -> KagemushaMintCreditV1 {
    try decodeExact(
      bytes, KagemushaWireV1.maximumMintCreditBytes, "KagemushaMintCreditV1", 16, decodeMintCredit
    ) {
      try encodeMintCreditShape($0, against: authorization)
    }
  }

  /// Encode an operation-21 body after validating both exact nested public archives.
  public static func encodeDeviceMintStageCommandShape(
    _ value: KagemushaDeviceMintStageCommandV1
  ) throws -> Data {
    _ = try validatedDeviceMintStageInputs(value)
    return try bounded(
      frameExact(
        deviceMintStageCommandSchema,
        fields([
          u16(value.version), vector(value.canonicalAuthorization),
          vector(value.canonicalMintCredit),
        ]),
        8),
      maximumDeviceMintStageCommandBytes)
  }

  /// Build and encode an operation-21 body from exact nested public archives.
  public static func encodeDeviceMintStageCommandShape(
    canonicalAuthorization: Data,
    canonicalMintCredit: Data
  ) throws -> Data {
    try encodeDeviceMintStageCommandShape(
      KagemushaDeviceMintStageCommandV1(
        canonicalAuthorization: canonicalAuthorization,
        canonicalMintCredit: canonicalMintCredit))
  }

  /// Decode one exact operation-21 body without granting staging authority.
  public static func decodeDeviceMintStageCommandShapeExact(
    _ bytes: Data
  ) throws -> KagemushaDeviceMintStageCommandV1 {
    try decodeExactSchema(
      bytes, maximumDeviceMintStageCommandBytes, deviceMintStageCommandSchema, 8,
      decodeDeviceMintStageCommand, encodeDeviceMintStageCommandShape)
  }

  /// Encode the fixed public operation-21 result.
  public static func encodeDeviceMintStageResultShape(
    _ value: KagemushaDeviceMintStageResultV1
  ) throws -> Data {
    try bounded(
      frameExact(
        deviceMintStageResultSchema,
        fields([u16(value.version), Data([value.disposition.rawValue]), value.creditID]),
        2),
      maximumDeviceMintStageResultBytes)
  }

  /// Decode one exact public operation-21 result.
  public static func decodeDeviceMintStageResultShapeExact(
    _ bytes: Data
  ) throws -> KagemushaDeviceMintStageResultV1 {
    try decodeExactSchema(
      bytes, maximumDeviceMintStageResultBytes, deviceMintStageResultSchema, 2,
      decodeDeviceMintStageResult, encodeDeviceMintStageResultShape)
  }

  /// Decode and bind a public operation-21 result to its command credit identity.
  public static func decodeDeviceMintStageResultShapeExact(
    _ bytes: Data,
    against command: KagemushaDeviceMintStageCommandV1
  ) throws -> KagemushaDeviceMintStageResultV1 {
    let result = try decodeDeviceMintStageResultShapeExact(bytes)
    let (_, credit) = try validatedDeviceMintStageInputs(command)
    guard result.creditID == credit.statement.lifecycle.creditID else {
      throw KagemushaWireEnvelopeErrorV1.invalidText
    }
    return result
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

  /// Encode one exact reserve-backed top-up request for generic Torii ingress.
  public static func encodeTopUpRequestShape(
    _ value: KagemushaTopUpRequestV1
  ) throws -> Data {
    try validateTopUpRequestPublicBindings(value)
    return try bounded(
      frameExact(topUpRequestSchema, topUpRequest(value), 16), maximumTopUpRequestBytes)
  }

  /// Decode one exact top-up request without granting its proof monetary authority.
  public static func decodeTopUpRequestShapeExact(
    _ bytes: Data
  ) throws -> KagemushaTopUpRequestV1 {
    try decodeExactSchema(
      bytes, maximumTopUpRequestBytes, topUpRequestSchema, 16,
      decodeTopUpRequest, encodeTopUpRequestShape)
  }

  /// Build the sole native top-up instruction for payer-signed transaction assembly.
  public static func topUpInstructionFrame(
    _ value: KagemushaTopUpRequestV1
  ) throws -> TransactionInstructionFrame {
    _ = try encodeTopUpRequestShape(value)
    let archive = frameExact(
      topUpInstructionSchema, fields([topUpRequest(value)]), 16)
    return try TransactionInstructionFrame(
      wireName: topUpInstructionWireName, framedPayload: archive)
  }

  /// Encode one exact full or partial redemption request for generic Torii ingress.
  public static func encodeRedemptionRequestShape(
    _ value: KagemushaRedemptionRequestV1
  ) throws -> Data {
    try validateRedemptionVoucherPublicBindings(value.voucher)
    return try bounded(
      frameExact(redemptionRequestSchema, redemptionRequest(value), 16), 8 * 1024)
  }

  /// Decode one exact redemption request without granting its voucher monetary authority.
  public static func decodeRedemptionRequestShapeExact(
    _ bytes: Data
  ) throws -> KagemushaRedemptionRequestV1 {
    try decodeExactSchema(
      bytes, 8 * 1024, redemptionRequestSchema, 16,
      decodeRedemptionRequest, encodeRedemptionRequestShape)
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

  /// Validate the sole three-message KAGEMUSHA exchange in frozen IPM1 order `1...3`.
  public static func validateCompleteExchangeShape(
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1
  ) throws -> Int {
    let payloads = try [
      encodePaymentRequestShape(request),
      encodePaymentShape(payment, against: request),
      encodeAcknowledgementShape(acknowledgement, against: request, payment: payment),
    ]
    return try validateAggregateTransportSize(
      payloads,
      kinds: [.paymentRequest, .payment, .acknowledgement],
      maximumRawBytes: KagemushaWireV1.maximumCompleteExchangeRawBytes,
      maximumTextBytes: KagemushaWireV1.maximumCompleteExchangeTextBytes)
  }

  // MARK: - Public binding checks

  private static func validatePaymentRequestPublicBindings(
    _ value: KagemushaPaymentRequestV1
  ) throws {
    let expectedPool = liabilityPoolID(
      networkID: value.networkID, asset: value.asset,
      incarnation: value.assetIncarnation)
    guard !value.amount.isZero, value.liabilityPoolID == expectedPool,
      value.hardwareCredential.networkID == value.networkID
    else { throw kagemushaInvalid("paymentRequest.publicBinding") }
  }

  private static func validateAggregateTransportSize(
    _ payloads: [Data], kinds: [KagemushaWirePayloadKindV1],
    maximumRawBytes: Int, maximumTextBytes: Int
  ) throws -> Int {
    guard payloads.count == kinds.count else { throw kagemushaInvalid("transport.kindCount") }
    let raw = payloads.reduce(0) { $0 + $1.count }
    guard raw <= maximumRawBytes else {
      throw KagemushaWireEnvelopeErrorV1.sizeExceeded(
        actual: raw, maximum: maximumRawBytes)
    }
    let text = try zip(payloads, kinds).reduce(0) { total, pair in
      total + (try KagemushaWireV1.encodeText(pair.0, kind: pair.1)).utf8.count
    }
    guard text <= maximumTextBytes else {
      throw KagemushaWireEnvelopeErrorV1.sizeExceeded(
        actual: text, maximum: maximumTextBytes)
    }
    return raw
  }

  private static func validatePaymentPublicBindings(
    _ value: KagemushaPaymentV1, _ requestValue: KagemushaPaymentRequestV1
  ) throws {
    try validatePaymentRequestPublicBindings(requestValue)
    _ = try decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit)
    _ = try encryptedCreditAADForPeerShape(
      output: value.output, request: requestValue)
    _ = try encodeCommitCertificateShape(value.commitCertificate)
    _ = try encodePaymentProofShape(value.proof)
    let certificate = value.commitCertificate
    guard value.version == KagemushaWireV1.wireVersion,
      value.output.version == value.version,
      certificate.transitionNullifier == value.output.transitionNullifier,
      certificate.commitEvidence == value.output.commitEvidence,
      value.proof.semanticDigest
        == (try paymentBodyDigestShape(
          output: value.output, encryptedCredit: value.encryptedCredit)),
      value.proof.candidateEnvelopeDigest == certificate.candidateEnvelopeDigest,
      value.proof.commitCertificateDigest == commitCertificateDigestShape(certificate),
      value.output.requestDigest == paymentRequestDigest(requestValue),
      value.output.amount == requestValue.amount,
      value.output.committedAtMS >= requestValue.issuedAtMS,
      value.output.committedAtMS < requestValue.expiresAtMS
    else { throw kagemushaInvalid("payment.publicBinding") }
  }

  private static func validateRedemptionVoucherPublicBindings(
    _ value: KagemushaRedemptionVoucherV1
  ) throws {
    let statement = value.statement
    let lifecycleDigest = lifecycleBindingDigestShape(statement.lifecycle)
    let certificateDigest = commitCertificateDigestShape(value.commitCertificate)
    guard statement.lifecycle.operationKind == .redeemSplit,
      statement.redemptionID == redemptionIDShape(statement),
      value.commitCertificate.lifecycleBindingDigest == lifecycleDigest,
      value.commitCertificate.transitionNullifier == statement.terminalNullifier,
      value.commitCertificate.commitEvidence == statement.commitEvidence,
      value.commitCertificate.hardwareProfileID == statement.lifecycle.hardwareProfileID,
      value.commitCertificate.policyEpoch == statement.lifecycle.policyEpoch,
      value.commitCertificate.certificateID == commitCertificateIDShape(value.commitCertificate),
      value.proof.semanticDigest == redemptionStatementDigestShape(statement),
      value.proof.candidateEnvelopeDigest == value.commitCertificate.candidateEnvelopeDigest,
      value.proof.commitCertificateDigest == certificateDigest
    else { throw kagemushaInvalid("redemptionVoucher.publicBinding") }
  }

  private static func validateMintAuthorizationPublicBindings(
    _ value: KagemushaMintAuthorizationV1
  ) throws {
    try validateMintAuthorizationContextPublicBindings(value.statement.context)
    guard value.proof.semanticDigest == (try mintAuthorizationStatementDigestShape(value.statement))
    else { throw kagemushaInvalid("mintAuthorization.publicBinding") }
  }

  private static func validateMintAuthorizationContextPublicBindings(
    _ context: KagemushaMintAuthorizationContextV1
  ) throws {
    let expectedPool = liabilityPoolID(
      networkID: context.networkID, asset: context.asset,
      incarnation: context.assetIncarnation)
    guard context.liabilityPoolID == expectedPool
    else { throw kagemushaInvalid("mintAuthorization.context") }
  }

  private static func validateMintCreditPublicBindings(_ value: KagemushaMintCreditV1) throws {
    let encryptedCredit = try encodeEncryptedCreditEnvelopeShape(value.encryptedCredit)
    guard value.statement.lifecycle.operationKind == .mintFold,
      value.proof.semanticDigest == (try mintCreditStatementDigestShape(value.statement)),
      value.encryptedCredit.version == value.version,
      value.statement.lifecycle.ciphertextDigest == ciphertextDigestShape(encryptedCredit)
    else { throw kagemushaInvalid("mintCredit.publicBinding") }
  }

  private static func validateMintCreditPublicBindings(
    _ value: KagemushaMintCreditV1, _ authorization: KagemushaMintAuthorizationV1
  ) throws {
    try validateMintCreditPublicBindings(value)
    try validateMintAuthorizationPublicBindings(authorization)
    let statement = value.statement
    let lifecycle = statement.lifecycle
    let context = authorization.statement.context
    guard lifecycle.releaseID == context.releaseID,
      lifecycle.suiteID == context.suiteID,
      lifecycle.vkDigest == context.vkDigest,
      lifecycle.networkID == context.networkID,
      lifecycle.asset == context.asset,
      lifecycle.assetIncarnation == context.assetIncarnation,
      lifecycle.scale == context.scale,
      lifecycle.liabilityPoolID == context.liabilityPoolID,
      lifecycle.hardwareProfileID == context.hardwareProfileID,
      lifecycle.policyEpoch == context.policyEpoch,
      statement.amount == context.amount,
      statement.recipient == context.recipient,
      statement.recipientCredentialCommitment == context.recipientCredentialCommitment,
      statement.creditCommitment == context.creditCommitment,
      statement.authorizationContextDigest == (try mintAuthorizationContextDigestShape(context)),
      statement.mintAuthorizationDigest == (try mintAuthorizationDigestShape(authorization)),
      statement.issuanceCommitment == authorization.statement.issuanceCommitment,
      lifecycle.creditID == authorization.statement.creditID,
      lifecycle.ciphertextDigest == authorization.statement.ciphertextDigest,
      value.artifactManifestDigest == context.artifactManifestDigest
    else { throw kagemushaInvalid("mintCredit.authorizationBinding") }
    _ = try encryptedCreditAADForMintShape(authorization.statement)
  }

  private static func validateTopUpRequestPublicBindings(
    _ value: KagemushaTopUpRequestV1
  ) throws {
    try validateMintAuthorizationPublicBindings(value.mintAuthorization)
    let statement = value.mintAuthorization.statement
    let context = statement.context
    let expectedPool = liabilityPoolID(
      networkID: value.networkID, asset: value.asset,
      incarnation: value.assetIncarnation)
    guard value.operationID == context.operationID,
      value.issuanceCommitment == statement.issuanceCommitment,
      value.creditID == statement.creditID,
      value.releaseID == context.releaseID,
      value.suiteID == context.suiteID,
      value.vkDigest == context.vkDigest,
      value.networkID == context.networkID,
      value.asset == context.asset,
      value.assetIncarnation == context.assetIncarnation,
      value.scale == context.scale,
      value.amount == context.amount,
      value.liabilityPoolID == expectedPool,
      value.liabilityPoolID == context.liabilityPoolID,
      value.payer == context.payer,
      value.recipient == context.recipient,
      value.hardwareCredential.credentialID == context.hardwareCredentialID,
      value.hardwareCredential.hardwareProfileID == context.hardwareProfileID,
      value.hardwareCredential.policyEpoch == context.policyEpoch,
      value.recipientCredentialCommitment == context.recipientCredentialCommitment,
      value.creditCommitment == context.creditCommitment,
      value.recipientOneTimeKey == context.recipientOneTimeKey,
      value.artifactManifestDigest == context.artifactManifestDigest,
      ciphertextDigestShape(value.encryptedCredit) == statement.ciphertextDigest
    else { throw kagemushaInvalid("topUpRequest.publicBinding") }
    _ = try decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit)
  }

  private static func validateAcknowledgementPublicBindings(
    _ value: KagemushaAcknowledgementV1, _ requestValue: KagemushaPaymentRequestV1,
    _ paymentValue: KagemushaPaymentV1
  ) throws {
    try validatePaymentPublicBindings(paymentValue, requestValue)
    guard value.requestDigest == paymentRequestDigest(requestValue),
      value.paymentDigest
        == (try paymentDigestShape(paymentValue, against: requestValue)),
      value.inboxReceipt.creditID == paymentValue.output.creditID
    else { throw kagemushaInvalid("acknowledgement.publicBinding") }
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
      u16(v.version), v.requestDigest, v.amount.littleEndianBytes,
      v.senderBeforeCommitment, v.senderAfterCommitment, v.preparedTransferDigest,
      v.recipientEncryptionKey.rawBytes,
    ])
  }

  private static func commitEvidence(_ v: KagemushaCommitEvidenceV1) -> Data {
    switch v {
    case .trustedTime(let value):
      enumPayload(v.wireTag, fields([value.timeEvidenceCommitment]))
    case .monotonicLease(let value):
      enumPayload(v.wireTag, fields([value.leaseEvidenceCommitment]))
    }
  }

  private static func commitEvidenceTranscript(_ v: KagemushaCommitEvidenceV1) -> Data {
    var bytes = u32(v.wireTag)
    bytes.append(v.evidenceCommitment)
    return bytes
  }

  private static func outboxReservationTranscript(_ v: KagemushaOutboxReservationV1) -> Data {
    var bytes = v.reservationID
    bytes.append(u32(v.operationKind.rawValue))
    bytes.append(u32(v.reservedOutboxBytes))
    bytes.append(u64(v.issuedAtMS))
    bytes.append(u64(v.expiresAtMS))
    return bytes
  }

  private static func hardwareTerminalBody(_ v: KagemushaHardwareTerminalBodyV1) -> Data {
    fields([
      u16(v.version), v.candidateEnvelopeDigest, v.lifecycleBindingDigest,
      v.transitionNullifier, v.outboxReservationCommitment,
      commitEvidence(v.commitEvidence), v.hardwareProfileID, u64(v.policyEpoch),
      v.privateSuccessorCommitment, v.privateJournalCommitment,
      v.privateRecoveryCommitment,
    ])
  }

  private static func commitCertificateIDTranscript(
    _ v: KagemushaCommitCertificateV1
  ) -> Data {
    var bytes = u16(v.version)
    for field in [
      v.candidateEnvelopeDigest, v.lifecycleBindingDigest, v.transitionNullifier,
      v.outboxReservationCommitment,
    ] { bytes.append(field) }
    bytes.append(commitEvidenceTranscript(v.commitEvidence))
    bytes.append(v.hardwareProfileID)
    bytes.append(u64(v.policyEpoch))
    bytes.append(v.hardwareTerminalCommitment)
    return bytes
  }

  private static func commitCertificateTranscript(
    _ v: KagemushaCommitCertificateV1
  ) -> Data {
    var bytes = u16(v.version)
    bytes.append(v.certificateID)
    bytes.append(contentsOf: commitCertificateIDTranscript(v).dropFirst(2))
    return bytes
  }

  private static func commitCertificate(_ v: KagemushaCommitCertificateV1) -> Data {
    fields([
      u16(v.version), v.certificateID, v.candidateEnvelopeDigest,
      v.lifecycleBindingDigest, v.transitionNullifier,
      v.outboxReservationCommitment, commitEvidence(v.commitEvidence),
      v.hardwareProfileID, u64(v.policyEpoch), v.hardwareTerminalCommitment,
    ])
  }

  private static func redemptionProof(_ v: KagemushaRedemptionProofV1) -> Data {
    fields([
      u16(v.version), v.eqProtocolDigest, v.epProtocolDigest, v.semanticDigest,
      v.candidateEnvelopeDigest, v.commitCertificateDigest,
      v.eqDeferredAudit, v.epDeferredAudit, vector(v.eqProof), vector(v.epProof),
      vector(v.eqHistory), vector(v.epHistory),
    ])
  }

  private static func paymentProof(_ v: KagemushaPaymentProofV1) -> Data {
    fields([
      u16(v.version), v.eqProtocolDigest, v.epProtocolDigest, v.semanticDigest,
      v.candidateEnvelopeDigest, v.commitCertificateDigest,
      v.eqDeferredAudit, v.epDeferredAudit, vector(v.eqProof), vector(v.epProof),
      vector(v.eqHistory), vector(v.epHistory),
    ])
  }

  private static func hardwareProfile(_ v: KagemushaHardwareProfileV1) -> Data {
    fields([
      u16(v.version), u16(v.protocolVersion), v.hardwareProfileID, v.providerID,
      enumUnit(v.platformClass.rawValue), v.productClassDigest, v.firmwarePolicyDigest,
      v.enrollmentAttestationVerifierDigest, v.attestationTrustRootsDigest,
      v.allowedSuiteCommitment, u64(v.policyEpoch), v.governanceCredentialPublicKey.sec1Bytes,
      u16(v.capabilityMask), v.qualificationReportDigest, u64(v.validFromMS), u64(v.expiresAtMS),
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
      enumUnit(v.operationKind.rawValue), v.requestID, v.receiverLaneCommitment,
      v.creditID, v.ciphertextDigest,
    ])
  }

  private static func request(_ v: KagemushaPaymentRequestV1) -> Data {
    fields([
      u16(v.version), v.releaseID, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.liabilityPoolID,
      v.recipient.canonicalPayload, v.amount.littleEndianBytes,
      v.recipientEncryptionKey.rawBytes,
      hardwareCredential(v.hardwareCredential),
      v.requestID, u64(v.issuedAtMS),
      u64(v.expiresAtMS), v.signature.rawBytes,
    ])
  }

  private static func paymentRequestUnsignedTranscript(_ value: KagemushaPaymentRequestV1) -> Data {
    var bytes = u16(value.version)
    for field in [
      value.releaseID, value.networkID, assetIdentityDigestShape(value.asset),
      value.assetIncarnation.bytes,
    ] { bytes.append(field) }
    bytes.append(u32(value.scale))
    bytes.append(value.liabilityPoolID)
    bytes.append(accountIdentityDigestShape(value.recipient))
    bytes.append(value.amount.littleEndianBytes)
    bytes.append(value.recipientEncryptionKey.rawBytes)
    bytes.append(value.hardwareCredential.credentialID)
    bytes.append(value.requestID)
    bytes.append(u64(value.issuedAtMS))
    bytes.append(u64(value.expiresAtMS))
    return bytes
  }

  private static func paymentOutput(_ v: KagemushaPaymentOutputV1) -> Data {
    fields([
      u16(v.version), v.requestDigest, v.amount.littleEndianBytes,
      v.senderBeforeCommitment, v.senderAfterCommitment, v.transitionNullifier,
      v.creditID, v.ciphertextCommitment, commitEvidence(v.commitEvidence), u64(v.committedAtMS),
    ])
  }

  private static func paymentOutputTranscript(_ value: KagemushaPaymentOutputV1) -> Data {
    var bytes = u16(value.version)
    bytes.append(value.requestDigest)
    bytes.append(value.amount.littleEndianBytes)
    for digest in [value.senderBeforeCommitment, value.senderAfterCommitment,
      value.transitionNullifier, value.creditID, value.ciphertextCommitment,
    ] { bytes.append(digest) }
    bytes.append(commitEvidenceTranscript(value.commitEvidence))
    bytes.append(u64(value.committedAtMS))
    return bytes
  }

  private static func payment(_ v: KagemushaPaymentV1) -> Data {
    fields([
      u16(v.version), paymentOutput(v.output), vector(v.encryptedCredit),
      commitCertificate(v.commitCertificate), paymentProof(v.proof),
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
      v.redemptionCommitment, v.redemptionID, commitEvidence(v.commitEvidence),
    ])
  }

  private static func redemptionVoucher(_ v: KagemushaRedemptionVoucherV1) -> Data {
    fields([
      u16(v.version), redemptionStatement(v.statement),
      commitCertificate(v.commitCertificate), redemptionProof(v.proof),
      v.artifactManifestDigest,
    ])
  }

  private static func topUpRequest(_ v: KagemushaTopUpRequestV1) -> Data {
    fields([
      u16(v.version), v.operationID, v.issuanceCommitment, v.creditID,
      v.releaseID, v.suiteID, v.vkDigest, v.networkID, v.asset.canonicalPayload,
      assetIncarnation(v.assetIncarnation), u32(v.scale), v.amount.littleEndianBytes,
      v.liabilityPoolID, v.payer.canonicalPayload, v.recipient.canonicalPayload,
      hardwareCredential(v.hardwareCredential), v.recipientCredentialCommitment,
      v.creditCommitment, v.recipientOneTimeKey.rawBytes, vector(v.encryptedCredit),
      v.artifactManifestDigest, optionSome(mintAuthorization(v.mintAuthorization)),
    ])
  }

  private static func redemptionRequest(_ v: KagemushaRedemptionRequestV1) -> Data {
    fields([u16(v.version), v.operationID, redemptionVoucher(v.voucher)])
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

  private static func decodeCommitEvidence(
    _ payload: Data
  ) throws -> KagemushaCommitEvidenceV1 {
    var r = OCReader(payload)
    let tag = try r.rawU32()
    let variant = try r.field()
    try r.finish()
    var v = OCReader(variant)
    let result: KagemushaCommitEvidenceV1
    switch tag {
    case 0:
      result = .trustedTime(
        try KagemushaTrustedCommitTimeV1(timeEvidenceCommitment: v.digestField()))
    case 1:
      result = .monotonicLease(
        try KagemushaMonotonicLeaseV1(leaseEvidenceCommitment: v.digestField()))
    default:
      throw kagemushaInvalid("commitEvidence")
    }
    try v.finish()
    return result
  }

  private static func decodeCommitCertificate(
    _ payload: Data
  ) throws -> KagemushaCommitCertificateV1 {
    var r = OCReader(payload)
    let value = try KagemushaCommitCertificateV1(
      version: r.u16Field(), certificateID: r.digestField(),
      candidateEnvelopeDigest: r.digestField(), lifecycleBindingDigest: r.digestField(),
      transitionNullifier: r.digestField(), outboxReservationCommitment: r.digestField(),
      commitEvidence: decodeCommitEvidence(r.field()), hardwareProfileID: r.digestField(),
      policyEpoch: r.u64Field(), hardwareTerminalCommitment: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeRedemptionProof(
    _ payload: Data
  ) throws -> KagemushaRedemptionProofV1 {
    var r = OCReader(payload)
    let value = try KagemushaRedemptionProofV1(
      version: r.u16Field(), eqProtocolDigest: r.digestField(),
      epProtocolDigest: r.digestField(), semanticDigest: r.digestField(),
      candidateEnvelopeDigest: r.digestField(), commitCertificateDigest: r.digestField(),
      eqDeferredAudit: r.digestField(), epDeferredAudit: r.digestField(),
      eqProof: r.vectorField(), epProof: r.vectorField(), eqHistory: r.vectorField(),
      epHistory: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodePaymentProof(
    _ payload: Data
  ) throws -> KagemushaPaymentProofV1 {
    var r = OCReader(payload)
    let value = try KagemushaPaymentProofV1(
      version: r.u16Field(), eqProtocolDigest: r.digestField(),
      epProtocolDigest: r.digestField(), semanticDigest: r.digestField(),
      candidateEnvelopeDigest: r.digestField(), commitCertificateDigest: r.digestField(),
      eqDeferredAudit: r.digestField(), epDeferredAudit: r.digestField(),
      eqProof: r.vectorField(), epProof: r.vectorField(), eqHistory: r.vectorField(),
      epHistory: r.vectorField())
    try r.finish()
    return value
  }

  private static func decodePeerCreditContext(
    _ payload: Data
  ) throws -> KagemushaPeerCreditContextV1 {
    var r = OCReader(payload)
    let value = try KagemushaPeerCreditContextV1(
      version: r.u16Field(), requestDigest: r.digestField(),
      amount: r.u128Field(), senderBeforeCommitment: r.digestField(),
      senderAfterCommitment: r.digestField(), preparedTransferDigest: r.digestField(),
      recipientEncryptionKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)))
    try r.finish()
    return value
  }

  private static func decodeHardwareProfile(_ payload: Data) throws -> KagemushaHardwareProfileV1 {
    var r = OCReader(payload)
    let version = try r.u16Field()
    let protocolVersion = try r.u16Field()
    let profileID = try r.digestField()
    let providerID = try r.digestField()
    guard let platform = KagemushaHardwarePlatformClassV1(rawValue: try decodeUnitEnum(r.field()))
    else {
      throw kagemushaInvalid("hardwareProfile.platformClass")
    }
    let value = try KagemushaHardwareProfileV1(
      version: version, protocolVersion: protocolVersion, hardwareProfileID: profileID,
      providerID: providerID, platformClass: platform,
      productClassDigest: r.digestField(), firmwarePolicyDigest: r.digestField(),
      enrollmentAttestationVerifierDigest: r.digestField(),
      attestationTrustRootsDigest: r.digestField(),
      allowedSuiteCommitment: r.digestField(), policyEpoch: r.u64Field(),
      governanceCredentialPublicKey: KagemushaDevicePublicKeyV1(sec1Bytes: r.exactField(65)),
      capabilityMask: r.u16Field(), qualificationReportDigest: r.digestField(),
      validFromMS: r.u64Field(), expiresAtMS: r.u64Field())
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
      receiverLaneCommitment: r.exactField(32), creditID: r.exactField(32),
      ciphertextDigest: r.exactField(32))
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
      amount: r.u128Field(),
      recipientEncryptionKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)),
      hardwareCredential: decodeHardwareCredential(r.field()),
      requestID: r.digestField(),
      issuedAtMS: r.u64Field(), expiresAtMS: r.u64Field(),
      signature: KagemushaDeviceSignatureV1(rawBytes: r.exactField(64)))
    try r.finish()
    return value
  }

  private static func decodePaymentOutput(_ payload: Data) throws -> KagemushaPaymentOutputV1 {
    var r = OCReader(payload)
    let value = try KagemushaPaymentOutputV1(
      version: r.u16Field(), requestDigest: r.digestField(), amount: r.u128Field(),
      senderBeforeCommitment: r.digestField(), senderAfterCommitment: r.digestField(),
      transitionNullifier: r.digestField(), creditID: r.digestField(),
      ciphertextCommitment: r.digestField(), commitEvidence: decodeCommitEvidence(r.field()),
      committedAtMS: r.u64Field())
    try r.finish()
    return value
  }

  private static func decodePayment(_ payload: Data) throws -> KagemushaPaymentV1 {
    var r = OCReader(payload)
    let value = try KagemushaPaymentV1(
      version: r.u16Field(), output: decodePaymentOutput(r.field()),
      encryptedCredit: r.vectorField(),
      commitCertificate: decodeCommitCertificate(r.field()), proof: decodePaymentProof(r.field()))
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

  private static func decodeDeviceMintStageCommand(_ payload: Data) throws
    -> KagemushaDeviceMintStageCommandV1
  {
    var reader = OCReader(payload)
    let value = try KagemushaDeviceMintStageCommandV1(
      version: reader.u16Field(),
      canonicalAuthorization: reader.vectorField(),
      canonicalMintCredit: reader.vectorField())
    try reader.finish()
    _ = try validatedDeviceMintStageInputs(value)
    return value
  }

  private static func decodeDeviceMintStageResult(_ payload: Data) throws
    -> KagemushaDeviceMintStageResultV1
  {
    var reader = OCReader(payload)
    let version = try reader.u16Field()
    let dispositionRaw = try reader.u8Field()
    let creditID = try reader.digestField()
    try reader.finish()
    guard let disposition = KagemushaDeviceMintStageDispositionV1(rawValue: dispositionRaw) else {
      throw KagemushaWireEnvelopeErrorV1.invalidText
    }
    return try KagemushaDeviceMintStageResultV1(
      version: version, disposition: disposition, creditID: creditID)
  }

  private static func validatedDeviceMintStageInputs(
    _ value: KagemushaDeviceMintStageCommandV1
  ) throws -> (KagemushaMintAuthorizationV1, KagemushaMintCreditV1) {
    guard value.version == 1 else { throw KagemushaWireEnvelopeErrorV1.invalidText }
    let authorization = try decodeMintAuthorizationShapeExact(value.canonicalAuthorization)
    let credit = try decodeMintCreditShapeExact(
      value.canonicalMintCredit, against: authorization)
    return (authorization, credit)
  }

  private static func decodeRedemptionStatement(_ payload: Data) throws
    -> KagemushaRedemptionStatementV1
  {
    var r = OCReader(payload)
    let value = try KagemushaRedemptionStatementV1(
      version: r.u16Field(), lifecycle: decodeLifecycle(r.field()), amount: r.u128Field(),
      beneficiary: KagemushaAccountIDV1(canonicalPayload: r.field()),
      terminalNullifier: r.digestField(), redemptionCommitment: r.digestField(),
      redemptionID: r.digestField(), commitEvidence: decodeCommitEvidence(r.field()))
    try r.finish()
    return value
  }

  private static func decodeRedemptionVoucher(_ payload: Data) throws
    -> KagemushaRedemptionVoucherV1
  {
    var r = OCReader(payload)
    let value = try KagemushaRedemptionVoucherV1(
      version: r.u16Field(), statement: decodeRedemptionStatement(r.field()),
      commitCertificate: decodeCommitCertificate(r.field()),
      proof: decodeRedemptionProof(r.field()), artifactManifestDigest: r.digestField())
    try r.finish()
    return value
  }

  private static func decodeTopUpRequest(_ payload: Data) throws
    -> KagemushaTopUpRequestV1
  {
    var r = OCReader(payload)
    let value = try KagemushaTopUpRequestV1(
      version: r.u16Field(), operationID: r.digestField(),
      issuanceCommitment: r.digestField(), creditID: r.digestField(),
      releaseID: r.digestField(), suiteID: r.digestField(), vkDigest: r.digestField(),
      networkID: r.exactField(32),
      asset: KagemushaAssetDefinitionIDV1(canonicalPayload: r.field()),
      assetIncarnation: decodeAssetIncarnation(r.field()), scale: r.u32Field(),
      amount: r.u128Field(), liabilityPoolID: r.digestField(),
      payer: KagemushaAccountIDV1(canonicalPayload: r.field()),
      recipient: KagemushaAccountIDV1(canonicalPayload: r.field()),
      hardwareCredential: decodeHardwareCredential(r.field()),
      recipientCredentialCommitment: r.digestField(),
      creditCommitment: r.digestField(),
      recipientOneTimeKey: KagemushaX25519PublicKeyV1(rawBytes: r.exactField(32)),
      encryptedCredit: r.vectorField(), artifactManifestDigest: r.digestField(),
      mintAuthorization: decodeRequiredOption(
        r.field(), decode: decodeMintAuthorization))
    try r.finish()
    return value
  }

  private static func decodeRedemptionRequest(_ payload: Data) throws
    -> KagemushaRedemptionRequestV1
  {
    var r = OCReader(payload)
    let value = try KagemushaRedemptionRequestV1(
      version: r.u16Field(), operationID: r.digestField(),
      voucher: decodeRedemptionVoucher(r.field()))
    try r.finish()
    return value
  }

  // MARK: - Canonical framing helpers

  private static func decodeExact<T>(
    _ bytes: Data, _ maximum: Int, _ type: String, _ alignment: Int,
    _ decoder: (Data) throws -> T, _ encoder: (T) throws -> Data
  ) throws -> T {
    try decodeExactSchema(bytes, maximum, model + type, alignment, decoder, encoder)
  }

  private static func decodeExactSchema<T>(
    _ bytes: Data, _ maximum: Int, _ schema: String, _ alignment: Int,
    _ decoder: (Data) throws -> T, _ encoder: (T) throws -> Data
  ) throws -> T {
    guard !bytes.isEmpty, bytes.count <= maximum else {
      throw KagemushaWireEnvelopeErrorV1.invalidText
    }
    // Data slices retain their source indices. Rebase only after enforcing the byte cap,
    // before the shared frame parser uses zero-based wire offsets.
    let canonical = bytes.startIndex == 0 ? bytes : Data(bytes)
    guard let decoded = noritoDecodeFrame(canonical),
      decoded.header.flags == NoritoHeader.compactLen,
      decoded.header.schema == noritoSchemaHash(forTypeName: schema),
      decoded.paddingLength == noritoHeaderPaddingLength(payloadAlignment: alignment)
    else { throw KagemushaWireEnvelopeErrorV1.invalidText }
    let value = try decoder(decoded.payload)
    guard try encoder(value) == canonical else {
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

private func optionSome(_ value: Data) -> Data {
  var writer = OCWriter()
  writer.raw(Data([1]))
  writer.field(value)
  return writer.data
}

private func decodeRequiredOption<T>(
  _ payload: Data, decode: (Data) throws -> T
) throws -> T {
  var reader = OCReader(payload)
  guard try reader.raw(1) == Data([1]) else {
    throw KagemushaWireEnvelopeErrorV1.invalidText
  }
  let value = try decode(reader.field())
  try reader.finish()
  return value
}

private func enumUnit(_ value: UInt32) -> Data { u32(value) }

private func enumPayload(_ tag: UInt32, _ payload: Data) -> Data {
  var writer = OCWriter()
  writer.raw(u32(tag))
  writer.field(payload)
  return writer.data
}

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

  mutating func u8Field() throws -> UInt8 {
    let value = try exactField(1)
    return value[value.startIndex]
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
