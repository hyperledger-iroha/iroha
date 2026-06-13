package org.hyperledger.iroha.android.privacy;

import java.math.BigInteger;
import java.util.Collections;

/** Commitment derivation matching {@code derive_confidential_note_v2} in Rust. */
public final class ConfidentialNoteCommitment {
  private ConfidentialNoteCommitment() {}

  public static byte[] deriveFromOpening(final ConfidentialNoteOpening opening) {
    final BigInteger amount = ConfidentialNoteScalars.scalarFromU128(opening.amount());
    final BigInteger rho =
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.note_rho", Collections.singletonList(opening.rho()));
    final BigInteger ownerTag =
        ConfidentialNoteScalars.littleEndianScalar(opening.ownerTag(), "ownerTag");
    final BigInteger assetTag =
        ConfidentialNoteScalars.littleEndianScalar(
            ConfidentialNoteTags.deriveAssetTag(opening.asset()), "assetTag");
    return ConfidentialNoteScalars.scalarToLittleEndian(
        ConfidentialNoteScalars.poseidonPair(
            amount,
            ConfidentialNoteScalars.poseidonPair(
                rho, ConfidentialNoteScalars.poseidonPair(ownerTag, assetTag))));
  }
}
