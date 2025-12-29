package org.hyperledger.iroha.android.telemetry;

import java.util.Optional;

/**
 * Supplies sanitised network context snapshots for telemetry emission.
 *
 * <p>SDK consumers provide the platform-specific implementation (pulling from Android's
 * {@code ConnectivityManager}, for example). The default provider returns an empty snapshot so
 * telemetry emission becomes a no-op when apps opt out or the device cannot determine its current
 * network.
 */
@FunctionalInterface
public interface NetworkContextProvider {

  /**
   * Returns a snapshot of the current network context.
   *
   * @return {@link Optional#empty()} when network details are unavailable or should not be exported.
   */
  Optional<NetworkContext> snapshot();

  /** Convenience helper that disables network-context emission. */
  static NetworkContextProvider disabled() {
    return Optional::empty;
  }
}
