package org.hyperledger.iroha.android.offline;

/** JNI availability probe for offline receipt challenge signing bytes. */
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

  private OfflineReceiptChallenge() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }
}
