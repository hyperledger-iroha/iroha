// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Objects;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceIntentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAccountIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetIncarnationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaCreditOpeningV1;
import org.hyperledger.iroha.sdk.offline.KagemushaCommitCertificateV1;
import org.hyperledger.iroha.sdk.offline.KagemushaCommitEvidenceV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDevicePublicKeyV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceMintStageCommandV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceMintStageResultV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEncryptedCreditAadV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEncryptedCreditEnvelopeV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareCredentialV1;
import org.hyperledger.iroha.sdk.offline.KagemushaHardwareProfileV1;
import org.hyperledger.iroha.sdk.offline.KagemushaLifecycleBindingV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationContextV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintCreditStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaMintCreditV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPastaStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentRequestModeV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentOutputV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentProofV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPeerCreditContextV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionProofV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.KagemushaTopUpRequestV1;
import org.hyperledger.iroha.sdk.offline.KagemushaX25519PublicKeyV1;

/**
 * Java mirror of the sole KAGEMUSHA V1 canonical shape codec.
 *
 * <p>The peer exchange is exactly request, acceptance intent, acceptance ticket,
 * payment, and acknowledgement. These methods validate framing and cross-field bindings only;
 * monetary authority remains in the shared native core and qualified device service.
 */
public final class KagemushaNoritoV1 {
  /** Maximum canonical bytes for the request embedded in {@code TopUpKagemushaV1}. */
  public static final int MAXIMUM_TOP_UP_REQUEST_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.MAXIMUM_TOP_UP_REQUEST_BYTES;

  /** Maximum canonical bytes in secure-device operation 21's public command body. */
  public static final int MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES;

  /** Maximum canonical bytes in secure-device operation 21's fixed public result. */
  public static final int MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES =
      org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES;

  private KagemushaNoritoV1() {}

  public static byte[] encodeAggregateStateShape(
      final KagemushaAggregateStateCommitmentV1 value) {
    return core().encodeAggregateStateShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaAggregateStateCommitmentV1 decodeAggregateStateShapeExact(
      final byte[] bytes) {
    return core().decodeAggregateStateShapeExact(copy(bytes));
  }

  public static byte[] encodeHardwareProfileShape(final KagemushaHardwareProfileV1 value) {
    return core().encodeHardwareProfileShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaHardwareProfileV1 decodeHardwareProfileShapeExact(final byte[] bytes) {
    return core().decodeHardwareProfileShapeExact(copy(bytes));
  }

  public static byte[] encodeHardwareCredentialShape(final KagemushaHardwareCredentialV1 value) {
    return core().encodeHardwareCredentialShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaHardwareCredentialV1 decodeHardwareCredentialShapeExact(
      final byte[] bytes) {
    return core().decodeHardwareCredentialShapeExact(copy(bytes));
  }

  public static byte[] encodePaymentRequestShape(final KagemushaPaymentRequestV1 value) {
    return core().encodePaymentRequestShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaPaymentRequestV1 decodePaymentRequestShapeExact(final byte[] bytes) {
    return core().decodePaymentRequestShapeExact(copy(bytes));
  }

  public static String encodePaymentRequestTextShape(final KagemushaPaymentRequestV1 value) {
    return core().encodePaymentRequestTextShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaPaymentRequestV1 decodePaymentRequestTextShapeExact(final String text) {
    return core().decodePaymentRequestTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeAcceptanceIntentShape(final KagemushaAcceptanceIntentV1 value) {
    return core().encodeAcceptanceIntentShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaAcceptanceIntentV1 decodeAcceptanceIntentShapeExact(
      final byte[] bytes) {
    return core().decodeAcceptanceIntentShapeExact(copy(bytes));
  }

  public static byte[] encodeAcceptanceIntentShape(
      final KagemushaAcceptanceIntentV1 value,
      final KagemushaPaymentRequestV1 request) {
    return core().encodeAcceptanceIntentShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static KagemushaAcceptanceIntentV1 decodeAcceptanceIntentShapeExact(
      final byte[] bytes, final KagemushaPaymentRequestV1 request) {
    return core().decodeAcceptanceIntentShapeExact(
        copy(bytes), Objects.requireNonNull(request, "request"));
  }

  public static String encodeAcceptanceIntentTextShape(
      final KagemushaAcceptanceIntentV1 value,
      final KagemushaPaymentRequestV1 request) {
    return core().encodeAcceptanceIntentTextShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] encodeAcceptanceTicketShape(
      final KagemushaAcceptanceTicketV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent) {
    return core().encodeAcceptanceTicketShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"));
  }

  public static KagemushaAcceptanceTicketV1 decodeAcceptanceTicketShapeExact(
      final byte[] bytes,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent) {
    return core().decodeAcceptanceTicketShapeExact(
        copy(bytes),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"));
  }

  public static String encodeAcceptanceTicketTextShape(
      final KagemushaAcceptanceTicketV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent) {
    return core().encodeAcceptanceTicketTextShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"));
  }

  public static byte[] encodeCommitCertificateShape(final KagemushaCommitCertificateV1 value) {
    return core().encodeCommitCertificateShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaCommitCertificateV1 decodeCommitCertificateShapeExact(
      final byte[] bytes) {
    return core().decodeCommitCertificateShapeExact(copy(bytes));
  }

  /** Encodes the bounded post-commit payment proof without granting monetary authority. */
  public static byte[] encodePaymentProofShape(final KagemushaPaymentProofV1 value) {
    return core().encodePaymentProofShape(Objects.requireNonNull(value, "value"));
  }

  /** Decodes the exact post-commit payment proof without cryptographic verification. */
  public static KagemushaPaymentProofV1 decodePaymentProofShapeExact(final byte[] bytes) {
    return core().decodePaymentProofShapeExact(copy(bytes));
  }

  /** Encodes the bounded post-commit redemption proof without granting monetary authority. */
  public static byte[] encodeRedemptionProofShape(final KagemushaRedemptionProofV1 value) {
    return core().encodeRedemptionProofShape(Objects.requireNonNull(value, "value"));
  }

  /** Decodes the exact post-commit redemption proof without cryptographic verification. */
  public static KagemushaRedemptionProofV1 decodeRedemptionProofShapeExact(final byte[] bytes) {
    return core().decodeRedemptionProofShapeExact(copy(bytes));
  }

  public static byte[] encodePeerCreditContextShape(final KagemushaPeerCreditContextV1 value) {
    return core().encodePeerCreditContextShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaPeerCreditContextV1 decodePeerCreditContextShapeExact(
      final byte[] bytes) {
    return core().decodePeerCreditContextShapeExact(copy(bytes));
  }

  public static KagemushaPeerCreditContextV1 peerCreditContextShape(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket,
      final KagemushaPaymentOutputV1 output) {
    return core().peerCreditContextShape(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"),
        Objects.requireNonNull(output, "output"));
  }

  public static byte[] peerCreditContextDigestShape(final KagemushaPeerCreditContextV1 value) {
    return core().peerCreditContextDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaEncryptedCreditAadV1 encryptedCreditAadForPeerShape(
      final KagemushaPaymentOutputV1 output,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().encryptedCreditAadForPeerShape(
        Objects.requireNonNull(output, "output"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static KagemushaEncryptedCreditAadV1 encryptedCreditAadForMintShape(
      final KagemushaMintAuthorizationStatementV1 statement) {
    return core().encryptedCreditAadForMintShape(Objects.requireNonNull(statement, "statement"));
  }

  public static byte[] encodePaymentShape(
      final KagemushaPaymentV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().encodePaymentShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static KagemushaPaymentV1 decodePaymentShapeExact(
      final byte[] bytes,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().decodePaymentShapeExact(
        copy(bytes),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static String encodePaymentTextShape(
      final KagemushaPaymentV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().encodePaymentTextShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static KagemushaPaymentV1 decodePaymentTextShapeExact(
      final String text,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().decodePaymentTextShapeExact(
        Objects.requireNonNull(text, "text"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static byte[] encodeAcknowledgementShape(
      final KagemushaAcknowledgementV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().encodeAcknowledgementShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static KagemushaAcknowledgementV1 decodeAcknowledgementShapeExact(
      final byte[] bytes,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().decodeAcknowledgementShapeExact(
        copy(bytes),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static String encodeAcknowledgementTextShape(
      final KagemushaAcknowledgementV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().encodeAcknowledgementTextShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static KagemushaAcknowledgementV1 decodeAcknowledgementTextShapeExact(
      final String text,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().decodeAcknowledgementTextShapeExact(
        Objects.requireNonNull(text, "text"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static byte[] encodeMintAuthorizationShape(final KagemushaMintAuthorizationV1 value) {
    return core().encodeMintAuthorizationShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaMintAuthorizationV1 decodeMintAuthorizationShapeExact(
      final byte[] bytes) {
    return core().decodeMintAuthorizationShapeExact(copy(bytes));
  }

  public static String encodeMintAuthorizationTextShape(final KagemushaMintAuthorizationV1 value) {
    return core().encodeMintAuthorizationTextShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaMintAuthorizationV1 decodeMintAuthorizationTextShapeExact(
      final String text) {
    return core().decodeMintAuthorizationTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeMintCreditShape(final KagemushaMintCreditV1 value) {
    return core().encodeMintCreditShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] encodeMintCreditShape(
      final KagemushaMintCreditV1 value, final KagemushaMintAuthorizationV1 authorization) {
    return core().encodeMintCreditShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(authorization, "authorization"));
  }

  public static KagemushaMintCreditV1 decodeMintCreditShapeExact(final byte[] bytes) {
    return core().decodeMintCreditShapeExact(copy(bytes));
  }

  public static KagemushaMintCreditV1 decodeMintCreditShapeExact(
      final byte[] bytes, final KagemushaMintAuthorizationV1 authorization) {
    return core().decodeMintCreditShapeExact(
        copy(bytes), Objects.requireNonNull(authorization, "authorization"));
  }

  public static String encodeMintCreditTextShape(final KagemushaMintCreditV1 value) {
    return core().encodeMintCreditTextShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaMintCreditV1 decodeMintCreditTextShapeExact(final String text) {
    return core().decodeMintCreditTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  /** Encodes a bounded operation-21 body after checking both nested public archives. */
  public static byte[] encodeDeviceMintStageCommandShape(
      final KagemushaDeviceMintStageCommandV1 value) {
    return core().encodeDeviceMintStageCommandShape(Objects.requireNonNull(value, "value"));
  }

  /** Builds and encodes a bounded operation-21 body from exact nested archives. */
  public static byte[] encodeDeviceMintStageCommandShape(
      final byte[] canonicalAuthorization, final byte[] canonicalMintCredit) {
    return core().encodeDeviceMintStageCommandShape(
        copy(canonicalAuthorization), copy(canonicalMintCredit));
  }

  /** Decodes an exact operation-21 body without granting staging authority. */
  public static KagemushaDeviceMintStageCommandV1 decodeDeviceMintStageCommandShapeExact(
      final byte[] bytes) {
    return core().decodeDeviceMintStageCommandShapeExact(copy(bytes));
  }

  /** Encodes the fixed public operation-21 result. */
  public static byte[] encodeDeviceMintStageResultShape(
      final KagemushaDeviceMintStageResultV1 value) {
    return core().encodeDeviceMintStageResultShape(Objects.requireNonNull(value, "value"));
  }

  /** Decodes an exact public operation-21 result. */
  public static KagemushaDeviceMintStageResultV1 decodeDeviceMintStageResultShapeExact(
      final byte[] bytes) {
    return core().decodeDeviceMintStageResultShapeExact(copy(bytes));
  }

  /** Decodes and binds a public operation-21 result to its exact command. */
  public static KagemushaDeviceMintStageResultV1 decodeDeviceMintStageResultShapeExact(
      final byte[] bytes, final KagemushaDeviceMintStageCommandV1 command) {
    return core().decodeDeviceMintStageResultShapeExact(
        copy(bytes), Objects.requireNonNull(command, "command"));
  }

  public static byte[] encodeRedemptionVoucherShape(final KagemushaRedemptionVoucherV1 value) {
    return core().encodeRedemptionVoucherShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaRedemptionVoucherV1 decodeRedemptionVoucherShapeExact(
      final byte[] bytes) {
    return core().decodeRedemptionVoucherShapeExact(copy(bytes));
  }

  public static String encodeRedemptionVoucherTextShape(
      final KagemushaRedemptionVoucherV1 value) {
    return core().encodeRedemptionVoucherTextShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaRedemptionVoucherV1 decodeRedemptionVoucherTextShapeExact(
      final String text) {
    return core().decodeRedemptionVoucherTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeTopUpRequestShape(final KagemushaTopUpRequestV1 value) {
    return core().encodeTopUpRequestShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaTopUpRequestV1 decodeTopUpRequestShapeExact(final byte[] bytes) {
    return core().decodeTopUpRequestShapeExact(copy(bytes));
  }

  /** Encodes the concrete registered payload for one native {@code TopUpKagemushaV1}. */
  public static byte[] encodeTopUpInstructionPayloadShape(
      final KagemushaTopUpRequestV1 value) {
    return core().encodeTopUpInstructionPayloadShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] encodeRedemptionRequestShape(
      final KagemushaRedemptionRequestV1 value) {
    return core().encodeRedemptionRequestShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaRedemptionRequestV1 decodeRedemptionRequestShapeExact(
      final byte[] bytes) {
    return core().decodeRedemptionRequestShapeExact(copy(bytes));
  }

  public static byte[] encodeCreditOpeningShape(final KagemushaCreditOpeningV1 value) {
    return core().encodeCreditOpeningShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaCreditOpeningV1 decodeCreditOpeningShapeExact(final byte[] bytes) {
    return core().decodeCreditOpeningShapeExact(copy(bytes));
  }

  public static KagemushaCreditOpeningV1 decodeCreditOpeningShapeExactAgainst(
      final byte[] bytes, final byte[] creditId, final BigInteger amount) {
    return core().decodeCreditOpeningShapeExactAgainst(
        copy(bytes), copy(creditId), Objects.requireNonNull(amount, "amount"));
  }

  public static byte[] encodeEncryptedCreditAadShape(final KagemushaEncryptedCreditAadV1 value) {
    return core().encodeEncryptedCreditAadShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaEncryptedCreditAadV1 decodeEncryptedCreditAadShapeExact(
      final byte[] bytes) {
    return core().decodeEncryptedCreditAadShapeExact(copy(bytes));
  }

  public static byte[] encodeEncryptedCreditEnvelopeShape(
      final KagemushaEncryptedCreditEnvelopeV1 value) {
    return core().encodeEncryptedCreditEnvelopeShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaEncryptedCreditEnvelopeV1 decodeEncryptedCreditEnvelopeShapeExact(
      final byte[] bytes) {
    return core().decodeEncryptedCreditEnvelopeShapeExact(copy(bytes));
  }

  public static byte[] encryptedCreditKdfSalt(
      final KagemushaX25519PublicKeyV1 recipient,
      final KagemushaX25519PublicKeyV1 ephemeral) {
    return core().encryptedCreditKdfSalt(
        Objects.requireNonNull(recipient, "recipient"),
        Objects.requireNonNull(ephemeral, "ephemeral"));
  }

  public static byte[] encryptedCreditKdfInfo(final KagemushaEncryptedCreditAadV1 aad) {
    return core().encryptedCreditKdfInfo(Objects.requireNonNull(aad, "aad"));
  }

  public static byte[] deviceKeyReference(final KagemushaDevicePublicKeyV1 key) {
    return core().deviceKeyReference(Objects.requireNonNull(key, "key"));
  }

  public static byte[] pastaStateCommitment(final KagemushaPastaStateCommitmentV1 value) {
    return core().pastaStateCommitment(Objects.requireNonNull(value, "value"));
  }

  public static byte[] liabilityPoolId(
      final NetworkId networkId,
      final KagemushaAssetDefinitionIdV1 asset,
      final KagemushaAssetIncarnationV1 incarnation) {
    return core().liabilityPoolId(
        Objects.requireNonNull(networkId, "networkId"),
        Objects.requireNonNull(asset, "asset"),
        Objects.requireNonNull(incarnation, "incarnation"));
  }

  public static byte[] paymentRequestDigest(final KagemushaPaymentRequestV1 value) {
    return core().paymentRequestDigest(Objects.requireNonNull(value, "value"));
  }

  /** Encodes the canonical framed asset identity used by the request transcript. */
  public static byte[] assetIdentityCanonicalShape(final KagemushaAssetDefinitionIdV1 value) {
    return core().assetIdentityCanonicalShape(Objects.requireNonNull(value, "value"));
  }

  /** Encodes the canonical framed account identity used by the request transcript. */
  public static byte[] accountIdentityCanonicalShape(final KagemushaAccountIdV1 value) {
    return core().accountIdentityCanonicalShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] paymentRequestModeDigestShape(
      final KagemushaPaymentRequestModeV1 value) {
    return core().paymentRequestModeDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] acceptanceIntentDigestShape(
      final KagemushaAcceptanceIntentV1 value, final KagemushaPaymentRequestV1 request) {
    return core().acceptanceIntentDigestShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] acceptanceTicketDigestShape(
      final KagemushaAcceptanceTicketV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent) {
    return core().acceptanceTicketDigestShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"));
  }

  public static byte[] peerCreditOpeningCommitmentShape(
      final byte[] requestDigest,
      final org.hyperledger.iroha.sdk.offline.KagemushaX25519PublicKeyV1 recipientOneTimeKey,
      final BigInteger amount,
      final byte[] creditCommitmentOpening,
      final byte[] recipientBindingOpening,
      final byte[] recoveryNonce) {
    return core()
        .peerCreditOpeningCommitmentShape(
            Objects.requireNonNull(requestDigest, "requestDigest"),
            Objects.requireNonNull(recipientOneTimeKey, "recipientOneTimeKey"),
            Objects.requireNonNull(amount, "amount"),
            Objects.requireNonNull(creditCommitmentOpening, "creditCommitmentOpening"),
            Objects.requireNonNull(recipientBindingOpening, "recipientBindingOpening"),
            Objects.requireNonNull(recoveryNonce, "recoveryNonce"));
  }

  public static byte[] lifecycleDigestShape(final KagemushaLifecycleBindingV1 value) {
    return core().lifecycleDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] expectedCommitCertificateIdShape(
      final KagemushaCommitCertificateV1 value) {
    return core().expectedCommitCertificateIdShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] commitCertificateDigestShape(
      final KagemushaCommitCertificateV1 value,
      final KagemushaLifecycleBindingV1 lifecycle,
      final KagemushaCommitEvidenceV1 commitEvidence,
      final byte[] transitionNullifier) {
    return core().commitCertificateDigestShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(lifecycle, "lifecycle"),
        Objects.requireNonNull(commitEvidence, "commitEvidence"),
        copy(transitionNullifier));
  }

  public static byte[] ciphertextDigestShape(final byte[] bytes) {
    return core().ciphertextDigestShape(copy(bytes));
  }

  public static byte[] preparedTransferDigestShape(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket,
      final byte[] transitionNullifier,
      final byte[] ciphertextCommitment) {
    return core().preparedTransferDigestShape(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"),
        copy(transitionNullifier),
        copy(ciphertextCommitment));
  }

  public static byte[] expectedPeerCreditIdShape(
      final KagemushaPaymentOutputV1 output,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent) {
    return core().expectedPeerCreditIdShape(
        Objects.requireNonNull(output, "output"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"));
  }

  /** Returns the fixed output digest without granting monetary authority. */
  public static byte[] paymentOutputDigestShape(final KagemushaPaymentOutputV1 value) {
    return core().paymentOutputDigestShape(Objects.requireNonNull(value, "value"));
  }

  /** Returns the acyclic payment-body digest committed by sender hardware. */
  public static byte[] paymentBodyDigestShape(
      final KagemushaPaymentOutputV1 output, final byte[] encryptedCredit) {
    return core().paymentBodyDigestShape(
        Objects.requireNonNull(output, "output"), copy(encryptedCredit));
  }

  public static byte[] paymentDigestShape(
      final KagemushaPaymentV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket) {
    return core().paymentDigestShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static byte[] mintAuthorizationContextDigestShape(
      final KagemushaMintAuthorizationContextV1 value) {
    return core().mintAuthorizationContextDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintAuthorizationStatementDigestShape(
      final KagemushaMintAuthorizationStatementV1 value) {
    return core().mintAuthorizationStatementDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintAuthorizationDigestShape(final KagemushaMintAuthorizationV1 value) {
    return core().mintAuthorizationDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] expectedMintCreditIdShape(final KagemushaMintCreditStatementV1 value) {
    return core().expectedMintCreditIdShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintCreditStatementDigestShape(final KagemushaMintCreditStatementV1 value) {
    return core().mintCreditStatementDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] expectedRedemptionIdShape(final KagemushaRedemptionStatementV1 value) {
    return core().expectedRedemptionIdShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] redemptionStatementDigestShape(final KagemushaRedemptionStatementV1 value) {
    return core().redemptionStatementDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static int validateCompleteExchangeShape(
      final KagemushaPaymentRequestV1 request,
      final KagemushaAcceptanceIntentV1 intent,
      final KagemushaAcceptanceTicketV1 ticket,
      final KagemushaPaymentV1 payment,
      final KagemushaAcknowledgementV1 acknowledgement) {
    return core().validateCompleteExchangeShape(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(acknowledgement, "acknowledgement"));
  }

  private static org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1 core() {
    return org.hyperledger.iroha.sdk.offline.KagemushaNoritoV1.INSTANCE;
  }

  private static byte[] copy(final byte[] value) {
    return Objects.requireNonNull(value, "bytes").clone();
  }
}
