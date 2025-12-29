package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Wrapper around the native offline receipt challenge encoder. */
public final class OfflineReceiptChallenge {
  private static final boolean NATIVE_AVAILABLE;

  static {
    boolean available;
    try {
      System.loadLibrary("connect_norito_bridge");
      available = true;
    } catch (UnsatisfiedLinkError error) {
      available = false;
    }
    NATIVE_AVAILABLE = available;
  }

  private final byte[] preimage;
  private final byte[] irohaHash;
  private final byte[] clientDataHash;

  private OfflineReceiptChallenge(byte[] preimage, byte[] irohaHash, byte[] clientDataHash) {
    this.preimage = preimage;
    this.irohaHash = irohaHash;
    this.clientDataHash = clientDataHash;
  }

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static OfflineReceiptChallenge compute(
      final String invoiceId,
      final String receiverAccountId,
      final String assetId,
      final String amount,
      final String nonceHex) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    final byte[] irohaHash = new byte[32];
    final byte[] clientHash = new byte[32];
    final byte[] preimage =
        nativeCompute(invoiceId, receiverAccountId, assetId, amount, nonceHex, irohaHash, clientHash);
    if (preimage == null) {
      throw new IllegalStateException("connect_norito_offline_receipt_challenge failed");
    }
    return new OfflineReceiptChallenge(preimage, irohaHash, clientHash);
  }

  public byte[] getPreimage() {
    return Arrays.copyOf(preimage, preimage.length);
  }

  public byte[] getIrohaHash() {
    return Arrays.copyOf(irohaHash, irohaHash.length);
  }

  public byte[] getClientDataHash() {
    return Arrays.copyOf(clientDataHash, clientDataHash.length);
  }

  private static native byte[] nativeCompute(
      String invoiceId,
      String receiverAccountId,
      String assetId,
      String amount,
      String nonceHex,
      byte[] irohaHashOut,
      byte[] clientHashOut);
}
