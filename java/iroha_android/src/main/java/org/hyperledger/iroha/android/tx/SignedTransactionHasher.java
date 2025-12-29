package org.hyperledger.iroha.android.tx;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;

/** Canonical hashing helpers for signed transactions. */
public final class SignedTransactionHasher {

  private SignedTransactionHasher() {}

  /** Computes the canonical BLAKE2b-256 hash bytes for the given signed transaction. */
  public static byte[] hash(final SignedTransaction transaction) {
    Objects.requireNonNull(transaction, "transaction");
    final byte[] canonicalBytes = canonicalBytes(transaction);
    return blake2bCanonical(canonicalBytes);
  }

  /** Computes the canonical BLAKE2b-256 hash as a lowercase hex string. */
  public static String hashHex(final SignedTransaction transaction) {
    return toHex(hash(transaction));
  }

  /** Computes the canonical hash for pre-encoded signed transaction bytes. */
  public static byte[] hashCanonicalBytes(final byte[] canonicalSignedTransaction) {
    Objects.requireNonNull(canonicalSignedTransaction, "canonicalSignedTransaction");
    return blake2bCanonical(canonicalSignedTransaction);
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
      return fallbackCanonicalBytes(transaction);
    }
  }

  private static byte[] fallbackCanonicalBytes(final SignedTransaction transaction) {
    // Preserve compatibility with pre-Norito datasets by hashing the original payload/signature tuple.
    final byte[] payload = transaction.encodedPayload();
    final byte[] signature = transaction.signature();
    final byte[] publicKey = transaction.publicKey();
    final byte[] bls = transaction.blsPublicKey().orElse(new byte[0]);
    final ByteBuffer buffer =
        ByteBuffer
            .allocate(
                Integer.BYTES * 4 + payload.length + signature.length + publicKey.length + bls.length)
            .order(ByteOrder.BIG_ENDIAN);
    buffer.putInt(payload.length).put(payload);
    buffer.putInt(signature.length).put(signature);
    buffer.putInt(publicKey.length).put(publicKey);
    buffer.putInt(bls.length).put(bls);
    return buffer.array();
  }

  private static byte[] blake2bCanonical(final byte[] canonical) {
    byte[] digest;
    try {
      final MessageDigest md = MessageDigest.getInstance("BLAKE2B-256");
      digest = md.digest(canonical);
    } catch (NoSuchAlgorithmException ex) {
      digest = Blake2b.digest(canonical);
    }
    digest[digest.length - 1] |= 1;
    return digest;
  }

  private static String toHex(final byte[] data) {
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (final byte b : data) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }
}
