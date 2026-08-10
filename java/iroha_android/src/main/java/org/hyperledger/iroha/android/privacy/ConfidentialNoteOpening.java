package org.hyperledger.iroha.android.privacy;

import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;

/** Confidential-v2 note opening material used by commitment and nullifier derivation. */
public final class ConfidentialNoteOpening {
  private final byte[] rho;
  private final byte[] spendKey;
  private final byte[] ownerTag;
  private final String asset;
  private final NetworkId networkId;
  private final String amount;

  public ConfidentialNoteOpening(
      final byte[] rho,
      final byte[] spendKey,
      final byte[] ownerTag,
      final String asset,
      final NetworkId networkId,
      final String amount) {
    this.rho = ConfidentialNoteScalars.fixedNonZeroBytes(rho, 32, "rho");
    this.spendKey = ConfidentialNoteScalars.fixedNonZeroBytes(spendKey, 32, "spendKey");
    this.ownerTag = ConfidentialNoteScalars.fixedScalar(ownerTag, "ownerTag");
    this.asset = ConfidentialNoteScalars.canonicalText(asset, "asset");
    this.networkId = Objects.requireNonNull(networkId, "networkId");
    this.amount = ConfidentialNoteScalars.canonicalU128(amount, "amount");
  }

  public static ConfidentialNoteOpening fromSpendKey(
      final byte[] rho,
      final byte[] spendKey,
      final String asset,
      final NetworkId networkId,
      final String amount) {
    return new ConfidentialNoteOpening(
        rho, spendKey, ConfidentialOwnerTag.deriveFromSpendKey(spendKey), asset, networkId, amount);
  }

  public static ConfidentialNoteOpening fromSpendKeyWithDiversifier(
      final byte[] rho,
      final byte[] spendKey,
      final byte[] diversifier,
      final String asset,
      final NetworkId networkId,
      final String amount) {
    return new ConfidentialNoteOpening(
        rho,
        spendKey,
        ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier),
        asset,
        networkId,
        amount);
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

  public NetworkId networkId() {
    return networkId;
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
        && networkId.equals(other.networkId)
        && amount.equals(other.amount);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        java.util.Arrays.hashCode(rho),
        java.util.Arrays.hashCode(spendKey),
        java.util.Arrays.hashCode(ownerTag),
        asset,
        networkId,
        amount);
  }
}
