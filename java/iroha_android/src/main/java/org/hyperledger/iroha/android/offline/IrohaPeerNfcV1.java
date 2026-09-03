// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcAPDUCodecV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcIntentAdmissionContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcIntentCommitContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommitContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableIntentAdmissionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurablePaymentAdmissionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcInfoV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeResultV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderTransceiverV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSnapshotV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointStoreV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointUpdaterV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderTicketStoreV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcStatusV1;

/** Java entry point for the strict five-message KAGEMUSHA NFC state machine. */
public final class IrohaPeerNfcV1 {
  public static final String APPLICATION_IDENTIFIER_HEX = "F0504B45504B524E464301";
  public static final int COMMAND_CLASS = 0x80;
  public static final int WIRE_VERSION = 1;
  public static final int SESSION_ID_BYTES = 16;
  public static final int HASH_BYTES = 32;
  public static final int MAXIMUM_CHUNK_BYTES = 4096;
  public static final int MAXIMUM_MESSAGE_BYTES =
      IrohaPeerWireMessageV1.HEADER_LENGTH
          + IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES;
  public static final int INFO_BYTES = 98;
  public static final int STATUS_BYTES = 178;

  private IrohaPeerNfcV1() {}

  public static byte[] applicationIdentifier() {
    return org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.applicationIdentifier();
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

  /** Builds the immutable profile shared by all five IPM1 messages. */
  public static IrohaPeerNfcProfilePolicyV1 profilePolicy(
      final IrohaPeerPayloadProfile profile) {
    return new IrohaPeerNfcProfilePolicyV1(sharedProfile(profile));
  }

  public static IrohaPeerNfcReceiverSnapshotV1 initialReceiverSnapshot(
      final byte[] sessionId,
      final byte[] encodedRequest,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcReceiverSnapshotV1.initial(
        copy(sessionId, "sessionId"),
        copy(encodedRequest, "encodedRequest"),
        profilePolicy,
        Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcReceiverSnapshotV1 decodeReceiverSnapshot(
      final byte[] encoded,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcReceiverSnapshotV1.decode(
        copy(encoded, "encoded"), profilePolicy, Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcReceiverSessionV1 receiver(
      final IrohaPeerNfcReceiverSnapshotV1 snapshot) {
    return new IrohaPeerNfcReceiverSessionV1(Objects.requireNonNull(snapshot, "snapshot"));
  }

  public static IrohaPeerNfcReceiverSessionV1 receiver(
      final byte[] sessionId,
      final byte[] encodedRequest,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return receiver(initialReceiverSnapshot(sessionId, encodedRequest, profilePolicy, limits));
  }

  /** Exact durable candidate that must be stored before BEGIN_INTENT succeeds. */
  public static IrohaPeerNfcDurableIntentAdmissionV1 durableIntentAdmission(
      final IrohaPeerNfcIntentAdmissionContextV1 context) {
    return new IrohaPeerNfcDurableIntentAdmissionV1(
        Objects.requireNonNull(context, "context"));
  }

  /** Generates the durable one-use TICKET snapshot after binding the compact INTENT. */
  public static IrohaPeerNfcDurableAcceptanceTicketV1 durableAcceptanceTicket(
      final IrohaPeerNfcIntentCommitContextV1 context,
      final byte[] encodedTicket) {
    return Objects.requireNonNull(context, "context")
        .durableTicket(copy(encodedTicket, "encodedTicket"));
  }

  /** Exact durable candidate that must be stored before BEGIN_PAYMENT succeeds. */
  public static IrohaPeerNfcDurablePaymentAdmissionV1 durablePaymentAdmission(
      final IrohaPeerNfcPaymentAdmissionContextV1 context) {
    return new IrohaPeerNfcDurablePaymentAdmissionV1(
        Objects.requireNonNull(context, "context"));
  }

  /** Generates the durable ACK snapshot after staging the complete PAYMENT. */
  public static IrohaPeerNfcDurableAcknowledgementV1 durableAcknowledgement(
      final IrohaPeerNfcCommitContextV1 context,
      final byte[] encodedAcknowledgement) {
    return Objects.requireNonNull(context, "context")
        .durableAcknowledgement(copy(encodedAcknowledgement, "encodedAcknowledgement"));
  }

  public static IrohaPeerNfcDurableIntentAdmissionV1 decodeIntentAdmission(
      final byte[] encoded,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcDurableIntentAdmissionV1.decode(
        copy(encoded, "encoded"), profilePolicy, Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcDurableAcceptanceTicketV1 decodeAcceptanceTicket(
      final byte[] encoded,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcDurableAcceptanceTicketV1.decode(
        copy(encoded, "encoded"), profilePolicy, Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcDurablePaymentAdmissionV1 decodePaymentAdmission(
      final byte[] encoded,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcDurablePaymentAdmissionV1.decode(
        copy(encoded, "encoded"), profilePolicy, Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcDurableAcknowledgementV1 decodeAcknowledgement(
      final byte[] encoded,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcDurableAcknowledgementV1.decode(
        copy(encoded, "encoded"), profilePolicy, Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcSenderCheckpointV1 senderCheckpoint(
      final byte[] sessionId,
      final byte[] encodedRequest,
      final byte[] encodedIntent,
      final byte[] encodedTicket,
      final byte[] encodedPayment,
      final byte[] encodedAcknowledgement,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return new IrohaPeerNfcSenderCheckpointV1(
        copy(sessionId, "sessionId"),
        copy(encodedRequest, "encodedRequest"),
        copy(encodedIntent, "encodedIntent"),
        nullableCopy(encodedTicket),
        nullableCopy(encodedPayment),
        nullableCopy(encodedAcknowledgement),
        profilePolicy,
        Objects.requireNonNull(limits, "limits"));
  }

  /** Runs the durable status-authoritative five-message reader exchange. */
  public static IrohaPeerNfcReaderExchangeResultV1 runReaderExchange(
      final byte[] restoredCheckpoint,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits,
      final int maximumActions,
      final IrohaPeerNfcReaderTransceiverV1 transceiver,
      final IrohaPeerNfcSenderCheckpointStoreV1 checkpointStore,
      final IrohaPeerNfcSenderTicketStoreV1 ticketStore,
      final IrohaPeerNfcSenderCheckpointUpdaterV1 checkpointUpdater) {
    return IrohaPeerNfcReaderExchangeV1.run(
        Objects.requireNonNull(profilePolicy, "profilePolicy"),
        Objects.requireNonNull(transceiver, "transceiver"),
        Objects.requireNonNull(checkpointStore, "checkpointStore"),
        Objects.requireNonNull(ticketStore, "ticketStore"),
        Objects.requireNonNull(checkpointUpdater, "checkpointUpdater"),
        nullableCopy(restoredCheckpoint),
        Objects.requireNonNull(limits, "limits"),
        maximumActions);
  }

  static org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile sharedProfile(
      final IrohaPeerPayloadProfile profile) {
    Objects.requireNonNull(profile, "profile");
    return org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.fromCode(profile.code());
  }

  private static byte[] copy(final byte[] value, final String name) {
    return Objects.requireNonNull(value, name).clone();
  }

  private static byte[] nullableCopy(final byte[] value) {
    return value == null ? null : value.clone();
  }
}
