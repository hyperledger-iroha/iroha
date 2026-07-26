package org.hyperledger.iroha.android.privacy;

import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;

/** Decrypts confidential-v2 note payload envelopes into validated note openings. */
public final class ConfidentialNoteDecryption {
  private ConfidentialNoteDecryption() {}

  public static ConfidentialNoteOpening decryptNote(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey) {
    return decryptNote(encryptedPayload, recipientPrivateKey, spendKey, null);
  }

  public static ConfidentialNoteOpening decryptNote(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey,
      final String expectedChainId) {
    return ConfidentialNoteCrypto.decryptNote(
        encryptedPayload,
        recipientPrivateKey,
        spendKey,
        ConfidentialOwnerTag.deriveFromSpendKey(spendKey),
        expectedChainId);
  }

  public static ConfidentialNoteOpening decryptNoteWithOwnerTag(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey,
      final byte[] expectedOwnerTag) {
    return decryptNoteWithOwnerTag(
        encryptedPayload, recipientPrivateKey, spendKey, expectedOwnerTag, null);
  }

  public static ConfidentialNoteOpening decryptNoteWithOwnerTag(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey,
      final byte[] expectedOwnerTag,
      final String expectedChainId) {
    return ConfidentialNoteCrypto.decryptNote(
        encryptedPayload, recipientPrivateKey, spendKey, expectedOwnerTag, expectedChainId);
  }
}
