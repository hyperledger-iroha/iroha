package org.hyperledger.iroha.android.client;

/** Public key metadata to which a private execution output is encrypted. */
public final class SoracloudUploadedModelEncryptionRecipient {
  private final long schemaVersion;
  private final String keyId;
  private final long keyVersion;
  private final String kem;
  private final String aead;
  private final String publicKeyBytesBase64;
  private final String publicKeyFingerprint;
  private final byte[] publicKeyBytes;

  public SoracloudUploadedModelEncryptionRecipient(
      final long schemaVersion,
      final String keyId,
      final long keyVersion,
      final String kem,
      final String aead,
      final String publicKeyBytesBase64,
      final String publicKeyFingerprint) {
    SoracloudPrivateModelValidation.requireSchemaVersion(schemaVersion, "schemaVersion");
    SoracloudPrivateModelValidation.requirePositiveU32(keyVersion, "keyVersion");
    if (!SoracloudPrivateModelValidation.X25519_HKDF_SHA256.equals(kem)) {
      throw new IllegalArgumentException("kem must equal X25519HkdfSha256");
    }
    if (!SoracloudPrivateModelValidation.AES_256_GCM.equals(aead)) {
      throw new IllegalArgumentException("aead must equal Aes256Gcm");
    }
    this.schemaVersion = schemaVersion;
    this.keyId = SoracloudPrivateModelValidation.requireCanonicalString(keyId, "keyId");
    this.keyVersion = keyVersion;
    this.kem = kem;
    this.aead = aead;
    this.publicKeyBytes =
        SoracloudPrivateModelValidation.decodeCanonicalX25519PublicKey(
            publicKeyBytesBase64, "publicKeyBytesBase64");
    this.publicKeyBytesBase64 = publicKeyBytesBase64;
    this.publicKeyFingerprint =
        SoracloudPrivateModelValidation.requireRecipientFingerprint(
            publicKeyFingerprint, this.publicKeyBytes, "publicKeyFingerprint");
  }

  public long schemaVersion() { return schemaVersion; }

  public String keyId() { return keyId; }

  public long keyVersion() { return keyVersion; }

  public String kem() { return kem; }

  public String aead() { return aead; }

  public String publicKeyBytesBase64() { return publicKeyBytesBase64; }

  /** Return a defensive copy of the decoded recipient public key. */
  public byte[] publicKeyBytes() { return publicKeyBytes.clone(); }

  public String publicKeyFingerprint() { return publicKeyFingerprint; }
}
