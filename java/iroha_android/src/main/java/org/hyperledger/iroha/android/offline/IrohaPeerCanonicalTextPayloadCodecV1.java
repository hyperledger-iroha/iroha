package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/** Exact Offline Note UTF-8 application-byte boundary shared by QR, NFC, and Nearby. */
public final class IrohaPeerCanonicalTextPayloadCodecV1 {
  private IrohaPeerCanonicalTextPayloadCodecV1() {}

  public static int maximumCanonicalTextBytes(final IrohaPeerPayloadProfile profile) {
    return maximumCanonicalTextBytes(profile, IrohaPeerWireLimitsV1.PEER_V1);
  }

  public static int maximumCanonicalTextBytes(
      final IrohaPeerPayloadProfile profile, final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(profile, "profile");
    Objects.requireNonNull(limits, "limits");
    requireOfflineNote(profile);
    return Math.min(limits.maximumCanonicalBytes(), limits.maximumEncodedBytes(profile));
  }

  public static byte[] canonicalBytes(
      final String text, final IrohaPeerPayloadProfile profile) {
    return canonicalBytes(text, profile, IrohaPeerWireLimitsV1.PEER_V1);
  }

  public static byte[] canonicalBytes(
      final String text,
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(text, "text");
    requireOfflineNote(profile);
    require(!text.isEmpty(), "Peer canonical text must not be empty");
    final byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
    require(
        new String(bytes, StandardCharsets.UTF_8).equals(text),
        "Peer canonical text is not exact UTF-8");
    requireWithinBound(bytes.length, profile, limits);
    return bytes;
  }

  public static String canonicalText(
      final byte[] bytes, final IrohaPeerPayloadProfile profile) {
    return canonicalText(bytes, profile, IrohaPeerWireLimitsV1.PEER_V1);
  }

  public static String canonicalText(
      final byte[] bytes,
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(bytes, "bytes");
    requireOfflineNote(profile);
    require(bytes.length != 0, "Peer canonical text must not be empty");
    requireWithinBound(bytes.length, profile, limits);
    final String text = new String(bytes, StandardCharsets.UTF_8);
    require(
        Arrays.equals(text.getBytes(StandardCharsets.UTF_8), bytes),
        "Peer canonical payload is not exact UTF-8");
    return text;
  }

  private static void requireWithinBound(
      final int actual,
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerWireLimitsV1 limits) {
    final int maximum = maximumCanonicalTextBytes(profile, limits);
    require(
        actual <= maximum,
        "Peer canonical text for " + profile + " is " + actual
            + " bytes; maximum is " + maximum);
  }

  private static void requireOfflineNote(final IrohaPeerPayloadProfile profile) {
    require(
        Objects.requireNonNull(profile, "profile") == IrohaPeerPayloadProfile.OFFLINE_NOTE,
        "Peer generic canonical text is only supported for OFFLINE_NOTE");
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}
