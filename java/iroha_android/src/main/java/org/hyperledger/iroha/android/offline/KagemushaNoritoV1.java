// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Objects;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.offline.KagemushaAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetIncarnationV1;
import org.hyperledger.iroha.sdk.offline.KagemushaCreditOpeningV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDevicePublicKeyV1;
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
import org.hyperledger.iroha.sdk.offline.KagemushaPaymentV1;
import org.hyperledger.iroha.sdk.offline.KagemushaPeerCreditContextV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.KagemushaTransferStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaX25519PublicKeyV1;

/**
 * Java mirror of the sole Kagemusha V1 canonical shape codec.
 *
 * <p>Only the request, payment, and acknowledgement are peer-exchange messages. These methods
 * validate framing and cross-field bindings only; monetary authority remains in the shared native
 * core and qualified device service.
 */
public final class KagemushaNoritoV1 {
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

  public static byte[] encodePeerCreditContextShape(final KagemushaPeerCreditContextV1 value) {
    return core().encodePeerCreditContextShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaPeerCreditContextV1 decodePeerCreditContextShapeExact(
      final byte[] bytes) {
    return core().decodePeerCreditContextShapeExact(copy(bytes));
  }

  public static KagemushaPeerCreditContextV1 peerCreditContextShape(
      final KagemushaTransferStatementV1 statement,
      final KagemushaPaymentRequestV1 request) {
    return core().peerCreditContextShape(
        Objects.requireNonNull(statement, "statement"),
        Objects.requireNonNull(request, "request"));
  }

  public static byte[] peerCreditContextDigestShape(final KagemushaPeerCreditContextV1 value) {
    return core().peerCreditContextDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static KagemushaEncryptedCreditAadV1 encryptedCreditAadForPeerShape(
      final KagemushaTransferStatementV1 statement,
      final KagemushaPaymentRequestV1 request) {
    return core().encryptedCreditAadForPeerShape(
        Objects.requireNonNull(statement, "statement"),
        Objects.requireNonNull(request, "request"));
  }

  public static KagemushaEncryptedCreditAadV1 encryptedCreditAadForMintShape(
      final KagemushaMintAuthorizationStatementV1 statement) {
    return core().encryptedCreditAadForMintShape(Objects.requireNonNull(statement, "statement"));
  }

  public static byte[] encodePaymentShape(
      final KagemushaPaymentV1 value, final KagemushaPaymentRequestV1 request) {
    return core().encodePaymentShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static KagemushaPaymentV1 decodePaymentShapeExact(
      final byte[] bytes, final KagemushaPaymentRequestV1 request) {
    return core().decodePaymentShapeExact(copy(bytes), Objects.requireNonNull(request, "request"));
  }

  public static String encodePaymentTextShape(
      final KagemushaPaymentV1 value, final KagemushaPaymentRequestV1 request) {
    return core().encodePaymentTextShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static KagemushaPaymentV1 decodePaymentTextShapeExact(
      final String text, final KagemushaPaymentRequestV1 request) {
    return core().decodePaymentTextShapeExact(
        Objects.requireNonNull(text, "text"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] encodeAcknowledgementShape(
      final KagemushaAcknowledgementV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment) {
    return core().encodeAcknowledgementShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
  }

  public static KagemushaAcknowledgementV1 decodeAcknowledgementShapeExact(
      final byte[] bytes,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment) {
    return core().decodeAcknowledgementShapeExact(
        copy(bytes),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
  }

  public static String encodeAcknowledgementTextShape(
      final KagemushaAcknowledgementV1 value,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment) {
    return core().encodeAcknowledgementTextShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
  }

  public static KagemushaAcknowledgementV1 decodeAcknowledgementTextShapeExact(
      final String text,
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment) {
    return core().decodeAcknowledgementTextShapeExact(
        Objects.requireNonNull(text, "text"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
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

  public static byte[] lifecycleDigestShape(final KagemushaLifecycleBindingV1 value) {
    return core().lifecycleDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] ciphertextDigestShape(final byte[] bytes) {
    return core().ciphertextDigestShape(copy(bytes));
  }

  public static byte[] expectedPeerCreditIdShape(final KagemushaTransferStatementV1 value) {
    return core().expectedPeerCreditIdShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] transferStatementDigestShape(final KagemushaTransferStatementV1 value) {
    return core().transferStatementDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] paymentDigestShape(
      final KagemushaPaymentV1 value, final KagemushaPaymentRequestV1 request) {
    return core().paymentDigestShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
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

  public static int validateTerminalDeliveryShape(
      final KagemushaPaymentRequestV1 request,
      final KagemushaPaymentV1 payment,
      final KagemushaAcknowledgementV1 acknowledgement) {
    return core().validateTerminalDeliveryShape(
        Objects.requireNonNull(request, "request"),
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
