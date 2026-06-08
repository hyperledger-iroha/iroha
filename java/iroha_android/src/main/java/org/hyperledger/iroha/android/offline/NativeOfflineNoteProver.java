package org.hyperledger.iroha.android.offline;

/** Native record-backed Offline Note recursive prover using a chain-supplied verifying key. */
public final class NativeOfflineNoteProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private NativeOfflineNoteProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] proveRedeem(final byte[] redeemNorito, final byte[] vkBoxNorito) {
    requireNonEmpty(redeemNorito, "redeemNorito");
    requireNonEmpty(vkBoxNorito, "vkBoxNorito");
    checkAvailable();
    final byte[] proofNorito = nativeProveNoteRedeemWithVk(redeemNorito, vkBoxNorito);
    if (proofNorito == null || proofNorito.length == 0) {
      throw new IllegalStateException("nativeProveNoteRedeemWithVk returned empty output");
    }
    return proofNorito;
  }

  public static byte[] proveAudit(final byte[] auditNorito, final byte[] vkBoxNorito) {
    requireNonEmpty(auditNorito, "auditNorito");
    requireNonEmpty(vkBoxNorito, "vkBoxNorito");
    checkAvailable();
    final byte[] proofNorito = nativeProveNoteAuditWithVk(auditNorito, vkBoxNorito);
    if (proofNorito == null || proofNorito.length == 0) {
      throw new IllegalStateException("nativeProveNoteAuditWithVk returned empty output");
    }
    return proofNorito;
  }

  public static boolean verifyRedeem(final byte[] redeemNorito, final byte[] vkBoxNorito) {
    requireNonEmpty(redeemNorito, "redeemNorito");
    requireNonEmpty(vkBoxNorito, "vkBoxNorito");
    checkAvailable();
    return nativeVerifyNoteRedeemWithVk(redeemNorito, vkBoxNorito);
  }

  public static boolean verifyAudit(final byte[] auditNorito, final byte[] vkBoxNorito) {
    requireNonEmpty(auditNorito, "auditNorito");
    requireNonEmpty(vkBoxNorito, "vkBoxNorito");
    checkAvailable();
    return nativeVerifyNoteAuditWithVk(auditNorito, vkBoxNorito);
  }

  static boolean detectNativeAvailability(final Runnable loadLibrary, final Runnable probeSymbol) {
    try {
      loadLibrary.run();
      probeSymbol.run();
      return true;
    } catch (final IllegalArgumentException ignored) {
      return true;
    } catch (final UnsatisfiedLinkError | SecurityException ignored) {
      return false;
    }
  }

  private static boolean loadLibrary() {
    return detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        () -> nativeProveNoteRedeemWithVk(new byte[0], new byte[0]));
  }

  private static void checkAvailable() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static void requireNonEmpty(final byte[] value, final String name) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException(name + " must not be empty");
    }
  }

  private static native byte[] nativeProveNoteRedeemWithVk(
      byte[] redeemNorito, byte[] vkBoxNorito);

  private static native byte[] nativeProveNoteAuditWithVk(byte[] auditNorito, byte[] vkBoxNorito);

  private static native boolean nativeVerifyNoteRedeemWithVk(
      byte[] redeemNorito, byte[] vkBoxNorito);

  private static native boolean nativeVerifyNoteAuditWithVk(byte[] auditNorito, byte[] vkBoxNorito);
}
