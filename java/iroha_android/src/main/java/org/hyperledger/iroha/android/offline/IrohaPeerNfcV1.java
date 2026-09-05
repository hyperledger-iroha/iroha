// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcAPDUCodecV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurablePaymentAdmissionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcInfoV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeResultV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderTransceiverV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcStatusV1;

/** Java entry point for the strict three-message KAGEMUSHA NFC state machine. */
public final class IrohaPeerNfcV1 {
  public static final String APPLICATION_IDENTIFIER_HEX =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX;
  public static final int APPLICATION_IDENTIFIER_SIZE =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.APPLICATION_IDENTIFIER_SIZE;
  public static final int COMMAND_CLASS =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.COMMAND_CLASS;
  public static final int WIRE_VERSION =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.WIRE_VERSION;
  public static final int SESSION_ID_BYTES =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.SESSION_ID_BYTES;
  public static final int HASH_BYTES =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.HASH_BYTES;
  public static final int MAXIMUM_CHUNK_BYTES =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES;
  public static final int MAXIMUM_MESSAGE_BYTES =
      org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES;

  private IrohaPeerNfcV1() {}

  public static byte[] applicationIdentifier() {
    return org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.applicationIdentifier();
  }

  public static boolean matchesApplicationIdentifier(final byte[] candidate) {
    return org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.matchesApplicationIdentifier(
        copy(candidate, "candidate"));
  }

  public static byte[] encodeCommand(final IrohaPeerNfcCommandV1 command) {
    return IrohaPeerNfcAPDUCodecV1.encode(Objects.requireNonNull(command, "command"));
  }

  public static IrohaPeerNfcCommandV1 decodeCommand(final byte[] apdu) {
    return IrohaPeerNfcAPDUCodecV1.decode(copy(apdu, "apdu"));
  }

  public static IrohaPeerNfcInfoV1 decodeInfo(final byte[] bytes) {
    return IrohaPeerNfcInfoV1.decode(copy(bytes, "bytes"));
  }

  public static IrohaPeerNfcStatusV1 decodeStatus(final byte[] bytes) {
    return IrohaPeerNfcStatusV1.decode(copy(bytes, "bytes"));
  }

  public static IrohaPeerNfcLimitsV1 limits(
      final int maximumMessageBytes,
      final int maximumReadChunkBytes,
      final int maximumWriteChunkBytes) {
    return new IrohaPeerNfcLimitsV1(
        maximumMessageBytes, maximumReadChunkBytes, maximumWriteChunkBytes);
  }

  /** Builds the immutable profile shared by all three IPM1 messages. */
  public static IrohaPeerNfcProfilePolicyV1 profilePolicy(
      final IrohaPeerPayloadProfile profile) {
    return new IrohaPeerNfcProfilePolicyV1(sharedProfile(profile));
  }

  public static IrohaPeerNfcReceiverSessionV1 receiver(
      final byte[] canonicalRequest,
      final byte[] sessionId,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return new IrohaPeerNfcReceiverSessionV1(
        copy(canonicalRequest, "canonicalRequest"),
        copy(sessionId, "sessionId"),
        Objects.requireNonNull(profilePolicy, "profilePolicy"),
        Objects.requireNonNull(limits, "limits"));
  }

  /** Builds the durable result returned after hardware stages one payment. */
  public static IrohaPeerNfcDurablePaymentAdmissionV1 durablePaymentAdmission(
      final IrohaPeerNfcPaymentAdmissionContextV1 context,
      final byte[] canonicalAcknowledgement) {
    return new IrohaPeerNfcDurablePaymentAdmissionV1(
        Objects.requireNonNull(context, "context"),
        copy(canonicalAcknowledgement, "canonicalAcknowledgement"));
  }

  /** Runs the transport-neutral reader exchange for request, payment, and acknowledgement. */
  public static IrohaPeerNfcReaderExchangeResultV1 runReaderExchange(
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits,
      final IrohaPeerNfcReaderTransceiverV1 transceiver,
      final IrohaPeerNfcReaderExchangeV1.PreparePayment preparePayment) {
    return IrohaPeerNfcReaderExchangeV1.run(
        Objects.requireNonNull(profilePolicy, "profilePolicy"),
        Objects.requireNonNull(limits, "limits"),
        Objects.requireNonNull(transceiver, "transceiver"),
        Objects.requireNonNull(preparePayment, "preparePayment"));
  }

  static org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile sharedProfile(
      final IrohaPeerPayloadProfile profile) {
    Objects.requireNonNull(profile, "profile");
    return org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.fromCode(profile.code());
  }

  private static byte[] copy(final byte[] value, final String name) {
    return Objects.requireNonNull(value, name).clone();
  }
}
