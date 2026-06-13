package org.hyperledger.iroha.android.privacy;

import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;

/** Encrypts confidential-v2 note openings into {@code ConfidentialEncryptedPayload} envelopes. */
public final class ConfidentialNoteEncryption {
  private ConfidentialNoteEncryption() {}

  public static byte[] publicKeyFromPrivateKey(final byte[] privateKey) {
    return ConfidentialNoteCrypto.publicKeyFromPrivateKey(privateKey);
  }

  public static ConfidentialEncryptedPayload encryptNote(
      final ConfidentialNoteOpening opening, final byte[] recipientPublicKey) {
    return ConfidentialNoteCrypto.encryptNote(opening, recipientPublicKey);
  }

  public static ConfidentialEncryptedPayload encryptNote(
      final ConfidentialNoteOpening opening,
      final byte[] recipientPublicKey,
      final byte[] ephemeralPrivateKey,
      final byte[] nonce) {
    return ConfidentialNoteCrypto.encryptNote(
        opening, recipientPublicKey, ephemeralPrivateKey, nonce);
  }
}
