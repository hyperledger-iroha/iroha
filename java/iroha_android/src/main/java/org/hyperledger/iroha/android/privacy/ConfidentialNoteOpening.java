package org.hyperledger.iroha.android.privacy;

import java.util.Objects;

/** Confidential-v2 note opening material used by commitment and nullifier derivation. */
public final class ConfidentialNoteOpening {
  private final byte[] rho;
  private final byte[] spendKey;
  private final byte[] ownerTag;
  private final String asset;
  private final String chainId;
  private final String amount;

  public ConfidentialNoteOpening(
      final byte[] rho,
      final byte[] spendKey,
      final byte[] ownerTag,
      final String asset,
      final String chainId,
      final String amount) {
    this.rho = ConfidentialNoteScalars.fixedBytes(rho, 32, "rho");
    this.spendKey = ConfidentialNoteScalars.copyNonEmpty(spendKey, "spendKey");
    this.ownerTag = ConfidentialNoteScalars.fixedScalar(ownerTag, "ownerTag");
    this.asset = ConfidentialNoteScalars.canonicalText(asset, "asset");
    this.chainId = ConfidentialNoteScalars.canonicalText(chainId, "chainId");
    this.amount = ConfidentialNoteScalars.canonicalU128(amount, "amount");
  }

  public static ConfidentialNoteOpening fromSpendKey(
      final byte[] rho,
      final byte[] spendKey,
      final String asset,
      final String chainId,
      final String amount) {
    return new ConfidentialNoteOpening(
        rho, spendKey, ConfidentialOwnerTag.deriveFromSpendKey(spendKey), asset, chainId, amount);
  }

  public byte[] rho() {
    return rho.clone();
  }

  public byte[] spendKey() {
    return spendKey.clone();
  }

  public byte[] ownerTag() {
    return ownerTag.clone();
  }

  public String asset() {
    return asset;
  }

  public String chainId() {
    return chainId;
  }

  public String amount() {
    return amount;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ConfidentialNoteOpening)) {
      return false;
    }
    final ConfidentialNoteOpening other = (ConfidentialNoteOpening) obj;
    return java.util.Arrays.equals(rho, other.rho)
        && java.util.Arrays.equals(spendKey, other.spendKey)
        && java.util.Arrays.equals(ownerTag, other.ownerTag)
        && asset.equals(other.asset)
        && chainId.equals(other.chainId)
        && amount.equals(other.amount);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        java.util.Arrays.hashCode(rho),
        java.util.Arrays.hashCode(spendKey),
        java.util.Arrays.hashCode(ownerTag),
        asset,
        chainId,
        amount);
  }
}
