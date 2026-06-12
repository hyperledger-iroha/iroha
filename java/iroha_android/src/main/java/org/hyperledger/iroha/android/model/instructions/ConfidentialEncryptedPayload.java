package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import java.util.Objects;

/** X25519/XChaCha20-Poly1305 encrypted note payload carried by {@code zk::Shield}. */
public final class ConfidentialEncryptedPayload {
  public static final int VERSION_V1 = 1;

  private final int version;
  private final byte[] ephemeralPublicKey;
  private final byte[] nonce;
  private final byte[] ciphertext;

  public ConfidentialEncryptedPayload(
      final byte[] ephemeralPublicKey, final byte[] nonce, final byte[] ciphertext) {
    this(VERSION_V1, ephemeralPublicKey, nonce, ciphertext);
  }

  public ConfidentialEncryptedPayload(
      final int version,
      final byte[] ephemeralPublicKey,
      final byte[] nonce,
      final byte[] ciphertext) {
    if (version != VERSION_V1) {
      throw new IllegalArgumentException("version must be " + VERSION_V1);
    }
    this.version = version;
    this.ephemeralPublicKey =
        ZkInstructionUtils.fixedBytes(ephemeralPublicKey, 32, "ephemeralPublicKey");
    if (ZkInstructionUtils.isAllZero(this.ephemeralPublicKey)) {
      throw new IllegalArgumentException("ephemeralPublicKey must not be all zero");
    }
    this.nonce = ZkInstructionUtils.fixedBytes(nonce, 24, "nonce");
    this.ciphertext = ZkInstructionUtils.copyNonEmpty(ciphertext, "ciphertext");
  }

  public int version() {
    return version;
  }

  public byte[] ephemeralPublicKey() {
    return ephemeralPublicKey.clone();
  }

  public byte[] nonce() {
    return nonce.clone();
  }

  public byte[] ciphertext() {
    return ciphertext.clone();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ConfidentialEncryptedPayload other)) {
      return false;
    }
    return version == other.version
        && Arrays.equals(ephemeralPublicKey, other.ephemeralPublicKey)
        && Arrays.equals(nonce, other.nonce)
        && Arrays.equals(ciphertext, other.ciphertext);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        version,
        Arrays.hashCode(ephemeralPublicKey),
        Arrays.hashCode(nonce),
        Arrays.hashCode(ciphertext));
  }
}
