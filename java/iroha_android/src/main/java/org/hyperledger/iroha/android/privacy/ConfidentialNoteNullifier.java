package org.hyperledger.iroha.android.privacy;

/** Nullifier derivation owned by the canonical native Rust V3 implementation. */
public final class ConfidentialNoteNullifier {
  private ConfidentialNoteNullifier() {}

  public static byte[] deriveFromOpening(final ConfidentialNoteOpening opening) {
    return PrivacyNativeBridge.deriveConfidentialNullifierV3(
        opening.networkId(), opening.asset(), opening.spendKey(), opening.rho());
  }
}
