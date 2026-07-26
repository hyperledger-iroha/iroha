package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;

/** Exact on-chain fee sponsor program returned by Torii. */
public final class FeeSponsorProgramResponse {
  private final FeeSponsorProgramId id;
  private final FeeSponsorProgramLifecycle lifecycle;
  private final Long activeRevision;
  private final Long stagedRevision;
  private final FeeSponsorProgramActivation scheduledActivation;

  public FeeSponsorProgramResponse(
      final FeeSponsorProgramId id,
      final FeeSponsorProgramLifecycle lifecycle,
      final Long activeRevision,
      final Long stagedRevision,
      final FeeSponsorProgramActivation scheduledActivation) {
    this.id = Objects.requireNonNull(id, "id");
    this.lifecycle = Objects.requireNonNull(lifecycle, "lifecycle");
    if (activeRevision != null && activeRevision.longValue() <= 0L) {
      throw new IllegalArgumentException("activeRevision must be positive");
    }
    if (stagedRevision != null && stagedRevision.longValue() <= 0L) {
      throw new IllegalArgumentException("stagedRevision must be positive");
    }
    this.activeRevision = activeRevision;
    this.stagedRevision = stagedRevision;
    this.scheduledActivation = scheduledActivation;
  }

  public FeeSponsorProgramId id() {
    return id;
  }

  public FeeSponsorProgramLifecycle lifecycle() {
    return lifecycle;
  }

  public Long activeRevision() {
    return activeRevision;
  }

  public Long stagedRevision() {
    return stagedRevision;
  }

  public FeeSponsorProgramActivation scheduledActivation() {
    return scheduledActivation;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) return true;
    if (!(other instanceof FeeSponsorProgramResponse)) return false;
    final FeeSponsorProgramResponse that = (FeeSponsorProgramResponse) other;
    return id.equals(that.id)
        && lifecycle == that.lifecycle
        && Objects.equals(activeRevision, that.activeRevision)
        && Objects.equals(stagedRevision, that.stagedRevision)
        && Objects.equals(scheduledActivation, that.scheduledActivation);
  }

  @Override
  public int hashCode() {
    return Objects.hash(id, lifecycle, activeRevision, stagedRevision, scheduledActivation);
  }
}
