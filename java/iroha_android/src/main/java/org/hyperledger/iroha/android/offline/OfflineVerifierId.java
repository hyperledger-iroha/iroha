package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Stable registry identity of a verifier selected for Offline transfers. */
public final class OfflineVerifierId {
  private final String backend;
  private final String name;

  public OfflineVerifierId(final String backend, final String name) {
    this.backend = OfflineReadinessText.requireBounded(backend, "backend", 256);
    this.name = OfflineReadinessText.requireBounded(name, "name", 256);
  }

  public String backend() {
    return backend;
  }

  public String name() {
    return name;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof OfflineVerifierId)) {
      return false;
    }
    final OfflineVerifierId that = (OfflineVerifierId) other;
    return backend.equals(that.backend) && name.equals(that.name);
  }

  @Override
  public int hashCode() {
    return Objects.hash(backend, name);
  }

}
