package org.hyperledger.iroha.android.offline;

public final class OfflineCashLifecycleTest {
  private OfflineCashLifecycleTest() {}

  public static void main(final String[] args) {
    configurationSnapshotRequiresCanonicalIssuerKey();
    configurationSnapshotRejectsMalformedNativeBridgeAbi();
    System.out.println("[IrohaAndroid] OfflineCashLifecycleTest passed.");
  }

  private static void configurationSnapshotRequiresCanonicalIssuerKey() {
    new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            "issuer-key",
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L)
        .requireUsableForOfflineExchange(999L, 7);

    final String[] rejectedIssuerKeys =
        new String[] {
          "",
          " ",
          " issuer-key",
          "issuer-key ",
          "issuer key",
          "issuer-key\n",
          "issuer-key\u2603"
        };
    for (final String issuerKey : rejectedIssuerKeys) {
      assertSnapshotFails(
          "missing_issuer_public_key",
          new OfflineCashLifecycle.ConfigurationSnapshot(
              "00000042",
              "pkr#sbp",
              true,
              issuerKey,
              7,
              "artifact-set",
              "kagemusha-recursive-compact-v1",
              100L,
              1_000L));
    }
  }

  private static void configurationSnapshotRejectsMalformedNativeBridgeAbi() {
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            "issuer-key",
            0,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L),
        7);
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            "issuer-key",
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L),
        0);
  }

  private static void assertSnapshotFails(
      final String expectedCode, final OfflineCashLifecycle.ConfigurationSnapshot snapshot) {
    assertSnapshotFails(expectedCode, snapshot, 7);
  }

  private static void assertSnapshotFails(
      final String expectedCode,
      final OfflineCashLifecycle.ConfigurationSnapshot snapshot,
      final Integer requiredNativeBridgeAbiVersion) {
    try {
      snapshot.requireUsableForOfflineExchange(200L, requiredNativeBridgeAbiVersion);
      throw new AssertionError("Expected ConfigurationSnapshotException");
    } catch (final OfflineCashLifecycle.ConfigurationSnapshotException error) {
      if (!expectedCode.equals(error.code())) {
        throw new AssertionError(
            "Expected " + expectedCode + " but got " + error.code(), error);
      }
    }
  }
}
