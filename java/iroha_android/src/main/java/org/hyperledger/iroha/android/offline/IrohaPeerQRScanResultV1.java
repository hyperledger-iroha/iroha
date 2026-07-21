package org.hyperledger.iroha.android.offline;

/** Immutable QR scan progress and optional completed peer message. */
public final class IrohaPeerQRScanResultV1 {
  private final IrohaPeerWireMessageV1 message;
  private final IrohaPeerPayloadProfile profile;
  private final IrohaPeerPayloadKind payloadKind;
  private final int receivedDataFrames;
  private final int totalDataFrames;
  private final int recoveredDataFrames;

  IrohaPeerQRScanResultV1(
      final IrohaPeerWireMessageV1 message,
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerPayloadKind payloadKind,
      final int receivedDataFrames,
      final int totalDataFrames,
      final int recoveredDataFrames) {
    this.message = message;
    this.profile = profile;
    this.payloadKind = payloadKind;
    this.receivedDataFrames = receivedDataFrames;
    this.totalDataFrames = totalDataFrames;
    this.recoveredDataFrames = recoveredDataFrames;
  }

  public IrohaPeerWireMessageV1 message() {
    return message;
  }

  public IrohaPeerPayloadProfile profile() {
    return profile;
  }

  public IrohaPeerPayloadKind payloadKind() {
    return payloadKind;
  }

  public int receivedDataFrames() {
    return receivedDataFrames;
  }

  public int totalDataFrames() {
    return totalDataFrames;
  }

  public int recoveredDataFrames() {
    return recoveredDataFrames;
  }

  public boolean isComplete() {
    return message != null;
  }

  public double progress() {
    if (totalDataFrames > 0) {
      return Math.min(1.0, (double) receivedDataFrames / totalDataFrames);
    }
    return isComplete() ? 1.0 : 0.0;
  }
}
