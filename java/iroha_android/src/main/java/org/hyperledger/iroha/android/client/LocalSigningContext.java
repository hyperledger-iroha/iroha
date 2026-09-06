package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;

/**
 * Immutable local context used to bind locally signed Torii requests and validate server-prepared
 * transaction drafts before signing.
 *
 * <p>The exact network identity is configured by the caller and is never inferred from a server
 * response.
 */
public final class LocalSigningContext {
  private final NetworkId networkId;

  public LocalSigningContext(final NetworkId networkId) {
    this.networkId = Objects.requireNonNull(networkId, "networkId");
  }

  /**
   * Returns the exact canonical genesis-hash identity required in locally signed requests or
   * drafts.
   */
  public NetworkId networkId() {
    return networkId;
  }

  @Override
  public boolean equals(final Object other) {
    return this == other
        || other instanceof LocalSigningContext
            && networkId.equals(((LocalSigningContext) other).networkId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(networkId);
  }
}
