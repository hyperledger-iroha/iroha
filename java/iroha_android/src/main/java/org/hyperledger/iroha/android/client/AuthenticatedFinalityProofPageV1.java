package org.hyperledger.iroha.android.client;

/** Native-canonical, content-addressed page of contiguous bridge finality proofs. */
public final class AuthenticatedFinalityProofPageV1 {
  private final byte[] evidenceArchive;
  private final String hashHex;

  AuthenticatedFinalityProofPageV1(final byte[] evidenceArchive, final String hashHex) {
    if (evidenceArchive == null
        || evidenceArchive.length == 0
        || (long) evidenceArchive.length
            > AuthenticatedTransactionDetailsNativeBridge.FINALITY_PAGE_MAX_BYTES) {
      throw new IllegalArgumentException("evidenceArchive violates its closed byte bound");
    }
    requireHash(hashHex, "hashHex");
    this.evidenceArchive = evidenceArchive.clone();
    this.hashHex = hashHex;
  }

  /** Canonical Norito `{ version: 1, proofs: Vec<BridgeFinalityProof> }` archive. */
  public byte[] evidenceArchive() { return evidenceArchive.clone(); }

  /** Marked Blake2b-256 Iroha hash of {@link #evidenceArchive()}. */
  public String hashHex() { return hashHex; }

  static void requireHash(final String value, final String field) {
    if (value == null
        || !value.matches("[0-9a-f]{64}")
        || (Character.digit(value.charAt(63), 16) & 1) == 0) {
      throw new IllegalArgumentException(
          field + " must be an exact lowercase marked 32-byte Iroha hash");
    }
  }
}
