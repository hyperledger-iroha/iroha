package org.hyperledger.iroha.android.tx;

import java.util.Objects;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.norito.NoritoException;
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

  /** Computes the canonical hash for pre-encoded signed transaction bytes. */
  public static byte[] hashCanonicalBytes(final byte[] canonicalSignedTransaction) {
    Objects.requireNonNull(canonicalSignedTransaction, "canonicalSignedTransaction");
    return IrohaHash.prehash(canonicalSignedTransaction);
  }

  /** Computes the canonical hash hex for pre-encoded signed transaction bytes. */
  public static String hashCanonicalHex(final byte[] canonicalSignedTransaction) {
    return toHex(hashCanonicalBytes(canonicalSignedTransaction));
  }

  /** Returns the canonical Norito bytes for the signed transaction. */
  public static byte[] canonicalBytes(final SignedTransaction transaction) {
    try {
      return SignedTransactionEncoder.encode(transaction);
    } catch (NoritoException ex) {
      throw new IllegalStateException("Failed to encode signed transaction", ex);
    }
  }

  private static String toHex(final byte[] data) {
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (final byte b : data) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }
}
