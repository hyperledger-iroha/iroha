package org.hyperledger.iroha.android.telemetry;

/**
 * Unit tests covering {@link TelemetryOptions} builders and hashing helpers.
 */
public final class TelemetryOptionsTests {

  private TelemetryOptionsTests() {}

  public static void main(final String[] args) {
    hashesAuthorityWithSalt();
    disabledRedactionSkipsHashing();
    builderRejectsMissingSalt();
    rotationDefaultsToSaltVersion();
    System.out.println("[IrohaAndroid] Telemetry options tests passed.");
  }

  private static void hashesAuthorityWithSalt() {
    final TelemetryOptions.Redaction redaction =
        TelemetryOptions.Redaction.builder()
            .setSaltHex("01020304")
            .setSaltVersion("2026Q1")
            .setRotationId("rot-7")
            .build();
    final String hash = redaction.hashAuthority("example.com").orElseThrow();
    assert "6eb717f2b14faf81c87763e4baaab75bdb82ffc8aab04e4d01131af126715dda".equals(hash)
        : "Unexpected hashed authority: " + hash;
    assert "rot-7".equals(redaction.rotationId()) : "rotation id mismatch";
  }

  private static void disabledRedactionSkipsHashing() {
    final TelemetryOptions options =
        TelemetryOptions.builder().setTelemetryRedaction(TelemetryOptions.Redaction.disabled()).build();
    assert !options.redaction().hashAuthority("example.com").isPresent()
        : "Disabled redaction must not hash authorities";
  }

  private static void builderRejectsMissingSalt() {
    boolean threw = false;
    try {
      TelemetryOptions.Redaction.builder().setSaltVersion("2026Q2").build();
    } catch (final IllegalStateException expected) {
      threw = true;
    }
    assert threw : "Builder must reject missing salt values";
  }

  private static void rotationDefaultsToSaltVersion() {
    final TelemetryOptions.Redaction redaction =
        TelemetryOptions.Redaction.builder()
            .setSaltHex("0a0b0c0d")
            .setSaltVersion("2026Q3")
            .build();
    assert "2026Q3".equals(redaction.rotationId())
        : "Rotation id should default to salt version when unspecified";
  }
}
