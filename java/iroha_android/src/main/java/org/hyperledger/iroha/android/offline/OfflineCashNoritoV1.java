// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Objects;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceIntentAuthorizationStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceIntentAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceIntentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAggregateStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashAssetIncarnationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashCreditOpeningV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashDevicePublicKeyV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashEncryptedCreditAadV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashEncryptedCreditEnvelopeV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareCredentialV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashHardwareProfileV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashLifecycleBindingV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashCommitCertificateV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintAuthorizationContextV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintAuthorizationStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintAuthorizationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintCreditStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashMintCreditV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashNoCommitClosureStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashNoCommitClosureV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashOutboxReservationV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPairedProofV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPastaStateCommitmentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentRequestModeV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentRequestV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPaymentV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashPeerCreditContextV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashRedemptionStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashRedemptionVoucherV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashTransferStatementV1;
import org.hyperledger.iroha.sdk.offline.OfflineCashX25519PublicKeyV1;

/**
 * Java mirror of the sole Offline Cash V1 canonical shape codec.
 *
 * <p>These methods validate framing and cross-field bindings only. Production monetary
 * authorization remains in the shared native core and qualified device service.
 */
public final class OfflineCashNoritoV1 {
  private OfflineCashNoritoV1() {}

  public static byte[] encodeAggregateStateShape(
      final OfflineCashAggregateStateCommitmentV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeAggregateStateShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashAggregateStateCommitmentV1 decodeAggregateStateShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAggregateStateShapeExact(copy(bytes));
  }

  public static byte[] encodeHardwareProfileShape(final OfflineCashHardwareProfileV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeHardwareProfileShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashHardwareProfileV1 decodeHardwareProfileShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeHardwareProfileShapeExact(copy(bytes));
  }

  public static byte[] encodeHardwareCredentialShape(
      final OfflineCashHardwareCredentialV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeHardwareCredentialShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashHardwareCredentialV1 decodeHardwareCredentialShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeHardwareCredentialShapeExact(copy(bytes));
  }

  public static byte[] encodePaymentRequestModeShape(
      final OfflineCashPaymentRequestModeV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodePaymentRequestModeShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashPaymentRequestModeV1 decodePaymentRequestModeShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodePaymentRequestModeShapeExact(copy(bytes));
  }

  public static byte[] encodePaymentRequestShape(final OfflineCashPaymentRequestV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodePaymentRequestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashPaymentRequestV1 decodePaymentRequestShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodePaymentRequestShapeExact(copy(bytes));
  }

  public static String encodePaymentRequestTextShape(final OfflineCashPaymentRequestV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodePaymentRequestTextShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashPaymentRequestV1 decodePaymentRequestTextShapeExact(
      final String text) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodePaymentRequestTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeAcceptanceIntentShape(
      final OfflineCashAcceptanceIntentV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeAcceptanceIntentShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static OfflineCashAcceptanceIntentV1 decodeAcceptanceIntentShapeExact(
      final byte[] bytes, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcceptanceIntentShapeExact(copy(bytes), Objects.requireNonNull(request, "request"));
  }

  public static String encodeAcceptanceIntentTextShape(
      final OfflineCashAcceptanceIntentV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .encodeAcceptanceIntentTextShape(
            Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static OfflineCashAcceptanceIntentV1 decodeAcceptanceIntentTextShapeExact(
      final String text, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcceptanceIntentTextShapeExact(
            Objects.requireNonNull(text, "text"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] encodeAcceptanceIntentAuthorizationShape(
      final OfflineCashAcceptanceIntentAuthorizationV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .encodeAcceptanceIntentAuthorizationShape(
            Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static OfflineCashAcceptanceIntentAuthorizationV1
      decodeAcceptanceIntentAuthorizationShapeExact(
          final byte[] bytes, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcceptanceIntentAuthorizationShapeExact(
            copy(bytes), Objects.requireNonNull(request, "request"));
  }

  public static String encodeAcceptanceIntentAuthorizationTextShape(
      final OfflineCashAcceptanceIntentAuthorizationV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .encodeAcceptanceIntentAuthorizationTextShape(
            Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static OfflineCashAcceptanceIntentAuthorizationV1
      decodeAcceptanceIntentAuthorizationTextShapeExact(
          final String text, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcceptanceIntentAuthorizationTextShapeExact(
            Objects.requireNonNull(text, "text"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] encodeAcceptanceTicketShape(
      final OfflineCashAcceptanceTicketV1 value,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeAcceptanceTicketShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"));
  }

  public static OfflineCashAcceptanceTicketV1 decodeAcceptanceTicketShapeExact(
      final byte[] bytes,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcceptanceTicketShapeExact(
            copy(bytes),
            Objects.requireNonNull(request, "request"),
            Objects.requireNonNull(authorization, "authorization"));
  }

  public static String encodeAcceptanceTicketTextShape(
      final OfflineCashAcceptanceTicketV1 value,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeAcceptanceTicketTextShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"));
  }

  public static OfflineCashAcceptanceTicketV1 decodeAcceptanceTicketTextShapeExact(
      final String text,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcceptanceTicketTextShapeExact(
            Objects.requireNonNull(text, "text"),
            Objects.requireNonNull(request, "request"),
            Objects.requireNonNull(authorization, "authorization"));
  }

  /** Encode one self-contained proof that a prepared sender authorization was cancelled. */
  public static byte[] encodeNoCommitClosureShape(final OfflineCashNoCommitClosureV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeNoCommitClosureShape(
        Objects.requireNonNull(value, "value"));
  }

  /** Decode one exact no-commit closure without granting its proof monetary authority. */
  public static OfflineCashNoCommitClosureV1 decodeNoCommitClosureShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeNoCommitClosureShapeExact(copy(bytes));
  }

  public static byte[] encodePeerCreditContextShape(
      final OfflineCashPeerCreditContextV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodePeerCreditContextShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashPeerCreditContextV1 decodePeerCreditContextShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodePeerCreditContextShapeExact(copy(bytes));
  }

  public static OfflineCashPeerCreditContextV1 peerCreditContextShape(
      final OfflineCashTransferStatementV1 statement,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentV1 intent,
      final OfflineCashAcceptanceTicketV1 ticket) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.peerCreditContextShape(
        Objects.requireNonNull(statement, "statement"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static byte[] peerCreditContextDigestShape(
      final OfflineCashPeerCreditContextV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.peerCreditContextDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashEncryptedCreditAadV1 encryptedCreditAadForPeerShape(
      final OfflineCashTransferStatementV1 statement,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentV1 intent,
      final OfflineCashAcceptanceTicketV1 ticket) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encryptedCreditAadForPeerShape(
        Objects.requireNonNull(statement, "statement"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(intent, "intent"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static OfflineCashEncryptedCreditAadV1 encryptedCreditAadForMintShape(
      final OfflineCashMintAuthorizationStatementV1 statement) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encryptedCreditAadForMintShape(
        Objects.requireNonNull(statement, "statement"));
  }

  public static byte[] encodePaymentShape(
      final OfflineCashPaymentV1 value, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodePaymentShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static OfflineCashPaymentV1 decodePaymentShapeExact(
      final byte[] bytes, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.decodePaymentShapeExact(
        copy(bytes), Objects.requireNonNull(request, "request"));
  }

  public static String encodePaymentTextShape(
      final OfflineCashPaymentV1 value, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodePaymentTextShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static OfflineCashPaymentV1 decodePaymentTextShapeExact(
      final String text, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.decodePaymentTextShapeExact(
        Objects.requireNonNull(text, "text"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] encodeAcknowledgementShape(
      final OfflineCashAcknowledgementV1 value,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashPaymentV1 payment) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeAcknowledgementShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
  }

  public static OfflineCashAcknowledgementV1 decodeAcknowledgementShapeExact(
      final byte[] bytes,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashPaymentV1 payment) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcknowledgementShapeExact(
            copy(bytes),
            Objects.requireNonNull(request, "request"),
            Objects.requireNonNull(payment, "payment"));
  }

  public static String encodeAcknowledgementTextShape(
      final OfflineCashAcknowledgementV1 value,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashPaymentV1 payment) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeAcknowledgementTextShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"));
  }

  public static OfflineCashAcknowledgementV1 decodeAcknowledgementTextShapeExact(
      final String text,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashPaymentV1 payment) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeAcknowledgementTextShapeExact(
            Objects.requireNonNull(text, "text"),
            Objects.requireNonNull(request, "request"),
            Objects.requireNonNull(payment, "payment"));
  }

  public static byte[] encodeMintAuthorizationShape(final OfflineCashMintAuthorizationV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeMintAuthorizationShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashMintAuthorizationV1 decodeMintAuthorizationShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeMintAuthorizationShapeExact(copy(bytes));
  }

  public static String encodeMintAuthorizationTextShape(
      final OfflineCashMintAuthorizationV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .encodeMintAuthorizationTextShape(Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashMintAuthorizationV1 decodeMintAuthorizationTextShapeExact(
      final String text) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeMintAuthorizationTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeMintCreditShape(final OfflineCashMintCreditV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeMintCreditShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashMintCreditV1 decodeMintCreditShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeMintCreditShapeExact(copy(bytes));
  }

  public static String encodeMintCreditTextShape(final OfflineCashMintCreditV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeMintCreditTextShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashMintCreditV1 decodeMintCreditTextShapeExact(final String text) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeMintCreditTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeMintCreditShape(
      final OfflineCashMintCreditV1 value, final OfflineCashMintAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeMintCreditShape(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(authorization, "authorization"));
  }

  public static OfflineCashMintCreditV1 decodeMintCreditShapeExact(
      final byte[] bytes, final OfflineCashMintAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.decodeMintCreditShapeExact(
        copy(bytes), Objects.requireNonNull(authorization, "authorization"));
  }

  public static byte[] encodeRedemptionVoucherShape(
      final OfflineCashRedemptionVoucherV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeRedemptionVoucherShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashRedemptionVoucherV1 decodeRedemptionVoucherShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeRedemptionVoucherShapeExact(copy(bytes));
  }

  public static String encodeRedemptionVoucherTextShape(
      final OfflineCashRedemptionVoucherV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .encodeRedemptionVoucherTextShape(Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashRedemptionVoucherV1 decodeRedemptionVoucherTextShapeExact(
      final String text) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeRedemptionVoucherTextShapeExact(Objects.requireNonNull(text, "text"));
  }

  public static byte[] encodeCreditOpeningShape(final OfflineCashCreditOpeningV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeCreditOpeningShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashCreditOpeningV1 decodeCreditOpeningShapeExact(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeCreditOpeningShapeExact(copy(bytes));
  }

  public static OfflineCashCreditOpeningV1 decodeCreditOpeningShapeExactAgainst(
      final byte[] bytes, final byte[] creditId, final BigInteger amount) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeCreditOpeningShapeExactAgainst(
            copy(bytes), copy(creditId), Objects.requireNonNull(amount, "amount"));
  }

  public static byte[] encodeEncryptedCreditAadShape(final OfflineCashEncryptedCreditAadV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encodeEncryptedCreditAadShape(
        Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashEncryptedCreditAadV1 decodeEncryptedCreditAadShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeEncryptedCreditAadShapeExact(copy(bytes));
  }

  public static byte[] encodeEncryptedCreditEnvelopeShape(
      final OfflineCashEncryptedCreditEnvelopeV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .encodeEncryptedCreditEnvelopeShape(Objects.requireNonNull(value, "value"));
  }

  public static OfflineCashEncryptedCreditEnvelopeV1 decodeEncryptedCreditEnvelopeShapeExact(
      final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .decodeEncryptedCreditEnvelopeShapeExact(copy(bytes));
  }

  public static byte[] encryptedCreditKdfSalt(
      final OfflineCashX25519PublicKeyV1 recipient,
      final OfflineCashX25519PublicKeyV1 ephemeral) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encryptedCreditKdfSalt(
        Objects.requireNonNull(recipient, "recipient"),
        Objects.requireNonNull(ephemeral, "ephemeral"));
  }

  public static byte[] encryptedCreditKdfInfo(final OfflineCashEncryptedCreditAadV1 aad) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.encryptedCreditKdfInfo(
        Objects.requireNonNull(aad, "aad"));
  }

  public static byte[] deviceKeyReference(final OfflineCashDevicePublicKeyV1 key) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.deviceKeyReference(
        Objects.requireNonNull(key, "key"));
  }

  public static byte[] pastaStateCommitment(final OfflineCashPastaStateCommitmentV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.pastaStateCommitment(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] liabilityPoolId(
      final NetworkId networkId,
      final OfflineCashAssetDefinitionIdV1 asset,
      final OfflineCashAssetIncarnationV1 incarnation) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.liabilityPoolId(
        Objects.requireNonNull(networkId, "networkId"),
        Objects.requireNonNull(asset, "asset"),
        Objects.requireNonNull(incarnation, "incarnation"));
  }

  public static byte[] paymentRequestDigest(final OfflineCashPaymentRequestV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.paymentRequestDigest(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] acceptanceIntentDigest(
      final OfflineCashAcceptanceIntentV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.acceptanceIntentDigest(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] acceptanceTicketDigest(
      final OfflineCashAcceptanceTicketV1 value,
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.acceptanceTicketDigest(
        Objects.requireNonNull(value, "value"),
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"));
  }

  public static byte[] acceptanceIntentAuthorizationStatementDigestShape(
      final OfflineCashAcceptanceIntentAuthorizationStatementV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .acceptanceIntentAuthorizationStatementDigestShape(
            Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] acceptanceIntentAuthorizationDigestShape(
      final OfflineCashAcceptanceIntentAuthorizationV1 value,
      final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .acceptanceIntentAuthorizationDigestShape(
            Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  /** Return the semantic digest constrained by both no-commit proof parities. */
  public static byte[] noCommitClosureStatementDigestShape(
      final OfflineCashNoCommitClosureStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .noCommitClosureStatementDigestShape(Objects.requireNonNull(value, "value"));
  }

  /** Return the canonical digest of a complete no-commit closure envelope. */
  public static byte[] noCommitClosureDigestShape(final OfflineCashNoCommitClosureV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.noCommitClosureDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] lifecycleDigestShape(final OfflineCashLifecycleBindingV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.lifecycleDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  /** Return the hiding sender-outbox commitment constrained by the terminal wrapper. */
  public static byte[] outboxReservationCommitmentShape(
      final OfflineCashOutboxReservationV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .outboxReservationCommitmentShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] ciphertextDigestShape(final byte[] bytes) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.ciphertextDigestShape(copy(bytes));
  }

  public static byte[] expectedPeerCreditIdShape(final OfflineCashTransferStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.expectedPeerCreditIdShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] transferStatementDigestShape(final OfflineCashTransferStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.transferStatementDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] paymentDigestShape(
      final OfflineCashPaymentV1 value, final OfflineCashPaymentRequestV1 request) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.paymentDigestShape(
        Objects.requireNonNull(value, "value"), Objects.requireNonNull(request, "request"));
  }

  public static byte[] expectedCommitCertificateIdShape(
      final OfflineCashCommitCertificateV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .expectedCommitCertificateIdShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] commitCertificateDigestShape(final OfflineCashCommitCertificateV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.commitCertificateDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintAuthorizationContextDigestShape(
      final OfflineCashMintAuthorizationContextV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .mintAuthorizationContextDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintAuthorizationStatementDigestShape(
      final OfflineCashMintAuthorizationStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1
        .mintAuthorizationStatementDigestShape(Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintAuthorizationDigestShape(final OfflineCashMintAuthorizationV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.mintAuthorizationDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] expectedMintCreditIdShape(final OfflineCashMintCreditStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.expectedMintCreditIdShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] mintCreditStatementDigestShape(final OfflineCashMintCreditStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.mintCreditStatementDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] expectedRedemptionIdShape(final OfflineCashRedemptionStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.expectedRedemptionIdShape(
        Objects.requireNonNull(value, "value"));
  }

  public static byte[] redemptionStatementDigestShape(final OfflineCashRedemptionStatementV1 value) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.redemptionStatementDigestShape(
        Objects.requireNonNull(value, "value"));
  }

  public static int validateTerminalDeliveryShape(
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashPaymentV1 payment,
      final OfflineCashAcknowledgementV1 acknowledgement) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.validateTerminalDeliveryShape(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(acknowledgement, "acknowledgement"));
  }

  public static int validatePreTicketExchangeShape(
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization,
      final OfflineCashAcceptanceTicketV1 ticket) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.validatePreTicketExchangeShape(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"),
        Objects.requireNonNull(ticket, "ticket"));
  }

  public static int validateCompleteExchangeShape(
      final OfflineCashPaymentRequestV1 request,
      final OfflineCashAcceptanceIntentAuthorizationV1 authorization,
      final OfflineCashAcceptanceTicketV1 ticket,
      final OfflineCashPaymentV1 payment,
      final OfflineCashAcknowledgementV1 acknowledgement) {
    return org.hyperledger.iroha.sdk.offline.OfflineCashNoritoV1.validateCompleteExchangeShape(
        Objects.requireNonNull(request, "request"),
        Objects.requireNonNull(authorization, "authorization"),
        Objects.requireNonNull(ticket, "ticket"),
        Objects.requireNonNull(payment, "payment"),
        Objects.requireNonNull(acknowledgement, "acknowledgement"));
  }

  private static byte[] copy(final byte[] value) {
    return Objects.requireNonNull(value, "bytes").clone();
  }
}
