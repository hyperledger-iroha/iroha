package org.hyperledger.iroha.android.alias;

/** Typed lifecycle operation decoded from one exact planner frame. */
public final class DecodedAliasLifecycleFrame {
  private final AliasLifecycleOperationV1 operation;
  private final byte[] reencodedFrame;

  /** Constructs a defensive decoded-frame result. */
  public DecodedAliasLifecycleFrame(
      final AliasLifecycleOperationV1 operation, final byte[] reencodedFrame) {
    if (operation == null || reencodedFrame == null) {
      throw new IllegalArgumentException("operation and reencodedFrame must not be null");
    }
    this.operation = operation;
    this.reencodedFrame = reencodedFrame.clone();
  }

  /** Returns the typed decoded operation. */
  public AliasLifecycleOperationV1 operation() {
    return operation;
  }

  /** Returns a defensive copy of the canonical re-encoding. */
  public byte[] reencodedFrame() {
    return reencodedFrame.clone();
  }
}
