package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;

/** Native source-lineage FastPQ proof builder for offline-offline receipts. */
public final class OfflineSourceLineageFastpqProof {
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

  private OfflineSourceLineageFastpqProof() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static String generate(final String requestJson) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    final byte[] response = nativeGenerate(requestJson);
    return new String(response, StandardCharsets.UTF_8);
  }

  public static boolean verify(final String requestJson, final String artifactJson) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    return nativeVerify(requestJson, artifactJson);
  }

  private static native byte[] nativeGenerate(String requestJson);

  private static native boolean nativeVerify(String requestJson, String artifactJson);
}
