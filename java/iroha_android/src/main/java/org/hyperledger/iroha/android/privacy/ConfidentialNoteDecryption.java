package org.hyperledger.iroha.android.privacy;

import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;

/** Fail-closed entry point reserved for the note plaintext contract once it is defined. */
public final class ConfidentialNoteDecryption {
  private ConfidentialNoteDecryption() {}

  public static ConfidentialNoteOpening decryptNote(
      final ConfidentialEncryptedPayload encryptedPayload, final byte[] recipientPrivateKey) {
    if (recipientPrivateKey == null || recipientPrivateKey.length != 32) {
      throw new IllegalArgumentException("recipientPrivateKey must be 32 bytes");
    }
    if (encryptedPayload == null) {
      throw new IllegalArgumentException("encryptedPayload must be provided");
    }
    if (encryptedPayload.version() != ConfidentialEncryptedPayload.VERSION_V1) {
      throw new IllegalArgumentException(
          "encryptedPayload version must be " + ConfidentialEncryptedPayload.VERSION_V1);
    }
    throw new UnsupportedOperationException(
        "confidential note plaintext layout is not defined by the node or bridge yet");
  }
}
