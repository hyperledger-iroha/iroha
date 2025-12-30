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
      final String chainId,
      final String invoiceId,
      final String receiverAccountId,
      final String assetId,
      final String amount,
      final long issuedAtMs,
      final String nonceHex) {
    if (chainId == null || chainId.trim().isEmpty()) {
      throw new IllegalArgumentException("chainId must not be empty");
    }
    requireScaleZeroAmount(amount);
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    final byte[] irohaHash = new byte[32];
    final byte[] clientHash = new byte[32];
    final byte[] preimage =
        nativeCompute(
            chainId,
            invoiceId,
            receiverAccountId,
            assetId,
            amount,
            issuedAtMs,
            nonceHex,
            irohaHash,
            clientHash);
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
      String chainId,
      String invoiceId,
      String receiverAccountId,
      String assetId,
      String amount,
      long issuedAtMs,
      String nonceHex,
      byte[] irohaHashOut,
      byte[] clientHashOut);

  private static void requireScaleZeroAmount(final String amount) {
    if (amount == null) {
      throw new IllegalArgumentException("amount must not be null");
    }
    final String trimmed = amount.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException("amount must not be empty");
    }
    boolean seenDot = false;
    int scale = 0;
    int index = 0;
    final char first = trimmed.charAt(0);
    if (first == '+' || first == '-') {
      index = 1;
    }
    if (index >= trimmed.length()) {
      throw new IllegalArgumentException("amount must be numeric: " + amount);
    }
    for (int i = index; i < trimmed.length(); i++) {
      final char ch = trimmed.charAt(i);
      if (ch == '.') {
        if (seenDot) {
          throw new IllegalArgumentException("amount must be numeric: " + amount);
        }
        seenDot = true;
        continue;
      }
      if (ch < '0' || ch > '9') {
        throw new IllegalArgumentException("amount must be numeric: " + amount);
      }
      if (seenDot) {
        scale++;
      }
    }
    if (scale != 0) {
      throw new IllegalArgumentException("amount must use scale 0: " + amount);
    }
  }
}
