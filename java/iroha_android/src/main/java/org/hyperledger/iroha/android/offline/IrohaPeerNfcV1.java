package org.hyperledger.iroha.android.offline;

import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcAPDUCodecV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommitContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurablePaymentAdmissionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcInfoV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeResultV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderTransceiverV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointStoreV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointUpdaterV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcStatusV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcTwoTapReducerV1;

/** Java-facing entry point to the default portable NFC V1 state machine. */
public final class IrohaPeerNfcV1 {
  public static final String APPLICATION_IDENTIFIER_HEX = "F0494C44534E464301";
  public static final int COMMAND_CLASS = 0x80;
  public static final int WIRE_VERSION = 1;
  public static final int SESSION_ID_BYTES = 16;
  public static final int HASH_BYTES = 32;
  public static final int MAXIMUM_CHUNK_BYTES = 4096;
  public static final int MAXIMUM_MESSAGE_BYTES =
      IrohaPeerWireMessageV1.HEADER_LENGTH
          + IrohaPeerWireMessageV1.MAXIMUM_OFFLINE_NOTE_ENCODED_BYTES;
  public static final int INFO_BYTES = 98;
  public static final int STATUS_BYTES = 174;

  private IrohaPeerNfcV1() {}

  public static byte[] applicationIdentifier() {
    return org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.applicationIdentifier();
  }

  public static byte[] encodeCommand(final IrohaPeerNfcCommandV1 command) {
    return IrohaPeerNfcAPDUCodecV1.encode(Objects.requireNonNull(command, "command"));
  }

  public static IrohaPeerNfcCommandV1 decodeCommand(final byte[] apdu) {
    return IrohaPeerNfcAPDUCodecV1.decode(Objects.requireNonNull(apdu, "apdu").clone());
  }

  public static IrohaPeerNfcInfoV1 decodeInfo(final byte[] bytes) {
    return IrohaPeerNfcInfoV1.decode(Objects.requireNonNull(bytes, "bytes").clone());
  }

  public static IrohaPeerNfcStatusV1 decodeStatus(final byte[] bytes) {
    return IrohaPeerNfcStatusV1.decode(Objects.requireNonNull(bytes, "bytes").clone());
  }

  public static IrohaPeerNfcLimitsV1 limits(
      final int maximumMessageBytes,
      final int maximumReadChunkBytes,
      final int maximumWriteChunkBytes) {
    return new IrohaPeerNfcLimitsV1(
        maximumMessageBytes, maximumReadChunkBytes, maximumWriteChunkBytes);
  }

  /** Builds the one immutable profile used by every phase of an NFC V1 session. */
  public static IrohaPeerNfcProfilePolicyV1 profilePolicy(
      final IrohaPeerPayloadProfile profile) {
    return new IrohaPeerNfcProfilePolicyV1(sharedProfile(profile));
  }

  public static IrohaPeerNfcReceiverSessionV1 receiver(
      final byte[] sessionId,
      final byte[] encodedReceiveRequest,
      final IrohaPeerNfcDurableAcknowledgementV1 durableAcknowledgement,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return receiver(
        sessionId,
        encodedReceiveRequest,
        durableAcknowledgement,
        profilePolicy,
        limits,
        null);
  }

  /** Restores either the exact admitted 84-byte BEGIN header or an IDA1. */
  public static IrohaPeerNfcReceiverSessionV1 receiver(
      final byte[] sessionId,
      final byte[] encodedReceiveRequest,
      final IrohaPeerNfcDurableAcknowledgementV1 durableAcknowledgement,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits,
      final IrohaPeerNfcDurablePaymentAdmissionV1 restoredPaymentAdmission) {
    return new IrohaPeerNfcReceiverSessionV1(
        copy(sessionId, "sessionId"),
        copy(encodedReceiveRequest, "encodedReceiveRequest"),
        durableAcknowledgement,
        profilePolicy,
        Objects.requireNonNull(limits, "limits"),
        restoredPaymentAdmission);
  }

  /** Creates the exact durable value that must be stored before BEGIN returns 9000. */
  public static IrohaPeerNfcPaymentAdmissionContextV1 paymentAdmissionContext(
      final IrohaPeerNfcReceiverSessionV1 receiver,
      final byte[] paymentHeader) {
    final IrohaPeerNfcReceiverSessionV1 checked = Objects.requireNonNull(receiver, "receiver");
    return new IrohaPeerNfcPaymentAdmissionContextV1(
        checked.getIdentity(),
        checked.getProfilePolicy(),
        copy(paymentHeader, "paymentHeader"),
        checked.getLimits());
  }

  /** Converts the callback context into the exact IPA1 record storage returns. */
  public static IrohaPeerNfcDurablePaymentAdmissionV1 durablePaymentAdmission(
      final IrohaPeerNfcPaymentAdmissionContextV1 context,
      final IrohaPeerNfcLimitsV1 limits) {
    return new IrohaPeerNfcDurablePaymentAdmissionV1(
        Objects.requireNonNull(context, "context"),
        Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcDurablePaymentAdmissionV1 decodePaymentAdmission(
      final byte[] encoded,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return IrohaPeerNfcDurablePaymentAdmissionV1.decode(
        copy(encoded, "encoded"),
        profilePolicy,
        Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcSenderCheckpointV1 senderCheckpoint(
      final byte[] sessionId,
      final byte[] encodedReceiveRequest,
      final byte[] encodedPayment,
      final byte[] encodedDurableAcknowledgement,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits) {
    return new IrohaPeerNfcSenderCheckpointV1(
        copy(sessionId, "sessionId"),
        copy(encodedReceiveRequest, "encodedReceiveRequest"),
        copy(encodedPayment, "encodedPayment"),
        encodedDurableAcknowledgement == null ? null : encodedDurableAcknowledgement.clone(),
        profilePolicy,
        Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcDurableAcknowledgementV1 durableAcknowledgement(
      final IrohaPeerNfcCommitContextV1 commitContext,
      final byte[] encodedAcknowledgement,
      final IrohaPeerNfcLimitsV1 limits) {
    return new IrohaPeerNfcDurableAcknowledgementV1(
        Objects.requireNonNull(commitContext, "commitContext"),
        copy(encodedAcknowledgement, "encodedAcknowledgement"),
        Objects.requireNonNull(limits, "limits"));
  }

  public static IrohaPeerNfcTwoTapReducerV1 twoTapReducer(
      final IrohaPeerNfcSenderCheckpointV1 checkpoint,
      final IrohaPeerNfcLimitsV1 localLimits) {
    return new IrohaPeerNfcTwoTapReducerV1(
        Objects.requireNonNull(checkpoint, "checkpoint"),
        Objects.requireNonNull(localLimits, "localLimits"));
  }

  /**
   * Runs the shared durable two-session reader state machine without
   * reimplementing phase, retry, chunking, or checkpoint rules in Java.
   */
  public static IrohaPeerNfcReaderExchangeResultV1 runReaderExchange(
      final byte[] restoredCheckpoint,
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits,
      final int maximumActions,
      final IrohaPeerNfcReaderTransceiverV1 transceiver,
      final IrohaPeerNfcSenderCheckpointStoreV1 checkpointStore,
      final IrohaPeerNfcSenderCheckpointUpdaterV1 checkpointUpdater) {
    return IrohaPeerNfcReaderExchangeV1.run(
        Objects.requireNonNull(profilePolicy, "profilePolicy"),
        Objects.requireNonNull(transceiver, "transceiver"),
        Objects.requireNonNull(checkpointStore, "checkpointStore"),
        Objects.requireNonNull(checkpointUpdater, "checkpointUpdater"),
        restoredCheckpoint == null ? null : restoredCheckpoint.clone(),
        Objects.requireNonNull(limits, "limits"),
        maximumActions);
  }

  /** Fresh-transfer convenience overload using the standard action budget. */
  public static IrohaPeerNfcReaderExchangeResultV1 runReaderExchange(
      final IrohaPeerNfcProfilePolicyV1 profilePolicy,
      final IrohaPeerNfcLimitsV1 limits,
      final IrohaPeerNfcReaderTransceiverV1 transceiver,
      final IrohaPeerNfcSenderCheckpointStoreV1 checkpointStore,
      final IrohaPeerNfcSenderCheckpointUpdaterV1 checkpointUpdater) {
    return runReaderExchange(
        null,
        profilePolicy,
        limits,
        IrohaPeerNfcReaderExchangeV1.DEFAULT_MAXIMUM_ACTIONS,
        transceiver,
        checkpointStore,
        checkpointUpdater);
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
