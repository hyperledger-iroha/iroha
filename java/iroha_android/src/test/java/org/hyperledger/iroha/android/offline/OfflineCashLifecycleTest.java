package org.hyperledger.iroha.android.offline;

public final class OfflineCashLifecycleTest {
  private static final String ISSUER_PUBLIC_KEY_BASE64 =
      "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8";
  private static final String ISSUER_PUBLIC_KEY_BASE64URL =
      "__________________________________________8";
  private static final String SHORT_ISSUER_PUBLIC_KEY_BASE64 =
      "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urqw";
  private static final String LONG_ISSUER_PUBLIC_KEY_BASE64 =
      "zc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3N";

  private OfflineCashLifecycleTest() {}

  public static void main(final String[] args) {
    configurationSnapshotRequiresCanonicalIssuerKey();
    configurationSnapshotRejectsMalformedIdentityFields();
    configurationSnapshotRejectsMalformedTimeFields();
    configurationSnapshotRejectsMalformedNativeBridgeAbi();
    System.out.println("[IrohaAndroid] OfflineCashLifecycleTest passed.");
  }

  private static void configurationSnapshotRequiresCanonicalIssuerKey() {
    new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L)
        .requireUsableForOfflineExchange(999L, 7);
    new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64URL,
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
          " " + ISSUER_PUBLIC_KEY_BASE64,
          ISSUER_PUBLIC_KEY_BASE64 + " ",
          "not base64",
          "!!!!",
          ISSUER_PUBLIC_KEY_BASE64 + "=",
          SHORT_ISSUER_PUBLIC_KEY_BASE64,
          LONG_ISSUER_PUBLIC_KEY_BASE64,
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

  private static void configurationSnapshotRejectsMalformedIdentityFields() {
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042\n",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1\n",
            100L,
            1_000L));
  }

  private static void configurationSnapshotRejectsMalformedTimeFields() {
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            -1L,
            1_000L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            -1L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            100L));
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
            7,
            "artifact-set",
            "kagemusha-recursive-compact-v1",
            100L,
            1_000L),
        7,
        -1L);
  }

  private static void configurationSnapshotRejectsMalformedNativeBridgeAbi() {
    assertSnapshotFails(
        "malformed_snapshot",
        new OfflineCashLifecycle.ConfigurationSnapshot(
            "00000042",
            "pkr#sbp",
            true,
            ISSUER_PUBLIC_KEY_BASE64,
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
            ISSUER_PUBLIC_KEY_BASE64,
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
    assertSnapshotFails(expectedCode, snapshot, requiredNativeBridgeAbiVersion, 200L);
  }

  private static void assertSnapshotFails(
      final String expectedCode,
      final OfflineCashLifecycle.ConfigurationSnapshot snapshot,
      final Integer requiredNativeBridgeAbiVersion,
      final long nowMs) {
    try {
      snapshot.requireUsableForOfflineExchange(nowMs, requiredNativeBridgeAbiVersion);
      throw new AssertionError("Expected ConfigurationSnapshotException");
    } catch (final OfflineCashLifecycle.ConfigurationSnapshotException error) {
      if (!expectedCode.equals(error.code())) {
        throw new AssertionError(
            "Expected " + expectedCode + " but got " + error.code(), error);
      }
    }
  }
}
