package org.hyperledger.iroha.android.privacy;

import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;

/** Decrypts confidential-v2 note payload envelopes into validated note openings. */
public final class ConfidentialNoteDecryption {
  private ConfidentialNoteDecryption() {}

  public static ConfidentialNoteOpening decryptNote(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey,
      final NetworkId expectedNetworkId) {
    return ConfidentialNoteCrypto.decryptNote(
        encryptedPayload,
        recipientPrivateKey,
        spendKey,
        ConfidentialOwnerTag.deriveFromSpendKey(spendKey),
        expectedNetworkId);
  }

  public static ConfidentialNoteOpening decryptNoteWithOwnerTag(
      final ConfidentialEncryptedPayload encryptedPayload,
      final byte[] recipientPrivateKey,
      final byte[] spendKey,
      final byte[] expectedOwnerTag,
      final NetworkId expectedNetworkId) {
    return ConfidentialNoteCrypto.decryptNote(
        encryptedPayload, recipientPrivateKey, spendKey, expectedOwnerTag, expectedNetworkId);
  }
}
