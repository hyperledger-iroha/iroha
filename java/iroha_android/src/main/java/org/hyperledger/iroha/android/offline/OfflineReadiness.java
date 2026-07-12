package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;

/** Readiness of the requested asset definition for Offline operations. */
public final class OfflineReadiness {
  private static final long U32_MAX = 0xffff_ffffL;
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final String assetDefinitionId;
  private final Long assetScale;
  private final BigInteger evaluatedBlockHeight;
  private final String evaluatedBlockHash;
  private final OfflineActiveTransferVerifier activeTransferVerifier;
  private final OfflineActiveTransferVerifier activeTopUpShieldVerifier;
  private final boolean ready;
  private final List<OfflineReadinessBlocker> blockers;

  public OfflineReadiness(
      final String assetDefinitionId,
      final Long assetScale,
      final BigInteger evaluatedBlockHeight,
      final String evaluatedBlockHash,
      final OfflineActiveTransferVerifier activeTransferVerifier,
      final OfflineActiveTransferVerifier activeTopUpShieldVerifier,
      final boolean ready,
      final List<OfflineReadinessBlocker> blockers) {
    this.assetDefinitionId = OfflineReadinessText.requireExact(assetDefinitionId, "assetDefinitionId");
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(this.assetDefinitionId)) {
      throw new IllegalArgumentException(
          "assetDefinitionId must be a canonical unprefixed Base58 asset definition id");
    }
    if (assetScale != null && (assetScale.longValue() < 0 || assetScale.longValue() > U32_MAX)) {
      throw new IllegalArgumentException("assetScale must fit in an unsigned 32-bit integer");
    }
    this.assetScale = assetScale;
    this.evaluatedBlockHeight = requireU64(evaluatedBlockHeight, "evaluatedBlockHeight");
    this.evaluatedBlockHash = requireLowercaseHash(evaluatedBlockHash, "evaluatedBlockHash");
    this.activeTransferVerifier = activeTransferVerifier;
    this.activeTopUpShieldVerifier = activeTopUpShieldVerifier;
    this.ready = ready;
    Objects.requireNonNull(blockers, "blockers");
    final ArrayList<OfflineReadinessBlocker> blockerCopy = new ArrayList<>(blockers.size());
    final Set<String> blockerCodes = new HashSet<>();
    for (final OfflineReadinessBlocker blocker : blockers) {
      final OfflineReadinessBlocker exact =
          Objects.requireNonNull(blocker, "blockers must not contain null");
      if (!blockerCodes.add(exact.code())) {
        throw new IllegalArgumentException("blockers must not repeat blocker codes");
      }
      blockerCopy.add(exact);
    }
    if (ready != blockerCopy.isEmpty()) {
      throw new IllegalArgumentException("ready must be true exactly when blockers is empty");
    }
    if (blockerCodes.contains("asset_scale_unavailable") != (assetScale == null)) {
      throw new IllegalArgumentException(
          "asset_scale_unavailable must be present exactly when assetScale is null");
    }
    if (blockerCodes.contains("asset_scale_unsupported")
        != (assetScale != null && assetScale.longValue() > 28)) {
      throw new IllegalArgumentException(
          "asset_scale_unsupported must be present exactly when assetScale exceeds 28");
    }
    if (blockerCodes.contains("transfer_verifier_unavailable")
        != (activeTransferVerifier == null)) {
      throw new IllegalArgumentException(
          "transfer_verifier_unavailable must be present exactly when no active verifier is reported");
    }
    if (blockerCodes.contains("topup_shield_verifier_unavailable")
        != (activeTopUpShieldVerifier == null)) {
      throw new IllegalArgumentException(
          "topup_shield_verifier_unavailable must be present exactly when no active top-up shield verifier is reported");
    }
    if (activeTransferVerifier != null
        && !activeTransferVerifier.isActiveAt(this.evaluatedBlockHeight)) {
      throw new IllegalArgumentException(
          "activeTransferVerifier must be active at evaluatedBlockHeight");
    }
    if (activeTopUpShieldVerifier != null
        && !activeTopUpShieldVerifier.isActiveAt(this.evaluatedBlockHeight)) {
      throw new IllegalArgumentException(
          "activeTopUpShieldVerifier must be active at evaluatedBlockHeight");
    }
    if (ready
        && (assetScale == null
            || assetScale.longValue() > 28
            || activeTransferVerifier == null
            || activeTopUpShieldVerifier == null)) {
      throw new IllegalArgumentException(
          "ready requires a supported asset scale, active transfer verifier, and active top-up shield verifier");
    }
    this.blockers = Collections.unmodifiableList(blockerCopy);
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  /** Authoritative u32 scale; values above 28 accompany asset_scale_unsupported. */
  public Long assetScale() {
    return assetScale;
  }

  public BigInteger evaluatedBlockHeight() {
    return evaluatedBlockHeight;
  }

  public String evaluatedBlockHash() {
    return evaluatedBlockHash;
  }

  public OfflineActiveTransferVerifier activeTransferVerifier() {
    return activeTransferVerifier;
  }

  /** Active public-to-confidential top-up shield verifier, without key material. */
  public OfflineActiveTransferVerifier activeTopUpShieldVerifier() {
    return activeTopUpShieldVerifier;
  }

  public boolean ready() {
    return ready;
  }

  public List<OfflineReadinessBlocker> blockers() {
    return blockers;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof OfflineReadiness)) {
      return false;
    }
    final OfflineReadiness that = (OfflineReadiness) other;
    return ready == that.ready
        && assetDefinitionId.equals(that.assetDefinitionId)
        && Objects.equals(assetScale, that.assetScale)
        && evaluatedBlockHeight.equals(that.evaluatedBlockHeight)
        && evaluatedBlockHash.equals(that.evaluatedBlockHash)
        && Objects.equals(activeTransferVerifier, that.activeTransferVerifier)
        && Objects.equals(activeTopUpShieldVerifier, that.activeTopUpShieldVerifier)
        && blockers.equals(that.blockers);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        assetDefinitionId,
        assetScale,
        evaluatedBlockHeight,
        evaluatedBlockHash,
        activeTransferVerifier,
        activeTopUpShieldVerifier,
        ready,
        blockers);
  }

  private static BigInteger requireU64(final BigInteger value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.signum() < 0 || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit in an unsigned 64-bit integer");
    }
    return value;
  }

  private static String requireLowercaseHash(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.length() != 64) {
      throw new IllegalArgumentException(field + " must be exact lowercase 32-byte hexadecimal");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalArgumentException(
            field + " must be exact lowercase 32-byte hexadecimal");
      }
    }
    return value;
  }
}
