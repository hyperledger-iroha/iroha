package org.hyperledger.iroha.android.alias;

/** A decoded EnsureAlias frame together with its canonical re-encoding. */
public final class DecodedEnsureAliasFrame {
  private final EnsureAlias instruction;
  private final byte[] reencodedFrame;

  /** Constructs an immutable decoded frame result. */
  public DecodedEnsureAliasFrame(
      final EnsureAlias instruction, final byte[] reencodedFrame) {
    if (instruction == null || reencodedFrame == null) {
      throw new IllegalArgumentException("instruction and reencodedFrame must not be null");
    }
    this.instruction = instruction;
    this.reencodedFrame = reencodedFrame.clone();
  }

  /** Returns the typed decoded instruction. */
  public EnsureAlias instruction() {
    return instruction;
  }

  /** Returns a defensive copy of the canonical re-encoding. */
  public byte[] reencodedFrame() {
    return reencodedFrame.clone();
  }
}
