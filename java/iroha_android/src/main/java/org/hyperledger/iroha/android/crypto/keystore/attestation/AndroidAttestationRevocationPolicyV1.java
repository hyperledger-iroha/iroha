package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/**
 * Canonical, offline Android attestation certificate-status snapshot.
 *
 * <p>Construction requires the exact domain-separated V1 snapshot bytes and a SHA-256 commitment
 * obtained through a separate trusted governance channel. Every freshness field and both deny
 * lists are decoded from those committed bytes; callers cannot assemble them independently.
 */
public final class AndroidAttestationRevocationPolicyV1 {

  /** Exact first line of the canonical V1 snapshot. */
  public static final String SNAPSHOT_DOMAIN =
      "iroha.android.attestation.revocation.snapshot.v1";

  private static final int SHA256_BYTES = 32;
  private static final int MAX_SNAPSHOT_BYTES = 512 * 1024;
  private static final int MAX_SERIALS = 4096;
  private static final int MAX_SERIAL_HEX_LENGTH = 40;
  private static final int MAX_TBS_DIGESTS = 256;
  private static final long MAX_CACHE_AGE_SECONDS = 86400L;

  private final byte[] payloadSha256;
  private final long responseDateEpochMillis;
  private final Long lastModifiedEpochMillis;
  private final long cacheMaxAgeSeconds;
  private final List<String> nonValidCertificateSerials;
  private final List<byte[]> revokedCertificateTbsSha256;

  private AndroidAttestationRevocationPolicyV1(
      final byte[] canonicalSnapshot, final byte[] trustedSnapshotSha256) {
    Objects.requireNonNull(canonicalSnapshot, "canonicalSnapshot");
    Objects.requireNonNull(trustedSnapshotSha256, "trustedSnapshotSha256");
    if (canonicalSnapshot.length == 0 || canonicalSnapshot.length > MAX_SNAPSHOT_BYTES) {
      throw new IllegalArgumentException("Revocation snapshot size is outside the V1 bounds");
    }
    if (trustedSnapshotSha256.length != SHA256_BYTES || allZero(trustedSnapshotSha256)) {
      throw new IllegalArgumentException(
          "Trusted revocation snapshot SHA-256 must be a non-zero 32-byte digest");
    }
    if (!MessageDigest.isEqual(trustedSnapshotSha256, sha256(canonicalSnapshot))) {
      throw new IllegalArgumentException(
          "Revocation snapshot SHA-256 does not match the trusted governance commitment");
    }

    final DecodedSnapshot decoded = decodeCanonicalSnapshot(canonicalSnapshot);
    this.payloadSha256 = decoded.payloadSha256.clone();
    this.responseDateEpochMillis = decoded.responseDateEpochMillis;
    this.lastModifiedEpochMillis = decoded.lastModifiedEpochMillis;
    this.cacheMaxAgeSeconds = decoded.cacheMaxAgeSeconds;
    this.nonValidCertificateSerials =
        Collections.unmodifiableList(new ArrayList<>(decoded.nonValidCertificateSerials));
    final List<byte[]> digests = new ArrayList<>();
    for (final byte[] digest : decoded.revokedCertificateTbsSha256) {
      digests.add(digest.clone());
    }
    this.revokedCertificateTbsSha256 = Collections.unmodifiableList(digests);
    freshUntilEpochMillis();
  }

  /** Verifies and decodes one canonical V1 snapshot against a trusted commitment. */
  public static AndroidAttestationRevocationPolicyV1 fromCanonicalSnapshot(
      final byte[] canonicalSnapshot, final byte[] trustedSnapshotSha256) {
    return new AndroidAttestationRevocationPolicyV1(
        canonicalSnapshot.clone(), trustedSnapshotSha256.clone());
  }

  /** Fails unless {@code evaluationTimeEpochMillis} is in the half-open freshness window. */
  public void validateAt(final long evaluationTimeEpochMillis)
      throws AttestationVerificationException {
    final long freshUntil = freshUntilEpochMillis();
    if (evaluationTimeEpochMillis < responseDateEpochMillis) {
      throw new AttestationVerificationException(
          "Revocation status response date is in the future");
    }
    if (evaluationTimeEpochMillis >= freshUntil) {
      throw new AttestationVerificationException("Revocation status snapshot is stale");
    }
  }

  /** Returns true when either governed deny list rejects the certificate. */
  public boolean rejects(
      final BigInteger certificateSerial, final byte[] certificateTbsSha256) {
    Objects.requireNonNull(certificateSerial, "certificateSerial");
    Objects.requireNonNull(certificateTbsSha256, "certificateTbsSha256");
    if (Collections.binarySearch(
            nonValidCertificateSerials, certificateSerial.toString(16))
        >= 0) {
      return true;
    }
    for (final byte[] candidate : revokedCertificateTbsSha256) {
      if (MessageDigest.isEqual(candidate, certificateTbsSha256)) {
        return true;
      }
    }
    return false;
  }

  private long freshUntilEpochMillis() {
    try {
      return Math.addExact(
          responseDateEpochMillis, Math.multiplyExact(cacheMaxAgeSeconds, 1000L));
    } catch (final ArithmeticException ex) {
      throw new IllegalArgumentException(
          "Revocation freshness bound overflows epoch milliseconds", ex);
    }
  }

  private static DecodedSnapshot decodeCanonicalSnapshot(final byte[] bytes) {
    if (bytes[bytes.length - 1] != (byte) '\n') {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot must end with one newline");
    }
    for (final byte current : bytes) {
      final int value = current & 0xff;
      if (value != '\n' && (value < 0x20 || value > 0x7e)) {
        throw new IllegalArgumentException(
            "Canonical revocation snapshot must contain printable ASCII lines");
      }
    }
    final String text = new String(bytes, StandardCharsets.US_ASCII);
    final String[] lines = text.substring(0, text.length() - 1).split("\\n", -1);
    for (final String line : lines) {
      if (line.isEmpty()) {
        throw new IllegalArgumentException(
            "Canonical revocation snapshot contains an empty line");
      }
    }
    final Cursor cursor = new Cursor(lines);
    if (!SNAPSHOT_DOMAIN.equals(cursor.next("domain"))) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot domain/version is unsupported");
    }
    final byte[] payloadSha256 =
        parseDigest(
            exactValue(cursor.next("payload_sha256"), "payload_sha256"),
            "payload_sha256");
    final long responseDate =
        parsePositiveLong(
            exactValue(cursor.next("response_date_ms"), "response_date_ms"),
            "response_date_ms");
    if (responseDate % 1000L != 0L) {
      throw new IllegalArgumentException(
          "Revocation response date must be a whole-second epoch timestamp");
    }
    final String lastModifiedValue =
        exactValue(cursor.next("last_modified_ms"), "last_modified_ms");
    final Long lastModified =
        "-".equals(lastModifiedValue)
            ? null
            : parsePositiveLong(lastModifiedValue, "last_modified_ms");
    if (lastModified != null
        && (lastModified % 1000L != 0L || lastModified > responseDate)) {
      throw new IllegalArgumentException(
          "Revocation last-modified date is outside the canonical bounds");
    }
    final long cacheMaxAge =
        parsePositiveLong(
            exactValue(cursor.next("cache_max_age_seconds"), "cache_max_age_seconds"),
            "cache_max_age_seconds");
    if (cacheMaxAge < 1L || cacheMaxAge > MAX_CACHE_AGE_SECONDS) {
      throw new IllegalArgumentException(
          "Revocation cache max-age is outside the V1 bounds");
    }

    final int serialCount =
        parseCount(
            exactValue(cursor.next("serial_count"), "serial_count"),
            "serial_count",
            MAX_SERIALS);
    final List<String> serials = new ArrayList<>(serialCount);
    for (int index = 0; index < serialCount; index++) {
      final String serial = exactValue(cursor.next("serial"), "serial");
      if (!isCanonicalSerial(serial)) {
        throw new IllegalArgumentException(
            "Revocation certificate serial is not canonical lowercase hexadecimal: " + serial);
      }
      if (!serials.isEmpty() && serials.get(serials.size() - 1).compareTo(serial) >= 0) {
        throw new IllegalArgumentException(
            "Revocation certificate serials must be sorted and unique");
      }
      serials.add(serial);
    }

    final int tbsCount =
        parseCount(
            exactValue(cursor.next("tbs_sha256_count"), "tbs_sha256_count"),
            "tbs_sha256_count",
            MAX_TBS_DIGESTS);
    final List<byte[]> tbsDigests = new ArrayList<>(tbsCount);
    String previousTbs = null;
    for (int index = 0; index < tbsCount; index++) {
      final String encoded = exactValue(cursor.next("tbs_sha256"), "tbs_sha256");
      final byte[] digest = parseDigest(encoded, "tbs_sha256");
      if (previousTbs != null && previousTbs.compareTo(encoded) >= 0) {
        throw new IllegalArgumentException(
            "Revoked certificate TBS SHA-256 values must be sorted and unique");
      }
      previousTbs = encoded;
      tbsDigests.add(digest);
    }
    cursor.requireEnd();
    return new DecodedSnapshot(
        payloadSha256, responseDate, lastModified, cacheMaxAge, serials, tbsDigests);
  }

  private static String exactValue(final String line, final String key) {
    final String prefix = key + "=";
    if (!line.startsWith(prefix) || line.length() == prefix.length()) {
      throw new IllegalArgumentException("Canonical revocation snapshot expected " + key);
    }
    return line.substring(prefix.length());
  }

  private static long parsePositiveLong(final String value, final String label) {
    if (!isDecimal(value) || value.charAt(0) == '0') {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " is not a positive decimal integer");
    }
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " overflows", ex);
    }
  }

  private static int parseCount(final String value, final String label, final int maximum) {
    if (!isDecimal(value) || (value.length() > 1 && value.charAt(0) == '0')) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " is not a canonical decimal integer");
    }
    final int parsed;
    try {
      parsed = Integer.parseInt(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " overflows", ex);
    }
    if (parsed < 0 || parsed > maximum) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " is outside the V1 bounds");
    }
    return parsed;
  }

  private static boolean isDecimal(final String value) {
    if (value.isEmpty()) {
      return false;
    }
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) < '0' || value.charAt(index) > '9') {
        return false;
      }
    }
    return true;
  }

  private static byte[] parseDigest(final String value, final String label) {
    if (value.length() != SHA256_BYTES * 2) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " is not lowercase SHA-256");
    }
    final byte[] decoded = new byte[SHA256_BYTES];
    for (int index = 0; index < decoded.length; index++) {
      final char highCharacter = value.charAt(index * 2);
      final char lowCharacter = value.charAt(index * 2 + 1);
      final int high = Character.digit(highCharacter, 16);
      final int low = Character.digit(lowCharacter, 16);
      if (high < 0
          || low < 0
          || (highCharacter >= 'A' && highCharacter <= 'F')
          || (lowCharacter >= 'A' && lowCharacter <= 'F')) {
        throw new IllegalArgumentException(
            "Canonical revocation snapshot " + label + " is not lowercase SHA-256");
      }
      decoded[index] = (byte) ((high << 4) | low);
    }
    if (allZero(decoded)) {
      throw new IllegalArgumentException(
          "Canonical revocation snapshot " + label + " must not be all zero");
    }
    return decoded;
  }

  private static boolean allZero(final byte[] bytes) {
    int aggregate = 0;
    for (final byte value : bytes) {
      aggregate |= value;
    }
    return aggregate == 0;
  }

  private static boolean isCanonicalSerial(final String serial) {
    if (serial.isEmpty() || serial.length() > MAX_SERIAL_HEX_LENGTH) {
      return false;
    }
    if (serial.length() > 1 && serial.charAt(0) == '0') {
      return false;
    }
    for (int index = 0; index < serial.length(); index++) {
      final char character = serial.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        return false;
      }
    }
    return true;
  }

  private static byte[] sha256(final byte[] bytes) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(bytes);
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  private static final class Cursor {
    private final String[] lines;
    private int index;

    Cursor(final String[] lines) {
      this.lines = lines;
    }

    String next(final String label) {
      if (index >= lines.length) {
        throw new IllegalArgumentException(
            "Canonical revocation snapshot is missing " + label);
      }
      return lines[index++];
    }

    void requireEnd() {
      if (index != lines.length) {
        throw new IllegalArgumentException(
            "Canonical revocation snapshot contains trailing fields");
      }
    }
  }

  private static final class DecodedSnapshot {
    final byte[] payloadSha256;
    final long responseDateEpochMillis;
    final Long lastModifiedEpochMillis;
    final long cacheMaxAgeSeconds;
    final List<String> nonValidCertificateSerials;
    final List<byte[]> revokedCertificateTbsSha256;

    DecodedSnapshot(
        final byte[] payloadSha256,
        final long responseDateEpochMillis,
        final Long lastModifiedEpochMillis,
        final long cacheMaxAgeSeconds,
        final List<String> nonValidCertificateSerials,
        final List<byte[]> revokedCertificateTbsSha256) {
      this.payloadSha256 = payloadSha256;
      this.responseDateEpochMillis = responseDateEpochMillis;
      this.lastModifiedEpochMillis = lastModifiedEpochMillis;
      this.cacheMaxAgeSeconds = cacheMaxAgeSeconds;
      this.nonValidCertificateSerials = nonValidCertificateSerials;
      this.revokedCertificateTbsSha256 = revokedCertificateTbsSha256;
    }
  }
}
