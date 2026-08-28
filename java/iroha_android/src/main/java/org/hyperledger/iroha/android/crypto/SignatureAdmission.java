package org.hyperledger.iroha.android.crypto;

/** Fixed-width structural admission checks for protocol signatures. */
public final class SignatureAdmission {

  /** Canonical detached Ed25519 signature length. */
  public static final int ED25519_SIGNATURE_LENGTH = 64;

  /** Canonical detached ML-DSA-65 signature length. */
  public static final int ML_DSA_65_SIGNATURE_LENGTH = 3309;

  private SignatureAdmission() {}

  /** Returns {@code true} when {@code signature} has the canonical shape for {@code algorithm}. */
  public static boolean isValid(
      final SigningAlgorithm algorithm, final byte[] signature) {
    if (algorithm == SigningAlgorithm.ED25519) {
      return hasFixedNonzeroShape(signature, ED25519_SIGNATURE_LENGTH);
    }
    if (algorithm == SigningAlgorithm.ML_DSA) {
      return hasFixedNonzeroShape(signature, ML_DSA_65_SIGNATURE_LENGTH);
    }
    return signature != null;
  }

  /** Returns {@code true} when {@code signature} has the canonical shape for a curve id. */
  public static boolean isValidForCurveId(final int curveId, final byte[] signature) {
    if (curveId == 0x01) {
      return hasFixedNonzeroShape(signature, ED25519_SIGNATURE_LENGTH);
    }
    if (curveId == 0x02) {
      return hasFixedNonzeroShape(signature, ML_DSA_65_SIGNATURE_LENGTH);
    }
    return signature != null && signature.length != 0;
  }

  private static boolean hasFixedNonzeroShape(
      final byte[] signature, final int expectedLength) {
    if (signature == null || signature.length != expectedLength) {
      return false;
    }
    for (final byte value : signature) {
      if (value != 0) {
        return true;
      }
    }
    return false;
  }
}
