package org.hyperledger.iroha.android.privacy;

import java.math.BigInteger;
import java.util.Collections;

/** Nullifier derivation matching {@code derive_confidential_nullifier_v2} in Rust. */
public final class ConfidentialNoteNullifier {
  private ConfidentialNoteNullifier() {}

  public static byte[] deriveFromOpening(final ConfidentialNoteOpening opening) {
    final BigInteger spendScalar =
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.spend_scalar", Collections.singletonList(opening.spendKey()));
    final BigInteger rho =
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.note_rho", Collections.singletonList(opening.rho()));
    final BigInteger assetTag =
        ConfidentialNoteScalars.littleEndianScalar(
            ConfidentialNoteTags.deriveAssetTag(opening.asset()), "assetTag");
    final BigInteger chainTag =
        ConfidentialNoteScalars.littleEndianScalar(
            ConfidentialNoteTags.deriveChainTag(opening.chainId()), "chainTag");
    return ConfidentialNoteScalars.scalarToLittleEndian(
        ConfidentialNoteScalars.poseidonPair(
            spendScalar,
            ConfidentialNoteScalars.poseidonPair(
                rho, ConfidentialNoteScalars.poseidonPair(assetTag, chainTag))));
  }
}
