package org.hyperledger.iroha.sdk.offline;

import java.util.Arrays;
import java.util.Objects;
import org.jetbrains.annotations.NotNull;

/** Safe public view of an applied top-up with opaque canonical anchor and proof bytes. */
public final class OfflineCashFinalizedTopUpV1 {
  private final byte[] anchorCanonical;
  private final byte[] finalityProofCanonical;
  private final long finalizedBlockHeight;
  private final long serverTimeMilliseconds;

  private OfflineCashFinalizedTopUpV1(
      @NotNull final byte[] anchorCanonical,
      @NotNull final byte[] finalityProofCanonical,
      final long finalizedBlockHeight,
      final long serverTimeMilliseconds) {
    this.anchorCanonical =
        new KagemushaRecursiveSpendProver.TopUpAnchorV4(
                Objects.requireNonNull(anchorCanonical, "anchorCanonical"))
            .noritoEncoded();
    this.finalityProofCanonical =
        new KagemushaRecursiveSpendProver.TopUpFinalityProof(
                Objects.requireNonNull(finalityProofCanonical, "finalityProofCanonical"))
            .noritoEncoded();
    this.finalizedBlockHeight = finalizedBlockHeight;
    this.serverTimeMilliseconds = serverTimeMilliseconds;
  }

  @NotNull
  static OfflineCashFinalizedTopUpV1 fromValidatedProjection(
      @NotNull final byte[] anchorCanonical,
      @NotNull final byte[] finalityProofCanonical,
      final long finalizedBlockHeight,
      final long serverTimeMilliseconds) {
    return new OfflineCashFinalizedTopUpV1(
        anchorCanonical,
        finalityProofCanonical,
        finalizedBlockHeight,
        serverTimeMilliseconds);
  }

  @NotNull
  public byte[] anchorCanonical() {
    return Arrays.copyOf(anchorCanonical, anchorCanonical.length);
  }

  @NotNull
  public byte[] finalityProofCanonical() {
    return Arrays.copyOf(finalityProofCanonical, finalityProofCanonical.length);
  }

  public long getFinalizedBlockHeight() {
    return finalizedBlockHeight;
  }

  public long getServerTimeMilliseconds() {
    return serverTimeMilliseconds;
  }
}
