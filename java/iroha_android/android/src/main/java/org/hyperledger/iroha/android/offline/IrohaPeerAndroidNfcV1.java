package org.hyperledger.iroha.android.offline;

import android.nfc.Tag;
import java.util.Objects;
import org.hyperledger.iroha.sdk.offline.IrohaPeerIsoDepLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerIsoDepTransceiverV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAdmissionHandlerV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableCommitHandlerV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverApduBridgeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;

/** Java facade for the default Android IsoDep/HCE NFC V1 adapters. */
public final class IrohaPeerAndroidNfcV1 {
  private IrohaPeerAndroidNfcV1() {}

  public static IrohaPeerIsoDepTransceiverV1 transceiver(final Tag tag) {
    return IrohaPeerIsoDepTransceiverV1.from(Objects.requireNonNull(tag, "tag"));
  }

  public static IrohaPeerNfcLimitsV1 limits(
      final int maximumTransceiveLength, final boolean supportsExtendedLengthApdu) {
    return IrohaPeerIsoDepLimitsV1.derive(
        maximumTransceiveLength, supportsExtendedLengthApdu);
  }

  /**
   * Builds the serialized async HCE boundary. BEGIN and COMMIT return 9000 only
   * after their callbacks return the exact durable admission or IDA1 record.
   */
  public static IrohaPeerNfcReceiverApduBridgeV1 receiverBridge(
      final IrohaPeerNfcReceiverSessionV1 receiver,
      final IrohaPeerNfcDurableAdmissionHandlerV1 durableAdmission,
      final IrohaPeerNfcDurableCommitHandlerV1 durableCommit) {
    return new IrohaPeerNfcReceiverApduBridgeV1(
        Objects.requireNonNull(receiver, "receiver"),
        Objects.requireNonNull(durableAdmission, "durableAdmission"),
        Objects.requireNonNull(durableCommit, "durableCommit"));
  }
}
