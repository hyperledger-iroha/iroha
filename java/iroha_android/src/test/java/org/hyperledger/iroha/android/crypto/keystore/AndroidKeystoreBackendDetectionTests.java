package org.hyperledger.iroha.android.crypto.keystore;

public final class AndroidKeystoreBackendDetectionTests {

  public static void main(final String[] args) {
    new AndroidKeystoreBackendDetectionTests().ensureDesktopRuntimeDoesNotExposeBackend();
    System.out.println("[IrohaAndroid] Android keystore detection tests passed.");
  }

  private void ensureDesktopRuntimeDoesNotExposeBackend() {
    assert AndroidKeystoreBackend.maybeCreate().isEmpty()
        : "AndroidKeystoreBackend should not instantiate outside Android runtimes";
  }
}
