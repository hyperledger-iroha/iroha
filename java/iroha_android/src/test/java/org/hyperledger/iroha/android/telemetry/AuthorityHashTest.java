package org.hyperledger.iroha.android.telemetry;

import java.util.List;
import java.util.Map;
import java.util.Optional;

/**
 * Tests proving authority hashes stay deterministic across salt rotations and can be mapped back to
 * canonical Torii hosts by replaying the recorded rotation history.
 */
public final class AuthorityHashTest {

  private static final String ROTATION_Q1_SALT =
      "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
  private static final String ROTATION_Q2_SALT =
      "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";
  private static final String TORII_AUTHORITY = "torii.sora.net:8888";
  private static final String KAIGI_AUTHORITY = "kaigi.sora.net:9443";

  private AuthorityHashTest() {}

  public static void main(final String[] args) {
    matchesRecordedRotationDigests();
    mapsHashedAuthoritiesAcrossRotations();
    normalisesAuthorityInput();
    System.out.println("[IrohaAndroid] Authority hash tests passed.");
  }

  private static void matchesRecordedRotationDigests() {
    final TelemetryOptions.Redaction rotationQ1 =
        rotation(ROTATION_Q1_SALT, "2026Q1", "salt-q1");
    final TelemetryOptions.Redaction rotationQ2 =
        rotation(ROTATION_Q2_SALT, "2026Q2", "salt-q2");

    final String q1Digest =
        rotationQ1
            .hashAuthority(TORII_AUTHORITY)
            .orElseThrow(() -> new IllegalStateException("missing Q1 digest"));
    final String q2Digest =
        rotationQ2
            .hashAuthority(TORII_AUTHORITY)
            .orElseThrow(() -> new IllegalStateException("missing Q2 digest"));

    assert "35ddd4734874f598807a520c49c56887b5bb480823280c34ddfdcea43c58b539".equals(q1Digest)
        : "Q1 digest mismatch";
    assert "1c45cddb98306830ddcc3b29db505d3b947cbd8028bb94dd03fde8600b8e0d58".equals(q2Digest)
        : "Q2 digest mismatch";
    assert !q1Digest.equals(q2Digest) : "Salt rotation must change the digest output";
  }

  private static void mapsHashedAuthoritiesAcrossRotations() {
    final TelemetryOptions.Redaction rotationQ1 =
        rotation(ROTATION_Q1_SALT, "2026Q1", "salt-q1");
    final TelemetryOptions.Redaction rotationQ2 =
        rotation(ROTATION_Q2_SALT, "2026Q2", "salt-q2");
    final List<TelemetryOptions.Redaction> rotations = List.of(rotationQ1, rotationQ2);
    final List<String> authorities = List.of(TORII_AUTHORITY, KAIGI_AUTHORITY);

    final Map<String, String> q1Digests =
        Map.of(
            TORII_AUTHORITY,
            rotationQ1.hashAuthority(TORII_AUTHORITY).orElseThrow(),
            KAIGI_AUTHORITY,
            rotationQ1.hashAuthority(KAIGI_AUTHORITY).orElseThrow());
    final Map<String, String> q2Digests =
        Map.of(
            TORII_AUTHORITY,
            rotationQ2.hashAuthority(TORII_AUTHORITY).orElseThrow(),
            KAIGI_AUTHORITY,
            rotationQ2.hashAuthority(KAIGI_AUTHORITY).orElseThrow());

    for (final Map.Entry<String, String> entry : q1Digests.entrySet()) {
      final Optional<String> resolved =
          resolveAuthority(entry.getValue(), authorities, rotations);
      assert entry.getKey().equals(resolved.orElse(""))
          : "Q1 digest should map back to " + entry.getKey();
    }
    for (final Map.Entry<String, String> entry : q2Digests.entrySet()) {
      final Optional<String> resolved =
          resolveAuthority(entry.getValue(), authorities, rotations);
      assert entry.getKey().equals(resolved.orElse(""))
          : "Q2 digest should map back to " + entry.getKey();
    }
  }

  private static void normalisesAuthorityInput() {
    final TelemetryOptions.Redaction rotationQ1 =
        rotation(ROTATION_Q1_SALT, "2026Q1", "salt-q1");
    final String canonical = rotationQ1.hashAuthority(TORII_AUTHORITY).orElseThrow();
    final String padded =
        rotationQ1
            .hashAuthority("  ToRii.SoRa.NeT:8888  ")
            .orElseThrow(() -> new IllegalStateException("missing padded digest"));
    assert canonical.equals(padded) : "Authority hashing must normalise case and whitespace";
  }

  private static Optional<String> resolveAuthority(
      final String digest,
      final List<String> authorities,
      final List<TelemetryOptions.Redaction> rotations) {
    for (final String authority : authorities) {
      for (final TelemetryOptions.Redaction rotation : rotations) {
        final Optional<String> candidate = rotation.hashAuthority(authority);
        if (candidate.isPresent() && candidate.get().equals(digest)) {
          return Optional.of(authority);
        }
      }
    }
    return Optional.empty();
  }

  private static TelemetryOptions.Redaction rotation(
      final String saltHex, final String epoch, final String rotationId) {
    return TelemetryOptions.Redaction.builder()
        .setSaltHex(saltHex)
        .setSaltVersion(epoch)
        .setRotationId(rotationId)
        .build();
  }
}
