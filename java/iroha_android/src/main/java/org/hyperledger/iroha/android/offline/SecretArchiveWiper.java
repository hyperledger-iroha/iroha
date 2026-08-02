package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;
import java.util.function.Function;

/** Package-private ownership and zeroization helpers for Kagemusha secret archives. */
final class SecretArchiveWiper {
  private SecretArchiveWiper() {}

  interface DigestCopyObserver {
    void copied(byte[] copy);
  }

  interface OpeningDigestAction<T> {
    T run(byte[] spendKey, byte[] rho, byte[] diversifier);
  }

  static <T> T transferChangeOpeningOwnership(
      final KagemushaRecursiveSpendProver.NoteOpening changeOpening,
      final Function<KagemushaRecursiveSpendProver.NoteOpening, T> transfer) {
    KagemushaRecursiveSpendProver.NoteOpening locallyOwned = changeOpening;
    try {
      final T result = Objects.requireNonNull(transfer, "transfer").apply(locallyOwned);
      locallyOwned = null;
      return result;
    } finally {
      if (locallyOwned != null) locallyOwned.destroy();
    }
  }

  static void wipe(final byte[] archive) {
    if (archive != null) Arrays.fill(archive, (byte) 0);
  }

  static void wipeAll(final byte[][] archives) {
    if (archives == null) return;
    for (final byte[] archive : archives) wipe(archive);
  }

  static <T> T withOpeningDigests(
      final byte[] spendKey,
      final String spendKeyName,
      final byte[] rho,
      final String rhoName,
      final byte[] diversifier,
      final String diversifierName,
      final OpeningDigestAction<T> action) {
    return withOpeningDigests(
        spendKey,
        spendKeyName,
        rho,
        rhoName,
        diversifier,
        diversifierName,
        copy -> {},
        action);
  }

  static <T> T withOpeningDigests(
      final byte[] spendKey,
      final String spendKeyName,
      final byte[] rho,
      final String rhoName,
      final byte[] diversifier,
      final String diversifierName,
      final DigestCopyObserver observer,
      final OpeningDigestAction<T> action) {
    byte[] spendKeyCopy = null;
    byte[] rhoCopy = null;
    byte[] diversifierCopy = null;
    try {
      spendKeyCopy = requireDigest(spendKey, spendKeyName);
      Objects.requireNonNull(observer, "observer").copied(spendKeyCopy);
      rhoCopy = requireDigest(rho, rhoName);
      observer.copied(rhoCopy);
      diversifierCopy = requireDigest(diversifier, diversifierName);
      observer.copied(diversifierCopy);
      return Objects.requireNonNull(action, "action")
          .run(spendKeyCopy, rhoCopy, diversifierCopy);
    } finally {
      wipe(diversifierCopy);
      wipe(rhoCopy);
      wipe(spendKeyCopy);
    }
  }

  static final class ChangeOpeningOwner implements AutoCloseable {
    private KagemushaRecursiveSpendProver.NoteOpening opening;
    private boolean transferred;
    private boolean closed;

    ChangeOpeningOwner(final KagemushaRecursiveSpendProver.NoteOpening opening) {
      this.opening = opening;
    }

    synchronized KagemushaRecursiveSpendProver.NoteOpening take() {
      if (closed) throw new IllegalStateException("change-opening owner has been closed");
      if (transferred) {
        throw new IllegalStateException("change opening has already been transferred");
      }
      transferred = true;
      final KagemushaRecursiveSpendProver.NoteOpening ownedOpening = opening;
      opening = null;
      return ownedOpening;
    }

    @Override
    public synchronized void close() {
      if (closed) return;
      if (opening != null) {
        opening.destroy();
        opening = null;
      }
      closed = true;
    }
  }

  private static byte[] requireDigest(final byte[] value, final String name) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(name + " must contain exactly 32 bytes");
    }
    int accumulator = 0;
    for (final byte octet : value) {
      accumulator |= octet;
    }
    if (accumulator == 0) {
      throw new IllegalArgumentException(name + " must be non-zero");
    }
    return Arrays.copyOf(value, value.length);
  }
}
