package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;

/** Key material used to authenticate {@link OfflineJournal} records. */
public final class OfflineJournalKey {

  private static final int KEY_LENGTH = 32;
  private final byte[] raw;

  public OfflineJournalKey(final byte[] rawBytes) throws OfflineJournalException {
    if (rawBytes == null || rawBytes.length != KEY_LENGTH) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.INVALID_KEY_LENGTH,
          "offline journal key must be exactly 32 bytes");
    }
    this.raw = Arrays.copyOf(rawBytes, KEY_LENGTH);
  }

  /** Derives a key from arbitrary seed material via SHA-256. */
  public static OfflineJournalKey derive(final byte[] seed) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      final byte[] output = digest.digest(seed == null ? new byte[0] : seed);
      return new OfflineJournalKey(output);
    } catch (final NoSuchAlgorithmException | OfflineJournalException ex) {
      throw new IllegalStateException("Failed to derive offline journal key", ex);
    }
  }

  /** Derives a key from a passphrase (UTF-8 encoded) */
  public static OfflineJournalKey deriveFromPassphrase(final char[] passphrase) {
    if (passphrase == null) {
      return derive(new byte[0]);
    }
    final byte[] utf8 = new String(passphrase).getBytes(StandardCharsets.UTF_8);
    return derive(utf8);
  }

  byte[] raw() {
    return Arrays.copyOf(raw, raw.length);
  }
}
