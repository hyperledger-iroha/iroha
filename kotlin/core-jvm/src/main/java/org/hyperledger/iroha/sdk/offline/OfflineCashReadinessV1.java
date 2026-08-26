package org.hyperledger.iroha.sdk.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.jetbrains.annotations.NotNull;

/** Strict universal Offline Cash V1 readiness DTO decoded by the retained internal substrate. */
public final class OfflineCashReadinessV1 {
  private final boolean mandatory;
  private final String cashHandoffCapability;
  private final int requiredBridgeAbiVersion;
  private final int maximumHops;
  private final boolean ready;
  private final List<OfflineCashReadinessBlockerV1> blockers;

  private OfflineCashReadinessV1(
      final boolean mandatory,
      @NotNull final String cashHandoffCapability,
      final int requiredBridgeAbiVersion,
      final int maximumHops,
      final boolean ready,
      @NotNull final List<OfflineCashReadinessBlockerV1> blockers) {
    this.mandatory = mandatory;
    this.cashHandoffCapability =
        Objects.requireNonNull(cashHandoffCapability, "cashHandoffCapability");
    this.requiredBridgeAbiVersion = requiredBridgeAbiVersion;
    this.maximumHops = maximumHops;
    this.ready = ready;
    this.blockers =
        Collections.unmodifiableList(
            new ArrayList<>(Objects.requireNonNull(blockers, "blockers")));

    // Reuse the strict invariant implementation for every package-private validated factory call.
    final List<KagemushaRecursiveSpendProver.ReadinessBlocker> substrateBlockers =
        new ArrayList<>(this.blockers.size());
    for (final OfflineCashReadinessBlockerV1 blocker : this.blockers) {
      substrateBlockers.add(
          new KagemushaRecursiveSpendProver.ReadinessBlocker(
              blocker.getCode(), blocker.getMessage()));
    }
    new KagemushaRecursiveSpendProver.OfflineStatus(
        mandatory,
        this.cashHandoffCapability,
        requiredBridgeAbiVersion,
        maximumHops,
        ready,
        Collections.emptyList(),
        substrateBlockers);
  }

  @NotNull
  static OfflineCashReadinessV1 fromValidatedProjection(
      final boolean mandatory,
      @NotNull final String cashHandoffCapability,
      final int requiredBridgeAbiVersion,
      final int maximumHops,
      final boolean ready,
      @NotNull final List<OfflineCashReadinessBlockerV1> blockers) {
    return new OfflineCashReadinessV1(
        mandatory,
        cashHandoffCapability,
        requiredBridgeAbiVersion,
        maximumHops,
        ready,
        blockers);
  }

  public boolean getMandatory() {
    return mandatory;
  }

  @NotNull
  public String getCashHandoffCapability() {
    return cashHandoffCapability;
  }

  public int getRequiredBridgeAbiVersion() {
    return requiredBridgeAbiVersion;
  }

  public int getMaximumHops() {
    return maximumHops;
  }

  public boolean getReady() {
    return ready;
  }

  @NotNull
  public List<Object> getAssets() {
    return Collections.emptyList();
  }

  @NotNull
  public List<OfflineCashReadinessBlockerV1> getBlockers() {
    return blockers;
  }
}
