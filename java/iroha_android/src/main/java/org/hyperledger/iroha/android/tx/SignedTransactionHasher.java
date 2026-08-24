package org.hyperledger.iroha.android.tx;

import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;

/** Canonical hashing helpers for signed transactions. */
public final class SignedTransactionHasher {

  private SignedTransactionHasher() {}

  /** Computes the canonical BLAKE2b-256 hash bytes for the given signed transaction. */
  public static byte[] hash(final SignedTransaction transaction) {
    Objects.requireNonNull(transaction, "transaction");
    final byte[] canonicalBytes = canonicalBytes(transaction);
    return IrohaHash.prehash(canonicalBytes);
  }

  /** Computes the canonical BLAKE2b-256 hash as a lowercase hex string. */
  public static String hashHex(final SignedTransaction transaction) {
    return toHex(hash(transaction));
  }

  /**
   * Computes Iroha's canonical external transaction identity from one exact unsigned
   * {@code TransactionPayload}.
   *
   * <p>The authorization proof is deliberately absent from Iroha transaction identity. This
   * permits detached-signing clients to bind the eventual Torii {@code tx_hash_hex} before they
   * release a signature.
   */
  public static byte[] hashCanonicalTransactionPayload(final byte[] canonicalTransactionPayload) {
    Objects.requireNonNull(canonicalTransactionPayload, "canonicalTransactionPayload");
    final byte[] snapshot = canonicalTransactionPayload.clone();
    byte[] preimage = null;
    try {
      NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(snapshot);
      preimage = wrapExternalEntrypoint(snapshot);
      return IrohaHash.prehash(preimage);
    } catch (final NoritoException ex) {
      throw new IllegalArgumentException(
          "transaction payload bytes are not an exact canonical encoding", ex);
    } finally {
      Arrays.fill(snapshot, (byte) 0);
      if (preimage != null) {
        Arrays.fill(preimage, (byte) 0);
      }
    }
  }

  /** Computes the canonical external transaction identity as lowercase hexadecimal. */
  public static String hashCanonicalTransactionPayloadHex(
      final byte[] canonicalTransactionPayload) {
    return toHex(hashCanonicalTransactionPayload(canonicalTransactionPayload));
  }

  /**
   * Computes the canonical hash for exact canonical bare {@code SignedTransaction} bytes.
   *
   * <p>The input must not include the version byte or an entrypoint wrapper. It is decoded and
   * re-encoded before hashing so truncated, non-canonical, versioned, and double-wrapped inputs
   * fail closed.
   */
  public static byte[] hashCanonicalBytes(final byte[] canonicalBareSignedTransaction) {
    return IrohaHash.prehash(canonicalBytesFromBare(canonicalBareSignedTransaction));
  }

  /** Computes the canonical hash hex for exact canonical bare signed transaction bytes. */
  public static String hashCanonicalHex(final byte[] canonicalBareSignedTransaction) {
    return toHex(hashCanonicalBytes(canonicalBareSignedTransaction));
  }

  /**
   * Validates and wraps exact canonical bare {@code SignedTransaction} bytes as
   * {@code TransactionEntrypoint::External}.
   */
  public static byte[] canonicalBytesFromBare(final byte[] canonicalBareSignedTransaction) {
    Objects.requireNonNull(canonicalBareSignedTransaction, "canonicalBareSignedTransaction");
    final byte[] snapshot = Arrays.copyOf(
        canonicalBareSignedTransaction, canonicalBareSignedTransaction.length);
    try {
      final SignedTransaction decoded = SignedTransactionEncoder.decode(snapshot);
      final byte[] reencoded = SignedTransactionEncoder.encode(decoded);
      if (!Arrays.equals(snapshot, reencoded)) {
        throw new IllegalArgumentException(
            "signed transaction bytes are not the exact canonical bare encoding");
      }
      return wrapExternalEntrypoint(decoded.encodedPayload());
    } catch (NoritoException ex) {
      throw new IllegalArgumentException(
          "signed transaction bytes are not a valid canonical bare encoding", ex);
    }
  }

  /**
   * Returns the canonical Norito bytes for the signed transaction.
   *
   * <p>Iroha hashes the {@code TransactionEntrypoint::External} discriminant and the signed
   * transaction payload, excluding the authorization proof. The encoding is:
   * {@code u32_LE(0) + COMPACT_LEN(payload.length) + payload}.
   */
  public static byte[] canonicalBytes(final SignedTransaction transaction) {
    try {
      // Encoding validates the exact signed-transaction wire shape and the nested payload.
      SignedTransactionEncoder.encode(transaction);
      return wrapExternalEntrypoint(transaction.encodedPayload());
    } catch (NoritoException | IllegalArgumentException ex) {
      throw new IllegalStateException("Failed to encode signed transaction", ex);
    }
  }

  private static byte[] wrapExternalEntrypoint(final byte[] canonicalTransactionPayload) {
    final byte[] lengthPrefix = encodeCompactLength(canonicalTransactionPayload.length);
    final byte[] result =
        new byte[4 + lengthPrefix.length + canonicalTransactionPayload.length];
    // u32 LE discriminant = 0 (External variant) — result[0..3] already zeroed
    System.arraycopy(lengthPrefix, 0, result, 4, lengthPrefix.length);
    System.arraycopy(
        canonicalTransactionPayload,
        0,
        result,
        4 + lengthPrefix.length,
        canonicalTransactionPayload.length);
    return result;
  }

  /** Encodes a canonical Norito {@code COMPACT_LEN} value using minimal unsigned LEB128. */
  static byte[] encodeCompactLength(final long value) {
    if (value < 0) {
      throw new IllegalArgumentException("compact length must be non-negative");
    }
    final byte[] output = new byte[10];
    long remaining = value;
    int count = 0;
    do {
      int next = (int) (remaining & 0x7F);
      remaining >>>= 7;
      if (remaining != 0) {
        next |= 0x80;
      }
      output[count++] = (byte) next;
    } while (remaining != 0);
    return java.util.Arrays.copyOf(output, count);
  }

  private static String toHex(final byte[] data) {
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (final byte b : data) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }
}
