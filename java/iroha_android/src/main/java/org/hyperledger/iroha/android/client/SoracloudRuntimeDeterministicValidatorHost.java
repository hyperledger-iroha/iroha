package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Exact active validator that attested a deterministic private execution receipt. */
public final class SoracloudRuntimeDeterministicValidatorHost {
  private final long laneId;
  private final String validatorAccountId;
  private final String peerId;

  public SoracloudRuntimeDeterministicValidatorHost(
      final long laneId, final String validatorAccountId, final String peerId) {
    this.laneId = laneId;
    this.validatorAccountId =
        Objects.requireNonNull(validatorAccountId, "validatorAccountId");
    this.peerId = Objects.requireNonNull(peerId, "peerId");
  }

  public long laneId() { return laneId; }

  public String validatorAccountId() { return validatorAccountId; }

  public String peerId() { return peerId; }
}
