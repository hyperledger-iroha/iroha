package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Shared canonical snapshot fixtures for Android attestation tests. */
public final class AndroidAttestationRevocationTestFixtures {

  /** SHA-256 of the canonical cross-language V1 vector used by Java, Kotlin, and Python. */
  public static final String CANONICAL_VECTOR_SHA256 =
      "154efc56abd2b7e403c5a362147971a561acc95f7274238ae0c52af196501b03";

  private AndroidAttestationRevocationTestFixtures() {}

  /** Builds and verifies a policy from one canonical committed test snapshot. */
  public static AndroidAttestationRevocationPolicyV1 policy(
      final long responseDateEpochMillis,
      final long cacheMaxAgeSeconds,
      final List<String> serials,
      final List<byte[]> tbsDigests) {
    final byte[] snapshot =
        canonicalSnapshot(
            responseDateEpochMillis, null, cacheMaxAgeSeconds, serials, tbsDigests);
    return AndroidAttestationRevocationPolicyV1.fromCanonicalSnapshot(snapshot, sha256(snapshot));
  }

  /** Encodes a canonical domain-separated V1 test snapshot. */
  public static byte[] canonicalSnapshot(
      final long responseDateEpochMillis,
      final Long lastModifiedEpochMillis,
      final long cacheMaxAgeSeconds,
      final List<String> serials,
      final List<byte[]> tbsDigests) {
    final List<String> canonicalSerials = new ArrayList<>(serials);
    Collections.sort(canonicalSerials);
    final List<String> canonicalTbs = new ArrayList<>();
    for (final byte[] digest : tbsDigests) {
      canonicalTbs.add(hex(digest));
    }
    Collections.sort(canonicalTbs);
    final StringBuilder builder = new StringBuilder();
    builder.append(AndroidAttestationRevocationPolicyV1.SNAPSHOT_DOMAIN).append('\n');
    builder.append("payload_sha256=").append(repeat("11", 32)).append('\n');
    builder.append("response_date_ms=").append(responseDateEpochMillis).append('\n');
    builder
        .append("last_modified_ms=")
        .append(lastModifiedEpochMillis == null ? "-" : lastModifiedEpochMillis)
        .append('\n');
    builder.append("cache_max_age_seconds=").append(cacheMaxAgeSeconds).append('\n');
    builder.append("serial_count=").append(canonicalSerials.size()).append('\n');
    for (final String serial : canonicalSerials) {
      builder.append("serial=").append(serial).append('\n');
    }
    builder.append("tbs_sha256_count=").append(canonicalTbs.size()).append('\n');
    for (final String digest : canonicalTbs) {
      builder.append("tbs_sha256=").append(digest).append('\n');
    }
    return builder.toString().getBytes(StandardCharsets.US_ASCII);
  }

  /** Computes SHA-256 for a canonical test snapshot. */
  public static byte[] sha256(final byte[] bytes) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(bytes);
    } catch (final Exception ex) {
      throw new AssertionError("SHA-256 unavailable", ex);
    }
  }

  /** Encodes bytes as lowercase hexadecimal. */
  public static String hex(final byte[] bytes) {
    final char[] alphabet = "0123456789abcdef".toCharArray();
    final char[] output = new char[bytes.length * 2];
    for (int index = 0; index < bytes.length; index++) {
      final int value = bytes[index] & 0xff;
      output[index * 2] = alphabet[value >>> 4];
      output[index * 2 + 1] = alphabet[value & 0x0f];
    }
    return new String(output);
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder builder = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      builder.append(value);
    }
    return builder.toString();
  }
}
