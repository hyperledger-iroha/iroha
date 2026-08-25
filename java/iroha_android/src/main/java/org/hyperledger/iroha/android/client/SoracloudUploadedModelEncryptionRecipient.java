package org.hyperledger.iroha.android.client;

import java.util.Base64;
import java.util.Objects;

/** Public key metadata to which a private execution output is encrypted. */
public final class SoracloudUploadedModelEncryptionRecipient {
  private final long schemaVersion;
  private final String keyId;
  private final long keyVersion;
  private final String kem;
  private final String aead;
  private final String publicKeyBytesBase64;
  private final String publicKeyFingerprint;

  public SoracloudUploadedModelEncryptionRecipient(
      final long schemaVersion,
      final String keyId,
      final long keyVersion,
      final String kem,
      final String aead,
      final String publicKeyBytesBase64,
      final String publicKeyFingerprint) {
    this.schemaVersion = schemaVersion;
    this.keyId = Objects.requireNonNull(keyId, "keyId");
    this.keyVersion = keyVersion;
    this.kem = Objects.requireNonNull(kem, "kem");
    this.aead = Objects.requireNonNull(aead, "aead");
    this.publicKeyBytesBase64 =
        Objects.requireNonNull(publicKeyBytesBase64, "publicKeyBytesBase64");
    this.publicKeyFingerprint =
        Objects.requireNonNull(publicKeyFingerprint, "publicKeyFingerprint");
  }

  public long schemaVersion() { return schemaVersion; }

  public String keyId() { return keyId; }

  public long keyVersion() { return keyVersion; }

  public String kem() { return kem; }

  public String aead() { return aead; }

  public String publicKeyBytesBase64() { return publicKeyBytesBase64; }

  /** Return a defensive copy of the decoded recipient public key. */
  public byte[] publicKeyBytes() { return Base64.getDecoder().decode(publicKeyBytesBase64); }

  public String publicKeyFingerprint() { return publicKeyFingerprint; }
}
