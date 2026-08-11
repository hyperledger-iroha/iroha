package org.hyperledger.iroha.android.privacy;

/** Commitment derivation owned by the canonical native Rust V3 implementation. */
public final class ConfidentialNoteCommitment {
  private ConfidentialNoteCommitment() {}

  public static byte[] deriveFromOpening(final ConfidentialNoteOpening opening) {
    return PrivacyNativeBridge.deriveConfidentialNoteCommitmentV3(
        opening.asset(), opening.amount(), opening.rho(), opening.ownerTag());
  }
}
